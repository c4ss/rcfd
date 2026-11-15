# ADR 0586: F-646 registration: linear paths about a pre-stressed hyperelastic state (the modes about a nonlinear static state of a stretched hyperelastic bar)

Status: **REGISTERED 2026-11-15, written before any code of F-646 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-646 was written and nothing of F-646 was compiled or run: the files
under "What was read" were read and the arithmetic below was done by hand (no script, no probe, no rcfd call). Results go in the
addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-646 (dep F-620), structural solver, advanced tier, the follow-up proposed by
[ADR 0555](0555-f620-registration-hyperelastic-materials-on-the-method-step-and-the-linear-paths.md) (its open question 2 and
the limit "the linear paths take the material at rest"). Number 0586 given by the lead. Branch `f646-work` from 8d531fab,
worktree `w49i-work/f646`, scratch `w49i-work/f646-scratch`. This is planned as a test-only item: no library line is planned
(D1 shows that the path exists).

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number); each registered run made once; existing tests not edited;
defaults bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new
dependency, no unsafe code; clean-room. The roadmap files are not edited (the entries go in "For the lead"). Every choice below
that the maintainer would normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the
recommendation). The concurrent item F-645 (hyperelastic Beam2 and shells on the linear paths) is not touched: this item uses
Hex8 solids only and edits no call site.

## What F-646 asks

The acceptance, binding:

> Every limit not stated here is registered in the ADR before the run. The modes about a nonlinear static state of a stretched hyperelastic bar take the tangent of mat_hyper at that state; their frequencies agree with central differences of the nonlinear static residual within 1e-6, and with the modes at rest within 1e-10 at a load of zero.

Clauses: **a0** every limit registered here; **a1** the modes about a nonlinear static state of a stretched hyperelastic bar
take the tangent of `mat_hyper` at that state (shown at the matrix level, and by mutations of `mat_hyper`'s tangent that vanish
at rest); **a2** their frequencies agree with central differences of the nonlinear static residual within `1e-6`; **a3** at a
load of zero they agree with the modes at rest within `1e-10`.

## What was read

