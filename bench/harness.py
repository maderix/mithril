#!/usr/bin/env python3
"""Mithril benchmark harness.

For every benchmark ported under bench/ports/<name>.py this runs three CPU
lanes (plus an optional GPU lane):

  * C      : the C twin bench/ports/<name>.c, compiled with `gcc -O2 -lm`
  * SEQ    : the Mithril program at `--threads 1`
  * PAR16  : the Mithril program at `--threads 16`
  * GPU    : `mithril run bench/ports/<name>.py --threads 16 --gpu`
             (only when MITHRIL_GPU=1 and --skip-gpu is not given)

The Mithril program is compiled ONCE per benchmark with
`mithril build bench/ports/<name>.py -o <tmp>/<name>` (identical pipeline to
`mithril run`, which is build + exec); the build time is recorded in the
notes and excluded from the lane times. The SEQ/PAR16 lanes then execute the
built binary with `--threads N`, exactly as `mithril run --threads N` does.

Timing: each lane is timed as wall clock. `--n N` fixes the number of runs
per lane; by default it is adaptive: 3 runs if the first run finished in
under 60 s, otherwise 1. The minimum wall time is kept.

Timeouts: `--timeout SECS` (default 1200) per run. A run exceeding it is
killed (whole process group) and the lane is recorded as DNF.

Environment for Mithril lanes: MITHRIL_STATS is removed and the default
arena sizes are used. If a run dies with "arena exhausted", the lane is
retried once with MITHRIL_NODES=1<<32 MITHRIL_RECS=1<<28 and the retry is
noted in the results.

EVERY completed run's stdout must equal the checksum in bench/expected.txt;
a mismatch or crash marks the row FAILED (the harness continues, and exits
nonzero at the end).

Outputs: bench/results.md (table + machine info + git HEAD + notes) and
bench/results.json (raw per-lane data). Both are rewritten after every
benchmark so a partial run leaves partial results.

Usage:
  python3 bench/harness.py [--dry-run] [--n RUNS] [--timeout SECS]
                           [--skip-gpu] [--only NAME [NAME ...]]
"""

import argparse
import json
import os
import platform
import shutil
import signal
import subprocess
import sys
import tempfile
import time

BENCH_DIR = os.path.dirname(os.path.abspath(__file__))
REPO_ROOT = os.path.dirname(BENCH_DIR)
PORTS_DIR = os.path.join(BENCH_DIR, "ports")
EXPECTED_TXT = os.path.join(BENCH_DIR, "expected.txt")
RESULTS_MD = os.path.join(BENCH_DIR, "results.md")
RESULTS_JSON = os.path.join(BENCH_DIR, "results.json")
TARGET_DIR = os.environ.get("CARGO_TARGET_DIR") or os.path.join(REPO_ROOT, "target")
MITHRIL_BIN = os.path.join(TARGET_DIR, "release", "mithril")

PAR_THREADS = 16
ADAPTIVE_CUTOFF = 60.0     # seconds: first run under this -> 3 runs, else 1
ADAPTIVE_MANY = 3
BIG_ARENAS = {"MITHRIL_NODES": "1<<32", "MITHRIL_RECS": "1<<28"}


# ----------------------------------------------------------------- inputs

def load_expected(path):
    """Parse expected.txt: `<name> <checksum>` per line, `#` comments allowed."""
    expected = {}
    if not os.path.isfile(path):
        return expected
    with open(path) as fh:
        for lineno, raw in enumerate(fh, 1):
            line = raw.strip()
            if not line or line.startswith("#"):
                continue
            parts = line.split()
            if len(parts) != 2:
                sys.exit("expected.txt:%d: malformed line: %r" % (lineno, raw))
            name, checksum = parts
            if name in expected:
                sys.exit("expected.txt:%d: duplicate entry for %s" % (lineno, name))
            expected[name] = checksum
    return expected


def discover():
    """Return (ports, c_twins): sorted benchmark names found under bench/ports/."""
    ports, c_twins = [], []
    if os.path.isdir(PORTS_DIR):
        for entry in sorted(os.listdir(PORTS_DIR)):
            stem, ext = os.path.splitext(entry)
            if stem.startswith("_"):
                continue
            if ext == ".py":
                ports.append(stem)
            elif ext == ".c":
                c_twins.append(stem)
    return ports, c_twins


