# ADR 0154: Job 48T, pressure spokes at the wall of the refined cylinder O-grid (investigation)

Status: **registered** 2026-10-28, before any solver run of this job and before any metric below has been computed on any
stored field (set-ups, metrics, limits and reading rules are fixed here); results in the addendum; not yet accepted by the
maintainer. House rules of [ADR 0130](0130-wave48a-overview-and-run-record-contract.md): limits are fixed before the registered
runs; a missed limit becomes an ignored `*_known_miss` test with the limit intact; a defective set-up gets an amendment written
before the corrected run; defaults of existing code stay bit-identical; `crates/rcfd-api/tests/baseline_hashes.rs` is not
re-recorded; no unsafe code, no new crate, the crate graph stays acyclic. **This job changes no solver numerics**: it is an
investigation; a fix is a recommendation to the maintainer and needs the maintainer's decision first. Clean-room: nothing of
any vendor's software is decompiled or read. Parents: [ADR 0142](0142-wave48h-example-projects.md) (the example project),
[ADR 0132](0132-wave48c-viewport-integration.md) (the viewport), [ADR 0069](0069-wave33b-transient-accuracy-coupling.md) and
[ADR 0073](0073-wave34b-transient-floor-pimple-lag.md) (PISO/PIMPLE coupling analysis). Related: job 48R
(ADR 0152 `0152-job48r-store-key-at-its-default-is-not-the-case-s-own.md`, on its own branch: explicit store keys at their
defaults; not re-investigated here).

## The report

The user, in the real Studio window on `example-projects/wind-tunnel`, case `cases/cylinder-re100.toml` (`cylinder_ogrid`,
r_in 0.5, r_out 15, grading 100, laminar PISO, `linear_upwind`, nu 0.01, U = 1 m/s, impulsive start from uniform flow plus a
sideways push): after raising `mesh.nx`, `mesh.ny` to 256 x 256 "it doesn't look correct at all"; after lowering `solver.dt` to
0.0156 (Courant number fixed) "there are still incredibly weird artifacts around the cylinder". The screenshots show p at
t about 3 (steps 193 to 205, 65,536 cells): bright and dark radial spokes of alternating sign from the wall into the near field,
circumferential wavelength one to a few cells, the rest of the domain flat blue; residuals flat (u about 2e-1, p about 9e-1 at
step 205); noisy cl and cd. An earlier run at dt 0.0625 on the same mesh looked similar but noisier. The committed 64 x 48 case
does not show it (the 48H agent's checks).

## Context (read at main 6867e40, orientation only)

### The case and the user's working copy

* The user's working copy of the case (main checkout, uncommitted) was saved by the Studio editor and lists every schema
  setting (`git diff --stat`: 886 insertions, 33 deletions). It contains for example `[fluid] mode = "compressible"`,
  `viscosity = 0.000018`, `[mesh] first_layer_height = 0.0`, `wall_layers = 0`, `[solver] alpha_u = 0.9`, `ddt_correction = true`.
* **P0 (orientation, a parse, no solver run).** Both files, copied to the work folder `E:\projects\cfd\w48t-work` (the user's
  files are not touched), through `rcfd run <case> --print-settings` (the resolved settings tree, defaults filled; built from
  this branch): 741 keys each; they differ in exactly three keys, `mesh.nx` 64 -> 256, `mesh.ny` 48 -> 256, `solver.dt`
  0.0625 -> 0.0156, the user's intended edits. `fluid.mode = "compressible"` is the schema default (it is in both trees) and its
  schema text says "ideal_gas only"; `fluid.density = "constant"` in both, so it selects nothing. `rcfd check --json` on both:
  exit 0, no diagnostics, no rule. The behavioural check of this is S0 below.
* `mesh.first_layer_height` and `mesh.wall_layers` apply to `box_2d` and `channel_2d` only (`check_mesh_options` in
  `crates/rcfd-api/src/sim.rs`); for `cylinder_ogrid` the first radial cell is set by `mesh.grading` alone
  (`Grading::Expansion`, last cell / first cell, `crates/rcfd-mesh/src/gen.rs`).
* `pressure_update = "bernoulli"` applies to `target_mass_flow_outlet` only (`bc_cat.rs`). At the cylinder (velocity fixed) the
  pressure condition is forced to zero gradient whatever the case says (`pressure_bc`, `crates/rcfd-fv/src/pressure.rs`). The
  far field is `Freestream` (velocity fixed on faces with n.U_ref < 0, pressure fixed on the others).

### The display path

* The Studio viewport (`crates/rcfd-studio/src/viewport.rs`): `StoreView::sample` reads the snapshot's cell values,
  `face_values_from_cells` gives every drawn face its owner cell's value, `Field::Face` colours each face flat. A 2D case is
  drawn as the `Empty`-patch faces facing +z (`surface_of`). The colour range defaults to `RangeMode::Snapshot`, the min and
  max of that snapshot's face values. The "linear" toggle interpolates **in time** between two snapshots (`Mode::Linear` of
  `Reader::field_at_time`); there is no spatial interpolation in this path. `rcfd-mcp render_field` draws the same cell values
  with the CPU renderer `rcfd_post::render::render_scalar`.
