#!/usr/bin/env python3
"""Checks H1 to H3 of ADR 0653 Amendment 3: the split of the full gate into shards does not depend on the order of the
binaries, scripts/gate_shards_check.py tells a complete set of shards from an incomplete one, and gate.yml passes the
shard only in a sharded run. Run: python -m unittest scripts/test_gate_shards.py
"""
import os
import random
import re
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import gate_shards_check as gsc  # noqa: E402
import gate_test  # noqa: E402

GATE_YML = os.path.join(HERE, "..", ".github", "workflows", "gate.yml")


def tests_of(n_bins=40, per=7, seed=1):
    """(keys, seconds): tests of n_bins binaries, two of them with one name in two crates, a third without a record."""
    rng = random.Random(seed)
    keys, secs = [], []
    for b in range(n_bins):
        name = "same_name" if b < 2 else f"bin{b:02d}"
        for t in range(per):
            keys.append((name, f"D:/a/r/crates/c{b:02d}/Cargo.toml", f"t{t}"))
            secs.append(None if rng.random() < 0.33 else round(rng.uniform(0.1, 300.0), 2))
    return keys, secs


class H1Split(unittest.TestCase):
    def split(self, keys, secs, n):
        order, part, digest = gate_test.shard_split(keys, secs, n)
        return {keys[k]: sh for k, sh in zip(order, part)}, digest

    def test_h1_the_split_does_not_depend_on_the_order_of_the_binaries(self):
        keys, secs = tests_of()
        a, da = self.split(keys, secs, 4)
        idx = list(range(len(keys)))
        random.Random(7).shuffle(idx)
        b, db = self.split([keys[i] for i in idx], [secs[i] for i in idx], 4)
        self.assertEqual(a, b)
        self.assertEqual(da, db)
        self.assertEqual(len(a), len(keys))  # every test once: the keys are distinct and each has one shard
        self.assertEqual(sorted(set(a.values())), [0, 1, 2, 3])
        load = [sum((s or 1.0) for k, s in zip(keys, secs) if a[k] == sh) for sh in range(4)]
        self.assertLess(max(load) - min(load), 300.0)  # within the longest test

    def test_h1_control_the_old_order_splits_differently(self):
        # the sort before Amendment 3: by seconds only, ties in the order given
        def old(keys, secs, n):
            order = sorted(range(len(keys)), key=lambda k: -(secs[k] or 0.0))
            part = gate_test.shard_of([secs[k] for k in order], n)
            return {keys[k]: sh for k, sh in zip(order, part)}

        keys, secs = tests_of()
        idx = list(range(len(keys)))
        random.Random(7).shuffle(idx)
        self.assertNotEqual(old(keys, secs, 4), old([keys[i] for i in idx], [secs[i] for i in idx], 4))

    def test_h1_another_record_gives_another_digest(self):
        keys, secs = tests_of()
        _, d1 = self.split(keys, secs, 4)
        secs2 = list(secs)
        secs2[0] = 12345.0
        _, d2 = self.split(keys, secs2, 4)
        self.assertNotEqual(d1, d2)


def shard_dir(root, k, n, m, total, digest, rows=None):
    d = os.path.join(root, f"gate-windows-latest-abc-shard-{k}")
    os.makedirs(d)
    with open(os.path.join(d, "test.log"), "w", encoding="utf-8") as h:
        h.write(f"gate_test: shard {k}/{n} runs {m} of {total} tests, split {digest}\ntest result: ok. 1 passed\n")
    with open(os.path.join(d, "times.tsv"), "w", encoding="utf-8") as h:
        h.write("binary\ttest\toutcome\tseconds\tpeak_mb\n")
        h.write("b\tign\tignored\t0\t0\n")
        for i in range(m if rows is None else rows):
            h.write(f"b\tt{k}_{i}\tok\t1.00\t5\n")
    return d


