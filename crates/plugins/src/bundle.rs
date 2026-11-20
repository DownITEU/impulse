//! A plugin's files read once, from a folder or a `.zip`, each with its hash: what is shown for approval, written at
//! install and opened later all come from one read. A zip is read in memory, never extracted.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs::{self, File},
    io::{BufReader, Read},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow, ensure};
use model::{Failure, FailureKind};
use sdk::manifest::Manifest;
use sha2::{Digest, Sha256};
use zip::{CompressionMethod, ZipArchive};

use crate::{MODULE, manifest, scope::Scope};

const MOST_CATALOGS: usize = 16;
const CATALOG: u64 = 256 << 10;
const ENTRIES: usize = 1024;
pub(crate) const MANIFEST: &str = "manifest.json";

/// Lower-case hex sha256: how a file is pinned.
pub(crate) fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// A plugin's manifest, module and catalogs, all within their bounds.
pub struct Bundle {
    pub(crate) manifest: Manifest,
    pub(crate) scope: Scope,
    pub(crate) module: Vec<u8>,
    manifest_bytes: Vec<u8>,
    /// Tag and text; a catalog that broke a rule is not here.
    catalogs: Vec<(String, String)>,
    /// Lower-case hex sha256 by file name: `manifest.json`, the module, `locales/<tag>.ftl`.
    hashes: BTreeMap<String, String>,
}

impl Bundle {
    /// The plugin in the folder or `.zip` at `pick` (blocking). Anything else is `Refused`; the manifest is checked
    /// before the module is read.
    pub fn read(pick: &Path) -> Result<Self, Failure> {
        Self::load(pick).map_err(Failure::from)
    }

    pub fn hashes(&self) -> &BTreeMap<String, String> {
        &self.hashes
    }

    /// One hash over every name and file hash: the same files give the same digest, whichever way they came.
    pub fn digest(&self) -> String {
        let mut sha = Sha256::new();
        for (name, hash) in &self.hashes {
            sha.update(name);
            sha.update([0]);
            sha.update(hash);
            sha.update(b"\n");
        }
        format!("{:x}", sha.finalize())
    }

    /// Every file by name, as it is written when installed.
    pub fn files(&self) -> Vec<(String, &[u8])> {
        let kept = [(MANIFEST.to_owned(), self.manifest_bytes.as_slice()), (self.manifest.module.clone(), self.module.as_slice())];
        let catalogs = self.catalogs.iter().map(|(tag, text)| (format!("locales/{tag}.ftl"), text.as_bytes()));
        kept.into_iter().chain(catalogs).collect()
    }

    pub(crate) fn catalogs(&self) -> &[(String, String)] {
        &self.catalogs
    }

    fn load(pick: &Path) -> Result<Self> {
        let meta = fs::metadata(pick).with_context(|| format!("open {}", pick.display()))?;
        let mut files: Box<dyn Reader> = if meta.is_dir() {
            Box::new(Folder(pick.to_owned()))
        } else if pick.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("zip")) {
            Box::new(Archive::open(pick)?)
        } else {
            return Err(refused(format!("{} is neither a folder nor a .zip", pick.display())));
        };
        let manifest_bytes = files.get(MANIFEST, manifest::MANIFEST)?;
        let (manifest, scope) = manifest::parse(&manifest_bytes)?;
        let module = files.get(&manifest.module, MODULE)?;
        let mut names: Vec<_> = files.locales().into_iter().filter(|name| name.ends_with(".ftl")).collect();
        names.sort();
        if names.len() > MOST_CATALOGS {
            log::debug!("{}: {} catalogs, the first {MOST_CATALOGS} are read", manifest.id, names.len());
            names.truncate(MOST_CATALOGS);
        }
        // Names that differ only by case cannot coexist on Windows: the first one read wins.
        let mut taken = HashSet::new();
        let catalogs: Vec<_> = names
            .iter()
            .filter_map(|name| {
                let found = catalog(&mut *files, name).inspect_err(|e| log::debug!("catalog skipped: {e:#}")).ok()?;
                if !taken.insert(found.0.to_ascii_lowercase()) {
                    log::debug!("catalog skipped: {name} differs only by case from another");
                    return None;
                }
                Some(found)
            })
            .collect();
        let mut hashes = BTreeMap::from([(MANIFEST.to_owned(), hash(&manifest_bytes)), (manifest.module.clone(), hash(&module))]);
        hashes.extend(catalogs.iter().map(|(tag, text)| (format!("locales/{tag}.ftl"), hash(text.as_bytes()))));
        Ok(Self { manifest, scope, module, manifest_bytes, catalogs, hashes })
    }
}