* Consequence (prediction, checked by D1): what the Studio shows is the stored cell field; a few extreme cells set the range and
  everything else looks flat.

### The solver path

* `solver.algorithm = "piso"` builds `PimpleSettings::piso(nu)` (`crates/rcfd-fv/src/pimple.rs`): n_outer 1, n_correctors 2,
  non-orthogonal correctors 0, `ddt_correction` on (a Choi-type carry-over `phiHbyA += gamma_p/dt * (phi_old - (U_old)_f . S)`,
  unlimited), implicit Euler, pressure CG + AMG (`amg_pressure` when cells > 2000; tol_rel 1e-7, tol_abs 1e-14, max 3000),
  momentum BiCGStab + ILU0 (tol_rel 1e-8, max 500), Green-Gauss pressure gradient, least-squares gradient for `linear_upwind`
  (deferred correction). With n_outer = 1, `alpha_u` and `alpha_p` are not used.
* The repository's own validated cylinder (F-029, `crates/rcfd-fv/tests/cylinder.rs`, 128 x 100, grading 150, dt 0.05) runs
  PIMPLE: n_outer 2, `alpha_u = alpha_p = 0.9`, 2 correctors.
* ADR 0073 R5: with one corrector the lagged pressure error is multiplied by about `-D` per step (`D` the diagonal-part diffusion
  number); "a second corrector ... returns zero lag error" in its constant-coefficient model. That model does not cover a
  graded, anisotropic wall layer.

### Mesh numbers (computed from `geometric()` of `gen.rs`; first radial cell h1, chord s at the first cell centre)

| Mesh, dt | h1 | in-plane aspect s/h1 | nu dt/h1^2 | D_w (wall cell, diagonal part) | rAU/dt (wall cell, no convection) | Co_theta (2U dt/s) |
|---|---|---|---|---|---|---|
| 64 x 48, 1/16 (committed) | 0.01366 | 3.64 | 3.35 | 3.73 | 0.088 | 2.51 |
| 128 x 96, 1/32 | 0.00693 | 3.57 | 6.51 | 7.42 | 0.047 | 2.53 |
| 256 x 256, 1/64 | 0.00262 | 4.69 | 22.8 | 24.7 | 0.014 | 2.54 |
| 256 x 256, 0.0156 (user) | 0.00262 | 4.69 | 22.7 | 24.6 | 0.014 | 2.54 |
| 256 x 256, 1/16 (user, earlier) | 0.00262 | 4.69 | 91.0 | 98.7 | 0.0036 | 10.2 |
| 256 x 256, 1/256 | 0.00262 | 4.69 | 5.69 | 6.17 | 0.054 | 0.63 |
| 256 x 48, 1/64 | 0.01366 | 0.91 | 0.84 | 2.83 | 0.18 | 2.51 |
| 256 x 48, 1/16 | 0.01366 | 0.91 | 3.35 | 11.3 | 0.053 | 10.1 |
| 64 x 256, 1/16 | 0.00262 | 18.8 | 91.0 | 90.9 | 0.0037 | 2.54 |
| 64 x 256, 1/64 | 0.00262 | 18.8 | 22.8 | 22.7 | 0.0145 | 0.64 |
| 256 x 256 grading 10.88, 1/64 | 0.01366 | 0.91 | 0.84 | 2.86 | 0.18 | 2.51 |
| 512 x 512, 1/16 (user) | 0.00131 | 4.68 | 362 | 394 | 0.0009 | 20.4 |

Grading 10.88 restores the 64 x 48 first cell (0.013657) at ny = 256. `rcfd check`: non-orthogonality about 1e-11 degrees,
skewness 0.006 (256 x 256) and 0.024 (64 x 48); the reported max aspect ratios 38.2 and 14.7 are depth over an in-plane edge
(out of plane), not the in-plane wall aspect above. Matching the Courant number does **not** match the wall diffusion number:
it grows like 1/h at a Courant-matched dt.

### The user's runs (main checkout, untracked; read-only, copied for analysis)

