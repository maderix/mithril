# Mithril runtimes: CPU, CUDA and Metal

A Mithril program has one meaning, given by its interaction rules, and
three runtimes that execute it: the CPU runtime (`mithril-rt`), the CUDA
runtime (`mithril-gpu`) and the Metal runtime (`mithril-metal`, driven by
`mithril-rt`). This document describes how each one is built, side by
side, so that the parts they share stay shared and the parts that differ
can be brought together. It describes structure and the reasons for it;
measurements live in `dev_log.md`, and the rule-level design in
`design.md` (sections 6, 7 and 10).

Every runtime gives the same two guarantees: any order of rule firings
gives the same result (confluence), and every runtime produces the same
bits for ints, f16, f32, f64 and NaN (bit-exactness). Everything below is
scheduling, storage or speed; none of it may change a result.

## 1. The common model

### 1.1 What every runtime receives

The front end, the net specializer and the lowering run once, on the host,
and produce the residual program as LIR (`mithril-codegen::lir`). Each
runtime receives that LIR printed for its device:

| runtime | printer | output |
|---|---|---|
| CPU | the Rust printer (`emit_rust`) | a Rust program linked against `mithril-rt` |
| CUDA | the C-family printer, CUDA dialect (`cprint`, `mithril-gpu/src/cuda.rs`) | `program.cu`, which includes `engine.cu` |
| Metal | the C-family printer, MSL dialect (`cprint::msl_leaves`) | the range leaves, embedded in the CPU program |

The printed code names the runtime only through a fixed helper vocabulary
(`alloc2`, `deliver`, `dive_to`, `arr_get_r`, `f32_add`, ...). Each runtime
implements that vocabulary over its own storage. The vocabulary is the
seam between the program and the runtime.

### 1.2 Values

A value is a **port**: a 64-bit word whose top byte is a tag. Small ints
are immediates; an int past 56 bits is a refcounted box. A constructor
port points at a **cell** (fields), a closure at a net cell, an f64 at a
boxed cell, an array at a **block** `[refcount, length | flags,
elements...]`. f32 and f16 values travel as their bit patterns in ints.
All three runtimes use these encodings; where a device stores a block
differently (CUDA addresses arrays by heap index), the encoding of the
port, the block's fields and the element values stay the same.

### 1.3 Units of work

* **Redex**: a pending rule firing (a CALL, a segment, a fill, a net
  pair). Firing it runs the rule's printed code.
* **Record**: a continuation waiting for values. A delivery fills one of
  its slots; the delivery that completes it fires the record at once.
* **Dive**: a native run of a function under a budget. A dive that runs out
  of budget suspends: what remains becomes records and redexes, which other
  workers can take. The budget is scheduling, not meaning.
* **Range request**: a proven fold (an index fill or a wrapping sum,
  section 5.6.0 of `design.md`) handed to the runtime as an index range
  `[lo, hi)` with its arguments. Every index runs the fold's native loop
  over one index; the runtime may run the indices in any order, anywhere.
* **Net region**: closures and other values the net keeps at runtime,
  reduced by the rule engine.

### 1.4 Scalar operations

The meaning of every float operation is `mithril_core::float`. Each runtime
implements the same operations and is checked against it bit for bit:

| | ints | f32 | f16 | f64 | NaN |
|---|---|---|---|---|---|
| CPU | Rust wrapping ops | hardware (`mithril_core::float`) | integer conversions | hardware | canonical where observed |
| CUDA | C wrapping through `u64` | `__fadd_rn` and friends | integer conversions | `__dadd_rn` and friends | canonical where observed |
| Metal | MSL through `ulong` | hardware on normal values, software f64 when a subnormal is involved | integer conversions | software (`soft64.h`) | canonical where observed |

`mithril-core/device/soft64.h` is one text that compiles as C++ and MSL;
the host test runs the very code the device runs.

### 1.5 The result

Every run delivers one port to the root record. The runtime walks it once
and writes text, or bytes through the output sink (`mithril_core::sink`):
the leaves of the value in order, written in pieces that threads encode at
their own file offsets.

