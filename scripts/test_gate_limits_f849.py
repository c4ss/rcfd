"""Tests of the per-test time limits of the full gate (ADR 0740, F-849): the table scripts/gate_limits.tsv, gate_test.py --limits,
gate_limit_check.py, the re-run budget of gate_attribute.py and the text of gate.yml.
Run: python -m unittest scripts/test_gate_limits_f849.py -v
The end-to-end tests (E1, E1n) build a tiny cargo crate in a temp folder with `cargo test --release`; they run only with
GATE_TEST_E2E=1."""
import io
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
from contextlib import redirect_stdout

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import gate_attribute as ga  # noqa: E402
import gate_limit_check as glc  # noqa: E402
import gate_test as gt  # noqa: E402

ROOT = os.path.dirname(HERE)
TABLE = os.path.join(HERE, "gate_limits.tsv")
GATE = os.path.join(HERE, "gate_test.py")
GATE_YML = os.path.join(ROOT, ".github", "workflows", "gate.yml")
A3 = ("immersed_solid_f219", "a3_two_d_two_strouhal_number")
A2T = ("immersed_unsteady_force_f346", "a2t_c1_two_d_two_on_t")
HEAD = "binary\ttest\tlimit_s\tadr\n"


class Tmp(unittest.TestCase):
    def setUp(self):
        self.d = tempfile.mkdtemp(prefix="f849_")

    def tearDown(self):
        shutil.rmtree(self.d, ignore_errors=True)

    def w(self, name, text):
        f = os.path.join(self.d, *name.split("/"))
        os.makedirs(os.path.dirname(f), exist_ok=True)
        with open(f, "w", encoding="utf-8", newline="\n") as h:
            h.write(text)
        return f


class Table(Tmp):
    def test_g2_the_committed_table_has_the_two_rows_and_each_is_a_test_that_is_not_ignored(self):
        self.assertEqual(gt.read_limits(TABLE), {A3: 10800.0, A2T: 10800.0})
        with open(TABLE, encoding="utf-8") as h:
            rows = [ln.split("\t") for ln in h.read().splitlines() if ln and not ln.startswith("#")][1:]
        self.assertEqual([r[3] for r in rows], ["0740", "0740"])
        for binary, test in (A3, A2T):
            with open(os.path.join(ROOT, "crates", "rcfd-fv", "tests", binary + ".rs"), encoding="utf-8") as h:
                src = h.read().splitlines()
            at = [i for i, ln in enumerate(src) if ln.startswith(f"fn {test}(")]
            self.assertEqual(len(at), 1, test)
            attrs = []
            i = at[0] - 1
            while i >= 0 and src[i].startswith(("#[", "///")):
                attrs.append(src[i])
                i -= 1
            self.assertIn("#[test]", attrs, test)
            self.assertFalse([a for a in attrs if a.startswith("#[ignore")], test)

    def test_l1_comments_and_empty_lines_are_skipped(self):
        f = self.w("t.tsv", "# why\n\n" + HEAD + "# a row follows\nbin_a\tm::t\t90\t0740\n\nbin_b\tt\t1.5\t0001\n")
        self.assertEqual(gt.read_limits(f), {("bin_a", "m::t"): 90.0, ("bin_b", "t"): 1.5})

    def test_l1_a_bad_table_is_refused_with_its_line(self):
        bad = {
            "a missing column": (HEAD + "bin_a\tt\t90\n", "line 2"),
            "a limit that is not a number": (HEAD + "bin_a\tt\t90\t0740\nbin_b\tt\tlong\t0740\n", "line 3"),
            "a limit of 0": (HEAD + "bin_a\tt\t0\t0740\n", "line 2"),
            "a negative limit": (HEAD + "bin_a\tt\t-5\t0740\n", "line 2"),
            "an infinite limit": (HEAD + "bin_a\tt\tinf\t0740\n", "line 2"),
            "a test named twice": ("# c\n" + HEAD + "bin_a\tt\t90\t0740\nbin_a\tt\t95\t0740\n", "line 4"),
            "no header": ("bin_a\tt\t90\t0740\n", "line 1"),
            "an empty test name": (HEAD + "bin_a\t\t90\t0740\n", "line 2"),
        }
        for what, (text, line) in bad.items():
            with self.assertRaises(SystemExit, msg=what) as e:
                gt.read_limits(self.w("bad.tsv", text))
            self.assertIn(line, str(e.exception), what)


