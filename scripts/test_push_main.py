"""Self-test of scripts/push_main.py: the pure parts and the count of merges since the newest green gate line.

    python -m unittest scripts/test_push_main.py
"""
import os
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import push_main as pm  # noqa: E402

CRATES = {"a": {"dir": "crates/a", "deps": []}, "a-x": {"dir": "crates/a/x", "deps": ["a"]},
          "b": {"dir": "crates/b", "deps": ["a"]}, "c": {"dir": "crates/c", "deps": ["b"]}, "d": {"dir": "crates/d", "deps": []}}


def t(crate, target, name, ignored=False, kind="test"):
    return {"crate": crate, "target": target, "binary": target, "name": name, "ignored": ignored, "kind": kind, "adrs": [],
            "file": f"crates/{crate}/tests/{target}.rs"}


IDX = {"crates": CRATES, "tests": [t("a", "fast", "one"), t("a", "fast", "two"), t("a", "slow", "one"), t("a", "norec", "one"),
                                   t("a", "heavy", "one", ignored=True), t("b", "bt", "one"), t("c", "ct", "one"),
                                   t("d", "dt", "one"), t("a", "a", "unit", kind="lib")]}
TIMES = {("fast", "one"): 5.0, ("fast", "two"): 6.0, ("slow", "one"): 500.0, ("bt", "one"): 30.0, ("ct", "one"): 20.0}


class Subject(unittest.TestCase):
    def test_branch_items_adrs(self):
        self.assertEqual(pm.parse_subject("Merge branch 'f529-work': F-529 partial (ADR 0503), F-431 note"),
                         ("f529-work", ["F-431", "F-529"], ["0503"]))
        self.assertEqual(pm.parse_subject("Merge commit '058ef506'"), ("", [], []))

    def test_adr_files(self):
        self.assertEqual(pm.adrs_in_files(["docs/adr/0735-f828-f814-proposal-x.md", "docs/adr/briefs/f1-work.md",
                                           "docs/adr/0800-electron-studio-roadmap.md", "crates/a/src/lib.rs"]),
                         (["0735", "0800"], ["F-814", "F-828"]))


class Selection(unittest.TestCase):
    def test_touched_takes_longest_prefix(self):
        self.assertEqual(pm.touched_crates(["crates/a/x/src/lib.rs", "docs/index.md", "crates/d/Cargo.toml"], CRATES), ["a-x", "d"])

    def test_dependents_are_transitive(self):
        self.assertEqual(pm.dependents(CRATES, ["a"]), ["a-x", "b", "c"])
        self.assertEqual(pm.dependents(CRATES, ["d"]), [])

    def test_units_sum_non_ignored_tests_of_integration_binaries(self):
        u = {x["target"]: x for x in pm.binary_units(IDX, {"a"}, TIMES, 60)}
        self.assertEqual(sorted(u), ["fast", "norec", "slow"])
        self.assertEqual((u["fast"]["seconds"], u["norec"]["seconds"], u["norec"]["cost"]), (11.0, None, 60.0))

    def test_order_failed_first_then_shortest(self):
        g = pm.order_group(pm.binary_units(IDX, {"a"}, TIMES, 60), {"slow"})
        self.assertEqual([x["target"] for x in g], ["slow", "fast", "norec"])

    def test_cut_stops_at_first_misfit_and_keeps_group_order(self):
        g1 = pm.order_group(pm.binary_units(IDX, {"a"}, TIMES, 60), set())
        g2 = pm.order_group(pm.binary_units(IDX, {"b", "c"}, TIMES, 60), set())
        taken, left, used = pm.cut([g1, g2], 100.0)
        self.assertEqual([x["target"] for x in taken], ["fast", "norec"])
        self.assertEqual([x["target"] for x in left], ["slow", "ct", "bt"])  # nothing after the first misfit is taken
        self.assertEqual(used, 71.0)
        taken, left, used = pm.cut([g1, g2], 1800.0, {"cargo test -p a --release --test fast"})
        self.assertEqual([x["target"] for x in taken], ["norec", "slow", "ct", "bt"])
        self.assertEqual((left, used), ([], 610.0))


class Ledger(unittest.TestCase):
    def test_append_read_and_recent_failures(self):
        with tempfile.TemporaryDirectory() as d:
            p = os.path.join(d, "docs", "process", "ledger.tsv")
            self.assertEqual(pm.read_ledger(p), [])
            pm.append_ledger(p, pm.ledger_line(date="2026-11-12", kind="gate", commit="49fbf1a3", result="green",
                                               detail=pm.gate_detail(3548, 0, 1023)))
            att = {"failures": [{"binary": "slow", "test": "one", "primary_adr": "0503", "adrs": ["0503"], "items": ["F-529"]},
                                {"binary": "bt", "test": "x", "primary_adr": None, "adrs": [], "items": []}], "doc_failures": []}
            pm.append_ledger(p, pm.ledger_line(date="2026-11-19", kind="gate", commit="abc", result="red",
                                               detail=pm.gate_detail(10, 2, 3, att)))
            pm.append_ledger(p, pm.ledger_line(date="2026-11-19", kind="merge", commit="def", branch="x\ty", result="pass"))
            rows = pm.read_ledger(p)
            self.assertEqual(open(p, encoding="utf-8").readline().rstrip("\n").split("\t"), pm.COLUMNS)
            self.assertEqual([r["kind"] for r in rows], ["gate", "gate", "merge"])
            self.assertEqual(rows[1]["detail"], "passed=10;failed=2;ignored=3;fails=slow::one=0503/F-529,bt::x=none/none")
            self.assertEqual(rows[2]["branch"], "x y")
            self.assertEqual(pm.recent_failed_binaries(rows), {"slow", "bt"})
            self.assertEqual(pm.newest_green(rows), "49fbf1a3")
            self.assertEqual(pm.newest_green(rows[1:]), pm.LAST_GREEN)

    def test_unknown_column_is_an_error(self):
        with self.assertRaises(ValueError):
            pm.ledger_line(when="today")

    def test_committed_ledger_has_the_header_and_the_seed_line(self):
        rows = pm.read_ledger(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", pm.LEDGER))
        self.assertTrue(rows and rows[0]["kind"] == "gate" and rows[0]["commit"].startswith("49fbf1a3"))
        self.assertTrue(all(r["kind"] in ("merge", "gate", "leftout", "mutants", "ci") for r in rows))


class Mutation(unittest.TestCase):
    def test_header(self):
        self.assertEqual(pm.expect_fail("# expect-fail: cargo test -p a --release --test fast -- one \ndiff --git a/x b/x\n"),
                         "cargo test -p a --release --test fast -- one")
        self.assertIsNone(pm.expect_fail("diff --git a/x b/x\n"))
        self.assertIsNone(pm.expect_fail(""))

    def test_paths(self):
        got = pm.patch_paths(["crates/a/tests/new.mutation.patch", "crates/a/src/lib.rs"],
                             ["crates/a/tests/fast.rs", "crates/a/tests/slow.rs"], lambda p: p.endswith("fast.mutation.patch"))
        self.assertEqual(got, ["crates/a/tests/fast.mutation.patch", "crates/a/tests/new.mutation.patch"])

    def test_verify_requires_failure_and_always_reverts(self):
        with tempfile.TemporaryDirectory() as d:
            open(os.path.join(d, "m.mutation.patch"), "w").write("# expect-fail: the-test\n")
            for code, ok in ((1, True), (0, False)):
                calls = []
                got, msg = pm.verify_patch(d, "m.mutation.patch", run=lambda cmd, cwd: (code, 0.0, ""),
                                           apply=lambda *a: calls.append(a) or 0)
                self.assertEqual((got, calls), (ok, [(), ("-R",)]), msg)
            calls = []

            def boom(cmd, cwd):
                raise RuntimeError("x")
            with self.assertRaises(RuntimeError):
                pm.verify_patch(d, "m.mutation.patch", run=boom, apply=lambda *a: calls.append(a) or 0)
            self.assertEqual(calls, [(), ("-R",)])
            open(os.path.join(d, "bad.mutation.patch"), "w").write("diff\n")
            self.assertFalse(pm.verify_patch(d, "bad.mutation.patch")[0])

    def test_skipped_count(self):
        self.assertEqual(pm.skipped_count("  3 skipped\n  40 passed"), 3)
        self.assertIsNone(pm.skipped_count("40 passed"))


class Mutants(unittest.TestCase):
    def test_command_names_the_diff_the_packages_and_the_timeout(self):
        self.assertEqual(pm.mutants_command(["rcfd-fv", "rcfd-api"], "d.diff", "out", 120.0),
                         ["cargo", "mutants", "--in-diff", "d.diff", "--output", "out", "--package", "rcfd-fv", "--package", "rcfd-api",
                          "--timeout", "120"])
        self.assertEqual(pm.mutants_command(["a"], "d", "o"), ["cargo", "mutants", "--in-diff", "d", "--output", "o", "--package", "a"])

    def test_counts_from_the_top_level_or_from_the_summaries(self):
        top = {"caught": 7, "missed": 2, "timeout": 1, "unviable": 3, "outcomes": []}
        self.assertEqual(pm.mutants_counts(top, "a.rs:1: replace x\n\nb.rs:2: replace y\n"),
                         ({"caught": 7, "missed": 2, "timeout": 1, "unviable": 3}, ["a.rs:1: replace x", "b.rs:2: replace y"]))
        listed = {"outcomes": [{"summary": s} for s in ("Success", "CaughtMutant", "CaughtMutant", "MissedMutant", "Unviable")]}
        self.assertEqual(pm.mutants_counts(listed)[0], {"caught": 2, "missed": 1, "timeout": 0, "unviable": 1})

    def test_detail_lists_the_first_missed_mutants(self):
        c = {"caught": 1, "missed": 7, "timeout": 0, "unviable": 0}
        d = pm.mutants_detail(c, [f"m{i}" for i in range(7)])
        self.assertEqual(d, "caught=1;missed=7;timeout=0;unviable=0;missed_list=m0 | m1 | m2 | m3 | m4 | and 2 more")
        self.assertEqual(pm.mutants_detail({"caught": 2, "missed": 0, "timeout": 0, "unviable": 0}, []),
                         "caught=2;missed=0;timeout=0;unviable=0")

    def test_gate_detail_carries_the_skipped_count(self):
        self.assertEqual(pm.gate_detail(10, 0, 3, skipped=4), "passed=10;failed=0;ignored=3;skipped=4")
        self.assertEqual(pm.gate_detail(10, 0, 3), "passed=10;failed=0;ignored=3")


class Git(unittest.TestCase):
    def test_merge_info_and_since_verified(self):
        with tempfile.TemporaryDirectory() as d:
            def g(*a):
                return subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@example.invalid", "-c", "commit.gpgsign=false",
                                       *a], cwd=d, check=True, stdout=subprocess.PIPE, text=True).stdout.strip()

            def commit(name, msg):
                os.makedirs(os.path.dirname(os.path.join(d, name)), exist_ok=True)
                open(os.path.join(d, name), "a").write("x\n")
                g("add", "-A")
                g("commit", "-q", "-m", msg)
            g("init", "-q", "-b", "main")
            commit("README", "first")
            green = g("rev-parse", "HEAD")
            for k, br in enumerate(("f001-work", "f002-work")):
                g("checkout", "-q", "-b", br)
                commit(f"docs/adr/090{k}-f00{k + 1}-registration-x.md", "registration")
                commit(f"crates/a/src/{br}.rs", "code")
                g("checkout", "-q", "main")
                g("merge", "-q", "--no-ff", br, "-m", f"Merge branch '{br}': F-00{k + 1} done")
            info = pm.merge_info(d, "HEAD")
            self.assertEqual((info["branch"], info["items"], info["adrs"]), ("f002-work", ["F-002"], ["0901"]))
            self.assertEqual(sorted(info["files"]), ["crates/a/src/f002-work.rs", "docs/adr/0901-f002-registration-x.md"])
            with self.assertRaises(SystemExit):
                pm.merge_info(d, "HEAD^2")
            rows = [{"kind": "gate", "result": "green", "commit": green, "detail": ""}]
            self.assertEqual(pm.since_verified(d, rows), (green, 2))
            rows.append({"kind": "gate", "result": "green", "commit": g("rev-parse", "HEAD~1"), "detail": ""})
            self.assertEqual(pm.since_verified(d, rows)[1], 1)


