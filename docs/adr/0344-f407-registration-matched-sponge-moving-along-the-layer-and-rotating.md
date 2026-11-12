# ADR 0344: F-407 registration: the matched sponge on a mesh that moves along the layer (the memory of the attached layer is unstable; a memory kept in the lab frame) and the plain sponge on a rotating mesh

Status: **REGISTERED 2026-11-12**, written before any code of F-407 and before any run of its checks. Results go in an
addendum; nothing above the addendum is changed after a run. Not yet accepted by the maintainer.

Parents: [ADR 0265](0265-f341-registration-sponge-layer-on-moving-meshes.md) (F-341: `SpongeFrame::AttachedToMesh`, the
sponged ALE step `step_ale_sponged`, DR3 the memory per cell, DR5 and limits L2, L3, question 3 from which F-407 comes),
[ADR 0237](0237-f331-registration-sponge-layer-of-the-compressible-solver.md) (F-331: the plain and matched forms, the
reflection measure, the layer of D7, the isolated pulses X1 and X45), [ADR 0248](0248-f340-registration-matched-sponge-shift-corner-and-damping-along-the-layer.md)
(F-340: the shifted form and its memories `q`, `r`, `m`; DR11 the zero-frequency growth of the unshifted form),
[ADR 0207](0207-f258-registration-compressible-ale-boundaries-and-mesh-speed-time-step.md) (F-258: moving characteristic
faces, `max_dt_ale`). Roadmap item F-407 (depends on F-341).

Number: the lead gave 0344. Branch `f407-moving-matched-sponge`, worktree `w49i-work/f407`, from main b8b98cbf.

House rules (the campaign rules of 2026-11-12): limits are fixed here and are not changed after a run; a missed limit
becomes an ignored `*_known_miss` test with the limit intact and the measured number recorded, and is diagnosed, not tuned
away; a defective set-up gets a corrected test by an amendment committed before the corrected run, the old test staying as
an ignored record; each registered run is made once; parameters are fixed here from a derivation; defaults stay
bit-identical; `baseline_hashes.rs`, the hash tables and the goldens are not touched; the tests of ADR 0237, 0248 and 0265
are not edited; no new third-party crate, no unsafe code; unsigned commits without trailers; no push. `pimple.rs`,
`pimple/*` and `overset.rs` are not edited.

Clean-room: the design is derived below from the linearised Euler equations, the matched form of ADR 0237 DR4 and the
moving-cell memory of ADR 0265 DR3. Papers are cited from memory where named; none was opened. No vendor code or
documentation was used.

## What was read and done before this text (disclosed)

- **Read:** the F-407 entry of `docs/roadmap.json`; ADR 0265 in full; `crates/rcfd-fv/src/sponge.rs` lines 1 to 510 (the
  forms, `SpongeFrame`, `add_source`, `layer_gradient_sum`, `add_source_shifted`); `crates/rcfd-fv/src/compressible.rs`
  lines 140 to 180 and 780 to 1030 (`new`, `step_ale`, `step_ale_sponged`, `replace_mesh`);
  `crates/rcfd-fv/tests/sponge_f341.rs` in full; `crates/rcfd-fv/tests/sponge_f331.rs` lines 1 to 515 (the measure, the
  tube, the trains, `half_space`, X1, X45); the list of tests of `sponge_f340.rs`; the measured values of X1, X45, A30b and
  A45b in ADR 0237's addendum and its run times; the list of mesh generators of `rcfd-mesh/src/gen.rs` and
  `cylinder_ogrid`; the default boundary conditions of `CompressibleSolver::new`. The compressible solver has no periodic
  condition (`CompBc` has none), which shapes set-up X below.
