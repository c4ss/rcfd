# ADR 0501: F-564 registration: damped critical speeds as unbalance-response peaks and checked Campbell crossings of the flexible rotor of ADR 0476

Status: **REGISTERED 2026-11-14, written before any code of F-564 and before any run of its checks**. Not yet accepted by the
maintainer. Results go in the addendum at the end; nothing above the addendum is changed after a run.

Roadmap item F-564 (deps F-416), structural solver, the second follow-up proposed by ADR 0476. Number 0501 given by the lead.
Branch `f564-work` from 059d2cb6 (the head of the F-563 branch, ADR 0498), worktree `w49i-work/f564`, scratch
`w49i-work/f564-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical (new behaviour only through new functions); no edit of `baseline_hashes.rs`, of the hash tables or of any golden
file; no existing test edited; no new dependency, no unsafe code; clean-room. Every choice below that the maintainer would
normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation), except the
open questions at the end.

## What F-564 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. On the damped Jeffcott rotor the response peak equals w_n / sqrt(1 - 2 zeta^2) within 1e-6 and the crossing of the forward track with engine order 1 equals the undamped critical speed within 1e-6 at zeta = 0.

Clauses: **c1** damped Jeffcott rotor, unbalance-response peak speed `= w_n / sqrt(1 - 2 zeta^2)` within 1e-6; **c2** at `zeta = 0`
the crossing of the forward track with engine order 1 equals the undamped critical speed within 1e-6. The title adds a
library function that finds the peaks and a checked crossing.

## What was read (base 059d2cb6, read only)

ADR 0476 and ADR 0498 (all of both), `crates/rcfd-fem/src/rotor.rs` (`unbalance_response`, `harmonic_response`, `whirl`,
`campbell`, `critical_speeds`, `critical_speeds_over`, `orbit`), `crates/rcfd-fem/src/rotating/whirl.rs` (`WhirlCampbell`,
`whirl_order_crossings`, `crossings`), `crates/rcfd-fem/tests/sa_rotor_speed.rs` and `sa_rotor_speed_default_bits.rs`.

What exists: `unbalance_response(spin, unbalances)` applies `F = amount spin^2 e^{i angle}` (force proportional to `spin^2`,
rotating) at the spin, so the response is a function of `spin` along the synchronous line `omega = spin`.
`WhirlCampbell::crossings(order)` (ADR 0411 D4) takes each track's `omega(Omega)` at the table speeds and interpolates it
**linearly in `Omega`** between two table speeds; that is exact only when `omega` is linear in `Omega` on the interval (a
constant `omega`, as for the Jeffcott rotor, is). For a gyroscopic rotor `omega(Omega)` is curved and the interpolated crossing
has an error of the order of the grid step squared; it is not checked against the frozen problem. `critical_speeds()` (ADR 0476
D6) is the undamped synchronous problem; each entry has a direction. F-563 added `critical_speeds_over` (bisection with the
frozen problem): reused here only as the pattern; its roots are of `crit_j - Omega`, not of a track.

## Arithmetic done before the registration (disclosed)

By hand, no solver run: the derivation of D1 below; for `k_s = 1.4647e6 N/m` (ADR 0498, cantilever 0.5 m, `r = 0.025`, `E = 2e11`,
`nu = 0.3`, `kappa = 0.9`), `B0 = 2e6`, `m = 20`: `k = 3.4647e6`, `w_n = sqrt(k/m) = 416.2` rad/s. No probe of the new functions
was run before this registration; the tolerances below come from the derivation (a golden-section search on a smooth function is
limited by `sqrt(eps)` of the value, about `1.5e-8` of the peak width, which is of order `zeta`).

## Design

### D1. The physics: which excitation gives which peak (hand derivation)

Jeffcott rotor: mass `m` on an isotropic support of stiffness `k` and damping `c`, `w_n = sqrt(k/m)`, `zeta = c/(2 m w_n)`,
`r = Omega/w_n`. Mass unbalance `u = m e` (eccentricity `e`) gives the rotating force `u Omega^2 e^{i Omega t}` (this is the
force of `unbalance_response`). The steady amplitude is `X = u Omega^2 / sqrt((k - m Omega^2)^2 + (c Omega)^2)`, that is

`X(r)/e = r^2 / sqrt((1 - r^2)^2 + (2 zeta r)^2)`.

Divide numerator and denominator by `r^2` and put `q = 1/r^2`: `e/X = sqrt((q - 1)^2 + 4 zeta^2 q)`. The radicand has its minimum
(the amplitude its maximum) where `2(q - 1) + 4 zeta^2 = 0`, i.e. `q = 1 - 2 zeta^2`. Hence

* **Unbalance (force `~ Omega^2`): the peak is at `Omega_p = w_n / sqrt(1 - 2 zeta^2)`**, exists for `zeta < 1/sqrt(2)`, and the
  peak amplitude is `X_p = e / (2 zeta sqrt(1 - zeta^2))` (insert `q = 1 - 2 zeta^2`: the radicand is `4 zeta^4 + 4 zeta^2 (1 -
  2 zeta^2) = 4 zeta^2 (1 - zeta^2)`).
* A force of **constant amplitude** `F`: `X = F / sqrt((k - m Omega^2)^2 + (c Omega)^2)`, peak at `Omega = w_n sqrt(1 - 2
  zeta^2)`, amplitude `F/(2 k zeta sqrt(1 - zeta^2))`.
* The damped natural frequency (the whirl frequency of the free mode) is `w_d = w_n sqrt(1 - zeta^2)`, the third distinct speed.

**The wording of the acceptance matches the physics of the solver's excitation** (unbalance): `w_n / sqrt(1 - 2 zeta^2)` is the
peak of the unbalance response. No mismatch to report. The constant-force form is registered as a check (P5) and as a
negative control (N1), and the two other speeds as negative controls (N2), so the finder is shown to see the right one.

At `zeta = 0` the three speeds coincide with `w_n`, the undamped critical speed, and the crossing of c2 is `w_n` too. For
`zeta > 0` the crossing of the track with the engine-order line `omega = Omega` is at `w_d = w_n sqrt(1 - zeta^2)` (the
whirl frequency of the damped free mode against the line); it differs from the response peak, and that is recorded as K2
(a closed form, a separate fact the acceptance does not ask for).

### D2. The peak finder `RotorModel::unbalance_peaks`

`unbalance_peaks(node, unbalances, lo, hi, samples) -> Result<Vec<ResponsePeak>>` with `ResponsePeak { speed, amplitude,
direction }` (`#[non_exhaustive]`). The amplitude at a spin is the semi-major axis of the orbit of the station `node` from
`unbalance_response(spin, unbalances)` (`Orbit::major`; for an isotropic support the circle radius). Method, registered:

