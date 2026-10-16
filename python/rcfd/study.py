"""Parametric studies: a thin typed helper over ``rcfd study`` (docs/studies.md)."""

import json
import os

from . import _binary
from .case import dumps
from .errors import RcfdError, StudyError, parse_diagnostics


class List:
    """Sweep over the given values (numbers, strings, or any JSON values such as vectors)."""

    def __init__(self, path, values):
        self.path = path
        self.values = list(values)

    def overrides(self, prefix):
        out = [(prefix + ".path", self.path)]
        vals = self.values
        if vals and all(isinstance(v, (int, float)) and not isinstance(v, bool) for v in vals):
            out.append((prefix + ".values", list(vals)))
        elif vals and all(isinstance(v, str) for v in vals):
            out.append((prefix + ".strings", list(vals)))
        elif vals:
            out.append((prefix + ".json", [dumps(v) for v in vals]))
        return out


class Linspace:
    """``count`` values from ``start`` to ``stop`` (both included)."""

    def __init__(self, path, start, stop, count):
        self.path, self.start, self.stop, self.count = path, start, stop, count
    kind = "linspace"

    def overrides(self, prefix):
        return [
            (prefix + ".path", self.path), (prefix + ".kind", self.kind), (prefix + ".start", self.start),
            (prefix + ".stop", self.stop), (prefix + ".count", self.count),
        ]


class Logspace(Linspace):
    """``count`` values in geometric progression from ``start`` to ``stop`` (both > 0, both included)."""
    kind = "logspace"


class StudyResult:
    """Result of ``study``: the exit code, the summary, the table rows and the output directory."""

    def __init__(self, exit_code, summary, out_dir):
        self.exit_code = exit_code
        self.summary = summary
        self.out_dir = out_dir

    @property
    def ok(self):
        return self.exit_code == 0

    @property
    def csv_path(self):
        return os.path.join(self.out_dir, "table.csv")

    @property
    def table(self):
        """The parsed ``table.json`` document (``rows``, ``parameters``, ``outputs``)."""
        with open(os.path.join(self.out_dir, "table.json"), encoding="utf-8") as fh:
            return json.load(fh)

    @property
    def rows(self):
        return self.table["rows"]

    def __repr__(self):
        return f"StudyResult(exit_code={self.exit_code}, summary={self.summary!r}, out_dir={self.out_dir!r})"


def study(case, parameters, outputs=None, combine="grid", parallel=1, out="study", resume=False,
          max_new_points=None, write_results=True):
    """Run a parametric study of ``case`` (a ``rcfd.Case``).

    ``parameters`` maps a parameter name to ``List``/``Linspace``/``Logspace``. Invalid sweeps raise
    ``StudyError`` (exit code 3 of the binary; ``.diagnostics`` hold the located ``study.*`` paths).
    Points that fail are rows of the table; ``result.exit_code`` is the contract exit code (0 all ok).
    """
    sets = []
    for name in sorted(parameters):
        sets += parameters[name].overrides(f"study.parameters.{name}")
    if outputs is not None:
        sets.append(("study.outputs", list(outputs)))
    sets.append(("study.combine", combine))
    sets.append(("study.write_results", bool(write_results)))
    args = ["study", case.path, "--out", os.path.abspath(out), "--json", "--quiet", "--parallel", str(int(parallel))]
    if resume:
        args.append("--resume")
    if max_new_points is not None:
        args += ["--max-new-points", str(int(max_new_points))]
    for p, v in case.overrides:
        args += ["--set", f"{p}={v}"]
    for p, v in sets:
        args += ["--set", f"{p}={dumps(v)}"]
    code, stdout, err = _binary.call(case._binary, args)
    lines = [ln for ln in stdout.splitlines() if ln.startswith("{")]
    if not lines or code in (2, 3):
        raise StudyError(err.strip() or f"rcfd study exited with {code}", code, err, parse_diagnostics(err, case.path))
    return StudyResult(code, json.loads(lines[-1]), os.path.abspath(out))
