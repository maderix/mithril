/-!
# Layer A: abstract rewriting with the diamond property

A reduction relation `step : α → α → Prop`, usually presented through labels
(`fire r x y`: firing redex `r` in `x` gives `y`). We prove, for any system with
the (one-step, "equal or join in one step each") diamond property:

1. `confluent` and `church_rosser`;
2. `same_length`: all reduction sequences from `x` to a normal form have the
   same length and end in the same normal form;
3. `steps_le` / `strongly_normalizing` / `no_infinite_reduction`: if one
   sequence from `x` reaches a normal form in `n` steps, then every sequence
   from `x` has at most `n` steps, so every sequence terminates;
4. `partial_reduction_sound`: any reduct of `x` has the same normal form as `x`.

Core Lean only; no Mathlib.
-/

namespace Confluence

universe u v

section
variable {α : Type u} (step : α → α → Prop)

/-- `StepN step n x y`: `x` reduces to `y` in exactly `n` steps. -/
inductive StepN : Nat → α → α → Prop
  | refl (x : α) : StepN 0 x x
  | cons {n : Nat} {x y z : α} : step x y → StepN n y z → StepN (n + 1) x z

/-- Reflexive-transitive closure. -/
def Star (x y : α) : Prop := ∃ n, StepN step n x y

/-- Conversion: the equivalence closure of `step`. -/
inductive Conv : α → α → Prop
  | base {x y : α} : step x y → Conv x y
  | refl (x : α) : Conv x x
  | symm {x y : α} : Conv x y → Conv y x
  | trans {x y z : α} : Conv x y → Conv y z → Conv x z

/-- `x` is a normal form: no step leaves it. -/
def Normal (x : α) : Prop := ∀ y, ¬ step x y

/-- The diamond property, with the degenerate case: two one-step reducts of
the same term are equal or join in exactly one step each. -/
def Diamond : Prop :=
  ∀ x y z, step x y → step x z → y = z ∨ ∃ w, step y w ∧ step z w

def Confluent : Prop :=
  ∀ x y z, Star step x y → Star step x z → ∃ w, Star step y w ∧ Star step z w

end

/-! ## Labelled presentation -/

section Labelled
variable {α : Type u} {ρ : Type v} (fire : ρ → α → α → Prop)

/-- The step relation of a labelled system: fire some redex. -/
def LStep (x y : α) : Prop := ∃ r, fire r x y

/-- Labelled diamond: firing the same redex is deterministic, and firing two
different redexes either gives the same result or the results join in one step
each. -/
structure LDiamond : Prop where
  det : ∀ {r : ρ} {x y z : α}, fire r x y → fire r x z → y = z
  diamond : ∀ {r s : ρ} {x y z : α}, r ≠ s → fire r x y → fire s x z →
    y = z ∨ ∃ w, (∃ s', fire s' y w) ∧ (∃ r', fire r' z w)

theorem LDiamond.toDiamond {fire : ρ → α → α → Prop} (h : LDiamond fire) :
    Diamond (LStep fire) := by
  intro x y z ⟨r, hr⟩ ⟨s, hs⟩
  by_cases e : r = s
  · subst e; exact Or.inl (h.det hr hs)
  · rcases h.diamond e hr hs with h1 | ⟨w, ⟨s', h2⟩, ⟨r', h3⟩⟩
    · exact Or.inl h1
    · exact Or.inr ⟨w, ⟨s', h2⟩, ⟨r', h3⟩⟩

end Labelled

/-! ## Basic facts about `StepN` and `Star` -/

section Basic
variable {α : Type u} {step : α → α → Prop}

theorem StepN.single {x y : α} (h : step x y) : StepN step 1 x y :=
  StepN.cons h (StepN.refl y)

theorem StepN.trans {n m : Nat} {x y z : α} (h1 : StepN step n x y) (h2 : StepN step m y z) :
    StepN step (n + m) x z := by
  induction h1 with
  | refl => simpa using h2
  | @cons n x y' z' hs _ ih =>
    have := StepN.cons hs (ih h2)
    rw [show n + 1 + m = n + m + 1 by omega]
    exact this

theorem StepN.snoc {n : Nat} {x y z : α} (h1 : StepN step n x y) (h2 : step y z) :
    StepN step (n + 1) x z :=
  StepN.trans h1 (StepN.single h2)

theorem StepN.zero_eq {x y : α} (h : StepN step 0 x y) : x = y := by
  cases h; rfl

theorem StepN.succ_inv {n : Nat} {x z : α} (h : StepN step (n + 1) x z) :
    ∃ y, step x y ∧ StepN step n y z := by
  cases h with
  | cons hs hr => exact ⟨_, hs, hr⟩

theorem Star.refl (x : α) : Star step x x := ⟨0, StepN.refl x⟩

theorem Star.single {x y : α} (h : step x y) : Star step x y := ⟨1, StepN.single h⟩

theorem Star.trans {x y z : α} (h1 : Star step x y) (h2 : Star step y z) : Star step x z :=
  let ⟨_, a⟩ := h1; let ⟨_, b⟩ := h2; ⟨_, StepN.trans a b⟩

