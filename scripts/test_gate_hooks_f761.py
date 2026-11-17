"""Tests of the feature-gated step of the gate (ADR 0681, F-761): gate_test.py --no-doc, several tsv/log files in
gate_attribute.py, the primary ADR in the attribution, the probe with a feature, and the structure of gate.yml.
Run: python -m unittest scripts/test_gate_hooks_f761.py -v"""
import os
import re
import subprocess
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import gate_attribute as ga  # noqa: E402
import gate_test as gt  # noqa: E402

GATE_YML = os.path.join(HERE, "..", ".github", "workflows", "gate.yml")


class Done:
    def __init__(self, code=0, out=b""):
        self.returncode, self.stdout = code, out


class NoDoc(unittest.TestCase):
    def cargo_calls(self, *argv):
        calls = []

        def fake(cmd, **kw):
            calls.append(list(cmd))
            return Done()

        old_run, old_argv, old_err = gt.subprocess.run, sys.argv, sys.stderr
        gt.subprocess.run, sys.argv = fake, ["gate_test.py", *argv]
        sys.stderr = open(os.devnull, "w")
        try:
            code = gt.main()
        finally:
            sys.stderr.close()
            gt.subprocess.run, sys.argv, sys.stderr = old_run, old_argv, old_err
        return code, calls

    def test_n1_no_doc_skips_the_doc_tests_and_keeps_the_selection(self):
        code, calls = self.cargo_calls("--no-doc", "--", "-p", "rcfd-project", "--features", "test-hooks", "--test", "edit_lock_f755")
        self.assertEqual(code, 0)
        self.assertEqual(len(calls), 1)
        self.assertEqual(calls[0][:8], ["cargo", "test", "-p", "rcfd-project", "--features", "test-hooks", "--test", "edit_lock_f755"])
        self.assertNotIn("--doc", calls[0])

    def test_n2_default_still_runs_the_doc_tests(self):
        code, calls = self.cargo_calls()
        self.assertEqual(code, 0)
        self.assertEqual(len(calls), 2)
        self.assertIn("--workspace", calls[0])
        self.assertEqual(calls[1], ["cargo", "test", "--workspace", "--release", "--no-fail-fast", "--doc"])


class Attribution(unittest.TestCase):
    def setUp(self):
        self.d = tempfile.mkdtemp(prefix="f761_")

    def w(self, name, text):
        f = os.path.join(self.d, name)
        with open(f, "w", encoding="utf-8", newline="\n") as h:
            h.write(text)
        return f

    def test_a1_several_tsv_and_logs_are_merged(self):
        t1 = self.w("a.tsv", "binary\ttest\toutcome\tseconds\tpeak_mb\nalpha\tone\tFAILED\t1.0\t1\n")
        t2 = self.w("b.tsv", "binary\ttest\toutcome\tseconds\tpeak_mb\nedit_lock_f755\ts1\tFAILED\t2.0\t1\nedit_lock_f755\ts2\tok\t1\t1\n")
        l1 = self.w("a.log", "gate_test: build failed (exit 101)\n")
        l2 = self.w("b.log", "     Running tests/edit_lock_f755.rs (/w/target/release/deps/edit_lock_f755-0a1b.exe)\n"
                             "gate_test: build failed (exit 7)\n")
        fails, exes, build, docs = ga.read_failures([t1, t2, os.path.join(self.d, "missing.tsv")], [l1, l2])
        self.assertEqual([(f["binary"], f["test"]) for f in fails], [("alpha", "one"), ("edit_lock_f755", "s1")])
        self.assertEqual(exes["edit_lock_f755"][0]["exe"], "/w/target/release/deps/edit_lock_f755-0a1b.exe")
        self.assertEqual(build, "gate_test: build failed (exit 101); gate_test: build failed (exit 7)")
        self.assertEqual(docs, [])
        # one path still works, and none
        self.assertEqual(len(ga.read_failures(t1, l1)[0]), 1)
        self.assertEqual(ga.read_failures(None, None), ([], {}, None, []))

    def test_a2_primary_adr_in_the_table_cell(self):
        rec = {"adrs": ["0135", "0669", "0676"], "primary_adr": "0676"}
        self.assertEqual(ga.adr_cell(rec), "**0676**, 0135, 0669")
        # no primary ADR (an older index): the cell is what it was
        self.assertEqual(ga.adr_cell({"adrs": ["0100", "0101"]}), "0100, 0101")
        self.assertEqual(ga.adr_cell({"adrs": [], "primary_adr": None}), "unmapped")

    def test_a3_cargo_probe_with_and_without_a_feature(self):
        rec = {"kind": "test", "target": "edit_lock_f755", "crate": "rcfd-project"}
        entry = {"test": "s1"}
        plain = ga.cargo_probe(entry, rec)
        self.assertEqual(plain, "cargo test -p rcfd-project --release --test edit_lock_f755 -- --exact s1 --test-threads=1")
        self.assertNotIn("--features", plain)
        self.assertIn("-p rcfd-project --features test-hooks --release --test edit_lock_f755", ga.cargo_probe(entry, rec, "test-hooks"))


