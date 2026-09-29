//! Monomorphic type inference over first-order Core.
//!
//! Types: `Int` (i56 immediates), `Flo` (boxed f64), `Tup(n)` (0xFFF tuple of
//! known arity), `Adt(d)` (a datatype = the union-find class of the ctors
//! that appear together in matches). Unification over a union-find; no
//! polymorphism, no generalization — every benchmark-relevant module is
//! monomorphic, and anything that fails to resolve simply stays `Dyn`
//! (emission falls back to the tagged helpers). `Int` is load-bearing:
//! the borrow analysis takes an integer binder as an immediate that never escapes.
//!
//! Global variables: one per function parameter, one per function return,
//! one per (ctor, field). Per-function locals get vars during the walk.

use mithril_front::core::{Core, CoreModule, UNREACHABLE_CTOR};

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Ty {
    Dyn,
    Int,
    Flo,
    Tup(u32),
    Adt(u32), // class id (representative ctor)
    /// an array; `true` when its elements are proven `Int` (then reads and
    /// writes need no reference counting of elements)
    Arr(bool),
}

/// A tuple's layout: each component a scalar (`None`) or a nested tuple.
/// Native code holds a tuple as its leaves in order (`width` of them).
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct Shape(pub Vec<Option<Shape>>);

impl Shape {
    pub fn flat(k: usize) -> Shape {
        Shape(vec![None; k])
    }
    pub fn width(&self) -> usize {
        self.0.iter().map(|c| c.as_ref().map_or(1, Shape::width)).sum()
    }
    /// The first leaf of component `i`.
    pub fn offset(&self, i: usize) -> usize {
        self.0[..i].iter().map(|c| c.as_ref().map_or(1, Shape::width)).sum()
    }
    pub fn is_flat(&self) -> bool {
        self.0.iter().all(Option::is_none)
    }
}

pub(crate) struct Types {
    /// ctor -> canonical ADT class (ctors co-matched share a class)
    pub class_of: Vec<u32>,
    /// fn -> parameter types
    pub params: Vec<Vec<Ty>>,
    /// fn -> return type
    pub ret: Vec<Ty>,
    /// (ctor, field) -> type
    pub field: Vec<Vec<Ty>>,
    /// fn -> local var -> type (indexed by var id; Dyn when out of range)
    pub locals: Vec<Vec<Ty>>,
    /// fn -> the layout of each tuple parameter, and of a tuple result
    pub pshape: Vec<Vec<Option<Shape>>>,
    pub rshape: Vec<Option<Shape>>,
}

impl Types {
    pub fn var(&self, fid: usize, v: u32) -> Ty {
        self.locals[fid].get(v as usize).copied().unwrap_or(Ty::Dyn)
    }
    pub fn expr(&self, fid: usize, e: &Core) -> Ty {
        match e {
            Core::Num(_) | Core::Cmp(..) => Ty::Int,
            Core::Flo(_) => Ty::Flo,
            Core::Var(i) => self.var(fid, *i),
            Core::Op2(_, a, _) => self.expr(fid, a), // operands share the type
            Core::If(_, t, _) => self.expr(fid, t),
            Core::Let(_, _, b) => self.expr(fid, b),
            Core::Call(g, _) => self.ret[*g as usize],
            Core::Ctor(c, _) | Core::Reuse(_, c, _) => self.field.get(*c as usize).map(|_| Ty::Adt(*c)).unwrap_or(Ty::Dyn),
            Core::Tuple(xs) => Ty::Tup(xs.len() as u32),
            Core::Prim(p, xs) => match p {
                mithril_front::core::Prim::ArrNew => Ty::Arr(self.expr(fid, &xs[1]) == Ty::Int),
                mithril_front::core::Prim::ArrSet => match self.expr(fid, &xs[0]) {
                    Ty::Arr(i) => Ty::Arr(i),
                    _ => Ty::Arr(false),
                },
                mithril_front::core::Prim::ArrLen => Ty::Int,
                mithril_front::core::Prim::ArrGet => match self.expr(fid, &xs[0]) {
                    Ty::Arr(true) => Ty::Int,
                    _ => Ty::Dyn,
                },
                _ => Ty::Int, // binary32 primitives
            },
            // not needed by emitters (a projection's type is refined during
            // inference; match arms carry theirs)
            Core::Proj(..) | Core::Match(..) | Core::Lam(..) | Core::App(..) => Ty::Dyn,
        }
    }
}

