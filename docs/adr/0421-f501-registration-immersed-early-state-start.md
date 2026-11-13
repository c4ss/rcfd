# ADR 0421: F-501 registration: the early-state pressure modes of the single corrector from a start without flow through the cylinder

Status: **REGISTERED 2026-11-13, written before any code of F-501 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-501 was written, compiled or run: the documents and code under "What was read" were
read, and the arithmetic under "Arithmetic before this registration" was done by hand from numbers in the repository's records.
Results go in the addendum at the end; a defective set-up is corrected by an amendment written and committed before the
corrected run; nothing above an amendment or the addendum is changed after a run.

Parents: [ADR 0405](0405-f494-registration-immersed-multistep-linearisation.md) (F-494: the k-step map D1, its central difference
D2, Arnoldi D3, the channel D4; the products PB, PF; the one-step spectra X3, X4; its question Q2, from which F-501 was made),
[ADR 0392](0392-f482-registration-immersed-step-linearisation.md) (F-482: the state vector, the one-step map, the Hessenberg QR),
[ADR 0374](0374-f363-registration-immersed-flux-with-one-piso-corrector.md) (F-363: the set-up R; B with one corrector stops at
step 34), [ADR 0249](0249-f346-registration-immersed-unsteady-force.md) (the start field of DFG 2D-2: the inflow profile in
every cell, through the body, and a blob of `v`), [ADR 0234](0234-f309-registration-immersed-face-flux-at-the-surface-crossing.md)
(the face flux of the second-order forms at the fluid-solid faces). Roadmap item F-501 (deps F-494). Branch
`f501-immersed-early-modes`, worktree `w49i-work/f501`, scratch `w49i-work/f501-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run; a missed limit becomes an ignored
`*_known_miss` test with the limit intact and the number recorded; each registered run is made once; defaults stay
bit-identical; no hash table, golden or existing test file is edited (the helpers and the linear algebra of
`immersed_multistep_linearisation_f494.rs` are copied into the new file; that file is not edited); no new third-party crate, no
unsafe code; unsigned commits without trailers; no push.

Clean-room: the step is rcfd's own PISO. The projection of the start field is the discrete Helmholtz-Hodge projection on the
cell faces (Chorin, Math. Comp. 22 (1968) 745-762, cited from memory; nothing of it is used beyond the idea, which is derived
below from the discrete continuity equation). The linear system is solved by Jacobi-preconditioned conjugate gradients
(Hestenes and Stiefel, J. Res. NBS 49 (1952) 409-436; Golub and Van Loan, cited from memory, written from the definition). The
linearisation tools are those of ADR 0405 and ADR 0392, copied. No vendor code, text, manual or binary.

## What F-501 asks

Acceptance (`docs/roadmap.json`), binding, clause by clause:

> The pressure modes above the cylinder of DFG 2D-2 on M2 with one PISO corrector at dt 1e-3 (ADR 0405: one-step eigenvalues
> 1.77 to 1.91 at steps 16 to 20 for the first-order forcing and 0.97 at step 100 at the same Courant number; ten-step growth
> 1.88 per step for the bounded form, 1.31 for the first-order forcing) are traced by a registered measurement to the start
> state, by repeating the one-step and ten-step linearisations of ADR 0405 from a start field without flow through the cylinder
> (projected to be divergence-free), and the bounded form with one corrector either runs 200 steps from that start or its stop
> is located with the eigenvalue per step.

1. *a start field without flow through the cylinder (projected to be divergence-free)* (D1; S1, S2 on M2, U5 on M1);
2. *repeating the one-step and ten-step linearisations of ADR 0405 from that start* (D2; R0 the harness reproduces ADR 0405,
   G0' the linearity; the runs Q1, Q2);
3. *traced by a registered measurement to the start state* (the attribution rule T: A1 and A2);
4. *the bounded form with one corrector either runs 200 steps from that start or its stop is located with the eigenvalue per
   step* (B1).

**What "traced to the start state" means here (taken on the maintainer's standing instruction of 2026-11-11, take the
recommendation).** The two states compared differ only in the start field: the ADR 0249 start `S_old` (the inflow profile
`4 U_m y (H - y) / H^2` in every cell, the cylinder's included, and the blob of `v`) and the projected start `S_new` of D1 (the
same field with the solid cells set to zero and projected onto the fluid region with no flux through the cylinder's faces).
Everything else (mesh, forcing, settings, `dt`, the step numbers of the windows) is that of ADR 0405. The modes are **attributed
to the start state** when, from `S_new`, they are absent at the steps where `S_old` has them (the registered rule T below). If they
are still there, the start field is not their cause; the measurement then says that they belong to the early time of an
impulsive start (both starts begin at rest at the body), not to the flow through the cylinder, and clause 3 is missed.

## What was read (disclosed)

ADR 0405 in full (registration and addendum: R1, R2, G0 to G4, X1 to X4, Q2); ADR 0249 (the start field); the parts of ADR 0392
and ADR 0374 that ADR 0405 cites; `crates/rcfd-fv/tests/immersed_multistep_linearisation_f494.rs` in full (the code copied here);
`crates/rcfd-fv/src/pimple.rs` `PimpleSolver::new` (the initial flux interpolated from `u` by `face_flux`), `snapshot`, `restore`,
`reinitialise` (public: fields, an optional face flux, empty history, step 0), `PimpleState`; `convection.rs` `face_flux`;
`ddt.rs` `History`; `immersed.rs` `ImmersedForcing::body_of_cell` (the body whose solid holds the cell centre); `rcfd-mesh`
`Mesh::patch`, `Patch::faces`, `face_area_vectors`, `face_centres`, `cell_centres`, `cell_volumes`; `FvGeometry` `n_internal`,
`weights`, `empty`.

## Arithmetic before this registration (disclosed)

**The flow through the cylinder of the old start.** The cylinder (radius 0.05, centre at `y = 0.2`) sits in the middle of the
channel, where the inflow profile is near `U_m = 1.5`; on M2 (core spacing 0.0025) about `pi 0.05^2 / 0.0025^2 = 1257` cells
are solid. The old start puts about 1.5 m/s through them, and the forcing (`K = 1e7`, implicit) removes it in the first step;
the pressure of the first steps is that of this impulsive stop of the flow through the body (a velocity jump of order 1.5 m/s
over one step, a pressure of order `D U / dt`, about 150, against the order 1 of the developed flow). The projected start has no
such jump: the flow goes round the body from the first step.

**What does not change.** Both starts are at rest at the surface only after the first step (the old) or from the start (the new);
in both the boundary layer on the cylinder grows from zero, `sqrt(nu t) = sqrt(1e-3 x 0.016) = 0.004` at step 16, under two
cells of 0.0025. ADR 0405 found the pressure modes at fluid cells 0.008 to 0.016 from the surface above the cylinder, inside
two to six cells of the wall, where also `C_max` is (1.18 at step 15 for F). If the modes belong to the thin early boundary
layer or the accelerated flow over the top of the body at that time, they remain from `S_new`. **No outcome is predicted**; the
rule T says which reading the numbers support.

**The thresholds of T.** The one-step dominant of F was 1.767 (step 16) and 1.911 (step 20) from `S_old`, 0.971 at step 100 (X3)
and 0.969 in the channel without the cylinder (X1): the "physical" part of the spectrum at these states lies just below one.
Limit A1, 1.2: a reduction of at least `(1.767 - 1.2) / (1.767 - 0.971) = 71` percent of the gap at step 16 and 77 percent at
step 20, a margin of 0.23 above the late value for what remains of the early transient of a smooth start. Limit A2, 1.10 on the
ten-step `g` of F over steps 16 to 25 (1.311 from `S_old`): ADR 0405 derived from the bounded run of F that a growth above
about 1.08 to 1.13 per step is not sustained; 1.10 is in that band, and is a reduction of two thirds of the excess over one.
The reading B (the modes persist) is printed with its own numbers: the one-step dominant at step 16 or 20 at least 1.5, and `g`
of F at least 1.2.

## Design (D1 to D3), all in a new test file

New file `crates/rcfd-fv/tests/immersed_early_state_start_f501.rs`. Copied from `immersed_multistep_linearisation_f494.rs`
without change of behaviour: the complex type, the Hessenberg QR, inverse iteration, Arnoldi, the Ritz values and vectors, the
true residual, `lcg_vector`, the DFG helpers, `r_solver`, `Layout`, `step_map`, `jv`, `tighten`, `fluid_cells`,
`surface_distance`, `k_step_map`, `pkv`, `per_step`, `sigma_max`, `c_max`, `run_to`, `Analysis`, `analyse`. The fast checks U1
to U4 of ADR 0405 are not copied (they test the code in its own file, which stays in the gate); U3' below runs the copied
k-step map once on M1.

**D1. The projected start `S_new`.** Given the solver of set-up R (its geometry, its forcing `ib`, its start velocity `u*` of
ADR 0249 and the boundary conditions of `u`):

1. *Solid set.* A cell is solid if `ib.body_of_cell[c]` is `Some`; `u*_c = 0` in every solid cell.
2. *Face flux.* `phi*` = `face_flux(geom, u*, boundary values of u*)` (the library's own interpolation, as `PimpleSolver::new`).
   Every face with a solid cell on either side gets `phi* = 0` (no flow through the cylinder; those faces are closed in the
   projection).
3. *The projection.* For each fluid cell `P`, the corrected flux of an open face is `phi_f = phi*_f - a_f (psi_N - psi_P)`
   (oriented out of `P`), with `a_f = |S_f| / |d_PN|`, `d_PN` the vector between the cell centres (the mesh is Cartesian, so
   `S_f` is parallel to `d_PN` and this is the exact two-point normal gradient). Open faces: fluid-fluid internal faces, and the
   outlet `xmax` with `psi = 0` on the face, `a_f = |S_f| / |x_f - x_P|`. Closed (flux kept, no correction): the inlet `xmin`
   and the walls `ymin`, `ymax` (fixed velocity), the fluid-solid faces (flux zero) and the empty faces. Discrete continuity
   `sum_f phi_f = 0` in every fluid cell gives `sum_f a_f (psi_P - psi_N) = -sum_f phi*_f`, a symmetric positive definite system
   (diagonally dominant, the outlet Dirichlet makes it definite; the fluid region is connected to the outlet). Solved by
   Jacobi-preconditioned CG from zero to `|r| <= 1e-13 |b|` (at most 50000 iterations; a failure panics).
4. *Cell velocity.* Fluid cells: `u_P = u*_P - (1 / V_P) sum_f psi_f S_f` (the Gauss gradient of `psi` with outward `S_f`;
   `psi_f` by the geometric weight on fluid-fluid faces, `psi_P` on the closed faces, 0 on the outlet; empty faces skipped).
   Solid cells: zero. The `z` component stays zero (`S_f` has no `z` part except on the empty faces).
5. *Pressure* zero, as the old start.
6. The solver is put at `S_new` with `PimpleSolver::reinitialise(u, p, Some(phi), 0.0)` (public; empty history, step 0, as `new`
   does from `S_old`). The solver's first step is then taken from the projected flux, not from one re-interpolated from `u`.

The cell velocity of 4 is not exactly the face interpolation of `phi` (a collocated mesh has no exact pair); the flux the step
convects with and corrects is `phi`, which is divergence-free. This is disclosed, not tested.

**D2. The linearisations.** The one-step and ten-step products of ADR 0405 D1 to D3 (`e = 1e-4`, `m = 40`, the start vector
of seed 482, the true residual converged at 0.03), from the states after the same steps, now along the trajectory from `S_new`.

**D3. Locating a stop of the bounded form.** If B from `S_new` stops at step `n_s` (the step whose guard stops it), the window is
the `k` steps before it: `n0 = max(n_s - 11, 2)`, `k = n_s - 1 - n0` (steps `n0 + 1` to `n_s - 1`; ten steps when `n_s >= 13`).
The product `P_k` from the state after step `n0` is analysed by D2. A function `stop_window(n_s) -> Option<(n0, k)>` (`None`
when `k < 1`) carries this and is checked by U6.

## The runs

Set-up R of ADR 0374 / ADR 0405 (copied): DFG 2D-2 on M2, `U_m = 1.5`, `nu = 1e-3`, `K = 1e7`, `PimpleSettings::piso` with BDF2,
`LinearUpwind`, AMG pressure, momentum and pressure `tol_rel` 1e-6, `dt = 1e-3`, `flux_weight_check = false`, one corrector.
`F'`, `B'`: F and B from `S_new`.

| Id | Record | Forcing, start | What | Role |
| --- | --- | --- | --- | --- |
| R0 | Q1 | F, `S_old` | one step after step 16 (ADR 0405 X4 F16) | harness reproduces 1.767 |
| S | Q1, Q2 | F, B: `S_new` | the projection D1 | S1, S2 |
| PF' | Q1 | F' | `P_10`, steps 16 to 25 | G0', A2 |
| XF' | Q1 | F' | one step after steps 16, 20, 25, 100 | A1 (16, 20); 25 and 100 printed |
| RF' | Q1 | F' | on to step 200 | printed: runs or stops, `max abs(u)` |
| RB' | Q2 | B' | from `S_new` to step 200 or a stop | B1 |
| LB' | Q2 | B' | if B' stops: `P_k` of D3 | B1 |
| PB' | Q2 | B' | `P_10`, steps 16 to 25, if B' does not stop before step 26 | printed (against 1.881) |

## Registered checks

Heavy records carry `#[ignore]` and are run once by me in release: Q1 (R0, S, PF', XF', RF'), Q2 (S, RB', LB', PB'). The fast
checks run in release in the gate (U6 also in debug).

