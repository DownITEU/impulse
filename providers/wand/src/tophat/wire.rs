//! The flatbuffers subset Tophat speaks: flat root tables of scalars, strings and byte vectors.
//! Reads are bounds-checked, so a malformed message reads as defaults instead of panicking.

/// A field to write; each `(slot, field)` pair is one vtable entry.
pub enum Field<'a> {
    Str(&'a str),
    Bytes(&'a [u8]),
    U64(u64),
    U32(u32),
    U8(u8),
    Bool(bool),
}

impl Field<'_> {
    // Inline size, which is also the alignment.
    fn size(&self) -> usize {
        match self {
            Field::Str(_) | Field::Bytes(_) | Field::U32(_) => 4,
            Field::U64(_) => 8,
            Field::U8(_) | Field::Bool(_) => 1,
        }
    }
}

/// A root table. Laid out front to back: root offset, vtable, table, then strings and vectors.
pub fn table(fields: &[(u16, Field)]) -> Vec<u8> {
    let slots = fields.iter().map(|(slot, _)| *slot as usize + 1).max().unwrap_or(0);
    let vtable = 4;
    let vtable_len = 4 + 2 * slots;
    let mut buf = vec![0u8; vtable + vtable_len];
    pad(&mut buf, 8);
    let start = buf.len();
    buf.extend_from_slice(&[0; 4]);
    let mut offsets = vec![0u16; slots];
    let mut pending = Vec::new();
    for (slot, field) in fields {
        pad(&mut buf, field.size());
        let at = buf.len();
        offsets[*slot as usize] = (at - start) as u16;
        match field {
            Field::Str(text) => pending.push((at, text.as_bytes(), true)),
            Field::Bytes(bytes) => pending.push((at, *bytes, false)),
            Field::U64(value) => buf.extend_from_slice(&value.to_le_bytes()),
            Field::U32(value) => buf.extend_from_slice(&value.to_le_bytes()),
            Field::U8(value) => buf.push(*value),
            Field::Bool(value) => buf.push(u8::from(*value)),
        }
        if matches!(field, Field::Str(_) | Field::Bytes(_)) {
            buf.extend_from_slice(&[0; 4]);
        }
    }
    let inline = buf.len() - start;
    for (at, bytes, text) in pending {
        pad(&mut buf, 4);
        let distance = buf.len() - at;
        put_u32(&mut buf, at, distance);
        buf.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        buf.extend_from_slice(bytes);
        if text {
            buf.push(0);
        }
    }
    put_u32(&mut buf, 0, start);
    buf[start..start + 4].copy_from_slice(&((start - vtable) as i32).to_le_bytes());
    buf[vtable..vtable + 2].copy_from_slice(&(vtable_len as u16).to_le_bytes());
    buf[vtable + 2..vtable + 4].copy_from_slice(&(inline as u16).to_le_bytes());
    for (n, offset) in offsets.iter().enumerate() {
        let at = vtable + 4 + 2 * n;
        buf[at..at + 2].copy_from_slice(&offset.to_le_bytes());
    }
    buf
}

fn pad(buf: &mut Vec<u8>, align: usize) {
    buf.resize(buf.len().next_multiple_of(align), 0);
}

fn put_u32(buf: &mut [u8], at: usize, value: usize) {
    buf[at..at + 4].copy_from_slice(&(value as u32).to_le_bytes());
}

/// A root table read back; a missing or out-of-bounds field reads as its default.
#[derive(Debug)]
pub struct Table<'a> {
    buf: &'a [u8],
    start: usize,
    vtable: usize,
    vtable_len: usize,
}

impl<'a> Table<'a> {
    pub fn root(buf: &'a [u8]) -> Option<Self> {
        let start = read::<4>(buf, 0).map(u32::from_le_bytes)? as usize;
        let back = read::<4>(buf, start).map(i32::from_le_bytes)?;
        let vtable = usize::try_from(start as i64 - i64::from(back)).ok()?;
        let vtable_len = read::<2>(buf, vtable).map(u16::from_le_bytes)? as usize;
        Some(Self {
            buf,
            start,
            vtable,
            vtable_len,
        })
    }

