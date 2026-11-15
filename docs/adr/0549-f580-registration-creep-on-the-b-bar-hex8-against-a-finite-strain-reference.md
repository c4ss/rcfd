# ADR 0549: F-580 registration: creep on the B-bar Hex8, the point-wise stationary field of the thick cylinder, and its growth bound measured against a finite-strain reference

Status: **REGISTERED 2026-11-15, written before any code of F-580 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-580 was written, compiled or run in the repository. The arithmetic below was done by
hand and evaluated with plain numpy prototypes in the scratch folder `w49i-work/f580-scratch` (`probe_ref1d.py`,
`probe_coarse2pt.py`, `probe_fbar1d.py`, `probe_fbar1d_loc.py`, `probe_at_points.py` and their `.log` files; no rcfd code;
disclosed, their numbers quoted below). The numbers of the rcfd Hex8 cylinder quoted here are those already recorded in
[ADR 0506](0506-f587-registration-b-bar-and-f-bar-hex8-in-the-nonlinear-driver.md) addendum 1 and its scratch log
`w49i-work/f587-scratch/run3-c4-diagnosis.log`. Results go in the addendum at the end; nothing above the addendum is changed
after a run.

Roadmap item F-580 (deps F-411, F-390), follow-up 1 of [ADR 0489](0489-f411-registration-power-law-creep-time-hardening.md).
Number 0549 given by the lead. Branch `f580-work` from e906faa8, worktree `w49i-work/f580`, scratch `w49i-work/f580-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test edited (new tests are appended to
`crates/rcfd-fem/tests/si_fbar.rs`, which carries the cylinder helpers; the scheme of ADR 0528); no new crate, no unsafe code;
clean-room.

## What F-580 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. The B-bar Hex8 takes the creep material on the Newton driver and solve_in_time. The plane-strain cylinder of ADR 0489 (b/a = 2, n = 4) reaches the stationary field point-wise within 2 percent of p after the registered redistribution time, and the point-wise error does not grow between 5 t* and 20 t* by more than the registered bound. The fully integrated elements keep their bits.

Clauses: (a0) every limit registered here; (a1) the B-bar Hex8 takes creep on the Newton driver and `solve_in_time`; (a2) the
cylinder point-wise within `2e-2` of `p` of the stationary field after the redistribution time; (a3) the point-wise error does
not grow between `5 t*` and `20 t*` by more than the registered bound; (a4) the fully integrated elements keep their bits.

The roadmap note (ADR 0506) records the state: (a1) and (a2) are met by ADR 0506 (C2 `9.54e-3` at `5 t*`, C3 `1.556e-2` at
`20 t*`); the bound `2e-3` on the growth of the point-wise error against the small-strain closed form (ADR 0506 C4) is missed at
`6.02e-3`, diagnosed as the total Lagrangian finite-strain drift; ADR 0528 (F-597) met it at `p/10` (alternative B) and left the
finite-strain reference (alternative A) undone. The note: "F-580 keeps its current status until the growth bound is measured on
a reference that allows for finite strain, or is re-registered at a smaller p/E." Alternative B is ADR 0528's result already.
**This ADR takes alternative A at the registered `p`**: a reference solution of the same cylinder that allows for finite strain.

## What was read (no run)

`AGENT_RULES.md`; the brief; ADR 0489 (arithmetic 6, D5, D6, the addendum, follow-up 1); ADR 0493 (the B-bar element, D1, the
cylinder mesh); ADR 0506 (readings, arithmetic 5, D1, D6, the checks C1 to C4 and CL, addendum 1 and the diagnosis of C4);
ADR 0528 (the choice of alternative B and its reasons against A, the checks, the mutations); `crates/rcfd-fem/src/mat_creep.rs`
(head, `return_map`), `nonlinear.rs` (`fbar_kinematics`, `evaluate_fbar`, `is_fbar`, the dead load of `solve_in_time` by
`nodal_load_vector`), `model.rs` (`Load::pressure`); the tests of `si_fbar.rs` (`cylinder`, `polar_points`, `creep_material`,
`lame`, `stationary`, `creep_error`, `creep_times`, `creep_run`, `c4_diagnosis_tenth_pressure`, the F-597 tests);
`docs/roadmap.json` (F-580, F-411, F-390, F-587, F-597).

## Readings, registered

- **(a1)** is the property that `solve_in_time` (which runs the Newton driver at a point context, ADR 0489 D5) completes the
  registered 70 steps to `20 t*` on a mesh whose one block is `SolidFormulation::BBar` with the creep material, and that every
  point crept (A1). It is ADR 0506's capability; F-580 adds no library code.
- **(a2)** The redistribution time is `5 t*` (ADR 0489's, kept); "point-wise" is every point of the stiffness rule (`gauss_2`),
  its second Piola-Kirchhoff stress (`hex8_second_piola_kirchhoff` of the BBar formulation, as ADR 0506) turned to `sigma_r`,
  `sigma_theta` by the angle of its reference position, compared with the closed form at its reference radius, relative to `p`
  (C2; C1 at `t = 0` against Lame, C3 at `20 t*`, as ADR 0506).
- **(a3)** "The point-wise error" is the error of the element, read against a reference that solves the same continuum
  problem without spatial discretisation error: the plane-strain cylinder at finite strain with the material law exactly as the
  library states it (the small-strain creep law on the Green-Lagrange strain and the second Piola-Kirchhoff stress, ADR 0489
  Limits), the dead bore pressure (the model's dead load, `nodal_load_vector`), and the same step times. It is the reference
  that "allows for finite strain" of the roadmap note. The error at a time is `e_F(t) = max_points max(|sigma_r - sigma_r^ref|,
  |sigma_theta - sigma_theta^ref|) / p`, the measure of ADR 0506's C2 with the reference in place of the closed form; the growth
  is `|e_F(20 t*) - e_F(5 t*)|` (F3), the measure of ADR 0506's C4. **The bound is ADR 0506's `2e-3`, kept** (F-580's
  "registered bound" was registered there; not re-derived from the probes below). The stronger point-wise difference
  `max_points |d(20 t*) - d(5 t*)|` (`d` the stress difference vector of a point) is reported without a limit (F4, arithmetic 5).
- **(a4)** No library file is changed (S0: `git diff --stat e906faa8 -- crates/*/src` empty at the end); with it the existing
  bit records `si_fbar_default_bits.rs`, `sa_creep_default_bits.rs`, `si_hex8x_bits.rs`, `si_hex8x_linear_bits.rs` and
  `baseline_hashes` pass (B).

## Arithmetic (before any code; probes disclosed)

1. **The stationary field** (derived; the classical sources cited from memory and NOT re-read, as in ADR 0489 arithmetic 6:
   F. K. G. Odqvist, Mathematical Theory of Creep and Creep Rupture, Clarendon Press, 1966 (2nd ed. 1974); H. Kraus, Creep
   Analysis, Wiley, 1980; J. T. Boyle and J. Spence, Stress Analysis for Creep, Butterworths, 1983). Small strain, stationary
   state: stresses constant, so the strain rate is the creep rate, deviatoric; plane strain `d eps_z = 0`, so `d eps_r = -d
   eps_theta`; the radial velocity `v` with `d eps_r = dv/dr`, `d eps_theta = v/r` gives `v = C/r`, `d eps_theta = C/r^2`. The
   flow rule `d eps_c = (3/2) A q^(n-1) s` with `s_z = 0` (so `sigma_z = (sigma_r + sigma_theta)/2`) gives `d eps_theta = (3/4)
   A q^(n-1) (sigma_theta - sigma_r)`, `q = (sqrt 3 / 2) |sigma_theta - sigma_r|`, hence `sigma_theta - sigma_r = B r^(-2/n)`.
   Equilibrium `d sigma_r/dr = (sigma_theta - sigma_r)/r` integrates to `sigma_r = -(n/2) B r^(-2/n) + D`; `sigma_r(a) = -p`,
   `sigma_r(b) = 0` give `sigma_r = -p ((b/r)^(2/n) - 1)/((b/a)^(2/n) - 1)` and `sigma_theta = p ((2 - n)/n (b/r)^(2/n) +
   1)/((b/a)^(2/n) - 1)`, the acceptance's field (independent of `E`, `nu`, `A`). These are the functions `stationary` and
   `lame` already in `si_fbar.rs`.
2. **The finite-strain reference (first principles).** The deformation of the plane-strain tube stays axisymmetric: `x = rho(R)
   e_R`, `F = diag(rho', rho/R, 1)` in the polar frame of the reference point, no rotation, so the Green-Lagrange strain is
   `E = diag((rho'^2 - 1)/2, ((rho/R)^2 - 1)/2, 0)` and the second Piola-Kirchhoff stress `S` is diagonal in the same frame
   (the library's polar recovery of `S` is this frame). First Piola-Kirchhoff `P = F S`: `P_rR = rho' S_RR`, `P_thTh = (rho/R)
   S_ThTh`. Equilibrium in the reference configuration, virtual work per radian and unit length: `int_a^b (P_rR du' + P_thTh
   du/R) R dR = p a du(a)` for the dead pressure `p` on `R = a` (nominal traction `p e_R`, fixed: the model's dead load) and a
   free outer surface. The material at a point, written in the test from the law (not by calling the library's
   `mat_creep`): trial `S_tr = lambda tr(E - eps_c) I + 2 G (E - eps_c)` over the three normal components (`E_zz = 0`), `q_tr`
   its von Mises stress, the backward Euler root `q_tr - q - 3 G A dt q^n = 0` by Newton from `q_tr` (`m = 0`, so the time
   factor is `dt`), `Delta = (q_tr - q)/(3 G)`, `eps_c += Delta (3/2) s_tr/q_tr`, `S = S_tr - 3 G Delta s_tr/q_tr` (the radial
   return of Simo and Hughes, Computational Inelasticity, Springer 1998, cited from memory, and of ADR 0489 D1; derivable in
   three lines from the flow rule). The reference uses the same backward Euler in time on the same 71 step ends (`t = 0` and
   `creep_times`) as the rcfd run, so time discretisation is common to both and the difference measures the element and the
   mesh. Space: `N` linear elements graded as the FE mesh (`R = a + (b - a)(e^(2s) - 1)/(e^2 - 1)`, `s` uniform), one point at
   the element midpoint (hoop strain from the mean nodal displacement; in one dimension there is no volumetric locking, and the
   midpoint stress of a linear element converges at `O(h^2)`), the stress at an FE point's reference radius by linear
   interpolation between midpoints. Newton per step on the nodal radial displacements with an element tangent by central
   differences (`h = 1e-7 p/E`) and the tridiagonal (Thomas) solve; no constraint is needed (the hoop stiffness fixes the
   radial mode). Converged when the largest correction is at most `1e-13` of the largest displacement. This is independent of
   the library: different discretisation (one dimension, exact kinematics, no F-bar), independent code for the law and the
   solve; what is shared is the continuum problem and the time steps. ADR 0528 rejected A because "a reference computed by
   the same code would not be independent"; this one is not computed by the same code.
3. **Probe of the reference** (`probe_ref1d.py`, numpy; log `probe_ref1d.log`). Small strain (`p = 200`, `A = 1/(E p^3)`, so
   `t* = 1 s` and every stress over `p` the same as at `2e5` in small strain), `N = 400`: against Lame at `t = 0` `7.3e-7`;
   against the stationary field at `20 t*` `8.4e-6` (at `5 t*` `1.46e-4`, at the outer surface: the transient's remnant). So
   the reference reproduces both closed forms in the small-strain limit, and the redistribution is complete to `1e-5` at `20
   t*`. At `p = 2e5`, `N = 400`: against Lame at `t = 0` `1.56e-4`; against the stationary field `2.33e-3` at `5 t*`, `4.42e-3`
   at `10`, `6.52e-3` at `15`, `8.64e-3` at `20 t*`, worst at the bore (`sigma_r`), largest equivalent creep strain `9.7e-3`
   (the library run of ADR 0506: `9.67e-3`); at the outer surface `9.1e-4` at `5 t*`, `2.8e-3` at `20 t*` (`sigma_theta`). The
   reference's own drift from the small-strain closed form between `5 t*` and `20 t*` is `6.3e-3` at the bore, the size of the
   library's C4 growth (`6.02e-3`): the drift ADR 0506 diagnosed is in the continuum problem, not in the element. `N = 200`
   against `N = 400`: the values above agree to about `1e-5` (the bore at radii `1.0008` and `1.0004`).
4. **A one-dimensional proxy of the FE** (`probe_coarse2pt.py`, `probe_fbar1d.py`; logs of the same names). To estimate the
   element's error against the reference before the run (the rcfd run itself is not probed): the same one-dimensional problem
   on the FE's radial mesh (48 graded linear elements), two Gauss points, (i) full integration, (ii) the variational F-bar of
   ADR 0506 D1 in one dimension (`J_bar` the volume-weighted element mean of `J = rho' rho/R`, `F_bar = (J_bar/J)^(1/3) F`, the
   law at `E_bar`, the force `sum w (dE_bar/du)^T S`). The proxy reproduces the library's recorded numbers: (i) full
   integration against the stationary field `0.102` at `5 t*`, `0.364` at `20 t*` (ADR 0506 CL `0.1025`, `0.364`); (ii) F-bar
   against Lame `4.5e-3` at `t = 0` (library C1 `5.37e-3`), against the stationary field `8.89e-3` at `5 t*` worst at `R =
   1.963` (library `9.54e-3` at `r = 1.962`), `1.65e-2` at `20 t*` worst at the bore (library `1.556e-2` at `r = 1.005`),
   growth `7.6e-3` (library `6.02e-3`), and at `p/10` growth `1.064e-4` (library `1.063e-4`). Against the finite-strain
   reference (ii) gives `e_F = 8.90e-3` at `5 t*`, `9.12e-3` at `10`, `9.45e-3` at `15`, `1.03e-2` at `20 t*`: **growth `1.37e-3`**
   (the estimate of F3, margin 1.45 to the bound `2e-3`; the library's numbers differ from the proxy's by up to 20 percent,
   so the margin is thin and a miss is possible; it would be recorded, not tuned).
5. **The point-wise difference** (F4, reported). In the proxy `max_points |d(20 t*) - d(5 t*)|` is `4.4e-3` at `R = 1.055`
   (`probe_fbar1d_loc.log`), equal in `sigma_r` and `sigma_theta` (`4.39e-3`, `4.37e-3`): a pressure that drifts inside the
   F-bar element (`9.1e-4`, `2.4e-3`, `4.4e-3` at `10, 15, 20 t*`, about linear in the creep strain), while the maximum error
   changes by `1.4e-3`. A plausible cause, not shown: the law's volumetric part is `tr(E - eps_c)` with a traceless `eps_c`,
   and `tr E = 0` is not `det F = 1` at second order, so the one constraint per element of the F-bar (on `J_bar`) does not
   match the material's constraint at finite strain. Not registered with a limit: the acceptance's "the point-wise error does not
   grow" is read, as by ADR 0506, on the error (the maximum over points), and a limit on this stronger measure would be set
   below its own estimate. Reported, with the proxy's number, as information for a follow-up.
6. **Reference resolution (R1).** `O(h^2)` at the midpoints; `N = 400` against `N = 800` at the FE points (interpolated): at
   most a few `1e-6` expected (arithmetic 3); limit `1e-4` (twenty times below F3's bound, so the reference's own error cannot
   decide F3).
7. **Cost.** The library run is ADR 0506's (`creep_run`, 70 steps, about 15 s in release for each of BBar and Full, ADR 0506
   log: two runs 35 s); the reference in Rust: 400 to 800 elements, 71 steps, a few Newton iterations, each with `3 N`
   return maps: well below a second. One test, below two minutes.

## Design

Test only, appended to `crates/rcfd-fem/tests/si_fbar.rs` under a heading `F-580 (ADR 0549)`; no library change.

- `fn ref_return(e: [f64; 3], c: &mut [f64; 3], a: f64, dt: f64) -> [f64; 3]`: the law of arithmetic 2 (`E = 2e9`, `nu = 0.3`,
  `n = 4`, at most 60 local iterations, stop at `|dq| <= 1e-15 q_tr`; a non-converged iteration panics).
- `struct Reference { rm: Vec<f64>, steps: Vec<Vec<[f64; 3]>>, eq_max: Vec<f64> }` and `fn reference(p: f64, nel: usize) ->
  Reference`: arithmetic 2 with `A = 1/(E p^3)` (the library's `creep_material(p)`), the steps `0` and `creep_times()`, the
  stress over `p` at the midpoints of every step end.
- `fn ref_at(r: &Reference, step: usize, radius: f64) -> [f64; 3]`: linear interpolation (clamped at the ends).
- `fn ref_error(pts: &[PolarPoint], r: &Reference, step: usize, p: f64) -> f64` (`e_F`) and the point-wise difference vectors.
- One test `f580_creep_bbar_against_the_finite_strain_reference`, run once: `creep_run(BBAR, CR_P)` and `creep_run(FULL,
  CR_P)` of ADR 0506 (unchanged helpers), `reference(CR_P, 400)`, `reference(CR_P, 800)`, `reference(CR_P / 1000, 400)`; all
  checks below printed and aggregated by `Checks`.

## Checks

Every measure is required finite before it is compared. `p = 2e5` unless stated; steps 0, 40, 70 are `t = 0`, `5 t*`, `20 t*`.

| Id | Clause | Measure | Limit | Estimate |
|---|---|---|---|---|
| A1 | a1 | BBar `solve_in_time`: 71 step ends, `steps[40].time = 5`, `steps[70].time = 20`, equivalent creep strain `> 0` at every point at `20 t*` | holds | ADR 0506 run (0 halvings) |
| C1 | a2 | BBar, `t = 0`: point-wise against Lame / `p` | `<= 1e-2` | `5.37e-3` (ADR 0506, the same bits) |
| C2 | a2 | BBar, `5 t*`: point-wise against the stationary field / `p` | `<= 2e-2` | `9.54e-3` (the same bits) |
| C3 | a2 | BBar, `20 t*`: the same | `<= 2e-2` | `1.556e-2` (the same bits) |
| F1 | a3 | BBar, `5 t*`: `e_F` against the finite-strain reference (`N = 400`) | `<= 2e-2` | `8.9e-3` (proxy) |
| F2 | a3 | BBar, `20 t*`: `e_F` | `<= 2e-2` | `1.03e-2` (proxy) |
| F3 | a3 | `|e_F(20 t*) - e_F(5 t*)|` | `<= 2e-3` (ADR 0506's bound, kept) | `1.37e-3` (proxy) |
| F4 | a3 (report) | `max_points |d(20 t*) - d(5 t*)|` | reported, no limit | `4.4e-3` (proxy) |
| R1 | a3 | reference `N = 400` against `N = 800` at every FE point, steps 0, 40, 70, over `p` | `<= 1e-4` | a few `1e-6` |
| R2 | a3 | reference at `p/1000` (`N = 400`) at the FE points: against Lame at `t = 0`; against the stationary field at `20 t*` | `<= 1e-5`; `<= 1e-4` | `7.3e-7`; `8.4e-6` |
| R3 | a3 (control) | the reference allows for finite strain: at `p`, its own deviation from the stationary field at the FE points at `20 t*`; its growth from `5 t*` | `>= 4e-3`; `>= 3e-3` | `8.5e-3`; `6.2e-3` |
| CL | a3 (control) | Full solve and Full recovery: `|e_F(20 t*) - e_F(5 t*)|` | `>= 0.1` | `0.26` (proxy (i); ADR 0506 CL) |
| S0 | a4 | `git diff --stat e906faa8 -- crates/*/src` | empty | no library change |
| B | a4 | the existing bit records (`si_fbar_default_bits`, `sa_creep_default_bits`, `si_hex8x_bits`, `si_hex8x_linear_bits`) and `baseline_hashes` | pass | unchanged library |
| S1 | | `cargo test -p rcfd-fem --release` (all), `cargo test -p rcfd-cli --release --test docs_site`, fmt, clippy `-D warnings` | 0 failed, clean | |

Reported without a limit: halvings, the largest creep increment, `e_F` at `10 t*` and `15 t*`, the worst point's radius at each
time, the largest equivalent creep strain of the run and of the reference.

Negative controls: R3 (a reference without the finite-strain terms would not show the drift; it shows that F3 is measured
against a reference that differs from the small-strain closed form by the drift that made ADR 0506's C4 miss); CL (a locking
element grows against the reference by far more than the bound); the closed-form growth `|C3 - C2|` is reported (expected
`6.02e-3`, ADR 0506's known miss, which stays as its ignored record).

## Planned mutations (scratch copy `f580-scratch/mut` with its own target dir; each run once against the new test)

| Id | Mutation | Expected catcher |
|---|---|---|
| MU1 | `fbar_kinematics` (`nonlinear.rs`): `alpha^2 - 1` set to `0` (the BBar arm becomes the fully integrated total Lagrangian element) | C2, F1, F3 (locking, `~0.1` to `0.26`) |
| MU2 | `mat_creep::return_map`: the residual `q_tr - q - c q^n` with `q^(n - 0.5)` (and its derivative accordingly) | C2, F1, F2 (the creep rate falls by a factor `~q^0.5`, `~450`: at `5 t*` the field is still near Lame, off the stationary field by `~0.2`) |
| MU3 | `fbar_kinematics`: the Green-Lagrange strain `green_lagrange(h)` replaced by its linear part `sym(h)` (small-strain measure in the BBar arm, the force and tangent unchanged) | F3 or a driver error (the element then lacks the finite-strain drift that the reference has, so `e_F` grows by about the reference's `6e-3`; an inconsistent tangent may stop the Newton loop, a failed solve counts as caught) |
| MU4 | `mat_creep::time_factor` for `m = 0`: `dt` replaced by `dt/2` | F3 (half the creep strain, half the drift: `e_F` at the bore changes by `~3e-3` between `5 t*` and `20 t*`), or C2 (the field at `5 t*` is the field at `2.5 t*`) |

## Decision rule

- **done**: A1, C1 to C3, F1 to F3, R1 to R3, CL, S0, B, S1 met in the single registered run, and every planned mutation caught
  by a registered check (or a failed solve).
- **partial**: any registered limit missed (an ignored `*_known_miss` test with the limit intact and the number), any planned
  mutation not caught, or any check met only after an amendment written after the run.
- **todo**: A1 or C2 missed.

## Limits (what is not covered)

The reference shares the continuum problem with the library: the small-strain creep law applied to `E` and `S` (ADR 0489
Limits). It does not say whether that law is the right finite-strain creep model (a multiplicative split or a logarithmic strain
would change the drift itself); it measures the element against the problem the library states. The time steps are common, so
time discretisation error is not measured (ADR 0489 measured the order). Only `m = 0`, `n = 4`, `b/a = 2`, `nu = 0.3`, the one
registered mesh. No case-file key. The incompatible-modes Hex8 stays refused in the nonlinear driver (ADR 0506).

## Amendment 1 (2026-11-15, written after run 1 and before any corrected run)

**Run 1** (`f580_creep_bbar_against_the_finite_strain_reference`, single run, 26 s in release; log
`f580-scratch/run1-f580.log`): both rcfd solves completed (BBar and Full, 0 halvings, largest creep increment `2.38e-4`), then
the reference panicked "reference: Newton not converged at step 0" before any check was measured. **A defective set-up, not a
result:** the registered stop of the reference's Newton loop (`max |du| <= 1e-13 max |u|`, arithmetic 2) lies at or below its
rounding floor. The numpy probe of arithmetic 3 did not assert convergence and so did not show it.

Diagnosis (one diagnostic run, `f580_reference_newton_diagnosis`, ignored, the stop set to 0 so every iteration of step 0 is
printed; log `f580-scratch/diag-a1.log`): at `p` the ratio falls `1, 1.2e-4, 9e-11, 5.6e-14` and then stays between `5e-15`
and `2e-14`; at `p/1000` it falls `1, 1.1e-4, 4e-8, 8e-11` and then stays between `6e-12` and `2e-11` (the Green-Lagrange
strain `(rho'^2 - 1)/2` of a strain of `1e-7` is formed by a subtraction and carries a relative rounding of `1e-9`; the
element tangent by central differences of step `1e-7 p/E` adds its own). The floor at `p` is above `1e-13` in about one
iteration in two, at `p/1000` always.

