# ADR 0218: F-219 registration: an immersed (non-conforming) solid with modified forcing

Status: **REGISTERED 2026-11-07**, written before any code of F-219 and before any run of its checks. Results in addendum 1 (every
acceptance check met: 2D-1 `C_D` -0.02 %, `C_L` +0.67 %, 2D-2 `St` -0.38 %; the order limit of V1 missed, 1.02 against 1.5, kept
as an ignored known miss; F-219 partial by the decision rule). Not yet accepted by the maintainer.

Parents: ADR 0029 D3 (the solid-cell mask of the block-coupled solver, a staircase without forcing), ADR 0035 (the Carman-Kozeny
damping, an implicit momentum sink), ADR 0067 (the divergence guard that carries the refusals), ADR 0214 (the house rules and the
refusal pattern followed here), ADR 0062 (wave 31A, the Strouhal measurement of `crates/rcfd-fv/tests/cylinder.rs`). Roadmap item
F-219 (depends on F-051 and F-114, both done).

Number: the lead gave 0218 to F-219. Main 981804b holds 0214 as its highest; 0215 is on `f247-local-remeshing`, 0216 on
`f250-sliding-interface`, 0217 on `f221-wet-steam`. No branch holds a 0218.

What was run before this text (disclosed): no code of F-219 and no run of a check below. One timing probe of the existing solvers,
on an empty DFG channel without any body (scratch test, deleted, not committed; release build of main 981804b): SIMPLEC with central
convection of momentum on 361 x 228 uniform cells (82 308) took 0.40 s per outer iteration (about 300 pressure iterations per
iteration); PISO with BDF2, central convection, two correctors and AMG pressure on the same mesh took 0.31 s per step at `dt = 0.001`, inflow
maximum 1.5. These numbers set the mesh sizes and run lengths below and nothing else.

