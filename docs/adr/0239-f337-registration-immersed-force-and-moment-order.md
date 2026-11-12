# ADR 0239: F-337 registration: immersed solid, second-order force and moment

Status: **REGISTERED 2026-11-12**, written before any code of F-337 and before any run of its checks. Not yet accepted by the
maintainer. The diagnosis and the design go in an amendment written before the corrected code and its runs; results go in an
addendum; nothing above an amendment or the addendum is changed after a run.

Parents: [ADR 0218](0218-f219-registration-immersed-solid-with-modified-forcing.md) (F-219: the forcing, `force_moment`, the
set-ups CC and DFG), [ADR 0234](0234-f309-registration-immersed-face-flux-at-the-surface-crossing.md) (F-309: the second-order
face flux; its question Q5, from which F-337 was made). Roadmap item F-337 (depends on F-309).

Number: the lead gave 0239. Branch `f337-immersed-force-order` from main 1dd5a0e0, worktree `E:/projects/cfd/w49i-work/f337`.

House rules (ADR 0218's, ADR 0234's): limits are fixed here and not changed after a run; a missed limit becomes an ignored
`*_known_miss` test with the limit intact and the number recorded, and is diagnosed, not tuned away; a defective set-up is
corrected by an amendment written and committed before the corrected run, and the old test stays as an ignored record; each
registered run is made once; every new code path is opt-in, and `force_moment` returns exactly what it returned before; no
case-schema key, no hash table, no golden and no existing test file is edited; no new third-party crate; no unsafe code; no
commit trailers.

Clean-room: the evaluation is derived here from the balance of momentum and of angular momentum of a set of finite-volume cells
(the control-volume form of the force on a body, as in Batchelor, An Introduction to Fluid Dynamics, 1967, section 3.2, and, for
immersed boundaries, the conservation argument of Lai and Peskin, J. Comput. Phys. 160 (2000) 705-719, and of Balaras, Comput.
Fluids 33 (2004) 375-404, who obtain the force from the forcing and the momentum of the forced cells). These are cited from
memory; no text of them was read for this work and no formula is copied: the algebra is derived on the discrete equations of
`crates/rcfd-fv/src/immersed.rs`. No vendor code, text, manual or binary was used.

## What F-337 asks

The roadmap entry: "Immersed solid: second-order force and moment"; acceptance "The torque on the immersed rotating cylinder of
circular Couette flow (the CC set-up of ADR 0218, second-order forcing) reaches an observed order of 1.5 between N 48 and 96, and
the DFG 2D-1 drag and lift on M2 are within 1 percent and 3 percent of the FeatFlow values."

Reading (a reading, not a decision of the maintainer): the torque is the z moment about the cylinder axis of the fluid on the
inner body per unit depth, its error `E(N) = abs(M_z/dz + T)/T` with `T = 4 pi rho nu omega R1^2 R2^2/(R2^2 - R1^2) = 4.18879`
(the fluid retards the cylinder: the moment is `-T`), the observed order `log2(E(48)/E(96))`; the runs are those of ADR 0234
(SIMPLEC, `build_second_order`), unchanged. The drag and lift are `C_D` and `C_L` of ADR 0218 on M2 with `build_second_order`
(the run V4 of ADR 0234), against 5.57953523384 and 0.010618948146. Both from the new, opt-in evaluation.

## What was read and run before this text (disclosed)

Read: ADR 0234 in full; ADR 0218 (the design D1, D2, the forces, the set-ups, the checks V2 and A1 to A3, Q5); the whole of
`crates/rcfd-fv/src/immersed.rs`; `crates/rcfd-fv/tests/immersed_second_order_f309.rs`; the roadmap entry.

Known numbers (ADR 0234 addendum 1, `force_moment`, second-order forcing): inner torque 4.04376, 4.12171, 4.15767, 4.17239 at
N 24, 48, 96, 192, errors 3.46, 1.60, 0.74, 0.39 percent (orders 1.11, 1.11, 0.92); with the forcing of ADR 0218 2.66, 1.15, 0.45
percent. DFG 2D-1 on M2, second: `C_D` 5.56246 (-0.306 percent), `C_L` 0.0108880 (+2.534 percent). ADR 0218 V2: on M1 the force
of the forcing equals the momentum balance of the domain boundary to 1.8e-13.

Run: no code of F-337 and no run of a check below. One probe of the existing code (a scratch test, deleted, not committed;
release build of main 1dd5a0e0): the FNV-1a hash (the function `fnv` of `immersed_second_order_f309.rs`) over the bits of the
three force and then the three moment components that `force_moment(b, u, 1.3, [0.3, -0.2, 0.1])` returns for the pseudo-random
field `u_c = (2 lcg - 1, 2 lcg - 1, 0)` (the `lcg` of that file, seed `0x0337`, two draws per cell in cell order):

- CC at `N = 24` (`cc_fields(24, ..)`), `build` (ADR 0218): inner body `5a21ad65f0798915`, outer body `1a364dafdefe88ba`;
- the same with `build_second_order`: `5a21ad65f0798915`, `1a364dafdefe88ba` (the terms of the two forcings are equal);
- DFG M1 (`dfg_family(1)`, `K = 1e7`, `nu = 1e-3`), `build_second_order`, the cylinder: `ead7c19fe86c3d8b`.

