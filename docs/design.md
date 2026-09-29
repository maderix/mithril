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
  -> specialization by the interaction rules (mithril-net::specialize):
     every function's body is a net over unknown parameters, reduced to
     quiescence; the residual net is read back as the new body (3b)
  -> Core->Core shapes codegen owns (mithril-codegen::rewrite): mutual
     tail recursion into loops, if-conversion of loop back-edges
  -> type inference (ty.rs, monomorphic) -> unboxing, linearity
  -> ANF normalize -> reuse marking
  -> dual-mode lowering to `lir` (mithril-codegen::lir): native
     sequential "dive" form + net "rule" form + native scalar form, one
     statement IR over a fixed helper vocabulary (3c)
  -> a printer per backend: `lir::rust` -> rustc -O against mithril-rt
     (CPU); the CUDA printer is stage 2 (mithril-gpu today is a separate
     v1 prototype)
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
| leaf inlining (now the net's inline policy, 3b), join-point desugar | smaller hot functions (warp: 2235 -> 471 asm lines) | 254G -> 241G |
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

## 3b. The net as the optimizer of record (`mithril-net::specialize`)

Constant folding, inlining, branch selection, static evaluation and
unrolling are not passes: they are the interaction rules firing early, on
the redexes that do not depend on runtime input. `mithril net f.py` prints
what reduction did per function (rewrites, calls kept, ops kept, calls
evaluated, size before/after; `MITHRIL_NET_CORE=1` dumps the bodies,
`MITHRIL_NET_TRACE=1` narrates). The Core rewrites that duplicated this
(`inline_leaves`, `unfold_static`, constant folding) are deleted.

How a function is specialized:

* its body is built as a net whose parameters are unfilled wires; the
  static gate lets a rule fire only between two non-variable ports, so
  everything independent of the parameters reduces (ops on constants,
  branches and matches on known values, sharing, calls the inline policy
  unfolds: call-free callees of size <= 96);
* a call the policy did not unfold is *settled*: all arguments known ->
  evaluated in a scratch net (fuel 200k, value <= 4096 cells) and replaced
  by its value; some known -> unfolded speculatively (below); else kept;
* every branch parked on an unknown value has its arms instantiated (each
  in its own scope frame, pattern binders as fresh unknown wires) and
  reduced the same way, to a fixpoint: code under runtime branches is
  specialized too;
* the residual net is read back as Core: single-use scalar expressions
  nest, calls / projections / data / matches are let-bound where used,
  a value shared through `Dup` is bound once in the frame it was created
  in (let-normal form only where sharing or evaluation order needs it,
  because codegen's cost models read the shape: if-conversion measures
  arm *work*, not bindings, for this reason).

Speculative unfolding (the old `unfold_static`, as rules): a call with
some known arguments is instantiated in a clone of the whole state and
specialized to its fixpoint, its own calls unfolded in turn (a loop with
a static bound unrolls as a chain, a branch inside it keeps both arms).
It is accepted, replacing the state, only when its control was static:
no call of its own remains, no match on a runtime value, no constructor
over unknown fields (a data builder is not code to unroll: `symreg`'s
`gen(5, ..)` doubled the program for no instruction gain), and the growth
is within 2000 agents (ops, branches, data; wires are free — the old
2000-binding limit). Budgets: 50k rewrites per top-level attempt shared
by everything nested in it, 400k per function; a failed attempt is
memoized by (callee, which-arguments-known) so it is not retried per
arm; the growth ceiling is inherited by nested attempts so a 300-iteration
chain stops at the ceiling, not at the depth limit (256). Inside a
speculation only the branches the speculated body parked are
instantiated, and the first new residual call aborts it (a rejected
attempt used to instantiate every arm of its 2^k paths first).

Evidence (tests/ci/fast.py vs the pre-specializer baseline): 16/16
checksums; instructions within 3 % everywhere (gameoflife's `board_step`
16-iteration unroll and merkle's `spk(22, ..)` chain recovered exactly
the old numbers, 3.54G and 0.27G); code size within the gate. `alias`
(one array lent and moved into the same call) is now inlined and lowered
natively: the native body reads the array before the in-place write.
A loop with one state variable returns the value itself (no 1-tuple, no
`Proj`; the fold join combines bare partial results), which also made
mandelbrot/terrain's loop helpers native-scalar.

The thesis on a spike-5 shape, with the first-order stack: an expression
interpreter (`ev`/`look` over an AST with `Let`/`If`/arith and an
association-list environment) applied to a constant program and a
dynamic input specializes, by the rules alone, to the program's own
arithmetic — no match, no call, no constructor left (the first Futamura
projection, `specialize_test::interpreter_over_a_static_program_...`):

```
run(x) = ev(prog(), Bind(0, x, Emp()))          # 605 rewrites
   ==>  let v = (x * 3) & M in
        if v != 0 { (v + ((x * x) & M)) & M } else { 7 }
```

Nothing in the specializer knows what an interpreter is: matches on
known constructors select, `look`'s recursion over the known environment
unfolds, the `Let` case's environment cell is consumed by the lookup that
reads it, and only the branch and ops on the input stay. Phase B
(closures shared by `DUP` at runtime) extends the same mechanism to
programs whose static part is only known at runtime.

### Phase B derisk: sharing on the real rule table (`examples/spike_w2.rs`)

Before building closures into the language, spike 5's W2 shape was run on
the product's rules and reducer (not the spike's toy evaluator): `g = λx.
x + heavy(k)` built once, copied through a chain of DUPs, applied N times.
This needed the Dup commutations the first-order rule set lacked
(Dup–Op/App/Swi/Mat: the consumer passes through the superposition; a
Ref facing a Dup just unfolds once with the Dup as its ret). k = 24,
W = 1,500,491 rewrites:

