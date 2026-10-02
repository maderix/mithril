# Confluence of Mithril's rule table (Lean 4)

## The claim in plain words

Take any net built from Mithril's core rules and fire its active pairs in any
order. If one order reaches a final net, every order reaches **the same final
net, in the same number of steps**. For example, in `(2 × 3) + (4 × 5)`,
computing either multiplication first ends at `26` after the same number of
firings.

Two consequences follow. If one order finishes, no order runs forever. And
stopping part-way is safe: a partly reduced net still reaches the same final
net, which is why the compiler may reduce until its budget runs out and hand
the half-reduced net to the runtime.

The proof does **not** show that a program finishes, does not cover the extra
rules listed under "What is not covered", and does not show that the Rust code
implements the modelled rules (tests check that; see `docs/design.md`,
section 8).

## Details

A machine-checked proof that the pure core of Mithril's interaction-net rule
table is confluent. Core Lean 4.34.1, no Mathlib, no `sorry`, no added
axioms.

```
cd proofs/confluence
lake build                 # checks every proof
lake env lean Axioms.lean  # prints the axioms of the main theorems
```

Every main theorem depends only on `propext`, `Quot.sound` and (for some)
`Classical.choice`.

## What is modelled

- **Agent ids are paths.** An agent of the initial net is `root n`. The k-th
  agent created by firing the redex `(a, b)` is `born a b k`. Firing order
  cannot change a name, so "the same net" is plain equality, not equality up
  to renaming.
- **Ports and wires.** A port is `(id, slot)`. Slot 0 is the principal port,
  and slots `1..arity` are the ordered auxiliary ports. Each port has an
  outer end and an inner end, and `peer : End → Option End` connects ends.
  When a redex fires, its agents die and the rule wires the inner ends of
  their auxiliary ports: to new agents, or directly to each other
  (interface-to-interface: beta, DUP–DUP annihilation, SWI/MAT selection).
  A dead port is a pass-through junction. `Wire N e t` is the deterministic
  walk from `e` through dead ports to the next live port. A firing only
  assigns ends it owns, so there is no global substitution.
- **Rules.** A table maps a pair of kinds to at most one `RHS` (new agents
  plus a wiring). `Oriented` means at most one rule per unordered pair; if
  both orders have a rule, the kinds are equal and the rule is symmetric
  (ERA–ERA, DUP–DUP with the same label).
- **Redex** `(a, b)`: two different live agents with a rule, whose principal
  ports are wired together, and whose future path names are unused.
- **Mithril kinds** (`Rules.lean`): ERA, NUM n, CON c n, DUP label, LAM, APP,
  OP0 code / OP1 code x (the two-agent OP), SWI, MAT tags, PROJ i,
  REF entry nargs. `Prog.compute` (OP) and `Prog.unfold` (REF) are abstract:
  any functions of the pair are allowed.

## What is proved

Layer A (`Abstract.lean`, any relation with the diamond property):
`confluent`, `church_rosser`, `unique_normal_form`, `same_length`,
`steps_le` (every sequence from `x` is at most as long as a sequence to a
normal form, and can still finish in the remaining steps),
`no_infinite_reduction`, `strongly_normalizing` (`Acc`),
`partial_reduction_sound`. The termination results (`steps_le`,
`no_infinite_reduction`, `strongly_normalizing`) assume one sequence from
`x` reaches a normal form: termination of one order implies termination of
every order. They do not prove that any program terminates. The labelled form is `LDiamond`, with redexes as
labels.

Layer B, for any oriented table with arbitrary local right-hand sides
(`Net.lean`, `Diamond.lean`):
- `Wire.det`: a wire has one far end.
- `redexes_share` / `redexes_disjoint`: two redexes that share an agent are
  the same pair.
- `rule_local`: firing changes only the redex's own ids and ends.
- `redex_persists`: firing one redex keeps every other redex, with its
  agents and kinds unchanged.
- `diamond`: two different redexes fired in either order give **equal** nets.
- `ldiamond`, then `net_confluent`, `net_church_rosser`, `net_same_length`,
  `net_steps_le`, `net_strongly_normalizing`, `net_partial_reduction_sound`.
- Invariants: `Good` (path names are fresh) and `WellWired` (`peer` is an
  involution, and the inner ends of live agents are unused) are preserved by
  every step (`Good.star`, `WellWired.star`). `Wire.reverse` and
  `active_pair_redex` then show that, in every reachable net, each active
  pair is a `Redex`. The step relation fires exactly the active pairs.

