//! What the overlay DLL and its host say: goverlay's tinyipc frame around the JSON messages of Wand's fork.

use model::{Cursor, Rect, Texture};
use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use serde_json::Value;

const KIND: &[u8] = b"json";
const TO_CLIENT: i32 = 1;
const MESSAGE_ID: i32 = 100;
// Four ints, then the kind's length and bytes, then the body's length.
const HEADER: usize = 16 + 4 + KIND.len() + 4;
/// More than any message the DLL sends; a bigger frame is not read.
pub const MAX_FRAME: usize = 64 * 1024;

/// tinyipc's frame: direction, the receiver's client id (the DLL drops a frame for an id it did not register), host
/// port 0, message id, then the kind and the body as length-prefixed strings.
pub fn pack(client: i32, body: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(HEADER + body.len());
    for n in [TO_CLIENT, client, 0, MESSAGE_ID, KIND.len() as i32] {
        out.extend(n.to_le_bytes());
    }
    out.extend(KIND);
    out.extend((body.len() as i32).to_le_bytes());
    out.extend(body.as_bytes());
    out
}

/// The sender's client id and the JSON body; `None` for another kind or lengths that do not add up.
pub fn unpack(frame: &[u8]) -> Option<(i32, &str)> {
    let int = |at: usize| Some(i32::from_le_bytes(frame.get(at..at + 4)?.try_into().ok()?));
    let client = int(4)?;
    let kind = usize::try_from(int(16)?).ok()?;
    if frame.get(20..20 + kind)? != KIND {
        return None;
    }
    let length = usize::try_from(int(20 + kind)?).ok()?;
    let body = frame.get(HEADER..HEADER.checked_add(length)?)?;
    Some((client, std::str::from_utf8(body).ok()?))
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ToDll<'a> {
    GlobalInitConfig {
        global_block_color: u32,
        global_block_hotkey: Hotkey,
    },
    OffscreenWindowInfo {
        window_id: u32,
        share_mem_name: &'a str,
        rect: Area,
        accept_input: bool,
        block_input: bool,
        transparent_test: bool,
        visible: bool,
        titlebar_height: u32,
    },
    OffscreenWindowFrameUpdate {
        window_id: u32,
        width: u32,
        height: u32,
        dirty_rect: Area,
        shared_handle: usize,
        /// The PCI device id as a decimal string, as the DLL reports its own.
        adapter_id: String,
    },
    OffscreenWindowCursorUpdate {
        window_id: u32,
        cursor: &'static str,
    },
    CloseOffscreenWindow {
        window_id: u32,
    },
    GlobalBlock {
        block: bool,
    },
}

/// The DLL's own hotkey for its global block. Key 0 is never down: our hotkey asks for the block.
#[derive(Serialize)]
pub struct Hotkey {
    name: &'static str,
    key: u32,
    ctrl: bool,
    alt: bool,
    shift: bool,
    on_keyup: bool,
}

#[derive(Serialize)]
pub struct Area {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

impl From<Rect> for Area {
    fn from(Rect { x, y, width, height }: Rect) -> Self {
        Self { x, y, width, height }
    }
}

impl<'a> ToDll<'a> {
    pub fn init() -> Self {
        let none = Hotkey { name: "overlay", key: 0, ctrl: false, alt: false, shift: false, on_keyup: false };
        Self::GlobalInitConfig { global_block_color: 0, global_block_hotkey: none }
    }

    /// A window that shows a texture; with `input` it takes the game's input over its rect.
    pub fn window(window_id: u32, share_mem_name: &'a str, rect: Rect, input: bool) -> Self {
        Self::OffscreenWindowInfo {
            window_id,
            share_mem_name,
            rect: rect.into(),
            accept_input: input,
            block_input: input,
            transparent_test: false,
            visible: true,
            titlebar_height: 0,
        }
    }

    pub fn frame(window_id: u32, frame: Texture) -> Self {
        let Texture { handle, width, height, adapter } = frame;
        let dirty_rect = Area { x: 0, y: 0, width, height };
        Self::OffscreenWindowFrameUpdate { window_id, width, height, dirty_rect, shared_handle: handle, adapter_id: adapter.to_string() }
    }

    /// The cursor by its Win32 name, as the DLL loads it.
    pub fn cursor(window_id: u32, cursor: Cursor) -> Self {
        let cursor = match cursor {
            Cursor::Arrow => "IDC_ARROW",
            Cursor::Text => "IDC_IBEAM",
            Cursor::Cross => "IDC_CROSS",
            Cursor::Hand => "IDC_HAND",
            Cursor::ResizeLeftRight => "IDC_SIZEWE",
            Cursor::ResizeUpDown => "IDC_SIZENS",
            Cursor::ResizeFalling => "IDC_SIZENWSE",
            Cursor::ResizeRising => "IDC_SIZENESW",
            Cursor::NotAllowed => "IDC_NO",
        };
        Self::OffscreenWindowCursorUpdate { window_id, cursor }
    }

    pub fn json(&self) -> String {
        serde_json::to_string(self).expect("plain data serializes")
    }
}

/// A message of the DLL this host acts on.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum FromDll {
    /// Its hook is in: on this adapter.
    #[serde(rename = "graphics-hook-info")]
    Hooked {
        #[serde(rename = "adapter_id", deserialize_with = "adapter")]
        adapter: u32,
    },
    /// The game's window has a new size.
    #[serde(rename = "graphics-window-info")]
    Resized { width: u32, height: u32 },
    /// A Win32 message to window `window_id`; `wparam` is sign-extended to 64 bits, unsigned.
    #[serde(rename = "offscreen-window-input")]
    Input { window_id: u32, msg: u32, wparam: u64, lparam: i64 },
    #[serde(rename = "global-block-state")]
    Blocked { block: bool },
}