    fn field(&self, slot: usize) -> Option<usize> {
        let entry = 4 + 2 * slot;
        if entry + 2 > self.vtable_len {
            return None;
        }
        let offset = read::<2>(self.buf, self.vtable + entry).map(u16::from_le_bytes)?;
        (offset != 0).then(|| self.start + offset as usize)
    }

    fn scalar<const N: usize>(&self, slot: usize) -> [u8; N] {
        self.field(slot)
            .and_then(|at| read(self.buf, at))
            .unwrap_or([0; N])
    }

    pub fn u64(&self, slot: usize) -> u64 {
        u64::from_le_bytes(self.scalar(slot))
    }

    pub fn u8(&self, slot: usize) -> u8 {
        self.scalar::<1>(slot)[0]
    }

    pub fn bool(&self, slot: usize) -> bool {
        self.u8(slot) != 0
    }

    pub fn bytes(&self, slot: usize) -> &'a [u8] {
        let vector = || {
            let at = self.field(slot)?;
            let start = at + u32::from_le_bytes(read(self.buf, at)?) as usize;
            let len = u32::from_le_bytes(read(self.buf, start)?) as usize;
            self.buf.get(start + 4..(start + 4).checked_add(len)?)
        };
        vector().unwrap_or_default()
    }

    pub fn text(&self, slot: usize) -> String {
        String::from_utf8_lossy(self.bytes(slot)).into_owned()
    }
}

/// `N` bytes at `at`, or `None` past the end.
pub fn read<const N: usize>(buf: &[u8], at: usize) -> Option<[u8; N]> {
    buf.get(at..at.checked_add(N)?)?.try_into().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_keeps_every_kind() {
        let raw = table(&[
            (0, Field::Str("register")),
            (1, Field::Bytes(&[1, 2, 3])),
            (2, Field::U64(u64::MAX - 1)),
            (3, Field::U32(5000)),
            (5, Field::Bool(true)),
            (6, Field::U8(3)),
        ]);
        let table = Table::root(&raw).unwrap();
        assert_eq!(table.text(0), "register");
        assert_eq!(table.bytes(1), [1, 2, 3]);
        assert_eq!(table.u64(2), u64::MAX - 1);
        assert_eq!(u32::from_le_bytes(table.scalar(3)), 5000);
        assert!(table.bool(5) && table.u8(6) == 3);
        assert!(table.text(4).is_empty() && table.u64(9) == 0);
    }

    #[test]
    fn fields_are_aligned() {
        let raw = table(&[(0, Field::Bool(true)), (1, Field::U64(7)), (2, Field::Str("x"))]);
        let table = Table::root(&raw).unwrap();
        assert_eq!(table.field(1).unwrap() % 8, 0);
        assert_eq!(table.field(2).unwrap() % 4, 0);
    }

    // From the Python flatbuffers builder (back to front): slots 0 "register", 2 u64 42, 4 "abc", 5 true.
    #[test]
    fn reads_the_reference_builder() {
        let raw = [
            0x14, 0, 0, 0, 0x10, 0, 0x1c, 0, 0x18, 0, 0, 0, 0x0c, 0, 0, 0, 0x08, 0, 0x07, 0, 0x10,
            0, 0, 0, 0, 0, 0, 0x01, 0x14, 0, 0, 0, 0x2a, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x0c, 0,
            0, 0, 0x03, 0, 0, 0, b'a', b'b', b'c', 0, 0x08, 0, 0, 0, b'r', b'e', b'g', b'i', b's',
            b't', b'e', b'r', 0, 0, 0, 0,
        ];
        let table = Table::root(&raw).unwrap();
        assert_eq!(table.text(0), "register");
        assert_eq!(table.u64(2), 42);
        assert_eq!(table.text(4), "abc");
        assert!(table.bool(5));
    }

    #[test]
    fn garbage_reads_as_defaults() {
        for raw in [&[][..], &[0xff; 3], &[0xff; 64], &[4, 0, 0, 0, 0xf0, 0xff, 0xff, 0x7f]] {
            if let Some(table) = Table::root(raw) {
                assert!(table.text(0).is_empty() && table.u64(2) == 0 && table.bytes(1).is_empty());
            }
        }
    }
}