| Id | Clause | Check | Limit |
| --- | --- | --- | --- |
| S1 (Q1, Q2) | 1 | `S_new` on M2: `max_P abs(sum_f phi_f) / max_P sum_f abs(phi_f)` over all cells, for F and for B | `<= 1e-10` |
| S2 (Q1, Q2) | 1 | `S_new` on M2: `u` in every solid cell and `phi` on every face with a solid cell on either side | exactly 0 |
| R0 (Q1) | 2 (set-up) | F from `S_old`, one step after step 16: dominant one-step Ritz value | within 1e-3 relative of 1.767 (ADR 0405 X4: 1.767, converged 2.5e-4); otherwise a defective copy |
| G0' (Q1) | 2 | PF': `P_10 z` of the dominant vector with `e = 1e-4` against `1e-5` | `<= 1e-3` relative |
| A1 (Q1) | 3 | F' one step after step 16 and after step 20: no converged (true residual `<= 0.03`) Ritz value of magnitude above 1.2 | none at either step |
| A2 (Q1) | 3 | PF': dominant of `P_10` converged and `g = abs(mu)^(1/10)` | residual `<= 0.03` and `g <= 1.10` |
| B1 (Q2) | 4 | B' from `S_new`: runs 200 steps (no stop), or it stops at `n_s` and the product of D3 has a converged dominant with `g > 1` | either |
| U5 (fast) | 1 | D1 on M1 (F): (a) S1's ratio `<= 1e-10`; (b) S2 exact; (c) projecting `S_new` again changes `phi` by `<= 1e-10` of `max abs(phi)` (a projection); (d) the old start has a positive flux into the solid region (`sum` of `abs(phi*)` over the fluid-solid faces before closing `> 0`, printed) and `reinitialise` with `S_new` then two steps of R run without a stop | as stated |
| U6 (fast) | (rule, D3) | `attributed(d16, d20, g)` on the registered limits: (i) the ADR 0405 numbers (1.767, 1.911, 1.311) give false; (ii) (0.97, 0.98, 0.99) give true; (iii) (1.19, 1.19, 1.11) false, (1.19, 1.19, 1.09) true, (1.21, 1.0, 1.0) false; `stop_window`: (34) is (23, 10), (13) is (2, 10), (5) is (2, 2), (3) is None | as stated |
| U3' (fast) | 2 | the copied k-step map on M1 from `S_new` (F, after step 2, `k = 3`): `S^3(0)` with the settings of R is the direct trajectory's step 5 to the bit | bit-equal |
| N1 | (defaults) | no library file changed (`git diff --stat` against the base lists only the new test file and docs) | as stated |

