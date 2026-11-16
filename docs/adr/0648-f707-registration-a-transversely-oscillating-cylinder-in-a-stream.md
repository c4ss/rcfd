# ADR 0648: F-707 registration: moving immersed shapes, a transversely oscillating cylinder in a stream

Status: **REGISTERED 2026-11-16, written before any code of F-707 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-707 was written, compiled or run: the documents and code under "What
was read" were read and the arithmetic under "Arithmetic before this registration" was done by hand. No timing probe was run;
run lengths are set from the timings of the F-308 record (ADR 0631 addendum 1 and its log: the immersed step 0.38 s on 56 644
cells, the body-fitted step 0.28 s on 32 768 cells). Results go in the addenda at the end; nothing above the addenda is changed
after a run.

Parents: [ADR 0631](0631-f308-registration-moving-immersed-shapes.md) (F-308: `immersed_motion.rs`, `MovingImmersed`, the force
D3, the body-fitted reference D2, the in-line KC 5 case; its amendment 2 lesson: the body-fitted mesh starts at the initial pose;
its question Q4, from which F-707 was made), [ADR 0218](0218-f219-registration-immersed-solid-with-modified-forcing.md) (the
forcing), [ADR 0249](0249-f346-registration-immersed-force-in-unsteady-flow.md) (`force_moment_unsteady`, `velocity_rate`, the
force in a stream), ADR 0252 (`patch_force_moment_wall_gradient`), ADR 0033 and ADR 0101 (the swept-volume moving-mesh step),
[ADR 0451](0451-f530-registration-piso-stability-at-large-diffusion-numbers.md) (the non-iterated PISO step diverges above a cell
diffusion number of about 2.5; the meshes below keep it under 1.4), ADR 0421 and ADR 0425 (the immersed start transient: it
concerns the first steps only and lies far outside the window used here). Roadmap item F-707 (deps F-308). Number 0648 given by
the lead. Branch `f707-work` from local main d40cd665, worktree `w49i-work/f707`, scratch `w49i-work/f707-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; existing tests not
edited; defaults bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new
dependency, no unsafe code, crate graph unchanged; clean-room. The roadmap files are not edited. Every choice below that the
maintainer would normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the
recommendation), except the open questions at the end.

## What F-707 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. A cylinder oscillating across a uniform stream at a low Reynolds number gives the mean drag, the lift amplitude and the phase of the lift against a body-fitted moving-mesh run of this repository within registered limits; a negative control with the body velocity left out of the forcing misses them.

Clauses: **c0** every limit registered here; **c1** a cylinder oscillating across a uniform stream at a low Reynolds number (a
moving immersed shape); **c2** its mean drag against a body-fitted moving-mesh run of this repository within a registered limit;
**c3** its lift amplitude, likewise; **c4** the phase of its lift, likewise; **c5** a negative control with the body velocity left
out of the forcing misses them (each of the three). The brief adds: reuse F-308's machinery, the default immersed path
bit-identical, the body-fitted mesh started at the initial pose, heavy runs as ignored records run once, negative controls and
mutations registered before the run.

### Readings, registered

- **The reference** is the body-fitted moving-mesh run of this repository (the swept-volume ALE PISO step,
  `PimpleSolver::step_ale_with_walls`, with the follow condition of the cylinder wall), as in ADR 0631 D2. No published value is
  used; its own accuracy is shown on two meshes (R1).
- **"Low Reynolds number": Re = U D / nu = 20.** Taken on the maintainer's standing instruction of 2026-11-11 (take the
  recommendation), alternatives in Q2. Reason: below the onset of vortex shedding of the fixed cylinder (about Re 47; cited from
  memory as the generally known threshold, not used as a number) the wake has no self-excited oscillation, so the flow forced by
  the motion has one periodic response at the forcing frequency. At Re 100 the response depends on lock-in, which is sensitive to
  the small difference of the natural shedding frequency between two discretisations near the phase jump of the lift (the reason
  the lift phase could differ by tens of degrees between two otherwise accurate runs); that comparison would test the
  discretisations' Strouhal numbers more than the moving forcing. A lock-in case is proposed as a follow-up.
- **"Drag" and "lift"**: the force components along the stream (x) and across it (y, the direction of the motion), per unit depth.
  "Mean drag" is the mean of `F_x` over the window; "lift amplitude" the modulus of the first harmonic of `F_y` at the forcing
  frequency; "phase of the lift" the argument of that harmonic relative to the first harmonic of the displacement,
  `phi_L = arg(Lhat / Yhat)` (positive when the lift leads the displacement).
- **"Misses them"**: the negative control's mean drag, lift amplitude and lift phase each lie outside the limit that the immersed
  run must meet (checks N1a, N1b, N1c), and its complex lift harmonic differs from the reference by more than half (N1d, which
  does not depend on a phase that becomes arbitrary when the lift vanishes).
- **The domain.** Both runs need the same closed problem. The O-grid generator gives a circle; the Cartesian mesh a box. The
  body-fitted O-grid is mapped so that its outer boundary is the square `[-8, 8]^2` of the Cartesian mesh (D2), and both runs use
  the same far-field condition on that square (the existing `Freestream` condition: velocity fixed to the stream on faces with
  `n . U < 0`, zero gradient elsewhere; pressure zero gradient on those faces and fixed to 0 elsewhere). On the square the faces of
  `x = -8` are the only inflow faces in both meshes; the faces of `y = +-8` have `n_x = 0` exactly (D2 makes the points exact),
  so the switch of the condition is the same in both meshes and is not decided by round-off.

## What was read

`w49i-work/AGENT_RULES.md`; the brief; ADR 0631 (all, with amendments 1 and 2 and the addendum); `crates/rcfd-fv/src/immersed_motion.rs`
(all); `crates/rcfd-fv/tests/immersed_moving_f308.rs` (all: the set-ups `run_ib`, `run_bf_corrected`, `stretched_axis`,
`harmonic`, `blend`); `crates/rcfd-fv/src/bc.rs` (`Freestream`, `Switched`, `Directed`); `crates/rcfd-fv/tests/cylinder.rs` and
`crates/rcfd-api/tests/validation_cylinder_re100.rs` (the far-field set-up of a cylinder in a stream with `Freestream`, linear
upwind convection); `crates/rcfd-mesh/src/gen.rs` (`cylinder_ogrid`: points `r (cos, sin)` at `theta = 2 pi i / n_theta`, the
last radius `r_out`; `box_2d_coords`: patches `xmin`, `xmax`, `ymin`, `ymax`, `frontAndBack`; `warp`); `crates/rcfd-mesh/src/mesh.rs`
(`polygon_geometry`: the face centre and area vector from the vertex average and a triangle fan); `crates/rcfd-fv/src/pimple/moving.rs`
(`step_ale_with_walls`: refuses an immersed forcing and non-Euler schemes; no refusal of the convection scheme);
`crates/rcfd-fv/src/sixdof.rs` (`patch_force_moment_wall_gradient`); `crates/rcfd-fv/src/ale.rs` (`move_mesh`,
`set_follow_conditions`); `crates/rcfd-mesh/src/motion.rs` (`moved_mesh`); `crates/rcfd-fv/tests/immersed_solid_f219.rs` (linear
upwind with the immersed forcing in the DFG channel); the headings and conclusions of ADR 0421, 0425, 0451; the F-308 record log
(`w49i-work/f308-scratch/heavy_record_corrected.log`, the run times); the "For the lead" sections of ADR 0572 and 0573.

## Arithmetic before this registration (disclosed, by hand)

1. **Case numbers.** D = 1, r = 0.5, U = 1 (the stream, along x), rho = 1, nu = 0.05 (Re 20). Motion across the stream:
   `y_c(t) = -A cos(omega t)` with `A = 0.5 D`, frequency `f D / U = 0.25`, `T = 4`, `omega = pi/2 = 1.5707963`
   (`HarmonicTranslation { amplitude: [0, 0.5, 0], omega: pi/2, phase: -pi/2 }`: `0.5 sin(omega t - pi/2) = -0.5 cos(omega t)`).
   From rest at the lower turning point: largest body velocity `A omega = 0.785 U`, largest acceleration `A omega^2 = 1.234`.
   Reduced velocity `U / (f D) = 4`. Largest angle of the relative flow `atan(0.785) = 38 degrees`.
2. **Rough sizes of the force (quasi-steady and potential-flow estimates, for orientation only; no limit uses them).** Drag
   coefficient at Re 20 about 2 (from memory) and somewhat more in the box: mean drag about 1.1 to 1.3. Lift: the part along the
   relative velocity, `-0.5 C_D |V| ydot`, amplitude about `0.5 x 2.2 x 1.15 x 0.785 = 1.0`, in phase with `-ydot`; the added
   mass `-C_a rho pi r^2 yddot` with `C_a` about 1, amplitude `0.785 x 1.234 = 0.97`, in phase with the displacement. Lift
   amplitude about 1.4, phase between 0 and -90 degrees. Both parts come from the body velocity in the forcing.
3. **Mean drag of the negative control.** With the body velocity left out, the solid holds zero velocity and changes place: the
   fluid sees a cylinder at rest whose position jumps by a fraction of a cell per step (at most `0.785 x 0.008 / 0.025 = 0.25`
   cells). Its mean drag is that of a fixed cylinder, about. The oscillating cylinder's mean drag is larger: quasi-steadily
   `F_x = 0.5 C_D(|V|) |V| U` with `C_D` proportional to `Re^-n`, `n` about 0.45 between Re 10 and 40 (from memory: C_D about 2.9,
   2.05, 1.5), so `F_x` grows like `|V|^(1-n) = 1 + (1-n) ydot^2 / 2`, over a period `+ 0.27 x (A omega)^2 / 2 = +8.5 %`. Estimate
   of the N1 mean-drag difference: 5 to 12 per cent, above the A1 limit 3 per cent with a margin of about 2 to 4. N1 lift: that of
   a cylinder at rest off the centre line of a box of half height 16 D, a few hundredths at most, against about 1.4: the amplitude
   misses by more than 0.9 of the reference.
4. **Immersed mesh G40.** Uniform core of `h = 0.025 = D/40` on `x in [-1.25, 2.5]`, `y in [-1.5, 1.5]` (150 x 120 cells; the
   cylinder covers `|y| <= 1.0`, `|x| <= 0.5`; the recirculation at Re 20 ends about one diameter behind the cylinder), outside it
   cells growing by 1.08 to the edges `+-8`, scaled so the last face lands on the edge (the rule of ADR 0631 arithmetic 3 per
   side): sides of lengths 6.75 (left), 5.5 (right), 6.5 (top, bottom) take 40, 38, 40, 40 cells
   (`0.025 x 1.08 (1.08^n - 1)/0.08 >= L`: `1.08^n >= 21.0, 17.3, 20.3`). 228 x 200 = 45 600 cells. G20 (C1): the same with
   `h = 0.05`: 32 + 75 + 29 by 31 + 60 + 31, 136 x 122 = 16 592 cells.
5. **Body-fitted mesh BF.** `cylinder_ogrid(256, 96, 0.5, 8.0, Grading::Expansion(12), 1.0)`: radial ratio
   `q = 12^(1/95) = 1.026502`, first cell `s0 = 7.5 (q - 1)/(q^96 - 1) = 7.5 x 0.026502 / 11.318 = 0.01756`, last cell 0.21;
   about 11 cells across the boundary layer of thickness `D / sqrt(Re) = 0.22`; circumferential spacing 0.0123 at the wall. 24 576
   cells. BFc (R1): `cylinder_ogrid(128, 48, 0.5, 8.0, Expansion(12), 1.0)`, `q = 12^(1/47) = 1.0543`, `s0 = 0.0350`, 6 144 cells.
   Both `n_theta` are multiples of 8, so the four corners `theta = pi/4 + k pi/2` are grid points.
6. **The square map of the O-grid (D2).** A point at radius `rho`, direction `(c, s) = (x, y)/rho`, `m = max(|c|, |s|)`, is moved
   radially by the factor `1 + b(rho)(1/m - 1)`, `b = 0` for `rho <= 2`, `b = ((rho - 2)/6)^2` for `2 < rho <= 8`. At `rho = 8`
   the point lies on the square of half side 8. The factor's radial derivative stays positive (`1 + b(1/m - 1) + rho b' (1/m - 1)
   > 0`), so no cell folds; the largest stretch is `sqrt 2` at the corners. The points of the outer ring are set exactly: `x =
   8 sign(c)`, `y = 8 s / |c|` where `|c| >= |s|`, the other coordinate exactly `+-8` at the corners, likewise for `|s| > |c|`.
   With all four points of a face on `x = -8` exactly, the vertex average has `x = -8` exactly and every triangle of the fan has
   `dx = 0`, so the area vector is `(n_x, 0, 0)` exactly; on `y = +-8` the area vector has `n_x = 0` exactly; `Freestream` then
   classifies the faces as in the box.
7. **Mesh motion of BF.** The displacement `y = y0 + y_c(t) w(rho0)` of the mapped points, `w` the blend of ADR 0631 arithmetic
   5 (`1` for `rho0 <= 1.5`, cosine to `0` at `rho0 = 6`): the cylinder and the layers within one diameter translate rigidly; the
   square (at `rho0 >= 8`) does not move. Largest strain `A max|w'| = 0.5 pi / 9 = 0.17`. The solver is built on the mesh at the
   initial pose `y0 + y_c(0) w` (ADR 0631 amendment 2), every step moves the points to `y0 + y_c(t_{n+1}) w` from the mapped
   points.
