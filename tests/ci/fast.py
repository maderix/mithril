#!/usr/bin/env python3
"""Fast regression gate for Mithril: every program the benchmarks and demos
run, at full size, on the CPU and the GPU, in about two minutes.

Programs: the bench ports (bench/ports), both demos (demos/) and the
generality corpus (bench/general). Phase 1 builds everything in parallel
(memory-capped) and runs the small checks; phase 2 times the full-size runs,
the CPU and the device as two serialized lanes running side by side.

  correctness  small build, --threads 1 and 16, exact value (a fixed
               expected checksum, or the reference oracle's value recorded
               by --update)
  full size    the unmodified program at --threads 1 and 16 (wall time) and
               on the device (device time and process wall); the three
               values must agree and equal the baseline's
  lowering     the set of functions native-scalar-lowered (fn s_<id>) must
               not shrink
  code size    generated lines and segment count within tolerance
  compile time code generation (dump_gen) and the small build within
               tolerance: a specializer or codegen blowup fails here
  perf         instruction count (perf stat, noise-free) of a mid-size run;
               every full-size time within tolerance of the baseline. A
               device run past GPU_RUN_CAP_S is recorded as a timeout: a
               program that finished before and now times out fails

Everything is compared against tests/ci/baseline.json, recorded on the
primary platform (Linux x86-64). Elsewhere (macOS arm64, experimental: CPU
only) values, lowering and code size still compare against it, so every
platform must compute the same values; times compare against a baseline of
that platform's own (baseline-<os>-<arch>.json), and Linux-only signals
(perf instruction counts, the address-space cap) are skipped. `--update` rewrites
this platform's baseline from the current toolchain, checking every program without a
fixed expected value against `mithril oracle` first (do this only from a
verified state).

Usage: tests/ci/fast.py [--update] [--only NAME ...] [--jobs N] [--no-gpu]
Exit status: number of hard failures.
"""
import argparse
import json
import os
import platform
import re
import shutil
import subprocess
import sys
import tempfile
import time
from concurrent.futures import ThreadPoolExecutor

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
PORTS = os.path.join(ROOT, "bench", "ports")
BASELINE = os.path.join(ROOT, "tests", "ci", "baseline.json")
LINUX = sys.platform.startswith("linux")
PRIMARY = LINUX and platform.machine() == "x86_64"
# another platform's own times; its values must equal the primary baseline's
PLATFORM_BASELINE = os.path.join(ROOT, "tests", "ci", f"baseline-{sys.platform}-{platform.machine()}.json")
TIMES = ("instr", "build_s", "gen_s", "cuda_s", "full_t1_s", "full_t16_s", "gpu_ms", "gpu_wall_ms")
TARGET = os.environ.get("CARGO_TARGET_DIR", os.path.join(ROOT, "target"))
BIN = os.environ.get("MITHRIL_BIN", os.path.join(TARGET, "release", "mithril"))
DUMP = os.path.join(TARGET, "release", "examples", "dump_gen")

BUILD_MEM_KB = 12_000_000
# a compile beyond this is a blowup (the gate's own budget)
BUILD_CAP_S = 60
RUN_CAP_S = 60
GPU_LOCK = __import__("threading").Lock()  # one device run at a time
GPU_BUILDS = __import__("threading").Semaphore(8)  # nvcc builds at once
# the slowest device compile measured cold is near 250 s (cicc 156 s +
# ptxas 92 s for a 4-function program: the engine dominates)
GPU_BUILD_CAP_S = 600
# a full-size run past these is recorded as a timeout, not waited for
CPU_RUN_CAP_S = 30
GPU_RUN_CAP_S = 8

DEMOS = os.path.join(ROOT, "demos")
GENERAL = os.path.join(ROOT, "bench", "general")

# small size: (sed substitutions, expected checksum) — mirrors tests/e2e/run.sh
SMALL = {
    "collatz": (["s/return total(3000000)/return total(1000)/"], "59431"),
    "heat2d": (["s/return run(1024, 500)/return run(32, 10)/"], "131504068"),
    "histogram": (["s/hist(536870912)/hist(1000)/"], "4456"),
    "kdtree": (["s/    n = 18/    n = 10/", "s/return qbatch(t, 18, 0)/return qbatch(t, 10, 0)/"], "2478099586"),
    "knapsack": (["s/return solve(8000, 100000)/return solve(20, 1000)/"], "7590"),
    "msort": (["s/gen(524288, /gen(1000, /"], "873274365"),
    "subsetsum": (["s/    n = 32/    n = 16/"], "7379"),
}