**Correction:** the stop becomes `max |du| <= 1e-9 max |u|` (`reference_with(p, nel, 1e-9, false)`), 50 times above the
largest floor seen and reached in three to four iterations. Its effect on the checks: a displacement error of at most `1e-9`
of `u(a)` (`1.9e-4` at `p`) is a strain error of at most `1e-9 u(a)/h_min = 1.5e-10` against strains of `1e-4`, so at most
`2e-6` of `p` in the stress, twenty times below R1's limit and five hundred times below F3's bound (the actual error is much
smaller: the last Newton correction is far below the stop). Nothing else changes: the same reference, the same rcfd solves,
the same checks, limits and mutations. The corrected run is the test `f580_amendment_1_creep_bbar_against_the_finite_strain_reference`;
run 1 stays as the ignored record `f580_creep_bbar_against_the_finite_strain_reference` (its body moved into the helper
`f580_checks`, which it calls with the original `reference`, the stop `1e-13`; the behaviour is that of run 1). The rcfd numbers
of run 1 (halvings, largest increment) were printed before the panic; no check value was seen.

## Addendum 1: results (2026-11-15)

Code: commit 768304e5 (tests only: `crates/rcfd-fem/tests/si_fbar.rs`, the F-580 section appended: `ref_return`,
`Reference`, `reference`, `reference_with`, `ref_at`, `ref_differences`, `ref_error`, `ref_against`, `f580_checks`, the ignored
run-1 record `f580_creep_bbar_against_the_finite_strain_reference`, the corrected test
`f580_amendment_1_creep_bbar_against_the_finite_strain_reference` and the ignored diagnostic `f580_reference_newton_diagnosis`).
No library file changed. Logs in `w49i-work/f580-scratch`: `run1-f580.log` (run 1, the set-up defect), `diag-a1.log`
(diagnostic of amendment 1), `run2-f580-amend1.log` (the corrected run, single, 28 s in release), `s1-rcfd-fem.log`,
`s2-baseline.log`, `s2-docs.log`, `s3-fmt.log`, `s3-clippy.log`, `mutations.log`, `mut-MU1.log` to `mut-MU4.log`.

