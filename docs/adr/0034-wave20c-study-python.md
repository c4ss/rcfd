# ADR 0034: Wave 20C, parametric studies (F-095) and Python access (F-083)

Status: accepted. Everything under "Decisions" and "Registered checks" was written before any code of this wave existed.
Limits are not changed, widened, replaced or deleted after a run. A miss is reported as measured and the assertion moves to an
ignored `*_known_miss` test with the registered limit intact; the cause is diagnosed, not tuned away. A test set-up defect gets a
NEW corrected test and the old one stays as a record. Addenda (file names, measured numbers, the crafted failing point) may be
appended at the end without touching a registered limit.

Scope. F-095 and F-083 are `todo`. This wave delivers a first slice of each and stops at `partial` with named gaps. F-239 and
F-238 are attempted only if cheap (they are not part of the registered checks below; whatever is done is recorded in an addendum,
otherwise they stay `todo`). F-139/F-242 (co-simulation, FMI) are NOT attempted. Clean-room: first principles only, no vendor
code, text, data or names. Baseline hashes (`crates/rcfd-api/tests/baseline_hashes.rs`) stay bit-identical and are not
re-recorded: nothing here touches a numerical kernel; the study runner calls the existing `Simulation` and `runctl::drive`
unchanged, and a case without a `study` section never enters it. Schema additions are additive with explicit unset sentinels
(empty list, `""`, 0).

## Decisions

### D1 Layout and shared files

New: `rcfd-api::study` (planning, running, tables), CLI file `rcfd-cli/src/cmd_study.rs` (command `rcfd study`), the package
`python/rcfd` with tests under `python/tests`, a Rust integration test that runs those with the system Python. Shared files get
small targeted edits only, re-read before editing: one `.field("study", ...)` line in `schema.rs`, one `out.extend(...)` in
`case.rs` (static checks), one `pub mod` in `lib.rs`, one dispatch line and usage text in `rcfd-cli/src/main.rs`.

### D2 The `study` section

```
[study]
outputs = ["q"]            # report names that become table columns; empty = all reports, sorted by name
combine = "grid"           # grid (cartesian product) | zip (parameters walk together; equal lengths required)
parallel = 1               # design points run at the same time (>= 1)
write_results = true       # write each point's result files under <out>/points/pNNNN/
[study.parameters.<name>]
path = "physics.conductivity"   # settings path the parameter overrides (any path `--set` accepts)
kind = "list"                   # list | linspace | logspace
values = [1.0, 2.0]             # list: numeric values
strings = []                    # list: string values (used instead of values when non-empty; values must then be empty)
start = 0.0  stop = 0.0  count = 0   # linspace / logspace
```

Sweep kinds. `list`: the given values in the given order. `linspace`: `count >= 1` values, `start + (stop - start) * i / (count - 1)`
for `i < count - 1` and exactly `stop` for the last (a single value is `start`). `logspace`: geometric progression between the two
actual values (not exponents): `start * (stop / start)^(i / (count - 1))`, last exactly `stop`, `start > 0` and `stop > 0`. A
value that is integral and whose target setting is an integer is passed as an integer (`mesh.nx` swept by a list `[8, 16]` works).

Design points. Parameter names are sorted lexicographically. `grid`: row-major with the first (alphabetical) parameter slowest;
`zip`: point i takes entry i of every parameter. The point index (0-based) is the position in this order and is the only ordering
key of every output. A point is the base case plus the overrides applied in parameter-name order with `Case::set`, exactly the
`--set` mechanism of `rcfd run` (so the two agree by construction), then `semantic_errors`.

Static checks (all located, collected together, exit 3, nothing runs and no output directory is created): unknown setting path
(`study.parameters.<n>.path`), a value of the wrong type for that path (`study.parameters.<n>.values.<i>` or `.strings.<i>`),
empty path, empty sweep, `count` below 1 or missing for linspace/logspace, logspace with non-positive ends, non-finite numbers,
`values` and `strings` both non-empty, `zip` with unequal lengths (`study.combine`), unknown output report name (`study.outputs.<i>`),
no parameters at all, `parallel` below 1, two parameters with the same path (`study.parameters.<n>.path`). A value that passes
the type check but makes a point unrunnable (build error, semantic error, mesh failure, divergence) is NOT a static error: it is a
failed row (D4). A `chain` in the base case is ignored by the study (the base case runs alone); this is a named gap.