// ---- inference machinery ----

#[derive(Clone, Copy, PartialEq)]
enum Node {
    Free,
    Int,
    Flo,
    /// a k-tuple; its component tyvars are `Inf::tups[.1]`
    Tup(u32, u32),
    Adt(u32),
    /// an array; its element type is the tyvar
    Arr(u32),
    /// a closure (a tagged runtime value; never an immediate, never an
    /// ADT: a field holding both ints and closures poisons to Dyn)
    Fun,
    Link(u32),
}

struct Uf {
    n: Vec<Node>,
}

impl Uf {
    fn fresh(&mut self) -> u32 {
        self.n.push(Node::Free);
        (self.n.len() - 1) as u32
    }
    fn find(&mut self, mut i: u32) -> u32 {
        while let Node::Link(j) = self.n[i as usize] {
            i = j;
        }
        i
    }
    fn set(&mut self, i: u32, k: Node) {
        let r = self.find(i);
        match (self.n[r as usize], k) {
            (Node::Free, _) => self.n[r as usize] = k,
            (a, b) if a == b => {}
            _ => self.n[r as usize] = Node::Adt(u32::MAX), // conflict: poison (reads as Dyn)
        }
    }
    fn union(&mut self, a: u32, b: u32) {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra == rb {
            return;
        }
        match (self.n[ra as usize], self.n[rb as usize]) {
            (Node::Free, _) => self.n[ra as usize] = Node::Link(rb),
            (_, Node::Free) => self.n[rb as usize] = Node::Link(ra),
            (a_, b_) if a_ == b_ => self.n[ra as usize] = Node::Link(rb),
            _ => {
                // conflict: poison both
                self.n[ra as usize] = Node::Adt(u32::MAX);
                self.n[rb as usize] = Node::Link(ra);
            }
        }
    }
    fn read(&mut self, i: u32) -> Ty {
        let r = self.find(i);
        match self.n[r as usize] {
            Node::Int => Ty::Int,
            Node::Flo => Ty::Flo,
            Node::Tup(k, _) => Ty::Tup(k),
            Node::Adt(u32::MAX) => Ty::Dyn,
            Node::Adt(c) => Ty::Adt(c),
            Node::Arr(e) => Ty::Arr(self.read(e) == Ty::Int),
            _ => Ty::Dyn,
        }
    }
}

struct Inf {
    uf: Uf,
    fparam: Vec<Vec<u32>>,
    fret: Vec<u32>,
    /// (ctor, field) -> tyvar; also one "class var" per ctor unified across
    /// co-matched ctors is implicit: Adt(c) nodes unify by ctor-class map
    cfield: Vec<Vec<u32>>,
    /// ctor -> class representative (union-find over ctor ids)
    cclass: Vec<u32>,
    /// component tyvars of each tuple node
    tups: Vec<Vec<u32>>,
    /// projections whose base was not yet known to be a tuple: (base, i,
    /// result), resolved after each pass
    pending: Vec<(u32, usize, u32)>,
}

impl Inf {
    fn cfind(&mut self, mut c: u32) -> u32 {
        while self.cclass[c as usize] != c {
            c = self.cclass[c as usize];
        }
        c
    }
    fn cunion(&mut self, a: u32, b: u32) {
        let (ra, rb) = (self.cfind(a), self.cfind(b));
        if ra != rb {
            self.cclass[ra.max(rb) as usize] = ra.min(rb);
        }
    }

    fn adt(&mut self, c: u32) -> Node {
        let r = self.cfind(c);
        Node::Adt(r)
    }