class Limit(unittest.TestCase):
    TABLE = {A3: 10800.0, A2T: 7200.0}
    # the cases of test_gate_test.Clamp: (recorded seconds, factor, min, max) -> limit
    CLAMP = [
        ((None, 5, 600, 3600), 3600), ((0.0, 5, 600, 3600), 600), ((0.07, 5, 600, 3600), 600), ((120, 5, 600, 3600), 600),
        ((121, 5, 600, 3600), 605), ((300, 5, 600, 3600), 1500), ((720, 5, 600, 3600), 3600), ((820, 5, 600, 3600), 3600),
        ((1e12, 5, 600, 3600), 3600), ((10, 3, 10, 10), 10), ((None, 5, 10, 10), 10), ((100, 2, 0, 1000), 200),
        ((100, 5, 600, 0), None), ((None, 5, 600, 0), None), ((100, 5, 600, -1), None),
    ]

    def test_l2_a_named_test_has_the_limit_of_its_row_whatever_the_options_give(self):
        for args, _ in self.CLAMP:
            if args[3] > 0:
                self.assertEqual(gt.test_limit(A3, *args, self.TABLE), 10800.0, args)
                self.assertEqual(gt.test_limit(A2T, *args, self.TABLE), 7200.0, args)
        # the gate's two forms: one job (no record, defaults) and a shard (a record, --timeout-min 3600)
        self.assertEqual(gt.test_limit(A3, None, 5.0, 600.0, 3600.0, self.TABLE), 10800.0)
        self.assertEqual(gt.test_limit(A3, 2065.57, 5.0, 3600.0, 3600.0, self.TABLE), 10800.0)

    def test_l2_every_other_test_has_the_limit_it_had(self):
        self.assertEqual(len(self.CLAMP), 15)
        others = [("some_binary", "some_test"), (A3[0], "a1_a2_two_d_one_drag_and_lift_on_m3"), ("another_binary", A3[1]),
                  (A2T[0], A3[1]), (A3[1], A3[0])]
        for key in others:
            for args, want in self.CLAMP:
                self.assertEqual(gt.test_limit(key, *args, self.TABLE), want, (key, args))
                self.assertEqual(gt.test_limit(key, *args, self.TABLE), gt.limit_s(*args), (key, args))
        for args, want in self.CLAMP:  # no table and an empty one: the limit of before for every test, the two included
            self.assertEqual(gt.test_limit(A3, *args), want, args)
            self.assertEqual(gt.test_limit(A3, *args, None), want, args)
            self.assertEqual(gt.test_limit(A3, *args, {}), want, args)

    def test_l2_timeout_max_0_turns_every_limit_off(self):
        for hi in (0, -1):
            self.assertIsNone(gt.test_limit(A3, None, 5, 600, hi, self.TABLE))
            self.assertIsNone(gt.test_limit(A3, 100.0, 5, 600, hi, self.TABLE))
            self.assertIsNone(gt.test_limit(("b", "t"), 100.0, 5, 600, hi, self.TABLE))


def times(*rows):
    return "binary\ttest\toutcome\tseconds\tpeak_mb\n" + "".join("\t".join(str(c) for c in r) + "\n" for r in rows)


OTHER = ("eps", "quick", "ok", "0.01", 5)


