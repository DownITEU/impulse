//! The Tophat messages: the `StreamMessage` envelope and the payload tables inside it.

use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use serde_json::Value;
use uuid::Uuid;

use super::wire::{Field, Table, read, table};

/// The `returnType` Wand sends with every command.
const RETURN_TYPE: u8 = 3;

/// What the service reports without being asked.
#[derive(Debug, PartialEq)]
pub enum Event {
    Initializing,
    Ready,
    Status(String),
    Log(String),
    /// The trainer's current value of a variable.
    Value {
        id: u64,
        name: String,
        value: f64,
    },
    Buffer {
        kind: String,
        data: Vec<u8>,
    },
}

pub enum Incoming<'a> {
    Reply {
        seq: u64,
        payload: &'a [u8],
    },
    /// The trainer's verdict on a pushed value: 0 set, 1 changed, 2 failed.
    Applied {
        id: u64,
        code: u64,
    },
    Event(Event),
    Unknown(String),
}

/// Parses a message from the service; `None` when it is malformed.
pub fn incoming(raw: &[u8]) -> Option<Incoming<'_>> {
    let message = Table::root(raw)?;
    let payload = message.bytes(1);
    let kind = message.text(0);
    Some(match kind.as_str() {
        "command_response" => Incoming::Reply { seq: message.u64(2), payload },
        "trainer_value_response" => Incoming::Applied { id: u64_at(payload, 8)?, code: u64_at(payload, 16)? },
        "trainer_initializing" => Incoming::Event(Event::Initializing),
        "trainer_ready" => Incoming::Event(Event::Ready),
        "trainer_status" => Incoming::Event(Event::Status(text(payload))),
        "trainer_log" => Incoming::Event(Event::Log(text(payload))),
        "trainer_value_request" => Incoming::Event(value(payload)?),
        "buffer_message" => Incoming::Event(buffer(payload)?),
        _ => Incoming::Unknown(kind),
    })
}

fn u64_at(payload: &[u8], at: usize) -> Option<u64> {
    read(payload, at).map(u64::from_le_bytes)
}

fn text(payload: &[u8]) -> String {
    String::from_utf8_lossy(payload).trim_end_matches('\0').to_owned()
}

// id u64 @8, name ascii[32] @16, value f64 @48.
fn value(payload: &[u8]) -> Option<Event> {
    let name = payload.get(16..48)?;
    let end = name.iter().position(|byte| *byte == 0).unwrap_or(name.len());
    Some(Event::Value {
        id: u64_at(payload, 8)?,
        name: String::from_utf8_lossy(&name[..end]).into_owned(),
        value: f64::from_le_bytes(read(payload, 48)?),
    })
}

// kind length u32, kind, data length u32, data.
fn buffer(payload: &[u8]) -> Option<Event> {
    let kind_len = u32::from_le_bytes(read(payload, 0)?) as usize;
    let rest = payload.get(4..)?;
    let kind = rest.get(..kind_len)?;
    let rest = rest.get(kind_len..)?;
    let data_len = u32::from_le_bytes(read(rest, 0)?) as usize;
    Some(Event::Buffer { kind: String::from_utf8_lossy(kind).into_owned(), data: rest.get(4..)?.get(..data_len)?.to_vec() })
}

/// A request the service must answer with the same `seq`.
pub fn request(kind: &str, payload: &[u8], seq: u64) -> Vec<u8> {
    table(&[
        (0, Field::Str(kind)),
        (1, Field::Bytes(payload)),
        (2, Field::U64(seq)),
        (3, Field::U64(now_ms())),
        (4, Field::Str("server")),
        (5, Field::Bool(true)),
    ])
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_millis() as u64)
}

pub fn heartbeat(count: u32) -> Vec<u8> {
    table(&[(0, Field::U64(now_ms())), (1, Field::U32(count))])
}

pub fn load_plugin(name: &str, path: &str) -> Vec<u8> {
    table(&[(0, Field::Str(name)), (1, Field::Str(path))])
}

pub fn unload_plugin(handle: u64) -> Vec<u8> {
    table(&[(0, Field::U64(handle))])
}

pub fn disconnect() -> Vec<u8> {
    table(&[])
}

/// `params` travel as JSON with a terminating NUL.
pub fn command(name: &str, params: Option<&Value>, client_id: &str, timeout_ms: u32) -> Vec<u8> {
    let params = params.map(|params| {
        let mut json = params.to_string().into_bytes();
        json.push(0);
        json
    });
    let request_id = Uuid::new_v4().to_string();
    let mut fields = vec![
        (0, Field::Str(name)),
        (2, Field::U8(RETURN_TYPE)),
        (3, Field::U32(timeout_ms)),
        (4, Field::Str(client_id)),
        (5, Field::Str(&request_id)),
    ];
    if let Some(json) = &params {
        fields.push((1, Field::Bytes(json)));
    }
    table(&fields)
}

