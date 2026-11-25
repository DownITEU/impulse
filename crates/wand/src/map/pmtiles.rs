//! A map's tiles in one PMTiles v3 archive (github.com/protomaps/PMTiles, spec/v3): the header and the directories
//! are read with range requests and kept, then each tile is one more range.

use anyhow::{Context, Result, bail, ensure};

/// The header and the root directory lie within the archive's first bytes.
pub(super) const FIRST: u64 = 16_384;
const HEADER: usize = 127;
// Leaves within leaves are allowed but discouraged: a few levels at most.
const MAX_DEPTH: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Compression {
    None,
    Gzip,
}

/// Offset and length of a part of the archive.
pub(super) type Section = (u64, u64);

#[derive(Debug, PartialEq)]
pub(super) struct Header {
    pub root: Section,
    pub leaves: Section,
    pub data: Section,
    /// Of the directories, and of the tiles.
    pub internal: Compression,
    pub tiles: Compression,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Entry {
    pub tile_id: u64,
    pub offset: u64,
    pub length: u64,
    /// Tiles from `tile_id` on that share these bytes; 0: the bytes are a leaf directory.
    pub run: u64,
}

/// A leaf directory, shared by the tiles it holds.
pub(super) type Leaf = std::sync::Arc<Vec<Entry>>;

pub(super) fn header(bytes: &[u8]) -> Result<Header> {
    ensure!(bytes.len() >= HEADER && bytes.starts_with(b"PMTiles") && bytes[7] == 3, "not a PMTiles v3 archive");
    let at = |offset: usize| u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("8 bytes"));
    let compression = |code: u8| match code {
        1 => Ok(Compression::None),
        2 => Ok(Compression::Gzip),
        other => bail!("compression {other} is not supported"),
    };
    Ok(Header {
        root: (at(8), at(16)),
        leaves: (at(40), at(48)),
        data: (at(56), at(64)),
        internal: compression(bytes[97])?,
        tiles: compression(bytes[98])?,
    })
}

/// A decompressed directory: its entries in columns of varints, ids as deltas, an offset 0 right after the last.
pub(super) fn directory(bytes: &[u8]) -> Result<Vec<Entry>> {
    let mut rest = bytes;
    let count = usize::try_from(varint(&mut rest)?)?;
    // An entry takes 4 bytes at least: a count past that is damage, not a size to allocate.
    ensure!(count > 0 && count <= bytes.len() / 4, "a directory of {count} entries in {} bytes", bytes.len());
    let mut entries = vec![Entry::default(); count];
    let mut id = 0_u64;
    for entry in &mut entries {
        id = id.checked_add(varint(&mut rest)?).context("tile ids overflow")?;
        entry.tile_id = id;
    }
    for entry in &mut entries {
        entry.run = varint(&mut rest)?;
    }
    for entry in &mut entries {
        entry.length = varint(&mut rest)?;
        ensure!(entry.length > 0, "an empty entry");
    }
    for at in 0..count {
        entries[at].offset = match (varint(&mut rest)?, at.checked_sub(1)) {
            (0, Some(before)) => entries[before].offset.checked_add(entries[before].length).context("offsets overflow")?,
            (0, None) => bail!("the first entry has no offset"),
            (offset, _) => offset - 1,
        };
    }
    Ok(entries)
}

fn varint(rest: &mut &[u8]) -> Result<u64> {
    let mut value = 0_u64;
    for shift in (0..64).step_by(7) {
        let (&byte, after) = rest.split_first().context("a directory ends early")?;
        *rest = after;
        // The tenth byte holds bit 63 alone.
        ensure!(shift < 63 || byte <= 1, "a varint past 64 bits");
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
    }
    bail!("a varint past 64 bits")
}

/// The tile's place on the Hilbert curve of its zoom, after every tile of the zooms above.
pub(super) fn tile_id(z: u8, x: u32, y: u32) -> Option<u64> {
    let n = 1_u64.checked_shl(u32::from(z)).filter(|_| z <= 31)?;
    let (mut x, mut y) = (u64::from(x), u64::from(y));
    if x >= n || y >= n {
        return None;
    }
    let mut id = (n * n - 1) / 3;
    let mut s = n / 2;
    while s > 0 {
        let (rx, ry) = (u64::from(x & s != 0), u64::from(y & s != 0));
        id += s * s * ((3 * rx) ^ ry);
        if ry == 0 {
            if rx == 1 {
                (x, y) = (n - 1 - x, n - 1 - y);
            }
            (x, y) = (y, x);
        }
        s /= 2;
    }
    Some(id)
}

/// The entry that holds `id` in one directory: its tile, or the leaf directory to look in next.
pub(super) fn find(entries: &[Entry], id: u64) -> Option<Entry> {
    let entry = entries[entries.partition_point(|entry| entry.tile_id <= id).checked_sub(1)?];
    (entry.run == 0 || id - entry.tile_id < entry.run).then_some(entry)
}

