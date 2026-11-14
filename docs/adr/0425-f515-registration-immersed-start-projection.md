# ADR 0425: F-515 registration: the immersed start projection in the library (an opt-in start field without flow through the body, and the refusal of ADR 0374 lifted for it)

Status: **REGISTERED 2026-11-14, written before any code of F-515 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-515 was written, compiled or run: the documents and code under "What was read" were
read, and the arithmetic under "Arithmetic before this registration" was done by hand from numbers in the repository's records.
Results go in the addendum at the end; a defective set-up is corrected by an amendment written and committed before the
corrected run; nothing above an amendment or the addendum is changed after a run.

Parents: [ADR 0421](0421-f501-registration-immersed-early-state-start.md) (F-501: the projected start D1 as test code; B with
one corrector runs 200 steps from it, `max abs(u)` 2.20802 at step 200; its questions Q1 and Q2, from which F-515 was made),
[ADR 0374](0374-f363-registration-immersed-flux-with-one-piso-corrector.md) (F-363: the refusal D2 of a reversed flux face with
one PISO corrector, `ImmersedForcing::single_corrector_face`, quantity `immersed flux corrector`; the stops of Sb at step 126 and
B at step 34 from the ADR 0249 start), [ADR 0405](0405-f494-registration-immersed-multistep-linearisation.md) (F-494: the
modes of the early state). Roadmap item F-515 (deps F-501). Number 0425 given by the lead. Branch
`f515-immersed-start-projection` from main e4e858d1, worktree `w49i-work/f515`, scratch `w49i-work/f515-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run; a missed limit becomes an ignored
`*_known_miss` test with the limit intact and the number recorded; each registered run is made once; defaults stay
bit-identical; no hash table, golden or existing test file is edited (`immersed_early_state_start_f501.rs` is not edited; its
projection is copied into the new test file as the reference); no new third-party crate, no unsafe code; unsigned commits
without trailers; no push.

Clean-room: the projection is the discrete Helmholtz-Hodge projection on the cell faces (Chorin, Math. Comp. 22 (1968)
745-762, cited from memory; only the idea is used, derived below from the discrete continuity equation, as in ADR 0421 D1). The
linear system is solved by Jacobi-preconditioned conjugate gradients (Hestenes and Stiefel, J. Res. NBS 49 (1952) 409-436,
cited from memory, written from the definition). No vendor code, text, manual or binary.

## What F-515 asks

Acceptance (`docs/roadmap.json`), binding, clause by clause:

> An opt-in projection of the start velocity onto the fluid region of the immersed bodies (ADR 0421 D1: solid cells zero,
> fluid-solid faces closed, discrete divergence at most 1e-10 of the flux scale, the start face flux passed to the solver) is
> added with the defaults bit-identical (baseline_hashes unchanged); with it DFG 2D-2 on M2 with build_second_order_bounded and
> one PISO corrector at dt 1e-3 runs 200 steps (ADR 0421: max abs(u) 2.208 at step 200), and the refusal of the bounded form
> with one corrector of ADR 0374 is revisited for a projected start.

1. *an opt-in projection ... (ADR 0421 D1: solid cells zero, fluid-solid faces closed, discrete divergence at most 1e-10 of the
   flux scale, the start face flux passed to the solver)* (D1, D2; checks P1, P2, E1);
2. *with the defaults bit-identical (baseline_hashes unchanged)* (N1, N2, N3);
3. *with it DFG 2D-2 on M2 with build_second_order_bounded and one PISO corrector at dt 1e-3 runs 200 steps (max abs(u) 2.208
   at step 200)* (H1);
4. *the refusal of the bounded form with one corrector of ADR 0374 is revisited for a projected start* (D3; RF1 to RF4, L2).

**How "revisited" is taken (on the maintainer's standing instruction of 2026-11-11, take the recommendation; ADR 0421 Q2
recommendation (b)).** The refusal of ADR 0374 D2 is lifted for a solver whose start was projected by D2 and is kept, unchanged
and with the same message, for every other solver (the default). It is lifted for every flux-face form the refusal covers (the
bounded form, and `build_second_order` with weights above 1), because the refusal does not know which constructor built the
forcing and the measurement of ADR 0421 says that the stop it guards against comes from the start field. For the unbounded form
with weights up to 2 (Sb of ADR 0374) nothing has been measured from a projected start; L2 measures it, and a miss there makes the
item partial with the lift of that form as an open question (the design is not narrowed after the run). The divergence guard,
`flux_growth` (weights above 5) and every other check stay active.

## What was read (disclosed)

ADR 0421 in full (registration and addendum: D1, S1, S2, B1, Q1 to Q3, the proposed follow-up whose text is the acceptance
above); the parts of ADR 0374 on the refusal (D2, RF1, RF2, the message, the mutations M1 to M3, M6, the effect on two existing
tests) and its R0 numbers (Sb stops at step 126, B at step 34 from the ADR 0249 start); the head and Q2 of ADR 0405;
`crates/rcfd-fv/tests/immersed_early_state_start_f501.rs` (the DFG helpers, `r_solver`, `solid_cells`, `open_faces`,
`divergence`, `divergence_ratio`, `project_flux`, `projected_start`, `solid_is_closed`, `start_new`, record Q2);
`crates/rcfd-fv/tests/immersed_single_corrector_f363.rs` (the forcings F, Sa, Sb, B1, B; `rf_refusal_with_one_corrector`;
`n0_fingerprints_of_the_defaults`; `u1_single_corrector_face`); `crates/rcfd-fv/src/pimple.rs` (`PimpleSolver` fields, `new`,
`scratch`, `lag_preflight`, `preflight_run`, `lag_trial`, the refusal block of `step_with` after the corrector count,
`Divergence::message` for `immersed flux corrector`, `restore`, `reinitialise`); `crates/rcfd-fv/src/immersed.rs`
(`ImmersedForcing` fields, `single_corrector_face`, `flux_growth`, `solid_mask`, `SINGLE_CORRECTOR_WEIGHT_LIMIT`);
`crates/rcfd-fv/src/pressure.rs` (`PressureBc`, `pressure_bc`: `fixed_u`, `coupled`, the coefficients `a, b, c, d` of
`bc::FaceCoeffs`); `crates/rcfd-fv/src/geometry.rs` (`weights`, `empty`); `crates/rcfd-fv/src/lib.rs` (the module list).

## Arithmetic before this registration (disclosed)

**The tolerance of H1 against the F-501 record.** ADR 0421 printed `max abs(u)` 2.20802 for B' at step 200 (five decimals:
the value lies in `[2.208015, 2.208025]`). The library path differs from F-501's run in three respects: (i) the projection is
the library's (P1 requires it bit-equal to the copied F-501 reference on M2, so if P1 holds the start is the same to the bit);
(ii) `flux_weight_check` is on (the default) instead of off: with it on, `flux_growth` reports only faces without an image of
weight above 5, which the bounded form has none of (ADR 0254 E1), and the refusal is lifted by D3, so the step does the same
operations; (iii) nothing else. The trajectory is then expected to be the same to the bit and `max abs(u)` within the rounding
of the print, `5e-6`. If (i) were to fail by round-off only, a perturbation of relative size `1e-13` (the CG tolerance) growing
at the ten-step rate of B' measured by ADR 0421, 1.057 per step, gives `1e-13 x 1.057^200 = 1e-13 x e^(200 x 0.0554) = 1e-13 x
6.6e4 = 7e-9` at step 200, far below the print rounding. **Limit: `abs(max abs(u) - 2.20802) <= 2e-5`** (the rounding `5e-6`
with a margin of `1.5e-5`; it would catch a different start or a different step, which ADR 0421 shows changes this number in
the third decimal: F from the old start 2.210, F' 2.212).

**Sb from a projected start.** From the ADR 0249 start Sb with one corrector stops at step 126 (ADR 0374 R0) and runs with two;
its stop is later than B's (step 34), whose stop ADR 0421 removed by the projection. No number predicts Sb' beyond this; L2's limit
is the acceptance's own (runs 200 steps).

**Cost.** ADR 0421 record Q2 (B' 200 steps on M2 and a ten-step analysis) took 70 s; one 200-step run on M2 is about 40 s to 60
s in release on a shared machine. H1 and L2 are heavy records (`#[ignore]`, run once). P1 on M2 is the projection only (1585 CG
iterations on about 18000 cells, under a second) plus the forcing build.

