import Confluence.Abstract
import Confluence.Net

/-!
# Layer B, part 2: the diamond property for any oriented local rule table

For any kind type `κ` and any rule table `T` that is `Oriented` (at most one
rule per unordered pair of kinds), and whose right-hand sides are arbitrary
local rewrites (`RHS`: new agents plus a wiring of the redex's interface and the
new ports, including interface-to-interface links):

* `redexes_share`: two active pairs that share an agent are the same pair
  (possibly written in the other order): each agent has one principal port and
  the wire from it has one far end;
* `rule_local`: firing changes only the redex's own ids and ends;
* `redex_persists`: firing one redex leaves every other redex a redex, with the
  same two agents and kinds;
* `diamond`: two different redexes fired in either order give *equal* nets
  (plain equality, thanks to path names);
* `ldiamond`, and through Layer A: `net_confluent`, `net_same_length`,
  `net_steps_le`, `net_strongly_normalizing`, `net_partial_reduction_sound`.

Finally `Good` (a freshness invariant on path names) is preserved by firing and
makes the `fresh` clause of `Redex` automatic.
-/

namespace Confluence.Net

variable {κ : Type} {T : Table κ} {N : Net κ}

/-! ## Small facts -/

theorem ruleAt_some {a b : AId} {R : RHS κ} (h : ruleAt T N a b = some R) :
    ∃ ka kb, N.agents a = some ka ∧ N.agents b = some kb ∧ T ka kb = some R := by
  unfold ruleAt at h
  split at h
  · exact ⟨_, _, by assumption, by assumption, h⟩
  · cases h

theorem bornIdx_cases (a b x : AId) : (∃ k, x = .born a b k) ∨ bornIdx a b x = none := by
  cases h : bornIdx a b x with
  | none => exact Or.inr rfl
  | some k => exact Or.inl ⟨k, bornIdx_eq_some.1 h⟩

theorem Redex.getRule {a b : AId} (h : Redex T N a b) :
    ∃ R, ruleAt T N a b = some R ∧ fire T a b N = apply R a b N := by
  obtain ⟨R, hR⟩ := Option.isSome_iff_exists.1 h.rule
  exact ⟨R, hR, by simp [fire, hR]⟩

theorem Redex.live_a {a b : AId} (h : Redex T N a b) : N.agents a ≠ none := by
  obtain ⟨R, hR, _⟩ := h.getRule
  obtain ⟨_, _, h1, _, _⟩ := ruleAt_some hR
  simp [h1]

theorem Redex.live_b {a b : AId} (h : Redex T N a b) : N.agents b ≠ none := by
  obtain ⟨R, hR, _⟩ := h.getRule
  obtain ⟨_, _, _, h2, _⟩ := ruleAt_some hR
  simp [h2]

/-- Names a redex will create are not live agents. -/
theorem Redex.born_ne {a b x : AId} {k : Nat} (h : Redex T N a b) (hx : N.agents x ≠ none) :
    x ≠ .born a b k := by
  intro e; subst e; exact hx (h.fresh k).1

theorem agentDom_false_of_live {a b x : AId} (h : Redex T N a b) (hx : N.agents x ≠ none)
    (ha : x ≠ a) (hb : x ≠ b) : agentDom a b x = false := by
  simp only [agentDom, Bool.or_eq_false_iff, beq_eq_false_iff_ne, ne_eq]
  refine ⟨⟨ha, hb⟩, ?_⟩
  cases hk : bornIdx a b x with
  | none => rfl
  | some k => exact absurd (bornIdx_eq_some.1 hk) (h.born_ne hx)

theorem apply_agents_of_not_dom {R : RHS κ} {a b x : AId} (h : agentDom a b x = false) :
    (apply R a b N).agents x = N.agents x := by
  simp [apply, h]

theorem apply_peer_of_not_dom {R : RHS κ} {a b : AId} {e : End} (h : peerDom a b e = false) :
    (apply R a b N).peer e = N.peer e := by
  simp [apply, h]

/-- `rule_local`: firing `(a, b)` changes only the ids and ends it owns. -/
theorem rule_local {a b : AId} (h : Redex T N a b) :
    (∀ x, agentDom a b x = false → (fire T a b N).agents x = N.agents x) ∧
    (∀ e, peerDom a b e = false → (fire T a b N).peer e = N.peer e) := by
  obtain ⟨R, _, hf⟩ := h.getRule
  rw [hf]
  exact ⟨fun _ hx => apply_agents_of_not_dom hx, fun _ he => apply_peer_of_not_dom he⟩

/-! ## Two active pairs share no agent -/

theorem out_inj {p q : Port} (h : End.out p = End.out q) : p = q := by cases h; rfl

