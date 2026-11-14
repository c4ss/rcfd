# ADR 0506: F-587 registration: the B-bar Hex8 in the nonlinear driver, a variational F-bar on the total Lagrangian kinematics (J2, creep, viscoelastic, geometric nonlinearity)

Status: **REGISTERED 2026-11-14, written before any code of F-587 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-587 was written, compiled or run in the repository: the files listed under "What was
read" were read, and the arithmetic below was done by hand and evaluated with plain numpy prototypes in the scratch folder
(`w49i-work/f587-scratch`: `probe_fbar.py` (the element of D1 and D2, two choices of the reference Jacobian, central
differences, the linearisation, a distorted patch; log `probe_fbar.log`) and `probe_cyl.py` (the plane-strain cylinder on
4-node quadrilaterals, small strain, B-bar or full integration, J2 and creep; logs `probe_j2_bbar.log`, `probe_j2_full.log`,
`probe_j2_nu499_32x16.log`, `probe_j2_nu499.log`, `probe_j2_final_full.log`, `probe_creep*.log`, `probe_graded_48x24.log`);
no rcfd code; disclosed, their numbers quoted below). The bit record B0 of the default paths is a test file written and run on
the base after this commit and before any library code (D8). Results go in the addenda at the end; nothing above the addenda is
changed after a run.

