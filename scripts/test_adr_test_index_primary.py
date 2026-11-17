"""Tests of the primary ADR from registering evidence only (ADR 0684: N1 to N8). Run: python -m unittest scripts/test_adr_test_index_primary.py -v
A small fake workspace is written to a temp folder; the metadata is given, so cargo is not run."""
import json
import os
import shutil
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import adr_test_index as ati  # noqa: E402
import gate_attribute as ga  # noqa: E402

T = "#[test]\nfn {}() {{}}\n"
FILES = {
    "crates/rcfd-w/src/lib.rs": "",
    "crates/rcfd-w/tests/only_seven_bin.rs": T.format("n1_e7_only"),       # N1: ADR 0120 names the binary, not the file
    "crates/rcfd-w/tests/plain_e3.rs": T.format("n2_distinctive_function_name"),  # N2: E3 only
    "crates/rcfd-w/tests/plain_e4.rs": T.format("n2_e4"),                  # N2: E4 only (roadmap note cites the file)
    "crates/rcfd-w/tests/cited_three.rs": T.format("n3_earliest"),         # N3
    "crates/rcfd-w/tests/cited_listing.rs": T.format("n4_listing"),        # N4
    "crates/rcfd-w/tests/cited_only_listing.rs": T.format("n4_only_listing"),
    "crates/rcfd-w/tests/cited_item_f020_x.rs": T.format("n5_e1_beats_e2"),  # N5
    "crates/rcfd-w/tests/hdr_beats_e2.rs": "//! F-020 (ADR 0105)\n" + T.format("n5_e0_beats_e2"),
    "crates/rcfd-w/tests/w12a_cited_early.rs": T.format("n5_e6_beats_e2"),
    "docs/adr/0090-early.md": "# ADR 0090\nThe file cited_item_f020_x.rs and hdr_beats_e2.rs and w12a_cited_early.rs.\n",
    "docs/adr/0095-wave12a-named.md": "# ADR 0095\n",
    "docs/adr/0105-f020-registration-item.md": "# ADR 0105\n",
    "docs/adr/0120-binary.md": "# ADR 0120\nonly_seven_bin runs ok.\n",
    "docs/adr/0121-fn.md": "# ADR 0121\nn2_distinctive_function_name passes.\n",
    "docs/adr/0130-later.md": "# ADR 0130\nSee cited_three.rs.\n",
    "docs/adr/0140-registration-late.md": "# ADR 0140\nSee cited_three.rs.\n",
    "docs/adr/0150-after.md": "# ADR 0150\nSee cited_listing.rs.\n",
    "docs/adr/0125-early-cite.md": "# ADR 0125\nSee cited_three.rs.\n",
    "docs/adr/0160-f900-notes.md": "# ADR 0160\n",
    "docs/adr/0653-index.md": "# ADR 0653\ncited_listing.rs cited_only_listing.rs\n",
    "docs/adr/0679-index.md": "# ADR 0679\ncited_only_listing.rs\n",
    "docs/adr/0684-index.md": "# ADR 0684\ncited_only_listing.rs\n",
    "docs/roadmap.json": json.dumps([{"id": "F-900", "note": "DONE. See plain_e4.rs."}]),
}


def meta(root):
    w = os.path.join(root, "crates", "rcfd-w")
    ts = [{"name": f[:-3], "kind": ["test"], "src_path": os.path.join(w, "tests", f), "test": True}
          for f in sorted(os.listdir(os.path.join(w, "tests")))]
    ts.append({"name": "rcfd_w", "kind": ["lib"], "src_path": os.path.join(w, "src", "lib.rs"), "test": True})
    return {"workspace_root": root, "packages": [
        {"name": "rcfd-w", "manifest_path": os.path.join(w, "Cargo.toml"), "dependencies": [], "targets": ts}]}