House rules (ADR 0207's, ADR 0211's and ADR 0214's):

- limits are fixed here, before the code and the runs, and are not changed after a run;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the measured number recorded, and the cause is
  diagnosed, not tuned away;
- a defective set-up gets a new corrected test by an amendment written before the corrected run, and the old one stays as an
  ignored record;
- every new code path is opt-in (`immersed: None` on both solvers by default): a run without it runs as before, bit for bit; no
  case-schema key is added, so `crates/rcfd-api/tests/baseline_hashes.rs`, `case_hashes_w48u.rs` and the goldens are not touched;
  no existing test file is edited;
- each registered run is made once, and nothing is re-run to pick a result;
- a change that the default path would need is not made: it is an open question with a recommendation;
- no new third-party crate, no commit trailers.

Clean-room: the forcing is the volume penalisation of Angot, Bruneau and Fabrie (Numer. Math. 81 (1999) 497-520) in the implicit
form of a momentum sink (the form rcfd already uses for porous media and the Carman-Kozeny damping), the limit of the direct forcing
of Mohd-Yusof (1997) and Fadlun, Verzicco, Orlandi and Mohd-Yusof (J. Comput. Phys. 161 (2000) 35-60) for a vanishing penalty
time. The modification of the forcing in the first fluid cells is the linear extrapolation of a Dirichlet value along a grid line
to the true boundary crossing, as in Gibou, Fedkiw, Cheng and Kang (J. Comput. Phys. 176 (2002) 205-227), written as a face
coefficient. These are cited from memory of the published papers; no text of them was read for this work and no formula is
copied: the algebra below is derived here on the discrete equations of `crates/rcfd-fv/src/simple.rs` and `pimple.rs`. The
reference values were read on 2026-11-07 on the public FeatFlow benchmark pages of TU Dortmund
(`wwwold.mathematik.tu-dortmund.de/~featflow/en/benchmarks/cfdbenchmarking/flow/dfg_benchmark1_re20.html` and
`dfg_benchmark2_re100.html`), which give the set-up of M. Schaefer and S. Turek, "Benchmark computations of laminar flow around a
cylinder" (Notes on Numerical Fluid Mechanics 52, Vieweg 1996, 547-566). The lower and upper bounds of the 1996 paper were not
obtained (the paper was not read) and are not used. No vendor code, text, manual or install was used.

## What F-219 asks

The roadmap entry: an immersed (non-conforming) solid domain with modified forcing; acceptance "Flow past a cylinder with an
immersed solid matches the published DFG 2D-1 drag, lift and Strouhal number within 5 percent".

**Reading of the acceptance (registered here).** DFG 2D-1 (Re 20) is steady and defines no Strouhal number; the Strouhal number of
the DFG set is defined by 2D-2 (Re 100, periodic), the same geometry and fluid with a five times larger inflow. The acceptance is
read as: the drag and lift coefficients of 2D-1 and the Strouhal number of 2D-2, each within 5 % of the published reference value,
with the cylinder an immersed solid on a Cartesian mesh that does not conform to it (Q1). "Published" is the FeatFlow reference:

| Quantity | Reference | Source | 5 % band |
| --- | --- | --- | --- |
| 2D-1 `C_D` | 5.57953523384 | FeatFlow 2D-1 page ("computed with high order spectral methods") | [5.30056, 5.85851] |
| 2D-1 `C_L` | 0.010618948146 | same | [0.0100880, 0.0111499] |
| 2D-2 `St` | 0.30188 | FeatFlow 2D-2 page, level 6, `k = 1/200` | [0.286786, 0.316974] |

Printed with their FeatFlow values, no limit: 2D-1 pressure difference (0.11752016697); 2D-2 max and min `C_D` (3.2200, 3.1569),
max and min `C_L` (0.9859, -1.0206), mean `C_D` (3.1884), `1/f` (0.33125 s).

**Reading of "modified forcing" (registered here).** Forcing: a momentum source that drives the velocity of the cells whose centre
lies in the solid to the solid velocity. Modified: in the first fluid cells the forcing is changed so that the no-slip value is
imposed at the true boundary crossing instead of at the centre of the solid neighbour (the staircase). Both are implicit (matrix
diagonal and right-hand side) and both are switched by one flag, so that the plain forcing is available as the negative control.

## What the code read shows (main 981804b, read only)

1. **SIMPLE/SIMPLEC** (`SimpleSolver::iterate`): per active component the momentum matrix is convection plus the Laplacian with
   face coefficient `gamma_f lap_coeff_f` (`lap_coeff = |S|^2/(S . d)`, ADR geometry), then `sink[c] V_c` is added to the diagonal
   (`sink: Option<Vec<f64>>`, the implicit `-sink u` of porous media), then the pseudo-time term, then `relax(alpha_u)`. `H = b -
   sum offdiag u` (from the solved predictor by default), `a_P` the component-averaged diagonal, `rAU = V/a_P`, SIMPLEC
   `rAtU = V/(a_P - h1)`. A right-hand-side term added to `m.b` with the diagonal term therefore enters `HbyA` and the face flux like
   any other part of `H`. The pressure gradient is Green-Gauss (`green_gauss`), so `sum_c V_c grad p_c = sum_b p_b S_b`.
2. **PISO/PIMPLE** (`PimpleSolver::step_with`): the same `sink` is added to the diagonal before `add_ddt`; the hook of an outer
   iteration (`OuterUpdate.sink`) replaces it. The fractional step (`step_fractional`, `pimple/fractional.rs`) does not read the
   sink. The moving-mesh step (`step_ale_with_walls`) installs the moved geometry and calls `step`; `replace_mesh` refuses when a
   sink is set (sized to the mesh). The ADR 0073 lag map does not include the sink.
3. **No forcing towards a non-zero velocity exists**: the sink pulls to zero, and a `BodyForce` is a balanced acceleration (its face
   part enters the flux like a pressure gradient), which is not a penalisation term.
4. **Mesh generators** (`rcfd-mesh/src/gen.rs`): `box_2d_graded` takes one `Grading` per direction; the tensor builder
   `build_masked` with arbitrary coordinates is private. Cells are ordered `i + nx j` (x fastest).
5. **Forces**: `sixdof::patch_force_moment` integrates a wall patch; nothing integrates a force over cells.

## Decisions (fixed now)

**D1. Library.** New module `crates/rcfd-fv/src/immersed.rs`:

- `SolidShape`: `Cylinder { centre, axis, radius }` (infinite circular cylinder; a circle on a 2-D mesh), `Sphere { centre, radius }`,
  `Cuboid { lo, hi }`; a signed distance `phi(x)` per shape (positive outside).
- `ImmersedBody { name, shape, inverted, velocity, omega, origin }`: the solid is `phi < 0` (or `phi > 0` when `inverted`); the
  solid velocity is the rigid field `u_b(x) = velocity + omega x (x - origin)`. The shape does not move (a moving shape is not
  delivered, Q4); a velocity that is not tangent to the surface is not checked.
- `ImmersedSettings { penalty, modified, theta_min }`; `ImmersedSettings::new(penalty)` gives `modified = true`,
  `theta_min = 1e-2`.
- `ImmersedForcing::build(geom, bodies, settings, gamma)` (`gamma` the momentum face diffusivity, `uniform_gamma(geom, nu)` for a
  laminar fluid) returns per cell a coefficient `coeff` (1/s) and a source `source` (m/s^2), the momentum term being
  `-(coeff u - source)` per unit mass, and per body the list of its terms (cell, coefficient, source, point of application).
- `ImmersedForcing::force_moment(body, u, rho, x_ref)`: `F = rho sum_t V_c (coeff_t u_c - source_t)` over the body's terms (the force
  of the fluid on the body, the reaction of the forcing), `M = sum_t (x_t - x_ref) x dF_t`.

**D2. The forcing.** A cell is a solid cell of body `b` when its centre is in `b`. Solid cell `c`: `coeff = K`, `source = K u_b(x_c)`,
point `x_c` (`K = settings.penalty`). With `modified`, for every internal face `f` between a fluid cell `c` and a solid cell `s` of
body `b`: `theta` is the fraction of the segment `x_c -> x_s` at which `phi_b` changes sign (bisection on `phi_b` to round-off),
`theta_e = max(theta, theta_min)`, `x_G = x_c + theta (x_s - x_c)`, `k_f = gamma_f lap_coeff_f`, and the fluid cell gets

    coeff_c += k_f (1/theta_e - 1) / V_c,    source_c += k_f (u_b(x_G)/theta_e - u_b(x_s)) / V_c,   point x_G.

