#!/usr/bin/env python3
"""Parity check: run a candidate `mithril` over the corpus and compare every
program/lane with the reference (tests/parity/reference.json, recorded from
the pinned reference binary, see build_ref.py).

The harness drives only the CLI, so any implementation can be checked:
  oracle   `mithril oracle f.py`                  (the reference interpreter)
  run_tN   `mithril run f.py --threads N`, N = 1, 4, 16
  fuelN    `mithril build f.py -o b`; `b --threads 4 --fuel N`, N = 1, 2, 7, 64
           (starved dive budgets; the budget is a flag of the built program,
           there is no environment variable for it)
  device   `mithril build --gpu f.py -o a`; `mithril exec a`
A lane's result is its exact stdout and exit status ("timeout" when it ran
out of time). stderr is kept (tail) for diagnosis but never compared.

Verdict per program/lane:
  EQUAL      same stdout and status as the reference (or as one of the
             outcomes a nondeterministic reference lane showed: its `alts`,
             recorded by --record --repeat N)
  FIXED      differs from the reference but equals the correct answer: the
             program's entry in known.json, else the reference oracle's
             output (when the oracle succeeded)
  REGRESSED  anything else
Exit status: 1 if anything REGRESSED, else 0.

Usage:
  tests/parity/check.py [--bin PATH] [--lanes L,..] [--only SUBSTR ..] [--jobs N]
  tests/parity/check.py --record        # re-record reference.json with the reference binary
  tests/parity/check.py --divergences   # lanes of the reference that differ from the correct answer
The candidate is --bin, else $MITHRIL_BIN, else target/release/mithril.
"""
import argparse
import hashlib
import json
import os
import shutil
import signal
import subprocess
import sys
import tempfile
import threading
import time
from concurrent.futures import ThreadPoolExecutor, as_completed

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
CORPUS = os.path.join(HERE, "corpus.json")
REFERENCE = os.path.join(HERE, "reference.json")
KNOWN = os.path.join(HERE, "known.json")
REF_BIN = os.path.join(ROOT, "target", "parity-ref", "mithril")
CAND_BIN = os.environ.get("MITHRIL_BIN", os.path.join(ROOT, "target", "release", "mithril"))

THREADS = (1, 4, 16)
FUELS = (1, 2, 7, 64)
FUEL_THREADS = 4
LANES = ["oracle"] + [f"run_t{t}" for t in THREADS] + [f"fuel{f}" for f in FUELS] + ["device"]
CPU_LANES = LANES[:-1]
MAX_INLINE = 4096
# the oracle copies arrays on update, so a program that is small for the net
# can need tens of GB there (lockless/histogram): cap it (the compiled
# programs reserve large virtual arenas, so only the oracle is capped)
ORACLE_MEM_KB = 6_000_000
# the arena reservation fast.py uses (reservations commit only what is used)
ENV = {k: v for k, v in os.environ.items() if k != "CARGO_TARGET_DIR"}
ENV.update(MITHRIL_NODES=str(1 << 28), MITHRIL_RECS=str(1 << 26))

GPU = threading.Lock()  # one device job at a time
NVCC = threading.Semaphore(4)