Roadmap item F-587 (deps F-390, F-411, F-397), follow-up 1 of [ADR 0493](0493-f390-registration-hex8-b-bar-and-incompatible-modes.md)
(F-390). Number 0506 given by the lead. Branch `f587-work` from 3aec9cdb, worktree `w49i-work/f587`, scratch
`w49i-work/f587-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test edited
except the records-and-copies scheme of ADR 0291 (D7); no new dependency, no unsafe code, the crate graph unchanged; clean-room.
Every choice below that the maintainer would normally be asked about is taken on the maintainer's standing instruction of
2026-11-11 (take the recommendation), except the open questions at the end.

## What F-587 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. The BBar formulation of F-390 taken by the nonlinear driver: at small strain the strain Bbar u at the points and the tangent sum w Bbar^T C_t Bbar, with geometric nonlinearity the F-bar deformation gradient (det F_0 / det F)^(1/3) F; the tangent against central differences within 1e-6; the F-390 patch test within 1e-10 through the driver; the J2 thick cylinder in plane strain and the creep cylinder of ADR 0489 point-wise within limits registered there, full integration the registered locking control; the Full results of ADR 0404, 0475, 0485 and 0489 bit-identical.

The brief adds: (b1) F-bar after de Souza Neto, Peric, Dutko and Owen (1996), `F_bar = (det F_0 / det F)^(1/3) F`, `F_0` at
the element centre in the brief's wording, its consistent tangent derived here and checked against central differences; (b2)
the Incompatible formulation stays refused in the nonlinear driver, the refusal saying so and naming a follow-up; (b3) the
limits of the J2 cylinder and the creep cylinder registered here after a probe, full integration missing by a registered amount;
(b4) no case-file key (proposed in the open questions); (b5) defaults keep their bits, `baseline_hashes` unchanged, a
default-bits test at the base before any library change; (b6) every aggregated test error required finite; (b7) edits kept to
`nonlinear.rs`, `nl_transient.rs` and `hex8x.rs` (not `loads.rs`, `geometric_struct.rs`, `shell.rs`, `rotor.rs`,
`builders.rs`, `rcfd-linalg`).

Clauses: (a0) every limit registered here; (a1) the driver takes the BBar formulation, at small strain the strain `Bbar u` and
the tangent `sum w Bbar^T C_t Bbar`; (a2) with geometric nonlinearity an F-bar deformation gradient; (a3) the tangent against
central differences within `1e-6`; (a4) the F-390 patch test within `1e-10` through the driver; (a5) the J2 thick cylinder in
plane strain point-wise within registered limits, full integration the locking control; (a6) the creep cylinder of ADR 0489
point-wise within registered limits, full integration the locking control; (a7) the Full results of ADR 0404, 0475, 0485, 0489
bit-identical.

### Readings, registered

- **(a1), (a2): one driver, one kinematics.** The repository has no geometrically linear ("small-strain") Newton driver: J2
  (ADR 0475), the viscoelastic material (ADR 0485) and creep (ADR 0489) are small-strain laws applied to the Green-Lagrange
  strain and the second Piola-Kirchhoff stress of the total Lagrangian driver of ADR 0404 (large rotation, small strain; ADR
  0475 L-section, ADR 0489 arithmetic 6). The follow-up text of ADR 0493 ("the small-strain driver (J2, creep, viscoelastic) the
  Bbar strain ... and the total Lagrangian driver F-bar") assumed two drivers. Reading registered: the BBar block in the driver
  is the F-bar element of D1 on the one total Lagrangian kinematics, and the clause "at small strain" is its small-strain limit,
  checked as three properties: SS1, its tangent at `u = 0` is the F-390 B-bar stiffness `sum w Bbar^T D Bbar` (the matrix of
  `hex8x::element_stiffness(BBar)`); SS2, its strain is `Bbar u` to first order (`E_bar - Bbar u = O(|u|^2)`); SS3, with a
  material of history at small strain its tangent is `sum w Bbar^T C_t Bbar` with `C_t` the consistent tangent of the return map
  at the strain `Bbar u`, to first order. A second, geometrically linear driver is not added (open question 1).
- **(a2): which `det F_0`.** Registered: `det F_0` is the element mean of `det F`, `J_bar = (1/V) int_V det F dV` (the ratio of
  the deformed to the reference element volume, `v_e / V_e`); `F_bar = (J_bar / det F)^(1/3) F` at every point. Reasons
  (arithmetic 1 to 3, the prototype): with the internal virtual work of `F_bar` (the variational form, D2) and `F_0` at the
  element centre the F-390 patch test fails on a distorted mesh (prototype `1.7e-2`, arithmetic 2), so clause (a4) could not
  hold; the non-variational form of de Souza Neto et al. (the Cauchy stress of `F_bar` against the unbarred virtual
  displacement gradient) passes it but has an unsymmetric tangent (also for an elastic material) that the driver's symmetric
  envelope `L D L^T` cannot factor, and its tangent at `u = 0` is `sum w B^T D Bbar_0`, not the `sum w Bbar^T D Bbar` of clause
  (a1); the variational form with the element mean passes the patch test on any element, has a symmetric tangent for a
  symmetric `C_t`, and its linearisation at `u = 0` is exactly the F-390 B-bar (prototype `5.1e-16`, arithmetic 3). The centre
  form is mutation MU3 (it must be caught by the patch test). This departs from the brief's "F_0 at the element centre" and
  from the brief's "its consistent tangent is unsymmetric"; the alternative (b) of open question 2 is the centre with an
  unsymmetric solver.
- **(a3)** T1: the tangent of the library's element function (D5) against central differences of its internal force, on a
  distorted Hex8, in five states (elastic small, elastic large with a rotation, J2 plastic from a non-zero history, creep at a
  point context, viscoelastic at a point context). Measure `max |K - K_fd| / max |K|`, step `h = 1e-8` (arithmetic 6).
- **(a4)** PT1, PT2: the patch of ADR 0493 PT1 and PT2 (the box of `sb_static.rs`, seed `0xF374_0275`, interior nodes moved by
  `0.1 h_k (2U - 1)`, tag 11, and by `0.25 h_k (2U - 1)`, tag 1001; the linear field `FIELD_C + FIELD_A x` of `si_hex8x.rs`
  prescribed on the boundary nodes), every block BBar, solved by `nonlinear::solve` (one load step, the driver defaults); the
  interior error relative to the largest field component as in ADR 0493. `nu = 0.3` for both distortions and `nu = 0.4999` for
  the second (the cases of ADR 0493). The total Lagrangian solution of a homogeneous field is that field (any homogeneous `F`
  is in equilibrium without body force), so the reading is the acceptance's. Companions: PT3 the field times 100 (strains
  `0.1`, four load steps), PT4 a J2 material that yields at every point (four load steps), PT5 the creep material through
  `solve_in_time`, PT6 the viscoelastic material through `solve_in_time`; PS1 the second Piola-Kirchhoff stress of every point
  of PT2 against `D E(F)` of the homogeneous `F`.
- **(a5)** The J2 cylinder: the plane-strain thick cylinder `b/a = 2` of D6, perfectly plastic J2, `nu = 0.499` (arithmetic 4),
  loaded by the bore pressure `p(c)` of the closed form for the plastic radius `c = 1.25 a` and `1.5 a`. "Point-wise within
  limits registered there": the limit of ADR 0475 for its sphere is `1e-2` on the pressure at the plastic radius read from the
  elastic points (S1, S2 of ADR 0475); here J1 is that reading for the cylinder with that limit, and J2 is the point-wise
  stress, registered here: `max |sigma_r - closed|, |sigma_theta - closed|, |sigma_z - closed|` over every point of the
  stiffness rule, relative to `p`. J3: the yielded points lie inside `c` (the companion C1 of ADR 0475). JL: full integration
  on the same mesh, J2's measure at least `0.2` (ten times J2's limit).
- **(a6)** The creep cylinder: the set-up of ADR 0489 D6 (`E = 2e9`, `nu = 0.3`, `p = 2e5`, `n = 4`, `m = 0`, `A = 1/(E p^3)`,
  `t* = 1 s`, the 40 times to `5 t*`, `max_creep_increment = 5e-4`) on the Hex8 slice of D6, continued to `20 t*`. "The limits
  registered there": ADR 0489's clause (a5) asks Lame at the start within `1e-2` of `p` and the stationary field within `2e-2`
  of `p`; ADR 0489 read the second on element means because the fully integrated Hex20 locked (its C2r, `3.2e-2` point-wise).
  Here both point-wise: C1 at `t = 0` against Lame, `1e-2`; C2 at `5 t*` against the stationary field, `2e-2`; and the two
  quantities of F-580 (the follow-up 1 of ADR 0489): C3 at `20 t*`, `2e-2`, and C4 the growth of the point-wise error from `5
  t*` to `20 t*`, registered bound `2e-3` of `p`. CL: full integration on the same mesh, C2's measure at least `5e-2` and C3's at
  least `0.2`.
- **(a7)** B0 (D8): digests of Full results of the four ADRs' paths at the base, re-run after the code; with it the existing
  default-bits tests and every existing test of `rcfd-fem` (S1).

## What was read (no run)

- `AGENT_RULES.md`; the brief; ADR 0493 (whole: D1 to D7, the refusals of D5, the addendum and its follow-ups); ADR 0404 (the
  check table, D1 to D5); ADR 0475 (the readings, arithmetic 1 to 4, the check table); ADR 0485 (D3, D4); ADR 0489 (arithmetic
  6, D5, D6, the addendum, follow-up 1); ADR 0499 (by its code); ADR 0501 for the form.
- `crates/rcfd-fem/src/nonlinear.rs` (whole), `hex8x.rs` (whole), `nl_transient.rs` (head, `Integrator::new`, the start,
  `strain_energy`), `geometric.rs` (`element_geometric_stiffness`), `element.rs` (`Element`, `PointGeometry`, the trait entry
  points), `model.rs` (`check_elements` and the `check_*` family, the material constructors), `mat_j2.rs` (`STATE_SIZE`,
  `Update`, `update`), `mat_creep.rs` and `mat_visco.rs` (`update`, `STATE_SIZE`), `mat_ortho.rs` (`is_point_material`),
  `material.rs` (`PronyTerm`, `PointContext`), callers of the driver's assembly (`follower.rs`, `arclength.rs`).
- Tests: `si_hex8x.rs` (the patch and its RNG, the cylinder mesh, `rf1_rf2_rf3_refusals`), `si_hex8x_bits.rs`, `sa_creep.rs`
  (T2, the cylinder), `sa_creep_default_bits.rs` (the digest scheme), `sa_visco.rs` (materials).
- `docs/roadmap.json` (F-580, F-587, F-425).

## Arithmetic done before the code (disclosed)

1. **The element mean Jacobian F-bar, kinematics** (first principles; the F-bar idea of de Souza Neto, Peric, Dutko and Owen,
   "Design of simple low order finite elements for large strain analysis of nearly incompressible solids", Int. J. Solids
   Struct. 33 (1996) 3277-3296, and the mean dilatation of Nagtegaal, Parks and Rice (1974) in the finite-strain form of Simo,
   Taylor and Pister, Comput. Methods Appl. Mech. Engrg. 51 (1985) 177-208; all cited from memory, NOT re-read; every formula
   below derived here). At the points `q` of the 2 x 2 x 2 rule, reference gradients `g_a = dN_a/dX`, weights `w_q = w det J`,
   `V = sum_q w_q`: `H = sum_a u_a (x) g_a`, `F = I + H`, `J = det F`, `J_bar = sum_q w_q J_q / V`, `alpha = (J_bar / J)^(1/3)`,
   `F_bar = alpha F`, `det F_bar = J_bar` at every point. `E_bar = (F_bar^T F_bar - I)/2 = alpha^2 E + (alpha^2 - 1) I / 2` with
   `E` the Green-Lagrange strain of `F`. Variations: `d_a = F^{-T} g_a`, `delta ln J = sum_a d_a . delta u_a`; `delta ln J_bar
   = sum_q omega_q delta ln J_q`, `omega_q = w_q J_q / (V J_bar)` (`sum omega_q = 1`), so `delta ln J_bar = dbar . delta u` with
   `dbar = sum_q omega_q d_q`. With `phi = ln J_bar - ln J` (`alpha^2 = exp(2 phi / 3)`) and `G_q = dbar - d_q` (a 24-vector),
   `delta E_bar = alpha^2 (delta E + (1/3) delta phi C)`, `C = F^T F = I + 2E`; in Voigt with engineering shear `delta E_bar =
   Bbar_NL delta u`, `Bbar_NL = alpha^2 (B_NL + (1/3) c G^T)`, `B_NL` the matrix of ADR 0404 D1 and `c = (C11, C22, C33, 2 C12,
   2 C23, 2 C31)`.
2. **Force and tangent (the variational form, D2).** The internal virtual work `sum_q w_q S_bar . delta E_bar`, `S_bar` the
   material's stress at `E_bar` (Voigt, tensor shear), so `f = sum_q w_q Bbar_NL^T S_bar`; for a hyperelastic material `f` is
   the gradient of `sum_q w_q W(E_bar)`. The tangent `K = sum_q w_q (Bbar_NL^T C_t Bbar_NL + S_bar : D^2 E_bar)`,
   `C_t = dS/dE_bar` the material's consistent tangent. Second variation, with `Delta alpha^2 = (2/3) alpha^2 Delta phi`,
   `Delta C = 2 Delta E`:
   `S_bar : Delta delta E_bar = alpha^2 [ S_bar : Delta delta E + (2/3)(Delta phi (S_bar : delta E) + delta phi (S_bar :
   Delta E)) + (2/9) s_c Delta phi delta phi + (1/3) s_c Delta delta phi ]`, `s_c = S_bar : C = S_bar . c`. In matrices, with
   `s_B = B_NL^T S_bar`: the first term is the geometric stiffness of ADR 0390 D1 for the stress `alpha^2 S_bar`
   (`element_geometric_stiffness`), the others `alpha^2 [(2/3)(s_B G^T + G s_B^T) + (2/9) s_c G G^T + (1/3) s_c M_q]` with
   `M_q = D^2 ln J_bar - D^2 ln J_q`. From `D^2 ln J = -tr(F^{-1} Delta F F^{-1} delta F)`: `(D^2 ln J_q)[(a,i),(b,j)] =
   -d_b[i] d_a[j]` (written `-T_q`, `T_q` the swapped outer product); from `delta ln J_bar = sum omega_p delta ln J_p` and
   `Delta omega_p = omega_p (Delta ln J_p - Delta ln J_bar)`: `D^2 ln J_bar = sum_p omega_p (-T_p + d_p d_p^T) - dbar dbar^T`.
   So `M_q = sum_p omega_p (d_p d_p^T - T_p) - dbar dbar^T + T_q`. Every part is symmetric: the tangent is symmetric for a
   symmetric `C_t` (J2, creep and the viscoelastic material have symmetric consistent tangents). **Patch test**: for a
   homogeneous `F`, `J_q = J_bar`, `alpha = 1`, `E_bar = E`, `S_bar = S` constant, `omega_q = w_q / V`, and `sum_q w_q G_q =
   V dbar - sum_q w_q d_q = 0` per element, so `f = sum_q w_q B_NL^T S`: the force of the Full element, which vanishes at the
   interior nodes of any patch. **The centre variant** (`J_bar` replaced by `det F(0)`, `dbar` by `d(0) = F_0^{-T} g_a(0)`):
   `sum_q w_q G_q = V d(0) - sum_q w_q d_q`, which is not zero on a distorted element and does not cancel between the elements of
   a patch (`sum_e V_e g_a,e(0) != 0` at an interior node of a distorted mesh while `sum_e int g_a = 0`): an interior force
   `(1/3) s_c F^{-T} sum_e (V_e g_a,e(0) - int_e g_a)`. Prototype (`probe_fbar.log`, a 4 x 3 x 3 box with interior nodes moved by
   `0.25 h (2U - 1)`, SVK `E = 2e11`, `nu = 0.3`, `F = I + A`): the interior force relative to the largest element force
   `3.3e-13` (`|A| ~ 1e-3`) and `5.3e-15` (`|A| ~ 0.1`) for the mean, `1.7e-2` and `1.8e-2` for the centre, `3.7e-15` and
   `1.5e-15` for Full.
3. **Linearisation at u = 0.** `alpha = 1`, `S_bar = 0` (no initial stress), `c = (1, 1, 1, 0, 0, 0)`, `F = I`, so `Bbar_NL =
   B + (1/3) m (gbar_a - g_a)^T` with `gbar_a = sum_q w_q g_a(q) / V`: the F-390 B-bar row for row (ADR 0493 D1: `b[i][3a + c]
   += (mean_a[c] - g_a[c]) / 3`, `i = 0, 1, 2`), and every second-variation term carries `S_bar = 0`. Hence `K(0) = sum w Bbar^T
   D Bbar`. Prototype on a distorted element: `|K_mean(0) - K_Bbar| / |K_Bbar| = 5.1e-16`; the centre variant `2.4e-2`. Strain:
   `E_bar - Bbar u` relative to `Bbar u`: `1.2e-4` at `|u| = 1e-4` and `1.3e-6` at `|u| = 1e-6` (quadratic remainder, the ratio
   about 100).
4. **The J2 cylinder in plane strain** (first principles; the partly plastic tube under internal pressure is classical, e.g.
   Hill, The Mathematical Theory of Plasticity, 1950, chapter V, cited from memory, NOT re-read; derived here). Plane strain
   with an incompressible material (`nu = 1/2`, plastic flow isochoric): `sigma_z = (sigma_r + sigma_theta)/2` everywhere, so the
   von Mises stress is `q = (sqrt 3 / 2) |sigma_theta - sigma_r|` and yield reads `sigma_theta - sigma_r = 2k`, `k = sigma_y /
   sqrt 3`. Plastic zone `a < r < c`, perfectly plastic: `d sigma_r / dr = 2k / r`, `sigma_r = -p + 2k ln(r/a)`, `sigma_theta =
   sigma_r + 2k`. Elastic zone `c < r < b`, Lame: `sigma_r = -A (b^2/r^2 - 1)`, `sigma_theta = A (b^2/r^2 + 1)`, yield at `c`:
   `A = k c^2 / b^2`; so in the elastic zone `q = sigma_y (c/r)^2` (the reading of J1: `c_i = r_i (q_i / sigma_y)^(1/2)` at
   every point that has not yielded, `c_FE` their mean). Continuity of `sigma_r` at `c`: `p(c) = 2k ln(c/a) + k (1 - c^2/b^2)`;
   `b/a = 2`: `p(1.25) = 1.055661 k`, `p(1.5) = 1.248433 k`, first yield `0.75 k`, limit `2 ln 2 k = 1.386294 k`. **The choice
   `nu = 0.499`**: the closed form is exact at `nu = 1/2`; at `nu < 1/2` the elastic `sigma_z = 2 nu A` differs from the mean `A`
   by `(1 - 2 nu) A`, `7.4e-4 p` at `c = 1.25 a` and `nu = 0.499`, and `sigma_r`, `sigma_theta` change only at second order
   (`q` depends on the square of the difference). The elastic part is then nearly incompressible (`K/G = 499`), the plastic
   part incompressible: both lock a fully integrated element. `nu = 0.4999` was probed first: the graded 48 x 24 mesh then did
   not reach the relative residual `1e-10` at `c = 1.5 a` (stagnation at `1.14e-10`, a rounding floor of the bulk term,
   `K/G = 4999`), the driver's default tolerance; at `0.499` every step converged in at most 5 iterations. Set-up: `E = 2e11`,
   `sigma_y = 2e7` (`sigma_y / E = 1e-4`, strains of order `1e-4`, so the total Lagrangian terms are of relative order `1e-4`),
   20 equal load steps to `p(c)`.
   **Probe** (`probe_cyl.py`, small strain, plane-strain 4-node quadrilaterals with 2 x 2 points, the in-plane rule of the
   one-layer Hex8 slice with `u_z = 0` at every node, whose solution is symmetric in `z`; B-bar the mean dilatation of the
   element; consistent bore pressure on the chords; global Newton to `1e-10`). Uniform radial meshes at `nu = 0.4999`: the
   point-wise error of `sigma_r`, `sigma_theta` falls as `h` (`5.6e-2`, `2.9e-2`, `2.2e-2`, `1.6e-2` for 12, 24, 32, 48 radial
   x 16), the worst point the first point at the bore: a constant element pressure (the mean dilatation) against the
   gradient `d sigma_m / dr = 2k/r` of the plastic zone, `0.29 h` away from the element centre, `0.29 h 2k / p`, `1.7e-2` at
   `h = 1/32`. Graded radially as the cylinder of ADR 0493 (`r = a + (b - a)(e^(2s) - 1)/(e^2 - 1)`), `nu = 0.499`: 32 x 16:
   `1.52e-2`, `1.70e-2`; **48 x 24 (registered): J2 measure `9.71e-3` (`c = 1.25 a`) and `1.07e-2` (`1.5 a`), `sigma_z`
   `5.16e-3`, `5.62e-3`; `|p(c_FE) - p| / p` `5.7e-4`, `4.2e-4`; no misplaced point; at most 5 iterations**. Full integration
   on the same mesh: `1.84` and `2.21` (point-wise; the pressure oscillates), `|p(c_FE) - p| / p` `1.1e-2`, `1.3e-2` (the
   deviatoric field of the elastic zone is nearly right). The limits: J1 `1e-2` (ADR 0475's, probe 20 times below); J2 `2e-2`
   (the `h` estimate `1.0e-2` at the bore element size `0.0065` of the graded mesh, the probe `1.07e-2`, margin 1.9; the total
   Lagrangian terms `1e-4`, the `nu` effect `7.4e-4` on `sigma_z`); JL `>= 0.2` (probe 9 times above).
5. **The creep cylinder on Hex8** (ADR 0489 arithmetic 6 for the closed forms). Probe as item 4 with the backward Euler creep
   return of ADR 0489 (`m = 0`), the 40 times of ADR 0489 to `5 t*` then 30 steps of `0.5 t*` to `20 t*`. Uniform meshes (`t =
   0`, point-wise Lame): `2.3e-2` (24 x 16), `1.36e-2` (48 x 16), `1.10e-2` (64 x 16), `8.6e-3` (64 x 32), `6.1e-3` (96 x 32);
   the worst point near the bore (`O(h)`: constant radial strain across a linear element against the radial gradient of the
   Lame stress, `8p/3` per unit radius at `r = a`). Graded: 24 x 16 `1.11e-2`, 32 x 16 `8.9e-3`, 32 x 32 `7.2e-3`, 48 x 16
   `6.8e-3`, **48 x 24 (registered, the J2 mesh): `5.41e-3` at `t = 0`; `9.47e-3` at `5 t*`, `9.48e-3` at `10, 15, 20 t*`
   (growth `7.5e-6`), deviatoric `2.9e-3`; the largest equivalent creep increment of a step `1.32e-4` to `5 t*`, `2.38e-4` after
   (below `5e-4`, so no halving is expected)**. Full integration on the same mesh: `1.27e-2` at `t = 0`, `0.103` at `5 t*`,
   `0.190`, `0.276`, `0.362` at `10, 15, 20 t*` (the point pressure drifts linearly, the locking of ADR 0489 arithmetic 6),
   deviatoric `3.6e-3` to `5.5e-3`. The limits: C1 `1e-2` (ADR 0489's, probe `5.4e-3`, margin 1.8); C2, C3 `2e-2` (margin 2.1);
   C4 `2e-3` (F-580's "registered bound"; the probe's growth `7.5e-6`; the bound is the size of a drift that a locking element
   shows within `0.3 t*`, see Full); CL `>= 5e-2` at `5 t*` (probe `0.103`) and `>= 0.2` at `20 t*` (probe `0.362`).
6. **Central differences (T1).** Truncation `O((h/l)^2)` with `l` the scale on which the tangent changes (the element size for
   the geometric terms, the yield or creep strain scale `1e-3` for the return maps), rounding `eps |f| / (h |K|) ~ eps |u| / h`.
   At `h = 1e-8`: `|u| ~ 1e-3` gives truncation `1e-10`, rounding `1e-11`; `|u| ~ 1` (the rotated state) truncation `1e-16`,
   rounding `1e-8`. Prototype (`probe_fbar.log`, numpy, `h = 1e-9` and `1e-7`): `1.6e-7` (`|u| ~ 1e-3`; the prototype forms
   `alpha^2 - 1` by subtraction, D3 does not) and `1.5e-9` (`|u| ~ 5e-2`), the centre variant the same; asymmetry
   `<= 1.7e-16`. Limit `1e-6`, the acceptance's.
7. **Small strain (SS3).** With J2 at small strain the driver's tangent differs from `sum w Bbar^T C_t(Bbar u) Bbar` by
   (i) `C_t` at `E_bar` instead of `Bbar u`, relative `|u|` times the condition of the return map, (ii) the geometric and F-bar
   second-variation terms, relative `|S| / E`, (iii) `Bbar_NL - Bbar`, relative `|u|`. Set-up: `sigma_y = 2e4` (yield strain
   `1e-7`), `|u| ~ 1e-6` on a unit element: each part about `1e-6`; limit `1e-4`.
8. **Default bits.** The Full arm of every function keeps its operations: the element loop of `evaluate` is not changed, the
   new arm is taken only for an element whose block is BBar (`hex8x::opted`), and the only changed condition of
   `nonlinear::check_model` is on a non-Full solid block. `nl_transient::strain_energy` gains an arm for an opted element only.

## Design

### D1. The element (`nonlinear.rs`, private `evaluate_fbar`)

For a Hex8 element whose block is `BBar`: the points of the stiffness rule (`gauss_2`, as `evaluate`), per point `g_a`,
`w_q`, `H`, `F`, `J - 1 = tr H + i2(H) + det H` (`i2 = ((tr H)^2 - tr H^2)/2`, without forming `det F - 1` by subtraction),
`J_bar - 1 = sum_q w_q (J_q - 1) / V`, `phi = ln_1p(J_bar - 1) - ln_1p(J - 1)`, `alpha^2 - 1 = expm1(2 phi / 3)`, `E` by
`green_lagrange(H)` (ADR 0404 amendment 1), `E_bar = alpha^2 E + (alpha^2 - 1) I/2` written `E + (alpha^2 - 1)(E + I/2)`. A
point with `J <= 0` (or not finite) is refused with the key `"deformation gradient"` as in `evaluate`. The material update of
the point at `E_bar` from the committed state is that of `evaluate` (the same match: J2, viscoelastic with states, creep, the
others by `Material::stress`); the new state is kept as in `evaluate`. Force and tangent by arithmetic 1 and 2; the geometric
part by `element_geometric_stiffness(Hex8, coords, alpha_q^2 S_bar_q)`. The stress reported per point is `S_bar`.

### D2. Dispatch (`nonlinear.rs`)

`internal_force_at` and `force_and_tangent` evaluate an element with `hex8x::opted(mesh, e) == Some(BBar)` by D1, every other
element as now. `check_model`: a solid block of the `BBar` formulation is taken; a block of the `Incompatible` formulation is
refused with `FemError::InvalidValue { key: "nonlinear" }` (the key of ADR 0404, kept), its reason naming the block, that the
nonlinear driver takes the `Full` and `BBar` formulations (ADR 0506), and the follow-up `hex8x::NONLINEAR_FOLLOW_UP = "a
follow-up of ADR 0506"` (enhanced strains at finite strain need their own derivation: the incompatible modes of Simo and Armero
are not a modification of `F` that the material sees); every other non-Full block keeps the reason of now. Every caller of the
driver's assembly (`solve`, `solve_with`, `solve_from_at`, `solve_in_time_with`, the arc-length step, the follower tangent,
`nl_transient`) takes BBar blocks through these two functions.

