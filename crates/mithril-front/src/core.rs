//! Core IR (desugared functional form) and a tiny reference interpreter
//! used as the semantic oracle by later tasks' tests.

use crate::ast::{BinOp, CmpOp};
use std::collections::HashMap;

pub type FnId = u32;
pub type CtorId = u32;

/// Sentinel `CtorId` used by desugar as an "unreachable" match fallthrough
/// (e.g. after an exhaustive-by-construction int-literal chain that a well
/// typed program will never actually fall through). `eval_core` panics if
/// it is ever evaluated.
pub const UNREACHABLE_CTOR: CtorId = u32::MAX;

#[derive(Clone, PartialEq, Debug)]
pub enum Core {
    Num(i64),
    Flo(f64),
    Var(u32),
    Op2(BinOp, Box<Core>, Box<Core>),
    Cmp(CmpOp, Box<Core>, Box<Core>),
    If(Box<Core>, Box<Core>, Box<Core>),
    Let(u32, Box<Core>, Box<Core>),
    /// Saturated call to a top-level fn.
    Call(FnId, Vec<Core>),
    Ctor(CtorId, Vec<Core>),
    /// Binders are var indices, in field order.
    Match(Box<Core>, Vec<(CtorId, Vec<u32>, Core)>),
    /// Static reuse rewrite: build ctor `c` from `args` in the cell of the
    /// value bound to var `v`, which the enclosing match consumed (last use).
    /// Semantically identical to `Ctor(c, args)`; the rewrite guarantees `v`
    /// is dead here and its cell has the same arity.
    Reuse(u32, CtorId, Vec<Core>),
    Tuple(Vec<Core>),
    Proj(Box<Core>, usize),
    /// A builtin with value semantics (see `Prim`).
    Prim(Prim, Vec<Core>),
}

/// Builtins. Arrays are values: `ArrSet` yields a new array (the compiled
/// code updates in place when it holds the only reference).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Prim {
    /// `array_new(n, v)`: n copies of v
    ArrNew,
    /// `array_get(a, i)`
    ArrGet,
    /// `array_set(a, i, v)`: a with element i replaced by v
    ArrSet,
    /// `array_len(a)`
    ArrLen,
    /// IEEE-754 binary32 arithmetic on bit patterns (ints in [0, 2^32)):
    /// `f32_add/sub/mul/div(a, b)`, `f32_sqrt(a)`, each correctly rounded
    /// to nearest-even; `f32_lt(a, b)` (1 when a < b, IEEE: false on NaN);
    /// `f32_from_u32(n)` (nearest f32 to the unsigned n); `f32_to_u32(a)`
    /// (truncated toward zero; NaN, negative or >= 2^32 give 0).
    F32Add,
    F32Sub,
    F32Mul,
    F32Div,
    F32Sqrt,
    F32Lt,
    F32FromU32,
    F32ToU32,
}

impl Prim {
    /// The binary32 primitives (all ints in, one int out).
    pub fn is_f32(self) -> bool {
        !matches!(self, Prim::ArrNew | Prim::ArrGet | Prim::ArrSet | Prim::ArrLen)
    }
}

/// Semantics of the binary32 primitives on bit patterns (the reference for
/// every backend).
pub fn f32_prim(p: Prim, a: &[i64]) -> i64 {
    let f = |x: i64| f32::from_bits(x as u32);
    let b = |x: f32| x.to_bits() as i64;
    match p {
        Prim::F32Add => b(f(a[0]) + f(a[1])),
        Prim::F32Sub => b(f(a[0]) - f(a[1])),
        Prim::F32Mul => b(f(a[0]) * f(a[1])),
        Prim::F32Div => b(f(a[0]) / f(a[1])),
        Prim::F32Sqrt => b(f(a[0]).sqrt()),
        Prim::F32Lt => (f(a[0]) < f(a[1])) as i64,
        Prim::F32FromU32 => b((a[0] as u32) as f32),
        Prim::F32ToU32 => {
            let x = f(a[0]);
            if x.is_nan() || x < 0.0 || x >= 4294967296.0 { 0 } else { x as u32 as i64 }
        }
        _ => unreachable!("not a binary32 primitive: {:?}", p),
    }
}

