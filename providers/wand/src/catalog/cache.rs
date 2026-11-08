use std::{fs, path::Path, time::Duration};

use anyhow::{Result, bail};

use super::Catalog;

const URL: &str = "https://storage-cdn.wemod.com/catalog.json";
const BODY: &str = "catalog.json";
const ETAG: &str = "catalog.etag";
// The catalog changes every few minutes (player counts), a trainer far less often.
const MAX_AGE: Duration = Duration::from_secs(3600);

/// The disk copy, without the network (blocking). An unparsable copy counts as none.
pub fn cached(data_dir: &Path) -> Option<Catalog> {
    Catalog::parse(&fs::read(data_dir.join(BODY)).ok()?).ok()
}

pub fn forget(data_dir: &Path) {
    let _ = fs::remove_file(data_dir.join(ETAG));
    let _ = fs::remove_file(data_dir.join(BODY));
}

/// Asks the CDN for a newer catalog (blocking); `None` means the disk copy is current.
/// `have_copy` is false when `cached` found nothing usable, which makes the request unconditional.
pub fn refresh(data_dir: &Path, have_copy: bool) -> Result<Option<Catalog>> {
    let tag_file = data_dir.join(ETAG);
    // The tag file's time is the last check: it is rewritten on every answer.
    let checked = fs::metadata(&tag_file).and_then(|meta| meta.modified());
    if have_copy && checked.is_ok_and(|at| at.elapsed().is_ok_and(|age| age < MAX_AGE)) {
        return Ok(None);
    }
    let etag = have_copy
        .then(|| fs::read_to_string(&tag_file).ok())
        .flatten();
    let response = fetch::get(URL, etag.as_deref())?;
    match response.status {
        // A 200 that does not parse (captive portal, cut body) is a failure too.
        200 => {
            let catalog = Catalog::parse(&response.body)?;
            if let Err(e) = write(data_dir, response.etag.as_deref(), &response.body) {
                log::warn!("catalog not cached: {e:#}");
            }
            Ok(Some(catalog))
        }
        304 if have_copy => {
            if let Some(tag) = etag {
                let _ = disk::replace(&tag_file, tag.as_bytes());
            }
            Ok(None)
        }
        status => bail!("HTTP {status}"),
    }
}

// The old tag goes first and the new one last, so a tag never vouches for another body.
fn write(dir: &Path, etag: Option<&str>, body: &[u8]) -> Result<()> {
    let _ = fs::remove_file(dir.join(ETAG));
    disk::replace(&dir.join(BODY), body)?;
    if let Some(tag) = etag {
        disk::replace(&dir.join(ETAG), tag.as_bytes())?;
    }
    Ok(())
}
