//! Short confirmation sounds for cheat controls. No audio files ship: the tones are built in memory
//! (`synth`), and a cue may play a WAV the user chose instead.

mod synth;

use std::{fs::File, io::Read, os::windows::ffi::OsStrExt, path::Path, ptr, sync::OnceLock};

use serde::{Deserialize, Serialize};
use windows_sys::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_FILENAME, SND_MEMORY, SND_NODEFAULT};

/// `PlaySound` loads the whole file and takes names of 256 characters at most, the NUL included.
const CLIP_MAX_BYTES: u64 = 1 << 20;
const PATH_MAX_UNITS: usize = 255;

/// The control change a sound confirms.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cue {
    ToggleOn,
    ToggleOff,
    Slider,
    Number,
    Button,
    Selection,
}

impl Cue {
    pub const ALL: [Cue; 6] = [Cue::ToggleOn, Cue::ToggleOff, Cue::Slider, Cue::Number, Cue::Button, Cue::Selection];
}

/// Starts `cue` without waiting for it, as the WAV `file` when that plays and else as its own tone; a cue
/// that starts while another plays replaces it. A file is opened inside the call.
pub fn play(cue: Cue, file: Option<&Path>) {
    if file.is_some_and(play_file) {
        return;
    }
    static CUES: OnceLock<[Vec<u8>; 6]> = OnceLock::new();
    let cues = CUES.get_or_init(|| Cue::ALL.map(synth::wav));
    let buffer = &cues[cue as usize];
    // SAFETY: the buffer lives in a static and never moves, so it outlives the async playback;
    // SND_MEMORY reads it as a complete WAV file and a null module is valid for it.
    unsafe {
        PlaySoundW(buffer.as_ptr().cast(), ptr::null_mut(), SND_MEMORY | SND_ASYNC | SND_NODEFAULT);
    }
}

/// Whether winmm took the file: it opens it before it returns, and a missing or unreadable one is refused.
fn play_file(file: &Path) -> bool {
    let name: Vec<u16> = file.as_os_str().encode_wide().chain([0]).collect();
    // SAFETY: `name` is NUL-terminated and, as the file is opened during the call, need not outlive it.
    unsafe { PlaySoundW(name.as_ptr(), ptr::null_mut(), SND_FILENAME | SND_ASYNC | SND_NODEFAULT) != 0 }
}

/// Whether `file` can stand in for a cue: a WAV with samples, of a size and a name that `PlaySound` takes.
/// Reads the file (blocking); whether its encoding plays is only known once it does.
pub fn is_clip(file: &Path) -> bool {
    if file.as_os_str().encode_wide().count() > PATH_MAX_UNITS {
        return false;
    }
    let Ok(open) = File::open(file) else { return false };
    let mut bytes = Vec::new();
    // One byte over the cap tells a larger file from one that fits.
    let read = open.take(CLIP_MAX_BYTES + 1).read_to_end(&mut bytes);
    read.is_ok() && bytes.len() as u64 <= CLIP_MAX_BYTES && has_samples(&bytes)
}

/// A RIFF/WAVE with a format chunk and a nonempty data chunk, each within the extent the RIFF header declares.
fn has_samples(bytes: &[u8]) -> bool {
    let Some(rest) = bytes.strip_prefix(b"RIFF") else { return false };
    let Some((size, rest)) = rest.split_first_chunk::<4>() else { return false };
    // The declared size counts what follows it; one past the file is a cut or forged header.
    let Some(mut chunks) = rest.get(..u32::from_le_bytes(*size) as usize).and_then(|riff| riff.strip_prefix(b"WAVE")) else {
        return false;
    };
    let (mut format, mut data) = (false, false);
    while let Some((head, rest)) = chunks.split_at_checked(8) {
        let size = u32::from_le_bytes([head[4], head[5], head[6], head[7]]) as usize;
        if rest.len() < size {
            break;
        }
        match &head[..4] {
            b"fmt " => format = size >= 16,
            b"data" => data = size > 0,
            _ => {}
        }
        // A chunk is padded to an even length.
        chunks = rest.get(size + (size & 1)..).unwrap_or_default();
    }
    format && data
}

#[cfg(test)]
mod tests {
    use super::*;

    fn written(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let file = std::env::temp_dir().join(format!("impulse-sound-test-{name}.wav"));
        std::fs::write(&file, bytes).unwrap();
        file
    }

    #[test]
    fn a_wav_is_a_clip_and_anything_else_is_not() {
        let good = written("good", &synth::wav(Cue::Button));
        let text = written("text", b"RIFF is only the start of this text file");
        let big = written("big", &[synth::wav(Cue::Button), vec![0; 2 << 20]].concat());
        let short = written("short", b"RIFF");
        let empty = written("empty", b"RIFF\x04\0\0\0WAVE");
        let mut no_samples = synth::wav(Cue::Button);
        no_samples.truncate(44);
        no_samples[40..44].fill(0);
        // The RIFF size covers the header only, so the empty data chunk is what refuses it.
        no_samples[4..8].copy_from_slice(&36u32.to_le_bytes());
        let no_samples = written("no-samples", &no_samples);
        assert!(is_clip(&good));
        assert!(!is_clip(&text) && !is_clip(&big) && !is_clip(&short) && !is_clip(&empty) && !is_clip(&no_samples));
        assert!(!is_clip(&good.with_file_name("impulse-sound-test-missing.wav")));
        // Chunks that lie past the declared RIFF size, or a size past the file, are no clip.
        let mut past = synth::wav(Cue::Button);
        past[4..8].copy_from_slice(&20u32.to_le_bytes());
        let past = written("past", &past);
        let mut long = synth::wav(Cue::Button);
        long[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        let long = written("long", &long);
        assert!(!is_clip(&past) && !is_clip(&long));
        for file in [good, text, big, short, empty, no_samples, past, long] {
            std::fs::remove_file(file).ok();
        }
    }

    #[test]
    fn a_cue_is_saved_by_name() {
        assert_eq!(serde_json::to_string(&Cue::ToggleOn).unwrap(), r#""toggle_on""#);
        assert_eq!(serde_json::from_str::<Cue>(r#""selection""#).unwrap(), Cue::Selection);
    }
}