1. evaluate the amplitude on `samples + 1` equally spaced spins in `[lo, hi]`;
2. every interior grid point with `a_i > a_{i-1}` and `a_i >= a_{i+1}` brackets a peak in `[x_{i-1}, x_{i+1}]` (a maximum at an
   end of the interval is not reported; a plateau is reported once, at its first point);
3. **golden-section search** for the maximum in the bracket, until `b - a <= 4 eps b` or 200 contractions; the peak speed is
   the middle of the final bracket, the amplitude is re-evaluated there, the direction is that of the orbit there.
   Unimodality in the bracket is assumed (a window at least one grid step wide around one peak), stated in the limits.

Precision: a golden-section search with function values only locates a maximum to about `sqrt(2 eps_f / |f''/f|)`; for
`X(r)` near the peak the relative curvature is of order `1/zeta^2` in `r`, so the speed is found to about `1.5e-8 zeta`
relative, about `1e-9` for the zeta of the checks; the limit `1e-6` of the acceptance has a margin of three orders. Each
evaluation is one dense solve of the model (`unbalance_response`).

Refusals: `lo`, `hi` not finite or `lo <= 0` or `lo >= hi` (`"range"`), `samples < 3` (`"samples"`), `node` out of range
(`"node"`), unbalances as `unbalance_response` (`"unbalance"`), a singular dynamic stiffness at a grid point (`"singular"`,
propagated).

### D3. The checked crossing `RotorModel::campbell_crossings`

`campbell_crossings(speeds, order: u32) -> Result<Vec<RotorCrossing>>` with `RotorCrossing { track, order, speed, frequency,
omega, direction, residual }` (`#[non_exhaustive]`), `frequency = order speed/(2 pi)` Hz as in `Crossing`. Method:

1. `campbell(speeds)` (so the tracking is the one of ADR 0411 D4);
2. on each track, the sign changes of `g = omega - order Omega` between adjacent speeds having values on both sides, as in
   `whirl_order_crossings` (a grid speed with `g = 0` exactly is returned as it is);
