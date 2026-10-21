# ADR 0090: Wave 38C, dynamic mesh layering (F-230): cell-layer addition and removal at a moving piston, ALE for the compressible solver, inverse-volume smoothing stiffness

Status: accepted (registration, then addendum). The registration below was committed before any code of this wave was written. Limits and decision
rules are fixed now and are not changed after a run; a miss becomes an ignored `*_known_miss` test with the limit intact, a defective set-up gets a NEW
corrected test and the old one stays ignored. Baseline hashes are never re-recorded. Clean-room: first principles only; no vendor documentation, code or
data is used. Parents: ADR 0033 (F-063: swept volumes, GCL, displacement-diffusion smoothing, incompressible ALE), ADR 0006 / the explicit
density-based `CompressibleSolver` (HLLC, MUSCL, SSP-RK).

## Question

Can a closed cylinder compressed and re-expanded by a piston, with cell layers removed and added next to the piston as their height crosses registered
thresholds, reproduce the isentrope `p V^gamma = const` within 2 percent (roadmap F-230 acceptance), with volume and mass conserved through every layer
removal and addition?

## What was known or run before this registration (disclosed, rule (d))

* Code read (read-only): the F-063 ALE path (`rcfd-fv/src/ale.rs`, `pimple/moving.rs`, `rcfd-mesh/src/motion.rs`) exists only for the
  incompressible PISO/PIMPLE solver. The density-based `CompressibleSolver` (`rcfd-fv/src/compressible.rs`) has no moving-mesh support: its residual,
  ghost states and face states assume a fixed mesh. An isentrope cannot be tested on the incompressible path, so this wave must add ALE to the
  compressible solver. `smoothing.rs` already has `Laplacian` and `InverseDistance(k)` diffusivities; nothing in the code base matches exhaustively on
  `SmoothingMethod` outside that file and `rcfd-api/src/dynmesh.rs` (an `if`, not a `match`).
* Set-up smoke (scratch test on bf2aa83, existing code only, release): `box_mesh([40, 2, 2], [1.0, 0.1, 0.1])` with the x coordinates re-mapped
  through `warp` builds (160 cells, six wall patches). The existing `CompressibleSolver` (defaults: HLLC, van Leer, SSP-RK3, CFL 0.4) on this closed
  box (all patches slip walls) at rest (`rho = 1`, `p = 1`, `gamma = 1.4`) stays at rest for 100 steps: `max |u| = 0`, mass and energy drift 0.
  `max_dt = 4.23e-3`.
* No moving-mesh compressible run, no layering code and no inverse-volume smoothing exists before this ADR.

## Decisions

### D1 Scope and files

Delivered slice: layering (cell-layer removal and addition at one moving planar boundary of a structured layered mesh) with exact conservative remap,
an opt-in ALE step of `CompressibleSolver`, and, if time permits, an `InverseVolume(k)` smoothing stiffness. NOT in this slice and declared not
delivered: local remeshing (cell-quality-driven re-triangulation), layering of unstructured or non-planar layers, layering in the incompressible
PIMPLE path, `rcfd-api` case-tree wiring (`dynamic_mesh` keys) for any of it, sliding interfaces.

Files: new `rcfd-mesh/src/layering.rs` (layer stack, thresholds, mesh build, cell source map, conservative remap; one `pub mod` line in
`rcfd-mesh/src/lib.rs`), additive hunks in `rcfd-fv/src/compressible.rs` (`step_ale`, `replace_mesh`; the fixed-mesh residual keeps its arithmetic),
one enum variant and one diffusivity arm in `rcfd-fv/src/smoothing.rs`, new test file `rcfd-fv/tests/layering_w38c.rs`. Every default path is
bit-identical (no existing function changes its floating-point operations); the baseline hashes are not touched.

### D2 ALE compressible step (first principles)

