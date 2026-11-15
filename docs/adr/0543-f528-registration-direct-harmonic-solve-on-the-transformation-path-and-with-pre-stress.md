# ADR 0543: F-528 registration: the direct harmonic solve on the transformation path (constraints, rigid sets, elastic supports) and with a pre-stress

Status: **REGISTERED 2026-11-15, written before any code of F-528 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-528 was written, compiled or run: the files under "What was read" were read, and the
arithmetic below was done by hand (no script, no rcfd run; disclosed). The default-bits record B0 (D7) is a test file written
and run on the unchanged base 498b5417 after this commit and before any library code. Results go in the addenda at the end;
nothing above the addenda is changed after a run.

Roadmap item F-528 (deps F-412), the follow-up named by [ADR 0448](0448-f412-registration-direct-harmonic-complex-ldlt.md)
(its refusal R9 and its limits). Number 0543 given by the lead. Branch `f528-work` from 498b5417, worktree `w49i-work/f528`,
scratch `w49i-work/f528-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored `*_known_miss`
test with the limit intact and the measured number); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe code; clean-room. The
brief asks for the F-528 entry of `docs/ROADMAP.md` and `docs/roadmap.json` to be updated by this branch in a separate last
commit (the svgs not regenerated). Areas of other agents not touched: `contact.rs` (F-601), the hyperelastic materials (F-425),
the factor choice of the Newton driver (F-618). Every choice below that the maintainer would normally be asked about is taken on
the maintainer's standing instruction of 2026-11-11 (take the recommendation).

## What F-528 asks

The acceptance, binding:

> Every limit not stated here is registered in the ADR before the run. The direct harmonic solve of ADR 0448 takes linear constraints, rigid sets and elastic supports through the transformation u = T q (complex right-hand side T^T F, response T q) and a pre-stress as the added stiffness K_sigma: a bar on a nodal spring at the tip equals the 1-D chain with that spring within 1e-8; a model with a rigid set equals the same model with the set's nodes merged within 1e-8; with a pre-stress the undamped direct solve equals the F-399 mode superposition with the pre-stressed basis and all modes within 1e-8; defaults bit-identical.

Clauses: **c0** every limit registered here; **c1** linear constraints, rigid sets and elastic supports through `u = T q`, right-hand
side `T^T F`, response `T q`; **c2** a pre-stress as the added `K_sigma`; **c3** bar on a tip spring against the 1-D chain within
`1e-8`; **c4** a rigid set against the model with the set's nodes merged within `1e-8`; **c5** pre-stressed undamped direct
against F-399 with the pre-stressed basis and all modes within `1e-8`; **c6** defaults bit-identical.

The brief adds: register how each damping form (Rayleigh, loss factor, viscoelastic, dashpots) transforms with `T` and which are
refused on the transformation path; register the elastic-support form; opt-in by model content; a default-bits record first;
`baseline_hashes` unchanged; no case-file key; mutations the checks catch; every aggregated test error finite.

## What was read (no run)

`AGENT_RULES.md`; the brief; ADR 0448 whole (D2 the operator `A(omega)`, the resonance pivot rule, checks H1 to H9, R1 to R11,
amendment 2 and R8b, addendum 1: H2 every bar response against the 1-D chain 5.6e-14 to 1.31e-11, H3 Rayleigh direct against
F-399 with all 162 modes 1.1e-12 to 2.6e-10); `crates/rcfd-fem/src/harmonic_direct.rs` at 498b5417 (1030 lines, with the
viscoelastic parts of ADR 0491, the stress of ADR 0494, the supernodal factor of ADR 0509 and the Hex8 formulations of ADR 0525:
`crate::hex8x::element_stiffness_for` in the per-material parts, `refuse_incompatible_viscoelastic`); ADR 0296 (the transformation
`x = T q + g`, the unknowns `x = (u, r)` with six reference unknowns per rigid set, the elimination order and step 5 for a row
whose designated unknown is already dependent, `K_r = T^T K_ext T`, `f_r = T^T (f - K g)`, `T^T rho = 0` for the constraint
force, D4 the support forms); `crates/rcfd-fem/src/mpc.rs` (`uses_transformation`, `transformation`, `Transformation::{t, expand,
transpose_times, reduce, independent, n_mesh}`, `support_triplets`, `add_supports`, `unknown_name`, `refuse_nonzero_values`,
`modal_analysis_mpc`); ADR 0391 (F-399, mode superposition, `ModalDamping::Rayleigh` as `c_r = alpha + beta lambda_r`, the
convention `x(t) = Re(X e^{i omega t})`) and `harmonic.rs` (`HarmonicSolver`, `ModalBasis::new`, `check_model`, the static
correction on the transformation path of ADR 0400 D2 with `K_sigma` of ADR 0400 D3); ADR 0390 and `geometric.rs` (`Prestress`,
`Prestress::uniform`, `check_fits`, `geometric_stiffness`, `assemble_geometric` on the pattern of the stiffness,
`prestressed_modes` and its transformation path of ADR 0398 with `T^T (K_ext + K_sigma) T`); `rcfd_linalg::eigen::
generalized_eigen_dense`; `crates/rcfd-fem/tests/sa_harmonic_direct.rs` (helpers `bar_with`, `chain_tip`, `Checks`, the refusal
test `r1_to_r11_refusals_amended` whose row R9 expects the refusal of an elastic support); `sa_harmonic_visco_default_bits.rs`
(the form of a default-bits record); ADR 0539 (form). `rcfd-api`, `rcfd-cli` and `rcfd-mcp` do not call
`rcfd_fem::harmonic_direct` (grep at 498b5417): the case file has no direct harmonic analysis, so no case file routes
constraints into it, no case result and no golden can change, and no case-file key is added.

## Design

### D1. The operator on the transformation path (c1)

A model with `crate::mpc::uses_transformation(model)` (a linear constraint, a rigid set, a cyclic constraint or an elastic
support) is no longer refused by `DirectHarmonic::new` (except as in D4): it is solved on the transformation of ADR 0296,
`x = T q` (`g = 0`, D4: zero constraint values only), `x = (u, r)` over `N = n + 6 n_R` unknowns. On the mesh unknowns, on an
extended pattern `P` (D3), the complex dynamic stiffness is

`A_ext(omega) = sum_m c_m(omega) K_m + K_s + K_sigma - omega^2 M + i omega (alpha M + beta K_beta + C_d)`,
`K_beta = K_0 + K_s + K_sigma`, `K_0 = sum_m K_m` (the instantaneous base stiffness of ADR 0491 for a viscoelastic part),

with `c_m` the coefficient of ADR 0448 / ADR 0491 of each stiffness part (`1`, `1 + i eta`, `s (1 + i eta)`, `E*/E`, `G*/G_0`,
`K*/K_0`), `K_s` the supports (D2), `K_sigma` the pre-stress (D5, zero without one), `C_d` the dashpots; the reference rows and
columns are zero. The steady harmonic balance with the constraint force `R`, `A_ext X = F + R`, admissible `X = T Q`, and `T^T R
= 0` (the constraint force does no work on admissible motions, ADR 0296; virtual work with `delta x = T delta q`) give

`A_r(omega) Q = T^T F`, `A_r = T^T A_ext T`, `X = T Q`,

with `F` the complex load extended by zeros at the reference unknowns, `T^T F = T^T Re F + i T^T Im F` (`T` is real). `A_r` is
complex symmetric (`A_ext^T = A_ext`, `T` real), so the complex `L D L^T` of ADR 0448 (envelope after reverse Cuthill-McKee)
applies unchanged. Because `T` is real and the congruence is linear, `A_r = sum_m c_m K_m,r + K_s,r + K_sigma,r - omega^2 M_r + i
omega (alpha M_r + beta K_beta,r + C_d,r)` with every real matrix reduced once in `new` (`Transformation::reduce`); only the
coefficients change with `omega`. The response returned is the mesh part of `T Q` (`HarmonicResponse` over the model's `n`
equations, as `modal_analysis_mpc` returns the mesh part of each mode); the reference unknowns are not returned (limit).

### D2. How each term transforms; the elastic-support form

| term | on the mesh | on the transformation path |
|---|---|---|
| loss factor (constant or table) of material `m` | `s_m (1 + i eta_m) K_m` | `s_m (1 + i eta_m) T^T K_m T` |
| viscoelastic part (ADR 0491) | `(Z*(omega)/Z_0) K_part` | `(Z*(omega)/Z_0) T^T K_part T` |
| Rayleigh | `i omega (alpha M + beta K_beta)` | `i omega (alpha T^T M T + beta T^T K_beta T)` |
| dashpots (ground, two-node) | `i omega C_d` | `i omega T^T C_d T` (a dashpot at a dependent unknown, e.g. a rigid-set node, acts through `T`; at a prescribed equation its row of `T` is zero and it does nothing) |
| elastic support | `K_s`, real | `T^T K_s T`, real |
| pre-stress | `K_sigma`, real | `T^T K_sigma T`, real |
| mass | `- omega^2 M` | `- omega^2 T^T M T` (the reference unknowns carry no mass of their own) |
| base motion | `- A_FP G_P` | **refused** (D4) |

The elastic support is the stiffness of ADR 0296 D4 (`mpc::support_triplets`): a `NodalSpring` `k` on the diagonal of each node
of its set at its component, a `Foundation` the consistent `int k_f N_a N_b n n^T dA` over the faces of its surface set. It is
real and undamped: no loss factor (a loss factor is given per element set of a material; a support is not one), no mass; a
damped support is a dashpot at the same component (taken). It enters `K_beta`, so that `beta` acts on the stiffness whose
eigenvalues the F-399 basis of the same model has (`ModalDamping::Rayleigh` gives `c_r = alpha + beta lambda_r` with `lambda_r`
of `T^T (K_0 + K_s) T`; D5 likewise for `K_sigma`). On the plain path without a pre-stress `K_beta = K_0` as before.

### D3. The extended pattern

Every real matrix of D1 is put on one pattern `P` over the mesh unknowns: the stiffness pattern of `stiffness_with` (the pattern of
`assemble_with`, which `consistent_with` and `assemble_geometric` share; checked) united row by row with the dashpot entries and
the support triplets. A matrix is zero where it has no entry. `Transformation::reduce` forms `T^T X T` by the structure of its
input, so every reduced matrix has the same pattern (checked; refused as an internal pattern error otherwise). The factor is the
envelope of ADR 0448 in reverse Cuthill-McKee order of the reduced pattern; the assembly takes the thread count; `reduce`, the
factor and the solve are sequential, so the result is bit-identical for any thread count (T1).

### D4. What the transformation path refuses

* A cyclic constraint (`key "constraint"`, naming `Cyclic`): its harmonic transformation (ADR 0386) belongs to the harmonic index,
  not to a forcing frequency.
* A non-zero constraint value: a prescribed value or a `Linear` value `!= 0` (`mpc::refuse_nonzero_values(model, "the direct
  harmonic solve")`, key `"constraint"`, "takes zero values only"): a static value has no harmonic meaning here.
* Base motion (`key "base motion"`, "transformation path"): the harmonic `g` of `x = T q + g` is a follow-up.
* The supernodal factor `DirectFactor::SupernodalNestedDissection` (`key "solver"`, "transformation path"): its node-coordinate
  order has no coordinates for the reference unknowns; the envelope is taken.
* As before on both paths: the model checks of `DirectHarmonic::new` (solid elements; `check_supported` or the viscoelastic check),
  negative or non-finite coefficients, the minimum-degree factor.

An undamped reduced `A_r` with a relative pivot `<= resonance_pivot` is refused as a resonance as in ADR 0448 D2, the unknown named
by `mpc::unknown_name` (a mesh component or a reference unknown of a rigid set).

### D5. The pre-stress (c2)

`DirectHarmonicOptions::prestress: Option<Prestress>` (default `None`; builder `.prestress(p)`): `K_sigma` of
`geometric::assemble_geometric` (ADR 0390 D1, the standard rule of each element as on the pre-stressed modal paths;
`Prestress::check_fits` refuses a pre-stress of another model) is added on the pattern, real, undamped, on both paths, and enters
`K_beta`. Its stiffness is that of the stress state, not of a material: no loss factor and no viscoelastic coefficient multiplies
it. It is the operator of `geometric::prestressed_modes` (`K + K_sigma`; on the transformation path `T^T (K_ext + K_sigma) T`), so
the undamped direct solve equals the mode superposition with all modes of that pencil (c5). `DirectHarmonic::stress` stays the
stress of the harmonic displacement (ADR 0448 amendment 2, ADR 0494); the pre-stress itself is not added. Beyond the buckling load
(`K + K_sigma` indefinite on the admissible motions) the solve is not refused (limit).

### D6. Opt-in by model content and options; the code of the default

The plain path (no transformation, `prestress` `None`) keeps its code: the transformation path is a separate branch of `new` and
of `solve`; the only change on the plain path is a `geometric: Option<Vec<f64>>` field read by `value` when `Some` (never for the
default) and a `transformation: Option<..>` field read by `solve` when `Some`. `free()` on the transformation path returns the
independent unknowns of `T` (ascending; reference unknowns are `>= n`).

### D7. Default bits and the superseded refusal row

B0, written and run on 498b5417 before any library code (expected digests printed by that run and written into the file): the
direct harmonic responses of the models of `sa_harmonic_visco_default_bits.rs` without constraints (bar constant loss, bar
table, two-material bar, cantilever Rayleigh with dashpots and its stress, bar base motion), a viscoelastic bar response, the
supernodal factor's response of the cantilever, and the refusal message of a non-zero prescribed value; after the library code
the same file gives the same digests.

Row R9 of `r1_to_r11_refusals_amended` (ADR 0448 amendment 2, `sa_harmonic_direct.rs`) expects the refusal that F-528 lifts. That
test gets an `#[ignore = "superseded by ADR 0543 (F-528): ..."]` attribute, its body unchanged (the precedent of
`f295_seam_diffusivity.rs`); the copy `r1_to_r11_refusals_f528` in the new test file runs every other row unchanged and R9' (the
same model accepted, its response finite). This is the only edit of an existing test file.

