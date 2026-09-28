fn ev_step(mut n: i64, mut c: i64) -> i64 {
    loop {
        if n == 1 { return c; }
        if n & 1 == 0 { n >>= 1; c += 1; } else { n = 3 * n + 1; c += 1; }
    }
}
fn dsum(n: i64) -> i64 { if n < 10 { n } else { dsum(n / 10) + n % 10 } }
fn run(n: i64) -> i64 {
    let (mut s, mut best) = (0i64, 0i64);
    for i in 1..n {
        let c = ev_step(i, 0);
        if c > best { best = c; }
        s = (s + c * dsum(i)) & 4294967295;
    }
    (s + best) & 4294967295
}
fn main() { println!("{}", run(std::env::args().nth(1).map(|a| a.parse().unwrap()).unwrap_or(3000000))); }
