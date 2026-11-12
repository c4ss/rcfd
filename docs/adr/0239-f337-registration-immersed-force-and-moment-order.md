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
