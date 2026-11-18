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
        self.assertTrue(all(r["kind"] in ("merge", "gate", "leftout", "ci") for r in rows))


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


if __name__ == "__main__":
    unittest.main()
