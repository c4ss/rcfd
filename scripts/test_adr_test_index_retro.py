"""Tests of rule H of ADR 0701: an ADR path in a file header (H0) and ADRs that register a file after the fact (H1 to H3).
Run: python -m unittest scripts/test_adr_test_index_retro.py -v
A small fake workspace is written to a temp folder (not a git repository: rule G gives no primary there); the metadata is given."""
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
    # H0 and its negative controls
    "crates/rcfd-w/tests/h0_path.rs": "//! Limits fixed in docs/adr/0150-plain.md.\n" + T.format("p1_h0_path"),
    "crates/rcfd-w/tests/h0_both.rs": "//! see docs/adr/0150-plain.md\n//! and ADR 0105\n" + T.format("n6_adr_before_path"),
    "crates/rcfd-w/tests/h0_late.rs": "use std::fmt;\n// docs/adr/0150-plain.md\n" + T.format("n5_path_after_code"),
    "crates/rcfd-w/tests/h0_missing.rs": "//! docs/adr/0999-none.md\n" + T.format("n5_path_missing_adr"),
    "crates/rcfd-w/tests/h0_listing.rs": "//! docs/adr/0679-index.md\n" + T.format("n5_path_listing_adr"),
    # H1 to H3 and negative controls
    "crates/rcfd-w/tests/r_h1.rs": T.format("p2_h1"),
    "crates/rcfd-w/tests/r_h2.rs": T.format("p3_h2"),
    "crates/rcfd-w/tests/r_h3.rs": T.format("p4_h3"),
    "crates/rcfd-w/tests/r_n1.rs": T.format("n1_mention_only"),
    "crates/rcfd-w/tests/r_n2.rs": T.format("n2_other_heading"),
    "crates/rcfd-w/tests/r_n3.rs": T.format("n3_phrase_elsewhere"),
    "crates/rcfd-w/tests/r_n4.rs": T.format("n4_listing_adrs"),
    "crates/rcfd-w/tests/r_n7.rs": T.format("n7_row_without_a_limit"),
    "crates/rcfd-w/tests/r_tier_a.rs": T.format("p5_h1_over_h2_h3"),
    "crates/rcfd-w/tests/r_tier_b.rs": T.format("p5_h2_over_h3"),
    "crates/rcfd-w/tests/r_low.rs": T.format("p6_lowest_in_tier"),
    "crates/rcfd-w/tests/r_e1_f020_x.rs": T.format("p8_e1_stays"),
    "crates/rcfd-w/tests/r_hdr.rs": "//! F-020 (ADR 0105)\n" + T.format("p8_e0_stays"),
    "docs/adr/0100-first.md": "# ADR 0100\nSee r_h1.rs, r_h2.rs, r_h3.rs, r_n1.rs, r_tier_a.rs, r_tier_b.rs, r_low.rs and r_e1_f020_x.rs.\n"
                              "| check | file | limit |\n|---|---|---|\n| row | r_tier_a.rs | 5 % |\n| row | r_tier_b.rs | 0.5 |\n",
    "docs/adr/0105-f020-registration-item.md": "# ADR 0105\n",
    "docs/adr/0110-old.md": "# ADR 0110\nText.\n\n## Test files (added 2026-11-17, ADR 0685)\n\n"
                            "Note: crates/rcfd-w/tests/r_h1.rs checks the thing.\n",
    "docs/adr/0115-retro.md": "# ADR 0115\nThe file r_h2.rs was committed without an ADR; this ADR describes it.\n",
    "docs/adr/0116-table.md": "# ADR 0116\n| id | file | limit |\n|---|---|---|\n| T1 | r_h3.rs | at most 3 |\n"
                              "| T2 | r_h3.rs | 5 % |\n",
    "docs/adr/0117-mention.md": "# ADR 0117\nSee r_n1.rs, which runs fine.\n",
    "docs/adr/0118-other-heading.md": "# ADR 0118\n## Tests\n\nSee r_n2.rs for the check.\n",
    "docs/adr/0119-elsewhere.md": "# ADR 0119\nSomething else was committed without an ADR.\n\nSee r_n3.rs.\n",
    "docs/adr/0120-phrase.md": "# ADR 0120\nr_tier_a.rs and r_tier_b.rs had no ADR.\n",
    "docs/adr/0121-row.md": "# ADR 0121\n| id | file | note |\n|---|---|---|\n| T | r_n7.rs | runs fine |\n",
    "docs/adr/0125-low.md": "# ADR 0125\nr_low.rs was committed without an ADR.\n",
    "docs/adr/0130-note.md": "# ADR 0130\n## Test files (added 2026-11-17, ADR 0685)\n\nr_tier_a.rs checks it.\n",
    "docs/adr/0140-low.md": "# ADR 0140\nr_low.rs was ADR-less.\n",
    "docs/adr/0150-plain.md": "# ADR 0150\nNothing cited.\n",
    "docs/adr/0170-f021-notes.md": "# ADR 0170\nr_e1_f020_x.rs and r_hdr.rs were committed without an ADR.\n",
    "docs/adr/0684-listing.md": "# ADR 0684\nr_n4.rs was committed without an ADR.\n",
    "docs/adr/0690-listing.md": "# ADR 0690\nr_n4.rs was committed without an ADR.\n",
    "docs/adr/0701-listing.md": "# ADR 0701\nr_n4.rs was committed without an ADR.\n",
}