# demos and the generality corpus: (path, sed substitutions); their value is
# the reference oracle's, recorded by --update
EXTRA = {
    "whitted": (os.path.join(DEMOS, "cornell_whitted.py"), ["s/^    return 512$/    return 8/"]),
    "path": (os.path.join(DEMOS, "cornell_path.py"), ["s/^    return 256$/    return 8/", "s/^    return 64$/    return 4/"]),
}
GENERAL_SMALL = {"collatz_mutual": 3000, "cow_versions": 3000, "dag_share": 3, "graph_dfs": 1, "interp": 5,
                 "persist_map": 2000, "sorts": 300, "stage_closure": 50, "pipeline_cfg": 20, "interp_closure": 200}
for g, n in GENERAL_SMALL.items():
    EXTRA[g] = (os.path.join(GENERAL, f"{g}.py"), [rf"s/^    return run([0-9]*)$/    return run({n})/"])

# mid size for the perf signals (no expected value; t1 and t16 must agree)
MID = {
    "heat2d": ["s/return run(1024, 500)/return run(256, 20)/"],
    "knapsack": ["s/return solve(8000, 100000)/return solve(200, 20000)/"],
    "whitted": ["s/^    return 512$/    return 48/"],
}
MID.setdefault("path", ["s/^    return 256$/    return 32/", "s/^    return 64$/    return 8/"])
TOL = {"segments": 1.5, "gen_lines": 1.3, "instr": 1.15, "build_s": 2.0, "gen_s": 2.0, "cuda_s": 1.25,
       "full_t1_s": 1.3, "full_t16_s": 1.5, "gpu_ms": 1.5, "gpu_wall_ms": 1.5}
SOFT = set()
# below these a time is noise, never a failure
FLOOR = {"build_s": 2.0, "gen_s": 1.0, "cuda_s": 10.0, "full_t1_s": 0.05, "full_t16_s": 0.05, "gpu_ms": 2.0, "gpu_wall_ms": 300.0}
HAS_GPU = shutil.which("nvidia-smi") is not None and subprocess.run(["nvidia-smi", "-L"], capture_output=True).returncode == 0
RUN_ENV = dict(os.environ, MITHRIL_NODES=str(1 << 28), MITHRIL_RECS=str(1 << 26))


def run(cmd, cap, env=None):
    t0 = time.time()
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=cap, env=env)
        return r.returncode, r.stdout, r.stderr, time.time() - t0
    except subprocess.TimeoutExpired:
        return -1, "", "TIMEOUT", time.time() - t0


def capped(cmd):
    # the address-space cap is Linux's; macOS refuses `ulimit -v`
    if not LINUX:
        return cmd
    return ["bash", "-c", f"ulimit -v {BUILD_MEM_KB}; exec \"$@\"", "--"] + cmd


def sed(text, expr):
    """Apply one `s/pattern/replacement/` (a basic regular expression, as the
    substitutions here are written) without depending on GNU sed."""
    _, pat, rep, _ = expr.split("/")
    for c in "(){}+?|":
        pat = pat.replace(c, "\\" + c)
    return re.sub(pat, rep.replace("\\", "\\\\"), text, flags=re.M)


def source(name):
    return EXTRA[name][0] if name in EXTRA else os.path.join(PORTS, f"{name}.py")


def variant(name, subs, tmp, tag):
    src = os.path.join(tmp, f"{name}_{tag}.py")
    text = open(source(name)).read()
    for e in subs:
        text = sed(text, e)
    open(src, "w").write(text)
    return src


def instructions(cmd, cap):
    if not LINUX:
        # no perf: the run still checks the value, the count is unknown
        rc, out, err, dt = run(cmd, cap, env=RUN_ENV)
        return None, (last(out, "") if rc == 0 else None)
    rc, out, err, dt = run(["perf", "stat", "-x,", "-e", "instructions:u"] + cmd, cap, env=RUN_ENV)
    if rc != 0:
        return None, None
    m = re.search(r"^(\d+),,instructions:u", err, re.M)
    val = (out.strip().split("\n") or [""])[-1]
    return (int(m.group(1)) if m else None), val


