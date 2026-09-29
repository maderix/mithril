//! Dive-form lowering (Core -> `lir`) and the expression lowering shared
//! with the rule form. Runtime values are Port raws (`u64`): NUM i56
//! immediates, CON cells (chained past arity 2), boxed floats. Ctor payloads
//! are read via `ctx.cell` (through the generated `field` helper), so no
//! readback is ever needed mid-run.
//!
//! # Linear cell discipline
//!
//! Values are consumed linearly: each function owns its (non-borrowed)
//! arguments, a `match`/`Proj` on the *last use* of a variable frees the
//! constructor spine (tree_drop-on-match), non-last uses of owned variables
//! deep-copy (`dup_val`), and read-only ("borrowed") parameters — inferred in
//! `lib.rs` — are passed raw, never freed by the callee, and reclaimed by the
//! caller (dive) or the CALL rule (fire) after completion. All frees inside a
//! dive are *deferred* into `fr` and applied only when the dive completes or
//! commits, so a fuel-out unwind never invalidates the original arguments.

use crate::lir::{work_fuel, self, as_i, bin, burn_fuel, c, cast, do_, err, free, i64_, idx, let_, num, ok, p, rec_addr, ret, set, truthy, u16_, u32_, u64_, u8_, usize_, v, Bop, FnDef, Inline, Pat, Ty, E, S};
use crate::rules::{emit_rec, SegQ};
use crate::ty::{Ty as CTy, Types};

/// Values emission shares (dups): their types can never be linear.
#[derive(Default)]
pub(crate) struct Shared {
    pub classes: HashSet<u32>,
    pub tuples: bool,
    pub poison: bool,
}
use crate::{cnt_dive, free_vars, has_call, Cnt};
use mithril_front::ast::{BinOp, CmpOp};
use mithril_front::core::{Core, CoreModule, UNREACHABLE_CTOR};
use std::collections::HashSet;

pub(crate) fn bin_code(op: &BinOp) -> u8 {
    match op {
        BinOp::Add => 0,
        BinOp::Sub => 1,
        BinOp::Mul => 2,
        BinOp::Div => 3,
        BinOp::FloorDiv => 4,
        BinOp::Mod => 5,
        BinOp::Shl => 6,
        BinOp::Shr => 7,
        BinOp::BitAnd => 8,
        BinOp::BitOr => 9,
        BinOp::BitXor => 10,
    }
}

pub(crate) fn cmp_code(op: &CmpOp) -> u8 {
    match op {
        CmpOp::Lt => 0,
        CmpOp::Le => 1,
        CmpOp::Gt => 2,
        CmpOp::Ge => 3,
        CmpOp::Eq => 4,
        CmpOp::Ne => 5,
    }
}

/// Native i64 arithmetic of a `bin_code` (wrapping; `floor_div` / `py_mod`
/// are helpers).
pub(crate) fn arith(code: u8, a: E, b: E) -> E {
    match code {
        0 => bin(Bop::Add, a, b),
        1 => bin(Bop::Sub, a, b),
        2 => bin(Bop::Mul, a, b),
        3 => bin(Bop::Div, a, b),
        4 => p("floor_div", vec![a, b]),
        5 => p("py_mod", vec![a, b]),
        6 => bin(Bop::Shl, a, b),
        7 => bin(Bop::Shr, a, b),
        8 => bin(Bop::And, a, b),
        9 => bin(Bop::Or, a, b),
        _ => bin(Bop::Xor, a, b),
    }
}

/// A comparison of a `cmp_code`.
pub(crate) fn compare(code: u8, a: E, b: E) -> E {
    bin([Bop::Lt, Bop::Le, Bop::Gt, Bop::Ge, Bop::Eq, Bop::Ne][code as usize], a, b)
}

/// The `Inline` of an attribute string from `lib.rs`.
pub(crate) fn inline_of(attr: &str) -> Inline {
    if attr.is_empty() {
        Inline::Default
    } else {
        Inline::Always
    }
}

/// The fuel argument of a dive call: the enclosing dive's budget, or a
/// dead local in the rule form (only bounded callees are called there).
fn fuel_arg(dive: bool) -> E {
    if dive {
        v("fuel")
    } else {
        E::Ref("fl0".into())
    }
}

/// How a match/proj scrutinee is held.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Hold {
    /// Owned last use: extract fields, free the spine.
    Consume,
    /// Shared owned value: dup extracted fields, leave the spine.
    BorrowDup,
    /// Borrow-derived value: raw field reads, owner is upstream.
    BorrowRaw,
}

/// Statement-oriented expression emitter. `dive` mode compiles calls as
/// nested `d_*` dives; rule mode must never see a call in expression
/// position (the body is ANF-normalized first).
pub(crate) struct Ex<'m> {
    pub tmp: u32,
    pub dive: bool,
    pub self_fid: u32,
    pub loop_form: bool,
    /// Remaining-use counts of owned variables (branch-aware max).
    pub rem: Cnt,
    /// >0 inside value-position If/Match branches: reads dup, never move.
    pub pinned: u32,
    /// the outer variables of the active value-position branches: only
    /// these are pinned (arm-local binders are owned normally)
    pub pinset: HashSet<u32>,
    /// per active value-position branch: the outer values dying in it
    pub dying: Vec<Vec<u32>>,
    /// Borrow-derived variables (dive mode only; empty in rule mode).
    pub bset: HashSet<u32>,
    /// Per-function parameter borrow modes.
    pub bor: &'m [Vec<bool>],
    /// Segment registry (dive mode only): fuel-out captures continuations
    /// as segments exactly like the rule form.
    pub sq: Option<&'m mut SegQ>,
    /// Enclosing value-position let continuations (x, body) whose RHS is
    /// mid-evaluation; a suspension inside chains records through them.
    pub kframes: Vec<(u32, Core)>,
    /// ctor id -> unbox slot (arity-1 int ctors carried in the port).
    pub unbox: &'m std::collections::HashMap<u32, u8>,
    /// fn -> returns a proven i56 (calls to these are int expressions).
    pub iret: &'m [bool],
    /// Inferred types (for share recording) and the module-wide recorder.
    pub tys: &'m Types,
    pub shared: &'m std::cell::RefCell<Shared>,
    /// Pending reuse tokens of the current call-free straight-line span
    /// (dive mode only): consumed cells a following construct may take over.
    pub toks: Vec<String>,
    /// Variables proven to hold i56 immediates (see `numeric_vars`): their
    /// dup/free are elided and arithmetic on them is emitted inline.
    pub ints: HashSet<u32>,
    /// Vars moved or shared into records by the capture being emitted.
    pub captured: HashSet<u32>,
    /// Rule form: dives whose continuation is being emitted inline, nested.
    pub inline_calls: u32,
    /// Vars holding a native scalar tuple result, as locals `q<var>_<i>`.
    pub ntup: HashSet<u32>,
    /// ... of those, the ones whose callee holds shifted ints
    pub ntup_sh: HashSet<u32>,
    /// The let binder whose right-hand side is being emitted.
    pub cur_let: Option<u32>,
    /// Dive form of a TRMC function: (ctor id, hole-fill rule).
    pub trmc: Option<(u32, u16)>,
    /// The delayed self call being emitted: (its var, evaluated args).
    pub pending: Option<(u32, Vec<E>)>,
    /// Destination-passing form: the tail parameter (not a real param).
    pub dps_param: Option<usize>,
    /// Native multi-value form (`n_<fid>`): the function returns its
    /// k-tuple as `[u64; k]` instead of a heap tuple (0: not this form).
    pub nret: usize,
}

thread_local! {
    /// `FOLDS[g]`: `g` is a proven fold split by its CALL rule (its dive
    /// form measures fuel per iteration, see fold.rs).
    pub(crate) static FOLDS: std::cell::RefCell<Vec<bool>> = const { std::cell::RefCell::new(Vec::new()) };
    /// the in-dive split code of each proven fold (see fold.rs)
    pub(crate) static FOLD_SPLIT: std::cell::RefCell<Vec<Option<Vec<S>>>> = const { std::cell::RefCell::new(Vec::new()) };
}

thread_local! {
    /// `NTUP[g] = k > 0`: dive function `g` has a native multi-value entry
    /// `n_<g>(..) -> Result<[u64; k], u64>` (see `ntup_fns`).
    pub(crate) static NTUP: std::cell::RefCell<Vec<usize>> = const { std::cell::RefCell::new(Vec::new()) };
}

pub(crate) fn ntup_of(g: u32) -> usize {
    NTUP.with(|n| n.borrow().get(g as usize).copied().unwrap_or(0))
}

/// `bo` starts with component bindings `xi = x[i]` (distinct i < k) and
/// then never mentions `x`: the shape tuple unpacking lowers to. Returns
/// the bindings and the rest.
pub(crate) fn proj_prefix(x: u32, k: usize, bo: &Core) -> Option<(Vec<(u32, usize)>, &Core)> {
    let mut binds: Vec<(u32, usize)> = Vec::new();
    let mut e = bo;
    while let Core::Let(y, r, rest) = e {
        match &**r {
            Core::Proj(v, i) if **v == Core::Var(x) && *i < k && binds.iter().all(|(_, j)| j != i) => {
                binds.push((*y, *i));
                e = rest;
            }
            _ => break,
        }
    }
    if binds.is_empty() || free_vars(e).contains(&x) {
        return None;
    }
    Some((binds, e))
}

/// The local of variable `i`.
pub(crate) fn vn(i: u32) -> String {
    format!("v{i}")
}

