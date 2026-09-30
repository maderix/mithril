# Mithril design

This document describes Mithril as the code implements it, with the
measurements that justify each decision. The original intent is in
`docs/superpowers/specs/`; where the two disagree, this document is what
the code does. The core constraint and the working rules are in
`CLAUDE.md`; nothing here overrides them. Every reference number is reference
(reference/reference, the reference runtime), never reference 1 or HVM.

Contents:

1. Thesis and what it claims
2. Pipeline
3. Semantic core: the rule table, specialization, readback
4. Types and ownership
5. Lowering
6. CPU runtime
7. Device runtime
8. Verification
9. Measurement method
10. Standings
11. Use cases and scope
12. Open items
13. Process rules
14. Prior art and provenance

## 1. Thesis and what it claims

Mithril is a language whose meaning is defined by interaction-net rules.
One rule table runs at compile time on the part of the net that does not
depend on runtime input, and at runtime on the rest.

The thesis has three claims.

* **Sequential parity.** A program whose semantic core is a net compiles
  to code that matches a strict compiler (reference) at one thread. The net
  costs nothing on the sequential path because the compiler proves the
  static properties that let it remove the runtime machinery (sections 4
  and 5).
* **Parallelism from the model.** The rules are confluent, so any redex
  order gives the same result. Work runs in parallel without annotations:
  the lowering finds the fork sites in the program's own dependencies, and
  the scheduler decides where redexes fire.
* **Lazy sharing as runtime staging.** A value copied through `DUP` is
  computed once, however many consumers it has. Work under a closure that
  does not depend on the closure's parameter runs once. Strict compilers
  cannot do this across data structures. Spike 5 measured 40x on unstaged
  precomputation; the product's rule table reproduces the effect (section
  3.4).

What the benchmark suite proves: viability, not superiority. Every port
is first-order strict code, the shape reference is designed for. The
sharing claim is shown by the closure corpus (section 11), not by the
suite.

## 2. Pipeline

```
Python-subset source
  -> parse, desugar (loops become tail-recursive functions; an if/match
     statement becomes one join continuation, not a copy of the rest
     per arm)
  -> Core IR (mithril-front::core; eval_core is the reference oracle)
  -> specialization by the interaction rules (mithril-net::specialize):
     each function body is a net over unknown parameters, reduced to
     quiescence and read back as Core
  -> Core rewrites codegen owns (mithril-codegen::rewrite): tail
     inlining, if-conversion of loop back-edges, reuse marking
  -> type inference (monomorphic), unboxing, linearity, borrowing
  -> ANF normalization
  -> lowering to LIR (mithril-codegen::lower -> LirProgram): dive forms,
     rule forms, native scalar forms, the rule table, the net region
  -> printers: lir::rust (rustc -O against mithril-rt, CPU) and
     mithril_gpu::emit_cuda (program.cu against engine.cu, device)
```

Nothing is decided twice. The CPU and the device run the same
`LirProgram`; the printers differ only in syntax and in the runtime they
call.

## 3. Semantic core

### 3.1 The rule table

The rules live in `mithril_core::rules`, behind two traits. `Cells` is
the store: cells, a redex worklist, a label supply. `Prog` is what a
`Ref` unfolds into, how a builtin computes, where an uncomputable op
goes, and the runtime's own value forms. The instantiation of a program
into a net is `mithril_core::lower`.

The same implementation serves two stores. The compile-time reducer is
`Net: Cells` with the specialization policy as its `Prog`. A CPU runtime
worker is `Wctx: Cells` (the same two-word cells) with the generated
program as its `Prog`. It is compiled once, in `mithril-rt`
(`reduce_net` takes `&dyn Prog`).

The rules: `REF` unfold, erase and closure copy; wiring as the static
gate; `Kont` delivery; beta; `OP` with the operand-swap half step and
`OP-SUP`; `SWI`; `MAT` with projections and unboxed constructors; `DUP`
of values and lambdas; the `DUP` commutations (`Dup-Op`, `Dup-App`,
`Dup-Swi`, `Dup-Mat`); `DUP-DUP`; `ERA`.

Dup labels are dynamic. Every sharing site takes a fresh 24-bit label. A
copy of a dup (through a constructor, a lambda or a commutation) carries
the copier's label. The same label means the two halves of one copy meet
and annihilate; different labels commute. A single label class is wrong
as soon as a copied closure shares its own parameter (`twice(sq, 3)`).
Each fan-out cell has its own label.

The device carries a second, hand-written implementation of the same
rules in `crates/mithril-gpu/cuda/engine.cu`. It is held to the first by
the oracle tests (section 8). One source for both is an open item.

### 3.2 Specialization: the net as the optimizer of record

Constant folding, inlining, branch selection, static evaluation and
unrolling are not passes. They are the rules firing early, on redexes
that do not depend on runtime input. `mithril net f.py` prints what
reduction did per function (rewrites, calls kept, ops kept, calls
evaluated, size before and after). `MITHRIL_NET_CORE=1` dumps the bodies;
`MITHRIL_NET_TRACE=1` narrates.

A function is specialized as follows.

* Its body is built as a net whose parameters are unfilled wires. The
  static gate lets a rule fire only between two non-variable ports, so
  everything independent of the parameters reduces: ops on constants,
  branches and matches on known values, sharing, and calls the inline
  policy unfolds (call-free callees of size at most 96).
* A call the policy did not unfold is settled. If all arguments are
  known, it is evaluated in a scratch net (200,000 rewrites, a value of
  at most 4,096 cells) and replaced by its value. If some are known, it
  is unfolded speculatively (below). Otherwise it is kept.
* A branch parked on an unknown value has its arms instantiated, each in
  its own scope frame with pattern binders as fresh unknown wires, and
  reduced the same way to a fixpoint. Code under runtime branches is
  specialized too.

Speculative unfolding instantiates the call in a clone of the whole state
and specializes it to its fixpoint, unfolding its own calls in turn. A
loop with a static bound unrolls as a chain; a branch inside it keeps
both arms. The attempt is accepted only when its control was static: no
call of its own remains, no match on a runtime value, no constructor over
unknown fields, and growth within 2,000 agents (ops, branches, data;
wires are free). A data builder is not code to unroll: symreg's
`gen(5, ..)` doubled the program for no instruction gain, hence the
constructor condition. Budgets: 50,000 rewrites per top-level attempt,
shared by everything nested in it; 400,000 per function; nesting depth
256. A failed attempt is memoized by (callee, which arguments are known).
The growth ceiling is inherited by nested attempts, so a 300-iteration
chain stops at the ceiling. Inside a speculation only the branches the
speculated body parked are instantiated, and the first new residual call
aborts it.

The first Futamura projection falls out of the rules. An expression
interpreter (`ev`/`look` over an AST with `Let`, `If` and arithmetic, the
environment an association list) applied to a constant program and a
dynamic input specializes to the program's own arithmetic, with no match,
call or constructor left:

```
run(x) = ev(prog(), Bind(0, x, Emp()))          # 605 rewrites
   ==>  let v = (x * 3) & M in
        if v != 0 { (v + ((x * x) & M)) & M } else { 7 }
```

Nothing in the specializer knows what an interpreter is. Test:
`specialize_test::interpreter_over_a_static_program_...`.

### 3.3 Readback

The residual net is read back as Core. Single-use scalar expressions
nest. Calls, projections, data and matches are let-bound where used. A
value shared through `Dup` is bound once. Let-normal form is used only
where sharing or evaluation order needs it, because codegen's cost models
read the shape (if-conversion measures arm work, not bindings).

Placement is one rule, `place`, for plain, shared and call values.

* The specializer records the scope frame in which the net created each
  pending call, keyed by the call's result wire. The key is the result
  wire because it is unique per residual call; the `Ref` port is not
  (every nullary call to one function shares it).
* A value created in an outer frame is bound in that frame: the arms
  above it are skipped, because the net made the value there
  unconditionally. A frame read this way is a floor, not a push, so the
  closures between stay visible.
* A value is never bound below the innermost frame that binds something
  it reads.