- **Arithmetic, scratch Python scripts, no rcfd code** (folder `w49i-work/f407-scratch`, not committed):
  1. `stability.py`, `stability2.py`: the eigenvalues of the 4 by 4 system of DR2 (pressure, the two velocities, the
     memory; uniform rate) over real wave numbers. With the attached memory and `V = 0.2`: the largest growth rate is
     `0.125 sigma` (`sigma` = 1, 10, 103.6), reached for `k_x = 0` and large `|k_y|`, for either sign of `V`; with the memory
     of the lab frame, or with `V = 0`: `1e-13` (round-off; neutral). Growth over `sigma` at `k_x = 0`, for `k_y` = 5, 10,
     25, 50, 100, 300: `sigma = 20`: 0.006, 0.019, 0.062, 0.097, 0.116, 0.124; `sigma = 50`: 0.001, 0.004, 0.019, 0.049,
     0.087, 0.119; `sigma = 103.6`: 0.000, 0.001, 0.005, 0.018, 0.047, 0.102. The largest over `k_x` at `k_y = 25` and 100
     equals the value at `k_x = 0`.
  2. `column_model.py`: the linear solution of the same system in one dimension along the layer (`k_x = 0`), a Gaussian
     simple wave `p = v = eps exp(-(y/0.04)^2)` running along `+y`, by the eigen-decomposition of each Fourier mode
     (8192 modes on a periodic line of length 40). `max |p| / eps` at `t` = 0.5, 1, 1.5, 2: attached, `sigma = 50`:
     1.01, 4.16, 21.3, 133; lab frame, `sigma = 50`: 0.34, 0.34, 0.339, 0.34; attached, `sigma = 20`: 0.977, 2.67, 6.92,
     17.4; lab, `sigma = 20`: 0.574, 0.575, 0.575, 0.574.
  3. `geometry.py`: the numbers of set-up X (DR6), printed below.
  4. `radial_model.py`, `radial_model2.py`, `radial_model3.py`: the linear axisymmetric acoustic equations
     `p_t = -(u_r + u/r) - sigma p`, `u_t = -p_r - sigma u` (`rho = c = 1`), central differences, RK4, `dr` = 0.0005
     (and 0.00025), a pulse `p = u = eps g(r - r0)` started outgoing, with the layer of ADR 0237 D7 from `r_i` and without
     it on a long line; the measure of ADR 0237 at `r_i - 0.1025`. Results: for the derivative pulse `g1` of DR6
     (zero mean), `r0 = r_i - 0.3`: `R = 1.003e-2` (`r_i = 0.5`), `5.016e-3` (1.0), `2.558e-3` (2.0); grid-converged
     (`1.002e-2` at half the step); for the Gaussian pulse of ADR 0237: `2.83e-2`, `1.60e-2`, `8.39e-3`.
- **By hand:** DR1 to DR8 below.
- Nothing was compiled and no test was run before this text.

## What F-407 asks

Acceptance (binding): "An isolated plane pulse at 30 and 45 degrees through a matched layer attached to a mesh translating
along the layer at Mach 0.2 reflects less than 1 percent and stays bounded over 20 transit times of the layer, or the
reflection and any instability are measured and documented; a plain layer on a rotating mesh reflects a normal pulse less
than 1 percent." The registered rule of the campaign: a clause met by the "or" route (measured and documented) with its
limit missed is **partial**, not done.

**Reading (registered here).**

1. *"Reflects less than 1 percent"*: the measure of ADR 0237 (the largest `|p_test - p_ref|` over the probe cells and the
   steps of the window, over the largest `p_ref - p0` there), against a reference run on a grid extended behind the
   layer that moves in the same way, all runs in lockstep.
2. *"An isolated plane pulse at 30 and 45 degrees"*: a plane pulse along the direction `n = (cos theta, sin theta)` with a
   smooth envelope along its front, wide against the pulse (DR6): a plane pulse limited in its front. The solver has no
   periodic condition, and a mirror wall (X1 of ADR 0237) cannot be used: on a mesh that translates along the layer every
   boundary normal to the layer moves normally to itself and a rigid wall there is a piston. An unlimited front would cross
   the moving boundaries at `t = 0`.
3. *"A matched layer attached to a mesh"*: a matched layer whose rates belong to the cells (the attached profile of ADR 0265
   DR2). Both frames below have attached rates; they differ in the memory. The F-341 frame (`AttachedToMesh`) is measured
   and documented; the acceptance is checked with the new frame `AttachedWithLabMemory` (DR3).
4. *"Translating along the layer at Mach 0.2"*: the layer with the normal `+x`, every point of both grids moving at
   `V = (0, +0.2, 0)` (with the tangential direction of the incident pulse; `c = 1`).
5. *"20 transit times of the layer"*: the transit time is `L/c` = 0.2 (the layer thickness `L = 0.2` of ADR 0237 D7), so the
   run lasts to `t = 4.0`. *"Stays bounded"*: `B(t) = max over the cells of |p - p0| / eps <= 1.5` at every step to
   `t = 4.0`, every value finite (the incident peak is 1; a stable scheme does not amplify a pulse; an instability of DR2
   grows without limit).