3. each bracket `[Omega_a, Omega_b]` is refined by **bisection on the frozen problem**: at `Omega`, `whirl(Omega)` is solved
   and the mode taken for the track is the one nearest in `omega` to the straight line through the two bracket-end values of
   the track, among the modes of the direction the track has at the ends (all modes if none); `g` of that mode decides the
   half. The bisection stops at `b - a <= 4 eps b` or 200 halvings;
4. the result carries `omega` (the mode frequency at the root), the direction of that mode and `residual = |omega - order
   Omega| / Omega` (the check: the speed really is on the line, to the accuracy of the frozen solve);
5. results sorted by speed, ties by track.

`order = 0` is refused (`"order"`), speeds as `campbell` (`"speeds"`). For a track whose frequency is linear in the speed this
gives the interpolated value; for a gyroscopic rotor it removes the interpolation error. The existing `WhirlCampbell::crossings`
is untouched. The "forward track" of c2 is a result with `direction == Whirl::Forward` (the Jeffcott rotor of c2 has no
gyroscopic term: both of its tracks are the degenerate pair, labelled whatever the sense of the orbit gives, so for it every
track is used; the check on a gyroscopic rotor (K3) filters on `Forward`).

## Models and parameters (fixed here from the derivation, not searched)

**Q (Jeffcott)**: massless cantilever (`L = 0.5`, nodes 0 and 1, node 0 clamped, density 0, `circle_section(0.025, 0.9)`, `E =
2e11`, `nu = 0.3`), disc `m = 20 kg` with zero inertias at node 1, an isotropic constant `Bearing::isotropic(B0 = 2e6, c)` at
node 1 (the model of ADR 0498 Q with `b1 = 0` and a constant bearing). `k = k_s + B0`, `k_s = 1/(L^3/(3 E I) + L/(kappa G A))`
(ADR 0498 D5), `w_n = sqrt(k/m)`, `c = 2 zeta m w_n`. The model is exact for the F-391 element (ADR 0498: 4.4e-16 against the
closed forms of this model). Unbalance `amount = 0.01 kg m` (`e = 5e-4 m`) at node 1, angle 0 unless stated.

**G (gyroscopic rotor)**: the five-station, four-element rotor of ADR 0498 model Y (1 m, `r = 0.025`, density 7850, discs `(20,
0.05, 0.1)` at station 2 and `(5, 0.01, 0.02)` at station 3) with isotropic bearings `Bearing::isotropic(2e6, 0)` at stations 0
and 4 (so the critical speeds and tracks have clean forward and backward labels), undamped.

## Checks (limits fixed now; each run once)

All aggregated errors are asserted finite before they are compared (an `f64::max` over NaN would hide a failure).