These numbers are the reference of check N1 and nothing else.

Arithmetic done beforehand (by hand, no program), disclosed because it shaped the hypotheses:

1. What `force_moment` is. Summing the steady momentum equation of a solid cell `s`, `rho V K (u_s - u_b(x_s))` is the net of
   the face fluxes of `s`. Summed over the solid cells of a body the fluxes of the solid-solid faces cancel, and with the
   modification terms the sum is, per fluid-solid face `f` (fluid cell `c`, `k_f = gamma_f lap_coeff_f`, `theta_e` the bounded
   crossing fraction), `rho k_f (u_c - u_G)/theta_e` (the one-sided wall difference) plus the face pressure `p_f S_f` and the
   convected momentum `phi_f u_f`. The force is therefore not a sum of surface stresses evaluated by the function; it is the
   reaction of the forcing, and by conservation it equals the momentum flux through the outer faces of the layer of first fluid
   cells (ADR 0218 V2 measured exactly this on the domain). An error in a flux of a fluid-solid face (the face pressure, the
   convected value) cancels between the two cells of the face.
2. The moment does not have that property: `force_moment` applies a solid-cell term at the centre of the solid cell and a
   modification term at the crossing `x_G`. Writing `v = u - u_b`, the viscous flux `k_f v_c` that the solid cell receives through
   `f` acts at `x_s`, and the modification `k_f v_c (1/theta - 1)` at `x_G`. If instead every term of a cell is applied at the
   centre of its cell, the sum of the moments is the discrete angular-momentum balance of the cells (face fluxes of a cell taken at
   its centre: pressure parts have no arm on an orthogonal mesh, the viscous parts sum to the integral of the vorticity, which
   telescopes to the outer faces). The two differ per face by `(x_c - x_G) x F_mod = -rho k_f (1 - theta) h n_f x v_c`, with
   `v_c` about `-theta h dv/dn`: a term of order `mu A h theta (1 - theta) dv/dn` per face, summed over a surface of area of
   order 1: first order. With `theta` uniform on `[0, 1]` (mean of `theta (1 - theta)` = 1/6) and a geometric factor of about
   0.8 for the staircase, the estimate for CC is a torque low by about `0.8 h/(6 R1)`: 1.3, 0.67 and 0.33 percent at N 48, 96,
   192, against the measured 1.60, 0.74 and 0.39.
3. The one-sided difference `(u_G - u_c)/(theta h)` of the exact profile is the derivative at the middle of the segment, not at
   the wall: for CC (`v' = -8/3`, `v'' = 16/3` at `R1`) about `theta h` relative, 2 percent at N 48 on the exact field.
4. The quadrature of a surface integral by the crossings with the weights `abs(S_f . n)`: a midpoint rule per grid direction of
   an integrand that vanishes like a square root where the grid lines are tangent to the surface, error of order `h^1.5`.

## Hypotheses for the first-order error (each with its measurement and the number that decides it)

All measured on the CC runs of ADR 0234 (second-order forcing, SIMPLEC, converged), inner body, N 48 and 96, by a diagnostic
test that uses only the public fields of the forcing and of the solver (no library change). `dE(N) = abs(M_fm - (-T))` is the
error of the moment of `force_moment`. "Holds" means at both N.

| Id | Hypothesis | Measurement | Decides |
| --- | --- | --- | --- |
| H1 | The arm of the forcing terms: the modification terms are applied at the crossing, the solid-cell terms at the solid centre, so the moment is not the angular-momentum balance of the cells (item 2). | `M_cc`: the moment of the same terms, each applied at the centre of its cell. | holds if `abs(M_cc + T) <= dE/3` |
| H2 | The stress evaluation point: the wall stress is the one-sided difference between the crossing and the first fluid centre (item 3). | `T_w`: the moment of `rho k_f (u_c - u_b(x_c))/theta_e` applied at `x_G` (the surface-stress evaluation with that difference; the pressure has no moment about the axis of a circle), with the solved field and with the exact field in the fluid cells. | holds if `abs(T_w + T) >= dE/2` with the solved field; the exact-field number says what the difference itself costs |
| H3 | The pressure part: the pressure of the staircase faces (interpolated between a fluid cell and a solid cell whose pressure is a mean of its fluid neighbours) carries a first-order error. | `T_p = sum_f (x_f - x_ref) x (sg_f p_f S_f)` over the fluid-solid faces, `p_f` the solver's face interpolation (exact value 0). | holds if `abs(T_p) >= dE/2` |
| H4 | The staircase surface: the area and position of the staircase faces in place of the surface (item 4). | `Q`: the exact wall traction of the Couette profile at the crossings, summed with the face areas, `sum_f (x_G - x_ref) x (mu sg_f A_f dv/dn_f)` (no solved field). | holds if `abs(Q + T) >= dE/2` |
| H5 | The momentum of the forced cells: the flux of ADR 0234 D1 carries momentum into the solid cells, where the mass source removes the mass. | `T_q = sum_s (x_s - x_ref) x (rho q_s u_b(x_s))` with `q_s = -sum_f sg_f (phi_f - R_f)` from the converged `phi`; and the comparison with the forcing of ADR 0218, which has no such flux. | holds if `abs(T_q) >= dE/2` |

