"""Tests of the wave-tag mapping (E6), the binary-name evidence (E7) and the primary ADR of scripts/adr_test_index.py
(ADR 0679: N1 to N6). Run: python -m unittest scripts/test_adr_test_index_wave.py -v
A small fake workspace is written to a temp folder; the metadata is given, so cargo is not run."""
import json
import os
import shutil
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import adr_test_index as ati  # noqa: E402

T = "#[test]\nfn {}() {{}}\n"
FILES = {
    "crates/rcfd-w/src/lib.rs": "",
    "crates/rcfd-w/tests/zz_w99x_case.rs": T.format("n1_no_adr"),          # N1: tag with no wave ADR
    "crates/rcfd-w/tests/amb_w50_case.rs": T.format("n2_prefix"),          # N2: prefix of 50a and 50b
    "crates/rcfd-w/tests/dup_w51a_case.rs": T.format("n2_dup"),            # N2: two ADR files of tag 51a
    "crates/rcfd-w/tests/y_wave10a_q.rs": T.format("n3_wave"),             # N3
    "crates/rcfd-w/tests/plain.rs": T.format("n3_plain"),
    "crates/rcfd-w/tests/cited_by_four_x.rs": T.format("n4_four"),         # N4
    "crates/rcfd-w/tests/cited_by_two_x.rs": T.format("n4_two"),
    "crates/rcfd-w/tests/only_in_index_x.rs": T.format("n4_only_0653"),
    "crates/rcfd-w/tests/f010_w10a_both.rs": T.format("n5_item_and_wave"),  # N5: E1 (ADR 0105) beats E6 (ADR 0100)
    "crates/rcfd-w/tests/w11a_reg.rs": T.format("n5_reg"),                 # N5: registration ADR preferred inside E6? no: one ADR
    "crates/rcfd-w/tests/f020_item.rs": T.format("n5_item_two_adrs"),      # N5: item with 2 ADRs, registration preferred
    "crates/rcfd-w/tests/w12a_cited.rs": T.format("n5_wave_and_file"),     # N5: E6 beats E2 although the E2 ADR has a lower number
    "crates/rcfd-w/tests/mapped_already_w10a.rs": T.format("n5_mapped_and_wave"),
    "crates/rcfd-w/tests/f020_hdr.rs": "//! F-020 (ADR 0105): header names the registering ADR.\n" + T.format("n7_header"),
    "crates/rcfd-w/tests/hdr_none.rs": "//! ADR 9999 does not exist.\n" + T.format("n8_missing"),
    "crates/rcfd-w/tests/hdr_idx.rs": "//! ADR 0653 lists binaries.\n" + T.format("n8_index"),
    "crates/rcfd-w/tests/hdr_late.rs": T.format("n9_late") + "// ADR 0105 after the first code line\n",
    "docs/adr/0100-wave10a-first.md": "# ADR 0100\n`mapped_already_w10a.rs` is described here.\n",
    "docs/adr/0101-wave50a-x.md": "# ADR 0101\n",
    "docs/adr/0102-wave50b-x.md": "# ADR 0102\n",
    "docs/adr/0103-wave51a-x.md": "# ADR 0103\n",
    "docs/adr/0104-wave51a-y.md": "# ADR 0104\n",
    "docs/adr/0105-f010-registration-item.md": "# ADR 0105\n",
    "docs/adr/0106-notes.md": "# ADR 0106\ncited_by_four_x and cited_by_two_x\n",
    "docs/adr/0107-notes.md": "# ADR 0107\ncited_by_four_x and cited_by_two_x\n",
    "docs/adr/0108-notes.md": "# ADR 0108\ncited_by_four_x\n",
    "docs/adr/0109-notes.md": "# ADR 0109\ncited_by_four_x\n",
    "docs/adr/0110-f020-first-notes.md": "# ADR 0110\n",
    "docs/adr/0111-f020-registration-second.md": "# ADR 0111\n",
    "docs/adr/0112-wave11a-reg.md": "# ADR 0112\n",
    "docs/adr/0095-wave12a-late-named.md": "# ADR 0095\n",
    "docs/adr/0090-notes.md": "# ADR 0090\nThe file `w12a_cited.rs` is cited here, with a lower number than its wave ADR.\n",
    "docs/adr/0653-index.md": "# ADR 0653\nonly_in_index_x cited_by_four_x\n",
    "docs/roadmap.json": json.dumps([{"id": "F-010", "note": "DONE."}]),
}


def meta(root):
    w = os.path.join(root, "crates", "rcfd-w")
    ts = []
    for f in sorted(os.listdir(os.path.join(w, "tests"))):
        ts.append({"name": f[:-3], "kind": ["test"], "src_path": os.path.join(w, "tests", f), "test": True})
    ts.append({"name": "rcfd_w", "kind": ["lib"], "src_path": os.path.join(w, "src", "lib.rs"), "test": True})
    return {"workspace_root": root, "packages": [
        {"name": "rcfd-w", "manifest_path": os.path.join(w, "Cargo.toml"), "dependencies": [], "targets": ts}]}