Derivation: the Laplacian already holds `k_f (u_s - u_c)` in row `c`, with `u_s = u_b(x_s)` to `O(1/K)`; the wanted wall flux, with the
no-slip value at `x_G` and the linear profile between `x_G` and `x_c`, is `(k_f/theta)(u_b(x_G) - u_c)`; the difference is the term
above. On an orthogonal mesh with the surface crossing a grid line this is the linear Dirichlet extrapolation of Gibou et al.
Without `modified` only the solid-cell terms exist (the plain staircase forcing).

Build errors (located, `Err`): `penalty` not finite or `<= 0`; `theta_min` outside `(0, 1]`; a non-positive or non-finite radius,
an empty cuboid or a zero axis; a cell centre inside two bodies (names both and the cell); a solid cell with a periodic boundary
face. A body that holds no cell centre is accepted (its force is zero); this is printed by the tests, not refused.

**D3. The solvers.** `SimpleSolver::immersed` and `PimpleSolver::immersed: Option<ImmersedForcing>`, default `None`. When set, per
active component `k` and cell: `diag += coeff V`, `b += source[k] V`, at the place of the existing sink (SIMPLE: before the pseudo
time and the relaxation; PIMPLE: before `add_ddt`); the hook does not change it. Nothing else changes: the pressure equation sees the
forcing through `a_P` and `H`. `PimpleSolver::scratch` (the trial copy) copies it.

**D4. Refusals (PIMPLE).** With `immersed` set: the fractional step stops at step 1 through the divergence guard (stage
`fractional step`, quantity `immersed solid`, the state left as it was, a message naming ADR 0218); `step_ale_with_walls` returns
`Err` naming ADR 0218 before it changes anything; `replace_mesh` refuses as it does for a sink. The block-coupled solver
(`coupled.rs`) has no `immersed` field (not delivered).

**D5. Mesh.** New `rcfd_mesh::gen::box_2d_coords(xs, ys, dz)`: the tensor mesh of `box_2d` (patches `xmin`, `xmax`, `ymin`,
`ymax`, `frontAndBack`) on given strictly increasing coordinates; `Err` for fewer than two coordinates or a non-increasing list.

## Set-ups

**DFG channel.** `[0, 2.2] x [0, 0.41]`, one cell of `dz = 0.01` in z (`frontAndBack` empty); cylinder `Cylinder { centre (0.2,
0.2, 0), axis z, radius 0.05 }`, stationary; `nu = 1e-3`, `rho = 1`. Inlet `xmin`: fixed velocity, per face the exact face average of
`4 U_m y (0.41 - y)/0.41^2`, `v = 0`. Walls `ymin`, `ymax`: velocity zero. Outlet `xmax`: velocity zero gradient, pressure fixed 0
(the FeatFlow set-up has the do-nothing condition; this is a known difference of the set-up, not corrected). `U_bar = 2 U_m/3`;
`C_D = 2 F_x/(rho U_bar^2 D dz)`, `C_L = 2 F_y/(rho U_bar^2 D dz)`, `D = 0.1`, with `F = force_moment(cylinder)`. Penalty
`K = 1e7` 1/s, `modified = true`, `theta_min = 1e-2`.

**Mesh family M1, M2, M3 (2D-1).** M1 coordinates, x: 10 cells on `[0, 0.12]` (geometric, last cell 0.005), 40 uniform cells on
`[0.12, 0.32]`, 38 cells on `[0.32, 2.2]` (geometric, first cell 0.005); y: 10 cells on `[0, 0.12]` (last cell 0.005), 32 uniform
cells on `[0.12, 0.28]`, 10 cells on `[0.28, 0.41]` (first cell 0.005); the geometric ratios from `expansion_for_end_cell`. M2 is M1
with every cell bisected in x and y, M3 is M2 bisected: 88 x 52 = 4 576, 176 x 104 = 18 304 and 352 x 208 = 73 216 cells; core
spacing `D/20`, `D/40`, `D/80`. The lines `x = 0.2` and `y = 0.2` are grid lines and the core is uniform over the cylinder and
one core cell beyond it, so the staircase is mirror-symmetric about both lines through the centre (asserted, S3).

**2D-1 runs.** `U_m = 0.3` (Re 20). SIMPLEC (`SimpleSettings::new(1e-3, Simplec)`: `alpha_u 0.9`, `alpha_p 1`), `LinearUpwind`
convection, momentum solver `tol_abs 1e-20`, pressure solver `tol_abs 1e-20` (relative tolerances and caps as the defaults). M1
starts from the inlet profile in every cell and `p = 0`; M2 and M3 start from the converged state of the mesh before, injected
(each cell takes the value of its parent cell). Convergence: after every 10 iterations `C_D` and `C_L` are evaluated; the run stops
at the first evaluation `k >= 200` with `abs(C_D(k) - C_D(k-200)) <= 1e-7 C_D` and `abs(C_L(k) - C_L(k-200)) <= 1e-8`; caps 4000
(M1), 4000 (M2), 6000 (M3) iterations.

