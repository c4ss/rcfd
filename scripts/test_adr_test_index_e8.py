"""Tests of evidence E8, the ADR or item named by the commit that adds a test file (ADR 0720: T1 to T11).
Run: python -m unittest scripts/test_adr_test_index_e8.py -v
The fake git repository of test_adr_test_index_git.py (imported, not edited) plus a second one for the cases that need their own."""
import json
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import adr_test_index as ati  # noqa: E402
from test_adr_test_index_git import ADRS, HAVE_GIT, T, TESTS, make_repo, put, run, tests_of  # noqa: E402
from test_adr_test_index_primary import meta  # noqa: E402

ADRS2 = dict(ADRS)
ADRS2["docs/adr/0150-retro-notes.md"] = (
    "# ADR 0150\n\n## Test files (added 2026-01-01, ADR 0150)\n\n- `t_retro.rs` checks the retro file.\n")
ADRS2["docs/adr/0160-f030-registration-other.md"] = "# ADR 0160\nNothing cited.\n"


def make_repo2(repo):
    run(repo, "init", "-q")
    n = [0]

    def commit(msg, files, body=None):
        n[0] += 1
        for p, text in files:
            put(repo, p, text)
        run(repo, "add", "-A")
        run(repo, "commit", "-q", "-m", msg, *(["-m", body] if body else []), date=f"2026-01-{n[0]:02d}T00:00:00")

    commit("Initial import", list(ADRS2.items()) + [("crates/rcfd-w/src/lib.rs", ""),
                                                      (TESTS + "t_retro.rs", T.format("r_retro"))])
    commit("w: nonexistent ADR (ADR 0999)", [(TESTS + "t_nx.rs", T.format("r_nx"))])
    commit("w: item without a registration ADR (F-099)", [(TESTS + "t_noreg.rs", T.format("r_noreg"))])
    commit("w: item 030 (F-030)", [(TESTS + "t_i30.rs", T.format("r_i30"))])
    commit("w: second ADR names both (ADR 0140, ADR 0120)", [(TESTS + "t_two.rs", T.format("r_two"))])
    commit("w: e3 only (ADR 0140)", [(TESTS + "t_e3.rs", T.format("r_e3_distinctive_name"))],
           body="Names ADR 0120 and F-020 in the body, which is not read.")
    commit("w: one more (ADR 0130)", [(TESTS + "t_late.rs", T.format("r_late"))])
    commit("w: later commit names ADR 0140", [(TESTS + "t_late.rs", T.format("r_late") + "// touched\n")])
    return repo


def strip(idx, keys=("adrs", "items", "evidence", "primary_adr", "primary_evidence")):
    return [{k: v for k, v in t.items() if k not in keys} for t in idx["tests"]]