class Wave(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.root = tempfile.mkdtemp(prefix="adr_wave_")
        for p, text in FILES.items():
            f = os.path.join(cls.root, *p.split("/"))
            os.makedirs(os.path.dirname(f), exist_ok=True)
            with open(f, "w", encoding="utf-8") as h:
                h.write(text)
        cls.idx = ati.build(cls.root, meta(cls.root))
        cls.t = {t["name"]: t for t in cls.idx["tests"]}
        cls.cov = cls.idx["coverage"]

    @classmethod
    def tearDownClass(cls):
        shutil.rmtree(cls.root, ignore_errors=True)

    def test_tag_regex(self):
        self.assertEqual(ati.waves_in_stem("cgrid_case_w34a"), ["34a"])
        self.assertEqual(ati.waves_in_stem("convergence_wave17a"), ["17a"])
        self.assertEqual(ati.waves_in_stem("w48g1_x"), ["48g1"])
        self.assertEqual(ati.waves_in_stem("w22"), ["22"])
        self.assertEqual(ati.waves_in_stem("flow_w3_x"), [])
        self.assertEqual(ati.waves_in_stem("showcase_w34a_and_w35c"), ["34a", "35c"])
        self.assertEqual(ati.waves_in_stem("awave12a"), [])

    def test_n1_no_wave_adr_stays_unmapped(self):
        t = self.t["n1_no_adr"]
        self.assertEqual(t["adrs"], [])
        self.assertIsNone(t["primary_adr"])
        self.assertIn("99x", self.cov["wave"]["no_wave_adr_tags"])

    def test_n2_ambiguous_not_mapped(self):
        self.assertEqual(self.t["n2_prefix"]["adrs"], [])
        self.assertEqual(self.cov["wave"]["ambiguous_tags"]["50"], ["0101", "0102"])
        self.assertEqual(self.t["n2_dup"]["adrs"], [])
        self.assertEqual(self.cov["wave"]["ambiguous_tags"]["51a"], ["0103", "0104"])

    def test_n3_wave_file_mapped_and_primary(self):
        t = self.t["n3_wave"]
        self.assertEqual(t["adrs"], ["0100"])
        self.assertEqual((t["primary_adr"], t["primary_evidence"]), ("0100", "E6"))
        self.assertEqual(self.cov["wave"]["mapped_tags"]["10a"], "0100")
        self.assertEqual(self.t["n3_plain"]["wave"], [])

    def test_n4_binary_named_in_adr_text(self):
        self.assertEqual(self.t["n4_four"]["adrs"], [])
        self.assertIn("cited_by_four_x", self.cov["wave"]["e7_too_common"])
        two = self.t["n4_two"]
        self.assertEqual(two["adrs"], ["0106", "0107"])
        self.assertEqual((two["primary_adr"], two["primary_evidence"]), ("0106", "E7"))
        self.assertEqual(self.t["n4_only_0653"]["adrs"], [])  # ADR 0653 lists binaries, it is no evidence

    def test_n5_primary_tier_order(self):
        t = self.t["n5_item_and_wave"]  # E1 (ADR 0105) before E6 (ADR 0100)
        self.assertEqual(sorted(t["adrs"]), ["0100", "0105"])
        self.assertEqual((t["primary_adr"], t["primary_evidence"]), ("0105", "E1"))
        t = self.t["n5_wave_and_file"]  # E6 (ADR 0095) before E2 (ADR 0090, the lower number)
        self.assertEqual(sorted(t["adrs"]), ["0090", "0095"])
        self.assertEqual((t["primary_adr"], t["primary_evidence"]), ("0095", "E6"))

    def test_n5_registration_before_lower_number(self):
        t = self.t["n5_item_two_adrs"]  # ADR 0110 (lower, not a registration) and 0111 (registration)
        self.assertEqual(sorted(t["adrs"]), ["0110", "0111"])
        self.assertEqual((t["primary_adr"], t["primary_evidence"]), ("0111", "E1"))

    def test_wave_is_added_to_a_file_that_is_already_mapped(self):
        t = self.t["n5_mapped_and_wave"]  # E2 (ADR 0100 cites it) and E6 (same ADR): one ADR, primary by E6
        self.assertEqual(t["adrs"], ["0100"])
        self.assertEqual(t["primary_evidence"], "E6")
        t = self.t["n5_reg"]
        self.assertEqual(t["adrs"], ["0112"])

    def test_p1_every_mapped_test_has_one_primary_in_its_adrs(self):
        for t in self.idx["tests"]:
            if t["adrs"]:
                self.assertIn(t["primary_adr"], t["adrs"], t["name"])
            else:
                self.assertIsNone(t["primary_adr"], t["name"])
        self.assertEqual(self.cov["primary"]["with_primary"], sum(1 for t in self.idx["tests"] if t["adrs"]))
        self.assertIn("rcfd-w/y_wave10a_q::n3_wave", self.idx["adrs"]["0100"]["primary_tests"])

    def test_n7_header_adr_is_primary(self):
        t = self.t["n7_header"]  # E1 would give 0111 (registration); the header names 0105
        self.assertEqual(sorted(t["adrs"]), ["0105", "0110", "0111"])
        self.assertEqual((t["primary_adr"], t["primary_evidence"]), ("0105", "E0"))

    def test_n8_header_adr_must_exist_and_not_be_the_index(self):
        for name in ("n8_missing", "n8_index"):
            self.assertEqual(self.t[name]["adrs"], [], name)
            self.assertIsNone(self.t[name]["primary_adr"], name)

    def test_n9_header_stops_at_first_code(self):
        self.assertEqual(self.t["n9_late"]["adrs"], [])

    def test_n6_old_fields_kept(self):
        for k in ("binaries", "crates", "tests", "adrs", "items", "coverage"):
            self.assertIn(k, self.idx)
        for k in ("crate", "binary", "target", "kind", "file", "name", "ignored", "adrs", "items", "evidence"):
            self.assertIn(k, self.idx["tests"][0])
        for k in ("files", "items", "tests"):
            self.assertIn(k, self.idx["adrs"]["0100"])
        for k in ("integration_binaries", "unit_binaries", "all_tests", "unmapped_binaries"):
            self.assertIn(k, self.cov)


if __name__ == "__main__":
    unittest.main()