### D3 Running

`rcfd study <case> [--out dir] [--parallel n] [--resume] [--max-new-points n] [--no-fields] [--list] [--json] [--set path=json]...`
(`--set` applies to the base case before the sweep; `--parallel` overrides `study.parallel`; `--list` prints the design points
and exits 0 without running; `--max-new-points n` stops after n newly run points, state `partial`, for chunked and tested resume).
Each point: build `Simulation`, mesh quality, setup rules, `runctl::drive` with the case's own limits, `write_results` into
`<out>/points/pNNNN/` (unless `write_results` is false), then the values of the chosen reports (`Simulation::report_values`) are
read. `parallel > 1` runs points on that many scoped OS threads taking indices from an atomic counter; results are stored by
index, so the table does not depend on the schedule. Every point is wrapped in `catch_unwind`; a panic is a failed row with exit 1.
Output dir default: `study` in the current directory.

### D4 Table, failure isolation, exit code, resume

Files in the output dir: `table.json` and `table.csv`, written (atomically, via a temporary file and rename) after every completed
point with the completed rows sorted by point index, and finally with all rows. No wall-clock or thread number enters them, so
they are byte-identical for any `parallel`. Row (JSON): `point`, `parameters` (name to value), `status` (`ok` or `failed`),
`exit_code`, `state`, `iterations`, `fingerprint`, `reports` (name to number or null), `message`. CSV: header
`point,<parameter names>,status,exit_code,state,iterations,<output names>,message`; numbers by the shortest round-trip decimal
(the JSON number text); null as an empty cell; the message quoted with `"` doubled when it contains `,` `"` or a newline.
`fingerprint` is a 64-bit FNV-1a hash (16 hex digits) of the resolved settings JSON of the point with the `study` section
removed. `status` is `ok` only for exit code 0; the report values of a non-converged or diverged point are still recorded.

Failure isolation: a point that fails (any non-zero exit code of the run-control contract, an invalid point, a panic) gives a row
and the other points run. Study exit code: 0 when every point has exit 0; otherwise the exit code of the lowest-index failed
point (the rule of `chain`: the contract's per-run codes 1 build/IO failure or panic, 3 invalid settings, 4 mesh, 5 not
converged, 6 diverged); 3 for static sweep errors; 2 for usage errors; `partial` (a `--max-new-points` stop, with no failed
row) is exit 0 and the final line says `partial`. The final stdout line with `--json` is one object: `format: "rcfd-study"`,
`points`, `run`, `skipped`, `failed`, `state` (`finished`, `failed`, `partial`), `exit_code`, `table`.

Resume (`--resume`): rows of an existing `table.json` in the output dir whose `status` is `ok` and whose `fingerprint` equals the
fingerprint of the planned point (and whose parameter values are equal) are kept and not run; failed or missing points are run
again; rows of the old table that no longer match any planned point are dropped. Without `--resume` an existing table is
overwritten and all points run.

### D5 Python package (F-083)

Feasibility check (done before the decision, facts recorded here): the workspace sets `unsafe_code = "forbid"` for every crate
and PyO3's macros expand to unsafe code, so an `rcfd-py` crate would have to opt out of the workspace lints; PyO3 and maturin are
not in the offline cargo registry cache; the only Python on this machine is a Microsoft Store Python 3.9 whose install lives in
the protected `WindowsApps` directory (linking against `python39.lib` and the unrelocatable DLL from a build is fragile there);
there is no wheel tooling pinned. Decision: deliver the pure-Python package `python/rcfd` driving the CLI (`rcfd run`, `check`,
`study`, `schema`) as subprocesses with an argument list (never a shell), and name the PyO3 extension (in-process access to
fields without VTU round trips, wheel packaging, numpy zero-copy) as a gap; F-083 stays `partial`. numpy is used when importable
(fields as arrays) and everything works without it (lists).

