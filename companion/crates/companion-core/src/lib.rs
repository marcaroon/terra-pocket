//! Companion core: cross-domain coordination between the Windows media session,
//! the BLE link and (later) system audio. Native APIs stay in their crates;
//! frontends (CLI now, Tauri in M10) only call into this crate.

pub mod sync;

#[cfg(windows)]
pub mod session;

/// Build version surfaced by the CLI without initializing any native service.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Companion version as (major, minor, patch) for HELLO_ACK.
pub fn version_triple() -> (u8, u8, u8) {
    let mut parts = VERSION.split('.').map(|p| p.parse::<u8>().unwrap_or(0));
    (parts.next().unwrap_or(0), parts.next().unwrap_or(0), parts.next().unwrap_or(0))
}
