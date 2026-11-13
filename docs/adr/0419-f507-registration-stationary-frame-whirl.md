# ADR 0419: F-507 registration: stationary-frame frequencies of a cyclic whirl table (the wave sense of a sector mode, the frequency omega + tau k Omega seen by a stationary observer, checked against the full rotor's modes whose nodal-diameter index and sense are read from their own eigenvectors)

Status: **REGISTERED 2026-11-13, written before any code of F-507 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-507 was written, compiled or run: the code listed under "What was read" was read, and
the arithmetic below was done with one scratch script (`f507-scratch/arith.py`, plain Python on the numbers printed by ADR
0411's registered run `f499-scratch/run1.log`, lines 86 to 110; it does not call rcfd; disclosed under "Arithmetic"). Results
go in the addendum at the end; nothing above the addendum is changed after a run.

Parent: [ADR 0411](0411-f499-registration-coriolis-gyroscopic-whirl-modes.md) (F-499: the Coriolis matrix `G = 2 Omega C`, the
whirl modes of `(K_eff - omega^2 M + i omega G) phi = 0`, the sector whirl modes of index `k` with the wave sense `tau` of D5,
`RotatingSector::whirl_campbell`, the disc sector DS against its full rotor; its limit "frequencies in the stationary frame ...
not computed here" and remaining question 3, recommendation (b): a column of the cyclic table). [ADR
0416](0416-f505-registration-gyroscopic-stabilisation.md) (F-505, `rotating/whirl/state.rs`) is not touched and its behaviour
does not change. Roadmap item F-507 (dep F-499), size S. Number 0419 given by the lead. Branch `f507-stationary-frame-whirl`
(base `ba017b70`), worktree `w49i-work/f507`, scratch `w49i-work/f507-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here before the code and the runs and not changed after a run (a miss
becomes an ignored `*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, the hash tables or a golden file; no new dependency, no unsafe code; clean-room.

Library edits, additive only and confined to the rotating module (another branch, F-391 / ADR 0417, changes the `DofMap` of
`rcfd-fem`; nothing here touches it). In `crates/rcfd-fem/src/rotating/whirl.rs`: one new public function
`stationary_frequency`, one new public field `stationary` of `CyclicWhirlCampbell` (filled in `RotatingSector::whirl_campbell`
after the families are built, from their stored values only) and one new method `CyclicWhirlCampbell::stationary_csv`. In
`crates/rcfd-fem/src/rotating.rs`: the name `stationary_frequency` added to the `pub use whirl::{...}` list. Nothing else:
`track_whirl`, `WhirlCampbell` (its fields, `to_csv`, `to_json`, `crossings`), `RotatingSector::whirl_modes`, the F-396 and
F-500 tables and everything of `state.rs` are unchanged, so every existing output is bit-identical (justified under D4). Tests: a
new file `crates/rcfd-fem/tests/si_rotating_whirl_stationary.rs`; no existing test is edited.

## What F-507 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. The cyclic whirl Campbell table carries for each mode of
> index k the stationary-frame frequency omega + tau k Omega (tau the wave sense); on the disc sector of ADR 0411 the stationary
> frequencies of a travelling pair agree with those of the full rotor's modes transformed to the stationary frame within 1e-8
> relative.

Clauses: (c1) every limit not stated in the acceptance registered here before the run; (c2) the cyclic whirl Campbell table
carries, for each mode of index `k`, the stationary-frame frequency `omega + tau k Omega`; (c3) on the disc sector DS of ADR
0411, the stationary frequencies of a travelling pair agree with those of the full rotor's modes transformed to the stationary
frame within 1e-8 relative. The brief adds: (c4) the transformation of a full-rotor mode identifies its nodal-diameter content
and sense from the full eigenvector, not from the sector result, so that the comparison is independent; (c5) the existing
outputs stay bit-identical and no existing test is edited.

## What was read (branch base ba017b70, read only)

- `AGENT_RULES.md`; the brief; ADR 0411 (all, with its addendum); ADR 0416 (head, interfaces).
- `crates/rcfd-fem/src/rotating/whirl.rs` (lines 1 to 140, 664 to 1296: `WhirlMode` with `wave`, `RotatingPart::whirl_at`,
  `RotatingSector::whirl_at` with the wave sense, `whirl_campbell`, `track_whirl`, `WhirlCampbell`, `CyclicWhirlCampbell`);
  `crates/rcfd-fem/src/rotating.rs` (the `pub use` list, `RotatingOptions`, `CampbellTable::to_csv`, `CyclicCampbell::to_csv`);
  `crates/rcfd-fem/src/cyclic.rs` (`rotation_matrix` (Rodrigues, right-handed), `boundary`, `FullRotor::{node, expand}`,
  `full_rotor`: copy `j` rotated by `+j 2 pi / N` about the constraint's axis, each full node written once from the copy in
  which it is not a high node); the grep `CyclicWhirlCampbell` over `crates/rcfd-fem/src` (built only in
  `RotatingSector::whirl_campbell`; `state.rs` does not build it).
- Tests: `si_rotating_whirl.rs` (lines 1 to 70, 485 to 760: DS, `full_of`, `ds_options`, `omega_ref`, S1 to S5).
- `f499-scratch/run1.log` (lines 86 to 111: `omega_ref`, the sector frequencies, whirl numbers and wave senses at `0.4` and
  `0.8 omega_ref`).

## Design

Notation of ADR 0411: speed `Omega > 0` about the unit axis `a` (the part turns in the right-handed sense about `a`: the
velocity of the frame point `x` is `Omega a x x`, ADR 0403 D1); `N` sectors of angle `alpha = 2 pi / N`; copy `j` of the
sector is the sector rotated by `theta_j = j alpha` about `a` (`full_rotor`, `R^j`); a mode `u = Re(phi e^{i omega t})`,
`omega > 0`, in the rotating frame.

### D1. The wave sense of a sector mode (first principles)

A sector mode of the pair index `k` (`0 < 2k < N`) is the pair `(a, b)` of complex sector fields (cosine and sine parts; `a =
a_r + i a_i` from the stored `re`, `im` of the first half, `b` from the second half). Its field on copy `j`, in the copy's own
components (the components that `R^j` maps to the global ones; for an axisymmetric rotor these are the cylindrical components up
to a fixed rotation), is (ADR 0386, `FullRotor::expand`)

`u~_j = cos(j k alpha) a - sin(j k alpha) b = e^{-i j k alpha} f + e^{+i j k alpha} g`, `f = (a - i b)/2`, `g = (a + i b)/2`

(from `cos x = (e^{ix} + e^{-ix})/2`, `sin x = (e^{ix} - e^{-ix})/(2i)`). The part `f` has the copy dependence `e^{-i j k alpha}`:
its real field `Re(f e^{i(omega t - k theta_j)})` keeps its phase on `k theta - omega t = const`, a pattern that travels in `+theta`,
the sense of `Omega`, at the angular speed `omega / k`. The part `g` travels against it. With the sector mass `M` (the full
mass on the `N` copies is the sum of the copies' masses, and the `e^{-/+ i j k alpha}` parts are orthogonal over the copies, so
the sector norms are the full energies divided by `N/2`):

`f^H M f - g^H M g = Im(a^H M b)`, `f^H M f + g^H M g = (a^H M a + b^H M b) / 2`

(expand `(a -/+ i b)^H M (a -/+ i b) = a^H M a + b^H M b +/- 2 Im(a^H M b)`, `M` real symmetric). Hence the wave sense stored by
ADR 0411 D5,

`tau = 2 Im(a^H M b) / (a^H M a + b^H M b) = (|f|_M^2 - |g|_M^2) / (|f|_M^2 + |g|_M^2)`, in `[-1, 1]`,

is `+1` for a pure forward-travelling wave (`g = 0`, `b = i a`), `-1` for a pure backward one (`f = 0`, `b = -i a`) and the
energy balance in between. ADR 0411 D5 showed that with `G != 0` the reduced pair problem commutes with `J: (a, b) -> (-b, a)`
and its eigenvectors are the eigenvectors of `J`, `b = -/+ i a`: a split (non-degenerate) pair mode is a pure travelling wave,
`|tau| = 1` (ADR 0411 S3: within 4.4e-16).

### D2. The frequency seen in the stationary frame

A stationary observer at the angle `theta_s` sees the material point at the rotating-frame angle `theta = theta_s - Omega t`. The
copy-local components of a vector at a material point are a fixed rotation of its cylindrical components, which are the same in
both frames (the radial direction of a material point is one physical direction at each instant). So the forward part becomes
`Re(f e^{i(omega t - k (theta_s - Omega t))}) = Re(f e^{i((omega + k Omega) t - k theta_s)})`: a stationary frequency `omega + k
Omega`, and the backward part `omega - k Omega`. For a pure travelling wave of sense `tau = +/-1`:

`omega_stat = omega + tau k Omega`,

the clause c2. It is signed: a negative value (a backward wave with `k Omega > omega`) is seen at `|omega_stat|` travelling
forward in the stationary frame; the table keeps the sign, so that the formula and the sense are both kept. For `|tau| < 1`
(only a degenerate pair, that is Coriolis off or `Omega = 0`) a stationary observer sees the two lines `omega -/+ k Omega` with
the energy weights `|g|^2`, `|f|^2`; `omega + tau k Omega` is then their energy-weighted mean (D1), which is what the column
holds; at `Omega = 0` it is `omega`. For the single indices: at `k = 0` the term `tau k Omega` is 0 (no wave), `omega_stat =
omega`; at `2k = N` the copy dependence `(-1)^j` is the same for `e^{-/+ i j k alpha}`, the mode is a standing wave with equal
weights, and the mean is `omega` (`tau` taken as 0; ADR 0411 stores `wave = None` there). So for every mode of index `k`:
`omega_stat = omega + tau k Omega` with `tau := 0` where the wave is `None`.

Aliasing (stated, not computed): the sector field carries the circumferential harmonics `k + m N` (`m` integer) of the
continuous angle; a stationary observer at a point sees also the lines `omega + (k + mN) Omega` of the harmonics `m != 0`. The
column holds the line of the index itself, as the acceptance states; the others are a limit.

### D3. The full rotor's modes in the stationary frame, independently of the sector

For a full-rotor whirl mode `phi` (rotating frame, `RotatingPart::whirl_modes` on the full mesh with Coriolis, as ADR 0411 S1),
its discrete circumferential content is read from `phi` alone. For each sector node `p` that is not a high node and each copy
`j`, the full node is `n = rotor.node(j, p)` (each full node exactly once); the copy-local vector is `u~(p, j) = (R^j)^T
phi_n` (`R^j = rotation_matrix(a, j alpha)`, three complex components). Its discrete Fourier coefficients over the copies

`U_q(p) = (1/N) sum_j u~(p, j) e^{+i q theta_j}`, `q = 0 .. N - 1`,

and the content `E_q = sum_p |U_q(p)|^2` (Euclidean; by Parseval `sum_j |u~(p, j)|^2 = N sum_q |U_q(p)|^2`, so the `E_q` split
the squared norm of `phi` over the free and fixed components). The signed index `q* in (-N/2, N/2]` (`q` for `q <= N/2`, `q - N`
otherwise) of the largest `E_q`, and its fraction `E_q* / sum_q E_q`. By D1 a forward-travelling field of index `k` (`u~ =
e^{-i j k alpha} f`) has all its content at `q = +k` and a backward one at `q = -k`. The full rotor commutes with the rotation by
`alpha` (its mesh is `N` exact copies, the axis that of the copies), so a non-degenerate full mode is an eigenvector of the shift
and lies in one `q` to rounding. The stationary frequency of the full mode is then, by D2 applied to the full field,

`omega_stat,full = omega_f + q* Omega`,

computed with no sector quantity: `q*` (magnitude and sign) from the full eigenvector, `omega_f` its rotating-frame
eigenvalue. This is the transformation of c3 and c4. The Euclidean content is used, not the mass-weighted one, so it does not
share the norm of `tau` (any shift-invariant norm splits orthogonally over `q`; for a pure mode both give one `q`).

**Pairing** (the only place the sector enters, and it does not use the sense): a travelling sector mode of index `k` at the
speed `Omega` with rotating frequency `omega_s` is paired with the full mode of the same speed whose `|q*| = k` and whose
`omega_f` is nearest `omega_s`. The comparison `omega_stat,sector` against `omega_stat,full` then tests the magnitude `k`, the
sense (`tau` against the sign of `q*`) and the frequency together.

### D4. Interfaces and why the existing outputs stay bit-identical

All new, in `rotating/whirl.rs`, re-exported from `rotating`:

- `pub fn stationary_frequency(omega: f64, harmonic: usize, wave: Option<f64>, speed: f64) -> f64`: `omega + tau harmonic
  speed`, `tau = wave.unwrap_or(0.0)` (D2), rad/s in, rad/s out.
- `CyclicWhirlCampbell::stationary: Vec<Vec<Vec<Option<f64>>>>`, per family, track and speed, Hz: `Some(f + tau k Omega / (2
  pi))` where the track has a frequency `f` (Hz) at that speed, with `tau` the stored `waves[t][j]` (`0` where `None`), `k` the
  family's index, `Omega` the speed; `None` where the track has no mode. At `Omega = 0` and on single indices the added term is
  zero and the entry has the bits of `f`.
- `CyclicWhirlCampbell::stationary_csv(&self) -> String`: header `harmonic,speed_rad_s,speed_rpm` then `track_t_stationary_hz`
  for `t` up to the widest family; one row per family and speed; values in shortest round-trip form, empty where `None` and in
  the padding of a narrower family.

Bit-identity (c5): the families are built by `track_whirl` exactly as before and the new field is computed afterwards from their
stored values only; `WhirlCampbell` and all its outputs, `RotatingSector::whirl_modes`, `RotatingPart`, the F-396 and F-500
tables and `state.rs` are not touched. `CyclicWhirlCampbell` had no `to_csv` or `to_json`; it is `#[non_exhaustive]` (no
code outside the crate can build it, so a new field breaks nothing) and its only constructor is `RotatingSector::whirl_campbell`.
Its derived `Debug` text gains the field; `Debug` is not an output of record (no golden, hash or test reads it: grep). The new
column is therefore added without an opt-in switch, by this justification.

## Set-up (fixed now)

DS of ADR 0411 exactly (its test helpers copied into the new test file): steel `E = 2e11 Pa`, `nu = 0.3`, `rho = 7850 kg/m^3`;
axis `e_z` through the origin; `N = 6`; the Hex20 annular sector from `hex_box([0.25, 0, -0.1], [0.75, alpha, 0.2], [2, 1,
1])`, `xmin` clamped, cyclic `ymin`/`ymax`; `ds_options(true)` (4 modes, modal tolerance 1e-9, 400 iterations, both effects,
Coriolis on); `omega_ref` the lowest rest frequency over `k = 0 .. 3`; `rs.whirl_campbell(&[0, 1, 2, 3], speeds)` with `speeds
= [0, 0.2, 0.4, 0.6, 0.8] x omega_ref` (ADR 0411 S5). The full rotor `full_of(sector)` with `xmin` clamped, `RotatingPart::new`
with the options of `ds_options(true)` except `modes = n_free - 1` (all but the highest mode of the 252 free unknowns, so that
every sector mode has its partner among the returned modes; `n_free` counted from `prescribed_equations` before the solve), at
the table's own speeds `speeds[2]` and `speeds[4]` (the same bits).

"Travelling" (registered reading of "a travelling pair"): a track of the families `k = 1, 2` at `speeds[2]` or `speeds[4]` with
`| |tau| - 1 | <= 1e-6` (the S3 limit of ADR 0411).

Relative deviation (registered, the denominator of "1e-8 relative"): `|s_sector - s_full| / max(|s_full|, 1e-3 omega_f)`; the
floor keeps the measure meaningful if a backward line falls near zero; by the arithmetic below it is not active (smallest
`|s| / omega = 0.099`).

## Arithmetic (done before the code; disclosed)

On the numbers of ADR 0411's run (`omega_ref = 1655.322604 rad/s`; indices 1 and 2 at `0.4` and `0.8 omega_ref`, 8 modes each,
all with `|tau| = 1` to 4.4e-16, signs `+ - + - + - - +` for `k = 1` and `+ - + - - + + -` for `k = 2` at both speeds):