API: `rcfd.find_binary()` (argument, `RCFD_BIN` environment variable, then the workspace `target` dirs; a clear error otherwise),
`rcfd.Case.load(path)` / `Case.from_dict`, `case.get(path)`, `case.set(path, value)` (validates immediately through the Rust
validator, raising `SettingsError` with `path` and `message` as the CLI prints them), `case.settings` (resolved tree from
`--print-settings`), `case.check()` (`rcfd check --json`), `case.run(out_dir, **limits)` returning a `RunResult` with `exit_code`,
`state`, `reports` (last value of every report), `field(name)` (parsed from `solution.vtu`, ASCII DataArray) and
`monitors`; `rcfd.study(case, parameters, outputs, combine, parallel, out, resume)` returning a table; `rcfd.schema()` (the JSON
schema from `rcfd schema`). The process environment is passed through unchanged except that nothing is added; no secret is read or
written by the package, the package never prints or stores the environment, and `python -I` is able to import it (no use of the
current directory or `PYTHONPATH`; the test adds the package directory with `sys.path` inside the script it runs, not through the
environment).

Overrides travel to the CLI as `--set path=<JSON>` with `json.dumps(value, allow_nan=False)` (Python floats round-trip exactly),
and `case.set` on a loaded case records the override list; running a modified case passes the same `--set` list, which makes the
Python path and the command line the same code path in the Rust binary.

### D6 F-239 / F-238 slices

Not part of the registered checks. If time allows after D1-D5 are done, a single cheap, additive slice may be added (candidate:
a post-object that computes the difference of two solution files, F-238 case comparison). Anything added gets its checks written
into an addendum before it is run, and the roadmap states it. Otherwise both items stay `todo`.

## Registered checks

Closed-form base case `SLAB`: `box_2d`, `nx = 16`, `ny = 2`, `lx = 0.1`, `ly = 0.02`, energy equation on, fixed temperature
`T_hot` on `xmin` and `T_cold = 300` on `xmax`, no flow (walls elsewhere), `simplec` with `tolerance_u = 1e-12`, 400 iterations
(the case of `report_conductivity.rs`). Steady 1-D conduction is linear in x, so the heat flow through `xmin` is exactly
`q = k * ly * (T_hot - T_cold) / lx` (unit depth); the discrete solution on a uniform mesh with fixed-temperature faces is exact
for a linear profile. The sign is the library's convention; the comparison is on the absolute value and the sign is recorded in
the addendum.

* S1 Analytic reproduction. Grid study: `physics.conductivity` list `[1.0, 2.0]` x `boundaries.xmin.temperature_value` linspace
  `start = 310, stop = 350, count = 3` x `mesh.lx` logspace `start = 0.05, stop = 0.2, count = 3` = 18 points. Limit: at every
  point `|q_i| / q_exact_i - 1` within `1e-6` (relative), and the recorded parameter values equal the sweep definition exactly
  (the logspace and linspace end points bit-equal).
* S2 Deterministic ordering. The S1 study run with `parallel = 1` and with `parallel = 3`: `table.csv` and `table.json`
  byte-identical (limit: zero differing bytes); rows ordered by point index 0..17; grid order first alphabetical parameter
  slowest; `zip` of three lists of 3 gives point i = entry i of each (3 rows).
* S3 Resume. Run S1 with `--max-new-points 7`: table has exactly 7 rows, state `partial`, exit 0. Then `--resume`: summary says
  `run = 11`, `skipped = 7`; the final `table.json` and `table.csv` are byte-identical to a fresh complete run. A second
  `--resume` runs 0 points and leaves the table byte-identical. A resume after changing one setting of the base case (the
  fingerprint changes) runs all 18 points again. Limits: those counts and zero differing bytes.
* S4 Failure isolation. A study with one crafted point that passes the static check and cannot run (a mesh size of 0, or if the
  schema rejects that statically, a mesh file that does not exist; the choice is recorded in the addendum) among three points:
  exactly one `failed` row with a non-empty message and null reports, the other two rows `ok` and within the S1 tolerance of the
  closed form, `rcfd study` exit code equal to the failed row's `exit_code` and not 0, with the other point files written.
  With `parallel = 3` the same table bytes.