def last(out, err):
    return out.strip().split("\n")[-1] if out.strip() else err.strip()[-100:]


def gpu_ms(err):
    # every attempt counts: a run the stack guard aborts is rerun with a
    # doubled stack, and the program pays for both
    runs = re.findall(r"device events: boot [0-9.]+ ms, run ([0-9.]+) ms", err)
    return round(sum(map(float, runs)), 3) if runs else None


def measure(name, tmp, gpu, update, base):
    r = {"name": name}
    subs, expect = SMALL[name] if name in SMALL else (EXTRA[name][1], None)
    if expect is None:
        expect = None if update else base.get(name, {}).get("expect")
    src = variant(name, subs, tmp, "small")
    # code metrics and compile time come from the unmodified program: at
    # small sizes the compile-time reducer may fold the whole program
    gen = os.path.join(tmp, f"{name}.gen.rs")
    rc, out, err, dt = run(capped([DUMP, source(name), gen]), BUILD_CAP_S)
    r["gen_s"] = round(dt, 2)
    if rc == 0 and os.path.exists(gen):
        code = open(gen).read()
        r["gen_lines"] = code.count("\n")
        r["segments"] = len(re.findall(r"^fn sg_\d+\(", code, re.M))
        r["scalar"] = sorted(int(x) for x in re.findall(r"^fn s_(\d+)\(", code, re.M))
    else:
        r["error"] = f"dump_gen ({dt:.0f} s): " + err.strip()[-160:]
        return r
    binp = os.path.join(tmp, f"{name}.small")
    rc, out, err, dt = run(capped([BIN, "build", src, "-o", binp]), BUILD_CAP_S)
    r["build_s"] = round(dt, 2)
    if rc != 0:
        r["error"] = "build: " + err.strip()[-160:]
        return r
    for t in (1, 16):
        rc, out, err, dt = run([binp, "--threads", str(t)], RUN_CAP_S, env=RUN_ENV)
        r[f"small_t{t}"] = last(out, err)
    if update and expect is None:
        # no fixed value: the reference oracle decides, and is recorded
        rc, out, err, dt = run([BIN, "oracle", src], RUN_CAP_S * 2)
        expect = last(out, err) if rc == 0 else None
        if expect is None:
            r["error"] = "oracle: " + err.strip()[-160:]
            return r
    if name not in SMALL:
        r["expect"] = expect
    r["small_ok"] = expect is not None and r["small_t1"] == expect and r["small_t16"] == expect
    # full size: the unmodified program, built for the CPU and the device
    r["_cpu"] = os.path.join(tmp, f"{name}.full")
    if run(capped([BIN, "build", source(name), "-o", r["_cpu"]]), BUILD_CAP_S)[0] != 0:
        r["error"] = "full-size build failed"
        return r
    if gpu:
        r["_gpu"] = os.path.join(tmp, f"{name}.full.gpu")
        with GPU_BUILDS:
            rc, out, err, dt = run(capped([BIN, "build", "--gpu", source(name), "-o", r["_gpu"]]), GPU_BUILD_CAP_S)
            if rc != 0:
                r["error"] = f"device build failed ({dt:.0f} s): " + err.strip()[-160:]
                return r
            # the recorded compile (cicc + ptxas), cached or not: --cold-gpu recompiles
            m = re.search(r"device compile ([0-9.]+) s", out)
            if m:
                r["cuda_s"] = float(m.group(1))
    if name in MID:
        msrc = variant(name, MID[name], tmp, "mid")
        mbin = os.path.join(tmp, f"{name}.mid")
        rc, out, err, dt = run(capped([BIN, "build", msrc, "-o", mbin]), BUILD_CAP_S)
        if rc == 0:
            r["_mid"] = mbin
    return r


