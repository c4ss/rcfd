# ADR 0102: Wave 41B, SIMPLEC with a velocity inlet and a forward vent (F-248): diagnosis and the boundary compensation

Status: registered before any diagnostic switch run and before any run of the fix. Limits are not changed after a run; a miss becomes an
ignored `*_known_miss` test with the limit intact. Defaults stay bit-identical: any fix is opt-in, and `crates/rcfd-api/tests/baseline_hashes.rs`
is not re-recorded. Clean-room: first principles only (the SIMPLEC velocity correction of Van Doormaal and Raithby, 1984, Numer. Heat Transfer
7, 147-163, UNVERIFIED as a citation; everything used here is re-derived below). Written at HEAD e325ab4 (scratch copy
`<scratch dir>`, target `<target dir>`).

## The defect

F-248 (found by wave 39D, ADR 0095): with plain SIMPLEC, a fixed inlet velocity and a vent outlet with K > 0 diverge from the first
iteration. The registered X7 test (`x7_vent_at_fixed_pseudo_dt_known_miss`, pseudo-time step 0.02) gives NaN at u = 2. ADR 0095 amendment 1
blamed the Courant number (2 at u = 2); it changed the test to a fixed Courant number of 1, which passes, but did not find the cause.

## What I already knew when writing this (smoke runs, default code, rule d)

Set-up of every run here: the X7 duct of `crates/rcfd-api/tests/heat_exchanger.rs` (100 x 4 cells, 2 m x 0.2 m, depth 0.02 m, periodic in y,
rho 1.2, nu 1.5e-5, upwind, SIMPLEC with the defaults alpha_u 0.9 and alpha_p 1, fixed inlet velocity u at xmin, vent K = 2.5 and p_ext 0 at
xmax, initial velocity u). Measured with a scratch harness that prints, per iteration, max |ux - u| and where it is:

* Plain SIMPLEC, u = 1: max |ux - u| = 5.057, 1.83e2, 1.83e5, 2.0e9 after iterations 1 to 4, always in the vent column (x = 1.99). The
  outlet-column pressure is 0.625 after iteration 1 (the vent pressure is relaxed: 0.5 of the construction value 0 plus 0.5 of 1.25).
  Rows stay nearly equal after iteration 1 (largest difference between cells of one column 6e-3, max |uy| 5.4e-6).
* Pseudo-time 0.02, u = 2: max |ux - u| = 1.125, 2.30, 3.90, 7.01, 17.3, 93.0, 2.5e3 after iterations 1 to 7, in the vent column; NaN at
  iteration 194.
* Pseudo-time 0.02, u = 1: 0.230 after iteration 1, then it decays (ADR 0095: it settles).
* From ADR 0095: K = 0 stays at the exact solution; reversed flow settles.

So the "inlet pressure treatment" suspect is already out: the first-iteration kick is in the vent column, not at the inlet (x = 0.01).

## Mechanism (derived from the code before any switch run)

The steady solver (`crates/rcfd-fv/src/simple.rs`, `iterate`) does this with SIMPLEC:

1. Momentum predictor with the old pressure; its Green-Gauss gradient uses the boundary pressure of this iteration (the vent value set by
   `bc_cat::Catalogue::refresh` just before the iteration).
2. Cell velocity: `u = HbyA - (rAU - rAtU) grad(p_old) - rAtU grad(p_new)`, with `rAU = V / a_P` and `rAtU = V / (a_P - sum|a_nb|)`.
3. Face flux: internal faces get the compact compensation `(rAtU - rAU)_f snGrad(p_old) |S|` and then `- rAtU_f snGrad(p_new) |S|`.
   **Boundary faces get no compensation**: the flux of a fixed-pressure face (and of a periodic seam face) is
   `HbyA_P . S - rAtU_P snGrad_b(p_new) |S|`.

