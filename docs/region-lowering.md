# Shared region lowering

Preserve the verified Symreg GPU fix and make subsequent execution strategies
small changes to shared machinery. The first service migration is implemented and verified; full native/growth
body unification remains subsequent work. The interaction rules remain the sole program semantics.

## Current evidence

Language source is 16,865 lines, versus 14,937 at HEAD and a 15,000-line cap.
The increase is cumulative; the seven `native*.rs` files account for 1,808
current lines. `scalar.rs` and `seq.rs` account for another 3,580 lines.
Symreg's final generated workload is 140.586 ms versus 1,736.240 ms before.
CPU instructions remain a regression: 3.674 billion versus 1.053 billion.
These fixes and open gates must survive the consolidation.

The native family separately implements substitution, path evaluation, purity
policies, graph successors, dependencies and physical-width facts. CUDA also
has its own helper-result lookup. These policies are not all interchangeable:
a borrowed read can be allowed in a copy path but forbidden in a total leaf
shortcut. Sharing the traversal must retain those separate proof requirements.
`lower()` also lowers the module twice around fold-plan discovery; the second
pass exists because discovered plans affect joins and entry classification.
Removing it requires separating discovery from emission, not dropping a pass.

## Improvement axes

| Axis | Concrete remaining cost | Candidate examples | Evidence level |
| --- | --- | --- | --- |
| Layout and field access | Chained constructors need extra cell accesses and checks | KD-tree; wide matrix nodes | KD-tree emits the new pair helper but takes its chained fallback; no win |
| Ownership and materialization | Creating, copying, retaining and erasing intermediate trees | Tree-matmul; bitonic; radix | Source exposes constructing computations; benefit needs an isolated proof |
| Continuation lifetime | Saving/restoring, allocation, zeroing and dispatch | CPU borrowed traversal; deep GPU regions | CPU profile and local frame probes establish the cost; fix remains open |
| Work exposure and dependencies | Long work before the first useful fork; uneven tasks | KD-tree construction; irregular search | KD-tree boot is about 565 ms of sequential program work, separate from its 557 ms work interval |
| Scalar representation | Tagged conversions and conservative operation/storage widths | Arithmetic and float-heavy regions | Candidate axis; no new speed claim |
| Setup and transfer | Context, allocation, readback and launch costs | Short device workloads | Merkle has about 24 ms work versus 159 ms process wall |

These costs recur across programs. A strategy applies where its assumptions
hold; a useful subsystem must make those assumptions explicit and reusable.

## Recommended boundary

Consolidate around the existing `Machine`, `Block`, `End` and LIR. Keep one
region graph and give its consumers three shared services:

1. **Operation contracts.** Record result representation and effects for runtime
   helpers: borrowed reads, mutation, ownership transfer, allocation, work/fuel
   observation and possible failure. Unknown operations are barriers. This is
   metadata about existing implementations, not another semantic rule table.
2. **Region facts.** Share successors/predecessors, liveness, call captures,
   invariant inputs, snapshot-aware substitution and width evidence. Facts are
   invalidated or recomputed after a graph mutation; they are not cached across
   a changed graph. Existing Core type/borrow/range facts remain their source.
3. **Boundary emission.** Preserve logical calls/returns and saved values until
   physical emission. Use one logical capture description for native frames and
   the existing growth/suspension protocol, preserving each protocol's encoding,
   ownership and charges. Rust and CUDA retain their printers and device-specific
   storage, while consuming the same logical region facts.

Start by folding the duplicated services into the existing native/LIR code.
Then migrate prefix, exit and path consumers onto those services. Only after
that migration proves reuse should native/growth body emission be unified.
A complete replacement of `scalar.rs` and `seq.rs` is not the first step.

A strategy should specify eligibility and a small transformation. For example,
sharing two field reads needs stable-owner and no-intervening-write checks,
field layout, original conversions and failure behavior. Those answers should
come from common services; the strategy should not add another recursive walker,
substitution environment, liveness solver or helper whitelist.

Other consumers retain their stronger certificates: additive fold splitting
still needs its checked summary law; leaf shortcuts still need totality; removal
of completed-task accounting still needs a call-closure proof that no caller
observes the budget. A single `pure` boolean cannot replace these requirements.

## Alternatives

Utility-only extraction is smaller initially but leaves strategies rebuilding
control and protocol analysis. A general optimization DSL or plugin system has
substantial upfront machinery and no demonstrated need here. Consolidating the
existing region representation gives existing users of the subsystem immediately
and a concrete deletion target.

## Scope and deletion targets

- `native.rs`: retain construction/physical emission; share its graph queries.
- `native_paths.rs`, `native_exit.rs`, `native_prefix.rs`: replace repeated
  substitution, variable-use and effect walks with the common services.
- `native_join.rs`, `native_store.rs`: consume one call-capture/layout result;
  preserve all current width and selector proofs.
- `native_fold.rs`: share graph traversal and dependencies; retain the affine
  coefficient proof and its bounded discovery search.
- `completed.rs`, `lir.rs`, GPU `cuda.rs`: share operation contracts where their
  policies overlap; preserve conservative rejection in each consumer.

The first migration must remove more production code than it introduces. It
must have at least two existing strategy consumers. A utility rename, moving
code into runtime/test files, compressed formatting, or merely deleting checks
does not satisfy this gate. The 15K language cap remains the final cleanup target;
there is no claim that the first extraction alone saves all 1,865 excess lines.

## Validation and order

Freeze current emitted Rust/CUDA, source hashes and measured artifacts first.
For an analysis-only extraction, require identical emitted source for all 17
ports and the existing closure/array/deep/shared fixtures. Add focused contract
and graph tests for unknown helpers, mutation/free barriers, trapping operations,
branch snapshots, cycles, join liveness and changed-graph invalidation.

Then reuse the services in at least two strategies and delete their old paths.
If emission changes, require Core/net/native/growth oracle equality in both
integer representations and the relevant actual CUDA matrix, including cache
spill and poisoned-route checks. Recheck full Symreg paired timing after any
physical emission or frame change. Run fast CI and report the known CPU failure
explicitly; do not refresh its baseline to turn preservation into a passing gate.

After the subsystem is smaller, continuation lifetime is the next local proof:
it is a verified open regression and exercises the shared capture/layout model.
Construction/materialization and chained-field access follow as separate proofs.
No next-example tuning or new scheduling policy is part of this consolidation.

## Implemented first migration (2026-10-02)

`lir::operations` supplies helper contracts and the existing three proof
policies. LIR supplies snapshot substitution and local reads. Machine supplies
successors, incoming counts, liveness and call captures; `Frame::layout` is the
single description consumed by call sharing and save/restore emission. Prefix,
exit, path, join and fold consumers reuse these services. Completed-task closure
checks retain their stronger requirements, and CUDA uses overlapping helper
result contracts without changing its remaining inference defaults.

All 340 emitted Rust/CUDA artifacts across 85 programs are byte-identical.
The normal GPU build reproduces the timed Symreg cubin hash and the full-size
checksum. Seven new tests and three assertion-failing mutation checks cover the
services; independent review has no findings. Source files shrink by nine lines
net, including four new cfg-test wiring lines. Language source is now 16,854;
this establishes reuse but does not substantially retire the 1,865-line debt.

Fast CI retains the Symreg CPU failure. Full workspace testing also exposes a
Queens completed-entry assertion that fails against the frozen original compiler;
that gate remains open. The CLI feature test passes in an isolated CPU build.
No commit or performance improvement is claimed by this extraction. Receipts,
source snapshots and commands: `target/region-lowering/report.md`.