def time_cpu(r):
    """Full-size runs at 1 and 16 threads (wall time), and the mid-size
    instruction count; one program at a time."""
    vals = []
    for t in (1, 16):
        rc, out, err, dt = run([r["_cpu"], "--threads", str(t)], CPU_RUN_CAP_S, env=RUN_ENV)
        if rc == -1:
            r[f"full_t{t}_timeout"] = True
            continue
        r[f"full_t{t}_s"] = round(dt, 3)
        vals.append(last(out, err))
    r["full_value"] = vals[0] if vals else None
    r["full_agree"] = len(set(vals)) <= 1
    if "_mid" in r:
        ins, v1 = instructions([r["_mid"], "--threads", "1"], CPU_RUN_CAP_S)
        rc2, out2, err2, _ = run([r["_mid"], "--threads", "16"], CPU_RUN_CAP_S, env=RUN_ENV)
        r["instr"] = ins
        r["mid_agree"] = v1 is not None and v1 == last(out2, "")
        if "arena exhausted" in err2 or v1 is None:
            r["mid_exhausted"] = True


def time_gpu(r):
    """The full-size device run: device time (best of two when short) and
    process wall, capped at GPU_RUN_CAP_S; one program at a time."""
    best = wall = None
    for i in range(2):
        rc, out, err, dt = run([BIN, "exec", r["_gpu"]], GPU_RUN_CAP_S, env=dict(RUN_ENV, MITHRIL_GPU_STATS="1"))
        if rc == -1:
            r["gpu_timeout"] = True
            return
        ms = gpu_ms(err)
        best = ms if best is None or (ms is not None and ms < best) else best
        wall = round(dt * 1000) if wall is None else min(wall, round(dt * 1000))
        r["gpu"] = last(out, err)
        if dt > 1.0:
            break
    r["gpu_ms"], r["gpu_wall_ms"] = best, wall