* S5 Located errors. Each of: unknown path, wrong type (string for a number, number for an enum), empty sweep, linspace without
  `count`, logspace with `start <= 0`, `values` and `strings` both set, `zip` with unequal lengths, unknown output report,
  duplicate path, no parameters, gives exit 3 with the diagnostic path exactly the one named in D2 and no output directory; a bad
  command line (`--parallel 0`, `--max-new-points x`) gives exit 2.
* S6 Exit-code contract. All points ok: 0. One point not converged (iteration cap hit, exit 5) and later points ok: the study
  exits 5 and every other row is recorded; a single `--list` run exits 0 and runs nothing.
* S7 Operating-point map (roadmap scope of F-095). Plane channel `box_2d`, `lx = 4`, `ly = 0.1`, `nx = 100`, `ny = 10`,
  `nu = 0.01`, `rho = 1`, uniform inlet velocity swept over a list of 5 values `0.02, 0.04, 0.06, 0.08, 0.1`, outlet pressure
  0, laminar steady. Report: area average of `p` on the inlet patch. Limit: within 1 percent of `12 * rho * nu * U * lx / ly^2`
  at every point (fully developed laminar plane Poiseuille flow, mean velocity U; the entrance and the boundary-face pressure
  extrapolation add at most a few tenths of a percent by estimate, no allowance is made in the limit).
* P1 Python settings round trip. For ten settings (floats, ints, strings, bools, a list) `Case.set` then `Case.get` and
  `settings` equal the value passed, and the resolved settings of the Python case equal `rcfd run --set ... --print-settings` of
  the same overrides, byte-identical JSON text after canonical re-serialisation (limit: equality).
* P2 Python run equals CLI run. Cavity-class steady case run through `Case.run` and directly by the binary with the same
  `--set`: `monitors.csv` and `solution.vtu` byte-identical (limit: zero differing bytes), and the Python `reports` equal the
  last row of the report columns of `monitors.csv` exactly.
* P3 Validation errors surface with the CLI's paths. For an unknown path, a wrong type and an enum violation, the Python
  `SettingsError.path` equals the path in the CLI's diagnostic (`rcfd run --set ... --check-only` stderr) and the study helper's
  static errors carry the same `study.*` paths as S5.
* P4 Study from Python. `rcfd.study` on the S1 setup returns the same rows as the CLI table (equal after JSON parse, and the CSV
  bytes of its output dir equal those of the CLI run).
* P5 No leak. A run with a marker variable `RCFD_TEST_SECRET=...` in the Python process environment: the marker appears in no
  file the package or the binary writes (table, results, logs captured by the package), and `RunResult`/`SettingsError`
  `repr`/`str` never contain it; the package source contains no use of `os.environ` other than reading `RCFD_BIN` and `PATH`
  semantics via the subprocess default (checked by a text search in the test).
* P6 Clean import. `python -I -c "import sys; sys.path.insert(0, <pkg dir>); import rcfd"` exits 0 with no output and imports
  only standard-library modules (plus numpy lazily and only inside the field reader).
* P7 Harness. The Rust workspace test runs the Python tests with the system Python; if no Python is found it prints
  `SKIPPED: no python interpreter found` to stderr and returns without asserting (the skip is visible in the test output, not a
  silent pass); if the `rcfd` binary of the workspace is not built the test builds nothing and fails with a clear message.

Not delivered by this wave, named up front: in-process PyO3 bindings and wheel (D5), adaptive/optimising studies and design
exploration, studies of chains, a distributed or queueing runner, expression-valued (named expression) sweep parameters,
parametrising mesh file imports other than by path, resuming a single interrupted point (resume is per completed point), the
service/MCP exposure of studies (the CLI and the Python package only), F-139 and F-242.

## Addendum: measured results and deviations (written after the runs; no registered limit was changed)

Everything below was measured with the release build (`cargo test --release -p rcfd-cli --test study_w20c` and
`--test python_w20c`), tests in `crates/rcfd-cli/tests/study_w20c.rs`, `python/tests/test_rcfd.py` and
`crates/rcfd-cli/tests/python_w20c.rs`. Baseline hashes: `baseline_hashes` passes unchanged (not re-recorded).

### Deviations from the registered set-up (disclosed)