/-- `redexes_disjoint`: if two redexes share an agent they are the same pair,
possibly in the other orientation. -/
theorem redexes_share {a b c d : AId} (h1 : Redex T N a b) (h2 : Redex T N c d)
    (hov : a = c ∨ a = d ∨ b = c ∨ b = d) : (c = a ∧ d = b) ∨ (c = b ∧ d = a) := by
  rcases hov with e | e | e | e <;> subst e
  · have := out_inj (h1.wab.det h2.wab); cases this; exact Or.inl ⟨rfl, rfl⟩
  · have := out_inj (h1.wab.det h2.wba); cases this; exact Or.inr ⟨rfl, rfl⟩
  · have := out_inj (h1.wba.det h2.wab); cases this; exact Or.inr ⟨rfl, rfl⟩
  · have := out_inj (h1.wba.det h2.wba); cases this; exact Or.inl ⟨rfl, rfl⟩

/-- Different redexes that do not form the same unordered pair share no agent. -/
theorem redexes_disjoint {a b c d : AId} (h1 : Redex T N a b) (h2 : Redex T N c d)
    (hne : (a, b) ≠ (c, d)) (hsw : (a, b) ≠ (d, c)) :
    a ≠ c ∧ a ≠ d ∧ b ≠ c ∧ b ≠ d := by
  have key : ¬ (a = c ∨ a = d ∨ b = c ∨ b = d) := by
    intro hov
    rcases redexes_share h1 h2 hov with ⟨e1, e2⟩ | ⟨e1, e2⟩ <;> subst e1 <;> subst e2
    · exact hne rfl
    · exact hsw rfl
  exact ⟨fun e => key (Or.inl e), fun e => key (Or.inr (Or.inl e)),
    fun e => key (Or.inr (Or.inr (Or.inl e))), fun e => key (Or.inr (Or.inr (Or.inr e)))⟩

/-! ## Firing one redex preserves the others -/

theorem peer_apply_eq {R : RHS κ} {a b : AId} {e : End} (h : Redex T N a b)
    (he : ∀ x i, e = .inn (x, i) → x ≠ a ∧ x ≠ b) (hp : N.peer e ≠ none) :
    (apply R a b N).peer e = N.peer e := by
  apply apply_peer_of_not_dom
  cases e with
  | inn p =>
    obtain ⟨x, i⟩ := p
    have := he x i rfl
    simp [peerDom, this.1, this.2]
  | out p =>
    obtain ⟨x, s⟩ := p
    simp only [peerDom]
    cases hk : bornIdx a b x with
    | none => rfl
    | some k =>
      rw [bornIdx_eq_some] at hk; subst hk
      exact absurd ((h.fresh k).2 s).1 hp

theorem wire_apply {R : RHS κ} {a b : AId} (h : Redex T N a b) {e t : End} (hw : Wire N e t)
    (he : ∀ x i, e = .inn (x, i) → x ≠ a ∧ x ≠ b)
    (ht : ∀ q, t = .out q → q.1 ≠ a ∧ q.1 ≠ b) : Wire (apply R a b N) e t := by
  induction hw with
  | @arrive e q hp hl =>
    have hq := ht q rfl
    refine Wire.arrive ?_ ?_
    · rw [peer_apply_eq h he (by simp [hp])]; exact hp
    · rw [apply_agents_of_not_dom (agentDom_false_of_live h hl hq.1 hq.2)]; exact hl
  | @passOut e t q hp hd hw' ih =>
    have hqa : q.1 ≠ a := fun e => h.live_a (e ▸ hd)
    have hqb : q.1 ≠ b := fun e => h.live_b (e ▸ hd)
    refine Wire.passOut ?_ ?_ (ih (fun x i e => by cases e; exact ⟨hqa, hqb⟩) ht)
    · rw [peer_apply_eq h he (by simp [hp])]; exact hp
    · have : agentDom a b q.1 = false := by
        simp only [agentDom, Bool.or_eq_false_iff, beq_eq_false_iff_ne, ne_eq]
        refine ⟨⟨hqa, hqb⟩, ?_⟩
        cases hk : bornIdx a b q.1 with
        | none => rfl
        | some k =>
          rw [bornIdx_eq_some] at hk
          obtain ⟨f, hf⟩ := hw'.lookup
          obtain ⟨x, s⟩ := q
          simp only at hk; subst hk
          rw [((h.fresh k).2 s).2] at hf; cases hf
      rw [apply_agents_of_not_dom this]; exact hd
  | @passIn e t q hp hw' ih =>
    refine Wire.passIn ?_ (ih (fun x i e => by cases e) ht)
    rw [peer_apply_eq h he (by simp [hp])]; exact hp

theorem ruleAt_apply {R : RHS κ} {a b c d : AId}
    (hc : (apply R a b N).agents c = N.agents c) (hd : (apply R a b N).agents d = N.agents d) :
    ruleAt T (apply R a b N) c d = ruleAt T N c d := by
  simp only [ruleAt, hc, hd]