| N | rewrites | strict N·W | per application | time |
|---|---|---|---|---|
| 1 | 1,500,491 | 1.5M | — | 0.094 s |
| 256 | 1,502,531 | 384M | 8.0 | 0.111 s |
| 4096 | 1,533,251 | 6.1G | 8.0 | 0.111 s |
| 65536 | 2,024,771 | 98G | 8.0 | 0.135 s |

Exactly W + 8N on both schedules (heavy before or after the copies are
queued), values oracle-equal; the reducer runs ~16M rewrites/s. So the
runtime claim is a property of the rules, not of the spike's evaluator,
and the constant per shared application is 8 rewrites. Regression:
`reduce_test::shared_closure_computes_its_free_work_once`.

### Phase B, steps 1–2: closures in Core and in the net

`Core::Lam(x, body)` / `Core::App(f, a)`; desugar curries `lambda a, b:`,
compiles a call on a local variable to `App`s, and eta-expands a
top-level function used as a value. Capture is wiring: a closure's free
variables are the enclosing `Var`s, lowered to the wires they already
flow through (dup-fanned by the same `bind` as every other shared value).
`eval_core` gets closure values (parameter, body, captured env) for the
oracle only.

Net: `Lam` cells `[param, body]` built eagerly (the body is a net region;
work in it that does not depend on the parameter fires when the closure is
built — once), `App` cells `[arg, ret]`; beta was there. Dup labels are
dynamic: every sharing site takes a fresh 24-bit label, a copy of a dup
(through a constructor, a lambda, a commutation) carries the copier's
label; same label = the two halves of one copy meeting = annihilate,
different = commute (Dup–Dup, Dup–Op/App/Swi/Mat). A single label class
was wrong the moment a copied closure shared its own parameter
(`twice(sq, 3)`). Clone discipline (`check_clone_discipline`): no closure
applied to itself (`App(f, Var v)` with `v` free in `f`, through `Let`
aliases) — the oracle-needing cases; `f(f(x))`, factories, closures
capturing closures, `map` with a lambda are in.

Readback: a residual `Lam` reads as `Core::Lam` with its own frame; a
compound read inside a closure is bound in the innermost frame that needs
it — closure frames whose parameter it does not mention are peeled (so
`mk(k) = λx. x + heavy(k)` reads back as `let h = heavy(k) in λx. x + h`:
the sharing the net computed survives into the residual program), branch
frames never are. A parked `App` on an unknown function reads as `App`.

Evidence: source programs (factory, `twice`, `compose`, `map` with a
capturing lambda) reduce to the oracle value; W2 from source is linear in
N (`reduce_test`); `mk(3)` applied twice specializes to a constant; a
returned closure specializes to a lambda; fast.py unchanged (first-order
paths untouched). Codegen refuses `Lam`/`App` until step 3.

### Phase B, step 3: the runtime runs the same rule table