6. *"A plain layer on a rotating mesh reflects a normal pulse"*: an outgoing cylindrical pulse (normal incidence on a
   circular layer at every point of the layer) on a mesh rotating about the centre of the layer, the layer radial, the
   rotation at Mach 0.2 at the front of the layer (DR7).

## Derivations

Notation: `rho0 = c = 1`, `p` the pressure perturbation, `(u, v)` the velocity perturbations normal (`x`) and tangential
(`y`) to the layer, `sigma` the rate, `Q` the memory of the matched form, `w` the mesh velocity, the time factor
`exp(-i omega t)`.

**DR1. The attached memory with a motion along the layer.** The source of the matched form (ADR 0237 D1, D2) is written in
the lab-frame perturbations of the conserved variables and is local, so it does not depend on the frame (ADR 0265 DR1). The
only frame-dependent part is the memory. In the attached frame the memory of a cell is the integral of `p` along the path
of the cell (ADR 0265 DR3): in the lab, `(d_t + w . grad) Q = p`. On a mesh translating along a planar layer the rates of
the cells do not change in the lab (`sigma` depends on `x` alone and the cells keep their `x`), so the layer itself is the
same as at rest; what changes is the memory equation, by the term `V d_y Q`.

**DR2. The attached matched layer is unstable for grazing waves.** Linearised, with a locally uniform rate:

    p_t + sigma p = -(u_x + v_y),   u_t + sigma u = -p_x,   v_t = -p_y - sigma Q_y,   Q_t + V Q_y = p.

At rest (`V = 0`, ADR 0237 DR4) this is the stretched-coordinate system with `s_x = 1 + i sigma/omega` and is matched. With
`V != 0` the memory is `Q = p / (-i (omega - V k_y))`, so the tangential momentum sees the stretching
`s' = 1 + i sigma/(omega - V k_y)` instead of `s_x`, and the dispersion relation in the layer becomes
`omega^2 = k_x^2 / s_x^2 + (s'/s_x) k_y^2`. The layer is no longer matched at oblique incidence (the factor `s'/s_x` differs
from 1 by an amount of order `M sin(theta)`), and it is unstable: for `k_x = 0` the relation is the cubic
`omega (omega - V k)(omega + i sigma) = k^2 (omega - V k + i sigma)` (`k = k_y`). At `sigma = 0` its roots are `omega = +-k`
and `omega = V k`; the first-order change of the root `omega = k` with `sigma` is

    d omega / d sigma = - (d f/d sigma) / (d f/d omega) = i a k / (2 k (k - a)) = i M / (2 (1 - M)),    a = V k,  M = V/c,

so a grazing wave that runs along the layer in the direction of the mesh motion grows at the rate `sigma M / (2 (1 - M))`
(0.125 `sigma` at `M = 0.2`) when `sigma << k`; the root `omega = -k` decays at `sigma M / (2 (1 + M))` and the root
`omega = V k` at `sigma / (1 - M^2)`. The scratch eigenvalue scan confirms `0.125 sigma` as the largest growth over all real
wave numbers, reached at `k_x = 0`, and shows the growth falling where `sigma / k` is not small (0.047 `sigma` at
`sigma = 103.6`, `k = 100`). In the layer of ADR 0237 D7 (`sigma` up to 103.6) a grazing mode of wavelength 0.06 grows
at about 5 per unit time, `e^20` over the 20 transit times of the acceptance. This is the instability that limit L2 of ADR
0265 expected for a velocity along the layer (Hu 2001 and Tam, Auriault and Cambuli 1998 discuss growing modes of matched
layers with a mean flow; from memory, not used): here the mean flow is the gas seen from the moving mesh, and the cause is
derived above.

**DR3. The memory in the lab frame.** The matched layer is matched when its memory is the integral of `p` at a fixed point
of the lab (`Q_t = p` in the lab). A cell that moves with `w_c` then carries the value of that field at its current
position, and its memory obeys

    d Q_c / dt = p_c + w_c . grad Q,

the transport of a field at rest through a moving control point (the first choice of ADR 0265 DR2 applied to the memory
only, the rates staying attached). On a mesh translating along a planar layer the rates are those of the layer at rest
(DR1), so with this memory the continuous system is exactly the matched layer at rest in the lab: matched at every angle,
and as stable as at rest (the scan: neutral to round-off). The same holds for the memories `r` (of `q`) and `m` (of the
momentum) of the shifted form (ADR 0248 DR14), which are time integrals at a fixed point as well: each gains its own
`w_c . grad`. The plain form has no memory and is not changed.