class Check(Tmp):
    """L3: gate_limit_check.py on fixture folders."""

    def gate(self, name, a3=("ok", "2065.57"), a2t=("ok", "2247.16"), shards=0, extra=()):
        ra3 = [(*A3, *a3, 146)] if a3 else []
        ra2t = [(*A2T, *a2t, 149)] if a2t else []
        if shards:
            self.w(f"{name}/gate-windows-latest-abc-shard-1/times.tsv", times(OTHER, *ra2t))
            self.w(f"{name}/gate-windows-latest-abc-shard-2/times.tsv", times(*ra3, *extra))
            for k in range(3, shards + 1):
                self.w(f"{name}/gate-windows-latest-abc-shard-{k}/times.tsv", times(OTHER))
        else:
            self.w(f"{name}/gate-windows-latest-abc/times.tsv", times(OTHER, *ra3, *ra2t, *extra))
        # the hooks step's times are not read, whatever they hold
        self.w(f"{name}/gate-windows-latest-abc{'-shard-1' if shards else ''}/times_hooks.tsv", times((*A3, "TIMEOUT", "1", 1)))
        return os.path.join(self.d, name)

    def run_check(self, *gates, extra=()):
        buf = io.StringIO()
        with redirect_stdout(buf):
            code = glc.main(["--limits", TABLE, *extra, *gates])
        return code, buf.getvalue()

    def test_l3_three_good_gates_of_one_job(self):
        code, out = self.run_check(self.gate("g1"), self.gate("g2", ("ok", "3060.22"), ("ok", "3230.17")), self.gate("g3"))
        self.assertEqual(code, 0, out)
        self.assertEqual(out.count("  met   "), 6, out)
        self.assertNotIn("MISSED", out)
        self.assertIn("gate_limit_check: met (2 tests, 3 gates)", out)
        self.assertIn("3230 s of 5400 s (0.5 of the limit 10800 s)", out)

    def test_l3_three_good_sharded_gates(self):
        code, out = self.run_check(*[self.gate(f"s{k}", shards=4) for k in range(3)])
        self.assertEqual(code, 0, out)
        self.assertIn("4 times file(s)", out)
        self.assertEqual(out.count("  met   "), 6, out)

    def test_l3_exactly_half_the_limit_is_met(self):
        code, out = self.run_check(self.gate("g1", ("ok", "5400.00")), self.gate("g2"), self.gate("g3"))
        self.assertEqual(code, 0, out)

    def defect(self, text, **kw):
        bad = self.gate("bad", **kw)
        for order in ((self.gate("g1"), self.gate("g2"), bad), (bad, self.gate("g1"), self.gate("g2"))):
            code, out = self.run_check(*order)
            self.assertEqual(code, 1, out)
            self.assertEqual(out.count("MISSED"), 2, out)  # the line of the test and the closing line
            line = next(ln for ln in out.splitlines() if ln.startswith("  MISSED"))
            self.assertIn(text, line)
            self.assertIn(bad, line)
            self.assertEqual(out.count("  met   "), 5, out)
        return out

    def test_l3_seconds_above_half_the_limit(self):
        out = self.defect(f"{A3[0]}::{A3[1]}", a3=("ok", "5400.51"))
        self.assertIn("5401 s is above 5400 s", out)

    def test_l3_a_time_out(self):
        self.assertIn("outcome TIMEOUT after 10800 s", self.defect(f"{A2T[0]}::{A2T[1]}", a2t=("TIMEOUT", "10800.19")))

    def test_l3_a_failure(self):
        self.assertIn("outcome FAILED", self.defect(f"{A3[0]}::{A3[1]}", a3=("FAILED", "1800.00")))

    def test_l3_an_ignored_test(self):
        self.assertIn("outcome ignored", self.defect(f"{A3[0]}::{A3[1]}", a3=("ignored", "0")))

    def test_l3_a_missing_row(self):
        self.assertIn("no row of this test", self.defect(f"{A2T[0]}::{A2T[1]}", a2t=None))
        # another test of the same binary, or the same test name in another binary, does not stand in for it
        self.assertIn("no row of this test", self.defect(
            f"{A3[0]}::{A3[1]}", a3=None, extra=[(A3[0], "a1_a2_two_d_one_drag_and_lift_on_m3", "ok", "1158.65", 256),
                                                  ("immersed_other", A3[1], "ok", "100.00", 10)]))

    def test_l3_a_test_in_two_shards(self):
        out = self.defect(f"{A3[0]}::{A3[1]}", shards=4, extra=[(*A3, "ok", "2000.00", 146)])
        self.assertIn("2 rows of this test", out)

    def test_l3_two_gates_only_and_a_gate_without_times(self):
        code, out = self.run_check(self.gate("g1"), self.gate("g2"))
        self.assertEqual(code, 1, out)
        self.assertIn("MISSED: 2 gate(s) given, 3 are needed", out)
        self.assertEqual(out.count("  met   "), 4, out)
        os.makedirs(os.path.join(self.d, "empty"))
        code, out = self.run_check(self.gate("g1"), self.gate("g2"), os.path.join(self.d, "empty"))
        self.assertEqual(code, 1, out)
        self.assertIn("no times.tsv", out)
        code, out = self.run_check()
        self.assertEqual(code, 1, out)


