# ADR 0511: F-559 registration: Craig-Bampton residual vectors of load patterns given by the caller and a harmonic acceptance at resonance

Status: **REGISTERED 2026-11-14, written before any code of F-559 and before any run of its checks**. Not yet accepted by the
maintainer. Before this text nothing of F-559 was written in the repository. One probe program on the existing API of 6c4ba2d9
was run in the scratch folder (`w49i-work/f559-scratch/probe`, log `probe1.log`); it is disclosed in full under "Arithmetic before
this registration". No check of this registration was run. Results go in the addendum at the end; nothing above the addendum is
changed after a run.

Roadmap item F-559 (deps F-552), structural solver, advanced tier (plan [ADR 0310](0310-structural-solver-advanced-tier-plan.md)),
the follow-up proposed by [ADR 0474](0474-f552-registration-craig-bampton-residual-vectors-cutoff.md) (its remaining questions 1
to 3). Number 0511 given by the lead. Branch `f559-work` from main 6c4ba2d9, worktree `w49i-work/f559`, scratch
`w49i-work/f559-scratch`.

Fan relevance: a reduced blade model under an aerodynamic load pattern (the residual vectors of the load) for fast
forced-response sweeps.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the number recorded); each registered run made once; defaults bit-identical (the new
behaviour is opt-in); no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test
edited (the new tests are in new files); no new dependency, no unsafe code; clean-room. Every choice below that the maintainer
would normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation); none
changes a default result, a golden or a hash table. No case-file key is added (proposed under the open questions).

## What F-559 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. The reduction of ADR 0474 takes residual vectors of load patterns given by the caller (each M-orthogonalised against the kept modes and the vectors before it). The bar of ADR 0469 with the corner load and the cutoff of ADR 0474: the harmonic response within 1 percent of the full model off resonance (every point farther than 2 eta from a frequency of the full model) up to half the lowest omitted fixed-interface frequency, each frequency of the assembly below that bound within 1e-3 of the full model's, the first eight frequencies strictly lowered by the residual vectors, a point at the axial resonance within the error 4 delta / eta of its frequency error, and a pattern per load component reproducing the statics of each component within 1e-10; without the option every result bit-identical to ADR 0474's.

Clauses: **b1** the reduction takes residual vectors of load patterns given by the caller, each M-orthogonalised against the kept
modes and the vectors before it; **b2** the bar of ADR 0469 with the corner load and the cutoff of ADR 0474: the harmonic response
within 1 percent of the full model at every point farther than `2 eta` from a frequency of the full model, up to half the lowest
omitted fixed-interface frequency; **b3** each frequency of the assembly below that bound within 1e-3 of the full model's; **b4**
the first eight frequencies strictly lowered by the residual vectors; **b5** a point at the axial resonance within the error
`4 delta / eta` of its frequency error; **b6** a pattern per load component reproduces the statics of each component within 1e-10;
**b7** without the option every result bit-identical to ADR 0474's.

**Clauses b2 and b3 are predicted to miss** (arithmetic items 2 and 3, the probe): the residual vectors of the three components of
the corner load change the frequencies of the assembly by at most 2e-5 relative (the seventh) and the frequency errors of the modes
near the bound stay at 3.5e-3 to 5.0e-3, so b3 (1e-3) fails at three of the ten frequencies below the bound, and the response at
off-resonance points near those modes (the error there is of the order `delta / (2 eta)` at `2 eta` from a pole, about 0.1 for
`delta` 5e-3) fails b2 whatever the vectors. They are registered as written with the predicted numbers; the checks that the
reduction can be held to (b1, b4, b5, b6, b7) are registered next to them, and the band where 1 percent holds is characterised.

## What was read (main 6c4ba2d9, read only)

ADR 0474 in full (design, set-ups, arithmetic, checks, addendum: H1R and H1R5 missed at 0.494 and 1.04, MU5 not caught, the
remaining questions that this item takes); ADR 0469 as cited by it (the bar, the set-ups A, B, F, S, the error measures);
`crates/rcfd-fem/src/cms.rs` (`CmsOptions`, `reduce` with the residual vector of the model's load, `ReducedPart`, `expand`,
`to_text`/`from_text` with header version 2 for residual vectors, `Assembly`, `Assembly::harmonic`);
`crates/rcfd-fem/src/damped.rs` `m_orthonormalise` (classical Gram-Schmidt twice in the M inner product, drop rule
`RESIDUAL_DROP = 1e-8`, shared by ADR 0474); `crates/rcfd-fem/src/mpc.rs` (`Transformation::transpose_times`, `n_mesh`,
`n_unknowns`); `tests/sa_cms_residual.rs` and `tests/sa_cms_residual_default_bits.rs` (the set-ups, the `Bar` helper, the error
measures, the digest harness); the probe `w49i-work/f552-scratch/probe/src/bin/p2.rs` (the Ritz-projection form used here again).

Published formulation cited from memory, **not read**: R. R. Craig and A. J. Kurdila, Fundamentals of Structural Dynamics (2nd ed.,
Wiley 2006), residual flexibility in component mode synthesis; J. M. Dickens, J. M. Nakagawa and M. J. Wittbrodt, Computers and
Structures 62 (1997) 985 (modal truncation augmentation: the static response of the load pattern added to the modal basis). Nothing
numeric is taken from them; everything used is derived below from the Rayleigh-Ritz method and block elimination.

## Design

### Residual vectors of load patterns (derivation)

