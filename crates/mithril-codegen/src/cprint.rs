//! The C-family printer of the lowered IR, in two dialects: CUDA (over the
//! device runtime `engine.cu`) and the Metal Shading Language (the range
//! leaves of the Metal backend).
//!
//! It spells the emitters' decisions in C++ over a device runtime that
//! supplies the IR's helper vocabulary without a context object.
//! Suspension results are `R` / `RA<k>` structs, tuples are `T<k>` /
//! `P2` structs, nested capture functions are lambdas, slice arguments
//! are hoisted to local arrays. MSL differs in four places: pointers name
//! their address space, 64-bit literals use `l` / `ul`, a loop is left by
//! `break` (through a flag from inside a switch) since MSL has no `goto`,
//! and the stack guard compares a nesting depth passed by value
//! (`msl_leaves`).

use crate::lir::{Bop, FnDef, Inline, Pat, Ty, E, S};
use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dialect {
    Cuda,
    Msl,
}

struct P<'a> {
    /// return type of every generated function, by name
    fnret: &'a HashMap<String, Ty>,
    /// types of the locals in scope
    locals: HashMap<String, Ty>,
    /// names declared per open block (a redeclaration prints as assignment)
    scopes: Vec<HashSet<String>>,
    /// statements hoisted before the one being printed (slice arrays)
    pre: String,
    tmp: u32,
    /// the enclosing function's return type
    ret: Ty,
    out: String,
    direct_machine: bool,
    loop_exits: Vec<u32>,
    dialect: Dialect,
    /// open loops and switches, innermost last: `Some(loop id)` or `None`
    /// for a switch (MSL leaves a loop from inside a switch by a flag)
    breakable: Vec<Option<u32>>,
    /// loops left through a flag from inside a switch
    flagged: HashSet<u32>,
}

/// The C type of an IR type; MSL pointers name their address space.
pub fn ty(t: Ty, dialect: Dialect) -> String {
    let space = if dialect == Dialect::Msl { "thread " } else { "" };
    match t {
        Ty::U64 => "u64".into(),
        Ty::I64 => "i64".into(),
        Ty::U32 => "u32".into(),
        Ty::U16 => "u16".into(),
        Ty::U8 => "u8".into(),
        Ty::Bool => "bool".into(),
        Ty::Usize => "usize".into(),
        Ty::Res => "R".into(),
        Ty::ResArr(k) => format!("RA<{k}>"),
        Ty::Arr(k) => format!("A<{k}>"),
        Ty::Tup(k) => format!("T{k}"),
        Ty::RefU64 => format!("{space}u64*"),
        Ty::RefU32 => format!("{space}u32*"),
        Ty::RefI64 => format!("{space}i64*"),
        Ty::Infer => "auto".into(),
        Ty::Frames | Ty::FrameRecords(_) => "NativeFrames".into(),
        Ty::Unit => "void".into(),
    }
}

/// Return type of a helper of the device runtime (see engine.cu).
fn helper_ret(f: &str) -> Ty {
    if let Some(h) = crate::lir::operations::helper(f) {
        use crate::lir::operations::Effect;
        if matches!(h.effect, Effect::Value | Effect::Borrow | Effect::Allocate) { return h.result; }
    }
    match f {
        "sat_mul" | "imax" | "fuel_of" | "atomic_load" | "work_mark" | "work_since" | "work_quantum" => Ty::I64,
        "con_ar" => Ty::U8,
        "con_addr" | "rec_d" | "rec_s" => Ty::U32,
        "arr_len_of" => Ty::Usize,
        "flag_load" | "is_err" | "native_empty" | "native_ok" => Ty::Bool,
        "apply" | "dive_res" | "dive_res_fork" => Ty::Res,
        "fork_fuel" => Ty::RefI64,
        _ if f.starts_with("f32_") => Ty::I64,
        _ => Ty::U64,
    }
}

impl<'a> P<'a> {
    fn fresh(&mut self, p: &str) -> String {
        self.tmp += 1;
        format!("{p}{}", self.tmp)
    }

    fn call_ret(&self, f: &str) -> Ty {
        if let Some(t) = self.fnret.get(f) {
            return *t;
        }
        if let Some(rest) = f.strip_prefix("untup::<").or_else(|| f.strip_prefix("consume_chain::<")) {
            let k: usize = rest.trim_end_matches('>').parse().unwrap_or(0);
            return Ty::Arr(k);
        }
        helper_ret(f)
    }

