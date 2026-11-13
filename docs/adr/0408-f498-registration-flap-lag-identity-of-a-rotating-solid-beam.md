# ADR 0408: F-498 registration: the flap-lag identity of a rotating solid beam, the acceptance restated (1 - 2 f_x), and the radial bar's clause

Status: **REGISTERED 2026-11-13, written before any code of F-498 and before any run of its checks.** Not yet accepted by the
maintainer. Results go in the addendum at the end; nothing above the addendum is changed after a run.

**Disclosure: the main result is known beforehand.** ADR 0403 already ran the same computation: its check A3b measured
`|(omega_flap^2 - omega_lag^2)/Omega^2 - (1 - 2 f_x,h)| = 1.12e-8` with `f_x,h = 9.610e-4`, and A3 measured `1.922e-3` against 1. The
tests of this item run the same library code on the same model at the same speed, so they will return the same bits unless the
library changed; the limits below are registered with these numbers in view. What this item adds is the restated acceptance as a
test file of its own with its own limits, the re-derivation of `1 - 2 f_x` (below), and the control and mutation record. The
radial bar's clause (A2a to A2c of ADR 0403) is likewise known: 1.9e-9, 1.7e-6 and 3.1e-5, 6.9e-5 and 1.1e-3.

Parent: [ADR 0403](0403-f396-registration-rotating-part-modes-and-campbell-table.md) (F-396), A3 (known miss) and its remaining
question 1, option (a), taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation). Roadmap item F-498
(deps F-396). Size S. Number 0408 given by the lead. Branch `f498-flap-lag-restated`, worktree `w49i-work/f498`, scratch
`w49i-work/f498-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here before the code and the runs; each registered run made once; defaults
bit-identical (test code only: no library line is changed, so no baseline hash run is required); no edit of `baseline_hashes.rs`,
the hash tables or a golden file; the tests of ADR 0403 (`si_rotating.rs`) are not edited; no new dependency, no unsafe code;
clean-room.

## What F-498 asks

The acceptance of the roadmap entry, binding: "Every limit not stated here is registered in the ADR before the run. On the square
radial beam of F-396 (Hex20, slenderness 20, nu = 0) at Omega = 0.2 omega_0: (omega_flap^2 - omega_lag^2)/Omega^2 = 1 - 2 f_x
within 1e-4, f_x the axial share of the flap mode's kinetic energy, and = 1 within 3 f_x; the radial bar's clause reads: with the
stress stiffening off (omega_n,0^2 - omega_n^2)/Omega^2 = 1 within 1e-3, with it the slope equals Q_n,h - 1 within 1e-3 and Q_n,h
equals k_n^2 L^2/3 + 1/4 within 1 percent. No new code is needed if the checks of ADR 0403 are accepted as these."

Clauses: (c1) identity `1 - 2 f_x` within 1e-4; (c2) identity `1` within `3 f_x`; (c3) bar, stiffening off: ratio 1 within 1e-3;
(c4) bar, stiffening on: slope `Q_n,h - 1` within 1e-3; (c5) `Q_n,h = k_n^2 L^2/3 + 1/4` within 1 percent.

## What was read (no run)

ADR 0403 (D1 to D3, arithmetic 2 and 3, A2, A3, A3b, the addendum table and its question 1);
`crates/rcfd-fem/src/rotating.rs` (`spin_softening_mass`: `M_perp = m_ab (delta_ij - a_i a_j)`; `RotatingPart::operator`:
`K + Omega^2 (s K_sigma,1 - c M_perp)`; `stiffening_quotient`, `slope`, `modes`); `crates/rcfd-fem/tests/si_rotating.rs` (the set-ups
`bar`, `beam`, the helpers `share`, `flap_lag`, `flap_lag_at`, and the checks A2, A3, A3b). Nothing was run. No arithmetic beyond the
re-derivation below was done; what is quoted from ADR 0403 is quoted, not recomputed.

## Design

### D1. The relation `(omega_flap^2 - omega_lag^2)/Omega^2 = 1 - 2 f_x`, re-derived

The part rotates about the axis `a = e_z` and the beam lies along `x` (flap = bending in `z`, along the axis; lag = bending in `y`,
in the plane of rotation). The operator in the rotating frame (ADR 0403 D1) is `K + Omega^2 (K_sigma,1 - M_perp)`, `M_perp = M (I -
a a^T)`: the centrifugal softening acts on the displacement components perpendicular to the axis, `x` and `y`, not on `z`. To
first order in `Omega^2` the shift of an eigenvalue of a mode `phi` (`phi^T M phi = 1`) is `Omega^2 phi^T (K_sigma,1 - M_perp) phi`.
The flap and lag modes of the square section are degenerate at rest but decoupled by the mirror symmetries `y -> -y`, `z -> -z`
(the operator is invariant under both), so the shift is taken in each mode's own symmetry class.

Modes of a slender solid beam with `nu = 0`: flap `u_z = w(x)`, `u_x = -z w'(x)` (the section rotates), `u_y = 0`; lag `u_y = v(x)`,
`u_x = -y v'(x)`, `u_z = 0`: Euler-Bernoulli motion plus the rotation of the section, the first correction of the solid.

