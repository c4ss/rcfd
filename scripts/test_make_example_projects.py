"""Self-test of scripts/make_example_projects.py (ADR 0142 P1, ADR 0150, ADR 0157): python -m unittest scripts/test_make_example_projects.py

Pure helpers against known values, `--check` on both committed trees (no binary needed), and the same checks on
tampered copies. The rebuild itself needs the solver and is not run here."""

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

    def test_generated_files_and_inputs(self):
        # ADR 0150: what a rebuild writes; everything else of a project folder is an input the trees share
        for p in ["runs/r-1/run.json", ".rcfd/.gitignore", "designs/variants.ledger.jsonl",
                  "meshes/naca0012-ogrid/points"]:
            self.assertTrue(mx.is_generated("wind-tunnel", p), p)
        for p in ["cases/cylinder-re100.toml", "meshes/naca0012-ogrid.recipe.toml", "notes/README.md",
                  "rcfd-project.json", "designs/other.md"]:
            self.assertFalse(mx.is_generated("wind-tunnel", p), p)
        self.assertFalse(mx.is_generated("quick-start", "meshes/README.md"))
        self.assertFalse(mx.is_generated("quick-start", "meshes/naca0012-ogrid/points"))

    def test_comparable_cuts_only_the_run_table_of_a_note(self):
        text = "head\n%s\n| a | b |\n%s\ntail\n" % (mx.RUNS_BEGIN, mx.RUNS_END)
        other = "head\n%s\n| c |\n%s\ntail\n" % (mx.RUNS_BEGIN, mx.RUNS_END)
        self.assertEqual(mx.comparable("notes/README.md", text.encode()), b"head\n\ntail\n")
        self.assertEqual(mx.comparable("notes/README.md", text.encode()),
                         mx.comparable("notes/README.md", other.encode()))
        self.assertNotEqual(mx.comparable("cases/a.toml", text.encode()), mx.comparable("cases/a.toml", other.encode()))

    def test_this_platform_has_a_tree_on_windows_and_linux(self):
        p = mx.this_platform()
        if os.name == "nt":
            self.assertEqual(p, "windows")
        elif sys.platform.startswith("linux") and mx.platform.machine().lower() in ("x86_64", "amd64"):
            self.assertEqual(p, "linux")
        else:
            self.assertIsNone(p)
        self.assertEqual(mx.platform_block()["os"], p or sys.platform)

    def test_docker_forwards_add_missing_and_refuses_it_with_a_rerun_check(self):
        # ADR 0157: `--docker --add-missing` runs `--add-missing` in the container's Linux tree
        class A:
            threads, project, rerun_check, add_missing = 1, None, False, True
        inner = mx.docker_inner(A())
        self.assertEqual(inner[:4], ["python3", "scripts/make_example_projects.py", "--tree", "linux"])
        self.assertIn("--add-missing", inner)
        self.assertNotIn("--rerun-check", inner)
        A.add_missing = False
        self.assertNotIn("--add-missing", mx.docker_inner(A()))
        for extra in (["--rerun-check"], ["--summarise-only"]):
            with self.assertRaises(SystemExit) as e:
                mx.main(["--add-missing"] + extra)
            self.assertIn("--add-missing", str(e.exception))

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

    def test_both_committed_trees_are_fresh_and_share_their_inputs(self):
        # ADR 0150: each tree's own check, the platform it was built on, and the inputs it shares with the Windows tree
        self.assertEqual(mx.check_trees(), [])

    def test_a_linux_tree_out_of_step_is_reported(self):
        def placeless(problems):
            # copies outside the repository cannot resolve the notes' `../../` paths into it (ADR 0150 amendment 1)
            return [p for p in problems if "names `../../" not in p]

        with tempfile.TemporaryDirectory() as d:
            roots = {t: os.path.join(d, os.path.basename(r)) for t, r in mx.TREES.items()}
            for t, r in mx.TREES.items():
                shutil.copytree(r, roots[t])
            self.assertEqual(placeless(mx.check_trees(roots)), [], "the unchanged copies")
            lin = roots["linux"]
            man = mx.read_json(os.path.join(lin, "manifest.json"))
            run = man["projects"]["quick-start"]["runs"][0]
            # a case edited in the Windows tree only (its own run is then stale too)
            with open(os.path.join(roots["windows"], "quick-start", run["case"]), "a", newline="\n") as fh:
                fh.write("# edited after the Linux build\n")
            # a note line that only the Linux tree has, outside the generated run table
            with open(os.path.join(lin, "wind-tunnel", "notes", "README.md"), "a", newline="\n") as fh:
                fh.write("\nA line only the Linux tree has.\n")
            # an input the Windows tree does not have
            with open(os.path.join(lin, "quick-start", "cases", "extra.toml"), "w", newline="\n") as fh:
                fh.write("# extra\n")
            # the manifest says Windows, and a run names a Windows path
            man["platform"]["os"] = "windows"
            with open(os.path.join(lin, "manifest.json"), "w", newline="\n") as fh:
                json.dump(man, fh, indent=2)
            rpath = os.path.join(lin, "quick-start", "runs", run["run"], "run.json")
            rec = mx.read_json(rpath)
            rec["case_dir"] = "E:\\x\\example-projects-linux\\quick-start\\cases"
            with open(rpath, "w", newline="\n") as fh:
                json.dump(rec, fh, indent=2)
            problems = placeless(mx.check_trees(roots))
        joined = "\n".join(problems)
        self.assertIn("example-projects-linux/quick-start/%s differs from example-projects/quick-start/%s"
                      % (run["case"], run["case"]), joined)
        self.assertIn("example-projects-linux/wind-tunnel/notes/README.md differs", joined)
        self.assertIn("example-projects-linux/quick-start/cases/extra.toml: not an input", joined)
        self.assertIn("built on windows x86_64, expected linux x86_64", joined)
        self.assertIn("case_dir 'E:\\\\x\\\\example-projects-linux", joined)
        self.assertNotIn("missing (ADR 0150)", joined)

    def test_a_rebuild_writes_only_the_tree_of_its_platform(self):
        other = "linux" if mx.this_platform() == "windows" else "windows"
        with self.assertRaises(SystemExit) as e:
            mx.resolve_tree(other, None)
        self.assertIn("build it there", str(e.exception))
        if mx.this_platform() is not None:
            self.assertEqual(mx.resolve_tree(None, None), (mx.this_platform(), mx.TREES[mx.this_platform()]))
        with tempfile.TemporaryDirectory() as d:
            self.assertEqual(mx.resolve_tree(None, d), (None, os.path.abspath(d)))

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