    /// The type an expression has (operand widths decide C's arithmetic).
    fn ty_of(&self, e: &E) -> Ty {
        match e {
            E::Int(_, t) => *t,
            E::Bool(_) => Ty::Bool,
            E::Flo(_) => Ty::I64,
            E::V(x) => self.locals.get(x).copied().unwrap_or(Ty::I64),
            E::Deref(x) => match self.locals.get(x) {
                Some(Ty::RefU64) => Ty::U64,
                Some(Ty::RefU32) => Ty::U32,
                _ => Ty::I64,
            },
            E::Ref(_) | E::Addr(_) => Ty::U64,
            E::Call { f, .. } => self.call_ret(f),
            E::Bin(op, a, _) => {
                if matches!(op, Bop::Lt | Bop::Le | Bop::Gt | Bop::Ge | Bop::Eq | Bop::Ne) {
                    Ty::Bool
                } else {
                    self.ty_of(a)
                }
            }
            E::Not(_) => Ty::Bool,
            E::Neg(_) => Ty::I64,
            E::Cast(_, t) => *t,
            E::Idx(..) => Ty::U64,
            E::Tup(xs) => Ty::Tup(xs.len()),
            E::Arr(xs) => Ty::Arr(xs.len()),
            E::Slice(_) => Ty::U64,
            E::Ok(_) | E::Err(_) => self.ret,
            E::Const(c) => match c.as_str() {
                "NOHOLE" => Ty::U32,
                c if c.starts_with("FOLD_EST") => Ty::I64,
                _ => Ty::U64,
            },
        }
    }

    fn ex(&mut self, e: &E) -> String {
        match e {
            // MSL has no `long long`: its 64-bit literals are `l` / `ul`
            E::Int(n, t) if self.dialect == Dialect::Msl && matches!(t, Ty::I64 | Ty::U64 | Ty::Usize) => match t {
                Ty::I64 => format!("((i64){}ul)", *n as u64),
                _ => format!("{}ul", *n as u64),
            },
            E::Int(n, t) => match t {
                Ty::I64 => format!("({n}ll)"),
                Ty::U64 | Ty::Usize => format!("{}ull", *n as u64),
                Ty::U32 => format!("{n}u"),
                Ty::U16 => format!("((u16){n})"),
                Ty::U8 => format!("((u8){n})"),
                _ => format!("{n}"),
            },
            E::Bool(b) => b.to_string(),
            E::Flo(x) => match self.dialect {
                Dialect::Cuda => format!("__longlong_as_double(0x{:016x}ll)", x.to_bits()),
                Dialect::Msl => unreachable!("an f64 value in MSL: boxed floats are not printed there"),
            },
            E::V(x) | E::Const(x) => x.clone(),
            E::Deref(x) => format!("(*{x})"),
            E::Ref(x) | E::Addr(x) => format!("(&{x})"),
            E::Call { f, args, .. } => {
                let name = f.replace("::<", "<");
                let mut a: Vec<String> = Vec::new();
                for x in args {
                    if let E::Addr(name) = x {
                        if let Some(Ty::Arr(k)) = self.locals.get(name) {
                            a.push(format!("{name}.a, {k}"));
                            continue;
                        }
                    }
                    if let E::Slice(xs) = x {
                        let n = xs.len();
                        let s = self.fresh("_s");
                        let items: Vec<String> = xs.iter().map(|y| self.ex(y)).collect();
                        let _ = writeln!(self.pre, "u64 {s}[{}] = {{{}}};", n.max(1), items.join(", "));
                        a.push(format!("{s}, {n}"));
                    } else {
                        a.push(self.ex(x));
                    }
                }
                format!("{name}({})", a.join(", "))
            }
            E::Bin(op, a, b) => {
                let t = self.ty_of(a);
                let (sa, sb) = (self.ex(a), self.ex(b));
                let ct = ty(t, self.dialect);
                let cmp = |o: &str| format!("(({ct})({sa}) {o} ({ct})({sb}))");
                match op {
                    Bop::Lt => cmp("<"),
                    Bop::Le => cmp("<="),
                    Bop::Gt => cmp(">"),
                    Bop::Ge => cmp(">="),
                    Bop::Eq => cmp("=="),
                    Bop::Ne => cmp("!="),
                    Bop::And => format!("(({sa}) & ({sb}))"),
                    Bop::Or => format!("(({sa}) | ({sb}))"),
                    Bop::Xor => format!("(({sa}) ^ ({sb}))"),
                    Bop::Add | Bop::Sub | Bop::Mul | Bop::Shl | Bop::Div | Bop::Shr => {
                        let o = match op {
                            Bop::Add => "+",
                            Bop::Sub => "-",
                            Bop::Mul => "*",
                            Bop::Shl => "<<",
                            Bop::Div => "/",
                            _ => ">>",
                        };
                        match (t, op) {
                            (Ty::I64, Bop::Div) => format!("idiv({sa}, {sb})"),
                            (Ty::I64, Bop::Shr) => format!("(({sa}) >> (({sb}) & 63))"),
                            (Ty::I64, Bop::Shl) => format!("((i64)((u64)({sa}) << (({sb}) & 63)))"),
                            // wrapping: through the unsigned width
                            (Ty::I64, _) => format!("((i64)((u64)({sa}) {o} (u64)({sb})))"),
                            (Ty::U32, Bop::Shl | Bop::Shr) => format!("((u32)({sa}) {o} (({sb}) & 31))"),
                            (Ty::U32, _) => format!("((u32)({sa}) {o} (u32)({sb}))"),
                            (_, Bop::Shl | Bop::Shr) => format!("(({sa}) {o} (({sb}) & 63))"),
                            _ => format!("(({sa}) {o} ({sb}))"),
                        }
                    }
                }
            }
            E::Not(a) => format!("(!({}))", self.ex(a)),
            E::Neg(a) => format!("(-({}))", self.ex(a)),
            E::Cast(a, t) => format!("(({})({}))", ty(*t, self.dialect), self.ex(a)),
            E::Idx(a, i) => format!("{}.a[{i}]", self.ex(a)),
            E::Tup(xs) => {
                let items: Vec<String> = xs.iter().map(|x| self.ex(x)).collect();
                format!("T{}{{{}}}", xs.len(), items.join(", "))
            }
            E::Arr(xs) => {
                let items: Vec<String> = xs.iter().map(|x| self.ex(x)).collect();
                format!("A<{}>{{{{{}}}}}", xs.len(), items.join(", "))
            }
            E::Slice(_) => unreachable!("slice outside an argument list"),
            E::Ok(a) => {
                let s = self.ex(a);
                match self.ret {
                    Ty::ResArr(k) => format!("RA<{k}>{{{s}, 0ull, true}}"),
                    _ => format!("R{{{s}, true}}"),
                }
            }
            E::Err(a) => {
                let s = self.ex(a);
                match self.ret {
                    Ty::ResArr(k) => format!("RA<{k}>{{A<{k}>{{}}, (u64)({s}), false}}"),
                    _ => format!("R{{(u64)({s}), false}}"),
                }
            }
        }
    }

