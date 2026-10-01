"""Executable continuations must agree with an independent placement oracle."""

import tempfile
import unittest
from pathlib import Path


class DemoTests(unittest.TestCase):
    def test_joint_net_matches_exhaustive_solver_for_tails_outputs_and_constraints(
        self,
    ):
        import json
        import random
        from demo import REPO, run
        from demo import source_for_joint
        from oracle import solve

        cases = [
            dict(length=0),
            dict(length=1),
            dict(length=17, scratch=0),
            dict(length=63, sram=0),
            dict(length=65, sram=128, transfer_price=0),
            dict(length=65, transfer_price=1000000),
        ]
        run(
            [
                "cargo",
                "build",
                "--quiet",
                "--release",
                "-p",
                "mithril-net",
                "--example",
                "compiler_planning",
            ]
        )
        rng = random.Random(91)
        with tempfile.TemporaryDirectory() as directory:
            for n in (1, 2, 3):
                nodes = [
                    [
                        rng.choice(("add", "mul", "xor", "div_odd")),
                        rng.randrange(-2, i),
                        rng.randrange(-2, i),
                    ]
                    for i in range(n)
                ]
                graph = dict(inputs=2, nodes=nodes, outputs=list(range(n)))
                path = Path(directory, "planner.py")
                path.write_text(source_for_joint(graph, cases))
                output, _, _ = run(
                    [REPO / "target/release/examples/compiler_planning", path, 2]
                )
                expected = [solve(graph, c)["key"] for c in cases]
                for line in output.splitlines():
                    self.assertEqual(json.loads(line)["value"], expected, (graph, line))

    def test_saved_net_forks_keep_joint_decisions_and_checkpoint_identity(self):
        from demo import capture

        graph = dict(inputs=1, nodes=[["add", -1, -1], ["div_odd", 0, -1]], outputs=[1])
        cases = [
            dict(label="Mixed", length=35),
            dict(label="No scratch", length=35, scratch=0),
        ]
        with tempfile.TemporaryDirectory() as directory:
            report = capture(graph, cases, Path(directory), seeds=1)
        self.assertTrue(report["verified"])
        self.assertIn("demos/compiler_planning/oracle.py", report["implementation"])
        groups = {}
        for row in report["runs"]:
            self.assertEqual(row["value"], [p["key"] for p in report["plans"]])
            self.assertTrue(row["trace"])
            self.assertEqual(row["trace"][-1][0], row["pops"])
            if row["checkpoint"]:
                group = groups.setdefault((row["prefix_policy"], row["checkpoint"]), [])
                group.append(row)
        self.assertEqual(len(groups), 4)
        for rows in groups.values():
            self.assertEqual(len(rows), 3)
            self.assertEqual(len({r["checkpoint_hash"] for r in rows}), 1)

    def test_existing_receipts_and_empty_cases_are_rejected(self):
        from demo import capture, source_for_joint

        graph = dict(inputs=1, nodes=[["add", -1, -1]], outputs=[0])
        with self.assertRaises(ValueError):
            source_for_joint(graph, [])
        with tempfile.TemporaryDirectory() as directory:
            Path(directory, "receipt").write_text("keep")
            with self.assertRaises(ValueError):
                capture(graph, [dict(length=35)], Path(directory))


if __name__ == "__main__":
    unittest.main()