At a vent face the pressure equation sets the flux to the inlet flux, so the cell pressure next to the vent follows the vent value
(`p_new` there is about `p_b`, so `grad(p_new)` in the vent cell is about 0). The cell velocity there keeps `+(rAtU - rAU) grad(p_old)`,
and `grad(p_old)` contains the jump between the new vent value and the old cell pressure: `(p_b - p_w,old) A / V`. Nothing in the face fluxes
carries that term, so it is a pure kick of the cell velocity. Numbers of the plain u = 1 run (V = 2e-5, A = 1e-3, flux 1e-3, alpha_u 0.9;
a_P = 1.111e-3, sum|a_nb| = 1.0008e-3): rAU = 0.0180, rAtU = 0.181, jump 0.625, gradient 31.25: kick (rAtU - rAU) 31.25 = 5.06; measured 5.057.
With pseudo-time 0.02 (V/dt = 1e-3 in a_P): 0.230 at u = 1 (measured 0.230) and 1.125 at u = 2 (measured 1.125).

The vent law reads the vent face velocity, which is the cell velocity (zero-gradient condition). A kicked cell velocity raises the next vent
pressure (0.5 of the old value plus 0.5 of `0.5 K u|u|`: 0.5 x 0.625 + 0.5 x 1.25 x 6.06^2 = 23.3; measured 23.2), whose jump kicks the
cell velocity again. Linearised, with `e_n` the vent-pressure error, `c = (rAtU - rAU) A / V` and the lag weight 0.5:
`e_{n+1} = (0.5 + G) e_n - G e_{n-1}` with the loop gain `G = 0.5 c K |u|`. Its roots are inside the unit circle exactly when G < 1.
G = 10.1 for plain SIMPLEC at u = 1, 1.125 for pseudo-time 0.02 at u = 2 (diverges), 0.46 at u = 1 and 0.46 at u = 2 with step 0.01
(settle). Reversed flow: the vent cell's upwind neighbour is the boundary, so `sum|a_nb|` is only the diffusion part, rAtU is about rAU and
there is no kick. K = 0: the vent value never changes, so there is no jump.

So the trigger is the uncompensated SIMPLEC term at the boundary face (the "seam compensation" defect of ADR 0084 finding S is the same
omission on periodic faces) and the amplifier is the vent law reading the kicked cell velocity. The Courant number of ADR 0095 is a proxy:
under the u-rescaling of the plug flow it is proportional to G, but G also depends on K.

## Diagnostics (scratch switches in the scratch copy only, removed before any commit)

Each runs the harness for 30 iterations (plain SIMPLEC u = 1, K = 2.5) and for 300 iterations (pseudo-time 0.02, u = 2, K = 2.5), unless
stated. "Kick" is max |ux - u| after iteration 1; "settles" means finite throughout and max |ux - u| <= 1e-2 at the last iteration;
"diverges" means non-finite or max |ux - u| > 1e3 at any iteration.

| Id | Switch | Suspect it tests | Prediction (mechanism above) | Falsified if |
| --- | --- | --- | --- | --- |
| D1 | boundary compensation (the fix candidate below) on fixed-pressure and periodic faces | SIMPLEC compensation missing on boundary faces | kick <= 1e-4 \|u\| (round-off and solver tolerance), settles | kick > 1e-4 \|u\| or does not settle |
| D1s | D1 on fixed-pressure faces only (periodic seams without it) | seam compensation | same as D1 within 1e-6 in the kick, settles | the kick differs by more than 1e-6 from D1 or it does not settle |
| D2a | vent pressure frozen at the exact 0.5 K u\|u\| | vent loss law (the feedback) | the kick stays (> 1, from the jump 0 -> 1.25 at plain u = 1), then settles | does not settle |
| D2b | vent law with the face velocity phi/\|S\| instead of the cell velocity | vent loss law | kick unchanged (5.057 at plain u = 1), settles | does not settle |
| D3 | `rAtU := rAU` (alpha_u 0.9, alpha_p 1 kept) | SIMPLEC consistent coefficient | kick <= 1e-4 \|u\| | kick > 1e-4 \|u\| |
| G1 | default code, pseudo-time 0.02, u = 2, K = 1 (G = 0.45, Courant 2) | gain model against the Courant hypothesis | settles (Courant hypothesis: diverges) | diverges |
| G2 | default code, pseudo-time 0.02, u = 1, K = 10 (G = 1.84, Courant 1) | same | diverges (Courant hypothesis: settles) | settles |