**DR4. The discrete transport.** In `step_ale_sponged` the velocity of a cell is the displacement of its centre over the
step, `w_c = (x_c^{n+1} - x_c^n) / dt` (exact for a translation; the chord velocity for a rotation). In every stage the
term `w_c . G_c(Q_p) / V_c` is added to the memory derivative of the cells in the layer, `G_c` the Green-Gauss sum of
`layer_gradient_sum` (the face value the mean between two layer cells, the cell's own value elsewhere; the gradient the
matched source already uses), on the new geometry, `V_c` the new cell volume; the memory then advances with the stage
weights of ADR 0265 DR3. This is a central difference: on a uniform Cartesian grid it is exact for a linear field and
second order otherwise; its truncation error for a pulse at 8 cells per width is about `(k h)^2 / 6`, a few `1e-3` of a
term that is itself `M sin(theta)` of the memory derivative. Where `w_c = 0` nothing is added, so on a mesh that does not
move the new frame gives the bits of the attached frame. The closure at the edge of the layer (own value) is exact for a
translation along a planar layer (the faces between layer and interior cells are normal to the layer and the translation
is along them) and approximate otherwise (limit L1).

**DR5. What the new frame does not fix.** A motion with a component normal to the layer moves the attached rates through
the lab (the layer moves; ADR 0265 DR2 chose that deliberately for a translating domain); with the lab memory the memory
then belongs to a point of the lab that the layer has left. A rotation turns the attached rates but not the `normal` of
the matched forms (ADR 0265 L3). Neither is checked here; the matched forms on a rotating mesh stay outside what is
derived (L2).

**DR6. Set-up X: a limited plane pulse through a layer on a mesh translating along it.** Pulse (zero mean):
`g1(s) = (s/w) exp(-(s/w)^2) / 0.428882` (peak 1), `w = 0.04` (8 cells), envelope `E(eta) = exp(-(eta/a)^4)`, `a = 0.4`,

    p' = eps g1(xi) E(eta),   xi = (x - x_c) cos + (y - y_c) sin,   eta = -(x - x_c) sin + (y - y_c) cos,

set as the isentropic simple wave along `n` of ADR 0237 (`superpose`), `eps = 1e-4 p0`. Why zero mean: the time integral of
`p` at a point is then zero for the plane part, so the memory left behind the pulse is of second order in `w/a`. With the
Gaussian of ADR 0237 the memory left in the layer would vary along the layer with the envelope, and a gradient of the memory
along the layer forces the tangential momentum for ever (the zero-frequency weakness of the unshifted matched form, ADR
0248 DR11), which would confuse the boundedness check with a property of the form at rest. Why the envelope: the
support `|xi| <= 4 w`, `|eta| <= 2 a = 0.8` (envelope `1.1e-7` there) is compact, so no boundary touches the pulse at
`t = 0` and the moving boundaries see a gas at rest; the angular spread is about `w / a = 0.1` rad; the envelope is above
0.85 within 0.25 of the centre line.

Grid `DX = 0.005`; the layer of ADR 0237 D7 (front `x_i`, `L = 0.2`, exponent 2, `R0 = 1e-6`, `sigma_max = 103.616`) up to
the end `x_i + L` (Thompson `NonReflecting { p0, 0 }` behind); `xmin`, `ymin`, `ymax` `Fixed(ambient)` (moving faces in a gas
at rest return it, ADR 0207 M3, M4; `xmin` moves along itself). The reference grid is the test grid extended behind the
layer by 0.6 (120 cells), Thompson at its end, the same `y` range. The numbers (`geometry.py`): the centre placed so that the
support is 0.05 from `xmin` and `ymin` and 0.02 in front of the layer; the probe the column of cells with centre
`x_p = x_i - 0.1025`; the incident centre line crosses `x_p` at `t_inc`, the reflection from the front of the layer returns
at `t_ref`, the one from the end of the layer at `t_deep = t_ref + 2L/cos`; `t_end = t_deep + 8 w` rounded up to 0.05;
`ymax` so that the support does not reach it before `t_end` (the domain moves up at 0.2); the probe cells those of the
column whose centre at `t = 0` is within 0.25 of the mean crossing point shifted back by the mesh motion.

