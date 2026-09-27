# Mithril — design as built (2026-09-28)

This is the design *as implemented*, with the numbers that justify each
part. The original spec is `docs/superpowers/specs/2026-09-27-mithril-design.md`;
where this document disagrees with it, this document is what the code does.

## 1. Thesis and verdict

Thesis: a language whose semantic core is interaction nets can be
compiled to code that matches a state-of-the-art strict compiler (reference)
sequentially, gets parallelism for free from the net model, and keeps the
one thing nets give that strict compilers cannot: lazy sharing
(`DUP`) as automatic runtime staging (spike 5: 40x on unstaged
precomputation).

Derisk benchmark: tree-bitonic, depth 23 (8M leaves), same box as reference.

| | SEQ | PAR (16 threads, 8 cores) |
|---|---|---|
| reference (compiled C) | 10.78 s | 1.78 s |
| C twin | 8.41 s | — |
| Mithril, hand-written engine on the real runtime | 10.39 s | — |
| Mithril, generated | **10.46 s** | **2.82 s** |

Verdict, stated plainly:

* **Sequential: the approach works.** Generated code reaches reference with
  no per-benchmark logic. Every mechanism that got it there is a static
  property the compiler proves about the program (§3) — the net runtime
  costs nothing on the sequential path because the compiler removes it.
* **Parallel: the net model gives parallelism without annotations, at a
  cost.** 2.82 s vs 1.78 s is the price of the *wave* runtime (§4): work
  becomes visible to other workers one barrier at a time, and each
  dependency hop across a suspension costs a wave. A work-stealing pool
  on the same protocol measured 2.05 s; it was reverted because it made
  the CPU and GPU runtimes different machines. The gap is a property of
  the wave model and is reported as such, not tuned around.
* The genuine IN value (lazy sharing) is demonstrated by spike 5, not by
  this suite; every port here is first-order strict code where reference's
  design (no runtime nets) is the natural fit. This suite proves
  *viability*, not superiority.

## 2. Pipeline

```
Python-subset source
  -> parse -> desugar (loops -> tail-recursive fns; if/match statements
     become one join continuation, not one copy of the rest per arm)
  -> Core IR (mithril-front::core; reference interpreter = oracle)
  -> compile-time net reduction of everything static (a small program
     folds to a constant; see tests/ci/fast.py's "lines=4" note)
  -> Core->Core rewrites (mithril-codegen::rewrite)
  -> type inference (ty.rs, monomorphic) -> unboxing, linearity
  -> ANF normalize -> reuse marking
  -> dual-mode emission: native sequential "dive" form + net "rule" form
  -> rustc -O against mithril-rt (CPU) / PTX via mithril-gpu
```

## 3. General mechanisms (each applies to every program)

Every item below is decided from the program's static structure; none
inspects a benchmark name or shape.

| mechanism | what it proves / does | measured effect (tree-bitonic SEQ) |
|---|---|---|
| native scalar lowering (`scalar.rs`) | a function whose params/returns are ints or int tuples is emitted as plain `i64` Rust (`s_<f>`), incl. tuple-valued join points | mandelbrot 175 s -> 4.4 s |
| unboxed unary int ctors | `Leaf(v)`-style ctors ride in the port word; no cell | part of 762G -> 254G instr |
| static linearity (`LIN`) | a type never shared anywhere in the program carries no refcount traffic | " |
| in-place reuse (`mark_reuse`) | a ctor built on a call-free path after a match consumed a same-arity cell reuses that cell | " |
| leaf inlining, join-point desugar | smaller hot functions (warp: 2235 -> 471 asm lines) | 254G -> 241G |
| bounded functions | a function on no call cycle can never run out of fuel: plain call, no capture | no cost on nbody-style helper chains |
| register-returned dives, out-of-line cold capture | `Result<u64,u64>`; suspension code never bloats the hot frame | 257G -> 241G |
| Lean-checked reassociation | fold combiners proven associative are split in parallel | (fold ports) |

Refcounting is Perceus-style (u8 saturating), used only where linearity
cannot be proven; chained (arity > 2) constructors move their fields on
consume like arity <= 2 ones do.

