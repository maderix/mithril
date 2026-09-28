//! Dive-form emission (Core -> sequential Rust) and the expression emitter
//! shared with the rule form. Runtime values are Port raws (`u64`): NUM i56
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

use crate::rules::{emit_rec, SegQ};
use crate::ty::{Ty, Types};

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
    /// The forwarding segment id (delivers its single slot to its parent).
    pub fwd: u16,
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
    pub pending: Option<(u32, Vec<String>)>,
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
    pub(crate) static FOLD_SPLIT: std::cell::RefCell<Vec<Option<String>>> = const { std::cell::RefCell::new(Vec::new()) };
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

/// Variables that are *used* as arithmetic/comparison operands or bound to
/// arithmetic results. Sound as an i56 proof only when the module contains
/// no float literal (floats cannot arise otherwise), which the caller
/// checks; returns an empty set when the proof does not hold.
pub(crate) fn numeric_vars(body: &Core, float_free: bool) -> HashSet<u32> {
    fn is_numeric_expr(e: &Core) -> bool {
        matches!(e, Core::Num(_) | Core::Op2(..) | Core::Cmp(..))
    }
    fn walk(e: &Core, out: &mut HashSet<u32>) {
        match e {
            Core::Op2(_, a, b) | Core::Cmp(_, a, b) => {
                for x in [a, b] {
                    if let Core::Var(i) = &**x {
                        out.insert(*i);
                    }
                    walk(x, out);
                }
            }
            Core::If(c, t, f) => {
                if let Core::Var(i) = &**c {
                    out.insert(*i);
                }
                walk(c, out);
                walk(t, out);
                walk(f, out);
            }
            Core::Let(x, r, b) => {
                if is_numeric_expr(r) {
                    out.insert(*x);
                }
                walk(r, out);
                walk(b, out);
            }
            Core::Call(_, a) | Core::Ctor(_, a) | Core::Tuple(a) | Core::Reuse(_, _, a) | Core::Prim(_, a) => {
                for x in a {
                    walk(x, out);
                }
            }
            Core::Match(s, arms) => {
                walk(s, out);
                for (_, _, b) in arms {
                    walk(b, out);
                }
            }
            Core::Proj(b, _) => walk(b, out),
            Core::Num(_) | Core::Flo(_) | Core::Var(_) => {}
        }
    }
    let mut out = HashSet::new();
    if float_free {
        walk(body, &mut out);
    }
    out
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
        fwd: u16,
        unbox: &'m std::collections::HashMap<u32, u8>,
        iret: &'m [bool],
        tys: &'m Types,
        shared: &'m std::cell::RefCell<Shared>,
    ) -> Ex<'m> {
        Ex { tmp: 0, dive, self_fid, loop_form, rem, pinned: 0, pinset: HashSet::new(), bset, bor, ints, captured: HashSet::new(), inline_calls: 0, ntup: HashSet::new(), ntup_sh: HashSet::new(), cur_let: None, trmc: None, pending: None, dps_param: None, nret: 0, sq, kframes: Vec::new(), fwd, unbox, iret, tys, shared, toks: Vec::new() }
    }

    /// Release every pending reuse token (before a call, branch, return or
    /// loop back-edge: tokens never cross them, preserving free-early LIFO
    /// locality for whatever runs next).
    pub(crate) fn flush_toks(&mut self, b: &mut String) {
        for t in self.toks.drain(..) {
            b.push_str(&format!("tok_free(ctx, {t});\n"));
        }
    }

    /// Record that variable `i`'s value is shared by emitted code.
    fn note_share(&self, i: u32) {
        let t = if self.self_fid == u32::MAX {
            Ty::Dyn
        } else {
            self.tys.var(self.self_fid as usize, i)
        };
        if std::env::var_os("MITHRIL_DEBUG_SHARE").is_some() {
            eprintln!("share: fn {} var {} ty {:?} pinned {} rem {:?}", self.self_fid, i, t, self.pinned, self.rem.get(&i));
        }
        let mut sh = self.shared.borrow_mut();
        match t {
            Ty::Int => {}
            Ty::Flo => {} // boxed floats always carry an rc
            Ty::Adt(c) => {
                sh.classes.insert(c);
            }
            Ty::Tup(_) => sh.tuples = true,
            Ty::Arr(_) => {} // arrays always carry a refcount
            Ty::Dyn => sh.poison = true,
        }
    }

    /// Read `e` without taking ownership (an array operand of a read): a
    /// variable is lent raw, and released after the read at its last use;
    /// any other expression is evaluated to a temporary released after.
    fn borrow_read(&mut self, e: &Core, b: &mut String) -> (String, Option<String>) {
        if let Core::Var(i) = e {
            if self.bset.contains(i) || (self.pinned > 0 && self.pinset.contains(i)) {
                return (format!("v{i}"), None);
            }
            match self.rem.get_mut(i) {
                Some(r) if *r >= 2 => {
                    *r -= 1;
                    return (format!("v{i}"), None);
                }
                Some(r) if *r == 1 => {
                    *r = 0;
                    return (format!("v{i}"), Some(format!("v{i}")));
                }
                _ => return (format!("v{i}"), None),
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
    pub(crate) fn emit_capture(&mut self, rv: &str, own: Option<(u32, &Core)>, b: &mut String) {
        let saved = self.rem.clone();
        let mut cb = String::new();
        let child = self.capture_chain(rv, own, &mut cb);
        // Every record took one reference (sharing when the frame still
        // had other uses); the frame itself is now abandoned, so release
        // what it still owns. Borrowed and held (pinned) values are not
        // ours to drop.
        if self.pinned == 0 {
            let mut left: Vec<u32> = self.rem.iter().filter(|(_, r)| **r > 0).map(|(v, _)| *v).collect();
            left.sort_unstable();
            for v in left {
                if !self.ints.contains(&v) && !self.bset.contains(&v) && self.captured.contains(&v) {
                    cb.push_str(&format!("free_val(ctx, v{v});\n"));
                }
            }
        }
        cb.push_str(&format!("{child} as u64\n"));
        self.rem = saved;
        // The chain runs once per suspension; keeping it out of line keeps
        // its record/spawn temporaries (and their stack slots) out of the
        // hot function so the entry fuel test can shrink-wrap.
        let name = format!("cap_{}", self.fresh());
        let vars = outer_vars(&cb);
        let params: String = vars.iter().map(|v| format!(", {v}: u64")).collect();
        let call = format!("{name}(ctx, {rv} as u32{})", vars.iter().map(|v| format!(", {v}")).collect::<String>());
        b.push_str(&format!(
            "#[cold] #[inline(never)] fn {name}(ctx: &mut Wctx, {rv}: u32{params}) -> u64 {{\n{cb}}}\nreturn Err({});\n",
            self.hole_exit(&call)
        ));
    }

    /// Emit the record chain for a suspension into `b`, returning the name
    /// of the outermost record (see `emit_capture`).
    fn capture_chain(&mut self, rv: &str, own: Option<(u32, &Core)>, b: &mut String) -> String {
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
                let mut env_j = free_vars(&j_body);
                env_j.remove(x);
                env_j.remove(&live);
                let env_j: Vec<u32> = env_j.into_iter().collect();
                let sid_j = self.sq.as_mut().expect("dive capture without a segment registry").add(self.self_fid, vec![*x, live], env_j.clone(), j_body.clone());
                let env_p: Vec<u32> = free_vars(&p_body).into_iter().collect();
                let sid_p = self.sq.as_mut().expect("dive capture without a segment registry").add(self.self_fid, vec![], env_p.clone(), p_body.clone());
                let rn = emit_rec(self, &env_j, sid_j, 2, "NONE", &j_body, b);
                b.push_str(&format!("ctx.set_parent({child}, ({rn} as u64) << 3);\n"));
                let rp = emit_rec(self, &env_p, sid_p, 0, &format!("((({rn} as u64) << 3) | 1)"), &p_body, b);
                b.push_str(&format!("ctx.ready_rec({rp});\n"));
                child = rn;
            } else {
                let mut env = free_vars(bo);
                env.remove(x);
                let env: Vec<u32> = env.into_iter().collect();
                let sid = self.sq.as_mut().expect("dive capture without a segment registry").add(self.self_fid, vec![*x], env.clone(), bo.clone());
                let rn = emit_rec(self, &env, sid, 1, "NONE", bo, b);
                b.push_str(&format!("ctx.set_parent({child}, ({rn} as u64) << 3);\n"));
                child = rn;
            }
        }
        child
    }

    /// A let whose RHS is a call: run the dive, capturing on suspension.
    fn let_call(&mut self, x: u32, g: u32, args: &[Core], bo: &Core, b: &mut String) {
        let (call, post) = self.dive_call(g, args, b);
        let t = self.fresh();
        b.push_str(&format!(
            "let {t} = match {call} {{\nOk(v) => v,\nErr(r) => {{\n"
        ));
        for p in &post {
            b.push_str(&format!("free_val(ctx, {p});\n"));
        }
        self.emit_capture("r", Some((x, bo)), b);
        b.push_str("}\n};\n");
        for p in post {
            b.push_str(&format!("free_val(ctx, {p});\n"));
        }
        self.emit_bind(x, &t, b);
    }

    /// `e` is an array whose elements are proven ints.
    fn int_arr(&self, e: &Core) -> bool {
        self.self_fid != u32::MAX && self.tys.expr(self.self_fid as usize, e) == Ty::Arr(true)
    }

    /// An expression whose runtime value is a proven i56 immediate.
    fn is_int(&self, e: &Core) -> bool {
        match e {
            Core::Prim(mithril_front::core::Prim::ArrGet, xs) => self.int_arr(&xs[0]),
            Core::Prim(mithril_front::core::Prim::ArrLen, _) => true,
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
    pub fn use_var(&mut self, i: u32, esc: bool, b: &mut String) -> String {
        if self.ints.contains(&i) {
            // immediates: no ownership, no copy
            if let Some(r) = self.rem.get_mut(&i) {
                *r = (*r - 1).max(0);
            }
            return format!("v{i}");
        }
        if self.dive && self.bset.contains(&i) {
            if esc {
                self.note_share(i);
                let t = self.fresh();
                b.push_str(&format!("let {t} = dup_val(ctx, v{i});\n"));
                return t;
            }
            return format!("v{i}");
        }
        if self.pinned > 0 && self.pinset.contains(&i) {
            self.note_share(i);
            let t = self.fresh();
            b.push_str(&format!("let {t} = dup_val(ctx, v{i});\n"));
            return t;
        }
        let r0 = self.rem.get(&i).copied();
        if r0.map_or(true, |r| r >= 2) {
            self.note_share(i);
        }
        match self.rem.get_mut(&i) {
            Some(r) if *r >= 2 => {
                *r -= 1;
                let t = self.fresh();
                b.push_str(&format!("let {t} = dup_val(ctx, v{i});\n"));
                t
            }
            Some(r) if *r == 1 => {
                *r = 0;
                format!("v{i}")
            }
            _ => {
                let t = self.fresh();
                b.push_str(&format!("let {t} = dup_val(ctx, v{i});\n"));
                t
            }
        }
    }

    /// Entering one branch of a tail-position branch point: `live` are the
    /// variables in scope that the branch point uses, `local` this branch's
    /// uses. Use counts at a branch point cover all branches, so each is
    /// rebased to this branch, and an owned boxed value this branch never
    /// uses is released here (else it leaks on this path, or its one use
    /// makes a needless copy whose extra reference is never dropped).
    pub(crate) fn enter_branch(&mut self, live: &std::collections::BTreeSet<u32>, local: &Cnt, b: &mut String) {
        for v in live {
            let l = local.get(v).copied().unwrap_or(0);
            let r = self.rem.get(v).copied().unwrap_or(0);
            let owned = self.pinned == 0
                && !self.ints.contains(v)
                && !self.bset.contains(v)
                && !self.ntup.contains(v)
                && self.pending.as_ref().map_or(true, |(x, _)| x != v);
            if l == 0 && r > 0 && owned {
                b.push_str(&format!("free_val(ctx, v{v});\n"));
            }
            if r > 0 {
                self.rem.insert(*v, l);
            }
        }
    }

    /// Bind `let v{x} = expr;` and free it right away if it is never used.
    fn emit_bind(&mut self, x: u32, expr: &str, b: &mut String) {
        if self.ints.contains(&x) {
            b.push_str(&format!("let v{x} = {expr};\n"));
            return;
        }
        if !self.bset.contains(&x) && self.rem.get(&x).copied().unwrap_or(0) == 0 {
            b.push_str(&format!("free_val(ctx, {expr});\n"));
        } else {
            b.push_str(&format!("let v{x} = {expr};\n"));
        }
    }

    /// Evaluate a match/proj scrutinee and decide its hold mode.
    pub(crate) fn scrutinee(&mut self, s: &Core, b: &mut String) -> (String, Hold) {
        if let Core::Var(i) = s {
            if self.dive && self.bset.contains(i) {
                return (format!("v{i}"), Hold::BorrowRaw);
            }
            if self.pinned > 0 && self.pinset.contains(i) {
                self.note_share(*i);
                return (format!("v{i}"), Hold::BorrowDup);
            }
            if self.rem.get(i).copied().map_or(true, |r| r >= 2) {
                self.note_share(*i);
            }
            return match self.rem.get_mut(i) {
                Some(r) if *r >= 2 => {
                    *r -= 1;
                    (format!("v{i}"), Hold::BorrowDup)
                }
                Some(r) if *r == 1 => {
                    *r = 0;
                    (format!("v{i}"), Hold::Consume)
                }
                _ => (format!("v{i}"), Hold::BorrowDup),
            };
        }
        let e = self.val(s, false, b);
        (e, Hold::Consume)
    }

    /// Bind a match arm's fields under the scrutinee's hold mode; Consume
    /// also frees the constructor spine.
    pub(crate) fn bind_fields(
        &mut self,
        sv: &str,
        hold: Hold,
        cid: u32,
        binders: &[u32],
        tok: Option<u32>,
        b: &mut String,
    ) {
        // Consume with a reuse token (the rewrite placed a `Reuse` of this
        // scrutinee var on some path of the arm)
        if self.dive && hold == Hold::Consume && binders.len() == 2 && tok.is_some() {
            let names: Vec<String> = binders.iter().map(|bv| format!("v{bv}")).collect();
            let tk = format!("tok_v{}", tok.unwrap());
            b.push_str(&format!(
                "let ({}, {}, {tk}) = consume2r(ctx, {sv}, {cid}u16);\n",
                names[0], names[1]
            ));
            self.toks.push(tk);
            for bv in binders {
                if self.rem.get(bv).copied().unwrap_or(0) == 0 {
                    b.push_str(&format!("free_val(ctx, v{bv});\n"));
                }
            }
            return;
        }
        if hold == Hold::Consume && !binders.is_empty() && binders.len() <= 2 {
            let names: Vec<String> = binders.iter().map(|bv| format!("v{bv}")).collect();
            match binders.len() {
                1 => b.push_str(&format!(
                    "let ({}, m_unused) = consume2k(ctx, {sv}, {cid}u16);\nfree_val(ctx, m_unused);\n",
                    names[0]
                )),
                _ => b.push_str(&format!("let ({}, {}) = consume2k(ctx, {sv}, {cid}u16);\n", names[0], names[1])),
            }
            for bv in binders {
                if self.rem.get(bv).copied().unwrap_or(0) == 0 {
                    b.push_str(&format!("free_val(ctx, v{bv});\n"));
                }
            }
            return;
        }
        if hold == Hold::Consume {
            // chained arity: move every field out, dropping the unused ones
            let names: Vec<String> = binders.iter().map(|bv| format!("v{bv}")).collect();
            b.push_str(&format!(
                "let [{}] = consume_chain::<{}>(ctx, {sv}, {cid}u16);\n",
                names.join(", "),
                binders.len()
            ));
            for bv in binders {
                if self.rem.get(bv).copied().unwrap_or(0) == 0 {
                    b.push_str(&format!("free_val(ctx, v{bv});\n"));
                }
            }
            return;
        }
        for (i, bv) in binders.iter().enumerate() {
            let cnt = self.rem.get(bv).copied().unwrap_or(0);
            match hold {
                Hold::BorrowRaw => {
                    b.push_str(&format!("let v{bv} = field(ctx, {sv}, {i});\n"));
                }
                Hold::Consume => unreachable!(),
                Hold::BorrowDup => {
                    if cnt > 0 {
                        b.push_str(&format!(
                            "let v{bv} = dup_val(ctx, field(ctx, {sv}, {i}));\n"
                        ));
                    }
                }
            }
        }
        if hold == Hold::Consume {
            b.push_str(&format!("free_val(ctx, {sv});\n"));
        }
    }

    /// A dive call `d_g(...)`; returns (call expression, frees to run after
    /// the call succeeds: owned values lent to borrowed parameters).
    fn dive_call(
        &mut self,
        g: u32,
        args: &[Core],
        b: &mut String,
    ) -> (String, Vec<String>) {
        self.dive_call_as(g, args, false, b)
    }

    /// `native`: call the multi-value entry `n_<g>` (see `NTUP`).
    fn dive_call_as(
        &mut self,
        g: u32,
        args: &[Core],
        native: bool,
        b: &mut String,
    ) -> (String, Vec<String>) {
        // rule-form segments have no fuel; only bounded callees (which never
        // consume any) can be called from them as plain expressions
        assert!(self.dive || crate::is_bounded(g), "codegen bug: call in a pure rule-form expression");
        let fuel = if self.dive { "fuel" } else { "&mut 0i64" };
        // args first (they may take tokens), then release the rest (below)
        let modes = &self.bor[g as usize];
        let mut post = Vec::new();
        let mut es = Vec::new();
        for (j, a) in args.iter().enumerate() {
            if modes[j] {
                // borrowed parameter: lend a read
                match a {
                    Core::Var(i) if self.bset.contains(i) => es.push(format!("v{i}")),
                    Core::Var(i) => {
                        if self.pinned > 0 && self.pinset.contains(i) {
                            es.push(format!("v{i}"));
                        } else {
                            match self.rem.get_mut(i) {
                                Some(r) if *r >= 2 => {
                                    *r -= 1;
                                    es.push(format!("v{i}"));
                                }
                                Some(r) if *r == 1 => {
                                    // last use: we still own it after the call
                                    *r = 0;
                                    es.push(format!("v{i}"));
                                    post.push(format!("v{i}"));
                                }
                                _ => es.push(format!("v{i}")),
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
                es.push(self.val(a, true, b));
            }
        }
        self.flush_toks(b);
        let argl: String = es.iter().map(|e| format!(", {e}")).collect();
        let entry = if native {
            "n"
        } else if self.dive && crate::fast::has_fast(g) {
            "q"
        } else {
            "d"
        };
        (format!("{entry}_{g}(ctx, {fuel}{argl})"), post)
    }

    /// Emit statements computing `e` into `b`; returns a Rust expression
    /// (temp name, local, or literal) holding the value.
    pub fn val(&mut self, e: &Core, esc: bool, b: &mut String) -> String {
        match e {
            Core::Prim(p, args) => {
                use mithril_front::core::Prim;
                let t = self.fresh();
                match p {
                    Prim::ArrNew if self.is_int(&args[1]) => {
                        let n = self.val(&args[0], false, b);
                        let v = self.val(&args[1], true, b);
                        b.push_str(&format!("let {t} = arr_new_i(as_i({n}), {v});\n"));
                    }
                    Prim::ArrGet if self.int_arr(&args[0]) => {
                        let (a, post) = self.borrow_read(&args[0], b);
                        let i = self.val(&args[1], false, b);
                        b.push_str(&format!("let {t} = arr_get_i({a}, as_i({i}));\n"));
                        if let Some(p) = post {
                            b.push_str(&format!("free_val(ctx, {p});\n"));
                        }
                    }
                    Prim::ArrSet if self.int_arr(&args[0]) => {
                        let a = self.val(&args[0], true, b);
                        let i = self.val(&args[1], false, b);
                        let v = self.val(&args[2], true, b);
                        b.push_str(&format!("let {t} = arr_set_i(ctx, {a}, as_i({i}), {v});\n"));
                    }
                    Prim::ArrNew => {
                        let n = self.val(&args[0], false, b);
                        // n copies of the element: its type is shared
                        if let Core::Var(v) = &args[1] {
                            self.note_share(*v);
                        } else if !self.is_int(&args[1]) {
                            self.shared.borrow_mut().poison = true;
                        }
                        let v = self.val(&args[1], true, b);
                        b.push_str(&format!("let {t} = arr_new(ctx, as_i({n}), {v});\n"));
                    }
                    Prim::ArrGet => {
                        let (a, post) = self.borrow_read(&args[0], b);
                        let i = self.val(&args[1], false, b);
                        // the element stays in the array too: shared
                        match self.cur_let {
                            Some(v) => self.note_share(v),
                            None => self.shared.borrow_mut().poison = true,
                        }
                        b.push_str(&format!("let {t} = arr_get(ctx, {a}, as_i({i}));\n"));
                        if let Some(p) = post {
                            b.push_str(&format!("free_val(ctx, {p});\n"));
                        }
                    }
                    Prim::ArrSet => {
                        let a = self.val(&args[0], true, b);
                        let i = self.val(&args[1], false, b);
                        let v = self.val(&args[2], true, b);
                        b.push_str(&format!("let {t} = arr_set(ctx, {a}, as_i({i}), {v});\n"));
                    }
                    Prim::ArrLen => {
                        let (a, post) = self.borrow_read(&args[0], b);
                        b.push_str(&format!("let {t} = num(arr_len_of({a}) as i64);\n"));
                        if let Some(p) = post {
                            b.push_str(&format!("free_val(ctx, {p});\n"));
                        }
                    }
                }
                t
            }
            Core::Num(n) => format!("num({}i64)", n),
            Core::Flo(x) => {
                let t = self.fresh();
                b.push_str(&format!(
                    "let {t} = flo(ctx, f64::from_bits(0x{:016x}u64));\n",
                    x.to_bits()
                ));
                t
            }
            Core::Var(i) => self.use_var(*i, esc, b),
            Core::Op2(op, x, y) => {
                let ints = self.is_int(x) && self.is_int(y);
                let own = (!self.is_braw(x) as u8) | ((!self.is_braw(y) as u8) << 1);
                let ex = self.val(x, false, b);
                let ey = self.val(y, false, b);
                let t = self.fresh();
                if ints {
                    let (a, c) = (format!("as_i({ex})"), format!("as_i({ey})"));
                    let body = match bin_code(op) {
                        0 => format!("{a}.wrapping_add({c})"),
                        1 => format!("{a}.wrapping_sub({c})"),
                        2 => format!("{a}.wrapping_mul({c})"),
                        3 => format!("{a}.wrapping_div({c})"),
                        4 => format!("floor_div({a}, {c})"),
                        5 => format!("py_mod({a}, {c})"),
                        6 => format!("{a}.wrapping_shl({c} as u32)"),
                        7 => format!("{a}.wrapping_shr({c} as u32)"),
                        8 => format!("{a} & {c}"),
                        9 => format!("{a} | {c}"),
                        _ => format!("{a} ^ {c}"),
                    };
                    // num keeps the low 56 bits and as_i sign-extends from
                    // bit 55: storing is the i56 wrap
                    b.push_str(&format!("let {t} = num({body});\n"));
                } else {
                    b.push_str(&format!(
                        "let {t} = bin(ctx, {}u8, {ex}, {ey}, {own}u8);\n",
                        bin_code(op)
                    ));
                }
                t
            }
            Core::Cmp(op, x, y) => {
                let ints = self.is_int(x) && self.is_int(y);
                let own = (!self.is_braw(x) as u8) | ((!self.is_braw(y) as u8) << 1);
                let ex = self.val(x, false, b);
                let ey = self.val(y, false, b);
                let t = self.fresh();
                if ints {
                    let o = match cmp_code(op) {
                        0 => "<",
                        1 => "<=",
                        2 => ">",
                        3 => ">=",
                        4 => "==",
                        _ => "!=",
                    };
                    b.push_str(&format!("let {t} = num((as_i({ex}) {o} as_i({ey})) as i64);\n"));
                } else {
                    b.push_str(&format!(
                        "let {t} = cmp(ctx, {}u8, {ex}, {ey}, {own}u8);\n",
                        cmp_code(op)
                    ));
                }
                t
            }
            Core::If(c, th, el) => {
                let ec = self.val(c, false, b);
                self.flush_toks(b);
                let outer = self.pin_enter(&[th, el], &[]);
                let bt = self.block_val(th, esc);
                let bf = self.block_val(el, esc);
                let t = self.fresh();
                b.push_str(&format!(
                    "let {t} = if as_i({ec}) != 0 {{\n{bt}}} else {{\n{bf}}};\n"
                ));
                self.pin_leave(&[th, el], outer, b);
                t
            }
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
                    if has_call(r) {
                        self.kframes.push((*x, (**bo).clone()));
                        self.cur_let = Some(*x);
                let er = self.val(r, false, b);
                self.cur_let = None;
                        self.kframes.pop();
                        self.emit_bind(*x, &er, b);
                        return self.val(bo, esc, b);
                    }
                }
                self.cur_let = Some(*x);
                let er = self.val(r, false, b);
                self.cur_let = None;
                self.emit_bind(*x, &er, b);
                self.val(bo, esc, b)
            }
            Core::Call(g, args) if crate::scalar::native_sig(*g).is_some_and(|s| s.ret == crate::scalar::Kind::S1) => {
                let es: Vec<String> = args.iter().map(|a| self.val(a, false, b)).collect();
                let fuel = if self.dive { "fuel" } else { "&mut 0i64" };
                let t = self.fresh();
                b.push_str(&format!(
                    "{}let {t} = {}(s_{g}({}{fuel}{}));\n",
                    if self.dive && crate::scalar::is_leaf(*g) { "*fuel -= 1;\n" } else { "" },
                    if crate::scalar::shifted(*g) { "retag" } else { "num" },
                    crate::scalar::ctx_arg(*g),
                    es.iter().map(|e| format!(", {}({e})", if crate::scalar::shifted(*g) { "sh" } else { "as_i" })).collect::<String>()
                ));
                t
            }
            Core::Call(g, args) => {
                let (call, post) = self.dive_call(*g, args, b);
                let t = self.fresh();
                if crate::is_bounded(*g) {
                    // bounded callee: cannot suspend
                    b.push_str(&format!("let {t} = match {call} {{ Ok(v) => v, Err(_) => unreachable!() }};\n"));
                } else {
                    // Only reachable for a suspendable call in bare value
                    // position, which ANF forbids; the tail/let paths own
                    // every real call site.
                    b.push_str(&format!("let {t} = match {call} {{\nOk(v) => v,\nErr(r) => {{\n"));
                    for p in &post {
                        b.push_str(&format!("free_val(ctx, {p});\n"));
                    }
                    self.emit_capture("r", None, b);
                    b.push_str("}\n};\n");
                }
                for p in post {
                    b.push_str(&format!("free_val(ctx, {p});\n"));
                }
                t
            }
            Core::Ctor(cid, args) => {
                if *cid == UNREACHABLE_CTOR {
                    let t = self.fresh();
                    b.push_str(&format!("let {t} = mith_unreachable();\n"));
                    return t;
                }
                assert!(*cid < 0xFFE, "codegen: ctor id {} collides with reserved tags", cid);
                if let Some(slot) = self.unbox.get(cid) {
                    let e0 = self.val(&args[0], false, b);
                    let t = self.fresh();
                    b.push_str(&format!("let {t} = ic({slot}u64, as_i({e0}));\n"));
                    return t;
                }
                let es: Vec<String> = args.iter().map(|a| self.val(a, true, b)).collect();
                let t = self.fresh();
                b.push_str(&mk_con_call(&t, *cid as u32, &es));
                t
            }
            Core::Reuse(v, cid, args) => {
                // decided by the reuse rewrite: build in v's consumed cell
                let es: Vec<String> = args.iter().map(|a| self.val(a, true, b)).collect();
                let t = self.fresh();
                let tk = format!("tok_v{v}");
                if self.dive && self.toks.iter().any(|x| *x == tk) {
                    self.toks.retain(|x| *x != tk);
                    b.push_str(&format!("let {t} = mk_con2r(ctx, {tk}, {}u16, {}, {});\n", *cid, es[0], es[1]));
                } else {
                    b.push_str(&mk_con_call(&t, *cid as u32, &es));
                }
                t
            }
            Core::Tuple(items) => {
                let es: Vec<String> = items.iter().map(|a| self.val(a, true, b)).collect();
                let t = self.fresh();
                b.push_str(&mk_con_call(&t, 0xFFF, &es));
                t
            }
            Core::Proj(x, i) if matches!(&**x, Core::Var(v) if self.ntup.contains(v)) => {
                let Core::Var(v) = &**x else { unreachable!() };
                if let Some(r) = self.rem.get_mut(v) {
                    *r = (*r - 1).max(0);
                }
                if self.ntup_sh.contains(v) { format!("retag(q{v}_{i})") } else { format!("num(q{v}_{i})") }
            }
            Core::Proj(x, i) => {
                let (sv, hold) = self.scrutinee(x, b);
                let t = self.fresh();
                if hold == Hold::Consume {
                    // the container dies here: move the field out
                    b.push_str(&format!("let {t} = take_field(ctx, {sv}, {i});\n"));
                } else {
                    // the container stays: the field is now shared, so its
                    // type must carry a refcount
                    match self.cur_let {
                        Some(v) => self.note_share(v),
                        None => self.shared.borrow_mut().poison = true,
                    }
                    b.push_str(&format!("let {t} = dup_val(ctx, field(ctx, {sv}, {i}));\n"));
                }
                t
            }
            Core::Match(s, arms) => {
                let (sv, hold) = self.scrutinee(s, b);
                self.flush_toks(b);
                let t = self.fresh();
                let (open, plan, close) = plan_arms(&sv, arms, self.unbox);
                let mut code = format!("let {t} = {open}");
                let arm_refs: Vec<&Core> = arms.iter().map(|(_, _, b)| b).collect();
                let arm_bs: Vec<u32> = arms.iter().flat_map(|(_, bs, _)| bs.iter().copied()).collect();
                let outer = self.pin_enter(&arm_refs, &arm_bs);
                for (i, pre, suf) in plan {
                    let (cid, binders, body) = &arms[i];
                    let mut ab = String::new();
                    if self.unbox.contains_key(cid) {
                        if let Some(bv) = binders.first() {
                            if self.rem.get(bv).copied().unwrap_or(0) > 0 || self.pinned > 0 {
                                ab.push_str(&format!("let v{bv} = num(as_i({sv}));\n"));
                            }
                        }
                    } else {
                        let tok = reuse_var(body, &sv);
                        self.bind_fields(&sv, hold, *cid, binders, tok, &mut ab);
                    }
                    let bb = self.block_val(body, esc);
                    code.push_str(&format!("{pre}{ab}{bb}{suf}"));
                }
                code.push_str(&format!("{close};\n"));
                b.push_str(&code);
                self.pin_leave(&arm_refs, outer, b);
                t
            }
        }
    }

    /// Enter a value-position branch point over `arms`: pin the outer
    /// variables they use (arms take their own references to those).
    fn pin_enter(&mut self, arms: &[&Core], arm_binders: &[u32]) -> Vec<u32> {
        fn binders(e: &Core, out: &mut HashSet<u32>) {
            match e {
                Core::Let(x, r, b) => {
                    out.insert(*x);
                    binders(r, out);
                    binders(b, out);
                }
                Core::Match(sc, arms) => {
                    binders(sc, out);
                    for (_, bs, b) in arms {
                        out.extend(bs.iter().copied());
                        binders(b, out);
                    }
                }
                Core::Op2(_, a, b) | Core::Cmp(_, a, b) => {
                    binders(a, out);
                    binders(b, out);
                }
                Core::If(a, b, c) => {
                    binders(a, out);
                    binders(b, out);
                    binders(c, out);
                }
                Core::Call(_, xs) | Core::Ctor(_, xs) | Core::Tuple(xs) | Core::Reuse(_, _, xs) | Core::Prim(_, xs) => {
                    xs.iter().for_each(|x| binders(x, out))
                }
                Core::Proj(b, _) => binders(b, out),
                Core::Num(_) | Core::Flo(_) | Core::Var(_) => {}
            }
        }
        let mut inner: HashSet<u32> = arm_binders.iter().copied().collect();
        for a in arms {
            binders(a, &mut inner);
        }
        let mut outer: Vec<u32> = Vec::new();
        for a in arms {
            for v in free_vars(a) {
                if !inner.contains(&v) && !self.pinset.contains(&v) && !outer.contains(&v) {
                    outer.push(v);
                }
            }
        }
        for v in &outer {
            self.pinset.insert(*v);
        }
        self.pinned += 1;
        outer
    }

    /// Leave it: the arms' uses of the pinned outer variables are retired
    /// (use counts include every arm), and an owned variable whose last
    /// use was in the arms is released once, here.
    fn pin_leave(&mut self, arms: &[&Core], outer: Vec<u32>, b: &mut String) {
        self.pinned -= 1;
        let mut used = Cnt::new();
        for a in arms {
            crate::cnt_expr(a, &mut used);
        }
        for v in outer {
            self.pinset.remove(&v);
            let k = used.get(&v).copied().unwrap_or(0);
            if k == 0 {
                continue;
            }
            if let Some(r) = self.rem.get_mut(&v) {
                let before = *r;
                *r = (*r - k).max(0);
                if before > 0
                    && *r == 0
                    && self.pinned == 0
                    && !self.ints.contains(&v)
                    && !self.bset.contains(&v)
                    && !self.ntup.contains(&v)
                {
                    b.push_str(&format!("free_val(ctx, v{v});\n"));
                }
            }
        }
    }

    /// `e` as a block body: statements plus a trailing value expression.
    pub fn block_val(&mut self, e: &Core, esc: bool) -> String {
        let outer = std::mem::take(&mut self.toks);
        let mut s = String::new();
        let v = self.val(e, esc, &mut s);
        self.flush_toks(&mut s);
        self.toks = outer;
        s.push_str(&v);
        s.push('\n');
        s
    }

    /// Emit `e` in dive tail position: ends every path with `return`, or
    /// `continue 'l` for self tail calls in loop form.
    /// An `Err` payload leaving the function: through the hole record in a
    /// TRMC loop.
    fn hole_exit(&self, r: &str) -> String {
        match self.trmc {
            Some((_, rule)) => format!("{{ let r = {r}; hole_wrap(ctx, r, th_head, th_hole, {rule}u16) }}"),
            None => r.to_string(),
        }
    }

    /// An `Ok` value leaving the function: into the hole in a TRMC loop.
    fn hole_value(&self, v: &str) -> String {
        match self.trmc {
            Some(_) => format!("hole_fill(ctx, th_head, th_hole, {v})"),
            None => v.to_string(),
        }
    }

    pub fn dive_tail(&mut self, e: &Core, b: &mut String) {
        match e {
            Core::Let(x, r, bo) => {
                if let (Some((cid, _)), Core::Call(g, args), None) = (self.trmc, &**r, &self.pending) {
                    let mut cs = Vec::new();
                    if *g == self.self_fid && delayed_ok(*x, bo, &mut cs) && cs.iter().all(|c| *c == cid) {
                        // delay the self call: arguments evaluated here,
                        // the call itself becomes the next loop iteration
                        let qs: Vec<String> = args
                            .iter()
                            .enumerate()
                            .map(|(i, a)| {
                                if Some(i) == self.dps_param {
                                    return String::new();
                                }
                                let e = self.val(a, true, b);
                                let q = self.fresh();
                                b.push_str(&format!("let {q} = {e};\n"));
                                q
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
                if has_call(r) {
                    self.kframes.push((*x, (**bo).clone()));
                    self.cur_let = Some(*x);
                let er = self.val(r, false, b);
                self.cur_let = None;
                    self.kframes.pop();
                    self.emit_bind(*x, &er, b);
                    return self.dive_tail(bo, b);
                }
                self.cur_let = Some(*x);
                let er = self.val(r, false, b);
                self.cur_let = None;
                self.emit_bind(*x, &er, b);
                self.dive_tail(bo, b);
            }
            Core::If(c, th, el) => {
                let ec = self.val(c, false, b);
                // tail branches inherit pending tokens; every arm ends in a
                // terminal (call/return/back-edge) that releases its unused ones
                let toks = self.toks.clone();
                let saved = self.rem.clone();
                let live = free_vars(e);
                let (mut lt, mut lf) = (Cnt::new(), Cnt::new());
                cnt_dive(th, &mut lt);
                cnt_dive(el, &mut lf);
                b.push_str(&format!("if as_i({ec}) != 0 {{\n"));
                self.enter_branch(&live, &lt, b);
                self.dive_tail(th, b);
                self.rem = saved.clone();
                self.toks = toks.clone();
                b.push_str("} else {\n");
                self.enter_branch(&live, &lf, b);
                self.dive_tail(el, b);
                self.rem = saved;
                self.toks.clear();
                b.push_str("}\n");
            }
            Core::Match(s, arms) => {
                let (sv, hold) = self.scrutinee(s, b);
                let toks = self.toks.clone();
                let saved = self.rem.clone();
                let mut live = free_vars(e);
                if let Core::Var(v) = &**s {
                    live.remove(v); // consumed by the match itself
                }
                let (open, plan, close) = plan_arms(&sv, arms, self.unbox);
                b.push_str(&open);
                for (i, pre, suf) in plan {
                    let (cid, binders, body) = &arms[i];
                    self.rem = saved.clone();
                    self.toks = toks.clone();
                    b.push_str(&pre);
                    let mut local = Cnt::new();
                    cnt_dive(body, &mut local);
                    self.enter_branch(&live, &local, b);
                    if self.unbox.contains_key(cid) {
                        if let Some(bv) = binders.first() {
                            if self.rem.get(bv).copied().unwrap_or(0) > 0 || self.pinned > 0 {
                                b.push_str(&format!("let v{bv} = num(as_i({sv}));\n"));
                            }
                        }
                    } else {
                        let tok = reuse_var(body, &sv);
                        self.bind_fields(&sv, hold, *cid, binders, tok, b);
                    }
                    self.dive_tail(body, b);
                    b.push_str(&suf);
                }
                self.rem = saved;
                self.toks.clear();
                b.push_str(&format!("{close}\n"));
            }
            Core::Call(g, args) if *g == self.self_fid && self.loop_form => {
                // Self tail call as a loop iteration: compute all next-state
                // values first (they read the current v*), then assign.
                // A borrow-derived value passed through a borrowed self
                // parameter stays raw (same frame, owner unchanged).
                let modes = self.bor[*g as usize].clone();
                let es: Vec<String> = args
                    .iter()
                    .enumerate()
                    .map(|(j, a)| {
                        if modes[j] && self.is_braw(a) {
                            if let Core::Var(i) = a {
                                format!("v{i}")
                            } else {
                                unreachable!()
                            }
                        } else {
                            self.val(a, true, b)
                        }
                    })
                    .collect();
                for (i, ea) in es.iter().enumerate() {
                    b.push_str(&format!("let n{i} = {ea};\n"));
                }
                for i in 0..es.len() {
                    b.push_str(&format!("v{i} = n{i};\n"));
                }
                self.flush_toks(b);
                b.push_str("continue 'l;\n");
            }
            Core::Ctor(c, a) | Core::Reuse(_, c, a)
                if self.pending.as_ref().is_some_and(|(x, _)| a.len() == 2 && a[1] == Core::Var(*x)) =>
            {
                let reuse = if let Core::Reuse(v, _, _) = e { Some(*v) } else { None };
                self.trmc_cons(*c, reuse, &a[0], b);
            }
            Core::Call(g, args)
                if self.pending.as_ref().is_some_and(|(x, _)| {
                    dps_of(*g).is_some_and(|(p, _)| args[p] == Core::Var(*x))
                }) =>
            {
                let (p, _) = dps_of(*g).unwrap();
                self.trmc_dps(*g, p, args, b);
            }
            Core::Var(v) if self.dps_param.is_some_and(|p| p as u32 == *v) => {
                // destination-passing callee reached its tail parameter:
                // the hole stays open for the caller
                self.flush_toks(b);
                b.push_str("*head_out = th_head;\n*hole_out = th_hole;\nreturn;\n");
            }
            Core::Call(g, args) if self.trmc.is_some() => {
                let (call, post) = self.dive_call(*g, args, b);
                b.push_str(&format!("let tr = {call};\n"));
                for p in post {
                    b.push_str(&format!("free_val(ctx, {p});\n"));
                }
                b.push_str(&format!(
                    "match tr {{\nOk(v) => return Ok({}),\nErr(r) => return Err({}),\n}}\n",
                    self.hole_value("v"),
                    self.hole_exit("r")
                ));
            }
            Core::Call(g, args)
                if self.nret > 0
                    && crate::scalar::native_sig(*g).is_some_and(|sig| sig.ret == crate::scalar::Kind::SK(self.nret)) =>
            {
                // the callee is native with this shape: its components come
                // back in registers, no bridge tuple
                let k = self.nret;
                let es: Vec<String> = args.iter().map(|a| self.val(a, false, b)).collect();
                let (to, from) = if crate::scalar::shifted(*g) { ("sh", "retag") } else { ("as_i", "num") };
                let fuel = if self.dive { "fuel" } else { "&mut 0i64" };
                self.flush_toks(b);
                if self.dive && crate::scalar::is_leaf(*g) {
                    b.push_str("*fuel -= 1;\n");
                }
                let rs: Vec<String> = (0..k).map(|i| format!("r{i}")).collect();
                b.push_str(&format!(
                    "let ({}) = s_{g}({}{fuel}{});\nreturn Ok([{}]);\n",
                    rs.join(", "),
                    crate::scalar::ctx_arg(*g),
                    es.iter().map(|e| format!(", {to}({e})")).collect::<String>(),
                    rs.iter().map(|r| format!("{from}({r})")).collect::<Vec<_>>().join(", ")
                ));
            }
            Core::Call(g, args) if self.nret > 0 => {
                // native multi-value form: a callee with the same native
                // shape passes its components straight through; any other
                // result is unpacked (a suspension still delivers the boxed
                // tuple to our caller's continuation)
                let k = self.nret;
                let native = ntup_of(*g) == k;
                let (call, post) = self.dive_call_as(*g, args, native, b);
                b.push_str(&format!("let tr = {call};\n"));
                for p in post {
                    b.push_str(&format!("free_val(ctx, {p});\n"));
                }
                if native {
                    b.push_str("return tr;\n");
                } else {
                    b.push_str(&format!(
                        "match tr {{\nOk(v) => return Ok(untup::<{k}>(ctx, v)),\nErr(r) => return Err(r),\n}}\n"
                    ));
                }
            }
            Core::Call(g, args) => {
                // Tail call: pass our own destination through, so a downstream
                // suspension spawns its pending call against the right parent.
                let (call, post) = self.dive_call(*g, args, b);
                if post.is_empty() {
                    b.push_str(&format!("return {call};\n"));
                } else {
                    b.push_str(&format!("let tr = {call};\n"));
                    for p in post {
                        b.push_str(&format!("free_val(ctx, {p});\n"));
                    }
                    b.push_str("return tr;\n");
                }
            }
            Core::Tuple(items) if self.nret == items.len() => {
                let es: Vec<String> = items.iter().map(|a| self.val(a, true, b)).collect();
                self.flush_toks(b);
                b.push_str(&format!("return Ok([{}]);\n", es.join(", ")));
            }
            other if self.nret > 0 => {
                let v = self.val(other, true, b);
                self.flush_toks(b);
                b.push_str(&format!("return Ok(untup::<{}>(ctx, {v}));\n", self.nret));
            }
            other => {
                let v = self.val(other, true, b);
                self.flush_toks(b);
                b.push_str(&format!("return Ok({});\n", self.hole_value(&v)));
            }
        }
    }

    /// `x = g(..)` with `g` a native multi-value dive function and `bo`
    /// binding its components (`proj_prefix`): call `n_<g>` and bind the
    /// components from the returned array, no heap tuple. Returns the rest
    /// of the body to emit. On suspension the continuation receives the
    /// boxed tuple and runs `bo` as written.
    fn ntup_let<'a>(&mut self, x: u32, r: &Core, bo: &'a Core, b: &mut String) -> Option<&'a Core> {
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
            b.push_str(&format!("let {t} = match {call} {{ Ok(a) => a, Err(_) => unreachable!() }};\n"));
        } else {
            b.push_str(&format!("let {t} = match {call} {{\nOk(a) => a,\nErr(r) => {{\n"));
            for p in &post {
                b.push_str(&format!("free_val(ctx, {p});\n"));
            }
            self.emit_capture("r", Some((x, bo)), b);
            b.push_str("}\n};\n");
        }
        for p in post {
            b.push_str(&format!("free_val(ctx, {p});\n"));
        }
        // x itself is never materialized
        self.rem.insert(x, 0);
        for i in 0..k {
            match binds.iter().find(|(_, j)| *j == i) {
                Some((y, _)) => self.emit_bind(*y, &format!("{t}[{i}]"), b),
                None => b.push_str(&format!("free_val(ctx, {t}[{i}]);\n")),
            }
        }
        Some(rest)
    }

    /// `x = g(..)` with `g` native scalar returning a tuple that `bo` only
    /// projects: destructure the native result into locals instead of
    /// packing it into a heap tuple.
    fn native_let(&mut self, x: u32, r: &Core, bo: &Core, b: &mut String) -> bool {
        let Core::Call(g, args) = r else { return false };
        let Some(sig) = crate::scalar::native_sig(*g) else { return false };
        let crate::scalar::Kind::SK(k) = sig.ret else { return false };
        if !only_projected(x, bo) {
            return false;
        }
        let es: Vec<String> = args.iter().map(|a| self.val(a, false, b)).collect();
        let fuel = if self.dive { "fuel" } else { "&mut 0i64" };
        b.push_str(&format!(
            "{}let ({}) = s_{g}({}{fuel}{});\n",
            if self.dive && crate::scalar::is_leaf(*g) { "*fuel -= 1;\n" } else { "" },
            (0..k).map(|i| format!("q{x}_{i}")).collect::<Vec<_>>().join(", "),
            crate::scalar::ctx_arg(*g),
            es.iter().map(|e| format!(", {}({e})", if crate::scalar::shifted(*g) { "sh" } else { "as_i" })).collect::<String>()
        ));
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
    fn trmc_continue(&mut self, b: &mut String) {
        let (_, qs) = self.pending.clone().expect("trmc_continue without a pending self call");
        self.flush_toks(b);
        for (i, q) in qs.iter().enumerate() {
            if Some(i) != self.dps_param {
                b.push_str(&format!("v{i} = {q};\n"));
            }
        }
        b.push_str("continue 'l;\n");
    }

    /// Tail case (a): `C(f0, x)` with `x` the pending self call.
    fn trmc_cons(&mut self, cid: u32, reuse: Option<u32>, f0: &Core, b: &mut String) {
        let e0 = self.val(f0, true, b);
        let p = self.fresh();
        let tk = reuse.map(|v| format!("tok_v{v}"));
        match tk {
            Some(tk) if self.toks.iter().any(|t| *t == tk) => {
                self.toks.retain(|t| *t != tk);
                b.push_str(&format!("let {p} = mk_con2r(ctx, {tk}, {cid}u16, {e0}, 0);\n"));
            }
            _ => b.push_str(&format!("let {p} = mk_con2(ctx, {cid}u16, {e0}, 0);\n")),
        }
        b.push_str(&format!("hole_link(ctx, &mut th_head, &mut th_hole, {p});\n"));
        self.trmc_continue(b);
    }

    /// Tail case (b): `g(.., x, ..)` with `x` the pending self call in
    /// `g`'s tail parameter: `g` appends its cells into our hole.
    fn trmc_dps(&mut self, g: u32, p: usize, args: &[Core], b: &mut String) {
        let es: Vec<String> =
            args.iter().enumerate().filter(|(i, _)| *i != p).map(|(_, a)| self.val(a, true, b)).collect();
        let argl: String = es.iter().map(|e| format!(", {e}")).collect();
        b.push_str(&format!("dp_{g}(ctx, fuel{argl}, &mut th_head, &mut th_hole);\n"));
        self.trmc_continue(b);
    }
}

/// The scrutinee variable (if `sv` names one) when `body` reuses its cell.
pub(crate) fn reuse_var(body: &Core, sv: &str) -> Option<u32> {
    let v: u32 = sv.strip_prefix('v')?.parse().ok()?;
    fn has(e: &Core, v: u32) -> bool {
        match e {
            Core::Reuse(w, _, xs) => *w == v || xs.iter().any(|x| has(x, v)),
            Core::Num(_) | Core::Flo(_) | Core::Var(_) => false,
            Core::Op2(_, a, b) | Core::Cmp(_, a, b) => has(a, v) || has(b, v),
            Core::If(c, t, f) => has(c, v) || has(t, v) || has(f, v),
            Core::Let(_, r, b) => has(r, v) || has(b, v),
            Core::Call(_, xs) | Core::Ctor(_, xs) | Core::Tuple(xs) | Core::Prim(_, xs) => xs.iter().any(|x| has(x, v)),
            Core::Proj(b, _) => has(b, v),
            Core::Match(s, arms) => has(s, v) || arms.iter().any(|(_, _, b)| has(b, v)),
        }
    }
    if has(body, v) { Some(v) } else { None }
}

/// Switch lowering for a constructor match on `sv`: dispatch on the port's
/// tag byte first (an unboxed ctor IS its tag byte), then on `con_tag` only
/// among >= 2 boxed ctors; the last boxed arm takes the remaining case
/// (matches are exhaustive by construction). Returns the opening text, the
/// arms in emission order as (index into `arms`, prefix, suffix), and the
/// closing text (without a trailing `;`).
pub(crate) fn plan_arms(
    sv: &str,
    arms: &[(u32, Vec<u32>, Core)],
    unbox: &std::collections::HashMap<u32, u8>,
) -> (String, Vec<(usize, String, String)>, String) {
    let live: Vec<usize> = (0..arms.len()).filter(|&i| arms[i].0 != UNREACHABLE_CTOR).collect();
    let ub: Vec<usize> = live.iter().copied().filter(|&i| unbox.contains_key(&arms[i].0)).collect();
    let bx: Vec<usize> = live.iter().copied().filter(|&i| !unbox.contains_key(&arms[i].0)).collect();
    let mut plan = Vec::new();
    for &i in &ub {
        let slot = unbox[&arms[i].0] as u64;
        plan.push((i, format!("{} => {{\n", 16 + slot), "}\n".to_string()));
    }
    let close;
    match bx.len() {
        0 => close = "_ => mith_unreachable(),\n}".to_string(),
        1 => {
            plan.push((bx[0], "_ => {\n".to_string(), "}\n".to_string()));
            close = "}".to_string();
        }
        n => {
            for (j, &i) in bx.iter().enumerate() {
                let pre = if j == 0 {
                    format!("_ => match con_tag({sv}) {{\n{} => {{\n", arms[i].0)
                } else if j == n - 1 {
                    "_ => {\n".to_string()
                } else {
                    format!("{} => {{\n", arms[i].0)
                };
                let suf = if j == n - 1 { "}\n}\n".to_string() } else { "}\n".to_string() };
                plan.push((i, pre, suf));
            }
            close = "}".to_string();
        }
    }
    (format!("match tag({sv}) {{\n"), plan, close)
}

/// Constructor allocation call: slice-free fast paths for arity 1 and 2.
pub(crate) fn mk_con_call(t: &str, cid: u32, es: &[String]) -> String {
    match es.len() {
        1 => format!("let {t} = mk_con1(ctx, {cid}u16, {});\n", es[0]),
        2 => format!("let {t} = mk_con2(ctx, {cid}u16, {}, {});\n", es[0], es[1]),
        _ => format!("let {t} = mk_con(ctx, {cid}u16, &[{}]);\n", es.join(", ")),
    }
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
) -> String {
    let f = &m.fns[fid as usize];
    let ar = f.arity;
    let nret = ntup_of(fid);
    let trmc = if nret > 0 || bor[fid as usize].iter().any(|b| *b) || std::env::var_os("MITHRIL_NO_TRMC").is_some() { None } else { trmc_ctor(fid, body, m, unbox) };
    let hole_rule = trmc.map(|c| sq.add_hole(c));
    let lp = f.self_tail_rec || trmc.is_some();
    let params: String =
        (0..ar).map(|i| format!(", {}v{}: u64", if lp { "mut " } else { "" }, i)).collect();
    let argl = (0..ar).map(|i| format!("v{i}")).collect::<Vec<_>>().join(", ");
    // Fuel-out at entry / loop top: the pending call IS the continuation;
    // spawn it against a forwarding record whose parent the caller sets.
    let cparams: String = (0..ar).map(|i| format!(", v{i}: u64")).collect();
    // the spawned call owns its arguments: borrowed params take a reference
    let dups: String = (0..ar)
        .filter(|&i| bor[fid as usize][i])
        .map(|i| format!("let v{i} = dup_val(ctx, v{i});\n"))
        .collect();
    let is_fold = FOLDS.with(|f| f.borrow().get(fid as usize).copied().unwrap_or(false));
    let est = if is_fold { crate::fold::est_update(fid) } else { String::new() };
    let fuel_check = format!(
        "*fuel -= 1;\nif *fuel < 0 {{\n{est}#[cold] #[inline(never)] fn cap(ctx: &mut Wctx{cparams}) -> u64 {{\n{dups}let r = ctx.alloc_rec({fwd}u16, 1, 0, 0, NONE);\nspawn_call(ctx, {}u16, &[{argl}], (r as u64) << 3);\nr as u64\n}}\nreturn Err({});\n}}\n",
        1 + fid,
        {
            let c = format!("cap(ctx{})", (0..ar).map(|i| format!(", v{i}")).collect::<String>());
            match hole_rule {
                Some(rule) => format!("{{ let r = {c}; hole_wrap(ctx, r, th_head, th_hole, {rule}u16) }}"),
                None => c,
            }
        }
    );
    let mut rem = Cnt::new();
    cnt_dive(body, &mut rem);
    let ints = crate::ints_of(tys, fid as usize);
    let mut ex = Ex::new(true, fid, lp, rem, bset.clone(), bor, ints, Some(sq), fwd, unbox, iret, tys, shared);
    ex.trmc = trmc.zip(hole_rule);
    ex.nret = nret;
    // the fuel-out spawn takes references to borrowed params
    for i in 0..ar as u32 {
        if bor[fid as usize][i as usize] {
            ex.note_share(i);
        }
    }
    let mut bb = String::new();
    if !lp || trmc.is_some() {
        // Owned parameters that the body never reads die immediately.
        for i in 0..ar as u32 {
            if !ex.bset.contains(&i) && ex.rem.get(&i).copied().unwrap_or(0) == 0 {
                bb.push_str(&format!("free_val(ctx, v{i});\n"));
            }
        }
    }
    ex.dive_tail(body, &mut bb);
    // A call-free body does bounded work: no fuel check, and it inlines
    // into its (recursive) callers.
    let leafy = !has_call(body);
    let inl = if crate::inline_attr(body).is_empty() { crate::inline_attr_fn(m, fid) } else { crate::inline_attr(body) };
    let mut s = if nret > 0 {
        // boxing entry for the runtime and callers outside the native shape
        format!(
            "#[allow(clippy::too_many_arguments)]\nfn d_{fid}(ctx: &mut Wctx, fuel: &mut i64{cparams}) -> R {{\nmatch n_{fid}(ctx, fuel, {argl}) {{\nOk(a) => Ok(mk_con(ctx, 4095u16, &a)),\nErr(r) => Err(r),\n}}\n}}\n\n{inl}#[allow(clippy::too_many_arguments)]\nfn n_{fid}(ctx: &mut Wctx, fuel: &mut i64{params}) -> Result<[u64; {nret}], u64> {{\n"
        )
    } else {
        if is_fold {
            format!("{}{inl}#[allow(clippy::too_many_arguments)]\nfn dd_{fid}(ctx: &mut Wctx, fuel: &mut i64{params}) -> R {{\n", crate::fold::heavy_wrapper(fid, &cparams, &argl))
        } else {
            format!("{inl}#[allow(clippy::too_many_arguments)]\nfn d_{fid}(ctx: &mut Wctx, fuel: &mut i64{params}) -> R {{\n")
        }
    };
    if trmc.is_some() {
        s.push_str("let mut th_head: u64 = 0;\nlet mut th_hole: u32 = NOHOLE;\n");
    }
    if lp {
        let fc = if leafy { String::new() } else { fuel_check.clone() };
        let split = if is_fold { FOLD_SPLIT.with(|f| f.borrow().get(fid as usize).cloned().flatten()).unwrap_or_default() } else { String::new() };
        if is_fold {
            s.push_str("let fold_start = v0;\n");
        }
        s.push_str(&format!("'l: loop {{\n{fc}{split}{bb}}}\n"));
    } else if leafy {
        s.push_str(&format!("let _ = fuel;\n{bb}unreachable!()\n"));
    } else {
        s.push_str(&format!("{fuel_check}{bb}unreachable!()\n"));
    }
    s.push_str("}\n\n");
    s
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
        match e {
            Core::Num(_) | Core::Flo(_) | Core::Var(_) => true,
            Core::Call(g, xs) => (*g == fid || f(*g)) && xs.iter().all(|x| calls_ok(fid, x, f)),
            Core::Op2(_, a, b) | Core::Cmp(_, a, b) => calls_ok(fid, a, f) && calls_ok(fid, b, f),
            Core::If(a, b, c) => calls_ok(fid, a, f) && calls_ok(fid, b, f) && calls_ok(fid, c, f),
            Core::Let(_, r, b) => calls_ok(fid, r, f) && calls_ok(fid, b, f),
            Core::Ctor(_, xs) | Core::Tuple(xs) | Core::Reuse(_, _, xs) | Core::Prim(_, xs) => xs.iter().all(|x| calls_ok(fid, x, f)),
            Core::Match(s, arms) => calls_ok(fid, s, f) && arms.iter().all(|(_, _, b)| calls_ok(fid, b, f)),
            Core::Proj(a, _) => calls_ok(fid, a, f),
        }
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
    p: usize,
    cid: u32,
    body: &Core,
    bor: &'m [Vec<bool>],
    sq: &'m mut SegQ,
    fwd: u16,
    unbox: &'m std::collections::HashMap<u32, u8>,
    tys: &'m Types,
    iret: &'m [bool],
    shared: &'m std::cell::RefCell<Shared>,
) -> String {
    let ar = m.fns[fid as usize].arity;
    let params: String = (0..ar).filter(|i| *i != p).map(|i| format!(", mut v{i}: u64")).collect();
    let mut rem = Cnt::new();
    cnt_dive(body, &mut rem);
    rem.remove(&(p as u32));
    let ints = crate::ints_of(tys, fid as usize);
    let mut ex = Ex::new(true, fid, true, rem, HashSet::new(), bor, ints, Some(sq), fwd, unbox, iret, tys, shared);
    ex.trmc = Some((cid, 0));
    ex.dps_param = Some(p);
    let mut bb = String::new();
    for i in 0..ar as u32 {
        if i as usize != p && ex.rem.get(&i).copied().unwrap_or(0) == 0 {
            bb.push_str(&format!("free_val(ctx, v{i});\n"));
        }
    }
    ex.dive_tail(body, &mut bb);
    format!(
        "#[allow(clippy::too_many_arguments)]\nfn dp_{fid}(ctx: &mut Wctx, fuel: &mut i64{params}, head_out: &mut u64, hole_out: &mut u32) {{\nlet mut th_head = *head_out;\nlet mut th_hole = *hole_out;\n'l: loop {{\n*fuel -= 1;\n{bb}}}\n}}\n\n"
    )
}

/// The `v<n>` variables a generated block reads from its enclosing scope:
/// every `v<n>` identifier it mentions that it does not itself `let`-bind.
pub(crate) fn outer_vars(code: &str) -> Vec<String> {
    let bytes = code.as_bytes();
    let mut used: Vec<String> = Vec::new();
    let mut bound: HashSet<String> = HashSet::new();
    let mut i = 0;
    let mut after_let = false;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if c.is_ascii_alphabetic() || c == '_' {
            let st = i;
            while i < bytes.len() && ((bytes[i] as char).is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            let id = &code[st..i];
            if id == "let" {
                after_let = true;
                continue;
            }
            if id == "mut" && after_let {
                continue;
            }
            if after_let {
                // everything up to the `=` is the pattern (tuples included)
                bound.insert(id.to_string());
            } else if id.len() > 1 && id.starts_with('v') && id[1..].bytes().all(|d| d.is_ascii_digit()) {
                if !used.iter().any(|u| u == id) {
                    used.push(id.to_string());
                }
            }
        } else {
            if c == '=' || c == ';' {
                after_let = false;
            }
            i += 1;
        }
    }
    used.retain(|u| !bound.contains(u));
    used
}

/// Split a suspended frame's continuation `bo` (a let chain) around the
/// pending value `x`: `Some((P, l, J))` where P is the chain of bindings
/// independent of x ending in its one live-out `l` (the single binder J
/// reads), and J is the dependent rest. `None` when P does no call or
/// has zero or several live-outs (those frames wait as plain records).
pub(crate) fn split_frame(x: u32, bo: &Core) -> Option<(Core, u32, Core)> {
    if std::env::var_os("MITHRIL_NO_SPLIT").is_some() {
        return None;
    }
    let mut binds: Vec<(u32, &Core)> = Vec::new();
    let mut cur = bo;
    while let Core::Let(v, r, b) = cur {
        binds.push((*v, r));
        cur = b;
    }
    let mut dep: HashSet<u32> = HashSet::from([x]);
    let (mut p, mut j): (Vec<(u32, &Core)>, Vec<(u32, &Core)>) = (Vec::new(), Vec::new());
    for (v, r) in binds {
        if free_vars(r).iter().any(|f| dep.contains(f)) {
            dep.insert(v);
            j.push((v, r));
        } else {
            p.push((v, r));
        }
    }
    if !p.iter().any(|(_, r)| has_call(r)) {
        return None;
    }
    let mut j_body = cur.clone();
    for (v, r) in j.iter().rev() {
        j_body = Core::Let(*v, Box::new((*r).clone()), Box::new(j_body));
    }
    let jf = free_vars(&j_body);
    let live: Vec<u32> = p.iter().map(|(v, _)| *v).filter(|v| jf.contains(v)).collect();
    if live.len() != 1 {
        return None;
    }
    let mut p_body = Core::Var(live[0]);
    for (v, r) in p.iter().rev() {
        p_body = Core::Let(*v, Box::new((*r).clone()), Box::new(p_body));
    }
    Some((p_body, live[0], j_body))
}

/// Every occurrence of `x` in `e` is directly under a `Proj`.
pub(crate) fn only_projected(x: u32, e: &Core) -> bool {
    match e {
        Core::Var(v) => *v != x,
        Core::Proj(a, _) => matches!(&**a, Core::Var(_)) || only_projected(x, a),
        Core::Num(_) | Core::Flo(_) => true,
        Core::Op2(_, a, b) | Core::Cmp(_, a, b) => only_projected(x, a) && only_projected(x, b),
        Core::If(a, b, c) => only_projected(x, a) && only_projected(x, b) && only_projected(x, c),
        Core::Let(_, r, b) => only_projected(x, r) && only_projected(x, b),
        Core::Call(_, xs) | Core::Ctor(_, xs) | Core::Tuple(xs) | Core::Reuse(_, _, xs) | Core::Prim(_, xs) => xs.iter().all(|a| only_projected(x, a)),
        Core::Match(s, arms) => only_projected(x, s) && arms.iter().all(|(_, _, a)| only_projected(x, a)),
    }
}
