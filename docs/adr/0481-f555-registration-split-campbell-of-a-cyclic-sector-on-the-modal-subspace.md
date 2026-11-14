# ADR 0481: F-555 registration: the split Campbell table of a cyclic sector on the modal subspace (the gyroscopic problem of ADR 0472 on the harmonic modes of each index, the doubled Coriolis matrix of ADR 0411 D5)

Status: **REGISTERED 2026-11-14, written before any code of F-555 and before any run of its checks**. Not yet accepted by the
maintainer. Results go in the addendum at the end; nothing above the addendum is changed after a run.

Roadmap item F-555 (deps F-415, F-544), structural solver, advanced tier ([ADR 0310](0310-structural-solver-advanced-tier-plan.md)),
the follow-up "Split Campbell table of a cyclic sector on the modal subspace" proposed by
[ADR 0472](0472-f415-registration-coriolis-split-campbell-modal-subspace.md) (F-415, remaining question 3). Number 0481 given by
the lead. Branch `f555-work` from main 4c422025, worktree `w49i-work/f555`, scratch `w49i-work/f555-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical (the new behaviour is reached
only through new functions, with the existing option `RotatingOptions::coriolis` that is off by default); no edit of
`baseline_hashes.rs`, of the hash tables or of any golden file; no existing test edited; no new dependency, no unsafe code;
clean-room. Every choice below that the maintainer would normally be asked about is taken on the maintainer's standing
instruction of 2026-11-11 (take the recommendation).

## What F-555 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. The gyroscopic problem of ADR 0472 on the harmonic modes of each index of a cyclic sector (a cos/sin basis and the doubled Coriolis matrix of ADR 0411 D5): on the disc sector of ADR 0411 at 0.4 and 0.8 of its reference speed the split frequencies of every index equal those of RotatingSector::whirl_modes within 1e-6 for the modes whose coupling stays inside the basis; the refusal of ADR 0472 is lifted only for sectors whose material axes pass the check of ADR 0462; without the option every result bit-identical.

Clauses: **a1** the gyroscopic problem of ADR 0472 on the harmonic modes of each index (cos/sin basis, doubled Coriolis
matrix); **a2** on the disc sector DS of ADR 0411 at `0.4` and `0.8 omega_ref`, every index, the split frequencies equal those of
`RotatingSector::whirl_modes` within 1e-6 for the modes whose coupling stays inside the basis (the rule of D5, fixed here);
**a3** the refusal of ADR 0472 lifted only for sectors whose material axes pass the check of ADR 0462; **a4** without the option
every result bit-identical. The brief of the lead adds: register the rule for "coupling inside the basis" before the run, by a
stated rule and not by picking modes after the results (D5); reuse the F-415 projected quadratic solve and tracker and the sector
harmonic modes, no new eigensolver; non-cyclic gyro results and every default keep their bits, with a default-bits test recorded
at the base before any library change (B2); no case-file key (recommended in the addendum).

## What was read (main 4c422025, read only)

ADR 0472 whole (D1 to D7, amendment 1, the Z1 lesson: F-396's reported eigenvalues differ from their own vectors' Rayleigh
quotients by up to 9e-10); ADR 0411 D1 to D6 (the Coriolis matrix, the whole-sector solve D2, the sector D5 with
`G_k = T_k^T blockdiag(G_s, G_s) T_k`, the wave sense), its set-up DS and its addendum (S1 2.2e-13 and 3.0e-13 against the
full rotor; indices with 42, 84, 84, 42 unknowns; S4 the split 0.355 of an in-plane pair of index 1 at `0.8 omega_ref`); ADR
0462 D1, D2 (`mat_ortho::check_cyclic_axes`, called by `RotatingSector::new` and `with_unit_prestress` before the frame), D8 (the
materials `ORTHO_C`, the shape DS); the source files `gyro.rs` (whole), `rotating.rs` (`RotatingSector`: `new`,
`with_unit_prestress`, `modes`, `campbell`, `check_harmonics`, `subspace_mac`), `rotating/whirl.rs` (`RotatingSector::whirl_at`,
`whirl_campbell`, `track_whirl`, `Entry`), `cyclic.rs` (`HarmonicModes`, `doubled_matrix`), `rcfd-linalg/src/eigen.rs`
(`lowest_eigenpairs`: the subspace size `min(n, max(2 p, p + 8))`, so a request of `p = n - 1` or `n - 2` iterates on the
whole space); the tests `si_rotating_whirl.rs` (DS, `omega_ref`), `sa_ortho_cyclic.rs` (W1 on DS with `ORTHO_C`), `sa_gyro.rs`,
`sa_gyro_default_bits.rs` (the digest pattern).

Published sources, cited from memory, **not read** for this work: B. N. Parlett, The Symmetric Eigenvalue Problem
(Prentice-Hall 1980), the chapter on Rayleigh-Ritz approximations (the Rayleigh quotient error quadratic in the angle to the
subspace); P. Lancaster, Lambda-Matrices and Vibrating Systems (Pergamon 1966), and K. P. Hadeler, Arch. Rational Mech. Anal. 27
(1968) 306, for the Rayleigh functional of an overdamped or gyroscopic quadratic family (the name only); L. Meirovitch,
Computational Methods in Structural Dynamics (Sijthoff and Noordhoff 1980), for the modal subspace of gyroscopic systems. Every
formula used is derived below; the sources are cited for the names.

## Design

### D1. The sector problem on the modal subspace (ADR 0411 D5, ADR 0472 D2)

For harmonic index `k` of a sector of `N` copies (`multiplicity` 1 for `k = 0` and `2 k = N`, 2 otherwise) the whole-sector
problem of ADR 0411 D5 is, in the reduced coordinates `q` of the rows `T_k` (`x = T_k q`, `x = (a, b)` the cosine and sine parts of
length `n` each for a pair, `x = a` for a single index):

`(lambda^2 T_k^T M_2 T_k + lambda T_k^T G_2 T_k + T_k^T K_eff,2 T_k) q = 0`, `A_2 = blockdiag(A, A)` for a pair, `A_2 = A` for a
single index, `G = 2 Omega C`.

The harmonic modes of F-396 at the speed, `RotatingSector::modes(k, Omega)` (ADR 0407: the eigenvectors of the reduced `K_eff`
and `M` of the index, expanded by `T_k` to `(cos, sin)`; `options.modal.modes x multiplicity` of them, a pair counted twice), are
a Ritz basis of the admissible motions of the index: `Phi = [x_1 ... x_m]`, `x_i = T_k q_i` (expanded vectors, length
`multiplicity n`). Since `x^T A_2 y = q^T T_k^T A_2 T_k q'` for any `A`, the projection on the expanded vectors with the doubled
matrices equals the projection in the reduced coordinates:

`M_r = Phi^T M_2 Phi`, `K_r = Phi^T K_eff,2(Omega) Phi`, `C_r = Phi^T C_2 Phi` (`m x m`), the first two symmetrised, `C_r`
skew-symmetrised, the defects before that reported (as ADR 0472 D2). The cosine/sine coupling of ADR 0411 D5 enters through the
basis (the rows of `T_k` mix `a` and `b` at the boundary, and the basis of a pair contains both members of each degenerate pair
at rest and at speed: the solver returns whole pairs since `count = 2 modes`), and `C_2 = blockdiag(C, C)` is the doubled
Coriolis matrix of the acceptance. `C_r` is skew since `C_2` is.

The reduced problem is solved by `gyro::split_modes_dense` (ADR 0472 D3: `quadratic_eigen_with` of ADR 0450 in the scaled time;
no new eigensolver). Basis: the **speed basis** (`modes(k, Omega)`) when the reduced `K_eff` of the index is positive definite,
the **rest basis** (`modes(k, 0)`, the modal analysis of the index, ADR 0386) at a loss of stiffness of the index, reported with
the loss (ADR 0472 D2). The shapes are `Phi q` (cosine then sine part for a pair, as `WhirlMode` of ADR 0411). The reporting is
ADR 0472 D4 (split modes with `omega = Im lambda`, growth, whirl number from `C_r`, `M_r`, direction, residual; real roots;
stability by `STABILITY_TOLERANCE`), plus for a pair the wave sense of ADR 0411 D5, `tau = 2 (a_r^T M b_i - a_i^T M b_r) / (|a|_M^2
+ |b|_M^2)` over the real and imaginary parts (the formula of `RotatingSector::whirl_at`).

