# ADR 0498: F-563 registration: speed-dependent bearing coefficients (tables k(Omega), c(Omega)) in the whirl, Campbell, critical-speed and unbalance-response paths of the flexible rotor of ADR 0476

Status: **REGISTERED 2026-11-14, written before any code of F-563 and before any run of its checks**. Not yet accepted by the
maintainer. Results go in the addendum at the end; nothing above the addendum is changed after a run.

Roadmap item F-563 (deps F-416), structural solver, the follow-up proposed by ADR 0476. Number 0498 given by the lead. Branch
`f563-work` from 9386de34, worktree `w49i-work/f563`, scratch `w49i-work/f563-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical (the new behaviour is reached only through new types and functions); no edit of `baseline_hashes.rs`, of the hash
tables or of any golden file; no existing test edited; no new dependency, no unsafe code; clean-room. Every choice below that the
maintainer would normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the
recommendation), except the open questions at the end.

## What F-563 asks

The acceptance, binding:

> Every limit not stated here is registered in the ADR before the run. A table constant in speed gives the bits of the constant bearing; a linear table gives the closed-form Jeffcott critical speed of the speed-dependent stiffness within 1e-6.

Clauses: **c1** a table constant in speed gives the bits of the constant bearing (every output of the model); **c2** a linear
table gives the closed-form Jeffcott critical speed of the speed-dependent stiffness within 1e-6. The context asks the table to
act in the whirl, the Campbell table and the unbalance response, and the registration to fix the interpolation, the behaviour
outside the table, and how the solver's critical speed is found.

## What was read (base 9386de34, read only)

ADR 0476 (all of it) and `crates/rcfd-fem/src/rotor.rs`: `Bearing { k, c }` (2 x 2 real matrices on `(u_x, u_y)`, force `f = -k u
- c u_t`), `RotorModel` with `bearings: Vec<(usize, Bearing)>`, `matrices()` (dense `M`, `K`, `C`, `G` on all unknowns; bearings
add `k` to `K` and `c` to `C` in a loop over `self.bearings` after the shaft and disc loops), `whirl(omega)` (calls `matrices()`,
then `whirl_with`), `campbell(speeds)` (one `matrices()` for all speeds), `critical_speeds()` (undamped synchronous problem `K z =
s (M - i G) z`, `s = Omega^2`, from `matrices()`; refuses an unsymmetric `K` with key `"cross_coupling"`), `harmonic_response(spin,
omega, loads)` and `unbalance_response(spin, ...)` (both from `matrices()`, the spin enters as `omega (C + spin G)`). The tests
`sa_rotor.rs` and `sa_rotor_default_bits.rs`.

Observation that sets the design: every bearing coefficient enters through `matrices()` alone, and the spin enters the rest of
the model only through `G`. A speed-dependent bearing is therefore a function from the spin to the same `K`, `C` contributions:
the minimal change is a `matrices_at(spin)` that evaluates the tables, with `matrices()` kept as the spin-independent matrices.

Published sources: none needed. The tables are a piecewise-linear function of the spin; the closed forms below are derived here
from the single-mass model (Genta, Dynamics of Rotating Systems, 2005, and Childs, Turbomachinery Rotordynamics, 1993, use
speed-dependent bearing coefficients in the same way; cited from memory for the names, not read, not used for any number).

## Arithmetic done before the registration (disclosed)

With python (not part of the repository) the parameters of the checks below were evaluated: for the cantilever shaft `L = 0.5
m`, `r = 0.025 m`, `E = 2e11`, `nu = 0.3`, `kappa = 0.9` the tip stiffness is `k_s = 1.4647e6 N/m` (D5); for the series model
(simply supported, `L = 1 m`, same section) `k_s = 2.9294e6 N/m`; with `m = 20 kg`, `b0 = 2e6`, `b1 = 1000` the quadratic root of
D5 is `441.96 rad/s` against `416.21 rad/s` for `b1 = 0`. These numbers fix the scale of the negative controls only; no
solver was run.

## Design

### D1. The table

`SpeedBearing` holds knots `speeds` (finite, `>= 0`, strictly increasing, at least one) and one `Bearing` per knot (every entry
finite). Key of every refusal: `"speed_bearing"` (empty table, length mismatch, speeds not strictly increasing or negative or
not finite, a non-finite entry).

**Interpolation (taken on the standing instruction): linear** in the spin, entry by entry, for `k` and for `c`:
`b(x) = b_i + t (b_{i+1} - b_i)`, `t = (x - s_i)/(s_{i+1} - s_i)`, `s_i <= x < s_{i+1}`. Exactly at a knot the stored bearing
is returned unchanged (no arithmetic); when the two bracketing bearings are bit-identical the first is returned unchanged.

**Outside the table (recommendation taken): clamp**: `x <= s_0` gives `b_0`, `x >= s_last` gives `b_last`, bit for bit. Extrapolating
a measured or computed table is unsafe (a linear extrapolation can give a negative stiffness), and refusing would make
every Campbell sweep that goes slightly beyond the data fail. `SpeedBearing::covers(x)` tells whether `x` is inside
`[s_0, s_last]` so a caller can refuse itself. A single knot is a constant bearing.

### D2. Where the tables act

`RotorModel::add_speed_bearing(node, table)` (node range as for bearings, key `"node"`). The matrices:
`matrices_at(spin)` is the code of `matrices()` with, after the loop over the constant bearings, one more loop over the speed
bearings in insertion order that adds `table.at(spin).k` to `K` and `.c` to `C` by the same statements as the constant loop (one
shared helper, so the same arithmetic). `matrices()` becomes `matrices_at(0.0)`: for a model with no speed bearing the code
runs the same statements in the same order (the new loop is empty), so the bytes of every existing output are unchanged. For a
model with tables `matrices()` is the matrices at spin 0 (documented).

* `whirl(omega)` uses `matrices_at(omega)`; `harmonic_response(spin, omega, ...)` uses `matrices_at(spin)` (the coefficients
  depend on the rotation speed, not on the excitation frequency); `unbalance_response(spin, ...)` is that with `omega = spin`;
  `campbell(speeds)` evaluates `matrices_at(speed)` at each speed (the mass matrix and the sense matrix do not depend on the
  speed and are taken once, as before). When the model has no speed bearing these functions reuse one `matrices()` as before.
* `critical_speeds()` is unchanged (spin-0 coefficients; documented, and not an error).

### D3. How the critical speed of a speed-dependent bearing is found

For a spin-dependent `K(Omega)` the synchronous critical speed is the self-consistent solution `Omega = crit_j(Omega)`, where
`crit_j(Omega)` is the `j`-th (ascending) undamped critical speed of the model frozen at spin `Omega`, i.e. the existing
D6 problem of ADR 0476 on `matrices_at(Omega)` (its Hermitian problem `K z = s (M - i G) z` with `G` per unit spin). New
method `critical_speeds_over(lo, hi, samples)` (`0 <= lo < hi`, finite, key `"range"`; `samples >= 1`, key `"samples"`):

1. `crit_j` at the `samples + 1` equally spaced speeds `Omega_i = lo + (hi - lo) i/samples`;
2. for each rank `j` present at both ends of an interval with `h_j = crit_j - Omega` of opposite signs (or exactly 0 at a
   grid speed), **bisection** of `h_j` on the interval until the width is `<= 4 eps |Omega|` or 200 halvings (each halving one
   frozen critical-speed solve);
3. each root is returned as a `CriticalSpeed` (speed = the root, shape, whirl number and direction of the frozen problem at
   the root), all ranks sorted by speed.

The sorted eigenvalues of a Hermitian pencil are continuous in `Omega`, so a rank is a continuous function and a sign change
brackets a root; a root where `h_j` touches zero without changing sign is not found (stated in the limits). The accuracy of the
root is the accuracy of the frozen solve (about `1e-13`), so `1e-6` is far inside what the bisection reaches. A cross-coupled
table is refused by the frozen solve (key `"cross_coupling"`), as for constant bearings. `critical_speeds()` itself is the
case of a constant `K`, and for a model without speed bearings `critical_speeds_over` agrees with it to `1e-12` (C3).

### D4. Bits of a constant table (c1)

A one-knot table, a two-knot table with bit-identical bearings and a three-knot table with bit-identical bearings all return
the stored bearing by copy (D1), so `matrices_at(spin)` adds the same `f64` values with the same statements as the constant
loop. The only difference between a model with `Bearing` B at a station and a model with `SpeedBearing::constant(B)` at that
station is the order of the additions into the entry of `K`/`C` if two bearings share a station (constant ones first, tables
after). The check replaces the sole bearing of its station, so the sums are in the same order and every output is bit-equal.
This limit of the claim (bearings sharing a station, one constant and one table, may differ in the last bit through the order
of the sum) is stated, not tested.

### D5. Closed forms (derived here)

Model Q (parallel): a massless cantilever shaft (node 0 clamped, node 1 free at `z = L`, density 0), a disc of mass `m`
(transverse and polar inertia 0) at node 1 and a speed-dependent bearing at node 1 with `k = b0 + b1 Omega` isotropic,
`c = c0 + c1 Omega`. The disc rotation carries no inertia and no stiffness other than the shaft's, so it is condensed (ADR 0476
D3) and the tip force-displacement stiffness of the Timoshenko cantilever is `k_s = 1/(L^3/(3 E I) + L/(kappa G A))`
(bending deflection `P L^3/(3 E I)` plus shear deflection `P L/(kappa G A)` under a tip load `P`; the F-391 element is exact
for a tip load). The bearing is in parallel: `k_eff(Omega) = k_s + b0 + b1 Omega`. The synchronous critical speed solves
`m Omega^2 = k_s + b0 + b1 Omega`, i.e. `m Omega^2 - b1 Omega - (k_s + b0) = 0`, with exactly one positive root
`Omega_c = (b1 + sqrt(b1^2 + 4 m (k_s + b0)))/(2 m)`.

Whirl of Q at spin `Omega`: `m lambda^2 + c lambda + k = 0`, `k = k_s + b0 + b1 Omega`, `c = c0 + c1 Omega`: `lambda = -c/(2m) +-
i sqrt(k/m - (c/(2m))^2)` (underdamped by the choice of the numbers); both the forward and backward whirl (no gyroscopic term)
have it. Unbalance (mass-eccentricity `u = m e`, rotating force `u Omega^2`): the orbit is a circle of radius `u Omega^2/
sqrt((k - m Omega^2)^2 + (Omega c)^2)` with `k = k_s + b0 + b1 Omega`.

Model S (series): a massless simply supported shaft (two equal elements, nodes 0, 1, 2, `L = 1 m` total, density 0) on two
speed-dependent isotropic bearings of stiffness `kb(Omega) = b0 + b1 Omega` at nodes 0 and 2, the disc of mass `m` at node
1. By symmetry the disc translates with no rotation; the shaft midpoint stiffness is `k_s = 1/(L^3/(48 E I) + L/(4 kappa G A))`
(Euler-Bernoulli `P L^3/(48 E I)` plus shear `P L/(4 kappa G A)`), the two bearings each carry `P/2` and the rigid-body
translation they permit adds `P/(2 kb)` to the compliance: `1/k_eff = 1/k_s + 1/(2 kb)`. With `a = 1/k_s` the condition `m
Omega^2 = k_eff` becomes `m Omega^2 (2 a kb + 1) = 2 kb`, a cubic
`p3 Omega^3 + p2 Omega^2 + p1 Omega + p0 = 0`, `p3 = 2 a m b1`, `p2 = m (2 a b0 + 1)`, `p1 = -2 b1`, `p0 = -2 b0`
(signs `+ + - -` for `b0, b1 > 0`: one sign change, by Descartes exactly one positive root). The reference value is that root
found by bisection of the polynomial to the last bit (the polynomial is the closed form; the root of a cubic is not solved by
a radical expression here, which is stated as the deviation from "quadratic"; model Q carries the quadratic closed form).

### D6. Parameters (fixed here from the derivation, not searched)

`E = 2e11`, `nu = 0.3`, `kappa = 0.9` (`circle_section(r, 0.9)`), `r = 0.025`, `m = 20 kg`, density of the shaft 0.
Q: `L = 0.5`, `b0 = 2e6`, `b1 = 1000`, `c0 = 400`, `c1 = 0.5` (table knots at `Omega = 0` and `1000 rad/s`, exact linear
data, so the interpolation reproduces the formula). S: `L = 1`, `b0 = 3e6`, `b1 = 800`, same knots. Critical-speed scan:
`[100, 1000]`, 90 samples. Whirl and response speeds: `Omega = 200, 400, 600` (whirl), `Omega = 0.5, 1, 2` times Q's `Omega_c`
(response, with `Omega_c` the closed form; the last is `883.9 < 1000`, inside the table).

### D7. Model X for the bits check

Two-bearing flexible rotor: 5 stations over 1 m, 4 steel shaft elements (`r = 0.025`, density 7850), a disc (`m = 20`, `I_t =
0.05`, `I_p = 0.1`) at station 2 and one more (`m = 5`, 0.01, 0.02) at station 3; bearings at stations 0 and 4 with anisotropic
`k = [[2e6, 3e5], [-1e5, 3e6]]` and `c = [[200, 20], [20, 300]]` (cross-coupled, so `critical_speeds` is refused: used for
whirl, campbell, harmonic and unbalance), and model Y with the same layout but symmetric `k = [[2e6, 1e5], [1e5, 3e6]]` (used for
the critical speeds). The constant bearing is replaced by `SpeedBearing` of 1, 2 (identical) and 3 (identical) knots at
`0, 500, 1500 rad/s`.

## Checks (limits fixed now; each run once)

All aggregated errors are asserted finite before they are compared (an `f64::max` over NaN would hide a failure).

| id | clause | check | limit | estimate |
|---|---|---|---|---|
| B1 | rule 3 | digests of model X and Y outputs with constant bearings, recorded at 9386de34 BEFORE any library change in `sa_rotor_speed_default_bits.rs`: `matrices()`, `whirl` at 0, 300, 900, `campbell` over 5 speeds (every table value, CSV text), `harmonic_response`, `unbalance_response`, Y's `critical_speeds`; the test passes unchanged after | digests `==` | bit-equal by construction |
| B2 | rule 3 | `sa_rotor`, `sa_rotor_default_bits`, `w46c_rotordyn` and `baseline_hashes` pass unchanged | pass | holds |
| T1 | c1 | model X with the bearing replaced by 1-, 2- and 3-knot constant tables: `whirl` at 0, 300, 900 (every root, every mode's `omega`, `growth`, whirl, residual, shape), `campbell` (speeds, CSV text, every frequency of every track, every growth), `harmonic_response` (spin 300, `omega` 123.4), `unbalance_response` at 300 and 900; model Y: `critical_speeds` | all bit-equal (`f64::to_bits`) | equal |
| T2 | c1 | a 3-knot table with distinct bearings, `whirl` and `unbalance_response` at each knot speed equal bit for bit to the constant-bearing model with that knot's bearing | bit-equal | equal |
| T3 | D1 | `at(x)`: knots `(0; k=[[1,0],[0,1]], c=[[2,0],[0,2]])`, `(10; k = 3 I, c = 6 I)`: `at(5)` has `k = 2 I`, `c = 4 I` exactly (`==`); `at(0)`, `at(10)` equal the knots by bits; `covers(-1)`, `covers(11)` false, `covers(5)` true | `==` | exact (`1 + 0.5 * 2 = 2`) |
| L1 | c2 | Q: `critical_speeds_over(100, 1000, 90)` returns two roots (forward and backward of the degenerate pair), each equals `Omega_c` of D5 | relative `<= 1e-6` each | `<= 1e-11` |
| L2 | c2 | S: the same, each equals the root of the cubic of D5 | relative `<= 1e-6` each | `<= 1e-11` |
| L3 | D2 | Q with `c0 = c1 = 0`: `whirl` at 200, 400, 600: both whirl modes `omega = sqrt(k_eff(Omega)/m)` | relative `<= 1e-9` each | `<= 1e-12` |
| L4 | D2 | Q damped: `whirl` at 200, 400, 600: each of the two modes `omega = sqrt(k/m - (c/2m)^2)` and `growth = -c/(2 m)` | relative to `|lambda|` `<= 1e-9` each | `<= 1e-12` |
| L5 | D2 | Q: `campbell` over `0, 100, ..., 1000` (11 speeds): every tracked frequency at every speed equals the closed form of L4 at that speed; two tracks | relative `<= 1e-9` | `<= 1e-12` |
| L6 | D2 | Q damped: `unbalance_response` at `0.5, 1, 2 Omega_c` (amount `u = 0.01 kg m`): the semi-major and the semi-minor axis of the disc orbit equal the circle radius of D5; direction forward | relative `<= 1e-9` each | `<= 1e-11` |
| L7 | D1 | outside the table: Q with knots at `[100, 800]`: `whirl(50)` and `whirl(900)` equal bit for bit those of the model with the constant bearing of the first and last knot respectively | bit-equal | equal |
| C3 | D3 | on model Y (constant bearing), `critical_speeds_over(lo, hi, 400)` over a range holding the first two critical speeds of `critical_speeds()` and the ranks of the frozen problem: each root against the synchronous problem is not applicable (the constant model's critical speed is `Omega = crit_j`, found as a root of `crit_j - Omega` only if `crit_j` is the fixed point, which it is since `crit_j` is constant): every rank `j` in range matches `critical_speeds()[j]` | relative `<= 1e-12` | `<= 1e-13` |
| N1 | negative control | the closed form of Q with `b1 = 0` differs from L1's solver value | relative `> 1e-2` | `6 %` |
| N2 | negative control | the closed form of Q with `b1` of the wrong sign differs from L1's value | relative `> 1e-2` | `~9 %` |
| X1 | refusals | `SpeedBearing::new` with empty list, mismatched lengths, decreasing, repeated, negative and NaN speeds, a NaN entry (key `"speed_bearing"`); `add_speed_bearing` at station 99 (`"node"`); `critical_speeds_over` with `lo >= hi`, NaN (`"range"`), 0 samples (`"samples"`); `critical_speeds_over` on model X (`"cross_coupling"`) | each refused with its key | refused |

Note on C3: the intended check is that a model without a speed bearing (and a model with a constant table) gives from
`critical_speeds_over` the same roots as `critical_speeds()`; it is run on Y over the range `[0.3 w1, 3 w2]` where `w1`, `w2`
are the first two entries of `critical_speeds()`, sampled 400 times; each rank whose entry of `critical_speeds()` lies in the
range is matched to its entry (the sorted speed lists compared pairwise, same count).

## Planned mutations (on a scratch copy with its own `CARGO_TARGET_DIR`; each must be caught)

| id | mutation | caught by |
|---|---|---|
| MU1 | `at` returns the lower knot (no interpolation) | T3, L1, L2, L3, L6 |
| MU2 | `whirl` uses `matrices()` (spin 0) | L3, L4 |
| MU3 | `campbell` uses one `matrices()` | L5 |
| MU4 | `harmonic_response` uses `matrices()` | L6 |
| MU5 | `at` extrapolates beyond the last knot instead of clamping | L7, T3 |
| MU6 | the damping table is not applied (`c` taken from the first knot) | L4, L6 |
| MU7 | `critical_speeds_over` returns the left end of the bracket (no bisection) | L1, L2 |
| MU8 | the stiffness table is added after the constant bearings but with `+=` of `c` and `k` swapped | T2, L3 |
| MU9 | `matrices()` evaluates the tables at the first knot instead of 0 (changes the spin-0 matrices of a table model) | a test of `matrices()` of a table model against `matrices_at(0)` (T4) |

T4 (added here with MU9): for a model with a table, `matrices()` equals `matrices_at(0.0)` bit for bit and equals the matrices of
the constant-bearing model with the table's value at 0, limit `==`.

Equivalent mutant noted: `at` using `a + t (b - a)` also for identical neighbours returns `a` (t times 0) bit for bit except for
a signed zero, so the shortcut is a speed-up, not a correctness need; it is not mutated.

## Decision rule

**Done** if B1, B2, T1 to T4, L1 to L7, C3, N1, N2, X1 are met and every planned mutation is caught. **Partial** if any limit is
missed or any mutation survives (the miss recorded as an ignored `*_known_miss` test with the number). **Todo** if the library
change breaks B1 or B2 (it is then reverted, nothing merged).

## Limits (not covered)

* Speed-independent shaft, disc and gyroscopic matrices; only bearings have tables.
* Piecewise-linear interpolation only (no spline, no monotone cubic), clamp outside.
* Self-consistent critical speeds are found by sign changes of `crit_j - Omega` on a grid; a tangential touch or two roots
  closer than one grid step with the same sign at both ends are missed. `critical_speeds()` keeps the spin-0 coefficients.
* Cross-coupled tables have no undamped critical speed (refused as before); their whirl and stability are by `whirl` and `campbell`.
* A constant table sharing a station with a constant bearing may differ in the last bit through the order of additions (D4).
* The closed-form series model is a cubic (reference root by polynomial bisection), the parallel model the quadratic.
* Damped critical speeds as response peaks are the other follow-up of ADR 0476 and not part of F-563.

## Open questions

1. **Case-file key.** No key is added. Recommendation: when a rotor case input exists, `[[rotor.bearing]]` with `node`, and
   `speeds = [...]`, `kxx = [...]`, `kxy`, `kyx`, `kyy`, `cxx`, `cxy`, `cyx`, `cyy` arrays of equal length (a missing array
   is zeros, a one-element array a constant); extrapolation policy `"clamp"` (default) or `"refuse"`.
2. **Clamp or refuse outside the table.** Taken: clamp, with `covers`. Alternative: refuse out-of-range speeds in
   `whirl`/`campbell` (stricter, breaks sweeps past the data).
3. **Interpolation.** Taken: linear. Alternative: monotone cubic for tables of hydrodynamic bearings with strong curvature.

## Addendum 1: results

(to be written after the runs)
