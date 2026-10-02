#!/usr/bin/env python3
"""Performance parity: the spatial-tree benchmark and the 2 demos at their BIG size (the
unmodified sources), built once each; per program:
  cpu_t1, cpu_t16   wall seconds of the built program (--threads 1 / 16)
  dev_wall          wall seconds of `mithril exec` on the `build --gpu` artefact
  dev_kernel_ms     the device run time the runtime reports
                    (MITHRIL_GPU_STATS=1: "setup .. ms, run <ms> ms on ..")
Each measurement: wait until the 1-minute load average is below 2, one
warm-up run, then the minimum of 3. The printed value of every run is kept
and must agree across runs and lanes (reported, not timed).

  tests/parity/perf.py --record          measure the reference (target/parity-ref/mithril)
                                          into perf_baseline.json
  tests/parity/perf.py [--bin PATH]      measure a candidate, print ratios to the
                                          baseline (candidate / reference; no threshold yet)
Options: --only NAME.., --lanes cpu_t1,cpu_t16,device
"""
import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
BASELINE = os.path.join(HERE, "perf_baseline.json")
REF_BIN = os.path.join(ROOT, "target", "parity-ref", "mithril")
CAND_BIN = os.environ.get("MITHRIL_BIN", os.path.join(ROOT, "target", "release", "mithril"))
PORTS = ["kdtree"]
PROGRAMS = [(n, f"bench/ports/{n}.py") for n in PORTS] + [
    ("cornell_whitted", "demos/cornell_whitted.py"), ("cornell_path", "demos/cornell_path.py")]
# the big-size arena of tests/e2e/run.sh (u32 cap; reservations commit only what is used)
ENV = {k: v for k, v in os.environ.items() if k != "CARGO_TARGET_DIR"}
ENV.update(MITHRIL_NODES="4294967295", MITHRIL_RECS=str(1 << 28))
CAP = 1800
REPS = 3
LOAD_MAX = 2.0


def wait_load():
    t0 = time.time()
    while os.getloadavg()[0] >= LOAD_MAX:
        if time.time() - t0 > 900:
            print(f"  (load still {os.getloadavg()[0]:.1f} after 15 min; measuring anyway)", flush=True)
            return
        time.sleep(5)


def once(cmd, env=None):
    t0 = time.time()
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=CAP, env=env or ENV)
    except subprocess.TimeoutExpired:
        return None, "TIMEOUT", ""
    dt = time.time() - t0
    out = r.stdout.strip().split("\n")[-1] if r.stdout.strip() else ""
    if r.returncode != 0:
        return None, f"rc={r.returncode} {r.stderr.strip()[-160:]}", r.stderr
    return dt, out, r.stderr


def digest(out):
    """A long printed value (the demos' images) is kept as its hash."""
    return out if len(out) <= 200 else f"sha256:{hashlib.sha256(out.encode()).hexdigest()[:16]}:{len(out)}"


def best(cmd, env=None, kernel=False):
    """min of REPS after one warm-up: (seconds, kernel ms or None, output)."""
    wait_load()
    dt, out, _ = once(cmd, env)
    if dt is None:
        return None, None, out
    walls, kms, outs = [], [], set()
    for _ in range(REPS):
        dt, out, err = once(cmd, env)
        if dt is None:
            return None, None, out
        walls.append(dt)
        outs.add(digest(out))
        m = re.search(r"mithril-gpu: setup \d+ ms, run (\d+) ms", err)
        if kernel and m:
            kms.append(int(m.group(1)))
    return round(min(walls), 4), (min(kms) if kms else None), (outs.pop() if len(outs) == 1 else f"DIFFERING {sorted(outs)}")


def measure(binp, name, path, lanes, tmp):
    src = os.path.join(ROOT, path)
    r = {}
    if lanes & {"cpu_t1", "cpu_t16"}:
        exe = os.path.join(tmp, name)
        b = subprocess.run([binp, "build", src, "-o", exe], capture_output=True, text=True, env=ENV)
        if b.returncode != 0:
            r["cpu_error"] = b.stderr.strip()[-200:]
        else:
            for t in (1, 16):
                if f"cpu_t{t}" in lanes:
                    s, _, out = best([exe, "--threads", str(t)])
                    r[f"cpu_t{t}"], r[f"out_t{t}"] = s, out
    if "device" in lanes:
        art = os.path.join(tmp, name + ".gpu")
        b = subprocess.run([binp, "build", "--gpu", src, "-o", art], capture_output=True, text=True, env=ENV)
        if b.returncode != 0:
            r["dev_error"] = b.stderr.strip()[-200:]
        else:
            s, k, out = best([binp, "exec", art], dict(ENV, MITHRIL_GPU_STATS="1"), kernel=True)
            r["dev_wall"], r["dev_kernel_ms"], r["out_dev"] = s, k, out
    return r


def fmt(v):
    return "-" if v is None else (f"{v:.3f}" if isinstance(v, float) else str(v))


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--bin")
    ap.add_argument("--record", action="store_true")
    ap.add_argument("--only", nargs="*")
    ap.add_argument("--lanes", default="cpu_t1,cpu_t16,device")
    a = ap.parse_args()
    lanes = set(a.lanes.split(","))
    binp = a.bin or (REF_BIN if a.record else CAND_BIN)
    progs = [(n, p) for n, p in PROGRAMS if not a.only or n in a.only]
    base = json.load(open(BASELINE))["results"] if os.path.exists(BASELINE) else {}
    tmp = tempfile.mkdtemp(prefix="mithril-perf-")
    keys = ["cpu_t1", "cpu_t16", "dev_wall", "dev_kernel_ms"]
    print(f"binary {binp}")
    print(f"{'program':16} " + " ".join(f"{k:>13}" for k in keys) + "  output")
    results = {}
    try:
        for n, p in progs:
            r = measure(binp, n, p, lanes, tmp)
            results[n] = r
            outs = {r.get(k) for k in ("out_t1", "out_t16", "out_dev") if r.get(k) is not None}
            cells = []
            for k in keys:
                v, b = r.get(k), base.get(n, {}).get(k)
                cells.append(f"{fmt(v):>13}" if a.record or not (v and b) else f"{fmt(v)} {v / b:4.2f}x".rjust(13))
            same = outs.pop() if len(outs) == 1 else f"DISAGREE {sorted(outs)}"
            errs = " ".join(r[k] for k in ("cpu_error", "dev_error") if k in r)
            print(f"{n:16} " + " ".join(cells) + f"  {same} {errs}", flush=True)
    finally:
        shutil.rmtree(tmp, ignore_errors=True)
    if a.record:
        doc = json.load(open(BASELINE)) if os.path.exists(BASELINE) else {"results": {}}
        doc["results"].update(results)
        doc["_comment"] = ("Reference perf (target/parity-ref/mithril), big sizes; seconds (wall) and device "
                           "kernel ms; min of 3 after a warm-up at load < 2. Machine: 16 threads, RTX 4090.")
        with open(BASELINE, "w") as f:
            json.dump(doc, f, indent=1, sort_keys=True)
            f.write("\n")
        print(f"baseline written: {BASELINE}")
    else:
        print("ratios are candidate / reference (lower is faster)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