| Id | Mesh | dt | State | Stored snapshots |
|---|---|---|---|---|
| U1 `r-20261028T183226-3c48` | 64 x 48 (committed case) | 0.0625 | finished, t = 80 | t = 1 to 80 |
| U2 `r-20261028T214359-9e9c` | 256 x 256 | 0.0625 | diverged at step 1148 (t = 71.69, Courant 1.93e6) | t = 1 to 71, 71.69 |
| U3 `r-20261028T215105-72b8` | 256 x 256 | 0.0156 | cancelled at step 615 (t = 9.59) | store without index (scanned) |
| U4 `r-20261028T214950-cfc6` | 256 x 256 | 0.0625 | cancelled at step 26 | t = 1 |
| U5 `r-20261028T214139-7184` | 512 x 512 | 0.0625 | diverged at step 22 (Courant 3.11e6) | t = 1 |

**Postdiction (data seen before this registration).** From `residuals.jsonl`: U1 at t = 3: cd 1.474, cl 0.247, initial p
residual 0.0118, u 0.0083, smooth from step to step. U3: cd 16.9, 18.8, -6.18 at steps 191-193 (t about 3), cd 8.41 and 0.106
at steps 601-602; initial p residual 0.96 throughout; already at steps 2-3 cd -26.7 then 14.7. A physical transient would not
make the force jump by tens of dynamic pressures between steps of 0.0156 at t = 3 while the coarse meshes stay smooth; the force
trace already contradicts "benign transient" (a). The registered falsifier of (a) is kept anyway (F-a below).

## Hypotheses

* **H-disp**: the pattern is drawn by the display (Studio or render core), not stored by the solver.
* **H-phys**: a physical start-up transient, resolved only on the fine mesh, that decays.
* **H-Dw**: PISO with one outer iteration loses the pressure-velocity coupling in the wall layer when the wall diffusion number
  `D_w` is large; the error is grid-scale in theta (odd-even) because the theta faces are the weakly coupled ones. Removed by a
  smaller dt (smaller `D_w`), a coarser first cell (grading), outer iterations, or (perhaps) more correctors.
* **H-Co**: driven by the Courant number instead.
* **H-Choi**: the unlimited transient Rhie-Chow carry-over (`ddt_correction`) feeds the odd-even mode.
* **H-lin**: unconverged linear solves (pressure CG/AMG, momentum BiCGStab/ILU0) leave the mode.
* **H-conv**: unbounded `linear_upwind` dispersion.
* **H-start**: the impulsive start (or the push) excites the mode.
* **H-nonorth**: non-orthogonal correction; predicted null (the mesh is orthogonal to 1e-11).

## Metrics (fixed here; none has been computed on any field before this registration)

Cells are grouped into rings by their centre radius (equal to 1e-9 relative within a ring of the O-grid) and ordered by angle in
each ring; `q = 0.5 rho U^2 = 0.5` (kinematic pressure, rho = 1, U = 1). For a ring j with `n_theta` cells and values `p_j(i)`,
`P_j(k)` is its DFT.

* **M_hf** (primary): RMS over the cells of the band rings (centre radius in [0.55, 0.70]) of the high-pass part of `p_j`, the
  inverse DFT of the modes `n_theta/8 <= k <= n_theta/2` (wavelength <= 8 cells), divided by q.
* **A_N** (secondary): the Nyquist (2-cell) amplitude `|(1/n_theta) sum_i p_j(i) (-1)^i| / q`; reported as the band mean, the
  value at the first (wall) ring, and the maximum over rings with centre radius <= 0.70.
* **M_2** (secondary): RMS over the band of the circumferential second difference `p_j(i+1) - 2 p_j(i) + p_j(i-1)`, divided by q.
* **lambda***: wavelength in cells `n_theta/k*` of the band-averaged `|P_j(k)|` maximum over `k >= n_theta/8`; also in units of D
  at r = 0.6.
* **Physical signal**: RMS of the low-pass part (`1 <= k < n_theta/8`) over the band, divided by q (printed).
* **Ring profile**: A_N and the high-pass RMS per ring from the wall to r = 1.5 (printed, figure).
* **Screen** (from the status stream of every run): RMS of the step-to-step change of cd over the steps with t in [2, 3], and
  the median initial p residual over the same steps; wall time per simulated second.

## Limits and reading rules

