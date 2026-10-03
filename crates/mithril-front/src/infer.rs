//! Static binary32 typing. A monomorphic inference over the surface AST:
//! every variable, parameter, function result, constructor field, tuple
//! component and array element has one type, found by unification. The
//! sources of `f32` are `sqrt(x)` and `f32(n)`; it spreads through
//! assignments, operators, calls and returns. Where a value is `f32`, a
//! literal becomes its binary32 bit pattern and an operator the `f32_*`
//! builtin, so later stages see ints and the existing rules: `x + y` on
//! f32 *is* `f32_add(x, y)`. A module with no f32 comes back unchanged but
//! for f64 negation, which becomes `-1.0 * x`.
//!
//! Surface: `+ - * /`, unary `-`, `< <= > >= == !=` (IEEE: false on NaN,
//! `!=` true), `sqrt(x)`, `f32(n)` (int to nearest f32, exact below 2^24)
//! and `int(x)` (toward zero; NaN or |x| >= 2^32 gives 0). Mixing f32 with
//! an int variable is an error: convert with `f32(n)` or `int(x)`.

use crate::ast::*;
use crate::Diag;
use std::collections::{BTreeSet, HashMap};

#[derive(Clone, Debug, PartialEq)]
enum Ty {
    Var,
    F32,
    Int,
    Tup(Vec<usize>),
    Arr(usize),
    /// a constructor value or a closure
    Opaque,
    /// two shapes met where no f32 was involved (a dynamically typed
    /// program); absorbs everything, reports nothing
    Poison,
}

/// Helpers the elaborated code calls, written in the language itself; only
/// the ones used are added to the module.
const PRELUDE: &str = "
def __f32_eq(a, b):
    return f32_le(a, b) & f32_le(b, a)

def __f32_of_int(n):
    if n < 0:
        if 0 - n < 0:
            return 3741319168
        return __f32_of_int(0 - n) ^ 2147483648
    if n < 4294967296:
        return f32_from_u32(n)
    k = 0
    sticky = 0
    while n >= 4294967296:
        sticky = sticky | (n & 1)
        n = n >> 1
        k = k + 1
    return f32_mul(f32_from_u32(n | sticky), (127 + k) << 23)

def __int_of_f32(x):
    if f32_lt(x, 0) == 1:
        return 0 - __int_of_f32(x ^ 2147483648)
    e = ((x >> 23) & 255) - 127
    if e < 32:
        return f32_to_u32(x)
    if e >= 63:
        return 0
    return f32_to_u32(x - ((e - 31) << 23)) << (e - 31)
";

const SIGN: i64 = 1 << 31;

fn bits(v: f64) -> i64 {
    (v as f32).to_bits() as i64
}

fn int_bits(n: i64) -> i64 {
    (n as f32).to_bits() as i64
}

struct Infer<'m> {
    ty: Vec<Ty>,
    up: Vec<usize>,
    /// the class holds a float literal (an f64 value when it is not f32)
    flo: Vec<bool>,
    at: HashMap<*const Expr, usize>,
    fns: HashMap<&'m str, usize>,
    params: Vec<Vec<usize>>,
    ret: Vec<usize>,
    fields: HashMap<&'m str, Vec<usize>>,
    /// tuple projections whose tuple's width is not known yet
    projs: Vec<(usize, usize, usize)>,
    /// float literals written as the argument of `f32()` or `int()`
    conv: Vec<usize>,
    /// destructuring assignments: the tuple and the number of names
    unpacks: Vec<(usize, usize)>,
    cur: &'m str,
    err: Option<Diag>,
    used: BTreeSet<&'static str>,
}

impl<'m> Infer<'m> {
    fn node(&mut self, t: Ty) -> usize {
        self.ty.push(t);
        self.up.push(self.up.len());
        self.flo.push(false);
        self.up.len() - 1
    }

    fn find(&mut self, mut x: usize) -> usize {
        while self.up[x] != x {
            self.up[x] = self.up[self.up[x]];
            x = self.up[x];
        }
        x
    }

    fn is(&mut self, x: usize, t: &Ty) -> bool {
        let r = self.find(x);
        &self.ty[r] == t
    }