8. **Time step.** `dt = T / 500 = 0.008`. Cell diffusion numbers: G40 `0.05 x 0.008 / 0.025^2 = 0.64`; BF at the wall
   `0.0004 / 0.01756^2 = 1.30`; BFc 0.33; all below the PISO onset of about 2.5 of ADR 0451. Courant: G40, fluid speed up to about
   1.7 near the body, `1.7 x 0.008 / 0.025 = 0.54`; BF, relative tangential speed about 1.2 at `r = 0.6` on a spacing of 0.0147,
   0.65. Run 10 periods (5000 steps, 40 D/U: the steady wake at Re 20 and the start transient of a strongly damped wake are
   expected to settle in a few tens of D/U; the periodicity is reported, P1). Window W of the measures: periods 9 and 10 (steps
   4001 to 5000).
9. **Cost.** From the F-308 timings (0.38 s per immersed step on 56 644 cells with the per-step rebuild, 0.28 s per body-fitted
   step on 32 768 cells): IB40 about 0.31 s x 5000 = 26 min, N1 the same, IB20 about 10 min, BF about 0.21 s x 5000 = 18 min,
   BFc about 5 min; the five runs on five threads of one ignored record, wall time about 30 min. Memory a few hundred MB (28.7 GB
   free at registration).
10. **Error estimates behind the limits.** The static forcing on D/40 (F-219, F-346, F-354): drag within 0.3 to 0.8 per cent and
    lift up to 2.7 per cent of the DFG values at Re 100; the moving forcing (F-308, in-line, Re 100): amplitude 0.84 per cent,
    phase 0.29 degree, mean 8e-5 of the amplitude. At Re 20 the layers are about 2.2 times thicker than at Re 100, which makes the
    first-fluid-cell error smaller; the staircase position error `h/2` remains, and the drag at Re 20 grows like `D^(1-n)` with
    the diameter at fixed U and nu, so an effective diameter error of `h/4` to `h/2` gives `0.55 x 0.006 to 0.0125 = 0.3 to 0.7`
    per cent. Estimates: mean drag 0.3 to 1.5 per cent; lift amplitude 0.5 to 2 per cent (the added-mass part is set by the
    volume of the solid, which the staircase approximates to about `h/D` per cent per cell row, averaged); lift phase about 1
    degree. The body-fitted reference, second order: BFc against BF estimated at 0.5 to 1.5 per cent in mean drag and 0.5 to 2 per
    cent in the lift harmonic (BFc has about 6 cells across the layer).

