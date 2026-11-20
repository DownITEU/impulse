//! One call into a plugin's module: a fresh instance with its limits, the host's imports, the reply read
//! back. Blocking: run it off the UI thread. Past a limit, the call fails as `Refused`.

#[cfg(test)]
mod tests;

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, ensure};
use model::{Failure, FailureKind, SourceId};
use sdk::wire::{self, HttpRequest, Reply};
use serde::de::DeserializeOwned;
use wasmi::{
    Caller, CompilationMode, Config, Engine, ExternType, FuncType, Linker, Memory, Module, Store, StoreLimits, StoreLimitsBuilder,
    TrapCode, ValType,
};

const MEMORY: usize = 64 << 20;
// About 8 times what parsing a 1.4 MB map takes.
const FUEL: u64 = 2_000_000_000;
/// The largest reply, and the largest body a request brings back.
pub const ANSWER: usize = 16 << 20;
const REQUESTS: usize = 64;
// Fuel does not run while the guest waits on the host: the wall clock bounds those waits.
const DEADLINE: Duration = Duration::from_secs(60);
const REQUEST: usize = 8 << 10;
const LOG_LINES: usize = 32;
const LOG_LINE: usize = 1 << 10;
// Far past a Rust guest's indirect calls; each element is host memory.
const TABLE: usize = 1 << 16;

/// A GET for the guest, or for a download: the body of a 2xx answer (at most the byte limit) within the time left, or
/// the error it sees. Arguments: the URL, the byte limit, the time left.
pub type Net = Arc<dyn Fn(&str, u64, Duration) -> Result<Vec<u8>, wire::Error> + Send + Sync>;

/// A plugin's module, compiled once; every call gets its own instance.
pub struct Plugin {
    source: SourceId,
    module: Module,
    linker: Linker<Host>,
}

struct Host {
    limits: StoreLimits,
    net: Net,
    source: SourceId,
    deadline: Instant,
    // Set once the guest has said which wire it speaks: imports work only then. Kept here, not looked up through
    // `Caller::get_export`, which reads a dangling instance in wasmi 2.0.1 (an access violation).
    memory: Option<Memory>,
    pending: Option<Vec<u8>>,
    requests: usize,
    logged: usize,
}

/// Whether `wasm` validates in full and exports what `run` looks up (nothing of it runs), so an install can refuse a
/// module that would never answer.
pub fn check(wasm: &[u8]) -> Result<()> {
    let module = compile(wasm, CompilationMode::Eager)?;
    let func = |name: &str, params: &[ValType], results: &[ValType]| {
        let wanted = FuncType::new(params.iter().copied(), results.iter().copied());
        match module.get_export(name) {
            Some(ExternType::Func(found)) if found == wanted => Ok(()),
            _ => Err(refused(format!("the module does not export the function {name} the host calls"))),
        }
    };
    func("impulse_api", &[], &[ValType::I32])?;
    func("impulse_alloc", &[ValType::I32], &[ValType::I32])?;
    func("impulse_call", &[ValType::I32, ValType::I32], &[ValType::I64])?;
    match module.get_export("memory") {
        Some(ExternType::Memory(_)) => Ok(()),
        _ => Err(refused("the module does not export its memory".to_owned())),
    }
}

fn refused(detail: String) -> anyhow::Error {
    anyhow::anyhow!(detail).context(Failure::new(FailureKind::Refused))
}

fn compile(wasm: &[u8], mode: CompilationMode) -> Result<Module> {
    let mut config = Config::default();
    config.compilation_mode(mode).consume_fuel(true);
    let engine = Engine::new(&config);
    Module::new(&engine, wasm).context("not a module").context(Failure::new(FailureKind::Refused))
}

impl Plugin {
    pub fn new(source: SourceId, wasm: &[u8]) -> Result<Self> {
        let module = compile(wasm, CompilationMode::Lazy)?;
        let mut linker = Linker::new(module.engine());
        linker.func_wrap("impulse", "http_get", http_get)?;
        linker.func_wrap("impulse", "take", take)?;
        linker.func_wrap("impulse", "log", log)?;
        Ok(Self { source, module, linker })
    }

    /// Asks the guest; its reply, decoded.
    pub fn call<T: DeserializeOwned>(&self, request: &wire::Request, net: Net) -> Result<T> {
        let refused = || Failure::new(FailureKind::Refused);
        let reply = self.run(request, net).with_context(|| self.source.as_str().to_owned()).context(refused())?;
        let reply: Reply<T> = serde_json::from_slice(&reply)
            .with_context(|| format!("{}: a reply that does not read", self.source.as_str()))
            .context(refused())?;
        Result::from(reply).map_err(|error| guest(&self.source, error))
    }