## 4. Runtime: waves, dives, records

The runtime is one model on CPU and GPU:

* A **redex** is a pending rule application; a **record** is a
  continuation waiting for `pend` values.
* A **dive** runs a function natively under a fuel budget. On fuel-out it
  *captures*: the pending call is re-spawned as a redex, and every native
  frame on the way up splits its continuation into the part independent
  of the pending value (spawned now as a task) and the dependent rest
  (a record, pend 2). This is where parallelism comes from: nothing in
  the source says "fork".
* The rule form (segments) dives too; it only allocates records on
  suspension, nests at most 4 inline dives before deferring the rest to a
  (memoized) record, so generated code is linear in chain length.
* **Waves**: the coordinator merges every worker's spawn buffer into
  per-rule buckets and drains the heaviest bucket across the pool
  (`entries x rule_cost`, where anything that may dive costs a full
  budget). Records whose last child delivers fire immediately on that
  worker (bounded nesting) instead of waiting for the next wave — this is
  part of the wave model, on every backend.
* Sequential runs use fuel 2^40 (one dive); parallel runs 16384.

Cost model, measured on tree-bitonic PAR16: per-wave barrier ~70 us;
every dependency hop across a suspension is one wave; the frontier grows
exponentially only while suspended frames have large independent siblings.
The remaining gap to reference (2.82 vs 1.78 s) is waves with fewer ready
entries than workers.

## 5. Gate: `tests/ci/fast.py` (~60 s)

Run before every commit. Per port, in parallel: small-size exact checksum
at 1 and 16 threads; the set of scalar-lowered functions must not shrink;
generated lines / segment count / build time within tolerance; instruction
count of a mid-size run within 15% of `tests/ci/baseline.json`; arena
exhaustion reported as its own failure class. `--update` rewrites the
baseline — only from a state verified against reference.

Why it exists: one day of tree-bitonic tuning silently dropped mandelbrot
off the scalar path (6x), double-freed in merkle, and made nbody's
generated code exponential; all were found hours later by full runs.

## 6. Standings vs reference (big sizes, this box; ours = wave runtime)

| bench | ours SEQ / PAR16 | reference | state |
|---|---|---|---|
| tree-bitonic | 10.46 / 2.82 | 10.78 / 1.78 | SEQ met; PAR wave-bound |
| mandelbrot | 4.39 / 0.55 | 4.96 / 0.62 | met |
| tree-radix | 5.3 / 0.56 | 4.84 / 0.75 | PAR met, SEQ 10% off |
| lexer | 3.6 / 0.50 | 2.96 / 0.40 | ~20% off |
| symreg | 7.3 / 0.96 | 4.76 / 0.60 | |
| tree-matmul | 7.8 / 1.00 | 4.16 / 0.61 | |
| merkle | 7.4 / 4.5 | 5.67 / 0.72 | PAR does not scale (Speck loops are scalar chains) |
| queens | 18.7 / 2.6 | 8.32 / 1.23 | |
| kmeans | 15.7 / 1.6 | 7.84 / 0.77 | |
| gameoflife | 30.5 / 3.9 | 9.89 / 1.50 | |
| bfs, editdist, hashmap, terrain | need > 2^26 cells | | array-as-tree spelling; memory, not time |
| nbody, raytrace | > 60 s | 6.3 / 7.6 | float emulation (no native f32) |

Numbers other than tree-bitonic/mandelbrot are from a 60 s-capped status
sweep, not clean runs. The rows that are off are off by *usecase geometry*
(array-as-tree memory, emulated floats, scalar loops that never suspend
so never fork) — the next general mechanisms are unique-owner in-place
arrays, native f32, and fork exposure for loop-shaped work; none is a
runtime change.

## 7. Process rules (from the user)

* One runtime model everywhere; a component that is bad is bad globally.
  No architecture swaps for small wins.
* Wins come from changing usecase geometry: rewrites, types, codegen.
* Every benchmark must meet reference with general mechanisms only.
* `tests/ci/fast.py` before every commit; no full-suite runs until each
  benchmark is individually cleared.
