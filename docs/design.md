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

Arrays (`array_new/get/set/len`; a heap block `[rc, len|boxed, elems]`,
value semantics, in place when unique). Measured on bfs, 2^19 mazes (reference
4.75 / 0.555 s):

| mechanism | what it proves / does | bfs SEQ / PAR16 |
|---|---|---|
| (arrays in dive form, heap tuples) | | 26.7 / 3.72 |
| native multi-value returns (`n_<f>`) | a dive function returning a k-tuple returns `[u64; k]`; `x = g(..)` followed by `xi = x[i]` bindings takes the components, no heap tuple; a suspension still delivers the boxed tuple to the continuation | 12.7 / 1.42 |
| borrowed array reads | a read (`get`/`len`) lends the array; a param only read is borrowed (same escape analysis as shared ADTs); inlining substitutes variable args instead of alias lets | (in the above) |
| int-element arrays | `Arr(true)` from inference (the element tyvar resolves to `Int`): tag-free reads, writes that free no old element; the `boxed` bit lets drop/copy skip element scans | 10.4 / 1.13 |
| native code over int arrays (`scalar.rs`) | params are borrowed (only read) or owned (inferred by fixpoint); owned arrays are linear on every path (consumed at most once, never read after, freed at path end); tuple results carry an array mask | 9.65 / 0.94 |
| small call-free functions inline always | rustc declines multi-site helpers; the loop body helper is the hot path | 5.63 / 0.59 |
| static uniqueness | inside native code arrays are linear, and the bridge makes an owned array unique once (`arr_own`), so writes skip the refcount check | 5.19 / 0.52 |
| leaf fuel at the call site | a call-free native function settles no fuel through the pointer; its one unit is a register increment in the caller (fuel keeps measuring the same work, so parallel split granularity is unchanged) | 4.85 / 0.48 |
| selects as mask arithmetic | an `if` choosing between computed atoms (what if-conversion leaves) is emitted as `(a & m) \| (b & !m)`, which the backend cannot turn back into a branch on a loop-carried chain | **4.48 / 0.45** |

Native int representation (editdist, whose DP cell is a loop-carried
add/min chain over array words: 4.05 s -> 2.80 s SEQ, 0.53 -> 0.35 s PAR):
native functions hold ints either *plain* (canonical i56 in an i64; an op
whose range is not proven re-wraps with two shifts; masked 32-bit
arithmetic runs in u32) or *pre-shifted* (`x << 8`: i64 wrapping is i56
wrapping, so add/sub/compare/min need no wrap; a var-by-var multiply, a
right shift, an array index and division cost one op). Int arrays store
the pre-shifted word (`ARR_RAW`), tagged <-> shifted is one op. Each
function takes the representation with the lower static op count,
charging the conversions on call edges to functions of the other
representation (so a recursive pair never splits: queens stays all
plain). The context argument is passed only to native functions that
touch arrays (argument registers matter for recursive natives), and
`#[inline(always)]` is decided on literally call-free bodies plus
single-site `while`/`for` helpers (a function's own loop); without the
latter, which member of a recursive cycle absorbs the other was an
accident of ordering (queens 4.8 vs 5.3 s).

Length locals (editdist 2.80 -> 2.27 s): in a call-free native loop that
writes one array and also accesses another, each array's length is held
in a loop variable, because the write may alias the other array's header
and would force a length reload per access. Elsewhere the backend already
hoists the header load, and a register length only obstructs it: it
blocked vectorization of bfs's fill loop and caused spills in its BFS loop,
which carries four inlined helpers. Measured, not assumed: every variant
was built by hand first.

Known inference weakness: ints are unified through arithmetic, so one
heterogeneous array (ints and lists in the same array) poisons the element
type and every int connected to it becomes `Dyn` (correct, tagged code;
slower). No port does this; `tests/fixtures/hetero_array.py` covers the
runtime conversion separately.

Each step was first proven on hand-edited generated code (the same
runtime helpers), then made a codegen rule; the final generated code
matches the hand proof.

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
baseline — only from a state verified against reference (`--update --only X`
refreshes X's entry and keeps the rest).

Why it exists: one day of tree-bitonic tuning silently dropped mandelbrot
off the scalar path (6x), double-freed in merkle, and made nbody's
generated code exponential; all were found hours later by full runs.

## 6. Standings vs reference (big sizes, this box; ours = wave runtime)

| bench | ours SEQ / PAR16 | reference SEQ / PAR16 | state |
|---|---|---|---|
| bfs | 4.42 / 0.45 | 4.75 / 0.555 | met |
| mandelbrot | 3.8 / 0.50 | 4.96 / 0.62 | met |
| tree-radix | 4.3 / 0.70 | 4.84 / 0.75 | met |
| lexer | 2.4 / 0.31 | 2.96 / 0.40 | met |
| symreg | 2.95 / 0.41 | 4.76 / 0.60 | met |
| tree-matmul | 3.6 / 0.58 | 4.16 / 0.61 | met |
| merkle | 5.2 / 0.58 | 5.67 / 0.72 | met |
| gameoflife | 9.07 / 1.19 | 9.89 / 1.50 | met |
| queens | 4.73 / 0.47 | 8.32 / 1.23 | met |
| tree-bitonic | 10.3 / 2.73 | 10.78 / 1.78 | SEQ met; PAR wave-bound |
| kmeans | 10.7 / 1.47 | 7.84 / 0.77 | needs lane-level (u32) vectorization; hand proof 5.81 |
| editdist | 2.27 / 0.27 | 2.44 / 0.39 | met |
| hashmap, terrain | | 3.22 / 0.40, 3.21 / 0.46 | being ported to arrays |
| nbody, raytrace | | 6.29 / 0.67, 7.60 / 0.96 | need native f32 |

## 7. Process rules (from the user)

* One runtime model everywhere; a component that is bad is bad globally.
  No architecture swaps for small wins.
* Wins come from changing usecase geometry: rewrites, types, codegen.
* Every benchmark must meet reference with general mechanisms only.
* `tests/ci/fast.py` before every commit; no full-suite runs until each
  benchmark is individually cleared.
