//! The build of an install: the stamp of the catalog's version file, read by its kind.

use std::{
    ffi::{CStr, c_void},
    fs::{self, File},
    io::Read,
    mem,
    os::windows::ffi::OsStrExt,
    path::Path,
    ptr, slice,
};

use windows_sys::Win32::{
    Security::Cryptography::{
        CMSG_CONTENT_PARAM, CMSG_SIGNER_UNAUTH_ATTR_PARAM, CRYPT_ATTRIBUTES, CryptMsgClose, CryptMsgGetParam, CryptMsgOpenToDecode,
        CryptMsgUpdate, PKCS_7_ASN_ENCODING, X509_ASN_ENCODING,
    },
    Storage::FileSystem::{GetFileAttributesExW, GetFileExInfoStandard, WIN32_FILE_ATTRIBUTE_DATA},
};

const RFC3161_COUNTERSIGNATURE: &[u8] = b"1.3.6.1.4.1.311.3.3.1";
// FILETIME counts 100 ns ticks from 1601.
const SECS_1601_TO_1970: u64 = 11_644_473_600;

/// `file` is relative to `location`. `packaged`: an Xbox install, whose files may refuse to be read (blocking).
pub fn build(location: &Path, file: &str, packaged: bool) -> Option<u32> {
    let path = location.join(file);
    let kind = Path::new(file).extension().map(|kind| kind.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    let read = match kind.as_str() {
        "exe" | "dll" => pe::header(&path).map(|header| header.timestamp),
        "p7x" => fs::read(&path).ok().and_then(|signature| signing_time(&signature)),
        "zip" | "jar" => head(&path).and_then(|head| zip_date(&head)),
        _ => None,
    };
    if read.is_some() || !packaged {
        return read;
    }
    // A package's file it cannot read gives its last write time; a signature it cannot reach, the manifest's.
    match (kind == "p7x", fs::metadata(&path).is_ok()) {
        (true, true) => None,
        (true, false) => modified(&location.join("AppxManifest.xml")),
        (false, _) => modified(&path),
    }
}

fn head(path: &Path) -> Option<[u8; 14]> {
    let mut head = [0; 14];
    File::open(path).and_then(|mut file| file.read_exact(&mut head)).ok().map(|()| head)
}

/// The first entry's DOS date and time in an archive's first local header, as UTC.
fn zip_date(head: &[u8; 14]) -> Option<u32> {
    if head[..4] != *b"PK\x03\x04" {
        return None;
    }
    let (time, date) = (u16::from_le_bytes([head[10], head[11]]), u16::from_le_bytes([head[12], head[13]]));
    let day = (i64::from(date >> 9) + 1980, i64::from((date >> 5) & 0xF), i64::from(date & 0x1F));
    let clock = (i64::from(time >> 11), i64::from((time >> 5) & 0x3F), i64::from(time & 0x1F) * 2);
    unix(day, clock)
}

/// The time of the first signer's RFC 3161 countersignature (its token's `genTime`).
fn signing_time(file: &[u8]) -> Option<u32> {
    // A package's signature file is the PKCS #7 message after a mark.
    let signed = Message::decode(file.strip_prefix(b"PKCX").unwrap_or(file))?;
    let (attributes, _) = signed.param(CMSG_SIGNER_UNAUTH_ATTR_PARAM)?;
    // SAFETY: the parameter is a CRYPT_ATTRIBUTES at the buffer's start, aligned, its arrays inside the same buffer.
    let attributes = unsafe { &*attributes.as_ptr().cast::<CRYPT_ATTRIBUTES>() };
    // SAFETY: `rgAttr` holds `cAttr` attributes.
    for attribute in unsafe { slice::from_raw_parts(attributes.rgAttr, attributes.cAttr as usize) } {
        // SAFETY: each attribute's id is a NUL-terminated string in the buffer.
        if unsafe { CStr::from_ptr(attribute.pszObjId.cast()) }.to_bytes() != RFC3161_COUNTERSIGNATURE || attribute.cValue == 0 {
            continue;
        }
        // SAFETY: `rgValue` holds `cValue` (at least one) blobs.
        let token = unsafe { &*attribute.rgValue };
        // SAFETY: a blob is valid for `cbData` bytes.
        let token = unsafe { slice::from_raw_parts(token.pbData, token.cbData as usize) };
        let token = Message::decode(token)?;
        let (info, len) = token.param(CMSG_CONTENT_PARAM)?;
        // SAFETY: the buffer holds at least `len` bytes.
        return generalized_time(unsafe { slice::from_raw_parts(info.as_ptr().cast::<u8>(), len) });
    }
    None
}

/// The first DER GeneralizedTime `YYYYMMDDHHMMSS.fZ` in `der`, in whole seconds. One without the fraction is no time.
fn generalized_time(der: &[u8]) -> Option<u32> {
    let times =
        der.windows(2).enumerate().filter(|(_, tag)| tag[0] == 0x18).filter_map(|(at, tag)| der.get(at + 2..at + 2 + usize::from(tag[1])));
    let digits = |text: &str| !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit());
    let text = times.filter_map(|time| str::from_utf8(time).ok()).find(|time| {
        let (whole, rest) = time.split_at_checked(14).unwrap_or_default();
        digits(whole) && rest.strip_prefix('.').and_then(|rest| rest.strip_suffix('Z')).is_some_and(digits)
    })?;
    let number = |at: usize, len: usize| text[at..at + len].parse::<i64>().ok();
    unix((number(0, 4)?, number(4, 2)?, number(6, 2)?), (number(8, 2)?, number(10, 2)?, number(12, 2)?))
}