1. The 32 stationary frequencies `omega + tau k Omega` range from `291.18 rad/s` (`k = 2`, `0.8 omega_ref`, the backward mode
   at `2939.70`) to `14802.75 rad/s`; the smallest `|s| / omega` is `0.0991`, so the floor `1e-3 omega_f` is not active.
2. The negative controls: with the opposite sense (`omega - tau k Omega`) the smallest relative deviation over the 32 modes is
   `0.100`; with no transformation (`omega` itself) `0.050`.
3. Expected deviation of c3: the sector and full-rotor rotating frequencies agree to `3.0e-13` (ADR 0411 S1); `k Omega` is
   exact on both sides (the same bits of `Omega`, `q* = +/- k` exact integers), so `|s_s - s_f| <= 3e-13 omega <= 3e-13 x 13100
   rad/s = 4e-9 rad/s`, relative to `|s| >= 291 rad/s`: `<= 1.4e-11`. Estimate `<= 1e-11` against the limit 1e-8.
4. Purity of the full modes: the dense solver's eigenvector error is `~ eps ||H|| / gap`; `||H|| <= ~1e6 rad/s` (the highest of
   252 Hex20 frequencies, an upper guess), so `~2e-10 rad/s / gap`; with any gap above `1 rad/s` between modes of different `q`
   the mixed amplitude is `<= 2e-10` and the content defect `<= 4e-20`. An exact accidental degeneracy of two different `q` is
   not excluded by symmetry; it would show as a content fraction below the limit (a miss, diagnosed, not tuned).