def compare(cur, base):
    """Returns (hard failures, soft warnings) as lists of strings."""
    hard, soft = [], []
    if "error" in cur:
        return [cur["error"]], soft
    if cur.get("small_ok") is False:
        hard.append(f"checksum small t1={str(cur.get('small_t1'))[:40]} t16={str(cur.get('small_t16'))[:40]} expect={str(cur.get('expect'))[:40]}")
    if cur.get("full_agree") is False:
        hard.append("full-size values differ between --threads 1 and 16")
    if "gpu" in cur and cur.get("full_value") is not None and cur["gpu"] != cur["full_value"]:
        hard.append(f"device value {str(cur['gpu'])[:40]} != CPU {str(cur['full_value'])[:40]}")
    if base.get("full_value") and cur.get("full_value") and cur["full_value"] != base["full_value"]:
        hard.append(f"full-size value {str(cur['full_value'])[:40]} != baseline {str(base['full_value'])[:40]}")
    for k, t in (("gpu", "gpu_timeout"), ("full_t1", "full_t1_timeout"), ("full_t16", "full_t16_timeout")):
        if cur.get(t) and base.get(f"{k}_ms" if k == "gpu" else f"{k}_s"):
            hard.append(f"{k} now times out (baseline finished)")
    if cur.get("mid_exhausted"):
        hard.append("mid-size run exhausted the arena (memory regression)")
    elif "instr" in cur and cur.get("mid_agree") is False:
        hard.append("mid-size t1/t16 checksums differ")
    if not base:
        return hard, soft
    lost = sorted(set(base.get("scalar", [])) - set(cur.get("scalar", [])))
    if lost:
        hard.append(f"scalar lowering lost for fns {lost}")
    for k, tol in TOL.items():
        b, c = base.get(k), cur.get(k)
        if b and c and c > b * tol and c > FLOOR.get(k, 0):
            msg = f"{k} {c} > {tol}x baseline {b}"
            (soft if k in SOFT else hard).append(msg)
    return hard, soft


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--update", action="store_true")
    ap.add_argument("--only", nargs="*")
    ap.add_argument("--jobs", type=int, default=8)
    ap.add_argument("--no-gpu", action="store_true")
    ap.add_argument("--cold-gpu", action="store_true", help="compile every device program afresh (an engine or lowering change)")
    a = ap.parse_args()
    for p in (BIN, DUMP):
        if not os.path.exists(p):
            sys.exit(f"missing {p}: cargo build --release -p mithril-cli --example dump_gen && cargo build --release -p mithril-cli")
    names = a.only or sorted(SMALL) + sorted(EXTRA)
    base = json.load(open(BASELINE)) if os.path.exists(BASELINE) else {}
    if not PRIMARY:
        # values, lowering and code size from the primary baseline, times
        # from this platform's (none yet: times are not compared)
        own = json.load(open(PLATFORM_BASELINE)) if os.path.exists(PLATFORM_BASELINE) else {}
        for n, b in base.items():
            base[n] = {k: v for k, v in b.items() if k not in TIMES} | {k: v for k, v in own.get(n, {}).items() if k in TIMES}
    gpu = HAS_GPU and not a.no_gpu
    tmp = tempfile.mkdtemp(prefix="mithril-ci-")
    if a.cold_gpu:
        os.environ["MITHRIL_GPU_CACHE"] = os.path.join(tmp, "gpu-cache")
    t0 = time.time()
    with ThreadPoolExecutor(max_workers=a.jobs) as ex:
        results = list(ex.map(lambda n: measure(n, tmp, gpu, a.update, base), names))
    t1 = time.time()
    ready = [r for r in results if "error" not in r]
    with ThreadPoolExecutor(max_workers=2) as ex:
        lanes = [ex.submit(lambda: [time_cpu(r) for r in ready])]
        if gpu:
            lanes.append(ex.submit(lambda: [time_gpu(r) for r in ready if "_gpu" in r]))
        for lane in lanes:
            lane.result()
    t2 = time.time()
    shutil.rmtree(tmp, ignore_errors=True)
    for r in results:
        for k in [k for k in r if k.startswith("_")]:
            del r[k]
    fails = 0
    print(f"{'program':15} {'small':5} {'scalar':>6} {'segs':>5} {'lines':>6} {'gen':>5} {'build':>6} {'cuda':>6} {'instr(G)':>9} {'t1 s':>7} {'t16 s':>7} {'gpu ms':>9} {'gpu wall':>8}  status")
    for r in results:
        b = base.get(r["name"], {})
        hard, soft = compare(r, b)
        fails += bool(hard)
        ins = r.get("instr")
        insg = f"{ins / 1e9:9.3f}" if ins else f"{'-':>9}"
        st = "OK" if not hard else "FAIL"
        notes = "; ".join(hard + [f"warn: {s}" for s in soft])
        small = {True: "PASS", False: "FAIL", None: "-"}[r.get("small_ok")]
        tm = lambda k: "timeout" if r.get(f"{k}_timeout") else str(r.get(f"{k}_s", "-"))
        gms = "timeout" if r.get("gpu_timeout") else (f"{r['gpu_ms']:.1f}" if r.get("gpu_ms") is not None else "-")
        print(f"{r['name']:15} {small:5} {len(r.get('scalar', [])):6} {r.get('segments', '-'):>5} {r.get('gen_lines', '-'):>6} {r.get('gen_s', '-'):>5} {r.get('build_s', '-'):>6} {r.get('cuda_s', '-'):>6} {insg} {tm('full_t1'):>7} {tm('full_t16'):>7} {gms:>9} {str(r.get('gpu_wall_ms', '-')):>8}  {st} {notes}")
    print(f"total {time.time() - t0:.1f}s (builds and small checks {t1 - t0:.1f}s, timed runs {t2 - t1:.1f}s), {fails} failing")
    slowest = max((r for r in results if r.get("cuda_s")), key=lambda r: r["cuda_s"], default=None)
    if slowest:
        print(f"slowest device compile: {slowest['name']} {slowest['cuda_s']} s (cicc + ptxas, as recorded when it compiled)")
    if a.update:
        # merge: `--update --only X` refreshes X and keeps every other entry;
        # off the primary platform only this platform's times are written
        path = BASELINE if PRIMARY else PLATFORM_BASELINE
        base = json.load(open(path)) if os.path.exists(path) else {}
        base.update({r["name"]: r if PRIMARY else {k: v for k, v in r.items() if k in TIMES} for r in results})
        json.dump(base, open(path, "w"), indent=1, sort_keys=True)
        print(f"baseline written: {path}")
    sys.exit(fails)


if __name__ == "__main__":
    main()
