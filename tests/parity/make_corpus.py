#!/usr/bin/env python3
"""Write tests/parity/corpus.json: every program the parity harness runs,
with its per-program settings. Run once when the corpus changes; check.py
reads only corpus.json (it never imports the CI scripts, so the corpus
stays fixed while the tree around it is rewritten).

Sources:
  fixture/  every .py under crates/*/tests/fixtures
  ports/    bench/ports at fast.py's SMALL sizes (its sed substitutions)
  general/  bench/general at run.py's SMALL sizes
  lockless/ bench/lockless/*/main.py at small.json's sizes
  demos/    demos/*.py at 12x12 (cornell_path: 2 samples per pixel)
  gen/      tests/parity/gen/g*.py (gen_programs.py, schedule_test.rs's generator)
  known/    tests/parity/known/*.py (bug probes, expected value in the header)
tests/e2e holds no programs (run.sh substitutes the ports), so it adds none.

Entry: {name, path (repo-relative), sed: [expr...] applied to a copy,
expect (documented value, informational), timeout (seconds per lane)}.
"""
import glob
import json
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
sys.path.insert(0, os.path.join(ROOT, "tests", "ci"))
sys.path.insert(0, os.path.join(ROOT, "bench", "general"))
from fast import SMALL as PORTS_SMALL  # noqa: E402

GENERAL_SMALL = {"collatz_mutual": 3000, "cow_versions": 3000, "dag_share": 3, "graph_dfs": 1,
                 "interp": 5, "persist_map": 2000, "sorts": 300,
                 "stage_closure": 50, "pipeline_cfg": 20, "interp_closure": 200}
DEMO_SED = {
    "cornell_whitted": ["/^def size/,/return/s/return [0-9][0-9]*/return 12/"],
    "cornell_path": ["/^def size/,/return/s/return [0-9][0-9]*/return 12/",
                     "/^def spp/,/return/s/return [0-9][0-9]*/return 2/"],
}


def rel(p):
    return os.path.relpath(p, ROOT)


def main():
    progs = []
    for p in sorted(glob.glob(os.path.join(ROOT, "crates", "*", "tests", "fixtures", "*.py"))):
        crate = p.split(os.sep)[-4]
        progs.append({"name": f"fixture/{crate}/{os.path.basename(p)[:-3]}", "path": rel(p), "sed": [], "timeout": 120})
    for n, (subs, expect) in sorted(PORTS_SMALL.items()):
        progs.append({"name": f"ports/{n}", "path": f"bench/ports/{n}.py", "sed": subs, "expect": expect, "timeout": 300})
    # run.py's re.sub(r"return run\((\d+)\)", ...) as sed
    for n, small in sorted(GENERAL_SMALL.items()):
        progs.append({"name": f"general/{n}", "path": f"bench/general/{n}.py",
                      "sed": [f"s/return run([0-9][0-9]*)/return run({small})/g"], "timeout": 300})
    lk = json.load(open(os.path.join(ROOT, "bench", "lockless", "small.json")))
    for n, s in sorted((k, v) for k, v in lk.items() if not k.startswith("_")):
        progs.append({"name": f"lockless/{n}", "path": f"bench/lockless/{n}/main.py", "sed": [s["sed"]],
                      "expect": s["expected"], "timeout": 300})
    for n, subs in sorted(DEMO_SED.items()):
        progs.append({"name": f"demos/{n}", "path": f"demos/{n}.py", "sed": subs, "timeout": 300})
    for p in sorted(glob.glob(os.path.join(HERE, "gen", "g*.py"))):
        progs.append({"name": f"gen/{os.path.basename(p)[:-3]}", "path": rel(p), "sed": [], "timeout": 60})
    for p in sorted(glob.glob(os.path.join(HERE, "known", "*.py"))):
        m = re.search(r"^# Correct: (.*?)  --", open(p).read(), re.M)
        e = {"name": f"known/{os.path.basename(p)[:-3]}", "path": rel(p), "sed": [], "timeout": 60}
        if m:
            e["expect"] = m.group(1)
        progs.append(e)
    out = os.path.join(HERE, "corpus.json")
    with open(out, "w") as f:
        json.dump({"_comment": __doc__.strip().split("\n")[0], "programs": progs}, f, indent=1)
        f.write("\n")
    by = {}
    for p in progs:
        k = p["name"].split("/")[0]
        by[k] = by.get(k, 0) + 1
    print(f"wrote {out}: {len(progs)} programs {by}")


if __name__ == "__main__":
    main()
