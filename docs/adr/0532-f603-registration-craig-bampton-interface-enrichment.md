# ADR 0532: F-603 registration: Craig-Bampton interface enrichment by Krylov blocks started from the constraint modes

Status: **REGISTERED 2026-11-14, written before any code of F-603 and before any run of its checks**. Not yet accepted by the
maintainer. Before this text nothing of F-603 was written in the repository. Three runs of one probe program on the existing API
of 291a210c were made in the scratch folder (`w49i-work/f603-scratch/probe`, logs `probe1.log`, `probe2.log`, `probe3.log`); they
are disclosed in full under "Arithmetic before this registration". No check of this registration was run. Results go in the
addendum at the end; nothing above the addendum is changed after a run.

Roadmap item F-603 (deps F-559), structural solver, advanced tier (plan [ADR 0310](0310-structural-solver-advanced-tier-plan.md)),
the follow-up proposed by [ADR 0511](0511-f559-registration-craig-bampton-residual-vectors-of-load-patterns.md) (its deviation 1
and remaining question 1 (b): the LF and OFF1 misses). Number 0532 given by the lead. Branch `f603-work` from main 291a210c,
worktree `w49i-work/f603`, scratch `w49i-work/f603-scratch`.

Fan relevance: a reduced blade or hub model whose frequencies and forced response stay accurate up to half the lowest omitted
fixed-interface frequency, so a sweep through the blade-passing orders can use a small reduced model.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the number recorded); each registered run made once; defaults bit-identical (the new
behaviour is opt-in); no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test
edited (the new tests are in new files); no new dependency, no unsafe code; clean-room. Every choice below that the maintainer
would normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation); none
changes a default result, a golden or a hash table. No case-file key is added (an open question, below).

## What F-603 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. An option of the reduction of ADR 0474 adds to each part vectors that enrich the interface (the residual flexibility of the constraint modes, or Krylov vectors of the stiffness-mass pair started from the constraint modes), each M-orthogonalised against the kept modes and the vectors before it. The bar of ADR 0469 with the corner load and the cutoff of ADR 0474: each frequency of the assembly below half the lowest omitted fixed-interface frequency within 1e-3 of the full model's, the harmonic response within 1 percent of the full model at every point farther than 2 eta from a frequency of the full model up to that bound, the frequencies strictly lowered against the cutoff alone, and the statics of the corner load exact within 1e-10; without the option every result bit-identical to ADR 0511's.

Clauses: **c1** an option of the reduction of ADR 0474 adds to each part vectors that enrich the interface, each M-orthogonalised
against the kept modes and the vectors before it; **c2** the bar with the corner load and the cutoff of ADR 0474: each frequency of
the assembly below half the lowest omitted fixed-interface frequency (`omega_half`) within 1e-3 of the full model's; **c3** the
harmonic response within 1 percent of the full model at every point farther than `2 eta` from a frequency of the full model, up to
`omega_half`; **c4** the frequencies strictly lowered against the cutoff alone; **c5** the statics of the corner load exact within
1e-10; **c6** without the option every result bit-identical to ADR 0511's.

## What was read (main 291a210c, read only)

ADR 0511 in full (design, set-ups, probe, checks, addendum: LF missed at 5.03e-3, OFF1 at 0.104, the diagnosis that the static
residual vectors of the load do not change the interface accuracy near the bound); ADR 0474 (the header, the design sections and
the cited sources) and ADR 0469 (the section list and, through ADRs 0474 and 0511, the set-ups and error measures);
`crates/rcfd-fem/src/cms.rs` (`CmsOptions`, `reduce` with the candidate loop of ADR 0511, `ReducedPart`, `reduce_load`, `expand`,
the version 2 file, `Assembly`, `harmonic`, `harmonic_with_load`); `crates/rcfd-fem/src/damped.rs` `m_orthonormalise` (classical
Gram-Schmidt twice in the M inner product, `RESIDUAL_DROP = 1e-8`); `tests/sa_cms_pattern.rs` and
`tests/sa_cms_pattern_default_bits.rs` (the set-ups, the `Bar` helper, the `Checks` helper with the finite test, the digest
harness); the F-559 probe `w49i-work/f559-scratch/probe/src/main.rs` (the Ritz-projection form, copied for this probe).

Published formulations cited from memory, **not read**: R. R. Craig and A. J. Kurdila, Fundamentals of Structural Dynamics (2nd
ed., Wiley 2006), residual flexibility in component mode synthesis; S. Rubin, AIAA J. 13 (1975) 995 and R. H. MacNeal, Computers
and Structures 1 (1971) 581 (residual flexibility); the block Lanczos and Krylov-subspace model reduction literature in general
(the Krylov space of `K^{-1} M`), not any one paper. Nothing numeric is taken from them; everything used is derived below from the
Rayleigh-Ritz method and block elimination.