theorem Star.head {x y z : α} (h1 : step x y) (h2 : Star step y z) : Star step x z :=
  Star.trans (Star.single h1) h2

theorem Normal.stepN_eq {n : Nat} {x y : α} (hx : Normal step x) (h : StepN step n x y) :
    n = 0 ∧ x = y := by
  cases h with
  | refl => exact ⟨rfl, rfl⟩
  | cons hs _ => exact absurd hs (hx _)

end Basic

/-! ## The diamond property and its consequences -/

section DiamondTheory
variable {α : Type u} {step : α → α → Prop}

/-- Strip lemma, with lengths: a single step `x → y` against `x →ⁿ z` either
lands on the sequence (`y →ⁿ⁻¹ z`) or closes with `y →ⁿ w` and `z → w`. -/
theorem strip (hd : Diamond step) {n : Nat} {x y z : α} (hy : step x y) (hz : StepN step n x z) :
    (∃ m, n = m + 1 ∧ StepN step m y z) ∨ ∃ w, StepN step n y w ∧ step z w := by
  induction hz generalizing y with
  | refl x => exact Or.inr ⟨y, StepN.refl y, hy⟩
  | @cons n x x1 z hx1 hrest ih =>
    rcases hd x y x1 hy hx1 with e | ⟨u, hyu, hx1u⟩
    · subst e; exact Or.inl ⟨n, rfl, hrest⟩
    · rcases ih hx1u with ⟨m, hm, hu⟩ | ⟨w, huw, hzw⟩
      · exact Or.inl ⟨n, rfl, by subst hm; exact StepN.cons hyu hu⟩
      · exact Or.inr ⟨w, StepN.cons hyu huw, hzw⟩

/-- 1. Diamond implies confluence. -/
theorem confluent (hd : Diamond step) : Confluent step := by
  have one : ∀ {x y z : α}, step x y → Star step x z → ∃ w, Star step y w ∧ Star step z w := by
    intro x y z hy ⟨n, hz⟩
    rcases strip hd hy hz with ⟨m, _, h⟩ | ⟨w, h1, h2⟩
    · exact ⟨z, ⟨m, h⟩, Star.refl z⟩
    · exact ⟨w, ⟨n, h1⟩, Star.single h2⟩
  intro x y z ⟨n, hy⟩ hz
  induction hy generalizing z with
  | refl => exact ⟨z, hz, Star.refl z⟩
  | @cons n x x1 y hx1 _ ih =>
    obtain ⟨w1, hx1w1, hzw1⟩ := one hx1 hz
    obtain ⟨w, hyw, hw1w⟩ := ih _ hx1w1
    exact ⟨w, hyw, Star.trans hzw1 hw1w⟩

/-- Church–Rosser: convertible terms have a common reduct. -/
theorem church_rosser (hd : Diamond step) {x y : α} (h : Conv step x y) :
    ∃ w, Star step x w ∧ Star step y w := by
  induction h with
  | base h => exact ⟨_, Star.single h, Star.refl _⟩
  | refl x => exact ⟨x, Star.refl x, Star.refl x⟩
  | symm _ ih => obtain ⟨w, a, b⟩ := ih; exact ⟨w, b, a⟩
  | trans _ _ ih1 ih2 =>
    obtain ⟨w1, a1, b1⟩ := ih1
    obtain ⟨w2, a2, b2⟩ := ih2
    obtain ⟨w, c1, c2⟩ := confluent hd _ _ _ b1 a2
    exact ⟨w, Star.trans a1 c1, Star.trans b2 c2⟩

/-- Unique normal forms. -/
theorem unique_normal_form (hd : Diamond step) {x y z : α}
    (hy : Star step x y) (hz : Star step x z) (ny : Normal step y) (nz : Normal step z) : y = z := by
  obtain ⟨w, hyw, hzw⟩ := confluent hd x y z hy hz
  obtain ⟨_, h1⟩ := hyw; obtain ⟨_, h2⟩ := hzw
  exact (ny.stepN_eq h1).2.trans (nz.stepN_eq h2).2.symm

