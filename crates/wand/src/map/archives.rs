//! Map tiles out of PMTiles archives on the maps CDN, behind the URLs `parse` mints (`wand:pmtiles/<uuid>/{z}/{x}/{y}`).
//! The archive's address is made again here from the uuid alone, so a map file never names a host we fetch from.

use std::{
    collections::HashMap,
    fmt,
    io::Read,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use anyhow::{Context, Result, anyhow, bail, ensure};
use flate2::read::MultiGzDecoder;
use model::{Failure, FailureKind};
use uuid::Uuid;

use super::pmtiles::{self, Compression, Entry, FIRST, Header, Leaf, Section};

const CDN: &str = "https://maps-cdn.wemod.com/maps/";
const FILE: &str = "/map.pmtiles";
const PREFIX: &str = "pmtiles/";
// A directory or a tile unpacked past this is damage.
const MOST: u64 = 4 << 20;
// What is kept: a few maps a session, and per archive leaves of this many entries (32 bytes each) in all.
const ARCHIVES: usize = 8;
const LEAF_ENTRIES: usize = 1 << 20;

/// The tile template for an archive on the maps CDN; `None` for one anywhere else.
pub(super) fn template(url: &str) -> Option<String> {
    let uuid = Uuid::try_parse(url.strip_prefix(CDN)?.strip_suffix(FILE)?).ok()?;
    Some(format!("{}:{PREFIX}{}/{{z}}/{{x}}/{{y}}", crate::SOURCE, uuid.hyphenated()))
}

type Slot = Arc<Mutex<Option<Arc<Archive>>>>;

/// The headers and directories read so far, by archive: a tile then costs one range.
#[derive(Default)]
pub struct Archives(Mutex<HashMap<Uuid, Slot>>);

impl Archives {
    /// A tile's bytes (blocking). `NotFound`: no such tile or archive; `Refused`: not a URL of ours, or an archive
    /// this cannot read.
    pub fn tile(&self, url: &str) -> Result<Vec<u8>, Failure> {
        let Some((uuid, z, x, y)) = parts(url) else {
            return Err(Failure { detail: format!("{url}: not a map tile of ours"), ..Failure::new(FailureKind::Refused) });
        };
        let Some(id) = pmtiles::tile_id(z, x, y) else { return Err(Failure::new(FailureKind::NotFound)) };
        let slot = self.slot(uuid);
        let archive = opened(&slot, uuid)?;
        let read = archive.tile(id);
        if read.as_ref().is_err_and(|e| e.downcast_ref::<Replaced>().is_some()) {
            let mut slot = lock(&slot);
            // Another tile may have read the new copy meanwhile.
            if slot.as_ref().is_some_and(|kept| Arc::ptr_eq(kept, &archive)) {
                *slot = None;
            }
        }
        read.map_err(Failure::from)
    }

    fn slot(&self, uuid: Uuid) -> Slot {
        let mut slots = lock(&self.0);
        if slots.len() >= ARCHIVES && !slots.contains_key(&uuid) {
            slots.clear();
        }
        slots.entry(uuid).or_default().clone()
    }
}

/// One read of a header at a time: the tiles of a map just opened all ask for it at once.
fn opened(slot: &Slot, uuid: Uuid) -> Result<Arc<Archive>> {
    let mut slot = lock(slot);
    if let Some(archive) = slot.as_ref() {
        return Ok(archive.clone());
    }
    let archive = Arc::new(Archive::open(format!("{CDN}{}{FILE}", uuid.hyphenated()))?);
    *slot = Some(archive.clone());
    Ok(archive)
}

/// `pmtiles/<uuid>/<z>/<x>/<y>` of a URL of ours.
fn parts(url: &str) -> Option<(Uuid, u8, u32, u32)> {
    let mut parts = url.strip_prefix(crate::SOURCE)?.strip_prefix(':')?.strip_prefix(PREFIX)?.split('/');
    let uuid = Uuid::try_parse(parts.next()?).ok()?;
    let (z, x, y) = (parts.next()?.parse().ok()?, parts.next()?.parse().ok()?, parts.next()?.parse().ok()?);
    parts.next().is_none().then_some((uuid, z, x, y))
}

struct Archive {
    url: String,
    header: Header,
    root: Vec<Entry>,
    // The CDN's files never change: another tag means this one was replaced between two reads.
    etag: Option<String>,
    leaves: Mutex<HashMap<(u64, u64), Leaf>>,
}

/// The archive changed between two reads: what was read of it no longer holds.
#[derive(Debug)]
struct Replaced;

impl fmt::Display for Replaced {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the archive changed while read")
    }
}

