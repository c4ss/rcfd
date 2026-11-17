# ADR 0674: F-753 registration: pressure part of the coupled non-orthogonal correction on rotational pairs

Status: **REGISTERED 2026-11-17, written before any code of F-753 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-753 was written and no run of any kind was made; the numbers quoted
are those measured and printed in ADR 0667 and ADR 0672. Results go in the addenda at the end; nothing above the addenda is
changed after a run.

Roadmap item F-753 (deps F-752), proposed by [ADR 0672](0672-f752-registration-pressure-part-of-the-non-orthogonal-correction-on-coupled-faces.md)
remaining question 1. Parents: ADR 0672 (F-752: the twin method on a skewed translational channel, the control
`PimpleSettings::coupled_non_orth_without_pressure`, the checks P1, NC1, D1, D1L; translational pairs only, so the rotation `R`
in the pressure part's face gradient was not exercised), [ADR 0667](0667-f744-registration-non-orthogonal-correction-on-coupled-faces.md)
(F-744: `PimpleSettings::coupled_non_orth`, `rcfd_fv::pressure::CoupledNonOrth`), [ADR 0664](0664-f743-registration-implicit-momentum-coupling-of-periodic-pairs.md)
(F-743: `PimpleSettings::rotational_periodic_implicit`), [ADR 0661](0661-f725-registration-sector-against-full-annulus-with-a-non-axisymmetric-inlet.md)
(F-725: the sector against the full annulus, cell matching by nearest centre). Number 0674 given by the lead. Branch
`f753-work` from main bf533cd8 (F-743, F-744 and F-752 merged), worktree `w49i-work/f753`, scratch `w49i-work/f753-scratch`.

House rules (`w49i-work/AGENT_RULES.md`) apply: limits fixed here; each registered run made once; existing tests not edited;
defaults bit-identical; no edit of `baseline_hashes.rs`, of a hash table or of a golden; no new dependency, no unsafe code;
clean-room; the roadmap files are not edited. Every choice the maintainer would normally be asked about is taken on the
maintainer's standing instruction of 2026-11-11 (take the recommendation), except where listed as open.

Clean-room: no source outside the repository was read. The correction checked is ADR 0667's (the repository's over-relaxed
split `k = S - d |S|^2 / (S . d)` applied to a coupled pair with the partner mapped into the owner's frame); the reference is a
closed-form second mesh of the same cells (the full annulus, below); the hydrostatic balance `grad p = grad phi` for a body
acceleration `grad phi` and the superposition of a balanced and an unbalanced body force are textbook. The arithmetic below was
done by hand before any run.

## What F-753 asks

The acceptance, binding (from the lead's brief):

> On a rotationally periodic sector against its unrolled full annulus under a balanced body force with the sector's period and a tangential pressure gradient across the wrap faces, ADR 0667's opt-in keeps the sector within a registered limit of the annulus on every step where the setting without its pressure part does not, and the corrected flux is divergence-free per cell within a registered limit; defaults bit-identical. Every limit not stated here is registered in the ADR before the run.

Clauses:

- **c1** a rotationally periodic sector against its unrolled full annulus, under a balanced body force with the sector's period
  and a pressure gradient tangential to the wrap faces: with ADR 0667's opt-in the sector is within a registered limit of the
  annulus on every step where the setting without its pressure part is not;
- **c2** the corrected flux is divergence-free per cell within a registered limit;
- **c3** defaults bit-identical.

The lead's brief adds: the item must exercise the rotation `R` in the pressure part's face gradient (`w grad_r + (1 - w) R
grad_c`, `CoupledNonOrth::face_gradient`), the gap F-752 left; a mutation that drops or transposes `R` in the pressure part is
registered.

**"Tangential pressure gradient across the wrap faces" (registered reading).** As in ADR 0672 (`g_y` along the tilted cut): the
component of `grad p` along the wrap face, which is what `k . grad p` picks up (`k` of a tilted face lies nearly along the face).
The wrap faces of a sector are radial lines, so this is the radial pressure gradient at the wrap, `dp/dr` at `theta = 0` (and
its image at `theta = alpha`).

**"Unrolled full annulus" (registered).** The full annulus made of six rotated copies of the sector's cells (closed form below):
no periodic pair at all, the sector's wrap faces are the annulus's internal faces at `theta = 0` and `theta = alpha` (and their
rotations), with the same `S`, `d` and `k` (H1). ADR 0661's sliding lockstep is not used (Q1): its sliding interface carries a
rest of `1.5e-4 U_in` (ADR 0661, ADR 0667) that has nothing to do with this item.

**The control (registered, as ADR 0672).** "The setting without its pressure part" is ADR 0672's
`coupled_non_orth_without_pressure = true` with `coupled_non_orth = true` (`M`); ADR 0667's full setting `F`; the setting off
`O`; the new field alone `X` (D3).

**Reading of c1 (registered).** With `R_j` the rotation by `j alpha` about `z` (`j = 0..5`), `m_j(c)` the annulus cell nearest
`R_j x_c` (the centre of sector cell `c` rotated into copy `j`), and `A` the annulus run with the same variant's settings:

`G_V(n) = max_j max_c |R_j u_SV(n)_c - u_AF(n)_{m_j(c)}| / U_ref`

the sector run `S_V` against the annulus run with the full setting `A_F`, over all six copies (stronger than copy 0 alone; the
annulus is six-fold symmetric up to its solves). The check is `G_F(n) <= eps` on every step `n`, the control `G_M(n) > eps` on
every step (NC1, the clause's "where ... does not" made non-vacuous on every step, as ADR 0672).

**Reading of c2 (registered, as ADR 0672).** (D1, binding) the per-cell divergence of the corrected flux of the sector with the
explicit non-orthogonal iteration converged (`non_orth_correctors = 100`, 5 steps), `max_c |sum_f phi_f| / (U_ref dr dz)`;
(D1L, binding) on the lockstep (six passes, not converged on the tilted internal faces of either mesh) the sector's per-cell
divergence equals the annulus's at every matched cell of every copy. The absolute lockstep divergence is printed.

## What the code read shows (bf533cd8, read only)

1. `CoupledNonOrth::build` (pressure.rs): for each coupled face `f` of a patch with a partner map, `rot =
   geom.periodic_rot[pi]` (the partner's frame to this face's frame), `d = (x_f - c_r) + R (c_c - x_g)`, `k = S - d |S|^2 / (S .
   d)`, `w = dn_g / (dn_f + dn_g)`. `face_gradient(e, grad) = w grad[r] + (1 - w) R grad[c]` is used by `add_scalar_rhs` (the
   pressure equation's right-hand side) and `fix_flux` (its flux) only: the pressure part. The momentum part has its own rotated
   tensor (`add_vector_rhs`, `R_ki R grad u_i`). `scalar_gradient` (the partner-point least-squares gradient) puts `R` into the
   displacement `d` only (a pressure is a scalar; its partner value is unrotated). So the rotation of the pressure part's face
   gradient is reached only through `face_gradient`, and only where `grad p` at the partner differs from `R^-1` of the owner's
   frame, i.e. on every non-axisymmetric pressure or on any radial pressure gradient (a radial vector at `theta = alpha` is not
   the radial vector at `theta = 0`).
2. `pimple.rs` `step_with`: `cn_p = (s.coupled_non_orth && !s.coupled_non_orth_without_pressure).then(...)` selects
   `solve_pressure_coupled` / `corrected_flux_coupled` (ADR 0672). With no coupled face (the annulus) `CoupledNonOrth` is empty and
   `least_squares_partner` with an empty list is `least_squares` bit for bit (gradient.rs doc), so on the annulus `F` and `O`
   are expected equal bit for bit, as ADR 0667's D2 found on its annulus.
3. Terms of the step that make a rotational sector differ from the annulus whatever this item does (ADR 0661, 0664, 0672
   read 3): the lagged partner of the momentum on rotational pairs (removed by `rotational_periodic_implicit`, ADR 0664; without
   it one outer iteration leaves the lag of ADR 0661, `2e-2`); the Rhie-Chow `ddt_correction` (internal faces only, `f < n_int`);
   the body force's face coefficient on a coupled face (the owner's `rAU` unless `coupled_body_pair_mean`, ADR 0080). Everything
   else on a coupled face is the internal face's formula: the orthogonal pressure coefficient `|S| / span`, the Green-Gauss face
   value `a p_P + (1 - a) p_q` (the momentum's `grad p` is Green-Gauss, `green_gauss(&g, &self.p.values, ...)`), the rotated
   `boundary_hby_a`, the central convection, the implicit block of ADR 0664. ADR 0667 measured its sector against the annulus on
   the first step at `3.8e-15` with this set.
4. `BodyForce { cell, face }` is public: `cell` the acceleration at the cell centres, `face` `a . S_f` at every face. The
   fixed-velocity walls get the Neumann pressure gradient `bf.face / |S|` (the balanced wall condition). The body face flux
   `gamma_bf[f] * bf.face[f]` is added to `phiHbyA` on every non-empty face.
5. `rcfd_mesh::gen::annulus_sector(n_theta, n_r, r_in, r_out, angle, grading, depth)`: points at `theta = angle it / n_theta`,
   patches `inner`, `outer` (walls), `theta_min` / `theta_max` (a rotational pair by `angle` about `z` through the origin),
   `frontAndBack` (empty). `cylinder_ogrid(n_theta, n_r, r_in, r_out, grading, depth)`: points at `theta = 2 pi it / n_theta`, the
   same `(theta, radial, z)` hexahedron layout, patches `cylinder` (wall, `r_in`), `farfield` (generic, `r_out`), `frontAndBack`.
   `gen::warp` maps the points and keeps topology and pairs; `FvGeometry::new` matches the rotational pair's faces (tolerance
   `1e-6`).

No library change is needed: the setting, its control and the rotation are all there. F-753 is a test item (one new test file).

## Design

### The mesh pair (closed form)

`alpha = pi / 3`, `r_in = 1`, `r_out = 1.5`, `r_m = 1.25`, depth `dz = 0.1`, `NT = 12` cells across the sector, `NR = 8`
radially (uniform). Sector `S`: `annulus_sector(12, 8, 1, 1.5, alpha, Uniform, 0.1)`; annulus `A`: `cylinder_ogrid(72, 8, 1,
1.5, Uniform, 0.1)`. Both warped by the same map `W`:

`theta' = theta + t(theta) (r - r_m) / r_m`, `r' = r`, `z' = z`,

`t(theta) = tau cos^2(2 pi s / alpha)` for `s = theta mod alpha` in `[0, alpha / 4]` or `[3 alpha / 4, alpha]`, `t = 0`
otherwise, `tau = 0.3` (ADR 0672's tilt function with `L -> alpha`): `alpha`-periodic, continuous with a continuous slope.
Because `t` has the sector's period, `W` commutes with the rotation by `alpha`: the warped `theta_max` face is the warped
`theta_min` face rotated by `alpha` (the pair stays a rotational pair), and the warped annulus is six rotated copies of the
warped sector; the sector's points are the annulus's points with `0 <= it <= 12` (equal up to the rounding of `pi it / 36`
computed two ways and of `atan2`, `cos`, `sin`). The lines `theta = const` near the wrap become `theta' = theta_0 + tau (r -
r_m) / r_m`: in the local frame their tangential slope is `r dtheta'/dr = tau r / r_m`, so the wrap faces are tilted from radial
by `atan(tau r / r_m)`; with `d` along the local tangent (the chord between the two centres at the same `r`), `|k| / |S| = tan
beta = tau r / r_m`: `0.247` at the innermost face centre (`r = 1.031`), `0.353` at the outermost (`r = 1.469`). Validity:
`dtheta'/dtheta = 1 + t'(theta) (r - r_m) / r_m >= 1 - (2 pi tau / alpha) 0.2 = 1 - 1.8 * 0.2 = 0.64`: no cell is inverted.

`S` has `96` cells and `2 NR = 16` wrap faces; `A` has `576` cells and no coupled face.

### The flow

Incompressible, `nu = 0.1`, body acceleration `b = grad phi + F0 theta_hat` with

- the balanced part `phi = (G r_m / 6) (r / r_m)^6 cos 6 theta`, `G = 2` (ADR 0672's suggestion `r^6 cos 6 theta`, harmonic,
  with the sector's period): `grad phi = G (r / r_m)^5 (cos 6 theta r_hat - sin 6 theta theta_hat)`; at the wrap (`theta = 0`)
  purely radial, `G (r / r_m)^5` from `0.65` (`r = 1.03`) to `4.4` (`r = 1.47`): the tangential gradient on the wrap faces;
- an unbalanced driver `F0 theta_hat`, `F0 = 3.2` (uniform magnitude, axisymmetric, so it has the sector's period too): it
  drives a swirl between the no-slip walls (plane estimate of the gap centre `F0 h^2 / (8 nu) = 3.2 * 0.25 / 0.8 = 1.0 =:
  U_ref`, `h = 0.5`; the curvature `h / r_m = 0.4` moves it by some tens of percent: printed, not checked), spun up on `h^2 /
  (pi^2 nu) = 0.25` (steady at `t = 2`). Reason (Q3): without it the exact velocity is zero and the comparison would be one of
  spurious currents only; with it the momentum part of the setting acts and the swirl's centripetal gradient `u^2 / r` adds to
  the radial pressure gradient at the wrap.

The exact solution is `u = u_theta(r) theta_hat`, `p = phi + p_s(r)` (`grad phi` absorbed by the pressure): a pressure with
the sector's period and a radial gradient at the wrap on every step. `BodyForce`: `cell = b(c)`, `face = b(x_f) . S_f` (the
exact field at the face centre: equivariant, `b(R x) = R b(x)`, so the two faces of a pair carry opposite values and each is
the annulus's internal-face value). Start from rest with `p0 = phi` at the cell centres; no fixed-pressure face, so `p_ref` =
(sector cell 0, `p0` there) and (annulus cell `m_0(0)`, `p0` there). No-slip (`FixedValue` zero) on `inner`, `outer`,
`cylinder`, `farfield`.

Settings (every run): `PimpleSettings::piso(0.1)`, `Central`, `n_outer = 1`, `n_correctors = 2`, `non_orth_correctors = 6`,
`TIGHT` momentum and pressure solvers (`1e-16` abs, `1e-13` rel, 5000; as ADR 0667, 0672), `ddt_correction = false`,
`coupled_body_pair_mean = true` (read 3, kept consistent with ADR 0672: the two internal-only or owner-only terms; Q2),
`rotational_periodic_implicit = true` (read 3: without it the sector lags the annulus by ADR 0661's `2e-2` whatever the
non-orthogonal setting; on the annulus it is a no-op bit for bit, ADR 0664), `coupled_non_orth` and
`coupled_non_orth_without_pressure` by variant. `dt = 0.01`, 200 steps (`t = 2`). Courant: at most `1.0 * 0.01 / (0.64 *
0.087) = 0.18`.

Runs: `S_F` (acceptance), `S_M` (control), `S_O` (setting off, printed), `S_X` (`coupled_non_orth_without_pressure` alone,
D3), `A_F` (the reference), `A_O` (D2). For D1: `S_F` and `S_M` with `non_orth_correctors = 100`, 5 steps.

## Estimates (before any run)

Cell sizes at `r_m`: `dr = 0.0625`, `dx = r_m alpha / 12 = 0.1091`, `V = dr dx dz = 6.82e-4`; the wrap face `|S| = dr dz =
6.25e-3`. Momentum diagonal `a_P = V / dt + nu sum |S| / d = 0.0682 + 0.1 (2 * 6.25e-3 / 0.1091 + 2 * 0.01091 / 0.0625) =
0.0682 + 0.0464 = 0.1146`, `rAU = V / a_P = 5.95e-3`.

- **P1 (c1).** `S_F` against `A_F`: by ADR 0667's derivation every term of every wrap face of `S` is that of the matching
  internal face of `A` (H1: same `S`, `d`, `k`; the partner point is the neighbour cell's image; read 3 with the three
  settings), so what differs is summation order and the iterative solves (the sector's block BiCGStab of ADR 0664 against the
  annulus's component solves, different numbering): `1e-13` relative per solve, at most added over 200 steps, `2e-11`; ADR
  0667's first sector step `3.8e-15`, ADR 0672's channel `5.8e-15` over 200 steps. Limit **`eps = 1e-8`** per step (margin
  500), over all six copies.
- **NC1 (control of c1).** `S_M` lacks `s = rAU k . grad_f p` on each wrap face, one sign along the whole cut (`dp/dr > 0` at
  `theta = 0`). As ADR 0672 (measured `1.48e-3` against its estimate `1.4e-3`): the pressure of `S_M` jumps across the cut by
  `J = (|k| / |S|) (dp/dr) d = (tau r / r_m) G (r / r_m)^5 (r alpha / 12)`; the cells beside the cut get the Green-Gauss gradient
  `J / (2 dx) = tau G (r / r_m)^6 / 2` and the velocity `rAU tau G (r / r_m)^6 / 2 = 5.95e-3 * 0.3 * 2 / 2 (r / r_m)^6 = 1.8e-3
  (r / r_m)^6` at step 1: `1.8e-3` at `r_m`, `4.7e-3` at the outermost cell. Afterwards the jump persists (the swirl's `u^2 / r
  > 0` adds to `dp/dr` at the wrap). Limit **`G_M(n) >= 1e-4` on every step** (margin 18 at `r_m`, step 1).
- **P2 (supporting).** The pressure, `max_j max_c |p_SF - p_AF(m_j(c))| / (G r_m)`: rounding as P1, **`1e-8`** per step.
- **D1L (c2, lockstep).** Fluxes are at most `U_ref dr dz = 6.25e-3`; the per-cell divergences of `S_F` and `A_F` differ by the
  rounding of the solves: **`max_j max_c |div_SF(c) - div_AF(m_j(c))| / (U_ref dr dz) <= 1e-9`** each step (margin 1000).
- **D1 (c2, converged).** The explicit iteration contracts by about the largest `|k| / |S|` per pass (`0.36`, taken as at most
  `0.5` with the tilted internal faces): `0.5^100 = 8e-31`; what is left is the last solve's `1e-13` relative: **`max_c |div| /
  (U_ref dr dz) <= 1e-10`** over the 5 steps. Under MN1 the converged flux has `s` at every wrap cell: `s = 5.95e-3 * (0.3 *
  6.25e-3) * 2 = 2.2e-5` at `r_m`, `3.6e-3` of the scale (about `1e-2` at the outer wall); under MN4 twice that.
- **D2.** `A_F` against `A_O`: no coupled face (read 2): **`1e-10`** per step (bit for bit expected; printed).
- **D3 (c3).** `S_X` against `S_O`: the field is read only with `coupled_non_orth`: **bit for bit** (velocity, pressure, flux)
  on every step.
- **G0.** For each copy `j` the map `c -> m_j(c)` (nearest annulus centre to `R_j x_c`) within **`1e-12`**, one-to-one within
  each copy, the six copies disjoint and together all 576 annulus cells.
- **H1.** Exact geometry up to rounding of coordinates of order 2: the sector's velocity and pressure `CoupledNonOrth` each
  have **16** faces (the same list), every one on a rotational pair whose `R` is a rotation by `alpha` about `z` (`R_22 = 1`,
  `R_00 = R_11 = cos alpha = 0.5`, `|R_01| = sin alpha` within **`1e-12`**) and maps the partner face's centre onto the face's
  (**`1e-12`**); every one is an internal face of `A` at the same centre with the same `S`, `d`, `k` oriented from the matched
  owner (**`1e-12`** each); the largest `|k| / |S|` in **`[0.32, 0.39]`** (derived `0.353`, the neighbouring columns' tilt
  `0.933 tau` moves `d` by about `0.004`).
- **H3 (the rotation is exercised).** On `p0 = phi`, per wrap face: `rho_f = |k . (1 - w) (R - I) grad phi(c_c)| / |k . grad
  phi(x_f)|`, the share of the pressure part's face term that the rotation carries (closed-form gradient, the cell and face
  centres of `S`). With `k` nearly radial and `grad phi(c_c)` nearly radial at `theta = alpha`, `(R - I)` turns a unit radial
  vector by `60` degrees: `k . (R - I) r_hat_alpha ~ |k| (1 - cos 60) = 0.5 |k|`, `w ~ 0.5`: `rho ~ 0.25`. Limit: **the largest
  `rho_f >= 0.1`**. (Dropping `R` changes the pressure part's face term by `rho`, about a quarter of `s`, so MR1 should move
  `G_F` by about `0.25 G_M ~ 5e-4`; transposing it (`R^T`, a turn by `120` degrees from the right one) by `k . (R^T - R) ~ 1.5
  |k|`, about `0.75 s`.)
- **Cost.** `4 * 96 + 2 * 576 = 1536` cell-runs of 200 steps against ADR 0672's `3072` (66 s): about 35 s in release; D1 and
  H1/H3 a few seconds. No heavy record (if the lockstep test were to take more than 150 s, it is reported as a deviation and an
  `#[ignore]` with a 20-step fast copy is added by amendment before any further run).

## Checks (limits fixed now)

| Id | Clause | Set-up | Check | Limit |
| --- | --- | --- | --- | --- |
| P1 | c1 | lockstep, 200 steps | on every step `G_F(n) = max_j max_c abs(R_j u_SF - u_AF(m_j)) / U_ref` | `<= 1e-8` each step |
| NC1 | c1 (the control "does not") | same | on every step `G_M(n)` | `>= 1e-4` each step |
| P2 | c1 (supporting) | same | on every step `max_j max_c abs(p_SF - p_AF(m_j)) / (G r_m)` | `<= 1e-8` each step |
| D1L | c2 (lockstep) | same | on every step `max_j max_c abs(div_SF(c) - div_AF(m_j(c))) / (U_ref dr dz)` | `<= 1e-9` each step |
| D1 | c2 (converged) | `S_F`, `non_orth_correctors = 100`, 5 steps | `max_c abs(div) / (U_ref dr dz)` over the steps | `<= 1e-10` |
| D2 | set-up of the reference | lockstep | on every step `max_c abs(u_AF - u_AO) / U_ref` | `<= 1e-10` each step |
| D3 | c3 | lockstep | `S_X` against `S_O`: velocity, pressure, flux, every step | bit for bit |
| G0 | set-up | the meshes | the six maps `m_j` within `1e-12`, each one-to-one, together a bijection onto the annulus | as stated |
| H1 | set-up (rotational pairs; the annulus is the unrolled sector) | the meshes | 16 and 16 faces, the same list; `R` a rotation by `alpha` about `z` mapping the partner face centre onto the face centre; each an annulus internal face with the same centre, `S`, `d`, `k`; largest `abs(k) / abs(S)` | `1e-12` each; `[0.32, 0.39]` |
| H3 | set-up (the rotation in the pressure part matters) | `p0 = phi`, closed-form gradient | largest `rho_f` | `>= 0.1` |
| B1 | c3 | `cargo test --release`: `rcfd-api --test baseline_hashes`; `rcfd-fv --lib`; the named files `skewed_periodic_channel_f752`, `sliding_annulus_inlet_f744`, `sliding_annulus_inlet_f743`, `periodic`, `consistent_periodic_f367`, `seam_no_harm_f344`, `non_conformal`, `gradient_pressure_f291` | pass (ignored records not run) | as stated |

Printed, no limit: `G_F`, `G_M`, `G_O` at step 1, their largest and its step, the copy `j` where `G_F` is largest; the
absolute lockstep divergence of every run; D1 of `S_M`; the control's largest pressure defect; the largest swirl speed of `S_F`
at the last step; `rho_f` smallest and largest; elapsed times.

Mutations (on a scratch copy of the library made after the checks; each applied alone; the new test file run once per mutation;
each must fail the named checks):

- **MR1** the pressure part's face gradient without the rotation (`face_gradient` takes `grad[c]` unrotated): must fail P1.
- **MR2** the pressure part's face gradient with the rotation transposed (`R^T grad[c]`): must fail P1.
- **MN1** the pressure equation without the coupled correction (`add_scalar_rhs` not called): must fail P1 or D1 or D1L.
- **MN4** the coupled faces' flux correction with the opposite sign (`fix_flux` adds): must fail D1 or D1L or P1.
- **MN3** the momentum's coupled correction with the opposite sign (`add_vector_rhs`): must fail P1.
- **MN5** the control field not read (`cn_p` built whenever `coupled_non_orth`): must fail NC1.

MR1 and MR2 leave D1 met by construction (the equation and its flux take the same wrong face gradient, so the flux stays
consistent with the solved equation); they are caught, if at all, by the comparison with the annulus.

## Decision rule

**Done** when P1, NC1, D1, D1L (c1, c2), D2, D3, G0, H1, H3 and B1 are met and MR1, MR2, MN1, MN3, MN4, MN5 are caught; P2 is
supporting and reported. **Partial** when P1 and D1 are met but another check or a mutation is missed; or when P1 or D1 is
missed with its cause diagnosed. **Todo** when P1 and D1 are both missed without a diagnosis, or c3 fails (D3 or B1).

## Limits (not covered)

- One sector angle (`60` degrees), one tilt (`0.3`), one balanced potential (`cos 6 theta`), 2-D (one layer), the PISO step;
  the fractional step, the steady solver and the refused combinations of ADR 0667 are not touched.
- A static mesh: no sliding interface and no seam faces (ADR 0661's lockstep and its seam pairs are not used; ADR 0667 covers
  them for the momentum part).
- `ddt_correction` off, `coupled_body_pair_mean` on and `rotational_periodic_implicit` on in every run (Q2; the first two as ADR
  0672; the default `ddt_correction` on coupled faces is F-754).
- No case key and no default change (ADR 0667 Q2 open).

## Open questions

- **Q1. The reference.** (a) a static warped sector against the static warped full annulus (taken: an exact reference, every
  wrap face an internal face, cheap); (b) ADR 0661's sliding lockstep with a body force (the sliding rest `1.5e-4` on top, 400
  heavy steps). Taken (a) on the maintainer's standing instruction of 2026-11-11 (take the recommendation).
- **Q2. The settings that isolate the item.** (a) `ddt_correction = false`, `coupled_body_pair_mean = true` (as ADR 0672) and
  `rotational_periodic_implicit = true` in every run (taken); (b) the defaults with limits covering those terms. Taken (a) on the
  standing instruction.
- **Q3. The driver.** (a) the balanced potential plus an axisymmetric unbalanced swirl driver `F0 theta_hat` (taken: a
  non-zero reference velocity, the momentum part active, the centripetal gradient adds to the radial gradient at the wrap); (b)
  the balanced potential alone (the reference velocity is the scheme's spurious current only). Taken (a) on the standing
  instruction.

## Addendum 1: results (2026-11-17)

Measured against the limits registered above; no limit was changed and no amendment was made. Release builds on Windows, on a
machine shared with other building agents. The registration is 413589d1 (alone, first); the test file 2f1c6828 (committed
before its run). No library code was written (as registered: the setting, its control and the rotation exist). Each registered
run was made once: the new test file (3 tests, 31 s; the lockstep 27.8 s, so no heavy record), `rcfd-fv --lib`,
`baseline_hashes`, the named files. Logs in `w49i-work/f753-scratch` (`fast.log`, `lib.log`, `suites.log`, `baseline.log`,
`mutations.log`).

### Results

| Check | Limit | Measured | Verdict |
| --- | --- | --- | --- |
| P1 (c1) `G_F(n)` over the six copies, every one of 200 steps | `<= 1e-8` each step | `6.78e-15` (step 86, copy 3; step 1 `1.18e-15`) | met |
| NC1 (c1, control) `G_M(n)`, every step | `>= 1e-4` each step | smallest `2.65e-3` (step 1), largest `4.57e-3` (step 200) | held |
| P2 (supporting) pressure, every step | `<= 1e-8` each step | `2.77e-13` | met |
| D1L (c2) `abs(div_SF - div_AF) / (U_ref dr dz)`, every step | `<= 1e-9` each step | `4.65e-15` | met |
| D1 (c2) converged flux (100 passes, 5 steps) | `<= 1e-10` | `4.01e-15` | met |
| D2 `A_F` against `A_O`, every step | `<= 1e-10` each step | `0` (bit for bit) | met |
| D3 `S_X` against `S_O`, every step | bit for bit | equal on every step | met |
| G0 six cell maps | `<= 1e-12`, one-to-one, a bijection | `2.3e-15`, yes, yes | met |
| H1 wrap faces | 16 and 16, same list, 2 rotational patches; `R`, `R x_g - x_f`, centre, `S`, `d`, `k` `<= 1e-12`; largest `abs(k)/abs(S)` in `[0.32, 0.39]` | 16, 16, same, 2; `0`, `5.0e-16`, `5.6e-16`, `5.5e-17`, `1.2e-15`, `8.0e-17`; `0.3633` | met |
| H3 largest share of the rotation | `>= 0.1` | `0.701` (smallest `0.065`) | met |
| B1 `baseline_hashes`; `--lib`; the named files | pass | see below | met |

Printed, no limit: `G_O` (setting off) step 1 `2.65e-3`, equal to `G_M`'s (at step 1 the velocity is zero, so the momentum part
does nothing and the whole step-1 difference is the pressure part's), largest `3.03e-2` at step 200; the control's pressure
differs from the annulus by up to `5.0e-2`; the largest absolute lockstep divergence in `U_ref dr dz`: `S_F` and `A_F`
`8.52e-10` (equal: the six-pass mismatch of the default scheme on the tilted internal faces), `S_M`, `S_O`, `S_X` `3.87e-10`;
D1 of `S_M` `5.4e-15`; `S_F`'s largest speed at step 200 `0.995 U_ref` (estimate `1.0`). Against the estimates: NC1 `2.65e-3`
at step 1 against `1.8e-3` (at `r_m`) to `4.7e-3` (outer wall); `|k|/|S|` `0.363` against `0.353` (in the range); `rho`
largest `0.70` against about `0.25` (the estimate took `k` and the partner gradient both radial; near the walls the
`theta_hat` parts of `grad phi(c_c)` and of `k` add).

Suites (release, run once): `baseline_hashes` 3 passed, 1 ignored; `rcfd-fv --lib` 115 passed, 6 ignored;
`consistent_periodic_f367` 13 (2 ignored), `gradient_pressure_f291` 3, `non_conformal` 2, `periodic` 3, `seam_no_harm_f344` 6
(1 ignored), `skewed_periodic_channel_f752` 3, `sliding_annulus_inlet_f743` 1 (1 ignored), `sliding_annulus_inlet_f744` 2 (1
ignored), all passed; ignored records not run. No library file changed, so these confirm the tree only. `cargo fmt -p rcfd-fv
--check` clean; `cargo clippy --release -p rcfd-fv --all-targets -- -D warnings` clean.

### Mutations (scratch copy `w49i-work/f753-scratch/mut` of 2f1c6828, script `mutate.py`, target `target-mut`; the new test file run once per mutation)

| Mutation | Must fail | Result |
| --- | --- | --- |
| MR1 the pressure part's face gradient without `R` | P1 | caught: P1 `5.58e-3`, P2 `7.2e-2`, D1 `1.33e-2`, D1L `1.33e-2` |
| MR2 the pressure part's face gradient with `R^T` | P1 | caught: P1 `5.25e-3`, P2 `7.8e-2`, D1 `1.22e-2`, D1L `1.22e-2` |
| MN1 the pressure equation without the coupled correction | P1 or D1 or D1L | caught: P1 `4.96e-3`, D1 `7.33e-3`, D1L `7.37e-3`, P2 `2.1e-2` |
| MN4 the coupled faces' flux correction with the opposite sign | D1 or D1L or P1 | caught: D1 `1.49e-2`, D1L `1.49e-2`, P1 `1.77e-3` |
| MN3 the momentum correction with the opposite sign | P1 | caught: P1 `6.52e-2`, P2 `8.8e-2` (D1, D1L met) |
| MN5 the control field not read | NC1 | caught: NC1 `1.0e-15` (the control equals `F`) |

Six of six caught; H1, H3, G0, D2 and D3 held under every mutation; the library restored after each (equal to 2f1c6828).

**Deviation from a registered expectation (not a check).** The registration said MR1 and MR2 leave D1 met "by construction".
They do not: D1 and D1L are missed by `1.2e-2` to `1.3e-2`. Diagnosis (by hand from the code, no further run): the two faces of
a pair each take their own face gradient, face `f` `w grad_r + (1 - w) R grad_c` and its partner `g` `(1 - w) grad_c + w R^-1
grad_r`, with `k_g = -R^-1 k_f`; with the right `R` the two corrections are opposite (`k_g . v_g = -k_f . R v_g`), with `R`
dropped or transposed they are not, so the pair puts a net source into the singular (all-Neumann) pressure system. That
system is then incompatible; the `p_ref` row absorbs the sum, and the converged flux carries it as a per-cell divergence (at
the reference cell, by this reading; not checked). The "by construction" argument holds only for `R = I` (ADR 0672). The
verdicts are unaffected (P1, the required check, caught both); the expectation was wrong, not the checks.

### Deviations

None from the registered set-up.

### Not covered

As registered: one sector angle, one tilt, one potential, 2-D, the PISO step; a static mesh (no sliding interface, no seam
pairs); `ddt_correction` off, `coupled_body_pair_mean` and `rotational_periodic_implicit` on in every run; no case key; the
default unchanged. `docs_site` not run (only prose under `docs/`: this ADR and one sentence of `docs/dynamic-mesh.md`; left to
the daily gate).

### Decision

By the registered rule: P1, NC1, D1, D1L, D2, D3, G0, H1, H3 and B1 met, MR1, MR2, MN1, MN3, MN4 and MN5 caught: **done**.

### Remaining questions

1. **The default (ADR 0667 Q2, open).** Unchanged: `coupled_non_orth` and `rotational_periodic_implicit` stay opt-in; making
   them the default changes default periodic results and the baseline hashes, so it is the maintainer's call (recommendation:
   one change together with F-746, hashes re-recorded by the maintainer).
2. **The pressure part on the sliding seam pairs.** Options: (a) a follow-up putting this body force on ADR 0661's sliding
   lockstep; (b) leave it (the seam pairs go through the same `CoupledNonOrth` code checked here, and the sliding rest of
   `1.5e-4` would mask the effect). Recommendation (b).
3. **`ddt_correction` on coupled faces.** Already proposed as F-754 (ADR 0672); on rotational pairs the face velocity defect
   needs `R`. Recommendation: F-754 checks rotational pairs too, with this item's sector as a second case.

## For the lead: roadmap entries

> Proposed status: done.

> ADR 0674, 2026-11-17: the pressure part of ADR 0667's non-orthogonal correction on rotational pairs, with the rotation R in its face gradient exercised. A 60-degree sector whose wrap faces are tilted (|k|/|S| up to 0.363 on the 16 wrap faces) under a balanced body force grad phi, phi = (G r_m/6)(r/r_m)^6 cos 6 theta (a radial, face-tangential pressure gradient at the wrap), plus a swirl driver, against its unrolled full annulus (six rotated copies of the same cells, no coupled face; every wrap face an annulus internal face with S, d and k equal to 1.2e-15). With PimpleSettings::coupled_non_orth (and rotational_periodic_implicit) the sector stays within 6.8e-15 U_ref of the annulus, over all six copies, on every one of 200 steps (limit 1e-8); with coupled_non_orth_without_pressure (the control) it misses by 2.6e-3 to 4.6e-3 U_ref on every step. The corrected flux is divergence-free per cell: 4.0e-15 U_ref dr dz with the explicit iteration converged (limit 1e-10), and equal to the annulus's to 4.7e-15 per cell at six passes (limit 1e-9). Six of six mutations caught, including the rotation dropped (P1 5.6e-3) or transposed (5.3e-3) in the pressure part's face gradient. Tests only, no library change; defaults bit-identical (baseline_hashes and the periodic, sliding and non-orthogonal suites pass). Static mesh, 2-D, ddt_correction off in the runs.

> Note for F-754 (Rhie-Chow ddt correction on coupled faces): ADR 0674's sector against its annulus (rotational pairs, the same settings with ddt_correction off) is a second case for its check; on rotational pairs the face velocity defect needs R.
