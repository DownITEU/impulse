pub use windows_registry::{CLASSES_ROOT, CURRENT_USER, Key, LOCAL_MACHINE};

const KEY_WOW64_64KEY: u32 = 0x0100;
const KEY_WOW64_32KEY: u32 = 0x0200;

/// Launchers are 32-bit and write under `WOW6432Node`; packaged apps live in the 64-bit view.
#[derive(Clone, Copy)]
pub enum View {
    Wow32,
    Wow64,
}

pub fn open(root: &Key, path: &str, view: View) -> Option<Key> {
    let flag = match view {
        View::Wow32 => KEY_WOW64_32KEY,
        View::Wow64 => KEY_WOW64_64KEY,
    };
    root.options().read().access(flag).open(path).ok()
}

pub fn string(key: &Key, name: &str) -> Option<String> {
    key.get_string(name).ok().filter(|s| !s.is_empty())
}

pub fn value(root: &Key, path: &str, name: &str, view: View) -> Option<String> {
    string(&open(root, path, view)?, name)
}

/// Subkeys opened read-only in the same view as `key`.
pub fn children(key: &Key, view: View) -> Vec<(String, Key)> {
    let Ok(names) = key.keys() else {
        return Vec::new();
    };
    names.filter_map(|name| open(key, &name, view).map(|k| (name, k))).collect()
}