## 2. The CPU runtime

Files: `mithril-rt/src/engine.rs` (the coordinator and waves),
`worker.rs` (a worker's context), `alloc.rs` (arenas), `prelude.rs` (the
helper vocabulary), `sync.rs` (the atomics), `template.rs` (the program's
`main` and its rule table), `metal.rs` (the Metal hook, section 4).

### 2.1 Coordinator and waves

One coordinator thread runs the program. Each **wave** it merges every
worker's spawn buffers into per-rule buckets and picks one bucket by its
estimated work. A bucket with enough work is drained across the pool:
workers claim blocks of entries from one shared cursor. A small bucket is
drained by the coordinator alone, which keeps its caches warm. Workers are
spawned once per run and park between waves.

Per redex and per record the runtime uses only atomic counters: the join
counter, the wave's claim cursor and refcounts. The control plane (parking,
the wave's shared slices) takes a lock once per wave per worker.

### 2.2 Records, dives and budgets

The delivery that completes a record fires it at once, up to a fixed
nesting; deeper completions wait for the next wave. A dive runs a function
natively with a budget; on suspension the residue becomes records and
redexes. Sequential runs use one large budget (recursion depth on a
large stack is the bound). Parallel runs size the budget so that each
worker has a few entries per wave, because suspension exists to expose
work to idle workers; a sequential chain doubles its budget while the
frontier stays narrow.

### 2.3 Range requests

A range request is recorded with its join record and runs at the next
round as a **range wave**: the workers claim blocks of the request's
indices from one counter and run the fold's native loop over each block.
A small request runs on the coordinator. Completion releases the request's
ports and delivers to the join record's two slots: a fill's array twice, or
a sum and the incoming accumulator.

### 2.4 Memory

Cells and records come from reserved arenas, carved in chunks by a global
bump and committed as they are touched. A freed cell goes on the worker's
intrusive free list; a freed record on the worker's free stack. No atomic
sits on the allocation or free path. Arrays are heap blocks with a
refcount; teardown runs in place on the worker that drops the last
reference.

### 2.5 Co-execution

With `--coop`, several engines (CPU processes, the CUDA runtime) run the
whole program and share its large sum folds through a table in a mapped
file (`mithril_core::coop`). An engine reaching such a fold offers it as a
job; engines claim chunks with a compare-and-swap, each chunk is the fold
over a sub-range from the identity, and the total is the wrapping sum of
the chunks. The chunk boundaries and their values do not depend on who ran
them.

## 3. The CUDA runtime

Files: `mithril-gpu/cuda/engine.cu` (the device runtime: vocabulary,
scheduler, arenas), `mithril-gpu/src/cuda.rs` (the program printer),
`runner.rs` (the host: build, sizing, launch, readback, sessions).

### 3.1 One cooperative kernel

The whole run is one cooperative launch, `k_run`, with grid barriers
between phases. All lanes are resident. A **round**:

* the leader snapshots the per-rule task rings;
* **RANGE**, when range requests are pending: warps claim consecutive
  indices of the requests from one counter, each lane runs one index;
  partial sums combine per warp; then one barrier and the completions;
* **GROW**, while forkable tasks exist and the frontier is narrower than
  the grow width: forkable tasks fire in the parallel world, where a fork
  site's callee gets a zero budget and becomes a task, so the frontier
  widens;
* **WORK** otherwise: each lane is dealt tasks of the snapshot and drains
  them depth-first on its own stack in the sequential world, with the dive
  budget, handing back what remains after its step limit.

A join completed by a delivery runs at once on the completing lane. A
record created in the parallel world continues there; one created in WORK
belongs to its lane.

### 3.2 Native launches and range launches outside `k_run`

A native entry (a function with a native form reached from the rule
engine) can run in its own kernel, `k_native_<rule>`, over the pending
tasks of its rule. A range phase that outlasts its budget inside `k_run`
continues in `k_range` from the same claim counter. Both are entered by
`k_run` returning a launch code to the host loop.

### 3.3 Memory