ATTR_TSV = times(("rcfd_c", "tests::slow", "TIMEOUT", "10800.00", 20), ("alpha_f010", "m::inner", "FAILED", "1.50", 10),
                 ("alpha_f010", "alpha_one", "ok", "0.10", 10), ("alpha_f010", "alpha_two", "FAILED", "2.50", 10),
                 ("ghost", "gone", "FAILED", "0.1", 0))
ATTR_LOG = ("     Running tests/alpha_f010.rs (/w/target/release/deps/alpha_f010-0123abcd)\n"
            "     Running unittests src/lib.rs (/w/target/release/deps/rcfd_c-77aa)\n")
ATTR_INDEX = {
    "binaries": {"alpha_f010": [{"crate": "rcfd-a", "target": "alpha_f010", "kind": "test", "file": "crates/rcfd-a/tests/alpha_f010.rs"}],
                 "rcfd_c": [{"crate": "rcfd-c", "target": "rcfd_c", "kind": "lib", "file": "crates/rcfd-c/src/lib.rs"}]},
    "crates": {"rcfd-a": {"dir": "crates/rcfd-a", "deps": []}, "rcfd-c": {"dir": "crates/rcfd-c", "deps": []}},
    "tests": [], "items": {},
}
ENV = dict(os.environ, GIT_AUTHOR_NAME="t", GIT_AUTHOR_EMAIL="t@example.invalid", GIT_COMMITTER_NAME="t",
           GIT_COMMITTER_EMAIL="t@example.invalid", GIT_CONFIG_NOSYSTEM="1")
SLOW, INNER, TWO = "tests::slow", "m::inner", "alpha_two"