impl<'m> Ex<'m> {
    pub fn new(
        dive: bool,
        self_fid: u32,
        loop_form: bool,
        rem: Cnt,
        bset: HashSet<u32>,
        bor: &'m [Vec<bool>],
        ints: HashSet<u32>,
        sq: Option<&'m mut SegQ>,
        unbox: &'m std::collections::HashMap<u32, u8>,
        iret: &'m [bool],
        tys: &'m Types,
        shared: &'m std::cell::RefCell<Shared>,
    ) -> Ex<'m> {
        Ex { tmp: 0, dive, self_fid, loop_form, rem, pinned: 0, pinset: HashSet::new(), dying: Vec::new(), bset, bor, ints, captured: HashSet::new(), inline_calls: 0, ntup: HashSet::new(), ntup_sh: HashSet::new(), cur_let: None, trmc: None, pending: None, dps_param: None, nret: 0, sq, kframes: Vec::new(), unbox, iret, tys, shared, toks: Vec::new() }
    }

    /// Release every pending reuse token (before a call, branch, return or
    /// loop back-edge: tokens never cross them, preserving free-early LIFO
    /// locality for whatever runs next).
    pub(crate) fn flush_toks(&mut self, b: &mut Vec<S>) {
        for t in self.toks.drain(..) {
            b.push(do_(c("tok_free", vec![v(t)])));
        }
    }

    /// Record that variable `i`'s value is shared by emitted code.
    fn note_share(&self, i: u32) {
        let t = if self.self_fid == u32::MAX {
            CTy::Dyn
        } else {
            self.tys.var(self.self_fid as usize, i)
        };
        let mut sh = self.shared.borrow_mut();
        match t {
            CTy::Int => {}
            CTy::Flo => {} // boxed floats always carry an rc
            CTy::Adt(c) => {
                sh.classes.insert(c);
            }
            CTy::Tup(_) => sh.tuples = true,
            CTy::Arr(_) => {} // arrays always carry a refcount
            CTy::Dyn => sh.poison = true,
        }
    }

    /// `dup_val(ctx, v<i>)` into a fresh temp.
    fn dup_into(&mut self, i: u32, b: &mut Vec<S>) -> E {
        let t = self.fresh();
        b.push(let_(&t, Ty::U64, c("dup_val", vec![v(vn(i))])));
        v(t)
    }

    /// Read `e` without taking ownership (an array operand of a read): a
    /// variable is lent raw, and released after the read at its last use;
    /// any other expression is evaluated to a temporary released after.
    fn borrow_read(&mut self, e: &Core, b: &mut Vec<S>) -> (E, Option<E>) {
        if let Core::Var(i) = e {
            if self.bset.contains(i) || (self.pinned > 0 && self.pinset.contains(i)) {
                return (v(vn(*i)), None);
            }
            match self.rem.get_mut(i) {
                Some(r) if *r >= 2 => {
                    *r -= 1;
                    return (v(vn(*i)), None);
                }
                Some(r) if *r == 1 => {
                    *r = 0;
                    return (v(vn(*i)), Some(v(vn(*i))));
                }
                _ => return (v(vn(*i)), None),
            }
        }
        let t = self.val(e, true, b);
        (t.clone(), Some(t))
    }

    /// Emit the suspension path for a dive call whose result `rv` (a record
    /// awaiting its parent) came back as `Err`: build the continuation
    /// record chain — `own` (this let's continuation) then every enclosing
    /// value-position frame — patch parents inward, and return the
    /// outermost record for the caller to attach.
    pub(crate) fn emit_capture(&mut self, rv: &str, own: Option<(u32, &Core)>, b: &mut Vec<S>) {
        let saved = self.rem.clone();
        let mut cb: Vec<S> = Vec::new();
        let child = self.capture_chain(rv, own, &mut cb);
        // Every record took one reference (sharing when the frame still
        // had other uses); the frame itself is now abandoned, so release
        // what it still owns. Borrowed and held (pinned) values are not
        // ours to drop.
        if self.pinned == 0 {
            let mut left: Vec<u32> = self.rem.iter().filter(|(_, r)| **r > 0).map(|(v, _)| *v).collect();
            left.sort_unstable();
            for x in left {
                if !self.ints.contains(&x) && !self.bset.contains(&x) && self.captured.contains(&x) {
                    cb.push(free(v(vn(x))));
                }
            }
        }
        cb.push(ret(cast(v(child), Ty::U64)));
        self.rem = saved;
        // The chain runs once per suspension; keeping it out of line keeps
        // its record/spawn temporaries (and their stack slots) out of the
        // hot function so the entry fuel test can shrink-wrap.
        let name = format!("cap_{}", self.fresh());
        let vars = lir::free_locals(&cb, &is_var_local);
        let mut params = vec![(rv.to_string(), Ty::U32)];
        params.extend(vars.iter().map(|x| (x.clone(), Ty::U64)));
        let mut args = vec![cast(v(rv), Ty::U32)];
        args.extend(vars.iter().map(|x| v(x.clone())));
        let call = E::Call { f: name.clone(), ctx: true, args };
        b.push(S::Fn(Box::new(FnDef { name, ctx: true, params, ret: Ty::U64, body: cb, inline: Inline::Never, cold: true })));
        let ex = self.hole_exit(call, b);
        b.push(ret(err(ex)));
    }

    /// Emit the record chain for a suspension into `b`, returning the name
    /// of the outermost record (see `emit_capture`).
    fn capture_chain(&mut self, rv: &str, own: Option<(u32, &Core)>, b: &mut Vec<S>) -> String {
        self.captured.clear();
        let mut frames: Vec<(u32, Core)> = Vec::new();
        if let Some((x, bo)) = own {
            frames.push((x, bo.clone()));
        }
        for f in self.kframes.iter().rev() {
            frames.push(f.clone());
        }
        let mut child = rv.to_string();
        // Each frame's continuation splits into P, the bindings that do not
        // (transitively) need the pending value x, and J, the rest plus the
        // tail. When P does real work it has not started yet — spawn it now
        // as a task behind a pend-2 join record so it runs in parallel with
        // the suspended x, exactly like the rule form's pair fork but for
        // the whole independent suffix (`fa = f(x); y = g(..); fb = f(y)`
        // must not wait for fa). Sequential order is untouched: this only
        // happens on fuel-out.
        for (x, bo) in &frames {
            if let Some((p_body, live, j_body)) = split_frame(*x, bo) {
                let mut sq = self.sq.take().expect("dive capture without a segment registry");
                let env_p: Vec<u32> = free_vars(&p_body).into_iter().collect();
                let sid_p = sq.add(self.self_fid, vec![], env_p.clone(), p_body.clone());
                let (rn, rx) = crate::rules::join_records(self, &mut sq, *x, live, &j_body, &E::Const("NONE".into()), b);
                self.sq = Some(sq);
                b.push(do_(c("set_parent", vec![v(&child), rec_addr(&rx)])));
                let rp = emit_rec(self, &env_p, sid_p, 0, &bin(Bop::Or, rec_addr(&rn), u64_(1)), b);
                b.push(do_(c("ready_rec", vec![v(rp)])));
                child = rn;
            } else {
                let mut sq = self.sq.take().expect("dive capture without a segment registry");
                let rn = crate::rules::cont_rec(self, *x, bo, &E::Const("NONE".into()), b, &mut sq);
                self.sq = Some(sq);
                b.push(do_(c("set_parent", vec![v(&child), rec_addr(&rn)])));
                child = rn;
            }
        }
        child
    }

    /// A let whose RHS is a call: run the dive, capturing on suspension.
    /// `let x = f(a)` on a closure value: applied in the net region; on
    /// suspension the continuation is captured like a call's.
    fn let_app(&mut self, x: u32, f: &Core, a: &Core, bo: &Core, b: &mut Vec<S>) {
        let ef = self.val(f, true, b);
        let ea = self.val(a, true, b);
        self.let_try(x, c("apply", vec![ef, ea]), Vec::new(), bo, b);
    }

    fn let_call(&mut self, x: u32, g: u32, args: &[Core], bo: &Core, b: &mut Vec<S>) {
        let (mut call, post) = self.dive_call(g, args, b);
        // a fork site (the continuation has an independent part): the
        // device's parallel world makes the callee a task at once
        // (`fork_fuel` hands it no budget); a cut runs inline with the
        // caller's budget; the CPU has one world
        if split_frame(x, bo).is_some() {
            if let E::Call { args, .. } = &mut call {
                args[0] = p("fork_fuel", vec![v("fuel")]);
            }
        }
        self.let_try(x, call, post, bo, b);
    }

    /// `let x = <call>` that may suspend: bind the value, or capture the
    /// continuation `bo` (releasing `post`, the borrowed reads, on both paths).
    fn let_try(&mut self, x: u32, call: E, post: Vec<E>, bo: &Core, b: &mut Vec<S>) {
        let t = self.fresh();
        let mut h: Vec<S> = post.iter().map(|q| free(q.clone())).collect();
        self.emit_capture("r", Some((x, bo)), &mut h);
        b.push(S::Try(Pat::One(t.clone()), call, "r".into(), h));
        for q in post {
            b.push(free(q));
        }
        self.emit_bind(x, v(t), b);
    }

    /// `e` is an array whose elements are proven ints.
    fn int_arr(&self, e: &Core) -> bool {
        self.self_fid != u32::MAX && self.tys.expr(self.self_fid as usize, e) == CTy::Arr(true)
    }

    /// An expression whose runtime value is a proven i56 immediate.
    fn is_int(&self, e: &Core) -> bool {
        match e {
            Core::Prim(mithril_front::core::Prim::ArrGet, xs) => self.int_arr(&xs[0]),
            Core::Prim(mithril_front::core::Prim::ArrLen, _) => true,
            Core::Prim(p, _) if p.is_f32() => true,
            Core::Num(_) | Core::Cmp(..) => true,
            Core::Op2(_, a, b) => self.is_int(a) && self.is_int(b),
            Core::Var(i) => self.ints.contains(i),
            Core::Call(g, _) => self.iret.get(*g as usize).copied().unwrap_or(false),
            Core::If(_, t, f) => self.is_int(t) && self.is_int(f),
            Core::Let(_, _, b) => self.is_int(b),
            _ => false,
        }
    }

    pub fn fresh(&mut self) -> String {
        self.tmp += 1;
        format!("t{}", self.tmp)
    }

    fn is_braw(&self, e: &Core) -> bool {
        self.dive && matches!(e, Core::Var(i) if self.bset.contains(i))
    }

    /// Read variable `i`. `esc` = the value escapes into a structure /
    /// owned call / result, so a borrow-derived read must be deep-copied.
    pub fn use_var(&mut self, i: u32, esc: bool, b: &mut Vec<S>) -> E {
        if self.ints.contains(&i) {
            // immediates: no ownership, no copy
            if let Some(r) = self.rem.get_mut(&i) {
                *r = (*r - 1).max(0);
            }
            return v(vn(i));
        }
        if self.dive && self.bset.contains(&i) {
            if esc {
                self.note_share(i);
                return self.dup_into(i, b);
            }
            return v(vn(i));
        }
        if self.pinned > 0 && self.pinset.contains(&i) {
            self.note_share(i);
            return self.dup_into(i, b);
        }
        let r0 = self.rem.get(&i).copied();
        if r0.map_or(true, |r| r >= 2) {
            self.note_share(i);
        }
        match self.rem.get_mut(&i) {
            Some(r) if *r >= 2 => {
                *r -= 1;
                self.dup_into(i, b)
            }
            Some(r) if *r == 1 => {
                *r = 0;
                v(vn(i))
            }
            _ => self.dup_into(i, b),
        }
    }

    /// Entering one branch of a tail-position branch point: `live` are the
    /// variables in scope that the branch point uses, `local` this branch's
    /// uses. Use counts at a branch point cover all branches, so each is
    /// rebased to this branch, and an owned boxed value this branch never
    /// uses is released here (else it leaks on this path, or its one use
    /// makes a needless copy whose extra reference is never dropped).
    pub(crate) fn enter_branch(&mut self, live: &std::collections::BTreeSet<u32>, local: &Cnt, b: &mut Vec<S>) {
        for x in live {
            let l = local.get(x).copied().unwrap_or(0);
            let r = self.rem.get(x).copied().unwrap_or(0);
            let owned = self.pinned == 0
                && !self.ints.contains(x)
                && !self.bset.contains(x)
                && !self.ntup.contains(x)
                && self.pending.as_ref().map_or(true, |(y, _)| y != x);
            if l == 0 && r > 0 && owned {
                b.push(free(v(vn(*x))));
            }
            if r > 0 {
                self.rem.insert(*x, l);
            }
        }
    }

    /// Bind `let v{x} = expr;` and free it right away if it is never used.
    fn emit_bind(&mut self, x: u32, expr: E, b: &mut Vec<S>) {
        if self.ints.contains(&x) {
            b.push(let_(vn(x), Ty::U64, expr));
            return;
        }
        if !self.bset.contains(&x) && self.rem.get(&x).copied().unwrap_or(0) == 0 {
            b.push(free(expr));
        } else {
            b.push(let_(vn(x), Ty::U64, expr));
        }
    }

    /// Evaluate a match/proj scrutinee and decide its hold mode.
    pub(crate) fn scrutinee(&mut self, s: &Core, b: &mut Vec<S>) -> (E, Hold) {
        if let Core::Var(i) = s {
            if self.dive && self.bset.contains(i) {
                return (v(vn(*i)), Hold::BorrowRaw);
            }
            if self.pinned > 0 && self.pinset.contains(i) {
                self.note_share(*i);
                return (v(vn(*i)), Hold::BorrowDup);
            }
            if self.rem.get(i).copied().map_or(true, |r| r >= 2) {
                self.note_share(*i);
            }
            return match self.rem.get_mut(i) {
                Some(r) if *r >= 2 => {
                    *r -= 1;
                    (v(vn(*i)), Hold::BorrowDup)
                }
                Some(r) if *r == 1 => {
                    *r = 0;
                    (v(vn(*i)), Hold::Consume)
                }
                _ => (v(vn(*i)), Hold::BorrowDup),
            };
        }
        let e = self.val(s, false, b);
        (e, Hold::Consume)
    }

    /// Bind a match arm's fields under the scrutinee's hold mode; Consume
    /// also frees the constructor spine.
    pub(crate) fn bind_fields(&mut self, sv: &E, hold: Hold, cid: u32, binders: &[u32], tok: Option<u32>, b: &mut Vec<S>) {
        let names: Vec<String> = binders.iter().map(|bv| vn(*bv)).collect();
        let free_unused = |me: &Self, b: &mut Vec<S>| {
            for bv in binders {
                if me.rem.get(bv).copied().unwrap_or(0) == 0 {
                    b.push(free(v(vn(*bv))));
                }
            }
        };
        // Consume with a reuse token (the rewrite placed a `Reuse` of this
        // scrutinee var on some path of the arm)
        if self.dive && hold == Hold::Consume && binders.len() == 2 && tok.is_some() {
            let tk = format!("tok_v{}", tok.unwrap());
            b.push(S::Let(Pat::Tup(vec![names[0].clone(), names[1].clone(), tk.clone()]), Ty::Infer, c("consume2r", vec![sv.clone(), u16_(cid as u64)])));
            self.toks.push(tk);
            free_unused(self, b);
            return;
        }
        if hold == Hold::Consume && !binders.is_empty() && binders.len() <= 2 {
            match binders.len() {
                1 => {
                    b.push(S::Let(Pat::Tup(vec![names[0].clone(), "m_unused".into()]), Ty::Infer, c("consume2k", vec![sv.clone(), u16_(cid as u64)])));
                    b.push(free(v("m_unused")));
                }
                _ => b.push(S::Let(Pat::Tup(names.clone()), Ty::Infer, c("consume2k", vec![sv.clone(), u16_(cid as u64)]))),
            }
            free_unused(self, b);
            return;
        }
        if hold == Hold::Consume {
            // chained arity: move every field out, dropping the unused ones
            b.push(S::Let(Pat::Arr(names.clone()), Ty::Infer, c(&format!("consume_chain::<{}>", binders.len()), vec![sv.clone(), u16_(cid as u64)])));
            free_unused(self, b);
            return;
        }
        for (i, bv) in binders.iter().enumerate() {
            let cnt = self.rem.get(bv).copied().unwrap_or(0);
            let fld = c("field", vec![sv.clone(), usize_(i)]);
            match hold {
                Hold::BorrowRaw => b.push(let_(vn(*bv), Ty::U64, fld)),
                Hold::Consume => unreachable!(),
                Hold::BorrowDup => {
                    if cnt > 0 {
                        b.push(let_(vn(*bv), Ty::U64, c("dup_val", vec![fld])));
                    }
                }
            }
        }
    }

    /// A dive call `d_g(...)`; returns (call expression, frees to run after
    /// the call succeeds: owned values lent to borrowed parameters).
    fn dive_call(&mut self, g: u32, args: &[Core], b: &mut Vec<S>) -> (E, Vec<E>) {
        self.dive_call_as(g, args, false, b)
    }

    /// `native`: call the multi-value entry `n_<g>` (see `NTUP`).
    fn dive_call_as(&mut self, g: u32, args: &[Core], native: bool, b: &mut Vec<S>) -> (E, Vec<E>) {
        // rule-form segments have no fuel; only bounded callees (which never
        // consume any) can be called from them as plain expressions
        assert!(self.dive || crate::is_bounded(g), "codegen bug: call in a pure rule-form expression");
        // args first (they may take tokens), then release the rest (below)
        let modes = &self.bor[g as usize];
        let mut post = Vec::new();
        let mut es = vec![fuel_arg(self.dive)];
        for (j, a) in args.iter().enumerate() {
            if modes[j] {
                // borrowed parameter: lend a read
                match a {
                    Core::Var(i) if self.bset.contains(i) => es.push(v(vn(*i))),
                    Core::Var(i) => {
                        if self.pinned > 0 && self.pinset.contains(i) {
                            es.push(v(vn(*i)));
                        } else {
                            match self.rem.get_mut(i) {
                                Some(r) if *r >= 2 => {
                                    *r -= 1;
                                    es.push(v(vn(*i)));
                                }
                                Some(r) if *r == 1 => {
                                    // last use: we still own it after the call
                                    *r = 0;
                                    es.push(v(vn(*i)));
                                    post.push(v(vn(*i)));
                                }
                                _ => es.push(v(vn(*i))),
                            }
                        }
                    }
                    _ => {
                        let t = self.val(a, true, b);
                        post.push(t.clone());
                        es.push(t);
                    }
                }
            } else {
                let t = self.val(a, true, b);
                es.push(t);
            }
        }
        self.flush_toks(b);
        let entry = if native {
            "n"
        } else if self.dive && crate::fast::has_fast(g) {
            "q"
        } else {
            "d"
        };
        (E::Call { f: format!("{entry}_{g}"), ctx: true, args: es }, post)
    }

    /// A call to native scalar `g`: `[*fuel -= 1;] s_g(ctx?, fuel, conv(args)..)`.
    fn native_call(&mut self, g: u32, args: &[Core], b: &mut Vec<S>) -> E {
        let es: Vec<E> = args.iter().map(|a| self.val(a, false, b)).collect();
        if self.dive && crate::scalar::is_leaf(g) {
            b.push(work_fuel(i64_(1)));
        }
        let conv = if crate::scalar::shifted(g) { "sh" } else { "as_i" };
        let mut a = vec![fuel_arg(self.dive)];
        a.extend(es.into_iter().map(|e| p(conv, vec![e])));
        E::Call { f: format!("s_{g}"), ctx: !crate::scalar::ctx_arg(g).is_empty(), args: a }
    }

    /// A native result component back into a port.
    fn native_back(g: u32, e: E) -> E {
        if crate::scalar::shifted(g) {
            p("retag", vec![e])
        } else {
            num(e)
        }
    }

    /// A binary op or comparison: native on proven ints, else the runtime helper.
    fn binop(&mut self, x: &Core, y: &Core, b: &mut Vec<S>, code: u8, helper: &str, int: impl FnOnce(E, E) -> E) -> E {
        let ints = self.is_int(x) && self.is_int(y);
        let own = (!self.is_braw(x) as u8) | ((!self.is_braw(y) as u8) << 1);
        let ex = self.val(x, false, b);
        let ey = self.val(y, false, b);
        let t = self.fresh();
        let e = if ints { num(int(as_i(ex), as_i(ey))) } else { c(helper, vec![u8_(code as u64), ex, ey, u8_(own as u64)]) };
        b.push(let_(&t, Ty::U64, e));
        v(t)
    }

    /// Emit statements computing `e` into `b`; returns the expression
    /// (temp name, local, or literal) holding the value.
    pub fn val(&mut self, e: &Core, esc: bool, b: &mut Vec<S>) -> E {
        match e {
            Core::Lam(..) => {
                // a closure value: its entry instantiated over the captured
                // values (owned by the net from here on)
                let caps: Vec<u32> = free_vars(e).into_iter().collect();
                let es: Vec<E> = caps.iter().map(|x| self.use_var(*x, true, b)).collect();
                let id = crate::closure_entry(caps, e);
                let t = self.fresh();
                b.push(let_(&t, Ty::U64, c("build_closure", vec![u16_(id as u64), E::Slice(es)])));
                v(t)
            }
            Core::App(..) => unreachable!("codegen bug: closure application in value position (ANF binds it)"),
            Core::Prim(pr, args) => {
                use mithril_front::core::Prim;
                let t = self.fresh();
                let post_free = |b: &mut Vec<S>, post: Option<E>| {
                    if let Some(q) = post {
                        b.push(free(q));
                    }
                };
                match pr {
                    Prim::ArrNew if self.is_int(&args[1]) => {
                        let n = self.val(&args[0], false, b);
                        let x = self.val(&args[1], true, b);
                        b.push(let_(&t, Ty::U64, p("arr_new_i", vec![as_i(n), x])));
                    }
                    Prim::ArrGet if self.int_arr(&args[0]) => {
                        let (a, post) = self.borrow_read(&args[0], b);
                        let i = self.val(&args[1], false, b);
                        b.push(let_(&t, Ty::U64, p("arr_get_i", vec![a, as_i(i)])));
                        post_free(b, post);
                    }
                    Prim::ArrSet if self.int_arr(&args[0]) => {
                        // index and value first (they may read the array), then
                        // take the array: its last use moves instead of dup+free
                        let i = self.val(&args[1], false, b);
                        let x = self.val(&args[2], true, b);
                        let a = self.val(&args[0], true, b);
                        b.push(let_(&t, Ty::U64, c("arr_set_i", vec![a, as_i(i), x])));
                    }
                    Prim::ArrNew => {
                        let n = self.val(&args[0], false, b);
                        // n copies of the element: its type is shared
                        if let Core::Var(x) = &args[1] {
                            self.note_share(*x);
                        } else if !self.is_int(&args[1]) {
                            self.shared.borrow_mut().poison = true;
                        }
                        let x = self.val(&args[1], true, b);
                        b.push(let_(&t, Ty::U64, c("arr_new", vec![as_i(n), x])));
                    }
                    Prim::ArrGet => {
                        let (a, post) = self.borrow_read(&args[0], b);
                        let i = self.val(&args[1], false, b);
                        // the element stays in the array too: shared
                        match self.cur_let {
                            Some(x) => self.note_share(x),
                            None => self.shared.borrow_mut().poison = true,
                        }
                        b.push(let_(&t, Ty::U64, c("arr_get", vec![a, as_i(i)])));
                        post_free(b, post);
                    }
                    Prim::ArrSet => {
                        let i = self.val(&args[1], false, b);
                        let x = self.val(&args[2], true, b);
                        let a = self.val(&args[0], true, b);
                        b.push(let_(&t, Ty::U64, c("arr_set", vec![a, as_i(i), x])));
                    }
                    Prim::ArrLen => {
                        let (a, post) = self.borrow_read(&args[0], b);
                        b.push(let_(&t, Ty::U64, num(cast(p("arr_len_of", vec![a]), Ty::I64))));
                        post_free(b, post);
                    }
                    _ => {
                        let es: Vec<E> = args.iter().map(|a| as_i(self.val(a, false, b))).collect();
                        b.push(let_(&t, Ty::U64, num(p(crate::scalar::f32_fn(*pr), es))));
                    }
                }
                v(t)
            }
            Core::Num(n) => num(i64_(*n)),
            Core::Flo(x) => {
                let t = self.fresh();
                b.push(let_(&t, Ty::U64, c("flo", vec![E::Flo(*x)])));
                v(t)
            }
            Core::Var(i) => self.use_var(*i, esc, b),
            // ints: storing is the i56 wrap (num keeps 56 bits, as_i sign-extends)
            Core::Op2(op, x, y) => self.binop(x, y, b, bin_code(op), "bin", |ex, ey| arith(bin_code(op), ex, ey)),
            Core::Cmp(op, x, y) => self.binop(x, y, b, cmp_code(op), "cmp", |ex, ey| cast(compare(cmp_code(op), ex, ey), Ty::I64)),
            Core::If(cd, th, el) => {
                let ec = self.val(cd, false, b);
                self.flush_toks(b);
                let outer = self.pin_enter(&[th, el], &[]);
                let t = self.fresh();
                let mut bt = self.arm_own(th);
                self.block_into(th, esc, &t, &mut bt);
                let mut bf = self.arm_own(el);
                self.block_into(el, esc, &t, &mut bf);
                b.push(S::Decl(t.clone(), Ty::U64));
                b.push(S::If(truthy(ec), bt, bf));
                self.pin_leave(&[th, el], outer, b);
                v(t)
            }
            // a binding is owned unless it aliases a lent value: copy a lent read into it
            Core::Let(x, r, bo) => {
                if self.native_let(*x, r, bo, b) {
                    return self.val(bo, esc, b);
                }
                if self.dive {
                    if let Core::Call(g, args) = &**r {
                        if !crate::is_bounded(*g) {
                            self.let_call(*x, *g, args, bo, b);
                            return self.val(bo, esc, b);
                        }
                    }
                    if let Core::App(f, a) = &**r {
                        self.let_app(*x, f, a, bo, b);
                        return self.val(bo, esc, b);
                    }
                    if has_call(r) {
                        self.kframes.push((*x, (**bo).clone()));
                        self.cur_let = Some(*x);
                        let er = self.val(r, !self.bset.contains(x), b);
                        self.cur_let = None;
                        self.kframes.pop();
                        self.emit_bind(*x, er, b);
                        return self.val(bo, esc, b);
                    }
                }
                self.cur_let = Some(*x);
                let er = self.val(r, !self.bset.contains(x), b);
                self.cur_let = None;
                self.emit_bind(*x, er, b);
                self.val(bo, esc, b)
            }
            Core::Call(g, args) if crate::scalar::native_sig(*g).is_some_and(|s| s.ret == crate::scalar::Kind::S1) => {
                let call = self.native_call(*g, args, b);
                let t = self.fresh();
                b.push(let_(&t, Ty::U64, Self::native_back(*g, call)));
                v(t)
            }
            Core::Call(g, args) => {
                let (call, post) = self.dive_call(*g, args, b);
                let t = self.fresh();
                if crate::is_bounded(*g) {
                    // bounded callee: cannot suspend
                    b.push(S::Try(Pat::One(t.clone()), call, "_".into(), vec![S::Unreachable]));
                } else {
                    // Only reachable for a suspendable call in bare value
                    // position, which ANF forbids; the tail/let paths own
                    // every real call site.
                    let mut h: Vec<S> = post.iter().map(|q| free(q.clone())).collect();
                    self.emit_capture("r", None, &mut h);
                    b.push(S::Try(Pat::One(t.clone()), call, "r".into(), h));
                }
                for q in post {
                    b.push(free(q));
                }
                v(t)
            }
            Core::Ctor(cid, args) => {
                if *cid == UNREACHABLE_CTOR {
                    let t = self.fresh();
                    b.push(let_(&t, Ty::U64, p("mith_unreachable", vec![])));
                    return v(t);
                }
                assert!(*cid < 0xFFE, "codegen: ctor id {} collides with reserved tags", cid);
                if let Some(slot) = self.unbox.get(cid) {
                    let e0 = self.val(&args[0], false, b);
                    let t = self.fresh();
                    b.push(let_(&t, Ty::U64, p("ic", vec![u64_(*slot as u64), as_i(e0)])));
                    return v(t);
                }
                let es: Vec<E> = args.iter().map(|a| self.val(a, true, b)).collect();
                let t = self.fresh();
                b.push(let_(&t, Ty::U64, mk_con(*cid, es)));
                v(t)
            }
            Core::Reuse(x, cid, args) => {
                // decided by the reuse rewrite: build in v's consumed cell
                let es: Vec<E> = args.iter().map(|a| self.val(a, true, b)).collect();
                let t = self.fresh();
                let tk = format!("tok_v{x}");
                if self.dive && self.toks.iter().any(|y| *y == tk) {
                    self.toks.retain(|y| *y != tk);
                    let mut a = vec![v(tk), u16_(*cid as u64)];
                    a.extend(es);
                    b.push(let_(&t, Ty::U64, c("mk_con2r", a)));
                } else {
                    b.push(let_(&t, Ty::U64, mk_con(*cid, es)));
                }
                v(t)
            }
            Core::Tuple(items) => {
                let es: Vec<E> = items.iter().map(|a| self.val(a, true, b)).collect();
                let t = self.fresh();
                b.push(let_(&t, Ty::U64, mk_con(0xFFF, es)));
                v(t)
            }
            Core::Proj(x, i) if matches!(&**x, Core::Var(y) if self.ntup.contains(y)) => {
                let Core::Var(y) = &**x else { unreachable!() };
                if let Some(r) = self.rem.get_mut(y) {
                    *r = (*r - 1).max(0);
                }
                let q = v(format!("q{y}_{i}"));
                if self.ntup_sh.contains(y) {
                    p("retag", vec![q])
                } else {
                    num(q)
                }
            }
            Core::Proj(x, i) => {
                let (sv, hold) = self.scrutinee(x, b);
                let t = self.fresh();
                if hold == Hold::Consume {
                    // the container dies here: move the field out
                    b.push(let_(&t, Ty::U64, c("take_field", vec![sv, usize_(*i)])));
                } else {
                    // the container stays: the field is now shared, so its
                    // type must carry a refcount
                    match self.cur_let {
                        Some(y) => self.note_share(y),
                        None => self.shared.borrow_mut().poison = true,
                    }
                    b.push(let_(&t, Ty::U64, c("dup_val", vec![c("field", vec![sv, usize_(*i)])])));
                }
                v(t)
            }
            Core::Match(s, arms) => {
                let (sv, hold) = self.scrutinee(s, b);
                self.flush_toks(b);
                let t = self.fresh();
                let arm_refs: Vec<&Core> = arms.iter().map(|(_, _, b)| b).collect();
                let arm_bs: Vec<u32> = arms.iter().flat_map(|(_, bs, _)| bs.iter().copied()).collect();
                let outer = self.pin_enter(&arm_refs, &arm_bs);
                let unbox = self.unbox;
                let sw = plan_arms(&sv, arms, unbox, |i| {
                    let (cid, binders, body) = &arms[i];
                    let mut ab = self.arm_own(body);
                    if unbox.contains_key(cid) {
                        if let Some(bv) = binders.first() {
                            if self.rem.get(bv).copied().unwrap_or(0) > 0 || self.pinned > 0 {
                                ab.push(let_(vn(*bv), Ty::U64, num(as_i(sv.clone()))));
                            }
                        }
                    } else {
                        let tok = reuse_var(body, &sv);
                        self.bind_fields(&sv, hold, *cid, binders, tok, &mut ab);
                    }
                    self.block_into(body, esc, &t, &mut ab);
                    ab
                });
                b.push(S::Decl(t.clone(), Ty::U64));
                b.push(sw);
                self.pin_leave(&arm_refs, outer, b);
                v(t)
            }
        }
    }

    /// Enter a value-position branch point over `arms`: pin the outer
    /// variables they use (arms take their own references to those).
    fn pin_enter(&mut self, arms: &[&Core], arm_binders: &[u32]) -> Vec<u32> {
        fn binders(e: &Core, out: &mut HashSet<u32>) {
            e.walk(&mut |e| match e {
                Core::Let(x, _, _) | Core::Lam(x, _) => {
                    out.insert(*x);
                }
                Core::Match(_, arms) => out.extend(arms.iter().flat_map(|(_, bs, _)| bs.iter().copied())),
                _ => {}
            });
        }
        let mut inner: HashSet<u32> = arm_binders.iter().copied().collect();
        for a in arms {
            binders(a, &mut inner);
        }
        let mut outer: Vec<u32> = Vec::new();
        for a in arms {
            for x in free_vars(a) {
                if !inner.contains(&x) && !self.pinset.contains(&x) && !outer.contains(&x) {
                    outer.push(x);
                }
            }
        }
        // An owned outer value with no use after the branch dies in it: each
        // arm moves it (or frees it if unused), like a tail branch. Only
        // values still live after the branch are pinned.
        let mut used = Cnt::new();
        for a in arms {
            crate::cnt_expr(a, &mut used);
        }
        let owned = |me: &Self, x: u32| !me.ints.contains(&x) && !me.bset.contains(&x) && !me.ntup.contains(&x);
        let dying: Vec<u32> = outer
            .iter()
            .copied()
            .filter(|x| owned(self, *x) && self.rem.get(x).copied().unwrap_or(0) == used.get(x).copied().unwrap_or(0) && used.get(x).copied().unwrap_or(0) > 0)
            .collect();
        outer.retain(|x| !dying.contains(x));
        for x in &outer {
            self.pinset.insert(*x);
        }
        self.pinned += 1;
        self.dying.push(dying);
        outer
    }

    /// Entering one arm of a value-position branch: the values dying in the
    /// branch are owned by this arm with its own use counts; one this arm
    /// never uses is released here.
    fn arm_own(&mut self, arm: &Core) -> Vec<S> {
        let dying = self.dying.last().cloned().unwrap_or_default();
        let mut local = Cnt::new();
        crate::cnt_expr(arm, &mut local);
        let mut pre = Vec::new();
        for x in dying {
            let k = local.get(&x).copied().unwrap_or(0);
            self.rem.insert(x, k);
            if k == 0 {
                pre.push(free(v(vn(x))));
            }
        }
        pre
    }

    /// Leave it: the arms' uses of the pinned outer variables are retired
    /// (use counts include every arm), and an owned variable whose last
    /// use was in the arms is released once, here.
    fn pin_leave(&mut self, arms: &[&Core], outer: Vec<u32>, b: &mut Vec<S>) {
        self.pinned -= 1;
        for x in self.dying.pop().unwrap_or_default() {
            self.rem.insert(x, 0);
        }
        let mut used = Cnt::new();
        for a in arms {
            crate::cnt_expr(a, &mut used);
        }
        for x in outer {
            self.pinset.remove(&x);
            let k = used.get(&x).copied().unwrap_or(0);
            if k == 0 {
                continue;
            }
            if let Some(r) = self.rem.get_mut(&x) {
                let before = *r;
                *r = (*r - k).max(0);
                if before > 0
                    && *r == 0
                    && self.pinned == 0
                    && !self.ints.contains(&x)
                    && !self.bset.contains(&x)
                    && !self.ntup.contains(&x)
                {
                    b.push(free(v(vn(x))));
                }
            }
        }
    }

    /// `e` as a block body assigning its value to `t`.
    pub fn block_into(&mut self, e: &Core, esc: bool, t: &str, s: &mut Vec<S>) {
        let outer = std::mem::take(&mut self.toks);
        let x = self.val(e, esc, s);
        self.flush_toks(s);
        self.toks = outer;
        s.push(set(t, x));
    }

    /// An `Err` payload leaving the function: through the hole record in a
    /// TRMC loop (the payload is bound first: two context calls never nest).
    fn hole_exit(&mut self, r: E, b: &mut Vec<S>) -> E {
        match self.trmc {
            Some((_, rule)) => {
                let t = self.fresh();
                b.push(let_(&t, Ty::U64, r));
                c("hole_wrap", vec![v(t), v("th_head"), v("th_hole"), u16_(rule as u64)])
            }
            None => r,
        }
    }

    /// An `Ok` value leaving the function: into the hole in a TRMC loop.
    fn hole_value(&self, x: E) -> E {
        match self.trmc {
            Some(_) => c("hole_fill", vec![v("th_head"), v("th_hole"), x]),
            None => x,
        }
    }

    /// The two arms of an `if` in tail position (`e` = the whole `if`), each
    /// entered as its own branch and emitted by `emit`; `cnt` counts a
    /// body's uses for the form (`cnt_dive` / `cnt_rule`). Reuse tokens
    /// (dive form only; the rule form never holds any) carry into each arm.
    pub(crate) fn if_arms(&mut self, e: &Core, cd: &Core, th: &Core, el: &Core, cnt: fn(&Core, &mut Cnt), mut emit: impl FnMut(&mut Self, &Core, &mut Vec<S>), b: &mut Vec<S>) {
        let ec = self.val(cd, false, b);
        // tail branches inherit pending tokens; every arm ends in a
        // terminal (call/return/back-edge) that releases its unused ones
        let tk = self.toks.clone();
        let saved = self.rem.clone();
        let live = free_vars(e);
        let (mut lt, mut lf) = (Cnt::new(), Cnt::new());
        cnt(th, &mut lt);
        cnt(el, &mut lf);
        let mut bt = Vec::new();
        self.enter_branch(&live, &lt, &mut bt);
        emit(self, th, &mut bt);
        self.rem = saved.clone();
        self.toks = tk;
        let mut bf = Vec::new();
        self.enter_branch(&live, &lf, &mut bf);
        emit(self, el, &mut bf);
        self.rem = saved;
        self.toks.clear();
        b.push(S::If(truthy(ec), bt, bf));
    }

    /// The arms of a `match` in tail position (`e` = the whole match): each
    /// enters its branch, binds its fields (reusing the matched cell in the
    /// dive form) and is emitted by `emit`.
    pub(crate) fn match_arms(&mut self, e: &Core, s: &Core, arms: &[(u32, Vec<u32>, Core)], cnt: fn(&Core, &mut Cnt), mut emit: impl FnMut(&mut Self, &Core, &mut Vec<S>), b: &mut Vec<S>) {
        let (sv, hold) = self.scrutinee(s, b);
        let tk = self.toks.clone();
        let saved = self.rem.clone();
        let mut live = free_vars(e);
        if let Core::Var(x) = s {
            live.remove(x); // consumed by the match itself
        }
        let unbox = self.unbox;
        let sw = plan_arms(&sv, arms, unbox, |i| {
            let (cid, binders, body) = &arms[i];
            self.rem = saved.clone();
            self.toks = tk.clone();
            let mut ab = Vec::new();
            let mut local = Cnt::new();
            cnt(body, &mut local);
            self.enter_branch(&live, &local, &mut ab);
            if unbox.contains_key(cid) {
                if let Some(bv) = binders.first() {
                    if self.rem.get(bv).copied().unwrap_or(0) > 0 || self.pinned > 0 {
                        ab.push(let_(vn(*bv), Ty::U64, num(as_i(sv.clone()))));
                    }
                }
            } else {
                let tok = reuse_var(body, &sv);
                self.bind_fields(&sv, hold, *cid, binders, tok, &mut ab);
            }
            emit(self, body, &mut ab);
            ab
        });
        self.rem = saved;
        self.toks.clear();
        b.push(sw);
    }

    /// Emit `e` in dive tail position: ends every path with `return`, or
    /// `continue 'l` for self tail calls in loop form.
    pub fn dive_tail(&mut self, e: &Core, b: &mut Vec<S>) {
        match e {
            Core::Let(x, r, bo) => {
                if let (Some((cid, _)), Core::Call(g, args), None) = (self.trmc, &**r, &self.pending) {
                    let mut cs = Vec::new();
                    if *g == self.self_fid && delayed_ok(*x, bo, &mut cs) && cs.iter().all(|c| *c == cid) {
                        // delay the self call: arguments evaluated here,
                        // the call itself becomes the next loop iteration
                        let qs: Vec<E> = args
                            .iter()
                            .enumerate()
                            .map(|(i, a)| {
                                if Some(i) == self.dps_param {
                                    return E::Tup(vec![]);
                                }
                                let ea = self.val(a, true, b);
                                let q = self.fresh();
                                b.push(let_(&q, Ty::U64, ea));
                                v(q)
                            })
                            .collect();
                        self.pending = Some((*x, qs));
                        self.dive_tail(bo, b);
                        self.pending = None;
                        return;
                    }
                }
                if self.native_let(*x, r, bo, b) {
                    return self.dive_tail(bo, b);
                }
                if let Some(rest) = self.ntup_let(*x, r, bo, b) {
                    return self.dive_tail(rest, b);
                }
                if let Core::Call(g, args) = &**r {
                    if !crate::is_bounded(*g) {
                        self.let_call(*x, *g, args, bo, b);
                        return self.dive_tail(bo, b);
                    }
                }
                if let Core::App(f, a) = &**r {
                    self.let_app(*x, f, a, bo, b);
                    return self.dive_tail(bo, b);
                }
                if has_call(r) {
                    self.kframes.push((*x, (**bo).clone()));
                    self.cur_let = Some(*x);
                    let er = self.val(r, !self.bset.contains(x), b);
                    self.cur_let = None;
                    self.kframes.pop();
                    self.emit_bind(*x, er, b);
                    return self.dive_tail(bo, b);
                }
                self.cur_let = Some(*x);
                let er = self.val(r, !self.bset.contains(x), b);
                self.cur_let = None;
                self.emit_bind(*x, er, b);
                self.dive_tail(bo, b);
            }
            Core::If(cd, th, el) => {
                self.if_arms(e, cd, th, el, cnt_dive, |ex, body, ab| ex.dive_tail(body, ab), b);
            }
            Core::Match(s, arms) => self.match_arms(e, s, arms, cnt_dive, |ex, body, ab| ex.dive_tail(body, ab), b),
            Core::Call(g, args) if *g == self.self_fid && self.loop_form => {
                // Self tail call as a loop iteration: compute all next-state
                // values first (they read the current v*), then assign.
                // A borrow-derived value passed through a borrowed self
                // parameter stays raw (same frame, owner unchanged).
                let modes = self.bor[*g as usize].clone();
                let es: Vec<E> = args
                    .iter()
                    .enumerate()
                    .map(|(j, a)| {
                        if modes[j] && self.is_braw(a) {
                            if let Core::Var(i) = a {
                                v(vn(*i))
                            } else {
                                unreachable!()
                            }
                        } else {
                            self.val(a, true, b)
                        }
                    })
                    .collect();
                for (i, ea) in es.iter().enumerate() {
                    b.push(let_(format!("n{i}"), Ty::U64, ea.clone()));
                }
                for i in 0..es.len() {
                    b.push(set(vn(i as u32), v(format!("n{i}"))));
                }
                self.flush_toks(b);
                b.push(S::Continue);
            }
            Core::Ctor(cid, a) | Core::Reuse(_, cid, a)
                if self.pending.as_ref().is_some_and(|(x, _)| a.len() == 2 && a[1] == Core::Var(*x)) =>
            {
                let reuse = if let Core::Reuse(x, _, _) = e { Some(*x) } else { None };
                self.trmc_cons(*cid, reuse, &a[0], b);
            }
            Core::Call(g, args)
                if self.pending.as_ref().is_some_and(|(x, _)| {
                    dps_of(*g).is_some_and(|(pp, _)| args[pp] == Core::Var(*x))
                }) =>
            {
                let (pp, _) = dps_of(*g).unwrap();
                self.trmc_dps(*g, pp, args, b);
            }
            Core::Var(x) if self.dps_param.is_some_and(|pp| pp as u32 == *x) => {
                // destination-passing callee reached its tail parameter:
                // the hole stays open for the caller
                self.flush_toks(b);
                b.push(S::Store("head_out".into(), v("th_head")));
                b.push(S::Store("hole_out".into(), v("th_hole")));
                b.push(lir::ret_unit());
            }
            Core::Call(g, args) if self.trmc.is_some() => {
                let (call, post) = self.dive_call(*g, args, b);
                b.push(let_("tr", Ty::Res, call));
                for q in post {
                    b.push(free(q));
                }
                let mut eb = Vec::new();
                let ex = self.hole_exit(v("r"), &mut eb);
                eb.push(ret(err(ex)));
                b.push(S::Res(v("tr"), "v".into(), vec![ret(ok(self.hole_value(v("v"))))], "r".into(), eb));
            }
            Core::Call(g, args)
                if self.nret > 0
                    && crate::scalar::native_sig(*g).is_some_and(|sig| sig.ret == crate::scalar::Kind::SK(self.nret)) =>
            {
                // the callee is native with this shape: its components come
                // back in registers, no bridge tuple
                let k = self.nret;
                let call = self.native_call(*g, args, b);
                self.flush_toks(b);
                let rs: Vec<String> = (0..k).map(|i| format!("r{i}")).collect();
                b.push(S::Let(Pat::Tup(rs.clone()), Ty::Infer, call));
                b.push(ret(ok(E::Arr(rs.iter().map(|r| Self::native_back(*g, v(r.clone()))).collect()))));
            }
            Core::Call(g, args) if self.nret > 0 => {
                // native multi-value form: a callee with the same native
                // shape passes its components straight through; any other
                // result is unpacked (a suspension still delivers the boxed
                // tuple to our caller's continuation)
                let k = self.nret;
                let native = ntup_of(*g) == k;
                let (call, post) = self.dive_call_as(*g, args, native, b);
                b.push(let_("tr", if native { Ty::ResArr(k) } else { Ty::Res }, call));
                for q in post {
                    b.push(free(q));
                }
                if native {
                    b.push(ret(v("tr")));
                } else {
                    b.push(S::Res(v("tr"), "v".into(), vec![ret(ok(c(&format!("untup::<{k}>"), vec![v("v")])))], "r".into(), vec![ret(err(v("r")))]));
                }
            }
            Core::App(f, a) => {
                let ef = self.val(f, true, b);
                let ea = self.val(a, true, b);
                self.flush_toks(b);
                b.push(ret(c("apply", vec![ef, ea])));
            }
            Core::Call(g, args) => {
                // Tail call: pass our own destination through, so a downstream
                // suspension spawns its pending call against the right parent.
                let (call, post) = self.dive_call(*g, args, b);
                if post.is_empty() {
                    b.push(ret(call));
                } else {
                    b.push(let_("tr", Ty::Res, call));
                    for q in post {
                        b.push(free(q));
                    }
                    b.push(ret(v("tr")));
                }
            }
            Core::Tuple(items) if self.nret > 0 && self.nret == items.len() => {
                let es: Vec<E> = items.iter().map(|a| self.val(a, true, b)).collect();
                self.flush_toks(b);
                b.push(ret(ok(E::Arr(es))));
            }
            other if self.nret > 0 => {
                let x = self.val(other, true, b);
                self.flush_toks(b);
                b.push(ret(ok(c(&format!("untup::<{}>", self.nret), vec![x]))));
            }
            other => {
                let x = self.val(other, true, b);
                self.flush_toks(b);
                let hv = self.hole_value(x);
                b.push(ret(ok(hv)));
            }
        }
    }

    /// `x = g(..)` with `g` a native multi-value dive function and `bo`
    /// binding its components (`proj_prefix`): call `n_<g>` and bind the
    /// components from the returned array, no heap tuple. Returns the rest
    /// of the body to emit. On suspension the continuation receives the
    /// boxed tuple and runs `bo` as written.
    fn ntup_let<'a>(&mut self, x: u32, r: &Core, bo: &'a Core, b: &mut Vec<S>) -> Option<&'a Core> {
        if !self.dive || self.pending.is_some() {
            return None;
        }
        let Core::Call(g, args) = r else { return None };
        let k = ntup_of(*g);
        if k == 0 {
            return None;
        }
        let (binds, rest) = proj_prefix(x, k, bo)?;
        let (call, post) = self.dive_call_as(*g, args, true, b);
        let t = self.fresh();
        if crate::is_bounded(*g) {
            // bounded work never suspends
            b.push(S::Try(Pat::One(t.clone()), call, "_".into(), vec![S::Unreachable]));
        } else {
            let mut h: Vec<S> = post.iter().map(|q| free(q.clone())).collect();
            self.emit_capture("r", Some((x, bo)), &mut h);
            b.push(S::Try(Pat::One(t.clone()), call, "r".into(), h));
        }
        for q in post {
            b.push(free(q));
        }
        // x itself is never materialized
        self.rem.insert(x, 0);
        for i in 0..k {
            match binds.iter().find(|(_, j)| *j == i) {
                Some((y, _)) => self.emit_bind(*y, idx(v(&t), i), b),
                None => b.push(free(idx(v(&t), i))),
            }
        }
        Some(rest)
    }

    /// `x = g(..)` with `g` native scalar returning a tuple that `bo` only
    /// projects: destructure the native result into locals instead of
    /// packing it into a heap tuple.
    fn native_let(&mut self, x: u32, r: &Core, bo: &Core, b: &mut Vec<S>) -> bool {
        let Core::Call(g, args) = r else { return false };
        let Some(sig) = crate::scalar::native_sig(*g) else { return false };
        let crate::scalar::Kind::SK(k) = sig.ret else { return false };
        // a projection after a suspendable call would put the tuple in a
        // capture, and it has no boxed form: the boxed bridge instead
        if !only_projected(x, bo) || (has_call(bo) && proj_prefix(x, k, bo).is_none()) {
            return false;
        }
        let call = self.native_call(*g, args, b);
        b.push(S::Let(Pat::Tup((0..k).map(|i| format!("q{x}_{i}")).collect()), Ty::Infer, call));
        self.ntup.insert(x);
        if crate::scalar::shifted(*g) {
            self.ntup_sh.insert(x);
        }
        true
    }

    /// Emit a delayed self call's continuation: the cell (tail case a) or
    /// the destination-passing callee (case b) is linked into the hole, then
    /// the self call runs as the next loop iteration with the arguments
    /// evaluated where the call stood.
    fn trmc_continue(&mut self, b: &mut Vec<S>) {
        let (_, qs) = self.pending.clone().expect("trmc_continue without a pending self call");
        self.flush_toks(b);
        for (i, q) in qs.iter().enumerate() {
            if Some(i) != self.dps_param {
                b.push(set(vn(i as u32), q.clone()));
            }
        }
        b.push(S::Continue);
    }

    /// Tail case (a): `C(f0, x)` with `x` the pending self call.
    fn trmc_cons(&mut self, cid: u32, reuse: Option<u32>, f0: &Core, b: &mut Vec<S>) {
        let e0 = self.val(f0, true, b);
        let pn = self.fresh();
        let tk = reuse.map(|x| format!("tok_v{x}"));
        match tk {
            Some(tk) if self.toks.iter().any(|t| *t == tk) => {
                self.toks.retain(|t| *t != tk);
                b.push(let_(&pn, Ty::U64, c("mk_con2r", vec![v(tk), u16_(cid as u64), e0, u64_(0)])));
            }
            _ => b.push(let_(&pn, Ty::U64, c("mk_con2", vec![u16_(cid as u64), e0, u64_(0)]))),
        }
        b.push(do_(c("hole_link", vec![E::Ref("th_head".into()), E::Ref("th_hole".into()), v(pn)])));
        self.trmc_continue(b);
    }

    /// Tail case (b): `g(.., x, ..)` with `x` the pending self call in
    /// `g`'s tail parameter: `g` appends its cells into our hole.
    fn trmc_dps(&mut self, g: u32, pp: usize, args: &[Core], b: &mut Vec<S>) {
        let mut a = vec![v("fuel")];
        for (i, x) in args.iter().enumerate() {
            if i != pp {
                let e = self.val(x, true, b);
                a.push(e);
            }
        }
        a.push(E::Ref("th_head".into()));
        a.push(E::Ref("th_hole".into()));
        b.push(do_(E::Call { f: format!("dp_{g}"), ctx: true, args: a }));
        self.trmc_continue(b);
    }
}

