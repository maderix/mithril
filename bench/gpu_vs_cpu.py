#!/usr/bin/env python3
"""The device path measured: every port at fast.py's mid size, run by the
CPU wave runtime (`mithril run --threads N`) and by the device (`mithril
run --gpu`), results compared, wall times reported. A measurement, not a
gate: it says where the device path stands before any of its known costs
(runtime helpers as calls, fuel-64 dives, the bump heap) is touched.

Usage: bench/gpu_vs_cpu.py [--only NAME ...] [--threads N]
"""
import argparse
import os
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "tests", "ci"))
from fast import MID, SMALL, TARGET, variant  # noqa: E402

BIN = os.path.join(TARGET, "release", "mithril")
CAP = 900


def run(cmd, env=None):
    t0 = time.time()
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=CAP, env=env)
        out = (r.stdout.strip().split("\n") or [""])[-1] if r.returncode == 0 else f"rc={r.returncode} {r.stderr.strip()[-120:]}"
        stats = [l for l in r.stderr.split("\n") if l.startswith("mithril-gpu: setup")]
        run.stats = stats[-1][len("mithril-gpu: "):] if stats else ""
    except subprocess.TimeoutExpired:
        out = "TIMEOUT"
        run.stats = ""
    return out, time.time() - t0


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--only", nargs="*", default=None)
    ap.add_argument("--threads", type=int, default=16)
    a = ap.parse_args()
    names = a.only or sorted(SMALL)
    tmp = tempfile.mkdtemp(prefix="mithril-gpu-perf-")
    print(f"{'bench':13} {'size':5} {'cpu t1':>8} {'cpu t16':>8} {'gpu':>8}  agree")
    for name in names:
        subs = MID.get(name, SMALL[name][0])
        size = "mid" if name in MID else "small"
        src = variant(name, subs, tmp, "perf")
        exe = os.path.join(tmp, f"{name}.bin")
        subprocess.run([BIN, "build", src, "-o", exe], capture_output=True, text=True, timeout=CAP)
        c1, t1 = run([exe, "1"])
        cn, tn = run([exe, str(a.threads)])
        g, tg = run([BIN, "run", src, "--gpu"], env={**os.environ, "MITHRIL_GPU_STATS": "1"})
        ok = "yes" if c1 == cn == g else f"NO cpu={c1} gpu={g}"
        print(f"{name:13} {size:5} {t1:8.2f} {tn:8.2f} {tg:8.2f}  {ok}  {run.stats}")


if __name__ == "__main__":
    main()
