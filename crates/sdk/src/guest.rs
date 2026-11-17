//! The guest's side: implement [`Provider`], then `sdk::export!(Type);` at the crate's root.
//!
//! The ABI it emits, imports from module `impulse`: `impulse_api() -> i32` (asked first), `impulse_alloc(len) -> ptr`
//! (the host writes the request there), `impulse_call(ptr, len) -> i64` (the request in; the reply's
//! `ptr << 32 | len` out). Host imports: `http_get(ptr, len) -> i64` (a pending result of `n` bytes: the body when
//! `n >= 0`, else a `Reply` error of `-n` bytes), `take(ptr)` (copies the pending result out and drops it; a trap
//! when none is pending), `log(ptr, len)`. An instance lives for one call: nothing here is freed.

use serde::Serialize;
use serde_json::{Map as Object, Value};

use crate::wire::{Call, Error, ErrorKind, HttpRequest, Map, MapOffer, Point, Reply, Request, Title};

/// What every call is given.
pub struct Context {
    /// One of the manifest's `locales`, the closest to the user's.
    pub locale: String,
    /// Each manifest field's value, checked against it.
    pub settings: Object<String, Value>,
}

/// A maps provider. A call it does not answer is `unsupported`.
pub trait Provider {
    fn titles(_: &Context) -> Result<Vec<Title>, Error> {
        Err(unsupported())
    }

    fn maps(_: &Context, _title: &str) -> Result<Vec<MapOffer>, Error> {
        Err(unsupported())
    }

    fn map(_: &Context, _map: &str) -> Result<Map, Error> {
        Err(unsupported())
    }
}

fn unsupported() -> Error {
    Error::new(ErrorKind::Unsupported, "not served")
}

/// Emits the module's exports for a [`Provider`].
#[macro_export]
macro_rules! export {
    ($provider:ty) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn impulse_api() -> u32 {
            $crate::API
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn impulse_alloc(len: u32) -> u32 {
            $crate::guest::alloc(len)
        }

        /// # Safety
        /// Only the host calls it, with `len` bytes at `ptr` from `impulse_alloc`.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn impulse_call(ptr: u32, len: u32) -> u64 {
            // SAFETY: the caller holds `call`'s contract.
            unsafe { $crate::guest::call::<$provider>(ptr, len) }
        }
    };
}

#[doc(hidden)]
pub fn alloc(len: u32) -> u32 {
    Box::leak(vec![0u8; len as usize].into_boxed_slice()).as_mut_ptr() as usize as u32
}

/// # Safety
/// `len` bytes at `ptr` are readable and stay so: memory `alloc` gave, which nothing frees.
#[doc(hidden)]
pub unsafe fn call<P: Provider>(ptr: u32, len: u32) -> u64 {
    // SAFETY: the caller's contract above.
    let bytes = unsafe { std::slice::from_raw_parts(ptr as usize as *const u8, len as usize) };
    let reply = match serde_json::from_slice::<Request>(bytes) {
        Ok(request) => answer::<P>(request),
        Err(e) => encode(&Reply::<()>::Error(Error::new(ErrorKind::Internal, format!("request: {e}")))),
    };
    let reply = Box::leak(reply.into_boxed_slice());
    ((reply.as_ptr() as usize as u64) << 32) | reply.len() as u64
}

fn answer<P: Provider>(request: Request) -> Vec<u8> {
    let context = Context { locale: request.locale, settings: request.settings };
    match request.call {
        Call::Titles => encode(&Reply::from(P::titles(&context))),
        Call::Maps { title } => encode(&Reply::from(P::maps(&context, &title))),
        Call::Map { map } => encode(&Reply::from(P::map(&context, &map))),
        Call::Unknown => encode(&Reply::<()>::Error(unsupported())),
    }
}