* A closure keeps a value exactly when the value reads something the
  closure binds (its parameter, or a value bound in it or in a frame
  nested in it). Otherwise the value moves out and every application
  shares it: `mk(k) = λx. x + heavy(k)` reads back as
  `let h = heavy(k) in λx. x + h`.
* A pending call or `Dup` whose value depends on an unapplied closure's
  parameter (`needs_param`) is not stamped at the first settle that
  reaches it; it is stamped at the settle where the application fires.
* A call created in the frame that uses it is read in place, so tail
  calls stay tail calls.
* A shared `Dup` binding is cached by (cell, selection), because one Dup
  read under different superposition sides is different values.

Placement matters for parallelism, not only for scope. The net fires a
call as soon as its arguments exist. Binding the call where its result is
first used adds a dependency the net does not have: in bitonic's `warp`
it nested the second recursive call inside the match on the first, and
the rule form could not fork the pair. With calls placed where the net
created them, a 2^16-leaf warp on the device took 12 ms against 1,454 ms,
and bitonic CPU instructions went from 0.965 G to 0.912 G.

A residual `Lam` reads as `Core::Lam` with its own frame. A parked `App`
on an unknown function reads as `App`. The reader un-superposes lazily
copied closures by selecting sides per copy label, with superpositions
classified to a fixpoint.

### 3.4 Closures and runtime sharing

`Core::Lam(x, body)` and `Core::App(f, a)`. Desugar curries
`lambda a, b:`, compiles a call on a local variable to `App`s, and
eta-expands a top-level function used as a value. Capture is wiring: a
closure's free variables are the wires they already flow through,
dup-fanned by the same binding as every other shared value.

In the net a `Lam` cell `[param, body]` is built eagerly: work in the
body that does not depend on the parameter fires when the closure is
built, once. The clone discipline (`check_clone_discipline`) rejects a
closure applied to itself (`App(f, Var v)` with `v` free in `f`, through
`Let` aliases). `f(f(x))`, factories, closures capturing closures and
`map` with a lambda are accepted.

The runtime runs the same table. A compiled program has a net region:

* an entry table (`net_entries`): the closures compiled code builds and,
  transitively, the branches and arms their bodies mention. A real
  function is never instantiated as a net; a `Ref` to it spawns its CALL
  rule, and a FILL record links the result back into the waiting wire;
* two engine rules: NET (generic redexes spilled on fuel-out, or a value
  delivered into the net) and FILL;
* the bridge: `build_closure`, `apply` (dive form: the App cell reduced
  in place, the value returned within budget, otherwise a forwarding
  record fed through a `Kont` port), `apply_spawn` (rule form),
  `dup_closure` (the `DUP-LAM` rule, with the original cell as the first
  copy so the caller's port stays valid), and erasure of a dropped
  closure by `ERA`.

Value forms cross the boundary unchanged. Closures in compiled code are
opaque `Lam` ports until applied. On the device, entries are printed as
straight-line net builders (`inst_<e>`) from the same `NExpr` bodies the
CPU interprets.

The sharing constant, measured on the W2 shape (`g = λx. x + heavy(k)`
built once, copied through a chain of DUPs, applied N times):

| engine | condition | rewrites | per application |
|---|---|---|---|
| compile-time reducer (`examples/spike_w2.rs`) | k = 24, W = 1,500,491, N = 1 to 65,536, both schedules | exactly W + 8N | 8 |
| generated program, CPU runtime | `heavy` opaque, 4,096 applications | 20,485 | 5 |
| generated program, CPU runtime | 65,536 applications | 327,685 | 5 |

The reducer runs about 16 M rewrites/s. The 4,096-application run takes
0.00 s against 0.61 s for a strict twin that calls `heavy` per
iteration; the rewrite count is the same at 16 threads. Regression tests:
`reduce_test::shared_closure_computes_its_free_work_once` and the
`w2_runtime` fixture.

## 4. Types and ownership

### 4.1 Types

Inference (`ty.rs`) is monomorphic. It has a function type, so an ADT
field holding both ints and closures poisons to `Dyn` (a closure is never
an immediate). An operator unifies its operands, and its result is a
fresh int (or float) once they are known: a conflict where a result is
used (an int stored in a tree that also holds tuples) stays at that use
and does not flow back into the arithmetic that made it; a result whose
operand is poisoned later takes the operand's type again (a fixpoint
after each pass). A closure's result has no known type and conflicts
with any type it meets, as do the components and elements read out of
it, so a parameter that receives any of them is not native. One
heterogeneous array (ints and lists in one array) still poisons the ints
unified with its elements to `Dyn`: correct tagged code, slower. No port
does this; `hetero_array.py` covers the runtime conversion.

Ints are i56, canonical in a 64-bit word. Floats are boxed f64 cells or
binary32 values. Comparisons with NaN follow IEEE (only `!=` holds), one
definition shared by the oracle and the reducer.

A variable stays boolean across loops and joins when every assignment to
it is boolean.

**f32 in the surface** (`infer.rs`, run by `parse`). A second monomorphic
inference, over the surface AST: every variable, parameter, result,
constructor field, tuple component and array element has one type, found
by unification. `sqrt(x)` and `f32(n)` introduce f32; it spreads through
assignments, operators, calls and returns, and a float literal written as
the argument of `f32()` or `int()` is f32 (any other f64 expression passed
to either is an error, not a retyping). f64 stays dynamically typed: a
helper may take ints and f64s, and the rules dispatch on the value.
Mixed int/f64 arithmetic (`2.5 * 3`) is a type error of the rules; native
code does not check it and computes a meaningless value (a known gap). Where a value is f32, a literal becomes its bit
pattern and `+ - * /` and comparisons become the `f32_*` builtins, so
Core, the net and every backend see ints and the existing rules: `x + y`
on f32 is `f32_add(x, y)`, nothing new in the rule table but `f32_le`
(IEEE `<=`; `==` is `le(a, b) & le(b, a)`, a helper written in the
language). Unary minus on f32 flips the sign bit.

* An int literal is an int. Only as the direct operand of an operator or
  a conditional expression does it take the other side's type (`x * 2`
  on f32 is `x * 2.0`). It never becomes f32 through a call, a return, a
  constructor field or a tuple slot.
* Every f32 conflict is an error that names the function: f32 meeting an
  int, a tuple, an array, a constructor, or a value that already mixes
  shapes. A function, field or slot has one type; a helper used at int
  and at f32 is an error, not a retyping. Integer operators on f32 are
  errors.
* `f32(n)` rounds correctly for every i56 and `int(x)` truncates toward
  zero, exactly below 2^55 (NaN and larger magnitudes give 0). Both are
  helpers written in the language, so a constant argument folds by the
  rules.
* `a, b = e` checks that `e` is a tuple of two.
* A program without f32 is unchanged, except negation: `-x` is `0 - x`
  on ints and `-1.0 * x` on an f64; a value used as both is an error.

### 4.2 Representation

* Unary constructors over an int (`Leaf(v)`) are unboxed: they ride in
  the port word, no cell.
* Arrays are heap blocks `[count, len | flags, elems]` with value
  semantics, written in place when unique. `Arr(true)` (element type
  resolved to `Int`) gives tag-free reads and writes that free no old
  element; the `boxed` flag lets drop and copy skip element scans. Int
  arrays store the pre-shifted word (`ARR_RAW`).
* A type never shared anywhere in the program is linear (`LIN`) and
  carries no count traffic.

### 4.3 The access model

Shared data is lent. Counts move only when ownership moves.

* A lent value costs no atomic. Field reads and pattern matches on a
  borrowed parameter touch no count.
* A stored, returned or owned-passed value costs one O(1) increment. A
  drop is a decrement; the last one tears the value down.
* A parameter only read is borrowed (escape analysis, a fixpoint over the
  program). An integer is an immediate and never escapes; counting an int
  field returned from a lent tree as an escape made the k-d tree's
  `nearest` own the tree and pay three atomics per node visit on cells
  every lane shares.
* A binding is owned unless it aliases a lent value. A lent read bound by
  a branch (`p = lft(a)` after the net inlines `lft`) is therefore copied
  (one increment). Fixture `lent_pick.py`.
* Applying a closure consumes it, so a closure read from a borrowed
  structure is copied first (`dup_val` on a closure is `DUP-LAM`).
