"""Registered checks P1-P6 of ADR 0034 (the Rust harness crates/rcfd-cli/tests/python_w20c.rs is P7)."""

import json
import os
import re
import subprocess
import sys
import tempfile
import unittest

import rcfd
from rcfd import _binary

PKG_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BIN = rcfd.find_binary()

SLAB = """title = "slab study"
[mesh]
generator = "box_2d"
nx = 16
ny = 2
lx = 0.1
ly = 0.02
[physics]
nu = 0.01
rho = 1.0
cp = 1000.0
conductivity = 2.0
[energy]
enabled = true
relax = 1.0
[initial]
temperature = 300.0
[boundaries.xmin]
velocity = "fixed"
temperature = "fixed"
temperature_value = 310.0
[boundaries.xmax]
velocity = "fixed"
temperature = "fixed"
temperature_value = 300.0
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[solver]
algorithm = "simplec"
tolerance_u = 1e-9
max_iterations = 400
[reports.q]
type = "heat_flow"
patch = "xmin"
"""

GRID_STUDY = """[study]
outputs = ["q"]
[study.parameters.k]
path = "physics.conductivity"
values = [1.0, 2.0]
[study.parameters.dt]
path = "boundaries.xmin.temperature_value"
kind = "linspace"
start = 310
stop = 350
count = 3
[study.parameters.lx]
path = "mesh.lx"
kind = "logspace"
start = 0.05
stop = 0.2
count = 3
"""


def cli(*args):
    return _binary.call(BIN, list(args))


def canon(obj):
    return json.dumps(obj, sort_keys=True)


