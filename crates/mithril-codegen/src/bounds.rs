//! Closed-module value bounds used only to choose lossless frame storage.
use mithril_front::ast::BinOp;
use mithril_front::core::{Core, CoreModule, Prim};
use std::collections::{BTreeSet, HashMap};

#[derive(Clone, PartialEq, Debug)]
enum Value { Bottom, Any, Int(i128, i128, Option<u32>), Tuple(Vec<Value>) }
const MIN: i128 = -(1 << 55);
const MAX: i128 = (1 << 55) - 1;
impl Value {
    fn join(&self, other: &Self, widen: bool) -> Self {
        match (self, other) {
            (Self::Bottom, x) | (x, Self::Bottom) => x.plain(),
            (Self::Int(a, b, _), Self::Int(c, d, _)) => {
                let (mut lo, mut hi) = ((*a).min(*c), (*b).max(*d));
                if widen && lo < *a && lo < 0 { lo = -(((-lo) as u128).next_power_of_two() as i128); }
                if widen && hi > *b && hi > 0 { hi = ((hi as u128 + 1).next_power_of_two() - 1) as i128; }
                Self::Int(lo.max(MIN), hi.min(MAX), None)
            }
            (Self::Tuple(a), Self::Tuple(b)) if a.len() == b.len() => Self::Tuple(a.iter().zip(b).map(|(a,b)| a.join(b,widen)).collect()),
            _ => Self::Any,
        }
    }
    fn plain(&self) -> Self {
        match self { Self::Int(a,b,_) => Self::Int(*a,*b,None), Self::Tuple(v) => Self::Tuple(v.iter().map(Self::plain).collect()), v => v.clone() }
    }
    fn unsigned(&self) -> bool { matches!(self, Self::Int(a,b,_) if *a >= 0 && *b <= u32::MAX as i128) }
}
struct Pass<'a> {
    params: Vec<Vec<Value>>,
    returns: &'a [Value],
    facts: Vec<HashMap<u32,Value>>,
    fid: usize,
}
impl Pass<'_> {
    fn eval(&mut self, e: &Core, env: &mut HashMap<u32, Value>) -> Value {
        match e {
            Core::Num(n) => Value::Int(*n as i128,*n as i128,None),
            Core::Var(x) => env.get(x).cloned().unwrap_or(Value::Any),
            Core::Let(x, r, b) => {
                let r = self.eval(r,env);
                self.facts[self.fid].entry(*x).and_modify(|v| *v=v.join(&r,false)).or_insert(r.plain());
                let mut inner = env.clone();
                if inner.contains_key(x) {
                    for value in inner.values_mut() { *value = value.plain(); }
                }
                inner.insert(*x,r);
                self.eval(b,&mut inner)
            }
            Core::Call(f, xs) => {
                let args: Vec<_>=xs.iter().map(|x| self.eval(x,env)).collect();
                for (p,a) in self.params[*f as usize].iter_mut().zip(args) { *p=p.join(&a,true); }
                self.returns[*f as usize].clone()
            }
            Core::Tuple(xs) => Value::Tuple(xs.iter().map(|x| match self.eval(x,env) { Value::Tuple(_) => Value::Any, v => v }).collect()),
            Core::Proj(x,i) => match self.eval(x,env) { Value::Tuple(xs) => xs.get(*i).cloned().unwrap_or(Value::Any), Value::Bottom => Value::Bottom, _ => Value::Any },
            Core::Cmp(_,a,b) => { self.eval(a,env); self.eval(b,env); Value::Int(0,1,None) }
            Core::If(c,a,b) => {
                self.eval(c,env);
                self.eval(a,&mut env.clone()).join(&self.eval(b,&mut env.clone()),false)
            }
            Core::Op2(op,a,b) => {
                let (x,y)=(self.eval(a,env),self.eval(b,env));
                if x==Value::Bottom || y==Value::Bottom { return Value::Bottom; }
                let Value::Int(x0,x1,_) = x else { return Value::Int(MIN,MAX,None) };
                let Value::Int(y0,y1,subset) = y else { return Value::Int(MIN,MAX,None) };
                if *op==BinOp::Sub && x0>=0 && matches!(&**a,Core::Var(n) if Some(*n)==subset) { return Value::Int(0,x1,None); }
                let constant=if y0==y1 { Core::Num(y0 as i64) } else { (**b).clone() };
                let (lo,hi)=if matches!(op,BinOp::Shl | BinOp::Shr) && y0>=0 && y1<=62 {
                    let points=[(x0,y0),(x0,y1),(x1,y0),(x1,y1)].map(|(x,y)| if *op==BinOp::Shl { x << y } else { x >> y });
                    (*points.iter().min().unwrap(),*points.iter().max().unwrap())
                } else { crate::range::op_iv(op,(x0,x1),(y0,y1),&constant) };
                let (lo,hi)=if lo>=MIN && hi<=MAX { (lo,hi) } else { (MIN,MAX) };
                let subset=if *op==BinOp::BitAnd && lo>=0 { match (&**a,&**b) { (Core::Var(x),_) | (_,Core::Var(x)) => Some(*x), _=>None } } else { None };
                Value::Int(lo,hi,subset)
            }
            Core::Lam(x,b) => { let mut env=env.clone(); env.insert(*x,Value::Any); self.eval(b,&mut env); Value::Any }
            Core::Match(sc,arms) => {
                self.eval(sc,env);
                let mut result=Value::Bottom;
                for (_,bs,b) in arms {
                    let mut env=env.clone();
                    for x in bs { env.insert(*x,Value::Any); self.facts[self.fid].insert(*x,Value::Any); }
                    result=result.join(&self.eval(b,&mut env),false);
                }
                result
            }
            Core::Prim(Prim::ArrGet, xs) => { for x in xs { self.eval(x,env); } Value::Any }
            _ => { for x in e.kids() { self.eval(x,env); } Value::Any }
        }
    }
}
pub(crate) fn analyze(m: &CoreModule) -> Vec<BTreeSet<u32>> {
    let mut params:Vec<Vec<Value>>=m.fns.iter().map(|f| vec![Value::Bottom;f.arity]).collect();
    let mut returns=vec![Value::Bottom;m.fns.len()];
    for _ in 0..128 {
        let mut pass=Pass { params:params.clone(), returns:&returns, facts:vec![HashMap::new();m.fns.len()], fid:0 };
        let mut next=returns.clone();
        for (f,d) in m.fns.iter().enumerate() {
            pass.fid=f;
            let mut env=params[f].iter().enumerate().map(|(p,v)| (p as u32,v.clone())).collect();
            let ret=pass.eval(&d.body,&mut env);
            next[f]=next[f].join(&ret,false);
        }
        if pass.params==params && next==returns {
            return pass.facts.into_iter().zip(params).map(|(mut facts,ps)| {
                for (p,v) in ps.into_iter().enumerate() { facts.entry(p as u32).and_modify(|f| *f=f.join(&v,false)).or_insert(v); }
                facts.into_iter().filter_map(|(p,v)| v.unsigned().then_some(p)).collect()
            }).collect();
        }
        params=pass.params;
        returns=next;
    }
    vec![BTreeSet::new();m.fns.len()]
}
#[cfg(test)] mod tests {
    use super::*;
    fn flags(source: &str, name: &str) -> BTreeSet<u32> {
        let m = mithril_front::desugar(&mithril_front::parse(source).unwrap()).unwrap();
        let f = m.fns.iter().position(|f| f.name == name).unwrap();
        analyze(&m)[f].iter().filter(|p| **p < m.fns[f].arity as u32).copied().collect()
    }
    #[test] fn positive_closed_calls_narrow_but_unknown_or_negative_values_do_not() {
        assert_eq!(flags("def f(x, y):\n    return x | y\n\ndef main():\n    return f(17, 4294967295)\n", "f"), BTreeSet::from([0, 1]));
        assert_eq!(flags("def f(x, y):\n    return x | y\n\ndef main():\n    return f(-1, array_get(array_new(2, 0), 0))\n", "f"), BTreeSet::new());
    }
    #[test] fn recursive_tuple_shapes_stay_finite() {
        let source = "def tree(n):\n    if n == 0:\n        return (1, 2)\n    return (tree(n - 1), tree(n - 1))\n\ndef main():\n    return tree(array_len(array_new(3, 0)))\n";
        assert!(flags(source, "tree").is_empty());
    }
    #[test] fn clearing_a_subset_keeps_recursive_words_unsigned() {
        let source = "def f(word, total):\n    if word == 0:\n        return total\n    bit = word & ((0 - word) & 4294967295)\n    return f(word - bit, (total + 1) & 4294967295)\n\ndef main():\n    return f(255, 0)\n";
        assert_eq!(flags(source, "f"), BTreeSet::from([0, 1]));
    }
}