fn unix((year, month, day): (i64, i64, i64), (hour, minute, second): (i64, i64, i64)) -> Option<u32> {
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    // Days since 1970 of a Gregorian date (Howard Hinnant's `days_from_civil`).
    let year = if month <= 2 { year - 1 } else { year };
    let (era, of_era) = (year.div_euclid(400), year.rem_euclid(400));
    let of_year = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let days = era * 146_097 + of_era * 365 + of_era / 4 - of_era / 100 + of_year - 719_468;
    u32::try_from(days * 86_400 + hour * 3600 + minute * 60 + second).ok()
}

/// The last write time, read without opening the file: a package folder may refuse that.
fn modified(path: &Path) -> Option<u32> {
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
    // SAFETY: plain data, zeroed.
    let mut data: WIN32_FILE_ATTRIBUTE_DATA = unsafe { mem::zeroed() };
    // SAFETY: the name is NUL-terminated and `data` is the structure this level writes.
    if unsafe { GetFileAttributesExW(wide.as_ptr(), GetFileExInfoStandard, (&raw mut data).cast()) } == 0 {
        return None;
    }
    let ticks = (u64::from(data.ftLastWriteTime.dwHighDateTime) << 32) | u64::from(data.ftLastWriteTime.dwLowDateTime);
    u32::try_from((ticks / 10_000_000).checked_sub(SECS_1601_TO_1970)?).ok()
}

/// A decoded PKCS #7 message, closed on drop.
struct Message(*mut c_void);

impl Message {
    fn decode(der: &[u8]) -> Option<Self> {
        // SAFETY: plain call; the handle is owned by the guard.
        let handle = unsafe { CryptMsgOpenToDecode(X509_ASN_ENCODING | PKCS_7_ASN_ENCODING, 0, 0, 0, ptr::null(), ptr::null()) };
        if handle.is_null() {
            return None;
        }
        let message = Self(handle);
        let len = u32::try_from(der.len()).ok()?;
        // SAFETY: `der` is valid for `len` bytes, the last and only chunk.
        (unsafe { CryptMsgUpdate(message.0, der.as_ptr(), len, 1) } != 0).then_some(message)
    }

    /// The parameter and its length in bytes, in a buffer aligned for the structures some parameters are.
    fn param(&self, kind: u32) -> Option<(Vec<u64>, usize)> {
        let mut len = 0;
        // SAFETY: a size query: no buffer.
        if unsafe { CryptMsgGetParam(self.0, kind, 0, ptr::null_mut(), &mut len) } == 0 {
            return None;
        }
        let mut buffer = vec![0u64; (len as usize).div_ceil(8)];
        // SAFETY: the buffer holds `len` bytes.
        (unsafe { CryptMsgGetParam(self.0, kind, 0, buffer.as_mut_ptr().cast(), &mut len) } != 0).then_some((buffer, len as usize))
    }
}

impl Drop for Message {
    fn drop(&mut self) {
        // SAFETY: the handle is owned and closed once.
        unsafe { CryptMsgClose(self.0) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_archive_gives_its_first_entry_date_in_utc() {
        let mut head = *b"PK\x03\x04\0\0\0\0\0\0\0\0\0\0";
        // 2021-03-04 05:06:08
        head[10..12].copy_from_slice(&((5u16 << 11) | (6 << 5) | 4).to_le_bytes());
        head[12..14].copy_from_slice(&((41u16 << 9) | (3 << 5) | 4).to_le_bytes());
        assert_eq!(zip_date(&head), Some(1_614_834_368));
        assert_eq!(zip_date(&[0; 14]), None);
    }

    #[test]
    fn a_generalized_time_is_read_in_whole_seconds() {
        let der = [&[0x18, 0x08][..], b"20240102", &[0x18, 19], b"20240102030405.123Z"].concat();
        assert_eq!(generalized_time(&der), Some(1_704_164_645), "the short one is no time");
        assert_eq!(generalized_time(&[&[0x18, 15][..], b"20240102030405Z"].concat()), None, "no fraction: not a time");
        assert_eq!(generalized_time(&[0x18, 40]), None, "a length past the end");
    }

    /// Every Windows has signed system packages; their times differ by build, so only the range is checked here.
    #[test]
    fn a_package_signature_gives_its_countersigned_time() {
        let mut packages = fs::read_dir(r"C:\Windows\SystemApps").unwrap().flatten();
        let signature = packages.find_map(|package| fs::read(package.path().join("AppxSignature.p7x")).ok()).expect("a package signature");
        let time = signing_time(&signature);
        assert!(time.is_some_and(|time| (1_420_070_400..4_000_000_000).contains(&time)), "{time:?}");
    }

    #[test]
    fn an_unreadable_package_file_gives_its_write_time() {
        let dir = std::env::temp_dir().join(format!("impulse-version-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("game.exe"), b"not a PE").unwrap();
        assert_eq!(build(&dir, "game.exe", false), None);
        assert!(build(&dir, "game.exe", true).is_some_and(|time| time > 1_700_000_000));
        fs::write(dir.join("AppxManifest.xml"), b"<Package/>").unwrap();
        assert!(build(&dir, "AppxSignature.p7x", true).is_some(), "no signature: the manifest's time");
        fs::remove_dir_all(&dir).unwrap();
    }
}