class Rerun(Tmp):
    """L4: the re-runs of gate_attribute.py with a fake runner."""

    def setUp(self):
        super().setUp()
        self.repo = os.path.join(self.d, "repo")
        self.w("repo/crates/rcfd-a/src/lib.rs", "// lib\n")
        for args in (("init", "-q", "-b", "main"), ("add", "-A"), ("commit", "-q", "-m", "initial")):
            subprocess.run(["git", *args], cwd=self.repo, env=ENV, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        self.tsv = self.w("out/times.tsv", ATTR_TSV)
        self.log = self.w("out/test.log", ATTR_LOG)
        self.index = self.w("out/index.json", json.dumps(ATTR_INDEX))
        self.limits = self.w("out/limits.tsv", HEAD + "rcfd_c\ttests::slow\t10800\t0740\n")

    def attr(self, *extra, outcomes=None):
        calls = []

        def fake(exe, test, cwd, timeout):
            calls.append((test, timeout))
            return (outcomes or {}).get(test, "FAILED"), 0.5

        js = os.path.join(self.d, "out", "attribution.json")
        if os.path.exists(js):
            os.remove(js)
        old = ga.run_test
        ga.run_test = fake
        try:
            code = ga.main(["attribute", "--tsv", self.tsv, "--log", self.log, "--index", self.index, "--repo", self.repo, "--base", "",
                            "--json", js, "--summary", os.path.join(self.d, "out", "summary.md"), "--rerun", *extra])
        finally:
            ga.run_test = old
        self.assertEqual(code, 0)
        with open(js, encoding="utf-8") as h:  # the JSON is written in every case
            res = json.load(h)
        return calls, res, {f["test"]: (f["rerun"]["outcome"], f["flaky"]) for f in res["failures"]}

    def test_l4_without_the_options_the_re_runs_are_those_of_before(self):
        calls, res, got = self.attr(outcomes={INNER: "ok"})
        self.assertEqual(calls, [(SLOW, 3600.0), (INNER, 3600.0), (TWO, 3600.0)])  # file order, --rerun-timeout each
        self.assertNotIn("rerun_budget_s", res)
        self.assertEqual(got, {SLOW: ("FAILED", False), INNER: ("ok", True), TWO: ("FAILED", False),
                               "gone": ("not run (no executable or crate)", False)})
        self.assertEqual([f["test"] for f in res["failures"]], [SLOW, INNER, TWO, "gone"])
        calls, _, _ = self.attr("--rerun-timeout", "77")
        self.assertEqual(calls, [(SLOW, 77.0), (INNER, 77.0), (TWO, 77.0)])

    def test_l4_the_table_gives_the_limit_of_the_named_test_only(self):
        calls, res, _ = self.attr("--limits", self.limits)
        self.assertEqual(calls, [(SLOW, 10800.0), (INNER, 3600.0), (TWO, 3600.0)])  # still file order: no budget
        self.assertNotIn("rerun_budget_s", res)

    def test_l4_a_budget_that_holds_all_runs_the_shortest_limit_first(self):
        calls, res, got = self.attr("--limits", self.limits, "--rerun-budget", "12000", outcomes={SLOW: "ok"})
        self.assertEqual([c[0] for c in calls], [INNER, TWO, SLOW])
        self.assertEqual([c[1] for c in calls[:2]], [3600.0, 3600.0])
        self.assertAlmostEqual(calls[2][1], 10800.0, delta=5.0)
        self.assertEqual(res["rerun_budget_s"], 12000.0)
        self.assertEqual(got[SLOW], ("ok", True))
        self.assertEqual([f["test"] for f in res["failures"]], [SLOW, INNER, TWO, "gone"])  # the record keeps the file order

    def test_l4_a_budget_below_the_limit_cuts_the_re_run_and_says_so(self):
        calls, res, got = self.attr("--limits", self.limits, "--rerun-budget", "5000", outcomes={SLOW: "TIMEOUT", INNER: "TIMEOUT"})
        self.assertAlmostEqual(dict(calls)[SLOW], 5000.0, delta=5.0)
        self.assertRegex(got[SLOW][0], r"^TIMEOUT \(cut at (4999|5000) s by the re-run budget; its limit is 10800 s\)$")
        self.assertFalse(got[SLOW][1])
        self.assertEqual(got[INNER], ("TIMEOUT", False))  # timed out at its own limit: the plain outcome
        with open(os.path.join(self.d, "out", "summary.md"), encoding="utf-8") as h:
            self.assertIn("by the re-run budget; its limit is 10800 s", h.read())

    def test_l4_a_budget_that_is_used_up_runs_nothing_and_still_writes_the_record(self):
        calls, res, got = self.attr("--limits", self.limits, "--rerun-budget", "59")
        self.assertEqual(calls, [])
        for t in (SLOW, INNER, TWO):
            self.assertEqual(got[t], ("not run (re-run budget used up)", False))
        self.assertEqual(got["gone"], ("not run (no executable or crate)", False))

    def test_l4_the_budget_is_spent_in_the_order_of_the_limits(self):
        todo = [(0, "e", SLOW, "c", 10800.0), (1, "e", INNER, "c", 3600.0), (2, "e", TWO, "c", 3600.0)]
        for budget, want_calls, want in (
            (5000.0, [(INNER, 3600.0), (TWO, 3600.0), (SLOW, 1300.0)],
             {0: "TIMEOUT (cut at 1300 s by the re-run budget; its limit is 10800 s)", 1: "ok", 2: "TIMEOUT"}),
            (3650.0, [(INNER, 3600.0), (TWO, 3550.0)],
             {0: "not run (re-run budget used up)", 1: "ok", 2: "TIMEOUT (cut at 3550 s by the re-run budget; its limit is 3600 s)"}),
            (3759.0, [(INNER, 3600.0), (TWO, 3600.0)], {0: "not run (re-run budget used up)", 1: "ok", 2: "TIMEOUT"}),
            (3760.0, [(INNER, 3600.0), (TWO, 3600.0), (SLOW, 60.0)],
             {0: "TIMEOUT (cut at 60 s by the re-run budget; its limit is 10800 s)", 1: "ok", 2: "TIMEOUT"}),
        ):
            now, calls = [1000.0], []

            def run(exe, test, cwd, timeout):
                calls.append((test, timeout))
                took = 100.0 if test == INNER else timeout  # INNER passes after 100 s; the others hang
                now[0] += took
                return ("ok" if test == INNER else "TIMEOUT"), took

            out = ga.rerun_all(todo, budget, run=run, clock=lambda: now[0])
            self.assertEqual(calls, want_calls, budget)
            self.assertEqual({k: v["outcome"] for k, v in out.items()}, want, budget)
        # without a budget: the given order, each with its own limit, however long they take
        calls = []
        out = ga.rerun_all(todo, run=lambda e, t, c, lim: (calls.append((t, lim)), ("TIMEOUT", lim))[1])
        self.assertEqual(calls, [(SLOW, 10800.0), (INNER, 3600.0), (TWO, 3600.0)])
        self.assertEqual({v["outcome"] for v in out.values()}, {"TIMEOUT"})

    def test_l4_the_budget_formula(self):
        b = ga.rerun_budget
        self.assertEqual(b(1000.0 + 6600.0, 1000.0, 340.0), 12000.0)  # 12 900 s left: the cap
        self.assertEqual(b(1000.0 + 8700.0, 1000.0, 340.0), 10800.0)  # the latest start that holds the long re-run whole
        self.assertEqual(b(1000.0 + 12900.0, 1000.0, 340.0), 6600.0)
        self.assertEqual(b(1000.0 + 12900.0, 1000.0, 340.0, 30.0), 4800.0)  # the bisect's minutes come off
        self.assertEqual(b(1000.0 + 19500.0, 1000.0, 340.0), 0.0)
        self.assertEqual(b(1000.0 + 30000.0, 1000.0, 340.0), 0.0)  # never below 0
        self.assertEqual(b(1000.0 + 100.0, 1000.0, 40.0), 1400.0)
        self.assertEqual(b(5.0, None, 340.0), 1500.0)  # the start of the job is not known
        self.assertEqual((ga.RERUN_CAP_S, ga.RERUN_RESERVE_S, ga.RERUN_FALLBACK_S, ga.RERUN_LEAST_S), (12000.0, 900.0, 1500.0, 60.0))
        self.assertEqual(ga.read_start(self.w("start", "1763546400\n")), 1763546400.0)
        self.assertIsNone(ga.read_start(os.path.join(self.d, "missing")))
        self.assertIsNone(ga.read_start(self.w("empty", "")))
        self.assertIsNone(ga.read_start(self.w("text", "soon\n")))

    def test_l4_the_budget_from_the_start_file(self):
        start = self.w("start", f"{time.time() - 12900.0:.0f}\n")
        _, res, _ = self.attr("--limits", self.limits, "--job-start-file", start, "--job-minutes", "340")
        self.assertAlmostEqual(res["rerun_budget_s"], 6600.0, delta=30.0)
        _, res, _ = self.attr("--job-start-file", start, "--job-minutes", "340", "--bisect", "--bisect-minutes", "30", "--probe-cmd", "exit 0")
        self.assertAlmostEqual(res["rerun_budget_s"], 4800.0, delta=30.0)
        _, res, _ = self.attr("--job-start-file", os.path.join(self.d, "missing"))
        self.assertEqual(res["rerun_budget_s"], 1500.0)
        _, res, _ = self.attr("--job-start-file", start, "--rerun-budget", "321")  # the stated budget wins
        self.assertEqual(res["rerun_budget_s"], 321.0)


class Workflow(unittest.TestCase):
    """L5: gate.yml as text."""

    def setUp(self):
        with open(GATE_YML, encoding="utf-8") as h:
            text = h.read().replace("\r\n", "\n")
        self.gate = text.split("\n  gate:\n", 1)[1].split("\n  # One issue per", 1)[0]
        self.steps = re.split(r"\n(?=      - )", self.gate)[1:]

    def step(self, needle):
        hits = [s for s in self.steps if ("name: " + needle) in s]
        self.assertEqual(len(hits), 1, needle)
        return "\n".join(ln for ln in hits[0].split("\n") if not ln.lstrip().startswith("#"))

    def test_l5_the_tests_step_passes_the_table_in_the_command_both_forms_share(self):
        tests = self.step("tests (scripts")
        self.assertEqual(tests.count("--limits"), 1)
        line = next(ln for ln in tests.split("\n") if "--limits" in ln)
        self.assertIn("python scripts/gate_test.py --limits scripts/gate_limits.tsv ", line)
        self.assertIn('"${args[@]}"', line)  # the one command of a run of one job and of a shard
        self.assertNotIn("--limits", self.step("tests with the test-hooks"))
        self.assertTrue(os.path.isfile(os.path.join(ROOT, "scripts", "gate_limits.tsv")))

    def test_l5_the_job_start_is_recorded_first_and_the_attribution_reads_it(self):
        job = int(re.search(r"\n    timeout-minutes: (\d+)\n", self.gate).group(1))
        self.assertEqual(job, 340)
        fmt = self.step("fmt")
        body = fmt.split("run: |\n", 1)[1].split("\n")
        self.assertEqual(body[0].strip(), 'date +%s > "$RUNNER_TEMP/gate_job_start"')
        first_run = next(s for s in self.steps if "run:" in s)
        self.assertIn("name: fmt", first_run)  # no step with a command runs before it
        attr = self.step("failure attribution")
        self.assertIn(' --rerun --limits scripts/gate_limits.tsv --job-start-file "$RUNNER_TEMP/gate_job_start" --job-minutes 340 ', attr)
        self.assertEqual(int(re.search(r"--job-minutes (\d+)", attr).group(1)), job)
        step_s = 60 * int(re.search(r"timeout-minutes: (\d+)", attr).group(1))
        self.assertEqual(step_s, 14400)
        bisect_default = 60 * int(re.search(r"BISECT_MINUTES: \$\{\{ inputs\.bisect_minutes \|\| (\d+) \}\}", attr).group(1))
        self.assertEqual(bisect_default, 1800)
        self.assertGreaterEqual(step_s, ga.RERUN_CAP_S + bisect_default)
        self.assertGreaterEqual(ga.RERUN_CAP_S, max(gt.read_limits(TABLE).values()))
        self.assertGreater(60 * job, step_s)


E2E_LIB = """
#[test]
fn long_a() {
    std::thread::sleep(std::time::Duration::from_secs(20));
}

#[test]
fn long_b() {
    std::thread::sleep(std::time::Duration::from_secs(20));
}

#[test]
fn quick() {}
"""


@unittest.skipUnless(os.environ.get("GATE_TEST_E2E") == "1", "set GATE_TEST_E2E=1 (builds a crate with cargo --release)")
class EndToEnd(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.dir = tempfile.mkdtemp(prefix="gate_limits_e2e_")
        os.makedirs(os.path.join(cls.dir, "src"))
        with open(os.path.join(cls.dir, "Cargo.toml"), "w") as f:
            f.write('[package]\nname = "lim"\nversion = "0.1.0"\nedition = "2021"\n\n[workspace]\n')
        with open(os.path.join(cls.dir, "src", "lib.rs"), "w") as f:
            f.write(E2E_LIB)
        cls.table = os.path.join(cls.dir, "limits.tsv")
        with open(cls.table, "w", encoding="utf-8", newline="\n") as f:
            f.write(HEAD + "lim\tlong_a\t120\t0740\n")

    @classmethod
    def tearDownClass(cls):
        shutil.rmtree(cls.dir, ignore_errors=True)

    def gate(self, *flags):
        tsv = os.path.join(self.dir, "times.tsv")
        if os.path.exists(tsv):
            os.remove(tsv)
        env = dict(os.environ, CARGO_TARGET_DIR=os.path.join(self.dir, "target"))
        r = subprocess.run([sys.executable, GATE, "--no-doc", "--tsv", tsv, "--timeout-min", "5", "--timeout-max", "5", *flags],
                           cwd=self.dir, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        out = r.stdout.decode("utf-8", "replace")
        print(f"\n--- flags {flags}: exit {r.returncode}\n{out}")
        with open(tsv) as f:
            rows = {ln.split("\t")[1]: ln.split("\t") for ln in f.read().splitlines()[1:]}
        return r.returncode, out, rows

    def test_e1_the_named_test_runs_to_its_end_and_the_other_is_stopped(self):
        code, out, rows = self.gate("--limits", self.table)
        self.assertNotEqual(code, 0)
        self.assertEqual(rows["long_a"][2], "ok")
        self.assertGreaterEqual(float(rows["long_a"][3]), 20.0)
        self.assertEqual(rows["long_b"][2], "TIMEOUT")
        self.assertLessEqual(float(rows["long_b"][3]), 15.0)
        self.assertEqual(rows["quick"][2], "ok")
        self.assertIn(f"gate_test: limits {self.table}: 1 rows, 1 name a test of this run", out)
        self.assertIn("test long_b ... FAILED (timeout after 5 s)", out)

    def test_e1n_without_the_table_both_are_stopped(self):
        code, out, rows = self.gate()
        self.assertNotEqual(code, 0)
        self.assertEqual((rows["long_a"][2], rows["long_b"][2], rows["quick"][2]), ("TIMEOUT", "TIMEOUT", "ok"))
        self.assertNotIn("gate_test: limits", out)


if __name__ == "__main__":
    unittest.main()
