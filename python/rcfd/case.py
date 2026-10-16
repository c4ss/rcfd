"""A case: the settings tree of the solver, driven through the ``rcfd`` command line.

Every validation and every run is done by the Rust binary, so the Python object, ``rcfd run --set`` and
``rcfd check`` always agree (same code, same diagnostics paths). Overrides travel as ``--set path=<JSON>``;
Python floats are written with ``json.dumps`` and round-trip exactly.
"""

import json
import os
import re
import tempfile
import xml.etree.ElementTree as ET

from . import _binary
from .errors import RcfdError, SettingsError, parse_diagnostics


def _plain(value):
    """Convert numpy scalars/arrays (if used) and tuples to plain JSON-able Python values."""
    if hasattr(value, "tolist"):
        return value.tolist()
    if isinstance(value, (list, tuple)):
        return [_plain(v) for v in value]
    if isinstance(value, dict):
        return {str(k): _plain(v) for k, v in value.items()}
    return value


def dumps(value):
    """JSON text of a settings value (finite numbers only; floats round-trip exactly)."""
    return json.dumps(_plain(value), allow_nan=False)


def _walk(tree, path):
    cur = tree
    for seg in path.split("."):
        if isinstance(cur, dict) and seg in cur:
            cur = cur[seg]
        elif isinstance(cur, list) and seg.isdigit() and int(seg) < len(cur):
            cur = cur[int(seg)]
        else:
            raise SettingsError(path, "settings path not found: " + path)
    return cur


class RunResult:
    """Outcome of ``Case.run``: exit code, state, iterations, the report values and the result files."""

    def __init__(self, exit_code, state, iterations, out_dir, events, report_names):
        self.exit_code = exit_code
        self.state = state
        self.iterations = iterations
        self.out_dir = out_dir
        self.events = events
        self._report_names = list(report_names)
        self._monitors = None

    @property
    def ok(self):
        """True when the run-control exit code is 0 (converged or finished)."""
        return self.exit_code == 0

    def __repr__(self):
        return f"RunResult(exit_code={self.exit_code}, state={self.state!r}, iterations={self.iterations}, out_dir={self.out_dir!r})"

    def monitors(self):
        """All monitor series of the run: ``{name: (x, y)}`` read from ``monitors.json``."""
        if self._monitors is None:
            with open(os.path.join(self.out_dir, "monitors.json"), encoding="utf-8") as fh:
                doc = json.load(fh)
            self._monitors = {k: (v["x"], v["y"]) for k, v in doc["series"].items()}
        return self._monitors

    @property
    def reports(self):
        """Last value of every report of the case: ``{name: float}``."""
        mon = self.monitors()
        return {n: mon[n][1][-1] for n in self._report_names if n in mon and mon[n][1]}

    def field(self, name, as_numpy=None):
        """Cell values of a field of ``solution.vtu`` (scalars: a list; vectors: a list of lists).

        With numpy installed (and ``as_numpy`` not False) an array is returned instead.
        """
        path = os.path.join(self.out_dir, "solution.vtu")
        root = ET.parse(path).getroot()
        found = None
        for section in ("CellData", "PointData"):
            for sec in root.iter(section):
                for arr in sec.findall("DataArray"):
                    if arr.get("Name") == name:
                        found = arr
                        break
                if found is not None:
                    break
            if found is not None:
                break
        if found is None:
            raise RcfdError(f"no field named {name!r} in {path}")
        if found.get("format", "ascii") != "ascii":
            raise RcfdError(f"field {name!r}: only ASCII VTK arrays are supported")
        vals = [float(t) for t in (found.text or "").split()]
        nc = int(found.get("NumberOfComponents", "1"))
        out = vals if nc == 1 else [vals[i:i + nc] for i in range(0, len(vals), nc)]
        if as_numpy is not False:
            try:
                import numpy  # optional
                return numpy.array(out)
            except ImportError:
                if as_numpy:
                    raise
        return out


