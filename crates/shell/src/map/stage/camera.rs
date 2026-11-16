//! Where the map is looked at from: plane points to view pixels and back.

use std::ops::RangeInclusive;

use model::Point;

/// A pixel of the view, from its top left.
pub type Pixel = (f64, f64);

// Seconds a glide takes to cover about two thirds of what is left.
const GLIDE: f64 = 0.07;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    /// The plane point in the middle of the view.
    pub center: Point,
    /// Pixels across the whole plane.
    pub span: f64,
}

impl Camera {
    pub fn to_view(self, at: Point, size: Pixel) -> Pixel {
        ((at.x - self.center.x) * self.span + size.0 / 2., (at.y - self.center.y) * self.span + size.1 / 2.)
    }

    pub fn to_plane(self, pixel: Pixel, size: Pixel) -> Point {
        Point { x: self.center.x + (pixel.0 - size.0 / 2.) / self.span, y: self.center.y + (pixel.1 - size.1 / 2.) / self.span }
    }

    pub fn pan(&mut self, by: Pixel) {
        self.center = Point { x: (self.center.x - by.0 / self.span).clamp(0., 1.), y: (self.center.y - by.1 / self.span).clamp(0., 1.) };
    }

    /// Keeps the view inside the box from `low` to `high`; a box smaller than the view sits in its middle.
    pub fn keep_in(&mut self, [low, high]: [Point; 2], size: Pixel) {
        // Not `clamp`: it panics on a box turned inside out.
        let keep = |at: f64, low: f64, high: f64, side: f64| {
            let half = side / 2. / self.span;
            if high - low <= half * 2. { (low + high) / 2. } else { at.max(low + half).min(high - half) }
        };
        self.center = Point { x: keep(self.center.x, low.x, high.x, size.0), y: keep(self.center.y, low.y, high.y, size.1) };
    }

    /// Scales by `factor` within `spans`; the plane point under `pixel` stays under it.
    pub fn zoom(&mut self, factor: f64, pixel: Pixel, size: Pixel, spans: &RangeInclusive<f64>) {
        let held = self.to_plane(pixel, size);
        self.span = (self.span * factor).clamp(*spans.start(), *spans.end());
        let moved = self.to_view(held, size);
        self.pan((pixel.0 - moved.0, pixel.1 - moved.1));
    }

    /// The closest view that shows the box between `low` and `high` whole.
    pub fn fit(low: Point, high: Point, size: Pixel, spans: &RangeInclusive<f64>) -> Self {
        let (width, height) = ((high.x - low.x).max(1e-9), (high.y - low.y).max(1e-9));
        Self {
            center: Point { x: (low.x + high.x) / 2., y: (low.y + high.y) / 2. },
            span: (size.0 / width).min(size.1 / height).clamp(*spans.start(), *spans.end()),
        }
    }

    /// The tile level whose tiles are drawn closest to their own size; `tile` is a tile's side in view pixels.
    pub fn level(&self, tile: f64, levels: &RangeInclusive<u8>) -> u8 {
        let level = (self.span / tile).log2().round();
        level.clamp(*levels.start() as f64, *levels.end() as f64) as u8
    }

    /// The columns and rows of `level` the view shows.
    pub fn tiles(&self, level: u8, size: Pixel) -> (RangeInclusive<u32>, RangeInclusive<u32>) {
        let count = (1u32 << level) as f64;
        let (low, high) = (self.to_plane((0., 0.), size), self.to_plane(size, size));
        let index = |at: f64| (at.clamp(0., 1.) * count).min(count - 1.) as u32;
        (index(low.x)..=index(high.x), index(low.y)..=index(high.y))
    }

    /// A tile's left, top, right and bottom in the view, on whole pixels so neighbours meet.
    pub fn tile(&self, level: u8, column: u32, row: u32, size: Pixel) -> [f64; 4] {
        let count = (1u32 << level) as f64;
        let corner = |column: u32, row: u32| self.to_view(Point { x: column as f64 / count, y: row as f64 / count }, size);
        let ((left, top), (right, bottom)) = (corner(column, row), corner(column + 1, row + 1));
        [left.round(), top.round(), right.round(), bottom.round()]
    }
}

/// The smallest box holding `points`: its top left and bottom right.
pub fn extent(points: impl IntoIterator<Item = Point>) -> Option<(Point, Point)> {
    let mut points = points.into_iter();
    let first = points.next()?;
    Some(points.fold((first, first), |(low, high), at| {
        (Point { x: low.x.min(at.x), y: low.y.min(at.y) }, Point { x: high.x.max(at.x), y: high.y.max(at.y) })
    }))
}

