//! mithril-gpu: the CUDA backend of the lowered IR + a CUDA driver-API
//! wave runner.
//!
//! [`emit_cuda`] lowers a specialized [`CoreModule`](mithril_front::core::CoreModule)
//! with `mithril_codegen::lower` (the same lowering the CPU program is
//! printed from) and prints it as `program.cu`, which `#include`s the fixed
//! device runtime (`cuda/engine.cu`, embedded in this crate).
//! [`compile_and_run`] builds it with docker nvcc (cached by source hash),
//! loads the .cubin through the CUDA driver API and drives the host wave
//! loop; the boot redex fires as rule 0 and the value delivered to ROOT
//! (record 0) is returned raw and printed like the CPU program prints it.
//!
//! Capacities: `MITHRIL_GPU_NODES` (cells, default 2^28), `MITHRIL_GPU_RECS`
//! (records, default 2^24), `MITHRIL_GPU_BUCKET` (per-rule bucket entries),
//! `MITHRIL_GPU_HEAP` (array words, default 2^26), `MITHRIL_GPU_FUEL` (per
//! dive, default 64), `MITHRIL_GPU_STACK` (device stack bytes, default
//! 32 KiB). Exhausting any of them yields a clean `Err`, never a CUDA
//! illegal access. The net region (closures) runs on the device in stage 3;
//! until then a program that reaches it fails with `Err("... closures ...")`.

mod cuda;
mod completion;
mod runner;

pub use runner::{compile_and_run, compile_to_cubin, free_vram, hold_context, run_cubin, run_cubin_to, ContextHold, GpuResult, GpuSession, GpuRunner, EXITING};

/// `program.cu` for a specialized module (see `mithril_codegen::lower`).
/// A program that reduced to a constant at compile time has nothing to
/// run: `Err(value)`.
pub fn emit_cuda(m: &mithril_front::core::CoreModule) -> Result<String, String> {
    let (prog, _) = mithril_codegen::lower(m);
    if let Some(v) = &prog.constant {
        return Err(mithril_codegen::fmt_val(v));
    }
    Ok(cuda::print(&prog))
}

/// The fixed device-side runtime source (written next to the generated
/// program.cu so its `#include "engine.cu"` resolves).
pub const ENGINE_CU: &str = runner::ENGINE_CU;