def run(cmd, timeout, cwd=None, mem_kb=None):
    """(stdout, rc | 'timeout', stderr tail). Kills the whole process group
    on timeout (`mithril run` has a child). `mem_kb` caps virtual memory."""
    lim = "ulimit -c 0;" + (f" ulimit -v {mem_kb};" if mem_kb else "")
    cmd = ["bash", "-c", lim + ' exec "$@"', "--"] + cmd
    p = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
                         errors="replace", env=ENV, cwd=cwd, start_new_session=True)
    try:
        out, err = p.communicate(timeout=timeout)
        rc = p.returncode
    except subprocess.TimeoutExpired:
        try:
            os.killpg(p.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        out, err = p.communicate()
        rc = "timeout"
    return out, rc, err[-400:]


def pack(out, rc, err):
    if len(out) > MAX_INLINE:
        out = f"sha256:{hashlib.sha256(out.encode()).hexdigest()}:{len(out)}"
    r = {"out": out, "rc": rc}
    if err.strip() and rc != 0:
        r["err"] = err.strip()[-300:]
    return r


def prepare(prog, tmp):
    src = os.path.join(ROOT, prog["path"])
    dst = os.path.join(tmp, os.path.basename(prog["path"]))
    shutil.copy(src, dst)
    for e in prog.get("sed", []):
        subprocess.run(["sed", "-i", e, dst], check=True)
    return dst


def run_program(binp, prog, lanes, workroot):
    tmp = tempfile.mkdtemp(prefix=prog["name"].replace("/", "_") + "-", dir=workroot)
    try:
        src = prepare(prog, tmp)
        cap = prog.get("timeout", 120)
        res = {}
        if "oracle" in lanes:
            res["oracle"] = pack(*run([binp, "oracle", src], cap, mem_kb=ORACLE_MEM_KB))
        for t in THREADS:
            if f"run_t{t}" in lanes:
                res[f"run_t{t}"] = pack(*run([binp, "run", src, "--threads", str(t)], cap))
        fl = [f"fuel{f}" for f in FUELS if f"fuel{f}" in lanes]
        if fl:
            exe = os.path.join(tmp, "prog")
            out, rc, err = run([binp, "build", src, "-o", exe], cap)
            for f in FUELS:
                if f"fuel{f}" not in lanes:
                    continue
                if rc != 0:
                    res[f"fuel{f}"] = pack("", rc, "build: " + err)
                else:
                    res[f"fuel{f}"] = pack(*run([exe, "--threads", str(FUEL_THREADS), "--fuel", str(f)], cap))
        if "device" in lanes:
            art = os.path.join(tmp, "prog.gpu")
            with NVCC:
                out, rc, err = run([binp, "build", "--gpu", src, "-o", art], max(cap, 600))
            if rc != 0:
                res["device"] = pack("", rc, "build: " + err)
            else:
                with GPU:
                    res["device"] = pack(*run([binp, "exec", art], cap))
        return res
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


def run_corpus(binp, progs, lanes, jobs):
    workroot = tempfile.mkdtemp(prefix="mithril-parity-")
    results = {}
    t0 = time.time()
    with ThreadPoolExecutor(max_workers=jobs) as ex:
        futs = {ex.submit(run_program, binp, p, lanes, workroot): p["name"] for p in progs}
        for i, f in enumerate(as_completed(futs), 1):
            results[futs[f]] = f.result()
            if sys.stderr.isatty():
                print(f"\r{i}/{len(progs)} {time.time() - t0:.0f}s", end="", file=sys.stderr, flush=True)
    if sys.stderr.isatty():
        print(file=sys.stderr)
    shutil.rmtree(workroot, ignore_errors=True)
    return results, time.time() - t0


def same(a, b):
    return a is not None and b is not None and a["out"] == b["out"] and a["rc"] == b["rc"]


def matches_ref(cand, refl):
    """Equal to the reference, or to one of the other outcomes the
    reference showed on repeated runs (`alts`: a lane of the reference
    that is not deterministic, e.g. a crash that depends on the schedule)."""
    return same(cand, refl) or any(same(cand, x) for x in refl.get("alts", []))


def correct(name, ref, known):
    """The correct answer for a program: known.json, else the reference oracle."""
    if name in known:
        return known[name]["correct"]
    o = ref.get(name, {}).get("oracle")
    return o if o and o["rc"] == 0 else None


def verdict(cand, refl, corr):
    if matches_ref(cand, refl):
        return "EQUAL"
    if corr is not None and same(cand, corr):
        return "FIXED"
    return "REGRESSED"


def show(r):
    if r is None:
        return "-"
    s = r["out"].strip().replace("\n", "\\n")
    s = s if len(s) <= 90 else s[:87] + "..."
    return s if r["rc"] == 0 else f"{s} [rc={r['rc']}] {r.get('err', '')[-120:]!r}"


def divergences(ref, known, lanes):
    """(program, lane, got, correct) for every reference lane that differs
    from the correct answer."""
    out = []
    for name, lr in sorted(ref.items()):
        corr = correct(name, ref, known)
        if corr is None:
            continue
        for lane in lanes:
            if lane in lr:
                for r in [lr[lane]] + lr[lane].get("alts", []):
                    if not same(r, corr):
                        out.append((name, lane, r, corr))
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--bin", default=None)
    ap.add_argument("--lanes", default=",".join(LANES), help="comma list; 'cpu' = all but device")
    ap.add_argument("--only", nargs="*", help="program name substrings")
    ap.add_argument("--jobs", type=int, default=os.cpu_count() or 8)
    ap.add_argument("--record", action="store_true", help="write reference.json from the reference binary")
    ap.add_argument("--repeat", type=int, default=1,
                    help="--record: run the corpus N times; outcomes that differ between runs are kept as the lane's alts")
    ap.add_argument("--divergences", action="store_true")
    ap.add_argument("--json", help="also write the candidate's results here")
    a = ap.parse_args()
    lanes = CPU_LANES if a.lanes == "cpu" else [l for l in a.lanes.split(",") if l]
    bad = [l for l in lanes if l not in LANES]
    if bad:
        sys.exit(f"unknown lanes {bad}; lanes: {LANES}")
    corpus = json.load(open(CORPUS))["programs"]
    progs = [p for p in corpus if not a.only or any(s in p["name"] for s in a.only)]
    known = json.load(open(KNOWN)) if os.path.exists(KNOWN) else {}
    known = {k: v for k, v in known.items() if not k.startswith("_")}
    ref = json.load(open(REFERENCE))["results"] if os.path.exists(REFERENCE) else {}

    if a.divergences:
        for name, lane, got, corr in divergences(ref, known, lanes):
            print(f"{name:34} {lane:8} got {show(got)}\n{'':34} {'':8} want {show(corr)}")
        return 0

    binp = a.bin or (REF_BIN if a.record else CAND_BIN)
    if not os.path.exists(binp):
        sys.exit(f"missing {binp} (reference: tests/parity/build_ref.py; candidate: cargo build --release -p mithril-cli --features gpu)")
    print(f"binary {binp}; {len(progs)} programs; lanes {','.join(lanes)}; jobs {a.jobs}", flush=True)
    results, dt = run_corpus(binp, progs, lanes, a.jobs)
    alts = {}
    for i in range(1, a.repeat if a.record else 1):
        again, t = run_corpus(binp, progs, lanes, a.jobs)
        dt += t
        for name, lr in again.items():
            for lane, r in lr.items():
                seen = [results[name][lane]] + alts.get((name, lane), [])
                if not any(same(r, x) for x in seen):
                    alts.setdefault((name, lane), []).append(r)
    for (name, lane), xs in alts.items():
        results[name][lane]["alts"] = xs
        print(f"not deterministic: {name} {lane}: {show(results[name][lane])} | " + " | ".join(show(x) for x in xs))

    if a.json:
        json.dump(results, open(a.json, "w"), indent=1, sort_keys=True)
    if a.record:
        doc = json.load(open(REFERENCE)) if os.path.exists(REFERENCE) else {"results": {}}
        for name, lr in results.items():
            old = doc["results"].setdefault(name, {})
            for lane, r in lr.items():
                # a lane seen to vary keeps every outcome ever recorded
                prev = old.get(lane)
                if prev is not None and (prev.get("alts") or r.get("alts")):
                    xs = [x for x in [prev] + prev.get("alts", []) + r.get("alts", []) if not same(x, r)]
                    r = dict(r, alts=[{k: v for k, v in x.items() if k != "alts"} for i, x in enumerate(xs)
                                      if not any(same(x, y) for y in xs[:i])])
                old[lane] = r
        doc["_comment"] = "Reference outputs (stdout, exit status) per program per lane; recorded by check.py --record from target/parity-ref/mithril (build_ref.py)."
        doc["results"] = dict(sorted(doc["results"].items()))
        with open(REFERENCE, "w") as f:
            json.dump(doc, f, indent=1, sort_keys=True)
            f.write("\n")
        print(f"recorded {len(results)} programs in {dt:.0f}s -> {REFERENCE}")
        return 0

    counts = {l: {"EQUAL": 0, "FIXED": 0, "REGRESSED": 0} for l in lanes}
    notes = []
    for name in sorted(results):
        corr = correct(name, ref, known)
        for lane in lanes:
            refl = ref.get(name, {}).get(lane)
            if refl is None:
                notes.append(("NOREF", name, lane, results[name][lane], None))
                continue
            v = verdict(results[name][lane], refl, corr)
            counts[lane][v] += 1
            if v != "EQUAL":
                notes.append((v, name, lane, results[name][lane], refl))
    print(f"{'lane':8} {'EQUAL':>6} {'FIXED':>6} {'REGRESSED':>9}")
    for l in lanes:
        c = counts[l]
        print(f"{l:8} {c['EQUAL']:6} {c['FIXED']:6} {c['REGRESSED']:9}")
    for v, name, lane, got, refl in notes:
        print(f"{v:9} {name:34} {lane:8} got {show(got)}" + (f"\n{'':9} {'':34} {'':8} ref {show(refl)}" if refl else " (no reference for this lane)"))
    reg = sum(c["REGRESSED"] for c in counts.values())
    print(f"total {dt:.0f}s, {reg} regressed, {sum(c['FIXED'] for c in counts.values())} fixed")
    return 1 if reg else 0


if __name__ == "__main__":
    sys.exit(main())