## Checks (limits fixed now)

| # | Clause | Check | Limit | Estimate behind it |
|---|---|---|---|---|
| T0 | c1 | this registration committed alone before any code | holds | rule 1 |
| T1 | c2 | DS `whirl_campbell([0, 1, 2, 3], speeds)`: `stationary` has 4 families, each with the family's track count and 5 speeds; `Some` exactly where `frequencies` is `Some`; every entry by `to_bits` equal to `f + tau k Omega / (2 pi)` recomputed in the test from the stored `frequencies`, `waves`, `harmonics` and `speeds` (`tau = 0` for `None`); at `Omega = 0` and for `k = 0, 3` equal to `f` by bits | exact (each) | by construction (D4) |
| T2 | c2 | `stationary_csv`: `1 + 4 x 5` lines; header `harmonic,speed_rad_s,speed_rpm,track_0_stationary_hz .. track_7_stationary_hz`; every value parses back to the bits of `stationary`; empty fields exactly at `None` and in the padding of the families `k = 0, 3` (4 tracks of 8) | exact | shortest round-trip |
| T3 | c3 | the travelling tracks (Set-up) of `k = 1, 2` at `speeds[2]`, `speeds[4]`: `max |s_sector - s_full| / max(|s_full|, 1e-3 omega_f)`, `s_sector = 2 pi x stationary` of the table, `s_full = omega_f + q* Omega` of the paired full mode (D3) | `<= 1e-8`; travelling count `>= 24` (of 32) | `<= 1e-11` (arithmetic 3); count 32 (arithmetic) |
| T4 | c4 | every paired full mode: content fraction of `q*` `>= 1 - 1e-8`; no full mode paired twice within one `(k, speed)`; the rotating frequencies of each pair agree, `|omega_f / omega_s - 1| <= 1e-8` | each | `<= 4e-20`; `3e-13` (ADR 0411 S1) |
| T5 | c4 | sense from the full eigenvector against the sector: `sign(q*) == sign(tau)` for every pair | 0 disagreements | D1, D3 |
| T6 | D2 | `stationary_frequency`: `(10, 2, Some(1), 3) = 16`, `(10, 2, Some(-1), 3) = 4`, `(10, 2, None, 3) = 10`, `(10, 0, Some(1), 3) = 10`, `(10, 1, Some(0.5), 4) = 12` | exact (each) | exact in binary |
| T7 | c5 | the existing tests of `rcfd-fem` pass unchanged (`cargo test -p rcfd-fem --release`, with `si_rotating_whirl` and `si_rotating_whirl_state`); the diff of `whirl.rs` and `rotating.rs` is additive (no line removed or changed outside the new items and the `pub use` list and the struct literal of `CyclicWhirlCampbell`) | each | D4 |