1. S1 set-up defect (my error): the registered closed form assumed a unit depth. `box_2d` is one cell thick with depth `lx / nx`, so
   the exact heat flow is `k * ly * (lx/nx) * dT / lx = k ly dT / nx`, independent of `lx`. The registered formula is kept as the
   ignored test `s1_unit_depth_known_miss` (measured: relative error 0.9969 at the worst point, i.e. a miss by the factor
   `lx/nx`); the corrected test `s1_corrected_depth` takes the depth from the mesh (total cell volume divided by `lx * ly`), not from
   the formula under test. The reported value is negative for this case (sign convention of the report not investigated): the comparison is on the absolute
   value as registered.
2. The registered solver setting `tolerance_u = 1e-12` never meets the residual target on this case (the energy residual stalls at
   about 1e-11; the run stops at the 400-iteration cap, exit 5, `not_converged`), so every point of the S1 study has status `failed`
   with exit 5 at that tolerance, which is also a demonstration of the exit-code contract. `s1_corrected_depth` is run at both
   tolerances (identical q to the digit shown below); S2 and S3 were first run only at 1e-9, which I switched to after the 1e-12 grid ended at the iteration cap
   (a set-up change made after seeing that outcome, not before); S4-S6 and the Python tests also use `tolerance_u = 1e-9` (converges in 4
   iterations, exit 0). The registered 1e-12 versions were added afterwards (second addendum below).
3. S7: the generator is `channel_2d` (patches `inlet`, `outlet`, `walls`), not `box_2d` with `ymin/ymax`, and `depth = 0.1`; the
   inlet velocity is a vector setting, which the scalar `values` list cannot express, so the parameter kind `list` got an additive key
   `json` (list of JSON texts) beside `values` and `strings`. This extends D2.
4. D2 said sweep paths are applied "exactly like `--set`"; to make `rcfd study --set study.parameters.<n>.path=...` (used by the Python
   helper) work, `Case::set` now creates named entries nested deeper than one level (it only handled `<collection>.<name>.<field>`).
   Existing paths behave as before (baseline hashes unchanged).
5. S4 crafted point: `mesh.nx = 0`, a negative `ly` and similar values are rejected statically by the schema ranges (exit 3, nothing
   runs), which is the S5 behaviour. The crafted point is therefore `mesh.source` swept over the strings `generator, file, generator`:
   the middle point selects `file` without `mesh.file` and fails with exit 3 (`mesh.source = "file" requires mesh.file`).
6. P5 is implemented as registered; the text search is a regular expression on `os.environ` / `getenv` in the package sources (the
   only hit is the `RCFD_BIN` read). P6 checks that every module newly imported by `import rcfd` under `python -I` lives under the
   interpreter prefix or the package directory and that numpy is not imported.
7. F-083 route: the PyO3 build was not attempted. The decision in D5 rests on three checked facts (unsafe code forbidden by the
   workspace lints, no pyo3 in the offline cargo cache, only a Microsoft Store Python 3.9), not on a failed build.
8. F-239 and F-238 slices (D6) were not done (time); both stay `todo`. F-139/F-242 were not attempted.

### Measured against the registered limits