## Design

### D1. No library change

The F-308 library covers the case as it is: a body with zero own velocity and a `HarmonicTranslation` across the stream;
`MovingImmersed::step` rebuilds the forcing at the pose and the rigid velocity of `t + dt`; `velocity_in_forcing = false` is the
negative control. No line of a library file is changed; the default immersed path and every default result keep their bits by
construction. Everything of F-707 is test code in a new file `crates/rcfd-fv/tests/immersed_transverse_f707.rs` (the set-ups of
the F-308 test file are copied, not imported: an integration test file cannot use another's functions, and the F-308 file is
not edited).

### D2. The two runs (test code)

- **Common.** `PimpleSettings::piso(0.05)` (implicit Euler, one outer iteration, two correctors), `convection =
  ConvScheme::LinearUpwind` (the far cells reach a cell Peclet number of about `1 x 0.55 / 0.05 = 11`; the repository's cylinder-in-
  a-stream runs use linear upwind), `amg_pressure = true`; nothing else changed. Initial field `u = (1, 0, 0)` in every cell,
  `p = 0`. Velocity `Freestream { u_ref: (1,0,0), value: (1,0,0), fixed_on_inflow: true }` and pressure `Freestream { u_ref:
  (1,0,0), value: 0, fixed_on_inflow: false }` on every far-field patch; `frontAndBack` empty; depth 1.