/// The PCI device id, a decimal string or a number.
fn adapter<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u32, D::Error> {
    let id = Value::deserialize(deserializer)?;
    let id = id.as_u64().or_else(|| id.as_str()?.parse().ok());
    id.and_then(|id| u32::try_from(id).ok()).ok_or_else(|| D::Error::custom("not an adapter id"))
}

/// `None` for any other message, or one that does not read.
pub fn read(body: &str) -> Option<FromDll> {
    serde_json::from_str(body).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_carries_the_client_id_kind_and_body_as_tinyipc_lays_them_out() {
        let frame = pack(5, "{}");
        let ints: Vec<i32> = frame[..16].chunks(4).map(|n| i32::from_le_bytes(n.try_into().unwrap())).collect();
        assert_eq!(ints, [1, 5, 0, 100]);
        assert_eq!(&frame[16..28], b"\x04\0\0\0json\x02\0\0\0");
        assert_eq!(&frame[28..], b"{}");
        assert_eq!(unpack(&pack(7, r#"{"a":1}"#)), Some((7, r#"{"a":1}"#)));
    }

    #[test]
    fn a_frame_whose_lengths_do_not_add_up_or_of_another_kind_is_not_read() {
        let frame = pack(5, "{\"a\":1}");
        assert_eq!(unpack(&frame[..frame.len() - 1]), None);
        assert_eq!(unpack(&frame[..10]), None);
        let mut binary = frame.clone();
        binary[20..24].copy_from_slice(b"bin\0");
        assert_eq!(unpack(&binary), None);
        let mut huge = frame;
        huge[24..28].copy_from_slice(&i32::MAX.to_le_bytes());
        assert_eq!(unpack(&huge), None);
    }

    #[test]
    fn messages_to_the_dll_have_its_field_names() {
        let rect = Rect { x: 10, y: -2, width: 300, height: 200 };
        let texture = Texture { handle: 0x4000_2142, width: 300, height: 200, adapter: 10370 };
        let json: Value = serde_json::from_str(&ToDll::window(1, "o-ab-win-9-1", rect, false).json()).unwrap();
        assert_eq!(json["type"], "offscreen-window-info");
        assert_eq!(json["rect"], serde_json::json!({"x": 10, "y": -2, "width": 300, "height": 200}));
        assert_eq!((json["share_mem_name"].as_str(), json["accept_input"].as_bool()), (Some("o-ab-win-9-1"), Some(false)));
        let json: Value = serde_json::from_str(&ToDll::window(1, "m", rect, true).json()).unwrap();
        assert_eq!((json["accept_input"].as_bool(), json["block_input"].as_bool()), (Some(true), Some(true)));
        assert_eq!(ToDll::GlobalBlock { block: true }.json(), r#"{"type":"global-block","block":true}"#);
        let cursor = r#"{"type":"offscreen-window-cursor-update","window_id":1,"cursor":"IDC_HAND"}"#;
        assert_eq!(ToDll::cursor(1, Cursor::Hand).json(), cursor);
        let json: Value = serde_json::from_str(&ToDll::frame(1, texture).json()).unwrap();
        assert_eq!(json["type"], "offscreen-window-frame-update");
        assert_eq!((json["shared_handle"].as_u64(), json["adapter_id"].as_str()), (Some(0x4000_2142), Some("10370")));
        let json: Value = serde_json::from_str(&ToDll::init().json()).unwrap();
        assert_eq!((json["type"].as_str(), json["global_block_hotkey"]["key"].as_u64()), (Some("global-init-config"), Some(0)));
    }

    #[test]
    fn the_dll_s_hook_size_input_and_block_are_read_and_anything_else_is_not() {
        let hooked = r#"{"adapter_desc":"NVIDIA","adapter_id":"10370","api":"DirectX12","type":"graphics-hook-info"}"#;
        assert_eq!(read(hooked), Some(FromDll::Hooked { adapter: 10370 }));
        assert_eq!(read(r#"{"type":"graphics-hook-info","adapter_id":42}"#), Some(FromDll::Hooked { adapter: 42 }));
        let resized = r#"{"height":1041,"type":"graphics-window-info","width":1772,"window":197896,"windowed":true}"#;
        assert_eq!(read(resized), Some(FromDll::Resized { width: 1772, height: 1041 }));
        // As captured in RimWorld: a wheel turned down, a key let go.
        let wheel =
            r#"{"lparam":3276850,"modifiers":[],"msg":522,"type":"offscreen-window-input","window_id":1,"wparam":18446744073701687296}"#;
        assert_eq!(read(wheel), Some(FromDll::Input { window_id: 1, msg: 522, wparam: 18446744073701687296, lparam: 3276850 }));
        let up = r#"{"lparam":3221225473,"modifiers":[],"msg":257,"type":"offscreen-window-input","window_id":1,"wparam":27}"#;
        assert_eq!(read(up), Some(FromDll::Input { window_id: 1, msg: 257, wparam: 27, lparam: 3221225473 }));
        assert_eq!(read(r#"{"type":"global-block-state","block":true}"#), Some(FromDll::Blocked { block: true }));
        for other in [r#"{"type":"offscreen-window-input","msg":512}"#, r#"{"type":"graphics-hook-info","adapter_id":"x"}"#, "not json"] {
            assert_eq!(read(other), None, "{other}");
        }
    }
}