fn encode<T: Serialize>(reply: &Reply<T>) -> Vec<u8> {
    serde_json::to_vec(reply).unwrap_or_else(|e| format!(r#"{{"error":{{"kind":"internal","message":"reply: {e}"}}}}"#).into_bytes())
}

/// A GET on a host of the manifest's `hosts`, over https; the body of a 2xx answer. The host follows redirects,
/// and refuses a host outside the manifest or a private address.
pub fn http_get(url: &str) -> Result<Vec<u8>, Error> {
    let request = serde_json::to_vec(&HttpRequest { url: url.to_owned() }).map_err(|e| Error::new(ErrorKind::Internal, e.to_string()))?;
    let length = host::http_get(&request)?;
    let mut out = vec![0; length.unsigned_abs() as usize];
    host::take(&mut out);
    if length >= 0 {
        return Ok(out);
    }
    match serde_json::from_slice::<Reply<()>>(&out) {
        Ok(Reply::Error(error)) => Err(error),
        _ => Err(Error::new(ErrorKind::Internal, "an unreadable host error")),
    }
}

/// One line in the host's log, prefixed with the plugin; the host keeps a few per call.
pub fn log(message: &str) {
    host::log(message.as_bytes());
}

/// A Web Mercator point (degrees) on the unit square.
pub fn mercator(lat: f64, lng: f64) -> Point {
    let lat = lat.to_radians();
    Point { x: (lng + 180.) / 360., y: (1. - (lat.tan() + 1. / lat.cos()).ln() / std::f64::consts::PI) / 2. }
}

#[cfg(target_arch = "wasm32")]
mod host {
    use crate::wire::Error;

    #[link(wasm_import_module = "impulse")]
    unsafe extern "C" {
        #[link_name = "http_get"]
        fn get(ptr: *const u8, len: usize) -> i64;
        #[link_name = "take"]
        fn take_into(ptr: *mut u8);
        #[link_name = "log"]
        fn write(ptr: *const u8, len: usize);
    }

    pub fn http_get(request: &[u8]) -> Result<i64, Error> {
        // SAFETY: the host reads `request` within this call only.
        Ok(unsafe { get(request.as_ptr(), request.len()) })
    }

    pub fn take(out: &mut [u8]) {
        // SAFETY: `out` is as long as the pending result the host reported.
        unsafe { take_into(out.as_mut_ptr()) }
    }

    pub fn log(message: &[u8]) {
        // SAFETY: the host reads `message` within this call only.
        unsafe { write(message.as_ptr(), message.len()) }
    }
}

/// Outside a module there is no host: a provider's logic still runs in native tests.
#[cfg(not(target_arch = "wasm32"))]
mod host {
    use crate::wire::{Error, ErrorKind};

    pub fn http_get(_: &[u8]) -> Result<i64, Error> {
        Err(Error::new(ErrorKind::Unsupported, "no host outside a plugin"))
    }

    pub fn take(_: &mut [u8]) {}

    pub fn log(_: &[u8]) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mercator_puts_the_equator_and_the_meridian_in_the_middle() {
        let middle = mercator(0., 0.);
        assert!((middle.x - 0.5).abs() < 1e-12 && (middle.y - 0.5).abs() < 1e-12);
        let north_west = mercator(85.051_128_779_806_59, -180.);
        assert!(north_west.x.abs() < 1e-12 && north_west.y.abs() < 1e-9, "{north_west:?}");
    }

    struct Maps;

    impl Provider for Maps {
        fn titles(context: &Context) -> Result<Vec<Title>, Error> {
            Ok(vec![Title { id: "t".into(), name: context.locale.clone(), stores: Vec::new() }])
        }
    }

    #[test]
    fn a_request_is_answered_through_the_provider_and_an_unknown_call_is_unsupported() {
        let ask = |call: &str| {
            let request = format!(r#"{{"call": {call}, "locale": "fr"}}"#);
            serde_json::from_slice::<Value>(&answer::<Maps>(serde_json::from_str(&request).unwrap())).unwrap()
        };
        assert_eq!(ask(r#"{"kind": "titles"}"#)["ok"][0]["name"], "fr");
        assert_eq!(ask(r#"{"kind": "map", "map": "m"}"#)["error"]["kind"], "unsupported");
        assert_eq!(ask(r#"{"kind": "heatmap"}"#)["error"]["kind"], "unsupported");
    }
}