Expectation written before the measurement: H1 holds; H2 does not hold with the solved field (by conservation the discrete
one-sided flux is the flux the full cell needs at its face) and the exact-field number is of the size of item 3; H3, H4 and H5 do
not hold. If H1 does not hold, the design is not the one sketched in item 2 and is derived in the amendment from what was
measured.

## Registered checks

All in the new file `crates/rcfd-fv/tests/immersed_force_order_f337.rs`, release (the heavy ones carry
`#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]`). "New" is the opt-in evaluation of the amendment; "old" is
`force_moment`. The CC and DFG set-ups are those of ADR 0218 with `build_second_order`, exactly as run in ADR 0234 (the helper
code is copied; the existing test files are not edited).

| Id | Check | Limit | Estimate behind it |
| --- | --- | --- | --- |
| A1 (acceptance) | CC, second-order forcing, new: `log2(E(48)/E(96))` | `>= 1.5` | the roadmap's number; old: 1.11 |
| A1b | CC, new: `E(96)` against the old error at N 96 | `<= 0.5` of it | with `E_new(48) <= E_old(48)` and order 1.5 the ratio is 0.77 at most; item 2 predicts about 0.1 |
| A2 (acceptance) | DFG 2D-1 on M2, second-order forcing, new: the convergence rule of ADR 0218 met on M1 and M2 (set-up); `C_D`, `C_L` | within 1 percent of 5.57953523384; within 3 percent of 0.010618948146 | the roadmap's numbers; old: -0.306 and +2.534 percent |
| S1 (set-up) | every CC run (N 24, 48, 96) | `max_du <= 1e-10` within 20 000 iterations | ADR 0234: 103, 102, 340 iterations |
| C1 (negative control of A1) | CC, old, the same runs: `log2(dE(48)/dE(96))` | `< 1.5` | ADR 0234: 1.11 |
| N1 (defaults) | `force_moment` on the pseudo-random fields of the probe above, on the final tree | the five hashes of the probe | `force_moment` is not edited |
| N2 (defaults) | the existing files `immersed_solid_f219.rs` and `immersed_second_order_f309.rs`, unedited, run once on the final tree | every test passes; the printed forces are those of ADR 0218 and ADR 0234 (M3 `C_D` 5.57833680, `C_L` 0.0106899517; second M2 `C_D` 5.56246, `C_L` 0.0108880; CC torque old 4.12171, 4.15767 at N 48, 96) to the digits given | nothing on the default path changes |

Unit checks of the new evaluation (its consistency with the forcing, the transfer of the reference point, what it returns for
the forcing of `build`) are fixed in the amendment with the design, before the code.

Printed, no limit: `E` new and old at N 24 and 192 and the orders 24 to 48 and 96 to 192; the new evaluation with the forcing of
ADR 0218 (`build`) on CC; the force on the inner cylinder (exact value 0; the mesh is symmetric about the axis); the 2D-1 values
on M1; the old values beside every new one.

### Negative controls and mutations (scratch, not committed)

C1 above. Mutations, each applied alone to the final tree by an exact text replacement (checked to occur once), the named tests
run once in release, the file restored and compared byte for byte with a kept copy:

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | the new evaluation made equal to the old one in the part the diagnosis names (for H1: the modification terms applied at the crossing) | A1 |
| MU2 | `force_moment` changed in the same part (for H1: its modification terms applied at the cell centre) | N1 |

Further mutations of the design (one per statement of the new evaluation that a check should be able to see) are listed in the
amendment before the code.

## Decision rule

- **done**: A1 and A2 met with S1 and C1 held; A1b, N1, N2 and the unit checks of the amendment met; every mutation failed a test
  it must fail.
- **partial**: N1 met and the cause of the first order established by a registered measurement, but A1 or A2 missed (kept as an
  ignored known miss with a diagnosis), or A1 and A2 met and another check missed, or a mutation was not caught and its amendment
  is not done.
- **todo** (unchanged): N1 missed, or no hypothesis is decided by the measurements; the library part is then not proposed for a
  merge.

A set-up check or control that does not hold (S1, C1, the convergence rule of A2) is a defective set-up: an amendment before any
corrected run, never counted as a pass.

## Limits (not covered)

Steady flow: the reaction of the forcing is the force on the body when the momentum of the forced cells does not change in time;
the unsteady part is not addressed and the 2D-2 case is not run. No case-schema key, no Studio control and no change of what
`sixdof` or any caller uses (the new evaluation is a library method that a caller must ask for). Orthogonal meshes only are
checked. One shape with a closed-form torque (the circular cylinder); the sphere and the cuboid are not run. A body that touches
the domain boundary (ADR 0218 Q5) is as before. No moving body. Whether the new evaluation becomes what `force_moment` returns is
a change of a default and is not taken here (Q1).