impl std::error::Error for Replaced {}

/// Bytes read whole that make no sense: asking again gives the same.
fn unreadable() -> Failure {
    Failure::new(FailureKind::Refused)
}

impl Archive {
    fn open(url: String) -> Result<Self> {
        let first = range(&url, 0, FIRST, None)?;
        let header = pmtiles::header(&first.body).with_context(unreadable)?;
        let root = within(&first.body, header.root).context("the root directory lies past the first bytes").with_context(unreadable)?;
        let root = directory(root.to_vec(), header.internal)?;
        Ok(Self { url, header, root, etag: first.etag, leaves: Mutex::default() })
    }

    fn tile(&self, id: u64) -> Result<Vec<u8>> {
        let found = pmtiles::locate(&self.root, id, |offset, length| self.leaf(offset, length))?;
        let (offset, length) = found.ok_or_else(|| anyhow!("no tile {id}").context(Failure::new(FailureKind::NotFound)))?;
        unpack(self.read(self.header.data, offset, length)?, self.header.tiles).with_context(unreadable)
    }

    fn leaf(&self, offset: u64, length: u64) -> Result<Leaf> {
        if let Some(leaf) = lock(&self.leaves).get(&(offset, length)) {
            return Ok(leaf.clone());
        }
        let leaf = Arc::new(directory(self.read(self.header.leaves, offset, length)?, self.header.internal)?);
        let mut leaves = lock(&self.leaves);
        if leaves.values().map(|kept| kept.len()).sum::<usize>() + leaf.len() > LEAF_ENTRIES {
            leaves.clear();
        }
        leaves.insert((offset, length), leaf.clone());
        Ok(leaf)
    }

    /// `length` bytes at `offset` within `section`.
    fn read(&self, (start, size): Section, offset: u64, length: u64) -> Result<Vec<u8>> {
        let inside = length <= MOST && offset.checked_add(length).is_some_and(|end| end <= size);
        let from = start.checked_add(offset).filter(|from| inside && from.checked_add(length).is_some());
        let from = from.context("an entry past its section").with_context(unreadable)?;
        let read = range(&self.url, from, length, self.etag.as_deref())?;
        // Shorter only at the file's end: the same file gives the same.
        if read.body.len() as u64 != length {
            return Err(anyhow!("{} bytes of {length} at {from}", read.body.len()).context(unreadable()));
        }
        Ok(read.body)
    }
}

fn directory(bytes: Vec<u8>, compression: Compression) -> Result<Vec<Entry>> {
    unpack(bytes, compression).and_then(|bytes| pmtiles::directory(&bytes)).with_context(unreadable)
}

/// A missing archive is `NotFound`; one whose tag is not `etag` any more, `Replaced`, whatever the status.
fn range(url: &str, from: u64, len: u64, etag: Option<&str>) -> Result<fetch::Response> {
    let response = fetch::range(url, from, len)?;
    if let (Some(kept), Some(now)) = (etag, response.etag.as_deref())
        && kept != now
    {
        bail!(Replaced);
    }
    match response.status {
        206 => Ok(response),
        403 | 404 | 410 => Err(anyhow!("GET {url}: HTTP {}", response.status).context(Failure::new(FailureKind::NotFound))),
        status => bail!("GET {url}: HTTP {status} {}", response.edge),
    }
}

fn within(bytes: &[u8], (at, len): Section) -> Option<&[u8]> {
    let at = usize::try_from(at).ok()?;
    bytes.get(at..at.checked_add(usize::try_from(len).ok()?)?)
}

fn unpack(bytes: Vec<u8>, compression: Compression) -> Result<Vec<u8>> {
    match compression {
        Compression::None => Ok(bytes),
        Compression::Gzip => {
            let mut out = Vec::new();
            MultiGzDecoder::new(bytes.as_slice()).take(MOST + 1).read_to_end(&mut out).context("unpack gzip")?;
            ensure!(out.len() as u64 <= MOST, "unpacked past {MOST} bytes");
            Ok(out)
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests;