    /// Two ADT nodes meeting in one value: their ctors belong to one
    /// datatype, so the classes merge (not a conflict).
    fn merge_adts(&mut self, a: u32, b: u32) -> bool {
        let (ra, rb) = (self.uf.find(a), self.uf.find(b));
        match (self.uf.n[ra as usize], self.uf.n[rb as usize]) {
            (Node::Adt(x), Node::Adt(y)) if x != u32::MAX && y != u32::MAX => {
                self.cunion(x, y);
                let k = self.adt(x);
                self.uf.n[ra as usize] = k;
                self.uf.n[rb as usize] = k;
                true
            }
            _ => false,
        }
    }

    fn unify(&mut self, a: u32, b: u32) {
        self.merge_adts(a, b);
        // arrays unify their element types, tuples their components
        let (ra, rb) = (self.uf.find(a), self.uf.find(b));
        if ra != rb {
            match (self.uf.n[ra as usize], self.uf.n[rb as usize]) {
                (Node::Arr(x), Node::Arr(y)) => {
                    self.uf.n[ra as usize] = Node::Link(rb);
                    self.unify(x, y);
                    return;
                }
                (Node::Tup(k, c), Node::Tup(l, d)) if k == l => {
                    self.uf.n[ra as usize] = Node::Link(rb);
                    for i in 0..k as usize {
                        let (x, y) = (self.tups[c as usize][i], self.tups[d as usize][i]);
                        self.unify(x, y);
                    }
                    return;
                }
                _ => {}
            }
        }
        self.uf.union(a, b);
    }

    /// The tyvar of component `i` of a value typed `tb`.
    fn proj(&mut self, tb: u32, i: usize) -> u32 {
        let r = self.uf.find(tb);
        if let Node::Tup(k, c) = self.uf.n[r as usize] {
            if i < k as usize {
                return self.tups[c as usize][i];
            }
        }
        let res = self.uf.fresh();
        self.pending.push((tb, i, res));
        res
    }

    /// The native layout of a tuple-typed tyvar: every leaf an int (native
    /// code holds leaves as i64s), nesting bounded (a type that reaches
    /// itself has no layout). `None` when it has no such layout.
    fn shape(&mut self, v: u32) -> Option<Shape> {
        match self.leaf(v, 0)? {
            Some(sh) => Some(sh),
            None => None, // an int, not a tuple
        }
    }

    /// `Some(None)`: an int leaf; `Some(Some(s))`: a tuple of layout `s`;
    /// `None`: neither (a non-int leaf, or nesting past the bound).
    fn leaf(&mut self, v: u32, depth: u32) -> Option<Option<Shape>> {
        let r = self.uf.find(v);
        match self.uf.n[r as usize] {
            Node::Int => Some(None),
            Node::Tup(_, c) if depth < 8 => {
                let comps = self.tups[c as usize].clone();
                let leaves: Option<Vec<Option<Shape>>> = comps.into_iter().map(|t| self.leaf(t, depth + 1)).collect();
                Some(Some(Shape(leaves?)))
            }
            _ => None,
        }
    }

    /// Resolve projections recorded before their base was known.
    fn settle(&mut self) {
        for (b, i, res) in std::mem::take(&mut self.pending) {
            let r = self.uf.find(b);
            if let Node::Tup(k, c) = self.uf.n[r as usize] {
                if i < k as usize {
                    let t = self.tups[c as usize][i];
                    self.unify(res, t);
                }
            }
        }
    }

    /// A fresh tyvar of shape `k`.
    fn node(&mut self, k: Node) -> u32 {
        let t = self.uf.fresh();
        self.uf.n[t as usize] = k;
        t
    }

    /// A fresh array node with element tyvar `e`.
    fn arr_of(&mut self, e: u32) -> u32 {
        self.node(Node::Arr(e))
    }

    fn int(&mut self) -> u32 {
        self.node(Node::Int)
    }

    fn set_adt(&mut self, t: u32, k: Node) {
        let r = self.uf.find(t);
        if let (Node::Adt(x), Node::Adt(y)) = (self.uf.n[r as usize], k) {
            if x != u32::MAX && y != u32::MAX {
                self.cunion(x, y);
                self.uf.n[r as usize] = self.adt(x);
                return;
            }
        }
        self.uf.set(t, k);
    }