### D3. `nl_transient.rs`

`Integrator::strain_energy` takes the strain of an opted element from D5's `hex8_strains` (the F-bar strain), every other
element as now. The mass is the consistent mass (formulation-independent), the damping tangent `K_0` is `nonlinear::tangent` at
`u = 0` (for a BBar block the F-390 B-bar stiffness, arithmetic 3).

### D4. `hex8x.rs`

Module documentation updated (the nonlinear driver takes BBar by ADR 0506); `pub const NONLINEAR_FOLLOW_UP: &str = "a follow-up
of ADR 0506"`. No change of arithmetic.

### D5. Element entry points (`nonlinear.rs`, public)

For a Hex8 of a given formulation (`Full`: the functions of ADR 0404 with `element_for(Hex8)`; `BBar`: D1; `Incompatible`:
refused as D2, key `"nonlinear"`): `hex8_force_and_tangent_at(formulation, coords, material, displacement, state, context)`,
`hex8_internal_force_at(...)` (force only), `hex8_second_piola_kirchhoff(formulation, coords, material, displacement, state)`
(the stress at the points, default context), `hex8_strains(formulation, coords, displacement)` (the Green-Lagrange strain of `F`
or of `F_bar` at the points, Voigt, engineering shear).

### D6. The cylinders (J1 to J3, JL, C1 to C4, CL)