## Open questions

- **Q1. Default.** Options: (a) a new method beside `force_moment`; (b) change `force_moment`. (b) changes numbers that existing
  tests print and that ADR 0218 recorded, so it is not taken. Recommendation (a) now; (b) is the maintainer's to decide with the
  results.

## Amendment 1 (2026-11-12): the diagnosis, the design and its checks

Written after the diagnostic run and before any library code of F-337 and any run of a registered check. The diagnostic is the
ignored test `diag_first_order_torque` of `crates/rcfd-fv/tests/immersed_force_order_f337.rs` (commit a6aec16d; it uses the public
fields of the forcing and of the solver only; no library change), run once in release on that commit.

### The measurements (CC, second-order forcing, SIMPLEC converged in 102 and 340 iterations; exact moment -4.18879)

| | N 48 | N 96 | order | Verdict by the registered number |
| --- | --- | --- | --- | --- |
| `force_moment` (old), `dE` | -4.12171, 6.708e-2 (1.601 %) | -4.15767, 3.112e-2 (0.743 %) | 1.11 | the known first order |
| H1 `M_cc` (every term at the centre of its cell) | -4.18431, error 4.478e-3 (0.107 %); `dE/3` 2.236e-2 | -4.18766, error 1.132e-3 (0.027 %); `dE/3` 1.037e-2 | 1.98 | **holds** |
| H2 `T_w`, solved field (one-sided wall difference at the crossing) | -4.18116, error 7.63e-3 (0.182 %); `dE/2` 3.35e-2 | -4.18442, error 4.37e-3 (0.104 %); `dE/2` 1.56e-2 | 0.80 | does not hold |
| H2 `T_w`, exact field | -4.15167, error 3.71e-2 (0.886 %) | -4.16891, error 1.99e-2 (0.475 %) | 0.90 | (information) |
| H3 `T_p` (staircase face pressure) | -3.16e-3 (0.075 % of T); `dE/2` 3.35e-2 | -3.30e-3 (0.079 %); `dE/2` 1.56e-2 | none | does not hold |
| H4 `Q` (exact traction at the crossings, face areas) | -4.20322, error 1.44e-2 (0.344 %) | -4.19391, error 5.12e-3 (0.122 %) | 1.50 | does not hold |
| H5 `T_q` (momentum removed with the mass source) | 3.8e-6 | 5.7e-7 | | does not hold |
| forcing of ADR 0218: old; cell arms | 1.151 %; 0.302 % | 0.447 %; 0.243 % | 1.36; 0.31 | (the comparison of H5) |

The force on the inner cylinder (exact 0) is 5.5e-10 and 1.3e-10 per unit depth.

**The cause.** H1: the first-order error of the torque is the arm of the forcing terms. `force_moment` applies the
modification term of a first fluid cell at the crossing; the moment of the forcing is the angular-momentum balance of the cells
only when every term is applied at the centre of its cell. The difference of the two is the term written in the registration,
`-rho k_f (1 - theta) h n_f x v_c` per face: 1.49 and 0.72 percent of T measured (the estimate written beforehand was 1.3 and
0.67). The other four are not the cause:

- H2. The exact-field number confirms that the one-sided difference of the exact profile is first order (0.89, 0.48 percent).
  The solved field does not have that error at that size (0.18, 0.10 percent), as expected from conservation, but `T_w` is still
  first order: a surface-stress evaluation with this difference would not meet the acceptance either.
- H3. The staircase face pressure has a moment that does not go down with the mesh (0.08 percent of T at both N) although the
  exact pressure has none: the face pressure of a fluid-solid face is in error (ADR 0234 D4 says so of the pressure gradient of
  the first fluid cells). It does not reach the moment of the forcing, because the same face pressure enters the two cells of the
  face with opposite signs and equal arms. This is why the evaluation below stays with the forcing and does not integrate
  stresses on the staircase.
- H4. The quadrature at the crossings has the predicted order 1.5 (1.495 measured): with exact stresses it would sit on the
  acceptance limit.
- H5. Negligible; and the forcing of ADR 0218, which has no flux of ADR 0234 D1, has the same first-order arm error. With the
  cell arms its torque error is 0.30 and 0.24 percent: what is left there is the first-order velocity field of that forcing.

Disclosure: `M_cc` is the quantity the new evaluation returns, so the diagnostic has already measured, from the test side, the
numbers that the acceptance check A1 will measure through the library (error 1.07e-3 and 2.70e-4 of T, order 1.98). A1's limit
was fixed in the registration before that measurement; its registered run through the new method is made once.

The sign in the registration's row H4 was loose: the traction of the fluid on the body through `f` is `-mu A_f dv/dn` with `n`
from the fluid centre to the solid centre (what the diagnostic computes).

### Design

**D1. The moment of the forcing with cell arms (derived).** New method
`ImmersedForcing::force_moment_second_order(body, u, rho, x_ref) -> (force, moment)`:

    dF_t = rho V_c (coeff_t u_c - source_t),   F = sum_t dF_t,   M = sum_t (x_c(t) - x_ref) x dF_t,