/// Walks from the root to the tile's bytes (offset in the data section, length); `leaf` reads a leaf directory at an
/// offset of the leaves' section. `None`: the archive has no such tile.
pub(super) fn locate(root: &[Entry], id: u64, mut leaf: impl FnMut(u64, u64) -> Result<Leaf>) -> Result<Option<(u64, u64)>> {
    let mut found = find(root, id);
    for depth in 0..=MAX_DEPTH {
        match found {
            None => return Ok(None),
            Some(entry) if entry.run > 0 => return Ok(Some((entry.offset, entry.length))),
            Some(entry) if depth < MAX_DEPTH => found = find(&leaf(entry.offset, entry.length)?, id),
            Some(_) => break,
        }
    }
    bail!("leaf directories nested past {MAX_DEPTH} levels")
}

#[cfg(test)]
pub(super) fn varints(values: &[u64]) -> Vec<u8> {
    let mut out = Vec::new();
    for &value in values {
        let mut value = value;
        while value >= 0x80 {
            out.push((value & 0x7f) as u8 | 0x80);
            value >>= 7;
        }
        out.push(value as u8);
    }
    out
}

/// A header with these sections and compression codes.
#[cfg(test)]
pub(super) fn header_bytes(root: Section, leaves: Section, data: Section, internal: u8, tiles: u8) -> Vec<u8> {
    let mut bytes = vec![0_u8; HEADER];
    bytes[..8].copy_from_slice(b"PMTiles\x03");
    for (at, value) in [(8, root.0), (16, root.1), (40, leaves.0), (48, leaves.1), (56, data.0), (64, data.1)] {
        bytes[at..at + 8].copy_from_slice(&value.to_le_bytes());
    }
    (bytes[97], bytes[98]) = (internal, tiles);
    bytes
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    #[test]
    fn tile_ids_follow_the_spec_s_hilbert_curve() {
        assert_eq!(tile_id(0, 0, 0), Some(0));
        assert_eq!([(0, 0), (0, 1), (1, 1), (1, 0)].map(|(x, y)| tile_id(1, x, y)), [Some(1), Some(2), Some(3), Some(4)]);
        assert_eq!(tile_id(12, 3423, 1763), Some(19_078_479));
        assert_eq!(tile_id(1, 2, 0), None);
    }

    #[test]
    fn a_directory_reads_its_columns_and_offsets_that_follow_the_last() {
        // Ids 1, 2, 7; runs 1, 4, 0 (a leaf); lengths 10, 20, 30; offsets 5, right after, 100.
        let bytes = varints(&[3, 1, 1, 5, 1, 4, 0, 10, 20, 30, 6, 0, 101]);
        let entries = directory(&bytes).unwrap();
        let ids: Vec<_> = entries.iter().map(|entry| (entry.tile_id, entry.offset, entry.length, entry.run)).collect();
        assert_eq!(ids, [(1, 5, 10, 1), (2, 15, 20, 4), (7, 100, 30, 0)]);
        assert_eq!(find(&entries, 4).map(|entry| entry.tile_id), Some(2));
        assert_eq!(find(&entries, 6), None, "past the run of 2");
        assert_eq!(find(&entries, 0), None);
        assert!(directory(&varints(&[1_000_000, 1])).is_err(), "a count the bytes cannot hold");
        assert!(directory(&varints(&[1, 1, 1, 1, 0])).is_err(), "a first offset of 0");
        assert!(directory(&varints(&[2, 0, 1, 1, 1, 2, 1, u64::MAX, 0])).is_err(), "an offset after the last past u64");
        let tenth = [&[0x80_u8; 9][..], &[0x02]].concat();
        assert!(varint(&mut tenth.as_slice()).is_err(), "bit 64");
    }

    #[test]
    fn a_tile_is_found_through_leaves_as_deep_as_allowed_and_a_missing_one_is_none() {
        let root = [Entry { tile_id: 0, offset: 0, length: 9, run: 0 }];
        let leaf = Arc::new(vec![Entry { tile_id: 3, offset: 40, length: 8, run: 2 }]);
        let read = |offset, length| {
            assert_eq!((offset, length), (0, 9));
            Ok(leaf.clone())
        };
        assert_eq!(locate(&root, 4, read).unwrap(), Some((40, 8)));
        assert_eq!(locate(&root, 9, |_, _| Ok(leaf.clone())).unwrap(), None);

        // Leaves that point at leaves: the last allowed one is read and used.
        let deeper = Arc::new(root.to_vec());
        let walk = |depth| {
            let (deeper, leaf, mut reads) = (deeper.clone(), leaf.clone(), 0);
            move |_, _| {
                reads += 1;
                Ok(if reads < depth { deeper.clone() } else { leaf.clone() })
            }
        };
        assert_eq!(locate(&root, 4, walk(MAX_DEPTH)).unwrap(), Some((40, 8)));
        assert!(locate(&root, 4, walk(MAX_DEPTH + 1)).is_err());
    }

    #[test]
    fn a_header_names_its_sections_and_compressions() {
        let mut bytes = header_bytes((127, 49), (309, 24_803), (25_112, 9_000), 2, 1);
        let header = header(&bytes).unwrap();
        let (internal, tiles) = (Compression::Gzip, Compression::None);
        assert_eq!(header, Header { root: (127, 49), leaves: (309, 24_803), data: (25_112, 9_000), internal, tiles });
        bytes[98] = 4;
        assert!(super::header(&bytes).is_err(), "zstd tiles");
    }
}
