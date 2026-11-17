# ADR 0710: F-769 registration: tables called from expressions and the checks of their test file

Status: **REGISTERED 2026-11-17, written before any run of the checks of [ADR 0705](0705-f769-registration-adrs-for-the-five-feature-groups-that-no-adr-describes.md)
(F-769, group 5 of 5).** Not yet accepted by the maintainer. Results are in the addendum of ADR 0705.

**These tests predate this ADR.** `crates/rcfd-core/tests/table_expr.rs` was committed on 2026-10-13 (`40473ad5`, "rcfd-core/api: lookup
tables and CSV profile boundary conditions (F-105)") with F-105 (wave 8, DONE); no ADR registered its limits. ADR 0036 lists table-file
laws as not delivered, so it does not describe this test. This ADR records what the file checks and which limits it carries, read from the
code and from the F-105 note of the roadmap. It is not a decision taken before the tests. No Rust file is edited and no test is run for it
(ADR 0705, Limits).

## The feature

F-105 ("profile and table inputs", acceptance: "a temperature-dependent viscosity table is interpolated correctly (unit test);
missing/out-of-range data raise errors") added `rcfd_core::table::Table` (CSV with bracketed header units, 1 to 3 inputs, linear,
multilinear grid or inverse-distance interpolation, error or clamp outside the range) and **table calls in expressions**:
`Context::add_table(name, table, output column)` makes `name(arg)` callable in an `Expr`; the arguments are dimension-checked against the
input units of the table, and the result carries the unit of the output column. The file checks only this last part, with one table of
one input.

## The table of the file

`nu_t`: input `T` [K], outputs `nu` [m^2/s] and `rho` [kg/m^3], three rows: 280 K, 1.4e-6, 999.9; 300 K, 0.85e-6, 996.5; 320 K, 0.58e-6,
989.9. Column 0 is registered as `nu_t` and column 1 as `nu_t_rho`.

## Limits carried by the file

| id | test | kind | limit | source |
|---|---|---|---|---|
| T1 | `table_calls_are_dimension_checked_and_interpolate` | exact | `nu_t(Tw) * nu_t_rho(Tw)` compiles with `Tw` of dimension temperature and the compiled expression has the dimension `KINEMATIC_VISCOSITY * DENSITY` | the assert |
| T2 | same | numeric | at `Tw = 310` K the value equals `(0.5 * (0.85e-6 + 0.58e-6)) * (0.5 * (996.5 + 989.9))`, the linear interpolation of both columns, with an **absolute** tolerance of **1e-15** (the bound is `1e-15 * max(|want|, 1.0)` and `|want|` is about 7.1e-4, so the factor is 1.0). That is a relative tolerance of about 1.4e-12 | the assert |
| T3 | same | exact | at `Tw = 400` K, outside the table (280 K to 320 K), the value is **NaN**: no silent extrapolation | the assert |
| T4 | same | exact | `nu_t(300 [K])` folds to a constant: `is_constant()` is true and `constant_value()` is exactly `Some(0.85e-6)` | the asserts |
| T5 | `bad_calls_are_rejected_with_a_span` | structural | compiling `nu_t(L)` with `L` a length gives an error whose message contains "must have dimension"; `nu_t(1 [K], 2 [K])` gives "takes 1 argument"; `nu_t(300)` (dimensionless) gives "must have dimension"; a table whose input column has no unit (`s`) called as `bare(0.5)` gives "has no unit" | the asserts |

The name of T5's test says "with a span", but the asserts read only the message text; the span of the error is **not** checked.

## What the file does not check

* Tables with two or three inputs, inverse-distance (scattered) interpolation, clamp mode, the unit of the result beyond T1, an output
  column other than the first two, a call with a wrong number of arguments for a 2-input table.
* The settings-tree route (`tables.<name>` in a case, `boundaries.<patch>.profile`, the sandbox of table files): those are checked by
  `rcfd-api` tests (`tests/tables.rs`, where the F-105 note records a Poiseuille inlet at 0.66 % against a limit of 2 % and a pressure
  drop at -1.7 % against 3 %; those numbers are not in this file).
* That out-of-range values during a transient run are trapped: the F-105 note says only the build at t = 0 is.
* The NaN of T3 is the value of the compiled expression at evaluation; whether a case run stops on it is not tested here.