# ---------------------------------------------------------------- ADR 0752 (F-851): the script after its first real runs
#
# Everything below runs in temporary repositories or on the script's functions. No cargo, rustfmt or npm is started and
# nothing is pushed: `do_run` is given a recorder in place of the function that runs a command.

import argparse  # noqa: E402
import contextlib  # noqa: E402
import io  # noqa: E402
import time  # noqa: E402
import json  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
WRITE, CHECK = "python scripts/process_report.py --write", "python scripts/process_report.py --check"
LISTED = "python scripts/process_report.py --list-generated"
SLOW, FAST = "cargo test -p a --release --test slow", "cargo test -p a --release --test fast"
CARGO_FAILED = """
running 2 tests
test one ... FAILED
test two ... ok

failures:

---- one stdout ----
thread 'one' panicked at crates/a/tests/slow.rs:3:5:
boom

failures:
    one

test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

error: test failed, to rerun pass `-p a --test slow`
"""


class Runner:
    """Stands in for push_main.sh: records each command and answers from `script` (command -> a list of (exit code, output),
    one per run; an exhausted or absent list means the command passes). Each run counts half a minute."""

    def __init__(self, script=None, listed=""):
        self.script, self.listed, self.calls = {k: list(v) for k, v in (script or {}).items()}, listed, []

    def __call__(self, cmd, cwd, capture=False):
        self.calls.append(cmd)
        if cmd == LISTED:
            return 0, 0.0, self.listed
        code, out = self.script[cmd].pop(0) if self.script.get(cmd) else (0, "")
        return code, 0.5, out

    def checks(self):
        """The commands of the plan that were run: without the regeneration and the listing of the generated files."""
        return [c for c in self.calls if c not in pm.REGENERATE and c != LISTED]