| id | clause | check | limit | estimate |
|---|---|---|---|---|
| B1 | rule 3 | digests of model G outputs recorded at 059d2cb6 BEFORE any library change in `sa_rotor_peaks_default_bits.rs`: `matrices()`, `whirl` at 0 and 700, `campbell` over 6 speeds (Debug and CSV), `WhirlCampbell::crossings(1)`, `harmonic_response`, `unbalance_response` at 400, `critical_speeds()`; unchanged after | digests `==` | by construction |
| B2 | rule 3 | `sa_rotor`, `sa_rotor_default_bits`, `sa_rotor_speed`, `sa_rotor_speed_default_bits`, the rest of `rcfd-fem` and `baseline_hashes` pass unchanged | pass | holds |
| P1 | c1 | Q at `zeta = 0.05, 0.1, 0.2, 0.3`: `unbalance_peaks(1, ..., 0.3 w_n, 3 w_n, 600)` peak speed against `w_n/sqrt(1 - 2 zeta^2)` of the closed form (`w_n` from `k_s` of ADR 0498 D5, not from the solver) | relative `<= 1e-6` each | `<= 1e-8` |
| P2 | c1 | the same: peak amplitude against `e/(2 zeta sqrt(1 - zeta^2))` | relative `<= 1e-6` each | `<= 1e-10` |
| P3 | c1 | exactly one peak is found in each window of P1; its direction is forward | count `== 1` | holds |
| P4 | c1 | Q, `zeta = 0.1`, unbalance angle 0.7 rad: the peak speed is the closed form as in P1 | relative `<= 1e-6` | `<= 1e-8` |
| P5 | D1 | constant force: `harmonic_response(spin = 0, omega, force 1 N)` of Q on the same window, the maximum of the orbit major axis at the node found by the same grid and golden-section method written in the test, against `w_n sqrt(1 - 2 zeta^2)`, `zeta = 0.1, 0.3` | relative `<= 1e-6` | `<= 1e-8` |
| N1 | negative | the constant-force closed form `w_n sqrt(1 - 2 zeta^2)` differs from the peak speed the finder returns for unbalance, each of the four zeta | relative `> 1e-3` | `>= 5e-3` (smallest at `zeta = 0.05`) |
| N2 | negative | `w_n` differs from the peak speed for each zeta | relative `> 1e-3` | `>= 2.5e-3` |
| N3 | negative | Q at `zeta = 0.8 > 1/sqrt(2)`: `unbalance_peaks` over the window finds no peak (the amplitude rises monotonically to `e`) | count `== 0` | holds |
| K1 | c2 | Q at `zeta = 0`: `campbell_crossings(0, 100, ..., 1000, order 1)` returns two crossings (the two tracks of the degenerate pair), each speed against the undamped critical speed `w_n` of the closed form and against `critical_speeds()[0].speed` | relative `<= 1e-6` each | `<= 1e-12` |
| K2 | D1 | Q at `zeta = 0.1, 0.3`: the same call: two crossings, each against `w_n sqrt(1 - zeta^2)`; and each differs from `w_n` and from the response peak of P1 | relative `<= 1e-6`; difference `> 1e-3` | `<= 1e-10`; `>= 5e-3` |
| K3 | c2 | G: speeds uniform, 41 points from 0 to `1.2 w_f2`, `w_f2` the second `Forward` entry of `critical_speeds()` (a rule fixed now, applied to the solver's own undamped list); the crossings with order 1 of direction `Forward`: their number equals the number of `Forward` entries of `critical_speeds()` that are `<= 1.2 w_f2`; sorted, each against its critical speed | relative `<= 1e-6` each | `<= 1e-9` |
| K3i | info | G, same grid: the error of the existing interpolated `WhirlCampbell::crossings(1)` of the `Forward` tracks against the same critical speeds | recorded, no limit | order `1e-4` to `1e-3` |
| K4 | D3 | every crossing of K1 to K3 and K5: `residual` | `<= 1e-9` | `<= 1e-12` |
| K5 | D3 | G: order 2 crossings (direction `Forward`, speeds up to `1.2 w_f2`): `residual`; the crossing speed is below the order 1 crossing of the same track (the line is steeper) | `<= 1e-9`; strictly below | holds |
| X1 | refusals | `unbalance_peaks`: `lo = 0`, `lo >= hi`, NaN (`"range"`), 2 samples (`"samples"`), node 99 (`"node"`), negative amount (`"unbalance"`); `campbell_crossings`: order 0 (`"order"`), non-increasing speeds (`"speeds"`) | each with its key | refused |

## Planned mutations (on a scratch copy with its own `CARGO_TARGET_DIR`; each must be caught)

| id | mutation | caught by |
|---|---|---|
| MU1 | `unbalance_peaks` returns the grid maximum, no golden section | P1, P2 (grid step about 2 rad/s, relative about 5e-3) |
| MU2 | golden section keeps the left interval always | P1 |
| MU3 | `campbell_crossings` returns the interpolated crossing, no bisection | K3 if the interpolation error of K3i is above 1e-6 (K3i estimate), K4 |
| MU4 | bisection stopped after 3 halvings | K3, K4 |
| MU5 | `campbell_crossings` uses order 1 in `g` whatever `order` is | K5 |
| MU6 | the amplitude of `unbalance_peaks` taken from the minor axis (equal to the major axis for the isotropic Q, so expected equivalent on Q) | declared equivalent mutant if it survives |
| MU7 | the sign of `g` reversed in the bisection (half chosen wrongly) | K1 to K3 |
| MU8 | `unbalance_peaks` forms the force without the factor `spin^2` (constant-force excitation) | P1, N1 (the peak moves to `w_n sqrt(1 - 2 zeta^2)`) |

## Decision rule

**Done** if B1, B2, P1 to P5, N1 to N3, K1 to K5, X1 are met and every planned mutation except the declared equivalent MU6 is
caught. **Partial** if any limit is missed or any other mutation survives (the miss recorded as an ignored `*_known_miss` test
with the number). **Todo** if the library change breaks B1 or B2 (it is then reverted, nothing merged).

## Limits (not covered)

* Peaks are found on a grid of spins; two peaks closer than one grid step, a peak in the first or last cell, and a flat top are
  missed or reported once. Unimodality in the bracket is assumed.
* The amplitude is the orbit semi-major axis of one station for the unbalances given; a peak of another station or another
  quantity (bending stress, bearing force) is not searched. The peak shifts with the station and with the damping
  distribution of a multi-mass rotor; the closed form is for the single-mass model.
* Speed-dependent bearings: the peak finder uses `unbalance_response`, which evaluates the table at the spin, so it applies;
  but no closed form is registered for it.
* The crossing refinement follows the track by the nearest mode of the track's direction between two grid speeds; a bracket in
  which two modes of one direction approach each other closer than the interpolation error can be refined on the wrong one
  (the residual then still is small; the result is a crossing, not necessarily of the tracked branch). Stated, not tested.
* The crossing is of the damped free-mode frequency with the line (a Campbell fact), not the resonance of the forced response.
* No case-file key, no CLI or report output.

## Open questions

1. **Case-file key.** None added. Recommendation, when a rotor case input exists: `[rotor.response]` with `node`, `lo`, `hi`,
   `samples` writing the peaks table (`speed`, `amplitude`), and `[rotor.campbell] orders = [1, 2]` writing the checked
   crossings next to the existing Campbell CSV; both off by default so defaults keep their bits.
2. **Peak method.** Taken: grid plus golden-section search on the amplitude. Alternative: a root of the derivative by
   bisection with analytic sensitivity (one more solve per step, no `sqrt(eps)` limit) when a limit tighter than `1e-8` is
   ever asked.
3. **Half-power damping estimate.** The inverse problem (estimating `zeta` from the peak width) is not part of F-564;
   recommendation: a follow-up only if a measured response is to be compared.

## Addendum 1: results

Written 2026-11-14 after the runs. Code: `rcfd_fem::rotor::{ResponsePeak, RotorCrossing}`, `RotorModel::{unbalance_peaks,
campbell_crossings}` (two new public methods, nothing existing changed). Tests `crates/rcfd-fem/tests/sa_rotor_peaks.rs` and
`sa_rotor_peaks_default_bits.rs` (B1 digests recorded at 059d2cb6 in a commit before any library change, passing unchanged
after it). Each registered run was made once. No probe was run before the registration.

| id | check | limit | measured | met |
|---|---|---|---|---|
| B1 | 9 digests of model G outputs at the base | `==` | equal | met |
| B2 | whole `cargo test -p rcfd-fem --release` (152 test binaries, 0 failed), `baseline_hashes` (3 passed, 1 ignored), `docs_site` (2 passed) | pass | pass | met |
| P1 | Q peak speed, zeta 0.05 to 0.3, against `w_n/sqrt(1 - 2 zeta^2)` | `<= 1e-6` | `4.4e-9` | met |
| P2 | peak amplitude against `e/(2 zeta sqrt(1 - zeta^2))` | `<= 1e-6` | `4.4e-16` | met |
| P3 | one forward peak per window, four zeta | count 1 | 1 each | met |
| P4 | peak speed at unbalance angle 0.7 | `<= 1e-6` | `1.6e-9` | met |
| P5 | constant-force peak against `w_n sqrt(1 - 2 zeta^2)`, zeta 0.1, 0.3 | `<= 1e-6` | `4.8e-9` | met |
| N1 | constant-force closed form differs from the unbalance peak | `> 1e-3` | `5.0e-3` (zeta 0.05) to `1.8e-1` | met |
| N2 | `w_n` differs from the peak | `> 1e-3` | `2.5e-3` (zeta 0.05) to `9.4e-2` | met |
| N3 | zeta 0.8: no peak | count 0 | 0 | met |
| K1 | Q zeta 0: two crossings equal `w_n` (closed form) and `critical_speeds()[0]` | `<= 1e-6` | `2.2e-16` both | met |
| K2 | Q zeta 0.1, 0.3: crossings against `w_n sqrt(1 - zeta^2)`; differ from `w_n` and from the peak | `<= 1e-6`; `> 1e-3` | `4.4e-16`; `>= 5.0e-3`, `>= 1.5e-2` | met |
| K3 | G: two forward crossings against the two forward critical speeds (224.318233767, 773.425670398 rad/s) | count equal; `<= 1e-6` | `6.3e-14` | met |
| K3i | interpolated crossings of the existing `WhirlCampbell::crossings(1)`, same grid | recorded | `2.3e-9`, `1.5e-7` | recorded (smaller than the estimate `1e-4` to `1e-3`: the 41-point grid is fine enough) |
| K4 | residual `|omega - order Omega|/Omega` of every crossing | `<= 1e-9` | `2.7e-16` (Jeffcott), `3.4e-13` (G, orders 1 and 2) | met |
| K5 | G order 2: three forward crossings, each below the order 1 crossing of its track | `<= 1e-9`; below | residual `3.4e-13`; 112.13, 377.04, 697.41 rad/s | met |
| X1 | 8 refusals with their keys | each | refused | met |

Physics: the wording of the acceptance matches the solver's excitation. The peak speed of the unbalance response is `w_n /
sqrt(1 - 2 zeta^2)` (found to 4.4e-9 at the worst zeta, 0.05), the constant-force response peaks at `w_n sqrt(1 - 2 zeta^2)` (P5,
4.8e-9), the damped crossing is at `w_n sqrt(1 - zeta^2)` (K2); the three are separated by 5e-3 to 1.4e-1 in the four zeta of
the run. The peak speed error `4.4e-9` is the `sqrt(eps)` limit of the golden-section search, as estimated (`<= 1e-8`).

Mutations (scratch copy `f564-scratch/mut`, own target dir `target-f564-mut`; script `f564-scratch/mutate.py`):

| id | caught by |
|---|---|
| MU1 grid maximum, no refinement | P1, P2, P4 |
| MU2 golden section keeps the left interval | P1, P2, P4, N1 (zeta 0.05) |
| MU3 interpolated crossing, no bisection | K4 (gyroscopic) only: K3 is met by the interpolated crossing too (K3i errors 2.3e-9 and 1.5e-7 are below 1e-6), as the registration allowed |
| MU4 bisection stopped after 3 halvings | K1, K2, K3, K4 |
| MU5 order fixed at 1 in `g` | K4 (gyroscopic), K5 |
| MU6 amplitude from the minor axis | survived, declared equivalent (isotropic Q: the orbit is circular, minor = major) |
| MU7 bisection half chosen wrongly | K1, K2, K3, K4 |
| MU8 force without the factor `spin^2` | P1, P2, P4, N1 |

Deviations: K3i estimate wrong by two to four orders (an estimate, not a limit); recorded above. MU6 is the declared
equivalent mutant; it also means the major-axis choice is untested on an anisotropic rotor (stated under limits, not run).

Not run: no anisotropic or multi-mass peak check against a closed form (none registered); no speed-dependent bearing combined
with the peak finder; no CLI or case-file path.

Gates: `cargo test -p rcfd-fem --release` all passed; `baseline_hashes` passed; `docs_site` passed; `cargo clippy -p rcfd-fem
--all-targets -- -D warnings` clean; `cargo fmt -p rcfd-fem --check` clean.

Decision by the rule: **done** (every limit met, every non-equivalent mutation caught, B1 bit-equal).

Open questions (recommendations as in the registration): case-file key (wait for a rotor case input, then `[rotor.response]`
and `[rotor.campbell] orders`, both off by default); peak method (grid plus golden section taken; a derivative root only if a
tighter limit than 1e-8 is asked); damping estimate from the half-power width (follow-up only for measured data).

## For the lead: roadmap entries

> F-564 (proposed status done): Structural solver, advanced tier. Follow-up of F-416 (ADR 0476, 2026-11-14). DONE. F-564 (ADR 0501, 2026-11-14): rcfd_fem::rotor::RotorModel::unbalance_peaks (peaks of the unbalance response over the spin, grid plus golden-section search) and campbell_crossings (Campbell crossings with an engine order refined by bisection on the frozen whirl problem, each with its residual). On the damped Jeffcott rotor the response peak equals w_n / sqrt(1 - 2 zeta^2) to 4.4e-9 and its amplitude e / (2 zeta sqrt(1 - zeta^2)) to 4.4e-16 (limit 1e-6; the wording of the acceptance matches the unbalance excitation, the constant-force peak at w_n sqrt(1 - 2 zeta^2) is confirmed to 4.8e-9); the crossing of the tracks with engine order 1 equals the undamped critical speed to 2.2e-16 at zeta = 0 and w_n sqrt(1 - zeta^2) to 4.4e-16 at zeta > 0; on a gyroscopic rotor the forward crossings equal critical_speeds() to 6.3e-14 with residuals below 3.4e-13; defaults bit-identical; 7 of 8 mutations caught, the eighth declared equivalent. No case-file key.