**T mesh and the 2D-2 run.** x: 240 uniform cells on `[0, 0.6]` (`h = 0.0025`), 84 cells on `[0.6, 2.2]` (geometric, first cell
0.0025); y: 164 uniform cells on `[0, 0.41]`; 324 x 164 = 53 136 cells, `D/40` around the cylinder, symmetric about `x = 0.2` and
`y = 0.2` over the cylinder. `U_m = 1.5` (Re 100). `PimpleSettings::piso(1e-3)` with BDF2, `LinearUpwind`, AMG pressure, two
correctors, momentum `tol_rel 1e-6`, pressure `tol_rel 1e-6` (absolute tolerances and caps as `piso`), `dt = 0.001`, to
`t = 8`. Initial state: `u = (u_in(y), 0.2 U_bar exp(-((x - 0.35)^2 + (y - 0.2)^2)/0.05^2), 0)`, `p = 0` (the inlet profile with an
asymmetric blob behind the cylinder to start the shedding early). `C_L(t)` and `C_D(t)` after every step. Window `t in [5.5, 8]`:
`St = f D/U_bar` with `f` from the upward crossings of `C_L - mean_window(C_L)` (linear interpolation), `f = (n - 1)/(t_n - t_1)`
over all `n` crossings in the window.

**CC (circular Couette).** `box_2d(N, N, 2.4, 2.4)` (`dz = 2.4/N`), centre `c = (1.2, 1.2)`; inner body `Cylinder { c, z, 0.5 }`
rotating, `omega = (0, 0, 1)` about `c`; outer body `Cylinder { c, z, 1.0 }`, `inverted`, stationary; every box wall velocity zero;
`nu = 1`, `rho = 1`, `K = 1e8`, `theta_min = 1e-2`; `N = 24, 48, 96` (`h = 0.1, 0.05, 0.025`). Exact: `u_theta = A r + B/r`,
`A = -omega R1^2/(R2^2 - R1^2)`, `B = omega R1^2 R2^2/(R2^2 - R1^2)`; torque on the inner cylinder per unit depth
`4 pi rho nu omega R1^2 R2^2/(R2^2 - R1^2) = 4.18879`. SIMPLEC with central convection, momentum and pressure `tol_abs 1e-20`, from
rest, until `max_du <= 1e-10` (the `IterationInfo` value) or 20 000 iterations. Error: `e = sqrt(sum_F V |u - u_exact|^2 / sum_F V) /
(omega R1)` over the fluid cells `F` (centre with `0.5 < r < 1.0`).

## Registered checks

All in the new test file `crates/rcfd-fv/tests/immersed_solid_f219.rs`, release (the heavy ones carry
`#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]`). "Within 5 %" is `abs(a - ref)/abs(ref) <= 0.05`.

### A, the acceptance

| Id | Check | Limit |
| --- | --- | --- |
| A1 | 2D-1 on M3: `C_D` | within 5 % of 5.57953523384 |
| A2 | 2D-1 on M3: `C_L` | within 5 % of 0.010618948146 |
| A3 | 2D-2 on T: `St` | within 5 % of 0.30188 |

Printed, no limit: `C_D`, `C_L` on M1 and M2 and the observed orders `log2(abs(C(M1) - C(M2))/abs(C(M2) - C(M3)))`; the pressure
difference (cell values of the fluid cells next to `(0.15, 0.2)` and `(0.25, 0.2)`, mean of the two rows at `y = 0.2 -+ h/2`); the
recirculation length (the last sign change of `u_x` on the two rows behind the cylinder, minus 0.25); the iteration counts and
times; the 2D-2 extremes, mean `C_D`, `1/f`; the 2D-1 forces on M2 with `modified = false` (the plain forcing).

### S, set-up checks (a miss is a defective set-up)