class TempRepo(unittest.TestCase):
    """A repository with one commit on main; `merge` makes a branch with files and merges it without fast-forward."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.d = self.tmp.name
        self.g("init", "-q", "-b", "main")
        for k, v in (("user.name", "t"), ("user.email", "t@example.invalid"), ("commit.gpgsign", "false"), ("core.autocrlf", "false")):
            self.g("config", k, v)
        self.put(".gitignore", "/target\n")
        self.commit("README", "first")
        self.base = self.g("rev-parse", "HEAD")

    def tearDown(self):
        self.tmp.cleanup()

    def g(self, *a):
        return subprocess.run(["git", *a], cwd=self.d, check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True).stdout.strip()

    def put(self, name, text="x\n", mode="w"):
        p = os.path.join(self.d, name)
        os.makedirs(os.path.dirname(p), exist_ok=True)
        with open(p, mode, encoding="utf-8", newline="\n") as fh:
            fh.write(text)

    def commit(self, name, msg, text="x\n"):
        self.put(name, text, "a")
        self.g("add", "-A")
        self.g("commit", "-q", "-m", msg)

    def merge(self, branch, files, subject=None):
        self.g("checkout", "-q", "-b", branch)
        for f in files:
            self.commit(f, "work on " + f)
        self.g("checkout", "-q", "main")
        self.g("merge", "-q", "--no-ff", branch, "-m", subject or f"Merge branch '{branch}'")
        return self.g("rev-parse", "HEAD")

    def plan(self, *cmds, **more):
        p = {"touched": [], "always": list(cmds), "selected": [], "left": [], "selected_minutes": 0.0, "left_minutes": 0.0,
             "patches": [], "studio": False}
        p.update(more)
        return p

    def run_plan(self, plan, runner, base=None, **flags):
        """do_run on the merge at HEAD: (exit code or the SystemExit text, what was printed)."""
        a = argparse.Namespace(push=False, left_out_run=False, fresh=False)
        vars(a).update(flags)
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            try:
                code = pm.do_run(self.d, a, pm.merge_info(self.d, "HEAD", base), plan, run=runner)
            except SystemExit as e:
                code = str(e)
        return code, buf.getvalue()

    def rows(self):
        return pm.read_ledger(os.path.join(self.d, pm.LEDGER))

    def state_files(self):
        d = os.path.join(self.d, "target", "push_main")
        return sorted(os.listdir(d)) if os.path.isdir(d) else []


class Retries(TempRepo):
    """Point 1: a failed check is run RETRIES further times; failed and then passed is flaky; failed every time stops."""

    def test_the_registered_number_of_further_runs_is_two(self):
        self.assertEqual(pm.RETRIES, 2)

    def test_failed_names_are_read_from_cargos_output(self):
        self.assertEqual(pm.failed_names(SLOW, CARGO_FAILED), ["slow::one"])
        self.assertEqual(pm.failed_names("cargo test -p rcfd-fv --release --lib -- --exact m::tests::x",
                                         "test m::tests::x ... FAILED\n"), ["rcfd_fv::m::tests::x"])
        self.assertEqual(pm.failed_names(SLOW, "error[E0308]: mismatched types\n"), ["slow::*"])  # no test ran
        self.assertEqual(pm.failed_names(CHECK, "FAIL r: x\n"), [CHECK])
        two = "test a ... FAILED\ntest b ... FAILED\n\nfailures:\n    a\n    b\n\ntest result: FAILED.\n"
        self.assertEqual(pm.failed_names(FAST, two), ["fast::a", "fast::b"])

    def test_a_command_that_fails_once_and_then_passes_is_run_twice_and_names_the_test(self):
        r = Runner({SLOW: [(101, CARGO_FAILED)]})
        with contextlib.redirect_stdout(io.StringIO()):
            got = pm.run_with_retries(SLOW, ".", r)
        self.assertEqual((got["ok"], got["runs"], got["failed"], got["minutes"]), (True, 2, {"slow::one": 1}, 1.0))
        self.assertEqual(r.calls, [SLOW, SLOW])

    def test_a_command_that_passes_is_run_once(self):
        r = Runner()
        got = pm.run_with_retries(SLOW, ".", r)
        self.assertEqual((got["ok"], got["runs"], got["failed"], r.calls), (True, 1, {}, [SLOW]))

    def test_a_command_that_fails_every_time_is_run_three_times_and_fails(self):
        r = Runner({SLOW: [(101, CARGO_FAILED)] * 5})
        with contextlib.redirect_stdout(io.StringIO()):
            got = pm.run_with_retries(SLOW, ".", r)
        self.assertEqual((got["ok"], got["runs"], got["code"], list(got["failed"])), (False, 3, 101, ["slow::one"]))
        self.assertEqual(r.calls, [SLOW] * 3)

    def test_a_flaky_test_is_named_in_the_ledger_line_and_in_the_output(self):
        self.merge("f001-work", ["crates/a/src/one.rs"])
        r = Runner({SLOW: [(101, CARGO_FAILED), (101, CARGO_FAILED)]})
        code, out = self.run_plan(self.plan(CHECK, SLOW, FAST), r)
        self.assertEqual(code, 0, out)
        self.assertEqual(r.checks(), [CHECK, SLOW, SLOW, SLOW, FAST])  # the run goes on after the flaky command
        row = self.rows()[-1]
        self.assertEqual(row["detail"], f"exit=0;mutations=0;flaky=slow::one;cmds={CHECK} | {SLOW} | {FAST}")
        self.assertEqual((row["result"], row["minutes"]), ("pass", "2.5"))  # five runs of half a minute
        self.assertIn(f"push_main: FLAKY: slow::one failed in run 1 of 3 of {SLOW}", out)
        self.assertIn("push_main: flaky in this run (failed, then passed in a further run): slow::one", out)

    def test_a_test_that_fails_every_time_stops_the_push_and_writes_no_ledger_line(self):
        self.merge("f001-work", ["crates/a/src/one.rs"])
        head = self.g("rev-parse", "HEAD")
        r = Runner({SLOW: [(101, CARGO_FAILED)] * 9})
        code, out = self.run_plan(self.plan(CHECK, SLOW, FAST), r)
        self.assertEqual(code, 1)
        self.assertEqual(r.checks(), [CHECK, SLOW, SLOW, SLOW])  # FAST is not reached
        self.assertEqual(self.rows(), [])
        self.assertEqual(self.g("rev-parse", "HEAD"), head)  # no commit
        self.assertIn(f"push_main: FAILED (exit 101): {SLOW}", out)
        self.assertIn("nothing written to the ledger; the merge is not pushed", out)


class FirstAttempt(TempRepo):
    """What a run that passes at the first attempt writes is what it wrote before ADR 0752."""

    def test_the_ledger_line_of_a_run_with_no_failure(self):
        sha = self.merge("f001-work", ["crates/a/src/one.rs", "docs/adr/0900-f001-registration-x.md"])
        r = Runner()
        code, out = self.run_plan(self.plan(CHECK, SLOW), r)
        self.assertEqual(code, 0, out)
        (row,) = self.rows()
        self.assertEqual(row, {"date": pm.today(), "kind": "merge", "commit": sha[:10], "branch": "f001-work", "items": "F-001",
                               "adrs": "0900", "result": "pass", "minutes": "1.0", "left_out": "0", "left_out_minutes": "0.0",
                               "detail": f"exit=0;mutations=0;cmds={CHECK} | {SLOW}"})
        self.assertEqual(self.g("log", "-1", "--format=%s"), "Ledger and generated pages after the merge of F-001")
        self.assertEqual(self.g("status", "--porcelain"), "")
        self.assertEqual(self.state_files(), [])
        self.assertNotIn("FLAKY", out)


class Resume(TempRepo):
    """Point 2: a stopped run is resumed without repeating the commands that passed on the same merge commit."""

    C1, C3 = "python scripts/process_report.py --check", "cargo test -p a --release --test slow"

    def stopped(self):
        self.merge("f001-work", ["crates/a/src/one.rs"])
        r = Runner({FAST: [(101, "test one ... FAILED\n")], self.C3: [(101, CARGO_FAILED)] * 3})
        code, _ = self.run_plan(self.plan(self.C1, FAST, self.C3), r)
        self.assertEqual((code, r.checks()), (1, [self.C1, FAST, FAST, self.C3, self.C3, self.C3]))
        self.assertEqual(self.rows(), [])
        self.assertEqual(self.state_files(), [f"run-{self.g('rev-parse', 'HEAD')[:8]}.json"])

    def test_the_next_run_repeats_nothing_that_passed_and_the_line_says_what_was_checked(self):
        self.stopped()
        r = Runner()
        code, out = self.run_plan(self.plan(self.C1, FAST, self.C3), r)
        self.assertEqual(code, 0, out)
        self.assertEqual(r.checks(), [self.C3])
        row = self.rows()[-1]
        self.assertEqual(row["detail"], f"exit=0;mutations=0;flaky=fast::one;resumed=2;cmds={self.C1} | {FAST} | {self.C3}")
        self.assertEqual(row["minutes"], "2.0")  # 0.5 and two runs of 0.5 from the stopped run, 0.5 now
        self.assertIn(f"push_main: passed earlier on this merge commit, not repeated: {self.C1}", out)
        self.assertEqual(self.state_files(), [])  # gone once the ledger line is written

    def test_the_record_of_another_merge_commit_is_not_used(self):
        self.stopped()
        name = self.state_files()[0]
        self.g("commit", "-q", "--amend", "-m", "Merge branch 'f001-work' again")  # another commit with the same tree
        new = self.g("rev-parse", "HEAD")
        os.replace(os.path.join(self.d, "target", "push_main", name), os.path.join(self.d, "target", "push_main", f"run-{new[:8]}.json"))
        r = Runner()
        code, out = self.run_plan(self.plan(self.C1, FAST, self.C3), r)
        self.assertEqual((code, r.checks()), (0, [self.C1, FAST, self.C3]), out)
        self.assertEqual(self.rows()[-1]["detail"], f"exit=0;mutations=0;cmds={self.C1} | {FAST} | {self.C3}")
        self.assertIn("is of another merge commit or base; not used", out)

    def test_fresh_repeats_everything(self):
        self.stopped()
        r = Runner()
        code, out = self.run_plan(self.plan(self.C1, FAST, self.C3), r, fresh=True)
        self.assertEqual((code, r.checks()), (0, [self.C1, FAST, self.C3]), out)
        self.assertNotIn("resumed", self.rows()[-1]["detail"])

    def test_a_verified_mutation_patch_is_not_verified_again(self):
        self.put("scripts/t.mutation.patch", "# expect-fail: the-test\n")
        self.commit("scripts/x.py", "a patch and a script")
        self.merge("f001-work", ["crates/a/src/one.rs"])
        applied = []
        real = subprocess.run

        def fake(cmd, **kw):  # verify_patch applies and removes the patch with `git apply`
            if cmd[:2] == ["git", "apply"]:
                applied.append(cmd)
                return argparse.Namespace(returncode=0)
            return real(cmd, **kw)
        plan = self.plan(self.C1, self.C3, patches=["scripts/t.mutation.patch"])
        pm.subprocess.run = fake
        try:
            code, _ = self.run_plan(plan, Runner({"the-test": [(1, "")], self.C3: [(1, "")] * 3}))
            self.assertEqual(code, 1)  # C3 fails every time, before the patch
            code, out = self.run_plan(plan, Runner({self.C3: [(1, "")] * 3}))
            self.assertEqual((code, applied), (1, []))  # the patch is only reached when the commands pass
            r = Runner({"the-test": [(1, "")]})
            code, out = self.run_plan(plan, r)
            # after the patch is removed the engine binaries are built again, before any Studio test (ADR 0823 addendum 3)
            self.assertEqual((code, len(applied), r.checks()), (0, 2, [self.C3, "the-test", pm.REBUILD_AFTER_MUTATIONS]), out)
        finally:
            pm.subprocess.run = real
        self.assertEqual(self.rows()[-1]["detail"], f"exit=0;mutations=1;resumed=1;cmds={self.C1} | {self.C3}")


class LeftRunning(unittest.TestCase):
    """A captured command ends when its process does, not when a process it left running lets go of the output pipe
    (a runner a test started kept push_main waiting for nine hours on 2026-11-27)."""

    def test_a_child_left_holding_the_pipe_is_not_waited_for(self):
        with tempfile.TemporaryDirectory() as d:
            pidfile = os.path.join(d, "child.pid")
            child = "import time; time.sleep(120)"
            # the child inherits the command's standard output, as a detached daemon inherits the pipe's handle
            parent = (f"import subprocess, sys; c = subprocess.Popen([sys.executable, '-c', {child!r}]); "
                      f"open({pidfile!r}, 'w').write(str(c.pid)); print('parent done', flush=True)")
            old = pm.PIPE_DRAIN_SECONDS
            pm.PIPE_DRAIN_SECONDS = 1
            out = io.StringIO()
            try:
                with contextlib.redirect_stdout(out):
                    t0 = time.time()
                    code, _, text = pm.sh(f'"{sys.executable}" -c "{parent}"', d, capture=True)
                    took = time.time() - t0
            finally:
                pm.PIPE_DRAIN_SECONDS = old
                with open(pidfile) as f:
                    pid = int(f.read())
                if os.name == "nt":
                    subprocess.run(["taskkill", "/PID", str(pid), "/F"], capture_output=True)
                else:
                    os.kill(pid, 9)
            self.assertEqual(code, 0)
            self.assertIn("parent done", text)
            self.assertLess(took, 60, "sh waited for the child instead of the command")
            self.assertIn("still held open", out.getvalue())

    def test_a_command_that_leaves_nothing_keeps_all_its_output(self):
        code, _, text = pm.sh(f'"{sys.executable}" -c "print(1); print(2)"', ".", capture=True)
        self.assertEqual((code, text.split()), (0, ["1", "2"]))


class Build(unittest.TestCase):
    """Point 3: the budget counts build time from a recorded figure."""

    def test_the_registered_figure(self):
        self.assertEqual(pm.BUILD_SECONDS, 1.8)

    def test_the_cost_of_a_binary_holds_the_build_figure(self):
        u = {x["target"]: x for x in pm.binary_units(IDX, {"a"}, TIMES, 60, 1.8)}
        self.assertAlmostEqual(u["fast"]["cost"], 12.8)
        self.assertAlmostEqual(u["norec"]["cost"], 61.8)
        self.assertEqual((u["fast"]["seconds"], u["norec"]["seconds"]), (11.0, None))  # the record itself is unchanged

    def test_fewer_binaries_fit_the_same_budget(self):
        without = pm.cut([pm.order_group(pm.binary_units(IDX, {"a"}, TIMES, 60), set())], 575.0)
        with_build = pm.cut([pm.order_group(pm.binary_units(IDX, {"a"}, TIMES, 60, 1.8), set())], 575.0)
        self.assertEqual(([x["target"] for x in without[0]], without[2]), (["fast", "norec", "slow"], 571.0))
        self.assertEqual([x["target"] for x in with_build[0]], ["fast", "norec"])
        self.assertAlmostEqual(with_build[2], 74.6)

    def test_the_plan_uses_the_figure_and_says_so(self):
        info = {"sha": "0" * 40, "first": "1" * 40, "branch": "f001-work", "items": [], "adrs": [], "files": ["crates/a/src/lib.rs"]}
        with tempfile.TemporaryDirectory() as d:
            plan = pm.make_plan(d, info, IDX, TIMES, [], 575.0 / 60.0, 60)
            self.assertEqual(([u["target"] for u in plan["selected"]], plan["build_s"]), (["fast", "norec"], 1.8))
            self.assertEqual([u["target"] for u in plan["left"]], ["slow", "ct", "bt"])
            self.assertAlmostEqual(plan["left_minutes"], (501.8 + 21.8 + 31.8) / 60.0)
            zero = pm.make_plan(d, info, IDX, TIMES, [], 575.0 / 60.0, 60, 0.0)
            self.assertEqual([u["target"] for u in zero["selected"]], ["fast", "norec", "slow"])
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            pm.print_plan(info, plan, "2" * 40, 0)
        self.assertIn("diff-selected (2 binaries, 1.2 recorded minutes, with 1.8 s of build counted per binary):", buf.getvalue())


class Joint(TempRepo):
    """Point 4: several merges are taken as one joint diff."""

    def two_merges(self):
        m1 = self.merge("f001-work", ["docs/adr/0900-f001-registration-x.md", "crates/a/src/one.rs"],
                        "Merge branch 'f001-work': F-001 done (ADR 0900)")
        self.commit("docs/notes.md", "the lead's notes")
        m2 = self.merge("f002-work", ["crates/b/src/two.rs", "docs/adr/0901-f002-registration-y.md"])
        return m1, m2

    def test_subject_branches(self):
        self.assertEqual(pm.subject_branches("Merge branch 'f529-work': F-529 partial (ADR 0503), F-431 note"), ["f529-work"])
        self.assertEqual(pm.subject_branches("Merge branches 'a', 'b' and 'c' into main"), ["a", "b", "c"])
        self.assertEqual(pm.subject_branches("Merge branches 'a' and 'b'"), ["a", "b"])
        self.assertEqual(pm.subject_branches("Merge remote-tracking branch 'origin/x'"), ["origin/x"])
        self.assertEqual(pm.subject_branches("Merge commit '058ef506'"), [])
        self.assertEqual(pm.subject_branches("Ledger: the line of 'x'"), [])

    def test_a_range_of_first_parent_merges_is_one_diff(self):
        m1, m2 = self.two_merges()
        info = pm.merge_info(self.d, "HEAD", self.base)
        self.assertEqual(sorted(info["files"]), ["crates/a/src/one.rs", "crates/b/src/two.rs", "docs/adr/0900-f001-registration-x.md",
                                                 "docs/adr/0901-f002-registration-y.md", "docs/notes.md"])
        self.assertEqual((info["branch"], info["items"], info["adrs"]), ("f001-work,f002-work", ["F-001", "F-002"], ["0900", "0901"]))
        self.assertEqual((info["sha"], info["first"], info["merges"]), (m2, self.base, [m1, m2]))
        alone = pm.merge_info(self.d, "HEAD")  # without --base the last merge alone, as before
        self.assertEqual((sorted(alone["files"]), alone["branch"], alone["merges"]),
                         (["crates/b/src/two.rs", "docs/adr/0901-f002-registration-y.md"], "f002-work", [m2]))

    def test_a_range_that_is_not_one_stops(self):
        m1, m2 = self.two_merges()
        side = self.g("rev-parse", "f001-work")  # an ancestor of HEAD that is not on main's first-parent line
        for base in (side, "HEAD"):
            with self.assertRaises(SystemExit):
                pm.merge_info(self.d, "HEAD", base)
        self.commit("docs/notes.md", "more notes")  # the last commit of the range is not a merge
        with self.assertRaises(SystemExit):
            pm.merge_info(self.d, "HEAD", self.base)

    def test_a_merge_of_more_than_two_parents_is_one_diff(self):
        for br, f in (("f003-x", "crates/a/src/three.rs"), ("f004-y", "crates/b/src/four.rs")):
            self.g("checkout", "-q", "-b", br, "main")
            self.commit(f, "work")
        self.g("checkout", "-q", "main")
        self.g("merge", "-q", "--no-ff", "f003-x", "f004-y", "-m", "Merge branches 'f003-x' and 'f004-y'")
        self.assertEqual(len(self.g("rev-list", "--parents", "-n", "1", "HEAD").split()), 4)
        info = pm.merge_info(self.d, "HEAD")
        self.assertEqual((sorted(info["files"]), info["branch"], info["items"]),
                         (["crates/a/src/three.rs", "crates/b/src/four.rs"], "f003-x,f004-y", ["F-003", "F-004"]))

    def test_one_ledger_line_for_the_joint_run(self):
        m1, m2 = self.two_merges()
        code, out = self.run_plan(self.plan(CHECK), Runner(), base=self.base)
        self.assertEqual(code, 0, out)
        (row,) = self.rows()
        self.assertEqual((row["commit"], row["branch"], row["items"], row["adrs"]),
                         (m2[:10], "f001-work,f002-work", "F-001,F-002", "0900,0901"))
        self.assertEqual(row["detail"], f"exit=0;mutations=0;joint={m1[:10]};cmds={CHECK}")
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            pm.print_plan(pm.merge_info(self.d, m2, self.base), self.plan(CHECK), "2" * 40, 2)
        self.assertIn(f"joint diff of 2 merges, {self.base[:8]}..{m2[:8]}: {m1[:8]}, {m2[:8]}", buf.getvalue())


class Times(TempRepo):
    """Point 5: --times has a default, the newest gate times path the ledger names."""

    ROWS = [{"kind": "gate", "commit": "aaa", "detail": "passed=1;failed=0;ignored=0;times=gates/old"},
            {"kind": "merge", "commit": "bbb", "detail": "exit=0;mutations=0;cmds=x;times=gates/not-a-gate-line"},
            {"kind": "gate", "commit": "ccc", "detail": "passed=1;failed=0;ignored=0;times=gates/new;GitHub run 7"},
            {"kind": "gate", "commit": "ddd", "detail": "passed=1;failed=0;ignored=0;times=gates/gone"},
            {"kind": "gate", "commit": "eee", "detail": "passed=1;failed=0;ignored=0;GitHub run 9"}]

    def test_the_newest_gate_line_with_a_path_that_exists(self):
        here = lambda p: not p.replace("\\", "/").endswith("gates/gone")  # noqa: E731
        self.assertEqual(pm.default_times(self.ROWS, "R", here), (os.path.join("R", "gates/new"), "ccc"))
        self.assertEqual(pm.default_times(self.ROWS[:2], "R", here), (os.path.join("R", "gates/old"), "aaa"))
        self.assertEqual(pm.default_times(self.ROWS, "R", lambda p: False), (None, None))
        self.assertEqual(pm.default_times(self.ROWS[4:], "R", here), (None, None))
        absolute = os.path.abspath(self.d)
        rows = [{"kind": "gate", "commit": "fff", "detail": "passed=1;failed=0;ignored=0;times=" + absolute}]
        self.assertEqual(pm.default_times(rows, "R"), (absolute, "fff"))

    def test_times_given_wins_and_the_plan_says_what_it_used(self):
        here = lambda p: True  # noqa: E731
        self.assertEqual(pm.times_source("mine.tsv", self.ROWS, "R", 60, here), ("mine.tsv", "recorded seconds: mine.tsv (--times)"))
        path, note = pm.times_source(None, self.ROWS, "R", 60, here)
        self.assertEqual((path, note), (os.path.join("R", "gates/gone"), f"recorded seconds: {path} (named by the gate line of ddd)"))
        path, note = pm.times_source(None, self.ROWS, "R", 60, lambda p: False)
        self.assertIsNone(path)
        self.assertIn("recorded seconds: none", note)
        self.assertIn("every binary counts 60 s", note)

    def test_the_gate_line_names_the_times_path(self):
        self.assertEqual(pm.gate_detail(10, 0, 3, note="GitHub run 1", skipped=2, times="../gate-abc"),
                         "passed=10;failed=0;ignored=3;skipped=2;times=../gate-abc;GitHub run 1")
        self.put("gates/abc/shard-1/times.tsv", "binary\ttest\toutcome\tseconds\tpeak_mb\nfast\tone\tok\t5.00\t6\nfast\ttwo\tok\t6.00\t6\n")
        self.put("gates/abc/shard-1/times_hooks.tsv", "binary\ttest\toutcome\tseconds\tpeak_mb\nfast\tone\tok\t9.00\t6\n")
        self.put("gates/abc/shard-2/times.tsv", "binary\ttest\toutcome\tseconds\tpeak_mb\nslow\tone\tok\t500.00\t6\nfast\ttwo\tok\t2.00\t6\n")
        self.put("gates/abc/shard-2/test.log", "not a times file\n")
        a = argparse.Namespace(commit="HEAD", passed=5, failed=0, ignored=1, skipped=None, attribution=None, date="2026-11-21",
                               minutes="", note="", gate_times="gates\\abc")
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(pm.do_gate_line(self.d, a), 0)
        rows = self.rows()
        self.assertEqual(rows[-1]["detail"], "passed=5;failed=0;ignored=1;times=gates/abc")
        path, gate = pm.default_times(rows, self.d)
        self.assertEqual((path, gate), (os.path.join(self.d, "gates/abc"), self.base[:10]))
        self.assertEqual(pm.read_times(path), {("fast", "one"): 9.0, ("fast", "two"): 6.0, ("slow", "one"): 500.0})
        self.assertEqual(pm.read_times(os.path.join(path, "shard-2", "times.tsv")), {("slow", "one"): 500.0, ("fast", "two"): 2.0})
        for bad in ("gates/abc;x", "gates/none"):
            a.gate_times = bad
            with self.assertRaises(SystemExit):
                pm.do_gate_line(self.d, a)
        self.assertEqual(len(self.rows()), len(rows))


class Tree(TempRepo):
    """Point 6: a left-out line in the working tree does not stop the next run. Finding 9: nor do the generated files."""

    HEAD_LINE = "\t".join(pm.COLUMNS) + "\n"
    GATE = pm.ledger_line(date="2026-11-12", kind="gate", commit="49fbf1a3", result="green", detail="passed=1;failed=0;ignored=0") + "\n"
    LEFT = pm.ledger_line(date="2026-11-21", kind="leftout", commit="abc", branch="f000-x", result="green", detail="ran=2;failed=0") + "\n"

    def setUp(self):
        super().setUp()
        self.put(pm.LEDGER, self.HEAD_LINE + self.GATE)
        self.put("docs/adr/index.md", "the index\n")
        self.commit("docs/page.md", "a ledger, an index and a page")
        self.merge("f001-work", ["crates/a/src/one.rs"])

    def test_appended_or_not(self):
        old = self.HEAD_LINE + self.GATE
        self.assertTrue(pm.ledger_appended(old, old + self.LEFT))
        self.assertTrue(pm.ledger_appended(old, old))
        self.assertTrue(pm.ledger_appended("", old))
        self.assertFalse(pm.ledger_appended(old, self.HEAD_LINE + self.GATE.replace("green", "red") + self.LEFT))
        self.assertFalse(pm.ledger_appended(old, self.HEAD_LINE))  # a line removed
        self.assertFalse(pm.ledger_appended(old.rstrip("\n"), old.rstrip("\n") + ";more\n"))  # the last line made longer

    def test_what_stops_a_run(self):
        gen = {"docs/adr/index.md", pm.LEDGER}
        self.assertEqual(pm.blocking_changes([pm.LEDGER], gen, True), [])
        self.assertEqual(pm.blocking_changes([pm.LEDGER], gen, False), [pm.LEDGER])
        self.assertEqual(pm.blocking_changes(["docs/adr/index.md", "crates/a/src/lib.rs", pm.LEDGER], gen, True), ["crates/a/src/lib.rs"])

    def test_a_left_out_line_in_the_working_tree_does_not_stop_the_run_and_is_committed(self):
        self.put(pm.LEDGER, self.LEFT, "a")
        code, out = self.run_plan(self.plan(CHECK), Runner())
        self.assertEqual(code, 0, out)
        self.assertEqual([r["kind"] for r in self.rows()], ["gate", "leftout", "merge"])
        self.assertEqual(self.g("status", "--porcelain"), "")
        self.assertIn("\tleftout\tabc\t", self.g("show", f"HEAD:{pm.LEDGER}"))

    def test_a_changed_line_of_the_ledger_stops_the_run(self):
        self.put(pm.LEDGER, self.HEAD_LINE + self.GATE.replace("green", "red") + self.LEFT)
        r = Runner()
        code, _ = self.run_plan(self.plan(CHECK), r)
        self.assertEqual((code, r.checks()), (f"push_main: the working tree has uncommitted changes: {pm.LEDGER}", []))

    def test_another_changed_file_stops_the_run_and_a_generated_one_does_not(self):
        self.put("docs/adr/index.md", "the index, regenerated by a stopped run\n")
        r = Runner(listed="docs/adr/index.md\ndocs/ROADMAP.md\n")
        self.put("docs/page.md", "edited\n")
        code, _ = self.run_plan(self.plan(CHECK), r)
        self.assertEqual((code, r.checks()), ("push_main: the working tree has uncommitted changes: docs/page.md", []))
        self.g("checkout", "--", "docs/page.md")
        code, out = self.run_plan(self.plan(CHECK), r)
        self.assertEqual(code, 0, out)
        self.assertEqual(self.g("show", "HEAD:docs/adr/index.md"), "the index, regenerated by a stopped run")


class NoPatch(unittest.TestCase):
    """Point 7: a merge whose ADR has registered tests and no mutation patch is named in the output."""

    def registered(self):
        def reg(crate, target, name, adrs, kind="test"):
            return dict(t(crate, target, name, kind=kind), adrs=adrs)
        return [reg("a", "fast", "one", ["0900"]), reg("a", "fast", "two", ["0900"]), reg("a", "a", "unit", ["0900"], kind="lib"),
                reg("b", "bt", "one", ["0901"]), reg("c", "ct", "one", [])]

    def test_which_adrs_have_no_patch(self):
        ts = self.registered()
        self.assertEqual(pm.unpatched_adrs(ts, ["0900", "0901", "0902"], []), [("0900", 3), ("0901", 1)])  # 0902 has no test
        self.assertEqual(pm.unpatched_adrs(ts, ["0900", "0901"], ["crates/a/tests/fast.mutation.patch"]), [("0901", 1)])
        self.assertEqual(pm.unpatched_adrs(ts, ["0900"], ["crates/b/tests/bt.mutation.patch", "scripts/test_x.mutation.patch"]), [])
        self.assertEqual(pm.unpatched_adrs(ts, ["0900", "0901"], ["scripts/test_x.mutation.patch"]), [])  # beside no ADR's test
        self.assertEqual(pm.unpatched_adrs(ts, [], []), [])

    def test_the_plan_names_them(self):
        info = {"sha": "0" * 40, "first": "1" * 40, "branch": "f001-work", "items": ["F-001"], "adrs": ["0900", "0901"],
                "files": ["crates/a/tests/fast.mutation.patch"]}
        idx = {"crates": CRATES, "tests": self.registered()}
        with tempfile.TemporaryDirectory() as d:
            plan = pm.make_plan(d, info, idx, {}, [], 30.0, 60)
        self.assertEqual(plan["no_patch"], [("0901", 1)])
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            pm.print_plan(info, plan, "2" * 40, 0)
        self.assertIn("no mutation patch: ADR 0901 has 1 registered tests and no committed mutation patch (agent rules, rule 16)",
                      buf.getvalue())
        self.assertNotIn("ADR 0900 has", buf.getvalue())


class LeadProcess(unittest.TestCase):
    """Point 8: lead-process.md says what the script does with the roadmap status (it writes none)."""

    def test_the_page_says_the_script_writes_no_roadmap_status(self):
        with open(os.path.join(HERE, "..", "docs", "process", "lead-process.md"), encoding="utf-8") as fh:
            text = " ".join(fh.read().split())
        self.assertIn("The script writes no roadmap status.", text)
        self.assertNotIn("with the item's roadmap status", text)
        self.assertIn("writes no roadmap status", pm.__doc__)


class Regenerate(TempRepo):
    """Finding 9: the generated files are regenerated before the process check, not after it."""

    def test_the_index_is_written_before_it_is_checked(self):
        self.merge("f001-work", ["docs/adr/0900-f001-registration-x.md"])
        docs = "cargo test -p rcfd-cli --release --test docs_site"
        r = Runner()
        code, out = self.run_plan(self.plan(docs, CHECK), r)
        self.assertEqual(code, 0, out)
        self.assertEqual(r.calls[:4], [WRITE, "python scripts/roadmap_progress.py", docs, CHECK])
        self.assertEqual(r.calls.count(WRITE), 1)
        self.assertEqual(self.rows()[-1]["detail"], f"exit=0;mutations=0;cmds={docs} | {CHECK}")  # not a command of the plan

    def test_a_failed_regeneration_stops_the_run(self):
        self.merge("f001-work", ["docs/adr/0900-f001-registration-x.md"])
        r = Runner({WRITE: [(1, "")]})
        code, _ = self.run_plan(self.plan(CHECK), r)
        self.assertEqual((code, r.calls, self.rows()), (1, [WRITE], []))


class ScriptTests(unittest.TestCase):
    """Finding 13: a merge that changes scripts/ has the script tests of scripts/ in its plan."""

    NAMES = ["test_b.py", "push_main.py", "test_a.py", "test_a.mutation.patch", "mytest_c.py", "test_d.txt"]

    def test_the_commands(self):
        want = ["python scripts/test_a.py", "python scripts/test_b.py"]
        self.assertEqual(pm.script_test_commands(["docs/x.md", "scripts/push_main.py"], self.NAMES), want)
        self.assertEqual(pm.script_test_commands(["scripts/docker/Dockerfile"], self.NAMES), want)
        self.assertEqual(pm.script_test_commands(["docs/scripts/x.md", "crates/a/scripts/y.py"], self.NAMES), [])

    def test_the_plan_begins_with_them(self):
        info = {"sha": "0" * 40, "first": "1" * 40, "branch": "f001-work", "items": [], "adrs": [], "files": ["scripts/push_main.py"]}
        with tempfile.TemporaryDirectory() as d:
            os.makedirs(os.path.join(d, "scripts"))
            for n in self.NAMES:
                open(os.path.join(d, "scripts", n), "w").close()
            plan = pm.make_plan(d, info, IDX, {}, [], 30.0, 60)
            self.assertEqual(plan["always"][:2], ["python scripts/test_a.py", "python scripts/test_b.py"])
            other = pm.make_plan(d, dict(info, files=["docs/x.md"]), IDX, {}, [], 30.0, 60)
            self.assertFalse(any(c.startswith("python scripts/test_") for c in other["always"]))

    def test_this_repository_has_script_tests_and_each_can_be_run_as_a_file(self):
        cmds = pm.script_test_commands(["scripts/x"], os.listdir(HERE))
        self.assertIn("python scripts/test_push_main.py", cmds)
        self.assertIn("python scripts/test_fmt_check.py", cmds)
        for c in cmds:
            with open(os.path.join(HERE, c.split("/")[-1]), encoding="utf-8") as fh:
                self.assertIn('__name__ == "__main__"', fh.read(), c)


class Items(TempRepo):
    """Finding 18: the ledger's item field is read from the brief or from the head of the branch name."""

    def test_the_head_of_a_branch_name(self):
        self.assertEqual(pm.branch_items("f855-edit-lock-f2-lockbusy"), ["F-855"])
        self.assertEqual(pm.branch_items("f828-f814-x"), ["F-814", "F-828"])
        self.assertEqual(pm.branch_items("citations-2026-w47"), [])
        self.assertEqual(pm.branch_items("dependabot/cargo/wat-1.263.0"), [])
        self.assertEqual(pm.branch_items("fix-f2-lock"), [])
        self.assertEqual(pm.branch_items("f855"), [])  # no segment after it: not the form of a branch of an item

    def test_the_items_line_of_a_brief(self):
        self.assertEqual(pm.brief_items("# Brief\n\nBranch: x\nItems: F-851, F-12\nADR: 0752\n"), ["F-012", "F-851"])
        self.assertEqual(pm.brief_items("Branch: x\nItems: none\n"), [])
        self.assertIsNone(pm.brief_items("Branch: x\nThe Items: F-1 of another line\n"))
        self.assertIsNone(pm.brief_items(None))

    def test_the_brief_wins_and_only_a_missing_brief_falls_back_to_the_name(self):
        briefs = {"f777-y": "Items: F-778\n", "f100-z": "Items: none\n", "citations-w47": "Items: F-431\n"}
        self.assertEqual(pm.merge_items(["f777-y"], briefs.get), ["F-778"])
        self.assertEqual(pm.merge_items(["f100-z"], briefs.get), [])
        self.assertEqual(pm.merge_items(["f855-edit-lock-f2-lockbusy", "citations-w47"], briefs.get), ["F-431", "F-855"])
        self.assertEqual(pm.brief_path("dependabot/cargo/wat-1"), "docs/adr/briefs/dependabot-cargo-wat-1.md")

    def test_a_merge_gives_no_item_from_inside_a_name_from_an_adr_file_or_from_the_subject(self):
        self.merge("f855-a-f2-b", ["docs/adr/0901-f855-registration-the-f2-x.md", "crates/a/src/lock.rs"],
                   "Merge branch 'f855-a-f2-b' (F-855, ADR 0901: the lock; F-431 note; F2 unchanged)")
        info = pm.merge_info(self.d, "HEAD")
        self.assertEqual((info["branch"], info["items"], info["adrs"]), ("f855-a-f2-b", ["F-855"], ["0901"]))

    def test_a_merge_reads_the_brief_of_its_branch_from_the_merge_commit(self):
        self.commit("docs/adr/briefs/f777-y.md", "a brief", "# Brief\n\nBranch: f777-y\nItems: F-778, F-779\n")
        self.merge("f777-y", ["crates/a/src/y.rs"])
        self.assertEqual(pm.merge_info(self.d, "HEAD")["items"], ["F-778", "F-779"])
        self.commit("docs/adr/briefs/tidy-up.md", "a brief with no item", "Branch: tidy-up\nItems: none\n")
        self.merge("tidy-up", ["crates/a/src/z.rs"])
        self.assertEqual(pm.merge_info(self.d, "HEAD")["items"], [])

    def test_the_corrected_line_of_the_f855_merge_is_as_the_lead_left_it(self):
        rows = [r for r in pm.read_ledger(os.path.join(HERE, "..", pm.LEDGER)) if r["commit"] == "26856a9027" and r["kind"] == "merge"]
        self.assertEqual([(r["branch"], r["items"]) for r in rows], [("f855-edit-lock-f2-lockbusy", "F-855")])


