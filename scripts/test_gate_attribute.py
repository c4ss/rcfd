"""Tests of scripts/gate_attribute.py (ADR 0653 T1, T6). Run: python -m unittest scripts/test_gate_attribute.py -v
A fake times.tsv / test.log and a small git history in a temp folder; the re-run is replaced by a fake. The end-to-end test
(T6) builds a tiny crate with cargo --release and runs scripts/gate_test.py; it runs only with GATE_TEST_E2E=1."""
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import gate_attribute as ga  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
ENV = dict(os.environ, GIT_AUTHOR_NAME="t", GIT_AUTHOR_EMAIL="t@example.invalid", GIT_COMMITTER_NAME="t",
           GIT_COMMITTER_EMAIL="t@example.invalid", GIT_CONFIG_NOSYSTEM="1")

TSV = """binary\ttest\toutcome\tseconds\tpeak_mb
alpha_f010\tm::inner\tFAILED\t1.50\t10
alpha_f010\talpha_one\tok\t0.10\t10
rcfd_c\ttests::slow\tTIMEOUT\t600.00\t20
eps\tquick\tok\t0.01\t5
beta\tbeta_one\tignored\t0\t0
ghost\tgone\tFAILED\t0.1\t0
"""

LOG = """     Running tests/alpha_f010.rs (/w/target/release/deps/alpha_f010-0123abcd)

running 2 tests
test alpha_one ... ok
test m::inner ... FAILED
---- m::inner output ----
    test result: FAILED. 0 passed; 1 failed; 0 ignored
     Running unittests src/lib.rs (D:\\a\\rcfd\\rcfd\\target\\release\\deps\\rcfd_c-77aa.exe)
gate_test: 3 test processes, 2 binaries, jobs=3, wall 1.0s, peak summed working set 0 MB, max time/limit 0.100
test crates/rcfd-c/src/lib.rs - api::f (line 12) ... FAILED
test crates/rcfd-c/src/lib.rs - api::g (line 30) ... ok
"""

INDEX = {
    "binaries": {
        "alpha_f010": [{"crate": "rcfd-a", "target": "alpha_f010", "kind": "test", "file": "crates/rcfd-a/tests/alpha_f010.rs"}],
        "rcfd_c": [{"crate": "rcfd-c", "target": "rcfd_c", "kind": "lib", "file": "crates/rcfd-c/src/lib.rs"}],
    },
    "crates": {"rcfd-a": {"dir": "crates/rcfd-a", "deps": ["rcfd-b"]}, "rcfd-b": {"dir": "crates/rcfd-b", "deps": []},
               "rcfd-c": {"dir": "crates/rcfd-c", "deps": []}},
    "tests": [
        {"crate": "rcfd-a", "binary": "alpha_f010", "target": "alpha_f010", "kind": "test",
         "file": "crates/rcfd-a/tests/alpha_f010.rs", "name": "m::inner", "ignored": False, "adrs": ["0100"],
         "items": ["F-010"], "evidence": [{"kind": "E1", "item": "F-010"}]},
        {"crate": "rcfd-c", "binary": "rcfd_c", "target": "rcfd_c", "kind": "lib", "file": "crates/rcfd-c/src/lib.rs",
         "name": "tests::slow", "ignored": False, "adrs": [], "items": [], "evidence": []},
    ],
    "items": {"F-010": ["0100"], "F-020": ["0102"]},
}


