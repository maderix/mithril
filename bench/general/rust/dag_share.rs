use std::rc::Rc;
enum T { Leaf(i64), Node(Rc<T>, Rc<T>) }
fn mk(d: i64, v: i64) -> Rc<T> { if d == 0 { Rc::new(T::Leaf(v)) } else { let x = mk(d - 1, v * 3 + d); Rc::new(T::Node(x.clone(), x)) } }
fn total(t: &T) -> i64 { match t { T::Leaf(v) => *v, T::Node(l, r) => (total(l) + total(r)) & 4294967295 } }
fn depth_mix(t: &T, k: i64) -> i64 { match t { T::Leaf(v) => (v * k) & 65535, T::Node(l, r) => (depth_mix(l, k + 1) * 31 + depth_mix(r, k + 2)) & 4294967295 } }
fn bump(t: &T, c: i64) -> Rc<T> { match t { T::Leaf(v) => Rc::new(T::Leaf((v + c) & 1023)), T::Node(l, _) => { let x = bump(l, c); Rc::new(T::Node(x.clone(), x)) } } }
fn run(n: i64) -> i64 {
    let mut s = 0i64;
    for i in 0..n {
        let a = mk(16, i);
        let b = bump(&a, i);
        s = (s + total(&a) + depth_mix(&b, i & 7) + total(&b)) & 4294967295;
    }
    s
}
fn main() { println!("{}", run(std::env::args().nth(1).map(|a| a.parse().unwrap()).unwrap_or(4000))); }
