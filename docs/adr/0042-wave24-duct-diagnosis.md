# ADR 0042: Wave 24 diagnosis of the segregated-solver duct misbehaviour

Status: diagnosis and acceptance limits registered before any change of solver code; results in the addendum.

## Scope

Two symptoms that earlier waves met in constant-density SIMPLE/SIMPLEC duct flow:

1. **Symmetry side walls.** An empty duct (`channel_2d`, side walls `velocity = "symmetry"`, uniform inlet and initial stream)
   deviates from the exact plug flow by 0.159 in SIMPLEC (wave 21B A4, `a4_run`; wave 20B earlier).
2. **Pressure-pressure duct.** The registered X4 set-up of wave 23A (fan boundary and porous jump in a duct between two fixed
   pressures) diverges to NaN in SIMPLEC for every variant tried.

The recorded neighbouring misses (Z3 of ADR 0012, ADR 0022 "frictionless duct driven by two pressure ends", the odd-even
modes of ADR 0029, the nozzle residual of ADR 0023, the piston cell velocity of ADR 0033) are examined only for a shared cause.

## Diagnosis (method, measurements and reasoning)

All numbers below were measured with the exploratory tests of `crates/rcfd-api/tests/duct_diagnosis.rs` before this ADR was
committed; they are diagnostics, not acceptance results.

### Symptom 1: the averaged momentum diagonal (documented opt-in default, ADR 0010)

* Reproduced: `channel_2d` 60 x 10, symmetry walls, `u = (1,0,0)` everywhere, fixed inlet: SIMPLEC max `|ux - 1|` 0.159 (cell
  next to the side wall, outlet end), max `|uy|` 1.8e-2, pressure range 1e-4 .. 0.136 instead of zero. SIMPLE diverges to NaN
  within 50 iterations. The same duct with the side walls fixed at the stream velocity: 2e-12. The deviation appears in the
  first iteration, in the wall cells only.
* Cause: the symmetry plane gives the wall-normal component (`uy`) a Dirichlet diagonal term and the tangential component none,
  so the three component matrices have different diagonals in wall cells. The solver averages the diagonals into one `a_p`
  (`simple.rs`: `a_p[c] /= n_active`) and forms `u = H / a_p`; the H of component `k` is built from `a_k`, so the tangential
  velocity is multiplied by `a_k / a_p < 1` in every wall cell. This is the mechanism already documented in the field comment of
  `component_diag_correction` (ADR 0008 / 0010), which is off by default for baseline bit-identity.
* Bisect: with `solver.component_diagonal = "corrected"` the same duct holds the plug flow to round-off (SIMPLE and SIMPLEC,
  measured `|ux - 1|` 1e-12, `|uy|` 5e-13, `|p|` 2e-13). Symmetry-plane boundary coefficients (`bc.rs`: `a = 1 - n_k^2`, no
  tangential gradient), the Rhie-Chow face flux, the boundary pressure gradient, the SIMPLEC d-factor, the non-orthogonal
  correction and the mesh are therefore not the cause: with the diagonal corrected nothing else perturbs the plug flow.
  (`box_2d` was not needed: `channel_2d` already shows it.)
* Verdict: this is **not a new defect** but the known price of the default `component_diagonal = "averaged"`. The remedy exists
  (the option). Making it the default changes the results of every case with a symmetry (or slip, shear, directed) boundary.
  Of the registered baselines, `packed_bed` uses `symmetry` (ADR 0010 already records that it changes that hash).

### Symptom 2: the physics, then the numerics

* **Pure lossless pressure-pressure duct with `dp != 0` is ill-posed.** For constant-density flow in a constant-area duct
  without loss, integrating the steady momentum equation from inlet to outlet gives `p_in - p_out = 0` (the convective flux
  of momentum is equal at both ends, the wall shear is zero for symmetry or periodic sides). With `p_in != p_out` there is no
  steady state; the transient solution is `du/dt = (p_in - p_out) / (rho L)`, unbounded growth. Divergence of the iteration
  is then the correct behaviour (measured: `|u|` ~ 1e6 after one iteration for `dp` = 100). This applies to the "without fan and
  without jump" variants; it must not be asked to converge.
* **The registered X4 set-up is not of that kind.** Fan boundary (`200 .. 0 Pa`) plus the quadratic-Darcy porous jump has a steady
  operating point (`fan(Q) = jump(Q/A)`, ADR 0039); it is well-posed, and the divergence is numerical. A pressure-pressure
  duct with only the jump, `p_in = 0 = p_out` (steady solution `u = 0`, flow decays), also diverges in SIMPLEC at molecular viscosity.