/// A `v<n>` local (the variables of the Core body).
fn is_var_local(x: &str) -> bool {
    x.len() > 1 && x.starts_with('v') && x[1..].bytes().all(|d| d.is_ascii_digit())
}

/// The scrutinee variable (if `sv` names one) when `body` reuses its cell.
pub(crate) fn reuse_var(body: &Core, sv: &E) -> Option<u32> {
    let E::V(name) = sv else { return None };
    let x: u32 = name.strip_prefix('v')?.parse().ok()?;
    let has = |e: &Core| e.any(&mut |e| if matches!(e, Core::Reuse(w, _, _) if *w == x) { Some(true) } else { None });
    if has(body) { Some(x) } else { None }
}

/// Switch lowering for a constructor match on `sv`: dispatch on the port's
/// tag byte first (an unboxed ctor IS its tag byte), then on `con_tag` only
/// among >= 2 boxed ctors; the last boxed arm takes the remaining case
/// (matches are exhaustive by construction). `arm(i)` emits arm `i`'s body.
pub(crate) fn plan_arms(sv: &E, arms: &[(u32, Vec<u32>, Core)], unbox: &std::collections::HashMap<u32, u8>, mut arm: impl FnMut(usize) -> Vec<S>) -> S {
    let live: Vec<usize> = (0..arms.len()).filter(|&i| arms[i].0 != UNREACHABLE_CTOR).collect();
    let ub: Vec<usize> = live.iter().copied().filter(|&i| unbox.contains_key(&arms[i].0)).collect();
    let bx: Vec<usize> = live.iter().copied().filter(|&i| !unbox.contains_key(&arms[i].0)).collect();
    let mut cases = Vec::new();
    for &i in &ub {
        cases.push((16 + unbox[&arms[i].0] as u64, arm(i)));
    }
    let default = match bx.len() {
        0 => None,
        1 => Some(arm(bx[0])),
        n => {
            let mut inner = Vec::new();
            for &i in &bx[..n - 1] {
                inner.push((arms[i].0 as u64, arm(i)));
            }
            let last = arm(bx[n - 1]);
            Some(vec![S::Switch(p("con_tag", vec![sv.clone()]), inner, Some(last))])
        }
    };
    S::Switch(p("tag", vec![sv.clone()]), cases, default)
}