Reading: the trigger is "the boundary SIMPLEC term" if D1 and D3 stand; the amplifier is "the vent law on the cell velocity" if D2a and
D2b stand; the seam part plays no role in X7 if D1s stands; the gain model replaces the Courant explanation if G1 and G2 stand. Every
outcome is recorded, falsified or not.

## Fix (choice rule fixed now)

Candidates, both opt-in (each changes results it touches, so neither can be the default):

* **B** `solver.simplec_boundary_compensation` (bool, default `false`; steady `simplec` only, a located error with any other algorithm,
  no effect with `simple`): add `(rAtU - rAU)_f snGrad_b(p_old) |S|` to `phiHbyA` on every boundary face whose flux the pressure sets, with
  the boundary forms the corrected flux already uses (owner value and the pressure condition's gradient on fixed or mixed pressure faces;
  the distance-weighted pair mean and the partner-cell difference on periodic faces). Then `rAtU_f - (rAtU - rAU)_f = rAU_f` on every face,
  as on internal faces. In the first iteration this cancels the kick exactly (to the solver tolerances), and at a fixed point every face
  carries `rAU`.
* **F** a vent-level key that evaluates the vent law with the face velocity phi/|S|. It removes the amplifier, not the trigger.

Choice rule: B if D1 and D3 stand. Else F if D2b stands. Else no fix (diagnosis only; X7 stays a known miss). B and F are not combined.

Why B also covers the seam defect of ADR 0084 (finding S): periodic faces are boundary faces in this code; B adds the same term there.
That part is checked by B2 below, not by X7 (the X7 flow is one-dimensional).

## Registered acceptance checks (run once each after the fix is in)

* **A1 `x7_vent_at_fixed_pseudo_dt`** (non-ignored, `heat_exchanger.rs`): the X7 sweep exactly as registered in ADR 0095 (pseudo-time
  0.02 at every flow, u = 0.5, 1, 2, -0.5, -1, -2, K = 2.5, the `run` helper: 2000 iterations plus the 300-iteration stationarity check at
  1e-8 with the finite check) with the one added line `simplec_boundary_compensation = true` in `[solver]`. Limit: outlet-column pressure
  within 1e-6 relative of 0.5 K u|u| and finite at all six flows (ADR 0022 C3 limit, unchanged). The ignored
  `x7_vent_at_fixed_pseudo_dt_known_miss` stays as it is (default code) and is run once to show it still gives NaN at u = 2.
* **A2 B1 / B2** (`crates/rcfd-api/tests/simplec_boundary_w41b.rs`, non-ignored): an exact identity. SIMPLE and SIMPLEC with the same
  alpha_u build the same momentum matrix, so their fixed points are the same once every face carries `rAU` (cells:
  `HbyA - (rAU - rAtU) grad p - rAtU grad p = HbyA - rAU grad p`; faces: `rAtU_f - (rAtU - rAU)_f = rAU_f` with the same interpolation
  weights). Set-up: duct 2 m x 0.5 m, 40 x 10 cells, nu 0.02, upwind, inlet 1 m/s at xmin, fixed pressure 0 at xmax, a Darcy block
  (permeability 1e-3) at x in [0.8, 1.2], y in [0.1, 0.3]. B1: no-slip walls at ymin and ymax. B2: periodic in y (the block is off the
  seam's symmetry line, so the seam faces carry pressure differences). Runs: `simple` (alpha_u 0.7, alpha_p 0.3), `simplec` (alpha_u 0.7,
  alpha_p 1) and `simplec` + the key, each iterated in blocks of 100 until the largest change of ux, uy and p over a block is below 1e-13 of
  the field's largest magnitude (cap 40000), linear solver tolerances 1e-12. Limits: compensated SIMPLEC against SIMPLE at most 1e-9 (ux and
  uy relative to max |ux|, p relative to the pressure range); default SIMPLEC against SIMPLE above 1e-6 (the check must see the defect);
  every run settles. The 1e-9 bound: the exact difference is 0; a run stopped at a block change of 1e-13 is within about 1e-13 / (1 - rho^100)
  of its fixed point, and rho^100 < 0.5 for runs that settle in a few hundred iterations, so 1e-9 leaves four orders for round-off in the
  pressure range. Smoke (default code only, rule d): simple settles in 400 (B1) and 300 (B2) iterations, default simplec in 400 and 400;
  default SIMPLEC against SIMPLE: B1 u 4.80e-3, p 3.74e-3; B2 u 2.62e-3, p 4.05e-3.