over the terms `t` of the body, `x_c(t)` the centre of the cell of the term (the solid cell of a solid-cell term, the fluid cell
of a modification term). The force is the sum `force_moment` forms, in the same order (bit-identical). `force_moment` is not
changed. The forcing stores the cell centres (a private field beside the private volumes; nothing public changes).

Derivation. Let `R` be the solid cells of the body and the fluid cells that carry its modification terms. In a steady state the
momentum equation of a cell `c` of `R` reads `sum_{f of c} g_{c,f} = dF_c`, where `g_{c,f}` is the force of the neighbour on
`c` through the face `f` as the solver discretises it (pressure `-p_f S_out`, viscous `k_f (u_nb - u_c)`, convection
`-rho phi_out u_f`) and `dF_c` the sum of the terms of the cell (the modification is a term of the cell, so the face fluxes are
the regular ones and `g_{c,f} = -g_{nb,f}` on every internal face). Then

    sum_c (x_c - x_ref) x dF_c = sum_c sum_f (x_f - x_ref) x g_{c,f} - sum_c sum_f (x_f - x_c) x g_{c,f}.

The first sum telescopes to the outer faces of `R` (the faces between a cell of `R` and a fluid cell outside it): the moment
about `x_ref` of the discrete pressure, viscous (Laplacian form) and momentum fluxes on a closed surface of regular fluid faces.
In the second sum, on an orthogonal mesh `x_f - x_c` is along the face normal, so the pressure part vanishes; the viscous part is
`(x_f - x_c) x k_f (u_nb - u_c) = mu A_f n_out x (u_f - u_c)` with `u_f` the linear face value, and its sum over the faces of a
cell is `mu sum_f A_f n_out x u_f` (the sum of `A_f n_out` over a closed cell is zero), the discrete integral of `mu curl u`
over the cell, which telescopes to `mu sum_{outer f} A_f n_out x u_f`; the convective part is, per cell,
`-rho sum_f abs(x_f - x_c) A_f n x u_f (u_f . n)`, which for a smooth field is `-rho V (u x u) = 0` up to a term of order
`h^2 V`. So, up to that term,

    sum_t (x_c(t) - x_ref) x dF_t = sum_{outer f} (x_f - x_ref) x g_f - mu sum_{outer f} A_f n_out x u_f.

The last sum is the part of the stress that the Laplacian form leaves out: for a divergence-free field on a closed surface
`integral x x (grad u^T n) dS = -integral n x u dS` (Stokes' theorem on the closed surface applied to `x_j u_l`; it needs
`div u = 0` on the surface only). The right-hand side is therefore the midpoint rule, on regular faces in the fluid, of the
moment of the full traction `-p n + mu (grad u + grad u^T) n` and of the momentum flux on a closed surface around the body, which
in a steady flow is the moment of the fluid on the body. Its error is that of the velocity and pressure on regular fluid faces
one cell from the surface. The same argument without the arms is the known statement for the force (ADR 0218 V2).

With the arm of a modification term at the crossing instead, the moment changes by `(x_G - x_c) x dF_t`, first order (above).
Nothing in the argument uses the flux of ADR 0234; with the forcing of ADR 0218 the evaluation is as accurate as that forcing's
velocity field.

**D2. What stays.** The force: `force_moment`'s, to the bit; A2 is therefore expected at the numbers of ADR 0234 (-0.306 and
+2.534 percent), and what A2 adds is that the new method returns them. No solver file is touched (no hunk in `simple.rs` or
`pimple.rs`).

