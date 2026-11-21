"""Self-test of scripts/fmt_check.py (ADR 0752, F-851 finding 14): the batches, a file that fails the check, an empty list.

    python scripts/test_fmt_check.py

rustfmt is not started: `main` is given a recorder in its place.
"""
import contextlib
import io
import os
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import fmt_check as fc  # noqa: E402


class Result:
    def __init__(self, code):
        self.returncode = code


class Batches(unittest.TestCase):
    def test_every_file_once_in_order_in_lists_of_at_most_forty(self):
        self.assertEqual(fc.BATCH, 40)
        for n, want in ((0, 0), (1, 1), (40, 1), (41, 2), (85, 3)):
            files = [f"t{i}.rs" for i in range(n)]
            got = fc.batches(files)
            self.assertEqual(len(got), want, n)
            self.assertEqual([f for b in got for f in b], files)
            self.assertTrue(all(0 < len(b) <= fc.BATCH for b in got))
        self.assertEqual(fc.batches(["a", "b", "c"], 2), [["a", "b"], ["c"]])


class Main(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = self.tmp.name
        self.put("Cargo.toml", '[workspace.package]\nedition = "2021"\n')
        self.put("crates/big/src/lib.rs", "")
        for i in range(84):
            self.put(f"crates/big/tests/t{i:02d}.rs", "")
        self.put("crates/big/tests/common/mod.rs", "")  # a module, not a target root
        self.put("crates/empty/README.md", "")
        self.calls = []

    def tearDown(self):
        self.tmp.cleanup()

    def put(self, rel, text):
        p = os.path.join(self.root, rel)
        os.makedirs(os.path.dirname(p), exist_ok=True)
        with open(p, "w", encoding="utf-8") as fh:
            fh.write(text)

    def main(self, argv, bad=()):
        def run(cmd, cwd=None):
            self.calls.append(cmd)
            return Result(1 if any(f in cmd for f in bad) else 0)
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            code = fc.main(argv, root=self.root, run=run)
        return code, buf.getvalue()

    def test_a_clean_crate_is_checked_in_batches(self):
        code, out = self.main(["big"])
        self.assertEqual(code, 0)
        self.assertEqual([len(c) - 4 for c in self.calls], [40, 40, 5])  # 85 target files after `rustfmt --check --edition 2021`
        self.assertTrue(all(c[:4] == ["rustfmt", "--check", "--edition", "2021"] for c in self.calls))
        files = [f for c in self.calls for f in c[4:]]
        self.assertEqual(files[0], "crates/big/src/lib.rs")
        self.assertEqual(len(set(files)), 85)
        self.assertNotIn("crates/big/tests/common/mod.rs", files)
        self.assertIn("fmt_check: big: 85 target files in 3 batches", out)

    def test_a_file_that_fails_the_check_fails_the_crate_and_the_later_batches_still_run(self):
        code, _ = self.main(["big"], bad=("crates/big/tests/t00.rs",))  # in the first batch
        self.assertEqual((code, len(self.calls)), (1, 3))

    def test_an_empty_list_runs_nothing(self):
        code, out = self.main([])
        self.assertEqual((code, self.calls), (2, []))
        code, out = self.main(["empty"])
        self.assertEqual((code, self.calls), (2, []))
        self.assertIn("no target file found for empty", out)


if __name__ == "__main__":
    unittest.main()
