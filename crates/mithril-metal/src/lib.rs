//! mithril-metal: the Apple GPU backend.
//!
//! [`metal`] drives Metal through the Objective-C runtime (libobjc and
//! Metal.framework, no other dependency): compile MSL from source, build
//! pipelines, allocate shared-storage buffers (the GPU and the CPU share
//! memory, so results are read in place) and run dispatches.
//!
//! The device's scalar operations (`msl/ops.metal`) give the bits every
//! other backend gives: ints wrap as the CPU's i64; binary32 runs in
//! hardware on normal values and through the software binary64 of
//! `mithril-core/device/soft64.h` where a subnormal is involved (the GPU
//! flushes them); binary16 converts in integer arithmetic; binary64 is
//! software. The crate is empty on other systems.

#[cfg(target_os = "macos")]
pub mod metal;

/// The device's scalar operations with the software float header in front:
/// the start of every program this backend compiles.
pub fn ops_source() -> String {
    [include_str!("../../mithril-core/device/soft64.h"), include_str!("../msl/ops.metal")].join("\n")
}