    /// Flush hoisted declarations, then the statement text.
    fn line(&mut self, s: String) {
        let pre = std::mem::take(&mut self.pre);
        self.out.push_str(&pre);
        self.out.push_str(&s);
        self.out.push('\n');
    }

    /// Declare `x: t = init` (assignment when already declared in scope).
    fn decl(&mut self, x: &str, t: Ty, init: Option<String>) {
        let known = self.scopes.last().map_or(false, |s| s.contains(x));
        if known {
            if let Some(i) = init {
                self.line(format!("{x} = {i};"));
            }
            return;
        }
        if let Some(s) = self.scopes.last_mut() {
            s.insert(x.to_string());
        }
        self.locals.insert(x.to_string(), t);
        match init {
            Some(i) => self.line(format!("{} {x} = {i};", ty(t, self.dialect))),
            None => self.line(format!("{} {x};", ty(t, self.dialect))),
        }
    }

    fn block(&mut self, body: &[S]) {
        self.block_with(body, HashSet::new())
    }

    /// A block whose scope starts with `declared` (a function's parameters).
    fn block_with(&mut self, body: &[S], declared: HashSet<String>) {
        self.out.push_str("{\n");
        self.scopes.push(declared);
        for s in body {
            self.stmt(s);
        }
        self.scopes.pop();
        self.out.push_str("}\n");
    }

    /// The zero of the enclosing function's return type (after an abort).
    fn zero_ret(&self) -> String {
        match self.ret {
            Ty::Unit => "return;".into(),
            Ty::Res => "return R{0ull, true};".into(),
            Ty::ResArr(k) => format!("return RA<{k}>{{}};"),
            Ty::Tup(k) => format!("return T{k}{{}};"),
            t => format!("return ({})0;", ty(t, self.dialect)),
        }
    }