### D2. Option and bits (a4)

The option is the existing `RotatingOptions::coriolis` (default false). New functions only:

- `RotatingSector::split_modes(harmonic, omega) -> HarmonicSplitModes` (in `gyro.rs`): without the option the F-396 modes of the
  index at the speed (`RotatingSector::modes(k, Omega)`) as real split modes (`omega = signum(l) sqrt|l|`, the eigenvalue, the
  frequency and the residual of F-396 by their bits, the shape the expanded `(cos, sin)` by bits, `im = 0`, whirl 0, wave `Some(0)`
  for a pair, no stability, no basis); with it D1.
- `RotatingSector::split_campbell(harmonics, speeds) -> SectorSplitCampbell`: without the option each family's table is the
  table of `RotatingSector::campbell(harmonics, speeds)` itself (moved, not recomputed: the bits of F-396 by construction), no
  whirl, direction, wave or growth; with it the split modes of every index and speed tracked per index by `track_whirl` (ADR 0411
  D4, the complex-line tracker), the growth per track and speed and the stability, basis and loss per index and speed.

Unchanged: `RotatingPart::split_modes` and `split_campbell` keep refusing a model with a cyclic constraint (key `"cyclic"`; the
whole part with cyclic rows is not the sector problem); the message gains the name of the new entry point (text only, the key
unchanged; no existing test reads the text, checked by grep before the change). Shared-file edits are visibility only:
`check_harmonics` in `rotating.rs` becomes `pub(crate)`; the re-exports of `gyro`. The projection and dense solve of
`RotatingPart::split_at` are factored into one private function used by both paths with the same operations in the same order
(B2 records the non-cyclic split results at the base to prove the bits).

### D3. The material axes (a3)

A `RotatingSector` exists only for a model that passes `mat_ortho::check_cyclic_axes` (ADR 0462 D1, D2: every material with its
own axes has cylindrical axes about the cyclic axis and origin within `CYCLIC_AXES_TOLERANCE`), since both constructors call it
before the frame. The new functions are methods of `RotatingSector`, so the refusal of ADR 0472 is lifted exactly for those
sectors; no second test is added (a duplicate would be dead code that no check could see). A sector with other axes is refused
by the constructor with the error of ADR 0462 (`FemError::NotYetSupported`, item `"material"`), check A1.

### D4. Interfaces (all new, in `gyro.rs`)

