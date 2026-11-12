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