## Design

### The exact interior motion for a given interface motion (derivation)

Notation of ADRs 0469, 0474 and 0511: a part's independent unknowns, boundary `b` and interior `i`; `Psi = -K_ii^{-1} K_ib`; `Phi`
the kept fixed-interface modes (M_ii-normalised, eigenvalues `Lambda`); `R` the residual vectors (M_ii-orthonormal columns). Under
harmonic motion at `omega` with the interface displacement `u_b` given and no interior load, the interior rows of `(K - omega^2 M)
u = 0` read `(K_ii - omega^2 M_ii) u_i = -(K_ib - omega^2 M_ib) u_b`. With `u_i = Psi u_b + d` and `K_ii Psi = -K_ib`:

`(K_ii - omega^2 M_ii) d = omega^2 G u_b`, `G = M_ib + M_ii Psi` (the interior inertia load of the constraint modes, `nb` columns).

So `d = omega^2 (K_ii - omega^2 M_ii)^{-1} G u_b`. In the fixed-interface modes (all of them, `phi_k`, `lambda_k`) this is
`d = sum_k phi_k (omega^2 / (lambda_k - omega^2)) phi_k^T G u_b`. The kept modes carry their terms exactly (their coordinates are
free in the reduction); the Craig-Bampton error is the sum over the **omitted** modes. For `omega^2 < lambda_k` it expands as
`omega^2 / (lambda_k - omega^2) = sum_{j >= 1} (omega^2 / lambda_k)^j`, so the omitted part of `d` lies in the span of

`B_j = P_omit (K_ii^{-1} M_ii)^{j-1} K_ii^{-1} G`, `j = 1, 2, ...`

with `P_omit = I - Phi Phi^T M_ii` (because `(K_ii^{-1} M_ii) phi_k = phi_k / lambda_k`). Truncated after `p` blocks the error of
each omitted term is `(omega^2 / lambda_k)^{p+1} / (1 - omega^2 / lambda_k)` of its own size; at `omega = omega_half` and the
lowest omitted mode `omega^2 / lambda_k = 1/4`, so the shape error of the interface motion falls by about 4 for every block, and
the Rayleigh quotient, being stationary, has the square of that in its frequency error (an estimate, not a bound; the probe
measured more than that, item 2).

**The two forms the acceptance names are the same space for one block.** `Phi^T M_ii K_ii^{-1} = Lambda^{-1} Phi^T` (from `K_ii
Phi = M_ii Phi Lambda`), so `P_omit K_ii^{-1} G = (K_ii^{-1} - Phi Lambda^{-1} Phi^T) G = G_res G`, with `G_res = sum_{k omitted}
phi_k phi_k^T / lambda_k` the **residual flexibility** of the omitted fixed-interface modes: the first block is the residual
flexibility of the constraint modes (applied to their inertia load). The second and later blocks are the Krylov vectors of the pair
`(K_ii, M_ii)` started from the constraint modes. The option takes the number of blocks `p`; `p = 1` is the residual-flexibility
form, `p >= 2` the Krylov form; both are registered on the bar.

### The vectors (registered construction)

After the vector of the model's load (ADR 0474, when `residual_vectors` is on) and those of the load patterns (ADR 0511), in that
order, for a part with `interface_enrichment(p)`, `p >= 1`:

1. Block 1: for each boundary column `j = 0 .. nb-1` (the order of the boundary unknowns) the candidate `K_ii^{-1} g_j`,
   `g_j = M_ib e_j + M_ii Psi e_j` (the column `j` of `G`, with `M_ii Psi` from the CSR `M_ii` and the dense `Psi`), solved with the
   same skyline factor of `K_ii` that gives `Psi`.
2. Block `j + 1` (`j >= 1`): for each vector **kept** in block `j`, in its order, the candidate `K_ii^{-1} (M_ii r)` (block Lanczos
   form: the next block from the M-orthonormalised vectors of the one before, which spans the same space as the raw Krylov block
   plus directions already in the basis or harmless to the Ritz projection).