Step `n -> n+1`, swept volume `W_f` of face `f` from `rcfd_mesh::motion::swept_volumes` (old points to new points; `sum_f sigma_f W_f = V^{n+1} - V^n`
to round-off, ADR 0033), face normal mesh speed `w_n = W_f / (dt |S_f|)` with `S_f` of the new mesh, `v = w_n n`. Semi-discrete form
`d(V U)/dt = R(U) = -sum_f (F(U).n - w_n U) |S_f|`. The ALE face flux is computed in the frame moving with `v`: both reconstructed states have their
velocity shifted by `-v`, the existing Riemann solver gives `F'`, and the lab-frame flux is `F_rho = F'_rho`, `F_m = F'_m + v F'_rho`,
`F_E = F'_E + v . F'_m + |v|^2/2 F'_rho` (algebraic identity, exact for any Galilean-consistent flux). A slip wall that moves is a mirror in the moving
frame: ghost velocity `u_g = u - 2 ((u - v) . n) n`; the face state used by the gradients has normal velocity `w_n`. Characteristic boundary
conditions (`TotalInlet`, `PressureOutlet`, `NonReflecting`, `FarField`) on a face with `w_n != 0` are rejected with an error (not supported).
SSP-RK stages carry the stage volume `V_s = V^n + c_s (V^{n+1} - V^n)` obtained by applying the same RK combination to `dV/dt = sum W / dt`
(`c = 1` for Euler and both RK2 stages; RK3 stage volumes `V^{n+1}`, `(V^n + V^{n+1})/2`, `V^{n+1}`), and the stage state is `(V U)_s / V_s`; this
makes every stage preserve a uniform state exactly (discrete GCL). The new geometry (and the least-squares matrices) replace the old ones at the start
of the step.

### D3 Layering

A layered mesh is `box_mesh([n, ny, nz], [L, ly, lz])` whose x plane coordinates are the list `xs` (`xs[0] = 0`, `xs[n]` = piston), built by
re-mapping the plane coordinates with `warp`; cell `(i, j, k)` is `i + n (j + ny k)` and layer `i` lies between `xs[i]` and `xs[i+1]`. Only the
piston plane moves; the layer next to the piston (`n - 1`) deforms. Thresholds with ideal height `h0` (the initial uniform layer height):

* removal: after a step, if `h = xs[n] - xs[n-1] < a_min h0` and `n > 2`, the plane `xs[n-1]` is deleted: layers `n-2` and `n-1` merge (cell-wise in
  the same column);
* addition: after a step, if `h > a_max h0`, a plane is inserted at `xs[n-1] + h0`: the piston layer splits into a layer of height `h0` next to the
  static stack and a piston layer of height `h - h0`.

Registered `a_min = 0.4`, `a_max = 1.6` (`1 + a_min <= a_max`, so a merged layer, at most `1.4 h0`, never splits and a split piston layer, at least
`0.6 h0`, never merges in the next check; the driver asserts that no layer is removed in the step after an addition and vice versa).
Conservative remap (cell totals): a merged cell gets `V U = V_a U_a + V_b U_b`; a split cell's children get the parent density `U` (each child's total
is `U` times its geometric volume; the fractions `h0/h` and `(h - h0)/h` sum to one). Geometry and state change together at fixed time.

### D4 Stiffness variant (if time permits)

`SmoothingMethod::InverseVolume(k)`: `gamma_f = (1 / V_f)^k` with `V_f` the mean of the owner and neighbour cell volumes (owner volume on boundary
faces). Small cells are stiffer and deform less.

## Registered checks

Common set-up P (piston): `box_mesh([40, 2, 2], [1.0, 0.1, 0.1])`, `h0 = 0.025`, all patches slip walls, piston = patch `xmax`. Gas `gamma = 1.4`,
initial state `rho0 = 1`, `u = 0`, `p0 = 1` (`c0 = 1.183`). `CompSettings::default()` (HLLC, van Leer, SSP-RK3, CFL 0.4). Piston position
`x_p(t) = 1 - 0.3 (1 - cos(pi t / T))` for `0 <= t <= 2T`, `T = 16`: compression to `x_p = 0.4` at `t = T` (volume ratio 2.5), re-expansion to
`x_p = 1` at `t = 2T`; the piston starts and stops each stroke at rest; peak piston speed `0.3 pi / T = 0.0589` (Mach 0.050 at `c0`). Time step
`dt = max_dt()` of the current mesh, clipped to land on `t = T` and `t = 2T`. Each step: new piston plane position, `step_ale`, then the layering check
of D3 (at most one event per step). The cross-section shape does not matter for slip walls with one-dimensional motion; the 2 x 2 cross-section makes a
layer more than one cell, so the cell-wise merge and split are exercised.