    fn unify(&mut self, a: usize, b: usize) {
        let (a, b) = (self.find(a), self.find(b));
        if a == b {
            return;
        }
        let (ta, tb) = (self.ty[a].clone(), self.ty[b].clone());
        let (keep, gone) = if ta == Ty::Var { (b, a) } else { (a, b) };
        self.up[gone] = keep;
        self.flo[keep] |= self.flo[gone];
        match (ta, tb) {
            (Ty::Var, _) | (_, Ty::Var) | (Ty::F32, Ty::F32) | (Ty::Int, Ty::Int) | (Ty::Opaque, Ty::Opaque) => {}
            (Ty::Tup(xs), Ty::Tup(ys)) if xs.len() == ys.len() => xs.into_iter().zip(ys).for_each(|(x, y)| self.unify(x, y)),
            (Ty::Arr(x), Ty::Arr(y)) => self.unify(x, y),
            (Ty::F32, t) | (t, Ty::F32) => {
                let what = match t {
                    Ty::Int => "an int",
                    Ty::Tup(_) => "a tuple",
                    Ty::Arr(_) => "an array",
                    Ty::Poison => "a value of mixed shape",
                    _ => "a constructor or closure",
                };
                self.err.get_or_insert(Diag::new(0, format!("in '{}': a value is used both as f32 and as {what} (convert with f32(n) or int(x))", self.cur)));
                self.ty[keep] = Ty::Poison;
            }
            _ => self.ty[keep] = Ty::Poison,
        }
    }

    fn fresh_is(&mut self, x: usize, t: Ty) {
        let n = self.node(t);
        self.unify(x, n);
    }

    fn expr(&mut self, e: &'m Expr, env: &mut HashMap<String, usize>) -> usize {
        let n = match e {
            // an int literal is an int; only as the direct operand of an
            // operator does it take its sibling's type (`x * 2` on f32)
            Expr::Int(_) => self.node(Ty::Int),
            Expr::Float(_) => {
                let n = self.node(Ty::Var);
                self.flo[n] = true;
                n
            }
            Expr::Bool(_) | Expr::Not(_) | Expr::Cmp(..) => {
                match e {
                    Expr::Not(a) => {
                        self.expr(a, env);
                    }
                    Expr::Cmp(_, a, b) => {
                        let (x, y) = (self.operand(a, env), self.operand(b, env));
                        self.unify(x, y);
                    }
                    _ => {}
                }
                self.node(Ty::Int)
            }
            Expr::Var(v) => match env.get(v) {
                Some(&n) => n,
                None if self.fns.contains_key(v.as_str()) => self.node(Ty::Opaque),
                None => self.node(Ty::Var),
            },
            Expr::Neg(a) => self.expr(a, env),
            Expr::Bin(_, a, b) | Expr::IfExp(_, a, b) => {
                if let Expr::IfExp(c, ..) = e {
                    self.expr(c, env);
                }
                let (x, y) = (self.operand(a, env), self.operand(b, env));
                self.unify(x, y);
                x
            }
            Expr::Bool2(_, a, b) => {
                self.expr(a, env);
                self.expr(b, env);
                self.node(Ty::Int)
            }
            Expr::Tuple(items) => {
                let xs = items.iter().map(|i| self.expr(i, env)).collect();
                self.node(Ty::Tup(xs))
            }
            Expr::Index(b, i) => {
                let t = self.expr(b, env);
                let r = self.node(Ty::Var);
                if let Expr::Int(k) = **i {
                    self.projs.push((t, k.max(0) as usize, r));
                }
                r
            }
            Expr::Lambda(ps, body) => {
                let mut inner = env.clone();
                for p in ps {
                    let n = self.node(Ty::Var);
                    inner.insert(p.clone(), n);
                }
                self.expr(body, &mut inner);
                self.node(Ty::Opaque)
            }
            Expr::Call(f, args) => {
                let xs: Vec<usize> = args.iter().map(|a| self.expr(a, env)).collect();
                // a float literal written as the argument of f32() or int()
                // is f32 (a float variable keeps its own type)
                if matches!((args.as_slice(), f.as_str()), ([Expr::Float(_)], "f32" | "int")) && self.builtin(f, env) {
                    self.conv.push(xs[0]);
                }
                self.call(f, &xs, env)
            }
        };
        self.at.insert(e as *const Expr, n);
        n
    }

    /// An operand: an int literal here takes the other operand's type.
    fn operand(&mut self, e: &'m Expr, env: &mut HashMap<String, usize>) -> usize {
        if let Expr::Int(_) = e {
            let n = self.node(Ty::Var);
            self.at.insert(e as *const Expr, n);
            return n;
        }
        self.expr(e, env)
    }