* **Mechanism.** At a fixed-pressure inlet with zero-gradient velocity, the convection matrix gives the first cell the boundary
  term `phi_b * a_b` with `phi_b < 0` (inflow) and `a_b = 1`, which cancels the outflow term exactly (`div phi = 0`). The cell
  diagonal is then only viscous: measured at iteration 1, `a_p[0] = 4.2e-6` against `5.6e-3` in every other cell, `rAU[0] = 23.7`
  against `1.8e-2` (1300 times). The momentum predictor of that cell is driven by the pressure force alone and its velocity
  (1e5 .. 1e6) enters `HbyA` of its neighbours through the convective off-diagonal.
* Evidence of causation (all SIMPLEC, `box_2d` 100 x 4 periodic in y, jump zone as X4):
  - first-iteration velocity scales as `1/nu` (1.26e6, 1.26e5, 1.2e4, 1.1e3, 1.1e2 for `nu` 1.5e-5 .. 1.5e-1) and the largest
    values are at `x = 0.01`, the inlet cell;
  - with `p_in = 0 = p_out` and the jump, the iteration diverges for `nu = 1.5e-5` and converges to `u = 0` for `nu >= 1.5e-3`;
  - a Darcy sink restricted to the first cell column (`K <= 1e-6 m^2`), added to the jump set-up, makes SIMPLEC converge
    (`dp = 100`: `u` 2.04 and 2.22 for `K` = 1e-6 and 1e-7), while the same set-up without it diverges; a sink over the whole duct
    (the wave-23A "bed") converges. A sink in the jump cells only does not help (diverges), so the position matters: the inlet cell.
  - SIMPLE (not SIMPLEC) still diverges with the inlet-column sink, so SIMPLE has a second, separate problem (not examined).
* **Why Z2/Z3 of ADR 0012 converge although they have the same inlet.** Their duct has `ny = 2` with symmetry walls, so the
  inlet cell is a wall cell and the averaged diagonal (symptom 1) adds half of the wall-normal Dirichlet diagonal to its
  `a_p`. The two defects mask each other. Z3 with `component_diagonal = "corrected"` converges to a stationary state
  (one-step change 9e-9) where the registered default has a persistent two-cycle of 2.9e-2 localised in the inlet cell
  (`x = 0.01`); there the measured patch pressure agrees with `R(Q)` to 7e-7 (registered limit 1e-4). So Z3 shares the
  symptom-1 root cause, not the symptom-2 one.
* Not shared (not examined further): the odd-even modes of ADR 0029 (fixed-velocity inlet, relaxation-dependent Rhie-Chow),
  the variable-density nozzle residual (`varden.rs`), the piston cell velocity (dynamic mesh), X1 stationarity (a round-off
  pressure compared with a floor of 1e-12 is a criterion defect, ADR 0039 deviation 10).

## Registered acceptance limits (fixed before any post-fix run)

The fixes are candidates; each is committed only if every item holds, the baseline items included. Limits are not changed after a run.

A. Plug flow between symmetry walls (`channel_2d` 60 x 10, `lx` 2, `ly` 0.2, depth 0.05, `nu` 1e-3, uniform initial and inlet
   `(1,0,0)`, 300 iterations; SIMPLE and SIMPLEC):
   * A1 max `|ux - 1| <= 1e-10`, A2 max `|uy|, |uz| <= 1e-10`, A3 max `|p| <= 1e-10` (kinematic), A4 relative mass imbalance
     `|sum phi| / sum |phi_inlet| <= 1e-12`.
   * A5 (current default, averaged): the 0.159 deviation stays a known-miss test (ignored) until the default is changed.
B. Well-posed pressure-pressure duct (closed form): `box_2d` 100 x 4, `lx` 2, `ly` 1, periodic in y, `rho` 1.2, `mu` 1.8e-5,
   fixed pressure 100 (kinematic) at the inlet, 0 at the outlet, zero-gradient velocity, Darcy bed `K = 1e-6` over the whole
   duct, 3000 iterations, SIMPLEC: outlet flux / area equals `dp K rho / (mu L)` = 3.3333 m/s to **1e-2** relative; the flux is
   the same at inlet and outlet to **1e-12** relative. (A value of 3.346, relative 3.8e-3, had been seen in an exploratory run of
   the same set-up before the limit was written; the limit 1e-2 was set with that knowledge.)
