# Mithril design

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

## 2. Pipeline

```
Python-subset source
  -> parse, including the f32 elaboration (mithril-front::infer,
     section 4.1)
  -> fold detection and proof on the surface AST
     (mithril-reassoc::analyze, section 5.6)
  -> desugar (loops become tail-recursive functions; an if/match
     statement becomes one join continuation, not a copy of the rest
     per arm)
  -> Core IR (mithril-front::core; eval_core is the reference oracle)
  -> specialization by the interaction rules (mithril-net::specialize):
     each function body is a net over unknown parameters, reduced to
     quiescence and read back as Core
  -> Core rewrite codegen still owns (mithril-codegen::rewrite): tail
     inlining (remaining drift; section 12)
  -> ANF normalization, then over the normalized bodies: type inference
     (monomorphic), unboxing, native-scalar classification, linearity,
     borrowing, reuse marking
  -> lowering to LIR (mithril-codegen::lower -> LirProgram): dive forms,
     rule forms, native scalar forms, the rule table, the net region
  -> printers: lir::rust (rustc -O against mithril-rt, CPU) and
     mithril_gpu::emit_cuda (program.cu against engine.cu, device)
```

Nothing is decided twice. The CPU and the device run the same
`LirProgram`; the printers differ only in syntax and in the runtime they
call. When specialization reduces `main` to a value, lowering emits no
functions: the program is that constant.

The CLI (`mithril-cli`):

* `run f.py [--threads N] [--gpu]` compiles and runs. On the CPU the
  generated Rust is compiled by `rustc -O` against a cached
  `mithril-rt`; with `--gpu` the CUDA program is compiled by nvcc (or
  loaded from the cache) and run by the device runner.
* `run ... --image out.ppm` reads the printed result as
  `(width, height, pixels)`, the pixels any nesting of tuples or
  constructors read depth first, each `0xRRGGBB` or an `(r, g, b)` of
  0 to 255, and writes a binary PPM. `--stats out.json` writes the
  backend, thread count, front-end, compile and run seconds, device
  rounds and image size.
* `build f.py -o out [--gpu]` writes the CPU binary, or the device cubin
  (a program that reduced to a constant writes `MITHRIL-CONST <value>`).
  `exec out` runs a device artefact with no front end.
* `net f.py` prints what specialization did per function (section 3.2);
  `prove f.py` writes and checks the Lean obligations of the proven
  folds (section 8).
* `MITHRIL_TIMING` prints the stage times.

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

At runtime a constructor or float cell may be shared (refcount > 1)
between compiled code and a closure in the net region. `Prog::shared`
says so, and a rule consuming such a value (MAT, DUP, ERA, and OP on
floats) copies its fields out and drops one reference instead of freeing
the cells; the device rules do the same (`shared_val`). Compile time never
shares. Fixtures `con_share`, `flo_share`, `flo_share_ops`.

The device carries a second, hand-written implementation of the same
rules in `crates/mithril-gpu/cuda/engine.cu`. It is held to the first by
the oracle tests (section 8). One source for both is an open item.

### 3.2 Specialization: the net as the optimizer of record

Constant folding, inlining, branch selection, static evaluation and
unrolling are not passes. They are the rules firing early, on redexes
that do not depend on runtime input. `mithril net f.py` prints what
reduction did per function (rewrites, calls kept, ops kept, calls
evaluated, size before and after). `MITHRIL_NET_CORE=1` dumps the bodies.
The reduction of each function runs under 2^20 rewrites (`REDUCE_FUEL`
in the CLI); running out is an internal error.

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
call of its own remains, no match on a runtime value, no new constructor
other than a tuple (a tuple is loop state), and growth within 2,000
agents (ops, branches, data; wires are free). A data builder is not code to unroll: symreg's
`gen(5, ..)` doubled the program for no instruction gain, hence the
constructor condition. Budgets: 50,000 rewrites per top-level attempt,
shared by everything nested in it; 400,000 speculative rewrites per
function (each top-level attempt is also charged the cells it cloned);
nesting depth 256. A failed attempt is memoized by (callee, which arguments are known).
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
where sharing or evaluation order needs it.

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
built, once. The clone discipline (`check_clone_discipline` in
`mithril-net`) detects a closure applied to itself (`App(f, Var v)` with
`v` free in `f`, through `Let` aliases) and accepts `f(f(x))`,
factories, closures capturing closures and `map` with a lambda. Only
`reduce_test` runs it; the compiler does not, so a self-application
compiles (section 12).

The runtime runs the same table. A compiled program has a net region:

* an entry table (`net_entries`): the closures compiled code builds and,
  transitively, the branches and arms their bodies mention. A `Ref` to a
  compiled function whose arguments are all produced spawns its CALL
  rule, and a FILL record links the result back into the waiting wire. A
  call met before its arguments exist (inside a closure body being
  built) runs the callee's body as a net, so the functions that
  net-region bodies call are shipped in the region too
  (`net_live_entries`);
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
straight-line net builders (`inst_<e>`) from the same `NExpr` bodies
the CPU interprets.

A value the net builds can still hold fields on unfilled wires when it
reaches compiled code (a tuple a closure returns, whose parts wait on a
compiled call delivered later). Compiled code reads fields as values, so
every entry from the net into compiled code settles the value first:
`apply`, the delivery of a `Kont`, and a net call fired into a rule form.
`settle` (`mithril-core` `rules.rs`) follows each field's wire, writes
the value back into its slot, and returns the fields still pending. When
none is pending the value is delivered at once. Otherwise three records
finish it. Like FILL, they are bridge records of the runtime, not
interaction rules: they move values between the net and compiled code
and never rewrite the net.

* FIELD: one record per pending field, fed through a `Kont` by that
  field's wire. A `Kont` delivery settles first, so the value it receives
  is whole; it writes the value into the field's slot and counts down its
  parent.
* WHOLE: the parent, holding the value; it delivers the value when its
  count reaches zero.
* RELINK: a net call whose top-level arguments exist but whose fields
  are pending waits for them as a whole value, then is pushed as a redex
  (not linked: a link would park the call in the wire as if it were the
  value).