/-- Key lemma: one step towards a normal form reached in `n+1` steps leaves
exactly `n` steps to that normal form. -/
theorem nf_step (hd : Diamond step) {n : Nat} {x x' y : α}
    (h : StepN step (n + 1) x y) (ny : Normal step y) (hx : step x x') : StepN step n x' y := by
  rcases strip hd hx h with ⟨m, hm, h'⟩ | ⟨w, _, hyw⟩
  · have : n = m := by omega
    subst this; exact h'
  · exact absurd hyw (ny w)

/-- 2. All reduction sequences from `x` to a normal form have the same length
and reach the same normal form. -/
theorem same_length (hd : Diamond step) {n m : Nat} {x y z : α}
    (hy : StepN step n x y) (ny : Normal step y) (hz : StepN step m x z) (nz : Normal step z) :
    n = m ∧ y = z := by
  induction hz generalizing n with
  | refl x =>
    obtain ⟨h1, h2⟩ := nz.stepN_eq hy
    exact ⟨h1, h2.symm⟩
  | @cons m x x2 z hx2 _ ih =>
    cases n with
    | zero =>
      have := hy.zero_eq; subst this
      exact absurd hx2 (ny _)
    | succ n =>
      obtain ⟨h1, h2⟩ := ih (nf_step hd hy ny hx2) nz
      exact ⟨by omega, h2⟩

/-- 3a. If `x` reaches a normal form in `n` steps, every reduction sequence from
`x` has at most `n` steps, and can still be completed to that normal form in the
remaining `n - m` steps. -/
theorem steps_le (hd : Diamond step) {n m : Nat} {x y z : α}
    (hy : StepN step n x y) (ny : Normal step y) (hz : StepN step m x z) :
    m ≤ n ∧ StepN step (n - m) z y := by
  induction hz generalizing n with
  | refl x => exact ⟨Nat.zero_le _, by simpa using hy⟩
  | @cons m x x1 z hx1 _ ih =>
    cases n with
    | zero =>
      have := hy.zero_eq; subst this
      exact absurd hx1 (ny _)
    | succ n =>
      obtain ⟨h1, h2⟩ := ih (nf_step hd hy ny hx1)
      refine ⟨by omega, ?_⟩
      rw [show n + 1 - (m + 1) = n - m by omega]
      exact h2

/-- 3b. No infinite reduction sequence starts from a term with a normal form. -/
theorem no_infinite_reduction (hd : Diamond step) {n : Nat} {x y : α}
    (hy : StepN step n x y) (ny : Normal step y) :
    ¬ ∃ f : Nat → α, f 0 = x ∧ ∀ i, step (f i) (f (i + 1)) := by
  intro ⟨f, h0, hf⟩
  have pre : ∀ k, StepN step k x (f k) := by
    intro k
    induction k with
    | zero => rw [h0]; exact StepN.refl x
    | succ k ih => exact StepN.snoc ih (hf k)
  have := (steps_le hd hy ny (pre (n + 1))).1
  omega

/-- 3c. Strong normalization: every reduction sequence from `x` terminates
(well-foundedness of the converse step relation at `x`). -/
theorem strongly_normalizing (hd : Diamond step) {n : Nat} {x y : α}
    (hy : StepN step n x y) (ny : Normal step y) : Acc (fun b a => step a b) x := by
  induction n generalizing x with
  | zero =>
    have := hy.zero_eq; subst this
    exact Acc.intro _ (fun z hz => absurd hz (ny z))
  | succ n ih =>
    exact Acc.intro _ (fun x' hx' => ih (nf_step hd hy ny hx'))

/-- 4. Partial reduction is sound: any reduct `y` of `x` has the normal form of
`x`, reached in exactly the remaining number of steps, and no other normal form. -/
theorem partial_reduction_sound (hd : Diamond step) {n k : Nat} {x y z : α}
    (hxz : StepN step n x z) (nz : Normal step z) (hxy : StepN step k x y) :
    k ≤ n ∧ StepN step (n - k) y z ∧ ∀ w, Star step y w → Normal step w → w = z := by
  obtain ⟨hk, hyz⟩ := steps_le hd hxz nz hxy
  refine ⟨hk, hyz, fun w hw nw => ?_⟩
  exact unique_normal_form hd hw ⟨_, hyz⟩ nw nz

end DiamondTheory

/-! ## Labelled corollaries -/

section LabelledCorollaries
variable {α : Type u} {ρ : Type v} {fire : ρ → α → α → Prop}

theorem LDiamond.confluent (h : LDiamond fire) : Confluent (LStep fire) :=
  Confluence.confluent h.toDiamond

theorem LDiamond.same_length (h : LDiamond fire) {n m : Nat} {x y z : α}
    (hy : StepN (LStep fire) n x y) (ny : Normal (LStep fire) y)
    (hz : StepN (LStep fire) m x z) (nz : Normal (LStep fire) z) : n = m ∧ y = z :=
  Confluence.same_length h.toDiamond hy ny hz nz

theorem LDiamond.steps_le (h : LDiamond fire) {n m : Nat} {x y z : α}
    (hy : StepN (LStep fire) n x y) (ny : Normal (LStep fire) y)
    (hz : StepN (LStep fire) m x z) : m ≤ n ∧ StepN (LStep fire) (n - m) z y :=
  Confluence.steps_le h.toDiamond hy ny hz

theorem LDiamond.partial_reduction_sound (h : LDiamond fire) {n k : Nat} {x y z : α}
    (hxz : StepN (LStep fire) n x z) (nz : Normal (LStep fire) z)
    (hxy : StepN (LStep fire) k x y) :
    k ≤ n ∧ StepN (LStep fire) (n - k) y z ∧
      ∀ w, Star (LStep fire) y w → Normal (LStep fire) w → w = z :=
  Confluence.partial_reduction_sound h.toDiamond hxz nz hxy

end LabelledCorollaries

end Confluence
