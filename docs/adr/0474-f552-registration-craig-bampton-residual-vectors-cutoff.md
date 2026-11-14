# ADR 0474: F-552 registration: Craig-Bampton with residual vectors of the interior loads and kept modes chosen by a cutoff

Status: **REGISTERED 2026-11-14, written before any code of F-552 and before any run of its checks**. Not yet accepted by the
maintainer. Before this text nothing of F-552 was written in the repository. Three probe programs on the existing API of
4abff5fe were run in the scratch folder (`w49i-work/f552-scratch/probe`, logs `probe1.log`, `probe2.log`, `probe3.log`); they
are disclosed in full under "Arithmetic before this registration". No check of this registration was run. Results go in the
addendum at the end; nothing above the addendum is changed after a run.

Roadmap item F-552 (deps F-430), structural solver, advanced tier (plan [ADR 0310](0310-structural-solver-advanced-tier-plan.md)),
the follow-up proposed by [ADR 0469](0469-f430-registration-substructuring-craig-bampton.md) (its remaining question 1). Number
0474 given by the lead. Branch `f552-work` from main 4abff5fe, worktree `w49i-work/f552`, scratch `w49i-work/f552-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical (the new behaviour is
opt-in); no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test edited (the
new tests are in new files); no new dependency, no unsafe code; clean-room. Every choice below that the maintainer would
normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation); none changes
a default result, a golden or a hash table.

## What F-552 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. An option of the reduction of ADR 0469 adds to each part the static residual vectors of its interior loads (M-orthogonalised against the kept modes) and keeps every fixed-interface mode below a given cutoff. The bar of ADR 0469 with the corner load: the harmonic response within 1 percent of the full model up to half the lowest omitted fixed-interface frequency (the H1 set-up), and the statics of an interior load exact within 1e-10; without the option every result bit-identical to ADR 0469's.

Clauses: **a1** the option adds to each part the static residual vectors of its interior loads, M-orthogonalised against the
kept modes; **a2** the option keeps every fixed-interface mode below a given cutoff; **a3** the bar of ADR 0469 with the corner
load: the harmonic response within 1 percent of the full model up to half the lowest omitted fixed-interface frequency (the H1
set-up); **a4** the statics of an interior load exact within 1e-10; **a5** without the option every result bit-identical to
ADR 0469's.

**Clause a3 is predicted to miss** on the H1 set-up with any cutoff (arithmetic items 2 and 3): the residual vectors make the
statics exact and lower the off-resonance error, but at a resonance of the full model the response error is `2 |delta| / eta`
(`delta` the relative frequency error of the assembly's mode, `eta = 0.02` the loss factor), so 1 percent needs `|delta| <=
1e-4` at every resonance below half the lowest omitted fixed-interface frequency; the fixed-interface reduction gives
`delta` of 4e-3 to 5e-3 for the modes near that bound, whatever the cutoff (probed from 2e4 to 8e4 rad/s). The checks register
the clause as written with its predicted number, and characterise the band where 1 percent holds.

## What was read (main 4abff5fe, read only)

ADR 0469 in full (design, set-ups, arithmetic, checks, addendum: H1 missed at 0.997, 8241 rad/s; the diagnosis and the follow-up
text that is this item); `crates/rcfd-fem/src/cms.rs` (ADR 0469: `reduce`, `CmsOptions`, `ReducedPart`, the file, `Assembly`);
`crates/rcfd-fem/tests/sa_cms.rs` and `sa_cms_default_bits.rs` (the set-ups S, F, A, B, R and the H1/H2 error measure); ADR
0464 (F-532: static residual vectors of the dashpots, classical Gram-Schmidt twice in the M inner product, the drop rule
`RESIDUAL_DROP = 1e-8`) and `crates/rcfd-fem/src/damped.rs` (`residual_vectors`, `m_norm`, `RESIDUAL_DROP`);
`crates/rcfd-fem/tests/sa_damped_residual.rs` (the tip-dashpot bar and its `bits` list); `rcfd_linalg::eigen`
(`generalized_eigen_dense`, `lowest_eigenpairs`); `loads.rs` (`nodal_load_vector`), `model.rs` (`SolidModel::loads`, private
`loads` field); `tests/sa_scaffold.rs` (the X1 copies: `cms` is already "landed" in `x1_modules_and_pending_after_f430`, so this
item retires no pending record and does not touch that file); the roadmap entries F-430, F-552.

Published formulations, cited from memory, **not read** for this work: R. R. Craig and A. J. Kurdila, Fundamentals of Structural
Dynamics (2nd ed., Wiley 2006), the residual flexibility and the rule of thumb that the fixed-interface modes are kept up to
about 1.5 to 2 times the top frequency of interest; S. Rubin, Improved component-mode representation for structural dynamic
analysis, AIAA J. 13 (1975) 995, and R. H. MacNeal, A hybrid method of component mode synthesis, Computers and Structures 1
(1971) 581 (residual flexibility in component mode synthesis); J. M. Dickens, J. M. Nakagawa and M. J. Wittbrodt, A critique of
mode acceleration and modal truncation augmentation methods for modal response analysis, Computers and Structures 62 (1997)
985 (modal truncation augmentation, as ADR 0464). Nothing numeric is taken from them; everything used is derived below from the
Rayleigh-Ritz method and block elimination.

## Design

### The residual vector of a part (derivation)

The notation of ADR 0469: a part's independent unknowns `q`, partitioned into boundary `b` and interior `i`; `Psi = -K_ii^{-1}
K_ib`; `Phi` the `m` kept fixed-interface modes, M_ii-normalised; the interior load `f_i` (the interior rows of `f_q = T^T f`,
`f` the model's loads by `nodal_load_vector`). The part's static interior response to its interior load with the boundary held is
`s = K_ii^{-1} f_i` (already computed and stored by ADR 0469 as `static_interior`). Its modal expansion is `s = sum_k phi_k
(phi_k^T f_i) / omega_k^2` over every fixed-interface mode; the kept modes carry the first `m` terms, and

`r = s - Phi Phi^T M_ii s = sum_{k > m} phi_k (phi_k^T f_i) / omega_k^2`

is the **residual flexibility** of the interior load: the part of `s` that the kept modes miss (ADR 0469 diagnosis (b)). It is
`M_ii`-orthogonal to the kept modes by construction, and `K_ii`-orthogonal as well (`phi_k^T K_ii r = omega_k^2 phi_k^T M_ii r
= 0`). The Ritz basis of the interior becomes `[Phi, r / ||r||_M]`. The Craig-Bampton transformation of ADR 0469 gains one
column per residual vector:

`(u_b, u_i) = T (u_b, eta, rho)`, `T = [[I, 0, 0], [Psi, Phi, R]]`, `R = [r_1 .. r_p]` (interior rows, `M_ii`-orthonormal).

The reduced matrices of order `n_b + m + p` (Rayleigh-Ritz projection, `T^T K_q T`, `T^T M_q T`):

- `K_r`: the blocks of ADR 0469, and `K_qR = Phi^T K_ii R`, `K_RR = R^T K_ii R`; the coupling of the constraint modes with `R`
  is `(K_bi + Psi^T K_ii) R = 0` exactly (as `K_ib + K_ii Psi = 0`), written as zeros like the published form of ADR 0469.
  `K_qR` is computed (it is zero in exact arithmetic, `Lambda Phi^T M_ii R`; its rounding is kept, not hidden).
- `M_r`: the blocks of ADR 0469, and `M_bR = M_bi R + Psi^T (M_ii R)`, `M_qR = Phi^T M_ii R`, `M_RR = R^T M_ii R`.
- the reduced load gains `R^T f_i`.

`K_r` and `M_r` are symmetrised after the products as in ADR 0469 and the asymmetry before is reported in the same field.
Dynamic recovery: `u_i = Psi u_b + Phi eta + R rho`. The static recovery `recover_static` of ADR 0469 is exact already and
unchanged.

**Statics exact (a4)**: with an interior load, the exact solution of the full model is `u_i = Psi u_b + s` with `u_b` from
`K_hat u_b = f_hat` (ADR 0469, block elimination), and `s = Phi Lambda^{-1} Phi^T f_i + r` (the expansion above), so the exact
solution lies in the range of `T`; the Ritz-Galerkin solution of a symmetric positive definite problem is exact when the exact
solution lies in the trial space. Without the vector the interior part `r` is missing (ADR 0469 had it only through
`recover_static`, not through the reduced coordinates of an assembly).

**Which loads**: the residual vector of the part's model load, one vector per part (the interior part of the sum of the model's
loads; the corner force of the bar is one load). Separate vectors per load entry or per caller-given load pattern would need
the single-load vector of `SolidModel` (its `loads` field is private to `model.rs`, a shared file) and are left as an open
question (below). A part whose interior load is zero (part A of the bar) gains no vector.

**M-orthogonalisation (reuse of ADR 0464)**: the loop of `damped.rs::residual_vectors` (classical Gram-Schmidt in the M inner
product against the basis, made twice, then the M-norm, the drop rule `after <= RESIDUAL_DROP * own` with `RESIDUAL_DROP = 1e-8`,
and the M-normalisation) is moved, unchanged in its operations and their order, into a crate-internal function
`damped::m_orthonormalise(mass, r, basis, y) -> bool` that `damped.rs` and `cms.rs` both call. `damped.rs` keeps the same bits
(check D2 records its residual path at 4abff5fe). In `cms.rs` the mass is `M_ii` (CSR) and the basis the kept modes `Phi`
(then the residual vectors kept so far). A vector dropped (in the span of the kept modes: for example every mode kept) adds
nothing and the part is that of the option off (check RV3).

### The cutoff (a2)

`CmsOptions::cutoff(omega_c)`: every fixed-interface mode with `omega_k^2 < omega_c^2` is kept (`omega_c` in rad/s); `modes`
must then be 0 (a mode count and a cutoff together are refused: "give a mode count or a cutoff"); `omega_c` must be finite and
positive (refused otherwise, `InvalidValue` key `cutoff`). On the dense path (`n_i <= dense_limit`) the eigen-solution has every
pair and the count is read from it; the first omitted eigenvalue is the next one (`None` when every mode is below the cutoff).
On the subspace path, `lowest_eigenpairs` for `k + 1` pairs, `k = 8` first and doubled until the `(k+1)`-th eigenvalue is at
least `omega_c^2` (the dense path is taken when `k + 1 >= n_i`); the kept ones are those below `omega_c^2`, the omitted one is
the next. The cutoff is not stored in the part (the kept eigenvalues and the omitted one are).

### Options and fields

`CmsOptions` gains `residual_vectors: bool` (default `false`, builder `residual_vectors(on)`) and `cutoff: Option<f64>` (default
`None`, builder `cutoff(omega)`). Both default to off: `reduce` then runs exactly the operations of ADR 0469 in the same order
(a5). `ReducedPart` gains the count `p` and `R` (`n_i x p`, row-major); accessors `residual_vectors()` (the count) and
`residual()`; `n_reduced()` is `n_b + m + p`; `modes()` stays the count of fixed-interface modes. `Assembly` maps `n_reduced -
n_b` interior coordinates of each part (modes, then residual vectors) instead of `modes` (the same number without the option).

### File (format change, registered)

A part without residual vectors is written exactly as by ADR 0469 (header `rcfd-cms-part 1`, the same sections; check D2 has
the bytes of such a file at 4abff5fe). A part with residual vectors is written with header `rcfd-cms-part 2`: section `sizes`
has four values (`n_mesh`, `n_q`, `modes`, `p`) and a section `residual` (`n_i p` floats as bits) follows `phi`. The reader
takes both: header 1 with three sizes and no `residual` section (every file of ADR 0469 reads as before, bit-exact), header 2
with four sizes and the `residual` section; anything else is refused (a version-1 body under header 2 is refused because
`sizes` has three values, so the ADR 0469 check F2 stays met).

No file other than `crates/rcfd-fem/src/cms.rs` and `crates/rcfd-fem/src/damped.rs` changes (no shared solver file of rcfd-fv
or rcfd-api; `baseline_hashes` is run anyway because `damped.rs` changes).

## Set-ups

Those of ADR 0469 (steel `E = 2.1e11`, `nu = 0.3`, `rho = 7850`, Hex8, `hex_box`, threads 1 unless said): the small model S
(interfaces `xmax` and `mid`, gravity and the `xmax` traction), the full bar F with the corner load, part A (`[0, 0.5]`,
clamped, interface `xmax`, no load), part B (`[0.5, 1]`, free, interface `xmin`, the corner load), all as in `sa_cms.rs`.

**The registered cutoff** `omega_c = 32964 rad/s` `= 4 x 8241`: four times the top of ADR 0469's H1 range (`omega_half =
8241.002`), so that half of any omitted frequency (at least `omega_c / 2 = 2 x 8241`) covers that range twice, above the rule
of thumb of 1.5 to 2 times the top frequency of interest (Craig and Kurdila, from memory). With it A keeps 5 modes (the next
33571 rad/s, A's first axial one, is omitted) and B 9 (its axial mode 16482 kept; the next 50095), so the lowest omitted is
33570.8 and the H1 range of this item ends at `omega_half = 16785.4` (probe 2). Disclosed: probe 1 ran the cutoffs 2e4, 2.5e4,
3e4, 3.5e4, 4e4, 5e4, 6e4 and 8e4 before this choice; clause a3 missed at every one of them (0.32 to 0.86), so the cutoff was not
chosen to pass or to miss anything; 32964 itself was run once, in probe 2, to give the estimates below.

"The option" in a check means `CmsOptions::new(0).cutoff(32964.0).residual_vectors(true)` on A and B; "m = 5 with the vectors"
means `CmsOptions::new(5).residual_vectors(true)` (the literal set-up of ADR 0469 H1 with the vectors added).

## Arithmetic before this registration (disclosed)

Run once each on the API of 4abff5fe in `f552-scratch/probe` (`src/main.rs` = probe 1, `src/bin/p2.rs` = probe 2, `src/bin/p3.rs`
= probe 3). The probes write the assembly of A and B as a Ritz projection of F onto the basis `V` of the constraint modes
(`psi()` of each part, joined at the interface), the kept modes (`phi()`) and, with the vectors, the residual vector of B
(`static_interior()` of B, M-orthogonalised twice against B's kept modes in the mass of F, which equals `M_ii` of B on vectors
supported on B's interior); `K = V^T K_F V`, `M = V^T M_F V` by full products, a dense complex solve, `V x` against
`DirectHarmonic` of F. A sanity line compares the probe at `m = 5` without vectors with `Assembly::eigen` of ADR 0469: the first
eight frequencies agree within 4.0e-12. No F-552 code existed.

1. **Fixed-interface frequencies** (rad/s, dense, every mode): A (`n_i = 189`): 10977, 12417, 19938, 27173, 29398, 33571, 40705,
   48611, 51107, 63141, ...; B (`n_i = 216`): 1894, 2260, 9882, 10944, 12481, 16482, 28146, 30050, 30874, 50095, ...; F (first
   20): 476.7, 572.6, 2921.4, 3460.0, 4926.4, 7945.5, 8179.0, 9236.8, 14830.3, 15026.7, 17101.5, 23915.7, ...
2. **The registered cutoff 32964** (probe 2), frequency errors `delta` of the assembly's modes below `omega_half = 16785.4`, without
   / with the vector (the vector changes them by at most 1e-6): 1.05e-6, 1.33e-6, 1.77e-4, 2.17e-4, 3.38e-4, 6.85e-5, 4.06e-3
   (axial), 6.11e-5, 3.52e-3, 5.11e-3. The response error at each resonance of F, with the vector: 1.06e-4, 1.34e-4, 1.77e-2,
   2.16e-2, 3.33e-2, 3.47e-2, 0.354, 8.30e-3, 0.279, 0.439, against `2 delta / eta` = 1.05e-4, 1.33e-4, 1.77e-2, 2.17e-2,
   3.38e-2, 6.85e-3, 0.406, 6.11e-3, 0.352, 0.511 (the measure of the diagnosis; 7945.5 sits next to the axial resonance and
   takes its error). **H1 with the option: largest 0.494 at 15106.9 rad/s** (without the vector 0.498); first point above 1e-2
   at 2921.4 (the resonance z2). Bands (the 50-point grid `j top / 50` and the frequencies of F below `top`; 30 points to 1000):
   with the vector, to 1000: 1.34e-4, to 2000: 5.39e-4, to 2800: 4.19e-3; without: 7.63e-4, 3.64e-3, 4.56e-3. Statics of the
   corner load (`omega = 0`, no loss, against `DirectHarmonic` of F at 0): 7.53e-12 with the vector, 1.39e-3 without.
   Orthogonality of the vector: largest `|phi^T M r|`, `|r^T M r - 1|` 1.1e-16; M-norm after the orthogonalisation over before
   1.40e-3.
3. **m = 5 with the vectors** (probe 2): frequency errors below 8241: 4.47e-6, 5.43e-6, 5.51e-4, 6.55e-4, 9.65e-4, 5.42e-4,
   8.07e-2 (axial, 9.99e-2 without); **H1: largest 1.038 at 8241.0** (0.997 without, ADR 0469); first above 1e-2 at 2801.9 (1977.8
   without). Bands with / without: to 1000: 5.46e-4 / 2.27e-3, to 2000: 1.66e-3 / 1.12e-2, to 2800: 1.25e-2 / 1.24e-2. Statics
   7.54e-12 / 2.07e-3. Orthogonality 3.3e-16; ratio after/before 9.07e-3.
4. **Probe 1, other cutoffs**, H1 largest with the vector (without): 2e4 and 2.5e4 (A 3, B 6 modes): 0.354 (0.355); 3e4: 0.518
   (0.507); 3.5e4 and 4e4: 0.491 (0.492); 5e4: 0.909 (0.865); 6e4: 0.324 (0.325); 8e4: 0.350 (0.393). The largest error sits at a
   resonance near `omega_half` each time, with `delta` of 4e-3 to 1e-2 there. With 6e4 and 8e4 and the vector, 1 percent holds
   up to the axial resonance 8179 (the first point above 1e-2).
5. **The small model S** (probe 2, one part, statics of gravity and the traction against `solve_static`): with the vector,
   `xmax` `m = 0` and `m = 5`: 3.67e-14; `mid` `m = 0`: 3.02e-14, `m = 5`: 2.98e-14; without: 1.14e-4, 2.40e-6, 0.132, 5.04e-3.
   With `m = 0` the ratio after/before is 1 (nothing to orthogonalise against).
6. **Rounding of the bar's statics** (probe 3): `solve_static` of F against `DirectHarmonic` at 0: equal; one ulp of `E`, with
   the `1/E` scaling removed: 4.57e-12 of the largest displacement. The probe's 7.5e-12 is of that size; the 1e-10 of a4 (the
   acceptance's own limit) is 13 times the probe and 22 times the one-ulp sensitivity. The 1e-10 limits of the orthogonality
   are 1e6 times their probes. No limit below is of the 1e-12 class except the exact symmetry (0 by construction) and the bit
   identities.

## Checks (limits fixed now)

Tests in new files `crates/rcfd-fem/tests/sa_cms_residual_default_bits.rs` (D2) and `crates/rcfd-fem/tests/sa_cms_residual.rs`
(the rest), release profile. Error measures as ADR 0469 (vectors: largest absolute difference over the largest absolute
reference entry; harmonic: the 2-norm of the complex difference over the whole bar, A and B recovered, over the 2-norm of F's
response; frequencies `f / f_full - 1`).

| Id | Clause | Check | Limit | Estimate |
|---|---|---|---|---|
| D2 | a5 | digests at 4abff5fe (printed by `sa_cms_residual_default_bits.rs`, written with the API of that commit and committed before any library line): the text of A and B (`m = 5`), of S condensed on `xmax` and of S on `mid` with `m = 5` (the bytes of `to_text`, which hold every field's bits); the eigenvalues of the assembly A + B (`m = 5`); its harmonic response at 1000 and 5000 rad/s (`re`, `im`); B expanded from the 1000 rad/s coordinates (`re`); `recover_static` of S `xmax` from its condensed solution; the ADR 0464 residual path of `damped_modes` (tip dashpot `c = 0.9 Z`, 20 modes, `residual_vectors(true)`, every field by the `bits` list of `sa_damped_residual.rs`); equal after the change | `==` | holds |
| D3 | a5 | the option asked but inert: A with `residual_vectors(true)` (no interior load) equals A without (`==`, every float) and writes the same bytes; `modes = 5` without a cutoff and without the vectors equals `CmsOptions::new(5)` | `==` | holds |
| O1 | a1 | B with the option and B with `m = 5` and the vectors: one vector kept (`residual_vectors() == 1`); the largest `abs` entry of the block `M_qR` of `M_r`, and `abs(M_RR - 1)` | `count 1`; `<= 1e-10` each | 1.1e-16, 3.3e-16 |
| O2 | a1 | the vector of B: `R` against `s - Phi Phi^T M_ii s` normalised, recomputed in the test from `static_interior()`, `phi()` and the mass of B's interior through the mass of F (as the probe): largest difference over the largest entry | `<= 1e-8` | 1e-13 (two Gram-Schmidt passes against one) |
| CO1 | a2 | A and B with the cutoff: `modes()` equals the count of the all-mode eigenvalues (`reduce` with `m = n_i`) below `omega_c^2` (5 and 9); the kept eigenvalues and the omitted one equal those all-mode eigenvalues in bits; the last kept below `omega_c^2`, the omitted one not below | exact | 5, 9; holds (the same dense eigen-solution) |
| C2 | a2 | S `xmax`, cutoffs 3e4 and 6e4 rad/s: the subspace path (`dense_limit(0)`) against the dense one: the same count, the kept and the omitted eigenvalues | count equal; `abs rel <= 1e-9` | 1e-12 (ADR 0469 C1: 7e-14) |
| X3 | a2 | refused (`InvalidValue`, key `cutoff`): cutoff 0, -1, NaN, +inf; refused: `CmsOptions::new(5).cutoff(3e4)` (both given) | refused | holds |
| ST5 | a4 | the bar, the option on A and B: `Assembly::harmonic(0, 0)`, both parts expanded, against `solve_static` of F, every equation | `<= 1e-10` | 7.5e-12 (one ulp 4.6e-12) |
| ST6 | a4 | the same with `m = 5` and the vectors | `<= 1e-10` | 7.5e-12 |
| ST7 | a4 | S, one part (`Assembly` of one), interfaces `xmax` and `mid`, `m = 0` and `m = 5`, with the vectors, the expanded static solution against `solve_static` of S | `<= 1e-10` each | 3.7e-14 |
| NC1 | a4 (negative control) | ST5 and ST7 (`mid`, `m = 5`) without the vectors (the cutoff kept for the bar) | `>= 1e-6` | 1.4e-3, 5.0e-3 |
| H1R | a3 | the H1 set-up with the option: loss factor 0.02 in F and in the assembly, the corner load, `omega_half = 0.5 min(sqrt(omitted_eigenvalue))` over A and B; 50 points `j omega_half / 50` and every frequency of F (20 modes) below `omega_half` | `<= 1e-2` at every point | **predicted miss**: 0.494 at 15106.9 (`omega_half` 16785.4) |
| H1R5 | a3 | the same with `m = 5` and the vectors (`omega_half` 8241.0) | `<= 1e-2` | **predicted miss**: 1.04 at 8241.0 |
| HB1 | a3 (band) | the option, 50 points to 2000 rad/s and the frequencies of F below 2000 (476.7, 572.6) | `<= 1e-2` | 5.4e-4 |
| HB2 | a3 (band) | `m = 5` with the vectors, the same points | `<= 1e-2` | 1.7e-3 (1.1e-2 without, recorded) |
| HR | a1, a3 | 30 points to 1000 rad/s and 476.7, 572.6: the largest error with the vectors over the largest without, for the cutoff and for `m = 5` | `<= 0.5` each | 0.18, 0.24 |
| RB | a1 | the first eight frequencies of the assembly with the option: not below F (`f / f_full - 1 >= -1e-9`) and not above those of the cutoff alone (`f_with / f_without - 1 <= 1e-9`, nested bases) | limits | holds (rounding 1e-12) |
| SP2 | a1 | B with the option: `relative_asymmetry` of `K_r`, `M_r`; `asymmetry()` before the symmetrisation; `lambda_min / lambda_max` of `M_r` | `<= 1e-13`; `<= 1e-10`; `>= 1e-12` | 0; 1e-15; positive |
| F3 | format | B with the option: the text starts with `rcfd-cms-part 2`; written, read: `==` and every float in bits (with `residual()`); written again byte-identical; the file cut in half refused; header 2 replaced by header 1 refused | exact / refused | holds |
| F4 | format | A (`m = 5`, no option) written: header `rcfd-cms-part 1` (with D2, the same bytes as at 4abff5fe) | exact | holds |
| RV3 | a1 (drop) | S `xmax`, `m = n_i = 81`, with the vectors: the vector dropped (`residual_vectors() == 0`) and the part `==` the part without the option | exact | holds (after/before about `1e-15`) |
| T2 | design | B with the option, threads 1 and 4: `K_r`, `M_r`, load, `Psi`, `Phi`, `R` bit-identical | bits | holds |

Records (reported, not in the decision rule): the H1R and H1R5 error curves (every point); the frequency errors of the assembly
below `omega_half` with and without the vectors and `2 delta / eta` against the resonance errors; the bands to 2800 rad/s; the
after/before ratios of the orthogonalisation; the asymmetries before symmetrisation; `|K_qR|` over the largest `K_r` entry.

## Negative controls and mutations

Negative controls: NC1 (the error the vectors remove), X3 (refusals), F3 (refused files), RV3 (the drop rule). Mutations (on a
scratch copy; each must be caught by at least one registered check):

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | the residual vector not M-orthogonalised (only normalised) | O1, O2 |
| MU2 | the reduced load of the vectors `R^T f_i` set to zero | ST5, ST6, ST7 |
| MU3 | `expand` without `R rho` | ST5, ST6, ST7 |
| MU4 | the cutoff compared with `lambda < omega_c` (not squared) | CO1 |
| MU5 | the coupling `M_bR` set to zero | RB, HB1, HR |
| MU6 | the drop rule removed (a vector always kept) | RV3 |
| MU7 | the version-2 file written without the `residual` section | F3 |
| MU8 | `Assembly` maps only `modes` interior coordinates of each part | ST5, ST6 |
| MU9 | `m_orthonormalise` with one Gram-Schmidt pass instead of two | D2 (the damped residual digest) |
| MU10 | the refusal of a mode count with a cutoff removed | X3 |

## Decision rule

**done**: every check met at its registered run, every mutation caught, no amendment after a run. **partial**: a check missed
(recorded as `*_known_miss` with its limit), a mutation not caught by a registered check, or a check met only after an amendment
made after its run. **todo**: the option does not build or the bar runs cannot be made. H1R and H1R5 (clause a3) are predicted
to miss (arithmetic items 2 to 4); if they do, the item is partial by this rule, with the cause diagnosed in the addendum.

## Limits (not covered)

- One residual vector per part, of the sum of its model's loads; separate vectors per load or per given pattern are not
  included (open question). Residual vectors of interface loads are not needed (the boundary is in the reduced unknowns).
- The cutoff is one frequency for every part; the bound `omega_half` of a3 is the clause's, not a property the method
  guarantees (the diagnosis above).
- Everything not covered by ADR 0469 stays not covered (free-interface methods, damping other than a uniform loss factor in the
  assembly, pre-stress, cyclic constraints, non-zero constraint values, reduced parts in an ordinary model: F-553).

## Addendum 1: results (2026-11-14)

Commits on `f552-work` from 4abff5fe: 0270649e (this registration, alone), a22284dc (the D2 digests printed at 4abff5fe by
`sa_cms_residual_default_bits.rs`, before any library line), 7204c96d (the options in `cms.rs`, the shared
`damped::m_orthonormalise`, the checks `tests/sa_cms_residual.rs`; then the registered run), fbccfae9 (H1R and H1R5 kept as
known misses, limits intact; `cargo fmt` of the F-552 lines, layout only). Each registered check was run once, release profile
(`cargo test --release -p rcfd-fem --test sa_cms_residual -- --test-threads 1`, log `f552-scratch/run1.log`; D2 with the
existing tests of `cms` and `damped`, log `run-d2-existing.log`). No amendment was made.

Library lines changed: `crates/rcfd-fem/src/cms.rs` (the module of ADR 0469: options, `modes_below`, the residual vector, its
blocks, accessors, `expand`, the file version 2, the `Assembly` map) and `crates/rcfd-fem/src/damped.rs` lines 254 to 297 (the
Gram-Schmidt loop of `residual_vectors` moved without change of operations into `m_orthonormalise`; D2 and the existing
`sa_damped_residual*` tests confirm the bits).

| Id | Clause | Limit | Measured | Met |
|---|---|---|---|---|
| D2 | a5 | `==` | 11 of 11 digests equal (and ADR 0469 D1 6 of 6) | yes |
| D3 | a5 | `==` | A with the vectors equal to A without, same bytes; explicit off equal to `new(5)` | yes |
| O1 | a1 | count 1; `<= 1e-10` | 1 and 1; `M_qR` 6.7e-17, 9.4e-17; `M_RR - 1` 1.1e-16, 3.3e-16 | yes |
| O2 | a1 | `<= 1e-8` | 5.2e-12, 3.5e-12 | yes |
| CO1 | a2 | exact | A 5 of 5, B 9 of 9; eigenvalues and omitted (33570.8, 50095.4 rad/s) equal in bits | yes |
| C2 | a2 | `<= 1e-9` | cutoff 3e4: 3 modes on both paths, 7.0e-14; 6e4: 7 modes, 7.0e-14 | yes |
| X3 | a2 | refused | 0, -1, NaN, inf, and a mode count with a cutoff, refused (key `cutoff`) | yes |
| ST5 | a4 | `<= 1e-10` | 1.52e-12 | yes |
| ST6 | a4 | `<= 1e-10` | 1.51e-12 | yes |
| ST7 | a4 | `<= 1e-10` | 5.7e-14, 5.7e-14, 3.2e-14, 1.3e-14 | yes |
| NC1 | a4 (control) | `>= 1e-6` | 1.39e-3 (bar), 5.04e-3 (S `mid`) | yes |
| H1R | a3 | `<= 1e-2` | **0.494** at 15106.9 rad/s (`omega_half` 16785.4); first above 1e-2 at the resonance 2921.4 (1.77e-2) | **no** (predicted) |
| H1R5 | a3 | `<= 1e-2` | **1.038** at 8241.0 (`omega_half` 8241.0); 0.935 at the axial resonance 8179.0; first above 1e-2 at 2801.9 | **no** (predicted) |
| HB1 | a3 (band) | `<= 1e-2` | 5.39e-4 | yes |
| HB2 | a3 (band) | `<= 1e-2` | 1.66e-3 (1.12e-2 without, record) | yes |
| HR | a1, a3 | `<= 0.5` | 0.175 (cutoff), 0.240 (`m = 5`) | yes |
| RB | a1 | `>= -1e-9`; `<= 1e-9` | smallest 1.05e-6; largest -5.1e-11 | yes |
| SP2 | a1 | `<= 1e-13`; `<= 1e-10`; `>= 1e-12` | 0, 0; 1.3e-15; 1.37e-6 | yes |
| F3 | format | exact / refused | header 2, bits and `==`, byte-identical, half file and header 1 refused | yes |
| F4 | format | exact | header 1 | yes |
| RV3 | a1 (drop) | exact | dropped; part equal | yes |
| T2 | design | bits | bit-identical | yes |

Records: `|K_qR|` over the largest `K_r` entry 2.2e-16 (cutoff) and 1.0e-15 (`m = 5`); `K_RR` 3.98e10 and 1.24e9 (the
vector's Rayleigh frequency about 2.0e5 and 3.5e4 rad/s); to 2800 rad/s the option 4.19e-3, the cutoff alone 4.56e-3; the first
eight frequencies with / without the vector (relative to F): 1.053e-6, 1.326e-6, 1.773e-4 / 1.774e-4, 2.167e-4, 3.376e-4,
6.853e-5 / 6.856e-5, 4.060e-3 / 4.072e-3 (axial), 6.107e-5. Every measured number equals the probe of the registration to
three digits except the statics (1.5e-12 against the probe's 7.5e-12: the library writes the exact zeros of `K_bR`, the probe
formed `V^T K V` by full products).

Mutations (scratch copy `f552-scratch/mut`, a git archive of fbccfae9, script `mutate.py`, logs `mutlogs/mut-MU*.log`,
summary `mutate.log`):

| Id | Caught by |
|---|---|
| MU1 | O1, O2, RV3 |
| MU2 | ST5, ST6, ST7, HB2, HR |
| MU3 | ST5, ST6, ST7, HB2, HR |
| MU4 | CO1, HB1, HR |
| MU5 | **not caught** (HB1 5.39e-4 to 5.62e-4, HR 0.175 and 0.240 to 0.175 and 0.242, RB largest -5.1e-11 to 0: every limit met) |
| MU6 | RV3 |
| MU7 | F3 (the reader refuses the file) |
| MU8 | ST5, HB1, RB (index out of range in `Assembly::new`) |
| MU9 | D2 (the digest of the ADR 0464 residual path) |
| MU10 | X3 |

`cargo fmt -p rcfd-fem --check`: clean. `cargo clippy -p rcfd-fem --all-targets -- -D warnings`: clean. `cargo test -p rcfd-fem
--release`: every test binary passed (547 passed, 0 failed, 117 ignored; log `fem-all.log`). `cargo test -p rcfd-api --release
--test baseline_hashes` (run because `damped.rs` changed) and `cargo test -p rcfd-cli --release --test docs_site`: 3 passed, 1 ignored; 2 passed.

### Deviations

1. **H1R and H1R5 missed, as predicted** (arithmetic items 2 and 3). Diagnosis with the measured numbers: the residual vector
   does what it is for (statics exact, 1.5e-12; the band to 1000 rad/s 5.7 and 4.2 times better; at `m = 5` 1 percent now
   holds to 2802 rad/s instead of 1978), but at a resonance of F the response error is `2 |delta| / eta`. At 2921.4 rad/s
   `delta` = 1.77e-4 gives the measured 1.77e-2; near `omega_half` the modes at 14830 and 15027 rad/s have `delta` 3.5e-3 and
   5.1e-3, giving 0.28 and 0.44 at their resonances. One percent at `eta = 0.02` needs `|delta| <= 1e-4` at every resonance
   below half the lowest omitted fixed-interface frequency; the fixed-interface reduction gives a `delta` of order 1e-3 near
   that bound whatever the cutoff (probe 1, 2e4 to 8e4 rad/s). The clause is not a property of the method at this loss factor.
2. **MU5 not caught.** With the mass coupling `M_bR` set to zero, the response changes by about `omega^2 / omega_R^2` (the
   vector's Rayleigh frequency `omega_R` is about 2.0e5 rad/s, so at most 1e-4 in the 2000 rad/s band), and most of the
   vector's small effect on the frequencies goes away, which brings them back to the cutoff-alone values. RB bounds them from
   one side only, so it is met. The registration expected RB, HB1 or HR to catch it; none does. Checks that would: the
   frequencies with the vector strictly below those without (the seventh falls by 3e-3 relative with the coupling), or a
   harmonic point at the axial resonance. Both are proposed for the follow-up below and were not added after the run.

Not run: the workspace test suite (the lead's gate).

### Decision

**partial** by the registered rule: H1R and H1R5 (clause a3) missed at their registered run (predicted, diagnosed above), and
MU5 not caught by a registered check. Every other check met at its registered run, no amendment.

### Remaining questions

1. The harmonic clause a3. Options: (a) restate it as "within 1 percent off resonance up to half the lowest omitted
   fixed-interface frequency, and each resonance frequency within 1e-3", which this reduction can be held to; (b) keep it and
   ask for a reduction that converges faster near the bound (a follow-up, for example Krylov enrichment of the interior load);
   (c) state it with the loss factor, within `max(1e-2, 4 |delta| / eta)`. Recommendation: (a) in the follow-up below, and (b)
   only if a product use case needs resonance accuracy of 1e-4 from a reduced model.
2. Per-load residual vectors. Options: (a) one vector per `Load` entry of the part's model (needs a crate-internal single-load
   vector in `model.rs` or `loads.rs`, shared files); (b) load patterns given by the caller in `CmsOptions`; (c) keep the
   vector of the total load. Taken now on the maintainer's standing instruction of 2026-11-11 (take the recommendation): (c).
   Recommendation for the future: (b) in the follow-up below (no shared file, covers harmonic loads unlike the static load).
3. MU5. Recommendation: the follow-up registers the strict decrease of the frequencies with the vector and a point at the
   axial resonance, which catch it.

### For the lead: roadmap entries

Proposed status of F-552: **partial**. New note:

> Structural solver, advanced tier. Follow-up of F-430 (ADR 0469). ADR 0474, 2026-11-14: options of rcfd_fem::cms, off by default and bit-identical when off (11 digests of 4abff5fe): CmsOptions::cutoff keeps every fixed-interface mode below a frequency (dense and subspace paths, counts and eigenvalues equal in bits to the full eigen-solution), CmsOptions::residual_vectors adds the static residual vector of each part's interior load, M-orthonormalised twice against the kept modes by the ADR 0464 routine now shared from damped.rs (orthogonality 1e-16, dropped when every mode is kept); part file version 2 only with residual vectors, version 1 files unchanged. Statics of an interior load exact within 1.5e-12 (bar) and 5.7e-14 (small model) against 1.4e-3 and 5.0e-3 without; the harmonic error to 2000 rad/s 5.4e-4, and 4 to 6 times lower to 1000 rad/s than without. H1 (1 percent up to half the lowest omitted frequency) missed as predicted: 0.494 at 15107 rad/s with the cutoff 32964 rad/s, 1.04 at 8241 with five modes; at a resonance the error is 2 delta / eta and the reduced frequencies near that bound are 3e-3 to 5e-3 high, so 1 percent at loss factor 0.02 needs frequency errors of 1e-4. One mutation (the boundary-to-vector mass coupling set to zero) not caught. Follow-up for given load patterns and a restated harmonic clause.

Follow-up item (number left to the lead). Title: "Craig-Bampton residual vectors of given load patterns and a harmonic acceptance at resonances", deps F-552.

> Every limit not stated here is registered in the ADR before the run. The reduction of ADR 0474 takes residual vectors of load patterns given by the caller (each M-orthogonalised against the kept modes and the vectors before it). The bar of ADR 0469 with the corner load and the cutoff of ADR 0474: the harmonic response within 1 percent of the full model off resonance (every point farther than 2 eta from a frequency of the full model) up to half the lowest omitted fixed-interface frequency, each frequency of the assembly below that bound within 1e-3 of the full model's, the first eight frequencies strictly lowered by the residual vectors, a point at the axial resonance within the error 4 delta / eta of its frequency error, and a pattern per load component reproducing the statics of each component within 1e-10; without the option every result bit-identical to ADR 0474's.