    fn stmt(&mut self, s: &S) {
        match s {
            S::Let(pat, t, e) => {
                let init = self.ex(e);
                match pat {
                    Pat::One(x) => {
                        let t = if *t == Ty::Infer { self.ty_of(e) } else { *t };
                        let t = if t == Ty::Infer { Ty::U64 } else { t };
                        self.decl(x, t, Some(init));
                    }
                    Pat::Tup(names) => {
                        let d = self.fresh("_d");
                        self.line(format!("auto {d} = {init};"));
                        let et = self.ty_of(e);
                        for (i, x) in names.iter().enumerate() {
                            let ft = match et {
                                Ty::Tup(_) => Ty::I64,
                                _ if i == 2 => Ty::U32, // consume2r's token
                                _ => Ty::U64,
                            };
                            self.decl(x, ft, Some(format!("{d}.f{i}")));
                        }
                    }
                    Pat::Arr(names) if names.is_empty() => {} // a nullary ctor has no cell
                    Pat::Arr(names) => {
                        let d = self.fresh("_d");
                        self.line(format!("auto {d} = {init};"));
                        for (i, x) in names.iter().enumerate() {
                            self.decl(x, Ty::U64, Some(format!("{d}.a[{i}]")));
                        }
                    }
                }
            }
            S::Decl(x, t) => self.decl(x, *t, None),
            S::Set(x, e) => {
                let v = self.ex(e);
                self.line(format!("{x} = {v};"));
            }
            S::Store(x, e) => {
                let v = self.ex(e);
                self.line(format!("(*{x}) = {v};"));
            }
            // the stack guard leaves the frame (value-initialized return)
            // MSL counts nesting in `dl` (see `msl_leaves`)
            S::Do(E::Call { f, .. }) if f == "stack_guard" && self.dialect == Dialect::Msl => {
                let z = self.zero_ret();
                self.line(format!("if (dl >= DEEP_LIMIT) {{ fuel[1] |= DEEP_FAULT; {z} }}"));
            }
            S::Do(E::Call { f, .. }) if f == "stack_guard" => self.line(format!("if (stack_deep()) return{};", if self.ret == Ty::Unit { "" } else { " {}" })),
            S::Do(e) => {
                let v = self.ex(e);
                self.line(format!("{v};"));
            }
            S::If(c, a, b) => {
                let cnd = self.ex(c);
                self.line(format!("if ({cnd}) "));
                self.block(a);
                if !b.is_empty() {
                    self.out.push_str("else ");
                    self.block(b);
                }
            }
            S::Switch(e, arms, d) => {
                let v = self.ex(e);
                self.breakable.push(None);
                self.line(format!("switch ({v}) {{"));
                for (keys, body) in crate::lir::grouped_arms(arms) {
                    for k in keys { self.out.push_str(&format!("case {k}: ")); }
                    self.block(body);
                    self.out.push_str("break;\n");
                }
                self.out.push_str("default: ");
                match d {
                    Some(body) => self.block(body),
                    None => {
                        let z = self.zero_ret();
                        self.out.push_str(&format!("{{ mith_unreachable(); {z} }}\n"));
                    }
                }
                self.out.push_str("break;\n}\n");
                self.breakable.pop();
                // a loop left from inside this switch: leave the next level
                if let Some(&id) = self.loop_exits.last() {
                    if self.flagged.contains(&id) && self.dialect == Dialect::Msl {
                        self.line(format!("if (brk_{id}) "));
                        self.stmt(&S::Break);
                    }
                }
            }
            S::Loop(body) => {
                let exit=self.tmp;self.tmp+=1;self.loop_exits.push(exit);
                if self.dialect == Dialect::Msl {
                    self.line(format!("bool brk_{exit} = false;"));
                }
                self.breakable.push(Some(exit));
                self.out.push_str("for (;;) ");
                // after a fault a leaf stops: its values are no longer
                // meaningful, and a loop on them need not end
                let mut body = body.clone();
                if self.dialect == Dialect::Msl && self.locals.contains_key("fuel") {
                    body.insert(0, S::If(E::Call { f: "deep_faulted".into(), args: vec![E::V("fuel".into())], ctx: false }, vec![S::Unreachable], vec![]));
                }
                self.block(&body);
                self.breakable.pop();
                self.loop_exits.pop();
                if self.dialect == Dialect::Cuda {
                    self.line(format!("loop_exit_{exit}:;"));
                }
            }
            S::Machine(_) if self.dialect == Dialect::Msl => unreachable!("a native machine in MSL: leaves are lowered without frames"),
            S::Machine(arms) => {
                fn dynamic(body: &[S]) -> bool {
                    body.iter().any(|s| match s {
                        S::Jump(e) => !matches!(e, E::Int(_, _)),
                        S::Set(n, _) if n == "pc" => true,
                        S::If(_, a, b) => dynamic(a) || dynamic(b),
                        S::Switch(_, arms, d) => arms.iter().any(|(_, b)| dynamic(b)) || d.as_ref().is_some_and(|b| dynamic(b)),
                        S::Loop(b) => dynamic(b),
                        _ => false,
                    })
                }
                self.direct_machine = arms.iter().all(|(_, b)| !dynamic(b));
                if self.direct_machine {
                    self.stmt(&S::Switch(E::V("pc".into()), arms.iter().map(|(id, _)| (*id, vec![S::Jump(E::Int(*id as i64, Ty::U64))])).collect(), None));
                    for (id, body) in arms { self.line(format!("native_case_{id}:")); self.block(body); self.line(format!("goto native_case_{id};")); }
                } else {
                    self.out.push_str("native_dispatch: for (;;) {\n");
                    self.stmt(&S::Switch(E::V("pc".into()), arms.clone(), None));
                    self.out.push_str("}\n");
                }
                self.direct_machine = false;
            }
            S::Jump(e) => {
                if self.direct_machine {
                    let E::Int(id, _) = e else { unreachable!() };
                    self.line(format!("goto native_case_{id};"));
                } else { let value = self.ex(e); self.line(format!("pc = {value}; goto native_dispatch;")); }
            }
            S::Continue => self.out.push_str("continue;\n"),
            S::Break if self.dialect == Dialect::Msl => {
                let id = *self.loop_exits.last().expect("break outside loop");
                if self.breakable.last() == Some(&Some(id)) {
                    self.line("break;".into());
                } else {
                    self.flagged.insert(id);
                    self.line(format!("{{ brk_{id} = true; break; }}"));
                }
            }
            S::Break => self.line(format!("goto loop_exit_{};",self.loop_exits.last().expect("break outside loop"))),
            S::Ret(e) => {
                if matches!(e, E::Tup(xs) if xs.is_empty()) {
                    self.out.push_str("return;\n");
                } else {
                    let v = self.ex(e);
                    self.line(format!("return {v};"));
                }
            }
            S::Try(pat, e, r, h) => {
                let rt = self.ty_of(e);
                let v = self.ex(e);
                let rn = self.fresh("_r");
                // On the device the budget a dive-form call hands down is
                // refunded when the callee returns, so the budget bounds the
                // native recursion depth (the device stack is small) and not
                // the work of a subtree: a wide, shallow subtree runs without
                // suspending. (The CPU keeps the work budget: its suspension
                // is what exposes work to idle threads.)
                let hands_budget = matches!(e, E::Call { args, .. } if matches!(args.first(), Some(E::V(f)) if f == "fuel") || matches!(args.first(), Some(E::Call { f, .. }) if f == "fork_fuel"));
                if hands_budget {
                    self.line(format!("i64 {rn}_keep = *fuel; auto {rn} = {v}; *fuel = {rn}_keep;"));
                } else {
                    self.line(format!("auto {rn} = {v};"));
                }
                self.out.push_str(&format!("if (!{rn}.ok) "));
                let recf = if matches!(rt, Ty::ResArr(_)) { "rec" } else { "v" };
                let mut hb = vec![];
                if r != "_" {
                    hb.push(S::Let(Pat::One(r.clone()), Ty::U32, E::Cast(Box::new(E::V(format!("{rn}.{recf}"))), Ty::U32)));
                }
                hb.extend(h.iter().cloned());
                self.block(&hb);
                let Pat::One(x) = pat else { unreachable!("try binds one value") };
                match rt {
                    Ty::ResArr(k) => self.decl(x, Ty::Arr(k), Some(format!("{rn}.a"))),
                    _ => self.decl(x, Ty::U64, Some(format!("{rn}.v"))),
                }
            }
            S::Res(e, o, a, r, b) => {
                let rt = self.ty_of(e);
                let v = self.ex(e);
                let rn = self.fresh("_r");
                self.line(format!("auto {rn} = {v};"));
                self.out.push_str(&format!("if ({rn}.ok) "));
                let mut ab = vec![match rt {
                    Ty::ResArr(k) => S::Let(Pat::One(o.clone()), Ty::Arr(k), E::V(format!("{rn}.a"))),
                    _ => S::Let(Pat::One(o.clone()), Ty::U64, E::V(format!("{rn}.v"))),
                }];
                ab.extend(a.iter().cloned());
                self.block(&ab);
                self.out.push_str("else ");
                let recf = if matches!(rt, Ty::ResArr(_)) { "rec" } else { "v" };
                let mut bb = vec![S::Let(Pat::One(r.clone()), Ty::U32, E::Cast(Box::new(E::V(format!("{rn}.{recf}"))), Ty::U32))];
                bb.extend(b.iter().cloned());
                self.block(&bb);
            }
            S::Fn(d) => {
                // a lambda over explicit parameters (no captures)
                let params: Vec<String> = d.params.iter().map(|(x, t)| format!("{} {x}", ty(*t, self.dialect))).collect();
                self.out.push_str(&format!("auto {} = [](
{}) -> {} ", d.name, params.join(", "), ty(d.ret, self.dialect)));
                let saved = (std::mem::take(&mut self.locals), self.ret);
                self.locals = d.params.iter().cloned().collect();
                self.ret = d.ret;
                self.block_with(&d.body, d.params.iter().map(|(x, _)| x.clone()).collect());
                self.out.push_str(";\n");
                self.locals = saved.0;
                self.ret = saved.1;
                self.locals.insert(d.name.clone(), d.ret);
            }
            S::Unreachable => {
                let z = self.zero_ret();
                self.out.push_str(&format!("mith_unreachable(); {z}\n"));
            }
            S::Comment(t) => {
                self.out.push_str(&format!("// {t}\n"));
            }
        }
    }
}

/// Every `T<k>` width a statement list mentions.
fn tuple_widths(body: &[S], out: &mut Vec<usize>) {
    fn ex(e: &E, out: &mut Vec<usize>) {
        match e {
            E::Tup(xs) => {
                if !xs.is_empty() {
                    out.push(xs.len());
                }
                xs.iter().for_each(|x| ex(x, out));
            }
            E::Call { args, .. } | E::Arr(args) | E::Slice(args) => args.iter().for_each(|a| ex(a, out)),
            E::Bin(_, a, b) => {
                ex(a, out);
                ex(b, out);
            }
            E::Not(a) | E::Neg(a) | E::Cast(a, _) | E::Idx(a, _) | E::Ok(a) | E::Err(a) => ex(a, out),
            _ => {}
        }
    }
    for s in body {
        match s {
            S::Let(_, t, e) => {
                if let Ty::Tup(k) = t {
                    out.push(*k);
                }
                ex(e, out);
            }
            S::Decl(_, Ty::Tup(k)) => out.push(*k),
            S::Set(_, e) | S::Store(_, e) | S::Do(e) | S::Ret(e) | S::Jump(e) => ex(e, out),
            S::If(e, a, b) => {
                ex(e, out);
                tuple_widths(a, out);
                tuple_widths(b, out);
            }
            S::Switch(e, arms, d) => {
                ex(e, out);
                arms.iter().for_each(|(_, b)| tuple_widths(b, out));
                d.iter().for_each(|b| tuple_widths(b, out));
            }
            S::Loop(b) => tuple_widths(b, out),
            S::Machine(arms) => { for (_, b) in arms { tuple_widths(b, out); } }
            S::Try(_, e, _, h) => {
                ex(e, out);
                tuple_widths(h, out);
            }
            S::Res(e, _, a, _, b) => {
                ex(e, out);
                tuple_widths(a, out);
                tuple_widths(b, out);
            }
            S::Fn(d) => {
                if let Ty::Tup(k) = d.ret {
                    out.push(k);
                }
                tuple_widths(&d.body, out);
            }
            _ => {}
        }
    }
}

/// A dive form (it returns a dive result) is compiled once, out of line.
/// The dispatch (`prog_dive`) reaches every dive form and dives call each
/// other: inlined, the program is copied into every function that calls
/// the dispatch or another dive, and the device compiler compiles each
/// copy (design.md 5.1).
pub fn signature(d: &FnDef, dialect: Dialect) -> String {
    let params: Vec<String> = d.params.iter().map(|(x, t)| format!("{} {x}", ty(*t, dialect))).collect();
    match dialect {
        Dialect::Cuda => {
            let once = if d.ret == Ty::Res && d.inline != Inline::Always { "__noinline__ " } else { "" };
            format!("__device__ {once}{} {}({})", ty(d.ret, dialect), d.name, params.join(", "))
        }
        Dialect::Msl => format!("{} {}({})", ty(d.ret, dialect), d.name, params.join(", ")),
    }
}

/// The definition of `d`.
pub fn function(d: &FnDef, fnret: &HashMap<String, Ty>, dialect: Dialect) -> String {
    let mut p = P {
        fnret,
        locals: d.params.iter().cloned().collect(),
        scopes: Vec::new(),
        pre: String::new(),
        tmp: 0,
        ret: d.ret,
        out: String::new(),
        direct_machine: false,
        loop_exits: Vec::new(),
        dialect,
        breakable: Vec::new(),
        flagged: HashSet::new(),
    };
    p.out.push_str(&signature(d, dialect));
    p.out.push(' ');
    p.block_with(&d.body, d.params.iter().map(|(x, _)| x.clone()).collect());
    p.out
}

/// The `T<k>` structs of every tuple width the functions use.
pub fn tuple_structs(fns: &[&FnDef]) -> String {
    let mut widths: Vec<usize> = Vec::new();
    for d in fns {
        if let Ty::Tup(k) = d.ret {
            widths.push(k);
        }
        tuple_widths(&d.body, &mut widths);
    }
    widths.sort_unstable();
    widths.dedup();
    let mut out = String::new();
    for k in widths {
        let fs: Vec<String> = (0..k).map(|i| format!("f{i}")).collect();
        let _ = writeln!(out, "struct T{k} {{ i64 {}; }};", if fs.is_empty() { "_z".to_string() } else { fs.join(", ") });
    }
    out
}

/// The range leaves of a lowered program in MSL: each range fold's native
/// loop with every function it reaches, and `prog_range_leaf`, which runs
/// index `i` of fold `fid` from the request's arguments `args` (decoded
/// ints, arrays as device handles, and a sum's accumulator 0: a term starts
/// from the identity). Empty when the program has no range folds.
pub fn msl_leaves(fns: &[FnDef], fills: &[crate::RangeFill]) -> String {
    if fills.is_empty() {
        return String::new();
    }
    let by_name: HashMap<&str, &FnDef> = fns.iter().map(|f| (f.name.as_str(), f)).collect();
    let mut reached: Vec<&FnDef> = Vec::new();
    let mut seen: HashSet<&str> = HashSet::new();
    let mut todo: Vec<String> = fills.iter().map(|r| format!("s_{}", r.fid)).collect();
    while let Some(name) = todo.pop() {
        let Some(&f) = by_name.get(name.as_str()) else { continue };
        if !seen.insert(f.name.as_str()) {
            continue;
        }
        reached.push(f);
        crate::lir::walk_exprs(&f.body, &mut |e| {
            if let E::Call { f, .. } = e {
                todo.push(f.clone());
            }
        });
    }
    reached.sort_by(|a, b| a.name.cmp(&b.name));
    // a boxed f64 or a frame machine has no MSL form: such leaves stay on
    // the CPU
    let mut printable = true;
    for f in &reached {
        crate::lir::walk_stmts(&f.body, &mut |s| printable &= !matches!(s, S::Machine(_) | S::Fn(_)));
        crate::lir::walk_exprs(&f.body, &mut |e| printable &= !matches!(e, E::Flo(_)));
    }
    if !printable {
        return String::new();
    }
    // recursion runs to a depth the host picks (DEEP_LIMIT): a function
    // that can reach a guarded call takes its nesting depth `dl` by value,
    // a guarded call passes dl + 1, and a guarded function past the limit
    // faults (range.metal deep_faulted). The count stays in a register.
    let Some(frames) = leaf_frames(&reached) else {
        // recursion no guard bounds: such leaves stay on the CPU
        return String::new();
    };
    let is_guarded = |f: &FnDef| matches!(f.body.first(), Some(S::Do(E::Call { f, .. })) if f == "stack_guard");
    let guarded: HashSet<&str> = reached.iter().filter(|f| is_guarded(f)).map(|f| f.name.as_str()).collect();
    let leveled = reaching(&reached, &guarded);
    let mut fnret: HashMap<String, Ty> = LEAF_F32.iter().map(|f| (format!("m_{f}"), Ty::I64)).collect();
    for d in &reached {
        fnret.insert(d.name.clone(), d.ret);
    }
    let mut defs: Vec<FnDef> = Vec::new();
    for d in &reached {
        let mut d = (*d).clone();
        let level = leveled.contains(d.name.as_str());
        if level {
            d.params.insert(1, ("dl".into(), Ty::I64));
        }
        crate::lir::mutate_stmts(&mut d.body, &mut |s| {
            if let (Some(e), _) = s.parts_mut() {
                e.rewrite(&mut |e| {
                    if let E::Call { f, args, .. } = e {
                        if level && leveled.contains(f.as_str()) {
                            let dl = if guarded.contains(f.as_str()) { E::Bin(Bop::Add, Box::new(E::V("dl".into())), Box::new(E::Int(1, Ty::I64))) } else { E::V("dl".into()) };
                            args.insert(1, dl);
                        } else if LEAF_F32.contains(&f.as_str()) {
                            // binary32 in a leaf marks its index for the exact
                            // pass where the GPU's flushing may have changed it
                            *f = format!("m_{f}");
                            args.insert(0, E::V("fuel".into()));
                        }
                    }
                });
            }
            true
        });
        defs.push(d);
    }
    let mut out = format!("// mithril: frames {frames}\n");
    out.push_str(&tuple_structs(&reached));
    for d in &defs {
        let _ = writeln!(out, "{};", signature(d, Dialect::Msl));
    }
    for d in &defs {
        out.push_str(&function(d, &fnret, Dialect::Msl));
    }
    let _ = writeln!(out, "i64 prog_range_leaf(uint fid, i64 i, device const ulong *args, thread i64 *fuel) {{\nswitch (fid) {{");
    for r in fills {
        let args: String = (2..r.ints.len()).map(|k| if k == r.acc && r.kind != 0 { ", 0l".to_string() } else { format!(", (i64)args[{k}]") }).collect();
        let dl = if leveled.contains(format!("s_{}", r.fid).as_str()) { ", 0l" } else { "" };
        let _ = writeln!(out, "case {}: return s_{}(fuel{dl}, i, i + 1{args});", r.fid, r.fid);
    }
    let _ = writeln!(out, "default: return 0;\n}}\n}}");
    out
}

/// The binary32 operations a leaf runs through `m_*` (range.metal): fast
/// in hardware, exact on the indices they mark.
const LEAF_F32: [&str; 7] = ["f32_add", "f32_sub", "f32_mul", "f32_div", "f32_sqrt", "f32_lt", "f32_le"];

/// The functions that can reach a guarded one (the guarded ones included).
fn reaching<'a>(fns: &[&'a FnDef], guarded: &HashSet<&str>) -> HashSet<&'a str> {
    let mut out: HashSet<&str> = fns.iter().filter(|f| guarded.contains(f.name.as_str())).map(|f| f.name.as_str()).collect();
    loop {
        let before = out.len();
        for f in fns {
            let mut calls = false;
            crate::lir::walk_exprs(&f.body, &mut |e| calls |= matches!(e, E::Call { f, .. } if out.contains(f.as_str())));
            if calls {
                out.insert(f.name.as_str());
            }
        }
        if out.len() == before {
            return out;
        }
    }
}