## Arithmetic done beforehand (disclosed)

By hand, no script:

* Bar of ADR 0448: `E = 2e11`, `rho = 7850`, `A = 1.6e-3 m^2`, `L = 1`, `c0 = sqrt(E/rho) = 5047.5 m/s`, `w1 = pi c0 / (2 L) =
  7928.8 rad/s`, `EA = 3.2e8 N`. Tip spring `K_t = EA/L = 3.2e8 N/m`, `k = K_t/4 = 8e7 N/m` on each of the four `xmax` nodes.
  Undamped continuous bar with the tip spring: resonance where `EA k cos kL + K_t sin kL = 0`, with `K_t = EA/L` `tan(kL) = -kL`,
  roots `kL = 2.029, 4.913`; the registered frequencies `0.5, 1.5, 2.5 w1` are `kL = 0.785, 2.356, 3.927`,
  not near a root (the 80-element chain differs from the continuum by about `(kh)^2/24 <= 1e-4`).
* Hex8 bar with `nu = 0`, lateral motion held and a load uniform over the section: each Hex8 gives on the axial unknowns of a
  uniform section exactly the 1-D bar `EA/h [1 -1; -1 1]` and the consistent mass `rho A h/6 [2 1; 1 2]` (the trilinear
  functions sum over the section to the linear ones); with the section's symmetry every node of a section has the same `u_x`.
  So the 3-D bar equals the chain up to rounding (ADR 0448 H2 measured 5.6e-14 to 1.31e-11). A foundation `k_f = E/L` on face
  `xmax` gives for a uniform face motion the total `k_f A = EA/L = K_t` (its consistent matrix applied to a uniform vector sums
  `N_a` to one).