| Id | Measured | Limit | Result |
|---|---|---|---|
| A1 | 71 step ends, `t = 5` at step 40, `t = 20` at step 70, smallest equivalent creep strain at `20 t*` `2.37e-3` (largest `9.67e-3`); 0 halvings, largest creep increment `2.38e-4` | holds | met |
| C1 | `5.373e-3` | `<= 1e-2` | met |
| C2 | `9.540e-3` | `<= 2e-2` | met |
| C3 | `1.556e-2` | `<= 2e-2` | met |
| F1 | `9.549e-3` (worst at `r = 1.962`) | `<= 2e-2` | met |
| F2 | `1.106e-2` (worst at `r = 1.350`) | `<= 2e-2` | met |
| F3 | `1.506e-3` | `<= 2e-3` | met |
| F4 | `4.396e-3` at `r = 1.0545` | reported | (proxy `4.4e-3` at `1.055`) |
| R1 | `2.20e-6` | `<= 1e-4` | met |
| R2 | `2.92e-6` (Lame), `8.54e-6` (stationary, `20 t*`) | `<= 1e-5`, `<= 1e-4` | met |
| R3 | `8.62e-3` at `20 t*` (`2.32e-3` at `5 t*`), growth `6.30e-3` | `>= 4e-3`, `>= 3e-3` | met |
| CL | Full `e_F` `0.1034` (`5 t*`), `0.3638` (`20 t*`), growth `0.2605` | `>= 0.1` | met |
| S0 | `git diff --stat e906faa8 -- crates/*/src` empty | empty | met |
| B | `si_fbar_default_bits`, `sa_creep_default_bits`, `si_hex8x_bits`, `si_hex8x_linear_bits` pass (in S1); `baseline_hashes` 3 passed, 1 ignored (as on the base) | pass | met |
| S1 | `cargo test -p rcfd-fem --release`: 242 test binaries, 1026 passed, 0 failed, 283 ignored; `cargo test -p rcfd-cli --release --test docs_site` 2 passed; `cargo fmt -p rcfd-fem --check` clean; `cargo clippy -p rcfd-fem --release --all-targets -- -D warnings` clean | 0 failed, clean | met |

