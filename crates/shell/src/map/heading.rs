//! Which way the player heads, from how they moved: the trainer tells where the player is, not where they look.
//! Lengths are on the map plane, which holds the game's world (8 to 17 km across on the maps seen), so
//! Wand's metre limits become parts of it.

use std::{
    f64::consts::PI,
    time::{Duration, Instant},
};

use model::Point;

// About a metre: a shorter move is not a step.
const STEP: f64 = 1e-4;
// About ten metres: a step weighs by its length up to this, and more the newer it is.
const REACH: f64 = 1e-3;
const STEPS: usize = 4;
// A sharper turn drops the older steps.
const TURN: f64 = 75. * PI / 180.;
/// A move this long and this fast (per second) is a teleport: nothing eases across it.
pub const JUMP: f64 = 1e-2;
const JUMP_SPEED: f64 = 2.5e-2;
// A bearing no step renewed for this long is the way the player last went: it stays, and the next step
// starts over.
const FRESH: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bearing {
    /// Clockwise from the plane's up, in radians.
    pub angle: f64,
    pub fresh: bool,
}

#[derive(Default)]
pub struct Heading {
    // Where the last step ended, and when the last fix came.
    from: Option<Point>,
    seen: Option<Instant>,
    // Bearings and weights of the last steps, oldest first.
    steps: Vec<(f64, f64)>,
    bearing: Option<(f64, Instant)>,
}

impl Heading {
    /// None until the player moves, and after a teleport.
    pub fn bearing(&self, now: Instant) -> Option<Bearing> {
        self.bearing.map(|(angle, at)| Bearing { angle, fresh: now.duration_since(at) <= FRESH })
    }

    pub fn track(&mut self, at: Point, now: Instant) {
        let seconds = self.seen.replace(now).map_or(0., |seen| now.duration_since(seen).as_secs_f64());
        let Some(from) = self.from.replace(at) else { return };
        // Fixes paused (no map was open): the move across the gap is no step.
        if seconds > FRESH.as_secs_f64() {
            return;
        }
        let (dx, dy) = (at.x - from.x, at.y - from.y);
        let length = dx.hypot(dy);
        if length < STEP {
            self.from = Some(from);
            return;
        }
        if length >= JUMP && length > JUMP_SPEED * seconds {
            (self.steps, self.bearing) = (Vec::new(), None);
            return;
        }
        // The plane's y grows downward.
        let step = dx.atan2(-dy);
        if self.bearing(now).is_none_or(|bearing| !bearing.fresh || arc(bearing.angle, step).abs() >= TURN) {
            self.steps.clear();
        }
        if self.steps.len() == STEPS {
            self.steps.remove(0);
        }
        self.steps.push((step, length.min(REACH)));
        let (x, y) = self.steps.iter().zip(1..).fold((0., 0.), |(x, y), (&(bearing, weight), age)| {
            let weight = weight * f64::from(age);
            (x + weight * bearing.sin(), y + weight * bearing.cos())
        });
        self.bearing = Some((x.atan2(y), now));
    }
}

/// The shortest turn from one bearing to another, -π to π.
pub fn arc(from: f64, to: f64) -> f64 {
    (to - from + PI).rem_euclid(2. * PI) - PI
}

#[cfg(test)]
mod tests {
    use super::*;

    const EAST: f64 = PI / 2.;

    fn walk(heading: &mut Heading, start: Instant, points: &[(f64, f64)]) -> Instant {
        let mut now = start;
        for &(x, y) in points {
            now += Duration::from_secs(1);
            heading.track(Point { x, y }, now);
        }
        now
    }

    fn near(got: Option<Bearing>, want: f64) -> bool {
        got.is_some_and(|got| got.fresh && arc(got.angle, want).abs() < 1e-6)
    }

    #[test]
    fn steps_give_the_way_the_player_goes() {
        let (mut heading, start) = (Heading::default(), Instant::now());
        let now = walk(&mut heading, start, &[(0.5, 0.5), (0.5005, 0.5), (0.501, 0.5)]);
        assert!(near(heading.bearing(now), EAST), "{:?}", heading.bearing(now));
        // North-east, a gentle turn: the earlier steps still count.
        let now = walk(&mut heading, now, &[(0.5015, 0.4995)]);
        let angle = heading.bearing(now).map(|bearing| bearing.angle);
        assert!(angle.is_some_and(|angle| angle > PI / 4. && angle < EAST), "{angle:?}");
    }

    #[test]
    fn tiny_moves_add_up_before_they_count() {
        let (mut heading, start) = (Heading::default(), Instant::now());
        let now = walk(&mut heading, start, &[(0.5, 0.5), (0.5, 0.49996)]);
        assert_eq!(heading.bearing(now), None);
        let now = walk(&mut heading, now, &[(0.5, 0.49992), (0.5, 0.49988)]);
        assert!(near(heading.bearing(now), 0.), "{:?}", heading.bearing(now));
    }

    #[test]
    fn a_sharp_turn_forgets_the_older_steps() {
        let (mut heading, start) = (Heading::default(), Instant::now());
        let now = walk(&mut heading, start, &[(0.5, 0.5), (0.5005, 0.5), (0.501, 0.5), (0.501, 0.5005)]);
        assert!(near(heading.bearing(now), PI), "{:?}", heading.bearing(now));
    }

    #[test]
    fn standing_still_keeps_the_last_way_and_a_new_walk_starts_over() {
        let (mut heading, start) = (Heading::default(), Instant::now());
        let now = walk(&mut heading, start, &[(0.5, 0.5), (0.5005, 0.5), (0.501, 0.5)]);
        let now = walk(&mut heading, now, &[(0.501, 0.5); 6]);
        let kept = heading.bearing(now);
        assert!(kept.is_some_and(|kept| !kept.fresh && arc(kept.angle, EAST).abs() < 1e-6), "{kept:?}");
        // West at once: the stale bearing does not weigh against it.
        let now = walk(&mut heading, now, &[(0.5005, 0.5)]);
        assert!(near(heading.bearing(now), -EAST), "{:?}", heading.bearing(now));
    }

    #[test]
    fn a_move_across_a_pause_in_the_fixes_is_no_step() {
        let (mut heading, start) = (Heading::default(), Instant::now());
        let now = walk(&mut heading, start, &[(0.5, 0.5), (0.5005, 0.5)]);
        let later = now + Duration::from_secs(60);
        heading.track(Point { x: 0.5005, y: 0.49 }, later);
        let kept = heading.bearing(later);
        assert!(kept.is_some_and(|kept| !kept.fresh && arc(kept.angle, EAST).abs() < 1e-6), "{kept:?}");
        // Steps count again from where the player was found.
        let now = walk(&mut heading, later, &[(0.5005, 0.4895)]);
        assert!(near(heading.bearing(now), 0.), "{:?}", heading.bearing(now));
    }

    #[test]
    fn a_teleport_ends_the_bearing() {
        let (mut heading, start) = (Heading::default(), Instant::now());
        let now = walk(&mut heading, start, &[(0.5, 0.5), (0.5005, 0.5)]);
        assert!(heading.bearing(now).is_some());
        let now = walk(&mut heading, now, &[(0.6, 0.5)]);
        assert_eq!(heading.bearing(now), None);
    }
}