* Pre-stress of the cantilever `hex_box([0;3], [0.6, 0.1, 0.1], [6, 2, 2])`: for tension `sigma_xx` the ratio of the geometric to
  the bending stiffness of the first mode is about `P L^2/(EI) = sigma L^2 12/(E h^2) = 432 sigma/E`; `sigma = 2e8 Pa` gives
  `0.43`, a first-mode eigenvalue change of the order of 10 to 40 percent: N1 is registered at `>= 1e-2`. Euler-Bernoulli `w_1 =
  3.516 sqrt(E h^2/(12 rho))/L^2 = 1.42e3 rad/s` (the Hex8 model is stiffer); the next modes are the other bending direction
  (equal, square section), torsion near `7.5e3 rad/s` (`5.3 w_1`) and second bending near `6.3 w_1`; the frequencies `0.5, 1.7,
  2.5 w_1` (of the pre-stressed basis) keep away from every eigenvalue by at least 30 percent of `w_1`. With all modes the mode
  superposition equals `(K + K_sigma - w^2 M)^-1 F` exactly in exact arithmetic; the difference is the eigensolution's rounding
  amplified by the condition of `K + K_sigma - w^2 M`, about `1e-12` (ADR 0448 H3 measured up to 2.6e-10 at a resonance-near
  frequency): limit `1e-8`.
