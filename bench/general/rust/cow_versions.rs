use std::rc::Rc;
// value-semantics arrays via Rc<Vec>: make_mut copies when shared
fn run(n: i64) -> i64 {
    let mut a: Rc<Vec<i64>> = Rc::new(vec![1; 256]);
    let mut snap = a.clone();
    let mut s = 0i64;
    for i in 0..n {
        let j = ((i * 37) & 255) as usize;
        let v = (a[(j + 1) & 255] + i) & 65535;
        Rc::make_mut(&mut a)[j] = v;
        if i & 63 == 0 { snap = a.clone(); }
        s = (s + snap[j] + a[(j * 7) & 255]) & 4294967295;
    }
    s
}
fn main() { println!("{}", run(std::env::args().nth(1).map(|a| a.parse().unwrap()).unwrap_or(300000000))); }