import datetime  # noqa: E402


class Measures(TempRepo):
    """ADR 0754 (F-863): the options of --run that record the queue age, the review minutes and the two statuses."""

    def merged(self):
        sha = self.merge("f001-work", ["crates/a/src/one.rs"])
        return sha, int(self.g("log", "-1", "--format=%ct", sha))

    def stops(self, word, **flags):
        """The run stops with `word` in its message before any check, and no ledger line is written."""
        r = Runner()
        code, out = self.run_plan(self.plan(CHECK), r, **flags)
        self.assertIsInstance(code, str, out)
        self.assertIn(word, code)
        self.assertEqual((r.calls, self.rows()), ([], []))

    def detail(self, **flags):
        code, out = self.run_plan(self.plan(CHECK), Runner(), **flags)
        self.assertEqual(code, 0, out)
        return self.rows()[-1]["detail"]

    def test_the_queue_age_is_the_minutes_from_the_report_to_the_merge_commit(self):
        _, ct = self.merged()
        reported = datetime.datetime.fromtimestamp(ct - 90 * 60, datetime.timezone(datetime.timedelta(hours=1)))
        self.assertEqual(self.detail(reported=reported.isoformat(timespec="seconds")),
                         f"exit=0;mutations=0;reported={reported.isoformat(timespec='minutes')};queue_minutes=90;cmds={CHECK}")

    def test_the_minutes_are_rounded_and_a_time_without_an_offset_is_local_time(self):
        merge = datetime.datetime(2026, 11, 21, 11, 29, 40, tzinfo=datetime.timezone.utc).timestamp()
        self.assertEqual(pm.queue_minutes(pm.report_time("2026-11-21T10:00+00:00"), merge), 90)
        self.assertEqual(pm.queue_minutes(pm.report_time("2026-11-21T12:00:00+02:00"), merge), 90)
        self.assertEqual(pm.queue_minutes(pm.report_time("2026-11-21 11:29+00:00"), merge), 1)
        local = pm.report_time("2026-11-21T10:00")
        self.assertIsNotNone(local.utcoffset())
        self.assertEqual(local.timestamp(), datetime.datetime(2026, 11, 21, 10, 0).timestamp())
        self.assertEqual(pm.measure_entries(reported="2026-11-21T10:00+00:00", merge_epoch=merge),
                         ["reported=2026-11-21T10:00+00:00", "queue_minutes=90"])

    def test_a_report_after_the_merge_or_an_unreadable_time_stops_before_any_check(self):
        _, ct = self.merged()
        late = datetime.datetime.fromtimestamp(ct + 120, datetime.timezone.utc)
        self.stops("--reported", reported=late.isoformat(timespec="seconds"))
        self.stops("--reported", reported="yesterday")
        self.stops("--reported", reported="2026-11-21")

    def test_the_review_minutes_are_written_as_the_lead_states_them(self):
        self.merged()
        self.assertEqual(self.detail(review_minutes="25"), f"exit=0;mutations=0;review_minutes=25;cmds={CHECK}")
        self.assertEqual(pm.measure_entries(review_minutes="12.5"), ["review_minutes=12.5"])
        self.assertEqual(pm.measure_entries(review_minutes=0), ["review_minutes=0"])

    def test_review_minutes_that_are_negative_or_no_number_stop_before_any_check(self):
        self.merged()
        for bad in ("-1", "soon", "nan", "inf"):
            self.stops("--review-minutes", review_minutes=bad)

    def test_the_proposed_and_the_written_status(self):
        self.merged()
        self.assertEqual(self.detail(proposed="done", written="partial"),
                         f"exit=0;mutations=0;proposed=done;written=partial;cmds={CHECK}")
        self.assertEqual(pm.measure_entries(proposed="todo"), ["proposed=todo"])
        self.assertEqual(pm.measure_entries(written="done"), ["written=done"])
        self.assertEqual(pm.measure_entries(proposed="F-863=done,F-864=partial", written="F-864=partial,F-863=partial"),
                         ["proposed=F-863=done,F-864=partial", "written=F-864=partial,F-863=partial"])

    def test_a_status_that_is_no_status_stops_before_any_check(self):
        self.merged()
        self.stops("--proposed", proposed="finished")
        self.stops("--written", proposed="done", written="Done.")
        self.stops("--written", proposed="done", written="F-863=done")
        self.stops("--written", proposed="F-863=done,F-864=done", written="F-863=done,F-865=done")
        self.stops("--proposed", proposed="F-863=done;F-864=done")

    def test_no_option_gives_no_entry(self):
        self.assertEqual(pm.measure_entries(), [])
        self.merged()
        self.assertEqual(self.detail(reported=None, review_minutes=None, proposed=None, written=None),
                         f"exit=0;mutations=0;cmds={CHECK}")

    def test_the_entries_stand_after_joint_and_before_cmds_in_the_registered_order(self):
        m1 = self.merge("f001-work", ["crates/a/src/one.rs"])
        self.commit("docs/notes.md", "the lead's notes")
        m2 = self.merge("f002-work", ["crates/b/src/two.rs"])
        ct = int(self.g("log", "-1", "--format=%ct", m2))
        reported = datetime.datetime.fromtimestamp(ct - 7 * 60, datetime.timezone.utc)
        code, out = self.run_plan(self.plan(CHECK, SLOW), Runner({SLOW: [(101, CARGO_FAILED)]}), base=self.base,
                                  written="F-001=done,F-002=partial", proposed="F-001=done,F-002=done", review_minutes="40",
                                  reported=reported.isoformat(timespec="seconds"))
        self.assertEqual(code, 0, out)
        detail = self.rows()[-1]["detail"]
        self.assertRegex(detail, r"^exit=0;mutations=0;flaky=[^;]+;joint=" + m1[:10] + ";reported=")
        self.assertTrue(detail.endswith(
            f";joint={m1[:10]};reported={reported.isoformat(timespec='minutes')};queue_minutes=7;review_minutes=40;"
            f"proposed=F-001=done,F-002=done;written=F-001=done,F-002=partial;cmds={CHECK} | {SLOW}"), detail)

    def test_an_option_without_run_stops(self):
        self.merged()
        for flag, value in (("--reported", "2026-11-21T10:00"), ("--review-minutes", "5"), ("--proposed", "done"),
                            ("--written", "done")):
            with self.assertRaisesRegex(SystemExit, flag + " needs --run"):
                pm.main(["--root", self.d, flag, value])
        with self.assertRaisesRegex(SystemExit, "--escaped needs --gate-line"):
            pm.main(["--root", self.d, "--run", "--escaped", "none"])
        self.assertEqual(self.rows(), [])