Negative controls (expected to fail the property, each with a limit):

| # | Control | Expected |
|---|---|---|
| N1 | T3 with `omega - tau k Omega` for the sector (the opposite sense) | deviation `> 1e-2` (arithmetic 2: `0.100`) |
| N2 | T3 with the rotating frequency `omega` for the sector (no transformation) | deviation `> 1e-2` (arithmetic 2: `0.050`) |
| N3 | T5 with the full content read with the opposite exponent `e^{-i q theta_j}` | every pair disagrees (32 of 32 if 32 are travelling) |

Planned mutations (on a `git archive` copy in `f507-scratch`, each run once against `si_rotating_whirl_stationary`; what catches
each is recorded):

1. The table's column with the opposite sense, `f - tau k Omega / (2 pi)`: T1, T3.
2. The index dropped, `f + tau Omega / (2 pi)`: T1, T3 (index 2).
3. The wave sense dropped, `f`: T1, T3.
4. The speed of the first column (`speeds[0] = 0`) used at every speed: T1, T3.
5. `stationary_csv` writing the rotating frequency instead of the stationary one: T2.
6. `stationary_frequency` returning `omega - tau k Omega`: T6.

## Decision rule

- **done**: T0 to T7 met and N1 to N3 as expected.
- **partial**: T1 and T3 met (clauses c2 and c3) with one other check missed and its cause diagnosed (a `*_known_miss`
  record), or a mutation not caught, or a check met only after an amendment made after its run.