* **A3 default bit-identity**: `baseline_hashes.rs` passes unchanged; the workspace gate has 0 failures; the key is read only into a field
  that is `false` unless set, and the added code runs only when it is `true`.
* **A4 located error**: `simplec_boundary_compensation = true` with `algorithm = "piso"` is an error at `solver.simplec_boundary_compensation`.

Status rule for F-248: done iff A1 passes as a non-ignored test and A3 holds. The acceptance text says "with plain SIMPLEC, without changing
the default settings of existing cases": A1 uses the SIMPLEC algorithm with one opt-in key and no other change; that reading is stated in the
F-248 note.

## Reported checks (not part of the status rule)

* R1 X7 without pseudo-time (plain SIMPLEC, alpha_u 0.9), six flows, with the key: measured and reported, no limit.
* R2 the F-200 C3 forward vent as registered in ADR 0022 (plain SIMPLEC from rest, mass-flow inlet, K = 2.5 and 10) with the key, run once
  with the C3 settled-field criterion and the C3 limits (`c3_check`); recorded only, F-200's entry is not touched.
* R3 (diagnosis only, last priority, step 3 of the task): the stalled residual of ADR 0022 E2 and the FAN-01 laminar stall named in the
  wave 36-38 ADRs. Only if time remains; no fix.

## Diagnostic results (registered runs, as registered)

"Kick" = max |ux - u| after iteration 1 (where); "last" = max |ux - u| after the last iteration (30 plain, 300 pseudo).

| Id | Plain SIMPLEC u = 1, K 2.5 | Pseudo 0.02, u = 2, K 2.5 | Verdict |
| --- | --- | --- | --- |
| default | kick 5.057 (x 1.99), NaN path (1.8e5 at iteration 3) | kick 1.125 (x 1.99), NaN at iteration 194 | (smoke) |
| D1 | kick **1.553e-4** (x 1.97), last 2.24e-5 | kick 5.70e-5 (x 1.97), last 9.5e-13, p 5.000000 | **falsified by its threshold at plain u = 1** (1.553e-4 > 1e-4); stands at pseudo u = 2 |
| D1s | kick 1.553e-4, last 2.24e-5 (equal to D1 to the printed 7 digits) | kick 5.70e-5, last 1.7e-12 | stands (seams play no role in X7) |
| D2a | kick 10.11 (x 1.99), last 5.7e-4 | kick 2.250, last 1.1e-12 | stands |
| D2b | kick 5.057, then **diverges** (8.0e7 at 30, rows differ by 3e14) | kick 1.125, **does not settle** (0.85 at 300, rows differ by 3.1) | **falsified** |
| D3 | kick 9.93e-5 (x 1.95) (then diverges: SIMPLE coefficients with alpha_p 1, not part of the prediction) | kick 4.91e-5, last 6.1e-10 | stands |
| G1 | - | K = 1: kick 0.450, last 2.75e-12 (settles at Courant 2) | stands: the Courant hypothesis of ADR 0095 is falsified |
| G2 | - | u = 1, K = 10: kick 0.921, NaN at iteration 100 (diverges at Courant 1) | stands |

Readings:

* Inlet: out (smoke, the kick is in the vent column). Seam: out (D1s = D1; the X7 flow is one-dimensional).
* The vent law is the amplifier (D2a: a frozen vent pressure takes the full kick, 10.1, and settles). Evaluating the law with the face
  velocity (candidate F) is **not** a cure: it diverges through a transverse mode. My reading, not tested: the per-face flux of a
  fixed-pressure face responds to its own face pressure with the gain rAtU |S| / d, so a per-face law `p_b(phi_b)` closes a face-by-face loop
  of gain about rAtU K |u| / d (45 at plain u = 1); the cell-velocity law averages over the cell and does not have this loop.