class Primary(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.root = tempfile.mkdtemp(prefix="adr_primary_")
        for p, text in FILES.items():
            f = os.path.join(cls.root, *p.split("/"))
            os.makedirs(os.path.dirname(f), exist_ok=True)
            with open(f, "w", encoding="utf-8") as h:
                h.write(text)
        cls.meta = meta(cls.root)
        cls.old = ati.build(cls.root, cls.meta)
        cls.new = ati.build(cls.root, cls.meta, "registered")
        cls.t = {t["name"]: t for t in cls.new["tests"]}
        cls.o = {t["name"]: t for t in cls.old["tests"]}

    @classmethod
    def tearDownClass(cls):
        shutil.rmtree(cls.root, ignore_errors=True)

    def test_n1_binary_named_in_an_adr_is_no_primary(self):
        self.assertEqual(self.t["n1_e7_only"]["adrs"], ["0120"])
        self.assertIsNone(self.t["n1_e7_only"]["primary_adr"])
        self.assertEqual((self.o["n1_e7_only"]["primary_adr"], self.o["n1_e7_only"]["primary_evidence"]), ("0120", "E7"))

    def test_n2_function_and_roadmap_evidence_is_no_primary(self):
        t = self.t["n2_distinctive_function_name"]
        self.assertEqual(t["adrs"], ["0121"])
        self.assertIsNone(t["primary_adr"])
        t = self.t["n2_e4"]
        self.assertEqual(t["adrs"], ["0160"])
        self.assertEqual(self.o["n2_e4"]["primary_evidence"], "E4")
        self.assertIsNone(t["primary_adr"])

    def test_n3_earliest_citing_adr_wins_not_the_registration_one(self):
        t = self.t["n3_earliest"]
        self.assertEqual(sorted(t["adrs"]), ["0125", "0130", "0140"])
        self.assertEqual((t["primary_adr"], t["primary_evidence"]), ("0125", "E2"))
        self.assertEqual(self.o["n3_earliest"]["primary_adr"], "0140")  # rule P: the registration file first

    def test_n4_listing_adrs_give_no_e2_primary(self):
        t = self.t["n4_listing"]
        self.assertEqual(sorted(t["adrs"]), ["0150", "0653"])
        self.assertEqual(t["primary_adr"], "0150")
        t = self.t["n4_only_listing"]
        self.assertEqual(sorted(t["adrs"]), ["0653", "0679", "0684"])
        self.assertIsNone(t["primary_adr"])
        self.assertEqual(set(ati.LISTING_ADRS), {"0653", "0679", "0684"})

    def test_n5_e0_e1_e6_still_beat_e2(self):
        for name, want in (("n5_e1_beats_e2", ("0105", "E1")), ("n5_e0_beats_e2", ("0105", "E0")), ("n5_e6_beats_e2", ("0095", "E6"))):
            t = self.t[name]
            self.assertIn("0090", t["adrs"], name)
            self.assertEqual((t["primary_adr"], t["primary_evidence"]), want, name)

    def test_n6_only_the_primary_fields_differ(self):
        for a, b in zip(self.old["tests"], self.new["tests"]):
            for k in a:
                if k not in ("primary_adr", "primary_evidence"):
                    self.assertEqual(a[k], b[k], (a["name"], k))
        for n, a in self.old["adrs"].items():
            for k in ("files", "items", "tests"):
                self.assertEqual(a[k], self.new["adrs"][n][k], (n, k))
        for k in ("binaries", "crates", "items"):
            self.assertEqual(self.old[k], self.new[k], k)
        for k, v in self.old["coverage"].items():
            if k != "primary":
                self.assertEqual(v, self.new["coverage"][k], k)
        self.assertNotEqual(self.old["adrs"]["0140"]["primary_tests"], self.new["adrs"]["0140"]["primary_tests"])

    def test_n6_primary_is_in_adrs_or_null_and_a_registering_kind(self):
        for t in self.new["tests"]:
            if t["primary_adr"]:
                self.assertIn(t["primary_adr"], t["adrs"])
                self.assertIn(t["primary_evidence"], ati.REGISTERED_TIERS)
            else:
                self.assertIsNone(t["primary_evidence"])

    def test_n7_command_line_default_is_the_registered_rule(self):
        mp = os.path.join(self.root, "meta.json")
        with open(mp, "w") as fh:
            json.dump(self.meta, fh)
        for flag, want in ((None, None), ("p0679", "0120"), ("registered", None)):
            out = os.path.join(self.root, f"out_{flag}.json")
            args = ["--root", self.root, "--metadata", mp, "--out", out] + (["--primary", flag] if flag else [])
            self.assertEqual(ati.main(args), 0)
            with open(out) as fh:
                idx = json.load(fh)
            self.assertEqual({t["name"]: t for t in idx["tests"]}["n1_e7_only"]["primary_adr"], want, flag)

    def test_n8_a_record_without_primary_keeps_the_adr_list_cell(self):
        self.assertEqual(ga.adr_cell({"adrs": ["0120", "0121"], "primary_adr": None}), "0120, 0121")
        self.assertEqual(ga.adr_cell({"adrs": ["0120", "0121"]}), "0120, 0121")


if __name__ == "__main__":
    unittest.main()