- **todo**: T1 or T3 missed.

## Limits (what is not covered)

The aliased lines `omega + (k + mN) Omega`, `m != 0`, that a stationary point sensor also sees (D2); the two lines of a
standing wave (single index `2k = N`, or a degenerate pair with Coriolis off) are reported by their energy-weighted mean, not
separately; the whole-part `WhirlCampbell` (no index, no stationary column); speeds above the loss of stiffness (the state-matrix
analysis of ADR 0416 has no sector form); output through case files and the CLI; one disc geometry.

## Addendum 1: results (2026-11-13)

Code (commit `e96ebcc0`): `crates/rcfd-fem/src/rotating/whirl.rs`, additive only (lines 1014 to 1018 the column built in
`RotatingSector::whirl_campbell`, 1024 the field in its struct literal, 1029 to 1056 `stationary_frequency` and
`stationary_column`, 1328 to 1358 the field `stationary` and `CyclicWhirlCampbell::stationary_csv`); `crates/rcfd-fem/src/rotating.rs`
lines 46 to 49, the `pub use` list with `stationary_frequency` (re-wrapped by rustfmt). Test: `crates/rcfd-fem/tests/si_rotating_whirl_stationary.rs`
(2 tests, 0.8 s in release). The registered run was made once (`f507-scratch/run1.log`), before `cargo fmt` re-wrapped two
lines (layout only); nothing was re-run to pick a result.

