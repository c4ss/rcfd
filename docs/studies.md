# Parametric studies (F-095, ADR 0034)

`rcfd study <case>` runs one simulation per design point of the `study` section (`docs/case-schema.md`) and writes a result table.

```toml
[reports.q]
type = "heat_flow"
patch = "xmin"
[study]
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
```

```
rcfd study <case> [--out dir] [--parallel n] [--resume] [--max-new-points n] [--no-fields] [--list] [--json] [--set path=json]...
```

* Sweeps: `list` (numbers, strings or JSON texts such as vectors), `linspace` (`count` values, the last exactly `stop`), `logspace`
  (geometric between `start` and `stop`, both > 0). A design point is the base case plus the overrides applied with the same
  mechanism as `rcfd run --set`, so a point is bit-identical to `rcfd run --set ...` of the same values (tested).
* Order: parameters sorted by name; `grid` is row-major with the first name slowest; `zip` walks together. The point index is the
  only ordering key.
* Outputs in `--out` (default `study`): `table.csv`, `table.json` (written after every completed point, atomically), and per point
  `points/pNNNN/` with the usual result files. The table contains no wall-clock times or thread numbers: it is byte-identical for
  any `--parallel` (tested, 1 against 3).
* Columns: `point`, the parameters, `status` (`ok` only for exit code 0), `exit_code`, `state`, `iterations`, the chosen reports,
  `message`. JSON rows also carry the point `fingerprint` (FNV-1a of the resolved settings without the `study` section).
* Failure isolation: a point that cannot be built, fails the mesh check or a setup rule, diverges, stops at its iteration cap or
  panics gives a failed row (reports of a run that did execute are still recorded) and the other points run. The study exits
  with the code of the lowest-index failed point (1, 3, 4, 5, 6 of `docs/run-control.md`), 0 when all points are ok, 3 for static
  sweep errors (nothing is written), 2 for usage errors.
* Resume: `--resume` keeps the `ok` rows of an existing table whose fingerprint and parameter values equal the planned point and
  runs the rest (failed points are retried). `--max-new-points n` stops after n new points (state `partial`, exit 0) for chunked
  runs. A resumed study ends with a table byte-identical to a fresh one (tested).
* `--list` prints the design points without running; `--json` ends with one summary object (`points`, `run`, `skipped`,
  `failed`, `state`, `exit_code`, `table`).

Measured checks and the deviations from the registered set-up are in the addendum of `docs/adr/0034-wave20c-study-python.md`.

## Not delivered (F-095 stays `partial`)

Service and MCP/assistant tools for studies, sweep parameters given as named expressions (F-104), studies of chained runs (the
chain is ignored), adaptive or optimising design exploration, resuming a point that was interrupted half way, a distributed or
queueing runner, report columns that are not report definitions (monitors, field statistics).