* Rigid set in the split cantilever: 9 junction nodes at `x = 0.3` duplicated (18 nodes in the set, 54 rows, 6 reference
  unknowns); the merged model has 9 nodes in its set (27 rows). Both admit exactly the same motions of the mesh (each pair of
  coincident nodes moves as the rigid face), with the same element stiffness and mass on them, so they are the same reduced
  problem in different coordinates: equal up to rounding (estimate 1e-12). In the split bar the rigid rows of the lateral
  components meet prescribed `y, z` of every node (step 5 of ADR 0296: rows on the reference unknowns, the redundant ones
  dropped).
* The unknowns: bar 80 cells `81 x 4` nodes, 324 axial unknowns; cantilever 189 equations, 27 clamped, merged with the rigid set
  `162 - 27 + 6 = 141` independent unknowns (dense eigensolution of `141 x 141` in the test).

## Checks

Common: `e(X, Y) = max_j |X_j - Y_j| / max_j |Y_j|` over every equation compared (complex modulus). Every aggregated error is
asserted finite (F1) before its limit is tested. Bar: 80 Hex8 `nu = 0`, `y, z` held on `all`, `x` held on `xmin`, `0.25 N` real on
each `xmax` node in `x` (the bar of ADR 0448). Cantilever `C`: `hex_box([0;3], [0.6, 0.1, 0.1], [6, 2, 2])`, steel `nu = 0.3`,
`xmin` clamped, tip load `(0.3, 1, -0.5)/9` real and `(0, 0.2, 0)/9` imaginary on each `xmax` node. `w_1` of a cantilever check is
`sqrt(lambda_1)` of the dense pencil of that check's reference (stated per row).

