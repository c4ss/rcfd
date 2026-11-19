# ADR 0536: F-553 registration: reduced parts as superelements in an ordinary model

Status: **REGISTERED 2026-11-14, written before any code of F-553 and before any run of its checks**. Not yet accepted by the
maintainer. Results go in the addendum at the end; nothing above the addendum is changed after a run.

Roadmap item F-553 (deps F-430), structural solver, advanced tier ([ADR 0310](0310-structural-solver-advanced-tier-plan.md)),
the follow-up named by [ADR 0469](0469-f430-registration-substructuring-craig-bampton.md) (F-430, remaining question 2). Number
0536 given by the lead. Branch `f553-work` from main 679081a1, worktree `w49i-work/f553`, scratch `w49i-work/f553-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical (the new behaviour is a new
module called by nothing else); no edit of `baseline_hashes.rs`, of the hash tables or of any golden file; no existing test
edited; no new dependency, no unsafe code; clean-room. Every choice below that the maintainer would normally be asked about is
taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).

## What F-553 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. A reduced part of ADR 0469 is added to a SolidModel on its interface nodes and solved with the model's ordinary elements in the static solve, the modal analysis and the direct harmonic solve. A bar of a reduced half and an ordinary half equals the full model within 1e-10 in statics and its first three frequencies are within 0.1 percent and not below the full model's; beams and shells as interface nodes covered; without a reduced part every result bit-identical.

Clauses: **c1** a reduced part is added to a model on its interface nodes; **c2** solved with the model's ordinary elements in the
static solve; **c3** in the modal analysis; **c4** in the direct harmonic solve; **c5** a bar of a reduced half and an ordinary
half equals the full model within 1e-10 in statics; **c6** its first three frequencies within 0.1 percent; **c7** and not below
the full model's; **c8** beams and shells as interface nodes covered; **c9** without a reduced part every result bit-identical.

## What was read (main 679081a1, read only)

ADR 0469 (F-430, the reduction, the assembly of reduced parts, the part file, its addendum and remaining questions), ADR 0474
(F-552, residual vectors and the cutoff), ADR 0511 (F-559, load patterns, `ReducedPart::reduce_load`); ADR 0532 (F-603, the
interface enrichment) is on main after 679081a1 and is **not** on this branch: it is left out (a part built with it is an
ordinary `ReducedPart` and needs nothing of this design, but no check here covers it). `crates/rcfd-fem/src/cms.rs`
(`reduce`, `ReducedPart`: `boundary`, `n_boundary`, `n_reduced`, `stiffness`, `mass`, `load`, `recover_static`, `expand`,
`reduce_load`, `MATCH_TOLERANCE`; `Assembly`); `mpc.rs` (`transformation`, `Transformation::{t, independent, n_mesh,
n_unknowns, reduce, expand, transpose_times}`, `add_supports`, `modal_analysis_mpc`, `refuse_nonzero_values`, `direct`);
`modal.rs` (`ModalOptions`); `harmonic.rs` (`HarmonicLoad`); `harmonic_direct.rs` (`DirectHarmonic`, `LossFactor::Constant`,
the complex stiffness `(1 + i eta) K`); `beam.rs` (Beam2, six unknowns per node in global axes), `shell.rs` (Shell4, six
unknowns per node, the drilling penalty); `model.rs` (`DofMap::{components, equation}`); `rcfd_linalg::{skyline::SkylineLdlt,
cskyline::{ComplexCsr, ComplexSkylineLdlt}, eigen::{lowest_eigenpairs, generalized_eigen_dense}, CsrMatrix::from_triplets}`;
`tests/sa_cms.rs` and `tests/sa_cms_default_bits.rs` (set-ups and the D1 pattern); `crates/rcfd-fem/src/lib.rs` (which says no
package edits it: the new module is therefore a child of `cms`).

Published formulation, cited from memory, **not read** for this work: the superelement (substructure) method as in R. R. Craig
and A. J. Kurdila, Fundamentals of Structural Dynamics (2006), the chapters on component mode synthesis; it is named, not
used for any number. Everything used is derived below from first principles (the reduction of ADR 0469 is a Galerkin
projection; adding a projected part to an ordinary model is the direct-stiffness assembly of its projected energy).

## Design

### The superelement (derivation)

A reduced part `P` of ADR 0469 has reduced unknowns `r = (u_b, eta)` (boundary unknowns, then its generalised coordinates: the
fixed-interface modal coordinates and, after them, the residual-vector coordinates of ADR 0474/0511 when it has any), and dense
symmetric `K_r`, `M_r`, the reduced load `f_r`, all obtained from its own model by `r -> x_P = T_P (u_b, Psi u_b + Phi eta + R
rho)`. Its strain and kinetic energies are `1/2 r^T K_r r` and `1/2 rdot^T M_r rdot` exactly for every motion in the range of
that map, and its load work `f_r^T r`.

The **host** is an ordinary `SolidModel` (any elements, constraints and elastic supports, zero constraint values) with the
transformation `x = T q` of ADR 0296 (`K_q = T^T (K + K_s) T`, `M_q = T^T M T`, `f_q = T^T f`; the path of
`modal_analysis_mpc`). The part is **attached**: each boundary unknown `b_j = (node, component)` of `P` is identified with the
unknown of the host node at the same position (within `MATCH_TOLERANCE` of the largest bounding-box extent of the host mesh,
exactly one host node required) and of the same component; that host unknown must be an independent unknown `q_a` of the host
transformation (refused otherwise: fixed in the host, or the dependent of a host constraint). The unknowns of the combined model
are

`z = (q, eta_1, eta_2, ...)`: the host independent unknowns in the order of the transformation (indices `0 .. n_q`), then the
generalised coordinates of part 1 in its reduced order, then those of part 2, ... (part `p` starts at `n_q + sum_{p' < p}
(n_r(p') - n_b(p'))`).

With the selection `r_p = S_p z` (row `j < n_b` of `S_p` the unit row of `q_a(j)`, row `n_b + k` the unit row of the part's
`k`-th generalised coordinate), the total energies are those of the host plus those of the parts, so by the principle of
stationary potential energy (statics) and Lagrange's equations (dynamics):

`K = K_q (+) sum_p S_p^T K_r,p S_p`, `M = M_q (+) sum_p S_p^T M_r,p S_p`, `f = f_q (+) sum_p S_p^T f_r,p`

(`(+)` the host matrices padded with zero rows and columns for the generalised coordinates). The reduced part's generalised
coordinates appear in the mass (the coupling block `M_bq` and `Phi^T M_ii Phi`) and in the stiffness (`Lambda`, and the
residual-vector blocks of ADR 0474 when present). `K` and `M` are sparse (`CsrMatrix::from_triplets`, duplicates summed),
symmetric by construction (the part's matrices are exactly symmetric after the symmetrisation of ADR 0469).

**Statics**: `K z = f` by `SkylineLdlt` (RCM, positive pivots; a failed pivot refused naming the unknown). The host displacements
are `x = T q`; the part's boundary displacements `u_b = S_p z` (first `n_b` rows) and its interior by the exact static recovery
of ADR 0469, `ReducedPart::recover_static(u_b) = Psi u_b + K_ii^{-1} f_i` (the generalised coordinates of a static solution
are not used for the interior). Exactness: the boundary block of `K_r` is the Schur complement `K_hat` and its coupling to the
generalised coordinates is zero (exact for the modes by `K_ib + K_ii Psi = 0`, ADR 0469; zero for the residual vectors by ADR
0474), and `f_hat = f_b + Psi^T f_i`; eliminating the part's interior exactly (block Gaussian elimination) from the full model
gives the same host-plus-`K_hat` system, so `q` and `u_b` equal the full solution up to rounding, for any mode count and any
interior load of the part; then the recovery is exact.

**Modal analysis**: `K phi = lambda M phi`, the lowest `m` pairs by `lowest_eigenpairs` with the options of `ModalOptions` (shift
`Shift::Auto` unless given, tolerance, residual test, subspace size, factorisation; the path of `modal_analysis_mpc`); mode
shapes as `z` vectors, the host's by `x = T q`, a part's by `ReducedPart::expand(S_p z)`. The combined basis (every host
unknown, the part's constraint and kept modes) is a subspace of the full model's unknowns, so by the min-max principle the
`k`-th frequency is not below the full model's `k`-th; it contains the basis of the two-part assembly of ADR 0469 (both halves
reduced with the same parts), so it is also not above that assembly's; with every fixed-interface mode kept it is the full
space and every frequency is exact.

**Direct harmonic solve** (damping form): hysteretic (structural) damping by loss factors, `Z(omega) = K + i D - omega^2 M`,
`D = eta_h K_q (+) sum_p eta_p S_p^T K_r,p S_p`, with one loss factor `eta_h` for the host (every host element and elastic
support) and one per part (`eta_p`, default `eta_h`); `Z X = F` by `ComplexSkylineLdlt` (complex symmetric `L D L^T`, RCM),
one factorisation per frequency. For a part made of one material with a uniform loss factor `eta`, `i eta K_r = T_cb^T (i eta
K_part) T_cb`: the reduced damping is the exact projection of the full model's, as the full model of `DirectHarmonic` with
`LossFactor::Constant(eta)` on all elements. The load: a `HarmonicLoad` over the host mesh equations (`T^T` of it) and, per
part, a `HarmonicLoad` over the part's reduced unknowns (for example `ReducedPart::reduce_load` of a pattern, ADR 0511).
`DirectHarmonic` itself (solid elements, its own assembly) is not changed; the solve of the combined model is a new function
of the new module that takes the host on the transformation path.

**Rotations of beam and shell interface nodes** (included, not refused): a part's boundary unknowns are those of its interface
nodes, every component the node has (six for a Beam2 or Shell4 node: three translations, three rotations about the global axes,
ADR 0417 and 0432), so a beam or shell part attached at a beam or shell node of the host is joined in all six; a part component
that the host node does not have (a beam or shell part at a solid node) is refused naming the node and component; a host node
with more components than the part's (a solid part at a shell node) keeps its extra components to the host alone (no check
covers this case: see Limits).

### Interface (new module `rcfd_fem::cms::superelement`, declared by one line in `cms.rs`)

`SuperelementModel::new(host)` (the host checked: `validate`, `check_supported_with_materials`, no cyclic constraint, zero
constraint values only); `add_part(part) -> index`; `n_host_unknowns`, `n_unknowns`, `part_offset(p)`, `part_map(p)` (the
combined index of every reduced unknown of part `p`); `matrices(threads)` (`K`, `M`, `f`, `D`'s two pieces); `solve_static(threads)`;
`modal_analysis(&ModalOptions)`; `harmonic(&SuperHarmonicOptions)` then `response(&SuperLoad, omega)`; `host_displacement(z)`,
`part_coordinates(p, z)`, `recover_part_static(p, part_model, z)`, `expand_part(p, part_model, z)`. Nothing else in the crate
calls it; `SolidModel` gets no new field (a field would change the struct every path matches, its serialisation and the case
file); the attachment is the wrapper that owns the host. No case-file key (open question 1).

### Entry checks (refusals)

Host: as above. Part attachment: a boundary node with no host node within the tolerance, or with two; a component the host node
lacks; a host unknown that is fixed or dependent; two boundary unknowns of one part on one host unknown. Solves: a mode count of
0 or at least the combined size (the eigensolver's refusal), a non-finite or negative loss factor or frequency, a load of the
wrong length.

Taken on the standing instruction: the wrapper instead of a `SolidModel` field; parts matched to host nodes by position (as
ADR 0469 matches parts); the host on the transformation path for all three analyses; loss-factor damping (uniform per host and
per part) for the harmonic solve; rotations included; a host interface unknown that is fixed or dependent refused; zero
constraint values only.

## Set-ups

Material steel, `E = 2.1e11`, `nu = 0.3`, `rho = 7850`, threads 1 unless said. Parts reduced by `reduce(.., CmsOptions::new(m)
.threads(1))`. Node positions of the halves and of the full model agree bit for bit (dyadic fractions) and are matched in the
tests by exact position.

- **Solid bar** (ADR 0469): full model F, `hex_box` `1 x 0.1 x 0.08`, `16 x 2 x 2`, `Hex8`, clamped on `xmin`; half A `[0, 0.5]`,
  `8 x 2 x 2`, clamped on `xmin`, interface `xmax`; half B from `x = 0.5`, `8 x 2 x 2`, interface `xmin`. Static loads (in F and
  in whichever half holds them): traction `(1e6, 2e6, -3e6)` Pa on `xmax` (x = 1) and gravity `(0, -9.81, -9.81)` everywhere.
  Harmonic load: nodal force `(1, 1, 1)` N at the corner `(1, 0.1, 0.08)`.
  - Arrangement **I**: A reduced (the part), B ordinary (the host, interface nodes its `xmin`).
  - Arrangement **II**: A ordinary (the host), B reduced (the part: the loads of x = 1 are in its interior).
- **Beam**: full model, Beam2 along x, `16` elements on `[0, 1]`, `rectangle_section(0.1, 0.08, 9.2e-6, 5/6)`, orientation
  `(0, 1, 0)`, clamped (six components) at x = 0; halves of 8 elements on `[0, 0.5]` (clamped) and `[0.5, 1]`; the interface is
  the one node at x = 0.5 (six unknowns). Static loads: tip force `(1e3, 2e3, -3e3)` N, tip moment `(10, 20, -30)` N m, gravity.
  Arrangements I and II as for the solid bar.
- **Shell**: full model, Shell4 strip `[0, 1] x [0, 0.1]` in the x-y plane, `16 x 2` cells, thickness 0.01, clamped (six
  components) on x = 0; halves `8 x 2` on `[0, 0.5]` (clamped) and `[0.5, 1]`; the interface is the three nodes at x = 0.5
  (eighteen unknowns). Static loads: force `(50, 20, -10)` N on each of the three nodes at x = 1, gravity. Arrangements I and II.
- Modal references: `modal_analysis(full, ModalOptions::new(8).tolerance(1e-12).residual_test(Backward).threads(1))` (the
  reference of ADR 0469); the combined models with the same options. Static reference: `solve_static(full, Direct)`. Harmonic
  reference: `DirectHarmonic` of F with `LossFactor::Constant(0.02)` on `all`; the combined model with `eta_h = eta_p = 0.02`.
- Two-part bound: `Assembly::new([A, B])` of ADR 0469 with the same two reduced halves (`m = 5` each), its dense eigen-solution.

## Arithmetic before this registration (disclosed)

Run once each on the API of 679081a1 (a temporary test file `f553_probe.rs`, removed before this commit; copies
`f553-scratch/probe1.rs`, `probe2.rs`, logs `probe1.log`, `probe2c.log`; `probe2.log` and `probe2b.log` are two failed
attempts of probe 2: `ModalBasis::all_modes` refuses a beam model and `stiffness_with_materials` is private, so the dense
spectrum was taken from a reduction keeping every fixed-interface mode, which is congruent to the full model):

1. Two-part assemblies (both halves reduced, `m = 5`), errors `f / f_full - 1` of the first three frequencies: solid bar (ADR
   0469 addendum) 4.53e-6, 5.46e-6, 5.56e-4; **beam** 5.04e-6, 6.10e-6, 6.00e-4 (full 417.85, 520.84, 2543.5 rad/s); **shell**
   2.33e-7, 3.27e-5, 1.16e-4 (full 52.91, 334.43, 566.23 rad/s). By the nesting above the combined models' errors lie between 0
   and these, up to rounding. Fixed-interface sizes: beam A 42 interior unknowns, B 48; shell A 126, B 144; solid A 189, B 216
   (ADR 0469 set-up). Every set-up reduces (beam and shell parts pass the entry checks of ADR 0469).
2. Rounding of the full models. `eps lambda_max / lambda_1` (eigenvalue, the dense bound): solid 3.4e-10, beam 1.0e-10,
   **shell 8.6e-8** (`lambda_max` 1.08e12, `lambda_1` 2.80e3). Dense against the subspace solution of the same pencil, first
   three frequencies: solid up to 2.2e-11, beam 2.5e-13, **shell 3.0e-10**. Static `Direct` against `SupernodalDirect` of the
   same model (largest difference over the largest displacement): solid 1.5e-12, beam 1.2e-13, **shell 1.43e-10**: two exact
   solvers of the shell disagree by more than 1e-10, so a shell limit of 1e-10 would test the rounding, not the method. The shell
   limits below are therefore 1e-8 (statics, 70 times the measured disagreement) and `-1e-8` for "not below" (33 times the
   measured eigen-solution disagreement, below the dense bound 4.3e-8 in frequency); the solid bar, the clause c5 to c7 of the
   acceptance, keeps 1e-10 (66 times its measured 1.5e-12) and `-1e-10` (4.5 times 2.2e-11); the beam keeps both 1e-10.
3. Harmonic rounding (solid, every mode kept): the condition of `Z` is about `lambda_max / min_k |lambda_k - omega^2|`, 5e5 or
   less at 300, 1000, 2000, 5000 rad/s (nearest eigenvalues from probe 1 of ADR 0469), giving `eps kappa` of 1e-10; at the full
   model's first resonance it is `lambda_max / (eta lambda_1) = 7.6e7`, `eps kappa` 1.7e-8. Limits 1e-8 off resonance and 1e-6
   at the resonance. With five modes: the two-part assembly's error up to 1000 rad/s was 2.27e-3 (ADR 0469
   H2); with one half ordinary the error is expected below it (not proved; the nesting holds for eigenvalues, not for responses).

## Checks (limits fixed now)

Tests in new files `crates/rcfd-fem/tests/sa_superelement_default_bits.rs` (D1) and `crates/rcfd-fem/tests/sa_superelement.rs`
(the rest), release profile. Relative errors of vectors are the largest absolute difference over the largest absolute
reference entry (complex modulus for the harmonic response), over every equation of the full model (host nodes and the recovered
part nodes, by position); frequency errors are `f / f_full - 1`. **Every aggregated error must be finite**: an aggregate that is
NaN or infinite fails its check whatever the limit (the maximum is taken so that a NaN propagates).

| Id | Clause | Check | Limit | Estimate |
|---|---|---|---|---|
| ST1 | c1, c2, c5 | solid bar I, `m = 5`: static displacements of the combined model (host, and the part recovered) against F | `<= 1e-10` | 1.5e-12 |
| ST2 | c1, c2, c5 | solid bar II, `m = 5` (the loads in the part's interior) | `<= 1e-10` | 1.5e-12 |
| ST3 | c8 | beam I and II, `m = 5`, each | `<= 1e-10` | 1.2e-13 |
| ST4 | c8 | shell I and II, `m = 5`, each | `<= 1e-8` | 1.43e-10 |
| MO1 | c3, c6, c7 | solid bar I, `m = 5`: first three frequencies of the combined model against F: `err_k <= 1e-3` and `err_k >= -1e-10` | limits | `0 < err_k <= ` 4.53e-6, 5.46e-6, 5.56e-4 |
| MO2 | c3, c6, c7 | solid bar II, `m = 5` | as MO1 | as MO1 |
| MO3 | c8 | beam I and II, `m = 5`: `err_k <= 1e-3`, `err_k >= -1e-10` | limits | `0 < err_k <=` 5.04e-6, 6.10e-6, 6.00e-4 |
| MO4 | c8 | shell I and II, `m = 5`: `err_k <= 1e-3`, `err_k >= -1e-8` | limits | `0 < err_k <=` 2.33e-7, 3.27e-5, 1.16e-4 |
| NB1 | c3 (design) | all six set-ups of MO1 to MO4: `err_k <= err_k(two-part assembly) + tol` (`tol` 1e-9; shell 1e-8), `k = 1, 2, 3` | limits | holds by nesting |
| AM1 | c3 | solid bar I and II with every fixed-interface mode kept (`m = n_i`: 189, 216): the first eight frequencies against F, each `abs(err) <= 1e-8` | `<= 1e-8` | 1.7e-10 |
| C1 | c3 (design) | solid bar I, `m = 5`: the first eight eigenvalues of `modal_analysis` of the combined model against the dense eigen-solution (`generalized_eigen_dense`) of its `K`, `M` (from `matrices`), relative | `<= 1e-9` | 1e-11 |
| HA1 | c4 | solid bar I and II, `m = n_i`, `eta = 0.02`, the corner load: response against `DirectHarmonic` of F at 300, 1000, 2000, 5000 rad/s | `<= 1e-8` each | 1e-10 |
| HA2 | c4 | same at the full model's first frequency (resonance) | `<= 1e-6` | 1.7e-8 |
| HB1 | c4 | solid bar I and II, `m = 5`, `eta = 0.02`: 30 points `j 1000 / 30` rad/s and the full model's frequencies below 1000 (476.7, 572.6) | `<= 1e-2` at every point | below 2.27e-3 |
| X1 | c1 (refusal) | part A attached to a host whose interface nodes are shifted by 1e-3 in x: refused (no host node at the position) | refused | holds |
| X2 | c8 (refusal) | the reduced beam half A attached to a solid host with a node at (0.5, 0, 0) (`hex_box` from (0.5, 0, 0), lengths `0.5 x 0.1 x 0.08`, `1 x 1 x 1` cells): refused (component 3 missing on the host node) | refused | holds |
| X3 | c1 (refusal) | part A attached to host B clamped on its `xmin` (the interface fixed in the host): refused | refused | holds |
| X4 | c1 (refusal) | host B with a prescribed value 1e-3 on `xmax` component 0: `SuperelementModel::new` refused | refused | holds |
| Z1 | c2 (design) | host B (clamped on `xmin`, its static loads) with no part: `solve_static` of the wrapper against `solve_static` of B (Direct) | `<= 1e-10` | 1e-13 |
| T1 | design | solid bar I, `m = 5`: `matrices` with threads 1 and 4 bit-identical (`K`, `M`, `f`) | bits | holds (ADR 0284) |
| D1 | c9 | digests at main 679081a1, printed by `sa_superelement_default_bits.rs` (written with the API of that commit and committed before any library line): `solve_static` of the solid, beam and shell full models (displacements), `modal_analysis` of F (eight eigenvalues), `DirectHarmonic` of F with the corner load at 1000 rad/s (`re`, `im`), `reduce` of A with `m = 5` (`K_r`, `M_r`, the load), the eigenvalues of the two-part `Assembly`; equal after the change | `==` | holds (no existing file changes behaviour) |

Records (reported, not in the decision rule): every measured error of the first eight frequencies in MO1 to MO4; the beam and
shell static errors against 1e-10 (ST4 against the acceptance number); the HB1 error curve; the sizes of the combined models.

## Negative controls and mutations

Negative controls: X1 to X4 (refusals). Mutations (on a scratch copy with its own `CARGO_TARGET_DIR`; each must be caught by at
least one registered check):

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | the part's mass scattered without its coupling block (`M_r[b][eta]` and `M_r[eta][b]` dropped) | MO1, AM1, HA1 |
| MU2 | the part's stiffness block of the generalised coordinates (`Lambda`) dropped | AM1, MO1 (a singular or wrong `K`) |
| MU3 | the part's reduced load not added to `f` | ST1, ST2 |
| MU4 | every part component joined to component 0 of the host node (component ignored) | ST1, MO1 |
| MU5 | the rotations of an interface node not joined (part components 3..5 dropped as if fixed) | ST3, ST4, MO3, MO4 |
| MU6 | the static interior of the part recovered from the generalised coordinates (`expand`) instead of `recover_static` | ST2, ST3 (II), ST4 (II) |
| MU7 | the part's loss factor not applied (`D` without the part's term) | HA1, HA2 |
| MU8 | `Z = K + i D + omega^2 M` (mass sign) | HA1, HB1 |
| MU9 | the position tolerance not applied (the nearest host node taken whatever its distance) | X1 |
| MU10 | the refusal of a component missing on the host node removed (the equation of the next node taken) | X2 |
| MU11 | the generalised coordinates of a part numbered from `n_q - 1` (overlapping the last host unknown) | ST1, MO1, AM1 |

## Decision rule

**done**: every check met at its registered run, every mutation caught, no amendment after a run. **partial**: a check missed
(recorded as `*_known_miss` with its limit), a mutation not caught by a registered check, or a check met only after an amendment
made after its run. **todo**: the module does not build or the bar runs cannot be made.

## Limits (not covered)

- No case-file key: a combined model is built through the library only (open question 1). The part's loads are those reduced
  with it (its model's loads, or `reduce_load` of a pattern for the harmonic solve); a static load given to the part after the
  reduction is not provided.
- Zero constraint values only (host and part); a host interface unknown that is fixed or the dependent of a constraint is refused
  (the part must be attached at free host unknowns); no cyclic constraint; parts matched to host nodes by position, by a search
  over every host node (cost `n_host_nodes x n_interface_nodes`).
- Harmonic damping is a loss factor per host and per part only (no Rayleigh damping, no dashpot, no frequency-dependent or
  viscoelastic modulus, no base motion); the harmonic solve of beams and shells is not checked (the reference `DirectHarmonic`
  is a solid-element path).
- A solid part at shell or beam host nodes (translations joined, the host rotations left to the host) and parts with the
  interface enrichment of ADR 0532 are not covered by any check; stresses of the recovered part are not checked here (ADR 0469
  checks the recovery).
- The combined matrices hold each part's dense blocks; a large interface defeats the purpose (ADR 0333).

## Open questions taken on the standing instruction

1. A case-file key for superelements: not added (it would change the case schema and its golden; F-603's agent recommended
   adding `interface_enrichment` together with this item's key). Left open for the maintainer; recommendation in the addendum.
2. The attachment as a wrapper (`SuperelementModel`) and not a field of `SolidModel`: taken (no existing type or golden changes).

## Addendum 1: results (2026-11-14)

Commits on `f553-work`: 1b17befc (this registration, alone), a21ec80a (D1 digests printed at main 679081a1 by
`sa_superelement_default_bits.rs`, before any library line), 5cda8f68 (`rcfd_fem::cms::superelement`, the one-line declaration
and a doc paragraph in `cms.rs`, the checks `tests/sa_superelement.rs`; formatted before the run), then this addendum and the
roadmap entry. Each registered check was run once, release profile (`cargo test --release -p rcfd-fem --test sa_superelement --
--nocapture --test-threads=1`, log `f553-scratch/run1.log`); no amendment was made. No shared solver file changed behaviour (the
only library lines are the new file and the declaration in `cms.rs`).

| Id | Clause | Limit | Measured | Met |
|---|---|---|---|---|
| ST1 | c1, c2, c5 | `<= 1e-10` | 6.35e-13 (solid I, 248 unknowns) | yes |
| ST2 | c1, c2, c5 | `<= 1e-10` | 4.06e-13 (solid II, 221) | yes |
| ST3 | c8 | `<= 1e-10` | 2.35e-14 (beam I), 4.84e-14 (beam II) | yes |
| ST4 | c8 | `<= 1e-8` | 8.54e-11 (shell I), 4.51e-11 (shell II); both also below 1e-10 (record) | yes |
| MO1 | c3, c6, c7 | `[-1e-10, 1e-3]` | 4.47e-7, 5.92e-7, 9.73e-5 | yes |
| MO2 | c3, c6, c7 | `[-1e-10, 1e-3]` | 4.08e-6, 4.87e-6, 4.59e-4 | yes |
| MO3 | c8 | `[-1e-10, 1e-3]` | I 5.40e-7, 7.18e-7, 1.17e-4; II 4.50e-6, 5.38e-6, 4.83e-4 | yes |
| MO4 | c8 | `[-1e-8, 1e-3]` | I 1.87e-8, 4.35e-6, 2.16e-5; II 2.14e-7, 2.83e-5, 9.43e-5 | yes |
| NB1 | c3 | `err - err_two <= 1e-9` (shell 1e-8) | all eighteen negative, the largest -1.88e-8 (shell II f1) | yes |
| AM1 | c3 | `<= 1e-8` | 9.13e-13 (I, `m` = 189), 3.06e-12 (II, `m` = 216) | yes |
| C1 | c3 | `<= 1e-9` | 1.27e-10 (248 unknowns) | yes |
| HA1 | c4 | `<= 1e-8` | 5.79e-12 (I), 1.09e-11 (II) | yes |
| HA2 | c4 | `<= 1e-6` | 1.06e-10 (I), 1.75e-10 (II) at 476.681 rad/s | yes |
| HB1 | c4 | `<= 1e-2` | 6.61e-5 (I), 7.02e-3 (II), both largest at 1000 rad/s | yes |
| X1 | c1 | refused | refused (0 host nodes within 4.99e-10) | yes |
| X2 | c8 | refused | refused (component 3 has no unknown on the solid host node) | yes |
| X3 | c1 | refused | refused (host interface unknown fixed) | yes |
| X4 | c1 | refused | refused (zero values only) | yes |
| Z1 | c2 | `<= 1e-10` | 0 (bit-equal to `solve_static`) | yes |
| T1 | design | bits | threads 1 and 4 bit-identical | yes |
| D1 | c9 | `==` | 10 of 10 digests equal | yes |

Records: the errors of all eight frequencies (log `run1.log`): solid I up to 3.3e-3 (f7, the axial mode), solid II 9.3e-2 (f7,
axial: B, the free half, keeps no axial fixed-interface mode, as in ADR 0469); beam II 8.3e-2 (f8); shell II 7.0e-3 (f7). The
errors of arrangements I and II add up to those of the two-part assembly for the first three frequencies of every family to
about 1 percent (for example solid f1: 4.47e-7 + 4.08e-6 = 4.53e-6, the two-part error): the truncation errors of the two halves
are nearly additive at this level. HB1 II is larger than HB1 I because the corner load is in the reduced part's interior and
the part has no residual vector (the cause measured in ADR 0469 and treated by ADR 0474/0511).

Mutations (scratch copy `f553-scratch/mut`, its own target, script `mutate.py`, logs `mutations.log`, `mut-MU*.log`), all caught:

| Id | Caught by |
|---|---|
| MU1 | AM1, HA1, HA2, HB1, MO1 to MO4, NB1 |
| MU2 | MO1 to MO4 ("not below"), C1, HA1, HA2, HB1; ST and AM1 through the solver's refusal of the singular `K` |
| MU3 | ST1 to ST4 |
| MU4 | X2; every other check through the attachment's refusal of two boundary unknowns on one host unknown |
| MU5 | MO3, MO4, NB1 (beam, shell), ST3, ST4 |
| MU6 | ST1 to ST4 |
| MU7 | HA1, HA2, HB1 |
| MU8 | HA1, HA2, HB1 |
| MU9 | X1 |
| MU10 | X2 |
| MU11 | ST, MO, AM1, C1, HA, HB through the solvers' refusal of the singular combined matrices (no printed limit reached) |

`cargo fmt -p rcfd-fem --check`: clean. `cargo clippy -p rcfd-fem --all-targets -- -D warnings`: clean. `cargo test -p
rcfd-fem --release`: 216 test binaries, 928 passed, 0 failed, 262 ignored. `cargo test -p rcfd-api --release --test
baseline_hashes`: 3 passed, 1 ignored (the hash table unchanged and not edited). `cargo test -p rcfd-cli --release --test
docs_site`: 2 passed.

### Deviations

1. MU2, MU4 and MU11 are caught in part through refusals of the library (a singular combined matrix, a double attachment): the
   tests of the expected checks fail; for MU11 no printed limit is reached because every solve refuses.
2. The probe file of the arithmetic was run before the registration and removed (disclosed above); its two failed attempts are
   logged.

Not run: the workspace test suite (the lead's gate).

### Decision

**done** by the registered rule: every check met at its registered run, every mutation caught, no amendment.

### Remaining questions

1. Case-file key. Options: (a) add a `superelements` key (part file path, host interface) together with F-603's
   `interface_enrichment` in one schema change with its golden re-recorded; (b) keep the library-only API. Recommendation: (a),
   as one follow-up item, since it changes the schema golden and needs the maintainer's approval.
2. Harmonic damping beyond loss factors (Rayleigh, dashpots, viscoelastic parts). Recommendation: a follow-up only when a case
   needs it; the loss-factor form is the exact projection for a uniform loss.
3. A solid part at shell or beam host nodes (the mixed interface) is accepted but not checked. Recommendation: check it in the
   follow-up of question 1 (it needs a solid face matched to shell nodes, which position matching does not give on its own).

### For the lead: roadmap entries

Proposed status of F-553: **done**. New note:

> Structural solver, advanced tier. Follow-up of F-430 (ADR 0469). ADR 0536, 2026-11-14: rcfd_fem::cms::superelement: SuperelementModel attaches reduced parts of ADR 0469 to an ordinary host model at the host nodes of their boundary positions (same component; rotations of beam and shell nodes joined, a component the host lacks refused), the part's generalised coordinates numbered after the host's independent unknowns; static solve (part interior by the exact static recovery), modal analysis (subspace iteration on the combined K, M) and direct harmonic solve with loss-factor damping per host and part. Bar of a reduced half and an ordinary half: statics within 6.4e-13 (beam 4.8e-14, shell 8.5e-11), first three frequencies 4.5e-7 to 4.6e-4 above the full model (beam 5.4e-7 to 4.8e-4, shell 1.9e-8 to 9.4e-5) and never above the two-part assembly's, every mode kept exact within 3.1e-12, harmonic within 1.1e-11 of DirectHarmonic with every mode kept and 7.0e-3 up to 1000 rad/s with five; defaults bit-identical (10 digests). No case-file key (with F-603's interface_enrichment, open).

Proposed follow-up (number left to the lead), title "Case-file keys for superelements and interface enrichment", deps F-553, F-603:

> Every limit not stated here is registered in the ADR before the run. A case file names reduced part files of ADR 0469 and the host node sets they attach to (key superelements) and the interface enrichment of ADR 0532 (key interface_enrichment); the bar of ADR 0536 run from a case file gives the library's results in bits; the schema golden re-recorded with the maintainer's approval; a case without the keys gives every result bit-identical.

## Answers of the maintainer (2026-11-18), written into this ADR by the lead on 2026-11-19

The maintainer decided these on 2026-11-18 as sets of options with a recommendation; until now the decision stood in the roadmap note of the item only. Nothing above this section is changed.

- **F-616, Addendum 1, remaining question 1 (case-file key).** Asked: the keys `superelements` and `interface_enrichment`, with the schema golden re-recorded once. Answer: Approved under the class rule for additive golden edits (agent rules, rule 3, from version 2): an edit that only adds entries and removes or changes none is reported by the agent as an "additive golden edit" with its diff, re-recorded by the lead and written in the ledger line of the merge. `baseline_hashes.rs` is never in this class, and an edit that changes or removes an entry still needs the maintainer's word for that item. The item may be issued. ADR 0532 (remaining question 1) asks for the second key; this answer covers it.