`HarmonicSplitModes { harmonic, multiplicity, modes: SplitModes, waves: Vec<Option<f64>> }` (`waves` parallel to `modes.modes`);
`SectorSplitCampbell { sectors, harmonics, multiplicities, coriolis, families: Vec<SplitCampbell> }` (one `SplitCampbell` of ADR
0472 per index; `unstable()` per family as there); `RotatingSector::{split_modes, split_campbell}`. Refusals: a harmonic above
`N/2` (`"harmonic"`), a negative or non-finite speed (`"speed"`), the harmonic list as `campbell` (`"harmonics"`), the speeds
(`"speeds"`). Case-file key: none (the maintainer's call; recommendation in the addendum).

### D5. The rule for "coupling inside the basis" (fixed now; derived)

Let `phi` be a whirl mode of the whole-sector problem of index `k` at `omega` (from `RotatingSector::whirl_modes`, the reference;
residual about 1e-13, ADR 0411 Q1), `Q(w) = K_eff,2 - w^2 M_2 + i w G_2` (Hermitian: `K`, `M` symmetric, `G` real skew), so
`Q(omega) phi = 0` and `phi^H Q(omega) = 0`. Let `s = P phi` be its `M_2`-orthogonal projection on `span Phi` (real and imaginary
parts projected separately; the Gram matrix of `Phi` solved, not assumed the identity) and `e = phi - s`. Then exactly

`s^H Q(omega) s = (phi - e)^H Q(omega) (phi - e) = e^H Q(omega) e`.

The Rayleigh functional `p(x)` of the Hermitian family (the root of `x^H Q(p) x = 0` near `omega`; Lancaster, Hadeler, from
memory) is stationary at eigenvectors, and the Ritz values of the projected problem are its stationary values on `span Phi`.
One Newton step from `omega` gives `p(s) - omega = - s^H Q(omega) s / s^H Q'(omega) s`, `Q'(w) = -2 w M_2 + i G_2`, so

`eta = |e^H Q(omega) e| / (omega |s^H (2 omega M_2 - i G_2) s|)`

is the first-order relative frequency error of the Ritz value that approximates `omega`. With `e = e_r + i e_i`, `s = s_r + i s_i`
and real arithmetic: `e^H Q e = e_r^T (K - omega^2 M) e_r + e_i^T (K - omega^2 M) e_i - 2 omega e_r^T G e_i`, `s^H (2 omega M - i G)
s = 2 omega (s_r^T M s_r + s_i^T M s_i) + 2 s_r^T G s_i` (all with the doubled sector matrices). In the modal coordinates of the
F-396 modes of the index (`K_eff = diag(w_j^2)`, `M = I`) and first order in `G` for the part outside the basis, `e = sum_out b_j
phi_j` with `b_j = -i omega G_ja/(w_j^2 - omega^2)` and `eta = sum_out |b_j|^2 (w_j^2 - omega^2)/(2 omega^2) = sum_out 2 Omega^2
|C_ja|^2/(w_j^2 - omega^2)` to leading order (the second-order shift of ADR 0472 arithmetic 4, with the coupling inside the basis
solved exactly). The difference between the Ritz value and `p(s)` is of the order `|Phi^T G e|^2/gap`, fourth order in the
coupling outside the basis, and is neglected.

**Rule (registered):** a reference mode's **coupling stays inside the basis** when `eta <= ETA_IN = 1e-7` and its stored residual
is `<= 1e-9`. `ETA_IN` sits a factor 10 below the 1e-6 limit of a2 to cover the neglected higher-order terms and the rounding of
the reduced solve (`<= 3.4e-11` relative, ADR 0472 amendment 1). The rule is computed in the test from the reference mode, the
basis and the sector matrices (`RotatingSector::part()`: `operator`, `mass`, `coriolis`), before the split modes are compared.

**Matching:** each inside reference mode is matched to the split mode of the same index and speed with the largest complex MAC
`|phi^H M_2 psi|^2/((phi^H M_2 phi)(psi^H M_2 psi))` (ties: the lower split mode); the matched mode must have MAC `>= 0.99` and the
matches of one index and speed must be distinct.

## Set-ups (fixed now)

Steel `E = 2e11 Pa`, `nu = 0.3`, `rho = 7850 kg/m^3`, axis `e_z` through the origin. **DS** of ADR 0411: `N = 6`; `hex_box([0.25, 0,
-0.1], [0.75, 2 pi/6, 0.2], [2, 1, 1])` mapped to `(r cos t, r sin t, z)`, Hex20 by `upgrade_to_hex20_mapped`, `xmin` clamped,
`Cyclic("ymin", "ymax", e_z, 0, 6)`; both effects of ADR 0403 on. Modal options `ModalOptions::new(m_b).tolerance(1e-9)`, 400
iterations (ADR 0411's options with the count `m_b`), `RotatingOptions::new(m_b).modal(..).coriolis(true)`. Three bases:
**B4** (`m_b = 4`, ADR 0411's own count), **B12** (`m_b = 12`) and **B41** (`m_b = 41`, the largest count the reference and the
basis accept on the single indices of 42 unknowns: `count < n`; the pairs `82 < 84`; the subspace iteration then works on the
whole space of the index). The reference is `RotatingSector::whirl_modes(k, Omega)` of the same sector (the lowest `m_b x
multiplicity` whirl modes, the dense solver of ADR 0411 D2, the default `WhirlSolver`). `omega_ref` = the lowest rest whirl
frequency over `k = 0 .. 3` of the B4 sector (`whirl_modes(k, 0).modes[0].omega`, ADR 0411's function), used for all bases.
Speeds `0.4 omega_ref` and `0.8 omega_ref`.

**DSO**: DS with `ORTHO_C` of ADR 0462 D8 (`E = (1.4e11, 1e10, 1e10)`, `nu = (0.3, 0.3, 0.45)`, `G = (5e9, 5e9, 3.5e9)`, `rho =
1600`, cylindrical axes about `e_z` through the origin), basis B41, `omega_ref,o` = the lowest rest whirl frequency over `k = 0 ..
3` of this sector; speeds `0.4` and `0.8 omega_ref,o`.

**DSF**, **DSL** (refused axes): DS with `ortho(Fixed(I))` and with `ortho(Cylindrical { origin: (0.1, 0, 0), axis: e_z })`.

**DSS** (loss path): DS with the stress stiffening off (`stress_stiffening(false)`, spin softening on), B4, at `10 omega_ref`
(`K_eff = K - Omega^2 M_perp`: the reduced in-plane stiffness of some index falls below zero once `Omega` exceeds the lowest
in-plane frequency of `(K, M_perp)`, a few `omega_ref` at most for this stocky disc; if no index loses stiffness the set-up is
defective, not the code).

**CB**, **CBA** (only for the bits): the cube on springs of ADR 0472 (`k_y = 1` and `2.25`), `modes = 3`, speeds `0.07 j w0`, `j =
0 .. 12`, and `{0, 0.5, 1.2, 1.8} w0`.

## Arithmetic before this registration (disclosed)

Nothing of F-555 was compiled or run before this text; the set-up numbers are ADR 0411's measured ones.

1. Sizes (ADR 0411 addendum): 42, 84, 84, 42 reduced unknowns for `k = 0 .. 3`; the dense reference of order 84 takes
   milliseconds.
2. Order of `eta` (D5, leading order): `eta ~ 2 Omega^2 sum_out |C_ja|^2/w_j^2`. For B4 the first modes outside the basis lie at a
   few `omega_ref` with couplings `|C_ja|` of order 0.1 (the in-plane pair of S4 has `kappa = 0.81`): at `0.8 omega_ref` `eta ~ 2
   x 0.64 x 1e-2/10 ~ 1e-3`, so few or no B4 modes are expected inside (recorded, N2). For B41 only the top mode (or pair) of each
   index is outside, at a frequency of order 30 to 100 `omega_ref` for this coarse mesh, and the coupling of a smooth low mode
   with it is small (the field `a x phi` of a low mode is smooth and projects little on the most oscillatory mode): `eta ~ 1.3
   |C|^2/1e3` to `/1e4`, below 1e-7 for `|C| < 0.03` to `0.1`. That the lowest modes of every index are inside on B41 at both
   speeds is therefore expected but not certain; it is a registered check (V1), a miss of which would be diagnosed, not tuned.
3. Accuracy of an inside mode: `|theta/omega - 1| <= eta + 3.4e-11 + 3e-13` (the Ritz error, the reduced solve, the reference),
   so `<= 1.1e-7` against the limit 1e-6.
4. Raw skew defect of `C_r`: ADR 0472 measured `<= 1.0e-15` on models of up to 2304 equations; the doubled sector basis gives
   the same order: estimate `1e-15` against 1e-11.

## Checks (limits fixed now)

Tests in new files `crates/rcfd-fem/tests/sa_gyro_sector_default_bits.rs` (B2) and `crates/rcfd-fem/tests/sa_gyro_sector.rs`
(the rest), release profile.

| Id | Clause | Check | Limit | Estimate |
|---|---|---|---|---|
| S1 | a1, a2 | DS, bases B4, B12, B41, `k = 0 .. 3`, `0.4` and `0.8 omega_ref`: every reference mode inside the basis by D5 is matched (D5 matching: MAC `>= 0.99`, distinct) and `max |theta/omega - 1|` over them | MAC, distinct; `<= 1e-6` | `<= 1.1e-7` (arithmetic 3) |
| S2 | a2, a3 | the same on DSO (B41) at `0.4` and `0.8 omega_ref,o` | as S1 | as S1 |
| V1 | a2 | B41 on DS and DSO: at each index and each speed at least one reference mode inside the basis | `>= 1` each | arithmetic 2 |
| K1 | D1 | DS (B4, B12, B41) and DSO at both speeds, every index: `split_modes` has basis `Speed`, no loss, `Stable`, exactly `m_b x multiplicity` split modes and no real roots (`K_r`, `M_r` positive definite: every root on the imaginary axis) | each | D1 |
| G3 | D1 | raw skew defect of the projected doubled `C` (the `skew_defect` field), every run of K1 | `<= 1e-11` | `1e-15` (arithmetic 4) |
| C1 | D2 | DS B4 `split_campbell(&[0, 1, 2, 3], {0, 0.2, 0.4, 0.6, 0.8} omega_ref)` with the option: 4 families, multiplicities `1, 2, 2, 1`, tracks `4, 8, 8, 4`; at every speed each stored frequency (Hz) of a track equals by bits the frequency of a split mode of `split_modes(k, Omega_j)`, and each track's stored modes are distinct at each speed; every `SplitSpeed` `Stable` | each | D2 (the tracker of ADR 0411 on the modes of D1) |
| L1 | D1 | DSS at `10 omega_ref`, `k = 0 .. 3`: `split_modes` has basis `Rest` with the loss of `RotatingSector::modes(k, Omega)` exactly where that reports a loss and `Speed` elsewhere; at least one index has a loss (else a defective set-up); every root finite | each | D1 |
| B1 | a4 | option off on DS (B4): `split_modes(k, 0.4 omega_ref)`, `k = 0 .. 3`: eigenvalues, frequencies, residuals and the `(cos, sin)` shapes equal by `to_bits` those of `modes(k, 0.4 omega_ref)`; `split_campbell(&[0, 1, 2, 3], speeds of C1)` off: every family table (eigenvalues, frequencies, MAC, modes, losses) equal by bits to `campbell` of the same sector | identical | by construction (D2) |
| B2 | a4 | digests printed at main 4c422025 by `sa_gyro_sector_default_bits.rs` (written with the API of that commit, committed before any library line): CB `split_campbell` with the option (eigenvalues, whirl numbers, growth), CB `split_campbell` without (eigenvalues), CBA `split_campbell` `unstable()` (speeds and growth), CB `split_modes(0.5 w0)` (frequencies, first shape), DS `campbell` over the C1 speeds (eigenvalues, MAC), DS `whirl_modes(1, 0.8 omega_ref)` (`frequencies_all`, the first mode's real part), DS `modes(2, 0.4 omega_ref)` (eigenvalues): all equal after the change; `cargo test -p rcfd-api --release --test baseline_hashes` passes; `cargo test -p rcfd-fem --release` passes | `==`; pass | holds |
| A1 | a3 | DSF and DSL: `RotatingSector::new` refused with `FemError::NotYetSupported { item: "material", .. }` (no split modes reachable); `RotatingPart::split_modes` of DS (the whole part with its cyclic rows) still refused with `"cyclic"`; DS with `ISO` and DSO take `split_modes` (K1) | each | D3 |
| X1 | D4 | refusals of the new functions on DS: `split_modes(4, .)` (`"harmonic"`), `split_modes(1, -1.0)` and `(1, NaN)` (`"speed"`), `split_campbell(&[1, 0], .)` (`"harmonics"`), `split_campbell(.., &[0, 2, 1])` (`"speeds"`) | each refused with its key | D4 |

Records (reported, not in the decision rule): per basis, index and speed the number of reference modes and of inside modes; for
every reference mode with `1e-10 <= eta <= 1e-3` the ratio of the measured `|theta/omega - 1|` (best-MAC match) to `eta` (the
validation of the rule, min and max); the whirl numbers and wave senses of the matched inside modes against the reference's; the
asymmetry of `K_r`, `M_r`; the residuals; `split_modes(k, 0)` with the option against F-396 at rest (the Z1 question of ADR 0472,
not a check here); the smallest stored MAC of C1.

## Negative controls and mutations

| Id | Control | Expected |
|---|---|---|
| N1 | DS B41, index 1 at `0.8 omega_ref`: the option-off split frequencies (each F-396 pair frequency twice) against the reference whirl frequencies, both sorted, the lowest 8: largest relative difference | `> 1e-3` (the Coriolis split of index 1 is 0.355 of an in-plane pair, ADR 0411 S4) |
| N2 | DS B4 at `0.8 omega_ref`: at least one reference mode has `eta > 1e-7`, and the largest `|theta/omega - 1|` over the excluded modes (best-MAC match) | `>= 1` excluded; `> 1e-6` |

Mutations (on a scratch copy of the worktree, each run once against `sa_gyro_sector`; each must be caught by a registered check):

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | the doubled Coriolis matrix replaced by `blockdiag(C, 0)` for a pair | S1, S2 (pair indices) |
| MU2 | the speed of the reduced solve halved (`G = Omega C`) in the sector path | S1, S2 |
| MU3 | the sine parts of a pair's basis set to zero | S1, S2, K1 |
| MU4 | the option-off path through the reduced solve with `G = 0` | B1 |
| MU5 | no rest basis: a loss speed returns an error | L1 |
| MU6 | `check_cyclic_axes` removed from `RotatingSector::new` | A1 |
| MU7 | the sector's `K_r` projected from the rest operator (`operator(0)`) instead of `operator(Omega)` | S1, S2 |

## Decision rule

**done**: every check met at its registered run (S1 to X1), N1 and N2 as expected, every mutation caught, no amendment after a run.
**partial**: a check missed (kept as a `*_known_miss` with its limit), a mutation not caught by a registered check, or a check met
only after an amendment made after its run; V1 missed while S1 and S2 hold counts as partial (the clause a2 then holds on fewer
indices or speeds than asked). **todo**: the module does not build or the DS runs cannot be made.

## Limits (not covered)

- The modal subspace is only as good as its basis: for B4 most modes are expected outside the rule; no static correction vectors
  are added. The rule of D5 is a first-order estimate, validated by the record, not a bound.
- The rest basis at a loss of stiffness of an index is a poorer Ritz basis; L1 checks the path, not its accuracy.
- Stationary-frame frequencies (ADR 0419) are not attached to the split table; the CSV of a family is ADR 0472's.
- No damping, no case-file key, no CLI output. The whole part with cyclic rows stays refused by `RotatingPart::split_*`.
- One disc geometry (and its orthotropic variant); the 120 mm fan sector is not run here.