* **S0 (settings, behaviour).** The committed case and the user's saved file, each run with `--set mesh.nx=64 --set
  mesh.ny=48 --set solver.dt=0.0625 --set solver.end_time=1.25` (20 steps, store at t = 1): every iteration event equal in
  index, time, res_u, res_p, continuity and both monitors, and the stored p at t = 1 equal bit for bit. Prediction: met.
* **RU (reproduction of the user's run).** 256 x 256, dt 0.0156, through this branch's binary: the stored p at t = 1, 2, 3
  bit-identical to U3's. Prediction: met (same solver code). A miss is reported as a deviation; the investigation then uses this
  branch's runs only.
* **Clean / spoky (L3).** A run is **clean** at a checked time if `M_hf <= max(2 M_hf(R64 at that time), 0.01)`, **spoky** if
  `M_hf >= 0.1`, otherwise **marginal**. Checked times: t = 1 and 3 for runs to t = 3; 1, 3 and 10 for runs to t = 10. A run is
  clean when it is clean at every checked time.
* **Screen (L4, supporting).** Smooth if the cd-change RMS <= 0.05 per step, jumpy if >= 0.5.
* **D1 (display).** For the R64 and R256 stores at t = 3 (and U3 at its snapshot nearest t = 3), rendered through the Studio's
  path (`StoreView::open`, `sample("p", t, false)`, `surface`, `Renderer::upload(.., Field::Face)`, `Renderer::pick`): every
  covered pixel's picked value equals the stored cell value of its face's owner cell as f32, bit for bit, with zero mismatches.
  Met means "not a display artefact" (H-disp rejected).
* **F-a (falsifier of the physical-transient reading).** (a) is rejected if any of: the dominant wavelength lambda* is <= 4 cells
  on a spoky run; the spoky pattern does not decay (L7); a smooth start (B6) is spoky; the force trace is jumpy (L4) on a mesh
  finer than a smooth one at the same Courant number.
* **L6 (wavelength tracks the cell).** When two or more resolutions are spoky: numerical if their lambda* in cells agree within a
  factor 1.5 while in units of D they differ by >= 1.8x. When only the finest is spoky, the reading is "appears with refinement".
* **L7 (decay).** R256 to t = 30: decays if `M_hf(30)/M_hf(10) <= 0.5`, otherwise persists; also printed for U2 over t = 1 to 71.
* **Bisection reading.** A single-factor arm **removes** the spokes if it is clean (L3) where its parent is spoky, **reduces**
  them if `M_hf <= 0.3 x` the parent's at the same time, else has **no effect**. H-Dw is supported if the clean/spoky
  classification of the R and B arms is monotone in `D_w` with one threshold and not monotone in `Co_theta`; H-Co the reverse.
  H-Choi, H-lin, H-conv, H-start are supported by "removes" of their arm (B11, B10, B7, B5/B6). H-nonorth is predicted to have no
  effect (B9).
* **W (user-facing workaround).** A candidate made from existing case keys only (no code change), on the user's mesh (256 x 256,
  grading 100) is **accepted** if it is clean at t = 1, 3 and 10 (L3), its screen is smooth (L4), and cd at t = 10 is within
  +-10 % of R128's when R128 is clean (otherwise within [1.0, 1.8]). The recommendation is the accepted candidate with the least
  wall time per simulated second (reported with its M_hf and run time).

## Set-ups (all from the committed compact case, copied; through the runner's command line)

Every run: `rcfd run <case> --case-dir <cases> --out <run>/output --status-file <run>/solver-status.jsonl --status-every 1 --quiet
--threads T --store-dir <run>/fields --status-monitors` (the command `rcfd-runner` builds, without its stop and pause files),
binary from this branch (`cargo build --release -p rcfd-cli`), T = 1 or 2 (results do not depend on it), 4 to 6 runs at a time
(other agents share the machine; free RAM is checked first). The case stores ux, uy, p, vorticity_z every 1.0 s in f32.
Overrides by `--set`:

| Id | Overrides | End | Purpose |
|---|---|---|---|
| S0a, S0b | committed / user-saved file, `mesh.nx=64 mesh.ny=48 solver.dt=0.0625 solver.end_time=1.25` | 1.25 | settings behaviour |
| R64 | none (64 x 48, dt 1/16) | 10 | control |
| R128 | nx 128, ny 96, dt 1/32 | 10 | resolution |
| R256 | nx 256, ny 256, dt 1/64 | 30 | resolution, decay (L7) |
| RU | nx 256, ny 256, dt 0.0156 | 10 | reproduction of U3 |
| B1 | nx 256, ny 48, dt 1/64 | 10 | nx only |
| B1b | nx 256, ny 48, dt 1/16 | 10 | nx only, Co 10 |
| B2 | nx 64, ny 256, dt 1/16 | 10 | ny only |
| B2b | nx 64, ny 256, dt 1/64 | 10 | ny only, Co 0.64 |
| B3 | 256 x 256, grading 10.88, dt 1/64 | 10 | first cell of 64 x 48 |
| B4a, B4b, B4c | 256 x 256, dt 1/16, 1/128, 1/256 | 3 | dt sweep on the fixed mesh |
| B5 | R256 + `initial.velocity_expression_y = "0 [m/s]"` | 3 | no push |
| B6 | R256 started by `rcfd restart` from R64's checkpoint at t = 10 (`--source-case`, `--method gradient`) | +3 | smooth start (developed field) |
| B6r | R256 with the far field `velocity = "fixed"`, `velocity_expression_x = "1 [m/s] * (1 - exp(-t / 0.5 [s]))"`, initial velocity 0 (no push) | 3 | smooth start (ramp; changes the far-field type: deviation by design) |
| B7 | R256 + `convection = "upwind"` | 3 | H-conv |
| B8a, B8b | R256 + `n_correctors = 3`, `4` | 3 | correctors |
| B9 | R256 + `non_orthogonal_correctors = 1` | 3 | null control |
| B10 | R256 + `pressure_rel_tol = 1e-12, pressure_abs_tol = 1e-20, pressure_max_iter = 20000, momentum_rel_tol = 1e-12, momentum_max_iter = 5000` | 3 | H-lin |
| B11 | R256 + `ddt_correction = false` | 3 | H-Choi |
| B12 | R256 + `time_scheme = "bdf2"` | 3 | time scheme |
| W1 | R256 + `algorithm = "pimple", n_outer = 2, alpha_u = 0.9, alpha_p = 0.9` (F-029's setting) | 10 | workaround |
| W2 | R256 + `algorithm = "pimple", n_outer = 4, alpha_u = 0.9, alpha_p = 0.9` | 10 | workaround |
| W3 | R256 + `time_advance = "fractional_step"` | 10 | other coupling path |
| W4 | B4c (dt 1/256) continued to t = 10 when B4c is clean | 10 | workaround (small dt) |

Arms whose parent result makes them uninformative are still run (no data-dependent pruning), except W4 (conditional as
written). If an arm cannot run as written (for example `rcfd restart` refuses the mesh transfer), that is reported as a
deviation and no substitute is run without an amendment.

## Tools (written after this registration, before their first use)

* `crates/rcfd-studio/tests/w48t_cylinder_spokes.rs` (ignored, environment-driven, no assertion on solver data except D1):
  `w48t_dump_stores` writes, per store, the cell centres and every snapshot's p (and step, time) as little-endian f64 through
  `rcfd_tsstore::Reader` (a store without an index is scanned); `w48t_display_matches_store` renders through the Studio path
  above and checks D1, writing the PNGs (zoom on the cylinder and the whole domain, the Studio's 2D start camera).
* `scripts/w48t_spoke_metrics.py`: the metrics above from the dumps and the status streams; figures (CPU maps of the stored p in
  the near-wall region, ring spectra, metric against time); a JSON summary.
* Known miss, if a solver defect is found: an ignored `*_known_miss` test in `crates/rcfd-api/tests/` on the cheapest spoky arm
  (by cell count; its limit is the clean limit L3 on M_hf at t = 3, computed in Rust with the same definition, cross-checked
  against the script on that arm), and an ignored heavy test of the accepted workaround.

## Amendment 1 (2026-10-28, before any run of DG1 or E1): the pressure-lag spectral radius with two correctors

Seen before this amendment: S0, RU, D1 (R64, U3) and the metrics of R64, R128, B1, B11, RU and U1-U5; B2 and B2b diverged
(steps 19 and 30). Nothing registered above changes. Added, because the case path enables the ADR 0073 pressure-lag check only
with one corrector (`sim.rs`: `n_correctors == 1`) while the default PISO has two:

* **Harness.** `crates/rcfd-api/tests/w48t_cylinder_spokes.rs`: the committed case with an arm's overrides is built by
  `Simulation::new`; its geometry (`sim.geom`) and initial cell fields (`ux`, `uy`, `p`) seed a `PimpleSolver` with the case's
  conditions (cylinder velocity fixed 0; far field `Freestream` for velocity and pressure, U_ref (1, 0, 0), p 0) and the
  settings `rcfd-api` builds for this case (`PimpleSettings::piso(0.01)`, `linear_upwind`, `amg_pressure` above 2000 cells,
  the arm's correctors / outer iterations / relaxation), plus `pressure_lag_check = LagCheck { start: 1, every: steps per unit
  time, iterations: 60, action: Warn }` (an estimate does not change the solution: it runs on copies of the step's matrices).
* **E1 (replica).** 64 x 48, dt 1/16: after 16 steps the harness's p and u equal `Simulation::step`'s bit for bit. A miss is a
  deviation: DG1 is then reported as a close replica with the measured largest relative difference.
* **DG1 (prints).** Run to t = 1 (or until the guard stops it); printed: rho and the radius and angle of the leading mode's
  largest entry at every estimate (step 1 and t = 1), the maximum and mean pressure iterations per step (`StepInfo::p_iters`;
  the cap is 3000 per corrector). Arms: G64 (64 x 48, 1/16), G128 (128 x 96, 1/32), G256x48 (256 x 48, 1/64), G64x256
  (64 x 256, 1/64, estimates every 16 steps), G256 (256 x 256, 1/64), G256s (256 x 256, 1/256), G256g (256 x 256, grading
  10.88, 1/64), G256c4 (G256 with 4 correctors), G256p2 (G256 with PIMPLE, n_outer 2, alpha_u = alpha_p = 0.9).
* **Reading rule.** The ADR 0073 pressure-lag mechanism, extended to two correctors on this mesh, is **supported** if for every
  DG1 arm whose `rcfd run` twin (R64, R128, B1, B2b, R256, B4c, B3, B8b, W1) is classified, `rho(step 1) >= 1` exactly when the
  twin is spoky or diverged; **rejected** if a clean twin has `rho >= 1` or a spoky/diverged twin has `rho < 1` at both
  estimates.

A run that the guard stops before t = 1 has no metric; it is classified **diverged**, which fails "clean" (a reading of L3, not a
change of it).

## Amendment 2 (2026-10-28, before any run of W5, W6, B2c, K1 or K2): the third corrector as a workaround candidate

Seen before this amendment, in addition to amendment 1's list: E1 (met), the metrics of B1b, B3, B4a, B6, B7, B8a, B8b, B9, B12
(B8a and B8b, `n_correctors` 3 and 4, are clean to t = 3); B5 and B6r diverged (steps 32 and 46). The registered W rule asks
for t = 10, which B8a did not run to. Added (same command line and reading rules as above):

| Id | Overrides | End | Purpose |
|---|---|---|---|
| W5 | R256 + `n_correctors = 3` | 10 | workaround candidate (user's mesh, dt 1/64) |
| W6 | nx 256, ny 256, dt 0.0156 (the user's file) + `n_correctors = 3` | 10 | the user's case with one key changed |
| B2c | B2b (64 x 256, dt 1/64) + `n_correctors = 3` | 3 | the known-miss set-up with the third corrector |

* **K1 (known miss).** `crates/rcfd-api/tests/w48t_cylinder_spokes.rs`: B2b's set-up (the cheapest failing arm: 16,384 cells,
  diverged at step 30 through `rcfd run`) through `Simulation` in-process to t = 3; limit: not stopped by the divergence guard,
  and clean by L3 at t = 3, `M_hf <= max(2 x 0.0057, 0.01) = 0.0114` (0.0057 is R64's measured M_hf at t = 3). Expected: miss;
  the test is `#[ignore]`d as a known miss with this limit.