| Id | Check | Limit |
| --- | --- | --- |
| S1 | each 2D-1 run (M1, M2, M3) | the convergence rule met within its cap |
| S2 | the 2D-2 window | at least 6 upward crossings; every period within 1 % of the mean period; the maximum of `C_L` in every period within 2 % of their mean; `max C_L - min C_L >= 1.0` |
| S3 | M1, M2, M3, T | the cell counts above; the solid cells mirror-symmetric about `x = 0.2` and about `y = 0.2` (each solid cell's mirror image is a solid cell) |
| S4 | CC runs (both forcings, three meshes) | `max_du <= 1e-10` within 20 000 iterations |

### E, the forcing holds the solid

| Id | Check | Limit |
| --- | --- | --- |
| E1 | 2D-1 on M1 and on M3, T at `t = 8`: `max |u|` over the solid cells | `<= 1e-3 U_bar` |

### V, verification

| Id | Check | Limit |
| --- | --- | --- |
| V1 | CC with `modified = true`: `e` at `N = 96`, and the observed order `log2(e(48)/e(96))` | `e(96) <= 1e-2`; order `>= 1.5` |
| V1n (negative control of V1) | CC with `modified = false` | `e(96) >= 2 e_modified(96)` |
| V2 | 2D-1 on M1, converged: the force of the forcing against the momentum balance of the domain `F_bal = -rho sum_b [phi_b u_b + p_b S_b - gamma_b lap_coeff_b (u_b - u_c)]` over the non-empty boundary faces (`u_b`, `p_b` the boundary values the solver uses: velocity condition, pressure condition of `pressure_bc`; the diffusive term on fixed-velocity faces only) | each of x and y: `abs(F - F_bal) <= 1e-3 abs(F_x)` |
| V2n (negative control of V2) | the same with the modification terms left out of `F` | x: `abs(F' - F_bal) >= 1e-2 abs(F_x)` |
| V3 | CC at `N = 48` with PISO (`piso(1)`, Euler, central, `dt = 0.0025`, 400 steps, from rest), `modified = true`: set-up `max |u^{n+1} - u^n| <= 1e-9` at the last step; `e` | `e <= 1.5 e_SIMPLEC(48)` |
| G1 | `phi` of each shape at 20 points against the closed form; `theta` of 20 segments across a circle against the root of the quadratic | `<= 1e-13`; `<= 1e-12` |
| B1 | each build error of D2 | `Err` whose message names the setting (or the cell and both bodies) |

Printed, no limit: the inner and outer torques of CC against 4.18879; `e` of every mesh; the number of solid and interface cells.

### R and N, refusals and defaults

| Id | Check | Limit |
| --- | --- | --- |
| R1 | PISO with `immersed` set and `fractional` on a 2-D box: `step` once | stopped at step 1, stage `fractional step`, quantity `immersed solid`; `u`, `p`, `phi` bit-identical to before; the message contains `ADR 0218` |
| R2 | `step_ale_with_walls` with `immersed` set | `Err` containing `ADR 0218`; `u`, `p`, `phi`, the geometry and the time unchanged |
| R3 | `replace_mesh` with `immersed` set | `Err` |
| N1 | `SimpleSolver::new` and `PimpleSolver::new` | `immersed` is `None` |
| N2 | the gate: the existing suites unedited (all of them, in particular `rcfd-api` `baseline_hashes`, H1 of `case_hashes_w48u`, `rcfd-fv` `w33b_default_bits`, `cylinder`, `solidification`, `piso_thin_gap_f299`) | pass |

### Mutations (scratch, not committed)

Each applied alone to the final tree by an exact text replacement (a script that checks that the replaced text occurs once), the
named tests run once in release in a separate target directory, the file restored and compared byte for byte with a kept copy.
`scripts/limit_audit.py` is run on the new test file and every hit is read.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | the modification terms not added (`modified` read as false in `build`) | V1 |
| MU2 | `theta` replaced by `1 - theta` | V1 |
| MU3 | the solid-cell terms not added | E1 (M1) |
| MU4 | SIMPLE: the source not added to `b` | V1 |
| MU5 | PIMPLE: the source not added to `b` | V3 |
| MU6 | PIMPLE: the coefficient not added to the diagonal | V3 |
| MU7 | `force_moment` skips the modification terms | V2 |
| MU8 | the fractional-step refusal removed | R1 |

Expected: MU1 gives the plain forcing (V1n's numbers, so order about 1 and V1's error limit or order missed); MU2 moves the wall by
up to a cell; MU3 lets the flow through the cylinder; MU4 and MU5 stop the inner cylinder (error of order 1); MU6 removes the
forcing from PISO; MU7 drops the modification's share of the drag (V2n says how much); MU8 runs the fractional step.

### The gate

`cargo fmt` per package, `cargo clippy --workspace --all-targets -- -D warnings`, and every test in release on a `git archive` copy
of the gated commit, one process per test, three jobs (`scripts/gate_test.py`), through the machine's gate lock (the 5 s poll of
`gate-f291/outer2.sh`), own target directory, started with at least 19 GB of physical memory free (free memory was 25.5 GB of 93.6
before this text; the threshold is below ADR 0211's 20 GB because the machine holds about 18 to 25 GB free while other agents
build). A failure of a known flaky test (earlier gate lines: `runner_w47b`, `journal_f273` c3s, preview g8/g10, the `rcfd-runner`
s1/s9 timing tests) is run again alone on the same build and reported with its name.

### Classes

A miss of A1 to A3 is a miss of the acceptance: reported with its number, kept as an ignored `*_known_miss` test with its limit,
diagnosed in the addendum. A set-up check (S1 to S4) or a negative control (V1n, V2n) that does not hold, or a mutation that does
not make its tests fail, means a check is not shown to be able to fail: reported as a defective set-up, corrected by an amendment
before any re-run, and never counted as a pass. A miss of E1, V1 to V3, G1, B1, R1 to R3 or N1 to N2 is a miss of the
implementation: ignored known miss, limit intact.

**Decision rule.** A1 to A3 met with S1 to S4 held, and E1, V1 to V3, G1, B1, R1 to R3, N1 to N2 met, V1n and V2n held, and every
mutation failed its tests: F-219 is set **done** (`[x]`) in the roadmap on the acceptance as read here (Q1), the scope limits named
under "Not delivered". Every acceptance check met but another check missed: **partial** (`[~]`), the miss named. A miss of A1 to A3:
**partial** if the library and the V checks are met (the miss named and kept as a known miss), else todo stays.

## Not delivered