- **IB (G40, G20, N1).** `box_2d_coords` of the two stretched axes (arithmetic 4); far-field patches `xmin`, `xmax`, `ymin`,
  `ymax`. One body, the cylinder of radius 0.5 centred at the origin with zero own velocity, motion of arithmetic 1;
  `ImmersedSettings::new(1e6)` (`K dt = 8000`, the ADR 0218 guideline `K >> a_P / V`, here about `1/dt + 4 nu / h^2 = 445`);
  `gamma = uniform_gamma(geom, nu)`; `MovingImmersed::step` per step. N1: G40 with `velocity_in_forcing = false`, the same 10
  periods.
- **BF (BF, BFc).** The O-grid of arithmetic 5 mapped by arithmetic 6 (`gen::warp`), moved by arithmetic 7; the cylinder starts
  as a fixed-value wall at rest and `set_follow_conditions` sets its velocity each step; `farfield` with the far-field condition
  (fixed, `w = 0` there); `step_ale_with_walls`.

### D3. Forces (existing library functions, as ADR 0631)

IB: `forcing.force_moment_unsteady(0, u^{n+1}, velocity_rate(u^{n+1}, u^n, None, dt), 1, x_c)`, the force of ADR 0631 D3 (the
staircase surface force at the new level, its storage cancelling the reaction of entering cells). BF:
`patch_force_moment_wall_gradient(geom, u, p, 1, nu, ["cylinder"], x_c, walls)` with the wall gradients of the step.

