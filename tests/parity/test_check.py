"""Acceptance must cover lanes even when their baseline entry is absent."""
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location('parity_check', Path(__file__).with_name('check.py'))
check = importlib.util.module_from_spec(spec)
spec.loader.exec_module(check)


class AcceptanceTests(unittest.TestCase):
    def setUp(self):
        self.answer = {'out': '128\n', 'rc': 0}

    def test_missing_reference_uses_independent_answer(self):
        self.assertEqual(check.verdict(self.answer, None, self.answer), 'FIXED')
        for failure in [{'out': '129\n', 'rc': 0}, {'out': '', 'rc': 1},
                        {'out': '', 'rc': 'timeout'}, {'out': '128\n', 'rc': -11}]:
            with self.subTest(failure=failure):
                self.assertEqual(check.verdict(failure, None, self.answer), 'REGRESSED')

    def test_missing_reference_and_answer_fail(self):
        self.assertEqual(check.verdict(self.answer, None, None), 'REGRESSED')

    def test_existing_reference_and_alternatives_remain_supported(self):
        self.assertEqual(check.verdict(self.answer, self.answer, None), 'EQUAL')
        ref = {'out': '', 'rc': -11, 'alts': [self.answer]}
        self.assertEqual(check.verdict(self.answer, ref, None), 'EQUAL')
        self.assertEqual(check.verdict(self.answer, {'out': '0\n', 'rc': 0}, self.answer), 'FIXED')

    def test_known_answer_precedes_oracle_and_failed_oracle_is_unusable(self):
        ref = {'p': {'oracle': self.answer}}
        other = {'out': '1\n', 'rc': 0}
        self.assertEqual(check.correct('p', ref, {'p': {'correct': other}}), other)
        self.assertEqual(check.correct('p', ref, {}), self.answer)
        self.assertIsNone(check.correct('p', {'p': {'oracle': {'out': '', 'rc': 1}}}, {}))


if __name__ == '__main__':
    unittest.main()