| | `x_c`, `y_c` | `x_i` | test grid | reference grid | `x_p` | probe `y` at `t = 0` | `t_inc`, `t_ref`, `t_deep` | `t_end` |
|---|---|---|---|---|---|---|---|---|
| X30 | 0.59, 0.825 | 1.15 | `box_2d(270, 417, 1.35, 2.085)` | `box_2d(390, 417, 1.95, 2.085)` | 1.0475 | [0.838, 1.338] | 0.528, 0.765, 1.227 | 1.55 |
| X45 | 0.73, 0.73 | 1.43 | `box_2d(326, 494, 1.63, 2.47)` | `box_2d(446, 494, 2.23, 2.47)` | 1.3275 | [1.125, 1.625] | 0.845, 1.135, 1.701 | 2.05 |

The earliest return of the reference's end to the probe column is 1.99 (X30) and 2.44 (X45), after `t_end`.

Groups, each in lockstep (`dt` the smallest `max_dt_ale()` (`max_dt()` at rest) over the runs named for it):

- *moving* (every point `p0 + V t`): the reference (no sponge); **L**: matched, `AttachedWithLabMemory`; **A**: matched,
  `AttachedToMesh` (the F-341 frame); **NC**: no sponge, Thompson at the test grid's end. `dt` from the reference, L and NC
  (A must not slow the group when it grows); A is stepped with the same `dt` and stopped when `step_ale` fails, a value is
  not finite or `B > 100`; its last time is recorded. After `t_end` the reference and NC stop; L and A (if alive) continue
  to `t = 4.0` with `dt` from L.
- *static*: the reference and **S** (matched) on the grids at rest with `step` (the static form of ADR 0237).

The probe cells of the reference are found by their centre at `t = 0` (the grids share the lattice up to `x_i + L`).

**DR7. Set-up R: a plain radial layer on a rotating mesh.** The plain layer reflects nothing at normal incidence in one
dimension (ADR 0237 DR2). For a cylindrical wave it is not exactly matched: the geometric term `u/r` couples the outgoing
and the incoming waves, the free field behind `r_i` sends back a weak incoming wave that the layer removes by damping the
outgoing one, and the scratch radial model gives the reflection `R ~ w / (8 r_i)` for the zero-mean pulse `g1`
(`1.00e-2`, `5.02e-3`, `2.56e-3` at `r_i` = 0.5, 1, 2; `8.4e-3` for the Gaussian at `r_i = 2`). So the layer front is put
at `r_i = 2.0` and the pulse is `g1`. The mesh: `cylinder_ogrid(256, n_r, 1.4, r_out, Uniform, 0.005)` with every point's
angle warped by `phi -> phi + 0.06 sin(5 phi)` (`rcfd_mesh::gen::warp`; the angular widths vary by +-30 percent, so the
cells of a ring differ and the gas moving across them in the frame of the mesh is not reproduced by a symmetric
discretisation; radii unchanged), radial cells of 0.005: test `r_out = 2.2` (`n_r = 160`), reference `r_out = 2.6`
(`n_r = 240`). Rotation about the `z` axis at `Omega = 0.1` (Mach 0.2 at `r = 2`; points rotated by `Omega t`). On this
grid every circumferential face is a chord of a circle about the axis, so a rotation sweeps no volume through it: the inner
wall (`cylinder`, `Slip`) and the outer boundary (`farfield`) move along themselves. Rates
`sigma_max ((r - 2)/0.2)^2` for `r > 2` (`r` the cell-centre radius at `t = 0`), the plain form, frame `AttachedToMesh` (the
plain form has no memory; both moving frames give the same bits, B3); the attached rates of a radial layer are the rates
at rest at every time (rotation invariance). Pulse `dp = eps g1(r - 1.7)` along `r_hat` (`superpose`). `farfield`:
Thompson (test, reference). Probe: the ring of cells `ir = 99` (centre radius about 1.8975; the same `(it, ir)` cells in
both grids). Timing: incident at the probe 0.20, front reflection back 0.40, end reflection back 0.80, `t_end = 1.15`; the
reference's end returns at 1.6. The inward part of the started pulse reflects from the inner wall and reaches the layer at
0.9; both runs have it. Groups: *rotating* (reference, **RR** plain, **NCR** no sponge with `PressureOutlet { p0 }` at the
test grid's `farfield`) and *static* (reference, **RS** plain, with `step`).

**DR8. Set-up I: the grazing instability in one dimension.** A column `box_2d(1, 400, 0.005, 2.0)` (the layer's normal `x`,
the column along the layer), `xmin`/`xmax` `Slip` (they move along themselves), `ymin`/`ymax` `Fixed(ambient)`, the uniform
rate 50 in every cell, the matched form; the Gaussian simple wave `p = v = eps exp(-((y - 0.4)/0.04)^2)` along `+y`; every
point moving at `V = (0, 0.2, 0)`. This is the `k_x = 0` problem of DR2 and the scratch column model: attached `max|p|/eps`
21.3 at `t = 1.5`, lab frame 0.34 from `t = 0.5` on. The scheme's damping of the short waves (`k` 50 to 100, 12 to 25 cells
per wavelength, the growing ones) lowers the growth; allowance a factor 4. The pulse stays in the column (it reaches 1.9 at
`t = 1.5`, the column's top is then at 2.3).

## Decisions

- **H1. Opt-in frame.** `sponge::SpongeFrame::AttachedWithLabMemory`: the rates attached to the cells as in `AttachedToMesh`,
  the memories those of the lab frame (DR3, DR4). `AttachedToMesh` and `StaticMesh` are unchanged (their steps run the same
  statements). Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation): an opt-in variant
  and not a change of `AttachedToMesh`, because the F-341 tests define the attached frame's bits and the defaults stay.
