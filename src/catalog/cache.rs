use std::{fs, path::Path};

use anyhow::{Context, Result, bail};

use super::Catalog;
use crate::transport::cdn;

const URL: &str = "https://storage-cdn.wemod.com/catalog.json";
const BODY: &str = "catalog.json";
const ETAG: &str = "catalog.etag";

pub enum Freshness {
    Downloaded,
    NotModified,
    /// The CDN failed; the cached copy is used. Holds the reason.
    Offline(String),
}

/// Revalidates the disk copy against the CDN (blocking).
pub fn load(data_dir: &Path) -> Result<(Catalog, Freshness)> {
    let cached = read(data_dir);
    let etag = cached.as_ref().and_then(|(tag, _)| tag.as_deref());
    // Ok(None) = not modified. A 200 that does not parse (captive portal, cut body) is a failure too.
    let fetched = cdn::get(URL, etag).and_then(|r| match r.status {
        200 => {
            let catalog = Catalog::parse(&r.body)?;
            if let Err(e) = write(data_dir, r.etag.as_deref(), &r.body) {
                eprintln!("catalog not cached: {e:#}");
            }
            Ok(Some(catalog))
        }
        304 => Ok(None),
        status => bail!("HTTP {status}"),
    });
    match (fetched, cached) {
        (Ok(Some(catalog)), _) => Ok((catalog, Freshness::Downloaded)),
        (Ok(None), Some((_, c))) => Ok((c, Freshness::NotModified)),
        (Err(e), Some((_, c))) => Ok((c, Freshness::Offline(format!("{e:#}")))),
        (Ok(None), None) => bail!("catalog: 304 without a cached copy"),
        (Err(e), None) => Err(e.context("catalog")),
    }
}

// An unparsable copy counts as none, so the next request is unconditional.
fn read(dir: &Path) -> Option<(Option<String>, Catalog)> {
    let catalog = Catalog::parse(&fs::read(dir.join(BODY)).ok()?).ok()?;
    let etag = fs::read_to_string(dir.join(ETAG)).ok();
    Some((etag, catalog))
}

// The old tag goes first and the new one last, so a tag never vouches for another body.
fn write(dir: &Path, etag: Option<&str>, body: &[u8]) -> Result<()> {
    fs::create_dir_all(dir).context("create data dir")?;
    let _ = fs::remove_file(dir.join(ETAG));
    replace(&dir.join(BODY), body)?;
    if let Some(tag) = etag {
        replace(&dir.join(ETAG), tag.as_bytes())?;
    }
    Ok(())
}

fn replace(path: &Path, data: &[u8]) -> Result<()> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, data).with_context(|| format!("write {}", tmp.display()))?;
    fs::rename(&tmp, path).with_context(|| format!("replace {}", path.display()))
}