A record created in a dive (WHOLE from `apply`, like the forwarding
record) gets its parent after the dive returns (`set_parent`). That is
safe because a record's inputs are produced by work this dive spawned,
and that work runs after the dive returns: on the lane's own stack, or
after the next wave or barrier. The CPU and the device implement the
same three records (one runtime model). The CPU counts the pending
fields first and allocates the parent with that count. The device
allocates the parent at the first pending field with one guard count,
adds one per field and drops the guard at the end; if every field
already arrived, the value is whole and the record is freed unfired.
Both walk the value head before tail, holding only constructors not
yet visited, so the worklist grows with left nesting, not with a list's
length.

Cost: settling walks the whole value at every crossing. A closure
returning an n-element list pays n steps per application, and a loop
that grows an accumulator through a closure pays O(n^2) over the loop.
`settle` does not walk array elements (section 12).

Oracle tests: the `net_values`, `net_lists` (a 200-element list, a
40-field tuple) and `closure_parts` fixtures, at 1, 4 and 16 threads,
with a starved budget, and on the device.

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

The mechanism (`ty::infer`): union-find over type nodes, two walks over
every normalized body, then a readout walk. After each walk `settle`
resolves projections recorded before their base was known, and `relink`
runs to a fixpoint: an int or float op records (operand, fresh result,
kind), and a result whose operand no longer has that kind (a later walk
poisoned it) is unified with the operand again; a conflict made by the
result's own consumer does not flow back. A comparison's result is
always a fresh int. A conflict, found by `unify` or by a direct `set`,
poisons both classes and everything read out of them (tuple components,
array elements) through a worklist that marks a class before visiting
its parts, so it ends on self-referential types (a list of pairs, an
array of arrays; fixture `self_types.py`). `App` yields a poisoned node
and `Lam` a function node; a lambda's body is still walked, so its
first-order parts get types (fixtures `closure_result.py`,
`closure_parts.py`). A poisoned class reads as `Dyn`.

From the result, codegen reads two more facts per function:

* `pshape` and `rshape` (`ty::Shape`): the layout of each tuple
  parameter and tuple result, a tree whose leaves are all ints, at most 8
  deep. A tuple with a non-int leaf, deeper nesting, or a type that
  reaches itself has no layout. `width()` is the leaf count and
  `offset(i)` the first leaf of component i.
* `pmixed`: a parameter whose class was poisoned by conflicting uses (an
  int in one place, a tuple, float or constructor in another). Its
  function gets no native form (section 5.2).

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
language). Unary minus on f32 flips the sign bit. The helpers (`__f32_eq`,
`__f32_of_int`, `__int_of_f32`) are language source; only the ones a
program uses are appended to it. A name that a local, function or
constructor shadows is not the builtin. A lambda has an opaque type
(f32 meeting it is an error); its parameters are fresh, and applying a
local closure gives a fresh type.

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
* A destructuring `a, b, ... = e` checks that `e` is a tuple of that
  many components.
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
* **Multi-value dive form** (`n_<f>`): a function returning a k-tuple,
  k from 2 to 8, returns `[u64; k]`; `x = g(..)` followed by projections
  takes the components with no heap tuple. A suspension delivers the
  boxed tuple. Not for `main`, nor for a function that has a native
  scalar, destination-passing or base-case form (`MITHRIL_NO_NTUP`
  disables it).
* **Base-case wrapper** (`q_<f>`, `fast.rs`): for a self-recursive dive
  function, a wrapper derived from its own body. It follows matches on
  parameters whose arms carry no cell, and pure int code; when the
  arguments reach a call-free, allocation-free tail it returns that
  value inline, otherwise it calls `d_<f>`. Every fallback happens before
  any side effect. Dive-form call sites call `q_<f>`.
* **Rule form** (segments): the function as rules over records. It dives
  too and allocates records only on suspension. At most 4 dives nest
  inline in one rule-form body before the rest is deferred to a memoized
  record, so generated code is linear in chain length.
* **Native scalar form** (`s_<f>`, `scalar.rs`): a function whose
  parameters and results are ints, int tuples or int arrays becomes plain
  `i64` code, including tuple-valued join points. It is ruled out by an
  f64 or constructor local, a non-int array parameter or result, a tuple
  parameter without an int layout, or a `pmixed` parameter (section
  4.1). f32 values are ints by then, so f32 code stays native. Native
  code never suspends, so a function that forks (forking recursion or a
  proven fold), and every function that transitively calls one, runs in
  dive form instead; its leaves still call native code. Recursion counts
  as forking when one activation can make two or more direct self calls:
  calls in sequence add up, the arms of a branch are alternatives (the
  larger count counts). A function recursing once per arm is linear and
  stays native. Counting alternatives as forks boxes the path tracer's
  `path` and every caller: measured 992 against 66 ms for the device
  kernel, 0.40 against 0.171 s at 16 CPU threads. Native recursion runs
  on the native stack with no budget cut-off, as plain linear recursion
  does (section 12). Array parameters are
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
* **Bounded functions**: a function on no call cycle, or with a native
  scalar form, cannot run out of budget, so it is a plain call with no
  capture.

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
* **Multi-value join.** When the independent calls are all calls to one
  function (the siblings of a fork tree), several live values cross the
  join as one tuple, bound to a fresh variable and projected in J. A
  chain of calls to different functions keeps the one-live-out rule:
  splitting every frame of it costs code cubic in its length. A 4-way fork is then
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

A native loop checks nothing per iteration. A stop check in a hot loop,
even one that reads the abort flag once per 2^20 iterations, cost
raytrace's device kernel 5.7x (385 ms against 68 ms; the likely cause is
that it blocks the backend's unrolling of the short counted sphere
loops). The loop scaffold (the back edge and its one fuel unit per
iteration) is emitted only for a function that calls itself, in the
native and the dive form (`self_tail_rec` holds vacuously for a
function with no self call). A runaway native loop is stopped with its
process (section 7.6): it holds the device until the run is abandoned
at twice `MITHRIL_GPU_TIMEOUT` (600 s by default), and the CLI then
exits.

