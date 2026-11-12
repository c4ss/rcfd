# ADR 0347: F-445 registration: prescribed flux on the faces between calculated and fringe cells of the overset step

Status: **REGISTERED 2026-11-12**, written before any code of F-445 and before any run of its checks. Not yet accepted by the
maintainer. Before this text nothing was compiled or run: the code, the documents and the existing run logs listed under
"What was read" were read, and the arithmetic under "Arithmetic before this registration" was done by hand. Results go in
the addendum at the end; nothing above the addendum is changed after a run.

Parents: [ADR 0340](0340-f349-registration-overset-background-fringe-response.md) (F-349: the set-ups RE and RL, `R_b`,
the following coefficient `a_c`, the kink relation `dJ = -(16 nu / 3 h) D`, the switch described in its addendum; question
2, whose recommendation is this item, taken on the maintainer's standing instruction of 2026-11-11),
[ADR 0219](0219-f134-registration-overset-coupling-of-a-moving-component-mesh.md) (F-134: the model M1 to M11, the check
U1), [ADR 0246](0246-f334-registration-overset-first-order-term.md) (F-334),
[ADR 0245](0245-f344-registration-bdf2-step-remainder.md) (F-344: the opt-in `rhie_chow_step_consistent`, which touches
the overset step), [ADR 0238](0238-f335-registration-overset-outer-iterations.md), [ADR 0241](0241-f338-registration-overset-bdf2.md),
[ADR 0251](0251-f336-registration-overset-quadratic-donor-interpolation.md). Roadmap item F-445 (depends on F-349, F-344).

Number: the lead gave 0347. Branch `f445-overset-prescribed-fringe-flux` from main 8165a0b3, worktree `w49i-work/f445`.

House rules (the campaign rules of 2026-11-12): limits and deciding numbers are fixed here and are not changed after a run;
a missed limit becomes an ignored `*_known_miss` test with the limit intact and the measured number recorded, and is
diagnosed, not tuned away; a defective set-up gets a corrected test by an amendment committed before the corrected run,
the old test staying as an ignored record; each registered run is made once; parameters are fixed here from a derivation;
defaults stay bit-identical; `baseline_hashes.rs`, the hash tables and the goldens are not touched; existing tests and
ignored records are not edited; no new third-party crate, no unsafe code; unsigned commits without trailers; no push.

Clean-room: everything below follows from the finite-volume definitions of the solver (ADR 0219 M1 to M11), the
Green-Gauss theorem, Taylor's theorem and the potential theory of the Laplace equation. No paper, book, code or vendor
document was read for this work.

## What F-445 asks

Acceptance (quoted, binding): "An opt-in of the overset step under which the flux of a face between a calculated and a
fringe cell is the face interpolation of the interpolated fringe velocity plus one correction per closed fringe ring that
makes the net flux of each enclosed part zero (ADR 0340, the switch described there): on RE of ADR 0340 at levels 1 to 3
the following coefficient a_c at both fringes is at least 0.9 at t = 0.025 and the order of R_b at t = 0.5 is at least 1.7
on both pairs, on RL the order of R_b is at least 1.7 within the parity families (levels 1 to 3 and 2 to 4), continuity of
the calculated cells and the mass of the domain hold to 1e-9, the uniform-flow check U1 of ADR 0219 holds, and without the
opt-in every overset result is bit-identical; or the ADR shows with a number why the prescribed flux is not admissible."

**Reading (registered here).** Six clauses, each with its check: `a_c` (A1), the order on RE (A2), the order on RL within
the parity families (A3), continuity and mass (C1, C2), U1 (U1), bit-identity without the opt-in (B1, B2). The last
sentence is an alternative for the case that no variant of the prescribed flux runs admissibly (see the decision rule).

## What was read (no run, nothing compiled)

- ADR 0340 in full; ADR 0219 (the model M1 to M11, the set-up U and the checks U1 to U4); the heads of ADR 0246 and ADR
  0344 (form); `crates/rcfd-fv/src/overset.rs` in full; `crates/rcfd-fv/src/pimple/face_consistent.rs` (the functions the
  overset step calls under `rhie_chow_step_consistent`); `green_gauss` and `gg_pass` of `gradient.rs`; `corrected_flux`
  and `corrected_flux_impl` of `pressure.rs`; `crates/rcfd-fv/tests/overset_f349.rs` in full; the set-up U and the test
  `u_uniform_flow_through_a_moving_rotating_component` of `overset_f134.rs`.
- **The logs of the runs of ADR 0340** in `E:/projects/cfd/w49i-work/f349-scratch` (existing output, nothing re-run):
  `f349-e_equal_spacings_levels_1_to_3.log`, `f349-fast.log`, `f349-a1-fast.log`, `f349-f_finer_level_4.log`. Numbers
  used below are quoted from them: on RE with the default coupling `a_c` at the background fringe is `0.122`, `0.288`,
  `0.404` at `t = 0.025` and `0.573`, `0.719`, `0.791` at `t = 0.5` (levels 1 to 3), at the component fringe `-0.720`,
  `-0.762`, `-0.619` and `0.347`, `0.612`, `0.729`; the truncation of the background fringe of RE has the root mean square
  `3.820e-4`, `9.586e-5`, `4.337e-5` and the mean vector `(1.433e-4, -1.433e-4)`, `(4.11e-5, -4.11e-5)`, `(1.82e-5,
  -1.82e-5)`; the background fringe ring of RE has 24, 48, 76 cells at `3.25 H` to `3.75 H` from the component centre and
  32, 56, 84 faces to calculated cells; run times of a pair (base, Qb) on a loaded machine: RE 33 s, 249 s, 815 s (levels
  1 to 3), RL at level 1 28 s, RL at level 4 2382 s.

## Arithmetic before this registration (disclosed, by hand; no script)

Notation of ADR 0340: `C` a calculated cell, `F` a fringe cell with a common face `f`, `Gamma` the set of such faces
(`Gamma_B` on the background, `Gamma_C` on the component), `n` the normal out of `C`, `t = d_F . n` and `c = d_C . n` the
normal changes of the two cell velocities between two runs, `m` the change of the flux over the face area, `theta = rAU /
dt`, `h` the cell size.

1. **The prescribed flux alone leaves a second soft link.** With `phi_f = (u)_f . S_f` on `Gamma` the flux defect `D` of
   ADR 0340 is zero and the kink relation between `D` and the pressure jump is gone. But the velocity of `C` is `u_C =
   HbyA_C - rAU (grad p)_C`, and the Green-Gauss gradient of `C` takes the face value `(p_C + p_F) / 2` on `f` with the
   interpolated `p_F` of the other part. A change `dJ` of `p_F` against the continuation of the pressure of the own part
   changes that gradient by `dJ / (2 h)`. Now the pressure of the own part is fixed, up to a constant, by its own fluxes
   (the rows of the calculated cells no longer hold `p_F`): if the part must change its flow by `m` within a step it
   needs a pressure change of the size `m r / rAU` at the distance `r` from its centre, which the other part does not
   share. For the uniform mode of the component part (a change `m` of the through-flow) the pressure change at the ring is
   `-(m / rAU) x`, the gradient of the ring cell changes by `+(m / rAU)(r_F - 2 h) / (2 h)` (`r_F` the distance of the
   fringe centre from the component centre) and its velocity by `-m (kappa_F / 2 - 1)`, `kappa_F = r_F / h`: the ring cell
   is pushed against the flow change, the more the finer the mesh. Two consequences, depending on how `(u)_f` is taken:
   - **lagged** (`(u)_f` of the velocity the corrector starts from): the change of the ring cell feeds the next
     prescription by its half, `m' = c / 2`, so a disturbance of the uniform mode is multiplied by `g = -(kappa_F - 2) / 4`
     per corrector. On RE `kappa_F = 7`, `14.5`, `22` at levels 1 to 3 (the component has `15 L` cells per side): `g =
     -1.25`, `-3.1`, `-5.0`. **Predicted: unstable**, the faster the finer the mesh;
   - **implicit** (if the loop were converged): the ring cell follows a change of the fringe velocity with the time
     constant `(h^2 / (3 nu)) (1 + kappa_F / 2)` (the pressure push against the viscous pull of the two neighbours),
     `0.093`, `0.042`, `0.027` on RE: `a_c` at `t = 0.025` would be about `0.24`, `0.45`, `0.60`. The soft imposition
     would be back, one cell further in.
2. **With the pressure gradient of the ring cells taken from their own part the loop is a contraction.** Let the gradient
   of `C` use, on its faces in `Gamma`, the linear extrapolation `p_C + (grad p)_C . (x_f - x_C)` instead of the face
   interpolation with `p_F`. On a uniform Cartesian mesh the normal part of the gradient is then `(p_C - p_Cm) / h` (`Cm`
   the neighbour on the far side), the same difference as in the flux of the face between `C` and `Cm`. From that flux,
   `m_in = (HbyA_C + HbyA_Cm) / 2 - rAU (p_C - p_Cm) / h + theta D_old`, follows `u_C . n = m_in + (HbyA_C - HbyA_Cm) / 2 -
   theta D_old`: the ring cell takes the flux of its inner face, whatever the pressure does, and no foreign pressure
   enters. The truncation of the one-sided difference goes into the value of `p_C` (second order), not into the velocity.
   With the lagged prescription `m = (c + t) / 2` and `abs(m_in) <= abs(m)` (equal for a mode that is smooth along the
   ring, smaller otherwise), a disturbance of `c` is multiplied by at most `1/2 + (1 - theta) / 2` per corrector (the
   second part through `HbyA`, whose neighbour part has the weight `1 - theta`; `theta` is `0.86` to `0.67` on levels 1 to
   3): **predicted stable**. The calculated cells of each part are then a problem with a given normal velocity on `Gamma`
   (the ghost-cell form of a velocity condition: the face value `(u_C + u_F) / 2` in the flux, `u_F` in the diffusion and
   convection stencils), and the pressures of the parts are tied only by their constants.
3. **The lag in time.** The predictor already holds the acceleration of the old pressure gradient, so in a flow of
   constant acceleration the velocity the first corrector starts from is the new velocity: the lag of the prescription is
   of the size of the change of the acceleration over a step, second order in the step. A uniform flow with a constant
   acceleration `a` and the pressure `-a . x` is reproduced exactly (check UA).
4. **`a_c` has a geometric ceiling below 0.9 at the background fringe on these levels.** The mean vector of the
   truncation of the background fringe is half of its root mean square (logs), so the leading change of the normal fringe
   velocity around the ring is that of a uniform vector, a dipole. Outside a closed ring the incompressible answer to a
   dipole of normal velocity decays like `(r_0 / r)^2`; the calculated cell lies one cell further out than the fringe cell.
   With the flux `(c + t) / 2` at `r_F + h / 2` and `c` the field at `r_F + h`: `c = rho t / (2 - rho)`, `rho = ((r_F + h /
   2) / (r_F + h))^2`; with `r_F = 3.5 H` (a circle in place of the square ring): `0.65`, `0.78`, `0.84` at levels 1 to 3.
   The default coupling reaches `0.57`, `0.72`, `0.79` at `t = 0.5` (logs) and `0.63`, `0.76`, `0.85` on RL at `t = 3`
   (ADR 0340): the late values of the soft coupling sit at this ceiling. So the limit 0.9 of the acceptance at the
   background fringe is **predicted missed on levels 1 to 3 whatever the coupling**; it is registered unchanged (A1), and
   two measures that do not carry the geometry are registered with it (S1, S2). Inside the component ring the answer to a
   dipole is a uniform flow and the ceiling is close to one.
5. **Orders.** If the response follows at once, `R_b` is the truncation times a factor that carries the ceiling of item 4:
   order about `2 - log2(0.78 / 0.65) = 1.74` (levels 1 to 2) and `2 - ln(0.84 / 0.78) / ln 1.5 = 1.82` (2 to 3). Close to
   the limit 1.7; no trusted estimate for RL, whose truncation is modulated along the ring with a period of three cells
   (a mode that the ring cell follows by a third only, item 2 with `m_in` near zero).
6. **Cost.** A pair costs what it cost in ADR 0340 (one pressure solve per corrector instead of one plus the response to
   the offset on the first): RE levels 1 to 3 about 20 minutes, RL levels 1 to 3 about 20 minutes, RL level 4 about 40
   minutes, on a loaded machine.

## The design

Opt-in of the solver, default off: `OversetSolver::set_fringe_flux(FringeFlux)`, with

- `FringeFlux::PressureEquation` (default): the step of ADR 0219, bit for bit;
- `FringeFlux::Prescribed` (**variant P**, the switch as ADR 0340 words it): D1 to D4 below;
- `FringeFlux::PrescribedOwnGradient` (**variant G**, the opt-in this item delivers if it holds): D1 to D5.

P is built because it is the literal switch and because item 1 predicts with a number that it is not admissible; G is
built because item 2 predicts that it is. Both share every line but D5.

**D1. The prescribed flux.** In every pressure corrector, on every internal face `f` between a calculated cell `C` and a
fringe cell `F` of the same part: `phi_f = (u)_f . S_f + sigma_f q_R A_f`, where `(u)_f` is the library's linear face
interpolation (`face_flux`) of the velocity field the corrector starts from (after the predictor: the predicted values
of the calculated cells and the donor interpolants of them in the fringe cells, which the fringe rows of the momentum
system give; after a corrector: the corrected values and their interpolants, M8), `sigma_f = +1` when `C` owns `f`, `A_f`
the face area and `q_R` one number per region (D2). It is imposed through the existing operators: the face coefficient
`gamma_f` of the pressure equation is set to zero on these faces and `phiHbyA_f` to the prescribed value, so the matrix,
its right-hand side and `corrected_flux` use it without a further change, and the rows of the calculated cells hold no
fringe pressure. The transient Rhie-Chow term and the terms of ADR 0245 are not added on these faces (they are parts of
the flux of the pressure equation, which is replaced there).

**D2. One correction per ring.** A *region* is a connected set of calculated cells (joined by faces between calculated
cells); in RE, RL and U there are two, the calculated background cells and the calculated component cells, each bounded by
the domain or wall boundary and by one closed ring of faces of D1. For a region `R` with the ring area `A_R = sum A_f`:

- no boundary face of `R` fixes the pressure: `q_R = (sum of the right-hand side of the pressure rows of R with q = 0) /
  A_R`. The rows of `R` then sum to zero, which is the condition for the pressure equation of `R` to have a solution (its
  matrix has the constant null vector): the net flux of `R` is zero and the soft pin adds no mass;
- a boundary face of `R` fixes the pressure and `R` is a background region: `q_R = -(sum_f sigma_f (u)_f . S_f) / A_R`,
  the net flux through the ring is zero. The ring encloses the hole region, whose fluid has a constant volume (ADR 0219
  M7); the offset `delta` of M7 is not used (reported as zero);
- a boundary face fixes the pressure and `R` is a component region: `q_R = 0`.

`MassMode` has no effect under P and G. A region without a ring face gets no correction.

**D3. The pressure levels.** The matrix keeps its fringe rows (the interpolation of the pressure, now one-way) and is
solved as today (GMRES with AMG). A region without a fixed pressure is pinned softly: the region that holds the reference
cell of a closed domain at the reference value (exact, since its rows are compatible), any other at its lowest cell with
the value that cell has. Such another region is then shifted by a constant `c_R` so that the area-weighted mean over its
ring of `p_F - p_C - (grad p)_C . (x_F - x_C)` is zero, `(grad p)_C` the gradient of D5 (which does not depend on the
constant) and `p_F` the interpolant from the donors, which must lie in regions whose level is already fixed (two regions:
always; otherwise the regions are taken in an order that satisfies this, and a step in which none exists is refused).
After a shift the fringe pressures are interpolated again. For a linear pressure the condition is exact.

**D4. After the solve.** As M8: `corrected_flux` (the prescribed value on the ring), the velocity of the calculated cells
`HbyA - rAU grad p`, the fringe and hole velocities, the flux of the faces without a calculated cell.

**D5 (G only). The pressure gradient of a ring cell from its own part.** For a calculated cell `C` with faces in `Gamma`
the gradient `g` used in the momentum predictor and in the velocity correction solves `g = (1 / V) [sum over the other
faces of the Green-Gauss terms + sum_{f in Gamma} sigma_f S_f (p_C + g . (x_f - x_C))]`, a 3 by 3 system per ring cell,
`(I - (1 / V) sum sigma_f S_f (x_f - x_C)^T) g = g_GG + (1 / V) sum sigma_f S_f (p_C - p_f)` with `g_GG` and `p_f` the
library's gradient and face value. On a Cartesian cell with one such face the matrix is `I - n n^T / 2` and the normal
part becomes the one-sided difference of item 2. A cell whose matrix has a determinant below `0.05` (fringe cells on
opposite sides) is refused. The gradient of ADR 0245's lagged pressure (`overset_lagged_gradient`) is not changed.

**Composition.** Outer iterations, BDF2 and `rhie_chow_step_consistent` are accepted together with P and G (the lag of D1
shrinks with the outer iterations; the harmonic face coefficient and the commutator of ADR 0245 act on the other faces).
Restriction, stated: under `rhie_chow_step_consistent` the lagged gradient of a ring cell keeps the interpolated fringe
pressure; the combination is covered by a smoke check (X) only. `set_fringe_flux` away from the default sets the stored
`delta` to zero.

**Defaults.** The new code is reached only when the mode is not `PressureEquation` and the step has fringe cells. No
existing function changes its result; `PimpleSettings`, the case tree, the schema and the goldens are not touched.

## Set-ups

RE and RL, the runs `base` and `Qb`, `R_b`, the classes and the diagnostics are those of ADR 0340 (`overset_f349.rs`,
helpers copied into the new file `crates/rcfd-fv/tests/overset_f445.rs`), with the fringe mode as a parameter of a run.
U is the set-up U of ADR 0219 (80 steps). **UA** (new): the meshes, hole and pose of RE at level 1, `nu = 0.1`, the
settings of ADR 0340; velocity `U(t) = (1 + 2 t, 0.5 + t, 0)` fixed on the four background sides at the new time, start `u
= U(0)`, `p = -a . (x - x_0)` with `a = (2, 1, 0)` and `x_0` the centre of the reference cell 0; 40 steps of `0.0025`.
The exact discrete solution is `u = U(t)`, `p = -a . (x - x_0)` (implicit Euler is exact for a velocity linear in time; a
uniform velocity has no convection or diffusion; the interpolation and D3, D5 are exact for a linear pressure). **UA2**:
UA under G with `0.7` added to the starting pressure of the calculated component cells.

## Registered checks

Release, `cargo test --release -p rcfd-fv --test overset_f445`; the unit check G0 in `crates/rcfd-fv/src/overset.rs`.
Tests of more than about two minutes are `#[ignore]` heavy records, run once; a fast test covers the same code at level
1. An order between levels run in different test processes is formed in the addendum from the printed numbers and kept in
a record test (as ADR 0340 did).

| Id | Check | Limit | Estimate behind it |
| --- | --- | --- | --- |
| A1 (acceptance) | G, RE, levels 1 to 3, `t = 0.025`: `a_c` on `Gamma_B` and on `Gamma_C` (base against Qb) | `>= 0.9` each | item 4: `Gamma_B` about `0.65`, `0.78`, `0.84`, **predicted missed** for the geometry; `Gamma_C` 0.9 to 1 |
| A2 (acceptance) | G, RE: the orders of `R_b` at `t = 0.5`, levels 1 to 2 and 2 to 3 | `>= 1.7` each | item 5: about `1.74`, `1.82` |
| A3 (acceptance) | G, RL: the orders of `R_b` at `t = 0.5`, levels 1 to 3 and 2 to 4 | `>= 1.7` each | none trusted (item 5) |
| C1 (acceptance) | continuity of the calculated cells (`OversetInfo::continuity`), every step of every run under P or G | `<= 1e-9` | solver tolerance, `1e-13` in ADR 0340 |
| C2 (acceptance) | mass of the domain, every step of every run under P or G: `abs(hole_flux)` over the largest face area; in the closed set-ups also the net flux through the domain boundary over the largest face area; in U `abs(Q_xmax - 0.5) / 0.5` | `<= 1e-9`, `<= 1e-9`, `<= 1e-10` | D2: round-off |
| C3 | unconverged linear solves, every run | `0` | ADR 0340: none |
| U1 (acceptance) | U under G and under P, every step, over the non-hole cells: `max abs(u_c - U) / abs(U)`, `max abs(p_c) / abs(U)^2`; with it U3's continuity and the set-up check U4 (kind changes `>= 20`) | `<= 1e-10` each | ADR 0219: `1.4e-14`, `6.2e-13` |
| B1 (acceptance) | without the opt-in: the FNV hash of `u`, `p`, `phi` after 40 steps of RE at level 1 and after 20 steps of U equals the hash the library of 8165a0b3 gives (measured once on a `git archive` copy of that commit with the same test source, before the library is edited); the same after `set_fringe_flux(PressureEquation)` | exact | D "Defaults" |
| B2 (acceptance) | `cargo test -p rcfd-api --release --test baseline_hashes`; the existing `overset_*.rs` test files of `rcfd-fv` and the unit tests of `rcfd-fv` (`--lib`), unedited, release | all pass | no existing function is changed |
| S1 | G, RE, levels 1 to 3, `t = 0.025` and `0.5`, both fringes: `abs(a_m - (1 + a_c) / 2)` (the flux follows the face interpolation of the two velocity changes) | `<= 0.02` | D1; the lag is of second order in the step (item 3) |
| S2 | G, RE, levels 1 to 3, both fringes: `a_c(0.025) / a_c(0.5)` (the following is there at once) | `>= 0.9` | item 2; the default coupling has `0.21`, `0.40`, `0.51` on `Gamma_B` (logs) |
| S3 | RE, levels 1 to 3, `t = 0.5`, base runs: the root mean square of the velocity error against the exact solution over the calculated cells (both parts), G over the default coupling | `<= 1.5` | item 2: the one-sided gradient costs no order in the velocity |
| UA | UA under the default coupling, P and G, every step, non-hole cells: `max abs(u - U(t)) / abs(U(t))`; `max abs(p - p_exact)` | `<= 1e-10`; `<= 1e-9` | exact (set-up) |
| UA2 | UA2 (G): the velocity as UA at every step; the pressure as UA from the end of the first step on | `<= 1e-10`; `<= 1e-9` | D3, D5 |
| G0 (unit) | D5 on the composite of RE at level 1: `p = 3 + 2 x - 5 y` in the calculated cells and arbitrary values (`1e3` times a fixed sequence) in the fringe cells: the gradient of every ring cell against `(2, -5, 0)`; a cell with ring faces on opposite sides is refused | `<= 1e-12`; `Err` | exact for a linear field |
| QS (control) | as ADR 0340: in a Qb run no component stencil changes and every background fringe stencil has nine calculated component cells with the linear donor first, no fallback | exact | ADR 0340 |
| NP (prediction, P) | P, RE, levels 1 and 2, base and Qb: P is **not admissible** if a run ends in an error, or `max abs(u)` passes 10, or C1 is violated, before `t = 0.5`; recorded: the step at which that happens and the growth per step of `max abs(u - u_exact)` over the last ten steps before it. If both levels run through: `a_c`, S1 and the order of `R_b` are printed and P is judged by the limits of A1 and A2 on that pair | (classification) | item 1: unstable, growth `1.25` and `3.1` per corrector at levels 1 and 2 |
| X (smoke) | RE at level 1, 40 steps, G with (i) `rhie_chow_step_consistent`, (ii) BDF2, (iii) two outer iterations (`alpha = 1`): C1, C3, and the root mean square of the velocity error over the calculated cells over that of G alone | `<= 1e-9`, `0`, `<= 2` | none trusted; a guard against a gross fault |

Printed with every pair, no limit: the diagnostics of ADR 0340 (classes, bands, `a_c`, `a_m`, the pressure jumps), `q_R`
is not printed (not exposed); the pressure error against the exact solution with the mean over the calculated cells of
each part removed.

### Mutations (scratch copy, not committed)

Each applied alone to a `git archive` copy of the final tree in `E:/projects/cfd/w49i-work/f445-scratch`, the named test
run once, and it must fail. `scripts/limit_audit.py` is run on the new test file and every hit is read.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | D2: `q_R = 0` | C1 or C2 (fast test at level 1) |
| MU2 | D5: the extrapolation term dropped (the face value `p_C`, matrix `I`) | G0 |
| MU3 | D1: the prescribed flux from the fringe velocity alone (`u_F . S_f`) | S1 (fast test at level 1) |
| MU4 | D3: the shift `c_R` not applied | UA2 (pressure) |
| MU5 | the default mode takes the branch of D1 | B1 |
| MU6 | D1: `gamma_f` not set to zero on the ring | S1 or C1 |
| MU7 | D5 not applied in the momentum predictor | UA2 (velocity) |

### Classes

A miss of A1 to A3, C1 to C3, U1, S1 to S3, UA, UA2, X is reported with its number and kept as an ignored `*_known_miss`
test (or record test) with the limit intact. A control (QS, B1, B2, G0) that does not hold, or a mutation that does not
make its check fail, is a defective set-up or a fault: corrected by an amendment before any re-run, never counted as a
pass. A design change that the runs make necessary is registered by an amendment committed before its code and runs.

## Predictions (mine, before any run)

C1 to C3, U1, B1, B2, UA, UA2, G0, QS, S1, S3 met. NP: P not admissible at both levels (if it runs through I was wrong
about item 1 and say so). A1 missed on `Gamma_B` at levels 1 to 3 (about `0.65`, `0.78`, `0.84`), met on `Gamma_C`. S2 met.
A2 near the limit, I do not know on which side; A3 unknown, more likely missed than met.

## Decision rule

- **done**: under G every acceptance check (A1, A2, A3, C1, C2, U1, B1, B2) is met, the controls held and the mutations
  failed their checks; or the alternative clause: neither P nor G is admissible, each shown by a registered number (NP's
  classification, or C1, C2 or U1 violated), in which case the opt-in is not offered (the modes are removed or refused)
  and the number is the result.
- **partial**: the opt-in G is built and C1, C2, U1, B1, B2 hold for it, and one or more of A1, A2, A3 is missed with the
  measured number and a diagnosed cause; or a control or mutation did not hold and was not corrected.
- **todo**: nothing is built, or no registered run could be made.

## Limits of this registration

- Taylor-Green on RE and RL only (uniform Cartesian parts, parallel grids, the component at rest, `nu = 0.1`); the
  moving component is covered by the uniform flow U alone (no accuracy check of a moving component under G).
- `R_b` is the response to the truncation of the background fringe (ADR 0340); the error against the exact solution is
  guarded on RE only (S3).
- RL at level 5 and RE above level 3 are not run (hours). P is run on RE at levels 1 and 2 only.
- D5 is first order in the pressure gradient of the ring cells; the pressure error is printed, not limited.
- No check isolates a fault confined to D5 in the velocity correction (MU2 and MU7 cover the formula and the
  predictor); A1, A2 and S2 depend on it.
- Several rings in one region share one correction; the level rule D3 is exercised with two regions only.
- The combination with `rhie_chow_step_consistent`, BDF2 and outer iterations has a smoke check only.
- No case-tree key: the opt-in is a call on `OversetSolver`, as the other overset opt-ins.

## Open questions

Listed with options and a recommendation in the addendum, after the results.

## Amendment 1 (2026-11-12, written after the code was committed and after one run of the unit check G0, before any other run of a check)

The library code and the tests were committed as `ce343abb` (compiled, formatted). Three things need this amendment
before the checks run.

**G0 missed, a defective check.** G0 ran once: the gradient of D5 is off by `2.02e-12` against the limit `1e-12` (76 ring
cells; with the fringe values the Green-Gauss gradient is off by `3.1e3`, the control). The limit is absolute and the
registered fringe values have the size `1e3`: their terms `p_f S_f / V`, of the size `500 / h = 6.4e3`, are added by the
Green-Gauss sum and taken off again by D5, which leaves a round-off of `6.4e3` times the machine precision, `1.4e-12`, per
face. The registration wrote "exact for a linear field" and did not count this. G0 is kept as the ignored unit test
`g0_..._known_miss` with its limit intact and is not run again. Corrected check **G0b**: the same with fringe values of
the size of the pressure itself (10 times the fixed sequence, at most 5 in size), limit `<= 1e-12` (round-off estimate:
`5 / (2 h) * 2.2e-16 * 4 = 3e-14`), the control "the Green-Gauss gradient is off by more than 1" and the refusal as
registered. MU2 must fail G0b.

**UA needs the pressure gradient on the boundary.** The library gives the pressure a zero gradient on a face with a
fixed velocity, so the Green-Gauss gradient of a cell on the domain boundary is half of `-a` in the normal direction and
the uniformly accelerating flow is not a discrete solution for any coupling. The set-up of UA and UA2 therefore passes the
exact gradient `dp/dn = -a . n` on the four background sides through `OversetStep::wall_dpdn` (the library's fixed-flux
wall gradient, ADR 0219 M10; the public field of the prepared step) in every step. With it the boundary value is `p_C +
d_n dp/dn`, exact for the linear pressure, and the argument of the set-up holds. The committed test already does this;
no UA run was made before this amendment. Limits unchanged.

**B1, where the reference hashes were measured.** Registered: on a `git archive` copy of 8165a0b3. Done instead in the
worktree at the registration commit `80216482`, whose `crates` directory is that of 8165a0b3 (`git diff 8165a0b3 80216482
-- crates` is empty), with a test file that held the set-ups and the hash of B1 only and before `overset.rs` was edited:
RE `65f3bb871ad94a6d`, U `7c424f2255adfe35` (log `f445-scratch/b1-base.log`). The same library, one build less. The
committed test compares against these two constants.

The decision rule is unchanged.

## Amendment 2 (2026-11-12, written after the runs of the fast tests, of RE and RL at levels 1 to 3 and of NP, while RL at level 4 was still running, and before any further run)

The registered tests ran once each; the numbers are in addendum 1. What matters here: under G the conservation checks,
U1, UA, UA2, B1, S1, A2 (`1.962`, `1.789`) and A3 on levels 1 to 3 (`2.704`) are met; A1 is missed at levels 1 and 2
(`a_c` at `t = 0.025` on `Gamma_B`: `0.598`, `0.856`, `0.931`); S2 is missed at levels 1 and 2; **S3 is missed at every
level**: the velocity error of G against the exact solution is `1.773`, `1.642`, `1.619` times that of the default
coupling (`9.52e-4`, `2.29e-4`, `1.06e-4` against `5.37e-4`, `1.40e-4`, `6.57e-5`), and `R_b` under G is `5.77e-4`,
`1.48e-4`, `7.16e-5`, nine to three times that of the default coupling (`6.43e-5`, `3.25e-5`, `2.15e-5`, ADR 0340). P
blows up within five steps, as predicted.

The tests that missed are kept as ignored known misses with their limits intact; the checks of the fast level-1 test
that are met are asserted by a new fast test on the same deterministic runs; the two NP tests get the assertion of their
registered classification.

New hypothesis for the miss of S3, with its measurement:

| Id | Hypothesis | Measurement | Deciding number |
| --- | --- | --- | --- |
| HT | the excess of the error of G over the default coupling is the truncation of the two fringe interpolations: the prescribed flux hands the interpolated normal velocity to the calculated cells in full (S1: the flux is the face interpolation to `1e-4`), where the default coupling hands on a part of it (`a_c` `0.57` to `0.79` at `t = 0.5`, ADR 0340) and so damps the interpolation error | RE, levels 1 to 3, base runs with the library's quadratic donor interpolation on both fringes (`set_donor_interpolation(Quadratic)`, ADR 0251: third-order truncation), under G and under the default coupling: the root mean square of the velocity error over the calculated cells at `t = 0.5`; `rho_Q = e(G, quadratic) / e(default, quadratic)` and `rho_G = e(G, quadratic) / e(G, linear)` | confirmed: `rho_Q <= 1.15` and `rho_G <= 0.7` at the three levels; ruled out: `rho_Q >= 1.5` at the three levels; otherwise mixed |

Estimate behind the numbers, disclosed: `R_b` under G (the answer to the background fringe truncation alone) is 60 to 67
percent of the error of G, so removing most of both truncations should take more than 30 percent off; I have no number
for the default coupling with the quadratic stencils on RE and no trusted prediction for `rho_Q`. Test
`ht_quadratic_donors_on_both_fringes` (ignored, slow, run once; C1 and C3 asserted on the G runs).

The decision rule is unchanged.

## Addendum 1: results (2026-11-12)

Measured against the limits and deciding numbers registered above and in amendments 1 and 2; none was changed. Release
builds on Windows. Commits: the registration `80216482` (alone, before any code); the library code and the tests as
registered `ce343abb`; amendment 1 `a4474329` (before the runs of every check but G0); amendment 2 with the known-miss
tests and HT `c3e2dd62` (before the run of HT); the record of A3 between test processes, doc comments and
`docs/dynamic-mesh.md` `c9b1eab9`; this addendum. Each registered run was made once; deterministic runs of level 1 and of
NP were repeated where a later test shares them (the fast test of the met checks of level 1, the NP tests with the error
history printed) and printed the same digits. Logs: `E:/projects/cfd/w49i-work/f445-scratch` (`b1-base.log`, `g0.log`,
`g0b.log`, `f445-fast.log`, `f445-a_re_levels_1_to_3.log`, `f445-a_rl_levels_1_to_3.log`, `f445-a_rl_level_4.log`,
`f445-np_variant_p_re_level_2.log`, `f445-ht.log`, `f445-fast2.log`, `f445-final.log`, `b2-*.log`, `f445-MU1.log` to
`f445-MU7.log`).

### What was built

`crates/rcfd-fv/src/overset.rs` only: `FringeFlux { PressureEquation, Prescribed, PrescribedOwnGradient }`,
`OversetSolver::set_fringe_flux` and `fringe_flux`; the private `FringeRing` (regions of calculated cells, their ring
faces, the matrices of D5), `prescribed_pressure` (D1 to D3) and three hooks in `piso` (the ring of the step, the
gradient of D5 in the predictor and in the velocity correction, the branch to `prescribed_pressure`). Without the opt-in
`piso` takes the lines it took before (two `let` bindings became `let mut`). No other source file, no setting, no
case-tree key, no schema, no golden.

### What the runs found, in short

1. **The literal switch (P) is not admissible.** With the flux prescribed and the interpolated fringe pressure left in
   the Green-Gauss gradient of the ring cells, the Taylor-Green run on RE ends at step 5 of 200 at level 1 and at step 3 of
   400 at level 2 with a velocity above 10 (base and Qb). The largest velocity error grows by `11.6`, `11.1`, `11.1`,
   `10.8` per step at level 1 (`4.2e-3`, `4.9e-2`, `0.54`, `6.0`, `65`) and by `61`, `58` at level 2 (`6.2e-3`, `0.38`, `22`):
   `3.4` and `7.7` per corrector. Item 1 of the arithmetic predicted an instability that grows with the refinement, at
   `1.25` and `3.1` per corrector for the uniform mode alone; the direction and the ratio between the levels (`2.3`
   measured, `2.5` predicted) hold, the rate is 2.5 to 2.7 times what I estimated. P reproduces the uniform flow U and the
   uniformly accelerating flow UA exactly: it fails only where the two parts must exchange a changing pressure.
2. **With the pressure gradient of the ring cells taken from their own part (G) the prescribed flux is stable,
   conservative and exact where it should be.** Continuity `4.5e-13` at most over every run, flux through `Gamma_B`
   `1.9e-14`, through the domain boundary `0`, no unconverged solve; U1 `3.6e-15` and `0`; UA `1.5e-15`, `1.8e-15`; a
   wrong pressure level of the component (UA2) does not reach the velocity and is gone after one step.
3. **The imposition is then an identity.** On RE the change of the flux of a ring face is the mean of the changes of
   the two cell velocities to `4.2e-4` at level 1 and to `2.7e-5` at level 3 at `t = 0.025` (S1; `6e-6` and less at `t =
   0.5`): the flux defect `D` of ADR 0340 is gone, and with it the kink relation.
4. **The order of `R_b` is restored.** RE: `5.765e-4`, `1.479e-4`, `7.163e-5`, orders **`1.962`, `1.789`** (default
   coupling: `0.983`, `1.015`). RL: `1.0049e-3`, `1.0333e-4`, `5.154e-5`, `2.370e-5`, within the parity families **`2.704`**
   (levels 1 to 3) and **`2.124`** (2 to 4) (default: `1.150`, `1.288`). The response per unit of truncation on RE,
   `R_b / rms(tau_b)`, is `1.51`, `1.54`, `1.65`: nearly the same at every level, where the default coupling has `0.168`,
   `0.339`, `0.497`.
5. **`a_c` at `t = 0.025` misses 0.9 on the two coarser levels.** `Gamma_B`: `0.598`, `0.856`, `0.931`; `Gamma_C`: `0.867`,
   `1.037`, `1.053` (default coupling: `0.122`, `0.288`, `0.404` and `-0.72`, `-0.76`, `-0.62`). By `t = 0.5`: `0.906`,
   `0.968`, `0.989` and `1.057`, `1.058`, `1.045`.
6. **The price: the interpolation error reaches the calculated cells in full.** `R_b` under G is 9.0, 4.6 and 3.3 times
   that of the default coupling on RE (20, 4.7, 3.7, 2.7 times on RL), and the velocity error against the exact solution
   is `9.52e-4`, `2.29e-4`, `1.06e-4` against `5.37e-4`, `1.40e-4`, `6.57e-5`: **1.77, 1.64, 1.62 times** (S3, limit 1.5,
   missed). Both converge at about second order on these levels (G `2.05`, `1.89`; default `1.94`, `1.86`). With the
   quadratic donor interpolation on both fringes the error of G falls to `5.99e-4`, `1.377e-4`, `6.01e-5`, 0.63, 0.60, 0.56
   of its value with the linear stencils and 1.145, 1.056, 1.035 times that of the default coupling with the same
   stencils (HT, confirmed).

### Checks

| Check | Limit | Measured | Verdict |
| --- | --- | --- | --- |
| A1 G, RE, levels 1 to 3, `a_c` at `t = 0.025`, `Gamma_B` | `>= 0.9` | `0.598`, `0.856`, `0.931` | **missed** at levels 1 and 2 (known-miss test) |
| A1 the same, `Gamma_C` | `>= 0.9` | `0.867`, `1.037`, `1.053` | **missed** at level 1 (known-miss test) |
| A2 G, RE, orders of `R_b` at `t = 0.5`, levels 1 to 2, 2 to 3 | `>= 1.7` | `1.962`, `1.789` | met |
| A3 G, RL, orders of `R_b` at `t = 0.5`, levels 1 to 3, 2 to 4 | `>= 1.7` | `2.704`, `2.124` (the second from two test processes, record test) | met |
| C1 continuity, every step of every run under P or G | `<= 1e-9` | largest `4.5e-13` (RL level 4); P up to its last step `1.5e-11` | met |
| C2 flux through `Gamma_B`; through the domain boundary; outflow of U | `<= 1e-9`; `<= 1e-9`; `<= 1e-10` | `1.9e-14`; `0`; `1.1e-16` | met |
| C3 unconverged linear solves | `0` | `0` in every run | met |
| U1 U under G and under P: velocity, pressure (245 kind changes) | `<= 1e-10` each | `3.6e-15`, `0` (both) | met |
| B1 hashes without the opt-in, and after `set_fringe_flux(PressureEquation)` | exact | RE `65f3bb871ad94a6d`, U `7c424f2255adfe35`, equal to those of the unedited library | met |
| B2 `baseline_hashes`; `overset_f134`, `f134_records`, `f314`, `f334`, `f335`, `f335_records`, `f336`, `f338`, `f349`, `f353`, `f357`, `f359`; `rcfd-fv --lib`; release, unedited | all pass | 3 passed; 12, 1, 4, 3, 5, 4, 5, 9, 3, 1, 5, 5 passed; 103 passed; none failed | met |
| S1 G, RE, `abs(a_m - (1 + a_c) / 2)`, both fringes, `t = 0.025` and `0.5` | `<= 0.02` | largest `4.2e-4` (level 1, `Gamma_C`, `t = 0.025`) | met |
| S2 G, RE, `a_c(0.025) / a_c(0.5)`, `Gamma_B`; `Gamma_C` | `>= 0.9` | `0.660`, `0.884`, `0.942`; `0.821`, `0.980`, `1.007` | **missed** at levels 1 and 2 (`Gamma_B`) and 1 (`Gamma_C`) (known-miss tests) |
| S3 RE, velocity error of G over the default coupling | `<= 1.5` | `1.773`, `1.642`, `1.619` | **missed** (known-miss test) |
| UA default, P, G: velocity; pressure | `<= 1e-10`; `<= 1e-9` | `1.5e-15`; `1.8e-15` (all three) | met |
| UA2 G with the component pressure 0.7 too high: velocity; pressure after the first step | `<= 1e-10`; `<= 1e-9` | `1.5e-15`; `1.8e-15` (also at the end of the first step) | met |
| G0 (unit) as registered | `<= 1e-12` | `2.02e-12` | **missed**, a defective check (amendment 1; known-miss test) |
| G0b (amendment 1); the refusal | `<= 1e-12`; `Err` | `6.8e-14` (with the fringe values: `59`); refused | met |
| QS in every pair | exact | no component stencil changed, every background fringe stencil nine calculated cells with the donor first, no fallback | held |
| NP P on RE, levels 1 and 2 | classification | not admissible at both: the runs end at steps 5 and 3 (finding 1) | as predicted |
| X G with `rhie_chow_step_consistent`, BDF2, two outer iterations: continuity; unconverged; error over G alone | `<= 1e-9`; `0`; `<= 2` | `1.8e-13`; `0`; `1.0001`, `0.9993`, `0.9994` | met |

| Hypothesis | Deciding number (amendment 2) | Measured | Verdict |
| --- | --- | --- | --- |
| HT the excess error of G is the fringe truncation handed on in full | confirmed: `rho_Q <= 1.15` and `rho_G <= 0.7` at the three levels | `rho_Q = 1.145`, `1.056`, `1.035`; `rho_G = 0.629`, `0.600`, `0.564` (no fallback; errors: default quadratic `5.23e-4`, `1.304e-4`, `5.80e-5`) | **confirmed** (level 1 at `1.145` against `1.15`) |

Printed, no limit. RL under G, `a_c` at `t = 0.025` on levels 1 to 4: `0.661`, `0.799`, `0.900`, `0.895` (`Gamma_B`), `0.979`,
`1.051`, `1.046`, `1.039` (`Gamma_C`); at `t = 0.5`: `0.887`, `0.944`, `0.966`, `0.971`. RL, velocity error of the base run
under G: `1.468e-3`, `2.059e-4`, `9.60e-5`, `4.82e-5`. `R_b` under G grows over the run as under the default coupling (RE
level 1: `1.9e-5`, `1.1e-4`, `2.6e-4`, `3.9e-4`, `5.8e-4` at `t = 0.0025` to `0.5`). Pressure error of the base run on RE
(mean of each part removed): G `1.21e-3`, `3.09e-4`, `1.50e-4`; default `6.8e-4`, `2.02e-4`, `1.03e-4`. Run times in one
process on the loaded machine, base runs on RE: G `3.6`, `47`, `128` s; default `5.2`, `58`, `297` s (G has no second
solve for the offset). Heavy records: RE 777 s, RL levels 1 to 3 460 s, RL level 4 952 s, HT 470 s.

### Diagnosis of the misses

- **A1 and S2.** The flux is imposed exactly (S1), so `a_c < 1` is not a defect of the imposition. `a_c` compares the
  velocities of two cell centres one cell apart, and the answer of the calculated cells to a change of the ring falls
  off away from it: at once like the potential flow of a dipole (item 4 of the arithmetic: `0.65` estimated at level 1,
  `0.598` measured), later like the slower viscous flow, which decays less (`0.906` at `t = 0.5`). On the finer levels the
  viscous layer after `t = 0.025`, `sqrt(nu t) = 0.64 H`, is wider than a cell and `a_c` is already `0.856` and `0.931`.
  My registered claim that this is a ceiling "whatever the coupling" was wrong in that form: it is the early value, it
  rises with time and with the refinement, and level 3 meets the limit. Not separated by a measurement: the shares of
  the potential and of the viscous answer. The limit 0.9 at `t = 0.025` on levels 1 and 2 is not reachable by a coupling
  that prescribes the face flux; S1 is the measure that says what the acceptance meant.
- **S3.** HT, confirmed: with the truncation of both fringes made third order the error of G falls by 37 to 44 percent
  and comes within 14, 6 and 4 percent of the default coupling. The default coupling hands on only a part of the
  interpolated normal velocity over the run (ADR 0340), which damps the interpolation error; that is the same softness
  that costs it the order of `R_b`.
- **G0.** Round-off of fringe values of the size `1e3` against an absolute limit (amendment 1).

### Mutations

Applied alone to a `git archive` copy of `c3e2dd62` (`f445-scratch/mut`, own target directory) by `mutate.py` (an exact
replacement that must occur once), the named test run once, the file restored and compared byte for byte (equal in all
seven). The commits after `c3e2dd62` change doc comments in `overset.rs`, add one record test without a flow run and two
lint fixes to the test file, and this addendum.

| Id | Must fail | Result |
| --- | --- | --- |
| MU1 `q_R = 0` | C1 or C2 | failed: continuity and flux through `Gamma_B` `1.0e-2` |
| MU2 D5 without the extrapolation term | G0b | failed: `2.69` |
| MU3 the flux from the fringe velocity alone | S1 | failed: `0.166`, `0.041` (`Gamma_B`), `0.035` (`Gamma_C`) |
| MU4 the level shift not applied | UA2 (pressure) | failed: `0.7` |
| MU5 the default mode takes the branch of D1 | B1 | failed: the hash of RE differs |
| MU6 `gamma_f` not set to zero on the ring | S1 or C1 | failed: both (continuity `7.3e-3`, S1 up to `0.39`) |
| MU7 D5 not applied in the predictor | UA2 (velocity) | failed: `92` |

`scripts/limit_audit.py` on the new test file: no hit.

### Deviations and disclosures

- G0 ran once before the tests were committed; amendment 1 followed before any other run. The test file of `ce343abb`
  already held the wall gradient of UA that amendment 1 registers.
- B1's reference hashes were measured in the worktree on the unedited library, not on an archive copy (amendment 1).
- NP: "the growth over the last ten steps" could not be formed (the runs end after 5 and 3 steps); the error after
  every step is printed by a deterministic repeat and quoted in finding 1. The two NP tests assert their classification
  since amendment 2; the level-2 test is no longer ignored (it takes a second).
- The fast level-1 test failed on S2; it is kept as registered (ignored known miss), and its met checks are asserted by
  `c_re_level_1_conservation_and_flux_identity_under_g` on the same deterministic runs.
- The printed diagnostics are fewer than ADR 0340's (two bands of the background, no share of the exact velocity).
- Doc comments of `FringeFlux` were rewritten after the runs to say what they showed; no code line changed after
  `a4474329`. `baseline_hashes` was run on `a4474329` and again on the final tree.
- HT's first deciding number is met at level 1 by `1.145` against `1.15`.
- My predictions: C1 to C3, U1, B1, B2, UA, UA2, QS, S1, NP, A1 missed on `Gamma_B`: right. G0, S3, S2 met: wrong. A1 met
  on `Gamma_C`: wrong at level 1. A3 "more likely missed": wrong, it is met.

### Not run

- RL at level 5, RE above level 3, P on RL: not registered.
- A moving component under G with a flow that is not uniform (accuracy): not covered; U alone moves.
- The combination with `rhie_chow_step_consistent`, BDF2 and outer iterations beyond the 40 steps of X.
- The ignored records of the other overset files and the workspace suite: the lead runs the gate.

### After the tests

`cargo fmt -p rcfd-fv --check` clean. `cargo clippy -p rcfd-fv --all-targets -- -D warnings` clean (the only crate
touched). Final tree: `overset_f445` 9 passed, 5 ignored (5 s); `rcfd-fv --lib overset` 2 passed, 1 ignored.

### Decision by the registered rule

**Partial.** The opt-in G is built and C1, C2, U1, B1, B2 hold for it; A2 and A3 are met; A1 is missed on levels 1 and 2
with the measured numbers and the diagnosed cause above. The alternative clause does not apply: P is shown not
admissible with a number, G is admissible. Said plainly: the prescribed flux does what ADR 0340 hoped for the order of
the response (0.98 to 1.96 on RE), but only with the pressure gradient of the ring cells taken from their own part, and
on the meshes tried it makes the solution less accurate than the default coupling by a factor 1.6 to 1.8 with the linear
stencils and 1.04 to 1.14 with the quadratic ones. It is not a candidate for a default.

### Open questions (each with options and a recommendation)

- **Q1. The status of F-445.** Options: (a) partial, as the rule gives; (b) done, reading the miss of A1 as a property
  of the measure `a_c` and not of the opt-in (S1 shows the identity). Recommendation (a): the clause is binding as
  written. Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation); not a decision the
  maintainer made in person, and (b) is the maintainer's to choose.
