/-!
# Layer B, part 1: interaction nets with path names

The model (generic in the type `κ` of agent kinds):

* **Agent ids are paths.** An agent of the initial net is `root n`; the `k`-th
  agent created by firing the redex `(a, b)` is `born a b k`. Firing order can
  therefore never change a name, and "the same net" is plain equality.
* **Ports** are `(id, slot)`: slot `0` is the principal port, slots `1..arity`
  the ordered auxiliary ports.
* **Wiring is explicit.** Every port has two *ends*: `out p` faces the wire
  outside the agent, `inn p` faces the inside of the agent. `peer : End → Option End`
  connects ends. While an agent is live only its outer ends are used. When a
  redex fires, its two agents die and the rule wires the *inner* ends of their
  auxiliary ports (the redex's interface) to the new agents' outer ends, or to
  each other. A dead port is thus a pass-through junction, and the wire seen
  from a live port is the deterministic walk `Wire` through dead ports.
  This representation lets a rule connect two interface ports directly
  (annihilation, beta, SWI/MAT selection) with no global substitution: a firing
  only ever *assigns ends it owns*, which is what makes firings commute.
* A **right-hand side** (`RHS`) lists new agents and a wiring over `RP`
  (interface ends of the two consumed agents and ports of the new agents).
  A rule table maps a pair of kinds to at most one right-hand side.
* `consumed` records which agents have fired (history only; it is never read
  by a rule, it serves the freshness invariant in `Diamond.lean`).
-/

namespace Confluence.Net

/-- Agent ids: paths from the redex that created the agent. -/
inductive AId where
  | root (n : Nat)
  | born (a b : AId) (k : Nat)
  deriving DecidableEq, Repr

/-- `(agent, slot)`; slot 0 is the principal port. -/
abbrev Port := AId × Nat

/-- The two ends of a port: outside (towards the wire) and inside (towards the agent). -/
inductive End where
  | out (p : Port)
  | inn (p : Port)
  deriving DecidableEq, Repr

structure Net (κ : Type) where
  agents : AId → Option κ
  peer : End → Option End
  consumed : AId → Bool

theorem Net.ext' {κ : Type} {N M : Net κ} (h1 : N.agents = M.agents) (h2 : N.peer = M.peer)
    (h3 : N.consumed = M.consumed) : N = M := by
  cases N; cases M; simp_all

variable {κ : Type}

/-- The wire seen from end `e`: follow `peer`, passing through dead ports
(entering at one end, leaving at the other), until the outer end of a live port. -/
inductive Wire (N : Net κ) : End → End → Prop
  | arrive {e : End} {q : Port} :
      N.peer e = some (.out q) → N.agents q.1 ≠ none → Wire N e (.out q)
  | passOut {e t : End} {q : Port} :
      N.peer e = some (.out q) → N.agents q.1 = none → Wire N (.inn q) t → Wire N e t
  | passIn {e t : End} {q : Port} :
      N.peer e = some (.inn q) → Wire N (.out q) t → Wire N e t

/-- The walk is deterministic: a wire has one far end. -/
theorem Wire.det {N : Net κ} {e t1 t2 : End} (h1 : Wire N e t1) (h2 : Wire N e t2) : t1 = t2 := by
  induction h1 generalizing t2 with
  | arrive hp hl =>
    cases h2 with
    | arrive hp' _ => rw [hp] at hp'; cases hp'; rfl
    | passOut hp' hd _ => rw [hp] at hp'; cases hp'; exact absurd hd hl
    | passIn hp' _ => rw [hp] at hp'; cases hp'
  | passOut hp hd _ ih =>
    cases h2 with
    | arrive hp' hl => rw [hp] at hp'; cases hp'; exact absurd hd hl
    | passOut hp' _ hw => rw [hp] at hp'; cases hp'; exact ih hw
    | passIn hp' _ => rw [hp] at hp'; cases hp'
  | passIn hp _ ih =>
    cases h2 with
    | arrive hp' _ => rw [hp] at hp'; cases hp'
    | passOut hp' _ _ => rw [hp] at hp'; cases hp'
    | passIn hp' hw => rw [hp] at hp'; cases hp'; exact ih hw

theorem Wire.lookup {N : Net κ} {e t : End} (h : Wire N e t) : ∃ f, N.peer e = some f := by
  cases h with
  | arrive hp _ => exact ⟨_, hp⟩
  | passOut hp _ _ => exact ⟨_, hp⟩
  | passIn hp _ => exact ⟨_, hp⟩

/-! ## Right-hand sides and rule tables -/

/-- Ports of a right-hand side: `ia i` / `ib j` are the interface (the inner end
of auxiliary port `i` of the first consumed agent / `j` of the second); `nw k s`
is port `s` of the `k`-th new agent. -/
inductive RP where
  | ia (i : Nat)
  | ib (j : Nat)
  | nw (k s : Nat)
  deriving DecidableEq, Repr

def RP.swap : RP → RP
  | .ia i => .ib i
  | .ib j => .ia j
  | p => p

structure RHS (κ : Type) where
  agents : List κ
  wire : RP → Option RP

/-- A rule table: at most one right-hand side per ordered pair of kinds. -/
abbrev Table (κ : Type) := κ → κ → Option (RHS κ)

/-- `bornIdx a b x = some k` iff `x = born a b k`. -/
def bornIdx (a b : AId) : AId → Option Nat
  | .born a' b' k => if a' = a ∧ b' = b then some k else none
  | _ => none

theorem bornIdx_eq_some {a b x : AId} {k : Nat} : bornIdx a b x = some k ↔ x = .born a b k := by
  cases x with
  | root n => simp [bornIdx]
  | born a' b' k' =>
    simp only [bornIdx, AId.born.injEq]
    by_cases h : a' = a ∧ b' = b
    · simp [h]
    · simp [h]; intro h1 h2; exact absurd ⟨h1, h2⟩ h

theorem bornIdx_born (a b : AId) (k : Nat) : bornIdx a b (.born a b k) = some k :=
  bornIdx_eq_some.2 rfl

/-- Name the right-hand side's ports in the net, from the redex `(a, b)`. -/
def realize (a b : AId) : RP → End
  | .ia i => .inn (a, i)
  | .ib j => .inn (b, j)
  | .nw k s => .out (.born a b k, s)

/-- Ids a firing of `(a, b)` rewrites: its two agents and the ids it creates. -/
def agentDom (a b x : AId) : Bool := x == a || x == b || (bornIdx a b x).isSome

def agentVal (R : RHS κ) (a b x : AId) : Option κ :=
  if x = a ∨ x = b then none
  else match bornIdx a b x with
    | some k => R.agents[k]?
    | none => none

/-- Ends a firing of `(a, b)` assigns: the inner ends of its two agents and the
outer ends of the agents it creates. -/
def peerDom (a b : AId) : End → Bool
  | .inn (x, _) => x == a || x == b
  | .out (x, _) => (bornIdx a b x).isSome

def peerVal (R : RHS κ) (a b : AId) : End → Option End
  | .inn (x, i) => if x = a then (R.wire (.ia i)).map (realize a b) else (R.wire (.ib i)).map (realize a b)
  | .out (x, s) => match bornIdx a b x with
    | some k => (R.wire (.nw k s)).map (realize a b)
    | none => none

/-- Fire the redex `(a, b)` with right-hand side `R`. Every component is an
override on a domain owned by the redex; everything else is unchanged. -/
def apply (R : RHS κ) (a b : AId) (N : Net κ) : Net κ where
  agents x := if agentDom a b x then agentVal R a b x else N.agents x
  peer e := if peerDom a b e then peerVal R a b e else N.peer e
  consumed x := N.consumed x || x == a || x == b

/-- The rule for the redex `(a, b)`, from the kinds of its two agents. -/
def ruleAt (T : Table κ) (N : Net κ) (a b : AId) : Option (RHS κ) :=
  match N.agents a, N.agents b with
  | some ka, some kb => T ka kb
  | _, _ => none

def fire (T : Table κ) (a b : AId) (N : Net κ) : Net κ :=
  match ruleAt T N a b with
  | some R => apply R a b N
  | none => N

/-- `(a, b)` is a redex of `N`: two different live agents whose principal ports
are wired to each other, with a rule for their kinds; the path names the firing
will create are unused (`fresh`; automatic in well-formed nets, see `Good` in
`Diamond.lean`). -/
structure Redex (T : Table κ) (N : Net κ) (a b : AId) : Prop where
  ne : a ≠ b
  rule : (ruleAt T N a b).isSome
  wab : Wire N (.out (a, 0)) (.out (b, 0))
  wba : Wire N (.out (b, 0)) (.out (a, 0))
  fresh : ∀ k, N.agents (.born a b k) = none ∧
    ∀ s, N.peer (.out (.born a b k, s)) = none ∧ N.peer (.inn (.born a b k, s)) = none

/-- The labelled step relation: fire the redex `r`. -/
def Fires (T : Table κ) (r : AId × AId) (N N' : Net κ) : Prop :=
  Redex T N r.1 r.2 ∧ N' = fire T r.1 r.2 N

/-- A right-hand side for a pair of equal kinds that does not depend on which
agent is called first: it creates nothing and only links interface ends, the
same way from either side (ERA–ERA, DUP–DUP with equal labels). -/
structure Symmetric (R : RHS κ) : Prop where
  no_agents : R.agents = []
  no_new : ∀ p k s, R.wire p ≠ some (.nw k s)
  nw_none : ∀ k s, R.wire (.nw k s) = none
  swap_ab : ∀ i, R.wire (.ib i) = (R.wire (.ia i)).map RP.swap

/-- At most one rule per unordered pair of kinds: if both orientations have a
rule, the kinds are equal and the rule is symmetric. -/
def Oriented (T : Table κ) : Prop :=
  ∀ k1 k2 R1 R2, T k1 k2 = some R1 → T k2 k1 = some R2 → k1 = k2 ∧ Symmetric R1

end Confluence.Net