    /// Type of expression `e`; unifies as it walks. `env[v]` = tyvar.
    fn walk(&mut self, fid: u32, e: &Core, env: &mut Vec<u32>) -> u32 {
        match e {
            Core::Num(_) => self.int(),
            Core::Flo(_) => self.node(Node::Flo),
            Core::Var(i) => env[*i as usize],
            Core::Op2(_, a, b) => {
                let ta = self.walk(fid, a, env);
                let tb = self.walk(fid, b, env);
                self.unify(ta, tb);
                ta
            }
            Core::Cmp(_, a, b) => {
                let ta = self.walk(fid, a, env);
                let tb = self.walk(fid, b, env);
                self.unify(ta, tb);
                self.int()
            }
            Core::If(c, x, y) => {
                let tc = self.walk(fid, c, env);
                self.uf.set(tc, Node::Int);
                let tx = self.walk(fid, x, env);
                let ty = self.walk(fid, y, env);
                self.unify(tx, ty);
                tx
            }
            Core::Let(x, r, b) => {
                let tr = self.walk(fid, r, env);
                bind(env, *x, tr);
                self.walk(fid, b, env)
            }
            Core::Call(g, args) => {
                for (j, a) in args.iter().enumerate() {
                    let ta = self.walk(fid, a, env);
                    let pj = self.fparam[*g as usize][j];
                    self.unify(ta, pj);
                }
                self.fret[*g as usize]
            }
            Core::Ctor(c, args) | Core::Reuse(_, c, args) => {
                if *c == UNREACHABLE_CTOR {
                    return self.uf.fresh();
                }
                for (j, a) in args.iter().enumerate() {
                    let ta = self.walk(fid, a, env);
                    let fj = self.cfield[*c as usize][j];
                    self.unify(ta, fj);
                }
                let t = self.uf.fresh();
                let k = self.adt(*c);
                self.set_adt(t, k);
                t
            }
            Core::Tuple(xs) => {
                let comps: Vec<u32> = xs.iter().map(|a| self.walk(fid, a, env)).collect();
                self.tups.push(comps);
                self.node(Node::Tup(xs.len() as u32, (self.tups.len() - 1) as u32))
            }
            Core::Proj(b, i) => {
                let tb = self.walk(fid, b, env);
                self.proj(tb, *i)
            }
            // closures: untyped here (Dyn); the body is walked so its
            // first-order parts get their types
            Core::Lam(x, b) => {
                let tx = self.uf.fresh();
                bind(env, *x, tx);
                let _ = self.walk(fid, b, env);
                self.node(Node::Fun)
            }
            Core::App(f, a) => {
                let tf = self.walk(fid, f, env);
                let fun = self.node(Node::Fun);
                self.unify(tf, fun);
                let _ = self.walk(fid, a, env);
                self.uf.fresh()
            }
            Core::Prim(p, args) => {
                use mithril_front::core::Prim;
                let ts: Vec<u32> = args.iter().map(|a| self.walk(fid, a, env)).collect();
                match p {
                    Prim::ArrNew => {
                        let n = self.int();
                        self.unify(ts[0], n);
                        self.arr_of(ts[1])
                    }
                    Prim::ArrGet => {
                        let e = self.uf.fresh();
                        let a = self.arr_of(e);
                        self.unify(ts[0], a);
                        let i = self.int();
                        self.unify(ts[1], i);
                        e
                    }
                    Prim::ArrSet => {
                        let a = self.arr_of(ts[2]);
                        self.unify(ts[0], a);
                        let i = self.int();
                        self.unify(ts[1], i);
                        ts[0]
                    }
                    Prim::ArrLen => {
                        let e = self.uf.fresh();
                        let a = self.arr_of(e);
                        self.unify(ts[0], a);
                        self.int()
                    }
                    // binary32 on bit patterns: ints in, an int out
                    _ => {
                        for t in &ts {
                            let i = self.int();
                            self.unify(*t, i);
                        }
                        self.int()
                    }
                }
            }
            Core::Match(s, arms) => {
                let ts = self.walk(fid, s, env);
                let arms: Vec<_> = arms.iter().filter(|(c, ..)| *c != UNREACHABLE_CTOR).collect();
                // unify scrutinee with each arm ctor's class; co-matched
                // ctors join one class
                let mut first: Option<u32> = None;
                for (c, _, _) in &arms {
                    if let Some(f) = first {
                        self.cunion(f, *c);
                    } else {
                        first = Some(*c);
                    }
                }
                if let Some(f) = first {
                    let k = self.adt(f);
                    self.set_adt(ts, k);
                }
                let mut tout: Option<u32> = None;
                for (c, binders, body) in &arms {
                    for (j, bv) in binders.iter().enumerate() {
                        bind(env, *bv, self.cfield[*c as usize][j]);
                    }
                    let tb = self.walk(fid, body, env);
                    if let Some(o) = tout {
                        self.unify(o, tb);
                    } else {
                        tout = Some(tb);
                    }
                }
                tout.unwrap_or_else(|| self.uf.fresh())
            }
        }
    }
}

