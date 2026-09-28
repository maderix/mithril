use std::rc::Rc;
enum E { Lit(i64), Var(i64), Add(Box<E>, Box<E>), Mul(Box<E>, Box<E>), If(Box<E>, Box<E>, Box<E>) }
enum Env { Emp, Bind(i64, Rc<Env>) }
fn look(env: &Env, i: i64) -> i64 { match env { Env::Emp => 0, Env::Bind(v, rest) => if i == 0 { *v } else { look(rest, i - 1) } } }
type F = Rc<dyn Fn(&Rc<Env>) -> i64>;
fn compile(e: &E) -> F {
    match e {
        E::Lit(n) => { let n = *n; Rc::new(move |_| n) }
        E::Var(i) => { let i = *i; Rc::new(move |env| look(env, i)) }
        E::Add(a, b) => { let (ca, cb) = (compile(a), compile(b)); Rc::new(move |env| (ca(env) + cb(env)) & 1048575) }
        E::Mul(a, b) => { let (ca, cb) = (compile(a), compile(b)); Rc::new(move |env| (ca(env) * cb(env)) & 1048575) }
        E::If(c, t, f) => { let (cc, ct, cf) = (compile(c), compile(t), compile(f)); Rc::new(move |env| if cc(env) != 0 { ct(env) } else { cf(env) }) }
    }
}
fn gen(d: i64, x: i64) -> E {
    let y = (x * 1103515245 + 12345) & 2147483647;
    let k = (y >> 8) % 4;
    if d == 0 { return if k & 1 == 0 { E::Lit(y & 255) } else { E::Var((y >> 4) % 3) }; }
    let a = gen(d - 1, y);
    let b = gen(d - 1, y ^ 99991);
    match k { 0 => E::Add(Box::new(a), Box::new(b)), 1 => E::Mul(Box::new(a), Box::new(b)), 2 => E::If(Box::new(a), Box::new(b), Box::new(E::Lit(y & 7))), _ => E::Add(Box::new(E::Mul(Box::new(a), Box::new(E::Lit(3)))), Box::new(b)) }
}
fn main() {
    let seed: i64 = std::hint::black_box(42);
    let p = compile(&gen(7, seed));
    let mut acc: i64 = 0;
    let mut i: i64 = 20000;
    while i != 0 {
        let env = Rc::new(Env::Bind(i, Rc::new(Env::Bind((i * 7) & 255, Rc::new(Env::Bind(i ^ 5, Rc::new(Env::Emp)))))));
        let v = p(&env);
        acc = (acc * 31 + v) & 1048575;
        i -= 1;
    }
    println!("{}", acc);
}
