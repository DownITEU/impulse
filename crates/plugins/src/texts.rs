//! A plugin's own text catalogs: `locales/<tag>.ftl` beside its manifest, read once when it opens.

use std::{fs, path::Path, sync::Arc};

use anyhow::{Context, Result, ensure};
use model::Files;

const MOST_FILES: usize = 16;
const FILE: u64 = 256 << 10;
const TAG: usize = 35;

/// A missing folder is none; a file that does not fit is skipped, the rest still load.
pub fn load(dir: &Path) -> Files {
    let Ok(entries) = fs::read_dir(dir.join("locales")) else { return Files::default() };
    let mut files: Vec<_> =
        entries.flatten().map(|entry| entry.path()).filter(|path| path.extension().is_some_and(|extension| extension == "ftl")).collect();
    files.sort();
    if files.len() > MOST_FILES {
        log::debug!("{}: {} catalogs, the first {MOST_FILES} are read", dir.display(), files.len());
        files.truncate(MOST_FILES);
    }
    files.iter().filter_map(|path| catalog(path).inspect_err(|e| log::debug!("catalog skipped: {e:#}")).ok()).collect()
}

fn catalog(path: &Path) -> Result<(Arc<str>, Arc<str>)> {
    let tag = path.file_stem().and_then(|stem| stem.to_str()).context("a name that is not text")?;
    ensure!(
        (1..=TAG).contains(&tag.len()) && tag.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'),
        "{}: not a language tag",
        path.display()
    );
    let source = String::from_utf8(crate::read(path, FILE)?).with_context(|| format!("{} is not text", path.display()))?;
    Ok((tag.into(), source.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("impulse-texts-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("locales")).unwrap();
        dir
    }

    #[test]
    fn a_catalog_with_a_bad_tag_or_size_or_text_is_skipped_and_the_rest_load() {
        let dir = folder("bad");
        let locales = dir.join("locales");
        fs::write(locales.join("en.ftl"), "a = A").unwrap();
        fs::write(locales.join("pt-BR.ftl"), "a = B").unwrap();
        fs::write(locales.join("bad tag.ftl"), "a = C").unwrap();
        fs::write(locales.join(format!("{}.ftl", "x".repeat(36))), "a = D").unwrap();
        fs::write(locales.join("big.ftl"), vec![b'a'; (FILE + 1) as usize]).unwrap();
        fs::write(locales.join("binary.ftl"), [0xFF, 0xFE]).unwrap();
        fs::write(locales.join("notes.txt"), "a = E").unwrap();
        let loaded: Vec<_> = load(&dir).iter().map(|(tag, source)| (tag.to_string(), source.to_string())).collect();
        assert_eq!(loaded, [("en".to_owned(), "a = A".to_owned()), ("pt-BR".to_owned(), "a = B".to_owned())]);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn at_most_sixteen_catalogs_load_and_a_missing_folder_is_none() {
        let dir = folder("many");
        for n in 0..20 {
            fs::write(dir.join("locales").join(format!("l{n:02}.ftl")), "a = A").unwrap();
        }
        assert_eq!(load(&dir).len(), MOST_FILES);
        assert!(load(&dir.join("nowhere")).is_empty());
        let _ = fs::remove_dir_all(dir);
    }
}