/// A reply whose first field says whether the call worked and whose second is the error text.
pub fn checked(payload: &[u8]) -> Result<Table<'_>> {
    let reply = Table::root(payload).context("malformed reply")?;
    if !reply.bool(0) {
        let error = reply.text(1);
        bail!("{}", if error.is_empty() { "the service refused" } else { &error });
    }
    Ok(reply)
}

pub fn plugin_handle(payload: &[u8]) -> Result<u64> {
    Ok(checked(payload)?.u64(2))
}

/// A message as the service sends it, for the tests' fake service.
#[cfg(test)]
pub fn from_service(kind: &str, payload: &[u8], seq: u64) -> Vec<u8> {
    table(&[(0, Field::Str(kind)), (1, Field::Bytes(payload)), (2, Field::U64(seq)), (4, Field::Str("service"))])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(kind: &str, payload: &[u8]) -> Option<Event> {
        match incoming(&from_service(kind, payload, 0))? {
            Incoming::Event(event) => Some(event),
            _ => None,
        }
    }

    #[test]
    fn requests_carry_what_the_service_reads() {
        let raw = request("Heartbeat", b"body", 7);
        let message = Table::root(&raw).unwrap();
        assert_eq!(message.text(0), "Heartbeat");
        assert_eq!(message.bytes(1), b"body");
        assert_eq!((message.u64(2), message.text(4), message.bool(5)), (7, "server".into(), true));
        assert!(message.u64(3) > 0);
    }

    #[test]
    fn commands_carry_json_with_a_nul_and_the_client_id() {
        let raw = command("run", Some(&serde_json::json!({"a": 1})), "cid", 5000);
        let command = Table::root(&raw).unwrap();
        assert_eq!((command.text(0), command.text(4), command.u8(2)), ("run".into(), "cid".into(), 3));
        assert_eq!(command.bytes(1), b"{\"a\":1}\0");
        assert_eq!(command.text(5).len(), 36);
        assert!(Table::root(&super::command("stop", None, "cid", 1)).unwrap().bytes(1).is_empty());
    }

    #[test]
    fn events_are_parsed() {
        assert_eq!(parsed("trainer_ready", &[]), Some(Event::Ready));
        assert_eq!(parsed("trainer_initializing", &[]), Some(Event::Initializing));
        assert_eq!(parsed("trainer_status", b"loading\0"), Some(Event::Status("loading".into())));
        assert_eq!(parsed("trainer_log", b"hi"), Some(Event::Log("hi".into())));

        let mut value = vec![0u8; 56];
        value[8..16].copy_from_slice(&9u64.to_le_bytes());
        value[16..18].copy_from_slice(b"hp");
        value[48..].copy_from_slice(&2.5f64.to_le_bytes());
        assert_eq!(parsed("trainer_value_request", &value), Some(Event::Value { id: 9, name: "hp".into(), value: 2.5 }));

        let buffer = [&3u32.to_le_bytes()[..], b"abc", &2u32.to_le_bytes(), &[7, 8]].concat();
        assert_eq!(parsed("buffer_message", &buffer), Some(Event::Buffer { kind: "abc".into(), data: vec![7, 8] }));
    }

    #[test]
    fn verdicts_and_replies_keep_their_ids() {
        let mut verdict = vec![0u8; 24];
        verdict[8..16].copy_from_slice(&4u64.to_le_bytes());
        verdict[16..].copy_from_slice(&1u64.to_le_bytes());
        let raw = from_service("trainer_value_response", &verdict, 0);
        assert!(matches!(incoming(&raw), Some(Incoming::Applied { id: 4, code: 1 })));
        let raw = from_service("command_response", b"x", 12);
        assert!(matches!(incoming(&raw), Some(Incoming::Reply { seq: 12, payload: b"x" })));
        let raw = from_service("something_new", &[], 0);
        assert!(matches!(incoming(&raw), Some(Incoming::Unknown(kind)) if kind == "something_new"));
    }

    #[test]
    fn truncated_payloads_and_garbage_are_dropped() {
        assert!(incoming(&[0xff; 9]).is_none());
        assert!(parsed("trainer_value_request", &[0; 10]).is_none());
        assert!(parsed("buffer_message", &[0xff; 8]).is_none());
        let short = [&u32::MAX.to_le_bytes()[..], b"ab"].concat();
        assert!(parsed("buffer_message", &short).is_none());
        assert!(incoming(&from_service("trainer_value_response", &[0; 20], 0)).is_none());
    }

    #[test]
    fn replies_report_the_services_error() {
        assert!(checked(&table(&[(0, Field::Bool(true))])).is_ok());
        let refused = table(&[(0, Field::Bool(false)), (1, Field::Str("no such plugin"))]);
        assert_eq!(checked(&refused).unwrap_err().to_string(), "no such plugin");
        assert_eq!(checked(&table(&[])).unwrap_err().to_string(), "the service refused");
        assert!(checked(&[1, 2]).is_err());
        let loaded = table(&[(0, Field::Bool(true)), (2, Field::U64(7))]);
        assert_eq!(plugin_handle(&loaded).unwrap(), 7);
    }
}