class Workflow(unittest.TestCase):
    def setUp(self):
        with open(GATE_YML, encoding="utf-8") as h:
            self.text = h.read().replace("\r\n", "\n")
        # steps of the job `gate`: blocks that start with "      - " at six spaces
        gate = self.text.split("\n  gate:\n", 1)[1].split("\n  # One issue per", 1)[0]
        parts = re.split(r"\n(?=      - )", gate)
        self.steps = parts[1:]

    def step(self, needle):
        hits = [s for s in self.steps if needle in s.split("\n", 1)[0] or ("name: " + needle) in s]
        self.assertEqual(len(hits), 1, needle)
        # without the comment lines: a command named only in a comment does not count
        return "\n".join(ln for ln in hits[0].split("\n") if not ln.lstrip().startswith("#"))

    def test_w1_order_and_the_feature_only_in_its_own_step(self):
        names = [re.search(r"name: (.+)", s).group(1) if "name:" in s else s.split("\n")[0].strip() for s in self.steps]
        order = ["fmt", "clippy", "feature isolation", "tests (scripts", "tests with the test-hooks", "summary", "upload logs",
                 "failure attribution", "upload attribution"]
        pos = [next(i for i, n in enumerate(names) if n.startswith(o)) for o in order]
        self.assertEqual(pos, sorted(pos), names)
        for s in self.steps:
            head = re.search(r"name: (.+)", s)
            n = head.group(1) if head else ""
            code = "\n".join(ln for ln in s.split("\n") if not ln.lstrip().startswith("#"))
            if "test-hooks" in code:
                self.assertTrue(n.startswith(("feature isolation", "tests with the test-hooks", "summary", "failure attribution")), n)
        hooks = self.step("tests with the test-hooks")
        self.assertIn("--features test-hooks --test edit_lock_f755", hooks)
        self.assertIn("-p rcfd-project", hooks)
        self.assertIn("--no-doc", hooks)
        tests = self.step("tests (scripts")
        self.assertNotIn("features", tests)

    def test_w2_the_attribution_runs_after_a_failure_of_the_hooks_step(self):
        hooks = self.step("tests with the test-hooks")
        self.assertIn("id: hooks", hooks)
        self.assertIn("steps.tests.outcome != 'skipped'", hooks)
        attr = self.step("failure attribution")
        cond = re.search(r"\n\s+if: (.+)", attr).group(1)
        self.assertIn("steps.hooks.outcome == 'failure'", cond)
        self.assertIn("steps.tests.outcome == 'failure'", cond)
        self.assertIn("gate-out/times_hooks.tsv", attr)
        self.assertIn("gate-out/test_hooks.log", attr)
        self.assertIn("--feature-binary edit_lock_f755=test-hooks", attr)

    def test_w3_yaml_parses_when_pyyaml_is_there(self):
        try:
            import yaml
        except ImportError:
            self.skipTest("PyYAML not installed")
        doc = yaml.safe_load(self.text)
        names = [s.get("name", s.get("uses")) for s in doc["jobs"]["gate"]["steps"]]
        self.assertEqual(len(names), 12)


if __name__ == "__main__":
    unittest.main()
