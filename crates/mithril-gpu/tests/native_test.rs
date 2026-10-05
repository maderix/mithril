//! Fixed hardware-stack depth must not limit native recursion.
use mithril_rt::Redex;

#[test]
#[ignore = "requires CUDA and nvcc; changes native and cache settings"]
fn shared_integer_prefixes_execute_in_native_and_growth_paths() {
    let cases = [
        ("scalar", "def tree(n):\n    if n < 2:\n        return 1\n    return tree(n - 1) + tree(n - 2)\n\ndef caller(n):\n    k = (n * 17 + 3) & 4294967295\n    if k % 2 == 0:\n        return tree(n - 1)\n    return tree(n)\n"),
        ("tuple", "def tree(n, a, b, c, d):\n    if n < 2:\n        return (a, b)\n    x = tree(n - 1, a, b, c, d)\n    y = tree(n - 2, a, b, c, d)\n    return ((x[0] + y[0] + c) & 4294967295, (x[1] + y[1] - d) & 4294967295)\n\ndef caller(n):\n    if n == 0:\n        return (17, 29)\n    return tree(n, 4294967295, 23, 17, 9)\n"),
    ];
    let cache = std::env::temp_dir().join("mithril-shared-prefixes");
    for (name, body) in cases { for n in [0, 6] {
        let source = format!("{body}\ndef main():\n    return caller(array_len(array_new({n}, 0)))\n");
        let m = mithril_front::desugar(&mithril_front::parse(&source).unwrap()).unwrap();
        let want = mithril_codegen::fmt_val(&mithril_front::eval_core(&m, m.main, &[]));
        let (lir, module) = mithril_codegen::lower(&m);
        let fid = module.fns.iter().position(|f| f.name == "caller").unwrap();
        let prefix = lir.fns.iter().find(|f| f.name == format!("prefix_s_{fid}")).unwrap_or_else(|| panic!("{name} n={n}: a shared prefix must actually be emitted"));
        let mithril_codegen::lir::Ty::Tup(width) = prefix.ret else { panic!("prefix result must be a tagged tuple") };
        let cu = mithril_gpu::emit_cuda(&m).unwrap();
        let header = format!("__device__ T{width} prefix_s_{fid}(");
        let begin = cu.rfind(&header).expect("missing CUDA prefix definition");
        let insert = begin + cu[begin..].find("{\n").unwrap() + 2;
        assert!(cu.contains("if (native_ready("), "forced modes must target real guards");
        for native in [false, true] {
            let mut code = cu.clone();
            code = code.replace("if (native_ready(", if native { "if (true || native_ready(" } else { "if (false && native_ready(" });
            for words in ["0", "auto"] {
                std::env::set_var("MITHRIL_GPU_NATIVE_WORDS", words);
                let got = mithril_gpu::compile_and_run(&code, Redex::default(), &cache);
                assert_eq!(got.map(|r| r.text), Ok(want.clone()), "{name} n={n} native={native} cache={words}");
            }
            // A poisoned private prefix must fail, proving both paths execute it.
            let mut poisoned = cu.clone();
            poisoned.insert_str(insert, "g_abort(4);\n");
            poisoned = poisoned.replace("if (native_ready(", if native { "if (true || native_ready(" } else { "if (false && native_ready(" });
            let error = mithril_gpu::compile_and_run(&poisoned, Redex::default(), &cache).expect_err("prefix must execute");
            assert!(error.contains("unsupported device feature"), "{name} n={n} native={native}: {error}");
        }
    } }
    std::env::remove_var("MITHRIL_PLAIN_INTS");
    std::env::remove_var("MITHRIL_GPU_NATIVE_WORDS");
}