* Counts are 32-bit atomics on both runtimes. Saturating 8-bit counts
  (fetch_add then store) let a count pass through zero under concurrent
  increments, so a reader saw a shared root as unique and freed it.
  Fixture `shared_tree.py` at 16 threads.

Refcounting is used only where linearity cannot be proven (Perceus
style). Constructors of any arity move their fields on consume.

### 4.4 Reuse

`mark_reuse`: a constructor built on a call-free straight-line path after
a match consumed a same-arity cell is built in that cell. The token never
crosses a call, a value-position branch or a loop back-edge, and every
path that does not use a token releases it at its terminal.

Inside native code arrays are linear, and the bridge makes an owned array
unique once (`arr_own`), so writes skip the count check. Owned arrays in
native code are consumed at most once on every path, never read after,
and freed at path end.

## 5. Lowering

### 5.1 LIR and printers

`mithril-codegen::lir` is one statement IR: typed locals; `If`, `Switch`
and `Loop` at statement level (value branches lower to a declaration plus
assignments); `Try`, a call that may suspend, with the handler that
leaves the function; `Res`, a two-outcome match; nested cold functions
for suspension captures, whose parameters come from the IR's free
locals. Every emitter builds LIR: dive forms, rule forms and segments,
fold split and join, CALL and hole rules, base-case wrappers, native
scalar forms.

Generated code names the runtime only through a fixed helper vocabulary
(`alloc2`, `alloc_rec`, `deliver`, `dive_to`, `dive_res`, `pop_chain`,
`rec_*`, `mk_con`, `consume*`, `dup_val`, `free_val`, `take_field`,
`arr_*`, ...). The CPU implements it in `mithril_rt::prelude`, generic
over a `Tables` trait the program implements; the device implements it
in `engine.cu`. The program templates (net region, `Program` impl,
`main`) live in `mithril_rt::template`.

The CUDA printer: tuples are `T<k>`/`P2` structs, capture functions are
lambdas, slice arguments are hoisted to local arrays, and C operand
widths come from the IR's declared local types. Device code is compiled
by nvcc in a container and cached by source hash.

Device inlining follows two measured rules. Non-trivial runtime functions
are `__noinline__`: inlining the rule table into every caller produced
437K lines of PTX and a 5-minute ptxas for a 3-function program, against
29K lines and 4 s. Small helpers are inlined: a `__noinline__` call out of
a 200-register function spills through local memory, 1.7M cycles per
tree-node visit (clock64 profile). Float arithmetic on the device uses
the `_rn` intrinsics, so it never contracts into fma; with contraction
nbody printed a different checksum.

### 5.2 Forms

Each function is emitted in the forms its uses need.

* **Dive form** (`d_<f>`): the function runs natively under a budget and
  returns `Result<u64, u64>` in registers: a value, or the record of its
  suspended residue. Suspension code lives in out-of-line cold functions,
  so it never bloats the hot frame.
* **Multi-value dive form** (`n_<f>`): a function returning a k-tuple
  returns `[u64; k]`; `x = g(..)` followed by projections takes the
  components with no heap tuple. A suspension delivers the boxed tuple.
* **Rule form** (segments): the function as rules over records. It dives
  too and allocates records only on suspension. At most 4 dives nest
  inline in one rule-form body before the rest is deferred to a memoized
  record, so generated code is linear in chain length.
* **Native scalar form** (`s_<f>`, `scalar.rs`): a function whose
  parameters and results are ints, int tuples or int arrays becomes plain
  `i64` code, including tuple-valued join points. Array parameters are
  borrowed or owned by fixpoint; tuple results carry an array mask. A
  tuple is held as its leaves: a nested one (a record such as
  `(t, (x, y, z), m)`) is flattened by its layout (`ty::Shape`: every
  leaf an int, nesting at most 8 deep; a tuple parameter holding anything
  else, or a parameter used at conflicting types, keeps the function
  boxed), and a
  projection of a nested component is a view of its leaves. A tuple may
  be used whole (returned, passed on, aliased, joined). Parameter and
  result layouts come from type inference, so a parameter that is only
  passed on still has its width. Bindings are scoped: a branch arm's or a
  let's inner bindings end with it (generated code may reuse a variable
  id in an inner scope). The dive bridge unpacks and packs nested tuple
  cells. `MITHRIL_WHY_BOXED=1` prints why each function has no native
  form.
* **Bounded functions**: a function on no call cycle cannot run out of
  budget, so it is a plain call with no capture.

The native int representation is chosen per function. Plain holds the
canonical i56 in an i64 and re-wraps an op whose range is not proven with
two shifts; masked 32-bit arithmetic runs in u32. Pre-shifted holds
`x << 8`, where i64 wrapping is i56 wrapping, so add, sub, compare and
min need no wrap; a var-by-var multiply, a right shift, an array index
and division cost one op. Each function takes the representation with
the lower static op count, charging conversions on call edges to
functions of the other representation, so a recursive pair never splits.
The worker context is passed only to native functions that touch arrays.

Two static emission rules follow measurements on hand-edited generated
code.

* **Selects as mask arithmetic.** An `if` choosing between computed atoms
  is emitted as `(a & m) | (b & !m)`, which the backend cannot turn back
  into a branch on a loop-carried chain.
* **Length locals.** In a call-free native loop that writes one array and
  also reads another, each array's length is a loop variable, because
  the write may alias the other header and force a reload per access.
  Elsewhere the backend already hoists the header load, and a register
  length blocked vectorization of bfs's fill loop and caused spills in
  its search loop.

The measured gains that justified the scalar and array mechanisms, each
on the tree at the time it was adopted:

| mechanism | measurement |
|---|---|
| native scalar lowering | mandelbrot SEQ 175 s to 4.4 s |
| unboxed ctors, `LIN`, reuse | tree-bitonic 762 G to 254 G instructions |
| register-returned dives, cold capture | tree-bitonic 257 G to 241 G instructions |
| native multi-value returns | bfs SEQ / PAR16 26.7 / 3.72 s to 12.7 / 1.42 s |
| int-element arrays | bfs 12.7 / 1.42 s to 10.4 / 1.13 s |
| native code over int arrays | bfs to 9.65 / 0.94 s |
| static uniqueness | bfs to 5.19 / 0.52 s |
| leaf budget at the call site | bfs to 4.85 / 0.48 s |
| selects as mask arithmetic | bfs to 4.48 / 0.45 s |
| pre-shifted int representation | editdist SEQ 4.05 s to 2.80 s |
| length locals | editdist SEQ 2.80 s to 2.27 s |
| tuples used whole, nested tuples | Cornell Whitted 512 x 512, CPU t16 0.33 s to 0.077 s, device 1.25 s (after the readback fix) to 0.145 s |

### 5.3 Fork sites and the frame split

Parallelism comes from suspension. When a dive runs out of budget, the
pending call is re-spawned as a task and every native frame on the way up
splits its continuation (a let chain around the pending value x):

* P, the bindings independent of x, ending in their live-out. P runs at
  once as a ready record.
* J, the rest, a record with pend 2 that joins x and P's result.

J splits again (`split_dep`): D, the bindings that need x but not P's
result, ending in one live-out m, a pend-1 record fired when x arrives;
and J2, the join of m and P's result. Without D, a call that needs only x
waits for all of P, a sequential dependency the net does not have. On
tree-bitonic depth 16 the two-way split made every merge run
right-then-left, a chain of 2^15 steps and 92,844 device rounds; the
three-way split gave 461. `split_dep` requires one live-out for D and a
J2 that does not read x; otherwise the frame falls back to the two-way
split.

`split_chain` decides the parts by two rules.

* **Projection rule.** A projection of an independent value is a read the
  join does, so it goes on the dependent side and the independent value
  itself is the one live-out. Without it a tuple-returning fork tree had
  k live-outs, did not split and never forked: gameoflife, nbody, queens
  and symreg ran at one-thread speed at 16 threads (gameoflife 8.45 s,
  queens 4.79 s at t16) and fork now (1.12 s and 0.45 s).