### 5.6 Fold splitting

`mithril-reassoc` detects folds, proves the combiner associative with an
identity (polynomial normal form), and emits Lean obligations for each
proven fold (section 8). The accepted combiners are wrapping add (mod
2^56, or the low 32 bits) and the same elementwise over a tuple
accumulator whose entry value is the identity; anything else is
declined. A proven fold's CALL rule splits its range into a binary fork
of records while (hi - lo) x est exceeds max(budget, 256), where est is
the measured budget per iteration (`FOLD_EST_<f>`, taken when a chunk's
dive runs out of budget; 1 until then). A join rule combines the two
partial results in order.

### 5.6.1 Independent loop iterations

A `for` loop that ends in `s = E(s, ..)`, whose other work never reads or
writes `s` and calls a user function, is split by `mithril-reassoc`
(`split.rs`). The work runs as a balanced tree of independent calls; each
leaf is a generated constructor holding that iteration's values. The tree
is then walked left to right, applying `E` in iteration order, so the
result is the sequential one. A loop is not split when its function is
already reached from parallel work: a split loop's work, a proven fold's
body, or a function that calls itself twice on one path.

The apply walk is native code: the scalar lowering accepts constructor
values and a `match` on them in tail position (tag dispatch). A value the
dive side lends is read in place (`PTy::H`); an owned one has exactly one
use and is consumed cell by cell as the dive form does (`PTy::O`). Measured on the plain-loop demos:
path tracer CPU 1.61 s at 1 thread, 0.168 s at 16; GPU 0.27 s (the
hand-split tree form: 0.20 s). Whitted GPU 0.51 s against 0.19 s for the
tree form: the in-order apply is one serial chain on one GPU thread
(about 1.2 us per pixel).

### 5.7 Core rewrites owned by codegen

One Core-to-Core rewrite remains in codegen, not in the rules:

* `tail_inline`: a small (size at most 64), non-self-recursive `g`
  tail-called from `f`, whose own calls are tail calls back to `f` or
  calls to call-free functions, is inlined at that site, so mutual tail
  recursion becomes a loop.

Runtime branch back-edges stay in the residual program. The former
fixed-threshold if-conversion pass speculated both arms outside the rules;
it has been removed. `branch_lowering_test` checks that lowering preserves
those branches, including nested arms and an untaken division by zero.

The core constraint forbids Core-level rewrites that duplicate what a
rule does. `tail_inline` remains a drift to resolve (section 12). The same
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

Synchronization. Per redex and per record the runtime uses only atomic
counters (`mithril-rt/src/sync.rs`: the join counter, the wave's claim
cursor, refcounts). The control plane takes a mutex, a read-write lock and
a condvar once per wave per worker (parking, the wave's shared slices,
each worker's context).

**Records.** The delivery that completes a record fires it at once on
that worker, up to 64 nested levels; beyond that it waits for the next
wave. A chain of nested joins then completes in one wave. A ready record
(no inputs, the rest of a body after a fork) is queued for the next wave.

**Budget.** Unless the program is given one (`--fuel N`), sequential
runs use a budget of 2^40 per dive (one dive; the runner thread has a
1 GiB stack, so recursion depth, not the budget, is the bound). A
parallel run uses 16,384 per dive while a wave holds at most four entries
per worker, and `16384 x ceil(entries / (4 x workers))` (capped at 2^10
times) beyond that (`wave_fuel`), because suspension exists to expose
work to idle workers and splitting past that only costs records and
locality (`MITHRIL_FIXED_FUEL` turns this off). hashmap PAR16 measured 0.49 s without this rule and 0.23 s with
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
requested. A freed cell goes on the worker's intrusive free list (the
link in cell word 0), a freed record on the worker's stack of free
record indices: both unbounded, no atomic on the alloc or free path. Defaults are 2^26
cells (`MITHRIL_NODES`) and 2^27 records (`MITHRIL_RECS`), at most 2^32
each, and capped at half of the memory available at start, so a runaway
program ends in "arena exhausted", not in swap. Teardown runs in place on
the worker that drops the last reference.

**Result.** When the run ends, `show` walks the port delivered to the
root and prints it: ints as numbers, f64 with Rust's `{:?}`, closures as
`<closure>`, tuples as `(..)`, constructors as `C<k>(..)` (an unboxed
one as `C<cid>(v)`), arrays as `[..]`. Any other port is an internal
error ("unprintable result port").

Diagnostics: `MITHRIL_STATS` (peak and live cells, waves, rewrites, live
arrays), `MITHRIL_TRACE_PICK` (every bucket pick with its size, work,
boost and time), `MITHRIL_CHECK_FREE` (double-free check, debug builds). Lowering
switches for probes: `MITHRIL_PLAIN_INTS` forces the plain int
representation, `MITHRIL_NO_TRMC` and `MITHRIL_NO_NTUP` turn those forms
off, `MITHRIL_WHY_BOXED` (section 5.2), `MITHRIL_DBG_FORK` prints the
fork-site decisions.

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

`k_run` is one cooperative launch with grid barriers between phases.
All lanes are resident: blocks of 256 threads, as many per SM as the
kernel's occupancy allows, at most 65,536 lanes (`MITHRIL_GPU_LANES`
caps them; one lane is the determinism probe). A host loop with one
launch and one sync per round cost about 130 us per round; a device round
costs 10 to 20 us. The host launches once and waits.

A round (the phase structure was adopted earlier; section 14):

* The leader snapshots the per-rule rings: pending tasks, forkable tasks
  (tasks whose rule can fork, and `ERA`), and the ready frontier (the
  total pending count). Each forkable rule keeps its largest ready count
  seen in this GROW episode. Newly pushed tasks are traced separately.