### D4. Measures

Over W (`N = 1000` steps, two whole periods), per run: the mean drag `Fbar_x = (1/N) sum F_x,n`; the first harmonics `Lhat =
(2/N) sum F_y,n exp(-i omega t_n)` and `Yhat` likewise of `y_c(t_n)`; the lift amplitude `|Lhat|`; the lift phase `phi_L =
arg(Lhat / Yhat)` in degrees, differences wrapped to `(-180, 180]`; the mean lift `Fbar_y`. Reported only: the drag second
harmonic (at `2 omega`), the parts of `Lhat` in phase with `-yddot` and with `-ydot` as `C_a = Re(Lhat conj(-Ahat)) / (rho pi r^2
|Ahat|^2)` and `C_v = Re(Lhat conj(-Vhat)) / (0.5 rho D U |Vhat|)` (`Ahat`, `Vhat` the harmonics of the body acceleration and
velocity), and the measures of period 9 and period 10 separately (P1).

### D5. Not built

A lock-in case (Re 100, Q2), rotating or deforming bodies, free motion, the second-order flux faces with motion, BDF2, SIMPLE,
three dimensions, a case-file key (Q1), the wake velocity profiles (not in the acceptance; the F-308 record checked them).

## Checks

Heavy record (one ignored test `f707_transverse_cylinder_record`, five runs on five threads, release, run once; it prints every
measure before it asserts). "BF" is the body-fitted fine run, "IB" the immersed run on G40.