Notation of ADRs 0469 and 0474: a part's independent unknowns `q`, boundary `b` and interior `i`, `T` the transformation of
ADR 0296 (`x = T q`, zero constraint values), `Psi = -K_ii^{-1} K_ib`, `Phi` the kept fixed-interface modes (M_ii-normalised),
`R` the residual vectors (M_ii-orthonormal columns). A **load pattern** is a vector `p` over the part's mesh equations (the
length of `nodal_load_vector`), a load shape given by the caller (for example the aerodynamic load of a blade, a single component of
a force, one of the model's load entries applied alone). Its reduced form is `p_q = T^T p` (`Transformation::transpose_times`,
the vector padded with zeros over the reference unknowns), split `(p_b, p_i)`.

The static interior response of the pattern with the boundary held is `s = K_ii^{-1} p_i`. As in ADR 0474 its part not carried by
the kept modes is the residual `r = s - Phi Phi^T M_ii s`; with several patterns the vectors are built **in the order given**, each
by the loop of `damped::m_orthonormalise` (two classical Gram-Schmidt passes in the M_ii inner product, the M-normalisation, the
drop rule) against `[Phi, R_1 .. R_{j-1}]`, the kept modes and the vectors kept before it. A vector whose M-norm after the
orthogonalisation is at most `RESIDUAL_DROP = 1e-8` times that before (the pattern lies in the span of the basis built so far, or is
zero) is dropped. Where the model's own load vector is also asked (`residual_vectors(true)`), its vector is the first residual
vector, as in ADR 0474, and the patterns follow.

Statics: with `p` any pattern given (or a linear combination of those given), the exact solution of `K x = p` of the part lies in
the range of the transformation `[[I, 0, 0], [Psi, Phi, R]]` for the span of the kept modes and vectors, because `u_i = Psi u_b +
K_ii^{-1} p_i` and `K_ii^{-1} p_i = Phi Phi^T M_ii s + r` is in `span(Phi, R_j)` for a pattern whose vector was kept, and when it
was dropped it is in the span of the earlier vectors (the drop rule's meaning). The Ritz-Galerkin solution of a symmetric positive
definite problem is exact when the exact solution lies in the trial space, so the reduced statics of each pattern (and of a
combination) are exact to rounding. The reduced load of a pattern is `[p_b + Psi^T p_i; Phi^T p_i; R^T p_i]` (the block `R^T p_i` is
what a model load alone gets in ADR 0474).

### What the harmonic acceptance means (derivation)

**Off resonance**: the response of the reduced model differs from the full model's by the frequency and shape errors of the modes
that carry it. A mode `k` with relative frequency error `delta` contributes the factor `1 / (1 - r^2 + i eta)`, `r = omega /
omega_k`, and `1 / (1 - r^2 (1 + delta)^-2 + i eta)` in the reduced model; at distance `d = |r - 1|` from the pole the relative
change is about `2 delta / sqrt((2 d)^2 + eta^2)`. At `d = 2 eta` that is `delta / (2.2 eta)`, so 1 percent at `eta = 0.02` needs
`|delta| <= 4.5e-4` for every mode that has a pole within reach. The registered clause is the one written, on a grid and with the
exclusion `|omega / omega_k - 1| <= 2 eta` for the twenty frequencies of F computed.

**The axial resonance and `4 delta / eta`**: at `omega = omega_k` with `delta` the relative frequency error of the reduced model's mode `k` and `eta` the
loss factor, one mode dominating, `H_full = 1 / (i eta)` and `H_red = 1 / (2 delta + i eta)` to first order in `delta`, so the
relative error is `|H_red / H_full - 1| = (2 |delta| / eta) / sqrt(1 + (2 delta / eta)^2) <= 2 |delta| / eta` (ADR 0474 diagnosis,
measured 0.354 against 0.406 at the axial mode of the cutoff reduction). The clause allows twice that, `4 |delta| / eta`, for the
mode-shape error (the load participation of the reduced mode differs from the full model's by an amount of the order of the
frequency error, the same order again). What is compared: the relative error of the harmonic response over the whole bar (the
measure of ADR 0474, A and B recovered, the 2-norm of the complex difference over the 2-norm of F's response) at
`omega = omega_k^F`, the frequency of F that is nearest to the closed-form axial frequency of the clamped-free bar `omega_ax =
(pi / (2 L)) sqrt(E / rho)` (`L = 1`, `E = 2.1e11`, `rho = 7850`: 8124.5 rad/s; the nearest of the twenty is 8179.0, within
0.7 percent, and the check requires it within 2 percent), against `4 |delta| / eta` with `delta = omega_k^R / omega_k^F - 1` of
the same index `k` (the sorted frequencies of the assembly and of F). The point is at a resonance of F, so it is excluded from the
off-resonance clause by the same rule.

**Strictly lowered** (Rayleigh-Ritz monotonicity): enriching the trial space can not raise any eigenvalue of `K x = lambda M x`,
and the eigenvalues of the Ritz problem are above the exact ones (nested spaces). A frequency is *strictly lowered* when
`omega_with / omega_without - 1 < -1e-14` (about 45 ulp of 1; the rounding of the dense eigen-solution is of the order 1e-12 to
1e-11 relative, ADR 0474 RB, so the tolerance only discriminates because the probe shows what the vectors do: at least 2.2e-8
against the cutoff alone, a factor 2e6 above it); `without` is the reduction of the same parts with the same cutoff and no
residual vector (the same kept modes). The
tolerance is probed by a negative control: the reduction compared with itself shows no lowering (0 > -1e-14). A second
comparison, against the vector of the model's load alone (ADR 0474's option), checks that the patterns lower the frequencies
**further**.

### Options and API

`CmsOptions` gains `load_patterns: Vec<Vec<f64>>` (default empty; builder `load_pattern(p)` appends one, `load_patterns(v)` replaces
the list). Empty means every operation of `reduce` is that of ADR 0474, in the same order (b7). Each pattern must have the
length of the part's mesh equations (`FieldLength`, key `load pattern`, otherwise) and finite entries (`InvalidValue`, key
`load_patterns`, otherwise). A given pattern asks for its vector whether or not `residual_vectors` is on; a pattern whose interior
part is zero, or in the span of the basis built so far, adds no vector (dropped; the part is then that without it).