`AGENT_RULES.md`; the brief; ADR 0555 (D1 to D3, its limits, open question 2 and the follow-up text that became F-646); ADR 0581
in full (the modal residual floor `c eps lambda_max / lambda_1` and its estimate, the frequency error of a stopping test,
arithmetic 2 there; the form of the "For the lead" section); ADR 0541's header in `mat_hyper.rs`; ADR 0572 and 0573 (the "For
the lead" form). Sources at 8d531fab: `crates/rcfd-fem/src/follower.rs` (`tangent_modes` 483-570: `nonlinear::tangent` at
`(u, states)` plus the contributions' tangents, `consistent_with`, reduction on the model's and the given prescribed equations
with zero values, `Shift::Value(0)` unless a rigid motion is free, `lowest_eigenpairs` with the `ModalOptions` fields, the result
`ModalResult::from_parts`), `crates/rcfd-fem/src/modal.rs` (`modal_analysis_with` 198-280: the same reduction, mass test, shift
rule and options, with `stiffness_with_materials` in place of the tangent; `ModalResult`: eigenvalues ascending, modes
M-orthonormal and expanded with zeros at prescribed equations), `crates/rcfd-fem/src/nonlinear.rs` (`evaluate_with` 180-285: the
Green-Lagrange strain, `point_update` -> `Material::stress`, the material part `B_NL^T D B_NL` and the geometric part
`element_geometric_stiffness(element, coords, &stress)` at 273-277; `internal_force`, `tangent`, `solve`, `NewtonOptions::new`:
residual `1e-10`, energy `1e-10`, 20 iterations, 5 halvings; `check_driver_model`), `crates/rcfd-fem/src/model.rs`
(`check_elements` 1205-1333: with `history = true` (the driver's paths, `tangent_modes` included) the hyperelastic material is
not refused; the refusals of ADR 0555 D3 apply to the linear paths only), `crates/rcfd-fem/src/material.rs` (`Material::stress`
456-459: the hyperelastic arm returns `mat_hyper::stress_and_tangent`; `linear_elasticity_matrix` 468-476),
`crates/rcfd-fem/src/mat_hyper.rs` (`kin`, `stress_of`, `stress_and_tangent` 239-280, `linearised_elasticity` 52-65),
`crates/rcfd-fem/tests/sa_follower.rs` (M1, M2, M6: `tangent_modes` of a stretched elastic beam against the linear pre-stressed
modes and, unloaded, against `modal_analysis` within `1e-8`), `crates/rcfd-fem/tests/sa_hyper_lock.rs` (`tangent_modes` already
called with the hyperelastic material on the sphere, as a diagnostic without limit), `crates/rcfd-fem/tests/sa_hyper_paths_f644.rs`
(the energies, the twin, the `Checks` helper).

## Design

### D1. No library change

`tangent_modes` (ADR 0480 D7) already takes the hyperelastic material: its model check is the driver's (`history = true`), and
its matrix is `nonlinear::tangent`, whose material part at every point is `Material::stress`, i.e. `mat_hyper::stress_and_tangent`
at the point's Green-Lagrange strain, plus the geometric part of the point's second Piola-Kirchhoff stress. No existing check
measures it against the residual it linearises, nor against the linear modal path at rest with this material. So F-646 is a set of
checks on existing calls. No line under `crates/*/src` is changed, every default keeps its bits by construction, and no existing
test can change its result (audit below).

### D2. Set-up (registered)

- **Energies** (those of ADR 0555 D7): NHc `neo_hookean(1e6, 1e7)` and MRc `mooney_rivlin(3.5e5, 1.5e5, 1e7)` (both `G = 1e6`,
  `K = 1e7`), density `1000`.
- **Bar S**: `hex_box([0, 0, 0], [2, 0.5, 0.5], [8, 2, 2])` (Full Hex8, 32 elements, 81 nodes, 243 equations), the material on
  `all`, the three components held on `xmin` (27 equations, 216 free), the dead load `Load::nodal_force("xmax", [P, 0, 0])` with
  `P = 4e3` N on each of the 9 nodes of `xmax` (total `3.6e4` N). Full rather than BBar so that the linear and the nonlinear
  element are the same element (the BBar block is the mean-dilatation element on the driver).
- **Stretched state**: `nonlinear::solve(&bar, &[], &[], &NewtonOptions::new(10))`, its displacement `u*` and states.
- **Zero-load state**: `u = 0`, `initial_states(&bar)` (the equilibrium at a load factor of zero; R0 below checks that the
  residual vanishes there).
- **Modes**: `tangent_modes(&bar, &[], u, states, lambda, &[], &ModalOptions::new(6))` (default tolerance `1e-10`, Relative)
  with `lambda = 1` at `u*` and `lambda = 0` at rest; `modal_analysis(&bar, &ModalOptions::new(6))` for the modes at rest.
- **Residual**: `r(u) = nonlinear::internal_force(&bar, u, states) - f_ext` with `f_ext` the dead load (`P` on the `x` equation of
  each `xmax` node; the load factor `1` at `u*`, `0` at rest); the states are those of the state (the material has none).

### D3. The central-difference frequencies (the reading of clause a2)

The acceptance asks the frequencies to agree with central differences of the residual. Two readings were considered:

(a) **Directional, along each mode** (taken): for each returned mode `phi_i`, scaled to `max_k |phi_k| = 1` (`phihat`), the
central difference of the residual along it, `lambda_FD,i = phihat . (r(u + a phihat) - r(u - a phihat)) / (2 a) / (phihat^T M
phihat)` with `M` the consistent mass (`mass::consistent`), and `f_FD,i = sqrt(lambda_FD,i) / (2 pi)`. Since `phi_i` is an
eigenvector of `K_T`, `lambda_FD,i - lambda_i = phihat^T (K_FD - K_T) phihat / phihat^T M phihat` to the order of the central
difference: a defect of `K_T` is seen at first order, and the error of the difference along a smooth mode is small (arithmetic
2).

(b) **The full matrix**: `K_FD` by columns, `(r(u + h e_j) - r(u - h e_j)) / (2h)`, symmetrised, reduced, and its six lowest
eigenvalues by `lowest_eigenpairs`. Its error along coordinate directions is amplified by `lambda_max / lambda_1` (arithmetic 3),
so no limit of `1e-6` on its eigenvalues can be derived beforehand at an affordable step. It is kept as a matrix-level check (KM,
clause a1: `K_T` against `K_FD` entry by entry, which is well conditioned) and its eigenvalues as a record (KE).

Taken: (a) for clause a2, with (b) as KM (a check) and KE (a record), on the maintainer's standing instruction of 2026-11-11
(take the recommendation), because (a) has a derived error well below the acceptance's limit and (b)'s eigenvalues do not.

### D4. Files

New: `crates/rcfd-fem/tests/sa_hyper_prestress_f646.rs` (the checks). This ADR. No other file.

## Arithmetic (disclosed, by hand)

1. **The stretched state.** Twin moduli of NHc: `E = 9KG/(3K + G) = 9e13/3.1e7 = 2.90e6`, `nu = (3K - 2G)/(2(3K + G)) = 28/62 =
   0.452`. Nominal stress `3.6e4 / 0.25 = 1.44e5`, axial strain `~ 1.44e5 / 2.9e6 = 5 %` (a stretch `~1.05`, `J - 1 ~ 0.05 (1 - 2
   nu) ~ 5e-3`), with the clamp restraining the lateral contraction near `xmin`. The nodal loads are not the consistent ones (a
   corner node carries the same as the centre), but each end element receives `9e3` N over `0.0625` m^2, the mean nominal stress:
   no element is driven far beyond the mean. Ten load steps of `0.5 %` strain each: Newton converges in a few iterations (the
   tangent is exact). Estimate: converged.
2. **Directional central difference (D3 a).** `g(t) = phihat . r(u + t phihat)`; `(g(a) - g(-a)) / (2a) = g'(0) + (a^2/6)
   g'''(0) + rounding`, `g'(0) = phihat^T K_T phihat`, `g''' = d^4 W / dt^4` along the mode (`W` the total strain energy).
   *Truncation*: for the first bending mode with a unit tip amplitude the largest slope is `~0.7` and the bending strain `eps_1 ~
   0.25 (1.875/2)^2 = 0.22`; the Green strain along the mode is `t eps_1 + t^2 eps_2` with `eps_2 ~ slope^2 / 2 ~ 0.25`, so with
   `W ~ int (D/2) (t eps_1 + t^2 eps_2)^2`: `W'''' / W'' ~ 12 eps_2^2 / eps_1^2 ~ 15` (per unit amplitude squared), and the
   relative truncation `(a^2/6) W''''/W'' ~ 2.6 a^2`. Taken with the stretched state's extra terms (the third and fourth strain
   derivatives of `W` at `E0 ~ 0.05` contracted with the mode) by a safety factor `10`: `<= 30 a^2` for the first bending pair;
   for the higher of the six modes (wave numbers up to `2.5` times larger, second bending `4.69` against `1.875`; axial and
   torsion) the ratio grows at most with the square of the wave number: `<= 180 a^2`. *Rounding*: `r` sums element forces of up to `~1e4` N (8 elements per node), so each entry of `r`
   carries `~c eps 8e4` with `c ~ 10` (the operations of a point evaluation), `~1.8e-10`; `phihat . r` over `216` entries of
   `|phihat_k| <= 1` (mean `~0.3`) carries `~65 * 1.8e-10 ~ 1.2e-8` (worst case, not random-walk). The difference to be measured
   is `2 a lambda phihat^T M phihat` with `phihat^T M phihat ~ 500 kg * 0.2 = 100` and `lambda >= 47` (the lowest at rest,
   arithmetic 4): `>= 9.4e3 a`. Relative rounding `<= 1.2e-8 / (9.4e3 a) = 1.3e-12 / a`. With **`a = 5e-6` m** (a 2e-5 fraction
   of the element size): truncation `<= 180 * 2.5e-11 = 4.5e-9`, rounding `<= 2.6e-7`; in the frequency half of that:
   **`<= 1.4e-7`**. (A larger `a` lowers the rounding and raises the truncation; `a = 5e-6` keeps both below the limit `1e-6`
   with a margin of `7` on the sum, the rounding bound being the worst-case one.)
