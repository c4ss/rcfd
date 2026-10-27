# Python access (F-083, ADR 0034)

This page describes the Python package: how it finds the solver, the calls it offers, how results and errors come back, and
what is not delivered. It is for people who want to script cases, runs and studies from Python. The package wraps the command
line rather than linking the solver in-process; the in-process extension is listed under "Not delivered". For the command-line
behaviour it relies on, see [run control](run-control.md); for the settings paths it sets, see the [case schema](case-schema.md).

`python/rcfd` is a pure-Python package (standard library only; numpy is used for field arrays when installed). It drives the `rcfd`
command line with argument lists (no shell). The Rust binary does all validation and all computation, so the Python object, `rcfd run
--set` and `rcfd check` agree by construction, with the same located diagnostic paths.

Locate the binary with `rcfd.find_binary()` (argument, `RCFD_BIN`, then `target*/release` or `debug` next to the repository). Use
the package from a checkout: `sys.path.insert(0, "<repo>/python")` (it also imports under `python -I`). There is no wheel yet.

```python
import rcfd
case = rcfd.Case.load("cases/cavity/case.toml")      # or Case.from_text(toml) / Case.from_dict(settings)
case.set("solver.dt", 0.05)                          # validated at once; SettingsError(.path, .message) on failure
case.get("solver.dt"); case.settings                 # resolved tree (defaults filled in)
case.check()                                         # rcfd check --json
res = case.run("out", max_iterations=200)            # RunResult: exit_code, state, iterations, reports, monitors(), field("p")
res.reports["q"]                                     # last value of each report
rcfd.study(case, {"k": rcfd.List("physics.conductivity", [1, 2]),
                  "dt": rcfd.Linspace("boundaries.xmin.temperature_value", 310, 350, 3)},
           outputs=["q"], parallel=2, out="study")   # StudyResult: rows, table, csv_path, exit_code
rcfd.schema()                                        # the JSON schema of the case
```

Non-zero run-control codes of a run (5 not converged, 6 diverged, ...) are outcomes in `RunResult.exit_code`, not exceptions; a case
that cannot start raises `RcfdError` (`exit_code`, `stderr`, `diagnostics` as `(path, message)`). Invalid sweeps raise `StudyError`.
Fields are read from the ASCII `solution.vtu` (cell data first). Settings are sent as `--set path=<JSON>`; Python floats round-trip exactly.
The package reads no environment variable except `RCFD_BIN` and writes no file except the temporary case of `from_text`/`from_dict`.

Tests: `python -I python/tests/run_tests.py` (needs the built binary), run by the workspace test `crates/rcfd-cli/tests/python_w20c.rs`;
without a Python interpreter that test prints `SKIPPED: ...` and returns.

## Not delivered (F-083 stays `partial`)

The PyO3 route: an in-process extension crate and a pip-installable wheel (the acceptance text of F-083), numpy zero-copy field
access without a VTU round trip, and a cavity reproduction through the extension. Reasons recorded in ADR 0034 D5: the workspace
forbids `unsafe_code` and PyO3's macros expand to unsafe code, PyO3/maturin are not in the offline cargo cache, and the only Python
here is a Microsoft Store 3.9 install. Also not delivered: access through the JSON-RPC service (`rcfd serve`), live access to a
running solver's fields, commands other than run/check/study, and a typed class per settings section (the tree is addressed by
dotted path and validated by the schema of the binary).

## Where to go next

- [Parametric studies](studies.md): the study command the package drives, its table and its exit codes.
- [Run control](run-control.md): the exit codes a run returns.