| Id | Check | Limit |
|---|---|---|
| A1 | `p_bar V^gamma / (p0 V0^gamma) - 1` at `t = T` (piston at rest, `V = 0.4 V0`); `p_bar = sum p_c V_c / sum V_c` | `|.| <= 0.02` |
| A2 | the same at `t = 2T` (`V = V0`) | `|.| <= 0.02` |
| B1 | every layer event: `sum V`, `sum rho V`, `sum rho E V` after vs before | relative `<= 1e-13` |
| B2 | every step: total mass vs initial | relative `<= 1e-12` |
| B3 | every step: total mesh volume vs `0.1 * 0.1 * x_p(t)` | relative `<= 1e-13` |
| B4 | number of removals and additions | each `>= 5` (expected about 24 each) |
| B5 | no removal in the step after an addition and no addition in the step after a removal | exact |
| C1 | GCL: open duct `box_mesh([20, 2, 2], [1, 0.1, 0.1])`, `xmin` and `xmax` `Fixed` to the uniform state `rho = 1`, `u = (0.3, 0, 0)`, `p = 1`; interior planes moved by `x_i + 0.01 sin(2 pi x_i) sin(2 pi t)` (end planes fixed), 200 steps of `step_ale` | `max_c |U_c - U0| / |U0|` per component `<= 1e-12` |
| C2 | zero motion: `step_ale` with the unchanged mesh vs `step` from the same non-uniform state (Sod-type initial data in the P mesh), one step | max relative difference `<= 1e-13` |
| D1 | moving-wall mass flux: in P, `sum` of the mass flux through `xmax` over the run (from the per-step mass balance; equals B2) | covered by B2 |
| S1 | `InverseVolume(1)`: all patches prescribed the same translation `d0` on `box_mesh([5, 4, 3], [1, 0.8, 0.6])` x-graded 2 | `max |d - d0| <= 1e-12` |
| S2 | graded 1-D compression: `box_mesh([20, 2, 1], [1, 0.1, 0.1])` x `Expansion(10)` (small cells at `xmin`), `xmin` prescribed `(0.1, 0, 0)`, `xmax` fixed, other patches slip; smallest-cell strain (relative x-height change of the first layer) with `InverseVolume(1)` vs `Laplacian` | ratio `< 1` (direction; the continuous 1-D value `h_min L / sum h_i^2` is reported, no limit) |
| S3 | S2's Laplacian run: strain of the first and last layer equal (a linear displacement is exact) | relative difference `<= 1e-9` |

Diagnostics reported, no limit: max over the run of `|p_bar V^gamma / (p0 V0^gamma) - 1|` at every step, cell-wise spread of `p / rho^gamma` at
`t = T`, event counts and times, step count and wall time.

Decision rules: A1, A2, B1-B5, C1, C2 all met -> F-230 slice accepted, but the roadmap status is set to `partial` regardless, because local remeshing
(in the feature title) and the case-tree wiring are not delivered (registered now). Any A or B miss -> `partial` with the miss reported, or `todo` if
no ALE step works at all. S1-S3 are a separate registered check; their outcome does not change the F-230 status.

## Addendum: measured (release build, scratch tree on 2f84f63 plus this wave's files)

Files as planned in D1: `rcfd-mesh/src/layering.rs` (`LayerStack`, `LayerChange`, `LayerEvent`, `remap`, one unit test), `pub mod layering` in
`rcfd-mesh/src/lib.rs`, `CompressibleSolver::step_ale` and `replace_mesh` in `rcfd-fv/src/compressible.rs` (the fixed-mesh residual is split
into `flux_sum` plus the unchanged division; with no mesh speed every flux goes through the same `solver.flux` call as before),
`SmoothingMethod::InverseVolume(k)` in `rcfd-fv/src/smoothing.rs`, tests in `rcfd-fv/tests/layering_w38c.rs`. The `rcfd-fv` compressible unit
tests, `mesh_smoothing` and `rcfd-api` `baseline_hashes` pass unchanged.

