use std::rc::Rc;
enum E { Lit(i64), Var(i64), Add(Box<E>, Box<E>), Mul(Box<E>, Box<E>), Sub(Box<E>, Box<E>), Lt(Box<E>, Box<E>), If(Box<E>, Box<E>, Box<E>), Let(i64, Box<E>, Box<E>) }
enum Env { Emp, Bind(i64, i64, Rc<Env>) }
fn look(env: &Env, i: i64) -> i64 { match env { Env::Emp => 0, Env::Bind(j, v, rest) => if i == *j { *v } else { look(rest, i) } } }
fn ev(e: &E, env: &Rc<Env>) -> i64 {
    match e {
        E::Lit(n) => *n, E::Var(i) => look(env, *i),
        E::Add(a, b) => (ev(a, env) + ev(b, env)) & 1048575,
        E::Mul(a, b) => (ev(a, env) * ev(b, env)) & 1048575,
        E::Sub(a, b) => (ev(a, env) + 1048576 - ev(b, env)) & 1048575,
        E::Lt(a, b) => (ev(a, env) < ev(b, env)) as i64,
        E::If(c, t, f) => if ev(c, env) != 0 { ev(t, env) } else { ev(f, env) },
        E::Let(i, v, b) => { let x = ev(v, env); ev(b, &Rc::new(Env::Bind(*i, x, env.clone()))) }
    }
}
// the Python source shares a and b between the If arms; clone the tree
fn cl(e: &E) -> Box<E> { Box::new(match e { E::Lit(n) => E::Lit(*n), E::Var(i) => E::Var(*i), E::Add(a, b) => E::Add(cl(a), cl(b)), E::Mul(a, b) => E::Mul(cl(a), cl(b)), E::Sub(a, b) => E::Sub(cl(a), cl(b)), E::Lt(a, b) => E::Lt(cl(a), cl(b)), E::If(a, b, c) => E::If(cl(a), cl(b), cl(c)), E::Let(i, a, b) => E::Let(*i, cl(a), cl(b)) }) }
fn gen(d: i64, x: i64, nv: i64) -> Box<E> {
    let y = (x * 1103515245 + 12345) & 2147483647;
    let k = (y >> 8) % 8;
    if d == 0 { return Box::new(if k & 1 == 0 { E::Lit(y & 255) } else { E::Var((y >> 4) % (nv + 1)) }); }
    let a = gen(d - 1, y, nv);
    let b = gen(d - 1, y ^ 99991, nv);
    Box::new(match k {
        0 => E::Add(a, b), 1 => E::Mul(a, b), 2 => E::Sub(a, b),
        3 => E::If(Box::new(E::Lt(cl(&a), cl(&b))), a, b),
        4 => E::Let(nv + 1, a, b),
        _ => E::Add(a, Box::new(E::Lit(k))),
    })
}
fn run(n: i64) -> i64 {
    let mut s = 0i64;
    for i in 0..n {
        let e = gen(14, i * 7919 + 1, 3);
        let env = Rc::new(Env::Bind(0, i & 1023, Rc::new(Env::Bind(1, 7, Rc::new(Env::Bind(2, i & 15, Rc::new(Env::Bind(3, 3, Rc::new(Env::Emp)))))))));
        s = (s + ev(&e, &env)) & 4294967295;
    }
    s
}
fn main() { println!("{}", run(std::env::args().nth(1).map(|a| a.parse().unwrap()).unwrap_or(3000))); }
