"""Joint decisions must respect placement legality and both memory budgets."""

import unittest


class JointTests(unittest.TestCase):
    def graph(self):
        return dict(
            inputs=2,
            nodes=[["add", -1, -2], ["mul", 0, -1], ["xor", 1, 0], ["div_odd", 2, -2]],
            outputs=[3],
        )

    def test_single_vector_operation_has_hand_checked_placement_and_cost(self):
        from oracle import solve

        graph = dict(inputs=1, nodes=[["add", -1, -1]], outputs=[0])
        plan = solve(graph, dict(length=64, sram=512))
        self.assertEqual(plan["key"], [24, 0, 2, 0, 1, 64])
        self.assertEqual(plan["regions"][0]["device"], "accelerator")
        self.assertEqual(
            solve(graph, dict(length=64, sram=0))["key"], [52, 0, 0, 0, 0, 64]
        )

    def test_scalar_operation_and_mixed_fused_placement_are_illegal(self):
        from oracle import evaluate

        graph = self.graph()
        self.assertIsNone(evaluate(graph, dict(length=255), 0, 15, 64))
        self.assertIsNone(evaluate(graph, dict(length=255), 0, 1, 64))

    def test_storage_and_transfer_facts_change_the_joint_plan(self):
        from oracle import solve

        graph = self.graph()
        fast = solve(graph, dict(length=255, sram=2048, transfer_price=0))
        self.assertNotEqual(fast["key"][4], 0)
        for constraint in (dict(scratch=0), dict(sram=0), dict(transfer_price=1000000)):
            config = dict(length=255, sram=2048, transfer_price=0)
            changed = solve(graph, dict(config, **constraint))
            self.assertEqual(changed["key"][4], 0)

    def test_tail_zero_and_output_buffers_are_accounted_for(self):
        from oracle import evaluate, solve

        graph = dict(inputs=1, nodes=[["add", -1, -1], ["mul", 0, -1]], outputs=[0, 1])
        result = evaluate(graph, dict(length=17), 1, 0, 16)
        self.assertEqual(result["key"][1], 0)
        self.assertEqual(solve(graph, dict(length=0))["key"][4], 0)

    def test_contract_rejects_unbounded_or_invalid_inputs(self):
        from oracle import solve

        for config in (
            dict(length=-1),
            dict(sram=-1),
            dict(tiles=(8,)),
            dict(transfer_price=-1),
        ):
            with self.assertRaises(ValueError):
                solve(self.graph(), config)


if __name__ == "__main__":
    unittest.main()
