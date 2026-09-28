//! CUDA emission: compile a `CoreModule` into `program.cu`.
//!
//! The emitted file `#define`s `PROG_NRULES`, `#include`s the fixed
//! `engine.cu`, and supplies the two device entry points the engine's
//! kernels call:
//!
//! * `prog_dive(f, args, fuel)` — sequential evaluation of function `f`
//!   with a shared fuel budget (`DIVE_FUEL` = 2^6 per fire); returns the
//!   result port raw or `SUSP` when fuel runs out.
//! * `prog_fire(rule, e0, e1, e2)` — the wave form. Rule layout:
//!   - rule `0`: boot — entry of `main` (`e0`,`e1` = args, `e2` = parent,
//!     which is `ROOT` = 0 for the boot redex, mirroring the CPU engine);
//!   - rules `1 ..= n_fns`: entry of function `rule-1` (spawned by calls);
//!   - rule `n_fns+1` / `n_fns+2`: generic Op2 / Cmp joins (record-
//!     activated; the record's `s` field carries the operator index);
//!   - rules `n_fns+3 ..`: per-program continuations (record-activated;
//!     the record's `d` field is a spilled-environment cell chain).
//!
//! Every function entry first *dives* with fresh fuel; on suspension it
//! decomposes its body one call layer (records + spawned child calls) and
//! lets later waves make progress. Dives never free cells, so a suspended
//! dive can be replayed by the decomposed form without use-after-free
//! (partial dive allocations are leaked; the arena capacity check turns
//! runaway leaks into a clean "arena exhausted" error).
//!
//! Value model (matches mithril-core ports): `Num` = tag 2 with an i56
//! payload; `Flo` = tag 3 boxing an f64 in a cell; constructors and tuples
//! = tag 4 (`addr:40 | ctor:12 | arity:4`, tuples use ctor 0xfff) whose
//! fields live in a cons-chain of cells `[field, next]` starting at `addr`.

use mithril_front::ast::{BinOp, CmpOp};
use mithril_front::core::{Core, CoreModule, UNREACHABLE_CTOR};
use std::collections::BTreeSet;
use std::fmt::Write as _;

const TUPLE_CTOR: u32 = 0xfff;

macro_rules! w {
    ($out:expr, $($arg:tt)*) => { writeln!($out, $($arg)*).unwrap() };
}