## Design (D1 to D4)

**D1. The projection, as a library function** (new module `crates/rcfd-fv/src/immersed_start.rs`, `pub mod immersed_start` in
`lib.rs`):

```text
pub fn project_start(geom: &FvGeometry, u: &VectorField, p: &ScalarField, ib: &ImmersedForcing)
    -> rcfd_core::Result<ProjectedStart>
pub struct ProjectedStart { pub u: Vec<Vec3>, pub phi: Vec<f64>, pub report: StartProjection }
pub struct StartProjection { pub solid_cells: usize, pub into_solid: f64, pub cg_iterations: usize,
                             pub divergence_ratio: f64 }
```

ADR 0421 D1 1 to 4 with the outlet generalised from the patch `xmax` to the boundary conditions:

1. *Solid set* `ib.solid_mask()`; `u*_c = 0` in every solid cell.
2. *Face flux* `phi* = face_flux(geom, u*, boundary values of u*)`; `into_solid = sum abs(phi*_f)` over the internal faces with
   exactly one solid cell; every internal face with a solid cell on either side gets `phi* = 0`.
3. *The projection.* Open faces: the fluid-fluid internal faces, `a_f = |S_f| / |d_PN|`; and the boundary faces where the
   pressure is fixed (from `pressure::pressure_bc(geom, u, p)`: not `fixed_u`, not periodic-coupled, coefficient `a == 0`, i.e.
   the face value of `p` does not depend on the cell; not empty), with `psi = 0` on the face and `a_f = |S_f| / |x_f - x_P|`.
   Every other face is closed (flux kept). On DFG 2D-2 this is exactly the set of F-501 (open: fluid-fluid faces and `xmax`; the
   inlet and the walls have `fixed_u`, the empty faces are skipped). The system `sum_f a_f (psi_P - psi_N) = -sum_f phi*_f` in
   every cell (the solid cells have no open face and are decoupled, diagonal 1, `psi = 0`) is symmetric positive definite when
   every fluid region touches an open boundary face; Jacobi-preconditioned CG from zero to `|r| <= 1e-13 |b|`, at most 50000
   iterations. Corrected flux `phi_f = phi*_f - a_f (psi_N - psi_P)` on the open internal faces, `phi_f = phi*_f + a_f psi_P` on
   the open boundary faces.