`ReducedPart::reduce_load(model, pattern) -> Result<Vec<f64>>`: the reduced load `[p_b + Psi^T p_i; Phi^T p_i; R^T p_i]` of any
load vector over the part's mesh equations (the model must be the part's, as in `expand`). For the model's own load vector it equals
`load()` (check HW2). `Assembly::load_from(&[Vec<f64>])` sums the reduced loads of the parts, one per part (empty: none), through the
part maps; `Assembly::harmonic_with_load(omega, loss, load)` is `harmonic` with the given assembly load, and `harmonic` calls it
with the assembly's own (the same operations, bit-identical, check HW1). The part files are unchanged: the residual vectors are in
the version 2 file of ADR 0474 (`R`), and `reduce_load` needs nothing else; a file written by ADR 0474 reads as before.

Changed library lines: `crates/rcfd-fem/src/cms.rs` only (no shared solver file; `damped.rs` is not touched, `m_orthonormalise` is
called as it is). `baseline_hashes` is run anyway.

## Set-ups

Those of ADRs 0469 and 0474 (steel, Hex8, `hex_box`, threads 1 unless said): the bar F with the corner load `[1, 1, 1]`, part A
`[0, 0.5]` clamped with the interface `xmax`, part B `[0.5, 1]` free with the interface `xmin` and the corner load, the small model S
(interfaces `xmax` and `mid`, gravity and the `xmax` traction). **The cutoff is 32964 rad/s** (ADR 0474): A keeps 5 modes, B 9, the
lowest omitted fixed-interface frequency is 33570.8 and the bound of b2 and b3 is `omega_half = 16785.4 rad/s` (the probe confirms).
**The patterns of the bar**: the three components of the corner force on B, each built as `nodal_load_vector` of a copy of B whose
only load is the force `[1,0,0]`, `[0,1,0]`, `[0,0,1]` at `corner` (the load of the full bar F in the same way for the references of
b6). "The option" is `CmsOptions::new(0).cutoff(32964).residual_vectors(true)` with the three patterns on B (and none on A): the
vector of the total load first (ADR 0474), then the patterns in the order x, y, z; the probe predicts three vectors kept in B (the
total, x and y; the z vector is in the span of the first three, ratio 1.5e-16, dropped). "The ADR 0474 option" is the same without
patterns. "Without any vector" is `CmsOptions::new(0).cutoff(32964)`.
**The patterns of S**: for the check ST-P4 the gravity load and the traction of S each as one pattern (`nodal_load_vector` of copies
of S with only that load), interfaces `xmax` and `mid`, `m = 0` and `m = 5`.

## Arithmetic before this registration (disclosed)

One probe (`f559-scratch/probe/src/main.rs`, log `f559-scratch/probe1.log`, run once on the API of 6c4ba2d9, the Ritz projection of
F onto the basis `V` of the constraint modes, the kept modes of the cutoff 32964 and the residual vectors of B built in the mass of F
as in the probe of ADR 0474, orthogonalised twice, drop at 1e-8; dense complex solve, `DirectHarmonic` of F with loss 0.02; 100
points `j omega_half / 100`; the full frequencies of F, 20 modes). No F-559 code existed. Items:

1. **Frequencies of F** (rad/s): 476.7, 572.6, 2921.4, 3460.0, 4926.4, 7945.5, 8179.0, 9236.8, 14830.3, 15026.7, 17101.5, 23915.7, ...;
   `omega_half = 16785.42`; ten frequencies below it. 72 of the 100 grid points are farther than `2 eta` (4 percent) from every
   frequency of F. The closed-form axial frequency 8124.5 is nearest to the seventh, 8179.0.
2. **Frequency errors `delta` of the assembly below `omega_half`** (cutoff only / with the ADR 0474 option / with the patterns):
   1.05e-6 / 1.05e-6 / 1.03e-6; 1.33e-6 / 1.33e-6 / 1.30e-6; 1.77e-4 / 1.77e-4 / 1.75e-4; 2.17e-4 / 2.17e-4 / 2.14e-4; 3.38e-4 /
   3.38e-4 / 3.33e-4; 6.86e-5 / 6.85e-5 / 6.73e-5; **4.07e-3 / 4.06e-3 / 4.05e-3 (axial, 8179.0)**; 6.11e-5 / 6.11e-5 / 5.99e-5;
   **3.52e-3 / 3.52e-3 / 3.47e-3** (14830.3); **5.11e-3 / 5.11e-3 / 5.03e-3** (15026.7). Three of ten above 1e-3 with the patterns.
