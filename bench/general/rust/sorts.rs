use std::rc::Rc;
// persistent cons lists (the input is sorted twice, so it is shared)
enum L { Nil, Cons(i64, Rc<L>) }
use L::*;
fn gen(n: i64, mut x: i64) -> Rc<L> { let mut acc = Rc::new(Nil); for _ in 0..n { x = (x * 1103515245 + 12345) & 2147483647; acc = Rc::new(Cons(x & 1048575, acc)); } acc }
fn split(l: &Rc<L>) -> (Rc<L>, Rc<L>) { let (mut a, mut b) = (Rc::new(Nil), Rc::new(Nil)); let mut c = l.clone(); loop { match &*c.clone() { Nil => return (a, b), Cons(h, t) => { let na = b; b = Rc::new(Cons(*h, a)); a = na; c = t.clone(); } } } }
fn merge(a: &Rc<L>, b: &Rc<L>) -> Rc<L> { match (&**a, &**b) { (Nil, _) => b.clone(), (_, Nil) => a.clone(), (Cons(x, xs), Cons(y, ys)) => if x <= y { Rc::new(Cons(*x, merge(xs, b))) } else { Rc::new(Cons(*y, merge(a, ys))) } } }
fn msort(l: &Rc<L>) -> Rc<L> { match &**l { Nil => l.clone(), Cons(_, t) => match &**t { Nil => l.clone(), _ => { let (a, b) = split(l); merge(&msort(&a), &msort(&b)) } } } }
fn part(l: &Rc<L>, piv: i64) -> (Rc<L>, Rc<L>) { let (mut lo, mut hi) = (Rc::new(Nil), Rc::new(Nil)); let mut c = l.clone(); loop { match &*c.clone() { Nil => return (lo, hi), Cons(h, t) => { if *h < piv { lo = Rc::new(Cons(*h, lo)) } else { hi = Rc::new(Cons(*h, hi)) }; c = t.clone(); } } } }
fn app(a: &Rc<L>, b: Rc<L>) -> Rc<L> { match &**a { Nil => b, Cons(h, t) => Rc::new(Cons(*h, app(t, b))) } }
fn qsort(l: &Rc<L>) -> Rc<L> { match &**l { Nil => l.clone(), Cons(h, t) => { let (lo, hi) = part(t, *h); app(&qsort(&lo), Rc::new(Cons(*h, qsort(&hi)))) } } }
fn check(l: &Rc<L>) -> i64 { let (mut i, mut acc) = (1i64, 0i64); let mut c = l.clone(); loop { match &*c.clone() { Nil => return acc, Cons(h, t) => { acc = (acc * 31 + h * i) & 4294967295; i += 1; c = t.clone(); } } } }
fn drop_list(l: Rc<L>) { let mut c = l; while let Ok(node) = Rc::try_unwrap(c) { match node { Nil => return, Cons(_, t) => c = t } } }
fn run(n: i64) -> i64 {
    let mut s = 0i64;
    for r in 0..8 {
        let l = gen(n, r + 1);
        let a = msort(&l); let b = qsort(&l);
        s = (s + check(&a) + check(&b)) & 4294967295;
        drop_list(a); drop_list(b); drop_list(l);
    }
    s
}
fn main() {
    let n = std::env::args().nth(1).map(|a| a.parse().unwrap()).unwrap_or(240000);
    let h = std::thread::Builder::new().stack_size(1 << 30).spawn(move || run(n)).unwrap();
    println!("{}", h.join().unwrap());
}