    fn call(&mut self, f: &str, xs: &[usize], env: &HashMap<String, usize>) -> usize {
        if env.contains_key(f) {
            return self.node(Ty::Var);
        }
        if let Some(fs) = self.fields.get(f).cloned() {
            xs.iter().zip(fs).for_each(|(&x, y)| self.unify(x, y));
            return self.node(Ty::Opaque);
        }
        if let Some(&g) = self.fns.get(f) {
            for (&x, y) in xs.iter().zip(self.params[g].clone()) {
                self.unify(x, y);
            }
            return self.ret[g];
        }
        let int = |s: &mut Self, x: Option<&usize>| x.iter().for_each(|&&x| s.fresh_is(x, Ty::Int));
        match (f, xs.len()) {
            ("sqrt", 1) => {
                self.fresh_is(xs[0], Ty::F32);
                xs[0]
            }
            ("f32", 1) => self.node(Ty::F32),
            ("int", 1) => self.node(Ty::Int),
            ("array_len", 1) => self.node(Ty::Int),
            ("array_new", 2) => {
                int(self, xs.first());
                self.node(Ty::Arr(xs[1]))
            }
            ("array_get", 2) | ("array_set", 3) => {
                let el = self.node(Ty::Var);
                let arr = self.node(Ty::Arr(el));
                self.unify(xs[0], arr);
                int(self, xs.get(1));
                if let Some(&v) = xs.get(2) {
                    self.unify(v, el);
                    return arr;
                }
                el
            }
            _ => self.node(Ty::Var),
        }
    }

    /// The one type of local `v` in this function.
    fn var(&mut self, v: &str, env: &mut HashMap<String, usize>) -> usize {
        if let Some(&x) = env.get(v) {
            return x;
        }
        let x = self.node(Ty::Var);
        env.insert(v.to_string(), x);
        x
    }

    fn stmts(&mut self, ss: &'m [Stmt], env: &mut HashMap<String, usize>) {
        for s in ss {
            match s {
                Stmt::Assign(v, e) => {
                    let t = self.expr(e, env);
                    let x = self.var(v, env);
                    self.unify(x, t);
                    // `a, b = e` (parse.rs): the hidden name ends in the count
                    if let Some(n) = v.strip_prefix("__tuple").and_then(|r| r.rsplit('_').next()).and_then(|n| n.parse().ok()) {
                        let xs = (0..n).map(|_| self.node(Ty::Var)).collect();
                        let tup = self.node(Ty::Tup(xs));
                        self.unify(x, tup);
                        self.unpacks.push((x, n));
                    }
                }
                Stmt::Return(e) => {
                    let t = self.expr(e, env);
                    let r = self.ret[self.fns[self.cur]];
                    self.unify(r, t);
                }
                Stmt::ExprStmt(e) => {
                    self.expr(e, env);
                }
                Stmt::If(c, a, b) => {
                    self.expr(c, env);
                    self.stmts(a, env);
                    self.stmts(b, env);
                }
                Stmt::While(c, body) => {
                    self.expr(c, env);
                    self.stmts(body, env);
                }
                Stmt::For(v, e, body, _) => {
                    let t = self.expr(e, env);
                    let x = self.var(v, env);
                    self.fresh_is(x, Ty::Int);
                    self.fresh_is(t, Ty::Int);
                    self.stmts(body, env);
                }
                Stmt::Match(e, arms) => {
                    let t = self.expr(e, env);
                    for (p, body) in arms {
                        if p.as_int_lit().is_some() {
                            self.fresh_is(t, Ty::Int);
                        } else if let Some(fs) = self.fields.get(p.ctor.as_str()).cloned() {
                            for (b, f) in p.binds.iter().zip(fs) {
                                let x = self.var(b, env);
                                self.unify(x, f);
                            }
                        }
                        self.stmts(body, env);
                    }
                }
            }
        }
    }

    /// Projections of tuples whose width became known, until none moves.
    fn settle(&mut self) {
        loop {
            let mut moved = false;
            for (t, k, r) in std::mem::take(&mut self.projs) {
                let root = self.find(t);
                match self.ty[root].clone() {
                    Ty::Var => self.projs.push((t, k, r)),
                    Ty::Tup(xs) => {
                        if let Some(&x) = xs.get(k) {
                            self.unify(x, r);
                        }
                        moved = true;
                    }
                    _ => moved = true,
                }
            }
            if !moved {
                return;
            }
        }
    }

    /// The class of `e` holds a float literal (f64 unless it is f32).
    fn flo_of(&mut self, e: &Expr) -> bool {
        let r = self.find(self.at[&(e as *const Expr)]);
        self.flo[r]
    }

    fn f32(&mut self, e: &Expr) -> bool {
        let n = self.at[&(e as *const Expr)];
        self.is(n, &Ty::F32)
    }

    fn helper(&mut self, f: &'static str, args: Vec<Expr>) -> Expr {
        self.used.insert(f);
        Expr::Call(f.into(), args)
    }