class Escaped(TempRepo):
    """ADR 0754 (F-863): --escaped of --gate-line names the commit that brought a failed test of a red gate in."""

    def gate(self, escaped, failed=2, note="GitHub run 1"):
        a = argparse.Namespace(commit="HEAD", passed=5, failed=failed, ignored=1, skipped=None, attribution=None,
                               date="2026-11-21", minutes="", note=note, gate_times=None, escaped=escaped)
        with contextlib.redirect_stdout(io.StringIO()):
            return pm.do_gate_line(self.d, a)

    def test_the_gate_line_names_the_commit_and_the_test(self):
        sha = self.merge("f001-work", ["crates/a/src/one.rs"])
        self.assertEqual(self.gate([f"{sha[:8]}=slow::one", f"{self.base}=bt::x", f"{sha}=slow::one"]), 0)
        self.assertEqual(self.rows()[-1]["detail"],
                         f"passed=5;failed=2;ignored=1;escaped={sha[:10]}=slow::one,{self.base[:10]}=bt::x;GitHub run 1")
        self.assertEqual(self.rows()[-1]["result"], "red")
        self.assertEqual(pm.gate_detail(5, 2, 1, note="n", times="gates/abc", escaped="escaped=abc=slow::one"),
                         "passed=5;failed=2;ignored=1;times=gates/abc;escaped=abc=slow::one;n")

    def test_none_says_that_no_commit_brought_a_failure_in(self):
        self.assertEqual(self.gate(["none"], note=""), 0)
        self.assertEqual(self.rows()[-1]["detail"], "passed=5;failed=2;ignored=1;escaped=none")

    def test_without_the_option_the_line_has_no_entry(self):
        self.assertEqual(self.gate(None, note=""), 0)
        self.assertEqual(self.rows()[-1]["detail"], "passed=5;failed=2;ignored=1")
        self.assertIsNone(pm.escaped_entry(None, lambda rev: None))
        self.assertIsNone(pm.escaped_entry([], lambda rev: None))

    def test_a_green_gate_or_a_value_that_cannot_be_read_stops_and_writes_no_line(self):
        sha = self.merge("f001-work", ["crates/a/src/one.rs"])
        cases = [dict(escaped=[f"{sha}=slow::one"], failed=0), dict(escaped=["none"], failed=0),
                 dict(escaped=["none", f"{sha}=slow::one"]), dict(escaped=["0123456789abcdef=slow::one"]),
                 dict(escaped=["slow::one"]), dict(escaped=[f"{sha}=slow"]), dict(escaped=[f"{sha}=slow::one;x=y"]),
                 dict(escaped=[f"{sha}=slow::one,bt::x"]), dict(escaped=[f"{sha}=slow::one two"])]
        for case in cases:
            with self.assertRaisesRegex(SystemExit, "--escaped", msg=str(case)):
                self.gate(**case)
        self.assertEqual(self.rows(), [])
        self.assertEqual(self.g("tag", "-l", "verified"), "")


