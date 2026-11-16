//! A new build of the panel embeds again: cargo watches `web/panel/dist`, made empty when missing, since a missing
//! path would rerun this and relink the app on every build (as it still does in a read-only checkout).

use std::{fs, path::Path};

fn main() {
    let dist = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/panel/dist");
    let _ = fs::create_dir_all(&dist);
    println!("cargo:rerun-if-changed={}", dist.display());
}
