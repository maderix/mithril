// The source as written: the closure body calls heavy(k) on every
// application. LLVM hoists the pure call out of the loop here (the
// closure is inlined into one function), so this twin does the work once
// too; pipeline_cfg is the shape where it cannot (closures in data).
fn heavy(k: i64) -> i64 { if k < 2 { k } else { heavy(k - 1) + heavy(k - 2) } }
fn main() {
    let k: i64 = std::hint::black_box(22);
    let g = |x: i64| x + heavy(k);
    let mut acc: i64 = 0;
    let mut i: i64 = 20000;
    while i != 0 { acc = (acc + g(i)) & 1048575; i -= 1; }
    println!("{}", acc);
}
