use std::path::Path;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use anyhow::Context;
use windows::Win32::Graphics::Direct3D11::ID3D11Device;

use crate::Frame;

/// Frames decoded ahead of the UI: enough to ride out a slow frame, few
/// enough to keep 1080p-sized textures from piling up.
const AHEAD: usize = 3;

/// What a playback times: when each of its frames is due.
pub trait Timed {
    /// From the start of the playback.
    fn at(&self) -> Duration;
}

/// A looping clip being decoded on a thread of its own.
pub struct Playback<F = Frame> {
    frames: Receiver<F>,
    /// Taken off the channel too early; waits for its time.
    held: Option<F>,
}

impl Playback {
    /// Starts decoding `file` (MP4, H.264) on a thread of its own into NV12 textures on `device`, the window's.
    /// The clip loops. Errors when the file cannot be opened or decoded (no decoder on Windows N without the
    /// Media Feature Pack, not a video).
    pub fn start(file: &Path, device: &ID3D11Device) -> anyhow::Result<Playback> {
        use std::sync::mpsc;

        let (frames_tx, frames) = mpsc::sync_channel(AHEAD);
        let (ready_tx, ready) = mpsc::channel();
        let (file, device) = (file.to_owned(), device.clone());
        std::thread::Builder::new()
            .name("video".into())
            .spawn(move || crate::decoder::run(&file, &device, frames_tx, ready_tx))
            .context("cannot start the decoder thread")?;
        ready.recv().context("the decoder thread died")??;
        Ok(Playback::new(frames))
    }
}

impl<F: Timed> Playback<F> {
    fn new(frames: Receiver<F>) -> Self {
        Playback { frames, held: None }
    }

    /// The newest frame due at `elapsed` since the start that is newer than the last one returned; `None` when nothing new is due yet. Never blocks.
    pub fn frame(&mut self, elapsed: Duration) -> Option<F> {
        let mut newest = None;
        while let Some(next) = self.held.take().or_else(|| self.frames.try_recv().ok()) {
            if next.at() > elapsed {
                self.held = Some(next);
                break;
            }
            newest = Some(next);
        }
        newest
    }

    /// When the next frame is due, since the start; `None` while the decoder has none ready.
    pub fn next_due(&self) -> Option<Duration> {
        self.held.as_ref().map(Timed::at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::{SyncSender, sync_channel};

    struct Due(Duration);

    impl Timed for Due {
        fn at(&self) -> Duration {
            self.0
        }
    }

    fn playback(times: &[u64]) -> (Playback<Due>, SyncSender<Due>) {
        let (tx, rx) = sync_channel(8);
        for &ms in times {
            tx.send(Due(Duration::from_millis(ms))).unwrap();
        }
        (Playback::new(rx), tx)
    }

    fn at(frame: Option<Due>) -> Option<u128> {
        frame.map(|f| f.0.as_millis())
    }

    #[test]
    fn picks_the_newest_due_frame() {
        let (mut playback, _tx) = playback(&[0, 40, 80, 120]);
        assert_eq!(at(playback.frame(Duration::from_millis(100))), Some(80));
    }

    #[test]
    fn keeps_the_frame_not_due_yet() {
        let (mut playback, _tx) = playback(&[0, 40, 80, 120]);
        playback.frame(Duration::from_millis(100));
        assert_eq!(playback.next_due(), Some(Duration::from_millis(120)));
        assert_eq!(at(playback.frame(Duration::from_millis(110))), None);
        assert_eq!(at(playback.frame(Duration::from_millis(125))), Some(120));
        assert_eq!(at(playback.frame(Duration::from_millis(500))), None);
    }

    #[test]
    fn sees_frames_that_arrive_later() {
        let (mut playback, tx) = playback(&[]);
        assert_eq!(at(playback.frame(Duration::from_secs(1))), None);
        tx.send(Due(Duration::from_millis(10))).unwrap();
        assert_eq!(at(playback.frame(Duration::from_secs(1))), Some(10));
    }

    #[test]
    fn survives_a_finished_decoder() {
        let (mut playback, tx) = playback(&[0]);
        drop(tx);
        assert_eq!(at(playback.frame(Duration::from_secs(1))), Some(0));
        assert_eq!(at(playback.frame(Duration::from_secs(2))), None);
    }
}