The rules moved to `mithril_core::rules` behind two traits — `Cells` (the
store: cells, a redex worklist, a label supply) and `Prog` (what a `Ref`
unfolds into, how a builtin computes, where an uncomputable op goes, the
runtime's own value forms) — and the derived program's instantiation to
`mithril_core::lower`. The compile-time reducer is `Net: Cells` with the
specialization policy as its `Prog`; a runtime worker is `Wctx: Cells`
(its arena is the same two-word cells) with the generated program as its
`Prog`. One implementation, compiled once in `mithril-rt`
(`reduce_net` takes `&dyn Prog`).

A compiled program's *net region*:

* its entry table (`net_entries`: the closures compiled code builds and,
  transitively, the branches/arms their bodies mention; a real function
  is never instantiated — a `Ref` to it spawns its CALL rule, and a FILL
  record links the result back into the wire the net was waiting on);
* two engine rules: NET (generic redexes spilled on fuel-out, or a value
  delivered into the net from another worker) and FILL;
* the bridge: `build_closure` (instantiate the closure's entry over the
  captured values), `apply` (dive form: an App cell reduced in place,
  the value returned when it arrives within budget, otherwise a
  forwarding record the result is delivered to through a `Kont` port —
  the dive suspends like on a call), `apply_spawn` (rule form: the
  continuation waits in a record), `dup_closure` (sharing a closure
  value from compiled code *is* the DUP–LAM rule, `copy_lam`, with the
  original cell as the first copy so the caller's port stays valid),
  and erasure of a dropped closure by the Era rule.

Value forms meet across the boundary unchanged (Num/Con encodings are
the same; arrays moved to tag 14, unboxed constructors are the runtime's
ext values the rules copy/erase/match through the program's helpers).
Closures in compiled code are opaque `Lam` ports until applied.

Evidence (`codegen_test`): every closure shape (factory, `twice` on a
top-level function and on a lambda, `compose`, `map` with a capturing
lambda, a closure shared and applied in a loop, closures in data) is
oracle-equal at 1/4/16 threads and under fuel starvation; W2 at runtime
(`mk(k) = λx. x + heavy(k)`, k opaque) applied 4096 times: the oracle
value, 20,485 rewrites (5 per application, `heavy` once, 0.00 s) against
a strict twin that calls `heavy` per iteration (0.61 s); 65,536
applications: 327,685 rewrites, 0.01 s. Same value and same rewrite
count at 16 threads. fast.py: instruction counts unchanged; generated
programs carry the region's fixed cost in rustc time (bfs 0.6 → 0.8 s).

### Phase B, step 4: the proof programs (`bench/general`)

Three closure programs joined the generality corpus, each with a Python
oracle and an idiomatic Rust twin written the way the source is written
(closures recompute what is under them; nobody hoists by hand):

| program | shape | Rust twin | Mithril t1 / t16 | note |
|---|---|---|---|---|
| `stage_closure` | W2: `mk(k) = λx. x + heavy(k)`, 20k applications | 0.00 s | 0.00 / 0.00 s | LLVM hoists the pure call out of the loop inside one function; both do the work once |
| `pipeline_cfg` | stage closures built from a runtime config, in a list, applied to 20k inputs | 0.29 s | **0.01 / 0.01 s** | closures stored in data: Rust cannot hoist across the `Box<dyn Fn>`; the net runs each stage's setup once (20–29× faster than the Rust program as written) |
| `interp_closure` | closure compilation of a runtime AST, applied to 20k environments | 0.01 s | 5.8 / 6.1 s | no work to share (everything depends on the environment): every application copies the closure tree by the DUP rules — the cost of the model on closure-heavy code with no sharing win, ~450× |

All oracle-equal, parallel == sequential. The first two are the thesis
at runtime: work under a closure that does not depend on its parameter
is done once by the rules, wherever the closure is composed. The third
is the price: a closure whose whole body depends on its parameter gains
nothing from lazy copying and pays the per-rewrite constant on every
application. That is the next target (a closure body with no
parameter-free work can be applied by compiled code directly, the same
rule table deciding when), not a tuning question.

reference on these shapes: its closures are affine (used once), so a
closure applied n times must be rewritten to recompute — reference runs the
strict twin, which is what the Rust column measures. A timed reference lane
for the corpus is not set up (the reference toolchain lives in a container;
see reference/reference-notes.md).

Sound-ness notes from getting here (all in the rule table or the
reader, none in a benchmark): a `Ref` to a compiled function waits for
produced arguments (a call met inside a closure being built runs as a net
instead); a `Dup` meeting an arm closure copies it (never unfolds it);
`Dup–Mat/Swi` commutations copy the arm closures eagerly so arm slots
always hold closures; `Op` with a superposed operand commutes (OP–SUP);
each fan-out cell has its own label; the reader un-superposes lazily
copied closures by selecting sides per copy label (sup/dup readback),
with superpositions classified to a fixpoint; type inference has a
function type so an ADT field holding both ints and closures poisons to
Dyn (a closure is never an immediate); applying a closure consumes it,
so one read from a borrowed structure is copied first (`dup_val` on a
closure is the DUP–LAM rule with the original cell as the first copy).

Oracle checks: `specialize_test.rs` (every fixture: specialized ==
original under `eval_core`; the policy cases above) and
`examples/spec_oracle.rs` (bisects a whole program to the function whose
specialization changed its value).

### 3b-2. Readback places calls where the net created them

The net fires a call as soon as its arguments exist. The reader used to
write a call where its result was first used, so in bitonic's `warp`
(whose inlined zip matches the first recursive result before touching the
second) the second call was nested inside the match on the first: a
dependency the net does not have, and one that stops the rule form from
forking the pair. The specializer records the scope frame of each pending
call, keyed by the call's result wire (unique per residual call; the Ref
port is not, every nullary call to a function shares one), and the reader
binds the call there. Frames are recorded per settle and a closure body
is not one, so the recorded frame alone cannot decide closures. One rule
places every bound value, plain, shared (Dup) or a call (`place`): while
a value created in an outer frame is read, the arms above that frame are
skipped (the net made the value there, unconditionally), and a closure
keeps the value exactly when the value reads something the closure binds
(its parameter, a value bound in it or in a frame nested in it);
otherwise the value moves out and every application shares it, which is
the net's DUP sharing. A call created in the frame that uses it is read
in place, so tail calls stay tail calls.

Two independent reviews shaped this. The first found a closure escape
and a key collision (nullary calls shared the Ref port as key; calls are
now keyed by their result wire). The second found that the first fix
still let a shared value escape a closure (`lambda x: sq(fib(x) + k)`,
pre-existing) and, worse, rebuilt a call the net computes once outside a
closure inside it, once per application (a sharing regression). Pushing
the recorded frame on top of the reader's stack hid the closures between;
reading "in frame F" is now a floor, not a push.

Numbers: a 2^16-leaf warp on the GPU 1454 ms -> 12 ms; bitonic CPU
instructions 0.965 G -> 0.912 G. Tests (specialize_test): independence of
the pair (fails with the hoist disabled), tail calls, closure-body calls
in scope (3 shapes), branch-only nullary call stays in its branch, shared
call stays in its arm, shared values in closure bodies stay in scope (2
shapes), a call outside a closure is shared by every application. Each
bug test fails on the commit it guards.

### 3b-3. Placement follows what a value reads; sharing per selection

The third review of 3b-2 found three readback bugs, two of them wrong
answers with no error. (1) A shared Dup's binding was cached by its cell
alone: a closure applied twice at compile time to different arguments
reads the same Dup under different superposition sides, and the second
application reused the first's value (oracle 1157, program 2). The cache
is keyed by (cell, selection); the key is the whole selection, so a
captured shared value read under three selections is bound three times
(duplicated work, not a wrong answer; keying by the selections a read
consults is the pending refinement). (2) A pending call inside an
unapplied closure's body was stamped with the frame of the first settle
that reached it, so when the closure was applied inside an arm the call
bound outside the arm: a diverging call then ran on the other path, or a
match binder was read before its match. The specializer now stamps no
pending call or Dup whose value depends on an unapplied closure's
parameter (`needs_param`, a walk over the producers); it is stamped at
the settle where the application fired. In the reader, `place` also never
binds a value below the innermost frame binding something it reads, and
a frame read as the floor is exactly that frame's position. Tests: the
shapes (the call's argument made in the branch, a parameter, a match
arm), oracle-equal, scoped, the call kept under its branch. Open: a
closure created in an arm capturing a pattern binder, applied twice,
ICEs in the reader (an ignored test); two specializer crashes on nested
closures with conditionals (`mithril-core` net.rs:54 and rules.rs:290,
the reviewer's probes b4/b5).

The device stack guard (with 3f): a native scalar function's non-tail
recursion has no budget, so on the device its frames overflowed the
32 KiB thread stack as a driver fault. Every dive form and every native
function whose frames can pile up (a non-tail self call, or a cycle
through other functions) now begins with `stack_guard`, a no-op on the
CPU; the device compares the PTX stack pointer with the one at kernel
entry against the runner's limit and leaves the frame with a named abort
("recursion too deep for the device"). Guarding every non-leaf native
function cost raytrace 3x (isect is called 10^8 times), hence the
recursion test; with it, raytrace and bitonic are unchanged.

### 3c. One lowering, printers per backend (stage 1 of the shared backend)

Decided 2026-09-28, after Phase B. Every emitter used to print Rust text
directly, and the GPU crate carried its own weaker lowering of Core. The
three stages toward one backend: (1) pull the decisions out of the
printers into an IR; (2) a device runtime and a CUDA printer of the same
IR; (3) the rule table (closures, DUP) on the device, GPU lane in the gate.

Stage 1, done (b890872..b1a8372):

* `Core` carries its traversal once (`kids/any/walk/fold/rename/
  free_vars/max_var`); 40 hand-written walkers across codegen, net and gpu
  became one-line predicates.
* `mithril-codegen::lir`: locals with declared types; `If`/`Switch`/`Loop`
  at statement level (value branches lower to a declaration plus
  assignments); `Try` = a call that may suspend, with the handler that
  leaves the function; `Res` = a two-outcome match; nested cold functions
  for suspension captures (their parameters come from `free_locals` on the
  IR, not from scanning text). Every emitter (dive, rule/segment, fold
  split/join, CALL and hole rules, base-case wrappers, native scalar)
  builds it; `lir::rust::func` prints it. Generated code names the worker
  context only through free functions of `mithril_rt::prelude` (`alloc2`,
  `alloc_rec`, `deliver`, `dive_to`, `dive_res`, `pop_chain`, `rec_*`, ..),
  so a backend without a context object supplies the same names over its
  own arena. The IR's helper vocabulary is the device runtime contract of
  stage 2.
* The value helpers (`mk_con`, `consume*`, `dup_val`, `free_val`,
  `take_field`, `untup`, `arr_*`, `bin`, `cmp`, `show`) left the generated
  prelude for `mithril_rt::prelude`, generic over a `Tables` trait the
  program implements (`lin`, `unbox_cid`, closure dup/drop). They were
  pasted into every program only for `lin(k)` to constant-fold; it still
  does, per instantiation.

Numbers: oracle-equal on all 31 codegen tests and all 16 ports at every
step; instruction counts identical to the baseline on every measured
port (the moved helpers carry `#[inline]`; without it lexer/tree-radix
ran 16-21% more instructions, i.e. it restores the previous placement,
not a new choice). Language crates (front, core, net, reassoc, codegen,
cli): 15,229 -> 14,798 lines; runtime 1,527 -> 2,093.

Not done in stage 1: the net region (`net_region`), the `Program` impl
and `main` are still Rust templates in lib.rs (program data + generic
glue); `fold::est_static` is a text static. The CUDA printer decides
their device form.

### 3d. Stage 2: the device runtime and the CUDA printer (25b156d)

`mithril_codegen::lower` returns the program as data (`LirProgram`:
functions, rule table, dive table, linearity and unbox tables); the CPU
`emit_rust` prints it, and `mithril_gpu::emit_cuda` prints the same value
as `program.cu`. Nothing decides anything twice: the v1 GPU prototype,
which lowered Core on its own (no arrays, no ownership, no TRMC, no
native scalar), is deleted.

* `crates/mithril-gpu/cuda/engine.cu` is the device runtime: the IR's
  helper vocabulary over the wave engine (cells with a refcount array,
  records and delivery, dives under a per-dive fuel with `R`/`RA<k>`
  suspension results, constructors and sharing, arrays on a device bump
  heap, TRMC holes, dynamic and native arithmetic). Float arithmetic uses
  the `_rn` intrinsics so the device never contracts into fma: nbody
  printed a different checksum until it did.
* The printer (`mithril_gpu::cuda`): tuples are `T<k>`/`P2` structs
  (declared per width used), capture functions are lambdas, slice
  arguments are hoisted to local arrays, a `Let` of a name already in
  scope prints as assignment (Rust shadowing), operand widths for C's
  arithmetic come from the IR's declared local types.
* Gate: `tests/ci/gpu.py` runs all 16 ports at their small size through
  `mithril run --gpu` and checks the CPU checksum; `MITHRIL_GPU=1 cargo
  test -p mithril-gpu --release -- --include-ignored --test-threads=1`
  runs the 22 closure-free codegen fixtures against the oracle plus the
  capacity/abort tests. All pass on the RTX 4090. A cold run is dominated
  by nvcc (5-77 s per port); warm runs are cached by source hash.
* Known limits after stage 2: the array heap is a bump allocator (blocks
  are never freed); per-dive fuel is 64 by default (device stack), so the
  device suspends far more often than the CPU (fuel 4096+), which is the
  parallelism the wave engine wants but also more records per program.

### 3e. Stage 3: the net region on the device (e6638ed)

The rule table runs on the device: `cuda/engine.cu` carries
`mithril_core::rules` rule for rule (REF unfold / erase / closure copy,
wiring as the static gate, Kont delivery, beta, OP with the operand-swap
half-step and OP-SUP, SWI, MAT with projections and unboxed constructors,
DUP of values and lambdas, the DUP commutations, DUP-DUP) over per-lane
redex worklists that spill to the program's net rule, and the closure
bridge compiled code uses (`build_closure`, `apply`, `apply_spawn`,
`dup_closure`, `drop_closure`). A generated program supplies its entries
as straight-line net builders (`inst_<e>`) printed from the same `NExpr`
bodies the CPU interprets with `instantiate`, its match tables, and the
FILL/NET rule ids. The CPU and the device are now checked against each
other on every closure program: the third implementation of the rules is
held to the first two by the oracle, as the core constraint asks.

Numbers: 24/24 codegen fixtures (closures and W2 sharing included), the
closure corpus (stage_closure, pipeline_cfg, interp_closure at run.py's
small sizes) and 16/16 ports print the CPU's result on the 4090. The GPU
gate (`tests/ci/gpu.py`, ports + corpus) runs in about two minutes warm.

Found on the way: the device compiler inlined the rule table into every
caller (437K lines of PTX and a 5-minute ptxas for a 3-function
program); every non-trivial runtime function is `__noinline__` now (29K
lines, 4 s). Performance of the device path is not measured yet: that is
the next use case to prove, not a number to tune (the runtime helpers as
calls, the 64-fuel dives, the bump heap are the known costs).

Open after stage 3: device-side memory for arrays is never reclaimed;
`interp_closure` remains the closure-copying case on both backends.

### 3f. The device path measured, and a cost model of the slowdown

`bench/gpu_vs_cpu.py`: every port at fast.py's mid size, CPU binary vs
the device, results equal on all 16 (two device bugs that only showed at
this size were fixed first: a 16-bit record field truncating the TRMC
hole cell, and records never recycled). Wall times were then 10-1000x
slower than the CPU. The per-fire cost, replicated standalone
(`bench/gpu/fire_cost.cu`, the real `engine.cu` with a stub program, a
synthetic fire: ~24 cell allocs, ~24 frees, ~31 reads, a record, a spawn
and a delivery; cycles per fire):

| component | one thread | full grid (65,536 threads), per thread |
|---|---|---|
| 24 cell allocs | 23,300 | 331,000 |
| 24 allocs + 24 frees | 18,300 | 350,000 |
| the whole fire | 30,000 | 560,000 |

so a lane's step costs 30k cycles alone and ~560k under load (memory
round-trips: 80 cycles per cell read, 300-1000 per allocation, and no
allocator variant gets below ~24k alone). The model that held up:

    T = sum over rounds of (the slowest lane's steps x cost per step)

Every slowdown found was in the first factor. In order (tree-bitonic,
depth 16 unless said; CPU t1 0.04 s; reference on this 4090: 0.576 s at
depth 23):

1. Host wave loop: 7,764 rounds (one launch and one sync per round,
   ~130 us of host per round): 1.8 s. Runtime unchanged.
2. Fork sites hand-edited into the program (a callee suspends at entry
   and becomes a task, the rest of the body goes on): 196,511 rounds,
   12 s. A completed join was queued globally, so every join level cost a
   round; running it at once on the completing lane (the CPU runtime's
   rule, and reference's): 4,586 rounds, 3.8 s.
3. The lowering's two-way frame split: after `warp_node(a)` suspends in
   `flow`, `flow(left)` (which needs only that result) sat in the same
   join as `Node(left, right)` and so waited for the whole right half:
   every merge ran right-then-left, a chain of 2^15 steps, three rounds
   each, 92,844 rounds. Fixed in the lowering (`split_dep`, 3f-1 below):
   461 rounds.
4. The driver on the device (`k_run`, one cooperative launch, grid
   barriers between phases; the rounds are now 10-20 us) and reference's grow
   rule (a sweep grew when it pushed anything; stop when it pushed at
   least a lane's worth): 833 sweeps, no work phase, 0.12 s. Depth 20
   still 2.6 s and depth 23 did not finish: the first WORK phase ran one
   lane for 40 s.
5. Static dealing in WORK (every lane gets every nl-th pending task; the
   claim race had left 90% of lanes idle) and, at first, no fuel-out
   suspension in the sequential world (superseded by 7): all 32,768
   lanes busy, the slowest lane a few dozen steps.
6. reference's fork in the parallel world (3f-2 below): the independent part
   of a body used to run its sibling call inline with the dive budget, so
   one side of every fork unfolded several levels per sweep and lanes got
   subtrees of wildly different sizes. A fork's children are now both
   tasks (`let x = f(..) in x` in the independent part is a tail call)
   and a cut runs inline. Depth 23: 0.84 s. (Every call a task, with no
   cuts, gave 0.62 s here, the frontier doubling exactly, but a
   sequential chain then cost a round per step: 100,000 steps, 100,001
   rounds, 1.3 s; with cuts inline it is 1,563 rounds and 0.3 s.)
7. The sequential world keeps the dive budget (an unbounded budget gave
   0.84 s against 1.03 s, but the budget is the only bound on native
   recursion depth on a 32 KiB device stack), and on the device a
   dive-form call refunds its budget when it returns, so the budget
   bounds the depth of the native recursion, not the work of a subtree:
   raytrace's work phase went 1.42 s (a suspension per 64 rewrites in
   every lane's subtree) back to 0.06 s, bitonic 23 to 0.86 s. The budget
   is a per-backend scheduling parameter, not semantics: the CPU keeps
   the work budget because its suspension is the split mechanism that
   exposes work to idle threads (the refund was not measured there; a
   depth budget would suspend only at depth 4096 and expose nothing on
   a wide, shallow tree). Fold estimates read the phase's budget.

Standing (device, this box, default caps): tree-bitonic depth 8 6 ms,
16 64 ms, 23 860 ms (reference GPU 576 ms; CPU t16 2.46 s); raytrace big
61 ms (reference GPU 544 ms; CPU t16 2.84 s); a boxed chain of 100,000
steps 0.3 s. Results equal the oracle on every fixture, port and corpus
program. The frontier at the first WORK is 49,152 tasks on 32,768 lanes
(a fork step of x1.5 on average), so half the lanes carry two subtrees:
that quantization is the gap to reference on bitonic.

#### 3f-1. The frame split follows the net's dependencies

A suspended frame's continuation is a let chain. `split_frame` gives P,
the bindings independent of the pending value x (they run at once, as a
ready record) ending in one live-out l, and J, the rest. J now splits
again (`split_dep`): D, the bindings that need x but not l, ending in
one live-out m (a pend-1 record chained under the join, fired when x
arrives), and J2, the join of m and l (pend 2). `join_records` builds
these for both forms. Without D, a call in J that needed only x waited
for P to finish: a sequential dependency the net does not have (in the
net the call node fires when its own input arrives). The three-way split
is exact dataflow for the shape `Node(f(g(a)), f(g(b)))`; a J that
reads x in J2, or a D with several live-outs, falls back to the two-way
split. Fixture `fork_chain.py`; every fixture stays oracle-equal on
every thread count and fuel.

#### 3f-2. Two worlds, one lowering

The device runtime runs reference's rhythm. GROW: every forkable pending
task fires in the parallel world. There a fork site's callee is a task at
once (`fork_fuel` hands it no budget, so it suspends at entry;
`dive_res_fork` in the rule form) and so is a segment's tail call
(`tail_to`, reference's marked call: the sibling in the independent part),
while a cut (a call whose result the next statement needs) runs inline
with the task's budget; the continuation is captured as records. (Two
fork sites still run as cuts: a native multi-value callee, and a call
past the rule form's inline-nesting limit; performance only.) A
`let x = g(..) in x` at any rule-form position is a tail call. The
frontier widens by one fork level per sweep. When a sweep pushed at
least as many tasks as there are lanes, or nothing, WORK: every lane
gets every nl-th pending task and drains it and everything it spawns
depth-first with the dive budget on its own stack; a cross-lane join it
completes runs at once in the parallel world, and what that forks goes to
the global rings for the next round. On the CPU `fork_fuel`,
`dive_res_fork` and `tail_to` are identities (one world, the wave runtime
as before), so the same lowered program runs on both; the schedule is
the only difference, as the core constraint requires. Records freed by a
lane past its list spill to a global ring (as cells do; a push never
overwrites an unread slot; a pop that finds an empty slot leaves the
entries below it stranded until later pops pass it: a leak bounded by
the ring, never a double hand-out). `k_run` needs a cooperative launch (all lanes
resident); the host launches once and reads the rounds back. A run that
does not converge stops at `MITHRIL_GPU_ROUNDS` rounds with an error, and
any device fault ends the run. Known limit: a native scalar function's
non-tail recursion has no budget and overflows the device stack (32 KiB
per thread) past a few thousand frames, as a driver fault that loses the
context. reference has the same kind of limit (a fixed value stack of 2048
words per device lane) but checks every push and fails with a numbered
error; a depth counter in the scalar lowering with a named abort is the
pending fix.

The old host loop, its kernels and the sequential-tail pump are gone;
the regression comparison is the round count: `gpu_schedule_is_bounded_
by_fork_levels` (tree-bitonic depth 8 in under 400 rounds; the host loop
took thousands at depth 8 and 7,764 at depth 16) and
`gpu_chain_costs_a_round_per_budget_not_per_step`. Fixtures
`fork_split_shapes.py` (every split shape and fallback, a boxed value
shared by the three records), `chain_boxed.py`, `deep_leaves.py` (deep
recursion inside a WORK lane).

### 3g. A use case outside the corpus: the k-d tree (generality probe)

`bench/ports/kdtree.py` (C twin `kdtree.c`): a 2-d tree over 2^18 hashed
points, built by midpoint splits over a list (subtrees as unequal as the
data makes them, a bucket for coincident points), then 2^18 nearest
neighbour queries with pruning, forked as a batch that only reads the
shared tree. Nothing in the corpus shares a large read-only structure
across every fork, or partitions lists, or chases pointers data-
dependently. C, CPython (the port shim) and Mithril agree at every size.
It found six general defects; none of the fixes looks at the program.

The access model, stated once (it was already the rule; two places broke
it):

* Shared data is lent. A lent value costs no atomic: `field` reads and
  pattern matches on a borrowed parameter touch no count. Counts move
  only when ownership moves (a stored, returned or owned-passed value: an
  O(1) increment; a drop: a decrement, the last one tears down). The
  escape analysis counted an integer field returned from a lent tree as
  an escape, so `nearest` owned the tree and every node visit paid three
  atomics on cells every lane shares; an integer is an immediate and
  never escapes. `nearest` and the batch now borrow the tree end to end.
  Borrowing more widely exposed a hole in the emitter: a `let` whose
  right-hand side is a branch (`p = lft(a)` after the net inlines `lft`)
  bound the arm's raw read of the lent tree without copying it, and the
  owned binding was then consumed or freed: kmeans freed a subtree of the
  shared stats tree and overflowed its stack on the corrupted cycle. A
  binding is owned unless it aliases a lent value, so a lent read bound by
  a branch is copied (an O(1) count increment). Fixture `lent_pick.py`
  fails on the old emitter (wrong sum) and is oracle-equal on this one,
  at every thread count and suspension budget.
* Counts are 32-bit plain atomics on both runtimes. The CPU's were 8-bit,
  saturating at 255 by a fetch_add then a store: under concurrent
  increments the count passed through 0 for a moment, a reader saw the
  root as unique and freed the shared tree (a panic at 16 threads, never
  at one). Fixture `shared_tree.py`, oracle-equal at 16 threads.
* Allocation is a per-lane arena: a global bump hands out chunks, freed
  cells and records go on the lane's intrusive free list (the link in
  the freed cell's first word, the record's `d`): unbounded, lock-free,
  no atomic on the alloc or free path. The device's lists were bounded
  (64, then a 1M ring, then dropped): the sequential build leaked its
  way to the arena cap on every program.
* Erasure is work. The net's ERA rewrites are independent; the runtime's
  teardown was one lane walking a value. The engine owns one rule past
  the program's table, ERA: a teardown past 256 nodes spills its
  subtrees as ERA tasks, dealt like any other. The k-d tree's end-of-run
  teardown (2^19 cells, 3.1G cycles on one lane, the whole work phase)
  became parallel.
* The device budget bounds recursion depth only: work units (loop
  iterations, native leaf calls) burn nothing there, a loop in the
  parallel world runs to its end (reference's), and a dive refunds its budget
  on return. The CPU keeps the work budget (its suspension is the split).
  tree-bitonic depth 23: 876 ms -> ~100 ms.
* Bounds on both sides. A device fire is bounded: every 2^20 work units
  force the frame's next budget check to suspend, so a runaway loop
  cycles rounds to the round limit instead of freezing the device (which
  is shared with the desktop); the host waits with a deadline
  (`MITHRIL_GPU_TIMEOUT`, 300 s) and tears the context down past it. The
  CPU arenas are capped at half of the memory available at start, so a
  runaway program ends in "arena exhausted", not in swap. A teardown also
  spills past 32 nested frames (its frames stack on the caller's): that
  was the 256-lane fault.
* The oracle interpreter deep-copied a value on every variable read
  (`Val::C(id, Vec<Val>)`): the shared-tree fixture cost it 21 GB and
  hung the machine twice under the test suite. Payloads are now shared
  (`Arc`), a clone copies nothing; the same fixture takes 69 MB. Test
  suites run under a memory watcher that kills them past a cap.
* Small runtime helpers are inlined on the device: a `__noinline__` call
  out of a 200-register function spills through local memory (1.7M
  cycles per tree-node visit, profiled with clock64; Nsight needs the
  admin counter permission on this box and could not attach to the
  cooperative launch).

Standing (this box; the device numbers were taken while a game held 6 GB
of VRAM, so they are upper bounds): CPU 0.42 s at one thread, 0.115 s at
16, C twin 0.35 s; device 0.54 s, of which ~0.45 s is the build's
sequential prefix (partitioning a 2^18 list on one lane, a property of
the model that reference shares). Open: tree-bitonic depth 23 can overflow
a rule ring (2M entries) now that work phases run whole subtrees: a
level of its merge is 2^22 tasks of one rule; the fix is one shared task
ring (reference's cube is 16M tasks), not per-rule rings: a regression at
that one size against b738538 (the loop budget kept the frontier small
by suspending work phases), traded for the 10x on every other size.
The ERA spill and the device work cap are device-only scheduling; the
CPU tears down in place and burns work units (its suspension is the
split), the same rules either way.

### 3h. Harness standing at 15f51dd, and the source review

The full harness (`bench/harness.py`, one run per lane, 300 s timeout,
default arenas, this 4090 with the desktop holding ~1 GB of VRAM):

* CPU, 17 ports: at or under reference seq on 12; ahead of reference par on 10 at
  16 threads. Four ports do not scale at all (gameoflife, nbody, queens,
  symreg run at one-thread speed): the sequential-prefix shapes.
* GPU, the honest number: raytrace 0.78 s (reference 0.54), kdtree 1.7 s,
  editdist 4.3 s, lexer 6.2 s, mandelbrot 15 s, merkle 18 s, nbody 70 s,
  tree-radix 72 s, kmeans 142 s, gameoflife 262 s, queens past 300 s;
  bfs, hashmap, symreg, terrain, tree-bitonic and tree-matmul exhaust a
  per-rule ring, the array heap or the record arena at defaults. Every
  finished run is checksum-equal. The device-only times quoted in 3f/3g
  (bitonic ~0.1 s, raytrace 0.056 s) were hand-picked sizes with raised
  arenas; the lane as a whole is not competitive yet. The two structural
  items are one shared task ring and the work-phase cycling on
  sequential prefixes.

Three independent full-source reviews (one per crate group, 2026-09-29)
found 16 defects, of which 10 were confirmed with probes and fixed in one
commit, each with a fixture: `range(a, b)` re-evaluated `a` every
iteration; a bool variable lost its boolness across a loop or a join and
was rejected in a later condition (now kept when every assignment to it
is boolean); an int `match` with no matching case fell into an
unreachable constructor instead of continuing (Python falls through); a
NaN comparison was an ICE (now IEEE: only `!=` holds, one definition
shared by the oracle and the reducer); hex literals from 2^63 wrapped
negative; a native tuple projected after a suspendable call did not
compile; `return ()` in a dive tail did not compile; a loop-form dive
leaked an owned parameter its body never read on every iteration; a
function whose only floats were parameters was classified as a native
scalar form and computed on the ports' bits. Unconfirmed (argued from the
code, kept on the list): an array leaking through an if-arm tuple mask, a
thread-local read before it is set on the device path, a borrow-inference
round cap. The reviews also list about 1500 lines of folds at low risk
and two Core-level rewrites in the code generator (tail inlining,
if-conversion) that the core constraint forbids at that layer: those get
their own commit and note.

Every reference number in this document is reference (reference/reference, the reference
runtime with the task cube, `bench/reference.csv` on this 4090), never reference
1 or HVM.

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
* Sequential runs use fuel 2^40 (one dive); parallel runs 16384 per dive
  while the frontier is thin, and `16384 x ceil(entries / (4 x workers))`
  once a wave holds more than four entries per worker (`wave_fuel`):
  suspension exists to expose work to idle workers, and splitting past
  that only costs records and locality. hashmap PAR16 0.49 -> 0.23 s (every
  suspension in the batch spine used to split off a sibling subtree until
  all 2048 tables were in flight, 20M live cells, cache-bound even on one
  worker); other ports within noise.

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
| hashmap | 1.82 / 0.23 | 3.22 / 0.40 | met |
| terrain | | 3.21 / 0.46 | being ported to arrays |
| nbody, raytrace | | 6.29 / 0.67, 7.60 / 0.96 | need native f32 |

## 6b. Generality corpus (`bench/general/run.py`)

Seven programs deliberately unlike the suite: a heavily shared DAG, a
persistent BST map with live old versions, copy-on-write array versions,
list merge/quick sort, an expression interpreter (many constructors, env
as a list), graph DFS over an array of adjacency lists, and mutual
recursion. Each is checked against the Python oracle (small size) and an
idiomatic Rust twin (`rust/*.rs`, Rc/Vec; same checksum), then timed.

| program | Rust | ours t1 | ours t16 |
|---|---|---|---|
| collatz_mutual | 0.30 | 0.43 | 0.43 |
| cow_versions | 1.00 | 0.98 | 0.97 |
| dag_share | 1.68 | 1.87 | 0.29 |
| graph_dfs | 0.86 | 1.20 | 0.27 |
| interp | 2.36 | 1.21 | 0.14 |
| persist_map | 1.47 | 1.49 | 1.49 |
| sorts | 6.21 | 1.41 | 0.60 |

Sequentially within 1.0-1.4x of idiomatic Rust (faster where Rust pays
per-node refcounting); 16 threads never slower than 1, and 4-9x faster
where iterations are independent. What the corpus found (all general
fixes): value-position `if`/`match` leaked every value whose last use was
in an arm (cow_versions 6 GB -> 2 MB); a fold's split never fired when
iterations were heavier than one budget; a sequential program paid for
splits at 16 threads (waves that expose nothing now grow the budget);
live dive bridges hid native call graphs from the backend; mutual tail
recursion stayed calls (tail inlining makes it a loop); an array write
evaluated the array before a value that reads it (dup + free per write).

## 7. Process rules (from the user)

* One runtime model everywhere; a component that is bad is bad globally.
  No architecture swaps for small wins.
* Wins come from changing usecase geometry: rewrites, types, codegen.
* Every benchmark must meet reference with general mechanisms only.
* `tests/ci/fast.py` before every commit; no full-suite runs until each
  benchmark is individually cleared.
