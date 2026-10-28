"""Self-test of scripts/make_example_projects.py (ADR 0142 P1): python -m unittest scripts/test_make_example_projects.py

Pure helpers against known values, `--check` on the committed examples (no binary needed), and the same check on a
tampered copy. The rebuild itself needs the solver and is not run here."""

import datetime
import json
import math
import os
import shutil
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import make_example_projects as mx  # noqa: E402


class Helpers(unittest.TestCase):
    def test_fnv64_known_vectors(self):
        # FNV-1a 64 test vectors (the same hash as rcfd_core::hash::fnv64)
        self.assertEqual(mx.fnv64(b""), 0xCBF29CE484222325)
        self.assertEqual(mx.fnv64(b"a"), 0xAF63DC4C8601EC8C)
        self.assertEqual(mx.fnv64(b"foobar"), 0x85944171F73967E8)
        self.assertEqual(mx.hash_bytes(b"a"), "fnv64:af63dc4c8601ec8c")
        self.assertTrue(mx.hash_matches("fnv1a64:af63dc4c8601ec8c", b"a"))
        self.assertFalse(mx.hash_matches("fnv64:af63dc4c8601ec8d", b"a"))

    def test_rfc3339_ms(self):
        self.assertEqual(mx.rfc3339_ms("1970-01-01T00:00:00Z"), 0)
        t = datetime.datetime(2026, 10, 28, 15, 11, 10, tzinfo=datetime.timezone.utc)
        self.assertEqual(mx.rfc3339_ms("2026-10-28T15:11:10.832608100Z"), int(t.timestamp()) * 1000 + 832)
        with self.assertRaises(ValueError):
            mx.rfc3339_ms("2026-10-28 15:11:10")

    def test_readme_paths(self):
        text = "Open `cases/a.toml`, then `runs/<id>/fields` and `../../docs/x.md`; not `p` or `meshes/*`. `notes/b.md`."
        self.assertEqual(mx.readme_paths(text), ["cases/a.toml", "../../docs/x.md", "notes/b.md"])

    def test_sign_changes(self):
        xs = [0.0, 1.0, 2.0, 3.0]
        self.assertEqual(mx.sign_changes(xs, [1.0, -1.0, -1.0, 3.0]), [0.5, 2.25])
        self.assertEqual(mx.sign_changes(xs, [1.0, 2.0, 3.0, 4.0]), [])

    def test_naca_warp_puts_the_rings_on_the_aerofoil_and_the_far_field(self):
        for k in range(16):
            th = 2.0 * math.pi * k / 16.0
            x, y, _ = mx.naca_warp((math.cos(th), math.sin(th), 0.0))
            self.assertAlmostEqual(abs(y), mx.naca_thickness(min(max(x, 0.0), 1.0)), delta=1e-12)
            x, y, _ = mx.naca_warp((2.0 * math.cos(th), 2.0 * math.sin(th), 0.1))
            self.assertAlmostEqual(math.hypot(x - 0.5, y), 20.0, delta=1e-12)

    def test_shedding_recovers_a_known_frequency(self):
        f = 0.2
        t = [i * 0.01 for i in range(20001)]
        cl = [0.3 * math.sin(2 * math.pi * f * x) for x in t]
        cd = [1.3 + 0.01 * math.sin(4 * math.pi * f * x) for x in t]
        with tempfile.TemporaryDirectory() as d:
            p = os.path.join(d, "monitors.json")
            with open(p, "w") as fh:
                json.dump({"series": {"cl": {"x": t, "y": cl}, "cd": {"x": t, "y": cd}}}, fh)
            s = mx.shedding(p)
        self.assertAlmostEqual(s["strouhal"], f, delta=1e-6)
        self.assertAlmostEqual(s["cd_mean"], 1.3, delta=1e-4)
        self.assertAlmostEqual(s["cl_amplitude"], 0.3, delta=1e-4)


@unittest.skipUnless(os.path.exists(os.path.join(mx.EXAMPLES, "manifest.json")), "no example-projects/manifest.json")
class Committed(unittest.TestCase):
    def test_the_committed_examples_are_fresh(self):
        problems = mx.check(mx.EXAMPLES)
        self.assertEqual(problems, [])

    def test_a_tampered_copy_is_reported(self):
        with tempfile.TemporaryDirectory() as d:
            root = os.path.join(d, "example-projects")
            shutil.copytree(mx.EXAMPLES, root)
            man = mx.read_json(os.path.join(root, "manifest.json"))
            run = man["projects"]["quick-start"]["runs"][0]
            # a case edited after its run
            with open(os.path.join(root, "quick-start", run["case"]), "a", newline="\n") as fh:
                fh.write("# edited after the run\n")
            # one snapshot fewer in index.json than in the store
            idx_path = os.path.join(root, "quick-start", "runs", run["run"], "fields", "index.json")
            idx = mx.read_json(idx_path)
            idx["snapshots"] = idx["snapshots"][:-1]
            with open(idx_path, "w") as fh:
                json.dump(idx, fh)
            # runtime state left behind
            with open(os.path.join(root, "wind-tunnel", ".rcfd", "runner.lock"), "w") as fh:
                fh.write("")
            problems = mx.check(root)
        joined = "\n".join(problems)
        self.assertIn("changed since the run", joined)
        self.assertIn("store and index.json disagree", joined)
        self.assertIn("runtime files left", joined)


if __name__ == "__main__":
    unittest.main()
