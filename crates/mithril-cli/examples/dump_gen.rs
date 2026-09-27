// scratch: dump generated main.rs + scalar classification for a port
fn main() {
    let path = std::env::args().nth(1).expect("port path");
    let out = std::env::args().nth(2).expect("out path");
    let src = std::fs::read_to_string(&path).unwrap();
    let mut m = mithril_front::parse(&src).unwrap();
    let _ = mithril_reassoc::analyze(&mut m);
    let cm = mithril_front::desugar(&m).unwrap();
    let mut net = mithril_net::build(&cm);
    mithril_net::reduce(&mut net, &cm, 1 << 20);
    let code = mithril_codegen::emit_rust(&cm, &net);
    std::fs::write(&out, &code).unwrap();
    for (i, f) in cm.fns.iter().enumerate() {
        let s = if code.contains(&format!("fn s_{i}(")) { "SCALAR" } else { "dive" };
        println!("{i:3} {s:6} tail_rec={} arity={} {}", f.self_tail_rec, f.arity, f.name);
    }
}