fn op_idx(op: BinOp) -> u32 {
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

fn cmp_idx(op: CmpOp) -> u32 {
    match op {
        CmpOp::Lt => 0,
        CmpOp::Le => 1,
        CmpOp::Gt => 2,
        CmpOp::Ge => 3,
        CmpOp::Eq => 4,
        CmpOp::Ne => 5,
    }
}

fn has_call(e: &Core) -> bool {
    match e {
        Core::Num(_) | Core::Flo(_) | Core::Var(_) => false,
        Core::Call(..) => true,
        Core::Op2(_, a, b) | Core::Cmp(_, a, b) => has_call(a) || has_call(b),
        Core::If(a, b, c) => has_call(a) || has_call(b) || has_call(c),
        Core::Let(_, a, b) => has_call(a) || has_call(b),
        Core::Ctor(_, args) | Core::Tuple(args) | Core::Reuse(_, _, args) | Core::Prim(_, args) => args.iter().any(has_call),
        Core::Match(s, arms) => has_call(s) || arms.iter().any(|(_, _, b)| has_call(b)),
        Core::Proj(a, _) => has_call(a),
        Core::Lam(_, a) => has_call(a),
        Core::App(f_, a_) => has_call(f_) || has_call(a_),
    }
}

fn free_vars(e: &Core, bound: &mut Vec<u32>, acc: &mut BTreeSet<u32>) {
    match e {
        Core::Num(_) | Core::Flo(_) => {}
        Core::Var(i) => {
            if !bound.contains(i) {
                acc.insert(*i);
            }
        }
        Core::Op2(_, a, b) | Core::Cmp(_, a, b) => {
            free_vars(a, bound, acc);
            free_vars(b, bound, acc);
        }
        Core::If(a, b, c) => {
            free_vars(a, bound, acc);
            free_vars(b, bound, acc);
            free_vars(c, bound, acc);
        }
        Core::Let(x, r, b) => {
            free_vars(r, bound, acc);
            bound.push(*x);
            free_vars(b, bound, acc);
            bound.pop();
        }
        Core::Call(_, args) | Core::Ctor(_, args) | Core::Tuple(args) | Core::Reuse(_, _, args) | Core::Prim(_, args) => {
            for a in args {
                free_vars(a, bound, acc);
            }
        }
        Core::Match(s, arms) => {
            free_vars(s, bound, acc);
            for (_, binds, body) in arms {
                let n = bound.len();
                bound.extend(binds.iter().copied());
                free_vars(body, bound, acc);
                bound.truncate(n);
            }
        }
        Core::Proj(a, _) => free_vars(a, bound, acc),
        Core::Lam(x, a) => {
            bound.push(*x);
            free_vars(a, bound, acc);
            bound.pop();
        }
        Core::App(f_, a_) => { free_vars(f_, bound, acc); free_vars(a_, bound, acc); }
    }
}

fn max_var(e: &Core, mx: &mut u32) {
    match e {
        Core::Num(_) | Core::Flo(_) => {}
        Core::Var(i) => *mx = (*mx).max(*i + 1),
        Core::Op2(_, a, b) | Core::Cmp(_, a, b) => {
            max_var(a, mx);
            max_var(b, mx);
        }
        Core::If(a, b, c) => {
            max_var(a, mx);
            max_var(b, mx);
            max_var(c, mx);
        }
        Core::Let(x, r, b) => {
            *mx = (*mx).max(*x + 1);
            max_var(r, mx);
            max_var(b, mx);
        }
        Core::Call(_, args) | Core::Ctor(_, args) | Core::Tuple(args) | Core::Reuse(_, _, args) | Core::Prim(_, args) => {
            for a in args {
                max_var(a, mx);
            }
        }
        Core::Match(s, arms) => {
            max_var(s, mx);
            for (_, binds, body) in arms {
                for b in binds {
                    *mx = (*mx).max(*b + 1);
                }
                max_var(body, mx);
            }
        }
        Core::Proj(a, _) => max_var(a, mx),
        Core::Lam(_, a) => max_var(a, mx),
        Core::App(f_, a_) => { max_var(f_, mx); max_var(a_, mx); }
    }
}

struct Em {
    tmp: u32,
    next_var: u32,
    n_fns: u32,
    n_conts: u32,
    conts: Vec<(u32, String)>, // (rule id, full kf_<rule> definition)
}

impl Em {
    fn t(&mut self) -> String {
        self.tmp += 1;
        format!("t{}", self.tmp)
    }

    fn fresh_var(&mut self) -> u32 {
        let v = self.next_var;
        self.next_var += 1;
        v
    }

    fn op2_join(&self) -> u32 {
        self.n_fns + 1
    }
    fn cmp_join(&self) -> u32 {
        self.n_fns + 2
    }

    /// Direct evaluation of `e` into a fresh temp; returns the temp's name.
    /// In dive mode calls go through `dv_*` and may bail with `return SUSP;`.
    /// In fire mode (`dive == false`) `e` must be call-free and `bail` is
    /// `return;` (only used by unreachable match arms).
    fn gen_val(&mut self, e: &Core, out: &mut String, dive: bool, bail: &str) -> String {
        match e {
            Core::Num(n) => {
                let tk = self.t();
                w!(out, "  u64 {tk} = mk_num({n}ll);");
                tk
            }
            Core::Flo(f) => {
                let tk = self.t();
                w!(out, "  u64 {tk} = mk_flo(__longlong_as_double(0x{:016x}ull));", f.to_bits());
                tk
            }
            Core::Var(i) => format!("v{i}"),
            Core::Op2(op, a, b) => {
                let ta = self.gen_val(a, out, dive, bail);
                let tb = self.gen_val(b, out, dive, bail);
                let tk = self.t();
                w!(out, "  u64 {tk} = d_op2({}u, {ta}, {tb});", op_idx(*op));
                tk
            }
            Core::Cmp(op, a, b) => {
                let ta = self.gen_val(a, out, dive, bail);
                let tb = self.gen_val(b, out, dive, bail);
                let tk = self.t();
                w!(out, "  u64 {tk} = d_cmp({}u, {ta}, {tb});", cmp_idx(*op));
                tk
            }
            Core::If(c, th, el) => {
                let tc = self.gen_val(c, out, dive, bail);
                let tk = self.t();
                w!(out, "  u64 {tk};");
                w!(out, "  if (as_i({tc}) != 0) {{");
                let tt = self.gen_val(th, out, dive, bail);
                w!(out, "  {tk} = {tt};");
                w!(out, "  }} else {{");
                let te = self.gen_val(el, out, dive, bail);
                w!(out, "  {tk} = {te};");
                w!(out, "  }}");
                tk
            }
            Core::Let(x, rhs, body) => {
                let tr = self.gen_val(rhs, out, dive, bail);
                let tk = self.t();
                w!(out, "  u64 {tk};");
                w!(out, "  {{ u64 v{x} = {tr}; (void)v{x};");
                let tb = self.gen_val(body, out, dive, bail);
                w!(out, "  {tk} = {tb}; }}");
                tk
            }
            Core::Call(f, args) => {
                debug_assert!(dive, "gen_val: Call in fire mode");
                let ts: Vec<String> =
                    args.iter().map(|a| self.gen_val(a, out, dive, bail)).collect();
                let tk = self.t();
                let mut cargs = ts.join(", ");
                if !cargs.is_empty() {
                    cargs.push_str(", ");
                }
                w!(out, "  u64 {tk} = dv_{f}({cargs}fuel);");
                w!(out, "  if ({tk} == SUSP) {bail}");
                tk
            }
            Core::Ctor(cid, args) | Core::Reuse(_, cid, args) => {
                if *cid == UNREACHABLE_CTOR {
                    let tk = self.t();
                    w!(out, "  u64 {tk} = mk_num(0);");
                    w!(out, "  g_abort(1u); {bail}");
                    return tk;
                }
                let ts: Vec<String> =
                    args.iter().map(|a| self.gen_val(a, out, dive, bail)).collect();
                let tk = self.chain(&ts, out);
                let tv = self.t();
                w!(out, "  u64 {tv} = mk_con({tk}, {cid}u, {}u);", args.len());
                tv
            }
            Core::Tuple(items) => {
                let ts: Vec<String> =
                    items.iter().map(|a| self.gen_val(a, out, dive, bail)).collect();
                let tk = self.chain(&ts, out);
                let tv = self.t();
                w!(out, "  u64 {tv} = mk_con({tk}, {TUPLE_CTOR}u, {}u);", items.len());
                tv
            }
            Core::Prim(..) => panic!("gpu: array primitives are not supported yet"),
            Core::Lam(..) | Core::App(..) => panic!("gpu: closures are not supported yet"),
            Core::Proj(e1, i) => {
                let te = self.gen_val(e1, out, dive, bail);
                let ak = self.t();
                w!(out, "  u32 {ak} = con_addr({te});");
                for _ in 0..*i {
                    w!(out, "  {ak} = (u32)cell1({ak});");
                }
                let tk = self.t();
                w!(out, "  u64 {tk} = cell0({ak});");
                tk
            }
            Core::Match(s, arms) => {
                let ts = self.gen_val(s, out, dive, bail);
                let tk = self.t();
                let tc = self.t();
                let ta = self.t();
                w!(out, "  u64 {tk} = mk_num(0); (void){tk};");
                w!(out, "  u32 {tc} = con_tag({ts});");
                w!(out, "  u32 {ta} = con_addr({ts}); (void){ta};");
                let mut first = true;
                for (cid, binds, body) in arms {
                    if *cid == UNREACHABLE_CTOR {
                        continue;
                    }
                    let kw = if first { "if" } else { "else if" };
                    first = false;
                    w!(out, "  {kw} ({tc} == {cid}u) {{");
                    self.bind_fields(&ta, binds, out);
                    let tb = self.gen_val(body, out, dive, bail);
                    w!(out, "  {tk} = {tb};");
                    w!(out, "  }}");
                }
                if first {
                    w!(out, "  g_abort(1u); {bail}");
                } else {
                    w!(out, "  else {{ g_abort(1u); {bail} }}");
                }
                tk
            }
        }
    }

    /// Build a cons-chain of cells for ctor/tuple fields; returns the u32
    /// temp holding the head address (0 for no fields).
    fn chain(&mut self, fields: &[String], out: &mut String) -> String {
        let ak = self.t();
        w!(out, "  u32 {ak} = 0u;");
        for f in fields.iter().rev() {
            w!(out, "  {ak} = alloc_node({f}, (u64){ak});");
        }
        ak
    }

    /// Bind ctor fields v<b0>, v<b1>, ... by walking the cell chain at `ta`.
    fn bind_fields(&mut self, ta: &str, binds: &[u32], out: &mut String) {
        let mut cur = ta.to_string();
        for (j, b) in binds.iter().enumerate() {
            w!(out, "  u64 v{b} = cell0({cur}); (void)v{b};");
            if j + 1 < binds.len() {
                let nx = self.t();
                w!(out, "  u32 {nx} = (u32)cell1({cur});");
                cur = nx;
            }
        }
    }

    /// Open a continuation: allocates a rule id and returns (rule, body
    /// prefix) with the frame loads emitted; caller appends resume code and
    /// passes the result to `cont_close`.
    fn cont_open(&mut self, fv: &BTreeSet<u32>) -> (u32, String) {
        let rule = self.n_fns + 3 + self.n_conts;
        self.n_conts += 1;
        let mut body = String::new();
        w!(body, "__device__ void kf_{rule}(u64 a0, u64 a1, u32 d, u64 parent) {{");
        w!(body, "  (void)a1; (void)d;");
        let mut cur = "d".to_string();
        let vars: Vec<u32> = fv.iter().copied().collect();
        for (j, v) in vars.iter().enumerate() {
            w!(body, "  u64 v{v} = cell0({cur}); (void)v{v};");
            let nx = self.t();
            if j + 1 < vars.len() {
                w!(body, "  u32 {nx} = (u32)cell1({cur});");
            } else {
                w!(body, "  u32 {nx} = 0u; (void){nx};");
            }
            w!(body, "  free_node({cur});");
            cur = nx;
        }
        (rule, body)
    }

    fn cont_close(&mut self, rule: u32, mut body: String) {
        w!(body, "}}");
        self.conts.push((rule, body));
    }

    /// Emit the continuation call site: spill `fv` to a frame chain,
    /// allocate the waiting record, and route `sub`'s value to its slot 0.
    fn cont_site(&mut self, rule: u32, fv: &BTreeSet<u32>, sub: &Core, dest: &str, out: &mut String) {
        let fr = self.t();
        w!(out, "  u32 {fr} = 0u;");
        for v in fv.iter().rev() {
            w!(out, "  {fr} = alloc_node(v{v}, (u64){fr});");
        }
        let rk = self.t();
        w!(out, "  u64 {rk} = (u64)alloc_rec({rule}u, 1, {fr}, 0u, {dest}) << 3;");
        self.gen_fire(sub, &rk, out);
    }

    fn fv_of(&self, e: &Core) -> BTreeSet<u32> {
        let mut acc = BTreeSet::new();
        free_vars(e, &mut Vec::new(), &mut acc);
        acc
    }

    /// Fire mode: arrange for `e`'s value to be delivered to `dest`.
    fn gen_fire(&mut self, e: &Core, dest: &str, out: &mut String) {
        if !has_call(e) {
            let tv = self.gen_val(e, out, false, "return;");
            w!(out, "  deliver({dest}, {tv});");
            return;
        }
        match e {
            Core::Op2(op, a, b) => {
                let pk = self.t();
                w!(
                    out,
                    "  u64 {pk} = (u64)alloc_rec({}u, 2, 0u, {}u, {dest}) << 3;",
                    self.op2_join(),
                    op_idx(*op)
                );
                self.gen_fire(a, &pk.clone(), out);
                self.gen_fire(b, &format!("({pk} | 1ull)"), out);
            }
            Core::Cmp(op, a, b) => {
                let pk = self.t();
                w!(
                    out,
                    "  u64 {pk} = (u64)alloc_rec({}u, 2, 0u, {}u, {dest}) << 3;",
                    self.cmp_join(),
                    cmp_idx(*op)
                );
                self.gen_fire(a, &pk.clone(), out);
                self.gen_fire(b, &format!("({pk} | 1ull)"), out);
            }
            Core::If(c, th, el) => {
                if !has_call(c) {
                    let tc = self.gen_val(c, out, false, "return;");
                    w!(out, "  if (as_i({tc}) != 0) {{");
                    self.gen_fire(th, dest, out);
                    w!(out, "  }} else {{");
                    self.gen_fire(el, dest, out);
                    w!(out, "  }}");
                } else {
                    let mut fv = self.fv_of(th);
                    fv.extend(self.fv_of(el));
                    let (rule, mut body) = self.cont_open(&fv);
                    w!(body, "  if (as_i(a0) != 0) {{");
                    self.gen_fire(th, "parent", &mut body);
                    w!(body, "  }} else {{");
                    self.gen_fire(el, "parent", &mut body);
                    w!(body, "  }}");
                    self.cont_close(rule, body);
                    self.cont_site(rule, &fv, c, dest, out);
                }
            }
            Core::Let(x, rhs, body) => {
                if !has_call(rhs) {
                    let tv = self.gen_val(rhs, out, false, "return;");
                    w!(out, "  {{ u64 v{x} = {tv}; (void)v{x};");
                    self.gen_fire(body, dest, out);
                    w!(out, "  }}");
                } else {
                    let mut fv = self.fv_of(body);
                    fv.remove(x);
                    let (rule, mut kb) = self.cont_open(&fv);
                    w!(kb, "  u64 v{x} = a0; (void)v{x};");
                    self.gen_fire(body, "parent", &mut kb);
                    self.cont_close(rule, kb);
                    self.cont_site(rule, &fv, rhs, dest, out);
                }
            }
            Core::Call(f, args) => {
                if let Some(i) = args.iter().position(has_call) {
                    // hoist the first call-carrying argument into a Let
                    let wv = self.fresh_var();
                    let mut args2 = args.clone();
                    args2[i] = Core::Var(wv);
                    let hoisted = Core::Let(
                        wv,
                        Box::new(args[i].clone()),
                        Box::new(Core::Call(*f, args2)),
                    );
                    self.gen_fire(&hoisted, dest, out);
                } else {
                    let ts: Vec<String> =
                        args.iter().map(|a| self.gen_val(a, out, false, "return;")).collect();
                    let rule = 1 + *f;
                    if args.len() <= 2 {
                        let a0 = ts.first().cloned().unwrap_or_else(|| "0ull".into());
                        let a1 = ts.get(1).cloned().unwrap_or_else(|| "0ull".into());
                        w!(out, "  spawn3({rule}u, {a0}, {a1}, {dest});");
                    } else {
                        let ak = self.chain(&ts, out);
                        w!(out, "  spawn3({rule}u, (u64){ak}, 0ull, {dest});");
                    }
                }
            }
            Core::Ctor(cid, args) | Core::Reuse(_, cid, args) => {
                let i = args.iter().position(has_call).unwrap();
                let wv = self.fresh_var();
                let mut args2 = args.clone();
                args2[i] = Core::Var(wv);
                let hoisted =
                    Core::Let(wv, Box::new(args[i].clone()), Box::new(Core::Ctor(*cid, args2)));
                self.gen_fire(&hoisted, dest, out);
            }
            Core::Tuple(items) => {
                let i = items.iter().position(has_call).unwrap();
                let wv = self.fresh_var();
                let mut items2 = items.clone();
                items2[i] = Core::Var(wv);
                let hoisted =
                    Core::Let(wv, Box::new(items[i].clone()), Box::new(Core::Tuple(items2)));
                self.gen_fire(&hoisted, dest, out);
            }
            Core::Prim(..) => panic!("gpu: array primitives are not supported yet"),
            Core::Lam(..) | Core::App(..) => panic!("gpu: closures are not supported yet"),
            Core::Proj(e1, i) => {
                let wv = self.fresh_var();
                let hoisted = Core::Let(
                    wv,
                    Box::new((**e1).clone()),
                    Box::new(Core::Proj(Box::new(Core::Var(wv)), *i)),
                );
                self.gen_fire(&hoisted, dest, out);
            }
            Core::Match(s, arms) => {
                if has_call(s) {
                    let wv = self.fresh_var();
                    let hoisted = Core::Let(
                        wv,
                        Box::new((**s).clone()),
                        Box::new(Core::Match(Box::new(Core::Var(wv)), arms.clone())),
                    );
                    self.gen_fire(&hoisted, dest, out);
                    return;
                }
                let ts = self.gen_val(s, out, false, "return;");
                let tc = self.t();
                let ta = self.t();
                w!(out, "  u32 {tc} = con_tag({ts});");
                w!(out, "  u32 {ta} = con_addr({ts}); (void){ta};");
                let mut first = true;
                for (cid, binds, body) in arms {
                    if *cid == UNREACHABLE_CTOR {
                        continue;
                    }
                    let kw = if first { "if" } else { "else if" };
                    first = false;
                    w!(out, "  {kw} ({tc} == {cid}u) {{");
                    self.bind_fields(&ta, binds, out);
                    self.gen_fire(body, dest, out);
                    w!(out, "  }}");
                }
                if first {
                    w!(out, "  g_abort(1u); return;");
                } else {
                    w!(out, "  else {{ g_abort(1u); return; }}");
                }
            }
            Core::Num(_) | Core::Flo(_) | Core::Var(_) => unreachable!("call-free"),
        }
    }
}

/// Emit `program.cu` for module `m`. See the module docs for the rule
/// layout and the value model the generated code shares with `engine.cu`.
pub fn emit_cuda(m: &CoreModule) -> String {
    let n_fns = m.fns.len() as u32;
    let mut em = Em { tmp: 0, next_var: 0, n_fns, n_conts: 0, conts: Vec::new() };

    let mut dv = String::new();
    let mut fb = String::new();
    let mut ent = String::new();

    for (i, f) in m.fns.iter().enumerate() {
        let mut mx = f.arity as u32;
        max_var(&f.body, &mut mx);
        em.next_var = mx;

        // dive form
        let sig: Vec<String> = (0..f.arity).map(|j| format!("u64 v{j}")).collect();
        let mut csig = sig.join(", ");
        if !csig.is_empty() {
            csig.push_str(", ");
        }
        w!(dv, "// {} (arity {})", f.name, f.arity);
        w!(dv, "__device__ u64 dv_{i}({csig}int *fuel) {{");
        w!(dv, "  if (--(*fuel) < 0) return SUSP;");
        for j in 0..f.arity {
            w!(dv, "  (void)v{j};");
        }
        let tb = em.gen_val(&f.body, &mut dv, true, "return SUSP;");
        w!(dv, "  return {tb};");
        w!(dv, "}}");
        w!(dv, "");

        // fire form (record decomposition)
        w!(fb, "__device__ void fb_{i}({csig}u64 parent) {{");
        for j in 0..f.arity {
            w!(fb, "  (void)v{j};");
        }
        em.gen_fire(&f.body, "parent", &mut fb);
        w!(fb, "}}");
        w!(fb, "");

        // entry: dive with fresh fuel, decompose on suspension
        w!(ent, "__device__ void ent_{i}(u64 e0, u64 e1, u64 e2) {{");
        w!(ent, "  (void)e0; (void)e1;");
        let vargs: Vec<String> = if f.arity <= 2 {
            (0..f.arity).map(|j| format!("e{j}")).collect()
        } else {
            let mut names = Vec::new();
            w!(ent, "  u32 c0 = (u32)e0;");
            let mut cvar = "c0".to_string();
            for j in 0..f.arity {
                w!(ent, "  u64 x{j} = cell0({cvar});");
                if j + 1 < f.arity {
                    let nxt = format!("c{}", j + 1);
                    w!(ent, "  u32 {nxt} = (u32)cell1({cvar});");
                    w!(ent, "  free_node({cvar});");
                    cvar = nxt;
                } else {
                    w!(ent, "  free_node({cvar});");
                }
                names.push(format!("x{j}"));
            }
            names
        };
        let mut cargs = vargs.join(", ");
        if !cargs.is_empty() {
            cargs.push_str(", ");
        }
        w!(ent, "  int fu = DIVE_FUEL;");
        w!(ent, "  u64 r = dv_{i}({cargs}&fu);");
        w!(ent, "  if (r != SUSP) {{ deliver(e2, r); return; }}");
        w!(ent, "  fb_{i}({cargs}e2);");
        w!(ent, "}}");
        w!(ent, "");
    }

    let total_rules = n_fns + 3 + em.n_conts;

    let mut out = String::new();
    w!(out, "// program.cu generated by mithril-gpu emit_cuda; do not edit.");
    w!(out, "#define PROG_NRULES {total_rules}");
    w!(out, "#include \"engine.cu\"");
    w!(out, "");
    // forward declarations (functions are mutually recursive)
    for i in 0..m.fns.len() {
        let f = &m.fns[i];
        let ps = vec!["u64".to_string(); f.arity];
        let mut sig = ps.join(", ");
        if !sig.is_empty() {
            sig.push_str(", ");
        }
        w!(out, "__device__ u64 dv_{i}({sig}int *fuel);");
        w!(out, "__device__ void fb_{i}({sig}u64 parent);");
        w!(out, "__device__ void ent_{i}(u64 e0, u64 e1, u64 e2);");
    }
    for (rule, _) in &em.conts {
        w!(out, "__device__ void kf_{rule}(u64 a0, u64 a1, u32 d, u64 parent);");
    }
    w!(out, "");
    out.push_str(&dv);
    for (_, body) in &em.conts {
        out.push_str(body);
        out.push('\n');
    }
    out.push_str(&fb);
    out.push_str(&ent);

    // prog_dive dispatcher
    w!(out, "__device__ u64 prog_dive(u32 f, const u64 *args, int *fuel) {{");
    w!(out, "  (void)args;");
    w!(out, "  switch (f) {{");
    for (i, f) in m.fns.iter().enumerate() {
        let mut cargs =
            (0..f.arity).map(|j| format!("args[{j}]")).collect::<Vec<_>>().join(", ");
        if !cargs.is_empty() {
            cargs.push_str(", ");
        }
        w!(out, "  case {i}u: return dv_{i}({cargs}fuel);");
    }
    w!(out, "  }}");
    w!(out, "  return SUSP;");
    w!(out, "}}");
    w!(out, "");

    // prog_fire dispatcher
    w!(out, "__device__ void prog_fire(u32 rule, u64 e0, u64 e1, u64 e2) {{");
    w!(out, "  switch (rule) {{");
    w!(out, "  case 0u: ent_{}(e0, e1, e2); return; // boot: main entry", m.main);
    for i in 0..m.fns.len() {
        w!(out, "  case {}u: ent_{i}(e0, e1, e2); return;", i + 1);
    }
    w!(out, "  default: break;");
    w!(out, "  }}");
    w!(out, "  // record-activated rules: e2 is the record index");
    w!(out, "  u32 ri = (u32)e2;");
    w!(out, "  if (ri >= G.rcap) ri = G.rcap - 1;");
    w!(out, "  Rec rr = G.recs[ri];");
    w!(out, "  switch (rule) {{");
    w!(out, "  case {}u: deliver(rr.parent, d_op2(rr.s, e0, e1)); return;", n_fns + 1);
    w!(out, "  case {}u: deliver(rr.parent, d_cmp(rr.s, e0, e1)); return;", n_fns + 2);
    for (rule, _) in &em.conts {
        w!(out, "  case {rule}u: kf_{rule}(e0, e1, rr.d, rr.parent); return;");
    }
    w!(out, "  default: g_abort(1u); return;");
    w!(out, "  }}");
    w!(out, "}}");
    out
}