- **Q2. The variant P.** Options: (a) keep `FringeFlux::Prescribed` with its doc comment ("unstable", kept for the
  record tests), so that the number that rules it out stays reproducible; (b) remove it and turn the NP tests into
  records of printed numbers. Recommendation (a). Taken on the standing instruction.
- **Q3. Should G be recommended, or become a default?** It changes results, so it is not taken: **open**. My reading:
  no default change; recommend G only together with the quadratic donor interpolation and only where the hard
  imposition or the absence of the pressure offset matters, until a moving-component accuracy check exists (Q4).
- **Q4. A moving component under G.** Options: (a) a follow-up item (text below); (b) leave it. Recommendation (a), low
  priority. Taken on the standing instruction as a proposal to the lead.
- **Q5. `a_c` as a measure in other items (F-369).** Recommendation: use the flux identity `abs(a_m - (1 + a_c) / 2)` or
  the defect itself where the hardness of the imposition is meant; `a_c` carries the decay of the response over one
  cell. A note for F-369 (text below). Taken on the standing instruction.

### For the lead: roadmap entries

No roadmap file was edited.

**F-445**, proposed status: `partial`. Proposed note:

> From ADR 0340 question 2, recommendation taken on the maintainer's standing instruction of 2026-11-11. ADR 0347, 2026-11-12: PARTIAL. Built as an opt-in of the overset solver, OversetSolver::set_fringe_flux, default off, every result without it bit-identical (state hashes of two runs equal to those of the library before, baseline_hashes and the twelve existing overset test files pass unedited). The flux of a face between a calculated and a fringe cell is the face interpolation of the velocity each pressure corrector starts from plus one correction per region of calculated cells that makes its net flux zero (it replaces the offset of the background fringe pressures). The switch as ADR 0340 words it (variant P, the interpolated fringe pressure left in the Green-Gauss gradient of the calculated cells next to the fringe) is not admissible: the Taylor-Green run on RE blows up at step 5 (level 1) and step 3 (level 2), the error growing 11-fold and 60-fold per step, as predicted before the run from the pressure mismatch of the two parts (growth with the refinement). With that gradient taken from the own part (variant G, FringeFlux::PrescribedOwnGradient) it is stable: continuity 4.5e-13, flux through the hole ring 1.9e-14, U1 3.6e-15, a uniformly accelerating flow exact to 2e-15, the flux of a ring face equal to the mean of the two cell velocities to 4e-4. Order of R_b at t = 0.5 on RE 1.962, 1.789 (default coupling 0.983, 1.015); on RL within the parity families 2.704 (levels 1 to 3) and 2.124 (2 to 4); the response per unit of truncation is 1.51, 1.54, 1.65 at levels 1 to 3 (default 0.168, 0.339, 0.497). Missed: a_c at t = 0.025 is 0.598, 0.856, 0.931 at the background fringe and 0.867, 1.037, 1.053 at the component fringe against 0.9 (levels 1 and 2 miss); diagnosed as the decay of the answer of the calculated cells over one cell (the dipole estimate made before the run gave 0.65 at level 1), not a softness of the imposition; it rises to 0.906, 0.968, 0.989 by t = 0.5. Found on the way, registered checks missed and kept as known misses: under G the interpolation error reaches the calculated cells in full, so the velocity error against the exact solution on RE is 1.77, 1.64, 1.62 times that of the default coupling (limit 1.5) and R_b itself is 9.0, 4.6, 3.3 times the default's; with the quadratic donor interpolation of F-336 on both fringes the error of G falls to 0.63, 0.60, 0.56 of that and is 1.145, 1.056, 1.035 times the default's (hypothesis confirmed by its registered numbers). Not a candidate for a default. One unit check missed by round-off of its own test values (2.0e-12 against 1e-12) and was corrected by an amendment before the corrected run. Seven mutations caught. Not covered: accuracy with a moving component, RL at level 5, the combination with the other overset opt-ins beyond a smoke check.