* **GROW** while some pending task can fork, the frontier is narrower
  than the grow width (default: the lane count, `MITHRIL_GPU_GROW_WIDTH`),
  and at least one forkable rule's ready count exceeds its episode's
  high-water mark. A newly ready rule can grow even when the total count
  contracts or stays unchanged. Replacements and oscillation cannot keep
  reusing an old peak. Boot and the snapshot after WORK seed a fresh episode.
  Every forkable task below
  the snapshot fires in the **parallel world**: a fork site's callee
  gets `fork_fuel`, a zero budget, so it suspends at entry and becomes a
  task; a segment's tail call is a task; a cut runs inline with the grow
  budget (`MITHRIL_GPU_GROW_FUEL`, default the dive budget); the
  continuation is captured as records. WORK reopens the opportunity to
  grow after it executes the exposed work; existing budgets remain in force.
* **WORK** otherwise. Every lane is dealt every nl-th pending task of the
  snapshot (dealt, not claimed: a claim race left 90% of lanes idle) and
  drains it and everything it spawns depth-first on its lane stack in the
  **sequential world**, with the dive budget. After a number of fires
  (`MITHRIL_GPU_WORK_STEPS`, default 2^30) a lane hands its remaining
  tasks back to the global rings.
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

**Measured frontier bottleneck (2026-10-01, RTX 4090).** The old any-push
stop rule kept one-for-one continuations in GROW, paying a grid barrier for
replacement work. Global ready-count growth fixes that chain but misses a new
subproblem whose arrival coincides with siblings finishing. Matmul's trace
contracts from 24,576 tasks to 384 tasks of a different rule; draining those
384 before they branch costs parallelism. Treating every contraction as
progress restores matmul but revives symreg's false growth. Per-rule high-water
marks retain new ready populations without permitting that oscillation.

Hand-edited copies of exact generated CUDA first tested this mechanism.
A rotating three-way comparison gave symreg 2,040.988 ms on the old engine,
1,662.730 ms with strict total-count growth and 1,668.115 ms with rule peaks;
the latter uses 560 rounds instead of 17,904. Matmul gave 423.693, 463.835 and
424.628 ms respectively, keeping its original 90 rounds. All checksums agree.
The contraction-only alternative was rejected: its normal generated symreg
interval returned to 1,979 ms, losing the gain despite passing result tests.

Final generated-path measurements are recorded separately below. Both sides
use the same compiler, including the pre-existing working-tree fork-dependency
analysis, with Core if-conversion removed. Only device growth accounting
differs. Warm built artifacts run through one host runner in three rotating
before/intermediate/final rounds. Minima are selected separately for wall and
run; build time is excluded. `MITHRIL_GPU_STATS=1 mithril exec <artifact>` reports
the CUDA stream interval between run events (including host enqueue gaps),
excluding boot. Wall includes setup, boot, readback and teardown.

| program | original run ms | strict-total run ms | final run ms | original wall s | final wall s |
|---|---:|---:|---:|---:|---:|
| tree-matmul | 421.346 | 459.480 | 424.812 | 0.590 | 0.592 |
| symreg | 2043.449 | 1674.586 | 1695.410 | 2.165 | 1.816 |
| queens | 1230.849 | 1242.404 | 1235.605 | 1.348 | 1.350 |
| nbody | 7.412 | 7.408 | 8.376 | 0.116 | 0.117 |

Symreg's final run interval drops 17.0%, wall 16.1%, and rounds from
17,904 to 560. Matmul recovers the strict-total regression (7.5% less run
time than that variant), staying within 0.8% of the original. Queens is
unchanged within 0.4%. Nbody's minimum rises 0.964 ms (13.0%), with nearly
unchanged wall; this is not a universal speedup. No final whole-suite speed
claim is inferred from these four cases. `paired-rule-final.json` and
`rule-final-provenance.json` retain every sample and the final hashes.


The focused regression runs 64 combinations: same-rule and oscillating
continuations, a newly ready binary-tree rule, 0/127 finishing siblings,
0/1/128 delay, depth 0/8, ring-counter wrap and WORK budgets of one/default.
All return the independent expected leaf count. Default-budget rounds remain
bounded; newly ready branching still grows. The original engine fails the
chain upper bound; global-count variants fail the new-rule branch lower bound.
Both failures were reproduced before promotion. The final production device
gate passes all 17 small ports and three closure programs; the focused GPU
test passes all 64 runs. The prior full workspace and 42-fixture GPU-oracle
checks covered the unchanged lowering/rules, and final timeout checks cover
stop/cleanup behavior separately. Broad CPU parity covers 596 programs at
eight settings. Its only 16 reference mismatches are the two Cornell demos'
pre-existing tuple-to-array output change; all 16 exactly match the pre-change
binary. No reference file was rewritten. Local samples, traces, source/
binary/cubin hashes and measurement scripts are in `target/gpu-improve/`;
rejected variants are retained there, outside the source tree.

#### Standalone continuation-storage probe (2026-10-01)

A locally archived standalone probe preserved the selected prefix space and
checks 863,992,044 visited candidate nodes, 6,899,189 solutions and checksum
2063750025 against a recursive CPU oracle. Three rotating rounds give a
6.736 ms minimum work interval with shared-memory continuations and an
occupancy-sized frontier (row 6), versus 1221.223 ms for the generated artifact's
`k_run`. These are device work intervals, not process wall times. The standalone
example is not generated by Mithril and does not implement its fuel/stop contract.

The controlled row-8 comparison gives local pending frames 17.782 ms minimum
search time, bounded register recursion 27.825 ms, and shared pending frames
5.115 ms. The shared layout is `[depth][field][lane]`; running state stays in
registers and only unfinished siblings are saved. At profiler clocks the shared
search takes 6.151 ms against a 3.601 ms instruction-inventory capacity bound:
58.5% of that mapping's ceiling. L2 throughput falls from 86.6% of peak for local
pending frames to 0.60%; VRAM bandwidth is 1.86%. This bound applies to measured
instruction/traffic inventory, not every possible Queens implementation.

Integration needs a lowering change as well as runtime storage. Generated Queens
uses native `s_0 -> s_5 -> s_0` recursion; its frames are not the runtime's
`G.lstk` suspended tasks. The proposed first case is a common typed continuation
representation for native recursive regions, preserving return points, live
values, ownership and work charges. CPU and GPU execute the same representation;
the GPU caches lane-owned frames in bank-distributed shared storage. Overflow,
suspension and cross-lane escape must materialize valid existing records. Shared
capacity must be included in both occupancy and cooperative launch configuration.
No kernel-local address may escape into a globally visible continuation.