Mesh, registered: `hex_box([0, 0, 0], [1, 1, 0.02], [48, 24, 1])`, every node `(s, t, z)` mapped to `(r cos theta, r sin theta,
z)`, `r = a + (b - a)(e^(2s) - 1)/(e^2 - 1)`, `theta = (pi/2) t`, `a = 1`, `b = 2` (the map of `si_hex8x.rs`), node and surface
sets kept; `u_y = 0` on `ymin`, `u_x = 0` on `ymax`, `u_z = 0` at every node (plane strain); `Load::pressure("xmin", p)`. Block 0
`BBar` or (control) the default `Full`. Stress at a point: `hex8_second_piola_kirchhoff` at the committed displacement and
states, turned to `sigma_r`, `sigma_theta` by the angle of the point's reference position, `sigma_z = S_zz`; compared with the
closed forms at the point's reference radius. The driver's Newton defaults, `threads(4)`.

- J2: `Material::j2(isotropic_elastic(2e11, 0.499, 7850, 0), 2e7, 0, 0)`; `nonlinear::solve`, 20 load steps, one solve per
  `c`. J1: `c_FE` from every point whose equivalent plastic strain (the last state value of `mat_j2`) is exactly 0, `|p(c_FE) -
  p| / p`. J2: the point-wise maximum over `sigma_r`, `sigma_theta`, `sigma_z`, relative to `p`. J3: no point with `r < c -
  0.05` unyielded and none with `r > c + 0.05` yielded.