class Retro(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.root = tempfile.mkdtemp(prefix="adr_retro_")
        for p, text in FILES.items():
            tg.put(cls.root, p, text)
        cls.meta = meta(cls.root)
        cls.idx = ati.build(cls.root, cls.meta, "git")
        cls.t = {t["name"]: t for t in cls.idx["tests"]}

    @classmethod
    def tearDownClass(cls):
        shutil.rmtree(cls.root, ignore_errors=True)

    def prim(self, name):
        return self.t[name]["primary_adr"], self.t[name]["primary_evidence"]

    def test_p1_h0_path_is_e0(self):
        self.assertEqual(self.prim("p1_h0_path"), ("0150", "E0"))
        self.assertIn({"kind": "E0", "adr": "0150"}, self.t["p1_h0_path"]["evidence"])

    def test_n6_adr_before_path(self):
        self.assertEqual(self.prim("n6_adr_before_path"), ("0105", "E0"))

    def test_n5_path_outside_the_block_or_to_nothing_is_no_e0(self):
        for n in ("n5_path_after_code", "n5_path_missing_adr", "n5_path_listing_adr"):
            self.assertEqual(self.t[n]["evidence"], [], n)
            self.assertIsNone(self.t[n]["primary_adr"], n)

    def test_n1_mention_only_is_not_a_registration(self):
        self.assertEqual(self.t["n1_mention_only"]["adrs"], ["0100", "0117"])
        self.assertIsNone(self.t["n1_mention_only"]["primary_adr"])

    def test_n2_other_heading(self):
        self.assertIn("0118", self.t["n2_other_heading"]["adrs"])
        self.assertIsNone(self.t["n2_other_heading"]["primary_adr"])

    def test_n3_phrase_in_a_unit_that_does_not_cite_the_file(self):
        self.assertIn("0119", self.t["n3_phrase_elsewhere"]["adrs"])
        self.assertIsNone(self.t["n3_phrase_elsewhere"]["primary_adr"])

    def test_n4_listing_adrs_are_no_source(self):
        self.assertEqual(self.t["n4_listing_adrs"]["adrs"], ["0684", "0690", "0701"])
        self.assertIsNone(self.t["n4_listing_adrs"]["primary_adr"])

    def test_n7_table_row_without_a_limit_is_no_registration(self):
        self.assertEqual(self.t["n7_row_without_a_limit"]["adrs"], ["0121"])
        self.assertIsNone(self.t["n7_row_without_a_limit"]["primary_adr"])

    def test_p5_tier_beats_a_lower_number_of_a_lower_tier(self):
        self.assertEqual(self.prim("p5_h1_over_h2_h3"), ("0130", "E2"))   # H1 0130 over H2 0120 over H3 0100
        self.assertEqual(self.prim("p5_h2_over_h3"), ("0120", "E2"))

    def test_p6_lowest_number_in_a_tier(self):
        self.assertEqual(self.prim("p6_lowest_in_tier"), ("0125", "E2"))

    def test_p8_other_primaries_are_not_changed(self):
        self.assertEqual(self.prim("p8_e1_stays"), ("0105", "E1"))
        self.assertEqual(self.prim("p8_e0_stays"), ("0105", "E0"))

    def test_files_map_h1_h2(self):
        """ADR 0714: the per-file list [tier, ADR] of the retro counts, whole map, literals derived by hand from FILES."""
        r = self.idx["coverage"]["primary"]["retro"]
        d = "crates/rcfd-w/tests/"
        self.assertEqual(r["files"], {
            d + "r_h1.rs": [1, "0110"],
            d + "r_tier_a.rs": [1, "0130"],   # H1 over the H2 phrase of 0120
            d + "r_h2.rs": [2, "0115"],
            d + "r_tier_b.rs": [2, "0120"],
            d + "r_low.rs": [2, "0125"],      # the lowest number of 0125 and 0140
        })
        for v in r["files"].values():
            self.assertIn(v[0], (1, 2))

    def test_retro_counts_agree_with_files(self):
        r = self.idx["coverage"]["primary"]["retro"]
        self.assertEqual((r["h1"], r["h2"], r["h3"]), (2, 3, 0))
        tiers = [v[0] for v in r["files"].values()]
        self.assertEqual((tiers.count(1), tiers.count(2)), (r["h1"], r["h2"]))

    def test_other_rules_and_retro_off_do_not_see_it(self):
        want = {"p2_h1": "0110", "p3_h2": "0115", "p4_h3": "0116"}
        for kw in ({"primary_rule": "git", "retro": False}, {"primary_rule": "registered"},
                   {"primary_rule": "registered", "retro": True}):
            idx = ati.build(self.root, self.meta, **kw)
            t = {x["name"]: x for x in idx["tests"]}
            self.assertEqual(t["p1_h0_path"]["evidence"], [], kw)
            self.assertNotIn("retro", idx["coverage"]["primary"], kw)
            for n, adr in want.items():
                self.assertNotEqual(t[n]["primary_adr"], adr if kw["primary_rule"] == "git" else None, (kw, n))
                if kw["primary_rule"] == "git":
                    self.assertIsNone(t[n]["primary_adr"], (kw, n))


@unittest.skipUnless(tg.HAVE_GIT, "git is not installed")
class RetroOnGit(unittest.TestCase):
    """P7: a rule-G primary is not changed by an ADR that says `without an ADR`; a file that G leaves null takes it."""

    def test_p7_rule_g_primary_stays(self):
        tmp = tempfile.mkdtemp(prefix="adr_retro_git_")
        try:
            root = os.path.join(tmp, "w")
            tg.make_repo(root)
            tg.put(root, "docs/adr/0150-retro.md", "# ADR 0150\nt_one.rs and t_init.rs were committed without an ADR.\n")
            m = meta(root)
            t = {x["name"]: x for x in ati.build(root, m, "git")["tests"]}
            self.assertEqual(t["g_one"]["primary_adr"], "0120")   # from git
            self.assertEqual(t["g_init"]["primary_adr"], "0150")  # G null, H2
            u = {x["name"]: x for x in ati.build(root, m, "git", retro=False)["tests"]}
            self.assertIsNone(u["g_init"]["primary_adr"])
        finally:
            shutil.rmtree(tmp, ignore_errors=True)


if __name__ == "__main__":
    unittest.main()