* The trigger is the SIMPLEC term `(rAtU - rAU) grad(p_old)` (D3 removes the kick) left uncompensated at the boundary face (D1 removes it to
  1.6e-4 of its size at plain u = 1 and to 5e-5 of it at pseudo u = 2, and both runs settle).
* G1 and G2: the stability limit is the loop gain G = 0.5 (rAtU - rAU) (A/V) K |u| < 1, not the Courant number.

The choice rule as registered ("B if D1 and D3 stand; else F if D2b stands; else no fix") gives **no fix**, because D1 missed its
threshold at plain u = 1 by a factor 1.55 and D2b is falsified.

## Amendment 1 (written after the diagnostic runs above, before any further run and before any acceptance run)

What I knew when writing it: every result in the table above. Nothing else has been run with the switches or the fix.

The D1 threshold 1e-4 |u| assumed that the only thing left after the compensation is solver tolerance. Reading the code after the run, a
second term exists: the momentum predictor decelerates the vent cell with `rAU grad(p_old)` (0.56 at plain u = 1), which is correct
SIMPLEC behaviour, and with `h_operator = "from_solution"` the neighbour cell's `HbyA` takes `a_E` times that deceleration (`a_E` is the
viscous coefficient only, upwind), so cell N-1 is off by about `(nu A / dx) / a_P x 0.56` = 7.5e-7 / 1.11e-3 x 0.56 = 3.8e-4 before the
pressure step. The measured 1.553e-4 is in cell N-1 (x = 1.97), not in the vent cell. If this is right, the residual kick is proportional to
the viscosity and is not the boundary defect.