C. X4 as registered (ADR 0039: fan boundary + jump, no bed): the registered limits of `x4_run` (5e-3 on `Q`, 2e-3 on the patch
   pressure) stay as they are; they are only claimed met if `x4_registered_lossless_setup_known_miss` passes unchanged.
D. Pressure-pressure duct with the jump only and `p_in = p_out = 0`, `u0 = 1` (decaying flow, SIMPLEC, `nu = 1.5e-5`): finite
   after 1500 iterations and `max|ux| <= 1e-6`.
E. Baselines: `baseline_hashes.rs` and every existing regression test pass unchanged; a fix that changes any registered hash
   is not committed (measurements and the list of changed hashes are reported instead).
F. Z3 (`z3_network_pressure_known_miss`) is not edited; whether it passes under a candidate is only reported.

Candidate fixes (symptom 2 only if A to E allow it):
* F1 (symptom 1): default `component_diagonal = "corrected"`. Expected to change `packed_bed` and the kind of case in Z2/Z3
  (they rely on the masking); therefore expected to violate E.
* F2 (symptom 2): treat the convective inflow through a boundary face that has an implicit link to the owner cell
  (`co.a != 0`, `flux < 0`) with the lagged owner value (`rhs -= flux * (a u_P_old + b)`, no diagonal term), so the cell keeps the
  outflow diagonal. The fixed point is unchanged; only cases with inflow through such faces change their iteration path.

## Deliberately out of scope

No new setting is introduced. If a candidate would need one to keep old behaviour it is reported, not added.

## Addendum: results, corrections, deviations (after the runs)

### What was changed

* Committed (F2): `convection.rs::lag_boundary_inflow`, called from the steady constant-density SIMPLE/SIMPLEC kernel
  (`simple.rs`) right after `convection(...)`. For every boundary face with inflow (`phi < 0`), a nonzero implicit link to the
  owner (`a_b != 0`) and no periodic coupling, the term `phi a_b` is removed from the owner diagonal and put on the right-hand
  side with the owner value of the previous iterate. The transient kernels (PISO/PIMPLE), the coupled solver and the
  variable-density kernel call `convection` as before and are unchanged. No setting was added.
* Not changed (F1, symptom 1): the default `component_diagonal = "averaged"`. Making `corrected` the default changes the
  registered baseline `packed_bed` from `(60, 19e53df373637262)` to `(16, fb46cdfee5d0d781)` (it converges after 16 iterations);
  sod, wedge, kepsilon_channel and all cavity and heated-cavity hashes stay identical. Per the process this is the user's
  decision; no hash was re-recorded and the default is as before.

### Correction of the diagnosis text above (Z3)

The diagnosis section attributed Z3 (ADR 0012) to the symptom-1 root cause. That was wrong or at least incomplete. With the F2
lag and the default (averaged) diagonal, the registered Z3 limits are met (Q rel 2.5e-6 against 5e-3; patch pressure rel 3.6e-12
against the registered 1e-4; the ignored `lumped.rs::z3_network_pressure_known_miss` also passes when run with `--ignored`, it was
not edited). With `corrected` and without F2 they are met too (2.4e-10). So Z3 is sensitive to the momentum predictor of the
fixed-pressure inlet cell, which both changes alter; its two-cycle was localised in that cell (x = 0.01). The mechanism of the
interplay of the two options in that cell was not worked out further. The pointer comment added to `lumped.rs` says "vanishes
with corrected", which is correct but not the whole story.

Also from the text above, "the fixed point is unchanged" for F2 is false in the strict sense: the converged discrete solution of
case B moved from `u = 3.346232` to `3.346468` m/s because the Rhie-Chow face coefficient uses `rAU`, which depends on the
diagonal. Converged results change wherever a face with a nonzero implicit link has inflow (pressure inlets, but also outlet
backflow faces). For pure Dirichlet faces, outflow, symmetry, walls and periodic faces nothing changes.

### Measured against the registered limits (release build)

