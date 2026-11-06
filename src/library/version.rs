use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

/// The catalog's `supportedVersions` hold the PE header TimeDateStamp of the game executable.
pub fn pe_timestamp(path: &Path) -> Option<u32> {
    let mut file = File::open(path).ok()?;
    let mut dos = [0u8; 64];
    file.read_exact(&mut dos).ok()?;
    if &dos[..2] != b"MZ" {
        return None;
    }
    let pe = u32::from_le_bytes(dos[60..64].try_into().ok()?);
    file.seek(SeekFrom::Start(pe.into())).ok()?;
    // Signature (4), Machine (2), NumberOfSections (2), TimeDateStamp (4).
    let mut head = [0u8; 12];
    file.read_exact(&mut head).ok()?;
    (&head[..4] == b"PE\0\0").then(|| u32::from_le_bytes([head[8], head[9], head[10], head[11]]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_timestamp_and_rejects_non_pe() {
        let mut image = vec![0u8; 0x90];
        image[..2].copy_from_slice(b"MZ");
        image[60..64].copy_from_slice(&0x80u32.to_le_bytes());
        image[0x80..0x84].copy_from_slice(b"PE\0\0");
        image[0x88..0x8C].copy_from_slice(&1_793_791_052u32.to_le_bytes());
        let dir = std::env::temp_dir().join(format!("impulse-pe-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("game.exe");
        std::fs::write(&exe, &image).unwrap();
        assert_eq!(pe_timestamp(&exe), Some(1_793_791_052));
        std::fs::write(&exe, b"not an image at all, just text padding to 64 bytes......").unwrap();
        assert_eq!(pe_timestamp(&exe), None);
        assert_eq!(pe_timestamp(&dir.join("missing.exe")), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
