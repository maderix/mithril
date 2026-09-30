import Confluence.Diamond

/-!
# Layer B, part 3: Mithril's P1 rule table

The pure core of `crates/mithril-core/src/rules.rs` (`process`), written as an
oriented table of local right-hand sides over Mithril's agent kinds. Port
layout (slot 0 is always the principal port; the aux slots follow the cell
layout in `rules.rs`):

| kind | aux ports |
|---|---|
| `era` | none |
| `num n` | none |
| `con c n` | fields `1..n` |
| `dup l` | `1` first copy, `2` second copy |
| `lam` | `1` parameter, `2` body |
| `app` | `1` argument, `2` return |
| `op0 code` | `1` the other operand, `2` return (waiting for its first operand) |
| `op1 code x` | `1` return (holding the first operand `x`: `OP_FLIP` as an agent) |
| `swi` | `1` return, `2` then (nonzero), `3` else (zero) |
| `mat tags` | `1` return, `2..tags.length+1` the arms, in arm order |
| `proj i` | `1` return (tuple projection of field `i`) |
| `ref entry n` | the `n` captured arguments |

REF unfold and OP compute are abstract (`Prog`), as `rules.rs` delegates them
to `Prog::unfold` and `Prog::compute`; both are functions of the pair.
-/

namespace Confluence.Mithril

open Confluence Confluence.Net

inductive Kind where
  | era
  | num (n : Int)
  | con (ctor : Nat) (arity : Nat)
  | dup (label : Nat)
  | lam
  | app
  | op0 (code : Nat)
  | op1 (code : Nat) (x : Int)
  | swi
  | mat (tags : List Nat)
  | proj (i : Nat)
  | ref (entry : Nat) (nargs : Nat)
  deriving DecidableEq, Repr

def arity : Kind → Nat
  | .era => 0
  | .num _ => 0
  | .con _ n => n
  | .dup _ => 2
  | .lam => 2
  | .app => 2
  | .op0 _ => 2
  | .op1 _ _ => 1
  | .swi => 3
  | .mat tags => tags.length + 1
  | .proj _ => 1
  | .ref _ n => n

def Kind.isRef : Kind → Bool
  | .ref _ _ => true
  | _ => false

/-! ## Right-hand sides (`ia` = the first agent of the pair, `ib` = the second) -/

/-- Link aux port `i` of one agent to aux port `i` of the other, `i = 1..n`,
creating nothing. APP–LAM (beta: argument–parameter, return–body) and DUP–DUP
with equal labels (annihilation) are `linkAux 2`. -/
def linkAux (n : Nat) : RHS Kind where
  agents := []
  wire
    | .ia i => if 1 ≤ i ∧ i ≤ n then some (.ib i) else none
    | .ib i => if 1 ≤ i ∧ i ≤ n then some (.ia i) else none
    | .nw _ _ => none

/-- ERA–K: one new ERA on each of K's `n` aux ports. -/
def eraseAll (n : Nat) : RHS Kind where
  agents := List.replicate n .era
  wire
    | .ia _ => none
    | .ib j => if 1 ≤ j ∧ j ≤ n then some (.nw (j - 1) 0) else none
    | .nw k s => if k < n ∧ s = 0 then some (.ib (k + 1)) else none

/-- DUP(l)–K: K passes through the dup. Two copies of K (new agents 0 and 1) on
the dup's two outputs, and a `l`-labelled dup on each of K's `n` aux ports
(new agent `j+1` for port `j`) joining the two copies' port `j`. This one shape
is `dup_rule` for NUM, CON and LAM (copy one layer; for LAM the parameter dup
is the superposition), `dup_commute` for OP, APP, SWI, MAT, and `dup_dup` with
different labels (the four dups of `rules.rs`). -/
def commute (l : Nat) (K : Kind) : RHS Kind :=
  let n := arity K
  { agents := K :: K :: List.replicate n (.dup l)
    wire := fun
      | .ia i => if i = 1 ∨ i = 2 then some (.nw (i - 1) 0) else none
      | .ib j => if 1 ≤ j ∧ j ≤ n then some (.nw (j + 1) 0) else none
      | .nw k s =>
        if k ≤ 1 then
          (if s = 0 then some (.ia (k + 1))
           else if s ≤ n then some (.nw (s + 1) (k + 1)) else none)
        else if k ≤ n + 1 then
          (if s = 0 then some (.ib (k - 1))
           else if s ≤ 2 then some (.nw (s - 1) (k - 1)) else none)
        else none }