- **H2. Code.** `sponge.rs`: the variant and a `pub(crate)` routine that adds `w_c . grad` of each memory to the memory
  derivatives (one layer: `q`; the shifted form: `q`, `r` and the three components of `m`; the layer cells of each form as
  in its source). `compressible.rs`: `step_ale` takes the sponged step for either attached frame; `step_ale_sponged`
  computes `w_c` and calls the routine in each stage only for `AttachedWithLabMemory` and a matched form. The documentation
  of `AttachedToMesh` states the instability of DR2.
- **H3. Not changed:** `step`, `step_sponged`, `step_sponged_shifted`, the sources, `replace_mesh` (refuses a sponge in every
  frame), `max_dt_ale`, the boundary conditions. No case-schema wiring, no file of `rcfd-api` (the sponge is library-only).

Code: `crates/rcfd-fv/src/sponge.rs`, `crates/rcfd-fv/src/compressible.rs` (shared solver file: the branch in `step_ale`, a
few lines in `step_ale_sponged`), new `crates/rcfd-fv/tests/sponge_f407.rs`, `docs/acoustics.md` (prose).

## Checks and limits (registered)

Common: `gamma = 1.4`, `rho0 = 1`, `p0 = 1/1.4` (`c = 1`), `eps = 1e-4 p0`, `CompSettings::default()` (HLLC, MUSCL van Leer,
SSP-RK3, CFL 0.4), a moving step with the mesh built by `rcfd_mesh::motion::moved_mesh`.

### Clause 1: plane pulses at 30 and 45 degrees through a matched layer on a mesh translating along it (heavy)

| id | check | limit | estimate |
|---|---|---|---|
| LR30, LR45 | run L (lab memory): `R` | `<= 1e-2` each | static matched (S) plus the transport error: `S` plus a few `1e-4`; ADR 0237 X1 `2.4e-5`, X45 `1.45e-4` (Gaussian pulse) |
| LB30, LB45 | run L to `t = 4.0`: `B(t)` at every step | `<= 1.5`, finite | DR3: as stable as at rest; `B` at most the incident 1 |
| SR30, SR45 | run S (static, matched): `R` | `<= 1e-2` each | `1e-4` to `1e-3` (ADR 0237 X1, X45; the zero-mean pulse has more short waves) |
| NC30, NC45 | run NC (no sponge, Thompson; the measure's control) | `[0.03, 0.12]`; `[0.08, 0.25]` | Thompson at oblique incidence `(cos - 1)/(cos + 1)` = 0.072, 0.172, less the peak loss of the path (about 10 percent), +-40 percent |
| AR30, AR45 | run A (attached, F-341): `R` over the window (or up to its stop) | record (measured and documented) | not estimated (DR2: mismatched at order `M sin(theta)`) |
| AB30, AB45 | run A: `B` | record; predicted: `B > 1.5` or a failed step before `t = 4.0` | DR2: growth about 5 per unit time for the grazing modes in the deep layer |

The AR and AB rows are the "measured and documented" route for the F-341 frame; they do not enter the decision except as
documentation (a missed prediction is reported and diagnosed).

### Clause 2: a plain layer on a rotating mesh, normal pulse (heavy)

| id | check | limit | estimate |
|---|---|---|---|
| RR | set-up R rotating, plain: `R` | `<= 1e-2` | the radial model `2.56e-3` plus the scheme |
| RS | set-up R at rest, plain: `R` | `<= 1e-2` | as RR |
| NCR | set-up R rotating, no sponge, `PressureOutlet { p0 }` at the test grid's end (the measure's control) | `>= 0.5` | `R = -1` at the release; spreading from 2.2 back to 1.8975, `sqrt(1.8975/2.5025) = 0.87`, less the peak loss: about 0.8 |

