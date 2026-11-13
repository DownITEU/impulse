use std::{env, fs, path::PathBuf};

use aes::cipher::{BlockDecryptMut, KeyIvInit, block_padding::Pkcs7};
use serde_json::Value;

use super::App;

pub fn apps() -> Vec<App> {
    let Some(data) = env::var_os("ProgramData") else {
        return Vec::new();
    };
    let path = PathBuf::from(data).join(r"Rockstar Games\Launcher\titles.dat");
    let Some(titles) = fs::read(path).ok().and_then(|raw| decrypt(&raw)) else {
        return Vec::new();
    };
    let Ok(Value::Object(file)) = serde_json::from_slice::<Value>(&titles) else {
        return Vec::new();
    };
    let Some(Value::Array(list)) = file.get("tl") else {
        return Vec::new();
    };
    list.iter()
        .filter_map(|title| {
            let id = title["ti"].as_str().filter(|id| *id != "launcher")?;
            let location = title["il"].as_str().filter(|l| !l.is_empty())?;
            title["if"].as_i64().filter(|flags| *flags != 0)?;
            Some(App { store_id: format!("rockstar:{id}"), location: location.into() })
        })
        .collect()
}

// AES-256-CBC with an all-zero key; the first 16 bytes are the IV.
fn decrypt(raw: &[u8]) -> Option<Vec<u8>> {
    let (iv, data) = raw.split_at_checked(16)?;
    cbc::Decryptor::<aes::Aes256>::new_from_slices(&[0; 32], iv).ok()?.decrypt_padded_vec_mut::<Pkcs7>(data).ok()
}