/-- OP0–NUM x: the first operand arrives; OP1 holding `x` faces the other operand. -/
def opFirst (code : Nat) (x : Int) : RHS Kind where
  agents := [.op1 code x]
  wire
    | .ia i => if i = 1 ∨ i = 2 then some (.nw 0 (i - 1)) else none
    | .ib _ => none
    | .nw k s => if k = 0 ∧ s ≤ 1 then some (.ia (s + 1)) else none

/-- OP1–NUM y: compute; the result goes to the return port. -/
def opSecond (v : Int) : RHS Kind where
  agents := [.num v]
  wire
    | .ia i => if i = 1 then some (.nw 0 0) else none
    | .ib _ => none
    | .nw k s => if k = 0 ∧ s = 0 then some (.ia 1) else none

/-- SWI–NUM: the taken branch is linked to the return port, the other erased. -/
def swiSel (taken dead : Nat) : RHS Kind where
  agents := [.era]
  wire
    | .ia i => if i = 1 then some (.ia taken) else if i = taken then some (.ia 1)
               else if i = dead then some (.nw 0 0) else none
    | .ib _ => none
    | .nw k s => if k = 0 ∧ s = 0 then some (.ia dead) else none

/-- PROJ i–CON c n: field `i` (0-based) to the return port, the other fields erased. -/
def projSel (i n : Nat) : RHS Kind where
  agents := List.replicate (n - 1) .era
  wire
    | .ia r => if r = 1 then some (.ib (i + 1)) else none
    | .ib j => if j = i + 1 then some (.ia 1)
               else if 1 ≤ j ∧ j ≤ n then some (.nw (if j ≤ i then j - 1 else j - 2) 0) else none
    | .nw k s => if k + 1 < n ∧ s = 0 then some (.ib (if k < i then k + 1 else k + 2)) else none

/-- MAT–CON c n with `c` the `j`-th of `m` arm tags: the selected arm is applied
to the constructor's fields (a chain of `n` APPs, new agents `0..n-1`, the last
returning to the MAT's return port); the other `m - 1` arms are erased (new
agents `n..`). `rules.rs` passes the fields to the arm closure as extra `Ref`
arguments; the application chain is the same call written with P1 agents. -/
def matSel (j m n : Nat) : RHS Kind where
  agents := List.replicate n .app ++ List.replicate (m - 1) .era
  wire
    | .ia i =>
      if i = 1 then some (if n = 0 then .ia (j + 2) else .nw (n - 1) 2)
      else if i = j + 2 then some (if n = 0 then .ia 1 else .nw 0 0)
      else if 2 ≤ i ∧ i < m + 2 then some (.nw (n + (if i - 2 < j then i - 2 else i - 3)) 0)
      else none
    | .ib f => if 1 ≤ f ∧ f ≤ n then some (.nw (f - 1) 1) else none
    | .nw k s =>
      if k < n then
        (if s = 0 then some (if k = 0 then .ia (j + 2) else .nw (k - 1) 2)
         else if s = 1 then some (.ib (k + 1))
         else if s = 2 then some (if k + 1 < n then .nw (k + 1) 0 else .ia 1)
         else none)
      else if k < n + m - 1 ∧ s = 0 then
        some (.ia ((if k - n < j then k - n else k - n + 1) + 2))
      else none

/-! ## The table -/

