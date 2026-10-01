//! Exercise frontier accounting with a conservative may-fork rule.
use mithril_gpu::compile_and_run;
use mithril_rt::Redex;

const PROGRAM: &str = r#"
#define PROG_NRULES 6
#define NFNS 1
#define NET_RULE 1
#define FILL_RULE 1
#define FWD_RULE 1
#define FIELD_RULE 1
#define WHOLE_RULE 1
#define RELINK_RULE 1
#include "engine.cu"
__device__ bool lin(u16) { return true; }
__device__ const u32 UNBOX_CID[1] = {0};
__device__ u32 unbox_cid(u64 slot) { return UNBOX_CID[slot]; }
__device__ bool prog_rec_rule(u32) { return false; }
__device__ bool prog_forks(u32 rule) { return rule == 1 || rule == 3 || rule == 4 || rule == 5; }
__device__ bool prog_mat_proj(u16, usize *) { return false; }
__device__ const u16 *prog_mat_arms(u16, int *n) { *n = 0; return nullptr; }
__device__ R prog_dive(u32, const u64 *, i64 *) { return R{0, true}; }
__device__ void prog_inst(u16, const u64 *, int, u64) { g_abort(AB_UNREACHABLE); }
__device__ void prog_fire(u32 rule, u64 delay, u64 depth, u64 extra) {
  if (rule == 0) {
    G.result[1] = num(0);
    for (u32 r = 1; r < PROG_NRULES; r++) G.blen[r] = G.bdone[r] = START_INDEX;
    for (u64 i = 0; i < (extra & 0xffffffffull); i++) spawn3(3, 0, 0, 0);
    spawn3(1, delay, depth, extra >> 32);
  } else if (rule == 3) {
    return;
  } else if (rule == 2) {
    atomicAdd(&G.result[1], 1ull);
    atomicExch(&G.result[0], 1ull);
  } else if (rule == 1 || rule == 5) {
    if (delay > 1) {
      if (extra && rule == 1) spawn3(3, 0, 0, 0);
      spawn3(extra && rule == 1 ? 5 : 1, delay - 1, depth, extra);
    } else spawn3(4, 0, depth, 0);
  } else if (depth) {
    spawn3(4, 0, depth - 1, 0);
    spawn3(4, 0, depth - 1, 0);
  } else {
    spawn3(2, 0, 0, 0);
  }
}
"#;

#[test]
#[ignore = "requires CUDA and the nvcc image"]
fn growth_requires_new_ready_work_for_a_rule() {
    if std::env::var("MITHRIL_GPU").ok().as_deref() != Some("1") {
        return;
    }
    let cache = std::env::temp_dir().join("mithril-frontier-tests");
    for start in [0, u32::MAX - 8] {
        let program = format!("#define START_INDEX {start}u\n{PROGRAM}");
        for steps in ["1073741824", "1"] {
            let old = std::env::var_os("MITHRIL_GPU_WORK_STEPS");
            std::env::set_var("MITHRIL_GPU_WORK_STEPS", steps);
            for oscillate in [0, 1] {
                for (delay, depth, siblings, leaves) in [
                    (0, 0, 0, "1"),
                    (128, 0, 0, "1"),
                    (0, 8, 0, "256"),
                    (128, 8, 0, "256"),
                    // Finished siblings narrow the frontier; the survivor can branch.
                    (1, 8, 127, "256"),
                    (1, 0, 127, "1"),
                    (128, 0, 127, "1"),
                    (128, 8, 127, "256"),
                ] {
                    let result = compile_and_run(
                        &program,
                        Redex {
                            a: delay,
                            b: depth,
                            aux: siblings | (oscillate << 32),
                        },
                        &cache,
                    )
                    .unwrap();
                    assert_eq!(result.text, leaves, "delay={delay} depth={depth} siblings={siblings} oscillate={oscillate} start={start} steps={steps}");
                    if steps != "1" {
                        if depth > 0 && (delay == 0 || (delay == 1 && siblings > 0)) {
                            assert!(
                                result.rounds >= depth + 1,
                                "genuine branching stopped growing before exposing its leaves"
                            );
                        }
                        assert!(
                            result.rounds <= 16,
                            "one-for-one continuation caused {} global rounds",
                            result.rounds
                        );
                    }
                }
            }
            if let Some(value) = old {
                std::env::set_var("MITHRIL_GPU_WORK_STEPS", value);
            } else {
                std::env::remove_var("MITHRIL_GPU_WORK_STEPS");
            }
        }
    }
}
