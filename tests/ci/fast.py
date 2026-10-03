#!/usr/bin/env python3
"""Fast regression gate for Mithril: a light version of every check the
benchmarks and demos make, in about a minute.

Programs: the bench ports (bench/ports), both demos (demos/, a tiny image)
and the generality corpus (bench/general, small sizes). For each, in
parallel with memory-capped builds:

  correctness  small build, --threads 1 and 16, exact value (a fixed
               expected checksum, or the reference oracle's value recorded
               by --update)
  device       the same small build on the GPU (`build --gpu` + `exec`),
               exact value (skipped without a GPU or with --no-gpu)
  lowering     the set of functions native-scalar-lowered (fn s_<id>) must
               not shrink (a function silently falling back to tagged dive
               code is a 5-30x sequential regression)
  code size    generated lines and segment count within tolerance
  compile time code generation of the unmodified program (dump_gen) and the
               small build, within tolerance of baseline: a specializer or
               codegen blowup fails here, not in a demo an hour later
  perf         instruction count (perf stat, noise-free) of a mid-size run
               within tolerance of baseline; the device run time of a
               mid-size demo within a looser tolerance

Everything is compared against tests/ci/baseline.json. `--update` rewrites
the baseline from the current toolchain, checking every program without a
fixed expected value against `mithril oracle` first (do this only from a
verified state).

Usage: tests/ci/fast.py [--update] [--only NAME ...] [--jobs N] [--no-gpu]
Exit status: number of hard failures.
"""
import argparse
import json
import os
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
TARGET = os.environ.get("CARGO_TARGET_DIR", os.path.join(ROOT, "target"))
BIN = os.environ.get("MITHRIL_BIN", os.path.join(TARGET, "release", "mithril"))
DUMP = os.path.join(TARGET, "release", "examples", "dump_gen")

BUILD_MEM_KB = 12_000_000
# a compile beyond this is a blowup (the gate's own budget)
BUILD_CAP_S = 60
RUN_CAP_S = 60
GPU_LOCK = __import__("threading").Lock()  # one device run at a time
GPU_BUILDS = __import__("threading").Semaphore(3)  # nvcc builds at once
GPU_BUILD_CAP_S = 180

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
# the device run time of these mid-size variants (MITHRIL_GPU_STATS)
GPU_MID = {"whitted", "heat2d", "path"}
# programs checked on the device (each costs an nvcc build)
GPU_PROGRAMS = set(SMALL) | {"whitted", "path"}
MID.setdefault("path", ["s/^    return 256$/    return 32/", "s/^    return 64$/    return 8/"])

TOL = {"segments": 1.5, "gen_lines": 1.3, "instr": 1.15, "build_s": 2.0, "gen_s": 2.0, "gpu_ms": 1.5}
SOFT = set()
# below these a time is noise, never a failure
FLOOR = {"build_s": 2.0, "gen_s": 1.0, "gpu_ms": 20.0}
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
    return ["bash", "-c", f"ulimit -v {BUILD_MEM_KB}; exec \"$@\"", "--"] + cmd


def source(name):
    return EXTRA[name][0] if name in EXTRA else os.path.join(PORTS, f"{name}.py")


def variant(name, subs, tmp, tag):
    src = os.path.join(tmp, f"{name}_{tag}.py")
    shutil.copy(source(name), src)
    for e in subs:
        subprocess.run(["sed", "-i", e, src], check=True)
    return src


def instructions(cmd, cap):
    rc, out, err, dt = run(["perf", "stat", "-x,", "-e", "instructions:u"] + cmd, cap, env=RUN_ENV)
    if rc != 0:
        return None, None
    m = re.search(r"^(\d+),,instructions:u", err, re.M)
    val = (out.strip().split("\n") or [""])[-1]
    return (int(m.group(1)) if m else None), val


def last(out, err):
    return out.strip().split("\n")[-1] if out.strip() else err.strip()[-100:]