### I: the grazing instability (fast)

| id | check | limit | estimate |
|---|---|---|---|
| I1a | set-up I, `AttachedToMesh`: `B` at the first step with `t >= 1.5` | `>= 5` | model 21.3 |
| I1l | set-up I, `AttachedWithLabMemory`: the largest `B` over the steps with `0.5 <= t <= 1.5` | `<= 0.5` | model 0.34 |
| I1s | set-up I at rest with `step`: as I1l; and `abs(B_l - B_s)` at the first step with `t >= 1.5` | `<= 0.5`; `<= 0.05` | the same equations (DR3); the ALE and transport errors |

### G3: the transport term against a closed form (fast)

Set-up G3: `box_2d(16, 8, 1.6, 0.8)`, the uniform rate 2 in every cell, the reference at rest `rho 1, p0` and every cell
at it, all four patches `Fixed(reference)`, every point moving at `(0.2, 0, 0)` (normal to the layer: the field is linear
along `x` so the tangential gradient and with it the force are zero, and the state stays), 50 steps of `dt = 0.01`.
Memories at `t = 0`: `q = beta x_c` (`beta = 1e-3`), `r = 0`, `m = (gamma_m x_c, 0, 0)` (`gamma_m = 2e-3`). Closed forms of
the lab frame (DR3, with `p = 0` and no shift): `q_c(t) = beta x_c(t)`, `r_c(t) = beta x_c(t) t`, `m_c(t) = (gamma_m x_c(t), 0, 0)`
(a polynomial of degree 2 in `t`, which SSP-RK3 integrates exactly). Compared in the cells off the first and last columns
(there the Green-Gauss sum has no boundary face, the gradient is half; not checked).

| id | check | limit | estimate |
|---|---|---|---|
| G3m | matched (normal `x`), `AttachedWithLabMemory`: `max |q_c - beta x_c(t)| / (1.6 beta)`; drift of the state | `<= 1e-12`; `<= 1e-12` | exact up to round-off (DR4: the Green-Gauss sum of a linear field is exact on the uniform grid) |
| G3s | shifted (`shift 0`, `along 0`, one layer, normal `x`), `AttachedWithLabMemory`: `q`, `r` (over `1.6 beta t_end`), `m` (over `1.6 gamma_m`); drift | `<= 1e-12` each | as G3m |
| G3c | control: G3m with `AttachedToMesh`: `max |q_c - beta x_c(t)| / (1.6 beta)` | `>= 0.05` | `0.2 x 0.5 / 1.6 = 0.0625` (the attached memory keeps `beta x_c(0)`) |

### U, B3, S5 (fast)

| id | check | limit |
|---|---|---|
| U1L | set-up U1 of ADR 0265 (the deforming set-up U of ADR 0207 to `t = 2`) with `AttachedWithLabMemory`: matched at rest and the shifted corner of U1s: drift; memories as in ADR 0265 U1 | `<= 1e-12` each |
| U2r | set-up R's grid, coarse (`cylinder_ogrid(64, 40, 1.4, 1.6)`, warped as R), rotating at `Omega = 0.1` for 100 steps of `max_dt_ale`, the plain radial layer from `r = 1.5` (rate up to 20), every cell at the reference at rest: drift | `<= 1e-12` |
| B3a | 30 steps of set-up U of ADR 0207 with its characteristic conditions (B2 of ADR 0265) and the plain layer of U1: `AttachedWithLabMemory` against `AttachedToMesh`: state and mesh speeds | bit-identical |
| B3b | set-up I with the mesh not moving (`step_ale` to the same points), 40 steps, matched and shifted (`shift 0.25`, `along 0.05`): `AttachedWithLabMemory` against `AttachedToMesh`: state and memories | bit-identical |
| B1 | pass unedited, release: `cargo test -p rcfd-fv --lib sponge`, `--lib compressible`, the test files `sponge_f341`, `sponge_f331`, `sponge_f340` (fast tests), `compressible_ale_f258`, `compressible_ale_f258_records`, `ale_gcl`; and `cargo test -p rcfd-api --release --test baseline_hashes` | pass |
| S5 | `AttachedWithLabMemory` is kept across `set_sponge`; `step_ale` accepts the plain, matched and shifted forms in it; `replace_mesh` refuses a sponge in it; the default is still `StaticMesh` | as stated |