fn catalog(files: &mut dyn Reader, name: &str) -> Result<(String, String)> {
    let tag = name.strip_suffix(".ftl").unwrap_or(name);
    ensure!(manifest::language_tag(tag), "{name}: not a language tag");
    let source = String::from_utf8(files.get(&format!("locales/{name}"), CATALOG)?).with_context(|| format!("{name} is not text"))?;
    Ok((tag.to_owned(), source))
}

fn refused(detail: String) -> anyhow::Error {
    anyhow!(detail).context(Failure::new(FailureKind::Refused))
}

/// Where the files come from.
trait Reader {
    /// The file `name` (`manifest.json`, `locales/en.ftl`) of at most `most` bytes. Missing: `NotFound`; larger:
    /// `Refused`.
    fn get(&mut self, name: &str, most: u64) -> Result<Vec<u8>>;
    /// The names inside `locales`, of any kind.
    fn locales(&self) -> Vec<String>;
}

struct Folder(PathBuf);

impl Reader for Folder {
    fn get(&mut self, name: &str, most: u64) -> Result<Vec<u8>> {
        crate::read(&self.0.join(name), most)
    }

    fn locales(&self) -> Vec<String> {
        let entries = fs::read_dir(self.0.join("locales"));
        entries.into_iter().flatten().flatten().filter_map(|entry| entry.file_name().into_string().ok()).collect()
    }
}

struct Archive {
    zip: ZipArchive<BufReader<File>>,
    /// Entry index by name below the plugin's folder.
    index: HashMap<String, usize>,
}

impl Archive {
    fn open(path: &Path) -> Result<Self> {
        let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
        let zip = ZipArchive::new(BufReader::new(file)).context("not a zip").context(Failure::new(FailureKind::Refused))?;
        let entries = zip.len();
        if entries > ENTRIES {
            return Err(refused(format!("{entries} entries, past {ENTRIES}")));
        }
        let names: Vec<_> = (0..entries).filter_map(|n| Some((zip.name_for_index(n)?.ok()?.replace('\\', "/"), n))).collect();
        let top = root(&names).map_err(|e| refused(format!("{e}")))?;
        let index = names.into_iter().filter_map(|(name, n)| Some((name.strip_prefix(&top)?.to_owned(), n))).collect();
        Ok(Self { zip, index })
    }
}

/// The folder prefix of the files: none when `manifest.json` is at the root, else the one top folder that holds it.
fn root(names: &[(String, usize)]) -> Result<String> {
    if names.iter().any(|(name, _)| name == MANIFEST) {
        return Ok(String::new());
    }
    let suffix = format!("/{MANIFEST}");
    let tops: Vec<_> =
        names.iter().filter_map(|(name, _)| name.strip_suffix(&suffix)).filter(|top| !top.is_empty() && !top.contains('/')).collect();
    match tops[..] {
        [top] => Ok(format!("{top}/")),
        _ => Err(anyhow!("no {MANIFEST} at the root of the zip or in its one top folder")),
    }
}

impl Reader for Archive {
    fn get(&mut self, name: &str, most: u64) -> Result<Vec<u8>> {
        let n = *self.index.get(name).ok_or_else(|| anyhow!("{name} is not in the zip")).context(Failure::new(FailureKind::NotFound))?;
        let mut entry = self.zip.by_index(n).map_err(|e| refused(format!("{name} in the zip: {e}")))?;
        let plain = matches!(entry.compression(), CompressionMethod::Stored | CompressionMethod::Deflated);
        if !(entry.is_file() && plain && !entry.encrypted()) {
            return Err(refused(format!("{name} in the zip is not a plain file")));
        }
        if entry.size() > most {
            return Err(refused(format!("{name} is past {most} bytes")));
        }
        let mut bytes = Vec::new();
        // The header may lie about the size: the read is bounded too.
        (&mut entry).take(most + 1).read_to_end(&mut bytes).map_err(|e| refused(format!("read {name} from the zip: {e}")))?;
        if bytes.len() as u64 > most {
            return Err(refused(format!("{name} is past {most} bytes")));
        }
        Ok(bytes)
    }

