//! What an Impulse plugin builds against: the v1 wire, the manifest's format and the guest's side of the ABI.
//!
//! A plugin is a core WASM module (`wasm32-unknown-unknown`) with a `manifest.json` beside it. It implements
//! [`guest::Provider`] and calls [`export!`]; the host runs one instance per call and reaches nothing else of it.

pub mod guest;
pub mod manifest;
pub mod wire;

/// The wire's version: a guest states it before the host decodes anything of it.
pub const API: u32 = 1;