/// `env[x] = t`, growing `env` (unbound slots are `u32::MAX`).
fn bind(env: &mut Vec<u32>, x: u32, t: u32) {
    if env.len() <= x as usize {
        env.resize(x as usize + 1, u32::MAX);
    }
    env[x as usize] = t;
}

/// Infer module types. Two passes over every body (the second lets sigs
/// settled late propagate), then a readout.
pub(crate) fn infer(m: &CoreModule) -> Types {
    let nf = m.fns.len();
    let mut inf = Inf {
        uf: Uf { n: Vec::new() },
        fparam: Vec::new(),
        fret: Vec::new(),
        cfield: Vec::new(),
        cclass: (0..m.ctors.len() as u32).collect(),
        tups: Vec::new(),
        pending: Vec::new(),
    };
    for f in &m.fns {
        let ps = (0..f.arity).map(|_| inf.uf.fresh()).collect();
        inf.fparam.push(ps);
        inf.fret.push(inf.uf.fresh());
    }
    for (_, ar) in &m.ctors {
        let fs = (0..*ar).map(|_| inf.uf.fresh()).collect();
        inf.cfield.push(fs);
    }
    for _pass in 0..2 {
        for (fid, f) in m.fns.iter().enumerate() {
            let mut env: Vec<u32> = inf.fparam[fid].clone();
            let tr = inf.walk(fid as u32, &f.body, &mut env);
            let fr = inf.fret[fid];
            inf.unify(tr, fr);
        }
        inf.settle();
    }
    // readout (locals need a third walk capturing every Let/binder var)
    let mut locals: Vec<Vec<Ty>> = Vec::with_capacity(nf);
    for (fid, f) in m.fns.iter().enumerate() {
        let mut env: Vec<u32> = inf.fparam[fid].clone();
        let _ = inf.walk(fid as u32, &f.body, &mut env);
        let tys = env
            .iter()
            .map(|&v| if v == u32::MAX { Ty::Dyn } else { inf.uf.read(v) })
            .collect();
        locals.push(tys);
    }
    let class_of: Vec<u32> = (0..m.ctors.len() as u32).map(|c| inf.cfind(c)).collect();
    let canon = |t: Ty| if let Ty::Adt(c) = t { Ty::Adt(class_of[c as usize]) } else { t };
    let mut rd = |vs: &[u32]| -> Vec<Ty> { vs.iter().map(|&v| canon(inf.uf.read(v))).collect() };
    let params = inf.fparam.iter().map(|ps| rd(ps)).collect();
    let ret = rd(&inf.fret);
    let field = inf.cfield.iter().map(|fs| rd(fs)).collect();
    let locals = locals.into_iter().map(|v| v.into_iter().map(canon).collect()).collect();
    let pshape = inf.fparam.clone().iter().map(|ps| ps.iter().map(|&t| inf.shape(t)).collect()).collect();
    let rshape = inf.fret.clone().iter().map(|&t| inf.shape(t)).collect();
    Types { class_of, params, ret, field, locals, pshape, rshape }
}