| Id | Check | Limit | Estimate (arithmetic) |
| --- | --- | --- | --- |
| A1 | mean drag `abs(Fbar_x,IB - Fbar_x,BF) / abs(Fbar_x,BF)` (c2) | <= 0.03 | 0.003 to 0.015 (10) |
| A2 | lift amplitude `abs(abs(Lhat_IB) - abs(Lhat_BF)) / abs(Lhat_BF)` (c3) | <= 0.04 | 0.005 to 0.02 (10) |
| A3 | lift phase `abs(phi_L,IB - phi_L,BF)` (c4) | <= 3 degrees | about 1 degree (10) |
| A4 | lift first harmonic, complex `abs(Lhat_IB - Lhat_BF) / abs(Lhat_BF)` (c3, c4 together) | <= 0.06 | 0.01 to 0.03 |
| A5 | mean lift `abs(Fbar_y,IB - Fbar_y,BF) / abs(Lhat_BF)` | <= 0.02 | both small (half-period symmetry of the periodic state) |
| R1 | reference: BFc against BF, mean drag (as A1) and complex lift harmonic (as A4) | <= 0.02; <= 0.03 | 0.005 to 0.015; 0.005 to 0.02 (10) |
| N1a | negative control (velocity left out, G40): mean drag as A1 | > 0.03 (must exceed) | 0.05 to 0.12 (3) |
| N1b | N1: lift amplitude as A2 | > 0.04 (must exceed) | above 0.9 (3) |
| N1c | N1: lift phase as A3 | > 3 degrees (must exceed) | arbitrary phase of a lift near zero; expected far off |
| N1d | N1: complex lift harmonic as A4 | > 0.5 (must exceed) | above 0.9 (3) |
| S1 | every run: no divergence stop, finite fields; largest step continuity residual reported | no stop | |
| C1 | trend, reported: the A1 and A4 differences of G20 larger than those of G40 | both larger | first order |
| P1 | periodicity, reported: period 9 against period 10 of mean drag and lift amplitude, per run | reported | |

Fast tests (not ignored; release), with limits:

