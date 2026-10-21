# ADR 0095: Wave 39D, partials close-out: field-threshold registers (F-124), monitor plots during a run (F-144), fan and vent curves (F-217)

Status: registered before any F-124 code was written and before the F-144 and F-217 tests were added to the repository. Limits are not changed
after a run; a miss becomes an ignored `*_known_miss` test with the limit intact. Baseline hashes are not re-recorded; the new register type is
opt-in, so a case without it builds and runs exactly as before. Clean-room: first principles only. Written at HEAD 819fc0f (scratch copy
`<scratch dir>`, target `<target dir>`).

The residual gaps come from the partials audit: `docs/audit/partials-group-4.md` (F-124) and `docs/audit/partials-group-7.md` (F-144, F-217).
I re-checked each claim against the code before writing this ADR: `crates/rcfd-api/src/registers.rs` has no register type that reads a field;
`crates/rcfd-gui/tests/monitors_plot.rs` m5 never compares two polls; `crates/rcfd-api/tests/heat_exchanger.rs` X4 runs one operating point and
no test in that file runs a vent.

## What I knew when I wrote the limits

The F-144 and F-217 smoke runs (rule d) were full runs, so their measured values were known when this ADR was written. To keep that from
steering the limits, every limit below is either taken from an earlier registered limit (ADR 0025 for the frame time, ADR 0039 for the fan, ADR
0022 for the vent) or is exact (counts, bit identity). None comes from the smoke numbers.

## F-124: `type = "field"` registers

Decision. A new register type `field` with keys `field` (one of `velocity_magnitude`, `ux`, `uy`, `uz`, `pressure`, `temperature`) and the
existing `above` and `below` (inclusive, 1e300 means unbounded). It selects the cells whose value lies in [above, below]. The field values come
from the **initial fields**. They are taken once, in `Simulation::new`, after the uniform values, `initial.velocity_expression_*` and the
user-function `initialise` hook, and before any `initial.patches` are applied. Pressure is the kinematic pressure the solver stores. That one
mask is used by every `initial.patches.<n>` entry and by `sources.<n>.zone = "register"`. The register is not re-evaluated during the run.

Limits of the semantics, stated in the schema doc and in docs/case-schema.md:
- `initial.method = "potential"` or `"coarse"` computes its fields after the patches. A field register combined with those methods gets a
  located error at `registers.<n>.type`.
- A restart (`continue-from`) is restored after construction, so the register keeps the mask of the case's own initial fields.
- Only `sources` and `initial.patches` (velocity, pressure, temperature) receive the initial fields. Any other consumer (zone sources, lumped
  components, solids, regions, turbulence-variable patches) gets a located error at `registers.<n>.type` that names the two supported uses. It
  never gets an empty or all-cells zone.

Registered checks (`crates/rcfd-api/tests/registers.rs`). The case is the existing 20 x 20 `box_2d` unit square with
`initial.velocity_expression_x = 4 [m/s] (y/1 m)(1 - y/1 m)` and register `fast`: `type = "field"`, `field = "velocity_magnitude"`,
`above = 0.75`.

| # | Check | Limit |
| --- | --- | --- |
| G1a | cell count of `fast` | exactly 200. Analytic: 4y(1-y) >= 0.75 for \|y - 0.5\| <= 0.25, so rows j = 5..14 of the centres (j + 0.5)/20 qualify. The nearest centres (0.6975 at y = 0.225, 0.7975 at y = 0.275) are far from the threshold, so there is no tie |
| G1b | mask of `fast` built through the settings tree | equal, cell by cell, to `rcfd_post::registers::Register::threshold` applied to \|u\| of `sim.field("ux"/"uy"/"uz")` with limits [0.75, inf) |
| G1c | `initial.patches.hot` (pressure 7.5 in `fast`) | p = 7.5 on exactly the 200 cells; every other cell is bit-identical to the case without the patch |
| G1d | porous sink (`darcy_forchheimer`, permeability 0.01) with `zone = "register"`, `register = "fast"`, through `Sources::from_case` with the initial fields | non-zero on exactly the 200 register cells, bit-zero elsewhere |
| G1e | end to end: 5 SIMPLEC iterations of the field-register case against the same case with a box register on y in [0.25, 0.75] (the same 200 cells) | ux, uy and p bit-identical; and max \|ux\| difference from the patched case without the source > 1e-6, so the comparison is not vacuous |
| G2 | errors | a field register used by `zone_sources` gives an error at `registers.<n>.type`; with `initial.method = "potential"` an error at `registers.<n>.type`; `above > below` an error at `registers.<n>.above` |

Smoke (rule d): the set-up is the R6/R7 case of the same file, which runs. The G1a count is analytic, and G1b checks it against the library
threshold. The new type cannot run before it is written, so its first run is the registered run.