theorem redex_apply {R : RHS κ} {a b c d : AId} (hr : Redex T N a b) (hs : Redex T N c d)
    (hac : a ≠ c) (had : a ≠ d) (hbc : b ≠ c) (hbd : b ≠ d) : Redex T (apply R a b N) c d := by
  have hc : (apply R a b N).agents c = N.agents c :=
    apply_agents_of_not_dom (agentDom_false_of_live hr hs.live_a (Ne.symm hac) (Ne.symm hbc))
  have hd : (apply R a b N).agents d = N.agents d :=
    apply_agents_of_not_dom (agentDom_false_of_live hr hs.live_b (Ne.symm had) (Ne.symm hbd))
  refine ⟨hs.ne, ?_, ?_, ?_, ?_⟩
  · rw [ruleAt_apply hc hd]; exact hs.rule
  · exact wire_apply hr hs.wab (fun _ _ e => by cases e)
      (fun q e => by cases e; exact ⟨Ne.symm had, Ne.symm hbd⟩)
  · exact wire_apply hr hs.wba (fun _ _ e => by cases e)
      (fun q e => by cases e; exact ⟨Ne.symm hac, Ne.symm hbc⟩)
  · intro k
    have hna : AId.born c d k ≠ a := fun e => hr.live_a (e ▸ (hs.fresh k).1)
    have hnb : AId.born c d k ≠ b := fun e => hr.live_b (e ▸ (hs.fresh k).1)
    have hbi : bornIdx a b (.born c d k) = none := by
      simp only [bornIdx]; split
      · rename_i hh; exact absurd hh.1.symm hac
      · rfl
    refine ⟨?_, fun s => ⟨?_, ?_⟩⟩
    · rw [apply_agents_of_not_dom (by simp [agentDom, hna, hnb, hbi])]; exact (hs.fresh k).1
    · rw [apply_peer_of_not_dom (by simp [peerDom, hbi])]; exact ((hs.fresh k).2 s).1
    · rw [apply_peer_of_not_dom (by simp [peerDom, hna, hnb])]; exact ((hs.fresh k).2 s).2

/-! ## Commutation of disjoint firings -/

theorem ite_comm_disjoint {β : Type} (p q : Bool) (u v w : β) (h : p = true → q = true → False) :
    (if q = true then v else if p = true then u else w) =
    (if p = true then u else if q = true then v else w) := by
  cases p <;> cases q <;> simp_all