| check | clause | set-up | limit | estimate |
|---|---|---|---|---|
| B0 | c6 | D7: the digests of 498b5417 | 0 digests differ | 0 |
| K1a..c | c1, c3 | bar with `NodalSpring("xmax", 0, 8e7)`, loss factor 0.02, at 0.5, 1.0, 1.5 `w1`: every axial unknown against the 1-D chain of 80 linear elements with consistent mass, `E (1 + 0.02 i)`, a real tip spring `3.2e8` (complex Thomas algorithm in the test): `e` | `<= 1e-8` | 1e-11 |
| K1d..f | c1, c3 | same, undamped, 0.5, 1.5, 2.5 `w1` | `<= 1e-8` | 1e-11 |
| K1g | c1, c3 | same, undamped material, plus the tip dashpot `c = 0.1 A sqrt(E rho)` (c/4 per node) at 1.0 `w1` against the chain with spring and dashpot | `<= 1e-8` | 1e-11 |
| FD1 | c1 | bar with `Foundation("xmax", E/L)` instead of the spring, loss factor 0.02, 1.0 `w1`, against the chain of K1b | `<= 1e-8` | 1e-11 |
| RS1a..c | c1, c4 | the bar split at `x = 0.5` (the four nodes there duplicated, the right half on the copies), `RigidSet("junction", (0.5, B/2, B/2))` on the eight, loss factor 0.02, 0.5, 1.0, 1.5 `w1`, against the merged bar (the plain path): `e` over every node of the split model against its merged node | `<= 1e-8` | 1e-12 |
| RS2a..c | c1, c4 | `C` split at `x = 0.3` (9 nodes duplicated), `RigidSet("junction", (0.3, 0.05, 0.05))` on the 18, plus `NodalSpring("xmax", 2, 1e6)`, loss factor 0.02, Rayleigh `alpha = 0.02 w_1`, `beta = 0.02 / w_1`, a ground dashpot `1e3 N s/m` on junction node `J` (the original node at `y = z = 0`) component 1, and the load of `C` plus `(0, 0, 0.4)` real and `(0.1, 0, 0)` imaginary on `J`; against the merged `C` with the rigid set on its 9 junction nodes and everything else the same; 0.5, 1.7, 2.5 `w_1` (`w_1` of the dense pencil `T^T (K + K_s) T, T^T M T` of the merged model): `e` as RS1 | `<= 1e-8` | 1e-12 |
| RY1a..d | c1 | the merged model of RS2 (rigid set and spring), Rayleigh only (`alpha = 0.02 w_1`, `beta = 0.02/w_1`), load of `C`, at 0.5, 1.0, 1.7, 2.5 `w_1`: direct against `HarmonicSolver` without static correction, `ModalDamping::Rayleigh`, on the basis of every eigenpair of the dense pencil `T^T (K + K_s) T, T^T M T` (each mode the mesh part of `T q`): `e` | `<= 1e-8` | 1e-11 |
| P1a..c | c2, c5 | `C` with `Prestress::uniform([2e8, 0, 0, 0, 0, 0])`, undamped, at 0.5, 1.7, 2.5 `w_1` (of the pre-stressed pencil): direct against `HarmonicSolver` without static correction, `ModalDamping::Ratio(0)`, on every eigenpair of the dense reduced pencil `(K + K_sigma)_r, M_r` (the free equations): `e` | `<= 1e-8` | 1e-12 |
| P2a..c | c1, c2, c5 | the merged model of RS2 with the pre-stress of P1, undamped, against every eigenpair of `T^T (K + K_s + K_sigma) T, T^T M T`, 0.5, 1.7, 2.5 `w_1` (of that pencil) | `<= 1e-8` | 1e-12 |
| P3a..b | c2 | P1 with Rayleigh `alpha = 0.02 w_1`, `beta = 0.02/w_1` against `ModalDamping::Rayleigh` on the pre-stressed basis, 1.0 and 2.5 `w_1` (D2, D5: `beta` on `K + K_sigma`) | `<= 1e-8` | 1e-11 |
| Z1 | c2 | P1a with a zero pre-stress against the solve without a pre-stress | `<= 1e-14` | 0 |
| T1 | D3 | K1b, RS2b, P2b with assembly threads 1 and 8 | 0 bits differ | 0 |
| F1 | brief | every aggregated error of the rows above finite | finite | finite |
| RT1 | D4 | RS2's merged model with a base motion on a clamped node: refused, key `"base motion"`, "transformation path" | as stated | as stated |
| RT2 | D4 | same with `DirectFactor::SupernodalNestedDissection`: key `"solver"`, "transformation path" | as stated | as stated |
| RT3 | D4 | `C` with a cyclic constraint of `ymin`, `ymax` (4 sectors about `x`): key `"constraint"`, containing "Cyclic" | as stated | as stated |
| RT4 | D4 | `C` with a `Linear` constraint of value `1e-3`: key `"constraint"`, "takes zero values only" | as stated | as stated |
| RT5 | D5 | a pre-stress of the bar given to `C`: refused (`FieldLength`, key `"stress"`, or the error of `check_fits`) | refused | refused |
| R1..R11' | D7 | `r1_to_r11_refusals_f528`: the rows of `r1_to_r11_refusals_amended` unchanged, R9' the sprung bar accepted with a finite response at 0.5 `w1` | as stated | as stated |