/// Constructor allocation: slice-free fast paths for arity 1 and 2.
pub(crate) fn mk_con(cid: u32, es: Vec<E>) -> E {
    let mut a = vec![u16_(cid as u64)];
    match es.len() {
        1 | 2 => {
            let f = if es.len() == 1 { "mk_con1" } else { "mk_con2" };
            a.extend(es);
            c(f, a)
        }
        _ => {
            a.push(E::Slice(es));
            c("mk_con", a)
        }
    }
}

/// The parameter list `v0..v<ar>`.
pub(crate) fn vparams(ar: usize) -> Vec<(String, Ty)> {
    (0..ar).map(|i| (vn(i as u32), Ty::U64)).collect()
}

/// The dive form of function `fid`: fuel per call / loop iteration; on
/// exhaustion with a known destination the pending call is spawned as a
/// redex (suspension), otherwise the dive unwinds and the caller falls back
/// to the rule form.
pub(crate) fn dive_fn<'m>(
    m: &CoreModule,
    fid: u32,
    body: &Core,
    bor: &'m [Vec<bool>],
    bset: &HashSet<u32>,
    sq: &'m mut SegQ,
    fwd: u16,
    unbox: &'m std::collections::HashMap<u32, u8>,
    tys: &'m Types,
    iret: &'m [bool],
    shared: &'m std::cell::RefCell<Shared>,
) -> Vec<FnDef> {
    let f = &m.fns[fid as usize];
    let ar = f.arity;
    let nret = ntup_of(fid);
    let trmc = if nret > 0 || bor[fid as usize].iter().any(|b| *b) || std::env::var_os("MITHRIL_NO_TRMC").is_some() { None } else { trmc_ctor(fid, body, m, unbox) };
    let hole_rule = trmc.map(|c| sq.add_hole(c));
    let lp = f.self_tail_rec || trmc.is_some();
    let argl: Vec<E> = (0..ar).map(|i| v(vn(i as u32))).collect();
    let is_fold = FOLDS.with(|f| f.borrow().get(fid as usize).copied().unwrap_or(false));
    // Fuel-out at entry / loop top: the pending call IS the continuation;
    // spawn it against a forwarding record whose parent the caller sets.
    // The spawned call owns its arguments: borrowed params take a reference.
    let fuel_check = {
        let mut cap: Vec<S> = (0..ar)
            .filter(|&i| bor[fid as usize][i])
            .map(|i| let_(vn(i as u32), Ty::U64, c("dup_val", vec![v(vn(i as u32))])))
            .collect();
        cap.push(let_("r", Ty::U32, c("alloc_rec", vec![u16_(fwd as u64), u32_(1), u32_(0), u32_(0), E::Const("NONE".into())])));
        cap.push(do_(c("spawn_call", vec![u16_(1 + fid as u64), E::Slice(argl.clone()), rec_addr("r")])));
        cap.push(ret(cast(v("r"), Ty::U64)));
        let mut out = Vec::new();
        if is_fold {
            out.extend(crate::fold::est_update(fid));
        }
        out.push(S::Fn(Box::new(FnDef { name: "cap".into(), ctx: true, params: vparams(ar), ret: Ty::U64, body: cap, inline: Inline::Never, cold: true })));
        let call = E::Call { f: "cap".into(), ctx: true, args: argl.clone() };
        let exit = match hole_rule {
            Some(rule) => {
                out.push(let_("hr", Ty::U64, call));
                c("hole_wrap", vec![v("hr"), v("th_head"), v("th_hole"), u16_(rule as u64)])
            }
            None => call,
        };
        out.push(ret(err(exit)));
        vec![do_(p("stack_guard", vec![])), burn_fuel(), S::If(bin(Bop::Lt, E::Deref("fuel".into()), i64_(0)), out, vec![])]
    };
    let mut rem = Cnt::new();
    cnt_dive(body, &mut rem);
    let ints = crate::ints_of(tys, fid as usize);
    let mut ex = Ex::new(true, fid, lp, rem, bset.clone(), bor, ints, Some(sq), unbox, iret, tys, shared);
    ex.trmc = trmc.zip(hole_rule);
    ex.nret = nret;
    // the fuel-out spawn takes references to borrowed params
    for i in 0..ar as u32 {
        if bor[fid as usize][i as usize] {
            ex.note_share(i);
        }
    }
    let mut bb = Vec::new();
    // Owned parameters that the body never reads die immediately (in a
    // loop form: on every iteration, after the budget check).
    for i in 0..ar as u32 {
        if !ex.bset.contains(&i) && ex.rem.get(&i).copied().unwrap_or(0) == 0 {
            bb.push(free(v(vn(i))));
        }
    }
    ex.dive_tail(body, &mut bb);
    // A call-free body does bounded work: no fuel check, and it inlines
    // into its (recursive) callers.
    let leafy = !has_call(body);
    let inl = if crate::inline_attr(body).is_empty() { crate::inline_attr_fn(m, fid) } else { crate::inline_attr(body) };
    let inline = inline_of(inl);
    let mut out = Vec::new();
    let mut params = vec![("fuel".to_string(), Ty::RefI64)];
    params.extend(vparams(ar));
    let (name, ret_ty) = if nret > 0 {
        // boxing entry for the runtime and callers outside the native shape
        let mut a = vec![v("fuel")];
        a.extend(argl.clone());
        out.push(FnDef {
            name: format!("d_{fid}"),
            ctx: true,
            params: params.clone(),
            ret: Ty::Res,
            body: vec![S::Res(
                E::Call { f: format!("n_{fid}"), ctx: true, args: a },
                "a".into(),
                vec![ret(ok(c("mk_con", vec![u16_(4095), E::Addr("a".into())])))],
                "r".into(),
                vec![ret(err(v("r")))],
            )],
            inline: Inline::Default,
            cold: false,
        });
        (format!("n_{fid}"), Ty::ResArr(nret))
    } else if is_fold {
        out.push(crate::fold::heavy_wrapper(fid, ar));
        (format!("dd_{fid}"), Ty::Res)
    } else {
        (format!("d_{fid}"), Ty::Res)
    };
    let mut s = Vec::new();
    if trmc.is_some() {
        s.push(let_("th_head", Ty::U64, u64_(0)));
        s.push(let_("th_hole", Ty::U32, E::Const("NOHOLE".into())));
    }
    if lp {
        let mut lb = if leafy { Vec::new() } else { fuel_check.clone() };
        if !leafy {
            lb[1] = work_fuel(i64_(1));
        }
        if is_fold {
            s.push(let_("fold_start", Ty::U64, v("v0")));
            lb.extend(FOLD_SPLIT.with(|f| f.borrow().get(fid as usize).cloned().flatten()).unwrap_or_default());
        }
        lb.extend(bb);
        s.push(S::Loop(lb));
    } else {
        if !leafy {
            s.extend(fuel_check);
        }
        s.extend(bb);
        s.push(S::Unreachable);
    }
    out.push(FnDef { name, ctx: true, params, ret: ret_ty, body: s, inline, cold: false });
    out
}

