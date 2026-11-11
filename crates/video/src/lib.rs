//! Short MP4 clips decoded to NV12 textures on the window's D3D11 device. Data only, no UI. Windows only.
#![cfg(windows)]

mod decoder;
mod gpu;
mod playback;
mod reader;
mod runtime;

use std::time::Duration;

pub use gpu::Texture;
pub use playback::{Playback, Timed};
pub use windows::Win32::Graphics::Direct3D11::ID3D11Device;

/// One decoded picture.
pub struct Frame {
    /// NV12, the picture's own size; back to the decoder once the frame is dropped.
    pub texture: Texture,
    pub width: u32,
    pub height: u32,
    /// From the start of the playback; it keeps growing across loops.
    pub at: Duration,
}

impl Timed for Frame {
    fn at(&self) -> Duration {
        self.at
    }
}