def gpu_ms(err):
    m = re.search(r"run (\d+) ms on", err)
    return int(m.group(1)) if m else None


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
    if gpu and name in GPU_PROGRAMS:
        art = os.path.join(tmp, f"{name}.small.gpu")
        with GPU_BUILDS:
            rc, out, err, dt = run(capped([BIN, "build", "--gpu", src, "-o", art]), GPU_BUILD_CAP_S)
        if rc != 0:
            r["gpu"] = "build: " + err.strip()[-100:]
        else:
            with GPU_LOCK:
                rc, out, err, dt = run([BIN, "exec", art], RUN_CAP_S)
            r["gpu"] = last(out, err)
        r["gpu_ok"] = r["gpu"] == expect
    if name in MID:
        msrc = variant(name, MID[name], tmp, "mid")
        mbin = os.path.join(tmp, f"{name}.mid")
        rc, out, err, dt = run(capped([BIN, "build", msrc, "-o", mbin]), BUILD_CAP_S)
        if rc == 0:
            ins, v1 = instructions([mbin, "--threads", "1"], RUN_CAP_S)
            rc2, out2, err2, _ = run([mbin, "--threads", "16"], RUN_CAP_S, env=RUN_ENV)
            v16 = out2.strip().split("\n")[-1] if out2.strip() else ""
            r["instr"] = ins
            r["mid_agree"] = (v1 is not None and v1 == v16)
            if "arena exhausted" in err2 or ins is None:
                r["mid_exhausted"] = True
            if gpu and name in GPU_MID:
                art = os.path.join(tmp, f"{name}.mid.gpu")
                with GPU_BUILDS:
                    built = run(capped([BIN, "build", "--gpu", msrc, "-o", art]), GPU_BUILD_CAP_S)[0] == 0
                if built:
                    best = None
                    for _ in range(2):
                        with GPU_LOCK:
                            rc3, out3, err3, _ = run([BIN, "exec", art], RUN_CAP_S, env=dict(RUN_ENV, MITHRIL_GPU_STATS="1"))
                        ms = gpu_ms(err3)
                        if rc3 == 0 and ms is not None:
                            best = ms if best is None else min(best, ms)
                        if last(out3, "") != v1:
                            r["gpu_mid_differs"] = True
                    r["gpu_ms"] = best
    return r


def compare(cur, base):
    """Returns (hard failures, soft warnings) as lists of strings."""
    hard, soft = [], []
    if "error" in cur:
        return [cur["error"]], soft
    if cur.get("small_ok") is False:
        hard.append(f"checksum small t1={str(cur.get('small_t1'))[:40]} t16={str(cur.get('small_t16'))[:40]} expect={str(cur.get('expect'))[:40]}")
    if cur.get("gpu_ok") is False:
        hard.append(f"device small {str(cur.get('gpu'))[:60]}")
    if cur.get("gpu_mid_differs"):
        hard.append("device mid-size value differs from the CPU's")
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
    a = ap.parse_args()
    for p in (BIN, DUMP):
        if not os.path.exists(p):
            sys.exit(f"missing {p}: cargo build --release -p mithril-cli --example dump_gen && cargo build --release -p mithril-cli")
    names = a.only or sorted(SMALL) + sorted(EXTRA)
    base = json.load(open(BASELINE)) if os.path.exists(BASELINE) else {}
    gpu = HAS_GPU and not a.no_gpu
    tmp = tempfile.mkdtemp(prefix="mithril-ci-")
    t0 = time.time()
    with ThreadPoolExecutor(max_workers=a.jobs) as ex:
        results = list(ex.map(lambda n: measure(n, tmp, gpu, a.update, base), names))
    shutil.rmtree(tmp, ignore_errors=True)
    fails = 0
    print(f"{'program':15} {'small':5} {'gpu':4} {'scalar':>6} {'segs':>5} {'lines':>6} {'gen':>5} {'build':>6} {'instr(G)':>9} {'gpu ms':>7}  status")
    for r in results:
        b = base.get(r["name"], {})
        hard, soft = compare(r, b)
        fails += bool(hard)
        ins = r.get("instr")
        insg = f"{ins / 1e9:9.3f}" if ins else f"{'-':>9}"
        st = "OK" if not hard else "FAIL"
        notes = "; ".join(hard + [f"warn: {s}" for s in soft])
        small = {True: "PASS", False: "FAIL", None: "-"}[r.get("small_ok")]
        dev = {True: "PASS", False: "FAIL", None: "-"}[r.get("gpu_ok")]
        print(f"{r['name']:15} {small:5} {dev:4} {len(r.get('scalar', [])):6} {r.get('segments', '-'):>5} {r.get('gen_lines', '-'):>6} {r.get('gen_s', '-'):>5} {r.get('build_s', '-'):>6} {insg} {str(r.get('gpu_ms') or '-'):>7}  {st} {notes}")
    print(f"total {time.time() - t0:.1f}s, {fails} failing")
    if a.update:
        # merge: `--update --only X` refreshes X and keeps every other entry
        base = json.load(open(BASELINE)) if os.path.exists(BASELINE) else {}
        base.update({r["name"]: r for r in results})
        json.dump(base, open(BASELINE, "w"), indent=1, sort_keys=True)
        print(f"baseline written: {BASELINE}")
    sys.exit(fails)


if __name__ == "__main__":
    main()