# -------------------------------------------------------------- execution

def run_once(cmd, timeout, env=None):
    """Run cmd once in its own process group.

    Returns dict(wall, stdout, stderr, rc, timeout). On timeout the whole
    process group is SIGKILLed.
    """
    t0 = time.perf_counter()
    proc = subprocess.Popen(
        cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
        cwd=REPO_ROOT, env=env, start_new_session=True,
    )
    try:
        out, err = proc.communicate(timeout=timeout)
        timed_out = False
    except subprocess.TimeoutExpired:
        try:
            os.killpg(proc.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        out, err = proc.communicate()
        timed_out = True
    wall = time.perf_counter() - t0
    return {"wall": wall, "stdout": (out or "").strip(), "stderr": err or "",
            "rc": proc.returncode, "timeout": timed_out}


def mithril_env(big_arenas=False):
    env = dict(os.environ)
    env.pop("MITHRIL_STATS", None)
    for k in BIG_ARENAS:
        env.pop(k, None)
    if big_arenas:
        env.update(BIG_ARENAS)
    return env


def run_lane(label, cmd, expected, n_fixed, timeout, env=None, arena_retry=False, warm=False):
    # the GPU lane compiles its device program on first use (docker nvcc,
    # cached by source hash): one untimed run warms the cache, as the CPU
    # lanes' binaries are built once before timing
    if warm:
        try:
            subprocess.run(cmd, capture_output=True, timeout=timeout)
        except Exception:
            pass
    """Run one lane; return a lane record dict.

    status: "ok" | "DNF" | "FAILED".  time: min wall of ok runs.
    """
    rec = {"label": label, "cmd": cmd, "status": "ok", "time": None,
           "runs": [], "notes": [], "error": None}
    big = False
    i = 0
    n_target = n_fixed if n_fixed else 1
    while i < n_target:
        r = run_once(cmd, timeout, env=mithril_env(big) if env == "mithril" else env)
        print("[harness]     %s run %d: %.3fs rc=%s%s" % (
            label, i + 1, r["wall"], r["rc"], " TIMEOUT" if r["timeout"] else ""),
            flush=True)
        if r["timeout"]:
            rec["status"] = "DNF"
            rec["error"] = "timeout after %gs (run %d)" % (timeout, i + 1)
            rec["time"] = None
            return rec
        if r["rc"] != 0:
            if arena_retry and not big and "arena exhausted" in r["stderr"]:
                big = True
                rec["notes"].append("arena exhausted at defaults; retried with "
                                    "MITHRIL_NODES=1<<32 MITHRIL_RECS=1<<28")
                print("[harness]     %s: arena exhausted -> retry with big arenas"
                      % label, flush=True)
                continue  # retry the same run index
            rec["status"] = "FAILED"
            tail = r["stderr"].strip().splitlines()[-3:]
            rec["error"] = "exit %s (run %d); stdout=%r; stderr tail=%r" % (
                r["rc"], i + 1, r["stdout"][:200], " | ".join(tail)[:400])
            rec["time"] = None
            return rec
        if r["stdout"] != expected:
            rec["status"] = "FAILED"
            rec["error"] = "checksum mismatch (run %d): expected %s, got %r" % (
                i + 1, expected, r["stdout"][:200])
            rec["time"] = None
            return rec
        rec["runs"].append(r["wall"])
        if rec["time"] is None or r["wall"] < rec["time"]:
            rec["time"] = r["wall"]
        if i == 0 and not n_fixed:
            n_target = ADAPTIVE_MANY if r["wall"] < ADAPTIVE_CUTOFF else 1
        i += 1
    return rec


def build_mithril_cli(gpu):
    cmd = ["cargo", "build", "--release", "-p", "mithril-cli"] + (["--features", "gpu"] if gpu else [])
    print("[harness] building mithril: %s (CARGO_TARGET_DIR=%s)" % (" ".join(cmd), TARGET_DIR), flush=True)
    proc = subprocess.run(cmd, cwd=REPO_ROOT)
    if proc.returncode != 0:
        sys.exit("[harness] cargo build failed (exit %d)" % proc.returncode)
    if not os.path.isfile(MITHRIL_BIN):
        sys.exit("[harness] mithril binary not found at %s" % MITHRIL_BIN)


def compile_c(name, out_dir):
    """gcc -O2 the C twin; return (binary or None, error)."""
    src = os.path.join(PORTS_DIR, name + ".c")
    binary = os.path.join(out_dir, name + ".c.bin")
    proc = subprocess.run(["gcc", "-O2", "-o", binary, src, "-lm"],
                          capture_output=True, text=True)
    if proc.returncode != 0:
        return None, "gcc -O2 failed: %s" % proc.stderr.strip()[:400]
    return binary, None


def compile_mithril(name, out_dir, timeout):
    """`mithril build` the port; return (binary or None, seconds, error)."""
    port = os.path.join(PORTS_DIR, name + ".py")
    binary = os.path.join(out_dir, name + ".mithril.bin")
    r = run_once([MITHRIL_BIN, "build", port, "-o", binary], timeout,
                 env=mithril_env())
    if r["timeout"]:
        return None, r["wall"], "mithril build timed out after %gs" % timeout
    if r["rc"] != 0:
        return None, r["wall"], "mithril build failed: %s" % r["stderr"].strip()[-400:]
    return binary, r["wall"], None


# ------------------------------------------------------------ machine info

def _cmd_out(cmd):
    try:
        return subprocess.run(cmd, capture_output=True, text=True,
                              cwd=REPO_ROOT).stdout.strip()
    except OSError:
        return "?"


def machine_info():
    cpu = "?"
    try:
        with open("/proc/cpuinfo") as fh:
            for line in fh:
                if line.startswith("model name"):
                    cpu = line.split(":", 1)[1].strip()
                    break
    except OSError:
        pass
    ram = "?"
    try:
        with open("/proc/meminfo") as fh:
            for line in fh:
                if line.startswith("MemTotal"):
                    ram = "%.1f GiB" % (int(line.split()[1]) / 1048576.0)
                    break
    except OSError:
        pass
    return {
        "cpu": cpu,
        "logical_cpus": os.cpu_count(),
        "ram": ram,
        "kernel": platform.release(),
        "rustc": _cmd_out(["rustc", "-V"]),
        "gcc": _cmd_out(["gcc", "--version"]).splitlines()[0] if _cmd_out(["gcc", "--version"]) else "?",
        "git_head": _cmd_out(["git", "rev-parse", "HEAD"]),
        "git_branch": _cmd_out(["git", "rev-parse", "--abbrev-ref", "HEAD"]),
    }


# ----------------------------------------------------------------- output

def fmt_time(secs):
    return "%.3fs" % secs if secs is not None else "-"


def fmt_lane(lane):
    if lane is None:
        return "-"
    if lane["status"] == "DNF":
        return "DNF"
    if lane["status"] == "FAILED":
        return "FAILED"
    return fmt_time(lane["time"])


def fmt_ratio(num, den):
    if num is None or den is None or den == 0:
        return "-"
    return "%.2fx" % (num / den)


def lane_time(lane):
    return lane["time"] if lane and lane["status"] == "ok" else None


def write_results(rows, meta, args):
    # a partial run (--skip-cpu / --skip-gpu / --only) merges its lanes into
    # the existing results instead of replacing them: the other lanes keep
    # their numbers and their own commit, noted per lane
    partial = args.skip_cpu or args.skip_gpu or args.only
    if partial and os.path.exists(RESULTS_JSON):
        try:
            old = json.load(open(RESULTS_JSON))
            fresh = {r["name"]: r for r in rows}
            for r in old.get("rows", []):
                if r["name"] in fresh:
                    for lane, v in r["lanes"].items():
                        fresh[r["name"]]["lanes"].setdefault(lane, dict(v, notes=(v.get("notes") or []) + ["kept from %s" % old["meta"].get("git_head", "?")[:7]]))
            rows = [fresh.get(r["name"], r) for r in old.get("rows", [])] + [r for r in rows if r["name"] not in {o["name"] for o in old.get("rows", [])}]
        except Exception:
            pass
    lines = [
        "# Mithril benchmark results (CPU, BIG size)",
        "",
        "- Git HEAD: `%s` (branch `%s`)" % (meta["git_head"], meta["git_branch"]),
        "- CPU: %s (%s logical CPUs)" % (meta["cpu"], meta["logical_cpus"]),
        "- RAM: %s; kernel %s" % (meta["ram"], meta["kernel"]),
        "- rustc: %s" % meta["rustc"],
        "- gcc: %s" % meta["gcc"],
        "- Date: %s" % meta["date"],
        "",
        "Lanes: `C` = C twin `gcc -O2`; `SEQ` = Mithril program at `--threads 1`; "
        "`PAR16` = `--threads %d`. Mithril programs are built once with "
        "`mithril build` (the same pipeline `mithril run` uses; build time in "
        "notes, excluded from lane times). Times are wall-clock minimum of "
        "%s runs per lane. Per-run timeout %gs -> `DNF`. Ratios are "
        "Mithril / C (lower is better). Every completed run's stdout was "
        "checked against `bench/expected.txt`; `FAILED` = wrong checksum or crash. "
        "Mithril env: MITHRIL_STATS unset, default arenas "
        "(retry with MITHRIL_NODES=1<<32 MITHRIL_RECS=1<<28 on `arena exhausted`, noted)."
        % (PAR_THREADS,
           ("%d" % args.n) if args.n else
           "adaptive (%d if first run < %gs, else 1)" % (ADAPTIVE_MANY, ADAPTIVE_CUTOFF),
           args.timeout),
        "",
        "| name | C | SEQ | PAR16 | SEQ/C | PAR16/C | DNF / notes |",
        "|---|---|---|---|---|---|---|",
    ]
    for row in rows:
        c, s, p = row["lanes"].get("C"), row["lanes"].get("SEQ"), row["lanes"].get("PAR16")
        failed = any(l and l["status"] == "FAILED" for l in (c, s, p)) or row["build_error"]
        notes = list(row["notes"])
        for l in (c, s, p):
            if l is None:
                continue
            if l["status"] == "DNF":
                notes.append("%s DNF (>%gs)" % (l["label"], args.timeout))
            for n in l["notes"]:
                notes.append("%s: %s" % (l["label"], n))
            if l["status"] == "ok":
                notes.append("%s n=%d" % (l["label"], len(l["runs"])))
        cells = [
            row["name"] + (" **FAILED**" if failed else ""),
            fmt_lane(c), fmt_lane(s), fmt_lane(p),
            fmt_ratio(lane_time(s), lane_time(c)), fmt_ratio(lane_time(p), lane_time(c)),
            "; ".join(notes) or "",
        ]
        lines.append("| " + " | ".join(cells) + " |")

    problems = []
    for row in rows:
        if row["build_error"]:
            problems.append("- `%s` build: %s" % (row["name"], row["build_error"]))
        for label in ("C", "SEQ", "PAR16"):
            l = row["lanes"].get(label)
            if l and l["status"] in ("FAILED", "DNF"):
                problems.append("- `%s` %s %s: %s" % (row["name"], label, l["status"], l["error"]))
    if problems:
        lines += ["", "## Failures / DNFs", ""] + problems
    lines.append("")
    with open(RESULTS_MD, "w") as fh:
        fh.write("\n".join(lines))
    with open(RESULTS_JSON, "w") as fh:
        json.dump({"meta": meta, "timeout": args.timeout, "n": args.n, "rows": rows},
                  fh, indent=1)


# ------------------------------------------------------------------- main

def dry_run(ports, c_twins, expected, args):
    print("[dry-run] mithril   : %s" % MITHRIL_BIN)
    print("[dry-run] runs/lane : %s" % (args.n or "adaptive"))
    print("[dry-run] timeout   : %gs" % args.timeout)
    problems = []
    for name in ports:
        exp = expected.get(name)
        if exp is None:
            problems.append("port %s.py has no entry in expected.txt" % name)
        print("[dry-run] %-14s expected=%s C=%s" % (name, exp or "MISSING",
                                                    "yes" if name in c_twins else "no"))
    for p in problems:
        print("[dry-run] ERROR: %s" % p)
    return 1 if problems or not expected else 0


def main():
    ap = argparse.ArgumentParser(description="Mithril benchmark harness")
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("--n", "-n", type=int, default=None,
                    help="runs per lane (default adaptive: 3 if first run < 60s else 1)")
    ap.add_argument("--timeout", type=float, default=1200.0,
                    help="per-run timeout in seconds; exceeding it records DNF (default 1200)")
    ap.add_argument("--skip-gpu", action="store_true", help="never run the GPU lane")
    ap.add_argument("--skip-cpu", action="store_true", help="skip the C, SEQ and PAR16 lanes (GPU only)")
    ap.add_argument("--no-build", action="store_true",
                    help="skip `cargo build` of mithril-cli (use existing binary)")
    ap.add_argument("--only", nargs="+", metavar="NAME")
    args = ap.parse_args()

    expected = load_expected(EXPECTED_TXT)
    ports, c_twins = discover()
    if args.only:
        unknown = sorted(set(args.only) - set(ports))
        if unknown:
            sys.exit("--only names have no port: %s" % " ".join(unknown))
        ports = [p for p in args.only]
    gpu_enabled = os.environ.get("MITHRIL_GPU") == "1" and not args.skip_gpu

    if args.dry_run:
        sys.exit(dry_run(ports, c_twins, expected, args))
    missing = [p for p in ports if p not in expected]
    if missing:
        sys.exit("[harness] expected.txt is missing entries for: %s" % " ".join(missing))

    if not args.no_build:
        build_mithril_cli(gpu_enabled)
    meta = machine_info()
    meta["date"] = time.strftime("%Y-%m-%d %H:%M:%S %Z")

    rows = []
    tmpdir = tempfile.mkdtemp(prefix="mithril-bench-")
    try:
        for name in ports:
            exp = expected[name]
            print("[harness] === %s (expect %s) ===" % (name, exp), flush=True)
            row = {"name": name, "expected": exp, "lanes": {}, "notes": [],
                   "build_error": None, "build_time": None}

            if name in c_twins and not args.skip_cpu:
                binary, err = compile_c(name, tmpdir)
                if err:
                    row["lanes"]["C"] = {"label": "C", "status": "FAILED", "time": None,
                                         "runs": [], "notes": [], "error": err, "cmd": []}
                else:
                    row["lanes"]["C"] = run_lane("C", [binary], exp, args.n, args.timeout)
            print("[harness]   C     : %s" % fmt_lane(row["lanes"].get("C")), flush=True)

            mbin, btime, berr = (None, 0.0, None) if args.skip_cpu else compile_mithril(name, tmpdir, args.timeout)
            row["build_time"] = btime
            if args.skip_cpu:
                pass
            elif berr:
                row["build_error"] = berr
                print("[harness]   BUILD FAILED: %s" % berr, flush=True)
            else:
                row["notes"].append("build %.1fs" % btime)
                for label, threads in (("SEQ", 1), ("PAR16", PAR_THREADS)):
                    row["lanes"][label] = run_lane(
                        label, [mbin, "--threads", str(threads)], exp, args.n,
                        args.timeout, env="mithril", arena_retry=True)
                    print("[harness]   %-6s: %s" % (label, fmt_lane(row["lanes"][label])),
                          flush=True)
            if gpu_enabled:
                # the device lane times a built artefact (`build --gpu`
                # once, `exec` per run), as the CPU lanes and reference do
                port = os.path.join(PORTS_DIR, name + ".py")
                art = os.path.join(tmpdir, name + ".gpu")
                b = subprocess.run([MITHRIL_BIN, "build", port, "--gpu", "-o", art], capture_output=True, text=True, timeout=args.timeout)
                if b.returncode != 0:
                    row["lanes"]["GPU"] = {"label": "GPU", "cmd": "", "status": "FAILED", "time": None, "runs": [], "notes": "", "error": "build --gpu: " + b.stderr.strip()[-300:]}
                else:
                    row["lanes"]["GPU"] = run_lane("GPU", [MITHRIL_BIN, "exec", art], exp, args.n, args.timeout, env="mithril")
            for l in row["lanes"].values():
                if l["status"] != "ok":
                    print("[harness]   %s %s: %s" % (l["label"], l["status"], l["error"]),
                          file=sys.stderr, flush=True)
            rows.append(row)
            write_results(rows, meta, args)
    finally:
        shutil.rmtree(tmpdir, ignore_errors=True)

    write_results(rows, meta, args)
    print("[harness] wrote %s" % RESULTS_MD)
    bad = any(r["build_error"] or any(l["status"] == "FAILED" for l in r["lanes"].values())
              for r in rows)
    if bad:
        print("[harness] FAILURES present — see results.md", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
