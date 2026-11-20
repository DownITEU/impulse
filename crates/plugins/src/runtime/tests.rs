use std::sync::Arc;

use model::{Failure, FailureKind, SourceId};
use sdk::wire;

use super::{Net, Plugin};

const REQUEST: &str = r#"{"url":"https://a.tiles.io/x"}"#;

/// A guest in WAT: its reply `{"ok":7}` sits at 0, a request for `REQUEST` at 100; `call` is `impulse_call`'s body.
fn guest(api: u32, call: &str, extra: &str) -> Vec<u8> {
    let wat = format!(
        r#"(module
            (import "impulse" "http_get" (func $get (param i32 i32) (result i64)))
            (import "impulse" "take" (func $take (param i32)))
            (import "impulse" "log" (func $log (param i32 i32)))
            (memory (export "memory") 1)
            (data (i32.const 0) "{{\"ok\":7}}")
            (data (i32.const 100) "{request}")
            (func (export "impulse_api") (result i32) i32.const {api})
            (func (export "impulse_alloc") (param i32) (result i32) i32.const 4096)
            (func (export "impulse_call") (param i32 i32) (result i64) (local $n i64) {call})
            {extra})"#,
        request = REQUEST.replace('"', "\\\""),
    );
    wat::parse_str(wat).unwrap()
}

fn net() -> Net {
    Arc::new(|url, _, _| match url {
        "https://a.tiles.io/x" => Ok(br#"{"ok":7}"#.to_vec()),
        _ => Err(wire::Error::new(wire::ErrorKind::Refused, "outside")),
    })
}

fn ask(wasm: &[u8]) -> Result<u32, Failure> {
    let request = wire::Request { call: wire::Call::Titles, locale: "en".into(), settings: Default::default() };
    Ok(Plugin::new(SourceId::new("plugin-t").unwrap(), wasm)?.call(&request, net())?)
}

#[test]
fn a_reply_comes_back_and_a_request_goes_through_the_host() {
    assert_eq!(ask(&guest(1, "i64.const 8", "")).unwrap(), 7);
    let fetched = format!(
        "(local.set $n (call $get (i32.const 100) (i32.const {}))) (call $take (i32.const 2048)) \
         (i64.or (i64.shl (i64.const 2048) (i64.const 32)) (local.get $n))",
        REQUEST.len()
    );
    assert_eq!(ask(&guest(1, &fetched, "")).unwrap(), 7);
}

fn requesting(n: usize) -> Vec<u8> {
    let call = format!(
        "(block $done (loop $again (br_if $done (i64.eq (local.get $n) (i64.const {n}))) \
         (drop (call $get (i32.const 100) (i32.const {}))) (local.set $n (i64.add (local.get $n) (i64.const 1))) (br $again))) \
         i64.const 8",
        REQUEST.len()
    );
    guest(1, &call, "")
}

#[test]
fn check_validates_every_body_and_wants_the_abi_exports() {
    super::check(&guest(1, "i64.const 8", "")).unwrap();
    let malformed = guest(1, "i64.const 8", "(func $bad (result i32) i64.const 1)");
    let wrong_type = wat::parse_str(r#"(module (memory (export "memory") 1) (func (export "impulse_api")))"#).unwrap();
    for (why, wasm) in [
        ("a body that does not validate", malformed),
        ("an empty module", wat::parse_str("(module)").unwrap()),
        ("a wrong type", wrong_type),
    ] {
        assert_eq!(super::check(&wasm).unwrap_err().downcast_ref::<Failure>().map(|f| f.kind), Some(FailureKind::Refused), "{why}");
    }
}

#[test]
fn a_call_makes_up_to_64_requests() {
    assert_eq!(ask(&requesting(64)).unwrap(), 7);
}

#[test]
fn a_guest_past_its_limits_or_off_the_abi_is_refused() {
    let without_call = r#"(module (memory (export "memory") 1) (func (export "impulse_api") (result i32) i32.const 1)
        (func (export "impulse_alloc") (param i32) (result i32) i32.const 0))"#;
    let cases = [
        ("out of fuel", guest(1, "(loop $again (br $again)) i64.const 8", "")),
        ("of memory", guest(1, "(drop (memory.grow (i32.const 2000))) i64.const 8", "")),
        ("too many linear memories", guest(1, "i64.const 8", "(memory 1)")),
        ("failed to instantiate table", guest(1, "i64.const 8", "(table 100000 funcref)")),
        ("too many tables", guest(1, "i64.const 8", "(table 1 funcref) (table 1 funcref)")),
        ("a reply of 17000000 bytes", guest(1, "i64.const 17000000", "")),
        ("speaks wire v2", guest(2, "i64.const 8", "")),
        ("nothing pending", guest(1, "(call $take (i32.const 2048)) i64.const 8", "")),
        ("before impulse_api", guest(1, "i64.const 8", "(func $early (call $log (i32.const 0) (i32.const 1))) (start $early)")),
        ("a request past the limits", requesting(65)),
        ("does not read", guest(1, "i64.const 4", "")),
        ("no impulse_call", wat::parse_str(without_call).unwrap()),
        ("not a module", b"not wasm".to_vec()),
    ];
    for (why, wasm) in cases {
        let failure = ask(&wasm).unwrap_err();
        assert_eq!(failure.kind, FailureKind::Refused, "{why}");
        assert!(failure.detail.contains(why), "{why}: {}", failure.detail);
    }
}