* **K2.** The same with `n_correctors = 3`; a passing (heavy, ignored) test when B2c is clean, else a second known miss.
* **M_hf in Rust.** The test computes M_hf with the definition above; cross-check (prints): on the control (64 x 48, dt 1/16,
  t = 3, in-process f64) against the script's 0.0057 on R64's stored f32 field, relative difference <= 1e-3; on the K1 arm there
  is no field at t = 3 (it diverges), which is why the control is used (a deviation from the registration's "on that arm").

## Not changed by this job

Solver numerics, defaults, schema, the Studio and the render core (a display fix only if D1 fails and the fix is small),
`baseline_hashes.rs`, the user's project folder and the main checkout.

## Addendum (2026-10-28): results against the registered limits

Everything below was measured after the registration and the two amendments; no limit or rule was changed after a result was
seen. Machine shared with other agents (CPU load 57 to 100 %); runs at `--threads 1` (R256, RU, W4: 2); wall times are therefore
rough. Work folder `E:\projects\cfd\w48t-work` (runs, dumps, `metrics-all.json`, `report\`); the findings, tables and image paths
for the user are in [the job's README](../investigations/job48t-cylinder-spokes/README.md).

### Answer: (b), a numerical artefact of the solver

The default transient coupling of the case, `solver.algorithm = "piso"` (`PimpleSettings::piso`: n_outer 1, `n_correctors` 2),
does not converge the pressure-velocity coupling in the thin wall cells once the wall diffusion number nu dt / dr1^2 exceeds an
onset between 6.5 (R128, clean) and 11.4 (B4b, spoky). The left-over error is amplified from step to step (DG1: the ADR 0073
pressure-lag map has rho = 2.42 on the user's set-up, 0.36 on the committed one; its leading mode sits in the wall cell layer),
saturates at about 50 q as grid-scale circumferential pressure oscillations confined to r < 0.7 D (the spokes, wavelength 4.5 to 6
cells at 256 cells around, 7.2 cells at both 256 and 512 at t = 1), and on several set-ups grows until the divergence guard stops
the run. The Courant-matched refinement of the report raised nu dt / dr1^2 from 3.35 to 22.7 because the grading put the added
radial cells at the wall (first cell 0.01366 -> 0.00262 D).

### Results

| Check | Limit (registered) | Measured | Outcome |
|---|---|---|---|
| S0 | 20 iteration events equal; stored p at t = 1 bit-identical | 20 of 20 events equal; `store.rts` (and `monitors.csv`) byte-identical | **met**: the Studio's save changed no physics |
| RU | stored p bit-identical to U3 at t = 1, 2, 3 | bit-identical at all 9 common snapshots (t = 1.014 to 9.001) | **met**: this branch reproduces the user's run |
| D1 | 0 mismatching pixels (R64 and R256 at t = 3, U3 near t = 3) | 0 on 5 adapters (RTX 5080 Vulkan/DX12, Radeon Vulkan/DX12, WARP), domain and zoom views; also R64, R128, R256, W3, W5 at t = 1, 3, 10 | **met**: H-disp rejected |
| L3 per run | clean: M_hf <= max(2 x R64, 0.01); spoky: >= 0.1 | README table: clean R64, R128, B1, B1b, B3, B4c, B8a, B8b, W3, W5, W6; spoky R256, RU, B2c, B4a, B4b, B6, B7, B9, B10, B11, B12; diverged B2, B2b, B5, B6r, W1, W2 | as listed |
| L4 (screen) | smooth <= 0.05, jumpy >= 0.5 per step | every clean run 0.0003 to 0.0043; every spoky run 6.2 to 70 | agrees with L3 everywhere |
| F-a | (a) rejected if any of four | lambda* 4.5 to 6 cells (> 4: criterion not met); no decay (L7); smooth starts B6 spoky, B6r diverged; jumpy on 256 x 256 while 64 x 48 and 128 x 96 are smooth at the same Courant number | **(a) rejected** (three criteria) |
| L6 | lambda* in cells within 1.5x while in D >= 1.8x apart | U2 (256) 7.31 cells, U5 (512) 7.21 cells at t = 1; 0.103 D against 0.052 D at r = 0.6 | **numerical** |
| L7 | decays if M_hf(30)/M_hf(10) <= 0.5 | R256 44.9/44.0 = 1.02; U2 62.7/52.9 = 1.19 (48 to 69 up to t = 71, then diverged) | **persists** |
| Bisection: H-Dw / H-Co | monotone in one with one threshold | with PISO-2: clean at nu dt/dr1^2 0.84 (B1, B3), 3.35 (R64, B1b), 5.7 (B4c), 6.5 (R128); spoky or diverged at 11.4, 22.7, 22.8, 91. Courant: clean at 10 (B1b), diverged at 0.64 (B2b) | **H-Dw supported, H-Co rejected** |
| H-Choi (B11) | removes | 49.1 at t = 3 (R256 48.3) | no effect, rejected |
| H-lin (B10) | removes | 45.3 at t = 3; pressure solves take 10 to 14 iterations (DG1 `p_iters`), cap 3000 | no effect, rejected |
| H-conv (B7) | removes | 40.3 (0.83 x) | no effect, rejected |
| H-start (B5, B6, B6r) | removes | B5 diverged (step 32), B6 49.9 at t0 + 3, B6r diverged (step 46) | rejected |
| H-nonorth (B9) | predicted no effect | 45.3 (0.94 x) | no effect, as predicted |
| B12 (BDF2) | reduces if <= 0.3 x | 15.3 = 0.317 x | no effect (just above the reduce line) |
| B8a, B8b (3, 4 correctors) | removes | 9.2e-8, 9.1e-8 at t = 3 | **removes** |
| B3 (grading 10.88), B4c (dt 1/256) | removes | 3.1e-7, 1.1e-7 at t = 3 | **removes** |
| W (workaround rule) | clean at 1, 3, 10; smooth; cd(10) within 10 % of R128's 1.286 | W3 fractional step: 1.9e-7 / 7.9e-8 / 4.1e-8, cd(10) 1.291, 29 s per simulated second. W5 three correctors: 2.7e-7 / 9.2e-8 / 5.3e-8, 1.296, 49 s. W6 (user's dt 0.0156, three correctors): 2.8e-7 / 9.1e-8 / 4.3e-8, 1.296, 49 s. W4: see below. W1, W2 (PIMPLE n_outer 2, 4, alpha 0.9): diverged | W3, W5, W6 **accepted**; W1, W2 rejected |
| Recommendation | least wall time per simulated second | W3, `solver.time_advance = "fractional_step"` | as registered |
| E1 | bit-identical after 16 steps | p, ux, uy bit-identical | **met** |
| DG1 | rho >= 1 exactly on spoky/diverged twins | 8 of 9 arms agree (failing twins rho 1.68 to 4.36, clean 0.22 to 0.75); G256s rho 1.17 / 1.07 with a clean twin (B4c) | **rejected** in the strict form; rho separates the classes at a threshold between 1.17 and 1.68, not at 1 |
| K1 | reaches t = 3, M_hf <= 0.0114 | stopped at step 30 (t = 0.453) | **miss**: `w48t_k1_..._known_miss`, ignored, limit intact |
| K2 | the same with 3 correctors | reaches t = 3, M_hf 282 | **miss**: `w48t_k2_..._known_miss`, ignored, limit intact |
| M_hf cross-check | <= 1e-3 relative | 2.0e-7 (Rust 5.699737e-3, script 5.699736e-3) | met |
| W3, W5 in-process (heavy tests) | M_hf <= 0.0114 at t = 3 | 7.7e-8, 9.0e-8 | pass |

### Proposed fix (a recommendation; nothing of it is implemented; each needs the maintainer's decision)

1. **Warn (no numerics change).** Run the existing ADR 0073 pressure-lag estimate on the case path also for `n_correctors >= 2`
   (`sim.rs` limits it to 1 corrector) in warn mode, at step 1; results stay bit-identical (the estimate works on copies). DG1 says
   the warning level must be above 1.17 and below 1.68 for this case family (for example 1.3, to be registered); the message
   names `n_correctors`, `time_advance = "fractional_step"`, a lower grading or a smaller dt. Cost at step 1: about 60 extra
   step-equivalents of the linear algebra (1 to 2 s on 65,536 cells).
2. **Numerics (opt-in first).** PISO correctors until converged: an opt-in key (for example a tolerance on the largest pressure
   change between two correctors, relative to the pressure range, with a cap), defaults bit-identical when off. Acceptance to
   register: K1 and K2 pass, R256 clean, the ADR 0069 / 0073 transient benchmarks unchanged within their limits. A fixed third
   corrector is not enough in general: K2 (aspect 18.8 at the wall) stays spoky with three.
3. **Example project.** A note in `notes/cylinder.md` that refining `ny` at `grading = 100` thins the wall cell, with the
   settings above; the file is the user's tracked file, so the maintainer decides.

### Deviations

1. The worktree started 197 commits behind local main (at 1248a76); it was fast-forwarded to main 6867e40 before any work, as
   the brief requires ("based on local main, includes 48A-48O").
2. B6's screen window is t in [12, 13] (t0 + [2, 3] for the run restarted at t0 = 10), its checked times t0 + 1, t0 + 3; the
   registered window reads t in [2, 3] for runs started at 0.
3. The M_hf cross-check ran on the control, not on the K1 arm (no field at t = 3 there); written in amendment 2 before the run.
4. Runs whose guard stopped them before t = 1 are classified "diverged" (amendment 1). B2 and W2 stored t = 1 before diverging
   and are "spoky then diverged".
5. P0 (the `--print-settings` parse) and `rcfd check` ran before the registration (orientation, no solver run); S0 is their
   registered behavioural check.
6. The analysis scripts run with the user's site packages (numpy, matplotlib), i.e. without `python -I`; they read only this
   job's own dumps.

### Not run

* The whole gate (workspace fmt, clippy, tests). Run: rustfmt on the two new test files, `cargo clippy -p rcfd-api -p rcfd-studio
  --all-targets -- -D warnings` (clean), the docs-site test (2 passed), the new ignored tests as reported above.
* `rcfd-mcp render_field` (the CPU renderer): D1 used the Studio's own wgpu path; both draw the stored cell values.
* A 512 x 512 arm of this branch (only the user's U5 store, diverged at step 22).
* Linux.
* Any change to the user's project, the main checkout or the committed example case.

### Questions for the maintainer (with recommendations)

1. Enable the pressure-lag warning for `n_correctors >= 2` (fix 1)? Recommendation: yes, with a registered level between 1.17
   and 1.68 (DG1), as a separate job; it changes no result.
2. Add the opt-in converged-corrector PISO (fix 2)? Recommendation: yes, opt-in first, with K1 and K2 as its acceptance tests;
   decide a default only after a registered benchmark run.
3. Should the example case or its note recommend `time_advance = "fractional_step"` or `n_correctors = 3` for refined meshes?
   Recommendation: a note only (the committed 64 x 48 case is clean as it is); `fractional_step` is cheaper here, but ADR 0073
   (verdict 4) records a temporal-order floor of the fractional path (its total-pressure face term, an O(dt h^2) error), so PISO
   with enough correctors is the conservative choice for time accuracy.
4. A setup rule on nu dt / dr1^2 instead of fix 1? Recommendation: no; DG1 shows rho is not a function of that number alone
   (G256s at 5.7 has rho 1.17, G128 at 6.5 has 0.75), so the lag estimate itself is the better test.
