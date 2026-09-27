//! mithril-gpu: CUDA emission + CUDA driver-API wave runner.
//!
//! [`emit_cuda`] compiles a [`CoreModule`](mithril_front::core::CoreModule)
//! into a `program.cu` that `#include`s the fixed device engine
//! (`cuda/engine.cu`, embedded in this crate). [`compile_and_run`] builds it
//! with docker nvcc (cached by source hash), loads the .cubin through the
//! CUDA driver API and drives the host wave loop; the boot redex fires as
//! rule 0 and the value delivered to ROOT (record 0) is returned raw.
//!
//! Capacities: `MITHRIL_GPU_NODES` (cells, default 2^28), `MITHRIL_GPU_RECS`
//! (records, default 2^24), `MITHRIL_GPU_BUCKET` (per-rule bucket entries,
//! default 2^20). Exhausting any of them yields a clean
//! `Err("... arena exhausted ...")`, never a CUDA illegal access.

mod emit;
mod runner;

pub use emit::emit_cuda;
pub use runner::{compile_and_run, GpuRunner};

/// The fixed device-side engine source (written next to the generated
/// program.cu so its `#include "engine.cu"` resolves).
pub const ENGINE_CU: &str = runner::ENGINE_CU;