class StopConditions(unittest.TestCase):
    """ADR 0754 (F-863): lead-process.md states the stop conditions, as a proposal until the maintainer approves the numbers."""

    def test_the_page_states_the_four_conditions_and_the_options(self):
        with open(os.path.join(HERE, "..", "docs", "process", "lead-process.md"), encoding="utf-8") as fh:
            page = fh.read()
        self.assertIn("\n## When no further agent is started\n", page)
        text = " ".join(page.split("\n## When no further agent is started\n", 1)[1].split("\n## ", 1)[0].split())
        for words in ("Reports not yet read", "Escaped defects", "A red full gate", "Agents at work at once", "--reported",
                      "--review-minutes", "--proposed", "--written", "--escaped", "ADR 0754"):
            self.assertIn(words, text)
        adr = [n for n in os.listdir(os.path.join(HERE, "..", "docs", "adr")) if n.startswith("0754-")]
        self.assertEqual(len(adr), 1)
        with open(os.path.join(HERE, "..", "docs", "adr", adr[0]), encoding="utf-8") as fh:
            record = fh.read()
        self.assertIn("Open questions for the maintainer", record)
        # the numbers are a proposal until the lead has written the maintainer's answers into the record
        if "Answers of the maintainer" not in record:
            self.assertIn("proposed, not in force until the maintainer approves them", text)
        for option in ("--reported", "--review-minutes", "--proposed", "--written", "--escaped"):
            self.assertIn(option, pm.__doc__)