- Creep: `Material::creep(isotropic_elastic(2e9, 0.3, 1200, 0), 1/(2e9 (2e5)^3), 4, 0)`, `p = 2e5`; `nonlinear::solve_in_time`
  with the 40 times of `sa_creep.rs` (the 39 of the formula and `5`) and `5 + 0.5 k`, `k = 1 .. 30`;
  `TimeSteppingOptions::new(5e-4)` with `threads(4)`. C1 at `t = 0` against Lame; C2 at `t = 5`, C3 at `t = 20` against the
  stationary field; C4 `|e(20) - e(5)|`. Halvings and the largest creep increment reported.

### D7. Records and copies

`si_hex8x.rs::rf1_rf2_rf3_refusals` asserts that `nonlinear::internal_force` and `nonlinear::tangent` refuse a BBar model; it
fails by design after D2. It gets `#[ignore = "record of ADR 0493 RF3: the nonlinear rows for BBar retired by ADR 0506; copy in
si_fbar.rs"]` (the scheme of ADR 0291 and ADR 0493), and `si_fbar.rs` carries the copy `rf1_rf2_rf3_refusals_after_f587`,
identical except that the two BBar nonlinear rows assert `Ok`. No other existing test is edited.

### D8. The bit record B0 (`crates/rcfd-fem/tests/si_fbar_default_bits.rs`)

