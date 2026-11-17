# ADR 0672: F-752 registration: pressure part of the non-orthogonal correction on coupled faces

Status: **REGISTERED 2026-11-16, written before any code of F-752 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-752 was written and no run of any kind was made; the numbers quoted
are those measured and printed in ADR 0667. Results go in the addenda at the end; nothing above the addenda is changed after a
run.

Roadmap item F-752 (deps F-744), proposed by [ADR 0667](0667-f744-registration-non-orthogonal-correction-on-coupled-faces.md)
remaining question 1. Parents: ADR 0667 (F-744: the opt-in `PimpleSettings::coupled_non_orth`; its mutations MN1, the pressure
equation without the coupled correction, and MN4, the coupled faces' flux correction with the opposite sign, were not caught,
because on ADR 0661's lockstep the wrap faces' `k` is tangential and the pressure nearly radial), [ADR 0664](0664-f743-registration-implicit-momentum-coupling-of-periodic-pairs.md),
[ADR 0661](0661-f725-registration-sector-against-full-annulus-with-a-non-axisymmetric-inlet.md),
[ADR 0629](0629-f304-registration-planar-and-sector-sliding-interfaces.md), ADR 0046 (translational periodic pairs, the
implicit partner in the momentum predictor), ADR 0080 (`coupled_body_pair_mean`). Number 0672 given by the lead. Branch
`f752-work` from main d397df17 (F-743 and F-744 merged), worktree `w49i-work/f752`, scratch `w49i-work/f752-scratch`.

House rules (`w49i-work/AGENT_RULES.md`) apply: limits fixed here; each registered run made once; existing tests not edited;
defaults bit-identical; no edit of `baseline_hashes.rs`, of a hash table or of a golden; no new dependency, no unsafe code;
clean-room; the roadmap files are not edited. Every choice the maintainer would normally be asked about is taken on the
maintainer's standing instruction of 2026-11-11 (take the recommendation), except where listed as open.

Clean-room: no source outside the repository was read. The correction checked is ADR 0667's (the repository's own over-relaxed
split of the face gradient, `k = S - d |S|^2 / (S . d)`); the reference is a closed-form construction of a second mesh of the
same cells (below), and the hydrostatic balance `grad p = rho g` is the textbook one. The arithmetic below was done by hand
before any run.

## What F-752 asks