thread_local! {
    /// Destination-passing callees of the module being emitted:
    /// `DPS[g] = Some((p, ctor))` when `g` only appends `ctor` cells onto
    /// its parameter `p` (see `dps_param`).
    pub(crate) static DPS: std::cell::RefCell<Vec<Option<(usize, u32)>>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn dps_of(g: u32) -> Option<(usize, u32)> {
    DPS.with(|d| d.borrow().get(g as usize).copied().flatten())
}

/// Every tail path of `e` uses the pending self-call result `x` exactly
/// once, as (a) the last field of a two-field ctor or (b) the tail
/// parameter of a destination-passing callee; nothing else reads `x` and
/// nothing between the call and the tail can suspend. Collects the ctor
/// of each tail.
pub(crate) fn delayed_ok(x: u32, e: &Core, cids: &mut Vec<u32>) -> bool {
    match e {
        Core::Let(_, r, rest) => !has_call(r) && !free_vars(r).contains(&x) && delayed_ok(x, rest, cids),
        Core::If(c, t, f) => !free_vars(c).contains(&x) && delayed_ok(x, t, cids) && delayed_ok(x, f, cids),
        Core::Match(sc, arms) => !free_vars(sc).contains(&x) && arms.iter().all(|(_, _, a)| delayed_ok(x, a, cids)),
        Core::Ctor(c, a) | Core::Reuse(_, c, a) => {
            if a.len() == 2 && a[1] == Core::Var(x) && !free_vars(&a[0]).contains(&x) {
                cids.push(*c);
                true
            } else {
                false
            }
        }
        Core::Call(g, a) => match dps_of(*g) {
            Some((p, c)) if a[p] == Core::Var(x) && a.iter().enumerate().all(|(i, v)| i == p || !free_vars(v).contains(&x)) => {
                cids.push(c);
                true
            }
            _ => false,
        },
        _ => false,
    }
}

/// The ctor every delayed self call of `body` builds, if it has any and
/// they agree (one boxed two-field ctor per function).
pub(crate) fn trmc_ctor(fid: u32, body: &Core, m: &CoreModule, unbox: &std::collections::HashMap<u32, u8>) -> Option<u32> {
    fn go(fid: u32, e: &Core, out: &mut Vec<u32>) -> bool {
        match e {
            Core::Let(x, r, bo) => {
                if matches!(&**r, Core::Call(g, _) if *g == fid) {
                    let mut cs = Vec::new();
                    if delayed_ok(*x, bo, &mut cs) {
                        out.extend(cs);
                        return true;
                    }
                }
                go(fid, bo, out)
            }
            Core::If(_, t, f) => {
                let a = go(fid, t, out);
                go(fid, f, out) || a
            }
            Core::Match(_, arms) => arms.iter().fold(false, |acc, (_, _, b)| go(fid, b, out) || acc),
            _ => false,
        }
    }
    let mut cs = Vec::new();
    if !go(fid, body, &mut cs) {
        return None;
    }
    let c = *cs.first()?;
    if cs.iter().any(|x| *x != c) || unbox.contains_key(&c) || m.ctors[c as usize].1 != 2 {
        return None;
    }
    Some(c)
}

/// `Some((p, ctor))` when `f` is a destination-passing callee: every tail
/// is `Var(p)` or a delayed self call that passes `p` through unchanged,
/// `p` is read nowhere else, and `f` calls nothing that can suspend.
pub(crate) fn dps_param(fid: u32, f: &mut dyn FnMut(u32) -> bool, body: &Core, arity: usize, m: &CoreModule, unbox: &std::collections::HashMap<u32, u8>) -> Option<(usize, u32)> {
    fn calls_ok(fid: u32, e: &Core, f: &mut dyn FnMut(u32) -> bool) -> bool {
        !e.any(&mut |e| match e {
            Core::Call(g, _) if *g != fid && !f(*g) => Some(true),
            _ => None,
        })
    }
    // tails: Var(p), or `x = self(.., Var(p), ..)` delayed
    fn tails(fid: u32, p: u32, e: &Core, n: &mut usize) -> bool {
        match e {
            Core::Var(v) => {
                *n += 1;
                *v == p
            }
            Core::Let(_, r, bo) => {
                if let Core::Call(g, a) = &**r {
                    if *g == fid && a.get(p as usize) == Some(&Core::Var(p)) {
                        *n += 1;
                        return !a.iter().enumerate().any(|(i, v)| i != p as usize && free_vars(v).contains(&p))
                            && !free_vars(bo).contains(&p);
                    }
                }
                !free_vars(r).contains(&p) && tails(fid, p, bo, n)
            }
            Core::If(c, t, f) => !free_vars(c).contains(&p) && tails(fid, p, t, n) && tails(fid, p, f, n),
            Core::Match(s, arms) => !free_vars(s).contains(&p) && arms.iter().all(|(_, _, a)| tails(fid, p, a, n)),
            _ => false,
        }
    }
    if !calls_ok(fid, body, f) {
        return None;
    }
    let c = trmc_ctor(fid, body, m, unbox)?;
    (0..arity).find_map(|p| {
        let mut n = 0;
        if tails(fid, p as u32, body, &mut n) && n > 0 {
            let mut uses = crate::Cnt::new();
            crate::cnt_expr(body, &mut uses);
            // p is read once per tail (as the value or as the passed-through arg)
            let reads = uses.get(&(p as u32)).copied().unwrap_or(0) as usize;
            (reads == n).then_some((p, c))
        } else {
            None
        }
    })
}

/// The destination-passing form of `g` (see `dps_param`): appends its cells
/// into the caller's hole and returns with the hole open. Charges fuel but
/// never suspends (it calls nothing that can).
pub(crate) fn dps_fn<'m>(
    m: &CoreModule,
    fid: u32,
    pp: usize,
    cid: u32,
    body: &Core,
    bor: &'m [Vec<bool>],
    sq: &'m mut SegQ,
    unbox: &'m std::collections::HashMap<u32, u8>,
    tys: &'m Types,
    iret: &'m [bool],
    shared: &'m std::cell::RefCell<Shared>,
) -> FnDef {
    let ar = m.fns[fid as usize].arity;
    let mut params = vec![("fuel".to_string(), Ty::RefI64)];
    params.extend((0..ar).filter(|i| *i != pp).map(|i| (vn(i as u32), Ty::U64)));
    params.push(("head_out".into(), Ty::RefU64));
    params.push(("hole_out".into(), Ty::RefU32));
    let mut rem = Cnt::new();
    cnt_dive(body, &mut rem);
    rem.remove(&(pp as u32));
    let ints = crate::ints_of(tys, fid as usize);
    let mut ex = Ex::new(true, fid, true, rem, HashSet::new(), bor, ints, Some(sq), unbox, iret, tys, shared);
    ex.trmc = Some((cid, 0));
    ex.dps_param = Some(pp);
    let mut bb = vec![do_(p("stack_guard", vec![])), work_fuel(i64_(1))];
    for i in 0..ar as u32 {
        if i as usize != pp && ex.rem.get(&i).copied().unwrap_or(0) == 0 {
            bb.push(free(v(vn(i))));
        }
    }
    ex.dive_tail(body, &mut bb);
    let body = vec![
        let_("th_head", Ty::U64, E::Deref("head_out".into())),
        let_("th_hole", Ty::U32, E::Deref("hole_out".into())),
        S::Loop(bb),
    ];
    FnDef { name: format!("dp_{fid}"), ctx: true, params, ret: Ty::Unit, body, inline: Inline::Default, cold: false }
}

