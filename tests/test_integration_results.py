"""Offline regression tests for the exact jq filter used by Windows integration.

Run: python3 -m unittest discover -s tests -p 'test_integration_*.py' -v
Requires jq; never executes installers or contacts the network.
"""

import copy
import json
from pathlib import Path
import subprocess
import unittest


FILTER = Path(__file__).resolve().parents[1] / ".github/scripts/windows-results.jq"
SKIP_REASON = "Kiro requires Windows 11 (SPEC §7.4)"


class WindowsResultsTests(unittest.TestCase):
    def setUp(self):
        self.rows = [
            {"id": "claude", "result": "ok"},
            {"id": "kiro", "result": "ok", "after": None,
             "reason": "installed; restart your shell"},
            {"id": "antigravity", "result": "ok"},
        ]

    def accepts(self, rows, build):
        result = subprocess.run(
            ["jq", "-e", "--argjson", "windows_build", str(build), "-f", str(FILTER)],
            input=json.dumps(rows), text=True, capture_output=True, check=False,
        )
        self.assertIn(result.returncode, (0, 1), result.stderr)
        return result.returncode == 0

    def test_success_including_server_2025_and_boundary(self):
        for build in (22000, 22631, 26100):
            with self.subTest(build=build):
                self.assertTrue(self.accepts(self.rows, build))

    def test_normal_skip_only_below_gate_or_unknown_build(self):
        self.rows[1].update(result="skip", reason=SKIP_REASON)
        for build in (0, 19045, 20348, 21999):
            with self.subTest(build=build):
                self.assertTrue(self.accepts(self.rows, build))
        self.assertFalse(self.accepts(self.rows, 26100))

    def test_success_below_gate_is_unexpected_on_clean_runner(self):
        self.assertFalse(self.accepts(self.rows, 20348))

    def test_skip_requires_exact_supported_reason(self):
        for reason in (None, "", "network failure", 42, False):
            with self.subTest(reason=reason):
                self.rows[1].update(result="skip", reason=reason)
                self.assertFalse(self.accepts(self.rows, 20348))
        del self.rows[1]["reason"]
        self.assertFalse(self.accepts(self.rows, 20348))

    def test_missing_duplicate_and_extra_tools_fail(self):
        malformed = [[], self.rows[:1], self.rows[:2], self.rows[::2]]
        for row in self.rows:
            malformed.append(self.rows + [row])
        malformed.append([self.rows[0], self.rows[0], self.rows[1]])
        malformed.append(self.rows + [{"id": "unknown", "result": "ok"}])
        for rows in malformed:
            with self.subTest(rows=rows):
                self.assertFalse(self.accepts(rows, 26100))

    def test_fail_skip_and_unknown_results_cannot_hide(self):
        for index in range(3):
            for outcome in ("fail", "skip", "unknown", None):
                with self.subTest(index=index, outcome=outcome):
                    rows = copy.deepcopy(self.rows)
                    rows[index]["result"] = outcome
                    self.assertFalse(self.accepts(rows, 26100))

    def test_non_arrays_fail(self):
        for rows in ({}, None, "ok", 42):
            with self.subTest(rows=rows):
                self.assertFalse(self.accepts(rows, 26100))


if __name__ == "__main__":
    unittest.main()