**D3. Name and placement (taken on the maintainer's standing instruction of 2026-11-11, take the recommendation; Q1, Q2).** A new
method beside `force_moment`, not a flag of the settings and not a change of `force_moment`.

### Checks added by this amendment (limits fixed before the code)

| Id | Check | Limit | Estimate behind it |
| --- | --- | --- | --- |
| U1 | the force of the new method against that of `force_moment`: CC N 24 (`build` and `build_second_order`, both bodies) and DFG M1, the pseudo-random field of the probe | bit-identical | D1 |
| U2 | the moment of the new method against the sum formed in the test from the public terms and the cell centres of the mesh, the same fields | `<= 1e-12` of the norm of the moment | the same sum |
| U3 | transfer of the reference point: `M(x1) - M(x2) - (x2 - x1) x F`, `x1 = (0.3, -0.2, 0.1)`, `x2 = (-1.1, 0.7, 0.4)`, the same fields | `<= 1e-10` of the norm of `M(x1)` | round-off of sums of a few hundred terms |
| U4 | a forcing with `modified = false` (CC N 24): new against old, force and moment | bit-identical | a solid-cell term is applied at its cell centre by both |
| V1 (the derivation) | CC at N 48, second, converged: the z moment of the new method against `M_out`, the right-hand side of D1 formed in the test on the outer faces of `R` of the inner body from the solved `u`, `p`, `phi` (viscous `k_f (u_nb - u_c)`, pressure `-p_f S_out` and convection `-phi_out u_f` with the means of the two cells as face values, and `-mu A_f n_out x u_f`) | `abs(M_new - M_out) <= 2e-3 T` | only the convective arm term differs: `rho V u^2` times a factor of order `h^2` over an area of about 0.9, about 1e-4 T at N 48 |

Printed, no limit: `M_out` at N 96; `M_out` against the exact moment.

### Mutations (scratch, not committed; the procedure of the registration)

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | new method: the arm of a term is its `point` (the crossing for a modification term) | A1, and U2 |
| MU2 | `force_moment`: the arm is the cell centre | N1 |
| MU3 | new method: the arm is the cell centre without `x_ref` subtracted | U3 |
| MU4 | new method: the modification terms are left out of the force and the moment | U1 |
| MU5 | new method: the solid-cell terms are left out of the moment | U2, and A1 |
| MU6 | the stored cell centres are those of the mesh shifted by one cell (`centres[c] = cc[(c + 1) % n]`) | U2 |

The decision rule of the registration is unchanged; "the unit checks of the amendment" are U1 to U4 and V1.

### Questions added

- **Q2. Name.** `force_moment_second_order` (taken) or a flag on `force_moment`. A flag would change the signature every caller
  uses. Recommendation: the separate method.

## Addendum 1: results (2026-11-12)

Measured against the limits registered above. No limit was changed and no registered run was repeated. Release builds on
Windows. The registration is 6925f107; the diagnostic test is a6aec16d (run once on that commit); amendment 1 is c75f93b5; the
library and the registered tests are 1b11face, committed before their first run; every registered run was made once on that
commit (one invocation of the test file, one thread). This addendum's commit changes no code.

**Built.** `crates/rcfd-fv/src/immersed.rs`: `ImmersedForcing::force_moment_second_order` (D1), the private field `centres`
(the cell centres, stored by the constructor), two documentation paragraphs; `force_moment` is not edited except for two lines of
its documentation comment. `crates/rcfd-fv/tests/immersed_force_order_f337.rs` (new: the diagnostic and the registered checks).
`docs/reference-data.md` (a section for F-337). `simple.rs` and `pimple.rs` are not touched (no hunk); no case-schema key,
golden, hash table or existing test file was touched; `overset.rs`, `compressible.rs`, `compressible_bc.rs`, `sim.rs` and
`schema.rs` were not touched.

### Hypotheses

| Id | Hypothesis | Measurement (N 48; N 96) | Verdict |
| --- | --- | --- | --- |
| H1 | the arm of the forcing terms (modification terms applied at the crossing) | with cell arms the error is 4.48e-3; 1.13e-3 against `dE/3` 2.24e-2; 1.04e-2 | **holds: the cause** |
| H2 | the stress evaluation point (one-sided wall difference) | `T_w` error 7.6e-3; 4.4e-3 against `dE/2` 3.4e-2; 1.6e-2 (exact field: 3.7e-2; 2.0e-2) | does not hold; the difference is first order, but it is not what `force_moment` sums |
| H3 | the pressure of the staircase faces | 3.2e-3; 3.3e-3 against `dE/2` | does not hold (the face pressure is in error, but cancels between the two cells of a face) |
| H4 | the staircase surface (quadrature at the crossings) | 1.44e-2; 5.1e-3 against `dE/2` (order 1.50) | does not hold |
| H5 | the momentum of the forced cells (the flux of ADR 0234 D1 and the mass source) | 3.8e-6; 5.7e-7 | does not hold |

The details and the derivation are in amendment 1. In one sentence: the force of the forcing was never first order (it is the
momentum balance of the cells); the moment was, because a modification term was given the arm of the surface crossing instead of
the arm of its cell.

### Results

| Check | Limit | Measured | Verdict |
| --- | --- | --- | --- |
| A1 (acceptance) CC second, new: order 48 to 96 | `>= 1.5` | **1.984** (`E` 3.959e-3, 1.069e-3, 2.702e-4 at N 24, 48, 96; 24 to 48: 1.889) | **met** |
| A1b `E_new(96)/E_old(96)` | `<= 0.5` | 0.0364 | met |
| A2 (acceptance) DFG 2D-1 second, new: convergence rule on M1, M2; `C_D`, `C_L` on M2 | within the caps 4000; within 1 percent; within 3 percent | 470 and 750 iterations; `C_D` 5.56246454 (-0.306 percent); `C_L` 0.0108880492 (+2.534 percent) | **met** |
| S1 CC convergence, three runs | `max_du <= 1e-10` within 20 000 | 103, 102, 340 iterations | held |
| C1 (negative control) CC, old: order | `< 1.5` | 1.108 (`dE/T` 3.462e-2, 1.601e-2, 7.430e-3) | held |
| N1 defaults: `force_moment` hashes | the five of the probe | `5a21ad65f0798915`, `1a364dafdefe88ba`, `5a21ad65f0798915`, `1a364dafdefe88ba`, `ead7c19fe86c3d8b` | met |
| N2 defaults: the existing files, unedited, on the final tree | every test passes; the recorded forces | `immersed_solid_f219.rs`: 11 passed, 2 ignored (its known miss and its diagnostic), M3 `C_D` 5.57833680, `C_L` 0.0106899517, `St` 0.30073, CC torque 4.07749, 4.14059, 4.17006; `immersed_second_order_f309.rs`: 9 passed, 1 ignored (its printed N 192 run), M2 `C_D` 5.56246454, `C_L` 0.0108880492, CC torque 4.04376, 4.12171, 4.15767, state hashes `3b49c4ed4863d1b6`, `9fff50792adb6d4a` | met |
| U1 force of new against old, five body/forcing pairs | bit-identical | bit-identical | met |
| U2 moment of new against the test-side sum | `<= 1e-12` | 0 (five pairs) | met |
| U3 transfer of the reference point | `<= 1e-10` | 4.5e-16, 4.2e-16, 4.5e-16, 4.2e-16, 3.0e-15 | met |
| U4 `modified = false`: new against old | bit-identical | bit-identical (both bodies) | met |
| V1 (the derivation) CC N 48: `abs(M_new - M_out)` | `<= 2e-3 T` | 1.53e-6 T (`M_new` -4.184312, `M_out` -4.184306) | met |

Printed, no limit. CC second on N 192: `E` new 6.949e-5, order 96 to 192 1.959 (old 3.915e-3, order 0.924); 1223 iterations.
`M_out` at N 96: -4.187658, 2.3e-7 T from `M_new`. The new evaluation with the forcing of ADR 0218 (`build`): `E` 3.058e-3,
3.025e-3, 2.431e-3 at N 24, 48, 96 (old 2.657e-2, 1.151e-2, 4.472e-3): smaller than the old at every N, but not converging at
second order, because that forcing's velocity field is first order (ADR 0234). The force on the inner cylinder (exact 0): 9.6e-10,
5.5e-10, 1.3e-10 per unit depth. 2D-1 on M1 second: `C_D` 5.51738362 (-1.114 percent), `C_L` 0.0114183079 (+7.528 percent), the
numbers of ADR 0234. The moment coefficient of the 2D-1 cylinder about its axis, `M_z/(q D)` (no reference value): new 1.1094e-3
on M1 and 9.897e-4 on M2; old 1.0772e-3 and 9.778e-4.

What the acceptance numbers say, plainly. A1 is met by the new evaluation with a margin (1.98 against 1.5, and 1.96 on the next
refinement). A2 is met, but not by anything F-337 built: the new method returns the force of `force_moment` to the bit, and the
drag and lift on M2 are the numbers ADR 0234 already measured (-0.306 and +2.534 percent). The diagnosis says why no change of
the force was made: the force is the momentum balance of the cells and its error is that of the velocity field (M1 to M2 the drag
error goes from -1.11 to -0.31 percent, the lift error from +7.5 to +2.5 percent). The lift's margin to its limit is 0.47
percentage points.

### Mutations

Applied one at a time to 1b11face by a scratch script (an exact text replacement checked to occur once), the named tests run
once in release, the file restored and compared byte for byte with the kept copy (equal after each of the six).

| Id | Must fail | Result |
| --- | --- | --- |
| MU1 new: the arm is the term's `point` | A1, and U2 | failed: A1 order 1.108 (A1b ratio 1.000, V1 1.49e-2 T); U2 2.3e-7 |
| MU2 `force_moment`: the arm is the cell centre | N1 | failed: hashes `3b6396b83298b62e`, `d1cfc1df9076b1bb`, `a62e2c030d3c20ae` |
| MU3 new: `x_ref` not subtracted | U3 | failed: U3 1.75 printed against 1e-10; the assertion that stopped the test was U2 (0.287), which comes first |
| MU4 new: the modification terms left out | U1 | failed: the force differs |
| MU5 new: the solid-cell terms left out of the moment | U2, and A1 | failed: U2 1.0; A1 order -0.145 |
| MU6 the stored centres shifted by one cell | U2 | failed: U2 0.108 |

### Tools

`cargo fmt -p rcfd-fv --check`: clean. `cargo clippy -p rcfd-fv --all-targets -- -D warnings`: clean (also with `--release`). `cargo test --release
-p rcfd-fv --test immersed_force_order_f337`: 5 passed, 2 ignored (the diagnostic, run once on a6aec16d, and the printed N 192
run, run once with `--ignored`). `scripts/limit_audit.py`: 0 hits on the new test file.

### Deviations and disclosures

- The diagnostic computed, from the test side, the quantity the new method returns, so the numbers of A1 were known (to the
  digits printed) before the registered run through the library; A1's limit is the registration's, fixed before the diagnostic.
  This is disclosed in amendment 1 and is the nature of a diagnosis-first item.
- The estimate written with V1 (about 1e-4 T) was sixty times too large; the measured difference is 1.5e-6 T.
- Before the first run of the registered checks only the build, `cargo fmt` and `cargo clippy` were run on the code; clippy asked
  for the negated comparison of C1 to be written through the helper `le`, which was done before the commit and the run. Nothing
  was tuned: the library of 1b11face is the first version that was run.
- N2 and the printed N 192 run were made from copies of the test executables built from 1b11face, so that the mutations could be
  compiled at the same time; the copies are byte-for-byte the cargo outputs.
- Not run: the workspace test suite and the gate (the lead's); the 2D-2 case and any unsteady force with the new evaluation; the
  PISO path (the evaluation is a function of the velocity field alone); a non-uniform or non-orthogonal mesh at the surface; the
  sphere and the cuboid; a cylinder whose axis is not on a mesh line (the CC mesh is symmetric about the axis).

### Decision by the registered rule

A1 and A2 met with S1 and C1 held; A1b, N1, N2, U1 to U4 and V1 met; each of the six mutations failed a test it must fail.
**F-337 is done by the rule**, in the library, pending the maintainer's acceptance. "Done" does not include: the new evaluation
as what `force_moment` returns (Q1), a case key or any caller in the solver or the API (the method is called by tests only, as
`force_moment` is), unsteady flow (Q3), and the limits of the registration.

### Remaining questions

- **Q1. Default** (as registered). Options: (a) keep both methods; (b) make the cell arms the moment of `force_moment` and drop
  the second method. The moment of `force_moment` is used by no library or application code; the tests of ADR 0218 and ADR 0234
  print it without a limit (their printed torques would change from 4.12171 to 4.18431 at N 48); no hash depends on it. (b) is a
  change of a default and is not taken. Recommendation: (b), by the maintainer's decision, with the acceptance of this item:
  the present moment is in error at first order and has no use that the corrected one does not serve better.
- **Q2. Name** (as in amendment 1). Taken: the separate method. If Q1 is answered (b) the question is moot.
- **Q3 (added). Unsteady flow.** The reaction of the forcing is the force on the body only when the momentum of the forced cells
  is steady; in the 2D-2 case the first fluid cells and the mass-source cells carry a time-dependent momentum that the reaction
  holds (ADR 0218 measured the 2D-2 force maxima within 2.6 percent with it). Options: (a) a follow-up item that subtracts the
  rate of change of the momentum (and angular momentum) of the forced cells, checked on 2D-2 against the FeatFlow extrema and on
  an oscillating case with a closed form; (b) leave it. Recommendation (a), at low priority, after the case key of the immersed
  solids exists (the item proposed by ADR 0234).
- **Q4 (added). The lift on M2.** +2.53 percent against the 3 percent of the acceptance: met, with a margin of 0.47 points, by
  the convergence of the field (M3 with the forcing of ADR 0218 is at +0.67 percent). Nothing to decide; recorded so that the
  margin is not read as a property of the new evaluation.

### For the lead: roadmap entries

**F-337: proposed status `done`** (pending acceptance). Proposed note (replacing the present one):

> DONE in the library (ADR 0239, 2026-11-12), pending acceptance. From ADR 0234 question 5. Diagnosis by measurement (five hypotheses registered before any code): the force of the forcing was never first order (it is the momentum balance of the forced cells, its error that of the velocity field); the moment was, because `force_moment` applies the modification term of a first fluid cell at the surface crossing and not at the centre of its cell (1.49 and 0.72 percent of the torque at N 48 and 96); the one-sided wall difference, the staircase face pressure, the quadrature at the crossings (order 1.50) and the momentum of the mass-source cells were measured and ruled out. Delivered: rcfd-fv immersed.rs, `ImmersedForcing::force_moment_second_order` (opt-in; `force_moment` unchanged and bit-identical, hashes checked): the same force, and the moment with every term at the centre of its cell, which is the angular-momentum balance of the forced cells, that is the moment of the full traction and momentum flux on the regular fluid faces around them (derived in the ADR; checked against that sum to 1.5e-6 of the torque). Measured (limits registered before the code): circular Couette torque error 0.396, 0.107, 0.027, 0.0069 percent at N 24, 48, 96, 192, observed order 1.98 between 48 and 96 (limit 1.5; 1.96 from 96 to 192) against 1.11 of `force_moment`; DFG 2D-1 on M2 C_D -0.306 %, C_L +2.534 % (limits 1 % and 3 %; the force is that of `force_moment`, unchanged); six mutations caught. Not included: the new evaluation as the default of `force_moment` (open, a change of a default); unsteady flow (the momentum of the forced cells); any caller outside the tests, a case key; non-orthogonal meshes; the limits of F-219 and F-309.

**Proposed follow-up item** (only if Q3 is answered (a)):

1. *Immersed solid: force and moment in unsteady flow.* Deps: F-337, and the case-key item proposed by ADR 0234 if it is made. Acceptance: "The force and moment of an immersed solid subtract the rate of change of the momentum and angular momentum of its forced cells; DFG 2D-2 maximum drag and lift on M2 are within 1.5 percent of the FeatFlow level-6 values, and a steady case returns the values of F-337 to round-off." Note:

> From ADR 0239 question 3, recommendation taken on the maintainer's standing instruction of 2026-11-11. ADR 0218 measured the 2D-2 extrema within 2.6 percent with the reaction of the forcing alone.