| # | Check | Limit | Measured | Met |
|---|---|---|---|---|
| T0 | registration committed alone first | holds | `c457ea6b`, before any code | yes |
| T1 | the column: shape, `Some` pattern, bits of `f + tau k Omega / (2 pi)`, `f` at rest and on `k = 0, 3` | exact | holds | yes |
| T2 | `stationary_csv`: 21 lines, header, round-trip, empty fields | exact | holds | yes |
| T3 | travelling tracks against the full rotor's `omega_f + q* Omega` | `<= 1e-8`; count `>= 24` | 7.2e-13; 32 | yes |
| T4 | content fraction of `q*`; no double pairing; rotating frequencies of the pairs | `1 - f <= 1e-8`; holds; `<= 1e-8` | 0 (the fraction rounds to 1); holds; 3.0e-13 | yes |
| T5 | `sign(q*) == sign(tau)` | 0 disagreements | 0 of 32 | yes |
| T6 | `stationary_frequency` point values | exact | holds (5 of 5) | yes |
| T7 | `cargo test -p rcfd-fem --release` (existing tests unchanged, incl. `si_rotating_whirl`, `si_rotating_whirl_state`); additive diff | each | 312 passed, 0 failed, 61 ignored; diff additive (hunks above) | yes |
| N1 | opposite sense | `> 1e-2` | 0.1005 | as expected |
| N2 | no transformation | `> 1e-2` | 0.0502 | as expected |
| N3 | opposite exponent in the full content | all disagree | 32 of 32 | as expected |