Status rule. If G1 and G2 pass, the acceptance as written ("a region defined by a field threshold can be patched with a value and used to
restrict a source term; registers are named objects in the settings tree") is met and F-124 becomes `done`. The note will say that the title's
"adaption" is marks only and that re-evaluation during a run is not offered.

## F-144: plots update while a run is in progress

Registered check M7 (`crates/rcfd-gui/tests/monitors_plot.rs`, `m7_plot_series_grows_and_frames_draw_while_running`). The set-up is the
lid-driven cavity template with `solver.max_iterations = 200000` and `tolerance_u = tolerance_p = 1e-14` (the schema floor), so the run lasts
far longer than the test. The test starts the run and wraps the controller in `RcfdApp` with the Monitors panel floated. It then draws headless
egui frames (1280 x 800) every 20 ms, about 20 ms apart. The test never polls the controller itself: the only polls are the app's own 250 ms
`tick`. For every frame drawn while the status is `running`, it records the frame time, the number of shapes and the res_u series length. It
stops after five distinct lengths or 60 s, then stops the run.

| # | Check | Limit |
| --- | --- | --- |
| M7a | distinct res_u series lengths seen in state `running` | at least 4 values, strictly increasing (three growth events between polls) |
| M7b | the last length seen while running | below 200000 (the run was mid-way) |
| M7c | frames drawn in state `running` | at least 3 |
| M7d | worst frame time in state `running` | below 1 s (the ADR 0025 non-blocking limit used by m5) |
| M7e | shapes produced by each running frame | more than 0 |

Smoke run: lengths [0, 205, 426, 675, 914], 49 running frames, worst frame 8.9 ms, at least 569 shapes per frame, 1.1 s in total. The first
length is 0, the value before the first poll, so M7a counts only lengths above 0.

Status rule. If M7 passes, all three clauses are measured: plots update during a cavity run (M7a, M7b, with the frame drawn), the UI thread
never blocks (M7d plus the existing m5), and incremental merge is unit-tested (existing m4). F-144 then becomes `done`. Human viewing is not
part of the acceptance.

## F-217: fan and vent curves at several operating points

**X6 fan.** `x6_fan_boundary_at_three_operating_points` (`crates/rcfd-api/tests/heat_exchanger.rs`) uses the corrected X4 set-up of ADR 0039
(fan boundary at xmin with the curve (0, 200), (0.03, 140), (0.06, 0) Pa, porous jump, Darcy bed over the whole duct). It moves the operating
point by changing only the bed permeability: k_bed = 3e-7, 1e-6 (the existing X4 point) and 1e-5 m^2. Per point, the ADR 0039 limits apply
unchanged: Q within 5e-3 of the bisected analytic operating point, and patch pressure within 2e-3 of the curve at Q. The patch-pressure check
holds by construction, because the boundary sets the patch pressure from the curve. The check that actually tests the curve is Q against the
analytic operating point, which uses the curve, the jump and the bed. In addition, one point must lie on each linear segment (Q < 0.03 and
Q > 0.03), and the flows must increase by more than 5 percent from point to point.

Smoke run: Q = 1.9293e-2, 3.0515e-2 and 3.6212e-2. The Q errors are 1.13e-3, 6.35e-4 and 8.6e-5; the curve errors are at most 1.8e-13.

**X7 vent, on its own set-up.** It does not use the F-200 channel of `bc_catalogue.rs`. The X1 duct of `heat_exchanger.rs` is periodic in y
(no side walls, plug flow), 100 x 4 cells, 2 m x 0.2 m, rho 1.2, and has these boundaries:
- xmin: a fixed inlet velocity u. When u is negative, the flow enters through the vent.
- xmax: `bc_kind = "vent"` with `loss_coefficient = 2.5` and `bc_pressure = 0`.

The initial velocity is u. The run uses pseudo-time stepping (`pseudo_mode = "physical"`, `pseudo_dt = 0.02`) and the file's `run` helper
(2000 iterations plus a 300-iteration stationarity check at 1e-8). The flows are u = 0.5, 1, 2, -0.5, -1 and -2 m/s, all on this one set-up.
The vent law is the kinematic face pressure p_ext/rho + 0.5 K u_n |u_n|. In a frictionless plug flow the pressure is uniform, so the mean
pressure of the outlet column must equal 0.5 K u |u|.

Limit: relative error at most 1e-6, the ADR 0022 C3 limit. The pressure must also be finite, because the `run` helper's max-fold skips NaN
and so a NaN field would pass its stationarity check. That weakness is reported here; the shared helper is not changed. Any limit below 0.5
separates the quadratic law from a linear one: at |u| = 0.5 the linear law gives twice the quadratic value, and at |u| = 2 half of it.

Smoke runs (rule d):
- Plain SIMPLEC, forward flow: diverges. At K = 2.5 and u = 1, max |ux| is 6.06, 1.8e2 and 1.8e5 after iterations 0, 1 and 2, and the fields
  are NaN by iteration 197. With K = 0 the same set-up stays at ux = 1 and p of order 1e-13.
- Plain SIMPLEC, reversed flow: settles, with relative errors up to 2.8e-13.
- Pseudo-time 0.02, forward u = 1, K = 2.5: settles at p = 1.250000. This is why pseudo-time is registered for all six flows. The forward
  divergence of plain SIMPLEC with a velocity inlet and a vent outlet is a robustness finding (the same class as the F-200 C3 registered
  miss). It is reported in the roadmap note and not fixed here.

The pseudo-time step size happens to match the one F-200's C3 used. The geometry, inlet kind, file and flow rates are independent of it.

Status rule. If X6 and X7 pass, the acceptance ("counterflow exchanger reproduces the effectiveness-NTU analytic result within 2 percent; fan
and vent boundaries reproduce their prescribed pressure-flow curves") is met: counterflow by the existing X1, fan by X4 and X6, vent by X7.
F-217 then becomes `done`. The note will keep the "algebraic macro model, not a resolved exchanger" caveat and the UNVERIFIED shell-and-tube
relation, which is outside the acceptance.

## Proposals

None. No acceptance text needs rewording for these three items.

## Amendment 1 (written after the first registered X7 run, before any further vent run)

What happened: the registered X7 run passed at u = 0.5 (relative error 2.85e-12) and u = 1 (1.36e-13), then gave NaN at u = 2 and stopped,
so the reversed flows did not run. The smoke runs had tried pseudo-time only at u = 1. That registered test stays, ignored, as
`x7_vent_at_fixed_pseudo_dt_known_miss` with its limit unchanged.

Hypothesis: the pseudo-time step 0.02 is a Courant number u dt / dx of 1 at u = 1 (dx = 0.02) but 2 at u = 2, which is too large for the
forward vent. In the frictionless plug flow, the case at speed u with step dt/|u| is the u = 1 case rescaled (pressure scales with u^2, time
with 1/|u|), so a fixed Courant number of 1 is the same set-up at every flow rate.

Corrected set-up (new test `x7_vent_reproduces_its_quadratic_loss_law`): `pseudo_dt = 0.02 / |u|`, so the Courant number is 1 for all six
flows. Everything else is unchanged: geometry, inlet, K = 2.5, the run helper and the 1e-6 limit with the finite check. If u = 2 still gives
NaN at Courant 1, the hypothesis is falsified, the corrected test also becomes a known miss, and F-217 stays partial.

## Addendum: measured values

All runs are release builds in the scratch copy at the HEAD named above, plus this wave's files.

| Check | Measured | Limit | Result |
| --- | --- | --- | --- |
| G1a field register count | 200 | exactly 200 | pass |
| G1b mask equals the library threshold of \|u\| | equal | equal | pass |
| G1c cells changed by the pressure patch | 200, all 7.5; the rest bit-identical | exactly the register | pass |
| G1d non-zero porous sink cells | 200; the rest bit-zero | exactly the register | pass |
| G1e field register against box register after 5 iterations | 0 differing cells in ux, uy, p; the source changes ux by up to 0.164 | 0, and > 1e-6 | pass |
| G2 located errors (zone_sources consumer, `initial.method = "potential"`, above > below) | at `registers.fast.type`, `registers.fast.type`, `registers.fast.above` | located | pass |
| M7a res_u lengths while running | 186, 367, 540, 729 (a second run: 205, 426, 675, 914) | at least 4, strictly increasing | pass |
| M7b last length while running | 729 | below 200000 | pass |
| M7c frames in state running | 48 | at least 3 | pass |
| M7d worst frame | 9.1 ms | below 1 s | pass |
| M7e shapes per running frame | at least 569 | more than 0 | pass |
| X6 Q against the operating point (k_bed 3e-7, 1e-6, 1e-5) | 1.13e-3, 6.35e-4, 8.6e-5 (Q = 1.9293e-2, 3.0515e-2, 3.6212e-2) | 5e-3 each | pass |
| X6 patch pressure against the curve | at most 1.8e-13 | 2e-3 | pass (by construction) |
| X6 segments and spacing | 0.0193 < 0.03 < 0.0362; each step above 5 percent | as registered | pass |
| X7 as registered (pseudo_dt 0.02) | u 0.5: 2.85e-12, u 1: 1.36e-13, u 2: NaN; reversed flows not reached | 1e-6 and finite | **miss**, ignored `x7_vent_at_fixed_pseudo_dt_known_miss` |
| X7 corrected (amendment 1, Courant 1) | u 0.5, 1, 2, -0.5, -1, -2: 1.44e-14, 1.36e-13, 3.65e-13, 3.94e-13, 9.74e-14, 1.33e-13 | 1e-6 and finite | pass |

Deviations: one, X7 amendment 1. It was written after the failed registered run and before the corrected run. When I wrote it I knew that
u = 0.5 and u = 1 pass at pseudo_dt 0.02, and that plain SIMPLEC reversed flow settles. The Courant-1 hypothesis was not falsified: u = 2 at
pseudo_dt 0.01 settles at 3.65e-13.

Findings reported but not fixed:
- Plain SIMPLEC with a velocity inlet and a forward vent outlet diverges from the first iteration when K > 0. This is the same class as the
  F-200 C3 registered miss.
- The `run` helper in `heat_exchanger.rs` takes the maximum change with a max-fold, which skips NaN, so a NaN field passes its stationarity
  check. X7 checks finiteness itself. The helper is unchanged, so the other tests behave exactly as before.

Statuses: F-124, F-144 and F-217 meet their acceptance text as written and are set to `done`. No acceptance text was reworded.