| Id | Measured | Limit | Verdict |
|---|---|---|---|
| A1 | `p_bar V^gamma / (p0 V0^gamma) - 1 = 1.53e-5` at `t = T` (`V/V0 = 0.400000`, 16 layers, `p_bar = 3.606805` vs isentrope `3.606750`) | `<= 2e-2` | met |
| A2 | `3.42e-5` at `t = 2T` (40 layers again) | `<= 2e-2` | met |
| B1 | `4.4e-16` (max over all events of V, mass, energy) | `<= 1e-13` | met |
| B2 | `5.4e-13` | `<= 1e-12` | met |
| B3 | `8.9e-16` | `<= 1e-13` | met |
| B4 | 24 removals, 24 additions | each `>= 5` | met |
| B5 | 0 | 0 | met |
| C1 | `4.5e-15` after 200 steps | `<= 1e-12` | met |
| C2 | `1.8e-16` | `<= 1e-13` | met |
| S1 | `6.8e-17` | `<= 1e-12` | met |
| S2 | smallest-cell strain Laplacian `0.1000`, `InverseVolume(1)` `0.01794`, ratio `0.179` (continuous 1-D value `0.173`); largest-cell strain `0.1000` vs `0.169` | ratio `< 1` | met |
| S3 | `3.4e-14` | `<= 1e-9` | met |

Diagnostics: the largest `|p_bar V^gamma / (p0 V0^gamma) - 1|` over all 9 192 steps is `3.43e-5`; at `t = T` the cell values of `p / rho^gamma`
lie in `[0.999986, 1.000137]` (initially 1); the whole piston run takes 4.8 s.

Reading: the 2 percent acceptance is met with a margin of about 600. At peak piston Mach 0.05 the case is nearly quasi-static, so this run shows
that the ALE fluxes and the layer remap are consistent. It is not a sensitive test of numerical entropy production. B2 is the closest margin
(about 2x): the mass drift is round-off that builds up over 9 192 steps of stage divisions by the volume.

Deviation (disclosed): the first S2 run printed the continuous 1-D reference as `0.3465`. The test divided the cell volumes by the full
cross-section `0.01` instead of the cell cross-section `0.005` (two cells across y). That reference value is a diagnostic with no limit. It was
corrected to `0.1732` after the run; the registered direction check and its measured ratio are unchanged.

Not delivered: local remeshing; layering of unstructured, prism or non-planar layers; more than one moving layered boundary; layering or
compressible ALE through the `rcfd-api` case tree (`dynamic_mesh` keys) or the CLI; layering in the incompressible PIMPLE path; ALE with
characteristic boundary conditions on moving faces (rejected with an error); mesh-speed-aware `max_dt`. The time step uses the absolute velocity
(shortcut: fine at piston Mach 0.05, needs `|u.n - w_n| + c` for fast walls). Roadmap: F-230 is set to `partial` under the registered decision
rule, because remeshing and case-tree wiring are missing even though every registered check is met.

Defaults bit-identical, verified: a scratch test ran the fixed-mesh `CompressibleSolver` with default settings and each of Euler, RK2 and
RK3 (300 steps, Sod data with a y shear, `box_2d(100, 3)`). The FNV hash of every conserved value's bits is identical between `compressible.rs`
at 2f84f63 and at 7de9a54 (`cea94485dbaa04c4`, `c78a1c90bc62d177`, `b798be9366eedace`). `baseline_hashes.rs` hashes no compressible case.

Final gate (fresh `git archive 0763acf`, `CARGO_TARGET_DIR=target-w38c`): `cargo fmt --all -- --check` clean, `cargo clippy --workspace
--all-targets -- -D warnings` clean, `cargo test --workspace --release --no-fail-fast` 1887 passed / 0 failed / 531 ignored. Wave 37 closed at
1881 / 0 / 531; the difference is this wave's 6 new tests. No LNK1201. This gate-totals paragraph is docs only and was not re-gated.