class Base(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory(prefix="rcfd-pytest-")
        self.addCleanup(self._tmp.cleanup)
        self.dir = self._tmp.name
        self.case_file = os.path.join(self.dir, "slab.toml")
        with open(self.case_file, "w", encoding="utf-8", newline="\n") as fh:
            fh.write(SLAB)

    def read(self, *parts):
        with open(os.path.join(*parts), "rb") as fh:
            return fh.read()


class P1SettingsRoundTrip(Base):
    VALUES = [
        ("physics.conductivity", 3.5),
        ("physics.nu", 0.0123456789012345),
        ("mesh.nx", 12),
        ("mesh.ny", 3),
        ("solver.algorithm", "simple"),
        ("energy.enabled", False),
        ("initial.velocity", [0.1, 0.0, -0.25]),
        ("title", "a round trip"),
        ("boundaries.xmin.temperature_value", 305.125),
        ("solver.max_iterations", 250),
    ]

    def test_round_trip_and_cli_equivalence(self):
        case = rcfd.Case.load(self.case_file)
        for path, value in self.VALUES:
            case.set(path, value)
            self.assertEqual(case.get(path), value, path)
        self.assertEqual(len(self.VALUES), 10)
        args = ["run", self.case_file, "--print-settings"]
        for path, value in self.VALUES:
            args += ["--set", f"{path}={json.dumps(value)}"]
        code, out, err = cli(*args)
        self.assertEqual(code, 0, err)
        self.assertEqual(canon(json.loads(out)), canon(case.settings))
        # a failed set leaves the case unchanged
        before = canon(case.settings)
        with self.assertRaises(rcfd.SettingsError):
            case.set("mesh.nx", 0)
        self.assertEqual(canon(case.settings), before)

    def test_float_exactness(self):
        case = rcfd.Case.load(self.case_file)
        x = 0.1 + 0.2
        case.set("physics.conductivity", x)
        self.assertEqual(case.get("physics.conductivity"), x)


class P2RunEqualsCli(Base):
    def test_bitwise_equal_results_and_reports(self):
        sets = [("physics.conductivity", 1.5), ("boundaries.xmin.temperature_value", 333.0)]
        case = rcfd.Case.load(self.case_file)
        for p, v in sets:
            case.set(p, v)
        res = case.run(os.path.join(self.dir, "py"))
        self.assertEqual(res.exit_code, 0)
        args = ["run", self.case_file, "--out", os.path.join(self.dir, "cli"), "--quiet"]
        for p, v in sets:
            args += ["--set", f"{p}={json.dumps(v)}"]
        code, _, err = cli(*args)
        self.assertEqual(code, 0, err)
        for name in ("monitors.csv", "solution.vtu", "monitors.json"):
            self.assertEqual(self.read(self.dir, "py", name), self.read(self.dir, "cli", name), name)
        last = self.read(self.dir, "cli", "monitors.csv").decode().strip().splitlines()
        header, row = last[0].split(","), last[-1].split(",")
        self.assertEqual(res.reports["q"], float(row[header.index("q")]))
        f = res.field("T", as_numpy=False)
        self.assertEqual(len(f), 32)
        self.assertAlmostEqual(f[0], 333.0 - 33.0 / 32.0, places=6)


class P3Errors(Base):
    def cli_path(self, setting):
        code, _, err = cli("run", self.case_file, "--set", setting, "--print-settings")
        self.assertEqual(code, 3, err)
        m = re.search(r"--set (\S+?):", err)
        self.assertTrue(m, err)
        return m.group(1)

    def test_same_paths_as_the_command_line(self):
        case = rcfd.Case.load(self.case_file)
        for path, value in [("physics.nope", 1), ("mesh.nx", 0), ("solver.algorithm", "foo"), ("physics.nu", "x")]:
            expect = self.cli_path(f"{path}={json.dumps(value)}")
            with self.assertRaises(rcfd.SettingsError) as cm:
                case.set(path, value)
            self.assertEqual(cm.exception.path, expect)
            self.assertEqual(cm.exception.path, path)
            self.assertEqual(cm.exception.exit_code, 3)

    def test_study_errors_carry_study_paths(self):
        case = rcfd.Case.load(self.case_file)
        cases = [
            ({"a": rcfd.List("physics.nope", [1.0])}, "study.parameters.a.path"),
            ({"a": rcfd.List("physics.nu", ["abc"])}, "study.parameters.a.strings.0"),
            ({"a": rcfd.List("solver.algorithm", [1.0])}, "study.parameters.a.values.0"),
            ({"a": rcfd.List("mesh.nx", [16, 0])}, "study.parameters.a.values.1"),
            ({"a": rcfd.Logspace("physics.nu", 0.0, 2.0, 3)}, "study.parameters.a.start"),
            ({"a": rcfd.Linspace("physics.nu", 1.0, 2.0, 0)}, "study.parameters.a.count"),
        ]
        for params, path in cases:
            with self.assertRaises(rcfd.StudyError) as cm:
                rcfd.study(case, params, out=os.path.join(self.dir, "never"))
            paths = [p for p, _ in cm.exception.diagnostics]
            self.assertIn(path, paths, cm.exception.stderr)
            self.assertEqual(cm.exception.exit_code, 3)
            self.assertFalse(os.path.exists(os.path.join(self.dir, "never")))


class P4Study(Base):
    def test_equals_cli_table(self):
        case = rcfd.Case.load(self.case_file)
        params = {
            "k": rcfd.List("physics.conductivity", [1.0, 2.0]),
            "dt": rcfd.Linspace("boundaries.xmin.temperature_value", 310, 350, 3),
            "lx": rcfd.Logspace("mesh.lx", 0.05, 0.2, 3),
        }
        res = rcfd.study(case, params, outputs=["q"], parallel=2, out=os.path.join(self.dir, "py"), write_results=False)
        self.assertEqual(res.exit_code, 0)
        self.assertEqual(len(res.rows), 18)
        study_file = os.path.join(self.dir, "grid.toml")
        with open(study_file, "w", encoding="utf-8", newline="\n") as fh:
            fh.write(SLAB + GRID_STUDY)
        code, _, err = cli("study", study_file, "--out", os.path.join(self.dir, "cli"), "--quiet", "--no-fields")
        self.assertEqual(code, 0, err)
        for name in ("table.csv", "table.json"):
            self.assertEqual(self.read(self.dir, "py", name), self.read(self.dir, "cli", name), name)
        self.assertEqual(res.table, json.loads(self.read(self.dir, "cli", "table.json")))


class P5NoLeak(Base):
    MARKER = "RCFD-TEST-SECRET-9f3a1c"

    def test_marker_never_appears(self):
        os.environ["RCFD_TEST_SECRET"] = self.MARKER
        self.addCleanup(os.environ.pop, "RCFD_TEST_SECRET", None)
        case = rcfd.Case.load(self.case_file)
        res = case.run(os.path.join(self.dir, "run"))
        st = rcfd.study(case, {"k": rcfd.List("physics.conductivity", [1.0, 2.0])}, out=os.path.join(self.dir, "st"))
        texts = [repr(res), str(res), repr(st), str(st), repr(res.events), repr(case.settings), repr(case.overrides)]
        try:
            case.set("physics.nope", 1)
        except rcfd.SettingsError as e:
            texts += [repr(e), str(e), e.stderr]
        for t in texts:
            self.assertNotIn(self.MARKER, t)
        for root, _, files in os.walk(self.dir):
            for name in files:
                with open(os.path.join(root, name), "rb") as fh:
                    self.assertNotIn(self.MARKER.encode(), fh.read(), os.path.join(root, name))

    def test_package_reads_only_rcfd_bin_from_the_environment(self):
        hits = []
        for root, _, files in os.walk(os.path.join(PKG_DIR, "rcfd")):
            for name in files:
                if name.endswith(".py"):
                    with open(os.path.join(root, name), encoding="utf-8") as fh:
                        for line in fh:
                            if re.search(r"os\.environ|getenv", line):
                                hits.append(line.strip())
        self.assertEqual(hits, ['env = os.environ.get("RCFD_BIN")'])


class P6CleanImport(unittest.TestCase):
    def test_import_under_isolated_mode(self):
        code = """
import sys, os, sysconfig
sys.path.insert(0, %r)
before = set(sys.modules)
import rcfd
std = os.path.normcase(os.path.realpath(sys.base_prefix))
pkg = os.path.normcase(os.path.realpath(%r))
bad = []
for n in sorted(set(sys.modules) - before):
    f = getattr(sys.modules[n], "__file__", None)
    if f and not os.path.normcase(os.path.realpath(f)).startswith((std, pkg)):
        bad.append(n)
assert "numpy" not in sys.modules
assert not bad, bad
""" % (PKG_DIR, PKG_DIR)
        proc = subprocess.run([sys.executable, "-I", "-c", code], stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        self.assertEqual(proc.returncode, 0, proc.stderr.decode())
        self.assertEqual(proc.stdout, b"")
        self.assertEqual(proc.stderr, b"")


if __name__ == "__main__":
    unittest.main()