theorem apply_comm {Rr Rs : RHS κ} {a b c d : AId} (hr : Redex T N a b) (hs : Redex T N c d)
    (hac : a ≠ c) (had : a ≠ d) (hbc : b ≠ c) (hbd : b ≠ d) :
    apply Rs c d (apply Rr a b N) = apply Rr a b (apply Rs c d N) := by
  apply Net.ext'
  · funext x
    simp only [apply]
    apply ite_comm_disjoint
    intro h1 h2
    simp only [agentDom, Bool.or_eq_true, beq_iff_eq, Option.isSome_iff_exists] at h1 h2
    rcases h1 with (e | e) | ⟨k, hk⟩
    · subst e
      rcases h2 with (e | e) | ⟨k', hk'⟩
      · exact hac e
      · exact had e
      · exact hs.born_ne hr.live_a (bornIdx_eq_some.1 hk')
    · subst e
      rcases h2 with (e | e) | ⟨k', hk'⟩
      · exact hbc e
      · exact hbd e
      · exact hs.born_ne hr.live_b (bornIdx_eq_some.1 hk')
    · rw [bornIdx_eq_some] at hk; subst hk
      rcases h2 with (e | e) | ⟨k', hk'⟩
      · exact hr.born_ne hs.live_a e.symm
      · exact hr.born_ne hs.live_b e.symm
      · rw [bornIdx_eq_some] at hk'; cases hk'; exact hac rfl
  · funext e
    simp only [apply]
    apply ite_comm_disjoint
    intro h1 h2
    cases e with
    | inn p =>
      obtain ⟨x, i⟩ := p
      simp only [peerDom, Bool.or_eq_true, beq_iff_eq] at h1 h2
      rcases h1 with e | e <;> subst e <;> rcases h2 with e | e
      · exact hac e
      · exact had e
      · exact hbc e
      · exact hbd e
    | out p =>
      obtain ⟨x, s⟩ := p
      simp only [peerDom, Option.isSome_iff_exists] at h1 h2
      obtain ⟨k, hk⟩ := h1; obtain ⟨k', hk'⟩ := h2
      rw [bornIdx_eq_some] at hk hk'; subst hk; cases hk'; exact hac rfl
  · funext x
    simp only [apply]
    generalize N.consumed x = p
    generalize (x == a) = q1
    generalize (x == b) = q2
    generalize (x == c) = q3
    generalize (x == d) = q4
    revert p q1 q2 q3 q4; decide

/-! ## The swapped orientation of a symmetric rule -/

theorem realize_swap {a b : AId} {p : RP} (hp : ∀ k s, p ≠ .nw k s) :
    realize b a p.swap = realize a b p := by
  cases p with
  | ia i => rfl
  | ib j => rfl
  | nw k s => exact absurd rfl (hp k s)

theorem apply_swap {R : RHS κ} {a b : AId} (hR : Symmetric R) (hr : Redex T N a b)
    (hs : Redex T N b a) : apply R a b N = apply R b a N := by
  have hab := hr.ne
  have hmap : ∀ i, (R.wire (.ia i)).map (realize a b) = (R.wire (.ib i)).map (realize b a) ∧
      (R.wire (.ib i)).map (realize a b) = (R.wire (.ia i)).map (realize b a) := by
    intro i
    rw [hR.swap_ab i]
    cases hw : R.wire (.ia i) with
    | none => simp
    | some p =>
      have hp : ∀ k s, p ≠ .nw k s := fun k s e => hR.no_new _ k s (e ▸ hw)
      refine ⟨?_, ?_⟩
      · simp only [Option.map_some, Option.some.injEq]; exact (realize_swap hp).symm
      · simp only [Option.map_some, Option.some.injEq]
        cases p with
        | ia i => rfl
        | ib j => rfl
        | nw k s => exact absurd rfl (hp k s)
  apply Net.ext'
  · funext x
    simp only [apply, agentDom, agentVal]
    by_cases hxa : x = a
    · subst hxa; simp
    by_cases hxb : x = b
    · subst hxb; simp
    have hba : ¬ (b = a) := fun e => hab e.symm
    rcases bornIdx_cases a b x with ⟨k, rfl⟩ | h1
    · have h2 : bornIdx b a (.born a b k) = none := by simp [bornIdx, hba]
      simp [h2, bornIdx_born, hR.no_agents, (hr.fresh k).1, hxa, hxb]
    · rcases bornIdx_cases b a x with ⟨k, rfl⟩ | h2
      · simp [h1, bornIdx_born, hR.no_agents, (hs.fresh k).1, hxa, hxb]
      · simp [h1, h2, hxa, hxb]
  · funext e
    cases e with
    | inn p =>
      obtain ⟨x, i⟩ := p
      simp only [apply, peerDom, peerVal]
      by_cases hxa : x = a
      · subst hxa; simp [hab, (hmap i).1]
      by_cases hxb : x = b
      · subst hxb; simp [Ne.symm hab, (hmap i).2]
      simp [hxa, hxb]
    | out p =>
      obtain ⟨x, s⟩ := p
      simp only [apply, peerDom, peerVal]
      have hba : ¬ (b = a) := fun e => hab e.symm
      rcases bornIdx_cases a b x with ⟨k, rfl⟩ | h1
      · have h2 : bornIdx b a (.born a b k) = none := by simp [bornIdx, hba]
        simp [h2, bornIdx_born, hR.nw_none, ((hr.fresh k).2 s).1]
      · rcases bornIdx_cases b a x with ⟨k, rfl⟩ | h2
        · simp [h1, bornIdx_born, hR.nw_none, ((hs.fresh k).2 s).1]
        · simp [h1, h2]
  · funext x
    simp only [apply]
    generalize N.consumed x = p
    generalize (x == a) = q1
    generalize (x == b) = q2
    revert p q1 q2; decide

theorem fire_swap (hT : Oriented T) {a b : AId} (hr : Redex T N a b) (hs : Redex T N b a) :
    fire T a b N = fire T b a N := by
  obtain ⟨R1, h1, f1⟩ := hr.getRule
  obtain ⟨R2, h2, f2⟩ := hs.getRule
  obtain ⟨ka, kb, ea, eb, t1⟩ := ruleAt_some h1
  obtain ⟨kb', ka', eb', ea', t2⟩ := ruleAt_some h2
  rw [ea] at ea'; cases ea'; rw [eb] at eb'; cases eb'
  obtain ⟨hk, hsym⟩ := hT _ _ _ _ t1 t2
  subst hk
  rw [t1] at t2; cases t2
  rw [f1, f2]; exact apply_swap hsym hr hs

/-! ## The diamond -/

/-- `redex_persists`: firing `(a, b)` leaves a different redex `(c, d)` a redex,
with its two agents unchanged. -/
theorem redex_persists {a b c d : AId} (hr : Redex T N a b) (hs : Redex T N c d)
    (hac : a ≠ c) (had : a ≠ d) (hbc : b ≠ c) (hbd : b ≠ d) :
    Redex T (fire T a b N) c d ∧ (fire T a b N).agents c = N.agents c ∧
      (fire T a b N).agents d = N.agents d := by
  obtain ⟨R, _, hf⟩ := hr.getRule
  rw [hf]
  exact ⟨redex_apply hr hs hac had hbc hbd,
    apply_agents_of_not_dom (agentDom_false_of_live hr hs.live_a (Ne.symm hac) (Ne.symm hbc)),
    apply_agents_of_not_dom (agentDom_false_of_live hr hs.live_b (Ne.symm had) (Ne.symm hbd))⟩

/-- **Diamond.** Two different redexes of the same net: either they are one pair
written both ways (and give the same net), or each stays a redex after the other
fires, and the two orders give *equal* nets. -/
theorem diamond (hT : Oriented T) {a b c d : AId} (hr : Redex T N a b) (hs : Redex T N c d)
    (hne : (a, b) ≠ (c, d)) :
    fire T a b N = fire T c d N ∨
    (Redex T (fire T a b N) c d ∧ Redex T (fire T c d N) a b ∧
      fire T c d (fire T a b N) = fire T a b (fire T c d N)) := by
  by_cases hov : a = c ∨ a = d ∨ b = c ∨ b = d
  · rcases redexes_share hr hs hov with ⟨e1, e2⟩ | ⟨e1, e2⟩ <;> subst e1 <;> subst e2
    · exact absurd rfl hne
    · exact Or.inl (fire_swap hT hr hs)
  · have hac : a ≠ c := fun e => hov (Or.inl e)
    have had : a ≠ d := fun e => hov (Or.inr (Or.inl e))
    have hbc : b ≠ c := fun e => hov (Or.inr (Or.inr (Or.inl e)))
    have hbd : b ≠ d := fun e => hov (Or.inr (Or.inr (Or.inr e)))
    refine Or.inr ⟨(redex_persists hr hs hac had hbc hbd).1,
      (redex_persists hs hr (Ne.symm hac) (Ne.symm hbc) (Ne.symm had) (Ne.symm hbd)).1, ?_⟩
    obtain ⟨Rr, hRr, fr⟩ := hr.getRule
    obtain ⟨Rs, hRs, fs⟩ := hs.getRule
    have hc : (apply Rr a b N).agents c = N.agents c :=
      apply_agents_of_not_dom (agentDom_false_of_live hr hs.live_a (Ne.symm hac) (Ne.symm hbc))
    have hd : (apply Rr a b N).agents d = N.agents d :=
      apply_agents_of_not_dom (agentDom_false_of_live hr hs.live_b (Ne.symm had) (Ne.symm hbd))
    have ha : (apply Rs c d N).agents a = N.agents a :=
      apply_agents_of_not_dom (agentDom_false_of_live hs hr.live_a hac had)
    have hb : (apply Rs c d N).agents b = N.agents b :=
      apply_agents_of_not_dom (agentDom_false_of_live hs hr.live_b hbc hbd)
    have g1 : fire T c d (apply Rr a b N) = apply Rs c d (apply Rr a b N) := by
      simp [fire, ruleAt_apply (T := T) hc hd, hRs]
    have g2 : fire T a b (apply Rs c d N) = apply Rr a b (apply Rs c d N) := by
      simp [fire, ruleAt_apply (T := T) ha hb, hRr]
    rw [fr, fs, g1, g2]
    exact apply_comm hr hs hac had hbc hbd

/-- The labelled diamond of Layer A, for nets. -/
theorem ldiamond (hT : Oriented T) : LDiamond (Fires T) where
  det := by
    intro r x y z h1 h2
    rw [h1.2, h2.2]
  diamond := by
    intro r s x y z hne h1 h2
    obtain ⟨hr, rfl⟩ := h1
    obtain ⟨hs, rfl⟩ := h2
    rcases diamond hT hr hs (by cases r; cases s; exact hne) with e | ⟨h1, h2, h3⟩
    · exact Or.inl e
    · exact Or.inr ⟨_, ⟨s, h1, rfl⟩, ⟨r, h2, h3⟩⟩

/-! ## Layer A instantiated: confluence of every oriented local rule table -/

/-- One step of net reduction: fire some redex. -/
abbrev NStep (T : Table κ) : Net κ → Net κ → Prop := LStep (Fires T)

theorem net_confluent (hT : Oriented T) : Confluent (NStep T) :=
  (ldiamond hT).confluent

theorem net_church_rosser (hT : Oriented T) {x y : Net κ} (h : Conv (NStep T) x y) :
    ∃ w, Star (NStep T) x w ∧ Star (NStep T) y w :=
  church_rosser (ldiamond hT).toDiamond h

theorem net_same_length (hT : Oriented T) {n m : Nat} {x y z : Net κ}
    (hy : StepN (NStep T) n x y) (ny : Normal (NStep T) y)
    (hz : StepN (NStep T) m x z) (nz : Normal (NStep T) z) : n = m ∧ y = z :=
  (ldiamond hT).same_length hy ny hz nz

theorem net_steps_le (hT : Oriented T) {n m : Nat} {x y z : Net κ}
    (hy : StepN (NStep T) n x y) (ny : Normal (NStep T) y) (hz : StepN (NStep T) m x z) :
    m ≤ n ∧ StepN (NStep T) (n - m) z y :=
  (ldiamond hT).steps_le hy ny hz

theorem net_strongly_normalizing (hT : Oriented T) {n : Nat} {x y : Net κ}
    (hy : StepN (NStep T) n x y) (ny : Normal (NStep T) y) :
    Acc (fun b a => NStep T a b) x :=
  strongly_normalizing (ldiamond hT).toDiamond hy ny

theorem net_partial_reduction_sound (hT : Oriented T) {n k : Nat} {x y z : Net κ}
    (hxz : StepN (NStep T) n x z) (nz : Normal (NStep T) z) (hxy : StepN (NStep T) k x y) :
    k ≤ n ∧ StepN (NStep T) (n - k) y z ∧
      ∀ w, Star (NStep T) y w → Normal (NStep T) w → w = z :=
  (ldiamond hT).partial_reduction_sound hxz nz hxy

/-! ## Freshness of path names is an invariant -/

/-- An id is in use: live, consumed, or with an assigned end. -/
def Used (N : Net κ) (x : AId) : Prop :=
  N.agents x ≠ none ∨ N.consumed x = true ∨
    ∃ s, N.peer (.out (x, s)) ≠ none ∨ N.peer (.inn (x, s)) ≠ none

/-- Well-formed naming: live agents have not fired, and an id `born a b k` is in
use only after `a` and `b` have fired. -/
structure Good (N : Net κ) : Prop where
  live_fresh : ∀ x, N.agents x ≠ none → N.consumed x = false
  born_used : ∀ a b k, Used N (.born a b k) → N.consumed a = true ∧ N.consumed b = true

/-- An initial net built from `root` ids only is `Good`. -/
theorem Good.init (h : ∀ x, Used N x → ∃ n, x = .root n) (hc : ∀ x, N.consumed x = false) :
    Good N :=
  ⟨fun x _ => hc x, fun a b k hu => by obtain ⟨n, e⟩ := h _ hu; cases e⟩

/-- In a `Good` net the names a live agent's redex would create are unused. -/
theorem Good.fresh (hG : Good N) {a b : AId} (ha : N.agents a ≠ none) (k : Nat) :
    N.agents (.born a b k) = none ∧
      ∀ s, N.peer (.out (.born a b k, s)) = none ∧ N.peer (.inn (.born a b k, s)) = none := by
  have hu : ¬ Used N (.born a b k) := by
    intro hu
    have := (hG.born_used a b k hu).1
    rw [hG.live_fresh a ha] at this; cases this
  simp only [Used, not_or, not_exists, ne_eq, Classical.not_not] at hu
  exact ⟨hu.1, fun s => ⟨(hu.2.2 s).1, (hu.2.2 s).2⟩⟩

/-- In a `Good` net, every active pair with a rule is a `Redex`: the freshness
clause is automatic. -/
theorem Good.redex (hG : Good N) {a b : AId} (hne : a ≠ b) (hrule : (ruleAt T N a b).isSome)
    (wab : Wire N (.out (a, 0)) (.out (b, 0))) (wba : Wire N (.out (b, 0)) (.out (a, 0))) :
    Redex T N a b := by
  obtain ⟨R, hR⟩ := Option.isSome_iff_exists.1 hrule
  obtain ⟨_, _, h1, _, _⟩ := ruleAt_some hR
  exact ⟨hne, hrule, wab, wba, hG.fresh (by simp [h1])⟩

theorem Good.apply (hG : Good N) {R : RHS κ} {a b : AId} (hr : Redex T N a b) :
    Good (apply R a b N) := by
  have ca : N.consumed a = false := hG.live_fresh a hr.live_a
  have cb : N.consumed b = false := hG.live_fresh b hr.live_b
  -- anything in use after the firing was in use before, or is `a`, `b`, or `born a b _`
  have used_back : ∀ x, Used (Net.apply R a b N) x →
      Used N x ∨ (∃ k, x = .born a b k) := by
    intro x hu
    rcases hu with hl | hc | ⟨s, ho | hi⟩
    · simp only [Net.apply] at hl
      cases hd : agentDom a b x
      · rw [hd] at hl; exact Or.inl (Or.inl (by simpa using hl))
      · simp only [agentDom, Bool.or_eq_true, beq_iff_eq, Option.isSome_iff_exists] at hd
        rcases hd with (e | e) | ⟨k, hk⟩
        · subst e; exact Or.inl (Or.inl hr.live_a)
        · subst e; exact Or.inl (Or.inl hr.live_b)
        · exact Or.inr ⟨k, bornIdx_eq_some.1 hk⟩
    · simp only [Net.apply, Bool.or_eq_true, beq_iff_eq] at hc
      rcases hc with (hc | e) | e
      · exact Or.inl (Or.inr (Or.inl hc))
      · subst e; exact Or.inl (Or.inl hr.live_a)
      · subst e; exact Or.inl (Or.inl hr.live_b)
    · simp only [Net.apply] at ho
      cases hd : peerDom a b (.out (x, s))
      · rw [hd] at ho; exact Or.inl (Or.inr (Or.inr ⟨s, Or.inl (by simpa using ho)⟩))
      · simp only [peerDom, Option.isSome_iff_exists] at hd
        obtain ⟨k, hk⟩ := hd
        exact Or.inr ⟨k, bornIdx_eq_some.1 hk⟩
    · simp only [Net.apply] at hi
      cases hd : peerDom a b (.inn (x, s))
      · rw [hd] at hi; exact Or.inl (Or.inr (Or.inr ⟨s, Or.inr (by simpa using hi)⟩))
      · simp only [peerDom, Bool.or_eq_true, beq_iff_eq] at hd
        rcases hd with e | e
        · subst e; exact Or.inl (Or.inl hr.live_a)
        · subst e; exact Or.inl (Or.inl hr.live_b)
  refine ⟨?_, ?_⟩
  · intro x hx
    simp only [Net.apply] at hx ⊢
    cases hd : agentDom a b x
    · rw [hd] at hx
      simp only [agentDom, Bool.or_eq_false_iff, beq_eq_false_iff_ne, ne_eq] at hd
      simp [hG.live_fresh x (by simpa using hx), hd.1.1, hd.1.2]
    · rw [hd] at hx
      simp only [agentVal] at hx
      by_cases hab : x = a ∨ x = b
      · simp [hab] at hx
      · simp only [hab, ite_false] at hx
        cases hk : bornIdx a b x with
        | none => rw [hk] at hx; simp at hx
        | some k =>
          rw [bornIdx_eq_some] at hk; subst hk
          have hc : N.consumed (.born a b k) = false := by
            cases h' : N.consumed (.born a b k)
            · rfl
            · have := (hG.born_used a b k (Or.inr (Or.inl h'))).1
              rw [ca] at this; cases this
          simp only [not_or] at hab
          simp [hc, hab.1, hab.2]
  · intro x y k hu
    have mono : ∀ z, N.consumed z = true → (Net.apply R a b N).consumed z = true := by
      intro z hz; simp [Net.apply, hz]
    rcases used_back _ hu with hu' | ⟨k', e⟩
    · exact ⟨mono _ (hG.born_used x y k hu').1, mono _ (hG.born_used x y k hu').2⟩
    · cases e; exact ⟨by simp [Net.apply], by simp [Net.apply]⟩

/-- `Good` is preserved by every step. -/
theorem Good.fire (hG : Good N) {a b : AId} (hr : Redex T N a b) : Good (fire T a b N) := by
  obtain ⟨R, _, hf⟩ := hr.getRule
  rw [hf]; exact hG.apply hr

theorem Good.step (hG : Good N) {N' : Net κ} (h : NStep T N N') : Good N' := by
  obtain ⟨r, hr, rfl⟩ := h
  exact hG.fire hr

theorem Good.star (hG : Good N) {N' : Net κ} (h : Star (NStep T) N N') : Good N' := by
  obtain ⟨n, h⟩ := h
  induction h with
  | refl => exact hG
  | cons hs _ ih => exact ih (hG.step hs)


/-! ## Wiring is an involution, and the wire from a principal port is symmetric -/

/-- A right-hand side's wiring is a partial involution. -/
def PInv (R : RHS κ) : Prop := ∀ p q, R.wire p = some q → R.wire q = some p

/-- Well-wired nets: `peer` is an involution, and inner ends of live agents are
unused (a live agent is entered only through its outer ends). -/
structure WellWired (N : Net κ) : Prop where
  symm : ∀ e f, N.peer e = some f → N.peer f = some e
  inner_free : ∀ x s, N.agents x ≠ none → N.peer (.inn (x, s)) = none

/-- Walks can be reversed: by induction on the forward walk, carrying
"every end whose peer is `u` walks to the target". -/
theorem Wire.reverse_aux (hW : WellWired N) {target : End} {u t : End} (hw : Wire N u t)
    (hK : ∀ f, N.peer f = some u → Wire N f target) : ∀ q, t = .out q → Wire N (.out q) target := by
  induction hw with
  | @arrive u q hp _ =>
    intro q' e; cases e
    exact hK _ (hW.symm _ _ hp)
  | @passOut u t q1 hp _ _ ih =>
    apply ih
    intro f hf
    exact Wire.passIn hf (hK _ (hW.symm _ _ hp))
  | @passIn u t q1 hp _ ih =>
    apply ih
    intro f hf
    have hq1 : N.agents q1.1 = none := by
      cases h : N.agents q1.1 with
      | none => rfl
      | some k =>
        have := hW.inner_free q1.1 q1.2 (by simp [h])
        rw [hW.symm _ _ hp] at this; cases this
    exact Wire.passOut hf hq1 (hK _ (hW.symm _ _ hp))

/-- In a well-wired net, the wire between two live ports is symmetric. -/
theorem Wire.reverse (hW : WellWired N) {p q : Port} (hp : N.agents p.1 ≠ none)
    (hw : Wire N (.out p) (.out q)) : Wire N (.out q) (.out p) :=
  Wire.reverse_aux hW hw (fun _ hf => Wire.arrive hf hp) q rfl

theorem peerVal_realize {R : RHS κ} {a b : AId} (hab : a ≠ b) (p : RP) :
    peerVal R a b (realize a b p) = (R.wire p).map (realize a b) := by
  cases p with
  | ia i => simp [realize, peerVal]
  | ib j => simp [realize, peerVal, Ne.symm hab]
  | nw k s => simp [realize, peerVal, bornIdx_born]

theorem peerDom_realize {a b : AId} (p : RP) : peerDom a b (realize a b p) = true := by
  cases p <;> simp [realize, peerDom, bornIdx_born]

theorem peerDom_inv {a b : AId} {e : End} (h : peerDom a b e = true) :
    ∃ p, e = realize a b p := by
  cases e with
  | inn p =>
    obtain ⟨x, i⟩ := p
    simp only [peerDom, Bool.or_eq_true, beq_iff_eq] at h
    rcases h with e | e <;> subst e
    · exact ⟨.ia i, rfl⟩
    · exact ⟨.ib i, rfl⟩
  | out p =>
    obtain ⟨x, s⟩ := p
    simp only [peerDom, Option.isSome_iff_exists] at h
    obtain ⟨k, hk⟩ := h
    rw [bornIdx_eq_some] at hk; subst hk
    exact ⟨.nw k s, rfl⟩

/-- Firing a rule whose right-hand side wiring is an involution keeps the net well-wired. -/
theorem WellWired.apply (hW : WellWired N) {R : RHS κ} {a b : AId} (hr : Redex T N a b)
    (hR : PInv R) : WellWired (Net.apply R a b N) := by
  have hab := hr.ne
  refine ⟨?_, ?_⟩
  · intro e f h
    simp only [Net.apply] at h ⊢
    cases hd : peerDom a b e
    · rw [hd] at h; simp only [Bool.false_eq_true, ite_false] at h
      have hf : peerDom a b f = false := by
        cases hdf : peerDom a b f
        · rfl
        · obtain ⟨p, rfl⟩ := peerDom_inv hdf
          have back := hW.symm _ _ h
          exfalso
          cases p with
          | ia i => simp only [realize] at back; rw [hW.inner_free a i hr.live_a] at back; cases back
          | ib j => simp only [realize] at back; rw [hW.inner_free b j hr.live_b] at back; cases back
          | nw k s => simp only [realize] at back; rw [((hr.fresh k).2 s).1] at back; cases back
      rw [hf]; simpa using hW.symm _ _ h
    · rw [hd] at h; simp only [ite_true] at h
      obtain ⟨p, rfl⟩ := peerDom_inv hd
      rw [peerVal_realize hab] at h
      cases hq : R.wire p with
      | none => rw [hq] at h; cases h
      | some q =>
        rw [hq] at h; simp only [Option.map_some, Option.some.injEq] at h; subst h
        rw [peerDom_realize]; simp only [ite_true]; rw [peerVal_realize hab, hR _ _ hq]; rfl
  · intro x s hx
    simp only [Net.apply] at hx ⊢
    have hi : peerDom a b (.inn (x, s)) = false := by
      simp only [peerDom, Bool.or_eq_false_iff, beq_eq_false_iff_ne, ne_eq]
      cases hd : agentDom a b x
      · simp only [agentDom, Bool.or_eq_false_iff, beq_eq_false_iff_ne, ne_eq] at hd
        exact hd.1
      · rw [hd] at hx; simp only [ite_true, agentVal] at hx
        by_cases h' : x = a ∨ x = b
        · simp [h'] at hx
        · simpa [not_or] using h'
    rw [hi]; simp only [Bool.false_eq_true, ite_false]
    cases hd : agentDom a b x
    · rw [hd] at hx; exact hW.inner_free x s (by simpa using hx)
    · simp only [agentDom, Bool.or_eq_true, beq_iff_eq, Option.isSome_iff_exists] at hd
      rcases hd with (e | e) | ⟨k, hk⟩
      · subst e; simp [peerDom] at hi
      · subst e; simp [peerDom] at hi
      · rw [bornIdx_eq_some] at hk; subst hk; exact ((hr.fresh k).2 s).2

/-- A table all of whose right-hand sides are wired by an involution. -/
def WiredTable (T : Table κ) : Prop := ∀ k1 k2 R, T k1 k2 = some R → PInv R

theorem WellWired.fire (hW : WellWired N) (hT : WiredTable T) {a b : AId} (hr : Redex T N a b) :
    WellWired (fire T a b N) := by
  obtain ⟨R, hR, hf⟩ := hr.getRule
  obtain ⟨_, _, _, _, ht⟩ := ruleAt_some hR
  rw [hf]; exact hW.apply hr (hT _ _ _ ht)

theorem WellWired.step (hW : WellWired N) (hT : WiredTable T) {N' : Net κ} (h : NStep T N N') :
    WellWired N' := by
  obtain ⟨r, hr, rfl⟩ := h
  exact hW.fire hT hr

theorem WellWired.star (hW : WellWired N) (hT : WiredTable T) {N' : Net κ}
    (h : Star (NStep T) N N') : WellWired N' := by
  obtain ⟨n, h⟩ := h
  induction h with
  | refl => exact hW
  | cons hs _ ih => exact ih (hW.step hT hs)

/-- **Active pairs are redexes.** In a `Good`, well-wired net, two different live
agents whose principal ports are wired together (one direction suffices) and
whose kinds have a rule form a `Redex`. With `Good.star` and `WellWired.star`
this holds in every net reachable from a well-formed initial net: the step
relation fires exactly the active pairs. -/
theorem active_pair_redex (hG : Good N) (hW : WellWired N) {a b : AId} (hne : a ≠ b)
    (hrule : (ruleAt T N a b).isSome) (wab : Wire N (.out (a, 0)) (.out (b, 0))) :
    Redex T N a b := by
  obtain ⟨R, hR⟩ := Option.isSome_iff_exists.1 hrule
  obtain ⟨_, _, h1, _, _⟩ := ruleAt_some hR
  exact hG.redex hne hrule wab (Wire.reverse hW (by simp [h1]) wab)

end Confluence.Net
