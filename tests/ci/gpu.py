#!/usr/bin/env python3
"""GPU lane of the gate: every port at its small size, run on the device
through `mithril run --gpu` (the same lowering as the CPU program, printed
as CUDA), must print the same checksum fast.py expects from the CPU build.

Needs the 4090 and the docker nvcc image (blaze-ptx:cu13x). Programs are
compiled once per source hash (cached under target/mithril-cache/gpu), so a
warm run is seconds; a cold run is dominated by nvcc (~20 s per port).

Usage: tests/ci/gpu.py [--only NAME ...]
Exit status: number of failures.
"""
import argparse
import os
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from fast import PORTS, ROOT, SMALL, TARGET, variant  # noqa: E402

BIN = os.environ.get("MITHRIL_GPU_BIN", os.path.join(TARGET, "release", "mithril"))
RUN_CAP_S = 600


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--only", nargs="*", default=None)
    a = ap.parse_args()
    names = a.only or sorted(SMALL)
    subprocess.run(
        ["cargo", "build", "--release", "-p", "mithril-cli", "--features", "gpu"],
        cwd=ROOT, check=True, stdout=subprocess.DEVNULL,
    )
    tmp = tempfile.mkdtemp(prefix="mithril-gpu-")
    fails = 0
    print(f"{'bench':13} {'time':>7}  status")
    for name in names:
        subs, expect = SMALL[name]
        src = variant(name, subs, tmp, "gpu")
        t0 = time.time()
        try:
            r = subprocess.run([BIN, "run", src, "--gpu"], capture_output=True, text=True, timeout=RUN_CAP_S)
            out, err, rc = r.stdout.strip(), r.stderr.strip(), r.returncode
        except subprocess.TimeoutExpired:
            out, err, rc = "", "TIMEOUT", -1
        dt = time.time() - t0
        got = (out.split("\n") or [""])[-1]
        if rc == 0 and got == expect:
            st = "OK"
        else:
            st = f"FAIL got {got!r} want {expect!r}" + (f" ({err[-200:]})" if err else "")
            fails += 1
        print(f"{name:13} {dt:7.1f}  {st}")
    print(f"{fails} failing")
    return fails


if __name__ == "__main__":
    sys.exit(main())