/// Frames from a guarded function's entry to the next guarded call, at
/// most (a guarded function is two: its wrapper and its body), or `None`
/// when recursion passes no guard.
fn leaf_frames(fns: &[&FnDef]) -> Option<usize> {
    let by_name: HashMap<&str, &FnDef> = fns.iter().map(|f| (f.name.as_str(), *f)).collect();
    let guarded = |f: &FnDef| matches!(f.body.first(), Some(S::Do(E::Call { f, .. })) if f == "stack_guard");
    let callees = |f: &FnDef| {
        let mut out = Vec::new();
        crate::lir::walk_exprs(&f.body, &mut |e| {
            if let E::Call { f, .. } = e {
                if by_name.contains_key(f.as_str()) {
                    out.push(f.clone());
                }
            }
        });
        out
    };
    // the longest chain below `f` that stops at guarded callees
    fn depth<'a>(f: &'a str, by_name: &HashMap<&str, &'a FnDef>, guarded: &dyn Fn(&FnDef) -> bool, callees: &dyn Fn(&FnDef) -> Vec<String>, memo: &mut HashMap<String, Option<usize>>, open: &mut HashSet<String>) -> Option<usize> {
        if let Some(d) = memo.get(f) {
            return *d;
        }
        if !open.insert(f.to_string()) {
            return None;
        }
        let def = by_name[f];
        let mut best = 0;
        for c in callees(def) {
            if guarded(by_name[c.as_str()]) {
                continue;
            }
            best = best.max(depth(&c, by_name, guarded, callees, memo, open)?);
        }
        open.remove(f);
        let d = Some(best + 1);
        memo.insert(f.to_string(), d);
        d
    }
    let mut memo = HashMap::new();
    let mut most = 0;
    for f in fns {
        let d = depth(&f.name, &by_name, &guarded, &callees, &mut memo, &mut HashSet::new())?;
        most = most.max(d);
    }
    Some(most + 2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lir::{bin, do_, i64_, let_, p, set, v, Bop, Inline};

    fn def(name: &str, body: Vec<S>) -> FnDef {
        FnDef { name: name.into(), ctx: false, params: vec![("fuel".into(), Ty::RefI64), ("v0".into(), Ty::I64)], ret: Ty::I64, body, inline: Inline::Default, cold: false }
    }

    fn print(d: &FnDef, dialect: Dialect) -> String {
        let fnret = HashMap::from([(d.name.clone(), d.ret)]);
        function(d, &fnret, dialect)
    }

    /// A loop left from inside a switch, a counted loop and a stack guard,
    /// in both dialects.
    fn sample() -> FnDef {
        let switch = S::Switch(v("v0"), vec![(1, vec![S::Break]), (2, vec![set("s", bin(Bop::Add, v("s"), i64_(-9223372036854775807 - 1)))])], Some(vec![S::Continue]));
        def("s_7", vec![do_(p("stack_guard", vec![])), let_("s", Ty::I64, i64_(0)), S::Loop(vec![switch]), S::Ret(v("s"))])
    }

    #[test]
    fn msl_leaves_a_loop_without_goto() {
        let msl = print(&sample(), Dialect::Msl);
        assert!(!msl.contains("goto"), "{msl}");
        // the break inside the switch sets the loop's flag; after the
        // switch the flag leaves the loop
        assert!(msl.contains("bool brk_0 = false;") && msl.contains("{ brk_0 = true; break; }") && msl.contains("if (brk_0)"), "{msl}");
        // after a fault the loop stops
        assert!(msl.contains("if (deep_faulted(fuel))"), "{msl}");
        let cuda = print(&sample(), Dialect::Cuda);
        assert!(cuda.contains("goto loop_exit_0;") && !cuda.contains("brk_") && !cuda.contains("deep_faulted"), "{cuda}");
    }

    #[test]
    fn msl_spells_pointers_literals_and_the_guard() {
        let msl = print(&sample(), Dialect::Msl);
        // pointers name their address space; 64-bit literals have no `ll`
        assert!(msl.contains("((i64)9223372036854775808ul)") && !msl.contains("ll)"), "{msl}");
        // the guard compares the nesting depth passed by value
        assert!(msl.contains("i64 s_7(thread i64* fuel, i64 v0) {"), "{msl}");
        assert!(msl.contains("if (dl >= DEEP_LIMIT) { fuel[1] |= DEEP_FAULT; return (i64)0; }"), "{msl}");
        assert!(!msl.contains("stack_deep"), "{msl}");
        let cuda = print(&sample(), Dialect::Cuda);
        assert!(cuda.contains("__device__ i64 s_7(i64* fuel, i64 v0)") && cuda.contains("if (stack_deep()) return {};") && cuda.contains("(-9223372036854775808ll)"), "{cuda}");
    }

    #[test]
    fn msl_leaves_reach_the_fold_and_bound_the_stack() {
        let leaf = def("s_3", vec![let_("x", Ty::I64, E::Call { f: "s_4".into(), args: vec![v("fuel"), v("v0")], ctx: false }), let_("y", Ty::I64, p("f32_mul", vec![v("x"), v("x")])), S::Ret(v("y"))]);
        let callee = def("s_4", vec![do_(p("stack_guard", vec![])), S::Ret(E::Call { f: "s_4".into(), args: vec![v("fuel"), v("v0")], ctx: false })]);
        let unrelated = def("s_9", vec![S::Ret(v("v0"))]);
        let fill = crate::RangeFill { fid: 3, ints: vec![true, true, true, false], acc: 3, kind: 1 };
        let out = msl_leaves(&[leaf.clone(), callee.clone(), unrelated], &[fill]);
        // one level: s_3 above the guarded s_4
        assert!(out.starts_with("// mithril: frames 3\n"), "{out}");
        assert!(!out.contains("s_9"), "{out}");
        // both take the nesting depth; a guarded call passes one more
        assert!(out.contains("i64 s_3(thread i64* fuel, i64 dl, i64 v0)") && out.contains("s_4(fuel, ((i64)((u64)(dl) + (u64)(((i64)1ul)))), v0)"), "{out}");
        assert!(out.contains("if (dl >= DEEP_LIMIT)"), "{out}");
        // binary32 goes through the marking operations
        assert!(out.contains("m_f32_mul(fuel, x, x)"), "{out}");
        // a sum's accumulator starts from the identity; the depth from 0
        assert!(out.contains("case 3: return s_3(fuel, 0l, i, i + 1, (i64)args[2], 0l);"), "{out}");
        // recursion no guard bounds stays on the CPU
        let loose = def("s_4", vec![S::Ret(E::Call { f: "s_4".into(), args: vec![v("fuel"), v("v0")], ctx: false })]);
        assert_eq!(msl_leaves(&[leaf, loose], &[crate::RangeFill { fid: 3, ints: vec![true, true, true], acc: 2, kind: 0 }]), "");
        assert_eq!(msl_leaves(&[callee], &[]), "");
    }
}