3. **Off-resonance response error** (72 points): the worst 0.124 at 15778.3 (cutoff only), 0.106 (ADR 0474 option), **0.104 (with
   the patterns)**; the first off-resonance point above 1e-2 at 4699.9 (cutoff only), 6882.0 (both vector options). Over all
   100 points the worst 0.498 / 0.494 / 0.485.
4. **Strictly lowered**: the first eight frequencies with the patterns against cutoff only, relative: -2.2e-8, -2.5e-8, -2.8e-6,
   -3.1e-6, -4.9e-6, -1.3e-6, -2.0e-5, -1.1e-6; against the ADR 0474 option: -2.2e-8, -2.5e-8, -2.7e-6, -3.1e-6, -4.8e-6, -1.3e-6,
   -8.1e-6, -1.1e-6. The ADR 0474 option against cutoff only: -6.3e-10, -5.1e-11, -8.0e-8, -6.8e-9, -7.4e-8, -3.2e-8, -1.2e-5,
   -1.2e-9 (all lowered, the smallest -5.1e-11). The cutoff reduction against itself: 0 for all eight.
5. **Axial resonance**: error at 8179.0 with the patterns 0.353, `delta` 4.05e-3, `2 delta / eta` 0.405, `4 delta / eta` 0.810.
6. **Statics per component** (corner force x, y, z, against `solve_static` of F with only that load): 7.7e-12, 3.1e-12, 5.3e-12 with
   the patterns (and with the patterns alone); 2.9e-2, 2.5e-3, 1.8e-3 with the ADR 0474 option (the total vector only); 3.9e-2, 2.9e-3,
   2.0e-3 with the cutoff alone.
7. **Orthogonalisation**: M-norm after over before, B: total 1.4e-3, x 1.4e-2, y 2.6e-3, z 1.5e-16 (dropped); with the patterns
   alone 1.6e-2, 2.6e-3, 1.3e-3 (three kept).

## Checks (limits fixed now)

Tests in new files `crates/rcfd-fem/tests/sa_cms_pattern_default_bits.rs` (D4) and `crates/rcfd-fem/tests/sa_cms_pattern.rs` (the
rest), release profile. Error measures as ADRs 0469 and 0474 (vectors: largest absolute difference over the largest absolute
reference entry; harmonic: the 2-norm of the complex difference over the whole bar over the 2-norm of F's response; frequencies
`omega_R / omega_F - 1`). **Every aggregated error is required finite** (a NaN or an infinity in any term fails the check; no
`f64::max` over unchecked values, the test asserts `is_finite` on every term before it aggregates).