A case-schema key or a Studio control for immersed solids (it would change the schema goldens; Q2); moving or deforming shapes
(Q4); the block-coupled solver, the fractional step and moving meshes (refused or absent); turbulence-model wall treatment at the
immersed surface (the modification uses the given face diffusivity; no wall function); energy and scalar equations (the forcing is
momentum only; a conjugate solid is F-051's conforming zones); 3-D validation (the sphere is implemented and checked by G1 only);
the do-nothing outlet of the DFG set-up; the 2D-3 case; the lift and drag via a surface integral on the immersed boundary.

## Open questions and choices for the maintainer

- **Q1. The reading of the acceptance.** 2D-1 has no Strouhal number; the Strouhal number is taken from 2D-2. Options: (a) accept
  this reading; (b) reword the acceptance to "DFG 2D-1 drag and lift and DFG 2D-2 Strouhal number within 5 percent". Recommendation
  (b), which states what is checked.
- **Q2. A case key.** `immersed_solids` as an optional table (shape, inverted, velocity, omega, origin, penalty, modified) would be
  `hash_omit_default`-safe, but the schema goldens change. Recommendation: a follow-up item with the golden edit approved.
- **Q3. The penalty.** `K` is absolute (1/s). Options: (a) keep it absolute; (b) scale it with the largest momentum diagonal per unit
  volume. Recommendation (a) with a validation that warns when `K` is below 1e3 times the largest `a_P/V` of the fluid cells, in the
  follow-up of Q2.
- **Q4. Moving shapes.** The forcing is built once; a moving body needs a rebuild per step and a treatment of the cells that change
  side (fresh cells). Recommendation: a separate item with its own registration, checked on an oscillating cylinder.

## Addendum 1: what was built and the results (2026-11-07)

Measured against the limits registered above; no limit was changed and no amendment was needed. Release builds on Windows. The
registration is 904f2da; the library and the registered tests are c6e9302, on which every registered run was made once; this
addendum's commit splits the V1 test into its met part and its known miss (no limit changed), adds one diagnostic test (printed
only, below) and carries the documents. The runs are deterministic (the M1 state hash `8d13ea8c24b75cd0` is the same in the V2 test
and in the A1/A2 chain) and none was repeated to change a result.

**Built.** `crates/rcfd-fv/src/immersed.rs`: `SolidShape` (cylinder, sphere, cuboid with signed distances), `ImmersedBody` (inverted
solids, rigid velocity), `ImmersedSettings`, `ImmersedForcing::build` (solid-cell penalisation, the modification terms of D2, the
located build errors), `force_moment`, `solid_mask`. `SimpleSolver::immersed` and `PimpleSolver::immersed` (default `None`; two
added statements each, behind `if let Some`), the trial copy, the refusals of D4 (the fractional step through the guard with its
own message, `step_ale_with_walls` and `replace_mesh` by `Err`; the existing `replace_mesh` message is unchanged).
`rcfd_mesh::gen::box_2d_coords`. `crates/rcfd-fv/tests/immersed_solid_f219.rs`: the registered checks, the ignored known miss of V1
and one ignored diagnostic. No case-schema key, golden, `baseline_hashes.rs`, `case_hashes_w48u.rs` or existing test file was
touched. Documents: `docs/reference-data.md` (the sources read), the F-219 entries of `docs/ROADMAP.md` and `docs/roadmap.json`
(partial, by the decision rule).

### Results