# ---------------------------------------------------------------- ADR 0776 (F-860): the left-out run names its commits, the
# ADR's tests count in the budget, the binaries named after a changed source file come first


class LeftOutCommits(TempRepo):
    """ADR 0776, point 1 (row T1): a left-out line records the checkout's commit at the start and at the end of the run."""

    def job(self, cmds):
        sha = self.merge("f001-work", ["crates/a/src/one.rs"])
        p = os.path.join(self.d, "target", "push_main", f"leftout-{sha[:8]}.json")
        os.makedirs(os.path.dirname(p), exist_ok=True)
        with open(p, "w", encoding="utf-8") as fh:
            json.dump({"commit": sha, "branch": "f001-work", "items": ["F-001"], "adrs": ["0900"], "cmds": cmds}, fh)
        return sha, p

    def left_out(self, path, runner):
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            code = pm.do_left_out(self.d, path, run=runner)
        return code, buf.getvalue()

    def test_a_checkout_that_moves_during_the_run_is_named(self):
        sha, p = self.job([FAST, SLOW])
        moved = []

        def runner(cmd, cwd, capture=False):
            if cmd == SLOW:  # the lead's next merge lands in the checkout while the left-out tests run
                self.commit("docs/notes.md", "the lead's next merge")
                moved.append(self.g("rev-parse", "HEAD"))
            return 0, 0.5, ""
        code, out = self.left_out(p, runner)
        self.assertEqual(code, 0, out)
        row = self.rows()[-1]
        self.assertEqual(row["detail"],
                         f"ran=2;failed=0;checkout_start={sha[:10]};checkout_end={moved[0][:10]};checkout_moved=yes")
        self.assertEqual((row["kind"], row["commit"], row["result"], row["minutes"]), ("leftout", sha[:10], "green", "1.0"))
        self.assertIn(f"push_main: the checkout moved during the left-out run, from {sha[:10]} to {moved[0][:10]}", out)

    def test_a_checkout_that_stays_gives_two_equal_commits_and_no_move(self):
        sha, p = self.job([FAST, SLOW])
        r = Runner()
        code, out = self.left_out(p, r)
        self.assertEqual((code, r.calls), (0, [FAST, SLOW]))
        self.assertEqual(self.rows()[-1]["detail"], f"ran=2;failed=0;checkout_start={sha[:10]};checkout_end={sha[:10]}")
        self.assertNotIn("moved", out)

    def test_a_failed_binary_is_still_read_after_the_new_fields(self):
        sha, p = self.job([FAST, SLOW])
        code, _ = self.left_out(p, Runner({SLOW: [(101, CARGO_FAILED)]}))
        rows = self.rows()
        self.assertEqual((code, rows[-1]["result"]), (1, "red"))
        self.assertEqual(rows[-1]["detail"], f"ran=2;failed=1;checkout_start={sha[:10]};checkout_end={sha[:10]};"
                                             f"fails=slow::*=none/none;cmds={SLOW}")
        self.assertEqual(pm.recent_failed_binaries(rows), {"slow"})