impl Prim {
    pub fn by_name(name: &str) -> Option<(Prim, usize)> {
        Some(match name {
            "array_new" => (Prim::ArrNew, 2),
            "array_get" => (Prim::ArrGet, 2),
            "array_set" => (Prim::ArrSet, 3),
            "array_len" => (Prim::ArrLen, 1),
            "f32_add" => (Prim::F32Add, 2),
            "f32_sub" => (Prim::F32Sub, 2),
            "f32_mul" => (Prim::F32Mul, 2),
            "f32_div" => (Prim::F32Div, 2),
            "f32_sqrt" => (Prim::F32Sqrt, 1),
            "f32_lt" => (Prim::F32Lt, 2),
            "f32_from_u32" => (Prim::F32FromU32, 1),
            "f32_to_u32" => (Prim::F32ToU32, 1),
            _ => return None,
        })
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct CoreFn {
    pub name: String,
    pub arity: usize,
    pub body: Core,
    /// True iff every self-call is the last expression evaluated on its
    /// control-flow path (i.e. all self-calls are in tail position).
    pub self_tail_rec: bool,
    /// Set by Task 4 on the `For` this function was generated from (if
    /// any); Task 3's desugar only ever plumbs through whatever the
    /// surface `For`'s `fold` field already carried (always `None` today,
    /// since the parser never sets it).
    pub fold: Option<FoldInfo>,
}

#[derive(Clone, PartialEq, Debug)]
pub struct FoldInfo {
    pub combiner: Combiner,
    pub proven: bool,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Combiner {
    WrapAdd,
    TupleWrapAdd(usize),
    /// u32-emulation folds (`& 4294967295`-masked): wrapping add mod
    /// 2^32; the join must re-mask its result to the low 32 bits.
    WrapAdd32,
    TupleWrapAdd32(usize),
    Fn(FnId),
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct CoreModule {
    pub fns: Vec<CoreFn>,
    pub ctors: Vec<(String, usize)>,
    pub main: FnId,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Val {
    I(i64),
    F(f64),
    C(CtorId, Vec<Val>),
    T(Vec<Val>),
    A(Vec<Val>),
}

/// Wrap a 64-bit result down to the signed 56-bit `int` range, matching
/// `mithril_core::port::Port::as_i64`'s sign-extension trick: shift the
/// low 56 bits to the top of the word and arithmetic-shift back down.
fn wrap56(v: i64) -> i64 {
    ((v as u64) << 8) as i64 >> 8
}

fn floor_div(a: i64, b: i64) -> i64 {
    let q = a.wrapping_div(b);
    let r = a.wrapping_rem(b);
    if r != 0 && (r < 0) != (b < 0) {
        q - 1
    } else {
        q
    }
}

fn py_mod(a: i64, b: i64) -> i64 {
    let r = a.wrapping_rem(b);
    if r != 0 && (r < 0) != (b < 0) {
        r + b
    } else {
        r
    }
}

/// Evaluate `f(args)` under module `m`. This is a small, direct-style
/// reference interpreter: no attempt is made to be fast, only correct and
/// simple enough to serve as an oracle for the net-based runtime's tests.
pub fn eval_core(m: &CoreModule, f: FnId, args: &[Val]) -> Val {
    let fdef = &m.fns[f as usize];
    assert_eq!(args.len(), fdef.arity, "eval_core: arity mismatch calling {}", fdef.name);
    let mut env: HashMap<u32, Val> = HashMap::new();
    for (i, a) in args.iter().enumerate() {
        env.insert(i as u32, a.clone());
    }
    eval(m, &env, &fdef.body)
}

fn as_i(v: &Val) -> i64 {
    match v {
        Val::I(x) => *x,
        other => panic!("eval_core: expected int, found {:?}", other),
    }
}

fn eval(m: &CoreModule, env: &HashMap<u32, Val>, e: &Core) -> Val {
    match e {
        Core::Num(n) => Val::I(*n),
        Core::Flo(n) => Val::F(*n),
        Core::Var(i) => env.get(i).cloned().unwrap_or_else(|| panic!("eval_core: unbound var {}", i)),
        Core::Op2(op, a, b) => {
            let (va, vb) = (eval(m, env, a), eval(m, env, b));
            match (&va, &vb) {
                (Val::I(x), Val::I(y)) => Val::I(wrap56(match op {
                    BinOp::Add => x.wrapping_add(*y),
                    BinOp::Sub => x.wrapping_sub(*y),
                    BinOp::Mul => x.wrapping_mul(*y),
                    BinOp::Div => x.wrapping_div(*y),
                    BinOp::FloorDiv => floor_div(*x, *y),
                    BinOp::Mod => py_mod(*x, *y),
                    BinOp::Shl => x.wrapping_shl(*y as u32),
                    BinOp::Shr => x.wrapping_shr(*y as u32),
                    BinOp::BitAnd => x & y,
                    BinOp::BitOr => x | y,
                    BinOp::BitXor => x ^ y,
                })),
                (Val::F(x), Val::F(y)) => Val::F(match op {
                    BinOp::Add => x + y,
                    BinOp::Sub => x - y,
                    BinOp::Mul => x * y,
                    BinOp::Div => x / y,
                    _ => panic!("eval_core: op {:?} not defined on floats", op),
                }),
                _ => panic!("eval_core: Op2 type mismatch: {:?} {:?} {:?}", op, va, vb),
            }
        }
        Core::Cmp(op, a, b) => {
            let (va, vb) = (eval(m, env, a), eval(m, env, b));
            let r = match (&va, &vb) {
                (Val::I(x), Val::I(y)) => cmp_bool(*op, x.partial_cmp(y)),
                (Val::F(x), Val::F(y)) => cmp_bool(*op, x.partial_cmp(y)),
                _ => panic!("eval_core: Cmp type mismatch: {:?} {:?}", va, vb),
            };
            Val::I(if r { 1 } else { 0 })
        }
        Core::If(c, t, e2) => {
            if as_i(&eval(m, env, c)) != 0 {
                eval(m, env, t)
            } else {
                eval(m, env, e2)
            }
        }
        Core::Let(i, rhs, body) => {
            let v = eval(m, env, rhs);
            let mut env2 = env.clone();
            env2.insert(*i, v);
            eval(m, &env2, body)
        }
        Core::Call(fid, args) => {
            let vals: Vec<Val> = args.iter().map(|a| eval(m, env, a)).collect();
            eval_core(m, *fid, &vals)
        }
        Core::Ctor(cid, args) | Core::Reuse(_, cid, args) => {
            if *cid == UNREACHABLE_CTOR {
                panic!("eval_core: non-exhaustive match reached at runtime");
            }
            let vals: Vec<Val> = args.iter().map(|a| eval(m, env, a)).collect();
            Val::C(*cid, vals)
        }
        Core::Match(scrut, arms) => {
            let v = eval(m, env, scrut);
            let (cid, fields) = match v {
                Val::C(cid, fields) => (cid, fields),
                other => panic!("eval_core: match on non-constructor value {:?}", other),
            };
            for (acid, binders, body) in arms {
                if *acid == cid {
                    let mut env2 = env.clone();
                    for (b, val) in binders.iter().zip(fields.iter()) {
                        env2.insert(*b, val.clone());
                    }
                    return eval(m, &env2, body);
                }
            }
            panic!("eval_core: no match arm for ctor {}", cid);
        }
        Core::Tuple(items) => Val::T(items.iter().map(|it| eval(m, env, it)).collect()),
        Core::Proj(e, i) => match eval(m, env, e) {
            Val::T(items) => items[*i].clone(),
            other => panic!("eval_core: Proj on non-tuple value {:?}", other),
        },
        Core::Prim(p, args) => {
            let vs: Vec<Val> = args.iter().map(|a| eval(m, env, a)).collect();
            let idx = |v: &Val, len: usize| -> usize {
                let i = as_i(v);
                assert!(i >= 0 && (i as usize) < len, "eval_core: array index {} out of bounds ({})", i, len);
                i as usize
            };
            match (p, vs.as_slice()) {
                (Prim::ArrNew, [n, v]) => {
                    let n = as_i(n);
                    assert!(n >= 0, "eval_core: negative array size {}", n);
                    Val::A(vec![v.clone(); n as usize])
                }
                (Prim::ArrGet, [Val::A(xs), i]) => xs[idx(i, xs.len())].clone(),
                (Prim::ArrSet, [Val::A(xs), i, v]) => {
                    let mut ys = xs.clone();
                    let k = idx(i, ys.len());
                    ys[k] = v.clone();
                    Val::A(ys)
                }
                (Prim::ArrLen, [Val::A(xs)]) => Val::I(xs.len() as i64),
                (p, vs) if p.is_f32() => Val::I(f32_prim(*p, &vs.iter().map(as_i).collect::<Vec<_>>())),
                (p, vs) => panic!("eval_core: bad arguments to {:?}: {:?}", p, vs),
            }
        }
    }
}

fn cmp_bool(op: CmpOp, ord: Option<std::cmp::Ordering>) -> bool {
    use std::cmp::Ordering::*;
    let ord = ord.expect("eval_core: incomparable values (NaN?)");
    match op {
        CmpOp::Lt => ord == Less,
        CmpOp::Le => ord != Greater,
        CmpOp::Gt => ord == Greater,
        CmpOp::Ge => ord != Less,
        CmpOp::Eq => ord == Equal,
        CmpOp::Ne => ord != Equal,
    }
}
