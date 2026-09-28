// The source as written: each stage closure folds its setup table on every
// application.
fn setup(c: i64) -> i64 { let mut s: i64 = 0; for i in 0..20000i64 { s = (s + ((i * c + 7) & 4095)) & 1048575; } s }
fn main() {
    let b: i64 = std::hint::black_box(3);
    let cfg = [b, b + 1, b + 2, b + 3];
    let stages: Vec<Box<dyn Fn(i64) -> i64>> = cfg.iter().map(|&c| Box::new(move |x: i64| (x * 3 + setup(c)) & 1048575) as Box<dyn Fn(i64) -> i64>).collect();
    let mut acc: i64 = 0;
    let mut i: i64 = 20000;
    while i != 0 {
        let mut x = i;
        for f in &stages { x = f(x); }
        acc = (acc + x) & 1048575;
        i -= 1;
    }
    println!("{}", acc);
}