4. *Cell velocity.* Fluid cells `u_P = u*_P - (1 / V_P) sum_f psi_f S_f` (`x`, `y` components; `z` kept), `psi_f` by the
   geometric weight on fluid-fluid faces, the cell value on faces with a solid side and on closed boundary faces, 0 on open
   boundary faces, empty faces skipped; solid cells zero.
5. `divergence_ratio = max_P abs(sum_f phi_f) / max_P sum_f abs(phi_f)` (S1 of ADR 0421; the flux scale is the largest sum of
   face-flux magnitudes of a cell).

The loops keep the operation order of the F-501 test code (internal faces in face order, then the open boundary faces in face
order; the same CG), so on DFG 2D-2 the result is expected to be that code's to the bit (P1).

Errors (`rcfd_core::Error::InvalidValue`, located): `ib` built for another mesh (`ib.n_cells() != n_cells`); no open boundary
face (the system is singular: a closed domain needs a pressure reference, not covered); CG not converged in 50000 iterations
(the message gives the residual ratio; for example a fluid region cut off from every open face).

**D2. The opt-in on the transient solver** (`pimple.rs`): `PimpleSolver::project_immersed_start(&mut self) ->
rcfd_core::Result<StartProjection>`. Refused with `InvalidValue` without `self.immersed` and after the first step
(`step_no != 0`). It calls `project_start(geom, &self.u, &self.p, ib)` and then `self.reinitialise(u, self.p.values.clone(),
Some(phi), self.time)` (the start face flux is the projected `phi`, not re-interpolated from `u`; ADR 0421 D1 6), and sets the
new private field `start_projected = true`. A getter `start_projected(&self) -> bool`. Nothing changes for a solver that does
not call it.