| Id | Clause | Check | Limit | Estimate |
|---|---|---|---|---|
| D4 | b7 | digests at 6c4ba2d9 (printed by `sa_cms_pattern_default_bits.rs`, written with the API of that commit and committed before any library line): the text of B and of S `mid` (`m = 5`) with the ADR 0474 option (header 2) and of B with `m = 5` and the vectors; the eigenvalues of the assembly A + B with the cutoff and the vectors; its harmonic response at 1000 and 5000 rad/s and its static `harmonic(0, 0)`; B expanded from the 1000 rad/s coordinates; `recover_static` of S; equal after the change. The 11 digests of ADR 0474 D2 (`sa_cms_residual_default_bits.rs`) also still pass | `==` | holds |
| D5 | b7 | `CmsOptions::new(5).load_patterns(vec![])` and `.cutoff(..).residual_vectors(true).load_patterns(vec![])` give parts `==` to those without, every float, same bytes | `==` | holds |
| HW1 | b7 | `Assembly::harmonic_with_load(w, loss, asm.load())` equals `harmonic(w, loss)` in bits at 0, 1000, 5000 | bits | holds |
| P1 | b1 | B with the option and the three patterns: `residual_vectors() == 3`; B with the three patterns and no `residual_vectors`: 3; B with the same pattern given twice (x, x): 1; B with a zero pattern: 0 and the part `==` the part without it | counts exact | 3, 3, 1, 0 |
| P2 | b1 | the vectors of B (option and patterns): the largest `abs` entry of the block `M_qR` and of `M_RR - I` (off-diagonals included: orthogonal to the earlier vectors) | `<= 1e-10` | 1e-16 (ADR 0474 O1) |
| P3 | b1 | the vector of each pattern recomputed in the test (the static response of the pattern on B's interior through `reduce` of the single-component copy, `static_interior()`, orthogonalised against `[Phi, earlier]` in the mass of B's interior, normalised): `R` against it, largest difference over the largest entry (B, patterns alone) | `<= 1e-8` | 1e-12 (ADR 0474 O2: 5e-12) |
| X4 | b1 | refused: a pattern of the wrong length (`FieldLength`, one entry too short and one too long), a pattern with NaN, with +inf (`InvalidValue`, key `load_patterns`) | refused | holds |
| ST-P | b6 | the bar with the option and the three patterns: for each component `c` the assembly load `load_from([[], B.reduce_load(model, e_c)])`, `harmonic_with_load(0, 0, load)`, both parts expanded, against `solve_static` of F with only component `c`, every equation | `<= 1e-10` each | 7.7e-12, 3.1e-12, 5.3e-12 (library about 1.5e-12) |
| ST-P2 | b6 | the same with the patterns alone (no `residual_vectors`) | `<= 1e-10` each | 7.7e-12, 3.1e-12, 5.3e-12 |
| ST-P3 | b6 | the same with `m = 5` modes and the patterns | `<= 1e-10` each | 1e-12 |
| ST-P4 | b6 | S, one part, interfaces `xmax` and `mid`, `m = 0` and `m = 5`, patterns gravity and traction (each as its own pattern): the static solution of the part under each load alone (`reduce_load`, `solve_static` of the condensed part plus the interior coordinates from the static system, expanded) against `solve_static` of S with only that load | `<= 1e-10` each | 5e-14 |
| ST-P5 | b6 | S `xmax`, `m = 5`, patterns, the subspace path (`dense_limit(0)`): the same statics | `<= 1e-10` each | 1e-13 |
| NC-P | b6 (negative control) | ST-P without the patterns (the ADR 0474 option, the component loads through `reduce_load`): the error of each component; and the same for S `mid` `m = 5` with the model's total load only | `>= 1e-6` each | 2.9e-2, 2.5e-3, 1.8e-3; S 5e-3 |
| HW2 | b6 | `reduce_load(model, nodal_load_vector(model))` against `load()` of B and of S (option, patterns): largest difference over the largest entry | `<= 1e-13` | 1e-16 |
| LF | b3 | the assembly's frequencies below `omega_half` (the ten indices with `omega_F < omega_half`): `abs(omega_R / omega_F - 1)`, the largest of the ten; also the count of the assembly's frequencies below `omega_half` equals ten | `<= 1e-3`; count 10 | **predicted miss** 5.0e-3 (three of ten above); count 10 |
| LF2 | b3 (band) | the seven frequencies of F below 9500 rad/s other than the axial one (indices 0 to 5 and 7): the same measure | `<= 1e-3` | 3.4e-4 |
| SL1 | b4 | the first eight frequencies of the assembly with the option and the patterns against the cutoff alone: each `omega_with / omega_without - 1` | `< -1e-14` each | -2.2e-8 at the least |
| SL2 | b4 | against the ADR 0474 option: each | `< -1e-14` each | -2.2e-8 at the least |
| SL3 | b4 | every frequency with the vectors not below that of F: `omega_with / omega_F - 1` over the first ten | `>= -1e-9` | 1e-6 and more |
| NC-S | b4 (negative control) | the cutoff-only assembly against itself: the first eight changes | not `< -1e-14` (0) | 0 |
| OFF1 | b2 | the response at the 72 (at least 60) points `j omega_half / 100`, `j = 1..100`, with `abs(omega / omega_F,k - 1) > 2 eta` for all twenty `omega_F,k`: the largest error; loss 0.02 in F and in the assembly, the corner load | `<= 1e-2` | **predicted miss** 0.104 at 15778.3 |
| OFF2 | b2 (band) | the same points below 4000 rad/s | `<= 1e-2` | below 1e-2 (first point above at 6882) |
| AX1 | b5 | the response error at the axial resonance of F (nearest to the closed form, within 2 percent) against `4 abs(delta) / eta` with `delta` of the same index | `error <= 4 abs(delta) / eta` | 0.353 against 0.810 |
| AX2 | b5 | the same error against the first-order bound `2 abs(delta) / eta` (derivation above), a record of the tighter reading | `<= 2 abs(delta) / eta` | 0.353 against 0.405 (a record: not in the decision) |
| AX3 | b5 (negative control) | the axial `delta` of the cutoff-only reduction is above 1e-3 and the point is not within 1e-2 (the clause is not vacuous: the error is of the size of the bound) | `delta >= 1e-3`; error `>= 1e-1` | 4.07e-3; 0.354 |
| FP | design | B with the option and three patterns written, read: `==`, every float in bits, header 2, `reduce_load` of the read part bit-equal to that of the written one, a file of ADR 0474 (without patterns) written and read unchanged | exact | holds |
| TH | design | B with the option and the patterns, threads 1 and 4: `K_r`, `M_r`, `R` and `reduce_load` bit-identical | bits | holds |
| SP3 | design | B with the option and the patterns: `relative_asymmetry` of `K_r`, `M_r` after symmetrisation, and `asymmetry()` before it; `lambda_min / lambda_max` of `M_r` | `<= 1e-13`; `<= 1e-10`; `>= 1e-12` | 0; 1e-15; positive |

Records (reported, not in the decision rule): AX2; the frequency errors of the assembly below `omega_half` with the cutoff alone, the
ADR 0474 option and the patterns; the off-resonance error curve at every point and the first point above 1e-2; the after/before
M-norm ratios of each pattern; `|K_qR|` over the largest `K_r` entry; the response errors at all 100 points including the excluded.

## Negative controls and mutations