class Case:
    """A case file plus a list of setting overrides.

    ``Case.load(path)`` reads a ``.toml`` or ``.json`` case; ``Case.from_text`` / ``Case.from_dict`` write
    one to a temporary directory (relative file names inside such a case must therefore be absolute).
    """

    def __init__(self, path, binary=None, _tmp=None):
        self.path = os.path.abspath(path)
        self._binary = _binary.find_binary(binary)
        self._tmp = _tmp
        self._overrides = []  # [(path, json text)]
        self._settings = None
        self._settings, _ = self._resolve([])  # validates the file itself

    # ---- construction -------------------------------------------------------------------------
    @classmethod
    def load(cls, path, binary=None):
        """Load a case file; invalid files raise ``RcfdError`` with the located diagnostics."""
        return cls(path, binary)

    @classmethod
    def from_text(cls, text, suffix=".toml", binary=None):
        """Case from TOML (or JSON with ``suffix=".json"``) text."""
        tmp = tempfile.TemporaryDirectory(prefix="rcfd-case-")
        path = os.path.join(tmp.name, "case" + suffix)
        with open(path, "w", encoding="utf-8", newline="\n") as fh:
            fh.write(text)
        return cls(path, binary, _tmp=tmp)

    @classmethod
    def from_dict(cls, settings, binary=None):
        """Case from a settings dictionary (the structure of ``Case.settings``)."""
        return cls.from_text(dumps(settings), suffix=".json", binary=binary)

    # ---- settings -----------------------------------------------------------------------------
    @property
    def overrides(self):
        """The overrides applied on top of the file, as ``(path, JSON text)`` pairs."""
        return list(self._overrides)

    def _set_args(self, overrides=None):
        args = []
        for p, t in self._overrides if overrides is None else overrides:
            args += ["--set", f"{p}={t}"]
        return args

    def _resolve(self, extra):
        """Resolved settings with ``extra`` overrides added; raises SettingsError/RcfdError on failure."""
        merged = [(p, t) for p, t in self._overrides if p not in {e[0] for e in extra}] + list(extra)
        code, out, err = _binary.call(self._binary, ["run", self.path, "--print-settings"] + self._set_args(merged))
        if code == 0:
            return json.loads(out), merged
        diags = parse_diagnostics(err, self.path)
        if code in (2, 3) and diags:
            path, message = diags[0]
            raise SettingsError(path, message, code, err, diags)
        raise RcfdError(err.strip() or f"rcfd exited with {code}", code, err, diags)

    @property
    def settings(self):
        """The resolved settings tree (defaults filled in, overrides applied) as nested dicts."""
        if self._settings is None:
            self._settings, _ = self._resolve([])
        return json.loads(json.dumps(self._settings))

    def get(self, path):
        """Value of a setting by dotted path (list items by index); ``SettingsError`` if it does not exist."""
        if self._settings is None:
            self._settings, _ = self._resolve([])
        return json.loads(json.dumps(_walk(self._settings, path)))

    def set(self, path, value):
        """Set a setting, validated immediately by the solver's own schema.

        An invalid path or value raises ``SettingsError`` (``.path`` and ``.message`` as the command line
        reports them) and leaves the case unchanged.
        """
        text = dumps(value)
        if not re.match(r"^[A-Za-z0-9_.\-]+$", path):
            raise SettingsError(path, "malformed setting path")
        settings, merged = self._resolve([(path, text)])
        self._settings, self._overrides = settings, merged

    def update(self, values):
        """Set several settings (a dict ``path -> value``); all or none are applied."""
        extra = []
        for p, v in values.items():
            if not re.match(r"^[A-Za-z0-9_.\-]+$", p):
                raise SettingsError(p, "malformed setting path")
            extra.append((p, dumps(v)))
        settings, merged = self._resolve(extra)
        self._settings, self._overrides = settings, merged

    def check(self):
        """``rcfd check --json``: ``{ok, exit_code, diagnostics, rules, mesh}``."""
        code, out, err = _binary.call(self._binary, ["check", self.path, "--json"] + self._set_args())
        try:
            doc = json.loads(out)
        except ValueError:
            raise RcfdError(err.strip() or "rcfd check failed", code, err, parse_diagnostics(err, self.path))
        return doc

    # ---- running --------------------------------------------------------------------------------
    def run(self, out_dir, max_iterations=None, threads=None, extra_args=()):
        """Run the case into ``out_dir`` and return a ``RunResult`` (non-zero run-control codes such as 5
        "not converged" are outcomes, not exceptions; a case that cannot start raises ``RcfdError``)."""
        out_dir = os.path.abspath(out_dir)
        args = ["run", self.path, "--out", out_dir, "--quiet", "--status-json"] + self._set_args()
        if max_iterations is not None:
            args += ["--max-iterations", str(int(max_iterations))]
        if threads is not None:
            args += ["--threads", str(int(threads))]
        args += list(extra_args)
        code, out, err = _binary.call(self._binary, args)
        events = []
        for line in out.splitlines():
            line = line.strip()
            if line.startswith("{"):
                try:
                    events.append(json.loads(line))
                except ValueError:
                    pass
        finish = [e for e in events if e.get("event") == "finish"]
        if not finish:
            raise RcfdError(err.strip() or f"rcfd run exited with {code}", code, err, parse_diagnostics(err, self.path))
        fin = finish[-1]
        names = sorted((self._settings or self.settings).get("reports", {}).keys())
        return RunResult(code, fin.get("state"), fin.get("iterations", 0), out_dir, events, names)