    fn ex(&mut self, e: &Expr, env: &HashMap<String, usize>) -> Result<Expr, Diag> {
        let b = |x: Expr| Box::new(x);
        let is32 = self.f32(e);
        Ok(match e {
            Expr::Int(v) if is32 => Expr::Int(int_bits(*v)),
            Expr::Float(v) if is32 => Expr::Int(bits(*v)),
            Expr::Int(_) | Expr::Float(_) | Expr::Bool(_) | Expr::Var(_) => e.clone(),
            Expr::Neg(a) => {
                let x = self.ex(a, env)?;
                let r = self.find(self.at[&(e as *const Expr)]);
                match (&self.ty[r], self.flo[r]) {
                    (Ty::F32, _) => Expr::Bin(BinOp::BitXor, b(x), b(Expr::Int(SIGN))),
                    // an f64 (a class of float literals only)
                    (Ty::Var, true) => Expr::Bin(BinOp::Mul, b(Expr::Float(-1.0)), b(x)),
                    (_, true) => return Err(Diag::new(0, format!("in '{}': negation of a value used both as an int and as an f64", self.cur))),
                    _ => Expr::Neg(b(x)),
                }
            }
            Expr::Bin(op, x, y) => {
                let (x, y) = (self.ex(x, env)?, self.ex(y, env)?);
                if !is32 {
                    return Ok(Expr::Bin(*op, b(x), b(y)));
                }
                let f = match op {
                    BinOp::Add => "f32_add",
                    BinOp::Sub => "f32_sub",
                    BinOp::Mul => "f32_mul",
                    BinOp::Div => "f32_div",
                    _ => return Err(Diag::new(0, format!("in '{}': operator {op:?} is not defined on f32", self.cur))),
                };
                Expr::Call(f.into(), vec![x, y])
            }
            Expr::Cmp(op, x0, y0) => {
                let on32 = self.f32(x0);
                let (x, y) = (self.ex(x0, env)?, self.ex(y0, env)?);
                if !on32 {
                    return Ok(Expr::Cmp(*op, b(x), b(y)));
                }
                let (f, args, want) = match op {
                    CmpOp::Lt => ("f32_lt", vec![x, y], CmpOp::Ne),
                    CmpOp::Gt => ("f32_lt", vec![y, x], CmpOp::Ne),
                    CmpOp::Le => ("f32_le", vec![x, y], CmpOp::Ne),
                    CmpOp::Ge => ("f32_le", vec![y, x], CmpOp::Ne),
                    CmpOp::Eq => ("__f32_eq", vec![x, y], CmpOp::Ne),
                    CmpOp::Ne => ("__f32_eq", vec![x, y], CmpOp::Eq),
                };
                let call = if f.starts_with("__") { self.helper("__f32_eq", args) } else { Expr::Call(f.into(), args) };
                Expr::Cmp(want, b(call), b(Expr::Int(0)))
            }
            Expr::Call(f, args) if args.len() == 1 && self.builtin(f, env) && ["sqrt", "f32", "int"].contains(&f.as_str()) => {
                let a = &args[0];
                let on32 = self.f32(a);
                let x = self.ex(a, env)?;
                match f.as_str() {
                    "sqrt" => Expr::Call("f32_sqrt".into(), vec![x]),
                    "int" if on32 => self.helper("__int_of_f32", vec![x]),
                    _ if on32 => x,
                    _ if self.flo_of(a) => return Err(Diag::new(0, format!("in '{}': {f}() of an f64 value (write the literal as f32, or keep the value f32)", self.cur))),
                    "int" => x,
                    _ => self.helper("__f32_of_int", vec![x]),
                }
            }
            Expr::Call(f, args) => Expr::Call(f.clone(), args.iter().map(|a| self.ex(a, env)).collect::<Result<_, _>>()?),
            Expr::Tuple(items) => Expr::Tuple(items.iter().map(|a| self.ex(a, env)).collect::<Result<_, _>>()?),
            Expr::Index(t, i) => Expr::Index(b(self.ex(t, env)?), i.clone()),
            Expr::Not(a) => Expr::Not(b(self.ex(a, env)?)),
            Expr::Bool2(op, x, y) => Expr::Bool2(*op, b(self.ex(x, env)?), b(self.ex(y, env)?)),
            Expr::IfExp(c, x, y) => Expr::IfExp(b(self.ex(c, env)?), b(self.ex(x, env)?), b(self.ex(y, env)?)),
            Expr::Lambda(ps, body) => {
                let mut inner = env.clone();
                ps.iter().for_each(|p| {
                    inner.insert(p.clone(), 0);
                });
                Expr::Lambda(ps.clone(), b(self.ex(body, &inner)?))
            }
        })
    }

