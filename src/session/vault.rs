use std::{fs, path::Path, ptr, slice};

use anyhow::{Context, Result};
use windows_sys::Win32::{
    Foundation::LocalFree,
    Security::Cryptography::{
        CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
    },
};

use super::Session;

/// None when nothing is stored or the file is unreadable (corrupt, another Windows user).
pub(super) fn load(path: &Path) -> Option<Session> {
    let plain = dpapi(&fs::read(path).ok()?, false)?;
    serde_json::from_slice(&plain).ok()
}

pub(super) fn save(path: &Path, session: &Session) -> Result<()> {
    let sealed = dpapi(&serde_json::to_vec(session)?, true).context("encrypt session")?;
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, sealed).context("write session")?;
    fs::rename(&tmp, path).context("write session")
}

// DPAPI user scope: only this Windows account can decrypt the file.
fn dpapi(data: &[u8], seal: bool) -> Option<Vec<u8>> {
    let input = CRYPT_INTEGER_BLOB {
        cbData: u32::try_from(data.len()).ok()?,
        pbData: data.as_ptr().cast_mut(),
    };
    let mut out = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: ptr::null_mut(),
    };
    let flags = CRYPTPROTECT_UI_FORBIDDEN;
    // SAFETY: `input` outlives the call; on success DPAPI allocates `out`, freed with LocalFree below.
    let ok = unsafe {
        if seal {
            CryptProtectData(
                &input,
                ptr::null(),
                ptr::null(),
                ptr::null(),
                ptr::null(),
                flags,
                &mut out,
            )
        } else {
            CryptUnprotectData(
                &input,
                ptr::null_mut(),
                ptr::null(),
                ptr::null(),
                ptr::null(),
                flags,
                &mut out,
            )
        }
    };
    if ok == 0 {
        return None;
    }
    // SAFETY: on success `out` points to `cbData` bytes owned by us.
    let bytes = unsafe { slice::from_raw_parts(out.pbData, out.cbData as usize) }.to_vec();
    unsafe { LocalFree(out.pbData.cast()) };
    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_is_encrypted() {
        let path =
            std::env::temp_dir().join(format!("impulse-vault-test-{}.bin", std::process::id()));
        let session = Session {
            access_token: "secret-access".to_owned(),
            refresh_token: "secret-refresh".to_owned(),
            expires_at: 1234,
        };
        save(&path, &session).unwrap();
        let raw = fs::read(&path).unwrap();
        assert!(!raw.windows(6).any(|w| w == b"secret"));
        let loaded = load(&path).unwrap();
        fs::remove_file(&path).unwrap();
        assert_eq!(
            (
                loaded.access_token.as_str(),
                loaded.refresh_token.as_str(),
                loaded.expires_at
            ),
            ("secret-access", "secret-refresh", 1234)
        );
        assert!(load(&path).is_none());
    }
}