Reported: `e_F` `9.77e-3` at `10 t*` (`r = 1.962`), `1.011e-2` at `15 t*` (`r = 1.916`); the closed-form growth `|C3 - C2|`
`6.02e-3` (ADR 0506's known miss, the same bits); the reference's largest equivalent creep strain `9.715e-3` at `p`,
`9.70e-6` at `p/1000`. The numbers of the library run repeat ADR 0506's bits (C1 to C3), as stated in the registration.

### Mutations (scratch copy `f580-scratch/mut` of 768304e5, target `f580-scratch/mut-target`, each run once, the file restored after)

| Id | Result of the test | Caught by |
|---|---|---|
| MU1 (`alpha^2 - 1 = 0`) | C1 `1.26e-2`, C2 `0.102`, C3 `0.364`, F1 `0.103`, F2 `0.364`, F3 `0.261` missed | **caught** (C1, C2, C3, F1 to F3) |
| MU2 (`q^(n - 0.5)` in the local residual) | 232 halvings, largest increment `1.1e-7`, 1915 s; C2 `0.842`, C3 `0.616`, F1 `0.844`, F2 `0.623`, F3 `0.222` missed | **caught** (C2, C3, F1 to F3) |
| MU3 (linear strain in the BBar arm) | passed: C2 `9.50e-3`, C3 `1.512e-2`, F1 `9.51e-3`, F2 `1.001e-2`, **F3 `5.09e-4`**, F4 `4.31e-3` at `r = 1.005`; closed-form growth `5.62e-3` | **not caught** |
| MU4 (time factor `dt/2`) | passed: largest increment `1.19e-4`; C2 `9.45e-3`, C3 `9.75e-3`, F1 `9.47e-3`, F2 `9.92e-3`, **F3 `4.51e-4`**, F4 `4.31e-3` at `r = 1.001`; closed-form growth `2.93e-4` | **not caught** |

Diagnosis of MU3 and MU4 (no new run). F3 is the growth of a maximum over the points, and at `5 t*` that maximum is the
stationary `O(h)` error of the linear element at the outer radius (`9.5e-3` at `r = 1.962`, the same in every run). A
mutation that changes the field near the bore by a few `1e-3` moves the maximum only once the bore error overtakes the outer
one, which in the unmutated run happens only after `15 t*` (F2's worst point `r = 1.35`). Under MU4 the bore drift is halved
and never overtakes; under MU3 the stress is nearly insensitive to the quadratic part of `E` (the creep strain absorbs the
deviatoric difference, and the F-bar the volumetric one), so the drift that remains is that of the equilibrium on the
reference configuration (`P = F S`), which MU3 keeps. Both mutated runs therefore pass F3 with a smaller growth than the
unmutated one. MU3's premise in the registration ("the element then lacks the finite-strain drift") was wrong: the closed-form
growth stays `5.6e-3` under MU3, so the drift is in the equilibrium, not in the strain measure. A witness that sees MU4 has to
look at the bore (for instance the error of the points with `r < 1.1` against the reference, whose closed-form drift under MU4
is half of the reference's) or use a mesh on which the bore error dominates; this is a follow-up, not tuned here.

### Deviations

- **Amendment 1** (the reference's Newton stop `1e-13` below its rounding floor): written and committed (1e0ce522) after run 1
  and before the corrected run; no check was measured in run 1. The corrected run met every limit in its single run. Run 1
  stays as the ignored record.
- MU3's expected catcher was wrong (diagnosis above); MU2 ran 32 minutes (the mutated residual and the unmutated tangent make the
  driver halve 232 times); both were run once, as planned.
- Nothing registered was skipped.

### Decision (by the rule)

Every registered check is met (A1, C1 to C3, F1 to F3, R1 to R3, CL, S0, B, S1), F3 against the finite-strain reference at
`1.51e-3` within the bound `2e-3` that ADR 0506 missed against the small-strain closed form (`6.02e-3`). But two planned
mutations (MU3, MU4) are not caught by any registered check, and the checks were met in the corrected run after amendment 1.
**Proposed status: partial.**

### Open questions

1. **The witness of the growth bound.** F3 on the maximum over points is blind to bore-side changes of a few `1e-3` (MU3,
   MU4). Options: (a) a bore-zone growth (points with `r < 1.1`) against the reference with its own registered limit;
   (b) a refined radial mesh (e.g. 96 x 24) on which the bore error dominates; (c) the point-wise growth F4 with a limit.
   Recommendation: (a), with MU4 as the planned catch target, since it keeps the registered mesh and measures where the
   finite-strain drift lives; (c) is not recommended until F4's own drift is understood (question 2).
2. **The pressure drift in the F-bar element** (F4: `4.4e-3` at `r = 1.05`, equal in `sigma_r` and `sigma_theta`, present also
   under MU3 and MU4 at the bore). Options: (a) diagnose by the one-dimensional proxy (F-bar against the reference with a
   `J`-based volumetric part of the law); (b) leave it as a property of the small-strain law on `E`. Recommendation: (a), as a
   diagnostic follow-up without a change of the default element.

## For the lead: roadmap entries

> F-580 (partial, ADR 0549, 2026-11-12/15): creep on the B-bar Hex8 through the Newton driver and solve_in_time (the capability of ADR 0506, no library change), the plane-strain cylinder b/a = 2, n = 4, at the registered p (p/E = 1e-4, largest creep strain 9.7e-3), measured against a finite-strain reference: a one-dimensional axisymmetric total Lagrangian solve written in the test (the same small-strain law on E and S, dead bore pressure, the same steps; it reproduces Lame within 2.9e-6 and the stationary field within 8.5e-6 at p/1000, converges to 2.2e-6 between 400 and 800 elements, and at p departs from the small-strain stationary field by 8.6e-3 at 20 t*, growth 6.3e-3, the drift that made ADR 0506's C4 miss). Met: point-wise against the stationary field 9.5e-3 at 5 t* and 1.6e-2 at 20 t* (limit 2e-2), against the reference 9.5e-3 and 1.1e-2, growth of the point-wise error against the reference 1.5e-3 within the bound 2e-3 of ADR 0506; Full integration 0.26 (locking control); no source file changed, the bit records and baseline_hashes pass. Partial by the rule: the checks were met after amendment 1 (the reference's Newton stop was below its rounding floor; no check had been measured), and two of four mutations were not caught (the linear strain in the BBar arm, the creep rate halved): the growth of a maximum over points is dominated by the outer-radius O(h) error and does not see bore-side changes of a few 1e-3. The point-wise difference grows by 4.4e-3 at r = 1.05 as a pressure inside the F-bar element (reported). C4 at p of ADR 0506 stays its known miss against the small-strain closed form.

> Follow-up proposed: a bore-zone witness for the finite-strain growth of the B-bar creep cylinder. Deps F-580. Acceptance: Every limit not stated here is registered in the ADR before the run. On the cylinder of ADR 0549 at the registered p, the error of the points with r < 1.1 a against the finite-strain reference of ADR 0549, its growth between 5 t* and 20 t* within a registered bound; the mutations of ADR 0549 that were not caught (the linear strain in the BBar arm, the creep rate halved) caught by a registered check; the checks of ADR 0549 unchanged.

> Follow-up proposed: the pressure drift of the F-bar Hex8 under creep at finite strain. Deps F-580, F-587. Acceptance: Every limit not stated here is registered in the ADR before the run. The growth of 4.4e-3 of p of the point-wise stress difference against the finite-strain reference at r = 1.05 (ADR 0549 F4, equal in sigma_r and sigma_theta) diagnosed: its scaling with p/E, its presence in a one-dimensional F-bar proxy, and whether a J-based volumetric part of the law removes it, each with a registered limit; the default element and every existing result bit-identical.
