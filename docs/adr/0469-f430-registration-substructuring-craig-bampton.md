# ADR 0469: F-430 registration: substructuring by static condensation and fixed-interface component mode synthesis

Status: **REGISTERED 2026-11-14, written before any code of F-430 and before any run of its checks**. Not yet accepted by the
maintainer. Results go in the addendum at the end; nothing above the addendum is changed after a run.

Roadmap item F-430 (deps F-408, F-382, F-383, F-393), structural solver, advanced tier ([ADR 0310](0310-structural-solver-advanced-tier-plan.md)),
work package SA-23, stub [ADR 0333](0333-substructuring-and-component-mode-synthesis.md); the module `rcfd_fem::cms` declared by
[ADR 0440](0440-f408-registration-advanced-tier-scaffold.md) (F-408). Number 0469 given by the lead. Branch `f430-work` from main
13b387a2, worktree `w49i-work/f430`, scratch `w49i-work/f430-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical (the new behaviour is a new
module called by nothing else); no edit of `baseline_hashes.rs`, of the hash tables or of any golden file; no existing test
edited (the scaffold row follows the standing pattern of ADR 0291, below); no new dependency, no unsafe code; clean-room. Every
choice below that the maintainer would normally be asked about is taken on the maintainer's standing instruction of 2026-11-11
(take the recommendation).

## What F-430 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. Statics: the condensed model gives the retained displacements and, after recovery, the interior ones and the stresses of the full model within 1e-10. Keeping every fixed-interface mode reproduces all frequencies of the full model within 1e-8 (small model). A bar of two substructures with five modes each: the first three frequencies of the assembly within 0.1 percent of the full model and not below it (1e-10), the error falling monotonically with the number of modes. The reduced matrices are symmetric within 1e-13 and the reduced mass is positive definite; a free substructure keeps six zero frequencies below 1e-6 of the seventh. Harmonic response of the assembly equals the full model within 1 percent up to half the lowest omitted fixed-interface frequency. A reduced part written to its file and read back gives the same bits. An interface set with no nodes and a mode count above the interior size are refused.

Clauses: **a1** statics, retained displacements of the condensed model within 1e-10; **a2** interior displacements after
recovery within 1e-10; **a3** stresses after recovery within 1e-10; **a4** every fixed-interface mode kept: all frequencies of
the full model within 1e-8 (small model); **a5** bar of two substructures, five modes each: the first three frequencies within
0.1 percent; **a6** and not below the full model's (1e-10); **a7** the error falls monotonically with the number of modes;
**a8** the reduced matrices symmetric within 1e-13; **a9** the reduced mass positive definite; **a10** a free substructure keeps
six zero frequencies below 1e-6 of the seventh; **a11** harmonic response of the assembly within 1 percent up to half the lowest
omitted fixed-interface frequency; **a12** a reduced part written to its file and read back gives the same bits; **a13** an
interface set with no nodes is refused; **a14** a mode count above the interior size is refused.

## What was read (main 13b387a2, read only)

ADR 0333 (stub), the roadmap entry; `crates/rcfd-fem/src/cms.rs` (the stub of ADR 0440, `PENDING = Some("ADR 0333 (F-430)")`);
`mpc.rs` (the transformation `x = T q + g` of ADR 0296: `transformation`, `Transformation::{t, g, independent, reduce, expand}`,
`add_supports`, `modal_analysis_mpc`, `refuse_nonzero_values`); `modal.rs` (`modal_analysis`, `ModalOptions`);
`harmonic.rs` (`ModalBasis::all_modes`, `HarmonicLoad`, `HarmonicResponse`); `harmonic_direct.rs` (`DirectHarmonic`,
`LossFactor::Constant`); `assemble.rs` (`stiffness_with_materials`); `mass.rs` (`consistent_with`); `loads.rs`
(`nodal_load_vector`); `recover.rs` (`IntegrationPointField::compute`); `builders.rs` (`hex_box`, the face sets); `solve.rs`
(`solve_static`, `StaticSolver`); `rcfd_linalg::eigen` (`generalized_eigen_dense`, `symmetric_eigen`, `lowest_eigenpairs`,
`ResidualTest::Backward`); `rcfd_linalg::skyline::SkylineLdlt`; `tests/sa_scaffold.rs` (the X1 copies and the ADR 0291
pattern); ADR 0467 for the form of this document.

Published formulations, cited from memory, **not read** for this work: R. J. Guyan, Reduction of stiffness and mass matrices,
AIAA J. 3 (1965) 380 (static condensation); R. R. Craig and M. C. C. Bampton, Coupling of substructures for dynamic analyses,
AIAA J. 6 (1968) 1313 (fixed-interface component mode synthesis). Everything used is derived below from first principles (block
elimination and the Rayleigh-Ritz method); the papers are cited for the names and the published form, not for any number.

## Design

### The reduction (derivation)

A part is a `SolidModel` with its own constraints and supports and a node set, the **interface**. Its unknowns are those of the
transformation of ADR 0296: `x = T q` (zero constraint values only, `g = 0`; a non-zero value is refused, as in the modal
analysis on the transformation), `K_q = T^T K T` (with the elastic supports), `M_q = T^T M T` (consistent mass), `f_q = T^T f`
(the model's loads by `nodal_load_vector`). The **boundary** unknowns `b` are the independent unknowns `q` of the mesh unknowns of
the interface nodes, every component of each node, nodes ascending, components ascending; an interface unknown that is fixed
(empty row of `T`) is left out; one that is the dependent of a linear constraint or a rigid set is refused (it is not an unknown
of the part). Every other independent unknown (rigid-set reference unknowns included) is **interior** `i`. Partition

`K_q = [[K_bb, K_bi], [K_ib, K_ii]]`, `M_q` alike, `f_q = (f_b, f_i)`.

**Constraint modes** (static condensation): with `u_b` given and no interior load, the interior equilibrium `K_ib u_b + K_ii u_i
= 0` gives `u_i = Psi u_b`, `Psi = -K_ii^{-1} K_ib` (`K_ii` positive definite once the interface holds the part: refused
otherwise with the failing pivot). With an interior load, `u_i = Psi u_b + K_ii^{-1} f_i` exactly, and eliminating `u_i` from
the boundary rows gives the condensed problem

`K_hat u_b = f_hat`, `K_hat = K_bb + K_bi Psi`, `f_hat = f_b + Psi^T f_i`

(block Gaussian elimination: the Schur complement; exact, Guyan 1965). Recovery: `u_i = Psi u_b + K_ii^{-1} f_i`, then `x = T q`
and the stresses of `x` by `recover::IntegrationPointField::compute` (the stress path of the full model). The static solve of the
condensed model is a dense Cholesky of `K_hat` (refused if not positive definite: a part held only through its interface).

**Fixed-interface modes**: `K_ii phi_k = omega_k^2 M_ii phi_k`, M-normalised, ascending; `Phi = [phi_1 .. phi_m]` the lowest `m`.
The Craig-Bampton transformation

`(u_b, u_i) = T_cb (u_b, eta)`, `T_cb = [[I, 0], [Psi, Phi]]`

gives the reduced matrices of order `n_b + m`

`K_r = [[K_hat, 0], [0, Lambda]]`, `M_r = [[M_hat, M_bq], [M_bq^T, Phi^T M_ii Phi]]`,
`M_hat = M_bb + M_bi Psi + Psi^T M_ib + Psi^T M_ii Psi`, `M_bq = (M_bi + Psi^T M_ii) Phi`,

with `Lambda = diag(omega_k^2)` and the zero coupling in `K_r` exact by `K_ib + K_ii Psi = 0` (written as zeros, the published
form), and the reduced load `f_r = T_cb^T f_q = (f_hat, Phi^T f_i)`. `K_hat` and `M_r` are computed by the products above and
then symmetrised, `a_ij = a_ji = (a_ij + a_ji)/2`; the asymmetry before that step (largest `|a_ij - a_ji|` over the largest
`|a|`) is kept and reported (`ReducedPart::asymmetry`). Dynamic recovery of a reduced vector `(u_b, eta)`: `u_i = Psi u_b + Phi
eta`, then `x = T q`.

Properties used by the checks (first principles): `T_cb` has full column rank, so `M_r` is positive definite when `M_q` is;
the reduction is a Rayleigh-Ritz projection, so every reduced eigenvalue of an assembly bounds the full one from above (the
min-max principle), and nested bases (`m` growing, the modes taken in the same ascending order) give eigenvalues that do not
increase; with `m = n_i` the matrix `T_cb` is square and nonsingular and the reduced pencil is congruent to the full one (every
frequency exact); a rigid motion `r` of a free part satisfies `K_q r = 0`, so `r_i = Psi r_b` and the constraint modes carry the
six rigid motions exactly (zero eigenvalues of `K_hat`).

Eigen-solution of the interior: dense (`generalized_eigen_dense`, every pair) when `n_i <= dense_limit` (default 600, the bound of
`ModalBasis::all_modes`) or when `m + 1 >= n_i`; otherwise `lowest_eigenpairs` for `m + 1` pairs (shift 0, tolerance 1e-12,
`ResidualTest::Backward`). The first omitted eigenvalue `omega_(m+1)^2` is kept (`ReducedPart::omitted_eigenvalue`, `None` when
`m = n_i`). `Psi` is computed column by column from one `SkylineLdlt` factor of `K_ii`.

### Assembly

`Assembly::new(parts)` joins reduced parts at their boundary unknowns: two boundary unknowns are the same unknown of the
assembly when their node positions agree within `1e-9` of the largest extent of the parts' interfaces and their components are
equal; the unknowns of the assembly are the distinct boundary unknowns in the order met (part by part) and then the modal
coordinates of each part in order. `K` and `M` of the assembly are the sums of the parts' reduced matrices (dense), the load the
sum of their reduced loads. Provided: the dense eigen-solution (`generalized_eigen_dense`), the harmonic response `((1 + i eta) K
- omega^2 M) X = F` (a uniform loss factor `eta`, dense complex Gaussian elimination with partial pivoting), and the coordinates
of one part in an assembly vector (for its recovery). An assembly unknown carried by one part only is a free boundary.

### File

`ReducedPart::write(path)` / `ReducedPart::read(path)`: a text file, first line `rcfd-cms-part 1`, then named sections of
integers and of floats, each float written as the 16 hexadecimal digits of its IEEE bits (exact by construction), LF line
endings. Read refuses a different header, a missing or short section and a malformed number. Recovery takes the part's model and
refuses one whose unknown counts or interface positions differ from those stored.

### Entry checks

`model.validate()`, `check_supported_with_materials()`, zero constraint values only (`refuse_nonzero_values`), a cyclic
constraint refused; the interface set must exist (`UnknownSet`), have at least one node, and give at least one boundary unknown
(**a13**, refused otherwise); `m <= n_i` (**a14**: `m > n_i` refused naming both numbers; `m = n_i` taken); `n_i >= 1`.

Taken on the standing instruction: interfaces matched by position (no global numbering is required of parts built apart); the
file is text with hexadecimal bits (exact, diffable, no new dependency); dense interior eigen-solution up to 600 unknowns; zero
constraint values only; a uniform loss factor is the damping of the assembly's harmonic response.

## Set-ups

Material steel, `E = 2.1e11`, `nu = 0.3`, `rho = 7850`, `Hex8` (formulation Full), `hex_box` meshes, threads 1 unless said.

- **Small model** S: box `0.4 x 0.1 x 0.08`, `4 x 2 x 2`, clamped on `xmin` (three components), traction `(1e6, 2e6, -3e6)` Pa
  on `xmax`, gravity `(0, -9.81, -9.81)`; 108 free unknowns. Interfaces: `xmax` (9 nodes, 27 boundary, 81 interior) and `mid`,
  the 9 nodes at `x = 0.2` (27 boundary, 81 interior, the interior in two pieces).
- **Bar**: full model F, box `1 x 0.1 x 0.08`, `16 x 2 x 2`, clamped on `xmin`; part A, box `[0, 0.5] x 0.1 x 0.08`, `8 x 2 x 2`,
  clamped on `xmin`, interface `xmax`; part B, box from `x = 0.5`, `0.5 x 0.1 x 0.08`, `8 x 2 x 2`, interface `xmin` (free at
  `x = 1`). The node positions of A and B on `x = 0.5` and those of F agree bit for bit (dyadic fractions). Section `0.1 x 0.08`
  (not square) so that no frequency of interest is a double one and the truncation at five modes is not inside a pair. Load of
  the harmonic checks: nodal force `(1, 1, 1)` N at the tip corner node `(1, 0.1, 0.08)` (in F and in B), which excites the
  axial, both bending and the torsion families.
- **Free part** R: box `0.2 x 0.1 x 0.08`, `4 x 2 x 2`, no constraint, interface `xmin`, five modes; record R': part B without
  constraint is already free (interface `xmin`), five modes.
- Full-model references: `modal_analysis` (`ModalOptions::new(8).tolerance(1e-12).residual_test(Backward)`; the relative test at
  1e-12 does not converge on F, probe 1), `ModalBasis::all_modes` (dense, every mode of S), `solve_static` (Direct) and
  `IntegrationPointField::compute`, `DirectHarmonic` with `LossFactor::Constant(0.02)` on element set `all`.

## Arithmetic before this registration (disclosed)

Run once each, logs in `w49i-work/f430-scratch`: a probe program on the existing API only (`probe/`, logs `probe1.log`,
`probe2.log`), and a numpy estimate of the truncation on one-dimensional families (`est/estimate.py`, `est/band.py`, logs
`estimate.log`, `band.log`, `band2.log`); no F-430 code existed.

1. Frequencies (rad/s) of F: 476.7, 572.6, 2921.4, 3460.0, 4926.4, 7945.5, 8179.0, 9236.8 (z-bending 1, y-bending 1, z2, y2,
   torsion 1, z3, axial 1, y3, identified by ratios). Fixed-interface (interior with both ends of A fixed, B fixed at `x =
   0.5`), A: 10977, 12417, 19938, 27173, 29398, then 33571; B: 1894, 2260, 9882, 10944, 12481, then 16482 (axial). With five modes
   each, each part keeps two modes of each bending family, one torsion mode and no axial mode; the lowest omitted
   fixed-interface frequency is 16482 rad/s (B), half of it 8241 rad/s, and the axial mode of F (8179) is inside that range.
2. Truncation estimate: each family as a 1-D chain of 16 elements (Hermite beams for bending, linear bars for torsion and axial,
   consistent mass), stiffness scaled to the first frequency of that family of F, split at the middle, Craig-Bampton with the
   per-family mode counts of item 1 (in the order of the modes for `m = 0..5`). Errors of the first three assembly frequencies
   (z1, y1, z2): `m = 0` 2.6e-2, 2.6e-2, 0.34; `m = 1` 6.8e-5, 2.6e-2, 6.4e-3; `m = 2, 3` 6.8e-5, 6.8e-5, 6.4e-3; `m = 4` 2.8e-6,
   6.8e-5, 3.4e-4; `m = 5` 2.8e-6, 2.8e-6, 3.4e-4. Non-increasing, constant when the added mode is of another family (the families
   of the symmetric mesh are decoupled). Other modes at `m = 5`: z3 5.7e-4, y2 3.4e-4, torsion 9.7e-4, **axial 0.10** (no axial
   mode kept: the constraint mode of B is its rigid translation).
3. Harmonic estimate, same chains, loss factor 0.02, load (1, 1, 1) at the corner (torsion moment 0.01 N m), displacement
   vector norm (rotations weighted by 0.037 m, the rms radius of the section nodes): over (0, 8241] plus the resonances, the
   largest relative error is **0.85 at the axial resonance 8179**; without the axial load 3.9e-2 at 6593 (off resonance, the
   missing flexibility of the interior load); the error first exceeds 1e-2 at 1894 rad/s, about 0.115 of the lowest omitted
   fixed-interface frequency, not 0.5. Largest error on (0, 1000]: 2.3e-3; on (0, 1500]: 6.3e-3. The loss factor does not move
   the off-resonance part (0.02, 0.05, 0.1 tried in the same run: 3.89e-2, 3.89e-2, 3.92e-2 without the axial load).
   **The clause a11 is predicted to miss on this set-up** (H1 below); H2 characterises the band where 1 percent holds.
4. Statics sensitivity of S (one-ulp probe, the F-542/F-545 lesson): `E` moved by one ulp changes the displacements by 1.06e-13
   and the stresses by 9.0e-14 (relative to the largest); Direct against SparseDirect: 1.13e-13 and 8.4e-14. The 1e-10 limits of
   a1 to a3 are about 1000 times these. `lambda_max / lambda_1` of S is 3.0e4 (dense, all modes): the eigenvalue rounding of a
   dense solution is about `eps lambda_max / lambda_k`, below 7e-12 relative, 3e-12 in frequency, against the 1e-8 of a4.
5. Rigid-mode floors (dense, every mode of the free full model, as the reduced free part will be solved): free box `0.2 x 0.1 x
   0.08`, `4 x 2 x 2`: largest `sqrt(|lambda_k| / lambda_7)`, `k <= 6`, 1.7e-7 (`lambda_max / lambda_7` = 148); free `0.5 x 0.1 x
   0.08`, `8 x 2 x 2` (part B free): 7.0e-7 (`lambda_max / lambda_7` = 2.7e3); the floor grows as `sqrt(eps lambda_max /
   lambda_7)`. The reduced part has fewer and smoother unknowns (its `lambda_max` is at most the full one's by the Ritz bound),
   so the floor of R is at most about 1.7e-7, 6 times below 1e-6; R' is near the limit and is a record, not a check.
6. Symmetry: with the symmetrisation the reduced matrices are exactly symmetric (measure 0, independent of any one-ulp change of
   the input); before it, the asymmetry of `K_bi Psi` is of order `eps ||K|| ||Psi||^2` (a backward-stable solve), with
   `||Psi||` of order one (displacements of unit interface motions): about 1e-14 to 1e-13 of the largest entry.
7. Rounding of the bar's low frequencies: about `eps lambda_max / lambda_1` with `lambda_max` below 3.5e11 (the full model's
   bound) and `lambda_1 = 2.3e5`: below 3.4e-10 in eigenvalue, 1.7e-10 in frequency, against truncation errors of 2.8e-6 and more
   (a6 and a7 have their tolerances from this).

## Checks (limits fixed now)

Tests in new files `crates/rcfd-fem/tests/sa_cms_default_bits.rs` (D1) and `crates/rcfd-fem/tests/sa_cms.rs` (the rest),
release profile. Relative errors of vectors are the largest absolute difference over the largest absolute reference entry;
frequency errors are `f / f_full - 1`.

| Id | Clause | Check | Limit | Estimate |
|---|---|---|---|---|
| ST1 | a1 | S, interface `xmax`, condensed (`m = 0`), `K_hat u_b = f_hat` solved: `u_b` against the full static solution at the boundary unknowns, over the largest full displacement | `<= 1e-10` | 1e-13 |
| ST2 | a2 | same, the recovered interior (`Psi u_b + K_ii^{-1} f_i`, expanded) against the full solution | `<= 1e-10` | 1e-13 |
| ST3 | a3 | same, the stresses of the recovered displacements against those of the full solution, every integration point and component, over the largest | `<= 1e-10` | 1e-13 |
| ST4 | a1-a3 | ST1 to ST3 with the interface `mid` | `<= 1e-10` each | 1e-13 |
| AM1 | a4 | S, interface `xmax`, `m = n_i = 81`: the 108 frequencies of the part (dense eigen-solution of `K_r`, `M_r`) against `ModalBasis::all_modes` of S, each | `abs <= 1e-8` | 3e-12 |
| AM2 | a4 | same with the interface `mid` | `abs <= 1e-8` | 3e-12 |
| B1 | a5 | bar, A and B with `m = 5`, assembly: the first three frequencies against the first three of F | `0 <= err <= 1e-3` (the lower bound is B2) | 2.8e-6, 2.8e-6, 3.4e-4 |
| B2 | a6 | same: `f / f_full - 1 >= -1e-10` for the first three | `>= -1e-10` | `>= +2.8e-6` |
| B3 | a7 | `m = 0, 1, .., 5` (both parts the same `m`): `err_k(m + 1) <= err_k(m) + 1e-9`, `k = 1, 2, 3`, `m = 0..4`; and `err_k(5) < err_k(0) / 10` | limits | non-increasing (rounding 1.7e-10); factor 9e3 to 1e3 |
| S1 | a8 | `K_r`, `M_r` of A, B (`m = 5`), S (`xmax`, `m = 81`) and R: `max abs(a_ij - a_ji) / max abs(a)` | `<= 1e-13` | 0 |
| S2 | a8 | `ReducedPart::asymmetry` (before the symmetrisation) of the same parts | `<= 1e-10` | 1e-13 |
| P1 | a9 | `M_r` of the same parts: the smallest eigenvalue by `symmetric_eigen` over the largest | `>= 1e-12` | positive, order 1e-4 (record) |
| R1 | a10 | R: the frequencies of the part (dense eigen-solution of `K_r`, `M_r`): `abs(f_k) <= 1e-6 f_7`, `k = 1..6`, and `f_7 > 0` | `<= 1e-6` | 1.7e-7 |
| H1 | a11 | bar, `m = 5`, loss factor 0.02 in F and in the assembly, the corner load: `omega_half = 0.5 min(sqrt(omitted_eigenvalue))` over A and B; 50 points `j omega_half / 50` and every frequency of F below `omega_half`; relative error of the complex displacement vector of the whole bar (every equation of F; the recovered parts, A at `x = 0.5`) | `<= 1e-2` at every point | **predicted miss**: 0.85 at 8179 rad/s; above 1e-2 from 1894 |
| H2 | a11 (band) | the same on 30 points `j 1000 / 30` rad/s and the frequencies of F below 1000 (476.7, 572.6) | `<= 1e-2` | 2.3e-3 |
| F1 | a12 | A (`m = 5`) and R: `write`, `read`: every float field equal in bits, every integer field equal; the file written from the part read back is byte-identical | exact | holds |
| F2 | a12 | a file cut in half, and one with header `rcfd-cms-part 2`: refused | refused | holds |
| X1 | a13 | S with a node set `none` of no nodes as interface: refused; an interface whose nodes are all fixed (`xmin` of S): refused; an unknown set name: `UnknownSet` | refused | holds |
| X2 | a14 | S `xmax`: `m = 82` refused (`n_i = 81`); `m = 81` taken | refused / taken | holds |
| C1 | design | S `xmax`, `m = 5`: `dense_limit(0)` (subspace path) against the default (dense): the five kept eigenvalues and the omitted one, and the frequencies of the part | `abs rel <= 1e-9` | 1e-12 |
| T1 | design | A, `m = 5`, threads 1 and 4: `K_r`, `M_r`, the reduced load, `Psi`, `Phi` bit-identical | bits | holds (assembly is thread-independent, ADR 0284) |
| D1 | defaults | digests at main 13b387a2 (printed by `sa_cms_default_bits.rs`, written with the API of that commit and committed before any library line): `solve_static` of S (displacements), the stresses of that solution, `modal_analysis` of F (eight eigenvalues), `ModalBasis::all_modes` of S (eigenvalues), `DirectHarmonic` of F with the corner load at 1000 rad/s (`re`, `im`); equal after the change | `==` | holds (no shared file is changed) |
| E1 | scaffold | `sa_scaffold.rs`: the X1 copy `x1_modules_and_pending_after_f419_f426` becomes an ignored record (ADR 0291 pattern) and a new copy with `cms` "landed" passes; `cms::PENDING == None` | pass | holds |

Records (reported, not in the decision rule): R' (part B free, `m = 5`, the largest rigid ratio, estimate up to 7e-7); the
frequencies 4 to 8 of the bar against F at `m = 5`; `err_k(m)` for every `m`; the asymmetries before symmetrisation; the
smallest-to-largest eigenvalue ratio of each `M_r`; the error curve of H1 (all points).

## Negative controls and mutations

Negative controls: X1, X2, F2 (refusals). Mutations (on a scratch copy; each must be caught by at least one registered check):

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | `Psi = +K_ii^{-1} K_ib` (sign) | ST1, ST2, AM1, R1 |
| MU2 | `M_hat` without `Psi^T M_ii Psi` | AM1, B1 |
| MU3 | the coupling `M_bq` set to zero | AM1 |
| MU4 | the highest `m` fixed-interface modes kept instead of the lowest | B1, B3 |
| MU5 | static recovery without `K_ii^{-1} f_i` | ST2, ST3 |
| MU6 | `f_hat = f_b` (no `Psi^T f_i`) | ST1 |
| MU7 | the file writes floats with `{:.15e}` instead of the bits | F1 |
| MU8 | the mode count refused at `m >= n_i` instead of `m > n_i` | X2 |
| MU9 | the empty-interface refusal removed | X1 |
| MU10 | the assembly joins boundary unknowns by position only (component ignored) | B1 |
| MU11 | the harmonic response with `(1 - i eta) K` | H2 |
| MU12 | `M_r` assembled without the symmetrisation and with `M_bi Psi` written twice instead of `M_bi Psi + Psi^T M_ib` | S2 |

## Decision rule

**done**: every check met at its registered run, every mutation caught, no amendment after a run. **partial**: a check missed
(recorded as `*_known_miss` with its limit), a mutation not caught by a registered check, or a check met only after an amendment
made after its run. **todo**: the module does not build or the bar runs cannot be made. H1 is predicted to miss (arithmetic
item 3); if it does, the item is partial by this rule, with the cause diagnosed in the addendum.

## Limits (not covered)

- Interfaces are matched by position; the reduced matrices are dense in the boundary unknowns (a large interface defeats the
  purpose, ADR 0333) and the interior eigen-solution is dense up to 600 unknowns.
- Free-interface methods, residual flexibility (static correction vectors), reduction with viscous damping or gyroscopic terms,
  pre-stress, cyclic constraints, non-zero constraint values, and mistuning studies are not included. A reduced part is
  assembled with other reduced parts only, not with ordinary elements of a model (a follow-up).
- Beams and shells pass the entry checks of the transformation (their rotational unknowns are boundary unknowns like the
  others) but no check here covers them.
- The harmonic response of the assembly takes a uniform loss factor only.

## Amendment 1 (2026-11-14, written before any run of the checks)

While writing `sa_cms.rs`, before any check had run: the mesh refuses a node set with no members
(`SolidMesh::add_node_set` returns `FemError::EmptySet`, `mesh.rs` `check_members`), so the first case of X1 as registered
("S with a node set `none` of no nodes as interface") cannot be set up, and an interface set with no nodes can never reach
the reduction; MU9 (the empty-interface refusal of `reduce` removed) could then be caught by no check. Corrected, before the
run: **X1** = (i) `add_node_set("none", vec![])` on the mesh of S is refused with `EmptySet` (the refusal of a13 holds at the
mesh, and the reduction keeps its own refusal of an empty set for a caller-built mesh path); (ii) the interface `xmin` of S,
whose nodes are all fixed (no boundary unknown), is refused by `reduce`; (iii) an unknown set name is refused with
`UnknownSet`. **MU9** becomes: the refusal of an interface without a free unknown removed (expected caught by X1 (ii)). No
limit changes. This amendment precedes the first run of every check, so it does not by itself make any check "met only after
an amendment made after its run".
