//! Pins as the view draws them: the shown ones, grouped where they would overlap.

use model::{Pin, Point};
use rustc_hash::FxHashMap as HashMap;

use super::camera::{Camera, Pixel};
use crate::map::markers::{HEAD_ABOVE, HEAD_RADIUS};

// Pins closer than this many pixels share a marker.
const CELL: f64 = 44.;
pub const GROUP_RADIUS: f64 = 13.;
// Beyond the shape, so a small pin is easy to hit.
const SLOP: f64 = 2.;

#[derive(Clone, Copy)]
pub struct Marker {
    pub at: Point,
    /// Its pin; the first one when it stands for several.
    pub pin: usize,
    pub count: usize,
}

/// One marker per `CELL` of a view `span` pixels across; one per pin when `apart`.
/// Southern markers come last, so they are drawn over their northern neighbours.
pub fn markers(pins: &[Pin], shown: impl Fn(usize) -> bool, span: f64, apart: bool) -> Vec<Marker> {
    let mut cells = HashMap::<(i64, i64), usize>::default();
    let mut markers = Vec::<Marker>::new();
    for (index, pin) in pins.iter().enumerate().filter(|(index, _)| shown(*index)) {
        let cell = ((pin.at.x * span / CELL) as i64, (pin.at.y * span / CELL) as i64);
        match cells.get(&cell) {
            Some(&held) if !apart => {
                let marker = &mut markers[held];
                marker.count += 1;
                let share = 1. / marker.count as f64;
                marker.at.x += (pin.at.x - marker.at.x) * share;
                marker.at.y += (pin.at.y - marker.at.y) * share;
            }
            _ => {
                cells.insert(cell, markers.len());
                markers.push(Marker { at: pin.at, pin: index, count: 1 });
            }
        }
    }
    markers.sort_by(|a, b| a.at.y.total_cmp(&b.at.y));
    markers
}

/// The topmost marker under `pixel`: a lifted pin, then a group's disc or a pin's head.
/// `lifted` are drawn over the rest, topmost first, with their pin, point and size now.
pub fn hit(markers: &[Marker], lifted: &[(usize, Point, f64)], camera: &Camera, size: Pixel, pixel: Pixel) -> Option<Marker> {
    let on_head = |at, scale| on_head(camera.to_view(at, size), scale, pixel);
    if let Some(&(pin, at, _)) = lifted.iter().find(|&&(_, at, scale)| on_head(at, scale)) {
        return Some(Marker { at, pin, count: 1 });
    }
    markers.iter().rev().copied().find(|marker| {
        if marker.count > 1 {
            let (x, y) = camera.to_view(marker.at, size);
            (x - pixel.0).hypot(y - pixel.1) <= GROUP_RADIUS + SLOP
        } else {
            on_head(marker.at, 1.)
        }
    })
}

/// Whether `pixel` is on the head of a marker `scale` times its size with its tip on `tip`.
pub fn on_head(tip: Pixel, scale: f64, pixel: Pixel) -> bool {
    (tip.0 - pixel.0).hypot(tip.1 - HEAD_ABOVE * scale - pixel.1) <= HEAD_RADIUS * scale + SLOP
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pin(x: f64, y: f64) -> Pin {
        Pin {
            id: String::new(),
            at: Point { x, y },
            name: String::new(),
            description: String::new(),
            group: (0, 0),
            image: None,
            video: None,
            position: None,
            level: None,
        }
    }

    #[test]
    fn near_pins_share_a_marker_until_the_view_is_close() {
        let pins = [pin(0.5, 0.5), pin(0.501, 0.5), pin(0.9, 0.1), pin(0.502, 0.5)];
        let far = markers(&pins, |_| true, 1024., false);
        let counts: Vec<_> = far.iter().map(|marker| (marker.pin, marker.count)).collect();
        assert_eq!(counts, [(2, 1), (0, 3)]);
        assert!((far[1].at.x - 0.501).abs() < 1e-12);
        assert_eq!(markers(&pins, |_| true, 1024. * 256., false).len(), 4);
        assert_eq!(markers(&pins, |_| true, 1024., true).len(), 4);
        assert_eq!(markers(&pins, |index| pins[index].at.x > 0.8, 1024., false).len(), 1);
    }

    #[test]
    fn the_cursor_finds_the_marker_under_it() {
        let pins = [pin(0.5, 0.5), pin(0.6, 0.5)];
        let (camera, size) = (Camera { center: Point { x: 0.5, y: 0.5 }, span: 1000. }, (400., 400.));
        let markers = markers(&pins, |_| true, camera.span, false);
        // The second pin's tip is at (300, 200), its head 19.5 px above.
        assert_eq!(hit(&markers, &[], &camera, size, (303., 182.)).map(|marker| marker.pin), Some(1));
        assert!(hit(&markers, &[], &camera, size, (300., 215.)).is_none());
        assert!(hit(&markers, &[], &camera, size, (250., 200.)).is_none());
        // The selected pin is drawn over its neighbour, so it takes the click.
        assert_eq!(hit(&markers, &[(7, pins[1].at, 1.35)], &camera, size, (303., 182.)).map(|marker| marker.pin), Some(7));
    }
}
