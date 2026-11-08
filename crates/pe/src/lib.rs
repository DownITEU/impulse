//! PE image headers: a game's version, an image's architecture, a DLL's exports.

use std::{
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::Path,
};

pub const X64: u16 = 0x8664;
pub const X86: u16 = 0x14c;

pub struct Header {
    pub machine: u16,
    /// The catalog's `supportedVersions` hold this for the game executable.
    pub timestamp: u32,
}

/// Reads only the headers: game executables can be gigabytes.
pub fn header(path: &Path) -> Option<Header> {
    let mut file = File::open(path).ok()?;
    let mut dos = [0u8; 64];
    file.read_exact(&mut dos).ok()?;
    if &dos[..2] != b"MZ" {
        return None;
    }
    file.seek(SeekFrom::Start(u32_at(&dos, 60)?.into()))
        .ok()?;
    // Signature (4), Machine (2), NumberOfSections (2), TimeDateStamp (4).
    let mut head = [0u8; 12];
    file.read_exact(&mut head).ok()?;
    (&head[..4] == b"PE\0\0").then(|| Header {
        machine: u16::from_le_bytes([head[4], head[5]]),
        timestamp: u32::from_le_bytes([head[8], head[9], head[10], head[11]]),
    })
}

/// The RVA of an exported function.
pub fn export(path: &Path, name: &str) -> Option<u32> {
    let image = fs::read(path).ok()?;
    let pe = u32_at(&image, 60)? as usize;
    let sections = u16_at(&image, pe + 6)? as usize;
    let optional = pe + 24;
    let directories = match u16_at(&image, optional)? {
        0x20b => optional + 112,
        _ => optional + 96,
    };
    let table = optional + u16_at(&image, pe + 20)? as usize;
    let offset = |rva: u32| {
        (0..sections).find_map(|n| {
            let at = table + 40 * n;
            let (size, va, raw) = (u32_at(&image, at + 8)?, u32_at(&image, at + 12)?, u32_at(&image, at + 20)?);
            let size = size.max(u32_at(&image, at + 16)?);
            (va..va.checked_add(size)?)
                .contains(&rva)
                .then(|| (rva - va + raw) as usize)
        })
    };
    let (directory, directory_size) = (u32_at(&image, directories)?, u32_at(&image, directories + 4)?);
    let exports = offset(directory)?;
    let names = offset(u32_at(&image, exports + 32)?)?;
    let ordinals = offset(u32_at(&image, exports + 36)?)?;
    let functions = offset(u32_at(&image, exports + 28)?)?;
    (0..u32_at(&image, exports + 24)? as usize).find_map(|n| {
        let at = offset(u32_at(&image, names + 4 * n)?)?;
        let end = at + image.get(at..)?.iter().position(|b| *b == 0)?;
        if &image[at..end] != name.as_bytes() {
            return None;
        }
        let ordinal = u16_at(&image, ordinals + 2 * n)? as usize;
        let rva = u32_at(&image, functions + 4 * ordinal)?;
        // An RVA inside the export directory is a forwarder string, not code.
        (!(directory..directory.saturating_add(directory_size)).contains(&rva)).then_some(rva)
    })
}

fn u16_at(buf: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(buf.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(buf: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(buf.get(at..at + 4)?.try_into().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_header_and_rejects_non_pe() {
        let mut image = vec![0u8; 0x90];
        image[..2].copy_from_slice(b"MZ");
        image[60..64].copy_from_slice(&0x80u32.to_le_bytes());
        image[0x80..0x84].copy_from_slice(b"PE\0\0");
        image[0x84..0x86].copy_from_slice(&X64.to_le_bytes());
        image[0x88..0x8C].copy_from_slice(&1_793_791_052u32.to_le_bytes());
        let dir = std::env::temp_dir().join(format!("impulse-pe-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("game.exe");
        fs::write(&exe, &image).unwrap();
        let header = header(&exe).unwrap();
        assert!(header.machine == X64 && header.timestamp == 1_793_791_052);
        assert_eq!(export(&exe, "Run"), None);
        fs::write(&exe, b"not an image at all, just text padding to 64 bytes......").unwrap();
        assert!(super::header(&exe).is_none());
        assert!(super::header(&dir.join("missing.exe")).is_none());
        fs::remove_dir_all(&dir).unwrap();
    }

    // Any system DLL exports this; the RVA must point inside the image.
    #[test]
    fn finds_an_export() {
        let dll = Path::new(r"C:\Windows\System32\kernel32.dll");
        let rva = export(dll, "LoadLibraryW").unwrap();
        assert!(rva > 0x1000 && rva < fs::metadata(dll).unwrap().len() as u32 * 2);
        assert_eq!(export(dll, "NoSuchExport"), None);
        assert_eq!(export(dll, "AcquireSRWLockExclusive"), None, "forwarded to ntdll");
    }
}
