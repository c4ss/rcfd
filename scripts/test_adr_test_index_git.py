"""Tests of the primary ADR from the first git commit that adds the cited file (ADR 0690: T1 to T12).
Run: python -m unittest scripts/test_adr_test_index_git.py -v
A small fake workspace is made a real git repository in a temp folder; the metadata is given, so cargo is not run."""
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
from test_adr_test_index_primary import meta  # noqa: E402

HAVE_GIT = shutil.which("git") is not None
T = "#[test]\nfn {}() {{}}\n"
TESTS = "crates/rcfd-w/tests/"
ADRS = {
    "docs/adr/0105-f020-registration-item.md": "# ADR 0105\nSee t_item.rs and t_both.rs and hdr.rs.\n",
    "docs/adr/0120-first.md": "# ADR 0120\nSee t_init.rs, t_one.rs, t_both.rs, t_readd.rs and t_ren.rs.\n",
    "docs/adr/0130-later.md": ("# ADR 0130\nSee t_init.rs, t_one.rs, t_item.rs, t_noreg.rs, t_notcited.rs, t_both.rs, t_ren.rs, "
                                "hdr.rs, t_readd.rs and g_e3_distinctive_name.\n"),
    "docs/adr/0140-other.md": "# ADR 0140\nNothing cited here.\n",
    "docs/adr/0170-f021-notes.md": "# ADR 0170\nNot a registration of F-021 (no such file name).\n",
}
EXPECT = {  # test name -> primary under rule G on the full history
    "g_init": None,          # T5: the first commit names nothing
    "g_one": "0120",         # T1, T3: ADR 0120 adds it, ADR 0130 touches it later
    "g_item": "0105",        # T2: F-020 has the registration ADR 0105
    "g_noreg": None,         # T2: F-021 has no registration ADR (ADR 0170 is `-f021-notes`, which is its registration: see test)
    "g_notcited": None,      # T6: the commit names ADR 0140, which does not cite the file
    "g_both": "0120",        # T7: ADR before item
    "g_ren": "0120",         # T4: renamed by a commit that names ADR 0130
    "g_hdr": "0105",         # T8: E0 is not changed
    "g_readd": "0120",       # T13
}


def run(cwd, *args, date="2026-01-01T00:00:00", check=True):
    env = dict(os.environ, GIT_AUTHOR_NAME="t", GIT_AUTHOR_EMAIL="t@example.org", GIT_COMMITTER_NAME="t",
               GIT_COMMITTER_EMAIL="t@example.org", GIT_AUTHOR_DATE=date, GIT_COMMITTER_DATE=date)
    return subprocess.run(["git", "-c", "commit.gpgsign=false", "-c", "core.autocrlf=false", *args], cwd=cwd, env=env, check=check,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE)


def put(base, path, text):
    f = os.path.join(base, *path.split("/"))
    os.makedirs(os.path.dirname(f), exist_ok=True)
    with open(f, "w", encoding="utf-8") as h:
        h.write(text)


def make_repo(repo, sub=""):
    """The workspace is repo/sub; commits in a fixed order with rising dates."""
    base = os.path.join(repo, sub) if sub else repo
    os.makedirs(base, exist_ok=True)
    run(repo, "init", "-q")
    n = [0]

    def commit(msg, files=(), rename=None, body=None):
        n[0] += 1
        for p, text in files:
            put(base, p, text)
        if rename:
            run(repo, "mv", os.path.join(sub, rename[0]), os.path.join(sub, rename[1]))
        run(repo, "add", "-A")
        run(repo, "commit", "-q", "-m", msg, *(["-m", body] if body else []), date=f"2026-01-{n[0]:02d}T00:00:00")

    put(base, "crates/rcfd-w/src/lib.rs", "")
    docs = list(ADRS.items())
    commit("Initial import", docs + [("crates/rcfd-w/src/lib.rs", ""), (TESTS + "t_init.rs", T.format("g_init"))],
           body="The body names ADR 0120, which is not read: only the subject is.")
    commit("w: one (ADR 0120)", [(TESTS + "t_one.rs", T.format("g_one"))])
    commit("w: item (F-020)", [(TESTS + "t_item.rs", T.format("g_item"))])
    commit("w: noreg (F-021)", [(TESTS + "t_noreg.rs", T.format("g_noreg"))])
    commit("w: not cited (ADR 0140)", [(TESTS + "t_notcited.rs", T.format("g_notcited"))])
    commit("w: both (ADR 0120, F-020)", [(TESTS + "t_both.rs", T.format("g_both"))])
    commit("w: old name (ADR 0120)", [(TESTS + "t_old.rs", T.format("g_ren"))])
    commit("ADR 0130 touches t_one", [(TESTS + "t_one.rs", T.format("g_one") + "// touched\n")])
    commit("ADR 0130 renames t_old to t_ren", rename=(TESTS + "t_old.rs", TESTS + "t_ren.rs"))
    commit("ADR 0130 adds hdr", [(TESTS + "hdr.rs", "//! F-020 (ADR 0105)\n" + T.format("g_hdr"))])
    commit("w: readd (ADR 0120)", [(TESTS + "t_readd.rs", T.format("g_readd"))])
    os.remove(os.path.join(base, *(TESTS + "t_readd.rs").split("/")))
    commit("ADR 0130 deletes t_readd (file)")
    commit("ADR 0130 re-adds t_readd", [(TESTS + "t_readd.rs", T.format("g_readd"))])
    commit("ADR 0130 adds e3", [(TESTS + "t_e3.rs", T.format("g_e3_distinctive_name"))])
    return base


