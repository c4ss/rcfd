# ADR 0657: F-731 registration: moving immersed shapes at lock-in, a transversely oscillating cylinder shedding vortices

Status: **REGISTERED 2026-11-16, written before any code of F-731 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-731 was written, compiled or run: the documents and code under "What
was read" were read and the arithmetic under "Arithmetic before this registration" was done by hand and with a few lines of
Python (pure arithmetic of the case numbers, no solver). No timing probe was run; run lengths are set from the timings of the
F-707 record (ADR 0648 addendum 1). Results go in the addenda at the end; nothing above the addenda is changed after a run.

Parents: [ADR 0648](0648-f707-registration-a-transversely-oscillating-cylinder-in-a-stream.md) (F-707: the transverse case at
Re 20, its test file `crates/rcfd-fv/tests/immersed_transverse_f707.rs`, the square map of the O-grid, the far field, the
measures; its surviving mutation M4, the body-fitted solver started on the unmoved grid; its question Q2, from which F-731 was
made), [ADR 0631](0631-f308-registration-moving-immersed-shapes.md) (F-308: `MovingImmersed`, the force D3, the body-fitted
reference, amendment 2: the body-fitted mesh starts at the initial pose), [ADR 0218](0218-f219-registration-immersed-solid-with-modified-forcing.md)
(the forcing; DFG 2D-2 Strouhal number within 0.38 per cent), [ADR 0249](0249-f346-registration-immersed-force-in-unsteady-flow.md)
(`force_moment_unsteady`, `velocity_rate`), ADR 0252 (`patch_force_moment_wall_gradient`), ADR 0033 and ADR 0101 (the
swept-volume moving-mesh step, `docs/dynamic-mesh.md`), [ADR 0451](0451-f530-registration-piso-stability-at-large-diffusion-numbers.md)
(the non-iterated PISO step diverges above a cell diffusion number of about 2.5), ADR 0060 (`validation_cylinder_re100.rs`: this
repository's first-order Strouhal number at Re 100 on O-grids, 0.147 to 0.159, and its start perturbation). Roadmap item F-731
(deps F-707). Number 0657 given by the lead. Branch `f731-work` from main ec78685f, worktree `w49i-work/f731`, scratch
`w49i-work/f731-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; existing tests not
edited; defaults bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new
dependency, no unsafe code, crate graph unchanged; clean-room. The roadmap files are not edited. Every choice below that the
maintainer would normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the
recommendation), except the open questions at the end.

## What F-731 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. A cylinder oscillating across a uniform stream at Re 100 inside the lock-in range, away from the phase jump, gives the shedding locked to the forcing, the mean drag, the lift amplitude and the phase of the lift against a body-fitted moving-mesh run of this repository within registered limits that allow for the natural-frequency difference of the two discretisations; a fast test checks that the body-fitted solver starts on the grid at the initial pose; a negative control with the body velocity left out of the forcing misses them.

Clauses: **c0** every limit registered here; **c1** Re 100, a cylinder oscillating across a uniform stream, inside the lock-in
range; **c2** away from the phase jump; **c3** the shedding locked to the forcing; **c4** mean drag, **c5** lift amplitude,
**c6** lift phase, each against a body-fitted moving-mesh run of this repository, within registered limits that allow for the
natural-frequency difference of the two discretisations; **c7** a fast test that the body-fitted solver starts on the grid at the
initial pose; **c8** the negative control with the body velocity left out of the forcing misses them (c4 to c6). The brief adds:
reference values only from this repository's body-fitted run or a closed form; published lock-in numbers may only frame the
forcing choice; heavy runs ignored records run once with fast tests over the same path; mutations and negative controls
registered before the run.

### Readings, registered

- **The reference** is the body-fitted moving-mesh run of this repository (`PimpleSolver::step_ale_with_walls` with the follow
  condition of the cylinder wall), exactly the F-707 path (ADR 0648 D2), on a finer O-grid for Re 100 (arithmetic 4). Its own
  accuracy is shown by a coarser O-grid (R1). No published value is used as a reference.
- **"Inside the lock-in range"** is shown, not assumed: the run is locked when its lift is periodic with the forcing period
  (L1, L2 below), measured in every forced run that is compared. The forcing frequency is framed from published knowledge cited
  from memory (Koopmann 1967 and the general picture of forced transverse oscillation: at Re 100 the lock-in range widens with
  the amplitude and reaches well beyond 10 per cent on either side of the natural frequency for A/D of a few tenths; the jump of
  the lift phase lies near or somewhat above the natural frequency). These are not used as numbers in any check; they choose
  `A/D = 0.5` (a wide lock-in range) and a forcing about 12 per cent below the expected natural frequency (arithmetic 1).
- **"Away from the phase jump"** is shown by the reference itself: two more body-fitted runs at forcing frequencies about 3 per
  cent below and above give the change of the lift phase across the bracket (J1). Near the jump the phase changes by order 100
  degrees over a few per cent of frequency; away from it by a few degrees.
- **"The natural-frequency difference of the two discretisations"** is measured: each discretisation is run with the cylinder
  at rest from the same perturbed start and its shedding frequency `f0` is taken from the zero crossings of the lift (F0, F1). The
  comparison is made at the same physical forcing frequency `f`; the two runs then sit at different frequency ratios `r = f / f0`.
  The limit of each measure is a fixed base plus an allowance: the slope of that measure with respect to `r` in the reference
  (from the bracket runs) times the ratio difference `Delta r = r_IB - r_BF` (D4). The allowance is registered as a formula here;
  its inputs are measured.
- **"Drag", "lift", "phase"** as ADR 0648: per unit depth, `F_x` and `F_y`; mean drag the window mean of `F_x`; lift amplitude
  `|Lhat|` of the first harmonic at the forcing frequency; lift phase `phi_L = arg(Lhat / Yhat)` in degrees (positive when the
  lift leads the displacement), differences wrapped to `(-180, 180]`.
- **"Misses them"**: the negative control's mean drag, lift amplitude and lift phase each lie outside the limit that the immersed
  run must meet (N1a to N1c, with the immersed run's own allowance), and its complex lift harmonic outside the A4 limit (N1d).
- **The domain** is that of F-707: the square `[-8, 8]^2`, the `Freestream` far field on its whole edge in both meshes, the
  O-grid mapped to the square with its outer ring placed exactly (ADR 0648 arithmetic 6), so that both meshes have the same inflow
  faces.

## What was read

`w49i-work/AGENT_RULES.md`; the brief; ADR 0648 (all, with its amendments and addendum); `crates/rcfd-fv/tests/immersed_transverse_f707.rs`
(all); `crates/rcfd-fv/src/immersed_motion.rs` (all); `crates/rcfd-fv/src/ale.rs` (`set_follow_conditions`); `crates/rcfd-fv/src/sixdof.rs`
(`patch_force_moment_wall_gradient`); `crates/rcfd-api/tests/validation_cylinder_re100.rs` (the start perturbation, the zero
crossings, its measured Strouhal numbers 0.14665, 0.15450, 0.15878 on three O-grids, first order, ADR 0060 known miss); the
Strouhal lines of ADR 0218 (IB DFG 2D-2 `St` -0.38 per cent) and ADR 0249; the "For the lead" sections of ADR 0572, 0573 and
0648; the F-707 record log (`w49i-work/f707-scratch/heavy_record.log`, the timings); `docs/dynamic-mesh.md` (headings).

## Arithmetic before this registration (disclosed)

1. **Case numbers.** D = 1, r = 0.5, U = 1 along x, rho = 1, `nu = 0.01` (Re 100). Motion across the stream `y_c(t) = -A cos(omega
   t)`, `A = 0.5 D` (`HarmonicTranslation { amplitude: [0, 0.5, 0], omega, phase: -pi/2 }`), forcing `f D / U = 0.14`, `T =
   7.142857`, `omega = 0.879646`. Largest body velocity `A omega = 0.440 U`, acceleration `A omega^2 = 0.387`. Natural Strouhal
   number expected 0.15 to 0.17 in this box (the repository's O-grid values 0.147 to 0.159 at first order, raised by a few per cent
   by the blockage 1/16): `r = f / f0` between 0.82 and 0.93.
2. **Time step and run lengths.** `dt = T / 700 = 0.0102041` for **every** run (fixed and forced; the time error of implicit Euler
   shifts `f0` and the response alike, so all runs share it). Bracket runs: forcing periods of 720 and 680 steps, `f- = 0.136111`
   (-2.78 per cent), `f+ = 0.144118` (+2.94 per cent), integer steps per period so the harmonics of D5 are exact on the step grid.
   Each forced run: 20 of its own periods from the uniform stream, the body at rest at the lower turning point at `t = 0` (14 000,
   14 400, 13 600 steps; 143 D/U). Window W: its last 4 periods (the lock-in transient of a wake forced at A/D 0.5 is expected to
   be over in well under 15 periods; L1 and P1 show it). Fixed runs: 14 000 steps (`t = 142.86`); the `f0` window `t >= 80`, about
   10 shedding cycles.
3. **Start of the fixed runs.** The symmetric start of a fixed cylinder on a symmetric mesh may take long to break; both fixed
   runs start from `u = (1, 0.1 exp(-((x - 2)^2 + (y - 0.3)^2)), 0)` (the perturbation of ADR 0060's test), the same function of
   position in both meshes. The forced runs start from the uniform stream (the motion breaks the symmetry), as F-707.
4. **Meshes.** IB: the F-707 G40 (uniform `h = D/40` core on `[-1.25, 2.5] x [-1.5, 1.5]`, growth 1.08 to the edges; 228 x 200 =
   45 600 cells): about 4 cells across the boundary layer `D / sqrt(Re) = 0.1`. BF: `cylinder_ogrid(256, 128, 0.5, 8, Expansion(20),
   1)` mapped to the square: `q = 20^(1/127) = 1.02387`, first cell `0.00919`, about 9 cells across the layer, 32 768 cells. BFc
   (R1): `cylinder_ogrid(128, 64, .., Expansion(20), ..)`, `q = 1.0487`, first cell 0.0183, 8 192 cells. Smoke: `(64, 32)`,
   first cell 0.0362, 2 048 cells. Mesh motion as ADR 0648 arithmetic 7 (rigid within `rho0 <= 1.5`, cosine blend to 0 at 6).
5. **Stability.** Cell diffusion numbers `nu dt / h^2`: IB 0.16; BF at the wall `0.01 x 0.0102 / 0.00919^2 = 1.21`; BFc 0.31; all
   below the PISO onset of about 2.5 (ADR 0451). Courant: IB `1.7 x 0.0102 / 0.025 = 0.69`; BF about `1.5 x 0.0102 / 0.0147 =
   1.04` at `r = 0.6` (implicit Euler; reported per run).
6. **The lock measure on a sinusoid at `f0`.** For `F = sin(2 pi f0 t)` the lagged difference `F(t) - F(t - T)` has the rms `2
   |sin(pi f0 / f)|` times that of `F`; for `r` between 0.82 and 0.93 that is 1.25 to 0.45, and still 0.19 at `r = 0.97`. A locked,
   periodic lift gives 0 up to the slow drift of the settling flow (estimate below 1e-2 after 16 periods).
7. **Cost.** From F-707 (0.59 s per immersed step on 45 600 cells and 0.376 s per body-fitted step on 24 576 cells, five runs on
   a shared machine): IB forced and N1 about 14 000 x 0.6 s = 2.3 h each, IB fixed (forcing built once) about 1.8 h, BF about 0.5 s
   x 14 000 = 1.9 h each (BF fixed, BF, BF-, BF+), BFc about 0.5 h each (fixed, forced). Nine runs on nine threads of one ignored
   record, wall time estimated 2.5 to 4 h. Memory: a few hundred MB per run (36 GB free at registration).
8. **Error estimates behind the base limits.** The static forcing at Re 100 (F-219: DFG 2D-2 `St` -0.38 per cent; drag 0.3 to 0.8
   per cent, lift up to 2.7 per cent over F-219/F-346/F-354); the moving forcing at Re 20 (F-707): drag 0.2 per cent, lift
   harmonic 0.9 per cent, phase 0.2 degree. At Re 100 the boundary layer is about 2.2 times thinner (4 cells on G40), and at lock-in
   the lift is set by the formation of the near-wake vortices, which both meshes resolve differently (the IB core ends at x = 2.5,
   the O-grid coarsens radially); after the frequency allowance: drag 0.5 to 2 per cent, lift amplitude 1 to 4 per cent, phase 1 to
   4 degrees, complex harmonic 2 to 6 per cent. The base limits are about twice the upper estimates. The natural-frequency
   difference `delta = f0_IB / f0_BF - 1`: effective diameter error of the staircase `h/4` to `h/2` (-0.6 to -1.2 per cent in
   `f0`) plus wake resolution, estimated `|delta|` 0.5 to 3 per cent. BFc against BF: 0.5 to 1.5 per cent drag, 1 to 4 per cent
   lift harmonic.

## Design

### D1. No library change

The F-308 library covers the case: `MovingImmersed` with a `HarmonicTranslation` across the stream and `velocity_in_forcing =
false` for the negative control. No line of a library file is changed; the default bits are unchanged by construction. All of
F-731 is test code in a new file `crates/rcfd-fv/tests/immersed_lockin_f731.rs` (helpers copied from the F-707 file, which is not
edited; the case constants are parameters of a run specification instead of constants).

### D2. Runs (test code)

Common: `PimpleSettings::piso(0.01)`, `ConvScheme::LinearUpwind`, `amg_pressure = true`; the far field of ADR 0648 D2; `dt` of
arithmetic 2.

- **IB forced** (G40, `f`), **N1** (G40, `f`, velocity left out): `MovingImmersed::step` per step, `ImmersedSettings::new(1e6)`
  (`K dt = 1e4`), force of ADR 0648 D3.
- **IB fixed** (G40): the forcing of the cylinder at the origin built once (`ImmersedForcing::build`, the same as
  `MovingImmersed::forcing_at` of a body without motion), `PimpleSolver::step` per step, the perturbed start of arithmetic 3; force
  as above.
- **BF forced** (BF at `f`, `f-`, `f+`; BFc at `f`): the F-707 body-fitted step, the solver built on the mesh at the initial pose
  `y0 + y_c(0) w` (ADR 0631 amendment 2, c7).
- **BF fixed** (BF, BFc): the same body-fitted step with `A = 0` (the mesh is moved to its own points each step, mesh flux zero,
  wall velocity zero), the perturbed start. Same path as the forced runs, so `f0` and the forced response come from one solver.

### D3. Natural frequency

Fixed runs: over the steps with `t_n >= 80`, the upward crossings of `F_y - mean` (linear interpolation between steps, as ADR
0060). With `k` crossings (at least 9 required), `f0 = 8 / (t_{k} - t_{k-8})` over the last eight cycles. Saturation: the
frequency of the first four and the last four of these cycles, and the lift half range `(max - min)/2` over each four cycles.

### D4. The comparison and its allowance

`r_X = f / f0_X` for X in {BF, BFc, IB} (BF and BFc from their own fixed runs, IB and N1 from the IB fixed run: the IB fixed
cylinder is the same mesh and forcing without motion). Bracket ratios `r- = f- / f0_BF`, `r+ = f+ / f0_BF`, width `w_r = r+ - r-`.
Slopes of the reference, from the bracket runs: `s_D = (Fbar_x+ - Fbar_x-) / (Fbar_x,BF w_r)`; `s_A = (|Lhat+| - |Lhat-|) /
(|Lhat_BF| w_r)`; `s_P = wrap(phi+ - phi-) / w_r` (degrees); `s_C = |Lhat+ - Lhat-| / (|Lhat_BF| w_r)`. The complex harmonics of
different frequencies are comparable because every run has `Yhat = -A` exactly (the displacement `-A cos(omega t)` over whole
periods of its own step grid). Allowance of measure X for a run at ratio `r`: `|s_X (r - r_BF)|`. Limit = base + allowance.

### D5. Measures

Over W (`N` = 4 periods of `N_p` steps): mean drag, first harmonic of the lift, phase, mean lift as ADR 0648 D4; and

- **L1 (lock, lagged)**: `sqrt(sum_{n in W, n >= w0 + N_p} (F_y,n - F_y,n-N_p)^2) / sqrt(sum_{same n} (F_y,n - Fbar_y)^2)`;
- **L2 (lock, spectral)**: the rms over W of `F_y` minus its mean and its harmonics 1 to 5 of the forcing frequency, over the rms
  of `F_y - Fbar_y`.

Reported: the drag second harmonic, `C_a` and `C_v` (ADR 0648 D4), the measures per period of W (P1), the Strouhal numbers.

### D5a. Not built

Other amplitudes or frequencies than the three forcing frequencies; the phase jump itself; rotating, deforming or freely moving
bodies; BDF2; second-order immersed flux faces; three dimensions; a case-file key (Q1 of ADR 0648); wake profiles.

## Checks

Heavy record: one ignored test `f731_lockin_record`, nine runs on nine threads in release, run once; it prints every measure
before it asserts. Runs: BF0, BFc0, IB0 (fixed); BF, BF-, BF+, BFc, IB, N1 (forced).

| Id | Check | Limit | Estimate (arithmetic) |
| --- | --- | --- | --- |
| F1 | fixed runs BF0, BFc0, IB0: at least 9 upward crossings after t = 80; first and last four cycles: frequency within 0.005, lift half range within 0.02 (relative) | as stated | saturated by t = 80 (3) |
| F0 | natural-frequency difference `abs(f0_IB / f0_BF - 1)` and `abs(f0_BFc / f0_BF - 1)` (the allowance stays a near interpolation) | <= 0.045 each | 0.005 to 0.03 (8) |
| F2 | reported: `St` of each, `r_BF`, `r_IB`; expected `r` in [0.75, 0.97] (the framing of c1) | reported | 0.82 to 0.93 (1) |
| L1 | lock, lagged: IB, BF, BF-, BF+, BFc (c1, c3) | <= 0.03 each | below 1e-2 (6) |
| L2 | lock, spectral: the same runs (c3) | <= 0.05 each | below 2e-2 |
| X1 | the lock measure discriminates: L1 of the fixed runs BF0 and IB0 evaluated with the forcing period over their last 2 800 steps | > 0.1 each (must exceed) | 0.45 to 1.25 (6) |
| J1 | away from the phase jump (c2): `abs(wrap(phi+ - phi-))` across the bracket; and `abs(abs(Lhat+) - abs(Lhat-)) / abs(Lhat_BF)` | <= 25 deg; <= 0.25 | a few degrees; a few per cent |
| A1 | mean drag `abs(Fbar_x,IB - Fbar_x,BF) / Fbar_x,BF` (c4) | <= 0.04 + allowance `abs(s_D Delta r)` | 0.005 to 0.02 (8) |
| A2 | lift amplitude `abs(abs(Lhat_IB) - abs(Lhat_BF)) / abs(Lhat_BF)` (c5) | <= 0.08 + `abs(s_A Delta r)` | 0.01 to 0.04 |
| A3 | lift phase `abs(wrap(phi_IB - phi_BF))` (c6) | <= 8 deg + `abs(s_P Delta r)` | 1 to 4 deg |
| A4 | lift harmonic, complex `abs(Lhat_IB - Lhat_BF) / abs(Lhat_BF)` (c5, c6) | <= 0.12 + `abs(s_C Delta r)` | 0.02 to 0.06 |
| A5 | mean lift `abs(Fbar_y,IB - Fbar_y,BF) / abs(Lhat_BF)` | <= 0.05 | both near 0 (half-period symmetry) |
| R1 | reference: BFc against BF at `f`, drag as A1 and complex harmonic as A4, with BFc's own allowance (`Delta r_c = r_BFc - r_BF`) | <= 0.03 + allowance; <= 0.08 + allowance | 0.005 to 0.015; 0.01 to 0.04 (8) |
| N1a | velocity left out (G40, `f`): mean drag as A1 | > the A1 limit (must exceed) | F-707: +10.8 % at Re 20; here unknown, expected a few to 15 % |
| N1b | N1: lift amplitude as A2 | > the A2 limit | F-707: -59 %; here expected 20 to 60 % |
| N1c | N1: lift phase as A3 | > the A3 limit | F-707: 63 deg; here expected tens of degrees |
| N1d | N1: complex harmonic as A4 | > the A4 limit | 0.3 to 0.9 |
| S1 | every run: no divergence stop, finite force and fields; largest step continuity reported | no stop | |
| P1 | reported: the measures per period of W, per forced run; L1 and L2 of N1 | reported | |

`Delta r = r_IB - r_BF`. N1 uses the IB allowance (the same mesh and forcing; its `f0` is the IB fixed run's).

Fast tests (not ignored; release):

| Id | Check | Limit |
| --- | --- | --- |
| T1 | the BF (256 x 128), BFc (128 x 64) and smoke (64 x 32) meshes of Expansion(20) mapped to the square: far-field face centres on the square within 1e-12, the faces with all points on `x = -8` have area vectors `(n_x < 0, 0, 0)` exactly and are exactly the inflow faces (`n_theta / 4`), faces on `y = +-8` have `n_x = 0` exactly, points with `rho <= 2` unchanged bit for bit, the smallest cell volume positive at the poses `y_c = -A, 0, A` | as stated |
| T2 | measures on synthetic series on the step grids of W (700, 720 and 680 steps per period): mean, amplitude and phase of `m + a cos(omega t + p) + 0.3 cos(2 omega t) + 0.2 sin(3 omega t)`; L1 and L2 of that series; L1 of `sin(2 pi f0 t)` with `f0 = f / 0.875` against the closed form `2 abs(sin(pi f0 / f))` of arithmetic 6; `f0` of D3 from a series `0.4 + sin(2 pi f0 t + 0.3)` sampled on the step grid; the allowance of D4 on given numbers against the closed form; every run specification has `dt` bit-equal to `T / 700` and an integer number of steps per period | 1e-12 relative (mean, amplitude, L1 and L2 of the periodic series as absolute 1e-12); 1e-9 deg; closed-form L1 within 1e-3 (finite window); `f0` within 1e-6 relative; allowance 1e-14 |
| T3 | IB smoke (G20 of F-707, Re 100 settings, `f`, 40 steps; with and without the velocity): no stop, finite force; the forcing after each step equals `forcing_at(solver.time)`; the solid cells hold `(0, ydot)` (or 0) within `1e-3 A omega` | as stated |
| T4 | BF smoke (64 x 32, `f`, 40 steps): no stop, every step continuity <= 1e-6; the mean of the `cylinder` patch points is `(0, y_c(t_n))` within 1e-12; the far-field points unchanged bit for bit | as stated |
| T5 | **the body-fitted solver starts on the grid at the initial pose** (c7): for the smoke mesh at `f`, before any step the solver's mesh points equal `y0 + y_c(0) w` bit for bit, the mean of its `cylinder` points is `(0, -A)` within 1e-12, and they differ from the unmoved points (largest difference `A` within 1e-12); the fixed specification (`A = 0`) starts on the unmoved points bit for bit | as stated |
| T6 | fixed set-ups smoke (G20 IB and 64 x 32 BF, 20 steps): no stop, finite force; before the first step the velocity in every cell equals the perturbation of arithmetic 3 at its centre bit for bit (BF; IB: every cell outside the solid, the solid cells hold 0 within 1e-3 after the first step) | as stated |

### Negative controls

N1 (heavy, c8) and its fast half in T3. X1 is the control of the lock measure (an unlocked lift must fail L1). The bracket runs
are the control of c2.

### Planned mutations (each run once against the fast tests, on a scratch copy)

| Id | Mutation | Expected to catch |
| --- | --- | --- |
| M1 | BF solver built on the unmoved grid (the F-707 M4 survivor) | T5 |
| M2 | BF solver built at the pose of `t = dt` instead of `t = 0` | T5 |
| M3 | L1 with the lag `N_p - 1` | T2 |
| M4 | `f0` of D3 dividing 8 cycles by the span of 7 | T2 |
| M5 | allowance with the slope over half the bracket width | T2 |
| M6 | the bracket run of 720 steps per period given `dt = T- / 700` (its own period) | T2 |
| M7 | the perturbation of the fixed start left out of the BF set-up | T6 |
| M8 | BF motion along x | T4 |
| M9 | library (scratch copy of `immersed_motion.rs`): `bodies_at` leaves out the motion velocity regardless of the flag | T3 |

## Decision rule

- **done**: F1, F0, L1, L2, X1, J1, A1 to A5, R1 met; N1a to N1d each exceeded; S1 for every run; T1 to T6 pass; at least eight
  of the nine mutations caught, M1 among them; no library line changed.
- **partial**: any of these missed (with the diagnosed cause), a run stopped, or a check met only after an amendment.
- **todo**: the case cannot be built or run.
- F2 and P1 are reported and do not change the decision.

## Limits (what this does not cover)

One amplitude (A/D 0.5) and one forcing frequency compared (with a bracket of +-3 per cent in the reference only), Re 100, two
dimensions, one mesh pair for the comparison, the square domain of half side 8 D with its short wake (7.5 D behind the
cylinder), the same in both runs. The time step is shared, so the time discretisation error is common and not measured. The
reference is this repository's body-fitted run, not a published data set; the lock-in range and the location of the phase jump
are not mapped (only the neighbourhood of the one frequency). The allowance assumes the response depends on the forcing through
`f / f0` alone near the chosen point (at a fixed amplitude and Re), which is the usual similarity of forced wakes; it is not
tested beyond the bracket. First-order forcing of ADR 0218. Items of D5a.

## Open questions

- **Q1. A case-file key for moving immersed bodies.** As ADR 0648 Q1. Open (schema golden), not taken.
- **Q2. The base limits.** They are wider than F-707's (lock-in lift is set by the near wake, resolved differently in the two
  meshes). Options: (a) as registered; (b) tighter, with a finer IB core. Taken (a) on the maintainer's standing instruction of
  2026-11-11 (take the recommendation).