| Id | Check | Limit |
| --- | --- | --- |
| T1 | the square map (BF and BFc meshes): every `farfield` face centre has `max(abs x, abs y) = 8` within 1e-12; every face on `x = -8` has area-vector components `y`, `z` exactly 0 and `x < 0`, every face on `y = +-8` has the `x` component exactly 0; the inflow faces (`n . U < 0`) are exactly the `n_theta / 4` faces on `x = -8`; the box's inflow faces are exactly its `xmin` faces; the points with `rho <= 2` are unchanged bit for bit; the smallest cell volume is positive at the poses `y_c = -A, 0, A` | as stated |
| T2 | the measures of D4 on synthetic series on the step grid of W: `F = m + a cos(omega t + p) + b cos(2 omega t) + c sin(3 omega t)` gives `m`, `a`, and the phase `arg(Lhat / Yhat)` of the series; `F = y_c` gives phase 0, `F = ydot` +90 degrees, `F = -yddot` 180 degrees (wrapped) | 1e-12 relative; 1e-9 degree |
| T3 | IB smoke (G20 set-up of D2, 40 steps; with and without the velocity): no stop, finite force; the forcing's body centre is `(0, y_c(t_n), 0)` exactly; every solid cell holds `(0, ydot(t_n))` within `1e-3 A omega` (with the velocity) or `0` within `1e-3 A omega` (without) | as stated |
| T4 | BF smoke (`cylinder_ogrid(64, 24, ..)` mapped and moved as D2, 40 steps): no stop, every step continuity <= 1e-6; the mean of the `cylinder` patch points is `(0, y_c(t_n))` within 1e-12 after the last step; the `farfield` points unchanged bit for bit | as stated |

### Negative controls

N1 (heavy, above, the clause c5) and its fast half in T3 (without the velocity the solid holds zero velocity). The fixed
cylinder is not run separately: N1 is close to it (arithmetic 3).

### Planned mutations (each run once against the fast tests, on a scratch copy)

| Id | Mutation | Expected to catch |
| --- | --- | --- |
| M1 | the exact placement of the outer ring left out (the map alone puts it on the square up to round-off) | T1 |
| M2 | the corner placement left out (`y = 8 s/abs(c)` also at the corners) | T1 |
| M3 | BF moves the mesh along x instead of y | T4 |
| M4 | BF solver built on the unmoved grid (the F-308 amendment 2 defect) | T4 |
| M5 | IB motion amplitude along x | T3 |
| M6 | the harmonic with `exp(+i omega t)` | T2 |
| M7 | the lift phase taken as `arg Lhat` (not relative to `Yhat`) | T2 |
| M8 | library (scratch copy of `immersed_motion.rs`): `bodies_at` leaves out the motion velocity regardless of the flag | T3 |

## Decision rule

- **done**: A1 to A5 and R1 met; N1a to N1d each exceeded; S1 for every run; T1 to T4 pass; at least seven of the eight
  mutations caught; no library line changed (the default bits by construction).
- **partial**: any of A1 to A5 or R1 missed (with the diagnosed cause), any of N1a to N1d not exceeded, a run stopped, or a check
  met only after an amendment.
- **todo**: the case cannot be built or run.
- C1 and P1 are reported and do not change the decision.

## Limits (what this does not cover)

One case (Re 20, A/D 0.5, f D/U 0.25, two dimensions, one mesh pair for the comparison, a square domain of half side 8 D with a
far-field condition, the same in both runs); lock-in and vortex shedding (Re 100) are not run (Q2). The time step is the same in
both runs, so the time discretisation error is common and not measured. The reference is this repository's body-fitted run, not
a published data set. The first-order forcing of ADR 0218. The early force (the start transient, ADR 0421/0425) is outside the
window and not compared. The wake is not compared. Items of D5.

## Open questions

- **Q1. A case-file key for moving immersed bodies.** As ADR 0631 Q1: (a) in the F-307 table when F-307 is made; (b) a separate
  item. Recommendation (a); open (needs the schema golden edit of F-307), not taken.
- **Q2. Which low Reynolds number.** (a) Re 20, below the shedding onset, one forced periodic response (taken, on the maintainer's
  standing instruction of 2026-11-11: take the recommendation); (b) Re 100 at lock-in, the classic case, with the phase of the
  lift sensitive to the natural frequency of each discretisation; (c) both. Recommendation (a) here and (b) as a follow-up with
  limits that allow for the frequency sensitivity.
