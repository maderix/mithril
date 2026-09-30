import Confluence.Rules

/-!
# A worked example (non-vacuity check)

`(λx. x) 5` with the result erased: APP `root 0`, LAM `root 1` (its parameter
wired to its own body), NUM 5 `root 2` on the argument, ERA `root 3` on the
return. Beta links the interface ends directly (argument–parameter,
return–body); afterwards the wire from the number runs through the two dead
agents' ports to the ERA, forming a new active pair, which then fires.
-/

namespace Confluence.Examples

open Confluence Confluence.Net Confluence.Mithril

def r (n : Nat) : AId := .root n

def peerOf (l : List (End × End)) (e : End) : Option End :=
  (l.find? (fun p => p.1 == e)).map (·.2)

def wires : List (End × End) :=
  let w := [(End.out (r 0, 0), End.out (r 1, 0)), (.out (r 0, 1), .out (r 2, 0)),
            (.out (r 0, 2), .out (r 3, 0)), (.out (r 1, 1), .out (r 1, 2))]
  w ++ w.map (fun p => (p.2, p.1))

def N0 : Net Kind where
  agents
    | .root 0 => some .app
    | .root 1 => some .lam
    | .root 2 => some (.num 5)
    | .root 3 => some .era
    | _ => none
  peer := peerOf wires
  consumed := fun _ => false

abbrev T := table P0

theorem born_not_listed (a b : AId) (k s : Nat) :
    peerOf wires (.out (.born a b k, s)) = none ∧ peerOf wires (.inn (.born a b k, s)) = none := by
  constructor <;> simp [peerOf, wires, r]

theorem redex0 : Redex T N0 (r 0) (r 1) where
  ne := by decide
  rule := rfl
  wab := Wire.arrive (by decide) (by decide)
  wba := Wire.arrive (by decide) (by decide)
  fresh := fun k => ⟨rfl, fun s => born_not_listed (r 0) (r 1) k s⟩

def N1 : Net Kind := fire T (r 0) (r 1) N0

/-- After beta, the number's wire reaches the ERA through both dead agents. -/
theorem wire1 : Wire N1 (.out (r 2, 0)) (.out (r 3, 0)) :=
  Wire.passOut (q := (r 0, 1)) (by decide) (by decide) <|
  Wire.passIn (q := (r 1, 1)) (by decide) <|
  Wire.passOut (q := (r 1, 2)) (by decide) (by decide) <|
  Wire.passIn (q := (r 0, 2)) (by decide) <|
  Wire.arrive (by decide) (by decide)

theorem wire1' : Wire N1 (.out (r 3, 0)) (.out (r 2, 0)) :=
  Wire.passOut (q := (r 0, 2)) (by decide) (by decide) <|
  Wire.passIn (q := (r 1, 2)) (by decide) <|
  Wire.passOut (q := (r 1, 1)) (by decide) (by decide) <|
  Wire.passIn (q := (r 0, 1)) (by decide) <|
  Wire.arrive (by decide) (by decide)

theorem redex1 : Redex T N1 (r 3) (r 2) where
  ne := by decide
  rule := rfl
  wab := wire1'
  wba := wire1
  fresh := fun k => by
    refine ⟨?_, fun s => ⟨?_, ?_⟩⟩ <;>
      simp [N1, fire, ruleAt, N0, T, table, Net.apply, agentDom, peerDom, bornIdx, r,
        born_not_listed]

def N2 : Net Kind := fire T (r 3) (r 2) N1

/-- The two firings leave no live agent among the four. -/
theorem N2_empty : ∀ n < 4, N2.agents (r n) = none := by decide

/-- `(0,1)` then `(3,2)` is a two-step reduction to a normal form. -/
theorem two_steps : StepN (NStep T) 2 N0 N2 :=
  StepN.cons ⟨(r 0, r 1), redex0, rfl⟩ (StepN.cons ⟨(r 3, r 2), redex1, rfl⟩ (StepN.refl _))

end Confluence.Examples