3. **Full-matrix central difference (D3 b).** Entry `(i, j)`: truncation `(h^2/6) d^3 r_i / d u_j^3`; a single nodal component
   moved by `h` strains its elements by `h / h_e` (`h_e = 0.25`), so `d^3 r_i / d u_j^3 ~ c K_max / h_e^2` with `c` up to `10`,
   relative to `K_max ~ 5e6` (`(lambda + 2G) h_e` times the assembled factor): `<= 10 (h / h_e)^2 / 6`. Rounding `~1.8e-10 /
   (2h)` relative to `5e6`: `1.8e-17 / h`. With **`h = 1e-6` m**: truncation `<= 2.7e-11`, rounding `<= 1.8e-11`; KM
   (`max |K_FD - K_T| / max |K_T|`) **`<= 5e-11`**. In the eigenvalues the same entry errors act on a low mode as an unstructured
   perturbation: `|phi^T dK phi| <= (sum of |dK_ij| |phi_i| |phi_j|) ~ 216 * 81 * |dK| * 0.045^2 ~ 35 |dK|` (M-normalised `phi`,
   `|phi_k| ~ 1/sqrt(500)`), `|dK| ~ 5e-11 * 5e6 = 2.5e-4`: `~9e-3` against `lambda_1 ~ 47 .. 200`: up to `2e-4`, far above `1e-6`
   in the worst case (with cancellation by the zero sums of the element forces perhaps 10 to 100 times less). No limit can be
   derived: KE is a record.