## Negative controls and mutations

Controls (each must show a difference, so that the equality checks are not vacuous):

| control | set-up | limit |
|---|---|---|
| N1 | P1a..c against the same solve without the pre-stress: `e` | `>= 1e-2` at each frequency |
| N2 | K1a against the bar without the spring: `e` | `>= 1e-2` |
| N3 | RS2b split model with the rigid set against the merged model without the rigid set: `e` | `>= 1e-5` |

Mutations, each run once on a scratch copy (`f528-scratch`, its own `CARGO_TARGET_DIR`), the new test file run, the catching
rows recorded:

| mutation | change | expected to be caught by |
|---|---|---|
| M1 | the response not expanded: the mesh part takes `Q` at the independent mesh unknowns, dependents left zero | RS1, RS2 |
| M2 | right-hand side `F` at the independent unknowns instead of `T^T F` | RS2 (the load on the dependent node `J`) |
| M3 | supports not added | K1, FD1, RY1, P2 |
| M4 | supports multiplied by the coefficient of the first stiffness part | K1a..c, FD1 |
| M5 | `K_sigma` omitted on the plain path | P1, P3 |
| M6 | `K_sigma` with the opposite sign | P1, P3 |
| M7 | `beta` on `K_0` only (not `K_s`, `K_sigma`) | RY1, P3 |
| M8 | dashpots dropped on the transformation path | K1g, RS2 |
| M9 | the imaginary part of the right-hand side dropped on the transformation path | RS2, RY1, P2 |
| M10 | `K_sigma` omitted on the transformation path | P2 |