| Check | Limit | Measured | Verdict |
| --- | --- | --- | --- |
| A1 2D-1 `C_D` on M3 | within 5 % of 5.57953523384 | 5.57833680 (`-0.0215 %`) | met |
| A2 2D-1 `C_L` on M3 | within 5 % of 0.010618948146 | 0.0106899517 (`+0.669 %`) | met |
| A3 2D-2 `St` on T | within 5 % of 0.30188 | 0.30073 (`-0.382 %`), `1/f` 0.33253 s | met |
| S1 convergence rule | met within the caps 4000, 4000, 6000 | M1 470, M2 750, M3 2190 iterations (last `max_du` 4.8e-15, 5.1e-11, 1.4e-10) | held |
| S2 the 2D-2 window `[5.5, 8]` | `>= 6` crossings; periods within 1 %; maxima within 2 %; amplitude `>= 1` | 7 crossings; 5.5e-6; 2.1e-4; 2.058 | held |
| S3 cell counts, mirror symmetry | as registered | 4 576, 18 304, 73 216, 53 136; symmetric on all four meshes (316, 1 264, 5 024, 1 264 solid cells) | held |
| S4 CC convergence | `max_du <= 1e-10` within 20 000 | 105 to 353 iterations (all six runs and V3's) | held |
| E1 solid velocity | `<= 1e-3 U_bar` | M1 `7.6e-6`, M3 `2.7e-5`, T `3.8e-5` (in units of `U_bar`) | met |
| V1 CC, modified: `e(96)` | `<= 1e-2` | `1.576e-3` (`e` 8.293e-3, 3.202e-3, 1.576e-3 at N 24, 48, 96) | met |
| V1 CC, modified: observed order 48 to 96 | `>= 1.5` | **1.023** | **missed** (ignored known miss `v1_circular_couette_order_known_miss`) |
| V1n (negative control) plain forcing | `e(96) >= 2 e_mod(96)` | `1.357e-2`, 8.6 times (5.43e-2, 2.98e-2, 1.36e-2; order 1.14) | held |
| V2 momentum balance, M1 | `<= 1e-3 abs(F_x)` per component | x `1.8e-13`, y `2.2e-14` | met |
| V2n (negative control) | `>= 1e-2 abs(F_x)` | `0.161` (the modification carries 16 % of the M1 drag) | held |
| V3 CC with PISO, N = 48 | set-up change `<= 1e-9`; `e <= 1.5 e_SIMPLEC(48)` | `6.9e-13`; `3.195e-3` against `3.202e-3` | met |
| G1 level sets; crossings | `<= 1e-13`; `<= 1e-12` | `2.2e-16`; `2.2e-16` | met |
| B1 build errors | located `Err` | every case (penalty 0, NaN, -1; `theta_min` 0, 1.5, NaN; radius 0, -1, inf; zero axis; empty cuboid; a cell in two bodies; a solid cell on a periodic face; a short `gamma`) | met |
| R1 fractional step | stopped at step 1, stage, quantity, state bit-identical, `ADR 0218` | as required | met |
| R2, R3 moving mesh | `Err` naming ADR 0218, state, geometry and time unchanged; `Err` | as required | met |
| N1 defaults | `None` | `None`, `None` | met |
| N2 the gate | pass | see the gate section | |

Printed, no limit. 2D-1 with the modified forcing: `C_D` 5.52887, 5.57022, 5.57834 (M1 to M3, observed order 2.35), `C_L`
0.0117566, 0.0109084, 0.0106900 (order 1.96); the pressure difference 0.11185, 0.11491, 0.11638 (FeatFlow 0.11752; from the cell
values next to the two points, so half a cell away from them) and the recirculation length 0.0834, 0.0843, 0.0846. With the plain
forcing: M1 `C_D` 5.32268 (`-4.60 %`), `C_L` 0.0117742 (`+10.9 %`); M2 5.47051 (`-1.95 %`), 0.0111246 (`+4.76 %`), against the
modified 5.57022 (`-0.17 %`) and 0.0109084 (`+2.73 %`) on M2. 2D-2 (FeatFlow level 6 in brackets): max `C_D` 3.2462 (3.2200), min
3.1805 (3.1569), mean 3.2135 (3.1884), max `C_L` 1.0113 (0.9859), min -1.0469 (-1.0206): all within 2.6 %. CC torque on the inner
cylinder -4.07749, -4.14059, -4.17006 against -4.18879. The outer body's sums (3.37, 3.58, 3.62) are not the torque of the fluid:
the outer solid touches the box walls, and its forcing also carries the reaction of the wall pressure; `force_moment` is the force
of everything on the solid, so a body that touches the domain boundary does not give the fluid force (Q5). Run times: M3 972 s
(the chain 1055 s), the 2D-2 run 1729 s, the CC series 17 s, each with two other registered runs on the machine.

**The V1 miss (diagnosed, not removed).** The modification makes the CC error 8.6 times smaller than the plain forcing at N = 96 but
leaves it first order (1.37 from 24 to 48, 1.02 from 48 to 96). A diagnostic after the registered runs (the ignored
`diag_v1_circular_couette_error_split`, printed only, no limit; N = 48, 96 and a fourth mesh, 192) splits the error: radial part
2.47e-3, 1.17e-3, 3.97e-4 and azimuthal part 2.04e-3, 1.05e-3, 4.34e-4 (the exact radial velocity is zero); cells within `2 h` of a
surface 4.33e-3, 2.85e-3, 1.32e-3, the other cells 2.16e-3, 1.05e-3, 4.37e-4. So the error is spread over the annulus, its order
wanders between 1.0 and 1.4 with the mesh (from 96 to 192 it is 1.42), and a radial velocity of the size of the azimuthal error
appears. Reading (a hypothesis, not checked): the modification corrects the viscous wall flux, which is the only place it acts; the
mass flux through the faces between fluid and solid cells is still the Rhie-Chow interpolation of `HbyA` across the surface (the
solid side carrying the rigid velocity of the solid cell centre), so continuity sees the staircase at first order and the pressure
correction puts a first-order normal velocity into the fluid. A face flux reconstructed with the same crossing would be the next
step (Q6). The acceptance quantities converge faster on the DFG family (orders 2.35 and 1.96), where the staircase is
mirror-symmetric about the cylinder centre.

### Mutations

Applied one at a time to the library of c6e9302 (with this commit's test split) by `w49i-work/f219-scratch/mutate.py` (an exact
text replacement that checks that the text occurs once), the named test run once in release in `target-f219-mut`, the file
restored and compared byte for byte with the kept copy (equal after each of the eight).

| Id | Must fail | Result |
| --- | --- | --- |
| MU1 modification not added | V1 | failed: `e(96)` 1.357e-2 (the plain forcing's number), V1n ratio 1.000 |
| MU2 `theta -> 1 - theta` | V1 | failed: `e(96)` 2.379e-2 |
| MU3 solid-cell terms not added | E1 (M1) | failed: 1.12 `U_bar` in the solid |
| MU4 SIMPLE source not added | V1 | failed: `e(96)` 0.457 (the inner cylinder at rest) |
| MU5 PIMPLE source not added | V3 | failed: `e` 0.455 |
| MU6 PIMPLE coefficient not added | V3 | failed: the run stopped by the divergence guard |
| MU7 `force_moment` without the modification | V2 | failed: `1.91e-1` |
| MU8 fractional-step refusal removed | R1 | failed: the step was not stopped |

`scripts/limit_audit.py` (workspace scan) reports no hit in the new test file.

### Disclosures

- Before the registration only the timing probe named there was run; between the registration and the runs only the build, clippy
  and fmt. The diagnostic of the V1 miss was written and run after all registered runs and has no limit.
- The convergence rule of the 2D-1 runs compares with the value 200 iterations earlier; the coefficients of the initial state
  (iteration 0) are recorded, so the first comparison is at iteration 200 as registered.
- The V1 test as registered asserted the error limit, then the order, then V1n; the order assertion stopped it after V1n's numbers
  were printed (ratio 8.609). The split tests assert the same limits.
- Not exercised: the sphere and the cuboid in a flow (G1 only), BDF2 with the forcing on a mesh other than T, a non-orthogonal mesh,
  a moving or deforming body, turbulence.

Status by the decision rule: A1 to A3 met with S1 to S4 held; E1, V2, V3, G1, B1, R1 to R3 and N1 met (N2 is the gate); V1n and V2n
held; every mutation failed its test; V1 missed its order limit. F-219 is set **partial** (`[~]`): the acceptance is met as read
(Q1), and the immersed forcing is first order (V1, known miss). Q1 to Q6 are open.

- **Q5 (added). The force of a body that touches the domain boundary.** `force_moment` sums the forcing, which for such a body also
  holds the reaction of the boundary. Options: (a) document it (done: the documentation of `force_moment`, a comment-only change after the mutations); (b) subtract the boundary-face
  terms of the solid cells. Recommendation (a) until a case needs it.
- **Q6 (added). The order of the immersed forcing.** Options: (a) a follow-up item that reconstructs the face flux between fluid and
  solid cells with the crossing, registered with V1 again as its check; (b) accept first order. Recommendation (a).

### The gate (2026-11-07)

The gate ran on a `git archive` copy of 2f9470b (addendum 1's commit) with three jobs and one process per test
(`scripts/gate_test.py`), in the target dir `E:/projects/cfd/w49i-work/target-f219-gate`, through the machine's gate lock (the 5 s
poll of `gate-f291/outer2.sh`; queued at 18:27:27 UTC behind f250, taken at 20:19:47, released at 22:07:16). It started with 21 GB
of physical memory free (registered threshold 19 GB); the lowest sample during the run was 17 GB (20:59), below the threshold, with
other agents building. `cargo fmt` per package 0, `cargo clippy --workspace --all-targets -- -D warnings` 0, tests **3188 passed, 2
failed, 894 ignored** (3190 test processes in 608 binaries, wall 5630 s, peak summed working set 3373 MB, largest time against its
limit 0.465, which is `a3_two_d_two_strouhal_number` at 1674 s; `a1_a2_two_d_one_drag_and_lift_on_m3` took 900 s).

The two failures:

- `rcfd-mcp` `adopt_f288::a_n_a_run_that_differs_in_one_item_is_not_adopted`: `Access is denied` on the replace of a `run.json`
  after 12 attempts (a file held by another process). Run again alone on the same build three times: passed three times. A flaky
  file-replace test, not in the earlier gate lines; not related to F-219.
- `rcfd-api` `f294_transient_keps_sa::s1_scope_of_transient_kepsilon_and_sa`: expects `turbulence.periodic_seam.diffusivity =
  "pair"` with k-epsilon on PIMPLE to be refused (ADR 0210 S1), and gets no diagnostic. Run again alone on the same build: fails the
  same way. Run on a `git archive` of main 981804b without F-219 (target `target-f219-chkmain`): fails the same way. It is an
  interaction of two merges on main (F-294's S1 written before F-296, ADR 0212, made `pair` valid for k-epsilon and SA), not an
  effect of F-219 (the test sets no immersed forcing, so none of F-219's statements run in it). The test file is not edited here (Q7).

Every test of `immersed_solid_f219.rs` that is not ignored passed (11); the known miss and the diagnostic were ignored. The suites
named under N2 passed unedited (`baseline_hashes`, H1 of `case_hashes_w48u`, `w33b_default_bits`, `cylinder`, `solidification`,
`piso_thin_gap_f299`). So N2 is met for every suite F-219 can reach; the one deterministic failure is the base's. This gate section
is a document only and was not gated again.

- **Q7 (added). The F-294 scope test on main.** `s1_scope_of_transient_kepsilon_and_sa` fails on main 981804b since the F-296 merge.
  Options: (a) the owner of F-294 updates its S1 case for `pair` (now accepted, ADR 0212) by an amendment of ADR 0210; (b) leave it
  failing. Recommendation (a), before the next merge is gated.