def tests_of(idx):
    return {t["name"]: t for t in idx["tests"]}


@unittest.skipUnless(HAVE_GIT, "git is not installed")
class GitPrimary(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.mkdtemp(prefix="adr_git_")
        cls.root = os.path.join(cls.tmp, "full")
        make_repo(cls.root)
        cls.meta = meta(cls.root)
        cls.idx = ati.build(cls.root, cls.meta, "git")
        cls.t = tests_of(cls.idx)
        cls.reg = tests_of(ati.build(cls.root, cls.meta, "registered"))

    @classmethod
    def tearDownClass(cls):
        def force(fn, p, _):
            os.chmod(p, 0o700)
            fn(p)
        shutil.rmtree(cls.tmp, onerror=force)

    def test_t1_adr_in_the_subject_of_the_adding_commit(self):
        t = self.t["g_one"]
        self.assertEqual(sorted(t["adrs"]), ["0120", "0130"])
        self.assertEqual((t["primary_adr"], t["primary_evidence"]), ("0120", "E2"))
        self.assertEqual(self.reg["g_one"]["primary_adr"], "0120")

    def test_t2_item_in_the_subject_takes_its_registration_adr(self):
        t = self.t["g_item"]
        self.assertEqual((t["primary_adr"], t["primary_evidence"]), ("0105", "E2"))
        self.assertEqual(self.reg["g_item"]["primary_adr"], "0105")  # E2 earliest is 0105 as well: the next test separates them

    def test_t2_item_without_a_registration_adr_gives_none(self):
        # ADR 0170 is named -f021-notes, so it is F-021's ADR by file name; it does not cite the file, so there is no primary
        t = self.t["g_noreg"]
        self.assertIsNone(t["primary_adr"])
        self.assertEqual(t["adrs"], ["0130"])
        self.assertEqual(ati.registration_adrs({"0150": {"files": ["docs/adr/0150-f9-registration-x.md"], "items": ["F-009"]},
                                                "0140": {"files": ["docs/adr/0140-f9-notes.md"], "items": ["F-009"]}}),
                         {"F-009": "0150"})
        self.assertIsNone(ati.commit_adr("w: x (F-077)", {"0140": {}}, {"F-009": "0140"}))

    def test_t3_later_commits_do_not_move_the_primary(self):
        self.assertEqual(self.t["g_one"]["primary_adr"], "0120")  # ADR 0130 touched the file in a later commit

    def test_t13_a_file_deleted_and_added_again_keeps_its_first_commit(self):
        t = self.t["g_readd"]
        self.assertEqual(sorted(t["adrs"]), ["0120", "0130"])
        self.assertEqual(t["primary_adr"], "0120")

    def test_t4_rename_chain_keeps_the_first_commit(self):
        t = self.t["g_ren"]
        self.assertEqual(sorted(t["adrs"]), ["0120", "0130"])
        self.assertEqual(t["primary_adr"], "0120")
        self.assertEqual(self.reg["g_ren"]["primary_adr"], "0120")

    def test_t5_first_commit_names_nothing(self):
        t = self.t["g_init"]
        self.assertEqual(sorted(t["adrs"]), ["0120", "0130"])
        self.assertIsNone(t["primary_adr"])
        self.assertEqual(self.reg["g_init"]["primary_adr"], "0120")

    def test_t6_adr_not_citing_the_file_is_no_primary_and_adrs_are_unchanged(self):
        t = self.t["g_notcited"]
        self.assertIsNone(t["primary_adr"])
        self.assertEqual(t["adrs"], ["0130"])
        g = self.idx["coverage"]["primary"]["git"]
        self.assertEqual(g["history"], "full")
        self.assertEqual(g["not_in_adrs"], 2)  # notcited (ADR 0140) and noreg (F-021 -> ADR 0170)
        self.assertEqual(g["from_git"], 5)  # one, item, both, ren, readd
        self.assertEqual(g["no_commit_rule"], 1)  # init
        self.assertEqual(g["e2_tests"], 8)

    def test_t7_adr_before_item(self):
        t = self.t["g_both"]
        self.assertEqual(sorted(t["adrs"]), ["0105", "0120", "0130"])
        self.assertEqual(t["primary_adr"], "0120")

    def test_t8_other_evidence_is_not_changed(self):
        t = self.t["g_hdr"]
        self.assertEqual((t["primary_adr"], t["primary_evidence"]), ("0105", "E0"))
        e3 = self.t["g_e3_distinctive_name"]
        self.assertIsNone(e3["primary_adr"])
        for name, a in self.reg.items():
            b = self.t[name]
            if a["primary_evidence"] != "E2":
                self.assertEqual((a["primary_adr"], a["primary_evidence"]), (b["primary_adr"], b["primary_evidence"]), name)
            for k in a:
                if k not in ("primary_adr", "primary_evidence"):
                    self.assertEqual(a[k], b[k], (name, k))

    def test_t6b_counts_add_up(self):
        g = self.idx["coverage"]["primary"]["git"]
        self.assertEqual(g["e2_tests"], g["from_git"] + g["no_commit_rule"] + g["not_in_adrs"] + g["no_origin"])
        self.assertEqual(self.idx["coverage"]["primary"]["with_primary"], sum(self.idx["coverage"]["primary"]["by_evidence"].values()))
        self.assertEqual(self.idx["adrs"]["0120"]["primary_tests"],
                         sorted(f"rcfd-w/{b}::{n}" for b, n in (("t_one", "g_one"), ("t_both", "g_both"), ("t_ren", "g_ren"), ("t_readd", "g_readd"))))

    def test_t9_shallow_clone_and_copy_without_git_give_no_primary(self):
        clone = os.path.join(self.tmp, "shallow")
        run(self.tmp, "clone", "-q", "--depth", "1", pathlib.Path(self.root).as_uri(), clone)
        self.assertEqual(run(clone, "rev-parse", "--is-shallow-repository").stdout.strip(), b"true")
        idx = ati.build(clone, meta(clone), "git")
        self.assertEqual(idx["coverage"]["primary"]["git"]["history"], "shallow")
        self.assertEqual(idx["coverage"]["primary"]["git"]["git_calls"], 1)
        for name, t in tests_of(idx).items():
            if self.reg[name]["primary_evidence"] == "E2":
                self.assertIsNone(t["primary_adr"], name)
            else:
                self.assertEqual(t["primary_adr"], self.reg[name]["primary_adr"], name)
        plain = os.path.join(self.tmp, "plain")
        shutil.copytree(self.root, plain, ignore=shutil.ignore_patterns(".git"))
        idx = ati.build(plain, meta(plain), "git")
        self.assertEqual(idx["coverage"]["primary"]["git"]["history"], "none")
        self.assertIsNone(tests_of(idx)["g_one"]["primary_adr"])
        self.assertEqual(tests_of(idx)["g_hdr"]["primary_adr"], "0105")

    def test_t10_enclosing_repository_is_not_read(self):
        outer = os.path.join(self.tmp, "outer")
        base = make_repo(outer, "ws")
        idx = ati.build(base, meta(base), "git")
        self.assertEqual(idx["coverage"]["primary"]["git"]["history"], "none")
        self.assertIsNone(tests_of(idx)["g_one"]["primary_adr"])

    def test_t11_at_most_two_git_processes(self):
        real = subprocess.run
        calls = []

        def counting(cmd, *a, **k):
            calls.append(cmd)
            return real(cmd, *a, **k)
        with mock.patch.object(ati.subprocess, "run", counting):
            idx = ati.build(self.root, self.meta, "git")
        self.assertEqual(len(calls), 2)
        self.assertTrue(all(c[0] == "git" for c in calls))
        self.assertEqual(idx["coverage"]["primary"]["git"]["git_calls"], 2)
        calls.clear()
        with mock.patch.object(ati.subprocess, "run", counting):
            ati.build(self.root, self.meta, "registered")
            ati.build(self.root, self.meta)
        self.assertEqual(calls, [])  # the other rules run no git

    def test_t12_command_line_default_is_the_git_rule(self):
        mp = os.path.join(self.tmp, "meta.json")
        with open(mp, "w") as fh:
            json.dump(self.meta, fh)
        for flag, want in ((None, None), ("git", None), ("registered", "0120")):
            out = os.path.join(self.tmp, f"out_{flag}.json")
            args = ["--root", self.root, "--metadata", mp, "--out", out] + (["--primary", flag] if flag else [])
            self.assertEqual(ati.main(args), 0)
            with open(out) as fh:
                idx = json.load(fh)
            self.assertEqual(tests_of(idx)["g_init"]["primary_adr"], want, flag)


if __name__ == "__main__":
    unittest.main()
