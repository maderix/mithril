use std::rc::Rc;
enum M { E, N(i64, i64, Rc<M>, Rc<M>) }
fn ins(m: &Rc<M>, k: i64, v: i64) -> Rc<M> {
    match &**m {
        M::E => Rc::new(M::N(k, v, Rc::new(M::E), Rc::new(M::E))),
        M::N(mk, mv, l, r) => {
            if k < *mk { Rc::new(M::N(*mk, *mv, ins(l, k, v), r.clone())) }
            else if k > *mk { Rc::new(M::N(*mk, *mv, l.clone(), ins(r, k, v))) }
            else { Rc::new(M::N(k, v, l.clone(), r.clone())) }
        }
    }
}
fn get(m: &M, k: i64) -> i64 { match m { M::E => 0, M::N(mk, mv, l, r) => if k < *mk { get(l, k) } else if k > *mk { get(r, k) } else { *mv } } }
fn size(m: &M) -> i64 { match m { M::E => 0, M::N(_, _, l, r) => 1 + size(l) + size(r) } }
fn run(n: i64) -> i64 {
    let mut m = Rc::new(M::E);
    let mut old = Rc::new(M::E);
    let (mut x, mut s) = (7i64, 0i64);
    for i in 0..n {
        x = (x * 1103515245 + 12345) & 2147483647;
        m = ins(&m, x & 65535, i);
        if i & 1023 == 0 { old = m.clone(); }
        s = (s + get(&m, (x >> 3) & 65535) + get(&old, x & 65535)) & 4294967295;
    }
    (s + size(&m) * 7 + size(&old)) & 4294967295
}
fn main() { println!("{}", run(std::env::args().nth(1).map(|a| a.parse().unwrap()).unwrap_or(1600000))); }