Cells, records, per-rule task rings and an array heap in size classes are
sized from free device memory at start. Each lane has its own free lists
and bump chunks; large array blocks go to a shared per-class stack. Arenas
are managed memory committed on first touch, and nothing is cleared:
every allocation writes its own refcount. Erasure is work: the engine owns
one extra rule, `ERA`, and a large teardown spills its subtrees as `ERA`
tasks like any other.

### 3.4 Bounds and results

Arena exhaustion, an index out of bounds, a stack overflow, an unreachable
arm or the round limit sets an abort flag; every lane stops at its next
check and the host reports the first cause. A deadline stops a run that
does not converge. After the kernel the host reads the root port and walks
only what the result reaches, taking one bulk snapshot of the cells if any
cell is needed.

### 3.5 Build and sessions

`program.cu` is compiled by nvcc and cached by its source hash; every dive
form is compiled once, out of line, so compile time follows program size.
A **session** keeps the context, each program's module and the last run's
buffers, so several jobs pay the setup once (`mithril exec a.gpu b.gpu`).

### 3.6 Co-execution

The CUDA runtime joins the co-execution table (section 2.5) through
`k_coop_args` and `k_coop_done`: the host claims a chunk, the device runs
the fold over it, and the host stores the partial.

## 4. The Metal runtime