3. Each candidate is passed to `damped::m_orthonormalise` against `[Phi, R_1 .. R_{k-1}]`, every vector kept before it (the load's,
   the patterns', the enrichment vectors before): classical Gram-Schmidt twice in the M_ii inner product and M-normalisation. **The
   orthogonalisation threshold is the drop rule `RESIDUAL_DROP = 1e-8`** of ADRs 0464, 0474 and 0511: a candidate whose M-norm
   after the two passes is at most 1e-8 of its own is dropped (it adds nothing that the basis lacks to that precision). A block with
   no kept vector ends the enrichment (the next block would start from nothing).

The kept vectors are appended to `R`: the transformation is `[[I, 0, 0], [Psi, Phi, R]]` as in ADR 0474, and every block of the
reduced matrices is the one ADR 0474 writes for `R` (`K_bR = 0` exactly, because `K_bi + Psi^T K_ii = 0` for any interior vector;
`K_qR = Phi^T K_ii R`, `K_RR`, `M_bR = M_bi R + Psi^T M_ii R`, `M_qR`, `M_RR`, the load `R^T f_i`). The file is the version 2 file of
ADR 0474 with these columns in `R`; `reduce_load` and `expand` use `R` as they are. No new file field: a file does not say which of
its vectors are of the load, of a pattern or of the enrichment (not needed to use it).

**What is registered for the bar** (the choice of "how many vectors", fixed now from the derivation and the probe): `p = 1` (the
residual flexibility: `nb = 27` vectors per part) is the configuration **E1**, and `p = 2` (Krylov, 54 per part) is **E2**; both
with the cutoff 32964 rad/s and the vector of the model's load (`CmsOptions::new(0).cutoff(32964).residual_vectors(true)
.interface_enrichment(p)` on A and on B). Both are in the decision rule. The load's vector is part of the configuration because the
acceptance asks for "an option of the reduction of ADR 0474" and the statics clause c5; without it the response misses c3 (probe
item 5, registered as a record).

### Options and API

`CmsOptions` gains `interface_blocks: usize` (default 0; builder `interface_enrichment(p)`). 0 means every operation of `reduce` is
that of ADR 0511, in the same order: nothing of the enrichment is computed (c6). Any `p` is accepted: the drop rule bounds the
number of vectors by the interior size. With `modes = 0` and no cutoff (static condensation) the enrichment still applies (the
basis is then `[Psi]` plus the vectors).

Changed library lines: `crates/rcfd-fem/src/cms.rs` only (no shared solver file; `damped.rs` is not touched). `baseline_hashes` is
run anyway.

## Set-ups

Those of ADRs 0469, 0474 and 0511 (steel `E = 2.1e11`, `nu = 0.3`, `rho = 7850`, Hex8 `hex_box`, threads 1 unless said): the bar F
`[0, 1] x [0, 0.1] x [0, 0.08]`, 16 x 2 x 2, clamped at `xmin`, the corner force `[1, 1, 1]` at `(1, 0.1, 0.08)`; part A `[0, 0.5]`
clamped with the interface `xmax`, part B `[0.5, 1]` free with the interface `xmin` and the corner force; the small model S (4 x 2 x
2, length 0.4, clamped, gravity and the `xmax` traction, interfaces `xmax` and `mid`). The cutoff 32964 rad/s keeps 5 modes in A
and 9 in B; the lowest omitted fixed-interface frequency is 33570.8 rad/s (A) and `omega_half = 16785.42` rad/s; F has ten
frequencies below it. Loss factor `eta = 0.02` in F (`DirectHarmonic`, `LossFactor::Constant`) and in the assembly. Configurations
on both parts: **C0** the cutoff alone; **C1** the ADR 0474 option (C0 and `residual_vectors(true)`); **E1** C1 and one block;
**E2** C1 and two blocks; **E2c** C0 and two blocks (a record); **P** the ADR 0511 option on B (C1 and the three component
patterns) with C1 on A.

## Arithmetic before this registration (disclosed)

One probe program (`f603-scratch/probe/src/main.rs`, copied from the F-559 probe: the Ritz projection of F onto the basis of the
constraint modes of both parts, the kept modes of the cutoff, B's load vector and the enrichment blocks of both parts, built in
F's stiffness and mass restricted to each part's interior with a dense Cholesky of `K_ii`, orthogonalised twice with the drop at
1e-8; dense complex solve; `DirectHarmonic` of F with loss 0.02; the 100 points `j omega_half / 100`; twenty frequencies of F). Run
three times on the API of 291a210c: `probe1.log` (the variants C0, C1, E1, E2, E2c and three blocks), `probe2.log` (E1 again
printing every M-norm ratio), `probe3.log` (E2 again printing the conditioning of the reduced matrices). No F-603 code existed. The
choice of the vectors (the construction above) was made from the derivation before the probe; the probe fixed nothing but the
predictions below and the choice of registering `p = 1` and `p = 2` (a third block is not needed).

1. **F**: frequencies 476.7, 572.6, 2921.4, 3460.0, 4926.4, 7945.5, 8179.0, 9236.8, 14830.3, 15026.7 (below `omega_half =
   16785.42`), 17101.5, ...; 72 of the 100 points are off resonance (farther than `2 eta` from all twenty).
2. **Frequency errors below `omega_half`** (largest of the ten `abs(omega_R / omega_F - 1)`): C0 5.11e-3, C1 5.11e-3, **E1 2.93e-6**
   (14830.3), **E2 1.35e-10**, three blocks 1.75e-11. The count below `omega_half` is ten in every variant. The smallest signed
   error is -3.2e-12 (rounding of the full model's eigen-solution, tolerance 1e-12, and of the dense Ritz solution).
3. **Off-resonance response error** (72 points): C0 0.124, C1 0.106, **E1 1.06e-3** (16281.9 rad/s), **E2 2.93e-5**, three blocks
   1.07e-5. Over all 100 points: E1 1.06e-3, E2 2.93e-5.
4. **Lowering against C0** (the ten below the bound): E1 and E2 lower every one, the least `-1.05e-6` (f1), the most `-5.09e-3`
   (f10); C1 against C0 the least `-5.1e-11` (f2, ADR 0511's number).
5. **Statics of the corner load** (largest difference over the largest entry of F's static solution): C0 1.39e-3, C1, E1, E2
   7.5e-12; **E2c 1.29e-3**, and E2c's off-resonance error is 6.6e-2 (first point above 1e-2 at 5203.5): the enrichment alone does
   not carry the static correction of the load.
6. **Vectors** (M-norm after over before): A, block 1: 27 kept of 27, the ratios 1.8e-5 to 2.4e-2; B, block 1 after the load's
   vector (1.4e-3): 27 kept of 27, the ratios 1.0e-7 to 8.0e-4; block 2 adds 27 more per part, none dropped (E2: A 54, B 55
   vectors). The smallest ratio 1.0e-7 is ten times the drop rule.
7. **Conditioning** (E2, the assembled Ritz matrices): `lambda_min / lambda_max` of `M_r` 8.9e-7, of `K_r` 9.5e-6.

The probe works in F's matrices and a dense factor; the library works in each part's matrices and the skyline factor, so the
numbers above are predictions to two or three digits, not values the tests compare with.

## Checks (limits fixed now)

Tests in new files `crates/rcfd-fem/tests/sa_cms_enrich_default_bits.rs` (D6) and `crates/rcfd-fem/tests/sa_cms_enrich.rs` (the
rest), release profile. Error measures as ADRs 0469, 0474 and 0511 (vectors and statics: largest absolute difference over the
largest absolute reference entry, every equation of F through the part maps; harmonic: the 2-norm of the complex difference over
the whole bar over the 2-norm of F's response; frequencies `omega_R / omega_F - 1` by index, both sorted). **Every aggregated error
is required finite**: each term is asserted finite before it is aggregated and each compared number is required finite by the
check itself (a NaN or an infinity fails it).

| Id | Clause | Check | Limit | Estimate |
|---|---|---|---|---|
| D6 | c6 | digests at 291a210c (printed by `sa_cms_enrich_default_bits.rs`, written with the API of that commit and committed before any library line): the text of B with the ADR 0511 option (P), of B with C1, of S `mid` (`m = 5`, the load's vector and the two S patterns), of A with C0; the eigenvalues of the assemblies P and C0; P's harmonic response at 1000 and 5000 rad/s; the component statics of P through `reduce_load` and `harmonic_with_load` (component y); B expanded from P's 1000 rad/s coordinates; `recover_static` of S `mid`; equal after the change. The digests of ADR 0511 D4 and ADR 0474 D2 also still pass (existing tests) | `==` | holds |
| D7 | c6 | `interface_enrichment(0)` with `m = 5`, with C1 and with P on B: the part `==` the part without, every float of `K_r`, `M_r` in bits, the same text | `==` | holds |
| E-P1 | c1 | vector counts (`residual_vectors()`): E1 A 27, B 28; E2 A 54, B 55; C0 with one block on B 27; C0 with one block on A 27 | exact | probe item 6 |
| E-P2 | c1 | E1 and E2 on A and on B: the largest `abs` entry of `M_qR` and of `M_RR - I` (off-diagonals included: each vector orthogonal to those before it) | `<= 1e-10` | 1e-15 (two passes; ratios at least 1e-7) |
| E-P3 | c1 | B with E1: the start vectors recomputed in the test (`x_j = K_ii^{-1} (M_ib e_j + M_ii Psi e_j)` from B's own matrices, `stiffness_with` and `consistent_with`, a dense Cholesky of `K_ii` in the test, the library's `Psi`): for each `j` the M-norm of `x_j` minus its M-projection on the span of `[Phi, R]` (an M-orthonormal basis built in the test from them), over the M-norm of `x_j`; the largest of the 27. The same with E2 for the second block, its start vectors `K_ii^{-1} M_ii r` of the library's 27 block-1 vectors | `<= 1e-8` each | 1e-10 (solve rounding, `cond(K_ii)` about 1e6) |
| NC-P3 | c1 (negative control) | the same measure of the 27 block-1 start vectors against the span of B with C1 (no enrichment): the largest | `>= 1e-6` | at least 8.0e-4 (probe item 6) |
| E-SP | design | E1 and E2 on B: `relative_asymmetry` of `K_r`, `M_r` after symmetrisation; `asymmetry()` before; `lambda_min / lambda_max` of `M_r` | `<= 1e-13`; `<= 1e-10`; `>= 1e-12` | 0; 1e-15; about 1e-6 (probe item 7) |
| E-TH | design | B with E2, threads 1 and 4: `K_r`, `M_r`, load, `Psi`, `Phi`, `R` bit-identical | bits | holds |
| E-FP | design | B with E2 written and read: `==`, every float in bits, header 2, 55 vectors | exact | holds |
| LF | c2 | E1 and E2: the largest `abs(omega_R / omega_F - 1)` over the ten indices with `omega_F < omega_half`, `omega_half` from the parts' omitted eigenvalues; the count of the assembly's frequencies below `omega_half` is ten | `<= 1e-3`; count 10 | E1 2.9e-6, E2 1.4e-10 |
| NC-LF | c2 (negative control) | the same largest error with C1 (the check sees what ADR 0511 missed) | `>= 1e-3` | 5.11e-3 |
| OFF1 | c3 | E1 and E2: the largest response error at the points `j omega_half / 100`, `j = 1..100`, with `abs(omega / omega_F,k - 1) > 2 eta` for all twenty `omega_F,k`; at least 60 points | `<= 1e-2` | E1 1.06e-3, E2 2.9e-5; 72 points |
| NC-OFF | c3 (negative control) | the same largest error with C1 | `>= 1e-2` | 0.106 |
| SL1 | c4 | E1 and E2 against C0: for each of the ten frequencies below `omega_half`, `omega_E / omega_C0 - 1`; the largest (least lowering) | `< -1e-14` | -1.05e-6 |
| SL2 | c4 | E1 and E2 against C1 (the lowering is the enrichment's, not only the load vector's): the largest of the ten | `< -1e-14` | -1.05e-6 |
| SL3 | c4 | E1 and E2: the smallest `omega_E / omega_F - 1` over the ten (not below the full model beyond rounding) | `>= -1e-9` | -3e-12 |
| NC-S | c4 (negative control) | C0 against itself: the largest of the ten changes | not `< -1e-14` | 0 |
| ST-E | c5 | E1 and E2: the static solution of the assembly (`harmonic(0, 0)`), both parts expanded, against `solve_static` of F, every equation | `<= 1e-10` each | 7.5e-12 (library lower, ADR 0511) |
| ST-E2 | c5 | E2 with the ADR 0511 patterns on B: the statics of each corner-force component (the `component_statics` of ADR 0511) | `<= 1e-10` each | 3e-12 |
| ST-E3 | c5 | S `mid` and `xmax`, `m = 5` with the load's vector and one block, and S `mid` with `m = 0`, no cutoff, the load's vector and one block: `recover_static` of the part's `solve_static` against `solve_static` of S | `<= 1e-10` each | 1e-13 |
| ST-E4 | c5 | S `xmax`, `m = 5`, the load's vector and two blocks, the subspace path (`dense_limit(0)`): the same | `<= 1e-10` | 1e-13 |
| NC-ST | c5 (negative control) | E2c (no load vector): the statics of the corner load | `>= 1e-6` | 1.29e-3 |

Records (reported, not in the decision rule): the ten frequency errors of C0, C1, E1, E2, E2c; the off-resonance curve of E1, E2
and E2c (predicted 6.6e-2, the reason the load's vector is in the configuration); the error at all 100 points including the
excluded ones; the axial-resonance error of E1 and E2 at 8179.0 rad/s against `2 abs(delta) / eta`; E2 against E1 (the change of
the ten frequencies, at the rounding level of the references); the M-norm ratios of the vectors if exposed by the test's own
recomputation; `|K_qR|` over the largest `K_r` entry.

## Negative controls and mutations

Negative controls: NC-P3 (the span measure sees a basis without the vectors), NC-LF and NC-OFF (the clauses are not vacuous on this
bar: the reduction without the enrichment misses them), NC-S (the strictness measure sees nothing when nothing changes), NC-ST (the
statics come from the load's vector, not from the enrichment). Mutations (on a scratch copy of the final tree, its own
`CARGO_TARGET_DIR`; each must be caught by at least one registered check):

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | block-1 start vectors without the term `M_ib e_j` | E-P3 |
| MU2 | block-1 start vectors without the term `M_ii Psi e_j` | E-P3 |
| MU3 | `K_ii^{-1}` left out (the start vectors `g_j` themselves orthonormalised) | E-P3, LF or OFF1 |
| MU4 | the enrichment vectors orthogonalised against the modes only (not the vectors before them) | E-P2, E-P1 or E-SP |
| MU5 | block 2 started again from the constraint-mode loads (block 1 repeated) | E-P1 (E2 counts), E-P3 (block 2) |
| MU6 | the block count ignored (one block whatever `p`) | E-P1 (E2 counts), E-FP |
| MU7 | the coupling `M_bR` set to zero for the enrichment vectors (with `K_bR = K_qR = M_qR = 0` they are then decoupled) | SL2, LF, OFF1 |
| MU8 | the enrichment only when `residual_vectors` is on | E-P1 (C0 with one block) |
| MU9 | the enrichment only on a part with a non-zero load (A skipped) | E-P1 (A counts), LF |
| MU10 | one block added when `p = 0` (the default path changed) | D6, D7 |
| MU11 | the strictness comparison made `<= 1e-14` instead of `< -1e-14` | NC-S |

## Decision rule

**done**: every check met at its registered run, every mutation caught, no amendment after a run. **partial**: a check missed
(recorded as an ignored `*_known_miss` test with its limit and the measured number), a mutation not caught by a registered check, or
a check met only after an amendment made after its run. **todo**: the option does not build or the bar runs cannot be made. No miss
is predicted.

## Limits (not covered)

- One set-up (the bar of ADR 0469, two parts, one interface of 27 unknowns). The vectors grow as `nb` per block: a part with a large
  interface (thousands of unknowns) gets thousands of vectors; interface reduction (characteristic constraint modes) is not
  included.
- The enrichment does not depend on the load or on the frequency band; the number of blocks is the caller's. No automatic choice
  of `p` from a target band.
- The bound `omega_half` is the clause's; the enrichment is not checked above it.
- No case-file key (open question). Everything not covered by ADRs 0469, 0474 and 0511 stays not covered (free-interface methods,
  damping other than a uniform loss factor in the assembly, pre-stress, cyclic constraints, non-zero constraint values, reduced
  parts in an ordinary model: F-553).

## Amendment 1 (2026-11-14, written before any run of the checks)

Found while writing the tests, before any check was run: `ReducedPart::recover_static` recovers the interior from
`static_interior` (`K_ii^{-1} f_i`, exact) and `ReducedPart::solve_static` solves `K_hat u_b = f_hat`, and `K_hat` does not see the
vectors (`K_bR = 0`), so ST-E3 and ST-E4 as registered do not go through `R` at all: they hold whatever the enrichment does. They
are kept as registered (a record that the static recovery path is unchanged) and each gets a second form that goes through the
vectors: **ST-E3b / ST-E4b**, the same parts and options, the single-part assembly `Assembly::new(vec![part])`, its `harmonic(0,
0)` with the model's load, `expand` of the coordinates, against `solve_static` of S, every equation; limit `<= 1e-10` each,
estimate 1e-13 (the measure of ADR 0511 ST-P4: 1.3e-14 to 9.5e-14). Both forms are in the decision rule. MU3 and MU4 are also
expected to be caught by ST-E3b if they break the M-orthonormality the expansion relies on; nothing else changes.

## Addendum 1: results (2026-11-14)

Commits on `f603-work` from 291a210c: a97a833d (this registration, alone), 02f0cef2 (the D6 digests printed at 291a210c by
`sa_cms_enrich_default_bits.rs`, before any library line), e727b1b4 (Amendment 1, before any run), 6ce8d2a1 (the option in
`cms.rs` and the checks `tests/sa_cms_enrich.rs`). Each registered check was run once, release profile (`cargo test --release -p
rcfd-fem --test sa_cms_enrich --test sa_cms_enrich_default_bits -- --test-threads 1 --nocapture`, log `f603-scratch/run1.log`, 8.7
s). No amendment was made after a run. No check missed, so there is no `*_known_miss` test.

Library lines changed: `crates/rcfd-fem/src/cms.rs` only (the module comment, `CmsOptions::interface_blocks` with the builder
`interface_enrichment`, the `reduce` doc line, and the enrichment block after the pattern vectors, guarded by
`interface_blocks > 0`). `damped.rs` and every shared solver file untouched.

| Id | Clause | Limit | Measured | Met |
|---|---|---|---|---|
| D6 | c6 | `==` | 12 of 12 digests of 291a210c equal; ADR 0511 D4 and ADR 0474 D2 pass in the crate run | yes |
| D7 | c6 | `==` | `interface_enrichment(0)` equal for `m = 5`, C1 and the ADR 0511 option | yes |
| E-P1 | c1 | exact | E1 A 27, B 28; E2 A 54, B 55; C0 with one block B 27, A 27 | yes |
| E-P2 | c1 | `<= 1e-10` | `M_qR` 1.4e-16 to 2.7e-16; `M_RR - I` 1.1e-15 to 1.8e-15 | yes |
| E-P3 | c1 | `<= 1e-8` | block 1 (E1) 5.4e-16; block 1 (E2) 3.6e-16; block 2 (E2) 5.1e-16 | yes |
| NC-P3 | c1 control | `>= 1e-6` | 3.0e-2 | yes |
| E-SP | design | limits | asymmetry after 0, 0; before 1.3e-15; `M_r` ratio 7.5e-7 (E1), 4.6e-7 (E2) | yes |
| E-TH | design | bits | threads 1 and 4 bit-identical | yes |
| E-FP | design | exact | header 2, equal in bits, 55 vectors | yes |
| LF | c2 | `<= 1e-3`; count 10 | **E1 2.93e-6** (14830.3 rad/s), **E2 1.35e-10**; count 10 and 10, `omega_half` 16785.42 | yes |
| NC-LF | c2 control | `>= 1e-3` | C1 5.11e-3 | yes |
| OFF1 | c3 | `<= 1e-2`, at least 60 points | **E1 1.06e-3**, **E2 2.93e-5** (both at 16281.9 rad/s); 72 points | yes |
| NC-OFF | c3 control | `>= 1e-2` | C1 0.106 (15778.3 rad/s) | yes |
| SL1 | c4 | `< -1e-14` | E1 and E2 -1.05e-6 (f1; down to -5.09e-3 at f10) | yes |
| SL2 | c4 | `< -1e-14` | E1 and E2 -1.05e-6 | yes |
| SL3 | c4 | `>= -1e-9` | E1 -1.4e-12, E2 -3.7e-12 | yes |
| NC-S | c4 control | not `< -1e-14` | 0 | yes |
| ST-E | c5 | `<= 1e-10` | E1 1.5e-12, E2 1.5e-12 | yes |
| ST-E2 | c5 | `<= 1e-10` | 3.3e-12, 4.4e-13, 1.5e-12 | yes |
| ST-E3 | c5 | `<= 1e-10` | 4.3e-14 (mid), 3.3e-14 (xmax), 4.3e-14 (mid, `m = 0`) | yes |
| ST-E3b | c5 | `<= 1e-10` | 1.3e-14 (mid), 5.7e-14 (xmax), 3.2e-14 (mid, `m = 0`) | yes |
| ST-E4 / ST-E4b | c5 | `<= 1e-10` | 3.3e-14 / 5.7e-14 (subspace path, 55 vectors) | yes |
| NC-ST | c5 control | `>= 1e-6` | E2c 1.29e-3 | yes |

Records: E2c (no load vector) has its frequencies within 9.2e-11 but its off-resonance response off by 6.6e-2 at 13092.6 rad/s
(first point above 1e-2 at 5203.5), as the probe predicted: the response needs both the static correction of the load and the
interface enrichment. At the axial resonance 8179.0 rad/s the error is 1.1e-4 with E1 (`delta` 4.2e-7) and 8.6e-7 with E2. E2
against E1 lowers the ten frequencies by 7e-13 to 2.9e-6. `|K_qR|` over the largest `K_r` entry is 2.7e-16 to 3.2e-16. Every
number is within a few percent of the probe's.

Mutations (scratch copy `f603-scratch/mut`, a git archive of 6ce8d2a1, script `mutate.py`, `CARGO_TARGET_DIR` `target-f603-mut`,
logs `mutlogs/mut-MU*.log`, summary `mutate.log` and `mutate-mu10b.log`):

| Id | Caught by |
|---|---|
| MU1 | E-P3 (3.5e-4, 7.7e-5) |
| MU2 | E-P3 (1.2e-2, 1.0e-3), LF E1 (2.0e-3), OFF1 E1 (3.9e-2) |
| MU3 | E-P3 (up to 0.60), LF (4.2e-3, 2.9e-3), OFF1 (8.7e-2, 6.1e-2) |
| MU4 | E-P2 (`M_RR - I` 0.99), E-P3, E-SP (`M_r` ratio 1.1e-16) |
| MU5 | E-P1 (E2: 27 and 28), E-P3 block 2 (0.63), E-FP |
| MU6 | E-P1 (E2: 27 and 28), E-P3 block 2 (0.63), E-FP |
| MU7 | LF (5.1e-3), OFF1 (0.10); not SL2 (deviation 2) |
| MU8 | E-P1 (C0 with one block: 0 on A and B) |
| MU9 | E-P1 (A: 0), LF (3.3e-3), OFF1 (6.5e-2) |
| MU10 | D6 (1 of 12 equal), NC-P3, SL1, SL2, NC-LF, NC-OFF; not D7 (deviation 3) |
| MU11 | NC-S |

Every mutation was caught by at least one registered check.

Other runs: `cargo test -p rcfd-fem --release`: every test binary passed (915 passed, 0 failed, by the summed result lines of
211 binaries, doc-tests included; log `fem-all.log`). `cargo test -p rcfd-api --release --test baseline_hashes`: 3 passed, 1 ignored
(log `baseline.log`). `cargo test -p rcfd-cli --release --test docs_site`: 2 passed (log `docs_site.log`). `cargo clippy -p
rcfd-fem --all-targets -- -D warnings`: clean. `cargo fmt -p rcfd-fem --check`: clean. Not run: the workspace test suite (the
lead's gate).

### Deviations

1. The mutation harness first ran the two test binaries with cargo's default fail-fast, so when `sa_cms_enrich` failed the D6
   binary was not run. That matters only for MU10 (the mutation D6 is for): MU10 was run a second time with `--no-fail-fast` (log
   `mutlogs/mut-MU10.log`, the first kept as `mut-MU10-first.log`, summary `mutate-mu10b.log`) and D6 caught it. No registered
   check was re-run.
2. MU7 (`M_bR = 0` for the enrichment vectors) was expected to be caught by SL2, LF and OFF1; LF and OFF1 caught it, SL2 did not
   (the least lowering against the ADR 0474 option was -2.8e-10 under the mutation). The registration's reason was wrong: the
   enrichment vectors are M-orthogonal to the load's vector but not K-orthogonal to it, and that vector keeps its `M_bR`, so the
   mutated vectors still couple to the interface through `K_RR` and lower the frequencies a little. The catch by LF and OFF1 stands.
3. MU10 was expected to be caught by D6 and D7; D6 caught it, D7 could not (both sides of its comparison carry the mutation). D7
   checks only that `interface_enrichment(0)` is the same option as not asking; D6 is the check of the default path.
4. E-P3 measured 5e-16 against the estimate 1e-10: the test's dense solve and the library's skyline solve agree far better than
   the condition-number estimate.

### Decision

**done** by the registered rule: every check met at its registered run, every mutation caught by a registered check, no amendment
after a run (Amendment 1 was written and committed before any run).

### Remaining questions

1. A case-file key. None is added. Options: (a) `"interface_enrichment": p` in the part entry of the substructure key that F-553
   (reduced parts in an ordinary model) needs anyway, default 0; (b) a key now on its own. Recommendation: (a), with F-553.
2. The default number of blocks. Options: (a) keep 0 (bit-identical defaults, the caller asks); (b) make 1 the default when a
   cutoff is given, which changes default results of the cutoff path. Recommendation: (a); document that one block (the residual
   flexibility of the constraint modes) gave 3e-6 frequencies and 1e-3 response to half the lowest omitted frequency on the bar,
   two blocks 1e-10 and 3e-5, both with the load's residual vector.
3. Large interfaces: the vectors grow as `nb` per block. Options: (a) leave it (the caller chooses `p`); (b) a follow-up with an
   interface reduction (characteristic constraint modes) before the enrichment. Recommendation: (a) now, (b) only when a fan hub
   or blade model with a large interface needs it.
4. F-559 (ADR 0511, partial): its missed clauses b2 and b3 hold on the same bar with this option added. Options: (a) leave F-559
   partial with its note pointing to F-603; (b) mark F-559 done through F-603. Recommendation: (a), since F-559's acceptance
   was about the load patterns alone.

### For the lead: roadmap entries

Proposed status of F-603: **done**. New note:

> Structural solver, advanced tier. Optional. Follow-up of F-559 (ADR 0511, 2026-11-14). DONE (ADR 0532, 2026-11-14). CmsOptions::interface_enrichment(p) adds to each part p Krylov blocks of (K_ii, M_ii) started from the constraint modes: block 1 is K_ii^-1 (M_ib + M_ii Psi), which after the M-orthogonalisation against the kept modes is the residual flexibility of the omitted fixed-interface modes applied to the inertia load of the constraint modes; each later block is K_ii^-1 M_ii of the vectors kept before; each vector M-orthonormalised twice against the kept modes and every vector before it (dropped below 1e-8), stored as residual vectors (part file version 2 unchanged). Off by default and bit-identical when off (12 digests of the ADR 0511 paths at 291a210c). The bar of ADR 0469 with the corner load, the cutoff 32964 rad/s and the load's residual vector: the ten frequencies below half the lowest omitted fixed-interface frequency (16785 rad/s) within 2.9e-6 with one block (27 vectors per part) and 1.3e-10 with two, against 5.1e-3 without; the response at the 72 off-resonance points within 1.1e-3 and 2.9e-5, against 0.106 without; every one of the ten lowered by at least 1.05e-6 against the cutoff alone; statics of the corner load 1.5e-12. The enrichment without the load's residual vector keeps the frequencies but misses the response (6.6e-2), so the two are used together. Eleven mutations all caught.

Proposed addition at the end of the F-559 note (status unchanged, partial):

> The two clauses missed by ADR 0511 (frequencies within 1e-3 and the off-resonance response within 1 percent up to half the lowest omitted fixed-interface frequency) hold on the same bar when the interface enrichment of F-603 (ADR 0532, 2026-11-14) is added to the residual vectors.

No follow-up item is proposed (remaining question 3 only if a model with a large interface needs it).
