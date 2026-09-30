#!/usr/bin/env python3
"""Count synchronization operation sites in each implementation's source.

    python3 bench/lockless/sync_count.py

One rule for both languages: a site is one place in the program text
(outside comments) where the program itself performs a synchronizing
operation on shared memory. Types and declarations are not sites (C
`_Atomic`/`pthread_mutex_t`, `sizeof(_Atomic ...)`, Rust `AtomicU32`/
`Mutex<..>` in types and signatures, `use` lines). Creating a lock or an
atomic is counted apart, in the `init` column, in both languages.

  ops, C:    atomic_*(...) calls except atomic_init; __atomic_*/__sync_*
             builtins; pthread_{mutex,spin,rwlock}_{lock,trylock,unlock,
             rdlock,wrlock,timedlock}, pthread_barrier_wait,
             pthread_cond_{wait,timedwait,signal,broadcast};
             `#pragma omp atomic|critical|barrier|flush|ordered`; each
             OpenMP `reduction(` clause (GCC lowers its combine into the
             shared variable to a `lock cmpxchg` loop for a scalar and to a
             GOMP_atomic_start/end locked region for an array section).
  ops, Rust: method calls .lock() .try_lock() .read() .write() (no
             arguments, the RwLock forms), and the atomic methods .load(
             .store( .swap( .fetch_*( .compare_exchange*(.
  init, C:   atomic_init(, pthread_{mutex,spin,rwlock,cond,barrier}_init(,
             ATOMIC_VAR_INIT, PTHREAD_*_INITIALIZER.
  init, Rust: Mutex::new, RwLock::new, Condvar::new, Barrier::new,
             Atomic*::new (also passed as a function, `.map(AtomicU32::new)`).
  Mithril:   0 by construction: the language has no lock, atomic, barrier
             or shared mutable cell. The source is grepped with the C and
             Rust patterns anyway as a check.

Not counted, in any language: fork/join structure and whatever the
library or runtime does inside it (libgomp's implicit barrier closing an
`omp parallel for`, rayon's work stealing and join counters, rayon
fold/reduce/sum/collect, which combine values returned by joins, and
Mithril's own runtime, which synchronizes with atomics in
crates/mithril-rt/src/sync.rs). This is a grep over program source, not an
analysis: a C plain operator on an `_Atomic` object (x++, x = v) is also
atomic and would be missed; none of these sources uses one. Also prints lines of code (non-blank, non-comment).
"""
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
PROGRAMS = ["histogram", "hashbuild", "bfs", "components", "fsum"]

C_OPS = {
    "atomic_*()": r"\batomic_(?!init\b)\w+\s*\(",
    "__atomic/__sync": r"\b__(?:atomic|sync)_\w+\s*\(",
    "pthread lock/wait": r"\bpthread_(?:(?:mutex|spin|rwlock)_(?:lock|trylock|unlock|rdlock|wrlock|timedlock|timedrdlock|timedwrlock)"
    r"|barrier_wait|cond_(?:wait|timedwait|signal|broadcast))\s*\(",
    "omp atomic/critical/barrier/flush/ordered": r"#\s*pragma\s+omp\s+(?:atomic|critical|barrier|flush|ordered)\b",
    "omp reduction(": r"\breduction\s*\(",
}
C_INIT = {
    "init": r"\batomic_init\s*\(|\bpthread_(?:mutex|spin|rwlock|cond|barrier)_init\s*\(|\bATOMIC_VAR_INIT\b|\bPTHREAD_\w+_INITIALIZER\b",
}
RUST_OPS = {
    "lock/read/write": r"\.(?:lock|try_lock)\s*\(|\.(?:read|write)\s*\(\s*\)",
    "atomic method": r"\.(?:load|store|swap|fetch_\w+|compare_exchange(?:_weak)?)\s*\(",
}
RUST_INIT = {
    "init": r"\b(?:Mutex|RwLock|Condvar|Barrier|Atomic\w+)::new\b",
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


def variants(name):
    """(label, language, stripped source) of every implementation."""
    out = [("mithril", "py", strip_py((HERE / name / "main.py").read_text()))]
    for f in sorted((HERE / name).glob("main*.c")):
        out.append(("c" + f.stem[len("main"):], "c", strip_c(f.read_text())))
    for f in sorted((HERE / "rust" / "src" / "bin").glob(f"{name}*.rs")):
        if f.stem == name or f.stem.startswith(name + "_"):
            out.append(("rust" + f.stem[len(name):], "rs", strip_c(f.read_text())))
    return out


def main():
    print("Synchronization operation sites in the program source (outside comments)")
    print()
    print("| program | impl | ops | init | LOC | ops breakdown |")
    print("|---|---|---|---|---|---|")
    for name in PROGRAMS:
        for label, lang, src in variants(name):
            if lang == "c":
                ops, init = count(src, C_OPS), count(src, C_INIT)
            elif lang == "rs":
                ops, init = count(src, RUST_OPS), count(src, RUST_INIT)
            else:
                ops = {k: v for pats in (C_OPS, RUST_OPS) for k, v in count(src, pats).items()}
                init = {k: v for pats in (C_INIT, RUST_INIT) for k, v in count(src, pats).items()}
                if sum(ops.values()) or sum(init.values()):
                    print(f"warning: {name}/main.py matches a synchronization pattern", file=sys.stderr)
            detail = ", ".join(f"{k} {v}" for k, v in ops.items() if v) or "-"
            print(f"| {name} | {label} | {sum(ops.values())} | {sum(init.values())} | {loc(src)} | {detail} |")
    print()
    print("ops: sites where the program synchronizes on shared memory; init: sites")
    print("that create a lock or an atomic. Mithril: 0 by construction (no lock,")
    print("atomic, barrier or shared mutable cell exists in the language). Runtime and")
    print("library synchronization (Mithril's runtime, libgomp, rayon) is not counted.")


if __name__ == "__main__":
    main()