Mithril (`Rules.lean`):
- `oriented` and `wired` (every right-hand side is wired by an involution,
  for all arities; REF unfold is assumed wired).
- `mithril_diamond`, `mithril_confluent`, `mithril_church_rosser`,
  `mithril_same_length`, `mithril_steps_le`,
  `mithril_strongly_normalizing`, `mithril_partial_reduction_sound`,
  `mithril_active_pair_redex`.
- `sample_rules_wf`: a kernel check (`decide`) that all 68 rules among 19
  sample kinds wire every valid port exactly once and leave no port
  dangling.
- `Examples.lean`: `(λx.x) 5` against ERA, reduced in two concrete steps.

## Mapping to `crates/mithril-core/src/rules.rs`

| Rust | Lean |
|---|---|
| `process` dispatch, both orders | `table P` (one order per pair; `oriented`) |
| `era_value`; REF–ERA branch of `process` | `table P .era k = eraseAll (arity k)` |
| `beta` | `(.app, .lam) ↦ linkAux 2` |
| `op_rule`, operand not yet there (`OP_FLIP` half step) | `(.op0 c, .num x) ↦ opFirst c x` |
| `op_rule`, compute | `(.op1 c x, .num y) ↦ opSecond` via `P.compute` |
| `op_rule`, `compute = None` → `park_op` | no rule (`none`) |
| `swi_rule` | `(.swi, .num v) ↦ swiSel 2 3` (v ≠ 0) / `swiSel 3 2` |
| `mat_rule`, `MatMeta::Arms` | `(.mat tags, .con c n) ↦ matSel` (arm applied to the fields with an APP chain; other arms erased) |
| `mat_rule`, `MatMeta::Proj` | `(.proj i, .con c n) ↦ projSel i n` |
| `dup_rule` (NUM, CON, LAM) | `(.dup l, K) ↦ commute l K` |
| `dup_commute` (OP, APP, SWI, MAT) | `(.dup l, K) ↦ commute l K` |
| `dup_dup`, same label | `(.dup l, .dup l) ↦ linkAux 2` |
| `dup_dup`, different labels | `(.dup l, .dup l') ↦ commute l (.dup l')` for `l < l'` |
| `Prog::unfold` (REF–anything) | `(.ref e n, K) ↦ P.unfold e n K` |
| `Prog::unfold` (REF–Var, pushed by `swi_rule`/`mat_rule`) | not modelled (see below) |

## What is not covered

- **OP reading its operand through `resolve`.** The model has only the two-agent
  OP0/OP1 form. The one-step compute of `op_rule`, when the other operand is
  already resolved, is not shown equal to it.
- **REF unfolding against a Var.** `process` unfolds a REF whose other
  side is an unfilled wire (`swi_rule` and `mat_rule` push such pairs).
  That is a one-agent firing, not an active pair of two agents, so it is
  outside the step relation here.
- **OP–SUP** (`op_rule` with a DUP operand). It consumes three agents and
  is not modelled.
- **`copy_closure`** (DUP meeting a closure REF, and the arm copies in
  `dup_commute` for SWI/MAT). The model has DUP–REF unfold and DUPs on arm ports.
  Nothing is proved to show `copy_closure` is derived from these.
- **Dynamic DUP labels** from a global counter. Labels here are given
  numbers; no renaming argument is made.
- **MAT's calling convention.** Rust passes the fields as extra `Ref`
  arguments; the model applies the arm through APPs. They are not shown equal.
- **Kont delivery, `park_op`, the static gate (Var wiring), and Ext / Arr /
  FLO values.** FLO follows the NUM rules and is not modelled separately.
  Rules that only exist at runtime or compile time fall outside this model.
- **ICE pairs** (pairs with no rule) are simply stuck. Confluence holds
  anyway.
- **`Prog.unfold` is abstract.** Confluence holds for any unfold. Keeping
  nets well-wired assumes `P.Wired`.
- **Every-port-wired (totality) of the right-hand sides** is checked on
  sample arities only (`sample_rules_wf`). The involution half is proved for
  all arities.
- **No executable reference reducer, and no proof that the Rust rule code
  conforms to this model.**
- A step here is one rule firing, which matches the rewrite count of
  `process` (1 per rule, 0 for `link`). Wiring is part of the net, not a
  step.