## Decision rule

* **done**: B0, K1, RS1, P1 (the acceptance's own checks) and every other registered check met, every control met, every mutation
  caught by at least one check.
* **partial**: the transformation path and the pre-stress built, but a check of c3, c4 or c5 missed (a `*_known_miss` record) or
  not run, or another registered check missed, or a mutation not caught.
* **todo**: the transformation path or the pre-stress not built, or B0 missed (a default changed).

## Limits (not covered)

* Base motion on the transformation path (a complex `g`), cyclic constraints (the harmonic transformation is per harmonic index),
  non-zero constraint values and the supernodal factor on the transformation path are refused (D4).
* The reference unknowns of the rigid sets are not returned; the constraint forces are not computed.
* Supports are undamped springs (a dashpot gives their damping); no loss factor on a support.
* A pre-stress beyond the buckling load is not detected; `K_sigma` uses the standard element rule (as the pre-stressed modes).
* Beams and shells stay refused by the model check of the direct solve, as before.
* No case-file key: the case file has no direct harmonic analysis.

## Amendment 1 (2026-11-15, written after the mutation runs and before the run of RY2)

The registered mutation runs (addendum 1) show that RS2, registered as the catcher of M1, M2, M8 and M9, cannot catch any
mutation of the transformation path: both of its sides (the split and the merged model) are solved on that path, so a mutation
changes both alike and the comparison stays equal. M1, M8 and M9 were caught by other rows (RS1, RY1, P2, K1g); M2 (right-hand
side `F` at the independent unknowns instead of `T^T F`) was caught by no row, because every load compared against an
independent reference (RY1, P2, K1) sits on independent unknowns. The set-up of the coverage is defective, not a limit; this
amendment adds one check and changes no registered limit or result:

| check | clause | set-up | limit | estimate |
|---|---|---|---|---|
| RY2a..b | c1 | RY1 with the load of RS2 (the tip load plus `(0, 0, 0.4)` real and `(0.1, 0, 0)` imaginary on the dependent junction node `J`), 0.5 and 1.7 `w_1`, direct against F-399 with every mode (`ModalDamping::Rayleigh`): `e` | `<= 1e-8` | 1e-11 |

RY2 is run once on the code of 1840f6ff, then M2 once more on the scratch copy against it. The registered M2 run stays the
record (not caught by the registered checks); the decision rule is applied to the registered checks as written.