The attribution rule T (clause 3): **the modes are attributed to the start state when A1 and A2 are both met.** A1 met and A2
missed, or the reverse, is a partial attribution (recorded as such). Reading B (printed): the converged one-step dominant at step
16 or 20 at least 1.5 and `g` of PF' at least 1.2.

Printed, no limit: the flux into the solid region of `S_old` and the CG iterations; for every product the Ritz values with per
step above 0.9, `sigma`, the nonlinear propagation, the shares and cells of the dominant vector; `C_max` at each analysed state;
the one-step spectra of F' after steps 25 and 100; whether F' runs 200 steps and its `max abs(u)` at step 200 (F from `S_old`:
2.210); PB' against 1.881; the wall time of each record.

**Negative controls.** R0 (the old start reproduces the old mode in the copied harness, so a disappearance from `S_new` is not a
broken harness); U6 (i) (the old numbers are not attributed); U5 (d) (the old start does have flow into the body).

### Mutations (scratch copy of the final tree, not committed)

Each applied alone by an exact text replacement checked to occur once, the named test run once in release, the file restored and
compared byte for byte with a kept copy.

| Id | Mutation | Must fail |
| --- | --- | --- |
| M1 | the sign of the flux correction (`phi* + a dpsi`) | U5 (a) |
| M2 | the fluid-solid faces left open (not zeroed and treated as fluid faces) | U5 (b) |
| M3 | the outlet coefficient dropped (`a_f = 0` on `xmax`) | U5 (a) or the CG panic in U5 |
| M4 | the A1 limit 1.2 made 2.0 | U6 |
| M5 | `stop_window` with `n0 = n_s - 10` | U6 |

## Decision rule

- **done**: S1, S2, R0, G0', A1, A2, B1, U5, U6, U3', N1 met, and every mutation caught by its named check.
- **partial**: S1, S2, R0, G0', U5, U6, U3', N1 met (the start delivered and the linearisations valid from it) and any of A1,
  A2, B1 missed, or a mutation not caught: the measured spectra and `g` from the new start are recorded as the located result.
- **todo**: S1, S2, R0, G0', U5, U6, U3' or N1 missed and not repaired.

A set-up check that does not hold (R0, U3') is a defective set-up: an amendment before any corrected run, never a pass.

## Limits (not covered)

One flow on one mesh, one `dt`, one projection (onto the fluid region with the solid faces closed; a projection over the whole
domain with the solid included is another start, not run). The cell velocity of the start is not the exact interpolation of its
flux. Both starts begin impulsively; a start from a developed flow (the separation of the early boundary layer from the start
field) is not covered. The windows are those of ADR 0405. No solver code is changed, so the refusal of ADR 0374 stays as it is.

## Open questions

- **Q1. Library start projection.** Options: (a) test code only (registered); (b) a public projection of a start field onto the
  fluid region of an immersed body. Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation): (a);
  (b) waits for the result.
