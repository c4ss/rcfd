# ADR 0592: F-668 registration: the negative-pivot count of the pre-stressed warping harmonic and transient solves

Status: **REGISTERED 2026-11-15, written before any code of F-668 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-668 was written, compiled or run in the repository: the files listed under "What was
read" were read, and the arithmetic below was done with one plain numpy script in the scratch folder
(`w49i-work/f668-scratch/arith.py`, log `arith.log`: pinned cubic-Hermite Euler-Bernoulli beams in both bending planes under
the axial force, the Timoshenko correction of the critical loads in closed form, the Hermite Vlasov twist model with Wagner's
term of ADR 0585 re-typed; no rcfd code; disclosed, its numbers quoted below). Results go in the addenda at the end; nothing
above the addenda is changed after a run.

Roadmap item F-668 (deps F-661), structural solver, advanced tier, the follow-up proposed by
[ADR 0585](0585-f661-registration-pre-stress-on-the-warping-harmonic-path.md) (its question Q3), with the clause in brackets
added by the lead on the recommendation of [ADR 0591](0591-f667-registration-pre-stress-on-the-warping-transient-path.md) (its
Q2). The harmonic path is `WarpingHarmonic` with `WarpingHarmonicOptions::prestress` (ADR 0585 D1), the transient path
`WarpingTransient::with_prestress` (ADR 0591 D1). Number 0592 given by the lead. Branch `f668-work` from f44b98ae (the F-667
branch, which carries F-635, F-643, F-661, F-663 and F-667), worktree `w49i-work/f668`, scratch `w49i-work/f668-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test edited;
no new dependency, no unsafe code, the crate graph unchanged; clean-room. Every choice below that the maintainer would normally
be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation), except the open
questions at the end.

## What F-668 asks

The acceptance, binding, verbatim (the bracket the lead's extension):

> Every limit not stated here is registered in the ADR before the run. WarpingHarmonic with a pre-stress reports the number of
> negative eigenvalues of K + K_sigma on the free equations (a Sturm count of the real factor at omega = 0); the fork-supported I
> beam under N = -0.5 of the torsional buckling load reports exactly the number of weak-axis bending modes beyond their Euler
> load (one), and 0 under N = +0.5; [and WarpingTransient::with_prestress reports the same count;] every result without the
> option bit-identical.

Clauses: **a0** every limit registered here; **a1** `WarpingHarmonic` with a pre-stress reports the number of negative
eigenvalues of the reduced `K + K_sigma`, by a Sturm count of its real `L D L^T` factor; **a2** the fork beam reports exactly
1 under `N = -0.5 |N_T|` and 0 under `N = +0.5 |N_T|`; **a3** `WarpingTransient::with_prestress` reports the same count; **a4**
every result without the option bit-identical.

### The count, derived (C6)

Sylvester's law of inertia: for a real symmetric `A = L D L^T` with `L` unit lower triangular (a congruence), `A` and `D` have
the same numbers of negative, zero and positive eigenvalues; `D` diagonal, so the number of negative eigenvalues of `A` is the
number of negative pivots `d_i`. A factorisation without pivoting exists when no leading principal minor in the elimination order
vanishes; the factor of the repository (`rcfd_linalg::skyline::SkylineLdlt`, ADR 0274) refuses a pivot with `|d_i| <= theta_i`
under `PivotRule::NonZero`, so a returned count is the inertia of the matrix it factored (up to the rounding of the factor, not
an issue away from a zero eigenvalue). This is the count `rcfd_linalg::eigen::count_below(K, M, 0, Rcm)` makes at the shift 0
(ADR 0283 D8: `K - 0 M = K` exactly, `combine` returns `K` at `s = 0`), the count used by every Sturm check of the repository.
The pencil `(K + K_sigma) x = omega^2 M x` with `M` positive definite has the same number of negative eigenvalues as `K +
K_sigma` (the congruence `M^{-1/2}`), so the count is also the number of modes with `omega^2 < 0`, i.e. "beyond a buckling load"
in the language of the acceptance.

**The expected count of the fork beam (derived, not taken from the acceptance).** With the axial pre-stress `N` alone (no
moments, no shear, a doubly symmetric section), the reduced `K + K_sigma` of the full fork beam is block diagonal in the bending
in `x-y` (weak, `I_z`), the bending in `x-z` (strong, `I_y`), the twist with the warping `(phi, chi)`, and the axial `u` (ADR 0546
D2; ADR 0585 HB- and ADR 0591 HB- measured the coupling exactly 0). Each block of a pinned (fork) beam has, in the continuum, the
modes `sin(k_m x)`, `k_m = m pi/L`, with the stiffness per mode

- bending: `E I k_m^4 + N k_m^2`, negative iff `-N > P_E,m = m^2 pi^2 E I/L^2` (with the shear flexibility of the Timoshenko
  element, `P_m = P_E,m/(1 + P_E,m/(kappa G A))`);
- twist (Vlasov with Wagner's term, C4 of ADR 0585): `(G J + N I_p/A) k_m^2 + E C_w k_m^4`, negative iff `-N > N_T,m = (G J + E
  C_w k_m^2) A/I_p`;
- axial: `E A k^2` plus at most an `N k^2` term, `E A = 6.16e8` against `|N| <= 1.6e6`: never negative.

`|N_T| = N_T,1 = 1.281370e6` N. Arithmetic 1: the weak bending loads `P_1 = 3.6557e5` (`0.2853 |N_T|`), `P_2 = 1.4542e6`
(`1.1349`), `P_3 = 3.2421e6` (`2.530`); the strong `P_1 = 4.4972e6` (`3.510`); the twist `N_T,2 = 2.6270e6` (`2.050`). Under `N =
-0.5 |N_T|` exactly one load is exceeded: the weak `m = 1` (`0.5 > 0.2853`; the next, weak `m = 2`, at `1.1349`, and the torsional
load at `1.0`). **The count is 1, as the acceptance states**; under `N = +0.5 |N_T|` every block stiffness is the stiffness plus a
positive semi-definite `N int w'^2` (and `N I_p/A int phi'^2`), so **the count is 0**. On the 16-element model the discrete loads
lie above the continuous ones by at most `1.3e-4` relative (Hermite, the weak `m = 2`: `1.465054e6` against `1.465006e6`; the
twist `+1.4e-6`, ADR 0585 arithmetic 2), far from every margin. The registered extra points (C4): `N = -0.25 |N_T|` count 0
(margin `0.2853/0.25 - 1 = 14 %`), `N = -1.2 |N_T|` count 3 (weak `m = 1` and `m = 2`, margin `1.1349/1.2`: 5.4 % below; and the
torsional `m = 1`, margin 17 %; the twist `m = 2` at `2.05` and the strong `m = 1` at `3.51` not exceeded); the torsion-only
model (bending and axial held) counts only the twist: 0 at `-0.5`, 1 at `-1.2`.

### Readings, registered

- **"Fork-supported I beam"**, **"N"**, **"the torsional buckling load"**, **"pre-stress"**: exactly those of ADR 0585 and ADR
  0591 (the section `doubly_symmetric_i(0.1, 0.01, 0.18, 0.006, 5/6)`, steel `E = 2e11`, `nu = 0.3`, `rho = 7850`, `L = 3`
  along `x`, 16 Beam2 elements, `u = 0` at node 0, `v = w = phi = 0` at both ends, the warping free; the pre-stress
  `WarpingPrestress::from_static` of the warping static solve of the same beam with the force `(N, 0, 0)` at `xl` and no other
  load; `N` tension positive; `|N_T| = (G J + E C_w k_1^2) A/I_p`). The midspan torque `T = 1000` is in the solver's model (as
  ADR 0585) but plays no part in the count (it is a load, not in `K` or `K_sigma`).
- **"Reports"**: a new method `negative_eigenvalues(&self) -> Result<usize>` on `WarpingHarmonic` and on `WarpingTransient`
  (D1). It is computed when asked, from the matrices the solver already holds; nothing is computed or stored at construction,
  so construction, `response`, `start`, `run` and every other result are unchanged (a4). Without a pre-stress the method counts
  `K` (0 for a supported model): reported, not an option, since the method is new.
- **"The real factor at omega = 0"**: the `L D L^T` factor of the real reduced `K + K_sigma` (the real part of the harmonic
  dynamic stiffness at `omega = 0` without damping) by `SkylineLdlt::factor_with` with the reverse Cuthill-McKee order,
  `PivotRule::NonZero` and the symmetry tolerance `1e-12` (the options of `count_below`). The harmonic path's own factor is
  complex (`ComplexSkylineLdlt`) and is not changed.
- **"The same count"** (a3): `WarpingTransient::with_prestress` of the same model and pre-stress reports the same number; its
  stiffness is `K + K_sigma` from `CsrMatrix::from_triplets` (ADR 0591 D1 step 3), each entry `k + g` as on the harmonic path,
  on a pattern without the mass's entries. Different patterns can give different elimination orders and pivots but the same
  inertia (C6).
- **A singular `K + K_sigma`** (at a buckling load, or a model with an unconstrained motion): the factor refuses a pivot and the
  method returns the refusal with the key `"stiffness"` (no count is reported for a matrix that is numerically singular in the
  elimination order). Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).

### Audit of refusals this change could lift (done before this text)

None. No path reports a count today; the new method refuses nothing that was accepted.

## What was read (no run)

- `AGENT_RULES.md`; the brief; ADR 0585 (whole, in particular C4, D4 "an accidental small pivot", arithmetic 4, Q3, the
  follow-up); ADR 0591 (whole, the compression handling, Q2 and the note for the lead).
- `crates/rcfd-fem/src/beam_warping.rs`: `reduced_dynamic`, `reduced_geometric`, `WarpingHarmonic` (`new`, `with_prestress`,
  `response`), `WarpingTransient` (whole), `buckling_factors_with`, `WarpingPrestress::check_for`, `local_geometric_stiffness`.
- `crates/rcfd-fem/src/transient.rs`: `Integrator` (fields), `from_matrices`, `build`, `finish`.
- `crates/rcfd-linalg/src/skyline.rs` (`PivotRule`, `SkylineOptions`, `factor_with`, the pivot test, `inertia`);
  `crates/rcfd-linalg/src/eigen.rs` (`count_below`, `count_unchecked`, `combine`, `Factor::negatives`).
- Tests: `sa_beam_warping_f667.rs` (the fork beam, `torsion_only`, `axial_prestress`, `n_t`, `Checks`).

## Design

### D1. The method

A private `fn sturm_negatives(a: &CsrMatrix) -> Result<usize>` in `beam_warping.rs`: `SkylineLdlt::factor_with(a,
&SkylineOptions { ordering: Rcm, pivots: PivotRule::NonZero, symmetry_tolerance: 1e-12 })` and `inertia().0`; a refusal mapped
to `FemError::InvalidValue { key: "stiffness", .. }` naming the node and component of the refused row.

- `WarpingHarmonic::negative_eigenvalues(&self) -> Result<usize>`: the `CsrMatrix` of the stored union pattern (`row_ptr`, `col`)
  with the values `k + g` (with the pre-stress) or `k` (without), then `sturm_negatives`.
- `WarpingTransient::negative_eigenvalues(&self) -> Result<usize>`: `sturm_negatives` of the integrator's stiffness (`K +
  K_sigma` from `with_prestress`, `K` from `new`), read through a new crate-private accessor `Integrator::stiffness(&self) ->
  &CsrMatrix` in `transient.rs` (D3).

### D2. What is not changed

Every existing function of `beam_warping.rs` and `transient.rs`, the fields of both solver types, `rcfd-linalg`.

### D3. Files

Changed: `src/beam_warping.rs` (the private `sturm_negatives`, the two methods, the module text), `src/transient.rs` (one
`pub(crate)` accessor; a shared solver file, so `baseline_hashes` is run). New test: `tests/sa_beam_warping_f668.rs`.

## Arithmetic done before the code (disclosed)

`arith.log`.

1. **Critical loads** (above): weak `3.662523e5` (16-element Hermite, Euler-Bernoulli) / `3.662516e5` (continuous) /
   `3.655734e5` (Timoshenko, `kappa G A = 1.974e8`), weak `m = 2` `1.465054e6` / `1.465006e6` / `1.454216e6`, strong `m = 1`
   `4.602024e6` / `4.602014e6` / `4.497190e6`; twist `1.281370e6`, `2.627037e6`, `4.869815e6`.
2. **Counts of the plane models** (eigenvalues of the reduced block matrices, 16 elements, Euler-Bernoulli): `+0.5`: (weak,
   strong, twist) `(0, 0, 0)`; `-0.25`: `(0, 0, 0)`; `-0.5`: `(1, 0, 0)`; `-1.2`: `(2, 0, 1)`; torsion-only `0, 0, 0, 1`.
3. **Buckling factors** of the compressive pre-stresses (critical load over `|N|`, Timoshenko estimate): `-0.5`: `0.5706`,
   `2.0000`, `2.2698`, `4.1004`; `-1.2`: `0.2378`, `0.8333`, `0.9457`, `1.7085`. The number below 1: 1 and 3.
4. **The transient construction** at `dt = T_1(-0.5)/400 = 9.214657e-5` (Newmark average acceleration): the smallest eigenvalue
   of `M + dt^2/4 (K + K_sigma)` over the largest of `M` is `2.9e-3` (weak), `1.9e-2` (strong), `3.3e-3` (twist) at `-0.5` and
   `2.9e-3`, `1.9e-2`, `3.3e-3` at `-1.2`: positive definite, so the integrator's pivot test accepts the effective matrix (the
   lowest `omega^2`: `-1.245e4` at `-0.5`, `-5.31e4` weak and `-1.16e4` twist at `-1.2`).

## Checks (limits fixed now)

The fork beam of the readings, 16 elements, the midspan torque, the pre-stress of `N = f |N_T|` with `f` as stated; the harmonic
solver `WarpingHarmonic::new(model, w, WarpingHarmonicOptions::new().prestress(p))`; the transient
`WarpingTransient::with_prestress(model, w, [the step torque of ADR 0591], TransientOptions::new(9.214657e-5), p)`. Counts are
integers and compared exactly.

| id | check | limit | estimate |
|---|---|---|---|
| B0 | the existing bit records (`sa_beam_warping_f598_bits`, `_f626_bits`, `_f635_bits`, `_f661_bits`, `_f663_bits`, `_f667_bits`) and `rcfd-api` `baseline_hashes` unchanged | equal (bit) | exact (no existing line's arithmetic changed) |
| C1- | **acceptance a1, a2**: harmonic, full beam, `f = -0.5` | `= 1` | 1 |
| C1+ | **acceptance a1, a2**: harmonic, full beam, `f = +0.5` | `= 0` | 0 |
| C2+- | **acceptance a3**: transient `with_prestress`, the same model and pre-stress as C1+-: equal to the harmonic count | `= 0`, `= 1` | 0, 1 |
| C3 | torsion-only model (bending and axial held; the pre-stress from the fork solve, as ADR 0585 H2T), `f = -0.5`, harmonic and transient | `= 0` | 0 |
| C4 | full beam `f = -0.25`: harmonic and transient | `= 0` | 0 |
| C4' | full beam `f = -1.2`: harmonic and transient | `= 3` | 3 |
| C4T | torsion-only `f = -1.2`: harmonic and transient | `= 1` | 1 |
| C5 | no pre-stress (`WarpingHarmonic` default options, `WarpingTransient::new`), full beam | `= 0` | 0 (K positive definite) |
| S1 | independent: `buckling_factors_with(fork, p, w, BucklingOptions::new(4))` at `f = -0.5` and `-1.2`: the number of factors `< 1` equal to C1- and C4' | equal | 1, 3 (factors as arithmetic 3) |
| R | the fork beam without the axial support (`u` free at node 0: an axial rigid motion, `K` singular), no pre-stress, harmonic and transient (below) | key `"stiffness"` | refused |

R, made concrete: the model `fork` with components `1, 2, 3` held at both ends and `u` not held (an unsupported axial motion),
no pre-stress: `WarpingHarmonic::new` (default options) and `WarpingTransient::new` construct (no factor of `K` alone is made at
construction: the transient factors `M + c K`, positive definite); `negative_eigenvalues` on each returns an error with the key
`"stiffness"`. Estimate: the axial block decouples exactly, so its last pivot in any order is a cancellation of `E A/h` terms to
a few `eps E A/h`, below `theta = (w + 1) eps (|a_ii| + sum)`. Risk disclosed: rounding could leave a pivot just above
`theta`; that would be a miss of R, not a change of the limit.

Negative controls: C1+ against C1- (the count sees the pre-stress), C5 (no pre-stress gives 0), C4/C4' (the count follows the
loads one by one through the derived margins), S1 (an independent eigen-solve agrees), R (a singular matrix is refused, not
counted).

## Planned mutations (on a scratch copy, one at a time, own `CARGO_TARGET_DIR`)

| id | mutation | expected caught by |
|---|---|---|
| Z1 | harmonic count of `k` only (the pre-stress ignored) | C1-, C4', C4T, C2- (harmonic and transient differ), S1 |
| Z2 | the positive pivots counted (`inertia().1`) | every count |
| Z3 | harmonic count of `k - g` | C1+ (tension becomes compression: 1), C1- |
| Z4 | `PivotRule::Positive` in `sturm_negatives` | C1- (a refusal), C4', C4T |
| Z5 | the transient method returns 0 | C2-, C4', C4T |
| Z6 | the refusal mapped to a count of 0 (`unwrap_or`) | R |

## Decision rule

**done**: B0, C1+-, C2+-, C3, C4, C4', C4T, C5, S1, R met, and every mutation caught. **partial**: any registered check missed (a
`*_known_miss` record), any mutation not caught, or a check met only after a post-run amendment. **todo**: C1- or C1+ missed, or
B0 missed.

## Limits (not covered)

- The count is of the linearised pre-stressed state: no statement on post-buckling or on a nonlinear path.
- A pre-stress with moments or shear (lateral-torsional coupling) is counted by the same code but not checked against a closed
  form; monosymmetric sections not checked.
- No count is reported for a numerically singular `K + K_sigma` (refused, key `"stiffness"`); no near-singular warning; no
  automatic report or refusal during `response` or `run` (the caller asks); no case-file key (F-625).
- The count is computed on demand by one more real factor of the reduced stiffness (cost of one static factor).

## Open questions (to be taken on the maintainer's standing instruction of 2026-11-11 when answered; none changes a default)

- **Q1. When to count.** (a) on demand, a method (this ADR; no default changed, no cost unless asked); (b) at construction,
  stored (a cost on every pre-stressed construction, and a construction that would have to fail or carry an error at a buckling
  load). Recommendation (a).
- **Q2. A singular matrix.** (a) refuse with key `"stiffness"` (this ADR); (b) report the count of the pivots below `-theta` and
  a flag for the near-zero ones. Recommendation (a): a count with a zero pivot is not an inertia.

## Addendum 1 (2026-11-15): results

Written after the one registered run of `sa_beam_warping_f668.rs` (log `f668-scratch/run1.log`, code commit 31eaf3c2) and the
mutation runs (`mutations.log`, `mut_Z*.log`). The numbers are those the tests printed (release). No limit was changed, no
amendment was written, and nothing was re-run to pick a result. Two commits after the run touch no result: 26dfb55a restores the
line continuations of the refusal message (the registered run printed the message with the source indentation inside it), and
ee5b6ff0 writes `map_or(false, ..)` of S1 as `is_ok_and(..)` for clippy (the same boolean).

### Results against the registered limits

| id | measured | limit | verdict |
|---|---|---|---|
| B0 | the bit records `sa_beam_warping_f598_bits`, `_f626_bits`, `_f635_bits`, `_f661_bits`, `_f663_bits`, `_f667_bits` pass unchanged; `rcfd-api` `baseline_hashes` 3 passed, 1 ignored (as before) | equal | met |
| C1- (acceptance) | harmonic, full beam, `N = -0.5 |N_T|`: 1 | `= 1` | met |
| C1+ (acceptance) | harmonic, full beam, `N = +0.5 |N_T|`: 0 | `= 0` | met |
| C2- / C2+ (acceptance a3) | transient `with_prestress`: 1 / 0, equal to the harmonic | `= 1` / `= 0` | met |
| C3 | torsion-only `-0.5`: 0 (harmonic and transient) | `= 0` | met |
| C4 | full `-0.25`: 0 / 0 | `= 0` | met |
| C4' | full `-1.2`: 3 / 3 | `= 3` | met |
| C4T | torsion-only `-1.2`: 1 / 1 | `= 1` | met |
| C5 | no pre-stress: 0 / 0 | `= 0` | met |
| S1 | factors at `-0.5`: `0.570602, 2.000001, 2.270067, 4.100448` (1 below 1, count 1); at `-1.2`: `0.237751, 0.833334, 0.945861, 1.708520` (3 below 1, count 3) | equal | met |
| R | harmonic and transient refused, key `"stiffness"`, pivot exactly `0` at node 0 component 0 against `theta = 1.6e-5` | key `"stiffness"` | met |

The buckling factors agree with the Timoshenko estimates of arithmetic 3 to four figures (`0.5706`, `2.0000`, `2.2698` against
`2.270067` measured, `4.1004`; `0.2378`, `0.8333`, `0.9457` against `0.945861`, `1.7085`): the weak `m = 2` load at `N = -1.2
|N_T|` is beyond its load by 5.4 % as derived, so the count 3 is not a near miss of the rounding.

### Mutations (scratch copy `f668-scratch/mut`, own target dir, one at a time)

| mutation | result | caught by |
|---|---|---|
| Z1 harmonic count of `k` only | caught | C1-, C4', C4T (harmonic 0), the transient-equals-harmonic lines, S1 |
| Z2 positive pivots counted | caught | every count (112 of 112 free equations, 32 on the torsion-only model) |
| Z3 harmonic count of `k - g` | caught | C1+ (1), C1-, C4', C4T, S1 |
| Z4 `PivotRule::Positive` | caught | C1-, C2-, C4', C4T, S1 (refused at the first negative pivot) |
| Z5 transient returns 0 | caught | C2-, C4', C4T (transient) |
| Z6 refusal mapped to 0 | caught | R (both) |

Six of six caught.

### Deviation from the registered design

D2 said the fields of both solver types are not changed. `WarpingTransient` has one new private field, `base_dofs` (the model's
`DofMap`, set in both constructors), so that the refusal names the node and component of the refused row as the harmonic path
does. It is read only by `negative_eigenvalues`; no arithmetic of the constructors, `start`, `run` or the results is touched (B0
and the `_f667` tests pass unchanged). `transient.rs` has the one registered `pub(crate)` accessor `Integrator::stiffness`.

### Other runs

`cargo test -p rcfd-fem --release` for `sa_beam_warping_f598_bits`, `_f626_bits`, `_f635`, `_f635_bits`, `_f661`, `_f661_bits`,
`_f663_bits`, `_f667`, `_f667_bits`: all passed (ignored records stay ignored); `cargo test -p rcfd-api --release --test
baseline_hashes`: 3 passed, 1 ignored. `cargo clippy -p rcfd-fem --release --all-targets -- -D warnings`: clean (after
ee5b6ff0). `cargo fmt -p rcfd-fem --check`: clean. The whole rcfd-fem suite was not run (the lead runs the gate).

### Decision

**Done** by the registered rule: every registered check met in the one run, every mutation caught, no amendment. By clause: a1
met (the method counts the reduced `K + K_sigma` by the real `L D L^T` factor; Z1, Z3 show the pre-stress enters); a2 met (1 under
compression, 0 under tension, the 1 derived from the Euler and torsional loads and confirmed independently by S1); a3 met (the
transient count equal in all six pre-stressed cases); a4 met (B0; nothing existing computes differently).

### Remaining questions

- Q1 (when to count): (a) on demand, as taken; (b) at construction. Recommendation (a).
- Q2 (a singular matrix): (a) refused, key `"stiffness"`, as taken (R); (b) a count with a near-zero flag. Recommendation (a).
- Q3 (case-file and report exposure): the count is library API only. Options: (a) leave it until F-625 offers the warping
  solves in case files; (b) add it now to a report. Recommendation (a): no case-file path to the warping solvers exists yet.

## For the lead: roadmap entries

> Proposed status: done.

> Structural solver, advanced tier. Follow-up of F-661 and F-667 (ADR 0585, ADR 0591, 2026-11-15). DONE. ADR 0592, 2026-11-15: the warping harmonic and transient solvers report the number of negative eigenvalues of the reduced stiffness at omega = 0 (`WarpingHarmonic::negative_eigenvalues`, `WarpingTransient::negative_eigenvalues`; K + K_sigma with a pre-stress, K without), a Sturm count of the real L D L^T factor (reverse Cuthill-McKee, no pivoting, the options of count_below at the shift 0; Sylvester's law of inertia), computed when asked; a numerically singular stiffness is refused (key "stiffness"). Fork-supported I beam, 16 elements: N = -0.5 of the torsional buckling load counts 1 (the weak-axis m = 1 bending mode, beyond its load at 0.285 of N_T), N = +0.5 counts 0, harmonic and transient alike; also -0.25 counts 0, -1.2 counts 3 (weak m = 1, 2 and the torsional mode), the torsion-only model 0 and 1, and the factors below 1 of the warping buckling solve agree. Defaults bit-identical (six warping bit records and baseline_hashes unchanged). Six of six mutations caught. No case-file key, no automatic report during a solve.