TIMES_LIB = {**TIMES, ("a", "unit"): 3.0}


def adr_index(*pairs):
    """IDX with the tests of the binaries `pairs` ((crate, target), ...) registered to ADR 0900."""
    tests = [dict(x, adrs=["0900"] if (x["crate"], x["target"]) in pairs else []) for x in IDX["tests"]]
    return {"crates": CRATES, "tests": tests}


def printed(info, plan):
    buf = io.StringIO()
    with contextlib.redirect_stdout(buf):
        pm.print_plan(info, plan, "2" * 40, 0)
    return buf.getvalue()


class AdrBudget(unittest.TestCase):
    """ADR 0776, point 2 (row T2): the tests the index maps to the merge's ADRs are charged to the budget, never cut."""

    INFO = {"sha": "0" * 40, "first": "1" * 40, "branch": "f001-work", "items": ["F-001"], "adrs": ["0900"],
            "files": ["crates/a/src/lib.rs"]}
    DT = "cargo test -p d --release --test dt"

    def test_the_cost_of_an_adr_command(self):
        cmds = ["cargo test -p a --release --lib -- --exact unit", "cargo test -p a --release --test fast", self.DT,
                "cargo test -p a --release --test fast -- --exact two"]
        got = pm.adr_units(IDX, cmds, TIMES_LIB, 60, 1.8)
        self.assertEqual([u["cmd"] for u in got], cmds)
        self.assertEqual([u["seconds"] for u in got], [3.0, 11.0, None, 6.0])
        for u, want in zip(got, [4.8, 12.8, 61.8, 7.8]):
            self.assertAlmostEqual(u["cost"], want)

    def test_the_adr_tests_are_charged_first_and_fewer_binaries_fit(self):
        with tempfile.TemporaryDirectory() as d:
            plan = pm.make_plan(d, self.INFO, adr_index(("d", "dt")), TIMES, [], 120.0 / 60.0, 60)
            alone = pm.make_plan(d, dict(self.INFO, adrs=[]), IDX, TIMES, [], 120.0 / 60.0, 60)
        self.assertIn(self.DT, plan["always"])
        self.assertAlmostEqual(plan["adr_minutes"], 61.8 / 60.0)
        self.assertEqual([u["target"] for u in plan["selected"]], ["fast"])
        self.assertEqual([u["target"] for u in alone["selected"]], ["fast", "norec"])  # the same budget without the ADR's tests

    def test_an_adr_cost_above_the_budget_leaves_every_binary_out_and_cuts_no_adr_test(self):
        with tempfile.TemporaryDirectory() as d:
            plan = pm.make_plan(d, self.INFO, adr_index(("d", "dt")), TIMES, [], 1.0, 60)
        self.assertEqual(plan["selected"], [])
        self.assertEqual([u["target"] for u in plan["left"]], ["fast", "norec", "slow", "ct", "bt"])
        self.assertIn(self.DT, plan["always"])

    def test_the_dry_run_prints_their_recorded_minutes(self):
        with tempfile.TemporaryDirectory() as d:
            plan = pm.make_plan(d, self.INFO, adr_index(("d", "dt")), TIMES, [], 120.0 / 60.0, 60)
            alone = pm.make_plan(d, dict(self.INFO, adrs=[]), IDX, TIMES, [], 120.0 / 60.0, 60)
        self.assertIn("ADR tests (charged to the budget first, never cut): 1 commands, 1.0 recorded minutes, with 1.8 s of "
                      "build counted per command; 1.0 of the 2.0 budget minutes left for the diff-selected binaries",
                      printed(self.INFO, plan))
        self.assertNotIn("ADR tests", printed(dict(self.INFO, adrs=[]), alone))


class NamedAfterSource(unittest.TestCase):
    """ADR 0776, point 3 (row T3): inside a touched crate the binaries named after a changed source file come first."""

    INFO = {"sha": "0" * 40, "first": "1" * 40, "branch": "f001-work", "items": [], "adrs": [], "files": ["crates/a/src/slow.rs"]}

    def test_the_rule_of_naming(self):
        files = ["crates/a/src/slow.rs", "crates/a/src/lib.rs", "crates/a/x/src/main.rs", "crates/b/src/grid/mod.rs",
                 "crates/a/tests/fast.rs", "crates/d/src/notes.md", "docs/x.rs", "crates/c/src/bin/tool.rs"]
        stems = pm.source_stems(files, CRATES)
        self.assertEqual(stems, {("a", "slow"), ("b", "grid"), ("c", "tool")})
        for crate, target, want in (("a", "slow", True), ("a", "slow_f1", True), ("a", "slow_f1_records", True),
                                    ("a", "slowly", False), ("a", "fast", False), ("b", "slow", False), ("b", "grid_x", True),
                                    ("a-x", "slow", False)):
            self.assertEqual(pm.named_after(crate, target, stems), want, (crate, target))

    def test_the_named_binaries_come_first_in_their_group(self):
        units = pm.binary_units(IDX, {"a"}, TIMES, 60)
        self.assertEqual([u["target"] for u in pm.order_group(units, {"fast"}, {("a", "slow")})], ["slow", "fast", "norec"])
        self.assertEqual([u["target"] for u in pm.order_group(units, {"fast"})], ["fast", "norec", "slow"])

    def test_the_plan_selects_them_first_and_says_so(self):
        with tempfile.TemporaryDirectory() as d:
            plan = pm.make_plan(d, self.INFO, IDX, TIMES, [], 575.0 / 60.0, 60)
            short = pm.make_plan(d, self.INFO, IDX, TIMES, [], 100.0 / 60.0, 60)
        self.assertEqual([u["target"] for u in plan["selected"]], ["slow", "fast"])  # the unchanged plan selected fast, norec
        self.assertEqual([u["target"] for u in plan["left"]], ["norec", "ct", "bt"])
        self.assertIn("named after a changed source file: 1 binaries, first in their crate's group; 0 of them left out by the "
                      "budget", printed(self.INFO, plan))
        self.assertIn("named after a changed source file: 1 binaries, first in their crate's group; 1 of them left out by the "
                      "budget", printed(self.INFO, short))


class UnchangedOutput(TempRepo):
    """ADR 0776, row U: the ledger's columns and the merge line of a run that passes at the first attempt are unchanged."""

    def test_the_columns(self):
        self.assertEqual(pm.COLUMNS, ["date", "kind", "commit", "branch", "items", "adrs", "result", "minutes", "left_out",
                                      "left_out_minutes", "detail"])

    def test_the_new_keys_of_a_plan_add_nothing_to_the_merge_line(self):
        sha = self.merge("f001-work", ["crates/a/src/slow.rs"])
        plan = self.plan(CHECK, SLOW, adr_units=[{"cmd": SLOW, "seconds": 500.0, "cost": 501.8}], adr_minutes=501.8 / 60.0,
                         named=[("a", "slow")], named_left=0)
        code, out = self.run_plan(plan, Runner())
        self.assertEqual(code, 0, out)
        (row,) = self.rows()
        self.assertEqual(row, {"date": pm.today(), "kind": "merge", "commit": sha[:10], "branch": "f001-work", "items": "F-001",
                               "adrs": "", "result": "pass", "minutes": "1.0", "left_out": "0", "left_out_minutes": "0.0",
                               "detail": f"exit=0;mutations=0;cmds={CHECK} | {SLOW}"})


if __name__ == "__main__":
    unittest.main()