Files: `mithril-metal/src/metal.rs` (Metal over the Objective-C runtime:
compile, pipelines, shared buffers, asynchronous submission),
`src/range.rs` (range launches), `msl/ops.metal` (scalar operations),
`msl/range.metal` (the leaves' helpers), `mithril-core/device/soft64.h`;
on the CPU side `mithril-rt/src/metal.rs` (the hook and the cost model)
and `engine.rs` (`drain_ranges_gpu`).

### 4.1 Scope: range launches, the CPU drives

The Metal runtime runs range requests on the Apple GPU; the CPU runtime
runs everything else. The program is lowered once, for the CPU, and the
range folds' native loops (with every function they reach) are printed in
MSL and embedded in the program when it is built for the GPU (`--metal`).
The leaves compile on their own thread while the program starts.

### 4.2 The GPU as a worker of the range wave

In a range wave the coordinator becomes the GPU's feeder. It claims blocks
from the same counter the CPU workers claim from, so every index runs
once. It keeps one block in flight on the GPU, submitted without waiting,
runs CPU blocks meanwhile, and accounts for the GPU's block when it
completes. A sum's GPU block adds its total; a fill's GPU block writes its
elements while CPU workers write the others. A block the GPU declines
runs on the CPU.

How much the GPU takes is a cost model from measurements, per fold: a GPU
block costs a fixed latency plus a time per index, a CPU thread a time per
index. The GPU takes a block only when it would finish no later than the
CPU threads finish the rest; before its rates are known it takes a small
probe. Two properties of the device shape this: a launch waits for the
GPU's scheduler before it starts, while work queued behind running work
starts at once; and the GPU runs wide uniform 32-bit work well, while
64-bit integers, divergent lanes and recursion cost it far more than they
cost a CPU core.

### 4.3 A launch

* **Staging.** The arrays a block reads are copied into one shared buffer
  and reach the leaves as device addresses in the CPU's block layout. A
  fill never reads its array, so its copy carries no elements, only a
  write map; the elements the GPU writes are copied back by that map.
  Buffers are kept between launches.
* **The kernel.** One thread per index runs the fold's leaf; partial sums
  reduce per threadgroup and on the host.
* **Binary32 in two passes.** The fast pass runs f32 in hardware and marks
  an index wherever flushing may have changed a result (a subnormal
  operand, or a zero result the exact operation need not give). The exact
  pass reruns the marked indices with the exact operations.
* **Recursion.** Functions that can reach a guarded call take their nesting
  depth by value; past the compiled limit a leaf faults, its loops stop,
  and the leaves compile again with a deeper limit. A request no limit
  holds, or that indexes out of bounds, runs on the CPU, which reports the
  fault.

### 4.4 What Metal does not have, and the answers

| missing on Metal | the runtime's answer |
|---|---|
| `double` | software f64 (`soft64.h`) |
| subnormal f32 | the exact pass on marked indices |
| `goto` | loops left by `break` (a flag from inside a switch) |
| a stack pointer | nesting depth passed by value |
| mutable program-scope globals | state in buffers; addresses in ports |
| 64-bit atomics, grid sync, an in-kernel clock | not needed by range launches (the full engine will need 32-bit protocols, rounds by dispatch and work budgets) |

## 5. Side by side

| | CPU | CUDA | Metal |
|---|---|---|---|
| what runs there | the whole program | the whole program | range requests |
| driver | coordinator thread, waves | one cooperative kernel, rounds | the CPU's range waves |
| unit of work | redexes, records, dives | tasks in rings, records, dives | indices of a range |
| work distribution | blocks claimed from a cursor | GROW/WORK dealing, warp claims for ranges | blocks claimed from the wave's cursor |
| parallel exposure | suspension by budget | GROW with zero-budget fork sites | none (indices are independent) |
| synchronization | atomic counters, one lock per wave | atomics, grid barriers | dispatch boundaries; 32-bit atomics |
| cells and records | worker arenas, no atomics on alloc | lane arenas in device memory | none (no cells) |
| arrays | heap blocks | heap in size classes, by index | staged copies, write maps |
| recursion | the thread's stack | lane stacks, depth guard by stack bytes | call stack, depth by value |
| budgets | work counts | work counts and device time | none (a leaf runs whole) |
| faults | panics with the cause | abort flag, first cause | decline: the CPU reruns |
| f64 | hardware | hardware | software |
| compile | rustc, cached runtime library | nvcc, cached by source hash | MSL at run time, on a thread |
| reuse across runs | none | sessions | kept buffers within a run |
| sharing a fold | co-execution table | co-execution table | the range wave's counter |

## 6. Toward one runtime

### 6.1 Already shared

* the LIR and the C-family printer (`cprint`, CUDA and MSL dialects);
* the scalar definitions (`mithril_core::float`) and the software f64
  header (`soft64.h`, one text for C++ and MSL);
* the output sink (`mithril_core::sink`) and the co-execution table
  (`mithril_core::coop`);
* the value encodings: ports, cells, the array block layout;
* the range request: the same fold, the same arguments, the same
  completion (deliver to the join record's two slots).

### 6.2 Still duplicated

* The scalar helpers: `prelude.rs`, `engine.cu` and `ops.metal` each spell
  the int, f32 and f16 operations. One header in the style of `soft64.h`
  could serve CUDA and Metal.
* The range protocol: three implementations of claim, run and combine
  (CPU waves, the CUDA RANGE phase, Metal launches). They share the request
  and the completion, not the code.
* The rule table and the engine: `mithril-rt` and `engine.cu` implement the
  same rules separately (issue #18), and a Metal engine would be a third
  copy unless it comes from one portable source (issue #2).

### 6.3 Constraints any shared engine must meet

Metal has relaxed 32-bit atomics and a device-scope fence only, no grid
barrier, no forward-progress guarantee between threadgroups, no in-kernel
clock and a call stack declared per pipeline. A shared engine therefore
needs: allocator and claim protocols on 32-bit atomics; a lane that
publishes only data it wrote itself; rounds driven by dispatches, sized on
the GPU (indirect dispatch); budgets in work counts; recursion bounded by a
depth passed by value or by explicit frames. CUDA can run all of these;
the CPU already uses work budgets.

### 6.4 Order of work

1. One scalar header for CUDA and Metal, checked by the conformance test
   on both devices.
2. One range protocol across the devices: the request, the claim counter,
   the partial sums and the write maps, with the CPU+GPU split as its
   scheduler.
3. The portable device engine (issue #2): `engine.cu` rewritten in a C++
   subset both compilers accept, meeting the constraints above.
4. One rule-table source for the CPU and the devices (issue #18).

Each step must keep every oracle check and the cross-device bit-exactness
tests green.