**F-349**, status unchanged (`partial`). Add to its note:

> ADR 0347 (F-445), 2026-11-12: with the flux of the fringe faces prescribed and the pressure gradient of the neighbouring calculated cells taken from their own part, the order of R_b is 1.962, 1.789 on RE and 2.704, 2.124 within the parity families of RL: removing the soft imposition named here restores the order. The response is then larger on the levels run (R_b 9.0 to 3.3 times), because the interpolation error is handed on in full.

**F-369**, status unchanged. Add to its note:

> ADR 0347 (F-445), 2026-11-12: the following coefficient a_c carries the decay of the response over one cell (0.60 at once on a coarse mesh even when the flux is imposed exactly); where the hardness of the imposition is meant, measure the flux defect or abs(a_m - (1 + a_c) / 2). The opt-in OversetSolver::set_fringe_flux(PrescribedOwnGradient) removes the defect and is available as a switch for this item's measurements; it has no accuracy check with a moving component yet.

Proposed follow-up item (the number is the lead's): **Overset: the prescribed fringe flux with a moving component.** Depends on F-445 and F-336. Acceptance:

> With OversetSolver::set_fringe_flux(PrescribedOwnGradient) and the quadratic donor interpolation of F-336, the Taylor-Green set-up of ADR 0219 with the moving, turning component (levels 1 and 2, the corrected time step of its amendment 2) is run against the default coupling with the same stencils: the largest velocity error over the calculated and fringe cells is at most 1.2 times that of the default coupling at both levels, continuity and the outflow balance hold to 1e-9 in every step, and no step is refused for a calculated cell between fringe cells on opposite sides; or the ADR names with a number what the moving fringe adds (the lag of the prescribed flux on the faces that change kind is the first candidate).