Deeper frontier expansion is a separate proof obligation. Retaining the original
four-row frontier gives 59.680 ms standalone shared search time; the larger gain
uses independent deeper subtree counts. The port threads accumulators through
recursive calls, so that decomposition needs an applicable rule-backed rewrite.
Existing proven range-fold splitting does not by itself prove this candidate-set
recursion can split. First validate native frame lowering with unchanged source
and decomposition, including forced spills, one-lane execution, both integer
representations and oracle equality; then treat deeper splitting as its own case.
The probe and its measurement receipts are retained locally; they are not part
of the published examples.

The first integration attempt was rejected before promotion. It implemented a
common native continuation machine, live-value captures, structured loop backedges
and a GPU shared ring with cell-chain overflow. The unchanged full Queens program
returned the correct checksum, but its final `k_run` took 2063.840 ms versus
1333.960 ms for a fresh run of the previous artifact. A separate outlined-region
probe took 1978.682 ms and 2151.968 ms on recheck. These are diagnostic samples,
not a statistical performance comparison; none establishes a runtime speedup.
The automatic cache held 20 words per lane (40 KiB per block, one block per SM).

The archived candidate passed 61 existing codegen tests (one existing ignore),
two new native tests including 32 generated executions across both integer
representations, thread counts and fuels, and a GPU depth-600 test with a fixed
8 KiB hardware stack at zero/one/automatic cache capacity. Full workspace and GPU
gates were not run on the rejected candidate. It also exceeded the language
budget at 15,312 lines. Its source, patch, tests, artifacts and measurements are
kept locally in `target/native-continuations/`; production files were restored.
The integration remains unfinished. Explicit frames plus shared storage alone
have not reproduced the standalone benefit, so this design needs further evidence
before another production implementation is justified.

### 7.3 Budgets on the device

The budget is a per-backend scheduling parameter, not semantics.

### 7.4 Erasure is work

The net's `ERA` rewrites are independent, so teardown is parallel work.
The engine owns one rule past the program's table, `ERA`: a teardown past
256 nodes, or past 32 nested frames (its frames stack on the caller's),
spills its subtrees as `ERA` tasks dealt like any other. The k-d tree's
end-of-run teardown (2^19 cells) cost 3.1 G cycles on one lane, the whole
work phase, as a walk; it is parallel as a rule. The CPU tears down in
place; the rules are the same either way.

Boxed arrays use the same node/depth budget. The last reference decrement
starts an element walk; on budget exhaustion one ERA continuation owns the
remaining suffix. Its second payload is the next element index plus one
(zero denotes ordinary value erasure). Resuming does not decrement the
reference count again, and the array block is freed only after its last
element. This bounds queued work by exposed subtrees, rather than array
length: an eager teardown exposes every remaining element as a task
(hashmap: 7,569,521 ERA tasks, over a 2^21 ring); with the continuation
hashmap's widest frontier is 4,096, with the same 33,423,361 cells and
checksum 1307803744. The continuation is one chain: a single huge boxed
array tears down over as many rounds as it has budget-sized chunks
(section 12). No ring enlargement
or program-specific decision is needed. CPU array erasure already walks
elements in place; ownership and ERA semantics are identical.

### 7.5 Arenas and sizing

| arena | structure | size |
|---|---|---|
| cells | 16-byte cells plus a 4-byte count; per-lane intrusive free list (link in the first word), per-lane bump chunks from a global counter | the rest of the budget at 20 bytes per cell, or `MITHRIL_GPU_NODES` (default request 2^28, capped to fit) |
| records | per-lane intrusive free list (link in `d`), global bump | 2^24 (`MITHRIL_GPU_RECS`) |
| task rings | one ring per rule, a power of two | 2^27 / rules entries, clamped to [2^14, 2^21] (`MITHRIL_GPU_BUCKET`) |
| lane stacks | 64 tasks per lane, overflow to the global rings | fixed |
| net worklists | 64 redex pairs per lane, spill to the program's net rule | fixed |
| array heap | blocks in size classes, 8 per octave (a block is at most 1/8 larger than its array, minimum 8 words); per-lane intrusive free list per class (link in word 0, class in word 1 bits 48 to 55); global bump | a third of the budget, clamped to [2^20, 2^32] words (`MITHRIL_GPU_HEAP`) |

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
  cooperative over the whole device). A run frees its buffers and module and
  releases the context when it ends; a successful run of a process
  that is exiting (`run --gpu`, `exec`) leaves that to the driver's exit
  path, and a run abandoned at twice its deadline (section 7.6) frees
  nothing, because every free would wait for its kernel.

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
  next scheduler check and the run ends with a named error. A lane inside
  a native loop or a runaway forking recursion (a dive form that leaves
  early reads as suspended, and its work keeps expanding as tasks) does
  not stop: at twice the deadline the run returns an error and the
  kernel is abandoned until the process exits, and every later run in
  the process fails at once (it would queue behind that kernel; a
  process-wide `STUCK` flag). The CLI runs one program per process, so
  its exit ends the kernel. A context reset does not stop a running
  kernel (measured), so none is attempted. Native loops carry no stop
  check: it cost raytrace's kernel 5.7x (section 5.5).
* A failed run that left a sticky device error (700) in the context
  resets it; a clean abort leaves the context as it is.
* Any arena exhaustion (cells, records, a rule ring, the array heap),
  out-of-bounds index, cell index outside the arena, stack overflow,
  unreachable match arm, unsupported device feature, cell walk past
  2^22 steps (a corrupted arena) or the round limit sets the abort flag;
  every lane stops at its next check and the host reports the named
  cause. Settling a net value (section 3.4) walks it with a 64-entry
  worklist that holds only constructors not yet visited, head before
  tail, so a list of any length settles; a value nested more than 64
  deep on the left aborts as unsupported (the CPU's worklist is
  unbounded). The first abort wins: a later one is
  a consequence of it. Two independent faults in one run may be reported
  in either order.