| check | limit | measured | verdict |
| --- | --- | --- | --- |
| S1 corrected (18 points) | relative error of every point <= 1e-6 | 3.34e-10 (both solver tolerances); sweep end points bit-equal | met; the registered unit-depth form is a known miss (deviation 1) |
| S1 equivalence | (added) point 7 equals `rcfd run --set` of the same values | `solution.vtu` and `monitors.csv` byte-identical | met |
| S2 | 0 differing bytes, parallel 1 vs 3; order by point index | 0 differing bytes in `table.csv` and `table.json`; row order and grid/zip order as registered | met |
| S3 | 7 rows after the partial run; resume runs 11, skips 7; final tables byte-identical to a fresh run; second resume runs 0; changed base setting runs all 18 | exactly these | met |
| S4 | one failed row, others ok within the S1 tolerance, study exit code = the failed row's code (non-zero), same bytes for parallel 1 and 3 | exit 3; other two rows within 1e-6; identical bytes | met |
| S5 | 15 invalid sweeps give exit 3 with the exact located path and create nothing; bad command lines give exit 2 | all 15 and the three exit-2 cases | met |
| S6 | not converged point gives exit 5, the other rows recorded; `--list` runs nothing | exit 5, rows 1 and 2 ok, values of the failed row recorded | met |
| S7 | every point within 1 percent of `12 rho nu U L / h^2` | 1.91, 1.90, 1.88, 1.87, 1.86 percent low for U = 0.02 ... 0.1 (ny = 10) | MISS, `s7_poiseuille_map_known_miss` is ignored with the limit intact |
| P1 | equality of get/set and of the resolved settings | 10 settings equal, resolved JSON equal to `--set ... --print-settings`; a failed `set` leaves the case unchanged; `0.1 + 0.2` round-trips | met |
| P2 | 0 differing bytes | `monitors.csv`, `monitors.json`, `solution.vtu` identical; report equals the last CSV row exactly | met |
| P3 | same paths as the command line and as S5 | four settings errors and six study errors carry the CLI paths | met |
| P4 | same rows, same CSV bytes as the CLI | identical `table.csv` and `table.json` | met |
| P5 | marker nowhere | not found in any file, repr or str; only `RCFD_BIN` read from the environment | met |
| P6 | clean import under `python -I` | imports only standard-library modules and the package, no output | met |
| P7 | visible skip without Python | the harness test ran the Python tests (11 after review); the skip branch prints `SKIPPED: ...` (not exercised on this machine) | met (skip path untested) |

S7 diagnosis (not tuned): at 10 cells across the gap the pressure deficit is 1.9 percent and it falls to 0.29 percent at 20 cells
(`poiseuille_error_decreases_with_refinement`, a diagnostic added after the miss, not a replacement). The probable cause (consistent with the
refinement trend, not isolated by a separate experiment) is the first-order
treatment of the wall in the cell-centred finite-volume scheme (wall shear from `u_P / (dy/2)` of a parabolic profile is low by
`dy / (2 h)`, so the discrete channel is slightly more conductive), plus half a cell of pressure extrapolation at the inlet face
(about 0.5 percent of `dp`). The estimate in S7 ("a few tenths of a percent") ignored the wall-treatment term. The sweep is also
nearly linear in `U` (dp/U constant to 1e-3 over three values; the convective terms break exactness), also unregistered.

## Second addendum: gaps closed after review

* S2 as registered (1e-12 study): `s2_registered_tolerance_and_solver_threads`, `table.csv` and `table.json` byte-identical for
  `--parallel 1` and `3` (both exit 5). Also, reading "thread count" as the solver's worker threads: the 1e-9 study with
  `RAYON_NUM_THREADS=1` against `3` gives byte-identical tables.
* S3 as registered (1e-12 study): MISS. All rows are `failed` (exit 5) and resume retries failed rows, so the second launch
  measured run 18, skipped 0 against the registered 11 and 7. Kept as the ignored test `s3_registered_tolerance_known_miss`; the
  1e-9 test passes with the registered counts.
* S4 registered fallback (a mesh file that does not exist, `mesh.source` and `mesh.file` zipped): the failed point has exit code 3
  (`mesh.file: <path>: io error ... cannot find the file specified`), two other rows ok, study exit 3, as for the first variant
  (`s4_registered_variant_missing_mesh_file`).
* P2 on a cavity-class case (12x12 `cavity_2d`, simplec, 40 iterations, exit 5 at the cap, `physics.nu` and the lid velocity set on
  both sides): `monitors.csv`, `monitors.json` and `solution.vtu` byte-identical between `Case.run` and `rcfd run --set`
  (`P2bCavityRunEqualsCli`), so momentum and pressure are exercised too; the slab test stays.
* Bug found by review and fixed: `rcfd.study` raised `StudyError` for any exit code 2 or 3, including a study that ran and had a failed
  point (exit 3). It now raises only when the binary printed no summary (static sweep errors, usage errors); `P4bFailedPointThroughPython`
  covers the failed-point case (exit code 3, rows `ok failed ok`).
* S5 gained `kind = "cubic"` (located at `study.parameters.a.kind`), 16 invalid sweeps in total. The Python harness now counts 11 tests.