    fn locales(&self) -> Vec<String> {
        self.index.keys().filter_map(|name| name.strip_prefix("locales/")).filter(|rest| !rest.contains('/')).map(str::to_owned).collect()
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use serde_json::json;
    use zip::write::SimpleFileOptions;

    use super::*;

    fn folder(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("impulse-bundle-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn manifest() -> Vec<u8> {
        let json =
            json!({"id": "maps", "name": "Maps", "version": "1.0", "api": 1, "kind": "maps", "module": "maps.wasm", "locales": ["en"]});
        serde_json::to_vec(&json).unwrap()
    }

    fn write_folder(dir: &Path, files: &[(&str, &[u8])]) {
        for (name, bytes) in files {
            let file = dir.join(name);
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            fs::write(file, bytes).unwrap();
        }
    }

    /// Files by name, as a zip with every entry under `top`.
    fn write_zip(file: &Path, top: &str, files: &[(&str, &[u8])]) {
        let mut zip = zip::ZipWriter::new(File::create(file).unwrap());
        for (name, bytes) in files {
            let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
            zip.start_file(format!("{top}{name}"), options).unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
    }

    fn files(manifest: &[u8]) -> Vec<(&str, &[u8])> {
        vec![
            ("manifest.json", manifest),
            ("maps.wasm", b"\0asm"),
            ("locales/en.ftl", b"a = A"),
            ("locales/bad tag.ftl", b"a = B"),
            ("locales/binary.ftl", &[0xFF, 0xFE]),
        ]
    }

    #[test]
    fn a_folder_and_a_zip_of_the_same_files_have_the_same_digest_and_a_bad_catalog_is_skipped() {
        let dir = folder("same");
        let manifest = manifest();
        write_folder(&dir.join("plugin"), &files(&manifest));
        let mut inside = files(&manifest);
        inside.push(("notes.txt", b"ignored"));
        write_zip(&dir.join("top.zip"), "maps/", &inside);
        write_zip(&dir.join("root.ZIP"), "", &files(&manifest));
        let from_folder = Bundle::read(&dir.join("plugin")).unwrap();
        assert_eq!(from_folder.hashes().keys().collect::<Vec<_>>(), ["locales/en.ftl", "manifest.json", "maps.wasm"]);
        for zip in ["top.zip", "root.ZIP"] {
            assert_eq!(Bundle::read(&dir.join(zip)).unwrap().digest(), from_folder.digest(), "{zip}");
        }
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_catalog_whose_name_differs_only_by_case_from_an_earlier_one_is_skipped() {
        let dir = folder("case");
        let manifest = manifest();
        let all: [(&str, &[u8]); 4] =
            [("manifest.json", &manifest), ("maps.wasm", b"\0asm"), ("locales/en-US.ftl", b"a = A"), ("locales/en-us.ftl", b"a = B")];
        write_zip(&dir.join("case.zip"), "", &all);
        let bundle = Bundle::read(&dir.join("case.zip")).unwrap();
        assert_eq!(bundle.catalogs().iter().map(|(tag, _)| tag.as_str()).collect::<Vec<_>>(), ["en-US"]);
        assert!(bundle.hashes().contains_key("locales/en-US.ftl") && !bundle.hashes().contains_key("locales/en-us.ftl"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_module_past_its_bound_or_a_pick_of_another_kind_is_refused() {
        let dir = folder("bounds");
        let manifest = manifest();
        let big = vec![0u8; MODULE as usize + 1];
        write_zip(&dir.join("big.zip"), "", &[("manifest.json", &manifest), ("maps.wasm", &big)]);
        write_folder(&dir.join("plain"), &[("manifest.json", &manifest), ("maps.wasm", &big)]);
        write_folder(&dir, &[("notes.txt", b"x")]);
        for pick in ["big.zip", "plain", "notes.txt"] {
            assert_eq!(Bundle::read(&dir.join(pick)).err().map(|e| e.kind), Some(FailureKind::Refused), "{pick}");
        }
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn at_most_sixteen_catalogs_load_and_one_past_its_size_is_skipped() {
        let dir = folder("many");
        let manifest = manifest();
        let mut all = vec![("manifest.json", manifest.as_slice()), ("maps.wasm", b"\0asm".as_slice())];
        let names: Vec<_> = (0..20).map(|n| format!("locales/l{n:02}.ftl")).collect();
        all.extend(names.iter().map(|name| (name.as_str(), b"a = A".as_slice())));
        write_folder(&dir.join("many"), &all);
        assert_eq!(Bundle::read(&dir.join("many")).unwrap().catalogs().len(), MOST_CATALOGS);
        let big = vec![b'a'; CATALOG as usize + 1];
        write_folder(&dir.join("big"), &[("manifest.json", &manifest), ("maps.wasm", b"\0asm"), ("locales/big.ftl", &big)]);
        assert!(Bundle::read(&dir.join("big")).unwrap().catalogs().is_empty());
        let _ = fs::remove_dir_all(dir);
    }
}