| Item | Limit | Measured |
| --- | --- | --- |
| A1 max abs(ux - 1), corrected diagonal, SIMPLE / SIMPLEC | 1e-10 | 1.2e-12 / 2.5e-12 |
| A2 max abs(uy), abs(uz) | 1e-10 | 4.8e-13 / 1.2e-13, 0 |
| A3 max abs(p) | 1e-10 | 2.3e-13 / 9.6e-14 |
| A4 mass imbalance | 1e-12 | 2.8e-14 / 3.7e-15 |
| A (default averaged diagonal, SIMPLEC) | 1e-10 | 1.59e-1: MET ONLY WITH THE OPT-IN `corrected`; ignored `a5_...known_miss` |
| B closed-form flow, bed over the whole duct | 1e-2 | 3.94e-3 after F2 (3.87e-3 before); end flux mismatch 2.6e-14 (limit 1e-12) |
| C X4 registered Q / patch pressure (5e-3 / 2e-3) | | 3.1e-13 / 4.8e-13 after F2; the old test `x4_registered_lossless_setup_known_miss` still FAILS its stationarity helper (change 4.06e-8 against 1e-8 at 8000 iterations), so C is NOT claimed met through the old test; the new test `x4_registered_setup_reaches_the_operating_point_with_the_inflow_lag` applies the same two quantity limits and passes |
| D jump only, p_in = p_out = 0, 1500 iterations | max abs(ux) <= 1e-6 | 4.96e-6: MISS (before F2: NaN); ignored `d_jump_only_decay_known_miss`, assertion unchanged |
| E baselines | bit-identical | all 12 registered hash checks identical under F2 (run with a recording check in the scratch copy: the hash test itself panics at the first difference); `packed_bed` is the only one that changes under F1 |
| F Z3 | report only | met under F2 (see above) |

Full workspace test run with F2 (scratch copy of HEAD, `cargo test --workspace --release --no-fail-fast`): every non-ignored test
passed except the two diagnostic tests of this ADR that describe the pre-fix behaviour (their copies at that point; they are now
ignored records, see deviation 2).

Known-miss tests re-run with `--ignored` under F2 and not affected: all ADR 0022 `*_as_registered_known_miss` tests of
`bc_catalogue.rs` (C2, C3, D1, D2, E2: "iteration limit 6000 reached without meeting the residual targets", identical outcome with and
without F2; these are residual-floor misses of frictionless uniform flow, not the inlet-cell effect), the actuator-disk A4 and A4c
misses, X1 stationarity (a round-off pressure against a floor of 1e-12; criterion defect, ADR 0039 deviation 10). Z4 nominal-formula
miss unchanged. Not run and not claimed: the odd-even modes of ADR 0029 (coupled/segregated Rhie-Chow; not touched), the
variable-density nozzle residual (`varden.rs` has its own convection assembly and was not changed; the same inlet-cell effect might
exist there, unverified) and the piston cell velocity (dynamic mesh; `g5c` records a scalar `a_p` x Symmetry anisotropy, which is
symptom 1's mechanism, unverified for the 2.9e-4 cell velocity).

### Deviations

1. The limit of B (1e-2) was written after an exploratory run had shown 3.8e-3 (disclosed above in the registered text). The
   remaining 0.4 percent difference to the closed form of a uniform Darcy duct is not explained and was not chased.
2. The tests `b1_` and `b2_` (committed at 8989cdd as records of the pre-fix mechanism) fail once F2 is in, so they are ignored
   records with a reason; post-fix counterparts `b1f_` (first-iteration velocity bounded independently of viscosity: 30.0 and
   25.5 m/s for nu 1.5e-5 and 1.5e-3, ratio 1.2; before F2: 1.3e6 and 1.2e4) and `b2f_` (closed form `v = 2` m/s from
   `12 v^2 + 36 v - 120 = 0`; measured 2.000156, limit 1e-3 set after a first run showed 7.8e-5 and that a limit of 1e-6 on the
   change between iterations 600 and 1200 was too tight for the still decaying tail) were added.
3. The B0 assertions (the lossless duct with `dp != 0` keeps growing; with `dp = 0` it keeps uniform flow) were adjusted after
   their first run (growth factor 1.2 per doubling of the iteration count instead of 10; 1e-6 instead of 1e-10 for the round-off
   drift 1.2e-9). They are diagnostics, not registered limits.
4. The X4 test above rebuilds the registered case text inside `duct_diagnosis.rs` (the helpers of `heat_exchanger.rs` are not
   shared between test files); initial speed 2 m/s, 8000 iterations, ly = 1.0 as registered in ADR 0039.
5. After F2 the first-iteration velocity of the lossless duct with `dp = 100` is still 30 m/s (cell 3); the iteration recovers, but
   the start is not benign. SIMPLE (not SIMPLEC) with the old inlet-column sink still diverged in the pre-fix runs; SIMPLE after F2
   was not examined.

### Not resolved

* Symptom 1 stays with the default; the remedy is the existing opt-in, and its default flip is a user decision (packed_bed hash).
* The slow tail of the jump-only duct (D) and of X4's `ux` (4e-8) is not explained.
* Whether the variable-density and transient kernels have the same inlet-cell cancellation was not examined.