* **Multi-value join.** Several live values cross the join as one tuple,
  bound to a fresh variable and projected in J. A 4-way fork is then
  three nested fork sites rather than one fork and two sequential cuts.
  Without it tree-matmul's `gen` had one fork site and the device ran
  49,890 grow sweeps with a flat frontier. Code size grows (tree-matmul:
  77 to 155 segments, recorded in the fast-CI baseline). `split_dep`
  does not use the tuple join.

A **fork site** is a call whose continuation splits. The lowering marks
it by passing `fork_fuel(fuel)` as the callee's budget (`fork_site` in
`seq.rs`; native multi-value callees are fork sites too). The rule form
uses `dive_res_fork` at fork sites and `tail_to` for a segment's tail
call (the sibling in the independent part); `let x = g(..) in x` at any
rule-form position is a tail call. A **cut** is a call whose result the
next statement needs; it runs inline with the caller's budget. On the
CPU `fork_fuel`, `dive_res_fork` and `tail_to` are identities. On the
device they separate the two worlds (section 7.2). The same lowered
program runs on both; only the schedule differs.

### 5.4 TRMC

A function whose tail builds a constructor around a recursive call is
emitted in destination-passing form (`dp_<f>`): a loop that writes each
new cell into the hole of the previous one (`th_head`, `th_hole`,
`hole_wrap`). A suspended TRMC loop carries its hole in the capture.
Fixtures `trmc_list.py`, `trmc_tree.py`.

### 5.5 Budget charging

One budget, two kinds of charge.

* A **dive entry** is one depth unit (`burn_fuel`), charged at the entry
  of every non-leaf dive form, loop form and destination-passing form.
  A fork-site callee handed a zero budget therefore suspends at entry.
  Without the charge at loop-form entries, tree-radix's `merge` (a TRMC
  loop) ran whole on one lane in the parallel world, 6 to 22 G cycles
  per sweep; with it the device run is 373 ms.
* A **loop iteration** and a native leaf call are one work unit
  (`work_fuel`). A loop checks the budget, then charges the iteration,
  so a budget of one still runs one iteration. Charging before the check
  livelocked at budget 1 (the entry charge plus the iteration charge
  re-spawned `main` forever until the record arena was exhausted).
* A call-free function does bounded work: no budget check, one work unit
  charged by the caller as a register decrement.

The CPU burns both kinds: its suspension is the split mechanism that
exposes work to idle workers. The device burns only depth: a dive-form
call refunds its budget when it returns, so the budget bounds native
recursion depth, not work (section 7.3).

Every dive form and every native function whose frames can pile up (a
non-tail self call, or a cycle through other functions) begins with
`stack_guard`: a no-op on the CPU; on the device a comparison of the
stack pointer with the one at kernel entry, which aborts with a named
error ("recursion too deep for the device"). Guarding every non-leaf
native function cost raytrace 3x (`isect` is called 10^8 times), hence
the recursion test.

### 5.6 Fold splitting

`mithril-reassoc` detects folds, proves the combiner associative with an
identity (polynomial normal form), and emits Lean obligations for each
proven fold (section 8). A proven fold heavier than one budget is split:
chunks run in parallel and their results are combined in order. The
split estimate reads the phase's budget.

### 5.7 Core rewrites owned by codegen

Two Core-to-Core rewrites run in codegen, not in the rules:

* `tail_inline`: a small (size at most 64), non-self-recursive `g`
  tail-called from `f`, whose own calls are tail calls back to `f` or
  calls to call-free functions, is inlined at that site, so mutual tail
  recursion becomes a loop.
* `if_convert`: a tail if-tree whose leaves are the function's own
  back-edge and whose arms are cheap (at most 32 ops, 128 bindings, int
  selects only) becomes one call with selected arguments.

The core constraint forbids Core-level rewrites that duplicate what a
rule does. Whether these two are rules, lowering decisions with a stated
cost model, or tunables to be marked is undecided (section 12). The same
holds for the `#[inline(always)]` decision: a call-free body of size at
most 192, plus a `__while`/`__for` helper with a single call site on a
recursive cycle with its caller (without the latter, which member of a
recursive cycle absorbed the other depended on ordering: queens 4.8 s
against 5.3 s).

## 6. CPU runtime

One model on CPU and device: redexes are pending rule applications;
records are continuations waiting for `pend` values; dives run functions
natively under a budget and split on suspension (section 5.3).

**Waves.** Each wave the coordinator merges every worker's spawn buffers
into per-rule buckets and picks the bucket with the largest
`entries x rule_cost` (ties to the lowest rule; anything that may dive
costs a full budget). A bucket whose work is at least 2^14 and that holds
at least two entries is drained across the pool; workers claim blocks of
entries from a shared cursor. Otherwise the coordinator drains it alone,
keeping its caches. Pool workers are spawned once per run and park
between waves.

**Records.** The delivery that completes a record fires it at once on
that worker, up to 64 nested levels; beyond that it waits for the next
wave. A chain of nested joins then completes in one wave. A ready record
(no inputs, the rest of a body after a fork) is queued for the next wave.

**Budget.** Sequential runs use a budget of 2^40 per dive (one dive). A
parallel run uses 16,384 per dive while a wave holds at most four entries
per worker, and `16384 x ceil(entries / (4 x workers))` (capped at 2^10
times) beyond that (`wave_fuel`), because suspension exists to expose
work to idle workers and splitting past that only costs records and
locality. hashmap PAR16 measured 0.49 s without this rule and 0.23 s with
it: every suspension in the batch spine had split off a sibling subtree
until all 2,048 tables were in flight (20 M live cells). The **boost**
covers sequential chains: while a wave's frontier did not grow and is
narrower than the workers, the budget doubles per wave, up to 64x; any
growth resets it.

**Cost model.** On tree-bitonic PAR16 the per-wave barrier is about
70 us, and every dependency hop across a suspension is one wave. The
frontier grows exponentially only while suspended frames have large
independent siblings.

**Arenas.** Cells (16 bytes plus a 4-byte count) and records are carved
from reserved arenas in chunks of 2^16 slots by a global bump; memory is
committed only as chunks are touched, with transparent huge pages
requested. Freed cells and records go on the worker's intrusive free
list: unbounded, no atomic on the alloc or free path. Defaults are 2^26
cells (`MITHRIL_NODES`) and 2^27 records (`MITHRIL_RECS`), at most 2^32
each, and capped at half of the memory available at start, so a runaway
program ends in "arena exhausted", not in swap. Teardown runs in place on
the worker that drops the last reference.

Diagnostics: `MITHRIL_STATS` (cells, waves, rewrites), `MITHRIL_TRACE_PICK`
(every bucket pick with its size, work, boost and time).

## 7. Device runtime

`crates/mithril-gpu/cuda/engine.cu` implements the helper vocabulary, the
rule table and the scheduler. `crates/mithril-gpu/src/runner.rs` sizes the
arenas, launches, waits and reports.

### 7.1 Cost model

