//! Companion foundation. The state coordinator is introduced after M4/M5.
//! Native media, audio and Bluetooth behavior remains in Rust in later milestones.

/// Build version surfaced by the CLI without initializing any native service.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