Negative controls: NC-P (the statics the patterns remove), NC-S (the strictness measure sees nothing when nothing changes), AX3 (the
axial point is not trivial), X4 (refusals), P1 (drops). Mutations (on a scratch copy of the final tree; each must be caught by at
least one registered check):

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | a pattern's vector not orthogonalised against the vectors before it (only against the modes) | P1 (the duplicate pattern kept), P2 |
| MU2 | `reduce_load` without the term `R^T p_i` | ST-P, ST-P2, HW2 |
| MU3 | every pattern's vector taken from the model's load (`static_interior`) instead of the pattern's | ST-P, ST-P2, P3 |
| MU4 | the patterns used only when `residual_vectors` is on | P1 (patterns alone: 3 expected), ST-P2 |
| MU5 | the drop rule removed for patterns (a vector always kept) | P1 (zero pattern, duplicate) |
| MU6 | `harmonic_with_load` ignoring its load argument (the assembly's own) | ST-P, ST-P2, ST-P3 |
| MU7 | the coupling `M_bR` set to zero for the pattern vectors | SL1 or SL2 (the seventh frequency falls by 3e-3 of ADR 0474's number with the coupling; the check is the one that item's MU5 needed) |
| MU8 | the pattern length check removed | X4 |
| MU9 | the finite check removed | X4 |
| MU10 | `Assembly::load_from` adds part 1's load at part 0's map | ST-P |
| MU11 | the strictness comparison made `<= 1e-14` instead of `< -1e-14` (sign) | NC-S (0 passes), SL1 |

## Decision rule

**done**: every check met at its registered run, every mutation caught, no amendment after a run. **partial**: a check missed
(recorded as an ignored `*_known_miss` test with its limit and the measured number), a mutation not caught by a registered check, or
a check met only after an amendment made after its run. **todo**: the option does not build or the bar runs cannot be made. LF
(clause b3) and OFF1 (clause b2) are predicted to miss (arithmetic items 2 and 3); if they do, the item is partial by this rule, with
the cause diagnosed in the addendum. A check that misses is written as an ignored test named `*_known_miss` from the start (the
miss is predicted) and run once with `--ignored`; the number is recorded in the addendum.

## Limits (not covered)

- Patterns are given per part as vectors over the part's mesh equations; a pattern given as a harmonic or complex load, a pattern
  depending on frequency, and a pattern as the name of a load entry of the model are not included. No case-file key.
- Residual vectors of interface loads are not needed (the boundary is in the reduced unknowns).
- The cutoff is one frequency for every part. The bound of b2 and b3 is the clause's, not a property the method guarantees.
- Everything not covered by ADRs 0469 and 0474 stays not covered (free-interface methods, damping other than a uniform loss factor
  in the assembly, pre-stress, cyclic constraints, non-zero constraint values, reduced parts in an ordinary model: F-553).

## Addendum 1: results (2026-11-14)

Commits on `f559-work` from 6c4ba2d9: d4b69ee2 (this registration, alone), c7ed28a8 (the D4 digests printed at 6c4ba2d9 by
`sa_cms_pattern_default_bits.rs`, before any library line), then the options in `cms.rs` with the checks `tests/sa_cms_pattern.rs`
(the registered run), then `cargo fmt` of the F-559 lines (layout only). Each registered check was run once, release profile
(`cargo test --release -p rcfd-fem --test sa_cms_pattern -- --test-threads 1`, log `f559-scratch/run1.log`; the two ignored
known misses once with `--ignored`, log `run1-ignored.log`). No amendment was made.

Library lines changed: `crates/rcfd-fem/src/cms.rs` only: `CmsOptions::load_patterns` with the builders `load_pattern` and
`load_patterns`, the validation and the pattern vectors in `reduce` (the vector loop generalised from one candidate to a list;
the first candidate sees the same basis and operations as before), `ReducedPart::reduce_load`, `Assembly::load_from`,
`Assembly::harmonic_with_load` (`harmonic` calls it with its own load). `damped.rs` and every shared solver file untouched.

| Id | Clause | Limit | Measured | Met |
|---|---|---|---|---|
| D4 | b7 | `==` | 10 of 10 digests of 6c4ba2d9 equal after the change; the 11 digests of ADR 0474 D2 also equal | yes |
| D5 | b7 | `==` | `load_patterns(vec![])` equal, same bytes, for `m = 5` and the ADR 0474 option | yes |
| HW1 | b7 | bits | equal at 0, 1000, 5000 | yes |
| P1 | b1 | counts 3, 3, 1, 0 | 3, 3, 1, 0 (the zero pattern gives the part equal to that without) | yes |
| P2 | b1 | `<= 1e-10` | `M_qR` 1.2e-16 (option) and 9.9e-17 (alone); `M_RR - I` 3.3e-16 and 4.4e-16 | yes |
| P3 | b1 | `<= 1e-8` | 6.3e-15 | yes |
| X4 | b1 | refused | one entry short and long: `FieldLength`; NaN, +inf, -inf: `InvalidValue` key `load_patterns` | yes |
| ST-P | b6 | `<= 1e-10` | 3.3e-12, 4.4e-13, 1.5e-12 (components x, y, z) | yes |
| ST-P2 | b6 | `<= 1e-10` | 3.3e-12, 4.4e-13, 1.5e-12 | yes |
| ST-P3 | b6 | `<= 1e-10` | 3.3e-12, 4.4e-13, 1.5e-12 | yes |
| ST-P4 | b6 | `<= 1e-10` | S: 9.5e-14 and 5.8e-14 (`xmax`, gravity and traction, both `m`), 2.4e-14 and 3.3e-14 (`mid`, `m = 0`), 1.5e-14 and 1.3e-14 (`mid`, `m = 5`) | yes |
| ST-P5 | b6 | `<= 1e-10` | 9.5e-14, 5.8e-14 (subspace path) | yes |
| NC-P | b6 control | `>= 1e-6` | bar 2.9e-2, 2.5e-3, 1.8e-3; S `mid` `m = 5` gravity 4.3e-3, traction 1.7e-5 | yes |
| HW2 | b6 | `<= 1e-13` | 0 (B and S: bit-equal to `load()`) | yes |
| LF | b3 | `<= 1e-3`; count 10 | **5.03e-3** (15026.7 rad/s); count 10 | **no** (predicted); count yes |
| LF2 | b3 band | `<= 1e-3` | 3.33e-4 | yes |
| SL1 | b4 | `< -1e-14` each | the least lowering -2.22e-8 (f1), -2.50e-8 (f2), then -2.8e-6 to -2.0e-5 | yes |
| SL2 | b4 | `< -1e-14` each | -2.16e-8 (f1), -2.49e-8 (f2), -2.7e-6, -3.1e-6, -4.8e-6, -1.3e-6, -8.1e-6 (f7), -1.1e-6 | yes |
| SL3 | b4 | `>= -1e-9` | 1.03e-6 (f1) | yes |
| NC-S | b4 control | not `< -1e-14` | 0 | yes |
| OFF1 | b2 | `<= 1e-2`, at least 60 points | **0.104** at 15778.3 rad/s (72 points; first above 1e-2 at 6882.0) | **no** (predicted); points yes |
| OFF2 | b2 band | `<= 1e-2` | 3.19e-3 (19 points below 4000 rad/s) | yes |
| AX1 | b5 | `error <= 4 abs(delta) / eta` | error 0.353 at 8179.0 rad/s (#7, closed form 8124.5), `delta` 4.052e-3, bound 0.810 (ratio 0.436) | yes |
| AX2 | b5 record | `<= 2 abs(delta) / eta` | ratio 0.871 against 0.405 (met) | yes (record) |
| AX3 | b5 control | `delta >= 1e-3`; error `>= 1e-1` | cutoff alone: 4.07e-3; 0.354 | yes |
| FP | design | exact | header 2, equal and bits, `reduce_load` bit-equal after reading; a part of ADR 0474 unchanged | yes |
| TH | design | bits | threads 1 and 4 bit-identical (`K_r`, `M_r`, load, `Psi`, `Phi`, `R`, `reduce_load`) | yes |
| SP3 | design | limits | 0, 0; 1.3e-15; 1.25e-6 | yes |

Records: the first off-resonance point above 1e-2 is 6882.0 rad/s with the patterns and with the ADR 0474 option (largest
0.104 and 0.106), 4699.9 with the cutoff alone (0.124); the frequency errors `delta` below the bound with the patterns
1.03e-6, 1.30e-6, 1.75e-4, 2.14e-4, 3.33e-4, 6.73e-5, 4.05e-3 (axial), 6.0e-5, 3.47e-3, 5.03e-3, equal to the probe to three
digits; `|K_qR|` over the largest `K_r` entry 4.9e-16; the library gives the statics 2 to 7 times lower than the probe's
Ritz form (the library writes the exact zeros of `K_bR`).

Mutations (scratch copy `f559-scratch/mut`, a git archive of the final tree, script `mutate.py`, logs `mutlogs/mut-MU*.log`):

| Id | Caught by |
|---|---|
| MU1 | P1 (four vectors, the duplicate kept twice), P2 (`M_RR - I` 0.52), P3, SP3, ST-P |
| MU2 | ST-P, ST-P2, ST-P3, ST-P4, ST-P5, HW2 (1.0 and 0.42) |
| MU3 | P1, P3, SL2, ST-P, ST-P4 |
| MU4 | P1 (patterns alone: 0), ST-P2, ST-P4, ST-P5 |
| MU5 | P1 (zero and duplicate kept), P2, SP3, ST-P, ST-P4 |
| MU6 | ST-P, ST-P2, ST-P3, ST-P4, ST-P5 |
| MU7 | SL1, SL2 (with the coupling zero all eight frequencies equal those of the cutoff alone, change 0: the strictness check that ADR 0474's MU5 needed) |
| MU8 | X4 |
| MU9 | X4 |
| MU10 | ST-P, ST-P2, ST-P3 |
| MU11 | NC-S (and SL1, SL2 become vacuous) |

Every mutation was caught by at least one registered check, each by the checks it was expected to be caught by.

`cargo fmt -p rcfd-fem --check`: clean. `cargo clippy -p rcfd-fem --all-targets -- -D warnings`: clean. `cargo test -p rcfd-fem --release`: every test binary passed (789 passed, 0 failed, 613 ignored by the summed result lines of 178 binaries, doc-tests included; log `fem-all.log`). `cargo test -p rcfd-api --release --test baseline_hashes` (3 passed, 1 ignored) and `cargo test -p rcfd-cli --release --test docs_site` (2 passed) run.

Not run: the workspace test suite (the lead's gate).

### Deviations

1. **LF (b3) and OFF1 (b2) missed, as predicted** (arithmetic items 2 and 3, which also predicted the numbers: 5.0e-3 and 0.104).
   Diagnosis with the measured numbers: the residual vectors of three patterns do what they are for (statics of each component
   exact to 3e-12, every one of the first eight frequencies lowered by at least 2.2e-8, the axial resonance within the bound), but
   they lower the frequency errors of the modes near the bound by only 1 to 2 percent (5.11e-3 to 5.03e-3 at 15026.7 rad/s). The
   error of a fixed-interface reduction at a mode comes from the omitted fixed-interface modes and the shape of the mode at the
   interface, not from the load's static correction, so a static residual vector of the load gives little there. The first
   off-resonance point above 1 percent is 6882 rad/s, the response error at the nearest poles `2 eta` away is about `delta / (2.2
   eta)`: 0.1 for `delta` 5e-3, as derived. The probes of ADR 0474 (cutoffs 2e4 to 8e4 rad/s) found no cutoff that gives `delta`
   near 1e-3 at the modes near half the lowest omitted frequency, so the cure is not more modes. A different enrichment of the
   interface is what is left (the constraint modes' own residual flexibility, Krylov vectors of the stiffness-mass pair); this
   reading of the cause is not tested here, the numbers above are.
2. The registered statics (7.7e-12, 3.1e-12, 5.3e-12 in the probe's Ritz form) came out lower in the library (3.3e-12, 4.4e-13,
   1.5e-12), as in ADR 0474.

### Decision

**partial** by the registered rule: LF (clause b3) and OFF1 (clause b2) missed at their registered run (predicted, diagnosed
above) and kept as the ignored tests `lf_each_frequency_below_the_bound_within_1e3_known_miss` and
`off1_off_resonance_to_half_the_lowest_omitted_known_miss` with their limits intact. Clauses b1, b4, b5, b6 and b7 are met, every
mutation was caught, and no amendment was made.

### Remaining questions

1. The off-resonance and frequency clauses (b2, b3). Options: (a) restate them for this reduction as "within 1 percent off
   resonance up to 4000 rad/s (the band that holds, 3.2e-3 measured) and the frequencies below 9500 rad/s other than the axial one
   within 1e-3 (3.3e-4)", with the bound at the highest frequency that the kept fixed-interface modes carry; (b) keep them and ask
   for a reduction whose interface accuracy improves near the bound (a follow-up: residual flexibility of the constraint modes,
   or Krylov enrichment of the stiffness-mass pair at the interface); (c) restate them with the error of the clause b5,
   `max(1e-2, 4 abs(delta) / eta)` at every point, which the reduction meets near resonances and not off them. Recommendation: (a)
   now, (b) only if a product use case needs 1e-3 frequencies up to half the lowest omitted fixed-interface frequency.
2. A case-file key. None is added. Proposal: `"substructure": {"parts": [{"model": ..., "interface": ..., "cutoff": ...,
   "load_patterns": ["load name", ...]}]}` where a pattern is named by an entry of the part's `loads` list applied alone (the
   single-load vector then comes from `SolidModel::loads`, a shared file, so it needs a separate item). Recommendation: wait
   for F-553 (reduced parts in an ordinary model), which needs the key anyway, and add the pattern list there.
3. Harmonic (complex) load patterns and frequency-dependent patterns (the aerodynamic load pattern of a blade rotates with the
   forcing order): a pattern is a real vector here, and the residual vector of a complex pattern is a pair of real vectors, which the
   caller can give as two patterns. Recommendation: no change; document the two-pattern form in the topic documentation if the
   fan blade case needs it.
4. Pattern vectors across parts: each part takes its own patterns (a pattern is a vector over that part's mesh equations). A
   pattern given as one vector over an assembled model would need the mapping of F-553. Recommendation: with F-553.

### For the lead: roadmap entries

Proposed status of F-559: **partial**. New note:

> Structural solver, advanced tier. Follow-up of F-552 (ADR 0474, 2026-11-14). PARTIAL. ADR 0511, 2026-11-14: CmsOptions::load_pattern gives vectors over a part's mesh equations, each adding its static residual vector K_ii^-1 p_i, M-orthonormalised twice against the kept modes and the vectors before it (dropped when in their span; zero and repeated patterns add nothing), after the vector of the model's load when that is asked; ReducedPart::reduce_load reduces any load vector to the part, Assembly::load_from and Assembly::harmonic_with_load apply it. Off by default and bit-identical when off (10 digests of the ADR 0474 option paths at 6c4ba2d9, the 11 of ADR 0474 D2, harmonic equal to harmonic_with_load of its own load). A pattern per corner-force component reproduces the statics of each component within 3.3e-12 (bar) and 9.5e-14 (small model, dense and subspace paths) against 2.9e-2 without; the first eight frequencies are lowered by at least 2.2e-8 (against the cutoff alone and against the ADR 0474 option); the response at the axial resonance (8179 rad/s) is off by 0.353 against the bound 4 delta / eta = 0.81 (delta 4.05e-3); orthogonality 4e-16. Two clauses missed as predicted: each frequency below half the lowest omitted fixed-interface frequency within 1e-3 (5.0e-3 at 15027 rad/s; three of ten above) and the response within 1 percent off resonance up to that bound (0.104 at 15778 rad/s; 1 percent holds to 6882 rad/s, 3.2e-3 to 4000); static residual vectors of the load do not change the interface accuracy near the bound. Eleven mutations all caught. Follow-up: a restated harmonic clause and an interface enrichment.

Follow-up item (number left to the lead), only if the lead wants the reading above taken further. Title: "Craig-Bampton interface enrichment for frequency accuracy near half the lowest omitted fixed-interface frequency", deps F-559.

> Every limit not stated here is registered in the ADR before the run. An option of the reduction of ADR 0474 adds to each part vectors that enrich the interface (the residual flexibility of the constraint modes, or Krylov vectors of the stiffness-mass pair started from the constraint modes), each M-orthogonalised against the kept modes and the vectors before it. The bar of ADR 0469 with the corner load and the cutoff of ADR 0474: each frequency of the assembly below half the lowest omitted fixed-interface frequency within 1e-3 of the full model's, the harmonic response within 1 percent of the full model at every point farther than 2 eta from a frequency of the full model up to that bound, the frequencies strictly lowered against the cutoff alone, and the statics of the corner load exact within 1e-10; without the option every result bit-identical to ADR 0511's.