def sh(repo, *args):
    subprocess.run(["git", *args], cwd=repo, env=ENV, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def write(repo, path, text):
    f = os.path.join(repo, *path.split("/"))
    os.makedirs(os.path.dirname(f), exist_ok=True)
    with open(f, "w", encoding="utf-8") as h:
        h.write(text)


def merge(repo, branch, path, text, subject):
    sh(repo, "checkout", "-q", "-b", branch)
    write(repo, path, text)
    sh(repo, "add", "-A")
    sh(repo, "commit", "-q", "-m", f"work on {branch}")
    sh(repo, "checkout", "-q", "main")
    sh(repo, "merge", "-q", "--no-ff", "-m", subject, branch)


def rev(repo, ref):
    return subprocess.run(["git", "rev-parse", ref], cwd=repo, stdout=subprocess.PIPE, check=True).stdout.decode().strip()


class Repo(unittest.TestCase):
    """History on main (first parent), oldest first:
    base  initial commit (state ok)
    m1    Merge F-010           touches crates/rcfd-a (direct for rcfd-a)
    m2    Merge F-020 (ADR 0102) touches crates/rcfd-b (a dependency of rcfd-a); sets state bad
    c3    roadmap: notes        touches docs only
    m3b   Merge F-025           touches crates/rcfd-b/tests only (not compiled into rcfd-a's tests)
    m4    Merge F-030           touches crates/rcfd-c
    m5    Merge F-040, F-041    touches crates/rcfd-a/tests/alpha_f010.rs (direct)
    """

    @classmethod
    def setUpClass(cls):
        cls.dir = tempfile.mkdtemp(prefix="gate_attr_")
        r = cls.repo = os.path.join(cls.dir, "repo")
        os.makedirs(r)
        sh(r, "init", "-q", "-b", "main")
        for c in ("a", "b", "c"):
            write(r, f"crates/rcfd-{c}/src/lib.rs", "// lib\n")
        write(r, "crates/rcfd-a/tests/alpha_f010.rs", "// t\n")
        write(r, "state.txt", "ok\n")
        sh(r, "add", "-A")
        sh(r, "commit", "-q", "-m", "initial")
        cls.base = rev(r, "HEAD")
        merge(r, "b1", "crates/rcfd-a/src/lib.rs", "// lib 1\n", "Merge F-010")
        cls.m1 = rev(r, "HEAD")
        sh(r, "checkout", "-q", "-b", "b2")
        write(r, "crates/rcfd-b/src/lib.rs", "// lib 2\n")
        write(r, "state.txt", "bad\n")
        sh(r, "add", "-A")
        sh(r, "commit", "-q", "-m", "work on b2")
        sh(r, "checkout", "-q", "main")
        sh(r, "merge", "-q", "--no-ff", "-m", "Merge F-020 (ADR 0102)", "b2")
        cls.m2 = rev(r, "HEAD")
        write(r, "docs/notes.md", "n\n")
        sh(r, "add", "-A")
        sh(r, "commit", "-q", "-m", "roadmap: notes")
        cls.c3 = rev(r, "HEAD")
        merge(r, "b3", "crates/rcfd-b/tests/only_b.rs", "// b test\n", "Merge F-025")  # a dependency's test: not a candidate
        cls.m3b = rev(r, "HEAD")
        merge(r, "b4", "crates/rcfd-c/src/lib.rs", "// lib 4\n", "Merge F-030")
        cls.m4 = rev(r, "HEAD")
        merge(r, "b5", "crates/rcfd-a/tests/alpha_f010.rs", "// t 5\n", "Merge F-040, F-041")
        cls.m5 = rev(r, "HEAD")
        cls.out = os.path.join(cls.dir, "out")
        os.makedirs(cls.out)
        for name, text in (("times.tsv", TSV), ("test.log", LOG), ("index.json", json.dumps(INDEX))):
            with open(os.path.join(cls.out, name), "w", encoding="utf-8") as h:
                h.write(text)

    @classmethod
    def tearDownClass(cls):
        shutil.rmtree(cls.dir, ignore_errors=True)

    def p(self, name):
        return os.path.join(self.out, name)

    def run_attr(self, *extra, base=None):
        js = self.p("attribution.json")
        md = self.p("summary.md")
        for f in (js, md):
            if os.path.exists(f):
                os.remove(f)
        argv = ["attribute", "--tsv", self.p("times.tsv"), "--log", self.p("test.log"), "--index", self.p("index.json"),
                "--repo", self.repo, "--base", self.base if base is None else base, "--json", js, "--summary", md,
                "--work", os.path.join(self.dir, "work"), *extra]
        self.assertEqual(ga.main(argv), 0)
        with open(js, encoding="utf-8") as h:
            res = json.load(h)
        with open(md, encoding="utf-8") as h:
            return res, h.read()

    def fail_of(self, res, binary):
        return next(f for f in res["failures"] if f["binary"] == binary)

    def test_read_failures(self):
        fails, exes, build, docs = ga.read_failures(self.p("times.tsv"), self.p("test.log"))
        self.assertEqual([(f["binary"], f["test"], f["outcome"]) for f in fails],
                         [("alpha_f010", "m::inner", "FAILED"), ("rcfd_c", "tests::slow", "TIMEOUT"), ("ghost", "gone", "FAILED")])
        self.assertEqual(exes["alpha_f010"][0]["exe"], "/w/target/release/deps/alpha_f010-0123abcd")
        self.assertEqual(exes["rcfd_c"][0]["src"], "src/lib.rs")
        self.assertIsNone(build)
        self.assertEqual(docs, ["crates/rcfd-c/src/lib.rs - api::f (line 12)"])
        self.assertEqual(ga.stem_of(r"D:\a\t\deps\rcfd_c-77aa.exe"), "rcfd_c")

    def test_build_failed_line(self):
        f = self.p("build.log")
        with open(f, "w") as h:
            h.write("gate_test: build failed (exit 101)\n")
        self.assertEqual(ga.read_failures(None, f)[2], "gate_test: build failed (exit 101)")

    def test_candidates_direct_and_dependency(self):
        res, md = self.run_attr()
        self.assertTrue(res["base_found"])
        self.assertEqual(res["commits"], [self.m1, self.m2, self.c3, self.m3b, self.m4, self.m5])
        a = self.fail_of(res, "alpha_f010")
        self.assertEqual([(c["sha"], c["relation"]) for c in a["candidates"]],
                         [(self.m1, "direct"), (self.m2, "dependency"), (self.m5, "direct")])
        self.assertEqual(a["candidates"][1]["items"], ["F-020"])
        self.assertEqual(a["candidates"][1]["adrs"], ["0102"])
        self.assertEqual(a["candidates"][2]["items"], ["F-040", "F-041"])
        self.assertTrue(a["candidates"][0]["merge"])
        self.assertEqual(a["adrs"], ["0100"])
        self.assertEqual(a["items"], ["F-010"])
        c = self.fail_of(res, "rcfd_c")
        self.assertEqual([x["sha"] for x in c["candidates"]], [self.m4])
        self.assertEqual(c["outcome"], "TIMEOUT")
        g = self.fail_of(res, "ghost")
        self.assertFalse(g["in_index"])
        self.assertIsNone(g["crate"])
        self.assertEqual(res["doc_failures"][0]["crate"], "rcfd-c")
        self.assertEqual([x["sha"] for x in res["doc_failures"][0]["candidates"]], [self.m4])
        self.assertIn("`rcfd-a/alpha_f010::m::inner` | FAILED | not run | 0100 | F-010 |", md)
        self.assertIn("unmapped", md)
        self.assertIn("doc-test failed", md)

    def test_base_unknown_falls_back(self):
        res, md = self.run_attr("--max-commits", "3", base="0" * 40)
        self.assertFalse(res["base_found"])
        self.assertEqual(res["commits"], [self.m3b, self.m4, self.m5])
        self.assertIn("not usable", md)
        res, _ = self.run_attr("--max-commits", "30", base="")
        self.assertEqual(res["commits"][0], self.base)

    def test_rerun_marks_flaky(self):
        calls = []

        def fake(exe, test, cwd, timeout):
            calls.append((exe, test, os.path.relpath(cwd, self.repo).replace("\\", "/")))
            return ("ok" if test == "m::inner" else "FAILED"), 0.5

        old = ga.run_test
        ga.run_test = fake
        try:
            res, md = self.run_attr("--rerun")
        finally:
            ga.run_test = old
        self.assertIn(("/w/target/release/deps/alpha_f010-0123abcd", "m::inner", "crates/rcfd-a"), calls)
        self.assertTrue(self.fail_of(res, "alpha_f010")["flaky"])
        self.assertFalse(self.fail_of(res, "rcfd_c")["flaky"])
        self.assertEqual(self.fail_of(res, "rcfd_c")["rerun"]["outcome"], "FAILED")
        self.assertTrue(self.fail_of(res, "ghost")["rerun"]["outcome"].startswith("not run"))
        self.assertIn("ok (flaky)", md)

    def test_bisect_finds_first_failing(self):
        for n in range(1, 9):
            for first in range(n):
                cands = [{"sha": f"s{i}"} for i in range(n)]
                seen = []

                def probe(sha):
                    seen.append(sha)
                    return int(sha[1:]) < first

                r = ga.bisect(cands, probe, 10.0)
                self.assertEqual(r["first_failing"], f"s{first}", (n, first))
                self.assertFalse(r["budget_exhausted"])
                self.assertNotIn(f"s{n - 1}", seen)  # head side is known to fail: never probed
                self.assertLessEqual(len(seen), max(0, (n - 1).bit_length()))

    def test_bisect_budget(self):
        cands = [{"sha": f"s{i}"} for i in range(6)]
        seen = []
        r = ga.bisect(cands, lambda s: seen.append(s) or True, 0.0)
        self.assertTrue(r["budget_exhausted"])
        self.assertIsNone(r["first_failing"])
        self.assertEqual(seen, [])
        self.assertEqual(r["range"], ["s0", "s5"])

    def test_bisect_in_worktrees_with_probe_cmd(self):
        cmd = f'"{sys.executable}" -c "import sys; sys.exit(open(\'state.txt\').read().strip() == \'bad\')"'
        res, md = self.run_attr("--bisect", "--bisect-minutes", "5", "--probe-cmd", cmd)
        a = self.fail_of(res, "alpha_f010")
        self.assertEqual(a["bisect"]["first_failing"], self.m2)
        self.assertIn(f"bisect: `{self.m2[:10]}`", md)
        # only one candidate: no bisect
        self.assertIsNone(self.fail_of(res, "rcfd_c")["bisect"])
        wt = subprocess.run(["git", "worktree", "list"], cwd=self.repo, stdout=subprocess.PIPE).stdout.decode()
        self.assertEqual(len(wt.strip().splitlines()), 1, wt)  # probe worktrees removed

    def test_issues_dry_run_and_dedupe(self):
        self.run_attr()

        class A:
            json = self.p("attribution.json")
            dry_run = True
            include_flaky = False

        self.assertEqual(ga.issues(A()), ["Gate failure: rcfd-a/alpha_f010::m::inner", "Gate failure: rcfd-c/rcfd_c::tests::slow",
                                          "Gate failure: ?/ghost::gone"])
        A.dry_run = False
        calls = []

        class R:
            def __init__(self, out):
                self.stdout = out

        def fake_run(cmd, **kw):
            calls.append(cmd)
            if cmd[:3] == ["gh", "issue", "list"]:
                return R(json.dumps([{"title": "Gate failure: rcfd-c/rcfd_c::tests::slow"}]).encode())
            return R(b"")

        made = ga.issues(A(), run=fake_run)
        self.assertEqual(made, ["Gate failure: rcfd-a/alpha_f010::m::inner", "Gate failure: ?/ghost::gone"])
        self.assertEqual(sum(1 for c in calls if c[:3] == ["gh", "issue", "create"]), 2)


E2E_LIB = r"""
#[test]
fn passes() {}

#[test]
fn fails() { panic!("always"); }

#[test]
fn flaky() {
    let marker = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("flaky.marker");
    if !marker.exists() {
        std::fs::write(&marker, "seen").unwrap();
        panic!("first run fails");
    }
}
"""


@unittest.skipUnless(os.environ.get("GATE_TEST_E2E") == "1", "set GATE_TEST_E2E=1 (builds a crate with cargo --release)")
class EndToEnd(unittest.TestCase):
    def test_t6_gate_then_attribution_with_rerun(self):
        d = tempfile.mkdtemp(prefix="gate_attr_e2e_")
        write(d, "Cargo.toml", '[package]\nname = "rcfd-e2e"\nversion = "0.1.0"\nedition = "2021"\n\n[workspace]\n')
        write(d, "src/lib.rs", E2E_LIB)
        sh(d, "init", "-q", "-b", "main")
        sh(d, "add", "-A")
        sh(d, "commit", "-q", "-m", "initial")
        env = dict(os.environ, CARGO_TARGET_DIR=os.path.join(d, "target"))
        out = os.path.join(d, "gate-out")
        os.makedirs(out)
        with open(os.path.join(out, "test.log"), "wb") as log:
            r = subprocess.run([sys.executable, os.path.join(HERE, "gate_test.py"), "--tsv", os.path.join(out, "times.tsv")],
                               cwd=d, env=env, stdout=log, stderr=subprocess.DEVNULL)
        self.assertNotEqual(r.returncode, 0)
        subprocess.run([sys.executable, os.path.join(HERE, "adr_test_index.py"), "--root", d, "--out",
                        os.path.join(out, "index.json")], cwd=d, env=env, check=True)
        r = subprocess.run([sys.executable, os.path.join(HERE, "gate_attribute.py"), "attribute", "--tsv",
                            os.path.join(out, "times.tsv"), "--log", os.path.join(out, "test.log"), "--index",
                            os.path.join(out, "index.json"), "--repo", d, "--rerun", "--json",
                            os.path.join(out, "attribution.json"), "--summary", os.path.join(out, "summary.md")],
                           cwd=d, env=env)
        self.assertEqual(r.returncode, 0)
        with open(os.path.join(out, "attribution.json"), encoding="utf-8") as h:
            res = json.load(h)
        got = {f["test"]: (f["outcome"], f["rerun"]["outcome"], f["flaky"]) for f in res["failures"]}
        print("\nT6:", got)
        self.assertEqual(got, {"fails": ("FAILED", "FAILED", False), "flaky": ("FAILED", "ok", True)})
        self.assertTrue(os.path.getsize(os.path.join(out, "summary.md")) > 0)
        shutil.rmtree(d, ignore_errors=True)


if __name__ == "__main__":
    unittest.main()