The acceptance, binding (from the lead's brief):

> On a flow with a tangential pressure gradient across non-orthogonal coupled faces (a swirling inlet on ADR 0661's lockstep, or a skewed translational periodic channel against its unrolled twin) the opt-in of ADR 0667 brings the periodic run within a registered limit of the reference on every step where the setting without its pressure part does not, and the corrected flux is divergence-free per cell within a registered limit; defaults bit-identical. Every limit not stated here is registered in the ADR before the run.

Clauses:

- **a1** on a flow with a tangential pressure gradient across non-orthogonal coupled faces, the periodic run with ADR 0667's
  opt-in is within a registered limit of the reference on every step where the setting without its pressure part is not;
- **a2** the corrected flux is divergence-free per cell within a registered limit;
- **a3** defaults bit-identical.

**Which flow (registered choice, Q1).** The skewed translational periodic channel against its unrolled twin. Reasons: (i) the
reference can be made exact: the twin is the same cells with the periodic cut moved to where the faces are orthogonal, so the
skewed coupled faces of the periodic run are internal faces of the twin, and ADR 0667's derivation says the corrected pair is
that internal face term for term (for a translation `R = I`); (ii) a body force across the channel gives a tangential pressure
gradient of any chosen size on every coupled face, which the swirling inlet of ADR 0661 gives only weakly and with the rest `R`
of ADR 0661 (`1.5e-4`) on top; (iii) it is cheap (a few hundred cells), so the fast test is the full record.

**Whether the setting applies to translational pairs (established from the code read, before any run).**
`CoupledNonOrth::build` takes every patch with a partner map whose condition is coupled; a translational pair has
`periodic_rot[pi] = None`, which the code reads as `R = I` (`rot.as_ref().map_or(back, ...)`), so `d = (x_f - c_r) + (c_c -
x_g)` and the face gradient `w grad_r + (1 - w) grad_c`. ADR 0667's U1 built 64 such faces on the translational Taylor-Green box
and ran the setting there. So the setting applies to translational pairs; H1 below checks it on this mesh. The note of ADR 0667
("rotational coupled faces") describes its acceptance flow, not a restriction of the code.

**"The setting without its pressure part" (registered).** ADR 0667's setting with its pressure part left out: the momentum
part (the coupled faces' correction of the momentum diffusion and the partner-point velocity gradients) on; the pressure
equation and its flux the default ones (`solve_pressure`, `corrected_flux`: the orthogonal coupling on the coupled faces, the
face-value least-squares gradient). No existing setting gives this, so it is a new opt-in (below). Called `M` below; ADR
0667's full setting `F`; the setting off `O`.

**Reading of a1 (registered).** With `G_V(n) = max_c |u_V(n)_c - u_T(n)_m(c)| / U_ref` (`m(c)` the twin cell of periodic cell
`c`, `u_T` the twin run with the full setting), the check is `G_F(n) <= eps` on every step `n`, and the control `G_M(n) > eps`.
If the control is outside `eps` on every step, "every step where the setting without its pressure part does not" is every step;
if it were outside on only some steps, P1 still asks for every step (stronger), and the clause is non-vacuous only when the
control is outside on at least one step (NC1 registers more: every step).

**Reading of a2 (registered).** Two forms. (D1, binding) the divergence of the corrected flux of a step whose explicit
non-orthogonal iteration is converged (`non_orth_correctors = 100`), so that the flux is that of the solved equation and the
per-cell divergence is the equation's residual: `max_c |sum_f phi_f| / (U_ref h dz)` (`h` the cell height, `dz` the depth).
(D1L, binding) on the registered lockstep (`non_orth_correctors = 6`, the setting of ADR 0667) the periodic run's per-cell
divergence equals the twin's at the matched cell: at six passes the explicit iteration is not converged, and the flux of every
face carrying a non-orthogonal part (internal faces too, in both meshes, by the repository's scheme) has the divergence of the
last pass's gradient change; the twin carries the same on its internal faces, so the coupled cells must carry what the twin's
internal-face cells carry. The absolute lockstep divergence is printed, no limit (it is the default scheme's, not this item's).

## What the code read shows (d397df17, read only)

1. `pimple.rs` `step_with`: `cn_u = s.coupled_non_orth.then(...)` (momentum: partner-point gradients, `add_vector_rhs`), and in
   the correctors `cn_p = s.coupled_non_orth.then(...)` selecting `solve_pressure_coupled` and `corrected_flux_coupled`; `cn_p`
   is read nowhere else.
2. `pressure.rs` `solve_pressure_inner`: with `coupled_no` and a gradient, `cn.add_scalar_rhs(&mut pm.b, &gamma_eff, gr)` adds
   `gamma_eff k_f . grad_f` to `b[r]` (MN1 removes this); `corrected_flux_coupled`: `corrected_flux_with` with the partner-point
   gradient, then `cn.fix_flux` subtracts `gamma_eff k_f . grad_f` (MN4 flips this). The final flux takes the gradient of the
   new pressure, the last equation pass that of the pressure before its solve (read 2 of ADR 0667): with a finite number of
   passes the flux divergence is `gamma k . (grad_new - grad_last)` summed over each cell's faces, internal faces included.
3. Internal-only terms of the step that would make a periodic run differ from its twin: the Rhie-Chow `ddt_correction`
   (`phi_hby_a[f] += gamma_p[f] / dt * ddt_defect[f]` for `f < n_int`; default `true`) and the step-consistent Rhie-Chow
   (`rhie_chow_step_consistent`, default `false`). The body force's face coefficient on a coupled face is the owner's `rAU`
   unless `coupled_body_pair_mean` (ADR 0080), in which case it is the pair mean, the internal face's weighted value.
   Everything else on a translational coupled face is the internal face's formula: the pressure's orthogonal coefficient
   `|S| / span` (ADR 0667 design), the face value `a phi_P + (1 - a) phi_q` with `a = dn_q / span` (the internal faces'
   `geom.weights` is the same ratio) in the Green-Gauss gradients, `boundary_hby_a`, the central convection, and the implicit
   partner of the momentum (ADR 0046).
4. `BodyForce::uniform(geom, a)`: `cell = a`, `face = a . S_f` on every face; `pressure_bc` gives the fixed-velocity walls the
   Neumann gradient `bf.face / |S|` (the balanced hydrostatic wall condition). With no fixed-pressure face the pressure is fixed
   by `p_ref = (cell, value)` (the diagonal doubled, `b += d value`).
5. `rcfd_mesh::gen::box_2d(nx, ny, lx, ly, true, false)` gives `xmin`/`xmax` as a translational pair (`set_periodic`, shift =
   the difference of the patches' mean face centres), walls `ymin`, `ymax`, depth `lx / nx`; `gen::warp` maps the points and
   keeps the topology and the pair; `FvGeometry::new` matches the pair's faces by the shift (`1e-6` tolerance).

## Design

### The new opt-in

`PimpleSettings::coupled_non_orth_without_pressure: bool`, default `false`: read only with `coupled_non_orth`; with both set
the pressure equation and its flux take the default path (`cn_p` not built), the momentum part stays. One line in
`step_with` (`cn_p = (s.coupled_non_orth && !s.coupled_non_orth_without_pressure).then(...)`), the field and its default. With
the field `false` every operation is the previous one (a3). It is a control for the effect of the pressure part (this item's
acceptance names it); it is not meant as a production setting.

### The mesh pair (closed form)

Periodic channel `P`: `box_2d(32, 16, L = 2, H = 1, periodic x)`, points mapped by `x' = x + t(x) (y - H/2)`, `y' = y` with

`t(x) = tau cos^2(2 pi s / L)` for `s = x mod L` in `[0, L/4]` or `[3L/4, L]`, `t = 0` for `s` in `(L/4, 3L/4)`, `tau = 0.3`.

`t` is periodic with period `L`, continuous with a continuous slope (`cos^2` and its slope vanish at `L/4`, `3L/4`), so the
lines `x = const` are straight segments tilted by `t` near the cut `x = 0 = L` and vertical in the middle half. The coupled
faces (`x = 0` and `x = L`) are tilted by `tau`: normal `n = (1, -tau) / sqrt(1 + tau^2)`; with the centre offset `d` along
`x` (both neighbouring columns nearly parallelograms), `k = S - d |S|^2 / (S . d) = |S| (n - x_hat / n_x)`, `|k| = |S| tau`:
`|k| / |S| = 0.3` (a non-orthogonality of 16.7 degrees). Cell shape: the horizontal spacing of two neighbouring lines is `dx (1
+ t'(x) (y - H/2))`, `|t'| <= tau 2 pi / L = 0.942`, so at least `0.53 dx`: no cell is inverted.

Twin `T`: the same with `x' = x + t(x + L/2) (y - H/2)`: the cells of `P` translated by `-L/2` modulo `L` (node `x_i` of `T` is
node `x_i + L/2` of `P` shifted by `-L/2`, and `t` is `L`-periodic; equal up to the rounding of `x_i +- L/2`). `T`'s cut lies at
`P`'s `x = L/2`, inside the band where `t = 0`: the cells on both sides of `T`'s cut and their neighbours are rectangles, so
`T`'s coupled faces have `k = 0` to rounding, and `P`'s skewed coupled faces are `T`'s internal faces at `x' = L/2`.

### The flow

Incompressible, `nu = 0.1`, a uniform body acceleration `g = (f_x, g_y) = (0.8, -2.0)`, no-slip walls `ymin`, `ymax`,
periodic in `x`; start from rest with the hydrostatic pressure `p0 = g_y (y - H/2)`; `p_ref` = (`P`'s cell 0, `p0` there) in
`P` and (its matched cell, `p0` there) in `T`. `f_x` drives the start-up of plane Poiseuille flow (steady centreline `f_x H^2
/ (8 nu) = 1.0 =: U_ref`; at `t = 2` about `1 - exp(-pi^2 nu t / H^2) = 0.86` of it); `g_y` gives the pressure gradient
`(0, g_y)`, whose component along every coupled face is `g_y / sqrt(1 + tau^2)`: the tangential pressure gradient the item
asks for. `k . grad p = -|S| tau g_y / sqrt(1 + tau^2) = 0.57 |S|`, not small.

Settings (every run): `PimpleSettings::piso(0.1)`, `Central`, `n_outer = 1`, `n_correctors = 2`, `non_orth_correctors = 6`,
`TIGHT` momentum and pressure solvers (`1e-16` abs, `1e-13` rel, 5000; as ADR 0667), `ddt_correction = false` and
`coupled_body_pair_mean = true` (read 3: the two internal-only or owner-only terms that would make `P` differ from `T` whatever
this item does; Q2), `dt = 0.01`, 200 steps (`t = 2`; Courant at most `0.86 * 0.01 / 0.0625 = 0.14`).

Runs: `P_F` (full setting, the acceptance variant), `P_M` (without its pressure part, the control), `P_O` (setting off,
printed), `P_X` (`coupled_non_orth_without_pressure = true` alone, for D3), `T_F` (the reference), `T_O` (the twin with the
setting off, for D2). For D1: `P_F` and `P_M` with `non_orth_correctors = 100`, 5 steps.

## Estimates (before any run)

- **P1 (a1).** `P_F` against `T_F`: by ADR 0667's derivation with `R = I` and read 3 every term of every coupled face of `P` is
  that of the matching internal face of `T`, the gradients the corrections use are the same fits (partner point = neighbour
  cell, same displacement, same value), and the fixed-point iteration of the six passes starts from the same pressure. What
  differs is the order of summation and the iterative solves (different numbering, different coarsening): ADR 0667 measured
  `3.8e-15` on its first step and U1 `5.6e-16` (velocity), `3.6e-14` (pressure); a solve to `1e-13` relative per step, at most
  added over 200 steps, `2e-11`. Limit **`eps = 1e-8`** per step (margin 500).
- **NC1 (control of a1).** `P_M` against `T_F`: the pressure equation of `P_M` lacks `s = gamma k . grad p` on each coupled
  face. With `rAU = V / a_P`, `V = (1/16)^3 = 2.44e-4`, `a_P = V / dt + 4 nu |S| / dx = 0.0244 + 0.025`, `rAU = 4.9e-3`; `|S| =
  (1/16)^2 sqrt(1.09) = 4.1e-3`; `s = gamma |S| tau |g_y| / sqrt(1 + tau^2) = 4.9e-3 * 4.1e-3 * 0.3 * 2 / 1.044 = 1.15e-5`
  per face. A defect of the same sign on every face of the cut (a 1-D balance along `x`): the
  pressure of `P_M` jumps by `J = s dx / (gamma |S|) = tau |g_y| dx / sqrt(1 + tau^2) = 0.036` across the cut (and ramps back by
  `J / L` over the period: a spurious mean pressure gradient `0.018`, 2.3 percent of `f_x`). The cells next to the cut get the
  Green-Gauss gradient `J / (2 dx) = 0.29` of it and the velocity `rAU * 0.29 = 1.4e-3` at step 1; from then on the jump and the
  spurious mean gradient persist with the hydrostatic gradient (the velocity error at the cut stays, the mean flow error grows
  toward 2.3 percent of `U_ref`). Limit **`G_M(n) >= 1e-4` on every step** (margin 14 at step 1).
- **P2 (supporting).** The pressure, `max_c |p_PF - p_TF| / (|g_y| H)`: rounding as P1, **`1e-8`** per step.
- **D1L (a2, lockstep).** Fluxes are at most `U_ref h dz = 3.9e-3`; the per-cell divergences of `P_F` and `T_F` differ by the
  rounding of the solves, `1e-13` relative: **`max_c |div_PF(c) - div_TF(m(c))| / (U_ref h dz) <= 1e-9`** each step (margin
  1000).
- **D1 (a2, converged).** The deferred correction contracts by about `|k| / |S|` per pass, `0.3` (taken as at most `0.5`):
  `0.5^100 = 8e-31`; what is left is the last solve's residual, `1e-13` relative to the pressure equation's right-hand side
  (fluxes of `3.9e-3` and less): **`max_c |div| / (U_ref h dz) <= 1e-10`** over the 5 steps. Under MN1 the converged flux has
  `s` at every cut cell: `1.15e-5 / 3.9e-3 = 3e-3`; under MN4, `2 s`: `6e-3`.
- **D2.** `T_F` against `T_O`: `T`'s coupled faces and every face of the cells next to them have `k = 0` to rounding; the
  gradients the setting changes enter only through `k`: rounding, accumulated over 200 steps as in P1: **`1e-10`** per step.
- **H1.** Exact geometry up to the rounding of coordinates of order 2: **`1e-12`** for centres, `S`, `d`, `k`; `|k| / |S|` of
  `P`'s coupled faces in **`[0.25, 0.35]`** (derived `0.3`, the neighbouring columns' slight trapezoid shape moves `d` off
  `x_hat` by a little); `T`'s coupled faces **`|k| / |S| <= 1e-12`**.
- **Cost.** 512 cells, `2 * 7` pressure solves per step, six runs of 200 steps and two of 5 steps at 100 passes: well under a
  minute in release. No heavy record is needed: the registered runs are the fast test.

## Checks (limits fixed now)

| Id | Clause | Set-up | Check | Limit |
| --- | --- | --- | --- | --- |
| P1 | a1 | lockstep, 200 steps | on every step `G_F(n) = max_c abs(u_PF - u_TF) / U_ref` | `<= 1e-8` each step |
| NC1 | a1 (the control "does not") | same | on every step `G_M(n)` | `>= 1e-4` each step |
| P2 | a1 (supporting) | same | on every step `max_c abs(p_PF - p_TF) / (abs(g_y) H)` | `<= 1e-8` each step |
| D1L | a2 (lockstep) | same | on every step `max_c abs(div_PF(c) - div_TF(m(c))) / (U_ref h dz)` | `<= 1e-9` each step |
| D1 | a2 (converged) | `P_F` with `non_orth_correctors = 100`, 5 steps | `max_c abs(div) / (U_ref h dz)` over the steps | `<= 1e-10` |
| D2 | set-up of the reference | lockstep | on every step `max_c abs(u_TF - u_TO) / U_ref` | `<= 1e-10` each step |
| D3 | a3 | lockstep | `P_X` (the new field alone) against `P_O`: velocity, pressure, flux, every step | bit for bit |
| G0 | set-up | the meshes | every `P` cell centre translated by `-L/2` (mod `L`) has a `T` cell within `1e-12`, one-to-one | as stated |
| H1 | set-up (the setting applies; the twin is the unrolled mesh) | the meshes | `P`'s velocity and pressure `CoupledNonOrth` each have `2 ny = 32` faces, every pair translational; every one has an internal face of `T` at its image centre with the same `S`, `d`, `k` (oriented from the matched owner); largest `abs(k) / abs(S)` of `P`'s; largest of `T`'s | `1e-12` each; `[0.25, 0.35]`; `<= 1e-12` |
| B1 | a3 | `cargo test -p rcfd-api --release --test baseline_hashes`; `rcfd-fv --lib`; the named suites `sliding_annulus_inlet_f744` (its fast test and H2: ADR 0667's numbers unchanged), `sliding_annulus_inlet_f743`, `periodic`, `consistent_periodic_f367`, `seam_no_harm_f344`, `non_conformal`, `gradient_pressure_f291`, `piso_thin_gap_f299` | pass | as stated |

Printed, no limit: `G_F`, `G_M`, `G_O` at step 1, their largest and its step; the absolute lockstep divergence of every run
(the six-pass mismatch of the default scheme); D1 of `P_M` (the default pressure pair is consistent too: a2 alone does not
discriminate, a1 does); the pressure jump of `P_M` across the cut at the last step; elapsed times.

Mutations (on a scratch copy of the library, after the checks; each applied alone; the new test file run once per mutation;
each must fail the named checks):

- **MN1** the pressure equation without the coupled correction (`add_scalar_rhs` not called; the flux keeps it): must fail P1
  or D1 or D1L.
- **MN4** the coupled faces' flux correction with the opposite sign (`fix_flux` adds): must fail D1 or D1L or P1.
- **MN3** the momentum's coupled correction with the opposite sign: must fail P1.
- **MN5** the new field not read (`cn_p` built whenever `coupled_non_orth`): must fail NC1.

Diagnostic mutation (reported, not in the decision rule): **MN2** the pressure part's gradient the plain least-squares
gradient (face values on the coupled faces) in place of the partner-point gradient.

## Decision rule

**Done** when P1, NC1, D1, D1L (a1, a2), D2, D3, G0, H1 and B1 are met and MN1, MN3, MN4, MN5 are caught; P2 is supporting
and reported. **Partial** when P1 and D1 are met but another check or a mutation is missed; or when P1 or D1 is missed with its
cause diagnosed. **Todo** when P1 and D1 are both missed without a diagnosis, or a3 fails (D3 or B1).

## Limits (not covered)

- Translational pairs only (`R = I`): the rotation in the pressure part's face gradient (`R grad_c`) is not exercised here;
  ADR 0667's MN2 (the partner's gradient not rotated, in pressure and momentum together) was caught on the annulus, but the
  pressure part's effect on rotational faces is not shown (follow-up proposed in the addendum if this item passes).
- `ddt_correction` off and `coupled_body_pair_mean` on in every run (Q2): with the default `ddt_correction` the coupled faces
  lack a term the internal faces carry, a difference of the default scheme, not of this item.
- One mesh pair, one tilt (`0.3`), one body force, 2-D (one layer), the PISO step; the fractional step, the steady solver and
  the refused combinations of ADR 0667 are not touched.
- The new field is reachable from `PimpleSettings` only (no case key; a key changes the schema golden).

## Open questions

- **Q1. The flow.** (a) the skewed translational channel against its twin (taken: an exact reference, a strong tangential
  gradient, cheap); (b) a swirling inlet on ADR 0661's lockstep (the rest `R` of `1.5e-4` on top of the effect, a radial
  pressure that the swirl tilts only partly, 400-step heavy runs). Taken (a) on the maintainer's standing instruction of
  2026-11-11 (take the recommendation).
- **Q2. `ddt_correction` and `coupled_body_pair_mean` in the test.** (a) `ddt_correction = false`, `coupled_body_pair_mean =
  true` in every run (taken: isolates the item); (b) the defaults, with a limit covering the internal-only term. Taken (a) on
  the standing instruction. Whether `ddt_correction` should act on coupled faces is a separate question for the maintainer
  (it changes default periodic results).
- **Q3. The control as a setting.** (a) a field of `PimpleSettings` (taken: the only way to run the step without the pressure
  part; opt-in, default bits unchanged); (b) a test-only copy of the step (not possible from an integration test). Taken (a)
  on the standing instruction.

## Addendum 1: results (2026-11-16)

Measured against the limits registered above; no limit was changed and no amendment was made. Release builds on Windows, on a
machine shared with other building agents. The registration is 94945ea5 (alone, first); the library change and the tests
da10e704 (committed before their run). Each registered run was made once: the new test file (3 tests, 73 s; the lockstep 66 s,
under the two-minute line, so no heavy record), `rcfd-fv --lib`, `baseline_hashes`, the named suites. Logs in
`w49i-work/f752-scratch` (`fast.log`, `lib.log`, `baseline.log`, `suites.log`, `mutations.log`, `checks.log`).

**Built as registered.** `PimpleSettings::coupled_non_orth_without_pressure` (default `false`) in `crates/rcfd-fv/src/pimple.rs`
(the field and its doc, lines 199-203; its default, line 1028; the condition of `cn_p` in `step_with`, lines 2285-2286); the new
test file `crates/rcfd-fv/tests/skewed_periodic_channel_f752.rs`. With the field `false` the step's operations are the previous
ones. No case key, no golden, no hash table.

### Results

| Check | Limit | Measured | Verdict |
| --- | --- | --- | --- |
| P1 (a1) `G_F(n)`, every one of 200 steps | `<= 1e-8` each step | `5.81e-15` (step 33; step 1 `2.06e-15`) | met |
| NC1 (a1, control) `G_M(n)`, every step | `>= 1e-4` each step | smallest `1.48e-3` (step 1), largest `1.92e-3` (step 7) | held |
| P2 (supporting) pressure, every step | `<= 1e-8` each step | `5.25e-14` | met |
| D1L (a2) `abs(div_PF - div_TF) / (U_ref h dz)`, every step | `<= 1e-9` each step | `5.94e-15` | met |
| D1 (a2) converged flux (100 passes, 5 steps) | `<= 1e-10` | `3.56e-15` | met |
| D2 `T_F` against `T_O`, every step | `<= 1e-10` each step | `0` (bit for bit) | met |
| D3 `P_X` against `P_O`, every step | bit for bit | equal on every step | met |
| G0 cell map `P` to `T` | `<= 1e-12`, one-to-one | `8.9e-16`, yes | met |
| H1 coupled faces as twin faces | 32 and 32 faces, translational; centre, `S`, `d`, `k` `<= 1e-12`; `abs(k) / abs(S)` in `[0.25, 0.35]`; `T`'s `<= 1e-12` | 32, 32, translational; `6.7e-16`, `1.4e-17`, `8.8e-16`, `3.9e-17`; `0.2979`; `0` | met |
| B1 `baseline_hashes`; `--lib`; the named suites | pass | see below | met |

Printed, no limit: `G_O` (setting off) step 1 `1.48e-3`, equal to `G_M`'s (at step 1 the velocity gradients are zero, so the
momentum part does nothing and the whole step-1 difference is the pressure part's), largest `1.33e-2` at step 200 (the momentum
part's share grows with the Poiseuille profile); the control's pressure differs from the twin's by up to `2.80e-2` (estimate:
the jump `J = 0.036`); the largest absolute lockstep divergence over the steps, in `U_ref h dz`: `P_F` `1.33e-10`, `T_F`
`1.33e-10` (the six-pass mismatch of the default scheme, the same in both), `P_M`, `P_O`, `P_X` `1.35e-9`; D1 of `P_M`
`6.1e-15` (the default pressure pair is consistent too, as registered: a2 alone does not discriminate, a1 does); `P_F`'s
largest `u_x` at step 200 `0.855 U_ref` (estimate `0.86`). Against the estimates: NC1 `1.48e-3` against `1.4e-3` at step 1;
P1 `5.8e-15` against up to `2e-11`.

Suites (release, run once): `baseline_hashes` 3 passed, 1 ignored; `rcfd-fv --lib` 115 passed, 6 ignored;
`consistent_periodic_f367` 13 (2 ignored), `gradient_pressure_f291` 3, `non_conformal` 2, `periodic` 3, `piso_thin_gap_f299` 18
(2 ignored), `seam_no_harm_f344` 6 (1 ignored), `sliding_annulus_inlet_f743` 1 (1 ignored), `sliding_annulus_inlet_f744` 2 (1 ignored; ADR 0667's N1J `3.8008e-5` and D0J `7.896331e-4` reproduced), all passed; ignored records not run. `cargo fmt -p rcfd-fv --check` clean; `cargo clippy --release -p rcfd-fv --all-targets -- -D
warnings` clean.

### Mutations (scratch copy `w49i-work/f752-scratch/mut` of da10e704, script `mutate.py`, target `target-mut`; the new test file run once per mutation)

| Mutation | Must fail | Result |
| --- | --- | --- |
| MN1 the pressure equation without the coupled correction | P1 or D1 or D1L | caught: D1 `2.88e-3`, D1L `2.88e-3`, P1 `2.06e-3`, P2 `1.40e-2` |
| MN4 the coupled faces' flux correction with the opposite sign | D1 or D1L or P1 | caught: D1 `5.76e-3`, D1L `5.76e-3` (step 1), P1 `6.68e-4`, P2 `9.7e-4` |
| MN3 the momentum correction with the opposite sign | P1 | caught: P1 `2.32e-2`, P2 `9.5e-2` |
| MN5 the new field not read | NC1 | caught: NC1 `1.6e-15` (the control equals `F`) |
| MN2 (diagnostic) plain least-squares gradient in the pressure part | none | P1 `1.41e-7`, P2 `1.71e-6` (both over their limits); D1, D1L, NC1 met |

Four of four required mutations caught. ADR 0667's two uncaught ones (MN1, MN4) are now caught by three checks each, the
divergence checks at the estimated size (MN1 `s`: `3e-3`; MN4 `2 s`: `6e-3`). D2, D3, H1 and G0 held under every mutation.
The library was restored after each (equal to da10e704). MN2 shows that on this flow the face-value gradient in the pressure
part costs `1.4e-7 U_ref`, four orders below the pressure part's own effect (`1.5e-3`) but above P1's limit: ADR 0667's Q1
choice (the partner-point gradient) is the one that makes the coupled face the twin's internal face.

### Deviations

None from the registered set-up.

### Not covered

As registered: translational pairs only (the rotation `R` in the pressure part's face gradient is not exercised here);
`ddt_correction` off and `coupled_body_pair_mean` on in every run; one mesh pair, one tilt, one body force, 2-D, the PISO step;
no case key; the default unchanged. `docs_site` not run (only prose under `docs/`: this ADR and one sentence of
`docs/dynamic-mesh.md`; left to the daily gate).

### Decision

By the registered rule: P1, NC1, D1, D1L, D2, D3, G0, H1 and B1 met, MN1, MN3, MN4 and MN5 caught: **done**.

### Remaining questions

1. **The pressure part on rotational pairs.** Options: (a) a follow-up item with a rotationally periodic sector against its
   unrolled full annulus under a balanced body force whose potential has the sector's period and a tangential gradient on the
   wrap faces (for example `phi = r^6 cos 6 theta` on a sixty-degree sector), with this item's checks; (b) rely on ADR 0667's
   MN2 (the rotation of the partner's gradient, caught on the annulus through the momentum part) and this item.
   Recommendation (a): cheap, and the rotation in `face_gradient` is the one term of the pressure part this item does not
   reach.
2. **`ddt_correction` on coupled faces (Q2).** The default Rhie-Chow `ddt_correction` acts on internal faces only, so a
   periodic run differs from its twin by that term whatever the non-orthogonal setting. Options: (a) a follow-up giving the
   coupled pairs the term behind an opt-in, checked by this item's lockstep with `ddt_correction` on; (b) leave it.
   Recommendation (a); it changes no default.
3. **The default (ADR 0667 Q2, open).** Unchanged recommendation: together with F-746 in one change, the hashes re-recorded by
   the maintainer.

## For the lead: roadmap entries

> Proposed status: done.

> ADR 0672, 2026-11-16: the pressure part of ADR 0667's non-orthogonal correction on coupled faces, on a periodic channel whose cut is tilted by 0.3 (|k|/|S| 0.298 on the 32 coupled faces) under a body force across it (a tangential pressure gradient on every coupled face), against its unrolled twin (the same cells with the cut where the faces are orthogonal; every coupled face of the channel is a twin internal face with S, d and k equal to 9e-16). With PimpleSettings::coupled_non_orth the channel stays within 5.8e-15 U_ref of the twin on every one of 200 steps (limit 1e-8); with the new opt-in PimpleSettings::coupled_non_orth_without_pressure (the momentum part only, a control; default false) it misses by 1.5e-3 to 1.9e-3 U_ref on every step (its pressure differs by up to 2.8e-2 at the cut). The corrected flux is divergence-free per cell: 3.6e-15 U_ref h dz with the explicit iteration converged (limit 1e-10), and equal to the twin's to 5.9e-15 per cell at six passes (limit 1e-9). ADR 0667's uncaught mutations MN1 (pressure equation without the coupled correction) and MN4 (flux correction sign) are caught by three checks each; with MN3 and the control's own mutation four of four. Defaults bit-identical (baseline_hashes and the periodic, sliding and non-orthogonal suites pass). Translational pairs only; ddt_correction off in the runs.

> Follow-up proposed: "Pressure part of the coupled non-orthogonal correction on rotational pairs". Deps F-752. Acceptance: on a rotationally periodic sector against its unrolled full annulus under a balanced body force with the sector's period and a tangential pressure gradient across the wrap faces, ADR 0667's opt-in keeps the sector within a registered limit of the annulus on every step where the setting without its pressure part does not, and the corrected flux is divergence-free per cell within a registered limit; defaults bit-identical. Every limit not stated here is registered in the ADR before the run.

> Follow-up proposed: "Rhie-Chow ddt correction on coupled faces". Deps F-752. Acceptance: an opt-in giving the coupled (periodic) pairs the ddt correction the internal faces carry, defaults bit-identical; on ADR 0672's channel against its twin with ddt_correction on, the periodic run is within a registered limit of the twin on every step where the run without the opt-in is not. Every limit not stated here is registered in the ADR before the run.