*Softening.* `phi^T M_perp phi` is the kinetic share of the components `x` and `y`. Flap: only `u_x` counts. Its kinetic energy is
`rho int z^2 dA int w'^2 = rho A r_g^2 int w'^2` against `rho A int w^2` of `u_z` (`r_g^2 = h^2/12`), so the share is `f_x = r_g^2
int w'^2/int w^2` (the mass of `u_x` inside the normalisation is second order). Lag: `u_y = v` and `u_x` both count, the share is 1.
The softening part of `(lambda_flap - lambda_lag)/Omega^2` is `-f_x + 1`.

*Stiffening.* `phi^T K_sigma,1 phi = int sigma_ij (d u_k/d x_i)(d u_k/d x_j) dV`, the pre-stress at 1 rad/s. The body force is `rho
Omega^2 (x, y, 0)`: no component along the axis, so `sigma_zz = 0`. With `nu = 0`, `sigma_xx(x) = rho (L^2 - x^2)/2` (axial
equilibrium, free end) and `sigma_yy = rho (h^2/4 - y^2)/2` (lateral equilibrium of the in-plane force `rho y`, free faces;
`sigma_xy` is neglected, an assumption of this derivation). The terms `int sigma_xx (d u_k/d x)^2` are the same for both modes by
the mirror symmetry of the square section (swapping `y` and `z` swaps the modes and leaves `sigma_xx` unchanged): both get `Q`,
which cancels in the difference. What remains: the lag mode has `d u_x/d y = -v'`, multiplying `sigma_yy`; the flap mode has `d
u_x/d z = -w'`, multiplying `sigma_zz = 0`. The mean of `(h^2/4 - y^2)/2` over `y` in `[-h/2, h/2]` is `(h^2/4 - h^2/12)/2 =
h^2/12`, so the lag's extra term per `Omega^2` and per `rho A int v^2` is `(h^2/12) int v'^2/int v^2 = f_x` (same `r_g^2`, same
mode shape as the flap, by the symmetry).

*Sum.* flap: `lambda_flap = lambda_0 + Omega^2 (Q - f_x)`; lag: `lambda_lag = lambda_0 + Omega^2 (Q + f_x - 1)`. Hence

`(lambda_flap - lambda_lag)/Omega^2 = (Q - f_x) - (Q + f_x - 1) = 1 - 2 f_x`.

For the Euler-Bernoulli beam `f_x = 0` and the difference is 1 (the stub's identity, ADR 0299). For the solid it is `1 - 2 f_x`, with
`f_x = (h^2/12) int w'^2/int w^2 = 9.68e-4` at `L/h = 20` (ADR 0403 arithmetic 3, from its Hermite model; quoted, not recomputed).
Terms of higher order in `Omega^2` are not in the formula; ADR 0403 measured 1.1e-8 for the whole remainder at `0.2 omega_0`, which
is the number the limit 1e-4 is set against.

The `f_x` of the acceptance is the discrete one: `f_x,h = phi_f^T M_x phi_f / phi_f^T M phi_f`, the `x` share of the kinetic energy
of the flap mode at the speed of the run, from the library's consistent mass (`share` of `si_rotating.rs`, copied).

### D2. The identity `1` within `3 f_x`

The deviation of the difference from 1 is `2 f_x` (D1), 1.9e-3 here. The acceptance's "= 1 within 3 f_x" says the Euler-Bernoulli
identity holds up to the solid's section-rotation term `2 f_x` with a margin of 1.5.

### D3. The radial bar's clause