## Mutations (registered; on a scratch copy, not committed)

Each is applied in a `git archive` copy with its own target directory; then `cargo test --release -p rcfd-fv --test
sponge_f407` (the fast tests) is run once.

| id | mutation | expected to be caught by |
|---|---|---|
| ML1 | the transport term dropped (the lab frame acts as the attached one) | G3m, G3s, I1l |
| ML2 | the sign of the transport term | G3m, G3s, I1l (the memory then convects at `-2V` in the lab: growth as DR2) |
| ML3 | the transport applied to `q` only in the shifted form | G3s |
| ML4 | the cell velocity halved | G3m, G3s |
| ML5 | `step_ale` refuses a sponge in `AttachedWithLabMemory` | S5 and every lab-frame test |

A mutation that no check catches is reported as surviving.

## Decision rule (registered)

- **Clause 1 met** when LR30, LR45 meet `1e-2` and LB30, LB45 hold, with SR30, SR45, NC30, NC45 held (the set-up and its
  measure are sound). AR and AB are reported in any case (the F-341 frame measured and documented).
- **Clause 2 met** when RR meets `1e-2` with RS and NCR held.
- **done** (proposed): both clauses met; I1, G3, U1L, U2r, B1, B3, S5 met; every mutation caught. The text says that
  clause 1 is met with the new frame and that the F-341 frame is unstable with a motion along the layer (documented).
- **partial**: a clause met only by measurement and documentation (LR or LB missed, or RR missed), or a fast check missed,
  or a mutation surviving; the miss named with its number and diagnosis.
- **todo**: the lab-memory step not usable (L not finite before `t_end`).

## Limits

- L1. The lab memory is derived for a translation along a planar layer (rates invariant). For a motion with a normal
  component or a rotation the attached rates move through the lab and the closure at the layer's edges is approximate
  (DR4, DR5); not checked.
- L2. The matched forms on a rotating mesh (the fixed `normal`, ADR 0265 L3) are not built; clause 2 is for the plain form.
- L3. Set-up X is a plane pulse limited by a smooth envelope (angular spread about 0.1 rad), not an unlimited plane pulse:
  the solver has no periodic condition and the moving boundaries exclude a mirror wall. One direction of the motion
  (`+y`, with the tangential direction of the pulse); the other sign is not run (the instability of DR2 exists for both).
- L4. The boundedness run lasts 20 `L/c`; a reading with 20 `2L/c` (to `t = 8`) is not run.
- L5. The plain layer reflects about `w/(8 r_i)` of a cylindrical pulse (DR7), a property of the plain form in cylindrical
  geometry, not of the rotation; set-up R puts the layer at `r_i = 2` to stay below the limit with margin.
- L6. Library only (as the sponge itself), two-dimensional grids, explicit source (`rate dt` well below 2.5).

## Sources

- The matched form and its memory: ADR 0237 DR4, ADR 0248 DR14, ADR 0265 DR3 (this repository); J.-P. Berenger,
  J. Comput. Phys. 114 (1994); F. Q. Hu, J. Comput. Phys. 173 (2001) (matched layer with a mean flow); C. K. W. Tam,
  L. Auriault, F. Cambuli, J. Comput. Phys. 144 (1998) (growing modes of the split layer with a mean flow). Cited from
  memory; nothing used here depends on them: DR2 and DR3 are derived above.
- The ALE step and its GCL: ADR 0090 D2, ADR 0207 M3, M4, ADR 0265 DR4.

## Open questions (with recommendations)

- Q1. Make `AttachedWithLabMemory` the behaviour of `AttachedToMesh` for the matched forms (the attached memory has no use
  once it is known to be unstable with a motion along the layer). Recommendation: yes, at the next change of the sponge's
  API with the maintainer's approval (it changes the bits of the F-341 frame, which its tests record); not taken here.
- Q2. A layer fixed in space for a motion with a normal component or a rotation (ADR 0265 Q1): the rates re-evaluated and
  the memories transported. Recommendation: only when a case needs it.