The full rotor has 342 equations, 252 free (251 modes requested). Smallest stationary frequency compared: the `k = 2` backward
mode at `0.8 omega_ref`, as arithmetic 1 predicted; the floor `1e-3 omega_f` was never active.

Mutations (on a `git archive` copy of `e96ebcc0` in `f507-scratch/mut`, separate target, each run once; logs
`f507-scratch/mut_*.log`): all six caught.

| Mutation | Caught by |
|---|---|
| 1 opposite sense in the column | T1, T3 (18.2) |
| 2 index dropped | T1, T3 (4.5, from `k = 2`) |
| 3 wave sense dropped | T1, T3 (9.1) |
| 4 speed of the first column | T1, T3 (9.1) |
| 5 CSV writes the rotating frequency | T2 |
| 6 `stationary_frequency` with the opposite sense | T6 |

`cargo fmt -p rcfd-fem --check`: clean (after one `cargo fmt` of the two new or edited files, layout only, before the code
commit). `cargo clippy -p rcfd-fem --all-targets -- -D warnings`: clean. `baseline_hashes` not run: no shared solver file is
touched (only the rotating module, used by no crate outside `rcfd-fem`, grep), and no default output changes.

Deviations: none from the registered set-up, limits or checks. Not run: anything listed under Limits.

**Decision by the registered rule: done.** T0 to T7 met, N1 to N3 as expected, all mutations caught, no amendment.

### Remaining questions

1. The aliased lines `omega + (k + mN) Omega` and the two lines of a standing wave. Options: (a) leave them out, as now; (b) a
   per-mode list of stationary lines with their energy weights from the sector's harmonic content. Recommendation: (a) until a
   rotor-stator excitation analysis needs them; then (b) as its own item.
2. The whole-part `WhirlCampbell` has no stationary column (no index). Options: (a) none; (b) an index read from the
   eigenvector by D3 when the part is cyclic-symmetric. Recommendation: (a); the sector route covers the fan.

### For the lead: roadmap entries

F-507, proposed status **done**. New note:

> From ADR 0411 question 3, recommendation taken on the maintainer's standing instruction of 2026-11-11. Size S. ADR 0419, 2026-11-13: the cyclic whirl Campbell table carries CyclicWhirlCampbell::stationary (Hz, f + tau k Omega / 2 pi, tau the wave sense of ADR 0411 D5 shown to be the forward-minus-backward energy balance, 0 on single indices) with stationary_csv and stationary_frequency; existing outputs unchanged; on the disc sector the 32 travelling pair modes of indices 1 and 2 at 0.4 and 0.8 omega_ref agree within 7.2e-13 with the full rotor's omega_f + q Omega, q (index and sense) read from the full eigenvectors by their discrete Fourier content over the copies (pure to rounding, sense agreeing 32 of 32); aliased lines omega + (k + mN) Omega not reported.
