use std::path::Path;
use std::sync::mpsc::{Sender, SyncSender};
use std::time::Duration;

use anyhow::{Result, bail};
use windows::Win32::Graphics::Direct3D11::ID3D11Device;

use crate::Frame;
use crate::reader::Reader;

/// The decoder thread: reports whether the file opened, then decodes
/// until the `Playback` is dropped.
pub fn run(file: &Path, device: &ID3D11Device, frames: SyncSender<Frame>, ready: Sender<Result<()>>) {
    let mut reader = match Reader::open(file, device) {
        Ok(reader) => reader,
        Err(e) => {
            let _ = ready.send(Err(e));
            return;
        }
    };
    if ready.send(Ok(())).is_err() {
        return;
    }
    if let Err(e) = decode(&mut reader, &frames) {
        log::warn!("video decoding stopped: {e:#}");
    }
}

fn decode(reader: &mut Reader, frames: &SyncSender<Frame>) -> Result<()> {
    let mut looped = Duration::ZERO;
    let mut in_pass = 0;
    loop {
        match reader.read()? {
            Some(mut frame) => {
                frame.at += looped;
                in_pass += 1;
                // Blocks while the channel is full; fails once the Playback is gone.
                if frames.send(frame).is_err() {
                    return Ok(());
                }
            }
            None if in_pass == 0 => bail!("the clip has no frames"),
            None => {
                looped += reader.duration();
                in_pass = 0;
                reader.rewind()?;
            }
        }
    }
}
