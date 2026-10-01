"""Build a self-contained replay of real net reductions and saved continuations."""

import argparse
import hashlib
import subprocess
import json
from pathlib import Path

from oracle import OPS, contract, solve

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parent.parent


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def run(command, timeout=90):
    result = subprocess.run(
        list(map(str, command)), capture_output=True, text=True, timeout=timeout
    )
    if result.returncode:
        raise RuntimeError(result.stderr or result.stdout)
    return result.stdout.strip(), result.stderr.strip(), None


GRAPH = dict(
    inputs=2,
    nodes=[["add", -1, -2], ["mul", 0, -1], ["xor", 1, 0], ["div_odd", 2, -2]],
    outputs=[3],
)
CASES = [
    dict(label="Ample memory", length=1023),
    dict(label="No scratch", length=1023, scratch=0),
    dict(label="Tight SRAM", length=1023, sram=320),
    dict(label="Expensive transfers", length=1023, transfer_price=100),
]


def source_for_joint(graph, cases):
    if not cases:
        raise ValueError("at least one compiler case is required")
    source = (ROOT / "planner.mithril").read_text()
    n = len(graph["nodes"])
    outputs = sum(1 << i for i in graph["outputs"])
    nodes = "NoNodes()"
    for op, left, right in reversed(graph["nodes"]):
        nodes = f"Node({OPS[op]}, {left}, {right}, {nodes})"
    lines = [
        "def main():",
        f"    nodes = {nodes}",
        f"    leaves = pure_leaves(nodes, {n}, 0)",
        f"    table = boundaries(leaves[0], {outputs})",
    ]
    for i, supplied in enumerate(cases):
        config = contract(graph, {k: v for k, v in supplied.items() if k != "label"})
        lines.append(
            f"    p{i} = (1000000000000000, 1000000000000000, 1000000000000000, 0, 0, 0)"
        )
        query = f"({config['length']}, {config['scratch']}, {config['sram']}, {config['transfer_price']})"
        for tile in sorted(config["tiles"]):
            lines.append(
                f"    p{i} = joint_tile(table, {n}, {outputs}, {tile}, {query}, p{i})"
            )
    lines.append("    return (" + ", ".join(f"p{i}" for i in range(len(cases))) + ",)")
    return source + "\n\n" + "\n".join(lines) + "\n"


def capture(graph, cases, directory, seeds=3):
    directory = Path(directory)
    if directory.exists() and any(directory.iterdir()):
        raise ValueError("use an empty output directory to preserve receipts")
    if type(seeds) is not int or not 0 <= seeds <= 100:
        raise ValueError("seed count must be 0..100")
    source = source_for_joint(graph, cases)
    directory.mkdir(parents=True, exist_ok=True)
    path = directory / "planner.py"
    path.write_text(source)
    run(
        [
            "cargo",
            "build",
            "--quiet",
            "--release",
            "--manifest-path",
            REPO / "Cargo.toml",
            "-p",
            "mithril-net",
            "--example",
            "compiler_planning",
        ]
    )
    probe = REPO / "target/release/examples/compiler_planning"
    stdout, stderr, _ = run([probe, path, seeds, "--demo"], timeout=180)
    (directory / "stdout.jsonl").write_text(stdout + "\n")
    (directory / "stderr.txt").write_text(stderr + "\n")
    rows = [json.loads(line) for line in stdout.splitlines()]
    if len(rows) != seeds + 15:
        raise AssertionError("missing direct runs or saved-state continuations")
    plans = [solve(graph, {k: v for k, v in c.items() if k != "label"}) for c in cases]
    expected = [p["key"] for p in plans]
    if any(r["value"] != expected or not r["oracle_equal"] for r in rows):
        raise AssertionError("a reduction order disagrees with the exhaustive oracle")
    groups = {}
    for row in rows:
        if row["checkpoint"]:
            groups.setdefault((row["prefix_policy"], row["checkpoint"]), []).append(row)
    if len(groups) != 4 or any(
        len(g) != 3 or len({r["checkpoint_hash"] for r in g}) != 1
        for g in groups.values()
    ):
        raise AssertionError("checkpoint continuations did not share one saved state")
    sources = [
        REPO / "Cargo.toml",
        REPO / "Cargo.lock",
        ROOT / "runtime.rs",
    ]
    for crate in ("mithril-core", "mithril-front", "mithril-net"):
        sources += [REPO / "crates" / crate / "Cargo.toml"]
        sources += sorted((REPO / "crates" / crate / "src").rglob("*.rs"))
    sources += [
        ROOT / name for name in ("demo.py", "view.html", "oracle.py", "runtime.rs")
    ]
    report = dict(
        graph=graph,
        cases=cases,
        plans=plans,
        runs=rows,
        verified=True,
        source_sha256=digest(path),
        probe_sha256=digest(probe),
        implementation={str(p.relative_to(REPO)): digest(p) for p in sources},
        scope="Direct sequential net reduction, executable saved-state forks. Explicit synthetic CPU/accelerator contract; no accelerator hardware is invoked. Peak cells exclude Vec capacity, queue and process overhead. Trace rows: worklist pops, rewrites, occupied cells, pending pairs.",
    )
    (directory / "demo-report.json").write_text(json.dumps(report, indent=2) + "\n")
    template = (ROOT / "view.html").read_text()
    payload = json.dumps(report).replace("<", "\\u003c")
    (directory / "demo.html").write_text(template.replace("__PROOF_DATA__", payload))
    return report


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--seeds", type=int, default=3)
    args = parser.parse_args()
    result = capture(GRAPH, CASES, args.out, args.seeds)
    print(
        f"Verified {len(result['runs'])} executions and {len(CASES)} coupled compiler cases."
    )
    print(f"Demo: {args.out / 'demo.html'}")