4. **Modes at rest and their floor (R1).** `lambda_1` at rest: Euler-Bernoulli first bending `omega_1 = 1.875^2 sqrt(E I /(rho A
   L^4)) = 3.516 sqrt(2.9e6 * 5.21e-3 / (1000 * 0.25 * 16)) = 6.83` rad/s, `lambda_1 ~ 47` (higher on the coarse Full mesh).
   `lambda_max ~ c_p^2 (2/h_e)^2 * 3 = 1.13e4 * 64 * 3 = 2.2e6`; ratio `~4.6e4`; residual floor `~0.1 * eps * 4.6e4 = 1e-12`,
   below the default tolerance `1e-10` (ADR 0555 L3c met at the default on Cantilever L with NHc). At the stretched state
   `lambda_1` rises (arithmetic 5): the floor falls. Estimate: both converge at the default tolerance.
5. **True difference at rest, tangent against linear (R1).** At `E = 0` `stress_and_tangent` gives `S = 0` and `D` equal to
   `linearised_elasticity` within `3.3e-16` of the largest entry (ADR 0555 L0t, NHc and MRc), the geometric part vanishes with
   `S`, and the element matrix `B^T D B` of the driver and of the linear element are the same products in a possibly different
   order: entry rounding `~eps` relative. ADR 0555 measured the hyperelastic modes against the twin's at `3.0e-14` on Cantilever L
   (NHc). The eigen solves stop at their own iterations; the frequency error at the stopping test is `<= tau^2 kappa(M) mu / g`
   (ADR 0581 arithmetic 2) `~ 1e-20 * 27 * 200 ~ 5e-17`. The amplification of an entry rounding by `lambda_max/lambda_1` (as in
   arithmetic 3, with `|dK| ~ eps * 5e6 = 1.1e-9`): `<= 35 * 1.1e-9 / 47 = 8e-10` worst case; in practice the rounding of the same
   products is far below the worst case (ADR 0555's `3.0e-14`). Estimate **`~1e-13`**, worst case `~1e-9` in the eigenvalue
   (`5e-10` in the frequency), which would exceed `1e-10`; the limit is the acceptance's and the worst case is disclosed.
6. **Negative controls.** *N1* (the state matters): the string stiffening of the first bending pair, `d lambda ~ P_tot k^2 /
   (rho A)` with `k^2 ~ 1.05` (the first cantilever mode's `int phi'^2 / int phi^2` for `L = 2`): `3.6e4 * 1.05 / 250 = 151`
   against `lambda_1 ~ 47 .. 60`: the first frequency rises by a factor `~2`; limit `>= 1e-2` on the worst relative difference of
   the six frequencies, stretched against rest. *N2* (the tangent at the state against the linear pre-stress path of ADR 0555,
   `prestressed_modes` with `Prestress::from_static` of the linear static solve: `D` at rest and the stress of the linear
   solution): they differ by the material and geometric nonlinearity at `5 %` strain, `O(1e-2)` in the stress and the
   stiffness; limit `>= 1e-4` on the worst relative difference of the six frequencies. *N3* (the directional measure sees a
   wrong matrix): F1's measure with `lambda_i` replaced by the Rayleigh quotient of the tangent at rest `K_T(0)` along the same
   `phihat` (the rest material without the state): differs by N1's size; limit `>= 1e-2`.

## Checks (limits fixed now)

Every measure is required finite before it is compared (a non-finite value or a refused call misses). Rows NH and MR are run for
every check except N2 and KE (NH only). Relative frequency differences are `|f - f_ref| / f_ref`, worst over the six modes,
position by position (both ascending) where two lists are compared.

| Id | Clause | What | Limit | Estimate |
|---|---|---|---|---|
| S0 | set-up | the nonlinear solve of Bar S converges (`Ok`); its stretch `1 + mean u_x(xmax) / 2` reported | `Ok` | arithmetic 1: `~1.05` |
| T0 | set-up | `tangent_modes` at `u*` and at rest and `modal_analysis`: `Ok`, six frequencies each, every residual `<= 1e-10` | `Ok` | arithmetic 4 |
| KM | a1 | `max_ij |K_FD - K_T|_ij / max_ij |K_T|_ij` at `u*`, `K_T = nonlinear::tangent`, `K_FD` the column central differences of `r` with `h = 1e-6`, all 243 equations | `<= 1e-8` | arithmetic 3: `<= 5e-11` |
| F1 | a2 | `max_i |f_FD,i - f_i| / f_i` at `u*`, D3 (a) with `a = 5e-6` | `<= 1e-6` | arithmetic 2: `<= 1.4e-7` |
| C0 | a2 (control of the measure) | F1's measure at rest (`u = 0`, load factor `0`) | `<= 1e-6` | arithmetic 2 (no pre-strain terms) |
| R0 | a3 (set-up) | `max |r(0)|` at the zero-load state | `<= 1e-9` N | `S(0) = 0`: `0` |
| R1 | a3 | `tangent_modes` at the zero-load state against `modal_analysis`: worst relative frequency difference | `<= 1e-10` | arithmetic 5: `~1e-13` (worst case `5e-10`) |
| N1 | negative | stretched against rest: worst relative frequency difference | `>= 1e-2` | arithmetic 6: `~1` |
| N2 | negative (NH) | `tangent_modes` at `u*` against `prestressed_modes` with `Prestress::from_static` of `solve_static` on Bar S | `>= 1e-4` | arithmetic 6: `~1e-2` |
| N3 | negative | `max_i |f_FD,i - f_0,i| / f_FD,i` with `f_0,i` from `phihat^T K_T(0) phihat / phihat^T M phihat` at `u*` | `>= 1e-2` | arithmetic 6 |
| KE | record (NH) | the six lowest frequencies of the symmetrised reduced `K_FD` (`lowest_eigenpairs`, shift `Value(0)`, tolerance `1e-10`) against `tangent_modes` at `u*`, and the asymmetry `max |K_FD - K_FD^T| / max |K_T|` | record | arithmetic 3 |
| B1 | defaults | no line under `crates/*/src` changed (`git diff --stat 8d531fab -- crates/*/src` empty) | empty | D1 |
| S1 | build | `cargo fmt -p rcfd-fem --check`; `cargo clippy -p rcfd-fem --release --all-targets -- -D warnings` | clean | |

The checks run in two tests (`s0_to_n3_stretched_bar` for NH and MR, `ke_full_difference_record` for KE); each well under a
minute in release (243 equations, `2 * 243` force evaluations for KM, 12 for F1).

### Audit of existing tests (before this registration)

No library line changes (D1), so no existing test can change its result; no refusal is lifted. `sa_hyper_lock.rs` calls
`tangent_modes` with the hyperelastic material already (a diagnostic, no limit): it is not edited. `sa_follower.rs` M6 checks
the unloaded `tangent_modes` against `modal_analysis` for an elastic beam within `1e-8`: not edited. The mutations below change
library lines on a scratch copy only.

## Negative controls

N1, N2, N3 (above): N1 shows the state matters, N2 that the tangent at the state is not the linear pre-stress path's matrix (`D`
at rest), N3 that the directional measure of F1 sees a wrong matrix along the same vectors. C0 controls the measure where its
truncation is smallest.

## Planned mutations (on a scratch copy: a `git archive` of the test commit, its own `CARGO_TARGET_DIR`, one at a time; `sa_hyper_prestress_f646.rs` run for each)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | `mat_hyper.rs` `let v_ii = -2.0 * kappa * k.j * k.jm1;` to `0.0 * kappa * k.j * k.jm1` (a volumetric tangent term that vanishes at rest) | KM, F1 (both rows); **not** R1, C0 (zero at rest) |
| MU2 | `mat_hyper.rs` `i1 / 3.0 * sym_ci` to `3.0 / 3.0 * sym_ci` (an isochoric tangent term, equal at rest) | KM, F1; not R1, C0 |
| MU3 | `material.rs` 458: the hyperelastic arm returns `(S, linearised_elasticity(energy))` (the tangent at rest at every state) | KM, F1 (or S0 if Newton fails); not R1 |
| MU4 | `nonlinear.rs` 276 `*v += g;` to `*v += 0.0 * g;` (the geometric part of the tangent dropped) | KM, F1 (or S0); not R1 |
| MU5 | `mat_hyper.rs` 62 `row[m] = g` to `row[m] = 2.0 * g` (the linearised material of the linear paths) | R1; **not** KM, F1, C0 (the driver does not use it) |

MU1, MU2 and MU3 are clause a1 ("the tangent of mat_hyper at that state"); MU4 the rest of the tangent at the state; MU5 shows
that R1 compares two different paths. A mutation is caught if any registered check misses or a set-up call fails; the "not"
entries are expectations recorded for the record (they are not required). No mutation is declared equivalent.

## Decision rule

- **done**: S0, T0, KM, F1, C0, R0, R1 and N1 to N3 met for both rows (N2 for NH); MU1 to MU5 caught (each by at least one of
  its expected checks); B1 and S1 met.
- **partial**: any of KM, F1, R1 missed (recorded as a `*_known_miss` test with its limit and measured value, diagnosed), or a
  negative control missed, or a mutation not caught, or a check met only after a post-run amendment.
- **todo**: the set-ups do not run (S0 or T0 missed for both rows).

## Limits (what is not covered)

Full Hex8 only (the BBar block's mean-dilatation element, Hex20 and Tet elements are not checked); one bar and one stretch
(`~5 %`); a dead load (no follower pressure about the hyperelastic state); the modes only: buckling and the harmonic response
about a nonlinear hyperelastic state (the rest of ADR 0555's follow-up text) have no path that takes the driver's tangent (the
linear pre-stress paths take `D` at rest, N2) and are not built here; Beam2 and shells (F-645). The full-matrix eigenvalues (KE)
are a record without a limit.

## Addendum 1: results (2026-11-15)

Commits: 4a69ea76 (registration), 7ee49acf (tests, as in run 1), and this addendum. Runs, each once, release, `--nocapture
--test-threads=1`, logs in `w49i-work/f646-scratch`: run 1, the whole new file (`run1.log`, on 7ee49acf); the mutations
(`mutations.log`, on a `git archive` of 7ee49acf with its own target, the test `s0_to_n3_stretched_bar` for each). No line under
`crates/*/src` changed. No amendment.

| Id | Measured | Limit | Met |
|---|---|---|---|
| S0 | NH: converged in 10 steps, stretch `1.05061`; MR: 10 steps, stretch `1.05132` | `Ok` | yes |
| T0 | all six calls `Ok` in 9 iterations, six frequencies each, worst residual `7.6e-11` (NH stretched), `5.9e-11` (MR stretched), `2.0e-11` (rest, both paths) | `Ok`, residuals `<= 1e-10` | yes |
| KM | NH `3.95e-12`, MR `4.66e-12` | `<= 1e-8` | yes |
| F1 | NH `1.33e-7`, MR `1.34e-7` | `<= 1e-6` | yes |
| C0 | NH `1.19e-7`, MR `1.19e-7` | `<= 1e-6` | yes |
| R0 | `0` (NH, MR) | `<= 1e-9` N | yes |
| R1 | `0` (NH, MR: the frequencies of `tangent_modes` at zero load and of `modal_analysis` are equal in every bit) | `<= 1e-10` | yes |
| N1 | NH `0.687`, MR `0.681` (first bending pair `1.2607` to `2.1265` Hz, NH) | `>= 1e-2` | yes |
| N2 | NH `6.75e-2` (linear pre-stress path `2.2529 .. 7.8762` Hz against `2.1265 .. 7.3447`) | `>= 1e-4` | yes |
| N3 | NH `0.305`, MR `0.301` | `>= 1e-2` | yes |
| KE (record) | frequencies of the symmetrised reduced `K_FD` against `tangent_modes`: `1.96e-10` (9 iterations); asymmetry of `K_FD` `1.46e-12` of `max |K_T|` | record | - |
| B1 | `git diff --stat 8d531fab -- crates/*/src` empty | empty | yes |
| S1 | `cargo fmt -p rcfd-fem --check` clean; `cargo clippy -p rcfd-fem --release --all-targets -- -D warnings` clean | clean | yes |

**Mutations** (`mutations.log`, one at a time; the log keeps the missed lines, so a check absent from a row was met):

| Id | Caught by | Expected "not" entries |
|---|---|---|
| MU1 (`v_ii` to zero) | KM `2.34e-2` (NH, MR), F1 `2.80e-2` (NH), `2.83e-2` (MR) | R1, C0 met, as expected |
| MU2 (`i1 / 3.0` to `3.0 / 3.0` in the isochoric tangent) | KM `1.69e-3`, `1.20e-3`; F1 `2.86e-3`, `2.04e-3` | R1, C0 met, as expected |
| MU3 (the hyperelastic arm returns the tangent at rest) | KM `0.175`, `0.182`; F1 `7.9e-2`, `9.1e-2` (Newton still converged with the wrong tangent) | R1 met, as expected |
| MU4 (geometric part dropped, `nonlinear.rs` 275; the registration said 276, the same line `*v += g;`) | S0 (Newton fails at load step 7 after 5 halvings, NH and MR) | R1 not reached (S0 returns first) |
| MU5 (linearised shear entries `2G`) | R1 `0.293` (NH, MR) | KM, F1, C0 met, as expected |

Five of five registered mutations caught, each by one of its expected checks.

**Deviations and diagnoses.** None of the limits was missed and no amendment was written. (1) R1 came out exactly `0`: at `E =
0` the driver's element tangent and the linear element stiffness give the same reduced matrices in every bit for this mesh and
material (the estimate was `~1e-13`; the worst case `5e-10` of arithmetic 5 did not occur). (2) F1 (`1.33e-7`) and C0
(`1.19e-7`) sit at the estimate `1.4e-7` of arithmetic 2, which was dominated by the worst-case rounding bound; their near
equality at rest and at the state agrees with rounding, not truncation, being the larger part. (3) KE (record): the eigenvalues
of the full-matrix central difference agree with the tangent modes within `1.96e-10`, far below the worst-case bound of
arithmetic 3; it is a record and no decision rests on it. (4) MU4 was caught by the set-up check S0 (Newton fails without the
geometric stiffness), one of its registered expectations. Not run: the whole workspace suite (the lead's gate) and
`baseline_hashes` (no shared solver file changed; the mutations of `nonlinear.rs`, `material.rs` and `mat_hyper.rs` were made on
the scratch copy only).

**Decision: done** by the registered rule: S0, T0, KM, F1, C0, R0, R1 and N1 to N3 met for both rows (N2 for NH) in the one
registered run, MU1 to MU5 caught, B1 and S1 met, no post-run amendment.

**Remaining questions** (none changes a default):

1. Buckling and the harmonic response about a nonlinear hyperelastic state (the rest of ADR 0555's follow-up text). Options: (a)
   a follow-up giving `follower::buckling_factors`-style and direct-harmonic entry points the driver's tangent at a state; (b)
   leave the linear pre-stress paths (`D` at rest, N2 shows a `6.7 %` difference at `5 %` strain) as they are and document it.
   Recommend (a), proposed below.
2. The BBar (mean-dilatation) block about a stretched state, near-incompressible. Options: (a) a follow-up with the same checks on
   the BBar bar at `K/G = 1e4` (the modal tolerance raised as in ADR 0581); (b) nothing. Recommend (a), folded into the
   follow-up below.

## For the lead: roadmap entries

> Proposed status: done.

> Structural solver, advanced tier. Follow-up of F-620 (ADR 0555, 2026-11-15). DONE. ADR 0586, 2026-11-15: test-only (no library line changed): follower::tangent_modes already takes the hyperelastic material (the driver's model check) and its matrix is nonlinear::tangent, mat_hyper::stress_and_tangent at each point's Green-Lagrange strain plus the geometric part. Stretched bar (Full Hex8 8 x 2 x 2, 2 x 0.5 x 0.5, clamped, 3.6e4 N axial, stretch 1.051; neo-Hookean mu 1e6 kappa 1e7 and Mooney-Rivlin 3.5e5, 1.5e5, 1e7): the tangent at the state against column central differences of the nonlinear static residual (h = 1e-6) 3.9e-12 and 4.7e-12 of the largest entry (limit 1e-8); the six lowest frequencies of tangent_modes against central differences of the residual along each mode (step 5e-6, derived) 1.3e-7 (limit 1e-6), the same measure at rest 1.2e-7; at zero load tangent_modes equals modal_analysis in every bit (limit 1e-10). Negative controls: stretched against rest 0.69, against the linear pre-stress path (D at rest) 6.7e-2, the directional measure against the rest tangent 0.30. Record: the eigenvalues of the full central-difference matrix agree within 2.0e-10. Five of five mutations caught: two mat_hyper tangent terms that vanish at rest and the tangent at rest at every state (by the matrix and frequency checks, not by the rest check), the dropped geometric part (Newton fails), the linearised shear doubled (only by the rest check). Defaults unchanged by construction. Not covered: BBar, Hex20, Tet, buckling and harmonic about the state.

> Follow-up proposed: buckling and harmonic response about a nonlinear hyperelastic state. Deps F-646. Acceptance: Every limit not stated here is registered in the ADR before the run. The buckling factors and the direct harmonic response about a nonlinear static state of a hyperelastic model take the driver's tangent at that state; at a load of zero they equal those of the linear paths within 1e-10, and the BBar bar at K/G = 1e4 about a stretched state gives frequencies that agree with central differences of the nonlinear static residual within 1e-6.
