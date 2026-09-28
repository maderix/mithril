// scratch: dump generated program.cu for a source file
fn main() {
    let path = std::env::args().nth(1).expect("source path");
    let out = std::env::args().nth(2).expect("out path");
    let src = std::fs::read_to_string(&path).unwrap();
    let mut m = mithril_front::parse(&src).unwrap();
    let _ = mithril_reassoc::analyze(&mut m);
    let cm = mithril_front::desugar(&m).unwrap();
    let (sm, _) = mithril_net::specialize(&cm, 1 << 20);
    match mithril_gpu::emit_cuda(&sm) {
        Ok(cu) => std::fs::write(&out, cu).unwrap(),
        Err(v) => println!("constant: {v}"),
    }
}