Corrected diagnostic **D1v** (plain SIMPLEC, u = 1, K = 2.5, D1 switch on, 30 iterations), two runs:
* nu = 1.5e-9 (10^-4 of X7's): prediction kick <= 1e-6 (falsified if larger);
* nu = 1.5e-4 (10 x X7's): prediction kick between 5e-4 and 5e-3, i.e. about 10 x 1.553e-4 (falsified if outside), and settles.

Amended choice rule: **B** if D1v stands (both runs) and D3 stands (it does); otherwise no fix. F is out either way (D2b falsified). The
original D1 verdict stays recorded as falsified by its threshold. The acceptance checks A1 to A4 and their limits are unchanged.

## Amendment 2 (step 3 diagnosis; written after D1v, A1-A4, R1 and R2 had run, before the run it governs)

What I knew when writing it: the D1v, A1-A4, R1 and R2 results (in the addendum below). No FAN-01 run has been made in this wave.

R3 (diagnosis only, no fix): ADR 0084 reports that FAN-01 (the ADR 0083 A0 set-up, FAN-01-C `camber_seam`, laminar, SIMPLEC with
`velocity_correction = "face_flux"`) stalls with `res_u`, `res_p` between 1e-3 and 7e-3 from iteration 100 on. That mesh has periodic seams
and a pressure outlet, so the missing boundary compensation is a candidate. Hypothesis H-R3: the stall is caused by the missing SIMPLEC
compensation on the seam and outlet faces. Diagnostic: the same 1500-iteration run (`w37a_fan01_with_face_flux_velocity_correction` with
the one added line `simplec_boundary_compensation = true`, as a new ignored report test), run once. H-R3 stands iff `res_u` and `res_p`
are both below 1e-4 at some iteration up to 1500; otherwise it is falsified (the stall has another cause). Reported only.

## Amendments

Amendments 1 and 2 above.

## Addendum: measured values, verdicts, deviations

Release builds of the scratch copy at e325ab4 plus this wave's files. Each registered run was made once unless stated.

### Diagnosis (amendment 1)

| Id | Measured | Prediction | Verdict |
| --- | --- | --- | --- |
| D1v nu 1.5e-9 | kick 1.59e-8 (x 1.97), last 2.5e-9 | <= 1e-6 | stands |
| D1v nu 1.5e-4 | kick 1.534e-3 (x 1.97), last 2.1e-4 | 5e-4 to 5e-3, settles | stands |

The kick left by the compensation is exactly proportional to the viscosity (1.59e-8, 1.553e-4, 1.534e-3 for nu 1.5e-9, 1.5e-5, 1.5e-4): it is
the viscous coupling of the momentum predictor, not the boundary defect. By the amended choice rule the fix is **B**, the opt-in key
`solver.simplec_boundary_compensation` (`crates/rcfd-fv/src/simple.rs` `simplec_boundary_flux_correction`; schema and located error in
`crates/rcfd-api/src/schema.rs` and `sim.rs`; documented in `docs/case-schema.md`).

Root cause in one sentence: SIMPLEC compensates `(rAtU - rAU) grad(p_old)` on internal faces only, so when the vent pressure changes the
vent-cell velocity takes the uncompensated `(rAtU - rAU)` times the jump (5.06 m/s in the first plain iteration at u = 1), and the vent law,
which reads that velocity, feeds it back with the gain G = 0.5 (rAtU - rAU) (A/V) K |u|; the run diverges when G > 1 (10.1 plain at u = 1,
1.125 with pseudo-time 0.02 at u = 2), whatever the Courant number (G1, G2).

### Acceptance

| Check | Measured | Limit | Result |
| --- | --- | --- | --- |
| A1 `x7_vent_at_fixed_pseudo_dt` (key on, pseudo 0.02) | u 0.5: 2.93e-12, 1: 1.27e-13, 2: 1.91e-14, -0.5: 2.2e-16, -1: 2.49e-13, -2: 6.42e-14; all finite and stationary | 1e-6, finite | **pass** |
| A1 default (`x7_vent_at_fixed_pseudo_dt_known_miss`, run once) | u 0.5: 2.85e-12, u 1: 1.36e-13 (the ADR 0095 values, equal in the printed digits), u 2: "p is not finite" | stays a miss | as expected; stays ignored |
| A2 B1 walled duct | key vs simple: u 3.48e-12, p 2.74e-12; default simplec vs simple: u 4.80e-3, p 3.74e-3; settled in 400 / 400 / 500 iterations | <= 1e-9; > 1e-6; settle | **pass** |
| A2 B2 periodic duct | key vs simple: u 4.69e-13, p 4.30e-13; default: u 2.62e-3, p 4.05e-3; settled in 300 / 400 / 400 | <= 1e-9; > 1e-6; settle | **pass** |
| A3 default bit-identity | `baseline_hashes.rs` 3 passed (not re-recorded); gate 0 failed | unchanged | **pass** |
| A4 key with `piso` | located error at `solver.simplec_boundary_compensation` | located | **pass** |

### Reported checks

* R1 X7 with the key and no pseudo-time (plain SIMPLEC, alpha_u 0.9), 2300 iterations each, scratch harness: outlet pressure relative error
  7.4e-14 (u 0.5), 4.1e-13 (1), 9.0e-14 (2), 6.4e-13 (-0.5), 9.6e-14 (-1), 4.0e-15 (-2); max |ux - u| at most 5.7e-12. Without the key,
  plain SIMPLEC at u = 1 diverges from iteration 1 (smoke); reversed flows settle (ADR 0095); u = 0.5 and 2 were not run on the default
  code. Scratch-harness measurement, not a committed test.
* R2 F-200 C3 forward vent as registered (plain SIMPLEC from rest, mass-flow inlet) with the key and the C3 settled-field criterion:
  K = 2.5 settles after 310 iterations, outlet-column error 4.38e-12 (C3 limit 1.25e-6), imbalance 4.7e-13; **K = 10 is non-finite at
  iteration 2767**. Scratch test added to a scratch copy of `bc_catalogue.rs`, not committed. Run twice (the second run only to print the panic message cut off by my output filter; deterministic, same failure).
  F-200's entry is not touched.
* Observation (not registered, X7 duct, plain SIMPLEC with the key, K = 10, u = 1, 3000 iterations): the vent-column rows separate from the
  first iteration (largest difference within a column 8.8e-4, 2.6e-3, 1.3e-2, 6.7e-2 at iterations 1, 4, 7, 10) and the run does not settle
  (max |ux - u| between 0.9 and 230 over iterations 150-2850). This is a second, transverse mode of the vent faces, seen also with the
  face-velocity law in D2b. Scratch-harness measurement, not a committed test. Not diagnosed further; with pseudo-time 0.02 the F-200 C3 K = 10 case settles on the default code (existing test).
* R3 (amendment 2, FAN-01 stall, one run of `w41b_fan01_with_boundary_compensation`, 209 s): `res_u` / `res_p` 8.4e-2 / 4.5e-2
  (iteration 10), 7.1e-3 / 1.2e-3 (100), 4.4e-3 / 1.6e-3 (300), 5.5e-3 / 1.6e-3 (1000), 4.7e-3 / 1.0e-3 (1500); never both below 1e-4.
  c10 6.94e-4, L 2.50e-4, L / c10 0.360 (ADR 0084 without the key: 0.330; residuals 3.5e-3 / 7.9e-4 at 1500). **H-R3 is falsified**: the
  FAN-01 stall is not caused by the missing boundary compensation. The compensation changes the stalled level only slightly. Its cause stays
  undiagnosed (the ADR 0084 candidates remain: the 83-degree faces, the rotating-frame terms, laminar flow at Re 1.4e5).

### Step 3 (other open foundations defects), diagnosis only

* SIMPLEC seam compensation (ADR 0084 finding S): diagnosed and covered by the same opt-in key. B2 shows the converged seam flux now
  carries `rAU` (compensated SIMPLEC equals SIMPLE to 4.7e-13 on a periodic duct whose seam faces carry pressure differences; default
  SIMPLEC differs by 2.6e-3). Not made the default: it changes every converged SIMPLEC result on periodic meshes and pressure boundaries.
* Stalled residual: ADR 0022 suggested (not proven) that its uniform-duct residual floor is the reference normalisation measuring round-off
  on a (nearly) uniform exact solution; ADR 0023 added the backward and scaled norms (`solver.residual_norm`) aimed at this class. Not re-run
  here, so not confirmed for that duct. The E2 stall (res_u 4.4e-12 with the fields stationary) was not
  examined. The FAN-01 stall: H-R3 falsified (above).

### Deviations from the registered plan

* D1 missed its registered threshold at plain u = 1 (1.553e-4 against 1e-4). Amendment 1 was written after the diagnostic table, it added
  D1v and changed the choice rule; without it the registered rule gives "no fix". D1v stood as predicted (proportional to the viscosity).
* R2 was run twice (the second time only to read the panic message; the code is deterministic and the failure was the same).
* The observation of the K = 10 transverse mode on the X7 duct was not registered.
* The scratch diagnostic switches (D1s, D2a, D2b, D3 and the harness) were never committed; the committed solver code is the D1 switch
  without its environment hook.

### Unverified references

Van Doormaal and Raithby (1984) for the SIMPLEC velocity correction, cited from memory (UNVERIFIED); nothing here depends on it, the
derivations above are first principles.

### Status

F-248: A1 passes as a non-ignored test with the opt-in key; defaults unchanged (A3, final gate). Set to done with the note that the
acceptance is met with the opt-in key `solver.simplec_boundary_compensation = true` (the only change to the registered X7 set-up) and that
the default-code X7 at pseudo-time 0.02 remains a known miss.

### Final gate

On a fresh `git archive` of 6352a32 (main with the other wave-41 agents' commits up to it): `cargo fmt --all -- --check` clean,
`cargo clippy --workspace --all-targets -- -D warnings` clean, `cargo test --workspace --release --no-fail-fast`: **1958 passed, 0 failed,
554 ignored**. This wave adds 4 running tests (A1, B1, B2, A4) and 1 ignored report (R3). The only later commit changes the text of this
ADR, and no test reads ADR files.