#[test]
#[ignore = "requires CUDA and nvcc; changes lane settings"]
fn ready_lane_groups_claim_every_task_once_and_reset_between_phases() {
    let m = mithril_front::desugar(&mithril_front::parse("def main():\n    return array_len(array_new(1, 0))\n").unwrap()).unwrap();
    let mut cu = mithril_gpu::emit_cuda(&m).unwrap().replace("#include \"engine.cu\"",
        "#define k_boot claim_unused_boot\n#define k_run claim_unused_run\n#include \"engine.cu\"\n#undef k_boot\n#undef k_run");
    cu.push_str(r#"
extern "C" __global__ void k_boot(u64,u64,u64,int) {}
extern "C" __global__ void k_run(u32,u32,int,u64) {
  cg::grid_group grid=cg::this_grid();
  u32 gid=blockIdx.x*blockDim.x+threadIdx.x;
  u32 masks[]={1u,0xffffffffu,0xaaaaaaaau,0x80000000u,0x00010001u,0x55555555u};
  u32 counts[]={0,1,17,31,32,33,255,257,65537};
  bool pass=true;
  for(u32 mask:masks) for(u32 count:counts) {
    if(gid==0) {
      // The previous phase has finished before its claim counter resets.
      g_native_next=0;
      for(u32 i=0;i<count;i++) G.heap[i]=0;
    }
    grid.sync();
    if(mask & (1u<<(threadIdx.x%warpSize))) {
      for(;;) {
        u64 index=native_task();
        if(index>=count) break;
        volatile u32 work=(u32)index;
        for(u32 n=0;n<index%31+1;n++) work=work*1664525u+1013904223u;
        atomicAdd((unsigned long long*)&G.heap[index],1ull);
      }
    }
    grid.sync();
    if(gid==0) for(u32 i=0;i<count;i++) pass &= G.heap[i]==1;
    grid.sync();
  }
  if(gid==0) deliver(ROOT,num(pass));
}
"#);
    for lanes in ["256", "65536"] {
        std::env::set_var("MITHRIL_GPU_LANES", lanes);
        let got = mithril_gpu::compile_and_run(&cu, Redex::default(), &std::env::temp_dir().join("mithril-ready-lane-claims"));
        assert_eq!(got.map(|r| r.text), Ok("1".into()), "lanes={lanes}");
    }
    std::env::remove_var("MITHRIL_GPU_LANES");
}

#[test]
#[ignore = "requires CUDA and nvcc; changes arena settings"]
fn resident_and_demand_regions_preserve_handles_at_every_boundary() {
    let m = mithril_front::desugar(&mithril_front::parse("def main():\n    return array_len(array_new(1, 0))\n").unwrap()).unwrap();
    let mut cu = mithril_gpu::emit_cuda(&m).unwrap().replace("#include \"engine.cu\"",
        "#define k_boot arena_unused_boot\n#include \"engine.cu\"\n#undef k_boot");
    cu.push_str(r#"
extern "C" __global__ void k_boot(u64,u64,u64,int fuel) {
 stack_mark(); s_mode[threadIdx.x]=0; s_fuel=fuel;
 bool pass=true;
 for(u32 base: {16u,48u}) {
   Arena<u64> a{G.heap+base};
   for(u32 i=0;i<9;i++) a[i]=1000+i;
   for(u32 i=0;i<9;i++) {
     pass &= a[i]==1000+i;
     pass &= G.heap[base+i]==1000+i;
   }
 }
 // This test admits one block: cross its initial committed allocator burst.
 u32 prefix=min(G.ncap,1u+256u*min(G.chunksz,(u32)INITIAL_CHUNK));
 u32 left=prefix<G.ncap ? prefix-1 : G.ncap-2;
 u32 right=prefix<G.ncap ? prefix : G.ncap-1;
 for(u32 i: {1u,left,right,G.ncap-1}) {
   setcell(i,0x123456789abcdef0ull+i,0xfedcba9876543210ull-i);
   pass &= cell0(i)==0x123456789abcdef0ull+i;
   pass &= cell1(i)==0xfedcba9876543210ull-i;
   rc_set1(i); rc_inc(i);
   pass &= !rc_dec(i) && rc_unique(i) && rc_dec(i);
   free_node(i);
   u32 reused=alloc2(num(7),num(11));
   pass &= reused==i && cell0(i)==num(7) && cell1(i)==num(11) && rc_unique(i);
 }
 u32 rprefix=min(G.rcap,1u+256u*(u32)LSCAP);
 u32 rleft=rprefix<G.rcap ? rprefix-1 : G.rcap-2;
 u32 rright=rprefix<G.rcap ? rprefix : G.rcap-1;
 for(u32 i: {1u,rleft,rright,G.rcap-1}) {
   rec_free(i);
   u32 reused=alloc_rec(1,2,13,17,ROOT);
   pass &= reused==i && rec_parent(i)==ROOT && rec_d(i)==13 && rec_s(i)==17;
   pass &= atomicSub(&G.recs[i].pend,1)==2 && atomicSub(&G.recs[i].pend,1)==1;
 }
 setcell(left,num(7),num(11)); setcell(right,num(13),num(17));
 setcell(1,con(left,4095,2),con(right,4095,2));
 *G.nbump=right+1;
 if(!pass) g_abort(4);
 deliver(ROOT,con(1,4095,2));
}
"#);
    std::env::set_var("MITHRIL_GPU_LANES", "1");
    std::env::set_var("MITHRIL_GPU_BUCKET", "256");
    // force the bulk snapshot: this test is about its span, not about few reads
    std::env::set_var("MITHRIL_GPU_CELL_READS", "0");
    for (nodes,records) in [("512","512"),("1024","32768")] {
        std::env::set_var("MITHRIL_GPU_NODES", nodes);
        std::env::set_var("MITHRIL_GPU_RECS", records);
        for poison in ["", "nodes,recs,ebuf"] {
            std::env::set_var("MITHRIL_GPU_POISON", poison);
            let got = mithril_gpu::compile_and_run(&cu, Redex::default(), &std::env::temp_dir().join("mithril-arena-boundaries")).unwrap();
            assert_eq!(got.text, "((7, 11), (13, 17))", "nodes={nodes} recs={records} poison={poison}");
            let cells = nodes.parse::<usize>().unwrap().min(514);
            assert_eq!(got.cell_readback_bytes, 16*cells, "committed and demand pages must form one logical snapshot");
        }
    }
    for key in ["MITHRIL_GPU_LANES","MITHRIL_GPU_BUCKET","MITHRIL_GPU_NODES","MITHRIL_GPU_RECS","MITHRIL_GPU_POISON","MITHRIL_GPU_CELL_READS"] {
        std::env::remove_var(key);
    }
}

#[test]
#[ignore = "requires CUDA and nvcc; changes device settings"]
fn generated_native_phases_reset_the_claim_counter() {
    let source = "def f(n):\n    if n == 0:\n        return 1\n    return (f(n - 1) * 31 + n) & 4294967295\n\ndef g(n, a):\n    if n == 0:\n        return a & 4294967295\n    return (g(n - 1, a + 2) * 3 + n) & 4294967295\n\ndef main():\n    n = array_len(array_new(3, 0))\n    return f(n) + g(n, n + 10)\n";
    let m = mithril_front::desugar(&mithril_front::parse(source).unwrap()).unwrap();
    let (lir, m) = mithril_codegen::lower(&m);
    let f = m.fns.iter().position(|f| f.name == "f").unwrap() as u32;
    let g = m.fns.iter().position(|f| f.name == "g").unwrap() as u32;
    assert!(f < g && lir.native_entries.contains(&f) && lir.native_entries.contains(&g), "f={f} g={g} entries={:?}", lir.native_entries);
    let mut cu = mithril_gpu::emit_cuda(&m).unwrap().replace("#include \"engine.cu\"",
        "#define k_boot phases_unused_boot\n#include \"engine.cu\"\n#undef k_boot\n__device__ u32 phase_seen=0;");
    let header = "__device__ bool prog_native_rule(u32 r) {";
    assert_eq!(cu.matches(header).count(), 1);
    // This dispatch check runs between launches, after the real completion kernel.
    cu = cu.replace(header, &format!("{header}\nif(g_native_next!=0) {{ g_abort(4); return false; }}"));
    cu = instrument_task_call(cu, 1 + f, "atomicAdd(&phase_seen,1u);");
    cu = instrument_task_call(cu, 1 + g, "atomicAdd(&phase_seen,1u);");
    assert_eq!(cu.matches("deliver(parent,num(value));").count(), 2);
    cu = cu.replace("deliver(parent,num(value));", "deliver(parent,num(atomicAdd(&phase_seen,0u)));");
    cu.push_str(&format!(r#"
extern "C" __global__ void k_boot(u64,u64,u64,int fuel) {{
 stack_mark(); s_mode[threadIdx.x]=0; s_fuel=fuel;
 u64 a[]={{num(3)}}, b[]={{num(19)}};
 spawn_call({f},a,1,ROOT); spawn_call({f},b,1,ROOT);
 u64 c[]={{num(7),num(13)}}, d[]={{num(2),num(18)}};
 spawn_call({g},c,2,ROOT); spawn_call({g},d,2,ROOT);
}}
"#, f=1+f, g=1+g));
    std::env::set_var("MITHRIL_GPU_GROW_WIDTH", "2");
    for lanes in ["256", "65536"] {
        std::env::set_var("MITHRIL_GPU_LANES", lanes);
        let got = mithril_gpu::compile_and_run(&cu, Redex::default(), &std::env::temp_dir().join("mithril-native-phases")).unwrap();
        assert_eq!(got.text, "4", "lanes={lanes}");
        assert_eq!(got.rounds, 3, "two native dispatches and the final empty phase");
    }
    std::env::remove_var("MITHRIL_GPU_LANES");
    std::env::remove_var("MITHRIL_GPU_GROW_WIDTH");
    std::env::remove_var("MITHRIL_PLAIN_INTS");
}

#[test]
#[ignore = "requires CUDA and the nvcc image; changes device settings"]
fn recursive_native_frames_spill_without_growing_the_cuda_stack() {
    let src = "def count(n):\n    if n == 0:\n        return 0\n    return count(n - 1) + n\n\ndef main():\n    return count(array_len(array_new(600, 0)))\n";
    let m = mithril_front::desugar(&mithril_front::parse(src).unwrap()).unwrap();
    let cu = mithril_gpu::emit_cuda(&m).unwrap();
    let cache = std::env::temp_dir().join("mithril-native-tests");
    std::env::set_var("MITHRIL_GPU_STACK", "8192");
    for words in ["0", "1", "auto"] {
        std::env::set_var("MITHRIL_GPU_NATIVE_WORDS", words);
        let r = mithril_gpu::compile_and_run(&cu, Redex::default(), &cache);
        assert_eq!(r.map(|r| r.text), Ok("180300".into()), "cache words={words}");
    }
    std::env::remove_var("MITHRIL_GPU_NATIVE_WORDS");
    std::env::remove_var("MITHRIL_GPU_STACK");
}

#[test]
#[ignore = "requires CUDA and the nvcc image; changes device settings"]
fn native_regions_match_the_oracle_with_every_cache_mode() {
    let cases = [
        ("wide", "def f(n, x):\n    if n == 0:\n        return x\n    y = f(n - 1, x + n)\n    return y - x\n", "f(n, -1099511627776)"),
        ("nested", "def g(n):\n    if n == 0:\n        return 1\n    return g(n - 1) + n\n\ndef f(n):\n    if n == 0:\n        return 0\n    x = f(n - 1)\n    return x + g(n)\n", "f(n)"),
        ("multi_entry", "def f(n):\n    if n == 0:\n        return 1\n    return g(n - 1) + n\n\ndef g(n):\n    if n == 0:\n        return 2\n    return h(n - 1) + f(n - 1)\n\ndef h(n):\n    if n == 0:\n        return 3\n    return g(n - 1) * 2\n", "f(n)"),
        ("returns", "def g(n):\n    if n == 0:\n        return (2, 3, 5)\n    x = f(n - 1)\n    return (x, x + n, n)\n\ndef f(n):\n    if n == 0:\n        return 7\n    x = g(n - 1)\n    return x[2] + x[0] * 3 + x[1]\n", "f(n)"),
        ("loop", "def f(n):\n    s = n\n    for i in range(n):\n        s = s + f(i)\n    return s\n", "f(n)"),
    ];
    std::env::set_var("MITHRIL_GPU_STACK", "8192");
    for (name, body, call) in cases {
        let source = format!("{body}\ndef main():\n    n = array_len(array_new(7, 0))\n    return {call}\n");
        let m = mithril_front::desugar(&mithril_front::parse(&source).unwrap()).unwrap();
        let want = mithril_codegen::fmt_val(&mithril_front::eval_core(&m, m.main, &[]));
        {
            let cu = mithril_gpu::emit_cuda(&m).unwrap();
            for words in ["0", "1", "auto"] {
                std::env::set_var("MITHRIL_GPU_NATIVE_WORDS", words);
                let got = mithril_gpu::compile_and_run(&cu, Redex::default(), &std::env::temp_dir().join("mithril-native-matrix"));
                assert_eq!(got.map(|r| r.text), Ok(want.clone()), "{name}: cache={words}");
            }
        }
    }
    std::env::remove_var("MITHRIL_GPU_NATIVE_WORDS");
    std::env::remove_var("MITHRIL_GPU_STACK");
}

#[test]
#[ignore = "requires CUDA and the nvcc image; changes device settings"]
fn native_region_budget_keeps_carry_and_large_charges() {
    let source = "def main():\n    return array_len(array_new(1, 0))\n";
    let m = mithril_front::desugar(&mithril_front::parse(source).unwrap()).unwrap();
    let mut cu = mithril_gpu::emit_cuda(&m).unwrap().replace("#include \"engine.cu\"", "#define k_boot budget_unused_boot\n#include \"engine.cu\"\n#undef k_boot");
    cu.push_str(r#"
extern "C" __global__ void k_boot(u64 a, u64 b, u64 c, int budget) {
  stack_mark();
  s_mode[threadIdx.x] = 0;
  struct Case { i64 charge; u32 carry; i64 fuel, want_fuel; u32 want_carry; };
  Case cases[] = {
    {0, 0, 100, 100, 0},
    {17, 3, 100, 100, 20},
    {WORK_CAP - 1, 2, 100, -1, 1},
    {WORK_CAP + 5, 0, 100, -1, 5},
    {4294967303ll, 2, 100, -1, 9},
    {0, WORK_CAP - 1, 100, 100, WORK_CAP - 1},
  };
  bool pass = true;
  for (Case t : cases) {
    s_work[threadIdx.x] = t.carry;
    i64 fuel = t.fuel;
    native_work_fuel(&fuel, t.charge);
    pass &= fuel == t.want_fuel && s_work[threadIdx.x] == t.want_carry;
  }
  deliver(ROOT, num(pass));
}
"#);
    let got = mithril_gpu::compile_and_run(&cu, Redex::default(), &std::env::temp_dir().join("mithril-native-budget"));
    assert_eq!(got.map(|r| r.text), Ok("1".into()));
}


#[test]
#[ignore = "requires CUDA and the nvcc image; changes device settings"]
fn recursive_summaries_match_counts_in_grow_and_native_phases() {
    let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source=std::fs::read_to_string(root.join("crates/mithril-codegen/tests/fixtures/recursive_counts.py")).unwrap();
    let solve=source.split("def main():").next().unwrap();
    std::env::set_var("MITHRIL_GPU_STACK","8192");
    for seed in [0u32,u32::MAX] {
        let source=format!("{solve}\ndef main():\n    n = array_len(array_new(8, 0))\n    full = (1 << n) - 1\n    return solve(n, 2, 0, 1, 0, {seed}, {seed})\n");
        let m=mithril_front::desugar(&mithril_front::parse(&source).unwrap()).unwrap();
        let cu=mithril_gpu::emit_cuda(&m).unwrap();
        let want=format!("({}, {})",256u32.wrapping_add(seed),510u32.wrapping_add(seed));
        for width in ["1","256","32768"] {
            std::env::set_var("MITHRIL_GPU_GROW_WIDTH",width);
            for words in ["0","1","auto"] {
                std::env::set_var("MITHRIL_GPU_NATIVE_WORDS",words);
                let got=mithril_gpu::compile_and_run(&cu,Redex::default(),&std::env::temp_dir().join("mithril-recursive-summary"));
                assert_eq!(got.map(|r| r.text),Ok(want.clone()),"seed={seed} width={width} cache={words}");
            }
        }
    }
    std::env::remove_var("MITHRIL_GPU_GROW_WIDTH");
    std::env::remove_var("MITHRIL_GPU_NATIVE_WORDS");
    std::env::remove_var("MITHRIL_GPU_STACK");
}


#[test]
#[ignore = "requires CUDA and the nvcc image; changes device settings"]
fn variable_width_frames_preserve_lifo_order_across_partial_capacity() {
    let m=mithril_front::desugar(&mithril_front::parse("def main():\n    return array_len(array_new(1, 0))\n").unwrap()).unwrap();
    let mut cu=mithril_gpu::emit_cuda(&m).unwrap().replace("#define NATIVE_FRAMES 0","#define NATIVE_FRAMES 1").replace("#include \"engine.cu\"","#define k_boot frames_unused_boot\n#include \"engine.cu\"\n#undef k_boot");
    cu.push_str(r#"
extern "C" __global__ void k_boot(u64 a, u64 b, u64 c, int budget) {
 stack_mark(); s_mode[threadIdx.x]=0;
 bool pass=true;
 u32 capacities[]={0,1,3,7,20};
 for (u32 capacity : capacities) {
  s_native_top[threadIdx.x]=0;
  NativeFrames frames=native_frames(0);
  frames.capacity=capacity;
  for (u32 i=0;i<80;i++) {
   u32 width=i%3+1;
   u64 slot=native_reserve(&frames,width+2);
   for (u32 j=0;j<width;j++) native_set32(&frames,slot,j,1000*i+j);
   native_set64(&frames,slot,width,0x1234567800000000ull+i);
  }
  for (u32 i=80;i>0;) {
   --i;
   pass &= native_pop(&frames)==0x1234567800000000ull+i;
   u32 width=i%3+1;
   u64 slot=native_take(&frames,width);
   for (u32 j=width;j>0;) { --j; pass &= native_get32(&frames,slot,j)==1000*i+j; }
   native_release(&frames,slot);
  }
  pass &= native_empty(&frames);
  native_done(&frames);
 }
 deliver(ROOT,num(pass));
}
"#);
    let got=mithril_gpu::compile_and_run(&cu,Redex::default(),&std::env::temp_dir().join("mithril-native-mixed-frames"));
    assert_eq!(got.map(|r| r.text),Ok("1".into()));
}

#[test]
#[ignore = "requires CUDA and the nvcc image; changes device settings"]
fn typed_launches_preserve_integer_arguments_returns_and_spill() {
    std::env::set_var("MITHRIL_PLAIN_INTS","1");
    let cases=[
        ("unary", "def f(n):\n    if n == 0:\n        return 1\n    return (f(n - 1) * 31 + n) & 4294967295\n", "f(n)"),
        ("binary", "def f(n, x):\n    if n == 0:\n        return x & 4294967295\n    return (f(n - 1, x) + n) & 4294967295\n", "f(n, 4294967295)"),
        ("wide", "def f(n, a, b, c, d):\n    if n == 0:\n        return (a + b + c + d) & 4294967295\n    return (f(n - 1, a, b, c, d) * 3 + n) & 4294967295\n", "f(n, -1099511627776, 1099511627777, 17, 23)"),
        ("tuple", "def f(n, a, b, c):\n    if n == 0:\n        return (a, b, c)\n    w = f(n - 1, a, b, c)\n    return ((w[0] + n) & 4294967295, (w[1] * 3) & 4294967295, (w[2] - n) & 4294967295)\n", "f(n, 4294967295, 19, 7)"),
    ];
    std::env::set_var("MITHRIL_GPU_GROW_WIDTH","2");
    std::env::set_var("MITHRIL_GPU_STACK","8192");
    for (name,body,call) in cases {
        let source=format!("{body}\ndef main():\n    n = array_len(array_new(19, 0))\n    return {call}\n");
        let m=mithril_front::desugar(&mithril_front::parse(&source).unwrap()).unwrap();
        let want=mithril_codegen::fmt_val(&mithril_front::eval_core(&m,m.main,&[]));
        let (lir,module)=mithril_codegen::lower(&m);
        let fid=module.fns.iter().position(|f| f.name=="f").unwrap() as u32;
        assert!(lir.native_entries.contains(&fid),"{name} must exercise a typed native launch");
        let mut cu=mithril_gpu::emit_cuda(&m).unwrap().replace("#include \"engine.cu\"",
            "#define k_boot typed_unused_boot\n#include \"engine.cu\"\n#undef k_boot\n__device__ u32 typed_expect_words;");
        let args=match name {
            "unary"=>"num(19)",
            "binary"=>"num(19),num(4294967295ll)",
            "wide"=>"num(19),num(-1099511627776ll),num(1099511627777ll),num(17),num(23)",
            "tuple"=>"num(19),num(4294967295ll),num(19),num(7)",
            _=>unreachable!(),
        };
        cu = cu.replace(&format!("extern \"C\" __global__ void k_native_{}() {{",1+fid), &format!("extern \"C\" __global__ void k_native_{}() {{\nif(typed_expect_words!=0xffffffffu && g_native_words!=typed_expect_words) {{ g_abort(4); return; }}",1+fid));
        cu.push_str(&format!("extern \"C\" __global__ void k_boot(u64 expected_words,u64,u64,int fuel) {{\ntyped_expect_words=(u32)expected_words; stack_mark(); s_mode[threadIdx.x]=0; s_fuel=fuel;\nu64 args[]={{{args}}}; spawn_call({},args,{},ROOT); spawn_call({},args,{},ROOT);\n}}",1+fid,lir.dives[fid as usize].0,1+fid,lir.dives[fid as usize].0));
        for lanes in ["256","65536"] {
            std::env::set_var("MITHRIL_GPU_LANES",lanes);
            for words in ["0","1","auto"] {
                std::env::set_var("MITHRIL_GPU_NATIVE_WORDS",words);
                let got=mithril_gpu::compile_and_run(&cu,Redex { a:words.parse::<u32>().unwrap_or(u32::MAX) as u64,b:0,aux:0 },&std::env::temp_dir().join("mithril-typed-launch"));
                assert_eq!(got.map(|r| r.text),Ok(want.clone()),"{name} lanes={lanes} words={words}");
            }
        }
    }
    for key in ["MITHRIL_PLAIN_INTS","MITHRIL_GPU_GROW_WIDTH","MITHRIL_GPU_STACK","MITHRIL_GPU_LANES","MITHRIL_GPU_NATIVE_WORDS"] { std::env::remove_var(key); }
}

#[test]
#[ignore = "requires CUDA and the nvcc image; changes device settings"]
fn typed_launches_consume_every_task_across_ring_counter_wrap() {
    std::env::set_var("MITHRIL_PLAIN_INTS","1");
    std::env::set_var("MITHRIL_GPU_GROW_WIDTH","2");
    let src = "def f(n):\n    if n == 0:\n        return 1\n    return (f(n - 1) * 31 + n) & 4294967295\n\ndef main():\n    return f(array_len(array_new(3, 0)))\n";
    let m = mithril_front::desugar(&mithril_front::parse(src).unwrap()).unwrap();
    let (lir,m) = mithril_codegen::lower(&m);
    let fid = m.fns.iter().position(|f|f.name=="f").unwrap() as u32;
    assert!(lir.native_entries.contains(&fid));
    let source = mithril_gpu::emit_cuda(&m).unwrap();
    for (begin,count) in [(0u32,16u32),(u32::MAX-7,16),(u32::MAX-255,257)] {
        let mut cu = source.replace("#include \"engine.cu\"",
            "#define k_boot wrap_unused_boot\n#define k_native_done wrap_unused_done\n#include \"engine.cu\"\n#undef k_boot\n#undef k_native_done\n__device__ u32 wrap_seen=0;");
        cu = instrument_task_call(cu, 1 + fid, "atomicAdd(&wrap_seen,1u);");
        cu.push_str(&format!(r#"
extern "C" __global__ void k_boot(u64,u64,u64,int fuel) {{
 stack_mark(); s_mode[threadIdx.x]=0; s_fuel=fuel;
 G.blen[{rule}]=G.bdone[{rule}]={begin}u;
 u64 args[]={{num(3)}};
 for(u32 i=0;i<{count}u;i++) spawn_call({rule},args,1,ROOT);
}}
extern "C" __global__ void k_native_done(u32 rule) {{
 G.bdone[rule]=g_snap[rule]; g_rounds[2]++;
 deliver(ROOT,num(wrap_seen));
}}
"#,rule=1+fid));
        for lanes in ["256","65536"] {
            std::env::set_var("MITHRIL_GPU_LANES",lanes);
            let got = mithril_gpu::compile_and_run(&cu,Redex::default(),&std::env::temp_dir().join("mithril-typed-wrap"));
            assert_eq!(got.map(|r|r.text),Ok(count.to_string()),"begin={begin} count={count} lanes={lanes}");
        }
    }
    for key in ["MITHRIL_PLAIN_INTS","MITHRIL_GPU_GROW_WIDTH","MITHRIL_GPU_LANES"] { std::env::remove_var(key); }
}

#[test]
#[ignore = "requires CUDA; runs an isolated runaway typed launch"]
fn typed_timeout_abandons_and_the_next_process_can_run() {
    let started = std::time::Instant::now();
    let mut child = std::process::Command::new(std::env::current_exe().unwrap());
    child.args(["--ignored", "--exact", "typed_timeout_child", "--nocapture"]);
    for (key, _) in std::env::vars().filter(|(key, _)| key.starts_with("MITHRIL_GPU_")) {
        child.env_remove(key);
    }
    let mut child = child.env("MITHRIL_TYPED_TIMEOUT_CHILD", "1")
        .env("MITHRIL_GPU_TIMEOUT", "2").env("MITHRIL_GPU_EAGER", "1")
        .env("MITHRIL_GPU_GROW_WIDTH", "2").env("MITHRIL_PLAIN_INTS", "1")
        .stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped())
        .spawn().unwrap();
    while child.try_wait().unwrap().is_none() {
        if started.elapsed().as_secs() > 30 {
            child.kill().unwrap();
            panic!("the typed timeout child did not exit");
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let output = child.wait_with_output().unwrap();
    let log = String::from_utf8_lossy(&output.stdout).to_string() + &String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "child failed: {log}");
    assert!(log.contains("exceeded 2 s") && log.contains("abandoned until the process exits"), "{log}");
    let source = "def main():\n    return array_len(array_new(7, 0))\n";
    let m = mithril_front::desugar(&mithril_front::parse(source).unwrap()).unwrap();
    let cu = mithril_gpu::emit_cuda(&m).unwrap();
    let got = mithril_gpu::compile_and_run(&cu, Redex::default(), &std::env::temp_dir().join("mithril-typed-timeout"));
    assert_eq!(got.map(|r| r.text), Ok("7".into()));
}

#[test]
#[ignore = "invoked by the isolated typed timeout test"]
fn typed_timeout_child() {
    if std::env::var_os("MITHRIL_TYPED_TIMEOUT_CHILD").is_none() { return; }
    let source = "def f(n):\n    if n == 0:\n        return 1\n    s = 0\n    for i in range(n):\n        s = (s * 31 + i) & 4294967295\n    return (f(n - 1) + s) & 4294967295\n\ndef main():\n    return f(array_len(array_new(1, 0)) << 40)\n";
    let m = mithril_front::desugar(&mithril_front::parse(source).unwrap()).unwrap();
    let (lir, module) = mithril_codegen::lower(&m);
    let fid = module.fns.iter().position(|f| f.name == "f").unwrap() as u32;
    assert!(lir.native_entries.contains(&fid));
    let mut cu = mithril_gpu::emit_cuda(&m).unwrap().replace("#include \"engine.cu\"",
        "#define k_boot timeout_unused_boot\n#include \"engine.cu\"\n#undef k_boot");
    cu.push_str(&format!("extern \"C\" __global__ void k_boot(u64,u64,u64,int fuel) {{\nstack_mark(); s_mode[threadIdx.x]=0; s_fuel=fuel;\nu64 args[]={{num(1ll<<40)}};\nspawn_call({},args,1,ROOT); spawn_call({},args,1,ROOT);\n}}", 1+fid, 1+fid));
    let error = mithril_gpu::compile_and_run(&cu, Redex::default(), &std::env::temp_dir().join("mithril-typed-timeout"))
        .expect_err("the typed loop must exceed the deadline");
    println!("{error}");
    let error = mithril_gpu::compile_and_run(&cu, Redex::default(), &std::env::temp_dir().join("mithril-typed-timeout"))
        .expect_err("this process still has an abandoned kernel");
    assert!(error.contains("earlier run of this process was abandoned"));
}


// Instrument one exported task kernel, independent of its chosen entry name.
fn instrument_task_call(mut source: String, rule: u32, statement: &str) -> String {
    let header = format!("extern \"C\" __global__ void k_native_{rule}() {{");
    assert_eq!(source.matches(&header).count(), 1, "missing/duplicate task kernel");
    let start = source.find(&header).unwrap() + header.len();
    let mut depth = 1;
    let end = source[start..].char_indices().find_map(|(i, c)| {
        if c == '{' { depth += 1; } else if c == '}' { depth -= 1; }
        (depth == 0).then_some(start + i)
    }).expect("unclosed task kernel");
    let body = &source[start..end];
    assert_eq!(body.matches("auto value=").count(), 1, "task call must be instrumented exactly once");
    let call = start + body.find("auto value=").unwrap();
    source.insert_str(call, &format!("{statement} "));
    source
}

#[test]
fn completed_native_launches_print_checked_value_entries_and_keep_normal_charges() {
    let source = "def f(n):\n    if n == 0:\n        return 1\n    return (f(n - 1) * 31 + n) & 4294967295\n\ndef main():\n    return f(array_len(array_new(19, 0)))\n";
    let m = mithril_front::desugar(&mithril_front::parse(source).unwrap()).unwrap();
    let (program, m) = mithril_codegen::lower(&m);
    let fid = m.fns.iter().position(|f| f.name == "f").unwrap() as u32;
    let root = format!("s_{fid}");
    let (entries, clones) = mithril_codegen::lir::completed::entries(&program.fns, &[root.clone()]);
    assert!(entries.contains_key(&root));
    let cu = mithril_gpu::emit_cuda(&m).unwrap();
    assert!(cu.contains(&format!("auto value={}(a0);", entries[&root])));
    assert!(cu.contains("native_work_fuel(fuel, native_work)"), "ordinary entry must retain its charge");
    for f in clones { assert!(cu.contains(&format!(" {}(", f.name))); }
    let counted = instrument_task_call(cu, 1 + fid, "atomicAdd(&wrap_seen,1u);");
    assert_eq!(counted.matches("atomicAdd(&wrap_seen,1u);").count(), 1);
    std::env::remove_var("MITHRIL_PLAIN_INTS");
}

#[test]
#[ignore = "requires CUDA and nvcc"]
fn grouped_records_retain_values_until_pop_in_prefix_and_spill() {
    let m=mithril_front::desugar(&mithril_front::parse("def main():\n    return array_len(array_new(1, 0))\n").unwrap()).unwrap();
    let mut cu=mithril_gpu::emit_cuda(&m).unwrap().replace("#define NATIVE_FRAMES 0","#define NATIVE_FRAMES 1").replace("#include \"engine.cu\"","#define k_boot records_unused_boot\n#include \"engine.cu\"\n#undef k_boot");
    cu.push_str(r#"
template<int K> __device__ A<K> record_value(u64 depth) {
 A<K> value{};for(int i=0;i<K;i++)value.a[i]=(depth*0x9e3779b97f4a7c15ull)^(~0ull-i);return value;
}
template<int K> __device__ bool record_case(u32 capacity) {
 A<K> shape{};NativeFrames frames=native_records(shape);frames.capacity=capacity;
 bool pass=native_record_empty(&frames);
 for(u32 i=0;i<81;i++)native_record_push(&frames,record_value<K>(i));
 for(u32 i=81;i>0;) {
  --i;A<K> expected=record_value<K>(i),actual=native_record_read(&frames,shape);
  for(int j=0;j<K;j++)pass&=actual.a[j]==expected.a[j];
  expected=record_value<K>(i+1000);native_record_replace(&frames,expected);
  actual=native_record_read(&frames,shape);
  for(int j=0;j<K;j++)pass&=actual.a[j]==expected.a[j];
  native_record_pop(&frames,shape);
 }
 pass&=native_record_empty(&frames);native_record_done(&frames);return pass;
}
extern "C" __global__ void k_boot(u64,u64,u64,int) {
 if(threadIdx.x!=0)return;stack_mark();s_mode[threadIdx.x]=0;bool pass=true;
 u32 capacities[]={0,1,3,7,20};
 for(u32 capacity: capacities) {
  s_native_top[threadIdx.x]=0;
  pass&=record_case<0>(capacity);pass&=record_case<1>(capacity);pass&=record_case<2>(capacity);
  pass&=record_case<3>(capacity);pass&=record_case<7>(capacity);pass&=record_case<17>(capacity);
 }
 deliver(ROOT,num(pass));
}
"#);
    let got=mithril_gpu::compile_and_run(&cu,Redex::default(),&std::env::temp_dir().join("mithril-grouped-records"));
    assert_eq!(got.map(|r|r.text),Ok("1".into()));
}