/// Split a let chain `e` around `seed`: `Some((I, l, D))` where I is the
/// bindings independent of `seed` ending in their one live-out `l` (the
/// single binder D reads), and D the dependent rest. `None` when I does no
/// call or has zero or several live-outs.
fn split_chain(seed: u32, e: &Core) -> Option<(Core, u32, Core)> {
    let mut binds: Vec<(u32, &Core)> = Vec::new();
    let mut cur = e;
    while let Core::Let(v, r, b) = cur {
        binds.push((*v, r));
        cur = b;
    }
    let mut dep: HashSet<u32> = HashSet::from([seed]);
    let (mut ind, mut de): (Vec<(u32, &Core)>, Vec<(u32, &Core)>) = (Vec::new(), Vec::new());
    for (v, r) in binds {
        if free_vars(r).iter().any(|f| dep.contains(f)) { dep.insert(v); de.push((v, r)); } else { ind.push((v, r)); }
    }
    if !ind.iter().any(|(_, r)| has_call(r)) { return None; }
    let wrap = |bs: &[(u32, &Core)], t: Core| bs.iter().rev().fold(t, |acc, (v, r)| Core::Let(*v, Box::new((*r).clone()), Box::new(acc)));
    let d_body = wrap(&de, cur.clone());
    let jf = free_vars(&d_body);
    let live: Vec<u32> = ind.iter().map(|(v, _)| *v).filter(|v| jf.contains(v)).collect();
    if live.len() != 1 { return None; }
    Some((wrap(&ind, Core::Var(live[0])), live[0], d_body))
}

/// A suspended frame's continuation `bo` around the pending `x`: (P, P's
/// live-out, J); frames that do not split wait as plain records.
pub(crate) fn split_frame(x: u32, bo: &Core) -> Option<(Core, u32, Core)> { split_chain(x, bo) }

/// A frame's dependent rest `j` around P's live-out `live`: D (needs x, not
/// `live`; one live-out `m`, a call) and J2 (reads `m`, `live`, not x).
pub(crate) fn split_dep(x: u32, live: u32, j: &Core) -> Option<(Core, u32, Core)> {
    split_chain(live, j).filter(|(_, _, j2)| !free_vars(j2).contains(&x))
}

/// Every occurrence of `x` in `e` is directly under a `Proj`.
pub(crate) fn only_projected(x: u32, e: &Core) -> bool {
    !e.any(&mut |e| match e {
        Core::Var(y) => Some(*y == x),
        Core::Proj(a, _) if matches!(&**a, Core::Var(_)) => Some(false),
        _ => None,
    })
}