/// A zoom on its way: the span it ends at, about the pixel that stays put.
#[derive(Clone, Copy)]
pub struct Glide {
    pub span: f64,
    pub pixel: Pixel,
}

impl Glide {
    /// Moves `camera` toward the end for `seconds`; true once it is there.
    pub fn step(self, camera: &mut Camera, seconds: f64, size: Pixel, spans: &RangeInclusive<f64>) -> bool {
        // The spans change with the window's scale.
        let left = (self.span.clamp(*spans.start(), *spans.end()) / camera.span).ln();
        let done = left.abs() < 1e-3;
        let part = if done { 1. } else { 1. - (-seconds / GLIDE).exp() };
        camera.zoom((left * part).exp(), self.pixel, size, spans);
        done
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZE: Pixel = (800., 600.);
    const SPANS: RangeInclusive<f64> = 256.0..=65536.0;

    fn camera() -> Camera {
        Camera { center: Point { x: 0.5, y: 0.5 }, span: 1024. }
    }

    #[test]
    fn view_and_plane_are_inverse() {
        let camera = camera();
        assert_eq!(camera.to_view(Point { x: 0.5, y: 0.5 }, SIZE), (400., 300.));
        let at = camera.to_plane((100., 50.), SIZE);
        assert_eq!(camera.to_view(at, SIZE), (100., 50.));
    }

    #[test]
    fn zoom_keeps_the_point_under_the_cursor_and_stops_at_the_limits() {
        let mut camera = camera();
        let held = camera.to_plane((100., 50.), SIZE);
        camera.zoom(2., (100., 50.), SIZE, &SPANS);
        let (x, y) = camera.to_view(held, SIZE);
        assert!((x - 100.).abs() < 1e-9 && (y - 50.).abs() < 1e-9 && camera.span == 2048.);
        camera.zoom(1e9, (0., 0.), SIZE, &SPANS);
        assert_eq!(camera.span, 65536.);
    }

    #[test]
    fn a_glide_eases_to_its_span_about_its_pixel() {
        let mut camera = camera();
        let held = camera.to_plane((100., 50.), SIZE);
        let glide = Glide { span: 4096., pixel: (100., 50.) };
        assert!(!glide.step(&mut camera, 0.07, SIZE, &SPANS));
        assert!(camera.span > 1024. && camera.span < 4096.);
        while !glide.step(&mut camera, 0.016, SIZE, &SPANS) {}
        let (x, y) = camera.to_view(held, SIZE);
        assert!((camera.span - 4096.).abs() < 1e-6 && (x - 100.).abs() < 1e-6 && (y - 50.).abs() < 1e-6);
    }

    #[test]
    fn fit_shows_the_whole_box() {
        let camera = Camera::fit(Point { x: 0.25, y: 0.5 }, Point { x: 0.75, y: 0.75 }, SIZE, &SPANS);
        assert_eq!((camera.center, camera.span), (Point { x: 0.5, y: 0.625 }, 1600.));
        let points = [Point { x: 0.75, y: 0.5 }, Point { x: 0.25, y: 0.75 }, Point { x: 0.5, y: 0.6 }];
        assert_eq!(extent(points), Some((Point { x: 0.25, y: 0.5 }, Point { x: 0.75, y: 0.75 })));
        assert_eq!(extent([]), None);
    }

    #[test]
    fn the_view_stays_inside_the_bounds() {
        let bounds = [Point { x: 0.25, y: 0.25 }, Point { x: 0.75, y: 0.75 }];
        // Wider than the view: an edge stops at the view's edge.
        let mut camera = Camera { center: Point { x: 0., y: 0.5 }, span: 4096. };
        camera.keep_in(bounds, SIZE);
        assert_eq!(camera.center, Point { x: 0.25 + 400. / 4096., y: 0.5 });
        // Smaller than the view: in its middle.
        let mut camera = Camera { center: Point { x: 0., y: 1. }, span: 1000. };
        camera.keep_in(bounds, SIZE);
        assert_eq!(camera.center, Point { x: 0.5, y: 0.5 });
    }

    #[test]
    fn tiles_cover_the_view_and_stay_in_the_pyramid() {
        let camera = camera();
        assert_eq!(camera.level(256., &(0..=8)), 2);
        assert_eq!(camera.level(256., &(3..=8)), 3);
        assert_eq!(camera.tiles(2, SIZE), (0..=3, 0..=3));
        assert_eq!(camera.tiles(0, SIZE), (0..=0, 0..=0));
        assert_eq!(camera.tile(2, 1, 1, SIZE), [144., 44., 400., 300.]);
        let corner = Camera { center: Point { x: 0., y: 1. }, span: 4096. };
        assert_eq!(corner.tiles(4, SIZE), (0..=1, 14..=15));
    }
}