**D3. The refusal revisited.** The refusal block of `step_with` (ADR 0374 D2) gets the extra condition `!self.start_projected`.
`start_projected` is `false` from `new`; it is copied by `scratch` (so that `lag_preflight` and `lag_trial` of a projected
solver are not refused in their trial steps); `reinitialise` sets it to `false` (a new start field is not a projected one; the
method sets it again after its own call to `reinitialise`); `restore` leaves it unchanged (a snapshot of the same solver's
trajectory). The message of the refusal is not changed.

**D4. No case-file key.** A key for the projection in the case schema would change the schema golden; it is left out (rule 3)
and reported as the remaining step that needs the maintainer.

## Registered checks

New test file `crates/rcfd-fv/tests/immersed_start_projection_f515.rs`; the DFG helpers and the F-501 projection
(`solid_cells`, `open_faces`, `divergence`, `divergence_ratio`, `project_flux`, `projected_start`) copied from
`immersed_early_state_start_f501.rs` as the reference. The set-up R of ADR 0374 / ADR 0421 (DFG 2D-2, `U_m = 1.5`, `nu = 1e-3`,
`K = 1e7`, `PimpleSettings::piso` with BDF2, `LinearUpwind`, AMG pressure, momentum and pressure `tol_rel` 1e-6, `dt = 1e-3`),
with `flux_weight_check` **on** (the default of the constructors) except in the reference comparison, where it does not matter.
B: `build_second_order_bounded`, `theta_min = 0.01`; Sb: `build_second_order`, `theta_min = 0.25` (ADR 0374).

Heavy records carry `#[ignore]` and are run once by me in release: H1, L2. The fast checks run in release (those on M2 are
`cfg_attr(debug_assertions, ignore)`); E1 also in debug.

| Id | Clause | Check | Limit |
| --- | --- | --- | --- |
| P1 | 1 | `project_start` against the copied F-501 reference, on M1 and on M2 with B: `u` and `phi` | bit-equal |
| P1 (a) | 1 | the same, library result: `divergence_ratio` (reported) and the one recomputed by the test | both `<= 1e-10` |
| P1 (b) | 1 | `u` exactly 0 in every solid cell; `phi` exactly 0 on every internal face with a solid cell on either side | exact |
| P1 (c) | 1 | `into_solid > 0` (the old start does flow into the body) and the reported `solid_cells` equals the count of `solid_mask` | as stated |
| P2 | 1 | on M2 with B: the start face flux passed to the solver (in full below) | as stated |
| P2 (d) | 1 | the divergence ratio recomputed by the test from the solver's `phi` after the call | `<= 1e-10` |
| E1 | 1 | on M1: (a) `project_immersed_start` without a forcing is `Err`; (b) after one step is `Err`; (c) with the pressure condition at `xmax` made zero-gradient (no open face) is `Err`; (d) `project_start` with a forcing built for another mesh is `Err` | as stated |
| N1 | 2 | `cargo test -p rcfd-api --release --test baseline_hashes` | passes (file not edited) |
| N2 | 2 | the existing tests that cover the changed lines, unchanged, release: `immersed_single_corrector_f363.rs` (fast ones: `n0_fingerprints_of_the_defaults`, `u1_single_corrector_face`, `u2_lag_map_with_the_reconstruction`, `rf_refusal_with_one_corrector`), `immersed_early_state_start_f501.rs` (fast ones), and the `rcfd-fv` library unit tests of `pimple` | all pass |
| N3 | 2 | `git diff --stat` against e4e858d1 lists only `pimple.rs`, `lib.rs`, the new module, the new test file and docs; the `pimple.rs` hunks are the field, its initialisers in `new` and `scratch`, the condition of the refusal, the line in `reinitialise`, the method and the getter | as stated |
| RF1 | 4 | default kept: B with one corrector on M2, not projected: refused at step 1, stage `pressure correctors`, quantity `immersed flux corrector`, message contains "ADR 0374" and "n_correctors" | as stated |
| RF2 | 4 | lifted: B with one corrector on M2, projected: 3 steps without a stop | as stated |
| RF3 | 4 | cleared: B projected, then `reinitialise` with the same `u`, `p`, `phi`: `start_projected()` false and refused at step 1 | as stated |
| RF4 | 4 | the trial: B with one corrector on M2: `lag_preflight(1e-3, 5)` is `None` unprojected (the trial is refused) and `Some` projected | as stated |
| H1 | 3 | heavy: B with one corrector, `dt = 1e-3`, M2, check on, `project_immersed_start` then 200 steps: no stop; `max abs(u)` at step 200 (the F-501 definition, `hypot(u_x, u_y)` over the cells); also S1 ratio and S2 of the M2 start | no stop; `abs(max abs(u) - 2.20802) <= 2e-5`; ratio `<= 1e-10`; S2 exact |
| L2 | 4 | heavy: Sb with one corrector, `dt = 1e-3`, M2, check on, projected, 200 steps | no stop (printed: `max abs(u)` at step 200, or the stop) |