class H2Check(unittest.TestCase):
    D1, D2 = "a" * 64, "b" * 64

    def run_check(self, specs):
        with tempfile.TemporaryDirectory() as root:
            return gsc.check([gsc.read_shard(shard_dir(root, *s)) for s in specs])

    def test_h2_a_complete_set_passes(self):
        self.assertEqual(self.run_check([(1, 3, 4, 10, self.D1), (2, 3, 3, 10, self.D1), (3, 3, 3, 10, self.D1)]), [])

    def test_h2_each_defect_fails_with_its_reason(self):
        cases = [
            ([(1, 3, 4, 10, self.D1), (3, 3, 3, 10, self.D1)], "expected 1 to 3"),
            ([(1, 2, 5, 10, self.D1), (2, 2, 5, 11, self.D1)], "different totals"),
            ([(1, 2, 5, 10, self.D1), (2, 2, 5, 10, self.D2)], "digests differ"),
            ([(1, 2, 5, 10, self.D1), (2, 2, 4, 10, self.D1)], "sum to 9"),
            ([(1, 2, 5, 10, self.D1), (2, 2, 5, 10, self.D1, 3)], "3 result rows"),
        ]
        for specs, word in cases:
            bad = self.run_check(specs)
            self.assertTrue(any(word in b for b in bad), (word, bad))

    def test_h2_a_folder_without_a_log_fails(self):
        with tempfile.TemporaryDirectory() as root:
            self.assertTrue(gsc.check([gsc.read_shard(root)]))


class H3Workflow(unittest.TestCase):
    def setUp(self):
        with open(GATE_YML, encoding="utf-8") as h:
            self.text = h.read().replace("\r\n", "\n")
        self.gate = self.text.split("\n  gate:\n", 1)[1].split("\n  # One issue per", 1)[0]
        self.code = "\n".join(ln for ln in self.text.split("\n") if not ln.lstrip().startswith("#"))

    def test_h3_the_matrix_comes_from_decide_and_one_shard_keeps_the_names(self):
        self.assertIn("shard: ${{ fromJSON(needs.decide.outputs.shards) }}", self.gate)
        self.assertIn("SHARDS: ${{ inputs.shards || '1' }}", self.text)  # a scheduled run has no input: one shard
        name = re.search(r"\n    name: (.+)", self.gate).group(1)
        self.assertTrue(name.startswith("full gate, ${{ inputs.os || 'windows-latest' }}${{ needs.decide.outputs.n != '1' && "), name)
        self.assertTrue(name.endswith("|| '' }}"), name)
        self.assertIn("SUFFIX: ${{ needs.decide.outputs.n != '1' && format('-shard-{0}', matrix.shard) || '' }}", self.gate)
        for art in ("gate-", "gate-attribution-"):
            self.assertIn(f"name: {art}${{{{ inputs.os || 'windows-latest' }}}}-${{{{ github.sha }}}}${{{{ env.SUFFIX }}}}", self.gate)

    def test_h3_the_shard_is_passed_only_in_a_sharded_run_with_the_old_time_limit(self):
        tests = [s for s in re.split(r"\n(?=      - )", self.gate) if "name: tests (scripts" in s][0]
        inside = tests.split('if [ "$SHARDS" != "1" ]; then', 1)[1].split("\n          fi\n", 1)[0]
        self.assertIn('args=(--shard "$SHARD/$SHARDS" --timeout-min 3600)', inside)
        self.assertEqual(tests.count("--shard"), 1)
        self.assertIn('"${args[@]}" > gate-out/test.log', tests)
        self.assertIn('gh run download "$ORDER_RUN"', inside)  # every shard reads the times of the run decide named

    def test_h3_hooks_in_shard_one_and_the_complete_job(self):
        hooks = [s for s in re.split(r"\n(?=      - )", self.gate) if "name: tests with the test-hooks" in s][0]
        self.assertIn("if: always() && steps.tests.outcome != 'skipped' && matrix.shard == 1", hooks)
        comp = self.code.split("\n  complete:\n", 1)[1]
        self.assertIn("needs: [decide, gate]", comp)
        self.assertIn("if: always() && needs.decide.outputs.n != '1' && needs.gate.result != 'skipped'", comp)
        self.assertIn("python3 scripts/gate_shards_check.py shards/*/", comp)
        self.assertIn("shell: bash", comp)  # with pipefail, so a failed check fails the step through the tee

    def test_h3_yaml_parses_when_pyyaml_is_there(self):
        try:
            import yaml
        except ImportError:
            self.skipTest("PyYAML not installed")
        doc = yaml.safe_load(self.text)
        self.assertEqual(sorted(doc["jobs"]), ["complete", "decide", "gate", "issues"])
        self.assertEqual(len(doc["jobs"]["gate"]["steps"]), 12)
        self.assertEqual(doc["jobs"]["gate"]["strategy"]["fail-fast"], False)


if __name__ == "__main__":
    unittest.main()
