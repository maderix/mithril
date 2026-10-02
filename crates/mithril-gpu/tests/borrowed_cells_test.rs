use mithril_rt::Redex;

#[test]
#[ignore = "requires CUDA and nvcc"]
fn borrowed_pairs_preserve_wide_shared_and_chained_fields_and_address_checks() {
    let source = "def main():\n    return array_len(array_new(1, 0))\n";
    let m = mithril_front::desugar(&mithril_front::parse(source).unwrap()).unwrap();
    let base = mithril_gpu::emit_cuda(&m).unwrap().replace(
        "#include \"engine.cu\"",
        "#define k_boot pairs_unused_boot\n#include \"engine.cu\"\n#undef k_boot",
    );
    let kernel = r#"
extern "C" __global__ void k_boot(u64,u64,u64,int) {
 if(threadIdx.x!=0) return;
 stack_mark(); bool pass=true; u64 fields[33];
 for(int i=0;i<33;i++) fields[i]=i%2 ? ic(0,4294967295ll) : num(-1099511627776ll+i);
 int widths[]={0,1,2,3,4,17,33};
 for(int width:widths) {
  u64 p=mk_con(0,fields,width), copy=dup_val(p);
  for(int repeat=0;repeat<7;repeat++) {
   auto a=read_pair(p),b=read_pair(copy);
   pass &= a.a[0]==field(p,0) && a.a[1]==field(p,1);
   pass &= b.a[0]==a.a[0] && b.a[1]==a.a[1];
  }
  free_val(p);
  if(width>0) pass &= read_pair(copy).a[0]==fields[0];
  free_val(copy);
 }
 deliver(ROOT,num(pass));
}
"#;
    let cache = std::env::temp_dir().join("mithril-borrowed-cells");
    let got = mithril_gpu::compile_and_run(&(base.clone() + kernel), Redex::default(), &cache).unwrap();
    assert_eq!(got.text, "1");
    let invalid = kernel.replace(
        "stack_mark(); bool pass=true;",
        "stack_mark(); read_pair(con(0xffffffffu,0,2)); bool pass=true;",
    );
    let error = mithril_gpu::compile_and_run(&(base + &invalid), Redex::default(), &cache)
        .expect_err("an invalid cell must be rejected");
    assert!(error.contains("cell index outside the arena"), "{error}");
}