P2 in full: after `project_immersed_start` on the solver of B: `s.u.values` and `s.phi` bit-equal to `project_start` called
before on the same solver's fields; `s.p.values` and `s.time` bit-equal to before; `start_projected()` true (false before). (A re-projection of the flux, U5 (c) of ADR 0421, is not repeated: the library
projects a velocity field, and the face interpolation of the projected cell velocity is not the projected flux, so a second
call is not the identity.)

Printed, no limit: the CG iterations, `into_solid`, the solid cell count, the wall time of each record, the step and the value
of a stop if any.

**Negative controls.** RF1 and the unprojected half of RF4 (the default still refuses); P1 (c) (the start does have flow into the
body, so the closure is not vacuous); E1 (c) (the generalised outlet is read from the pressure condition).

### Mutations (scratch copy of the final tree, not committed)

Each applied alone by an exact text replacement checked to occur once, the named tests run once in release, the file restored
and compared byte for byte with a kept copy.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | the lift never applies (`!self.start_projected` made `true` in the refusal condition) | RF2 |
| MU2 | the lift always applies (`!self.start_projected` made `false`) | RF1 |
| MU3 | `reinitialise` does not clear the flag | RF3 |
| MU4 | `scratch` does not copy the flag (`false`) | RF4 |
| MU5 | the sign of the flux correction on the internal faces | P1 |
| MU6 | the fluid-solid faces not zeroed in `phi*` | P1 |
| MU7 | the fixed-pressure boundary faces not opened | P1 (an `Err`, unwrapped) |
| MU8 | the method passes `None` for the flux to `reinitialise` (re-interpolated from `u`) | P2 |

## Decision rule

- **done**: P1, P2, E1, N1, N2, N3, RF1, RF2, RF3, RF4, H1 and L2 met, every mutation caught by its named check, no amendment.
- **partial**: P1, P2, N1, N2, N3, RF1, RF3 met (the projection delivered, the defaults and the default refusal kept) and any of
  E1, RF2, RF4, H1, L2 missed or not run, or a mutation not caught, or a check met only after an amendment made after its run.
- **todo**: P1, P2, N1, N2, N3, RF1 or RF3 missed and not repaired.

A set-up defect (P1 not bit-equal because of a defective copy of the reference) is corrected by an amendment before a corrected
run, never a pass.

## Limits (not covered)

The projection uses the two-point coefficient `|S_f| / |d|`: its flux is divergence-free on any mesh (the system is the discrete
continuity equation for whatever positive `a_f`), but on a non-orthogonal mesh it is not the exact normal gradient of `psi`, so
the cell velocity of step 4 is a less exact Gauss-gradient correction there; only Cartesian DFG meshes are checked. The cell
velocity is not the exact interpolation of the flux (as in ADR 0421). Periodic faces are closed (their flux kept), moving bodies
are projected at their position at the start time with the solid at rest (the rigid velocity of a moving solid is not imposed);
neither is checked. Closed domains (no fixed-pressure face) are refused. The fractional step, SIMPLE and the other solvers do not
get the opt-in (only `PimpleSolver`); a caller can use `project_start` and `reinitialise` with any solver that takes a face flux.
One flow, one mesh pair, one time step; the lift for Sb rests on L2 alone; other forms and `dt` are not measured. No case-file key
(D4).

## Open questions

- **Q1. Which forms the lift covers.** Options: (a) every form the refusal covers, when the start is projected (registered); (b)
  only forcings without a non-image face of weight above 1. Taken on the maintainer's standing instruction of 2026-11-11 (take
  the recommendation): (a), measured by L2 for Sb.
- **Q2. A case-file key** for the projection (schema golden). Not taken (rule 3): a remaining step for the maintainer.
