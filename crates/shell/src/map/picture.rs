//! A pin's picture or clip in a box: cut to fill it, its edges trimmed.

use gpui_kit::{AnyElement, Div, ElementId, Img, ObjectFit, div, image_cache, prelude::*, px, retain_all};

// Pin pictures and clips are 16:9 screen captures, some with the capture's border along their edges.
pub const ASPECT: f32 = 16. / 9.;
// Of the width, cut from each edge.
const TRIM: f32 = 0.006;

/// `layers` (pictures, a clip over them) covering a box of `size`.
/// The pictures it decodes (a full size one is about 8 MB) are freed once no frame draws `pin`'s.
pub fn picture(pin: usize, layers: impl IntoIterator<Item = AnyElement>, (width, height): (f32, f32)) -> Div {
    let trim = width * TRIM;
    let layer = |content| div().absolute().top(px(-trim)).left(px(-trim)).w(px(width + trim * 2.)).h(px(height + trim * 2.)).child(content);
    let cached = image_cache(retain_all(("pin-pictures", pin))).size_full().children(layers.into_iter().map(layer));
    div().relative().flex_none().w(px(width)).h(px(height)).overflow_hidden().child(cached)
}

/// A picture filling its layer; the id keeps its loading state, so it shows its placeholder while it loads.
pub fn fill(image: Img, id: impl Into<ElementId>) -> AnyElement {
    image.id(id).size_full().object_fit(ObjectFit::Cover).into_any_element()
}