/-- The program-specific half of the rules, as in `rules.rs`'s `Prog` trait. -/
structure Prog where
  /-- `Prog::compute`: an operator on two numbers; `none` = cannot compute (no rule). -/
  compute : Nat → Int → Int → Option Int
  /-- `Prog::unfold`: REF `entry` with `nargs` arguments meets an agent of the
  given kind; the right-hand side splices the body (its new agents named by
  path, like every rule's). Any function of the pair is allowed. -/
  unfold : Nat → Nat → Kind → Option (RHS Kind)

/-- Mithril's P1 rule table: one orientation per unordered pair of kinds. -/
def table (P : Prog) : Kind → Kind → Option (RHS Kind)
  | .era, k => some (eraseAll (arity k))
  | _, .era => none
  | .ref e n, k => if k.isRef then none else P.unfold e n k
  | _, .ref _ _ => none
  | .dup l, .dup l' =>
    if l = l' then some (linkAux 2) else if l < l' then some (commute l (.dup l')) else none
  | .dup l, k => some (commute l k)
  | _, .dup _ => none
  | .app, .lam => some (linkAux 2)
  | .op0 c, .num x => some (opFirst c x)
  | .op1 c x, .num y => (P.compute c x y).map opSecond
  | .swi, .num v => some (if v ≠ 0 then swiSel 2 3 else swiSel 3 2)
  | .proj i, .con _ n => if i < n then some (projSel i n) else none
  | .mat tags, .con c n =>
    match tags.idxOf? c with
    | some j => some (matSel j tags.length n)
    | none => none
  | _, _ => none

/-! ## The table is oriented (at most one rule per pair of kinds) -/

theorem linkAux_symmetric (n : Nat) : Symmetric (linkAux n) where
  no_agents := rfl
  no_new := by
    intro p k s
    cases p <;> simp only [linkAux] <;> (try split) <;> simp
  nw_none := fun _ _ => rfl
  swap_ab := by
    intro i
    simp only [linkAux]
    split <;> simp [RP.swap]

theorem eraseAll_zero_symmetric : Symmetric (eraseAll 0) where
  no_agents := rfl
  no_new := by
    intro p k s
    cases p with
    | ia i => simp [eraseAll]
    | ib j => simp [eraseAll]; omega
    | nw k' s' => cases s' <;> simp [eraseAll]
  nw_none := by
    intro k s
    cases s <;> simp [eraseAll]
  swap_ab := by
    intro i
    simp [eraseAll]; omega

theorem oriented (P : Prog) : Oriented (table P) := by
  intro k1 k2 R1 R2 h1 h2
  cases k1 <;> cases k2 <;> simp [table, Kind.isRef] at h1 h2 ⊢
  · subst h1; exact eraseAll_zero_symmetric
  · rename_i l l'
    by_cases e : l = l'
    · subst e; simp at h1; subst h1; exact ⟨rfl, linkAux_symmetric 2⟩
    · have e' : ¬ l' = l := fun h => e h.symm
      simp only [e, e', ite_false] at h1 h2
      by_cases hl : l < l'
      · have : ¬ l' < l := by omega
        simp [this] at h2
      · simp [hl] at h1



/-! ## Every right-hand side of the table is wired by an involution

So firing keeps nets well-wired (`WellWired.fire`), and in reachable nets every
active pair is a redex (`active_pair_redex`). -/

theorem linkAux_pinv (n : Nat) : PInv (linkAux n) := by
  intro p q h; cases p <;> simp [linkAux] at h <;> grind [linkAux]

theorem eraseAll_pinv (n : Nat) : PInv (eraseAll n) := by
  intro p q h; cases p <;> simp [eraseAll] at h <;> grind [eraseAll]

theorem opFirst_pinv (c : Nat) (x : Int) : PInv (opFirst c x) := by
  intro p q h; cases p <;> simp [opFirst] at h <;> grind [opFirst]

theorem opSecond_pinv (v : Int) : PInv (opSecond v) := by
  intro p q h; cases p <;> simp [opSecond] at h <;> grind [opSecond]

theorem swiSel_pinv : PInv (swiSel 2 3) ∧ PInv (swiSel 3 2) := by
  constructor <;> (intro p q h; cases p <;> simp [swiSel] at h <;> grind [swiSel])

theorem commute_pinv (l : Nat) (K : Kind) : PInv (commute l K) := by
  intro p q h; cases p <;> simp [commute] at h <;> grind [commute]

theorem projSel_pinv (i n : Nat) (hi : i < n) : PInv (projSel i n) := by
  intro p q h; cases p <;> simp [projSel] at h <;> grind [projSel]

theorem matSel_pinv (j m n : Nat) (hj : j < m) : PInv (matSel j m n) := by
  intro p q h; cases p <;> simp [matSel] at h <;> grind [matSel]

/-- A `Prog` whose unfoldings are wired by an involution. -/
def Prog.Wired (P : Prog) : Prop := ∀ e n k R, P.unfold e n k = some R → PInv R

theorem wired (P : Prog) (hP : P.Wired) : WiredTable (table P) := by
  intro k1 k2 R h
  cases k1 <;> cases k2 <;> simp [table, Kind.isRef] at h <;>
    first
    | (subst h; first
        | exact linkAux_pinv _ | exact eraseAll_pinv _ | exact commute_pinv _ _
        | exact opFirst_pinv _ _ | exact swiSel_pinv.1 | exact swiSel_pinv.2)
    | exact hP _ _ _ _ h
    | skip
  · rename_i l l'
    split at h
    · cases h; exact linkAux_pinv 2
    · split at h
      · cases h; exact commute_pinv _ _
      · cases h
  · obtain ⟨v, _, rfl⟩ := h; exact opSecond_pinv v
  · subst h; split
    · exact swiSel_pinv.2
    · exact swiSel_pinv.1
  · split at h
    · rename_i j hj; cases h
      exact matSel_pinv _ _ _ (List.idxOf?_eq_some_iff.1 hj).1
    · cases h
  · obtain ⟨hi, rfl⟩ := h; exact projSel_pinv _ _ hi

/-! ## Mithril's rule table is confluent -/

/-- Diamond for Mithril's table: two different redexes are one pair written both
ways (same net), or they commute and the two orders give equal nets. -/
theorem mithril_diamond (P : Prog) {N : Net Kind} {a b c d : AId}
    (hr : Redex (table P) N a b) (hs : Redex (table P) N c d) (hne : (a, b) ≠ (c, d)) :
    fire (table P) a b N = fire (table P) c d N ∨
    (Redex (table P) (fire (table P) a b N) c d ∧ Redex (table P) (fire (table P) c d N) a b ∧
      fire (table P) c d (fire (table P) a b N) = fire (table P) a b (fire (table P) c d N)) :=
  Net.diamond (oriented P) hr hs hne

theorem mithril_confluent (P : Prog) : Confluent (NStep (table P)) :=
  net_confluent (oriented P)

theorem mithril_church_rosser (P : Prog) {x y : Net Kind} (h : Conv (NStep (table P)) x y) :
    ∃ w, Star (NStep (table P)) x w ∧ Star (NStep (table P)) y w :=
  net_church_rosser (oriented P) h

theorem mithril_same_length (P : Prog) {n m : Nat} {x y z : Net Kind}
    (hy : StepN (NStep (table P)) n x y) (ny : Normal (NStep (table P)) y)
    (hz : StepN (NStep (table P)) m x z) (nz : Normal (NStep (table P)) z) : n = m ∧ y = z :=
  net_same_length (oriented P) hy ny hz nz

theorem mithril_steps_le (P : Prog) {n m : Nat} {x y z : Net Kind}
    (hy : StepN (NStep (table P)) n x y) (ny : Normal (NStep (table P)) y)
    (hz : StepN (NStep (table P)) m x z) : m ≤ n ∧ StepN (NStep (table P)) (n - m) z y :=
  net_steps_le (oriented P) hy ny hz

theorem mithril_strongly_normalizing (P : Prog) {n : Nat} {x y : Net Kind}
    (hy : StepN (NStep (table P)) n x y) (ny : Normal (NStep (table P)) y) :
    Acc (fun b a => NStep (table P) a b) x :=
  net_strongly_normalizing (oriented P) hy ny

theorem mithril_partial_reduction_sound (P : Prog) {n k : Nat} {x y z : Net Kind}
    (hxz : StepN (NStep (table P)) n x z) (nz : Normal (NStep (table P)) z)
    (hxy : StepN (NStep (table P)) k x y) :
    k ≤ n ∧ StepN (NStep (table P)) (n - k) y z ∧
      ∀ w, Star (NStep (table P)) y w → Normal (NStep (table P)) w → w = z :=
  net_partial_reduction_sound (oriented P) hxz nz hxy


/-- In every net reachable from a `Good`, well-wired initial net, an active pair
(two different live agents, principal ports wired, a rule for their kinds) is a
redex: the step relation fires exactly the active pairs. -/
theorem mithril_active_pair_redex (P : Prog) (hP : P.Wired) {N0 N : Net Kind}
    (hG : Good N0) (hW : WellWired N0) (hN : Star (NStep (table P)) N0 N) {a b : AId}
    (hne : a ≠ b) (hrule : (ruleAt (table P) N a b).isSome)
    (wab : Wire N (.out (a, 0)) (.out (b, 0))) : Redex (table P) N a b :=
  active_pair_redex (hG.star hN) (hW.star (wired P hP) hN) hne hrule wab

/-! ## The table's entries, as `rules.rs` names them -/

section Entries
variable (P : Prog)

/-- `beta` -/
example : table P .app .lam = some (linkAux 2) := rfl
/-- `dup_dup`, same label: annihilate -/
example (l : Nat) : table P (.dup l) (.dup l) = some (linkAux 2) := by simp [table]
/-- `dup_dup`, different labels: commute (four dups) -/
example : table P (.dup 1) (.dup 2) = some (commute 1 (.dup 2)) := by simp [table]
example : table P (.dup 2) (.dup 1) = none := by simp [table]
/-- `dup_rule` (copy NUM / CON / LAM) and `dup_commute` (OP, APP, SWI, MAT) -/
example (l : Nat) (n : Int) : table P (.dup l) (.num n) = some (commute l (.num n)) := rfl
example (l c n : Nat) : table P (.dup l) (.con c n) = some (commute l (.con c n)) := rfl
example (l : Nat) : table P (.dup l) .lam = some (commute l .lam) := rfl
example (l : Nat) : table P (.dup l) .app = some (commute l .app) := rfl
example (l c : Nat) : table P (.dup l) (.op0 c) = some (commute l (.op0 c)) := rfl
example (l : Nat) : table P (.dup l) .swi = some (commute l .swi) := rfl
example (l : Nat) (ts : List Nat) : table P (.dup l) (.mat ts) = some (commute l (.mat ts)) := rfl
/-- `era_value` / REF–ERA: erase every aux port -/
example (k : Kind) : table P .era k = some (eraseAll (arity k)) := by cases k <;> rfl
/-- `op_rule`: first operand (the `OP_FLIP` half step), then compute -/
example (c : Nat) (x : Int) : table P (.op0 c) (.num x) = some (opFirst c x) := rfl
example (c : Nat) (x y : Int) : table P (.op1 c x) (.num y) = (P.compute c x y).map opSecond := rfl
/-- `swi_rule`: nonzero takes the first branch -/
example : table P .swi (.num 7) = some (swiSel 2 3) := by simp [table]
example : table P .swi (.num 0) = some (swiSel 3 2) := by simp [table]
/-- `mat_rule`: arm selection and tuple projection -/
example : table P (.mat [4, 9]) (.con 9 2) = some (matSel 1 2 2) := by simp [table]; rfl
example : table P (.proj 1) (.con 0 3) = some (projSel 1 3) := by simp [table]
/-- REF unfold (`Prog::unfold`) against anything but ERA and REF -/
example (e n : Nat) : table P (.ref e n) .app = P.unfold e n .app := rfl
example (e n e' n' : Nat) : table P (.ref e n) (.ref e' n') = none := by simp [table, Kind.isRef]

end Entries

/-! ## Right-hand sides are proper nets (checked on instances)

`rhsWf` checks, on a bounded range of ports, that a right-hand side's wiring is
a fixed-point-free involution on exactly the valid ports (interface aux ports
and new agents' ports), so every rule consumes its interface once and leaves no
dangling port. The checks below are evaluated by the kernel (`decide`). A proof
for all arities is not part of P1 (see README). -/

def rhsValid (R : RHS Kind) (na nb : Nat) : RP → Bool
  | .ia i => decide (1 ≤ i ∧ i ≤ na)
  | .ib j => decide (1 ≤ j ∧ j ≤ nb)
  | .nw k s => match R.agents[k]? with
    | some kd => decide (s ≤ arity kd)
    | none => false

def rhsPorts (R : RHS Kind) (na nb : Nat) : List RP :=
  (List.range (na + 3)).map .ia ++ (List.range (nb + 3)).map .ib ++
    (List.range (R.agents.length + 2)).flatMap (fun k => (List.range 7).map (.nw k))

def rhsWf (R : RHS Kind) (ka kb : Kind) : Bool :=
  let na := arity ka
  let nb := arity kb
  (rhsPorts R na nb).all fun p =>
    if rhsValid R na nb p then
      match R.wire p with
      | some q => rhsValid R na nb q && q != p && R.wire q == some p
      | none => false
    else R.wire p == none

/-- A concrete `Prog` for the checks. -/
def P0 : Prog := ⟨fun _ x y => some (x + y), fun _ _ _ => none⟩

def wfAt (ka kb : Kind) : Bool :=
  match table P0 ka kb with
  | some R => rhsWf R ka kb
  | none => true

def sampleKinds : List Kind :=
  [.era, .num 0, .num 3, .con 0 0, .con 1 1, .con 2 3, .dup 0, .dup 1, .lam, .app, .op0 0,
   .op1 0 2, .swi, .mat [0], .mat [2, 1], .mat [0, 1, 2], .proj 0, .proj 2, .ref 0 2]

/-- Every rule of the table between the sample kinds is a proper local net rewrite. -/
theorem sample_rules_wf :
    sampleKinds.all (fun ka => sampleKinds.all (fun kb => wfAt ka kb)) = true := by
  decide

/-- The sample exercises 68 rules, covering every rule shape of the table. -/
theorem sample_rules_count :
    (sampleKinds.flatMap fun ka => sampleKinds.filter fun kb => (table P0 ka kb).isSome).length = 68 := by
  decide

end Confluence.Mithril
