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


class WindowsResultsTests(unittest.TestCase):
    def setUp(self):
        self.rows = [
            {"id": "claude", "result": "ok"},
            {"id": "antigravity", "result": "ok"},
        ]

    def accepts(self, rows):
        result = subprocess.run(
            ["jq", "-e", "-f", str(FILTER)],
            input=json.dumps(rows), text=True, capture_output=True, check=False,
        )
        self.assertIn(result.returncode, (0, 1), result.stderr)
        return result.returncode == 0

    def test_success_for_exact_windows_tool_set(self):
        self.assertTrue(self.accepts(self.rows))
        self.assertTrue(self.accepts(list(reversed(self.rows))))

    def test_missing_duplicate_and_extra_tools_fail(self):
        malformed = [[], self.rows[:1], self.rows[1:]]
        for row in self.rows:
            malformed.append(self.rows + [row])
        malformed.append([self.rows[0], self.rows[0]])
        malformed.append(self.rows + [{"id": "unknown", "result": "ok"}])
        for rows in malformed:
            with self.subTest(rows=rows):
                self.assertFalse(self.accepts(rows))

    def test_fail_skip_and_unknown_results_cannot_hide(self):
        for index in range(2):
            for outcome in ("fail", "skip", "unknown", None):
                with self.subTest(index=index, outcome=outcome):
                    rows = copy.deepcopy(self.rows)
                    rows[index]["result"] = outcome
                    self.assertFalse(self.accepts(rows))

    def test_non_arrays_fail(self):
        for rows in ({}, None, "ok", 42):
            with self.subTest(rows=rows):
                self.assertFalse(self.accepts(rows))


if __name__ == "__main__":
    unittest.main()