**Readback.** After the kernel the host reads the port delivered to the
root (a run that delivered none is an error). `show` follows the result's
storage: immediate ints, unboxed and nullary constructors, and closure
markers need no cell transfer. Arrays are read through their heap headers;
only an element that refers to a cell demands the cell arena. At the first
cell read the host takes one bulk snapshot, shared by the rest of the walk.
This preserves aggregate throughput (one transfer per cell cost about
1.7 s for a 512 x 512 image) without transferring dead intermediate cells
for a scalar result. The cost is zero cell bytes for a cell-free result,
otherwise the allocation high-water range, independent of program names.
The full suite transfers zero cell bytes for all 17 scalar ports; the
two aggregate demo results retain their bulk snapshots.
Justification: copying the high-water range for a scalar result cost
tree-matmul 1.64 GB and 556 ms, and tree-radix 2.11 GB and 708 ms; with
the result-directed walk their device wall clocks are 0.565 and 0.650 s,
merkle's 0.142 s, all checksum-equal.
The CPU already walks only the root value; neither lowering nor the rule
table changes.

Diagnostics: `MITHRIL_GPU_STATS` (context creation and release, module
load, arena setup, run time and readback, rounds, grow sweeps and work
phases with their cycles, widest frontier, cells and records issued, stream-event
intervals for boot and run, readback time and cell bytes);
with it, `MITHRIL_GPU_TRACE` adds the per-round log (phase, pending,
frontier, K cycles, slowest lane, busy lanes), per-rule pending counts
for the first rounds, and a histogram of lanes by cycles for the last
work phase. `MITHRIL_GPU_CU` runs a hand-edited `program.cu`;
`MITHRIL_GPU_NET_FUEL` (default 4,096) is the rewrites a lane's net
reduction runs before spilling to the net rule;
`MITHRIL_GPU_POISON` and `MITHRIL_GPU_DEBUG` are probes.

## 8. Verification

Correctness is by construction and checked by oracle. What is proved,
what is checked and what is only claimed:

| property | status | how |
|---|---|---|
| fold reassociation: each proven combiner is associative with an identity; folding chunk results equals folding the concatenation | **proved** | Lean 4 obligations emitted per fold by `mithril-reassoc`, checked by `lean` in `reassoc_test` and `cli_test` (skipped when lean is not installed); the generic `chunked_foldl` lemma proved once |
| specialization preserves meaning | checked | `specialize_test` (every fixture: specialized equals original under `eval_core`); `examples/spec_oracle.rs` bisects a program to the function whose specialization changed its value |
| generated code equals the oracle | checked | `codegen_test` over the fixtures in `crates/mithril-codegen/tests/fixtures`, at 1 to 16 threads (the counts vary per test) and under budget starvation (budgets of 1 to 64) |
| parallel equals sequential | checked | the same tests; fast.py compares t1 and t16 checksums per port |
| both int representations equal the oracle | checked | the `int_reps` codegen test: fixture `int_reps.py` with each representation forced |
| device equals CPU | checked | `tests/ci/gpu.py` (every port at its small size, plus the closure corpus); `MITHRIL_GPU=1 cargo test -p mithril-gpu --release -- --include-ignored --test-threads=1` (the codegen fixtures against the oracle, plus capacity and abort tests) |
| ports compute the right answer | checked | every run's checksum against `bench/expected.txt`, which the C twin and the CPython shim agree with |
| every redex order reaches the same value | checked on samples | `mithril-net/tests/schedule_test.rs`: 10,000 random programs (arithmetic, branches, a shared closure applied twice, a shared list consumed twice, tuples), each reduced by `mithril_core::rules` under 5 sampled redex orders, equal to `eval_core` |
| every redex order takes the same number of rewrites | **false** for the extended table | most generated programs take order-dependent counts, a spread of about 1% per program: OP's half step fires only when its first operand arrives before the second; values never differ. The test is kept, ignored with this reason |
| the rule table is confluent (pure core) | **proved** | `proofs/confluence` (Lean 4, no Mathlib, no `sorry`; `lake build`): for nets with agents named by the redex that created them, any oriented rule table with local right-hand sides has the diamond property (`diamond`), hence one normal form, reached in the same number of steps by every order (`net_same_length`; if one order terminates, every order does), and any partial reduction keeps it (`net_partial_reduction_sound`, the justification for stopping compile-time reduction at a budget). Instantiated for Mithril's kinds (`mithril_confluent`, `mithril_same_length`): ERA, DUP-DUP by label, DUP copying, beta, SWI and MAT selection, REF unfold and OP compute left abstract |
| the rule table is confluent (Mithril's extensions) | **claimed**, tested as above | not in the proof: REF unfolding against an unfilled wire (one agent, pushed by SWI and MAT selection), OP reading its operand through a wire and OP-SUP (three agents), closure copy on DUP, labels from a global counter, MAT's arm calling convention, `Kont` delivery and the static gate (`proofs/confluence/README.md`). The rewrite-count test above shows the extended table is not diamond in step counts |
| lockless protocols (CPU runtime): a join record fires once and sees every argument; a wave entry is claimed once; a refcounted cell or array is torn down once, after every other owner's reads | **model-checked** | loom over the functions the runtime calls (`mithril-rt/src/sync.rs`), every interleaving and C11 ordering: `cargo test -p mithril-rt --features loom --release --test loom_test`. It found two orderings that let a cell's last owner free or reuse it before another owner's read was ordered before it (a stale or reused value, not undefined behaviour, since cells are atomics): the release was `Release` only, and the unique-owner check a `Relaxed` load. The release is `AcqRel` and the check `Acquire` |
| the device rule table equals `mithril_core::rules` | **claimed**, checked by tests only | two implementations held together by the oracle |
| lowering preserves meaning | **claimed**, checked by tests only | oracle equality of generated code |

The oracle agrees with the net, not the other way round: an unused
binding is never evaluated. `eval_core` skips a `Let` whose variable is
not free in its body; `instantiate` never builds such a right-hand side
(the uses of outer variables it would take are linked to ERA); a REF
whose result wire already holds ERA erases the call and its arguments
instead of unfolding it; and an op whose other operand arrives as ERA
(its enclosing closure was erased) is erased with it. The device rules do
the same. Fixtures `dead_binding`, `dead_division` (a diverging and a
failing dead binding: every lane returns 5). `eval_core` shares payloads (`Arc`), so a
clone copies nothing; a deep copy per variable read cost 21 GB on the
shared-tree fixture. fast.py caps each build's memory (`ulimit -v`).

Every bug fix lands with a fixture that fails on the tree before the fix.

## 9. Measurement method

**Machine.** AMD Ryzen 7 7800X3D (8 cores, 16 threads), 62 GiB, RTX 4090
(driver 580.82.09), Ubuntu 22.04, rustc 1.86.0, gcc 11.4.0.

**Mithril harness** (`bench/harness.py`, results in `bench/results.md`).
Lanes: `C` (the C twin, `gcc -O2`), `SEQ` (`--threads 1`), `PAR16`
(`--threads 16`), `GPU` (`--gpu`, when `MITHRIL_GPU=1`). Programs are
built once with `mithril build`; build time is reported, not timed. Times
are wall clock, minimum of n runs (by default 3 when the first run takes
under 60 s, else 1), timeout 1,200 s per run (`--timeout`; the recorded
results used 300 s). Every run's output is checked against
`bench/expected.txt`. On "arena
exhausted" at defaults the lane is retried with
`MITHRIL_NODES=2^32 MITHRIL_RECS=2^28` and the retry is recorded.

**GPU timing.** The device program is compiled by nvcc in a container on
first use (5 to 77 s per port) and cached by source hash. The harness's
GPU lane builds the artefact once (`build --gpu`, which compiles or loads
the cubin) and times `exec` of it after a warm-up that persists the
program's stack hint. Distinct measurements are:

* **run time**: the runner's host wait around the cooperative `k_run`
  launch (`MITHRIL_GPU_STATS`), including launch and polling overhead.
  It omits most `k_boot` execution; wall minus run is not all host cost.
* **boot and run stream intervals**: CUDA events surrounding each launch.
  These include small host enqueue gaps as well as device execution.
  Kdtree measures 531 ms in boot and 581 ms in `k_run`; neither belongs
  in context creation (typically about 42 ms, observed 34--56 ms).
* **wall**: the whole `exec` process, warm cache, including setup,
  readback, output and exit. Readback follows result storage (section
  7.6); it is not a fixed startup constant.

Phase cycle counters include useful work and the closing grid barrier;
they are not sums of work over all lanes. A calibration of three empty
grid barriers costs 1.56--1.64 us per round on the measured launch widths.
Nsight warp-stall percentages describe warp cycles, not additive shares
of process wall time. Profile clocks differ from ordinary runs, so use
unprofiled runs for before/after performance comparisons.

**Instruction counts.** fast.py measures a mid-size run with `perf stat`
(noise-free) and compares against `tests/ci/baseline.json`.

## 11. Use cases and scope

### 11.1 Generality corpus

`bench/general/run.py`: programs unlike the suite, each checked against
the Python oracle at a small size and against an idiomatic Rust twin
(`rust/*.rs`, `Rc`/`Vec`, written the way the source is written; nobody
hoists by hand), then timed. Times in seconds, measured by `run.py` at
`ce63d1e` (one run each, a load below 1).

| program | shape | Rust | Mithril t1 | Mithril t16 |
|---|---|---|---|---|
| collatz_mutual | mutual recursion | 0.29 | 0.41 | 0.41 |
| cow_versions | copy-on-write array versions | 1.00 | 0.91 | 0.91 |
| dag_share | heavily shared DAG | 1.67 | 1.91 | 0.27 |
| graph_dfs | DFS over an array of adjacency lists | 0.86 | 1.30 | 0.25 |
| interp | expression interpreter, env as a list | 2.45 | 1.23 | 0.14 |
| persist_map | persistent BST with live old versions | 1.47 | 1.55 | 1.56 |
| sorts | list merge sort and quicksort | 6.04 | 1.43 | 0.55 |
| stage_closure | W2: `mk(k) = λx. x + heavy(k)`, 20k applications | 0.00 | 0.00 | 0.00 |
| pipeline_cfg | stage closures from a runtime config, in a list, over 20k inputs | 0.29 | 0.01 | 0.01 |
| interp_closure | closure compilation of a runtime AST over 20k environments | 0.01 | 3.77 | 3.83 |

### 11.2 The k-d tree probe

`bench/ports/kdtree.py` (C twin `kdtree.c`): a 2-d tree over 2^18 hashed
points built by midpoint splits over a list (subtrees as unequal as the
data makes them, a bucket for coincident points), then 2^18
nearest-neighbour queries with pruning, forked as a batch that only reads
the shared tree. No other port shares a large read-only structure across
every fork, partitions lists, or chases pointers data-dependently.

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
type and f32 arrays (these now exist, section 4.1); buffers crossing a foreign-call boundary without
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

| demo | CPU 16 threads (wall) | device (wall) |
|---|---|---|
| Whitted, 512 x 512 x 4 samples | 0.061 s | 0.871 s |
| path, 256 x 256 x 64 paths | 0.238 s | 0.335 s |

The device image is byte-identical to the CPU image for both. All ray
code is native (vectors and hit records are nested tuples held in
registers, section 5.2). These static demo timings are medians of three
built-artifact runs after a warmup on 2 October 2026, using the same compiler
as the animation measurements. [Raw samples](img/raytracer-static.json). Device wall includes result formatting and process setup;
its `k_run` interval alone does not describe end-to-end rendering cost.
A test renders the Whitted demo at 12 x 12 and checks it against the
oracle at 1, 4 and 16 threads.

## 12. Open items

Device:

1. Scheduling after rule populations stop growing: per-rule high-water
   accounting fixes replacement and oscillation barriers (section 7.2).
   A same-rule handoff can still hide future forks. A further general policy
   needs evidence about useful exposed work and lane imbalance; changing a
   width or budget to rescue one port is not a solution.
2. Work efficiency and divergence: baseline Nsight profiles measure 3.10 active threads
   per warp instruction for symreg, 2.01 for queens, 4.71 for kdtree and
   17.86 for tree-bitonic, with about eight active warps per SM. Their
   profiled kernels use 255, 210, 224 and 226 registers respectively. These are
   shape-dependent costs, not evidence for a register cap. A diagnostic
   removing native work accounting cuts queens' stream interval from
   1,192 to 771 ms but does not help symreg (1,955 to 1,966 ms). Any
   region-level accounting lowering must retain budget and stop behavior.
3. Startup and result costs: context creation is typically about 42 ms (34--56 ms
   across selected warm runs). Root-driven readback avoids cell copies for scalar
   results; aggregates referencing cells still copy the allocation
   high-water range. Kdtree's 531 ms boot interval is sequential program
   work, not context creation. Eager allocation leaves it at 523 ms, so
   managed-memory faults do not explain that cost.
4. Global snapshots and barriers still serialize rounds. The initial
   phase structure has the provenance in section 14; subsequent scheduling
   changes must follow Mithril's measured work and communication costs.

5. Array teardown on the device is one continuation chain per array: a
   single boxed array of 2^20 elements is erased over about 4,096 rounds in
   sequence. Splitting the unvisited suffix in two at each resume keeps the
   frontier bounded with logarithmic depth. The device erasure tests check
   results, not free-list balance; a double free or a leaked block would
   pass them.

6. The device refcount decrement (`rc_dec` in `engine.cu`) is a bare
   `atomicSub` with no fence before the last owner frees and reuses the
   cell: the same ordering bug the CPU had (section 8, loom). It needs a
   fence on the freeing path and a device test.

CPU:

Semantic core:

12. Readback: a closure created in an arm, capturing a pattern binder and
    applied twice, is an ICE in the reader (the ignored test
    `a_closure_capturing_an_arm_binder_applied_twice` in
    `specialize_test.rs`). Two specializer crashes on nested closures with
    conditionals (in `mithril-core` `net.rs` and `rules.rs`).
13. The Dup cache is keyed by the whole selection, so a captured shared
    value read under three selections is bound three times (duplicated
    work, not a wrong answer). Keying by the selections a read consults
    fixes it.
14. The clone discipline (section 3.4) is not enforced: the compiler does
    not call `check_clone_discipline`, so a closure applied to itself
    (`f = lambda g: 3; f(f)`) compiles and runs. Undecided: run it in the
    pipeline, or state what self-application does.

Constraint and proofs:

15. `tail_inline` remains a Core-level rewrite in codegen with an
    unmarked threshold (64), and the inline decision uses a
    threshold (192) and a name prefix (`__while`/`__for`). Undecided:
    move into the rules, justify by a stated cost model, or mark as
    tunables with their evidence.
16. Lean proof of confluence: the pure core is proved (section 8); the extensions (section 8, `proofs/confluence/README.md`) and the conformance link from `mithril_core::rules` to the Lean model are not.
17. One rule-table source for the CPU and the device: not started.

Measurement:

Unconfirmed findings from source review (argued from the code, no
failing probe yet): an array leaking through an if-arm tuple mask; a
thread-local read before it is set on the device path; the borrow
inference's round cap.

Planned: the MoE router probe (section 11.3).

Undecided: Mithril's role as the graph generator for Blaze's irregular
and sparse computation (section 11.3).

## 13. Process rules

The working rules that shape this design:

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
  under 15,000 lines of source (`src/`); the working tree remains above that budget.
* Design decisions are recorded here with the numbers that justify them.

## 15. Compiler planning demo

`demos/compiler_planning/` is one standalone demo of coupled fusion, SIMD,
tiling, scratch storage and CPU/accelerator placement. Immutable SSA input
becomes shared boundary summaries; an explicit synthetic cost and SRAM contract
selects the plan. Division is CPU-only. Python supplies I/O and an independent
set-based exhaustive reference; all decisions also execute inside the net.

The viewer shows four resource scenarios, reduction traces and saved-net
continuations. It replays actual captures from the shared rule table. LIFO,
FIFO, random and alternating orders must match Core evaluation and the reference.
Saved states contain the arena and pending work; each continuation starts from
an actual copied state, without replaying the prefix. These are sequential net
runs, with modeled accelerator decisions. Arena counts exclude vector capacity,
queue and process overhead; no device kernel or speedup is claimed.

Run `python3 demos/compiler_planning/demo.py --out target/compiler-planning/demo`.
Open the generated `demo.html`. Broader exploratory receipts are archived
locally. The generic loop-join correction and its frontend regressions remain
part of the compiler.

## 16. Shared region services

The verified borrowed-traversal fix now uses common services in the existing
LIR and native Machine. `operations.rs` describes helper result types, effects,
context and failure; unknown helpers are barriers. Copy paths, leaf exits and
integer prefixes keep separate admissions over one expression walk. Snapshot
substitution resolves each assignment once, never revisiting replacement values.
Machine centralizes successors, incoming counts, liveness and saved-call values;
call sharing and physical frame emission consume the same ordered width layout.
These facts are recomputed after mutations. Stronger additive-fold and completed
budget certificates remain separate, as do growth and native return protocols.

The extraction migrates existing consumers, rather than adding a strategy DSL.
All 340 generated Rust/CUDA artifacts (85 programs: 17 ports and 68 fixtures)
are identical to the frozen compiler. The normal GPU build reproduces Symreg's
measured cubin hash; a full-size device smoke returns 2383953211. No physical
execution change or new performance benefit is claimed. Seven new service tests
pass; three deliberately broken implementations trigger assertion failures.
Independent review finds no issues. Source files shrink by nine lines net;
language-source count is 16,854 against the unchanged 15,000 cap. This is modest
savings and demonstrated reuse, not completion of the overall cleanup.

The retired third-party benchmark set is no longer distributed. The spatial-tree
benchmark, original fixtures, generality corpus, lockless corpus and render demos
remain the regression inputs. Source accounting remains above the 15,000-line
budget; consolidation is still open. Current test results are recorded by the
local development workflow.