    fn run(&self, request: &wire::Request, net: Net) -> Result<Vec<u8>> {
        let limits = StoreLimitsBuilder::new()
            .memory_size(MEMORY)
            .memories(1)
            .table_elements(TABLE)
            .tables(1)
            .instances(1)
            .trap_on_grow_failure(true)
            .build();
        let deadline = Instant::now() + DEADLINE;
        let host = Host { limits, net, source: self.source.clone(), deadline, memory: None, pending: None, requests: 0, logged: 0 };
        let mut store = Store::new(self.module.engine(), host);
        store.limiter(|host| &mut host.limits);
        store.set_fuel(FUEL)?;
        let instance = self.linker.instantiate_and_start(&mut store, &self.module).map_err(trap)?;
        let api =
            instance.get_typed_func::<(), u32>(&store, "impulse_api").context("no impulse_api")?.call(&mut store, ()).map_err(trap)?;
        ensure!(api == sdk::API, "speaks wire v{api}, this app v{}", sdk::API);
        let memory = instance.get_memory(&store, "memory").context("no memory")?;
        store.data_mut().memory = Some(memory);
        let alloc = instance.get_typed_func::<u32, u32>(&store, "impulse_alloc").context("no impulse_alloc")?;
        let call = instance.get_typed_func::<(u32, u32), u64>(&store, "impulse_call").context("no impulse_call")?;
        let request = serde_json::to_vec(request)?;
        let at = alloc.call(&mut store, request.len() as u32).map_err(trap)?;
        memory.write(&mut store, at as usize, &request).context("request out of the guest's memory")?;
        let packed = call.call(&mut store, (at, request.len() as u32)).map_err(trap)?;
        ensure!(Instant::now() <= deadline, "past the call's {} s", DEADLINE.as_secs());
        let (at, len) = ((packed >> 32) as usize, (packed & 0xFFFF_FFFF) as usize);
        ensure!(len <= ANSWER, "a reply of {len} bytes, past {ANSWER}");
        let mut reply = vec![0; len];
        memory.read(&store, at, &mut reply).context("reply out of the guest's memory")?;
        Ok(reply)
    }
}

/// A guest's or the network's error as the app words it: the kind is the `Failure`'s.
pub fn guest(source: &SourceId, error: wire::Error) -> anyhow::Error {
    let kind = match error.kind {
        wire::ErrorKind::NotFound => FailureKind::NotFound,
        wire::ErrorKind::Offline => FailureKind::Offline,
        wire::ErrorKind::Refused => FailureKind::Refused,
        wire::ErrorKind::Unsupported => FailureKind::Unsupported,
        wire::ErrorKind::Internal => FailureKind::Internal,
    };
    anyhow::anyhow!("{}: {}", source.as_str(), line(error.message.as_bytes())).context(Failure::new(kind))
}

/// A guest's text as one bounded log line.
fn line(text: &[u8]) -> std::borrow::Cow<'_, str> {
    String::from_utf8_lossy(&text[..text.len().min(LOG_LINE)])
}

fn trap(error: wasmi::Error) -> anyhow::Error {
    match error.as_trap_code() {
        Some(TrapCode::OutOfFuel) => anyhow::anyhow!("out of fuel: past {FUEL}"),
        Some(TrapCode::GrowthOperationLimited) => anyhow::anyhow!("past {MEMORY} bytes of memory or {TABLE} table elements"),
        _ => anyhow::anyhow!("trapped: {error}"),
    }
}

/// The guest's memory and the host's state; a trap before the guest stated its wire.
fn parts<'a>(caller: &'a mut Caller<'_, Host>) -> Result<(&'a mut [u8], &'a mut Host), wasmi::Error> {
    let memory = caller.data().memory.ok_or_else(|| wasmi::Error::new("an import called before impulse_api"))?;
    Ok(memory.data_and_store_mut(caller))
}

fn http_get(mut caller: Caller<'_, Host>, at: u32, len: u32) -> Result<i64, wasmi::Error> {
    let (bytes, host) = parts(&mut caller)?;
    if host.requests >= REQUESTS || len as usize > REQUEST {
        return Err(wasmi::Error::new(format!("a request past the limits ({REQUESTS} per call, {REQUEST} bytes)")));
    }
    let left = host.deadline.saturating_duration_since(Instant::now());
    if left.is_zero() {
        return Err(wasmi::Error::new(format!("past the call's {} s", DEADLINE.as_secs())));
    }
    host.requests += 1;
    let request = bytes.get(at as usize..at as usize + len as usize).ok_or_else(|| wasmi::Error::new("request out of memory"))?;
    let outcome = match serde_json::from_slice::<HttpRequest>(request) {
        Ok(request) => (host.net)(&request.url, ANSWER as u64, left),
        Err(e) => Err(wire::Error::new(wire::ErrorKind::Internal, format!("request: {e}"))),
    };
    let (pending, length) = match outcome {
        Ok(body) => {
            let length = body.len() as i64;
            (body, length)
        }
        Err(error) => {
            let reply = serde_json::to_vec(&Reply::<()>::Error(error)).map_err(|e| wasmi::Error::new(e.to_string()))?;
            let length = -(reply.len() as i64);
            (reply, length)
        }
    };
    host.pending = Some(pending);
    Ok(length)
}

fn take(mut caller: Caller<'_, Host>, at: u32) -> Result<(), wasmi::Error> {
    let (bytes, host) = parts(&mut caller)?;
    let pending = host.pending.take().ok_or_else(|| wasmi::Error::new("take with nothing pending"))?;
    let into = bytes.get_mut(at as usize..at as usize + pending.len()).ok_or_else(|| wasmi::Error::new("take out of memory"))?;
    into.copy_from_slice(&pending);
    Ok(())
}

fn log(mut caller: Caller<'_, Host>, at: u32, len: u32) -> Result<(), wasmi::Error> {
    let (bytes, host) = parts(&mut caller)?;
    if host.logged < LOG_LINES {
        host.logged += 1;
        let text = bytes.get(at as usize..at as usize + len as usize).ok_or_else(|| wasmi::Error::new("log out of memory"))?;
        log::debug!("{}: {}", host.source.as_str(), line(text));
    }
    Ok(())
}
