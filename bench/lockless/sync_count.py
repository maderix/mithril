#!/usr/bin/env python3
"""Count synchronization primitives in each implementation's source.

    python3 bench/lockless/sync_count.py

Counts every occurrence, outside comments, of:
  C:       pthread_mutex_* / pthread_spin_* / pthread_rwlock_* /
           pthread_barrier_* / pthread_cond_*, _Atomic, atomic_*, __sync_*,
           __atomic_*, `#pragma omp atomic|critical|barrier`, and OpenMP
           `reduction(` clauses
  Rust:    Mutex, RwLock, Condvar, Barrier, Atomic*, fetch_*,
           compare_exchange*
  Mithril: none possible. The language has no lock, atomic, barrier or
           shared mutable cell; the count is 0 by construction (the source
           is still grepped for the Rust and C names as a check).
Declarations and `use` lines count too: this is a grep, not an analysis.
Synchronization inside libraries (libgomp, rayon's work stealing, the
implicit barrier closing an `omp parallel for`) is not counted.
Also prints lines of code (non-blank, non-comment) per implementation.
"""
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
PROGRAMS = ["histogram", "hashbuild", "bfs", "components", "fsum"]

C_PATTERNS = {
    "pthread lock/barrier": r"\bpthread_(?:mutex|spin|rwlock|barrier|cond)_\w+",
    "_Atomic": r"\b_Atomic\b",
    "atomic_*": r"\batomic_\w+",
    "__sync/__atomic": r"\b__(?:sync|atomic)_\w+",
    "omp atomic/critical/barrier": r"#\s*pragma\s+omp\s+(?:atomic|critical|barrier)\b",
    "omp reduction": r"\breduction\s*\(",
}
RUST_PATTERNS = {
    "Mutex/RwLock/Condvar/Barrier": r"\b(?:Mutex|RwLock|Condvar|Barrier)\b",
    "Atomic*": r"\bAtomic\w*",
    "fetch_*": r"\bfetch_\w+",
    "compare_exchange*": r"\bcompare_exchange\w*",
}


def strip_c(src):
    src = re.sub(r"/\*.*?\*/", "", src, flags=re.S)
    return re.sub(r"//[^\n]*", "", src)


def strip_py(src):
    return re.sub(r"#[^\n]*", "", src)


def loc(src):
    return sum(1 for l in src.splitlines() if l.strip())


def count(src, pats):
    return {k: len(re.findall(p, src)) for k, p in pats.items()}


def main():
    rows = []
    detail = []
    for name in PROGRAMS:
        c = strip_c((HERE / name / "main.c").read_text())
        rs = strip_c((HERE / "rust" / "src" / "bin" / f"{name}.rs").read_text())
        py = strip_py((HERE / name / "main.py").read_text())
        cc, rc = count(c, C_PATTERNS), count(rs, RUST_PATTERNS)
        # Mithril: the language has no such names; grep anyway as a check
        mc = sum(count(py, C_PATTERNS).values()) + sum(count(py, RUST_PATTERNS).values())
        if mc:
            print(f"warning: {name}/main.py matches a synchronization name ({mc})", file=sys.stderr)
        rows.append((name, sum(cc.values()), sum(rc.values()), mc, loc(c), loc(rs), loc(py)))
        detail.append((name, cc, rc))
    print("Synchronization primitives in the source (occurrences outside comments)")
    print()
    print("| program | C | Rust | Mithril | LOC C | LOC Rust | LOC Mithril |")
    print("|---|---|---|---|---|---|---|")
    for r in rows:
        print("| %s | %d | %d | %d | %d | %d | %d |" % r)
    print()
    print("Mithril: 0 by construction (no lock, atomic, barrier or shared mutable")
    print("cell exists in the language).")
    print()
    print("Breakdown:")
    for name, cc, rc in detail:
        cs = ", ".join(f"{k} {v}" for k, v in cc.items() if v) or "none"
        rs = ", ".join(f"{k} {v}" for k, v in rc.items() if v) or "none"
        print(f"  {name}: C [{cs}]; Rust [{rs}]")


if __name__ == "__main__":
    main()