Written after this commit and run on the base 3aec9cdb before any library code; FNV-1a 64 digests of the bits, the expected
values printed by that run and written into the file then. Content (Full Hex8 throughout, API of the base only): `nonlinear::solve`
of a distorted Hex8 box stretched and rotated (ADR 0404's path, SVK); `nonlinear::solve` of a Hex8 J2 bar and of the J2
cylinder of D6 on a 12 x 6 mesh (ADR 0475's path); `solve_in_time` of the creep cylinder on 12 x 6 to `t = 1` (ADR 0489's
path); `solve_in_time` and `solve_from_at` of a viscoelastic Hex8 cube and `nl_transient` of a viscoelastic Hex8 bar with its
strain energy (ADR 0485, 0499 paths); `internal_force`, `tangent` of a distorted J2 Hex8 model; `element_force_and_tangent_at`,
`element_second_piola_kirchhoff`, `element_trial_state` of a distorted Hex8 with J2 and with creep.

## Checks

Every measure is required finite before it is compared (a non-finite value misses).

| Id | Clause | Measure | Limit | Estimate |
|---|---|---|---|---|
| SS1 | a1 | the PT2 patch model (BBar, `nu = 0.3`) and a 4 x 2 x 2 box: `nonlinear::tangent` at `u = 0` against `assemble::stiffness` (F-390 B-bar), `max |K_T - K| / max |K|` | `<= 1e-13` | `5.1e-16` |
| SS2 | a1 | distorted Hex8 (T1's), random `u` of amplitude `1e-6` and `1e-4`: `max_q |E_bar - Bbar u| / max |Bbar u|` (`hex8_strains` against `hex8x::point_strains(BBar)`) | `<= 1e-5` and `<= 1e-3`; the ratio of the two in `[30, 300]` | `1.3e-6`, `1.2e-4`, 92 |
| SS3 | a1 | T1's element, J2 (`sigma_y = 2e4`, `H_iso = 2e9`, `H_kin = 1e9`) from the zero state, `u` of amplitude `1e-6`: the element tangent against `sum w Bbar^T C_t Bbar`, `C_t` of `mat_j2::update` at `Bbar u`; the force against `sum w Bbar^T sigma(Bbar u)` | `<= 1e-4` each (relative to the largest entry); set-up: every point plastic | `~1e-6` |
| T1 | a3 | distorted Hex8 (seed `0xF587_0506`, corners moved by `0.1 (2U - 1)` on the unit cube): `max |K - K_fd| / max |K|`, `h = 1e-8`, states (a) SVK `u` amplitude `1e-3`; (b) SVK, rotation by 90 degrees about `z` plus `u` amplitude `5e-2`; (c) J2 (`2e8`, `2e9`, `1e9`) from a committed state (plastic strains `2e-4`, back stresses `1e7`, `alpha = 3e-4` at random), `u` amplitude `3e-3`, set-up every point plastic; (d) creep `(1e-25, 4, -0.5)` with committed creep strains `2e-4`, `eps_c,eq = 3e-4`, context `(t, dt) = (2, 0.5)`, set-up every point creeping; (e) viscoelastic, the unequal terms of `sa_visco.rs`, zero state, context `(0.01, 0.01)` | `<= 1e-6` each | `<= 1e-8` |
| T2 | a3 | T1 (a) to (e): `max |K - K^T| / max |K|` | `<= 1e-12` | `1.7e-16` |
| PT1 | a4 | ADR 0493 PT1 patch (0.1 h, tag 11, `nu = 0.3`), BBar, `nonlinear::solve` one step: interior error | `<= 1e-10` | `<= 1e-12` |
| PT2 | a4 | ADR 0493 PT2 patch (0.25 h, tag 1001), `nu = 0.3` and `0.4999` | `<= 1e-10` each | `<= 1e-12` |
| PT3 | a4 | PT2 (`nu = 0.3`) with `100 FIELD_A`, four load steps | `<= 1e-10` | `<= 1e-12` |
| PT4 | a4 | PT2 (`nu = 0.3`) with J2 (`sigma_y = 1e8`, `2e9`, `1e9`), four load steps; set-up every point yielded | `<= 1e-10` | `<= 1e-12` |
| PT5 | a4 | PT2 (`nu = 0.3`) with creep (`A = 1e-25`, `n = 4`, `m = 0`, base `2e11`, `0.3`), `solve_in_time` to `[1, 2]`, interior error at each step end; set-up creep strain `> 0` at every point at `t = 2` | `<= 1e-10` each | `<= 1e-12` |
| PT6 | a4 | PT2 (`nu = 0.3`) with the unequal viscoelastic material, `solve_in_time` to `[0.01, 0.1, 1]` | `<= 1e-10` each | `<= 1e-12` |
| PS1 | a4 | PT2 (`nu = 0.3`): `hex8_second_piola_kirchhoff` at every point against `D E(F)`, `F = I + FIELD_A`, relative to the largest | `<= 1e-10` | `<= 1e-13` |
| J1 | a5 | D6 J2 cylinder BBar, `c = 1.25 a` and `1.5 a`: `|p(c_FE) - p| / p` | `<= 1e-2` each | `5.7e-4`, `4.2e-4` |
| J2 | a5 | the same: point-wise `max(|sigma_r - .|, |sigma_theta - .|, |sigma_z - .|) / p` | `<= 2e-2` each | `9.7e-3`, `1.07e-2` |
| J3 | a5 | the same: misplaced points (yielded outside `c + 0.05 a`, unyielded inside `c - 0.05 a`) | `0` | `0` |
| C1 | a6 | D6 creep cylinder BBar, `t = 0`: point-wise `max(|sigma_r - Lame|, |sigma_theta - Lame|) / p` | `<= 1e-2` | `5.4e-3` |
| C2 | a6 | `t = 5 t*`: point-wise against the stationary field | `<= 2e-2` | `9.5e-3` |
| C3 | a6 | `t = 20 t*`: the same | `<= 2e-2` | `9.5e-3` |
| C4 | a6 | `|e(20 t*) - e(5 t*)|` of C2, C3's measure | `<= 2e-3` | `7.5e-6` |
| NT1 | D3 | a BBar Hex8 bar (4 x 1 x 1, `xmin` clamped) under `nl_transient` (average acceleration, `dt = 1e-5`, a tip load history, 3 steps): `strain_energy` against `sum_q w_q S . E_bar / 2` from `hex8_strains` and the material in the test | `<= 1e-12` relative | rounding |
| RF1 | b2 | an Incompatible block: `nonlinear::solve`, `solve_in_time`, `internal_force`, `tangent`, `nl_transient::Integrator::new` refused with `InvalidValue { key: "nonlinear" }` whose reason names `"ADR 0506"` and `NONLINEAR_FOLLOW_UP`; `hex8_force_and_tangent_at(Incompatible, ..)` the same | exact | D2 |
| RF2 | D7 | `rf1_rf2_rf3_refusals_after_f587` (the copy) | passes | D7 |
| B0 | a7 | `si_fbar_default_bits.rs`: every digest equal to the base's | bit-equal | D8 |
| S1 | a7 | `cargo test -p rcfd-fem --release` (with the existing default-bits tests and the tests of ADR 0404, 0475, 0485, 0489, 0493, 0499) | 0 failed | |
| S2 | b5 | `cargo test -p rcfd-api --release --test baseline_hashes`; `cargo test -p rcfd-cli --release --test docs_site` | pass | |
| S3 | | `cargo fmt -p rcfd-fem --check`, `cargo clippy -p rcfd-fem --all-targets -- -D warnings` | clean | |

### Negative controls (the locking controls of the acceptance)

| Id | Control | Expected |
|---|---|---|
| JL | J2's measure with full integration on the same mesh, `c = 1.25 a` and `1.5 a` | `>= 0.2` each (probe `1.84`, `2.21`) |
| CL | C2's and C3's measures with full integration on the same mesh | `>= 5e-2` at `5 t*` (probe `0.103`), `>= 0.2` at `20 t*` (probe `0.362`) |
| N3 | SS1 with the Full block (`nonlinear::tangent` at `u = 0` of the Full model against the B-bar stiffness) | `>= 1e-3` (the measure sees the formulation) |

### Planned mutations (on a scratch copy, each run once against `si_fbar.rs`)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | the term `(1/3) s_c M_q` dropped from the tangent | T1 (b, c) |
| MU2 | the cross terms `(2/3)(s_B G^T + G s_B^T)` dropped | T1 (b, c) |
| MU3 | `J_bar` and `dbar` taken at the element centre (`det F(0)`, `F(0)^{-T} g_a(0)`) | PT1, PT2, SS1 |
| MU4 | the factor `1/3` of `Bbar_NL` replaced by `1/2` | SS1, T1 |
| MU5 | the dispatch of D2 dropped (a BBar element evaluated as Full) | SS1, J2, C2 |
| MU6 | the geometric part with `S_bar` instead of `alpha^2 S_bar` | T1 (b) |
| MU7 | `E_bar = alpha^2 E` (the term `(alpha^2 - 1) I/2` dropped) | T1, J2 |
| MU8 | `check_model` takes Incompatible (evaluated as BBar) | RF1 |
| MU9 | `strain_energy` with the strain of `F` for a BBar element | NT1 |

## Decision rule

- **done**: every check above met in its single registered run (SS1 to SS3, T1, T2, PT1 to PT6, PS1, J1 to J3, C1 to C4,
  NT1, RF1, RF2, B0, S1 to S3), the negative controls JL, CL, N3 as stated, every planned mutation caught by a registered
  check, `baseline_hashes` unchanged.
- **partial**: the code lands with B0, S1 and every check of (a1) to (a4) met, but a check of (a5) or (a6) is missed
  (recorded as a `*_known_miss` test with the limit intact, diagnosed), or a control does not behave as stated, or a mutation is
  not caught.
- **todo**: B0 or S1 fails and cannot be fixed without changing a default result, or a check of (a1) to (a4) is missed.

## Limits (not covered)

- One kinematics: no geometrically linear driver (open question 1). The materials remain small-strain laws on `(E_bar,
  S_bar)`: large rotation, small strain (ADR 0475, ADR 0489); no multiplicative plasticity.
- The variational F-bar with the element mean Jacobian, not the centre form of de Souza Neto et al. (open question 2).
- Hex8 only (the BBar formulation is a Hex8 choice, ADR 0493 D2). Incompatible stays refused in the nonlinear driver (b2).
- The arc-length step, the follower contributions and contact take BBar blocks through the assembly of D2 without checks of
  their own here; the corotated shells are unchanged.
- The paths refused by ADR 0493 D5 (temperature loads, per-point materials, the linearised geometric stiffness, the damped and
  direct harmonic solves, the patch recovery) stay refused; the nonlinear driver refuses temperature loads and per-point
  materials for every formulation as before.
- The cylinders are plane strain by a one-layer slice; the point-wise errors are those of a linear element (`O(h)` at the
  points of the rule), hence the graded 48 x 24 mesh.

## Open questions (with the recommendation)

1. **A geometrically linear (small-strain) kinematics in the driver** (`E = sym grad u`, no geometric stiffness), opt-in by a
   field of `NewtonOptions`: (a) not added (taken here: one kinematics, the small-strain clause read as the limit); (b) a
   follow-up item with its own checks (the small-strain J2 and creep closed forms to `1e-10`, the driver's order).
   Recommend (a) until a model needs it; the total Lagrangian driver reproduces small-strain results to the order of the strain.
2. **`det F_0` at the centre** (the brief): (a) the element mean (taken; patch test, symmetric tangent, the F-390 B-bar at
   `u = 0`); (b) the centre with the non-variational form and an unsymmetric tangent, which needs an unsymmetric factorisation
   in the driver (a change of the shared driver and of its pivot tests). Recommend (a).
3. **A case-file key** for the formulation of a nonlinear solid block (b4): not added. Recommend the key of ADR 0493's open
   question (the block property `formulation = "bbar"`) to cover the nonlinear paths once the maintainer adds it; no separate
   key for the nonlinear driver.
4. **F-580** (creep on the B-bar Hex8, point-wise, growth between `5 t*` and `20 t*`): C2 to C4 are its acceptance on this
   driver. Recommend that the lead mark F-580 by ADR 0506 if they are met (the note proposed in the addendum).

## Amendment 1 (2026-11-14, written after the first registered run of `si_fbar.rs` and before any corrected run)

**What happened.** In the single registered run (log `f587-scratch/run1-si_fbar.log`) the test
`pt1_to_pt6_ps1_the_patch_through_the_driver` ran PT1 to PT4 and PS1 (all met, numbers in addendum 1) and then stopped at PT5
with an error of the driver: `solve_in_time` refused step 1 after 5 halvings, "not converged in 20 iterations (relative
residual 3.0e-16, energy ratio 2.8e-1)". PT6 was not reached.

**Diagnosis: a defective set-up, not the element.** The boundary values of PT5 are constant in time and the creep flow of a
homogeneous field is homogeneous, so at the start of every time step the interior is already in equilibrium: the first
correction of the attempt is rounding noise and so is every later one, and the driver's energy criterion (the last correction's
energy at most `1e-10` of the first, ADR 0404 D5) compares two rounding-level numbers and cannot be met although the residual is
`3e-16`. The same holds for every formulation (the Full element gives the same homogeneous field) and for PT6. The set-up gives
the driver no work to do; it does not test the patch.

**Corrected set-up, registered now.** PT5' and PT6' prescribe the boundary field through a displacement history of
`solve_in_time_with` (ADR 0499 D1): every boundary node component `u_k(x) = (FIELD_C + FIELD_A x)_k` as a `MotionHistory` with
the factor `s(t)` linear from `s(0) = 1` to `s(2) = 2` (PT5', the creep material and times `[1, 2]`) and from `s(0) = 1` to
`s(1) = 2` (PT6', the viscoelastic material and times `[0.01, 0.1, 1]`); the interior error at each step end against `s(t_k)`
times the linear field, relative to the largest component of that scaled field (the measure of PT1). Each step then starts
from the interior of the previous step end with new boundary values, a correction of order the field's increment. Limits
unchanged: `1e-10` each; the set-up check of PT5 unchanged (creep strain `> 0` at every point at `t = 2`). The rows met in the
first run (PT1 to PT4, PS1) continue unchanged in `pt1_to_pt4_ps1_met_rows`; the test of the first run stays as the ignored
record of that run.

**The known miss C4 is not amended.** C4 (the growth of the point-wise error from `5 t*` to `20 t*`, limit `2e-3`) was
measured at `6.02e-3`: a miss of a registered limit, recorded as `c1_to_c4_cl_creep_cylinder_known_miss` with the limit intact;
the met rows of that run (C1 to C3, CL) continue in `c1_to_c3_cl_creep_cylinder_met_rows`. Its diagnosis (addendum 1) uses one
diagnostic run registered here, without a limit: the same cylinder with `p / 10` and `A = 1/(E (p/10)^3)` (so `t* = 1 s` and
every stress over `p` unchanged in small strain, every strain ten times smaller), the point-wise error over `p` printed at `5,
7.5, ..., 20 t*` with the radius and angle of the worst point; expected, if the growth is the total Lagrangian geometry change
against the small-strain closed form, a growth about ten times smaller.

## Addendum 1: results (2026-11-14)

Code in commit 669d264c (`nonlinear.rs`, `nl_transient.rs`, `hex8x.rs`, the new `tests/si_fbar.rs` and the ignore of the
record `rf1_rf2_rf3_refusals` in `tests/si_hex8x.rs`, D7). Logs in `w49i-work/f587-scratch`: `run1-si_fbar.log` (the
registered run), `run2-amend1.log` (PT5', PT6' of amendment 1), `run3-c4-diagnosis.log` (the diagnostic run of amendment 1),
`s1-rcfd-fem.log`, `s2-baseline.log`, `s2-docs.log`, `s3-fmt.log`, `s3-clippy.log`, `mutations.log` and `mut-MU1.log` to
`mut-MU9.log`.

| Id | Measured | Limit | Result |
|---|---|---|---|
| SS1 | `2.65e-16` (patch), `3.92e-16` (box) | `<= 1e-13` | met |
| SS2 | `7.58e-7`, `6.80e-5`, ratio `89.7` | `<= 1e-5`, `<= 1e-3`, `[30, 300]` | met |
| SS3 | tangent `2.06e-6`, force `1.07e-5`; set-up 8 of 8 points plastic | `<= 1e-4` each | met |
| T1 | (a) `2.53e-11`, (b) `1.10e-8`, (c) `9.72e-11` (8 of 8 plastic), (d) `8.77e-11` (8 of 8 creeping), (e) `3.14e-11` | `<= 1e-6` each | met |
| T2 | `<= 2.4e-16` for (a) to (e) | `<= 1e-12` | met |
| PT1 | `1.75e-16` | `<= 1e-10` | met |
| PT2 | `1.75e-16` (`nu = 0.3`), `4.63e-15` (`nu = 0.4999`) | `<= 1e-10` each | met |
| PT3 | `1.73e-16` | `<= 1e-10` | met |
| PT4 | `1.75e-16`; every point yielded | `<= 1e-10` | met |
| PT5 | run 1: the driver refused step 1 (set-up defect, amendment 1) | | not measured |
| PT5' | `8.87e-12` (`t = 1`), `1.61e-12` (`t = 2`); creep strain `> 0` at every point | `<= 1e-10` each | met |
| PT6 | run 1: not reached | | not measured |
| PT6' | `1.73e-16`, `1.59e-16`, `6.12e-16` | `<= 1e-10` each | met |
| PS1 | `1.63e-15` | `<= 1e-10` | met |
| J1 | `6.02e-4` (`1.25 a`), `4.50e-4` (`1.5 a`); 20 steps, no halving, at most 5 iterations | `<= 1e-2` each | met |
| J2 | `9.99e-3`, `1.107e-2` | `<= 2e-2` each | met |
| J3 | `0`, `0` | `0` | met |
| C1 | `5.37e-3` | `<= 1e-2` | met |
| C2 | `9.54e-3` | `<= 2e-2` | met |
| C3 | `1.556e-2` | `<= 2e-2` | met |
| C4 | `6.02e-3` | `<= 2e-3` | **missed** (`c1_to_c4_cl_creep_cylinder_known_miss`) |
| NT1 | `0.0` | `<= 1e-12` | met |
| RF1 | the six refusals with key `"nonlinear"`, naming ADR 0506 and `NONLINEAR_FOLLOW_UP` | exact | met |
| RF2 | `rf1_rf2_rf3_refusals_after_f587` passes | passes | met |
| B0 | the 11 digests equal to the base's (after the code) | bit-equal | met |
| S1 | `cargo test -p rcfd-fem --release`: 167 test binaries, 758 passed, 0 failed, 197 ignored | 0 failed | met |
| S2 | `baseline_hashes` 3 passed (1 ignored, as on the base); `docs_site` 2 passed | pass | met |
| S3 | `cargo fmt -p rcfd-fem --check` clean; `cargo clippy -p rcfd-fem --all-targets -- -D warnings` clean | clean | met |

Reported without a limit: the creep cylinder's point-wise error at `10 t*` `9.75e-3` and at `15 t*` `1.15e-2`; at most
`2.38e-4` of creep increment per step, no halving; the Full cylinder's J1 `1.11e-2` and `1.30e-2`.

Negative controls: JL `1.84`, `2.21` (`>= 0.2`), as stated; CL `0.1025` at `5 t*` (`>= 5e-2`) and `0.364` at `20 t*`
(`>= 0.2`), as stated; N3 `0.250`, `0.208` (`>= 1e-3`), as stated.

### Mutations (scratch copy `f587-scratch/mut`, each run once against `si_fbar.rs`, the copy restored and compared after)

| Id | Caught by (the registered checks that missed) | Planned catchers |
|---|---|---|
| MU1 | T1 (a) `1.0e-4`, (b) `1.6e-2`, (c) `2.7e-4`, (d) `5.3e-4`, (e) `1.4e-4`; PT5' `4.3e-9` | T1 (b, c): caught |
| MU2 | T1 (a) to (e), (b) `2.0e-2`; PT5' `3.6e-9` | T1 (b, c): caught |
| MU3 | PT1 `1.9e-4`, PT2 to PT4, PT5' `9.7e-2`, PT6' `1.8e-3`, SS1, SS2, SS3, T1 | PT1, PT2, SS1: caught |
| MU4 | SS1 `6.3e-2`, `5.2e-2`; SS3 `0.18`; T1 (a) `5.8e-2`; PT5'; J1 to J3 | SS1, T1: caught |
| MU5 | SS1 `0.250`, `0.208`; J1 `1.11e-2`, `1.30e-2`; J2 `2.76e-2`, `4.10e-2` | SS1, J2 caught; **C2 not** |
| MU6 | T1 (b) `1.35e-3`, (c) `1.7e-6`, (d) `1.04e-6` | T1 (b): caught |
| MU7 | SS2 `0.19`, `0.15`, ratio `0.75`; T1; PT5' `3.6e-9`; J1 to J3; C1 to C3 | T1, J2: caught |
| MU8 | RF1 (all six taken), RF2 (`internal_force`, `tangent` taken) | RF1: caught |
| MU9 | NT1 `3.1e-5` | NT1: caught |

Every planned mutation is caught by a registered check. One planned catcher did not catch: C2 under MU5. The creep checks
recover the stress with `hex8_second_piola_kirchhoff` of the formulation under test (BBar), from the solution's displacement
and states. Under MU5 the solve runs with the Full element, and the F-bar recovery of the locked field (the element mean of
`det F` in place of the point values) removes most of the point-wise pressure error that CL measures with the Full recovery.
CL checks a Full solve with a Full recovery, so it does not cover this combination. The mutation is still caught by SS1, J1
and J2: in the J2 cylinder the plastic state of each point follows the solve.

### Deviations

- **Amendment 1** (PT5 and PT6): the set-up of run 1 gave the driver no work (an equilibrium start of every step, the energy
  criterion on rounding noise). PT5' and PT6' corrected it before any corrected run, with the limits unchanged, and both were
  met in their single run. The run-1 test stays as the ignored record `pt1_to_pt6_ps1_the_patch_through_the_driver`.
- **C4, known miss.** The growth of the point-wise error from `5 t*` to `20 t*` is `6.02e-3` against the limit `2e-3` (probe
  `7.5e-6`). The diagnostic run of amendment 1 (`c4_diagnosis_tenth_pressure`) shows the cause. At `p` the worst point stays at
  `r = 1.962 a` until `12.5 t*` and then moves to the bore (`r = 1.005 a`), where the equivalent creep strain reaches
  `9.67e-3`. At `p / 10` (the same `t*`, every strain ten times smaller) the growth is `1.06e-4`, the worst point stays at
  `r = 1.962 a` and the largest equivalent creep strain is `9.65e-4`. Strains ten times smaller cut the growth by a factor of
  about 57. The growth is the bore's change of geometry under the total Lagrangian kinematics, measured against the
  small-strain closed form. It is not locking: on the same mesh the Full element is at `0.364`. The probe was small strain and
  so showed none of this; the limit of C4 was set from it and does not allow for the finite-strain drift at a creep strain
  near `1e-2`.

### Decision (by the rule)

B0, S1 and every check of (a1) to (a4) are met (SS1 to SS3, T1, T2, PT1 to PT4, PT5', PT6', PS1). C4, a check of (a6), is
missed, recorded as `c1_to_c4_cl_creep_cylinder_known_miss` with the limit intact, and diagnosed. The controls behave as stated
and every planned mutation is caught. **Proposed status: partial.**

## For the lead: roadmap entries

> F-587 (partial, ADR 0506, 2026-11-12/14): the B-bar Hex8 in the nonlinear driver as a variational F-bar element on the total Lagrangian kinematics (element mean of det F; tangent at rest equal to the F-390 B-bar stiffness to 4e-16; consistent tangent to 1e-8 for SVK, J2, creep and viscoelastic; patch tests to 5e-15 at nu 0.4999; J2 cylinder at nu 0.499 within 1.1e-2 of the closed form where Full gives 1.8 to 2.2; creep cylinder point-wise within 1.6e-2 at 20 t* where Full gives 0.36). Incompatible stays refused in the nonlinear driver. The registered growth bound C4 (2e-3 between 5 t* and 20 t*) is missed at 6.0e-3, diagnosed as the total Lagrangian finite-strain drift of the bore against the small-strain closed form (1.1e-4 at p/10). Defaults bit-identical (11 digests).

> F-580 (note, ADR 0506, 2026-11-12/14): creep on the B-bar Hex8 through the nonlinear driver is met point-wise at 5 t* (9.5e-3) and 20 t* (1.6e-2) against the stationary field; the growth bound between them is missed by the finite-strain drift (6.0e-3 against 2e-3). F-580 keeps its current status until the growth bound is measured on a reference that allows for finite strain, or is re-registered at a smaller p/E.

> Follow-up (ADR 0506, 2026-11-12/14): the incompatible-modes Hex8 at finite strain (an enhanced assumed strain element with the internal modes condensed in the Newton loop); the nonlinear driver refuses it naming the follow-up of ADR 0506.

> Follow-up (ADR 0506, 2026-11-12/14): a creep-cylinder reference for C4 that allows for finite strain, or C4 re-registered at a p/E small enough that the total Lagrangian drift stays below the bound (the diagnostic gives 1.1e-4 at p/10); and a locking control on the creep cylinder that solves with Full and recovers with BBar, the combination the mutation MU5 showed C2 does not see.

> Follow-up (ADR 0506, 2026-11-12/14): the case-file key for the formulation of a nonlinear solid block, as the block property formulation = "bbar" of ADR 0493's open question, covering the nonlinear paths (open question 3).
