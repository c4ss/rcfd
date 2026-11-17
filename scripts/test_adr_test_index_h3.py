"""Tests of ADR 0703 (F-785): rule H3 of ADR 0701 (a table row with a limit as a registration) is removed; H0 to H2 are unchanged.
Run: python -m unittest scripts/test_adr_test_index_h3.py -v   (a fake workspace in a temp folder; no git needed)"""
import os
import shutil
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import adr_test_index as ati  # noqa: E402
from test_adr_test_index_primary import meta  # noqa: E402
import test_adr_test_index_git as tg  # noqa: E402

T = "#[test]\nfn {}() {{}}\n"
FILES = {
    "crates/rcfd-w/src/lib.rs": "",
    "crates/rcfd-w/tests/a1_row.rs": T.format("a1_row_with_limit"),
    "crates/rcfd-w/tests/a3_h1.rs": T.format("a3_h1"),
    "crates/rcfd-w/tests/a3_h2.rs": T.format("a3_h2"),
    "crates/rcfd-w/tests/a4_h0.rs": "//! Limits fixed in docs/adr/0150-plain.md.\n" + T.format("a4_h0"),
    "crates/rcfd-w/tests/a5_both.rs": T.format("a5_row_and_phrase"),
    "docs/adr/0100-f001-registration-item.md": "# ADR 0100\n| id | file | limit |\n|---|---|---|\n| T1 | a1_row.rs | 5 % |\n"
                                               "| T2 | a5_both.rs | 0.5 |\n",
    "docs/adr/0110-old.md": "# ADR 0110\n## Test files (added 2026-11-17, ADR 0685)\n\nNote: a3_h1.rs checks the thing.\n",
    "docs/adr/0115-retro.md": "# ADR 0115\nThe file a3_h2.rs was committed without an ADR; this ADR describes it.\n",
    "docs/adr/0120-phrase.md": "# ADR 0120\na5_both.rs had no ADR.\n",
    "crates/rcfd-w/tests/a6_row_phrase.rs": T.format("a6_row_phrase"),
    "crates/rcfd-w/tests/a7_row_section.rs": T.format("a7_row_section"),
    "docs/adr/0130-rows.md": "# ADR 0130\n| id | file | note |\n|---|---|---|\n| T | a6_row_phrase.rs | had no ADR |\n\n"
                             "## Test files (added 2026-11-17, ADR 0685)\n\n| file | note |\n|---|---|\n| a7_row_section.rs | checks it |\n",
    "docs/adr/0150-plain.md": "# ADR 0150\nNothing cited.\n",
}


class H3Removed(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.root = tempfile.mkdtemp(prefix="adr_h3_")
        for p, text in FILES.items():
            tg.put(cls.root, p, text)
        cls.idx = ati.build(cls.root, meta(cls.root), "git")
        cls.t = {t["name"]: t for t in cls.idx["tests"]}

    @classmethod
    def tearDownClass(cls):
        shutil.rmtree(cls.root, ignore_errors=True)

    def prim(self, name):
        return self.t[name]["primary_adr"], self.t[name]["primary_evidence"]

    def test_constant_is_off(self):
        self.assertIs(ati.RETRO_H3, False)

    def test_a1_row_with_a_limit_gives_no_primary_even_in_a_registration_adr(self):
        self.assertEqual(self.t["a1_row_with_limit"]["adrs"], ["0100"])
        self.assertIsNone(self.t["a1_row_with_limit"]["primary_adr"])

    def test_a2_h3_key_present_and_zero(self):
        r = self.idx["coverage"]["primary"]["retro"]
        self.assertEqual(r["h3"], 0)
        self.assertEqual((r["h1"], r["h2"]), (2, 3))

    def test_a3_h1_and_h2_unchanged(self):
        self.assertEqual(self.prim("a3_h1"), ("0110", "E2"))
        self.assertEqual(self.prim("a3_h2"), ("0115", "E2"))

    def test_a4_h0_unchanged(self):
        self.assertEqual(self.prim("a4_h0"), ("0150", "E0"))

    def test_a6_a7_table_rows_of_h1_and_h2_still_count(self):
        self.assertEqual(self.prim("a6_row_phrase"), ("0130", "E2"))
        self.assertEqual(self.prim("a7_row_section"), ("0130", "E2"))

    def test_a5_h2_phrase_wins_over_a_row(self):
        self.assertEqual(self.prim("a5_row_and_phrase"), ("0120", "E2"))
        self.assertEqual(self.t["a5_row_and_phrase"]["adrs"], ["0100", "0120"])


if __name__ == "__main__":
    unittest.main()