    /// `f` names the language's builtin here: no local, function or
    /// constructor of that name shadows it.
    fn builtin(&self, f: &str, env: &HashMap<String, usize>) -> bool {
        !env.contains_key(f) && !self.fns.contains_key(f) && !self.fields.contains_key(f)
    }

    fn st(&mut self, ss: &[Stmt], env: &mut HashMap<String, usize>) -> Result<Vec<Stmt>, Diag> {
        let mut out = Vec::with_capacity(ss.len());
        for s in ss {
            out.push(match s {
                Stmt::Assign(v, e) => {
                    let x = self.ex(e, env)?;
                    env.insert(v.clone(), 0);
                    Stmt::Assign(v.clone(), x)
                }
                Stmt::Return(e) => Stmt::Return(self.ex(e, env)?),
                Stmt::ExprStmt(e) => Stmt::ExprStmt(self.ex(e, env)?),
                Stmt::If(c, a, b) => Stmt::If(self.ex(c, env)?, self.st(a, env)?, self.st(b, env)?),
                Stmt::While(c, body) => Stmt::While(self.ex(c, env)?, self.st(body, env)?),
                Stmt::For(v, e, body, f) => {
                    let e = self.ex(e, env)?;
                    env.insert(v.clone(), 0);
                    Stmt::For(v.clone(), e, self.st(body, env)?, f.clone())
                }
                Stmt::Match(e, arms) => {
                    let e = self.ex(e, env)?;
                    let mut out = Vec::new();
                    for (p, body) in arms {
                        p.binds.iter().for_each(|b| {
                            env.insert(b.clone(), 0);
                        });
                        out.push((p.clone(), self.st(body, env)?));
                    }
                    Stmt::Match(e, out)
                }
            });
        }
        Ok(out)
    }
}

/// Type the module and lower its f32 operations to the builtins.
pub fn elaborate(m: &mut Module) -> Result<(), Diag> {
    let src: &Module = &m.clone();
    let mut s = Infer {
        ty: Vec::new(),
        up: Vec::new(),
        flo: Vec::new(),
        at: HashMap::new(),
        fns: HashMap::new(),
        params: Vec::new(),
        ret: Vec::new(),
        fields: HashMap::new(),
        projs: Vec::new(),
        conv: Vec::new(),
        unpacks: Vec::new(),
        cur: "",
        err: None,
        used: BTreeSet::new(),
    };
    for (i, f) in src.fns.iter().enumerate() {
        s.fns.insert(&f.name, i);
        let ps = f.params.iter().map(|_| s.node(Ty::Var)).collect();
        s.params.push(ps);
        let r = s.node(Ty::Var);
        s.ret.push(r);
    }
    for d in &src.datas {
        for (c, fs) in &d.ctors {
            let ns = fs.iter().map(|_| s.node(Ty::Var)).collect();
            s.fields.insert(c, ns);
        }
    }
    for (i, f) in src.fns.iter().enumerate() {
        s.cur = &f.name;
        let mut env: HashMap<String, usize> = f.params.iter().cloned().zip(s.params[i].clone()).collect();
        s.stmts(&f.body, &mut env);
    }
    s.settle();
    for x in std::mem::take(&mut s.conv) {
        let r = s.find(x);
        if s.flo[r] && s.ty[r] == Ty::Var {
            s.fresh_is(x, Ty::F32);
        }
    }
    s.settle();
    for (x, n) in std::mem::take(&mut s.unpacks) {
        let r = s.find(x);
        if !matches!(&s.ty[r], Ty::Tup(xs) if xs.len() == n) {
            s.err.get_or_insert(Diag::new(0, format!("cannot unpack a value into {n} names: it is not a tuple of {n}")));
        }
    }
    if let Some(d) = s.err.take() {
        return Err(d);
    }
    for (i, f) in src.fns.iter().enumerate() {
        s.cur = &f.name;
        // the names in scope as the first pass saw them: the parameters,
        // then each local from its first assignment on (only whether a
        // name is local matters here)
        let mut env: HashMap<String, usize> = f.params.iter().map(|p| (p.clone(), 0)).collect();
        m.fns[i].body = s.st(&f.body, &mut env)?;
    }
    if !s.used.is_empty() {
        let pre = crate::parse::parse_module(&crate::lex::lex(PRELUDE)?)?;
        m.fns.extend(pre.fns.into_iter().filter(|f| s.used.contains(f.name.as_str())));
    }
    Ok(())
}
