---
paths:
  - "crates/shell/src/map/**"
---

# shell map

The map modal, the minimap and their parts: `MapView`, `Board`, `Stage`, `Sidebar`, `Card`, `LiveMap`, `Tracker`.

## Surface
- `MapView`: the modal (`show`, `hide`, `set_run`, `set_live`, `set_docked`); emits `MapEvent::Close`.
- `Minimap`: a mini `Stage` over the live board, one per window; `hide(window)` lets go of its tiles.
- `LiveMap`, `LiveRun`, `Run`: the live game's map for the minimap and the run it follows.
- `Tracker`: one per live run whose trainer can place the player; `Page` makes it.
- `bind_keys`, `progress_dir`.

## Rules
- Each part is its own entity drawn with `cached`, observing the shared `Board`. The board notifies on what a person does, never per frame; the player's moves are `BoardEvent::Moved`, which only the stage hears.
- `Board::live` (the minimap's) never writes the user's state: `saves` is off and it adopts the big board's `Rc<Saved>`. Only the big board writes.
- Map state is written blocking, on each change, in order. A board saves before it swaps or closes a map (`pending`, `hide`); `forget` does not save. Map ids reach a file name only through `state::file`; a new `Saved` field is `#[serde(default)]`.
- `Tracker` asks the position only while a `Lease` is held, one question at a time. The board holds its lease (`_fixes`) only while open with a tracker and a map that has a `Frame`; dropping it stops the asking.
- A `Cached` snapshot is skipped when the board already holds a shared copy (`MapView::choose`, `live::load`).
- `Tiles` drops the least recently drawn, never what the last frame drew or a load just brought. Whatever leaves `Tiles` or `Images` goes to `window.drop_image` (`stale`), in the window it was drawn in: `hide(window)` when a window stops showing a part.
- `Stage::mini` only draws: no input, hover, chip or menu; its tile cache is small.
- Clips: `ClipView` downloads through `art::path`, decodes with `video` on the window's D3D11 device, paints with `paint_surface` (never the atlas), and starts again when the device changed. A clip no frame asked for stops.
- Pin links: `PIN_LINK` selects a pin; only `http://` and `https://` open in the browser; any other link, `wemod:` included, does nothing.
- A closed map forgets its pins after 3 minutes (`forget_later`), unless a trainer is live in its game; `show` cancels that, and `forget_later` drops a load on its way before forgetting.
- Paint allocates nothing per pin or label: per-map data is made once (`Shown::of`, `Images`), markers regroup per half zoom step, regions and labels off the view are skipped.
- Ask for frames only while something moves (`animate`); a size change asks for one more.
- Anything laid over the stage calls `occlude()`: pins hover only while the stage's own hitbox is hovered.

## Traps
- The tracker keeps the heading for the whole run on the leaseholder's `Frame`; a lease with another frame starts it over.
- Tiles are decoded with a 1 px border: the atlas filters across an image's edge and shows seams otherwise.
- Closed, a board shows the tracker's last fix, not a live one; its bearing goes stale.
- Hidden regions are saved by name, hidden groups by id.