A lane's step is expensive. A synthetic fire (about 24 cell allocations,
24 frees, 31 reads, a record, a spawn and a delivery; `bench/gpu/fire_cost.cu`
with the real engine) costs about 30,000 cycles on one thread and about
560,000 cycles per thread with the full grid of 65,536 threads (memory
round trips: 80 cycles per cell read, 300 to 1,000 per allocation). The
model that holds:

    T = sum over rounds of (the slowest lane's steps x cost per step)

Every scheduler rule below reduces the first factor.

### 7.2 One cooperative kernel, grow and work

`k_run` is one cooperative launch (all lanes resident; 32,768 lanes on
the RTX 4090) with grid barriers between phases. A host loop with one
launch and one sync per round cost about 130 us per round; a device round
costs 10 to 20 us. The host launches once and waits.

A round (this grow/work policy is reference's published runtime design,
adopted, not derived here; section 14):

* The leader snapshots the per-rule rings: pending tasks, forkable tasks
  (tasks whose rule can fork, and `ERA`), and the frontier (tasks pushed
  by the last phase).
* **GROW** while some pending task can fork, the frontier is narrower
  than the grow width (default: the lane count), and the last grow sweep
  pushed anything. Every forkable task below the snapshot fires in the
  **parallel world**: a fork site's callee gets `fork_fuel`, a zero
  budget, so it suspends at entry and becomes a task; a segment's tail
  call is a task; a cut runs inline with the phase's budget; the
  continuation is captured as records. The frontier widens by one fork
  level per sweep.
* **WORK** otherwise. Every lane is dealt every nl-th pending task of the
  snapshot (dealt, not claimed: a claim race left 90% of lanes idle) and
  drains it and everything it spawns depth-first on its lane stack in the
  **sequential world**, with the dive budget. After `WORK_STEPS` fires
  (default 2^30) a lane hands its remaining tasks back to the global
  rings.
* The run ends when nothing is pending or the run aborted.

Making every call a task (no cuts) doubles the frontier exactly but
costs a sequential chain one round per step: 100,000 steps took 100,001
rounds and 1.3 s, against 1,563 rounds and 0.3 s with cuts inline.

**Join at once.** A join completed by a delivery runs at once on the
completing lane. A record created in the parallel world is marked `par`:
its continuation runs in the parallel world (`PAR_TASK`), and what it
forks goes to the global rings for the next GROW. A record created in
WORK is the lane's own and continues there. A ready record runs at once
the same way, so a body reaches all its fork sites in one step. Queuing
completed joins globally cost a round per join level: tree-bitonic depth
16 with hand-edited fork sites took 196,511 rounds, and 4,586 with join
at once.

### 7.3 Budgets on the device

* The dive budget is 64 per dive (`MITHRIL_GPU_FUEL`). It bounds native
  recursion depth in dive forms; the stack guard catches the rest. An
  unbounded budget in WORK measured 0.84 s against 1.03 s on
  tree-bitonic depth 23; the bound is kept because of the stack.
* The per-thread stack starts at 8 KiB and doubles when the stack guard
  aborts a run. An aborted run has no observable effect (its result is
  discarded and the retry starts from fresh buffers), so the retry is
  sound. Doubling stops where the device cannot back a deeper stack (the
  driver refuses the limit, or the arenas no longer fit); the run then
  reports the depth error of the last size that ran (64 KiB on the RTX
  4090). Any other error of a retry is reported as it is. The size that worked is kept beside the artefact
  (`<artefact>.stack`, unless `MITHRIL_GPU_STACK` fixed it) and the next
  run starts there. The driver backs the stack with local memory for
  every resident thread, so a large fixed stack costs setup time on every
  run (16 GB eager arenas plus 32 KiB stack: 0.31 s).
* A dive-form call refunds its budget when it returns. The budget bounds
  depth, not the work of a subtree. With a work budget, raytrace's work
  phase suspended every 64 rewrites in every lane's subtree (1.42 s);
  with the refund it runs in 0.06 s.
* Work units do not touch the depth budget. A loop in the parallel world
  runs to its end, as in reference. Every 2^20 work units (`WORK_CAP`) force
  a dive form's next budget check to suspend, so no dive runs unbounded;
  a runaway loop cycles rounds to the round limit instead of freezing the
  device, which the desktop shares. A native loop never suspends: once
  per `WORK_CAP` of its own iterations (a register count per frame) it
  checks the abort flag, and leaves when the run is aborting (an abort,
  or the host's stop at the deadline; a no-op on the CPU). Not covered: a
  native loop whose body calls a function with an inner loop, each under
  `WORK_CAP` iterations per frame, and long non-forking native recursion;
  neither is checked. A counter shared across frames and checked at every
  recursive entry covered them but cost the Whitted demo's kernel 19 to
  32 ms; the per-frame check costs 2 ms.
* The stack guard (`stack_deep` on the device) compares against the
  thread's stack less a 4 KiB margin.
  The host writes the limit before the first launch (`k_boot` runs dives
  too); a limit left at its default faulted small stacks with error 700.

The budget is a per-backend scheduling parameter, not semantics.

### 7.4 Erasure is work

The net's `ERA` rewrites are independent, so teardown is parallel work.
The engine owns one rule past the program's table, `ERA`: a teardown past
256 nodes, or past 32 nested frames (its frames stack on the caller's),
spills its subtrees as `ERA` tasks dealt like any other. The k-d tree's
end-of-run teardown (2^19 cells) cost 3.1 G cycles on one lane, the whole
work phase, as a walk; it is parallel as a rule. The CPU tears down in
place; the rules are the same either way.

### 7.5 Arenas and sizing

| arena | structure | size |
|---|---|---|
| cells | 16-byte cells plus a 4-byte count; per-lane intrusive free list (link in the first word), per-lane bump chunks from a global counter | the rest of the budget at 20 bytes per cell, or `MITHRIL_GPU_NODES` (default request 2^28, capped to fit) |
| records | per-lane intrusive free list (link in `d`), global bump | 2^24 (`MITHRIL_GPU_RECS`) |
| task rings | one ring per rule, a power of two | 2^27 / rules entries, clamped to [2^14, 2^21] (`MITHRIL_GPU_BUCKET`) |
| lane stacks | 64 tasks per lane, overflow to the global rings | fixed |
| net worklists | 64 redex pairs per lane, spill to the program's net rule | fixed |
| array heap | blocks in size classes, 8 per octave (a block is at most 1/8 larger than its array, minimum 8 words); per-lane intrusive free list per class (link in word 0, class in word 1 bits 48 to 55); global bump | a third of the budget, clamped to [2^26, 2^32] words (`MITHRIL_GPU_HEAP`) |

The budget is free VRAM at start, less the fixed buffers, less the
driver's stack reserve for all resident threads (SMs x threads per SM x
the stack size), less 1 GiB of slack.

No free path takes an atomic. A bump-only heap leaked every array block;
bfs and terrain exhausted it at the big size and run (255 ms, 296 ms)
with the free lists. Heap words 0 and 1 are an empty array that is never
freed: an allocation that aborts on a full heap returns it, so no caller
writes past the heap.

Setup cost is paid only for memory a run touches:

* Each buffer states how its pages are committed. The arenas (cells,
  counts, records, task rings, array heap) fill from their start as the
  run allocates: they are managed memory with the device as preferred
  location, committed on first touch. Per-lane tables every lane writes
  as the kernel starts are committed at allocation (demand paging would
  only move the faults into the kernel). `MITHRIL_GPU_EAGER=1` commits
  everything at allocation (a diagnostic).
* No arena is cleared: every refcounted allocation writes its own count.
* A lane's cell chunks start at 64 cells and double, so a lane holds
  what it uses.
* The runner uses the device's primary context. The CLI sets one
  hardware connection (`CUDA_DEVICE_MAX_CONNECTIONS=1` unless set)
  before any thread starts. Runs are serialized (the kernel is
  cooperative over the whole device). A run frees its buffers and module
  and releases the context when it ends; a successful run of a process
  that is exiting leaves that to the driver, and a run past its deadline
  leaves it to the context reset.

Measured on a bare process: one connection 0.05 s; 16 GB eager arenas
with a 32 KiB stack 0.31 s; the same arenas managed 0.06 s.

### 7.6 Bounds

* Round limit: `MITHRIL_GPU_ROUNDS`, default 2^24; a run that does not
  converge stops with an error.
* Deadline: at `MITHRIL_GPU_TIMEOUT` (300 s) the host writes a stop code
  into the abort flag (managed memory, written while the kernel runs, on
  devices with concurrent managed access, and only when no abort is set;
  the host's read and write are not one atomic step, so a device abort
  landing between them is reported as the timeout). Lanes stop at their
  next check and the run ends with a named error.
  Measured: a runaway native loop stops within the deadline plus about
  3 s. A runaway forking recursion does not stop (a dive form that leaves
  early reads as suspended, and its work keeps expanding as tasks): at
  twice the deadline the run returns an error and the kernel is
  abandoned until the process exits, and every later run in the process
  fails at once (it would queue behind that kernel). A context reset does
  not stop a running kernel (measured), so none is attempted.
* A failed run that left a sticky device error (700) in the context
  resets it; a clean abort leaves the context as it is.
* Any arena exhaustion, out-of-bounds index, bad cell index or stack
  overflow sets the abort flag; every lane stops at its next check and
  the host reports the named cause. The first abort wins: a later one is
  a consequence of it. Two independent faults in one run may be reported
  in either order.

Diagnostics: `MITHRIL_GPU_STATS` (setup and run time, rounds, grow sweeps
and work phases with their cycles, widest frontier, cells and records
issued); `MITHRIL_GPU_TRACE` adds the per-round log (phase, pending,
frontier, K cycles, slowest lane, busy lanes) and per-rule pending counts
for the first rounds.

## 8. Verification

Correctness is by construction and checked by oracle. What is proved,
what is checked and what is only claimed:

| property | status | how |
|---|---|---|
| fold reassociation: each proven combiner is associative with an identity; folding chunk results equals folding the concatenation | **proved** | Lean 4 obligations emitted per fold by `mithril-reassoc`, checked by `lean` in `reassoc_test` and `cli_test` (skipped when lean is not installed); the generic `chunked_foldl` lemma proved once |
| specialization preserves meaning | checked | `specialize_test` (every fixture: specialized equals original under `eval_core`); `examples/spec_oracle.rs` bisects a program to the function whose specialization changed its value |
| generated code equals the oracle | checked | `codegen_test` over the fixtures in `crates/mithril-codegen/tests/fixtures`, at 1, 4 and 16 threads and under budget starvation |
| parallel equals sequential | checked | the same tests; fast.py compares t1 and t16 checksums per port |
| both int representations equal the oracle | checked | forced-representation runs (`int_reps.py`) |
| device equals CPU | checked | `tests/ci/gpu.py` (every port at its small size, plus the closure corpus); `MITHRIL_GPU=1 cargo test -p mithril-gpu --release -- --include-ignored --test-threads=1` (the codegen fixtures against the oracle, plus capacity and abort tests) |
| ports compute the right answer | checked | every run's checksum against `bench/expected.txt`, which the C twin and the CPython shim agree with |
| the rule table is confluent | **claimed** | the parallelism argument rests on it; no proof exists |
| the device rule table equals `mithril_core::rules` | **claimed**, checked by tests only | two implementations held together by the oracle |
| lowering preserves meaning | **claimed**, checked by tests only | oracle equality of generated code |

The oracle agrees with the net, not the other way round: an unused
binding is never evaluated. `eval_core` shares payloads (`Arc`), so a
clone copies nothing; a deep copy per variable read cost 21 GB on the
shared-tree fixture. Test suites run under a memory watcher that kills
them past a cap.

Every bug fix lands with a fixture that fails on the tree before the fix.

## 9. Measurement method

**Machine.** AMD Ryzen 7 7800X3D (8 cores, 16 threads), 62 GiB, RTX 4090
(driver 580.82.09), Ubuntu 22.04, rustc 1.86.0, gcc 11.4.0.

**Mithril harness** (`bench/harness.py`, results in `bench/results.md`).
Lanes: `C` (the C twin, `gcc -O2`), `SEQ` (`--threads 1`), `PAR16`
(`--threads 16`), `GPU` (`--gpu`, when `MITHRIL_GPU=1`). Programs are
built once with `mithril build`; build time is reported, not timed. Times
are wall clock, minimum of n runs (n = 1 by default), timeout 300 s.
Every run's output is checked against `bench/expected.txt`. On "arena
exhausted" at defaults the lane is retried with
`MITHRIL_NODES=2^32 MITHRIL_RECS=2^28` and the retry is recorded.

**GPU timing.** The device program is compiled by nvcc in a container on
first use (5 to 77 s per port) and cached by source hash; the harness
runs the GPU lane once untimed to warm that cache. Two numbers exist:

* **kernel time**: the runner's `run` time, from the cooperative launch
  to completion (`MITHRIL_GPU_STATS`). It excludes process start, context
  creation, module load, and arena allocation and clearing, which
  together cost about 0.4 s;
* **wall**: the whole process, warm cache.

**Instruction counts.** fast.py measures a mid-size run with `perf stat`
(noise-free) and compares against `tests/ci/baseline.json`.

**reference** (`bench/reference-notes.md`, results in `bench/reference.csv`). reference
2.0.31 (the exact revision is in `bench/reference-notes.md`), C emitted by
`reference -o`, compiled with clang 21 `-O3` in a container (the generated C
needs clang 19+), run on the host. seq: `--threads 1 --gpu off`; par:
`--threads 16 --gpu off`; gpu: `--gpu <mem>` from reference's own memory
table, the CUDA device code built by NVRTC. Minimum of 3 timed runs after
one warm-up, wall clock including process start; for gpu that includes
context creation and cubin load (0.05 to 0.1 s). The machine was shared:
every run waited for a 1-minute load of 8 or lower; load at run start
averaged 5.1 (max 7.97). This most likely inflates reference's par numbers.
The JS lane was not measured. These are same-machine numbers for
comparison, not a reproduction of reference's own pins (Apple M4 Max), which
are 1.2 to 3.8x faster sequentially than this x86 box.

## 10. Standings

All at the big size, on the machine of section 9. Times in seconds
unless marked ms.

| port | C twin | Mithril t1 | Mithril t16 | reference seq | reference par | Mithril device kernel (wall) | reference gpu wall |
|---|---|---|---|---|---|---|---|
| bfs | 4.56 | 4.83 | 0.432 | 4.543 | 0.397 | 255 ms (0.48 wall) | 0.217 |
| editdist | 2.20 | 2.25 | 0.255 | 2.389 | 0.308 | 212 ms (0.48 wall) | 0.234 |
| gameoflife | 28.29 | 8.45 | 1.121 | 9.706 | 1.146 | 18 ms (0.28 wall) | 0.089 |
| hashmap | 0.769 | 1.82 | 0.237 | 3.112 | 0.288 | fails: a rule ring (2^21) exhausted by a 7.5 M-task frontier | 0.662 |
| kdtree | 0.341 | 0.41 | 0.090 | no port | no port | 603 ms (1.30 wall) | no port |
| kmeans | 10.18 | 11.86 | 1.462 | 5.610 | 0.677 | 339 ms (0.63 wall) | 0.299 |
| lexer | 1.07 | 2.51 | 0.347 | 2.868 | 0.310 | 157 ms (0.39 wall) | 0.348 |
| mandelbrot | 1.95 | 4.08 | 0.850 r | 4.806 | 0.468 | 76 ms (0.29 wall) | 0.093 |
| merkle | 5.78 | 5.43 | 0.601 | 5.463 | 0.535 | 14 ms (0.27 wall) | 0.101 |
| nbody | fails: gcc 11 rejects `musttail` | 5.82 | 0.524 | 6.220 | 0.521 | 4 ms (0.28 wall) | 0.086 |
| queens | 3.91 | 4.71 | 0.454 | 8.118 | 0.911 | 1,204 ms (1.45 wall) | 0.760 |
| raytrace | 9.04 | 7.68 | 0.849 | 7.263 | 0.724 | 58 ms (0.29 wall) | 0.545 |
| symreg | 2.96 | 3.02 | 0.400 | 4.608 | 0.431 | 2,028 ms (2.34 wall) | 0.261 |
| terrain | 4.17 | 4.58 | 0.559 | 3.040 | 0.346 | 296 ms (0.47 wall) | 0.212 |
| tree-bitonic | 8.46 | 10.79 | 1.554 | 10.077 | 1.455 | 990 ms (1.27 wall) | 0.568 |
| tree-matmul | 4.24 | 3.80 | 0.575 | 4.094 | 0.460 | 383 ms (0.70 wall) | 0.225 |
| tree-radix | 2.67 | 4.05 | 0.579 | 4.622 | 0.537 | 373 ms (0.65 wall) | 0.333 |
Conditions:

* C twin: `bench/results.md`, harness at `15f51dd`, one run.
* Mithril t1, t16: `bench/results.md`, harness at `723be4c`, a load
  below 2, min of 3 runs, wall clock (the same conditions as reference's).
* **r**: default arenas exhausted; run with `MITHRIL_NODES=2^32
  MITHRIL_RECS=2^28`.
* reference seq, par, gpu: `bench/reference_quiet.csv` (section 9): reference's
  compiler output rebuilt, a load below 2, min of 3 runs, wall clock.
* Mithril device: kernel time (section 9), warm cache, default arenas,
  at `723be4c`; the wall clock in parentheses is the harness's timing of
  the built artefact (`build --gpu` once, `exec` per run, min of 3), the
  same measurement as reference's gpu lane. The difference between the two is
  the fixed cost: CUDA context creation (73 ms) and teardown of the
  arenas (133 ms), module load, arena setup and readback (about 45 ms).

Warm wall clock on the device, same tree: gameoflife 0.47 s, nbody
0.42 s, kmeans 0.62 s, mandelbrot 0.55 s (reference gpu wall 0.148, 0.090,
0.301, 0.095). On wall clock the small ports are slower than reference
because of the fixed startup cost.

Read plainly:

* **CPU, one thread.** At or under reference seq on 11 of 16 ports. Over it:
  bfs (4.83 against 4.54), kmeans (11.86 against 5.61), raytrace (7.68 against 7.26), terrain (4.58 against 3.04), tree-bitonic (10.79 against 10.08).
* **CPU, 16 threads.** At or under reference par on 5 of 16 ports. Over it:
  bfs (0.43 against 0.40), kmeans (1.46 against 0.68), lexer (0.35 against 0.31), mandelbrot (0.85 against 0.47), merkle (0.60 against 0.54), nbody (0.52 against 0.52), raytrace (0.85 against 0.72), terrain (0.56 against 0.35), tree-bitonic (1.55 against 1.46), tree-matmul (0.57 against 0.46), tree-radix (0.58 against 0.54).
* **Device.** Every measured port is checksum-equal. Wall clock of the
  built artefact is under reference's gpu wall on 1 of 15 measured ports
  (raytrace 0.29 against 0.55) and over it on 14, by 1.1x to 9x. The
  fixed cost (about 0.25 s) is most of the difference on the small
  ports; symreg (17,902 grow sweeps), tree-bitonic (966 rounds), queens
  and tree-matmul are also round-bound, because a sweep counts as growth
  when it pushed anything (section 12). hashmap fails.

## 11. Use cases and scope

### 11.1 Generality corpus

`bench/general/run.py`: programs unlike the suite, each checked against
the Python oracle at a small size and against an idiomatic Rust twin
(`rust/*.rs`, `Rc`/`Vec`, written the way the source is written; nobody
hoists by hand), then timed. Times in seconds, measured by `run.py` when
each program was added; not re-measured on the current tree.

| program | shape | Rust | Mithril t1 | Mithril t16 |
|---|---|---|---|---|
| collatz_mutual | mutual recursion | 0.30 | 0.43 | 0.43 |
| cow_versions | copy-on-write array versions | 1.00 | 0.98 | 0.97 |
| dag_share | heavily shared DAG | 1.68 | 1.87 | 0.29 |
| graph_dfs | DFS over an array of adjacency lists | 0.86 | 1.20 | 0.27 |
| interp | expression interpreter, env as a list | 2.36 | 1.21 | 0.14 |
| persist_map | persistent BST with live old versions | 1.47 | 1.49 | 1.49 |
| sorts | list merge sort and quicksort | 6.21 | 1.41 | 0.60 |
| stage_closure | W2: `mk(k) = λx. x + heavy(k)`, 20k applications | 0.00 | 0.00 | 0.00 |
| pipeline_cfg | stage closures from a runtime config, in a list, over 20k inputs | 0.29 | 0.01 | 0.01 |
| interp_closure | closure compilation of a runtime AST over 20k environments | 0.01 | 5.8 | 6.1 |

Sequentially Mithril is within 1.0 to 1.4x of idiomatic Rust, faster
where Rust pays per-node refcounting; 16 threads are never slower than
one. `stage_closure`: LLVM hoists the pure call out of the loop inside
one function, so both do the work once. `pipeline_cfg` is the thesis at
runtime: Rust cannot hoist across `Box<dyn Fn>`, and the net runs each
stage's setup once. `interp_closure` is the price: every application
copies a closure whose whole body depends on its parameter, so lazy
copying gains nothing and costs about 450x. reference's closures are affine,
so on these shapes reference runs the strict twin, which the Rust column
measures; a reference lane for the corpus is not set up.

### 11.2 The k-d tree probe

`bench/ports/kdtree.py` (C twin `kdtree.c`): a 2-d tree over 2^18 hashed
points built by midpoint splits over a list (subtrees as unequal as the
data makes them, a bucket for coincident points), then 2^18
nearest-neighbour queries with pruning, forked as a batch that only reads
the shared tree. No other port shares a large read-only structure across
every fork, partitions lists, or chases pointers data-dependently.

It is a generality result: the program ran oracle-equal at every size
after six general fixes, none of which looks at the program. They are the
access model of section 4.3 (lent data, int immediates, owned-unless-
aliasing bindings, 32-bit counts), per-lane intrusive arenas, erasure as
a rule, and bounds on both runtimes. CPU (condition b): 0.416 s at one
thread, 0.109 s at 16, C twin 0.341 s. The device run is dominated by the
build's sequential prefix (partitioning a 2^18 list on one lane), a
property of the program's shape that reference shares; the last device
measurement (0.54 s, of which about 0.45 s was the prefix, taken while
another process held 6 GB of VRAM) predates the current device arenas.

### 11.3 Scope: ML runtimes and graph compilers

Fit is by task shape, not by domain.

Fits:

* **Irregular planning.** MoE routing (top-k gating per token,
  capacity-limited dispatch, overflow, load balancing), sparsity
  patterns, graph traversals. This is the k-d tree's shape: a shared
  read-only structure, partitioning, data-dependent branching, balanced
  by the scheduler rather than by atomics and sorts.
* **Collective and fabric algorithms as models.** Ring and tree
  all-reduce, all-to-all for expert exchange, hierarchical reductions over
  a chip topology are local rules on a graph. Mithril expresses and
  checks such an algorithm oracle-equal with the same rules on CPU and
  GPU: an executable specification and a schedule simulator, not a driver
  of real links.
* **Graph-compiler passes.** A rewrite system over an IR is a rule table,
  run by the compile-time reducer.

Does not fit:

* Dense math (matmul, attention, expert FFNs): no tensors, layouts or
  tensor cores.
* Cross-chip execution: no I/O, one device.
* In-place mutable state: arrays are values; ownership reclaims in place
  when it can.

What a combination with Blaze (`../simple-lang`: typed arrays, loops, a
schedule language, native kernels) would need, in order: the static f32
type and f32 arrays; buffers crossing a foreign-call boundary without
copies (an engine rule that hands a buffer out and takes one back); a
probe done like the k-d tree (a MoE router with top-2 gating and capacity
overflow over hashed logits, C twin, oracle-checked, CPU and device)
whose output is a dispatch plan of index buffers. Whether Mithril's role
is to generate graphs that Blaze lowers (the residual net exported as a
graph of Blaze calls on buffers, the irregular structure decided at
compile time or by a residual Mithril region) is undecided.

### 11.4 Demos: a ray tracer and a path tracer

`demos/cornell_whitted.py` and `demos/cornell_path.py` render a Cornell
box (red and green walls, a ceiling emitter, a mirror sphere, a glass
sphere, a diffuse block), written the way a person writes them: f32
vectors as tuples, one function per shape, the image forked as a tree of
rows and columns. Nothing in them names a thread, a device or a task.
`mithril run --image out.ppm` writes the result, `--stats out.json` the
stage times; the same file runs on the CPU and the device.

* Whitted: 512 x 512, 2 x 2 samples, mirror and glass (Schlick's
  Fresnel) to four reflections or refractions, direct light from four
  points of the emitter.
* Path tracing: 256 x 256, 64 paths per pixel, next-event estimation
  plus cosine-weighted bounces (Malley's method in Frisvad's
  orthonormal basis), glass chosen by the Fresnel probability, up to six
  surface hits per path, no Russian roulette. The random numbers are a hash of (pixel, sample,
  bounce), so the image is identical on every run, thread count and
  device.
* Both approximate glass shadows the same way: a shadow ray passes
  through glass dimmed. The path tracer leaves caustics out (after a
  diffuse bounce light arrives by next-event estimation only).

| demo | CPU 16 threads (run) | device (run) |
|---|---|---|
| Whitted, 512 x 512 x 4 samples | 0.077 s | 0.145 s |
| path, 256 x 256 x 64 paths | 0.40 s | 1.06 s |

The device image is byte-identical to the CPU image for both. All ray
code is native (vectors and hit records are nested tuples held in
registers, section 5.2); before that the hot functions were boxed and
the two demos took 0.34 s and 1.37 s on the CPU, 2.73 s and 7.59 s on the
device. The device is still 2 to 3 times slower than 16 CPU threads here
(section 12). A test renders the Whitted
demo at 12 x 12 and checks it against the oracle at 1, 4 and 16 threads.

## 12. Open items

Device:

1. Grow policy: a sweep counts as growth when it pushed anything; it
   should count only when a fork happened. symreg runs 17,902 grow sweeps
   and tree-bitonic 966 rounds.
2. hashmap: per-rule rings of 2^21 entries overflow on a 7.5 M-task
   frontier. Replace them with one chunked shared task ring (reference's
   cube holds 16 M tasks).
3. Fixed startup cost: reduced by the mechanisms in section 7.5. Device
   wall clock, harness, n=3, before and after: gameoflife 0.28 to 0.13 s
   (reference 0.09), nbody 0.28 to 0.12 (0.09), raytrace 0.29 to 0.16 (0.55),
   lexer 0.39 to 0.26 (0.35), terrain 0.47 to 0.29, symreg 2.34 to 1.68.
   The standings table in section 10 predates this.
4. Not measured on the current tree: tree-matmul and kdtree.
5. The grow/work policy (section 7.2) is reference's design. It is to be
   replaced by a policy derived from Mithril's own cost model (section
   14).
6. Register pressure: the whole program is one kernel (`k_run` inlines
   the dispatch of every rule). With the Cornell demos it takes 255
   registers per thread (Whitted; path 144) and spills to a 1,200-byte
   stack, so an SM holds 256 resident threads. Whitted's kernel runs in
   19 ms and its wall clock is fixed cost; the path tracer's kernel takes
   0.97 s. A register budget or out-of-line functions would be a tuning
   knob; it needs a cost model (spill traffic against occupancy) first.

CPU:

7. Values built by the net that reach compiled code keep their fields
   as unresolved wires: `apply` (rt template.rs) resolves only the
   top-level port it returns, and so do the continuation delivery and the
   arguments of a net call fired into a rule form. Compiled code then
   reads a wire as a field. A closure applied at runtime that returns a
   tuple fails ("unprintable result port") or gives a silently wrong
   value (`t = ap(lambda y: (y, y), n, n); t[0] + t[1]` prints a float
   for 6). Programs whose applications the net removes at compile time
   are correct. Fix: one boundary that resolves every field of a net
   value (reducing or suspending on a pending one) wherever a net value
   enters compiled code. Open.
8. kmeans: 11.81 s at one thread against reference 7.84, 1.45 s at 16 against
   0.771. It needs lane-level (u32) vectorization; a hand-edited proof
   reached 5.81 s at one thread.
9. terrain at one thread (4.59 s against 3.21); mandelbrot at 16 threads
   (0.827 s against 0.616, and default arenas exhausted); tree-matmul at
   16 threads (0.66 s against 0.61).
10. `interp_closure`: a closure body with no parameter-free work should be
   applied by compiled code directly, the rule table deciding when.
11. Inference: one heterogeneous array poisons connected ints to `Dyn`.

Semantic core:

12. Readback: a closure created in an arm, capturing a pattern binder and
    applied twice, is an ICE in the reader (the ignored test in
    `specialize_test.rs`). Two specializer crashes on nested closures with
    conditionals (in `mithril-core` `net.rs` and `rules.rs`).
13. The Dup cache is keyed by the whole selection, so a captured shared
    value read under three selections is bound three times (duplicated
    work, not a wrong answer). Keying by the selections a read consults
    fixes it.

Constraint and proofs:

14. `tail_inline` and `if_convert` are Core-level rewrites in codegen,
    with unmarked thresholds (64, 32, 128), and the inline decision uses a
    threshold (192) and a name prefix (`__while`/`__for`). Undecided:
    move into the rules, justify by a stated cost model, or mark as
    tunables with their evidence.
15. Lean proof of confluence of the core rule table: not started.
16. One rule-table source for the CPU and the device: not started.

Measurement:

17. reference re-measured on a quiet machine: not done.
18. The efficiency study: work per port against the C twin and a hand
    CUDA kernel; speedup per core; lanes busy per phase. Not started.
19. The nbody C twin does not compile with gcc 11 (`musttail` placement).
20. A reference lane for the generality corpus is not set up.

Unconfirmed findings from source review (argued from the code, no
failing probe yet): an array leaking through an if-arm tuple mask; a
thread-local read before it is set on the device path; the borrow
inference's round cap.

Planned: the static f32 type and f32 arrays; a Cornell box demo; the MoE
router probe (section 11.3).

Undecided: Mithril's role as the graph generator for Blaze's irregular
and sparse computation (section 11.3).

## 13. Process rules

The working rules are in `CLAUDE.md`. The ones that shape this design:

* One rule table, one semantics, compile time and runtime. Every static
  optimization is a rule firing; lowering never changes meaning.
* One runtime model on CPU and device. A component that is bad is bad
  globally; fix it, do not swap architectures for a local win. A
  work-stealing CPU pool was not adopted for this reason.
* Wins come from program geometry: rewrites, types, lowering. No
  per-benchmark logic; nothing looks at a program's name or shape to pick
  a strategy.
* A change is proven on hand-edited generated code (or a hand net) first,
  then made a rule or a lowering, then tested, with measurements before
  and after.
* Every benchmark is met with general mechanisms only.
* `tests/ci/fast.py` (about a minute) runs before every commit. Per port,
  in parallel: small-size checksum at 1 and 16 threads; the set of
  native-scalar functions must not shrink; generated lines, segment count
  and build time within tolerance; mid-size instruction count within 15%
  of the baseline; arena exhaustion as its own failure class. `--update`
  rewrites the baseline only from a state verified against the expected
  checksums (`--update --only X` refreshes one entry). The gate exists
  because unchecked tuning once dropped mandelbrot off the scalar path
  (6x), double-freed in merkle and made nbody's generated code
  exponential, found only by full runs.
* `tests/ci/gpu.py` (about two minutes warm) runs for device changes.
* The language crates (front, core, net, reassoc, codegen, cli) are held
  under 15,000 lines; they are at 13,239.
* Design decisions are recorded here with the numbers that justify them.

## 14. Prior art and provenance

Mithril stands on published work and says where. An independent review
audited what came from reference. Findings:

* **No runtime or compiler code is copied.** The rule table, lowering,
  CPU runtime and device engine are written here.
* **Device scheduling policy is reference's design** (the reference paper,
  sections 3.1, 3.2, 5 and 6.3): the grow/work round, the frontier as
  tasks pushed by the last phase, the sequential and parallel worlds, a
  completed join run at once in the parallel world, fork-free tasks
  skipped in grow, and the stop rule. It was adopted after reading
  reference's paper and runtime. It will be replaced by a policy derived from
  Mithril's own model (demand-driven sharing: a fork is shared only when
  lanes are idle; classical work stealing, Blumofe and Leiserson 1999,
  Arora, Blumofe and Plaxton 1998), on the CPU and the device together.
* **Device setup** (primary context, one hardware connection, managed
  arenas) is standard CUDA driver usage. The choice was prompted by
  reading reference's host code, before the rule below; each effect was
  measured here (section 7.5).
* **Benchmarks.** Except kdtree (Mithril's own), `bench/ports/*.c` are
  reference's reference C programs (`bench/runtime/<name>/main.c`,
  Apache-2.0), kept verbatim as the C baseline; `bench/ports/*.py` are
  translations of reference's `main.reference` programs. Both carry reference's
  license (`bench/ports/LICENSE.reference`, `bench/ports/NOTICE`). They are
  to be rewritten as Mithril's own benchmark set once the device lane is
  near reference.
* **Mithril's own** (confirmed by the review): fork sites derived from
  the net, suspension on an empty budget as the fork mechanism, borrowing
  and reuse, erasure as parallel work, the stack guard and stack
  doubling, static dealing, per-rule rings, the one-kernel driver.

Rule: reference is a measured baseline. Its runtime source is not read to
design Mithril's.