The bar B of ADR 0403 (40 Hex8 along, one element across, `y` and `z` held, clamped at `x = 0`, `nu = 0`) has axial modes only. On the
tensor-product mesh `K = K_1D (x) M_yz`, `M = M_1D (x) M_yz`, so for `u_x = f(x)` the softening mass is `M` itself and the operator
is `K - Omega^2 M + Omega^2 K_sigma,1`. With the stiffening off the eigenvalues are `lambda_n,0 - Omega^2` and `(lambda_n,0 -
lambda_n)/Omega^2 = 1` to the eigensolver's accuracy (c3). With it on, the first-order slope is `Q_n,h - 1`, `Q_n,h = phi_n,0^T
K_sigma,1 phi_n,0` the discrete quotient (c4).

The closed form of the quotient (c5): the modes are `sin(k_n x)`, `k_n L = (2n - 1) pi/2`, the tension per `rho A Omega^2` is `(L^2 -
x^2)/2`, so `Q_n = int_0^L (L^2 - x^2)/2 k_n^2 cos^2(k_n x) dx / int_0^L sin^2(k_n x) dx`. With `cos^2 = (1 + cos 2kx)/2`: `int_0^L
(L^2 - x^2)/2 dx = L^3/3`. For `int_0^L (L^2 - x^2) cos 2kx dx`, integrate by parts: `[(L^2 - x^2) sin 2kx/(2k)]_0^L = 0`, and what
is left is `(1/k) int_0^L x sin 2kx dx = (1/k)[-x cos 2kx/(2k) + sin 2kx/(4k^2)]_0^L = -L cos(2kL)/(2k^2) = L/(2k^2)`, because `2kL
= (2n - 1) pi`, `cos = -1`, `sin = 0`. The numerator is `(k^2/4)(2L^3/3 + L/(2k^2)) = k^2 L^3/6 + L/8`, the denominator `L/2`:
`Q_n = k_n^2 L^2/3 + 1/4`. With `L = 1`, `Q_1 = 1.0724670`, `Q_2 = 7.6522033` (ADR 0403). In the test `Q_n` is computed from the
formula, not typed.

Speed of the bar: `Omega_B = sqrt(1e-4 E/rho)` as in A2 (the centrifugal stress per `E` stays below 1e-4: a small-displacement
pre-stress, no geometric nonlinearity claimed).

### D4. Where it goes

A new test file `crates/rcfd-fem/tests/si_rotating_flap_lag.rs` with its own copies of the set-ups (`bar`, `beam`) and of `share` and
`flap_lag`. `si_rotating.rs` (ADR 0403's tests, including the ignored record A3) and the library are not edited.

## Registered checks

All with `E = 2e11`, `rho = 7850`, `nu = 0`, axis `e_z`, origin 0, the modal options of ADR 0403 (backward-residual test, tolerance
1e-12, 4 modes). Beam F: `hex_box([0, -0.025, -0.025], [1, 0.05, 0.05], [20, 2, 2])` upgraded to Hex20, clamped at `xmin`, `omega_0 =
sqrt(lambda_1)` of the rest solve, `Omega = 0.2 omega_0`. The flap mode: the lowest mode with a `z` share above 0.5; the lag mode:
the lowest with a `y` share above 0.5. `d = (lambda_flap - lambda_lag)/Omega^2`.

| Id | Clause | Measure | Limit | Estimate |
| --- | --- | --- | --- | --- |
| S1 | c1 | `abs(d - (1 - 2 f_x,h))` | `<= 1e-4` | 1.1e-8 (ADR 0403 A3b, same code and set-up) |
| S2 | c2 | `abs(d - 1)` | `<= 3 f_x,h` | 1.92e-3 against 2.88e-3 (`2 f_x,h`, `f_x,h = 9.61e-4`) |
| S3 | D1 | `abs(f_x,h/9.683e-4 - 1)`, the discrete share against the beam-theory value of the derivation | `<= 5e-2` | 7e-3 (ADR 0403: 9.610e-4 against 9.68e-4) |
| B1 | c3 | bar B, `stress_stiffening = false`, `Omega_B`: `abs((lambda_n,0 - lambda_n)/Omega_B^2 - 1)`, n = 1, 2 | `<= 1e-3` | 1.9e-9, 1.8e-9 |
| B2 | c4 | bar B, full operator: `abs(slope - (Q_n,h - 1))`, `slope = (lambda_n - lambda_n,0)/Omega_B^2`, n = 1, 2 | `<= 1e-3` | 1.7e-6, 3.1e-5 |
| B3 | c5 | `abs(Q_n,h/(k_n^2 L^2/3 + 1/4) - 1)`, n = 1, 2, `Q_n,h` from `stiffening_quotient` of the rest mode | `<= 1e-2` | 6.9e-5, 1.1e-3 |
| N1 | control | `abs(d - 1)`: the Euler-Bernoulli limit 1e-4 would be missed, so the restated clause is not vacuous | `> 1e-3` | 1.92e-3 |
| N2 | control | F with `spin_softening = false`: `abs(d - (1 - 2 f_x,h))` | `> 0.5` | 1.001 (`d = -f_x`) |
| N3 | control | F with `stress_stiffening = false`: `abs(d - (1 - 2 f_x,h))` | `> 5e-4` | 9.6e-4 (the lag loses its `+f_x`: `d = 1 - f_x`) |
| N4 | control | bar B, full operator: `(lambda_1,0 - lambda_1)/Omega_B^2`, the literal "ratio 1" with the stiffening on | `< 0` | -0.0725 (`1 - Q_1`, ADR 0403 N4) |

Time: ADR 0403's ten tests ran in 7.8 s together in release; each new test runs in seconds.

### M. Planned mutations (each alone, on a `git archive` copy of the code commit in the scratch folder, after the registered run)

| Id | Mutation (in `rotating.rs` of the copy) | Expected to be caught by |
| --- | --- | --- |
| MU1 | `spin_softening_mass` without the projection (`p = I`): the flap is softened as much as the lag; `d = -f_x` | S1, S2, S3 (the flap's `x` share unchanged: S3 stays met), N2 |
| MU2 | the `y` component of the softening dropped (`p[1][1] = 0`): the lag loses its `y` softening; `d` about `-f_x` | S1, S2 |
| MU3 | `operator` with `2 c M_perp` | S1, S2, B1, B2 |
| MU4 | `operator` with the sign of the stiffening reversed (`-s K_sigma,1`) | B2 (slope `-Q - 1`), S1 (`d` off `1 - 2 f_x` by `4 f_x`); B3 stays met |

## Decision rule

- **done**: S1, S2, B1, B2, B3 met (S3 and N1 to N4 as stated), and each planned mutation caught by at least one of S1, S2, B1, B2.
- **partial**: S1, S2, B1 to B3 met but a mutation not caught (diagnosed in the addendum), or a clause of the acceptance not run.
- **todo**: a clause missed without a diagnosed cause.

A miss of S1 or S2 is not tuned away: it becomes an ignored `*_known_miss` test with the limit intact.

## Limits (what is not covered)

- One speed (`0.2 omega_0`) and one section (square, `L/h = 20`, Hex20 20 x 2 x 2). At other speeds the identity has second-order
  terms in `Omega/omega_0` that are not claimed; ADR 0403 A4 and A5 cover the speed dependence of the flap slope.
- `nu = 0`: with `nu > 0` the clamped root adds a local stress the derivation does not separate.
- The derivation neglects `sigma_xy`, the mass of the section rotation in the normalisation and second-order terms; their sum is
  what S1 measures.
- No Coriolis coupling (F-499): the modes are those of the rotating frame without gyroscopic terms.
- This is a restatement with known numbers (disclosed above): it checks the acceptance as worded, not the model against an
  independent solid reference.

## Open points taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. Test code only, in a new file: the tests of ADR 0403 are not edited (house rule), and A3 stays an ignored record there.
2. S3 and N1 to N4 are added to the clauses of the acceptance as a consistency check of the derivation and as controls; the decision
   rule does not depend on S3.

## Addendum 1: results (2026-11-13)

Code commit: the test file `crates/rcfd-fem/tests/si_rotating_flap_lag.rs` (test code only; no library line, `si_rotating.rs`, hash
table or golden file changed). Release profile, `CARGO_BUILD_JOBS=4`, `--test-threads=2`. The registered run was made once (two
tests, 0.46 s); all limits met, no amendment.

### Checks

| Id | Limit | Measured | Met |
| --- | --- | --- | --- |
| S1 `abs(d - (1 - 2 f_x,h))` | `<= 1e-4` | 1.117e-8 (`d = 0.9980779584`, `1 - 2 f_x,h = 0.9980779473`, `Omega = 51.150904` rad/s) | yes |
| S2 `abs(d - 1)` | `<= 3 f_x,h = 2.883e-3` | 1.922e-3 (`2 f_x,h`) | yes |
| S3 `abs(f_x,h/9.683e-4 - 1)` | `<= 5e-2` | 7.51e-3 (`f_x,h = 9.610264e-4`) | yes |
| B1 n = 1, 2 | `<= 1e-3` | 1.92e-9, 1.80e-9 | yes |
| B2 n = 1, 2 | `<= 1e-3` | 1.66e-6 (`Q_1,h = 1.07254057`), 3.12e-5 (`Q_2,h = 7.66047610`) | yes |
| B3 n = 1, 2 | `<= 1e-2` | 6.86e-5, 1.081e-3 | yes |
| N1 `abs(d - 1)` | `> 1e-3` | 1.922e-3 | as expected |
| N2 spin softening off | `> 0.5` | 0.99904 (`d = -9.610e-4`) | as expected |
| N3 stress stiffening off | `> 5e-4` | 9.631e-4 (`d = 0.99903690`) | as expected |
| N4 bar, full operator, literal ratio | `< 0` | -7.2539e-2 | as expected |

As disclosed, these are the numbers of ADR 0403 (A3b 1.12e-8, A3 1.922e-3, A2a to A2c) to the printed digits; the existing
`si_rotating` was re-run after the new file and is unchanged (9 passed, 1 ignored, 7.8 s). The estimates of the registration are
met with large margins (S1 by a factor 9e3, S2 by 1.5, B1 to B3 by at least 9).

### Mutations (copy of the code commit from `git archive` in `f498-scratch/copy`, own target directory, script `mutate.py`, logs `MU1.log` to `MU4.log`)

| Id | Result in the new tests |
| --- | --- |
| MU1 (no projection in `M_perp`) | caught by S1 (0.999) and S2 (1.001); B1 to B3 stay met, as predicted |
| MU2 (`y` softening dropped) | caught by S1 (0.999) and S2 (1.001) |
| MU3 (`2 c M_perp`) | caught by S1, S2, B1 (1.0), B2 (1.0), N4 |
| MU4 (stiffening sign reversed) | caught by B2 (2.15 and 15.3), N4 and S1 (1.93e-3, not the 4 f_x = 3.8e-3 of the registration: the sign of the common `Q` term also flips, which leaves `2 f_x`; the estimate in the table was wrong, the catch is not affected); N1 reports as not expected (`abs(d - 1)` falls to 1e-9), a second signal; B3 stays met |

All four planned mutations are caught by at least one of S1, S2, B1, B2.

### Deviations

- The registered estimate of MU4's S1 deviation (`4 f_x`) was wrong (measured `2 f_x`); the mutation is caught either way.
- fmt and clippy: `cargo fmt -p rcfd-fem --check` clean, `cargo clippy -p rcfd-fem --all-targets -- -D warnings` clean.

### What was not run

Other speeds, sections and Poisson ratios (see "Limits"); the whole-workspace tests and `baseline_hashes` (no shared solver file
changed).

### Decision by the registered rule

**done**: S1, S2, B1, B2, B3 met, S3 and N1 to N4 as stated, and MU1 to MU4 each caught. The decision is formal in the sense of the
disclosure above: the numbers were known from ADR 0403, and the value of the item is the restated acceptance, the derivation and
the control record. Done is the maintainer's call.

### Remaining questions

1. The identity is checked at one speed and `nu = 0`. Options: (a) add the second-order speed dependence and `nu > 0` as a follow-up;
   (b) none. Recommendation: (b); the speed dependence of the flap slope is in ADR 0403 A4 and A5 and the identity is the
   acceptance of this item.
2. ADR 0403's ignored record `a3_flap_lag_identity_known_miss` stays as it is (house rule). Options: (a) keep it as the record of
   the unrestated clause; (b) remove it later by an approved edit. Recommendation: (a).

## For the lead: roadmap entries

Proposed status for F-498: done.

> Flap-lag identity of the rotating solid beam restated. ADR 0408, 2026-11-13: new test file si_rotating_flap_lag.rs (test code only, the numbers known beforehand from ADR 0403 A3b and A2 and disclosed in the registration, with the 1 - 2 f_x relation re-derived there); on the square radial Hex20 beam (slenderness 20, nu = 0) at 0.2 omega_0 the flap-lag difference per Omega^2 equals 1 - 2 f_x within 1.1e-8 (limit 1e-4, f_x = 9.61e-4 the axial kinetic share of the flap mode) and equals 1 within 1.92e-3 (limit 3 f_x = 2.88e-3); the radial bar with the stiffening off gives ratio 1 within 1.9e-9, with it the slope equals Q_n,h - 1 within 3.1e-5 and Q_n,h equals k_n^2 L^2/3 + 1/4 within 1.1e-3 (limits 1e-3, 1e-3, 1e-2); four planned mutations of rotating.rs caught, four controls as expected. Limits: one speed, nu = 0, one section.