@unittest.skipUnless(HAVE_GIT, "git is not installed")
class E8(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.mkdtemp(prefix="adr_e8_")
        cls.root = os.path.join(cls.tmp, "full")
        make_repo(cls.root)
        cls.meta = meta(cls.root)
        cls.off = ati.build(cls.root, cls.meta, "git")
        cls.on = ati.build(cls.root, cls.meta, "git", e8=True)
        cls.t, cls.o = tests_of(cls.on), tests_of(cls.off)
        cls.root2 = os.path.join(cls.tmp, "second")
        os.makedirs(cls.root2)
        make_repo2(cls.root2)
        cls.meta2 = meta(cls.root2)
        cls.on2 = tests_of(ati.build(cls.root2, cls.meta2, "git", e8=True))
        cls.off2 = tests_of(ati.build(cls.root2, cls.meta2, "git"))

    @classmethod
    def tearDownClass(cls):
        def force(fn, p, _):
            os.chmod(p, 0o700)
            fn(p)
        shutil.rmtree(cls.tmp, onerror=force)

    def e8(self, t):
        return [(e["adr"], e["via"]) for e in t["evidence"] if e["kind"] == "E8"]

    def test_t1_adr_named_by_the_commit_without_citing_the_file(self):
        t, o = self.t["g_notcited"], self.o["g_notcited"]
        self.assertEqual(self.e8(t), [("0140", "subject")])
        self.assertEqual(t["adrs"], ["0130", "0140"])
        self.assertEqual(o["adrs"], ["0130"])
        self.assertIsNone(o["primary_adr"])
        self.assertEqual((t["primary_adr"], t["primary_evidence"]), ("0140", "E8"))
        self.assertEqual(self.on["coverage"]["primary"]["git"]["not_in_adrs"], 0)
        self.assertEqual(t["evidence"][:len(o["evidence"])], o["evidence"])  # the old evidence first, E8 appended

    def test_t2_item_named_takes_its_registration_adr_and_the_adr_comes_first(self):
        t = self.t["g_item"]
        self.assertEqual(self.e8(t), [("0105", "F-020")])
        self.assertEqual((t["primary_adr"], t["primary_evidence"]), ("0105", "E8"))
        both = self.t["g_both"]
        self.assertEqual(self.e8(both), [("0120", "subject"), ("0105", "F-020")])
        self.assertEqual(both["primary_adr"], "0120")
        self.assertIn("F-020", both["items"])
        noreg = self.t["g_noreg"]  # ADR 0170 is named -f021-notes, so it is F-021's registration ADR by the file-name rule
        self.assertEqual(self.e8(noreg), [("0170", "F-021")])
        self.assertIn("F-021", noreg["items"])
        self.assertNotIn("F-021", self.o["g_noreg"]["items"])
        self.assertEqual(noreg["primary_adr"], "0170")

    def test_t3_only_the_adding_commit_counts(self):
        self.assertEqual(self.e8(self.t["g_one"]), [("0120", "subject")])  # ADR 0130 touched it later
        self.assertEqual(self.e8(self.t["g_ren"]), [("0120", "subject")])  # renamed by a commit naming ADR 0130
        self.assertEqual(self.e8(self.t["g_readd"]), [("0120", "subject")])  # deleted, added again by ADR 0130
        self.assertEqual(self.e8(self.on2["r_late"]), [("0130", "subject")])

    def test_t4_nothing_named_no_such_adr_no_registration(self):
        self.assertEqual(self.e8(self.t["g_init"]), [])  # initial import; its body names ADR 0120
        self.assertEqual(self.e8(self.on2["r_nx"]), [])  # ADR 0999 does not exist
        self.assertEqual(self.e8(self.on2["r_noreg"]), [])  # F-099 has no registration ADR
        self.assertIsNone(self.on2["r_nx"]["primary_adr"])
        self.assertEqual(self.e8(self.on2["r_i30"]), [("0160", "F-030")])  # F-030 has the registration ADR 0160
        self.assertIn("F-030", self.on2["r_i30"]["items"])

    def test_t5_body_is_not_read(self):
        self.assertEqual(self.e8(self.on2["r_e3_distinctive_name"]), [("0140", "subject")])

    def test_t6_tiers(self):
        h = self.t["g_hdr"]
        self.assertEqual((h["primary_adr"], h["primary_evidence"]), ("0105", "E0"))  # E0 outranks E8
        self.assertEqual(self.e8(h), [("0130", "subject")])
        one = self.t["g_one"]
        self.assertEqual((one["primary_adr"], one["primary_evidence"]), ("0120", "E8"))  # E8 beats E2
        e3 = self.t["g_e3_distinctive_name"]  # evidence E3 only, plus E8
        self.assertEqual((e3["primary_adr"], e3["primary_evidence"]), ("0130", "E8"))
        self.assertIsNone(self.o["g_e3_distinctive_name"]["primary_adr"])
        two = self.on2["r_two"]
        self.assertEqual(self.e8(two), [("0140", "subject"), ("0120", "subject")])
        self.assertEqual(two["primary_adr"], "0140")
        retro = self.on2["r_retro"]  # E2-only, no E8: rule H1 of ADR 0701 as before
        self.assertEqual((retro["primary_adr"], retro["primary_evidence"]), ("0150", "E2"))
        self.assertEqual((self.off2["r_retro"]["primary_adr"], self.off2["r_retro"]["primary_evidence"]), ("0150", "E2"))

    def test_t7_off_is_adr_0690_exactly(self):
        self.assertEqual(self.off, ati.build(self.root, self.meta, "git", e8=False))
        self.assertEqual(self.off, ati.build(self.root, self.meta, "git"))
        for t in self.off["tests"]:
            self.assertFalse([e for e in t["evidence"] if e["kind"] == "E8"])
        self.assertNotIn("e8", self.off["coverage"]["primary"])
        for rule in ("registered", "p0679"):
            self.assertEqual(ati.build(self.root, self.meta, rule), ati.build(self.root, self.meta, rule, e8=True))

    def test_t8_on_differs_only_in_the_extended_fields(self):
        self.assertEqual(strip(self.on), strip(self.off))
        for key in ("binaries", "crates", "items"):
            self.assertEqual(self.on[key], self.off[key])
        for n, a in self.on["adrs"].items():
            self.assertEqual(a["files"], self.off["adrs"][n]["files"])
        for name, t in self.t.items():
            o = self.o[name]
            self.assertTrue(set(o["adrs"]) <= set(t["adrs"]), name)
            self.assertTrue(set(o["items"]) <= set(t["items"]), name)
            self.assertEqual([e for e in t["evidence"] if e["kind"] != "E8"], o["evidence"], name)
            added = {e["adr"] for e in t["evidence"] if e["kind"] == "E8"}
            self.assertEqual(set(t["adrs"]), set(o["adrs"]) | added, name)
            if o["primary_evidence"] in ("E0", "E1", "E6"):
                self.assertEqual((t["primary_adr"], t["primary_evidence"]), (o["primary_adr"], o["primary_evidence"]), name)
        e = self.on["coverage"]["primary"]["e8"]
        links = sum(len([x for x in t["evidence"] if x["kind"] == "E8"]) for t in self.on["tests"])
        self.assertEqual(e["links"], links)
        self.assertEqual(e["tests"], sum(1 for t in self.on["tests"] if any(x["kind"] == "E8" for x in t["evidence"])))
        self.assertEqual(e["primary_tests"], sum(1 for t in self.on["tests"] if t["primary_evidence"] == "E8"))
        g = self.on["coverage"]["primary"]["git"]
        self.assertEqual(g["e2_tests"], g["from_git"] + g["no_commit_rule"] + g["not_in_adrs"] + g["no_origin"])
        self.assertEqual(self.on["coverage"]["primary"]["with_primary"], sum(self.on["coverage"]["primary"]["by_evidence"].values()))

    def test_t9_no_full_history_no_e8(self):
        clone = os.path.join(self.tmp, "shallow")
        run(self.tmp, "clone", "-q", "--depth", "1", pathlib.Path(self.root).as_uri(), clone)
        plain = os.path.join(self.tmp, "plain")
        shutil.copytree(self.root, plain, ignore=shutil.ignore_patterns(".git"))
        outer = os.path.join(self.tmp, "outer")
        base = make_repo(outer, "ws")
        for root, hist in ((clone, "shallow"), (plain, "none"), (base, "none")):
            m = meta(root)
            on, off = ati.build(root, m, "git", e8=True), ati.build(root, m, "git")
            self.assertEqual(on["coverage"]["primary"]["git"]["history"], hist)
            self.assertEqual(on["coverage"]["primary"]["e8"]["links"], 0)
            self.assertEqual(on["coverage"]["primary"]["e8"]["primary_tests"], 0)
            for t in on["tests"]:
                self.assertFalse([e for e in t["evidence"] if e["kind"] == "E8"])
            self.assertEqual(on["tests"], off["tests"])

    def test_t10_at_most_two_git_processes(self):
        real = subprocess.run
        calls = []

        def counting(cmd, *a, **k):
            calls.append(cmd)
            return real(cmd, *a, **k)
        with mock.patch.object(ati.subprocess, "run", counting):
            idx = ati.build(self.root, self.meta, "git", e8=True)
        self.assertEqual(len(calls), 2)
        self.assertTrue(all(c[0] == "git" for c in calls))
        self.assertEqual(idx["coverage"]["primary"]["git"]["git_calls"], 2)

    def test_t11_command_line(self):
        mp = os.path.join(self.tmp, "meta.json")
        with open(mp, "w") as fh:
            json.dump(self.meta, fh)
        for flags, want in (([], "0140"), (["--e8"], "0140"), (["--no-e8"], None), (["--primary", "registered"], "0130"),
                            (["--primary", "registered", "--e8"], "0130")):
            out = os.path.join(self.tmp, "out.json")
            self.assertEqual(ati.main(["--root", self.root, "--metadata", mp, "--out", out] + flags), 0)
            with open(out) as fh:
                idx = json.load(fh)
            self.assertEqual(tests_of(idx)["g_notcited"]["primary_adr"], want, flags)


if __name__ == "__main__":
    unittest.main()
