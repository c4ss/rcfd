# ADR 0520: F-586 registration: the viscoelastic damped modes at each root's own frequency

Status: **REGISTERED 2026-11-14, written before any code of F-586 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-586 was written, compiled or run in the repository. Two computations were made
beforehand and are disclosed below with the numbers they printed: (P1) a numpy script on the 40-element chain
(`f586-scratch/rod_arith.py`: the scalar fixed point of each rod root, its contraction rate and its distance to the
frozen-modulus root of ADR 0495); (P2) a probe program in the scratch folder (`f586-scratch/probe`, its own Cargo workspace
with a path dependency on this worktree's crates and its own target directory `target-f586-probe`) that ran only the
**existing** F-582 code (`DampedOptions::viscoelastic_reference`, the frozen modulus) on 3-D cantilevers with unequal weights
for bases 6, 12, `n-2`, `n-1`, and a dense full-space solve of the same frozen problem by
`rcfd_linalg::eigen_general::quadratic_eigen_with`. No library code was written or changed. The limits below are fixed from the
derivations and these numbers; results go in the addenda; nothing above the addenda is changed after a run.

Roadmap item F-586 (dep F-582). Number 0520 given by the lead. Branch `f586-work` from 2f8c1d57, worktree `w49i-work/f586`,
scratch `w49i-work/f586-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number); each registered run made once; defaults bit-identical; no
edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe code, the
crate graph unchanged; no existing test edited; clean-room. The files of F-604 (contact tests), F-602 (J2 sphere test) and
F-600 (contact / nonlinear active-set record) are not touched. No case-file key is added (open question 1).

## What F-586 asks

The acceptance of the roadmap entry, verbatim, binding:

> Every limit not stated here is registered in the ADR before the run. The damped modes of a viscoelastic material iterate the reference frequency of each root to its own Im lambda (a fixed point of lambda^2 M + K*(Im lambda) x = 0 on the modal basis of the storage moduli). On the 40-element rod A the converged first root equals the root of the discrete chain with the complex modulus at that root's own frequency within 1e-8 relative, and differs from the frozen-modulus root of F-582 by a registered amount. A 3-D cantilever with unequal weights, full basis against truncated basis, registers the truncation error. Models without the material keep their bits.

Clauses: (a0) every limit registered here; (a1) each root's reference frequency iterated to its own `Im lambda`, a fixed point
of `lambda^2 M + K*(Im lambda) x = 0` on the modal basis of the storage moduli; (a2) rod A: the converged first root against
the discrete chain with `Mod*` at that root's own frequency, `1e-8`; (a3) its distance to the frozen root of F-582, a
registered amount; (a4) a 3-D cantilever with unequal weights, full basis against truncated basis, the truncation error
registered; (a5) models without the material keep their bits.

The brief adds: (b1) register the iteration (start, update, convergence test, maximum iterations, refusal) and the frequency of
the basis; (b2) opt-in, the F-582 single-reference path keeps its bits (a default-bits record at the base before any library
code); (b3) no case-file key; (b4) every aggregated error finite.

## What was read (no run)

- `AGENT_RULES.md`; the brief; ADR 0495 (whole: the option, the storage clone, `D''`, rod A, the follow-up), ADR 0491
  (arithmetic 1 to 3, the rods), ADR 0485 (D1, the complex moduli), ADR 0450 (D3, the projected quadratic problem).
- `crates/rcfd-fem/src/damped.rs` (whole), `mat_visco.rs` (`complex_moduli`), `assemble.rs` (`assemble_with`,
  `ElementContext`, `stiffness`), `mass.rs` (`consistent`), `modal.rs` (the basis needs fewer modes than free unknowns),
  `rcfd-linalg/src/eigen_general.rs` (`quadratic_eigen_with`), `error.rs` (the variants).
- Tests: `sa_damped_visco.rs` (the rods, `chain_mu1`, the check helpers), `sa_damped_visco_default_bits.rs` (the digest scheme).

## Arithmetic and probes done before the code (disclosed)

1. **Linearity in the moduli.** For an isotropic material the stiffness is linear in `D = 2 G I_dev + K 1 (x) 1`, so over the
   elements of a viscoelastic material `m`: `K_m(G, K) = G K^G_m + K K^K_m` with `K^G_m = int B^T (2 I_dev) B`,
   `K^K_m = int B^T (1 (x) 1) B` (unit moduli). At a frequency `w` the complex stiffness of the material is
   `G*(w) K^G_m + K*(w) K^K_m`; its real part the storage stiffness, its imaginary part the loss stiffness of ADR 0495.
   Projected on a fixed basis `Phi`: `P^G_m = Phi^T K^G_m Phi`, `P^K_m = Phi^T K^K_m Phi`, computed once; the projected problem
   at any `w` is a dense combination of these, no re-assembly.
2. **The rod reference (P1).** As ADR 0495 (a3) the rod's complex stiffness is `(Mod*/Mod_0) K_c`, every mode of the chain an
   eigenvector of the complex problem for any modulus: root `j` at frequency `w` is `i sqrt(Mod*(w) mu_j/rho)` for any basis
   that contains mode `j`. The own-frequency root is the scalar fixed point `w* = Im i sqrt(Mod*(w*) mu_j/rho)`. P1 (numpy,
   `mu_j` from a dense generalised symmetric eigen-solve of the 40-element chain; `mu_1..4 = 2.4677182, 22.232306, 61.883460,
   121.66588`) printed, for `j = 1..4`:
   - rod A: own roots `-49.46268723 + 1979.78160340 i`, `-85.928 + 6059.795 i`, `-91.091 + 10138.385 i`, `-92.614 +
     14227.385 i`; the frozen roots at `omega_c = 1979.0173529`: `-49.46316776 + 1979.76251766 i` (equal to the F-582 measured
     first root), ...; relative distance own to frozen `9.6405e-6`, `2.24e-2`, `2.76e-2`, `2.98e-2`; contraction rate of the
     iteration `0.0250` (root 1), `0.0082`, `0.0033`, `0.0017` (later roots); 6 to 8 iterations to `1e-15`.
   - rod B: distance own to frozen `7.760e-6` (root 1), rate `0.0230`; rod C: `3.722e-5`, rate `0.0404`.
   The rate agrees with the hand estimate: `d ln Im lambda / d ln w = (1/2) d ln Mod'/d ln w`, and for the standard solid at
   `w tau = 1` `d ln Mod'/d ln w = g/(2 - g)` for equal weights (`(2/21)/(2 - 2/21) = 0.0500`, half of it `0.025`).
3. **When the iteration contracts.** For one Prony term `d ln Mod'/d ln w <= g/(2 - g) < 1`, so the rate is below `1/2`;
   for several terms with total weight near 1 at low `w tau` the rate approaches 1 and the iteration slows: a maximum
   iteration count with a refusal is needed (D1).
4. **The 3-D truncation (P2).** The existing F-582 frozen path, the cantilever `[0.6, 0.1, 0.1]` with `6 x 1 x 1` Hex8 (72
   free unknowns), steel base `nu = 0.3`, one term `g = 0.15`, `k = 0.05`, `tau = 1/w1`, `omega_ref = w1` (the first
   undamped frequency of the base, `1744.57 rad/s`), no other damping: the lowest roots with basis 6 against the dense
   full-space frozen problem, relative, roots 1..4: `1.52e-4, 1.52e-4, 9.1e-14, 1.40e-4` (root 3 is torsion, pure shear:
   proportional loss, no truncation); basis 12: `1.52e-4, 1.52e-4, 9.0e-14, 1.39e-4`; basis 70 and 71 (`n-2`, `n-1`):
   `5.4e-12, 6.0e-12, 9.6e-14, 1.2e-13`. The `6 x 2 x 2` cantilever (162 unknowns): basis 6 `1.15e-4`, basis 161 `1.9e-11,
   9.1e-11` (a degenerate pair of the square section; matching by `Im` alone is ambiguous there). Weights `g = 0.3`, `k = 0.02`
   on `6 x 1 x 1`: basis 6 `1.40e-3`, basis 71 `2.4e-12, 5.4e-12`. Times (release, this machine): the damped modes with basis
   71 of the `6 x 1 x 1` model `0.13 s`, the dense full-space solve `0.12 s`. The truncation error is set by the high modes that
   carry the bulk/shear mismatch (basis 12 barely improves on 6); the missing highest mode of basis `n-1` costs about `5e-12`.
   The registered cantilever has a non-square section `[0.6, 0.1, 0.06]` (no degenerate pair) and is not the probed one; its
   truncation is estimated from the probe at the same order, `1e-4` to `2e-4`.

## Design

### D1. The fixed point, per root

New `OwnFrequency { roots, tolerance, max_iterations }` (`OwnFrequency::new(roots)`: tolerance `1e-12`, maximum 50
iterations; builders `tolerance`, `max_iterations`) and `DampedOptions::viscoelastic_own_frequency(OwnFrequency)` (field
`own_frequency: Option<OwnFrequency>`, default `None`). It needs `viscoelastic_reference(omega_ref, temperature)` as well.

- **Basis: kept at one frequency.** The basis is that of F-582: the undamped modes of the storage clone at the options'
  `omega_ref` (plus the residual vectors when asked). It is **not** rebuilt per root (the acceptance's "on the modal basis of
  the storage moduli"; the frequency of those moduli is `omega_ref`). `M_r`, `C_r` (Rayleigh `a M_r + b K_r(omega_ref)`: the
  Rayleigh stiffness part acts on the storage stiffness **at `omega_ref`** and is not re-evaluated, so `C` does not depend on
  the frequency; plus the dashpots) and the constant-loss-factor part `H_eta` of the other materials are as F-582.
- **The projected problem at `w`.** `K_r(w) = K_rest + sum_m (G'_m(w) P^G_m + K'_m(w) P^K_m)`, `H_r(w) = H_eta + sum_m
  (G''_m(w) P^G_m + K''_m(w) P^K_m)`, the moduli from `complex_moduli(material_m, w, temperature)`; `K_rest = Phi^T K_e Phi`
  of the elements of the non-viscoelastic materials (assembled once; absent when every material is viscoelastic). Solved by
  `quadratic_eigen_with` as F-582.
- **Start.** The F-582 frozen solve at `w^(0) = omega_ref`, its reported roots ordered as F-582 by `(Im, |lambda|)`; the lowest
  `roots` of them (all if fewer are reported; a note says so) are iterated, each separately: `lambda^(0)` is that frozen root.
- **Update.** `w^(k+1) = Im lambda^(k)` (`>= 0` by the reporting; 0 for a real root, whose moduli are then the relaxed ones).
  The problem is solved at `w^(k+1)` and `lambda^(k+1)` is the candidate (oscillating or real, the reporting filter of F-582,
  the same shift of a real root to `Im = 0`) nearest to `lambda^(k)` in modulus of the difference.
- **Convergence test.** Before each solve: converged when `|w^(k+1) - w^(k)| <= tolerance * w^(k+1)` or `w^(k+1) == w^(k)`;
  then `lambda^(k)` (solved at `w^(k)`) is reported with its shape and residual, `iterations = k`.
- **Refusal.** If not converged when `k == max_iterations`: `FemError::Solver { solver: "viscoelastic own frequency", reason }`
  naming the root, the count and the last relative change. Options refused (key `"own frequency"`): without a reference
  frequency, `roots == 0`, a tolerance not finite or `<= 0`, `max_iterations == 0`.
- **Report.** `modes` holds the converged roots, ordered by `(Im, |lambda|)`; a new field `own_frequency: Vec<OwnFrequencyRoot>`
  in the same order, each `{ frozen: C64, omega: f64, iterations: usize, change: f64 }` (the starting root at `omega_ref`, the
  frequency `w^(k)` at which the root was solved, the count, the last relative change); empty without the option. `roots` stays
  the `2 m` roots of the frozen solve at `omega_ref`, `omegas` the basis's undamped frequencies (both documented). A note names
  the count iterated and the largest iteration count; a note if two iterated roots converge to the same root (relative
  distance `<= 1e-9`).
- **Default paths.** Without `own_frequency` every line runs as before (the new code is a branch entered only with the option;
  the F-582 values `kr`, `hr` are computed by the existing expressions). The F-582 path is recorded at the base (D3).

The reported root solves `lambda^2 M_r + lambda C_r + K*_r(w) x = 0` at `w` with `|w - Im lambda| <= tolerance Im lambda`;
with rate `r` its distance to the exact fixed point is about `r tolerance |lambda|`.

### D2. Documentation

The module doc of `damped.rs` gains the paragraph of ADR 0520; the module doc of `mat_visco.rs` names the own-frequency
iteration. No case-file key (open question 1).

### D3. The bit record of the F-582 path (B1)

`crates/rcfd-fem/tests/sa_damped_visco_own_default_bits.rs`, written and run on the base 2f8c1d57 after this commit and before
any library code, digests (FNV-1a 64 as ADR 0495 D5) written into the file from that run: (1) rod A, basis 6, at `omega_c`;
(2) rod B, basis 4, Rayleigh, a ground dashpot, residual vectors; (3) the `6 x 2 x 2` cantilever with equal weights `0.2` at
`0.8 w1`, Rayleigh and a dashpot (the DE set-up of ADR 0495); (4) a two-material cantilever (viscoelastic left half with
unequal weights, aluminium right half with `Constant(0.01)`), Rayleigh; (5) the refusal message of a reference frequency on a
model without the material. The B0 record of ADR 0495 (`sa_damped_visco_default_bits.rs`) covers the paths without the
material and is run unchanged.

## The test set-ups

- **Rods A, B, C**: as ADR 0495 (copied into the new test file; `sa_damped_visco.rs` is not edited). `mu_j` of the chain in the
  test by bisection on the Sturm count of the tridiagonal `K_c1 - mu M_1` (the count of negative pivots of its `LDL^T` is the
  count of eigenvalues below `mu`), to `1e-15` relative; `mu_1` also by the inverse iteration of ADR 0495 (set-up check SU).
  The reference root `j` is the scalar fixed point `w <- Im i sqrt(Mod*(w) mu_j/rho)` from `omega_c`, iterated until `w`
  repeats or 200 steps, `lambda_own,j = i sqrt(Mod*(w*) mu_j/rho)`; the frozen reference `lambda_frz,j` at `omega_c`.
- **Cantilever T** (acceptance (a4)): `[0.6, 0.1, 0.06]`, `6 x 1 x 1` Hex8, `xmin` held, 72 free unknowns; steel base
  (`E = 2e11`, `nu = 0.3`, `rho = 7850`), one term `g = 0.15`, `k = 0.05`, `tau = 1/w1`, `omega_ref = w1` (the base's first
  undamped frequency, computed in the test); no other damping; `OwnFrequency::new(4)`. Truncated basis 6, full basis 71
  (`n - 1`, the most the modal analysis gives).
- **Dense full-space reference** (independent of `damped.rs`): the test assembles on the free unknowns, by `assemble_with` and
  the element stiffness with its own `D` matrices (`2 G I_dev + K 1 (x) 1`, written in the test), `K'(w)`, `K''(w)` and `M`
  (`mass::consistent`), and iterates the same fixed point on the full problem with `quadratic_eigen_with` (start: its own frozen
  roots at `omega_ref` ordered by `Im`, nearest-root matching, `|dw| <= 1e-13 w`, at most 100 steps).
- **Cantilever DM** (two materials): as T, elements 0..2 the viscoelastic material, 3..5 aluminium (`E = 7e10`, `nu = 0.33`,
  `rho = 2700`) with `Constant(0.01)`, Rayleigh `(5, 2e-6)` and a ground dashpot of `30` at a tip node along `z`; the dense
  reference uses `C = a M + b K'(omega_ref) + dashpot`, as D1.
- **Cantilever DE** (equal weights, 3-D): as ADR 0495 DE (`6 x 2 x 2`, `[0.6, 0.1, 0.1]`, weights `0.2`, `tau = 1/omega_ref`,
  `omega_ref = 0.8 w1`), Rayleigh mass part only `(5, 0)` and the ground dashpot; `OwnFrequency::new(6)`. With equal weights the
  storage stiffness at any `w` is `E'(w)/E_0` times that of the base, so the basis at `omega_ref` is exactly the basis at every
  `w`: each converged root `j` equals the root, nearest to it, of the elastic model `E'(w_j)`, `nu = 0.3`, with
  `Constant(E''/E'(w_j))`, the same Rayleigh and dashpot, basis 6, `w_j` the reported `omega` of root `j` (a Rayleigh `b`
  would differ: it acts at `omega_ref` in D1).
- **Cantilever TE** (control of T): the T cantilever with equal weights `0.2`: basis 6 against basis 71, no truncation.

## Checks (limits fixed now)

| Id | Clause | What | Limit | Estimate behind the limit |
|---|---|---|---|---|
| SU | set-up | `mu_1` by bisection against the inverse iteration, and against the library's undamped `w1^2 rho/E_0` of the elastic rod, relative, worst | `<= 1e-9` | `1e-13` (ADR 0495 measured `4e-14`) |
| SR | set-up | the test's scalar fixed point of rod A root 1: `|Im lambda_own - w*|/w*` | `<= 1e-14` | converged to repetition |
| DA | a2 | rod A, basis 6, `omega_ref = omega_c`, `OwnFrequency::new(4)`: first root against `lambda_own,1`, relative | `<= 1e-8` (acceptance) | `1e-13` (ADR 0495 `2.5e-14`, plus `r tol = 2.5e-14`) |
| DAm | a2 | rod A, bases 2 and 39, `new(1)`: the same | `<= 1e-8` | `1e-13` (any basis exact) |
| DD | a3 | rod A: `|lambda_1 - lambda_frz,1|/|lambda_frz,1|` from the library (own root against the F-582 frozen root of the same model and options without the iteration) | in `[9.5e-6, 9.8e-6]` | `9.6405e-6` (P1) |
| DDr | a3 | the same against the test's `|lambda_own,1 - lambda_frz,1|/|lambda_frz,1|`, relative difference of the two distances | `<= 1e-4` | `1e-13/1e-5 = 1e-8` |
| DS | a1 (start) | rod A with `omega_ref = 0.5 omega_c` and `2 omega_c` (basis frequency and start): first root against `lambda_own,1`, worst | `<= 1e-8` | `1e-13` (any storage basis is exact for the rod) |
| DH | a1 (each root) | rod A roots 2..4, basis 6: against `lambda_own,j`, worst | `<= 1e-8` | `1e-12` |
| DB, DC | a1 (unequal weights, shear) | rods B and C, basis 6, roots 1..4: against their `lambda_own,j`, worst | `<= 1e-8` | `1e-12` |
| FP | a1 (fixed point) | every iterated root of DA, DH, DB, DC, T, DM: `|Im lambda - omega|/Im lambda` (`omega` reported) | `<= 1e-12` | `<= tolerance` by D1 |
| IT | D1 | rod A root 1: iteration count | in `[3, 15]` | 6 (P1 rate `0.025`, first change `3.8e-4`) |
| T | a4 | cantilever T, own frequency, 4 roots: basis 6 against basis 71, `max_j rel` (the registered truncation error) | in `[1e-5, 1e-3]` | `1.5e-4` (P2, same order) |
| TF | a4 (full basis exact) | cantilever T: basis 71 own-frequency roots against the dense full-space fixed point, worst over 4 | `<= 1e-9` | `5e-12` (P2) |
| TT | a4 (record) | cantilever T: basis 6 against the dense full-space fixed point, worst | record (no limit) | `1.5e-4` |
| TE | a4 (control) | cantilever TE: basis 6 against basis 71, worst over 4 | `<= 1e-10` | `1e-13` (proportional loss) |
| DM | a1 (two materials, Rayleigh, dashpot) | cantilever DM: basis 71 against the dense full-space fixed point, worst over 4 | `<= 1e-9` | `5e-12` (P2; second order in the missing mode) |
| DE | a1 (3-D, equal weights) | cantilever DE: 6 converged roots against the elastic model at each root's own frequency, worst | `<= 1e-9` | `1e-13` (ADR 0495 DE `1.0e-13`) |
| DF | a1 (shift) | rod B with the WLF shift of ADR 0495 DF at `T = 383 K` against `tau a_T` unshifted, own frequency, first root | `<= 1e-12` | `1e-14` |
| C0 | a1 | no terms on the rod A base, `omega_ref = 1`, own frequency: first root against the elastic base, and `iterations <= 1` | `<= 1e-12`; count `<= 1` | `1e-14` |
| P1 | D1 | rod B, own frequency, `threads(1)` and `threads(4)`: roots, shapes, `own_frequency` records | bits equal | the assembly is per element, the algebra sequential |
| RV1 | D1 | rod B with a ground dashpot and `residual_vectors(true)`, own frequency: runs, every reported root finite | finite | D1 |
| NC1 | negative control of DA | the F-582 frozen first root of rod A against `lambda_own,1` | `>= 1e-6` | `9.6e-6` (P1) |
| NC2 | negative control of DA | the test's scalar iterate after one update (`w = Im lambda_frz,1`) against `lambda_own,1` | `>= 5e-8` | `0.025 * 9.6e-6 = 2.4e-7` |
| NC3 | negative control of T | TE against T: `T >= 1e4 TE` | holds | `1.5e-4` against `1e-13` |
| RF | refusals | (i) own frequency without a reference frequency: key `"own frequency"`; (ii) `roots = 0`, tolerance `0`, `NaN`, `-1e-12`, `max_iterations = 0`: key `"own frequency"`; (iii) rod A with `max_iterations(1)`: `FemError::Solver`, solver `"viscoelastic own frequency"`; (iv) own frequency and a reference frequency on a model without the material: key `"reference frequency"` (F-582) | each as stated | D1 |
| N1 | a5, b2 | B1: every digest of the base 2f8c1d57 (F-582 path) | equal | D3 |
| N1b | a5 | B0 of ADR 0495 (`sa_damped_visco_default_bits.rs`, models without the material) and `sa_damped_visco.rs`, unchanged files | pass | D1 |
| N2 | a5 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | no shared output path changed |
| N3 | a5 | `cargo test -p rcfd-fem --release` | pass | D1 |

Every aggregated error is required finite (a NaN anywhere is a miss; `worst` returns infinity for it). The roots are matched
to their references by the nearest root where an index could be ambiguous (T, TF, DM, DE, TE). Files:
`crates/rcfd-fem/tests/sa_damped_visco_own.rs` (all but N1) and `sa_damped_visco_own_default_bits.rs` (N1). If the
whole check function takes more than about two minutes in release, the 3-D parts move to an ignored heavy record with a fast
test kept on the rods (rule 7); estimated about 10 s (P2 times).

## Planned mutations (on a scratch copy with its own `CARGO_TARGET_DIR`, one at a time)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | the frequency not updated (every solve at `omega_ref`) | DA, DD, DH, DB, DC, FP |
| MU2 | `w/(2 pi)` passed to `complex_moduli` in the iteration | DA, DH, DB, DC, DS |
| MU3 | `P^G` and `P^K` swapped (shear and bulk unit matrices) | DB, DC, TF, DM, DA (`G_0 != K_0` at `nu = 0`) |
| MU4 | the viscoelastic loss part dropped from `H_r(w)` | DA, DH, DB, DC, DE |
| MU5 | converged after the first update whatever the change | DA, IT |
| MU6 | the first candidate taken instead of the nearest | DH, DB, DC |
| MU7 | not converging returns the last iterate instead of refusing | RF (iii) |
| MU8 | `K_rest` dropped from `K_r(w)` | DM |
| MU9 | `H_eta` (the constant loss factors of other sets) dropped from `H_r(w)` | DM |
| MU10 | the Rayleigh stiffness part re-evaluated at `w` (`C_r` rebuilt from `K_r(w)`) | DM |

## Decision rule

- **done**: every check of the table met, every planned mutation caught by at least one check, fmt and clippy clean, B1, B0
  and the baseline hashes unchanged.
- **partial**: B1, B0 and the baseline hashes unchanged, DA, DD and T (the acceptance) met, and a miss elsewhere diagnosed and
  recorded as a `*_known_miss` test (or a mutation uncaught and diagnosed).
- **todo**: otherwise (in particular any change of the default bits, or DA, DD or T missed).

## Limits (not covered)

- The basis is that of the storage moduli at one frequency `omega_ref`; for a general model whose loss stiffness is not
  proportional to its storage stiffness, a root far from `omega_ref` carries a truncation error that T measures only at
  `omega_ref = w1`. A basis rebuilt per root (a modal analysis per iteration) is not done.
- The iteration is a plain fixed point on `Im lambda`: it converges when the frequency dependence of the moduli is mild
  (arithmetic 3) and is refused otherwise; no Newton step, no root-tracking through crossings beyond the nearest-root rule.
- The real part of `lambda` does not enter the moduli (the acceptance's `K*(Im lambda)`), so the iterated root is not a root of
  `lambda^2 M + K*(lambda/i)` for a strongly damped root (the analytic continuation in `lambda` is not taken).
- The isotropic base, solid elements, the elimination path only (as ADR 0495).

## Amendment 1 (2026-11-14): three misses of the registered run, diagnosed; corrected set-ups registered before their run

Written after the registered run and a diagnostic, before any corrected run. Nothing above is changed.

**The registered run** (commit 07997bbe, `f586-scratch/run1.log`, made once): every check met except **TF** (`inf`: the dense
reference returned NaN for one root), **DM** (`6.87e-9` against `1e-9`) and **RV1** (refused: `Solver`, "viscoelastic own
frequency", root 0 not converged in 50 iterations, last change `3.3e-5`). The acceptance checks DA (`1.09e-13`), DD
(`9.6405e-6`) and T (`1.504e-4`) are met. The numbers of every check go in Addendum 1.

**Diagnostic** (disclosed; `f586-scratch/probe/src/bin/diag.rs`, the test's helpers copied, the dense iteration instrumented,
output `f586-scratch/diag1-tail.log` and the console):

1. **TF.** The test's dense fixed point stalls at its own noise: the relative frequency changes of T root 1 go `3.3e-2,
   1.1e-3, 3.7e-5, 1.3e-6, 4.3e-8, 1.4e-9, 4.0e-11` and then wander between `3e-12` and `2.7e-11`; the convergence test
   `1e-13` is below the noise floor of the dense 144-unknown quadratic solve (the library's 71-mode solve converges to
   `2e-14`). A defect of the test's reference, not of the library.
2. **DM.** With the point dashpot the missing highest mode of basis `n - 1` is not negligible: library basis `n - 2` against
   `n - 1` differs by `2.0e-8, 2.0e-9, 1.9e-8, 2.3e-8` with the dashpot and `1.9e-14, 2.0e-9, 3.3e-14, 1.3e-14` without; the
   DM error against the dense solve is `5.8e-9, 2.2e-11, 5.6e-9, 6.9e-9` with the dashpot and `5.6e-11, 1.2e-11, 3.8e-12,
   1.9e-13` without (Rayleigh, two materials and the loss factor kept). The probe P2 behind the limit had no dashpot: the
   estimate was wrong for a point dashpot, which couples into the high modes (the reason for the residual vectors of ADR
   0464). The library is not at fault; the basis `n - 1` is not the full space for this load.
3. **RV1.** The lowest reported root of the frozen solve is the pseudo-mode of the residual vector (`-2.137e5 + 1251 i`,
   share `0.986`, flagged `pseudo`); its own-frequency iteration contracts at about `0.86` per step (changes `2.0e-1, 1.2e-1,
   8.0e-2, 4.5e-2` after 1, 2, 3, 5 iterations, `1.6e-2` after 10, `3.0e-3` after 20, `3.3e-5` after 50) and converges
   after 170 iterations at `634 rad/s` (with `max_iterations(1000)`); the physical roots take 7 and 6. Its `Im` is a tiny
   part of a huge `|lambda|` and very sensitive to the moduli. The refusal is the registered behaviour (D1); the registered
   expectation (runs) was wrong for a basis whose lowest root is a pseudo-mode.

**Records.** The registered test is kept as the ignored record `f586_checks_known_miss` (its checks and limits unchanged;
the function body moves into a helper run with the registered variant, which executes the same values).

**Corrected set-ups** (variant `amended`, a new test `f586_checks_amended`; every other check as registered, same limits):

| Id | Correction | Limit |
|---|---|---|
| TF | the dense reference converged when `|dw| <= 1e-10 w` (above the noise floor `2.7e-11` of item 1); not converged in 100 steps is still NaN | `<= 1e-9` (unchanged) |
| TT | the same reference | record |
| DMb | DM without the dashpot (Rayleigh `(5, 2e-6)`, the aluminium set with `Constant(0.01)`, `K_rest`), dense tolerance as TF | `<= 1e-9` (unchanged; disclosed: the diagnostic printed `5.6e-11` for this set-up, so this limit is not set blind) |
| RV1b | RV1 with `OwnFrequency::new(3).max_iterations(500)`: runs, every reported root finite, and root 0 takes more than 50 iterations | finite; `> 50` (disclosed: the diagnostic printed 170) |

DM (with the dashpot) and RV1 (50 iterations) stay missed in the record. The decision rule applies as registered: these misses
are diagnosed and recorded, so at best **partial**. MU10 (Rayleigh at `w`) and MU8, MU9 are expected to be caught by DMb.

## Amendment 2 (2026-11-14): the amended TF missed, diagnosed; no further correction

The amended run (commit 57f8239f, `f586-scratch/run2-amended.log`, made once): DMb `5.51e-11` (met), RV1b runs with 170
iterations of root 0 (met), every other check as in the registered run, and **TF missed again: `1.15e-8` against `1e-9`**.

Diagnostic (disclosed; `f586-scratch/probe/src/bin/diag2.rs` to `diag4.rs`, logs `diag2.log` to `diag4.log`): per root the
library's own-frequency roots with basis 70 and 71 against the dense full space are `2.2e-11, 1.15e-8, 7.2e-12, 1.9e-12`;
the frozen F-582 solve shows the same (`1.9e-10, 1.145e-8, 2.5e-12, 3.2e-13`), so the iteration is not the cause. The
full-space residual of the library's root 2 with basis 71 is `4.6e-3` (roots 1, 3, 4: `3e-12` to `2e-13`); the dense roots,
with the balancing of `quadratic_eigen_with`, have residuals `1e-14`. The library's 71 undamped frequencies equal the lowest
71 of the dense solve (all within `1e-8`; no mode skipped); the one left out, the 72nd at `361876 rad/s`, is `2.3e-4` above the
71st and carries the loss coupling of root 2 (the stiff-direction bending of the non-square section). So basis `n - 1` is not
the full space for this cantilever: the probe P2 (square section) had no such coupling to the top mode. The library is not at
fault; the registered TF set-up (the most modes the modal analysis gives) cannot reach the full space. TF is recorded as a
known miss and not corrected again (a full basis of `n` modes would need the modal analysis to give every mode, which it
refuses by design, ADR 0450). The acceptance clause (a4) is met by T (`1.504e-4`, basis 6 against basis 71), and the record
TT (basis 6 against the dense full space, `1.504e-4`) agrees with it to three digits.

**Tests.** `f586_checks_known_miss` (registered variant) and `f586_checks_amended_known_miss` (amended variant) are ignored
records; `f586_checks` runs the amended variant with TF printed as a record (no limit), so every met check is kept as a
regression test. It is a re-run of met checks for the suite, not a registered run.
