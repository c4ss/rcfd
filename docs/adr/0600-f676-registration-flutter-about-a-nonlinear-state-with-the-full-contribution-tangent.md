# ADR 0600: F-676 registration: flutter about a nonlinear state with the full contribution tangent

Status: **REGISTERED 2026-11-15, written before any code of F-676 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-676 was written in the worktree. As the brief asks for the load-level
arithmetic to be checked numerically before registering, one set-up probe was written and run in the scratch folder only
(`w49i-work/f676-scratch/probe`, a `git archive` of the base 6c38e510 with one added test file `probe_f676.rs`; log
`probe1.log`). It measured set-up quantities only: the convergence of the unsymmetric Newton solve, the dead linear buckling
factors (the load level), and the existing `follower::buckling_factors_full` at rest (the zero-load reference of clause a4: its
factors, `residuals`, `backward_errors`, steps). It did not call the new function (which did not exist), did not call
`tangent_modes_full` at a state, and computed no central difference. Its numbers are disclosed in "Set-up probe"; checks whose
value it already measured are marked "known". Results go in the addenda at the end; nothing above the addenda is changed after a
run.

Roadmap item F-676 (deps F-671, F-653), structural solver, advanced tier: the follow-up proposed by
[ADR 0596](0596-f671-registration-contributions-about-a-nonlinear-state-on-the-buckling-and-direct-harmonic-paths.md) (its D1
reading: the full unsymmetric path about a state). Number 0600 given by the lead. Branch `f676-work` from the local main tip
6c38e510 (which holds F-653's `eigen_pencil`, `tangent_modes_full`, `buckling_factors_full`; F-664's backward errors; F-669,
F-671, F-677), worktree `w49i-work/f676`, scratch `w49i-work/f676-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number); each registered run made once; existing tests not edited;
defaults bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new
dependency, no unsafe code, the crate graph unchanged; clean-room. The roadmap files are not edited. Every choice the maintainer
would normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).

## What F-676 asks

The acceptance, binding:

> Every limit not stated here is registered in the ADR before the run. The complex pencil path of ADR 0580 takes the driver's tangent at a nonlinear state with the full (unsymmetric) tangents of its contributions; on a free-edged follower load (Beck's column of Full Hex8 at a compressed state) the complex eigenvalues satisfy the central-difference residual equation of the nonlinear residual including the follower force within 1e-6 (vector measure), with the backward error of ADR 0587, and at zero load they equal buckling_factors_full within 1e-10.

Clauses: **a0** every limit registered here; **a1** the complex pencil path takes the driver's tangent at a nonlinear state with
the full tangents of the contributions; **a2** on a free-edged follower load (Beck's column of Full Hex8, compressed state) the
complex eigenvalues satisfy the central-difference residual equation within `1e-6` by a vector measure; **a3** with the backward
error of ADR 0587 (reported for each pair and within that ADR's limit form); **a4** at zero load they equal
`buckling_factors_full` within `1e-10`. Brief: additive API, existing bits unchanged.

Readings, registered:

- **a1, which path.** `tangent_modes_full` (ADR 0580 D3) already takes `nonlinear::tangent(u, states)` plus `add_full` of the
  contributions at `(u, load_factor)`: the modal pencil about a state exists. The buckling pencil of ADR 0580
  (`buckling_factors_full`) is at rest only (`point_stiffness`, `K_p(0, 1)`), and a4 compares with it, so the item's new code is
  its state form: `follower::tangent_buckling_factors_full_with` (D1), the full counterpart of ADR 0596's
  `tangent_buckling_factors_with`. The modal path about the state is checked by the same measure (MV, below), test only, as the
  flutter path proper.
- **a2, "the complex eigenvalues".** Every pair returned by the call (the `count` of smallest modulus, plus a conjugate partner
  where returned), each with its vector; the measure is D2.
- **a3.** The `backward_errors` of each pair (ADR 0587 D1, D2, carried by `ComplexBuckling`) within ADR 0587's registered form
  `eta <= 2 tolerance`.
- **a4.** At `u = 0`, the initial states, `load_factor = 0`, the follower as the contribution and the same pre-stress and
  options: the factors of the new function against `buckling_factors_full(model, pre, &[&fp], options)`, position by position.

## What was read

`AGENT_RULES.md`; the brief; ADR 0580 (whole: D1 to D5, amendment 2, the residual-measure lesson: the relative residual is
amplified by `kappa = ||A - sigma B|| ||x|| / ||A x||`); ADR 0587 (whole: arithmetic 1 to 4, the limit `2 tolerance`, the
measured `eta`); ADR 0590 (D3, arithmetic 2 and 3: the D4 rounding on the Full Hex8 bar, B0: `K_T(0)` equals the linear
stiffness in every bit on that bar); ADR 0596 (D1 reading, D3 measures, Bar F records RNv `0.92`, RNa `2.0`, amendment 1: RNq
moved to `8.3e-5` under MC); ADR 0599 (the probe practice, the `AtZero` test contribution, the "For the lead" form). Sources at
6c38e510: `crates/rcfd-fem/src/follower.rs` (`FollowerPressure::add`, `add_full`, `load_stiffness`, `contribution_triplets`,
`tangent_buckling_factors_with`, `ComplexBuckling`, `tangent_modes_full`, `buckling_factors_full`, `expand`, `pencil_failed`),
`nonlinear.rs` (`Contribution`, `add_triplets`, `check_driver_model`), `mpc.rs` (`zero_shift_failing_pivot`: `SkylineLdlt`,
which validates symmetry), `rcfd-linalg/src/eigen_pencil.rs` (module documentation, options, result),
`crates/rcfd-fem/tests/sa_state_contributions_f671.rs` (`State`, `d4_of`, `bar`, Bar F).

## Set-up probe (disclosed; set-up quantities only)

Bar of ADR 0590/0596 (`hex_box([0;3], [2, 0.5, hz], [8, 2, 2])`, Full Hex8, `neo_hookean(1e6, 1e7)`, `rho = 1000`, `xmin`
held in `x, y, z`), `FollowerPressure::new(.., "xmax", p)` alone, `NewtonOptions::new(10).unsymmetric(true)`:

| `hz` | `p` (Pa) | Newton | max `|u|` per component (m) | dead linear factors (`buckling::factors`, dead pressure) | `buckling_factors_full` at rest, 6 factors | `residuals` / `eta`, worst | `residuals / eta` |
|---|---|---|---|---|---|---|---|
| 0.5 | 9e3 | 10 steps | `5.84e-3, 3.6e-4, 3.6e-4` | `5.4535, 5.4535, 36.15` | real, `111.111 .. 112.178` (a near-multiple cluster), 86 steps | `2.6e-10`, `1.3e-12` | `186 .. 439` |
| 0.4 | 9e3 | 10 steps | `5.85e-3, 3.5e-4, 2.9e-4` | `3.7375, 5.3910, 27.64` | three complex pairs `107.601 +/- 0.655 i`, `110.543 +/- 0.787 i`, `111.331 +/- 0.096 i`, 74 steps | `1.06e-10`, `3.7e-13` | `288 .. 2049` |
| 0.4 | 1.5e4 | 10 steps | `9.7e-3, ...` | `2.2425, ...` | the same values scaled by `9e3/1.5e4` | `1.07e-10`, `3.7e-13` | same |

Reading. The square section has exactly double lateral factors (the dead pair `5.4535` twice) and its full spectrum is a cluster
of near-multiple real values, which a single-vector Arnoldi process resolves poorly; the section `0.5 x 0.4` separates the two
planes and gives distinct complex pairs. The factors of smallest modulus of the full static pencil sit at `mu p ~ 9.7e5 ..
1.0e6 Pa`, near the shear modulus `mu_s = 1e6` of the material (`111.11 x 9e3 = 1.0e6`): they are modes of the loaded end face
under the follower (the static pencil of Beck's column has no physical divergence factor, ADR 0580 arithmetic 3; the
coefficient `20.05` is a flutter, i.e. modal, quantity), and they are complex: the eigenvalues a2 is about.

## The set-up (registered)

- **Model ("Bar R")**: `hex_box([0, 0, 0], [2, 0.5, 0.4], [8, 2, 2])`, Full Hex8 (the default formulation of `hex_box` as in
  ADR 0590 Bar C), `Material::hyperelastic(neo_hookean(1e6, 1e7), 1000)`, `xmin` held in all three components. 81 nodes, 243
  equations, 216 free.
- **Load**: `fp = FollowerPressure::new(&bar, "xmax", P_F)`, `P_F = 9e3` Pa (positive: pressing on the end face, a compressive
  force `P = 1800` N that stays normal to the deformed end face: Beck's load). Free-edged: the four edges of the loaded face are
  free (checked by SA below).
- **State**: `solve_with(bar, [], [], NewtonOptions::new(10).unsymmetric(true), contributions = [&fp])`, `u*`, states, at
  `lambda* = 1` (known: 10 steps). **Compressed, below Euler's load**: the dead linear factor is `3.7375` (known), so the state
  carries `1/3.7375 = 0.268` of the dead Euler load of the mesh. Beck's flutter load of this bar (Euler-Bernoulli, `E = 9 K mu_s
  / (3 K + mu_s) = 2.903e6` Pa, `E I = 2.903e6 x 0.5 x 0.4^3 / 12 = 7742` N m^2 in the weak plane, `P L^2/(E I) = 1800 x 4 /
  7742 = 0.930` against `20.05`) is far above the state. F-653's limit stands: the Newton driver refuses an indefinite tangent,
  so states above the Euler load are not reached; this state is below it.
- **Pre-stress**: `Prestress::from_static` of the linear static solve of a copy with the dead `Load::pressure("xmax", P_F)`.
- **Calls**: `tangent_buckling_factors_full_with(bar, [], u*, states, 1.0, [&fp], pre, BucklingOptions::new(6))` (default
  tolerance `1e-10`); `tangent_modes_full(bar, [], u*, states, 1.0, [&fp], ModalOptions::new(4))` (default tolerance `1e-10`,
  shift 0).

## Arithmetic (disclosed; by hand, after the probe)

1. **The pencil about the state** (first principles, extending ADR 0596 D1 to the full tangent). The residual `r(w, lambda) =
   f_int(w) - c(w, lambda)`, `c` the contributions' forces (`Contribution::add_full` adds `-c` to the force and the full tangent
   `-dc/dw`). Its tangent is `K(w, lambda) = K_T(w) + K_c(w, lambda)`, `K_c` the triplets of `add_full` (for the follower `-K_p(w,
   lambda)`, unsymmetric). The one-point linearisation about `(u*, lambda*)` in the load factor: `K(u*, lambda* + mu) ~
   K(u*, lambda*) + mu (K_sigma(S_ref) + K_c'(u*))`, `K_c' = K_c(u*, 1) - K_c(u*, 0)` (exact for a contribution linear in the
   load factor; the follower's `K_p` is `lambda` times that at 1). The factors: `(A + mu G) x = 0`, `A = K_T + K_c(u*, lambda*)`,
   `G = K_sigma + K_c'`, the pencil `A x = mu B x`, `B = -G`, reduced on the prescribed equations. At `u = 0`, initial states,
   `lambda* = 0`: `K_T(0)` is the linear stiffness (bit-equal on this element and material, ADR 0590 B0), `K_c(0, 0) = 0`
   (pressure zero), `K_c' = -K_p(0, 1)`: the pencil of `buckling_factors_full` (`K x = mu B x`, `B = -(K_sigma - K_p)`).
2. **The central-difference residual equation (D2, registered).** `D4_f[v] = (8 (f(u* + a v) - f(u* - a v)) - (f(u* + 2 a v) -
   f(u* - 2 a v))) / (12 a)`, `a = 1e-4` m (ADR 0590 D3), of `r(w) = internal_force(bar, w, states) - fp.force(w, 1)` and of
   its load-factor derivative `g(w) = -fp.force(w, 1)`. A complex direction is taken by parts: `D4[x] = D4[Re x] + i D4[Im x]`
   (the derivative is linear). For a pair `(mu, x)` with `x` scaled so that `max_k |x_k| = 1` (divided by its entry of largest
   modulus): `e = D4_r[x] + mu (K_sigma x + D4_g[x])` and **`V_B = max_(k free) |e_k| / max_(k free) |D4_r[x]_k|`** (moduli;
   `K_sigma` the assembled `geometric_stiffness` of `pre`, applied to the complex vector). The modal equation: `e = D4_r[phi] -
   omega^2 M phi` with the consistent mass, **`V_M`** likewise. These are the "central-difference residual equations of the
   nonlinear residual including the follower force": `D4_r[x] -> (K_T - K_p(u*, 1)) x` and `D4_g[x] -> -K_p(u*, 1) x` as `a ->
   0`, so `e` is the residual of the exact linearised problem for the returned pair.
3. **Expected error.** `e = r_pen + t_r + mu t_g`, `r_pen = A x - mu B x` the pencil residual of the returned pair, `t_r =
   D4_r[x] - A x`, `t_g = D4_g[x] - K_c' x` (zero for an exact consistent tangent, except for the difference itself). (i) The
   pencil part: `||r_pen||_2 = eta (||A||_F + |mu| ||B||_F) ||x||_2` and the library's `residuals` is `||r_pen|| / (||A x|| + |mu|
   ||B x||)`; with `A x ~ mu B x` the 2-norm ratio `||r_pen|| / ||A x|| ~ 2 residuals`, and the max-entry ratio is within a
   factor `sqrt(n_f) = sqrt(216) = 14.7` of it: `V_B,pen <= 29.4 residuals`. The probe's rest pencils: `residuals <= 1.1e-10`
   (`eta <= 3.7e-13`, `kappa = residuals/eta <= 2.0e3`); at the state the operators differ by `O(1/mu) ~ 1e-2` and the same
   orders are expected: `V_B,pen` typical `2e-10`, worst `3.2e-9`. The worst case from the tolerance alone (`eta <= rho tol`,
   `kappa <= 2e3`, factor 29.4): `5.9e-6`; disclosed as the risk of a2 (the measured `eta` of ADR 0587 and the probe are 3 orders
   below the tolerance bound). (ii) The difference part: the internal-force rounding per entry of the bar `~1.8e-10` N (ADR 0590
   arithmetic 3), D4's `1.5 x 1.8e-10 / a = 2.7e-6` N, against `max |D4_r[x]|`: for the end-face modes `x` (unit entry at a face
   node) the stiffness entries `(K + 4 mu_s/3) h_e ~ 1.1e7 x 0.2 = 2e6` N/m give `max |A x| ~ 1e5 .. 1e6` N: `<= 3e-11`. The
   follower force (`~p h_e^2 ~ 9e3 x 0.05 = 450` N per node) rounds at `1e-13` N: negligible. Truncation: the follower force is
   quadratic along a line (exact in D4); the neo-Hookean force is not polynomial, its D4 error `(a^4/30) f^(5)` with strain
   increments `a/h_e = 5e-4` per unit `x`: relative `~(5e-4)^4 x 10^2 ~ 6e-12`. **Estimate of BV: `<= 3.2e-9` (typical
   `2e-10`); limit `1e-6` (the acceptance).**
   Modes (MV): `kappa` of the low modes of this coarse bar `~ omega_max^2/omega_1^2 ~ 4.6e4` (ADR 0586 arithmetic 5, quoted in
   ADR 0590 arithmetic 4); with `eta ~ 4e-14 .. 4e-13` (ADR 0587 B1 on Beck's column, and the probe): `residuals ~ 2e-9 ..
   2e-8`, `V_M,pen <= 29.4 x 2e-8 = 5.9e-7`, typical `~4e-9`; the difference part as BV but against `max |K_T phi| >= 100` N
   (ADR 0590 arithmetic 3): `<= 2.7e-8`. **Estimate of MV: typical `1e-8`, worst `6e-7`; limit `1e-6`** (the acceptance's, taken
   for the modal path too; the risk is disclosed: the worst case is near the limit).
4. **a3.** ADR 0587 arithmetic 2 to 4 at shift 0: `eta <= rho tolerance + delta < 2 tolerance`. Probe: `<= 3.7e-13`. Limit `<=
   2e-10` for every pair (BE, ME).
5. **a4.** By arithmetic 1 the two problems are the same; in the operations: `nonlinear::tangent(0)` and `point_stiffness` are
   bit-equal on this bar (ADR 0590 B0); `add_full` at `lambda = 0` adds triplets of value `+/-0` to `K_T`; the increment is the
   triplets of `-K_p(0, 1)` in the order of `load_stiffness` (as `buckling_factors_full` builds `extra`) followed by `+/-0`
   triplets; `add_triplets` adds them one by one in place, and adding a zero leaves every value unchanged. Estimate **`0`**
   (bit-equal); limit `1e-10` (the acceptance).
6. **Negative controls** (each a different, wrong problem measured by the registered `V_B` or `V_M`).
   - **NS**: the full path with a test contribution `SymPart(&fp)` whose `add_full` is `fp.add` (the symmetric part of the load
     stiffness, as `tangent_buckling_factors_with`): its pairs miss the antisymmetric edge term. On the square bar the symmetric
     path's vector measure was `0.92` for the global mode (ADR 0596 RNv); here the end-face modes exist because of the
     follower, whose share is dominant: estimate `0.1 .. 1`. Limit `>= 1e-2`.
   - **NZ** (the acceptance's state, inverted): `AtZero(&fp)` (both `add` and `add_full` evaluated at `u = 0`): the face's
     state geometry is lost. The state strain is `~3e-3` (`max |u_x| = 5.85e-3` m over 2 m, and the rotations of the face edges
     `~3.5e-4/0.25`): `K_p(u*) - K_p(0) ~ 3e-3 K_p`, the follower share `~1`: estimate `1e-4 .. 3e-3` (ADR 0596 MC on the square
     bar, Rayleigh, global mode, `K_c'` only: `8.3e-5`). Limit `>= 1e-5`.
   - **NG**: `V_B` with `D4_g` dropped (`e = D4_r[x] + mu K_sigma x`): the follower's load-factor derivative carries the
     increment of these face modes: estimate `~1`. Limit `>= 1e-2`.
   - **NR**: the rest pairs of `buckling_factors_full` measured by `V_B` at the state: `mu` off by `~1` in `107` and the tangent
     by the state: estimate `~1e-2`. Limit `>= 1e-3`.
   - **NSM**: the modes of the symmetric `tangent_modes` (same state, `[&fp]`, `ModalOptions::new(4)`, its default residual
     test; a mode is real) measured by `V_M` with their `omega^2`: the antisymmetric part at the load factor 1 against the
     bending stiffness; on the global buckling mode at `19.5` times the load the share was `0.92`, at `1` time `~0.05`.
     Estimate `1e-2 .. 1e-1`. Limit `>= 1e-3`.
   - **SA**: `max_(ij free) |K_p,ij - K_p,ji| / max |K_p,ij|` of `fp.load_stiffness(u*, 1)`: free-edged, ADR 0596 RNa `2.0`
     on the square bar; limit `>= 1e-1`.

## Design

### D1. `follower::tangent_buckling_factors_full_with` (`follower.rs`, additive)

```text
pub fn tangent_buckling_factors_full_with(model, prescribed: &[Prescribed], u: &[f64], states: &[ElementState],
                                          load_factor: f64, contributions: &[&dyn Contribution], prestress: &Prestress,
                                          options: &BucklingOptions) -> Result<ComplexBuckling>
```

- The model checks of `tangent_buckling_factors_with` (`validate`, `check_supported_nonlinear`, `check_driver_model`, so the
  transformation path and temperature loads are refused with key `"nonlinear"`); the prescribed equations (model's and
  `prescribed`, zero values); the rigid-motion test.
- `K_T = nonlinear::tangent(u, states)`, `K_sigma = assemble_geometric(prestress)`; with contributions: the triplets of
  `add_full` at `(u, load_factor)` added to `K_T`; the triplets of `add_full` at `(u, 1)` followed by the negated triplets at
  `(u, 0)` added to `K_sigma` (errors keyed `"contribution"` as `contribution_triplets`), each by `add_triplets`.
- Reduced; `options.factors` checked as `tangent_buckling_factors_with` (key `"factors"`).
- No ADR 0402 zero-shift pivot test: it factors by the symmetric `SkylineLdlt`, which refuses an unsymmetric matrix; the
  refusal of a singular `A` is that of the pencil's `EnvelopeLu` factor (`Factor`, reported as `FemError::Solver {
  "nearest_pencil_eigenpairs" }`). (Open question 1.)
- `B = -G` reduced; `nearest_pencil_eigenpairs(A, B, options.factors, PencilOptions::new().tolerance(options.tolerance))`,
  shift 0; result `ComplexBuckling` (factors, modes expanded, residuals, backward_errors, steps, unknowns, dofs). The other
  fields of `options` (residual test, iteration cap, subspace size, factorisation, coordinate order) are not used, as in
  `buckling_factors_full`.
- Module documentation: one bullet. Nothing else in the library changes; no existing line is edited.

### D2. Tests (`crates/rcfd-fem/tests/sa_flutter_state_f676.rs`)

- `flutter_state_buckling` (S0, SA, BT, BV, BE, BZ, NS, NZ, NG, NR, R1).
- `flutter_state_modes` (S0, MT, MV, ME, NSM).
- Helpers copied from `sa_state_contributions_f671.rs` (that file is not edited); the test contributions `SymPart` and
  `AtZero` (`AtZero::add` calls `fp.add(0, ..)`, `add_full` calls `fp.add_full(0, ..)`).

## Checks (limits fixed now)

Every call must be `Ok` and every measure finite before it is compared (otherwise a miss). "Known": measured by the probe.

| Id | Clause | What | Limit | Estimate |
|---|---|---|---|---|
| S0 | set-up | the unsymmetric Newton solve of Bar R under `P_F` | `Ok` | known: 10 steps |
| SA | a2 (free-edged) | antisymmetry of `K_p(u*, 1)` on the free unknowns | `>= 1e-1` | `~2` |
| BT | a1 | `tangent_buckling_factors_full_with` at the state: `Ok`, at least 6 pairs, a complex pair among them (`|Im mu| > 1e-6 |mu|`) | holds | the probe's rest spectrum shifted by `~-1` |
| BV | a2 | `max` over the pairs of `V_B` | `<= 1e-6` | `<= 3.2e-9` (typical `2e-10`) |
| BE | a3 | `max eta` of the pairs (`backward_errors`), and `backward_errors.len() == factors.len()` | `<= 2e-10`; holds | `~4e-13` |
| BZ | a4 | at zero load against `buckling_factors_full`: same count, `max_j |mu_j - mu_ref,j| / |mu_ref,j|` | `<= 1e-10` | `0` |
| NS | negative | `V_B` of the pairs of the full path with `SymPart(&fp)` | `>= 1e-2` | `0.1 .. 1` |
| NZ | negative | `V_B` of the pairs with `AtZero(&fp)` | `>= 1e-5` | `1e-4 .. 3e-3` |
| NG | negative | `V_B` without `D4_g`, the pairs of BV | `>= 1e-2` | `~1` |
| NR | negative | `V_B` at the state of the rest pairs (BZ's reference) | `>= 1e-3` | `~1e-2` |
| R1 | D1 | refusals: `factors = 0` (key `"factors"`); the bar with no constraint (`RigidBodyMotionFree`) | hold | |
| MT | a1 (modal) | `tangent_modes_full` at the state: `Ok`, 4 or 5 pairs | holds | real `omega^2` (`p = 0.93` against `20.05`) |
| MV | a2 (modal) | `max` over the pairs of `V_M` | `<= 1e-6` | typical `1e-8`, worst `6e-7` |
| ME | a3 (modal) | `max eta` of the modal pairs | `<= 2e-10` | `<= 4e-13` |
| NSM | negative (modal) | `V_M` of the symmetric `tangent_modes` pairs | `>= 1e-3` | `1e-2 .. 1e-1` |
| L0 | defaults | `git diff 6c38e510 -- crates/*/src` removes no line (additions only) | holds | |
| N1 | defaults | the existing tests of the touched file's paths: `sa_f653` (non-ignored), `sa_f653_default_bits`, `sa_f664`, `sa_f664_default_bits`, `sa_state_contributions_f671`, `sa_state_contributions_f677`, `sa_follower`, `sa_follower_default_bits` (release) | pass | |
| N2 | defaults | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | |
| S1 | build | `cargo fmt -p rcfd-fem --check`; `cargo clippy -p rcfd-fem --all-targets -- -D warnings` | clean | |

Each test is estimated under 10 s in release (243 equations, about 50 force evaluations).

## Planned mutations (scratch copy by `git archive`, own target, one at a time; the new test file run for each)

| Id | Mutation (in D1) | Can be caught by |
|---|---|---|
| MU1 | the tangent at the state by `add` (symmetric part) in place of `add_full` | BV, BZ not (zero at rest) |
| MU2 | the increment by `add` in place of `add_full` | BV, BZ |
| MU3 | the increment not added to `K_sigma` | BV, BZ |
| MU4 | `B = +G` | BV, BZ |
| MU5 | every contribution tangent evaluated at `u = 0` | BV (the size of NZ) |
| MU6 | the tangent at the state at load factor 1 in place of `load_factor` | BZ (at `lambda* = 0`) |
| MU7 | `point_stiffness` in place of `nonlinear::tangent` | BV |

A mutation is caught if any registered check misses. None is declared equivalent.

## Decision rule

- **done**: every check met, MU1 to MU7 caught, no check met only after a post-run amendment, L0, N1, N2, S1 met.
- **partial**: L0, N1, N2 and S1 met, but a registered check missed (kept as a `*_known_miss` test, diagnosed), or a mutation
  not caught, or a check met only after a post-run amendment.
- **todo**: otherwise, in particular any change of the default bits.

## Limits (not covered)

- One bar, one load level, one mesh, one material (neo-Hookean, Full Hex8); shells and beams not covered (the driver's model
  checks); `CentrifugalFollower` with `add_full` (its default, the symmetric `add`) not checked.
- States below the Euler load only: the Newton driver refuses an indefinite tangent (ADR 0580). The flutter load of Beck's
  column about a nonlinear state (bisection of the modal path in the load, each load a Newton solve) is not searched; the bar
  is stubby (`L/h = 5`) and the state far below flutter, so the modal check has real `omega^2`; the complex eigenvalues of the
  check are those of the buckling pencil (the end-face modes).
- No ADR 0402 zero-shift pivot test on the new path (D1); no case-file key.
- The one-point linearisation in the load factor (`K_sigma(S_ref)` for `dK_T/dlambda`), as ADR 0590/0596: the factors are not
  those of a nonlinear path-following buckling search.

## Open questions taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. **The pivot test.** Options: (a) none (the pencil's LU refuses a singular `A`); (b) the ADR 0402 test on the symmetric part
   of `A`; (c) a test on the `EnvelopeLu` pivots. Taken (a): (b) refuses states whose symmetric part is indefinite but whose
   full tangent is regular (Beck's column above the symmetric divergence load), (c) needs a new linalg API.
2. **Rectangular section.** Taken `0.5 x 0.4` in place of ADR 0596's square bar: the square section has exactly double
   factors, which a single-vector Arnoldi process resolves poorly (probe: a near-multiple cluster, 86 steps).
3. **The modal path checked too** (MV, ME, NSM), beyond the acceptance's buckling reading, because the item's title is flutter
   and `tangent_modes_full` is the flutter path about a state.
4. **Case-file keys.** Not added (a schema golden would change).

## Addendum 1 (2026-11-15): results

Commits: 12430655 (registration), 2cac357f (library function D1 and the test file D2, run 1), and this addendum. The registered
run was made once, in release, `cargo test -p rcfd-fem --release --test sa_flutter_state_f676 -- --nocapture
--test-threads=1` on 2cac357f (`w49i-work/f676-scratch/run1.log`). N1 (`n1.log`), N2 (`n2.log`), S1 (`clippy.log`) and the
mutations (`mutate.py`, `mutations.log`, a `git archive` of 2cac357f with its own target) on 2cac357f. Shared solver file
changed: `crates/rcfd-fem/src/follower.rs`, additions only (88 lines: two module-doc lines 25-26 and the new function
`tangent_buckling_factors_full_with`, lines 984-1069 at 2cac357f); no existing line edited.

| Id | Measured | Limit | Met |
|---|---|---|---|
| S0 (known) | converged, 10 steps, max `|u|` `5.848e-3` m | `Ok` | yes |
| SA | `2.00` | `>= 1e-1` | yes |
| BT | 7 pairs at the state: `49.491 +/- 66.357 i`, `69.620 +/- 63.999 i`, `-107.207`, `107.595 +/- 0.660 i`; 67 steps | holds | yes |
| BV | `3.08e-9` | `<= 1e-6` | yes |
| BE | `8.35e-13` (7 of 7 lengths) | `<= 2e-10` | yes |
| BZ | `0` (factors, residuals and backward errors print identically; 74 steps both) | `<= 1e-10` | yes |
| NS | `1.05` (the symmetric part gives six real factors `-9.88 .. 41.8`) | `>= 1e-2` | yes |
| NZ | `2.41e-3` | `>= 1e-5` | yes |
| NG | `5.41` | `>= 1e-2` | yes |
| NR | `6.58e-3` | `>= 1e-3` | yes |
| R1 | `factors = 0` refused (key `"factors"`); rigid-free refused (`RigidBodyMotionFree`) | hold | yes |
| MT | 4 pairs, `omega^2 = 46.257, 64.587, 596.22, 1210.9` (imaginary parts `0`), 15 steps | holds | yes |
| MV | `4.33e-9` | `<= 1e-6` | yes |
| ME | `9.70e-13` | `<= 2e-10` | yes |
| NSM | `1.02` (symmetric `omega^2 = 45.838, 64.296, 596.21, 1210.9`) | `>= 1e-3` | yes |
| L0 | `git diff --numstat 6c38e510 -- crates/*/src`: `88 0 follower.rs` | additions only | yes |
| N1 | `sa_f653` (6 passed, 2 ignored), `sa_f653_default_bits`, `sa_f664`, `sa_f664_default_bits`, `sa_state_contributions_f671` (5), `sa_state_contributions_f677` (3), `sa_follower` (5), `sa_follower_default_bits`: all pass | pass | yes |
| N2 | `baseline_hashes`: 3 passed, 1 ignored | pass | yes |
| S1 | `cargo fmt -p rcfd-fem --check` clean; `cargo clippy -p rcfd-fem --all-targets -- -D warnings` clean | clean | yes |

**Mutations** (one at a time):

| Id | Caught by (measured) |
|---|---|
| MU1 state tangent by `add` | BV `0.29` |
| MU2 increment by `add` | BT (no complex pair), BV `1.05`, BZ `1.12` |
| MU3 increment not added | BT, BV `0.99`, BZ `0.97`, NG `9.8e-10` |
| MU4 `B = +G` | BV `2.0`, BZ `2.0` |
| MU5 every contribution tangent at `u = 0` | BV `2.41e-3` (equal to run 1's NZ, as D2 predicts) |
| MU6 state tangent at load factor 1 | BZ `0.86` |
| MU7 `point_stiffness` for the driver tangent | BV `4.5e-2` |

Seven of seven caught, each by a check of its listed column.

**Deviations and remarks.** None from the registration; no amendment. BV (`3.1e-9`) sits at the registered worst estimate of the
pencil part (`3.2e-9`), driven by the last returned pair (`residuals 1.7e-9`, `eta 8.4e-13`); MV (`4.3e-9`) at the typical end of
its estimate. The spectrum of the full buckling pencil changes with the state: the two complex pairs of smallest modulus at the
state (`|mu| ~ 83, 95`) are absent at rest, where the end-face pairs near `mu p ~ mu_s` lead; NR (`6.6e-3`) measures the rest
pairs against the state. On this stubby bar at `0.93 E I/L^2` the follower moves the modal `omega^2` by `0.3 .. 0.9` percent and
the symmetric part gives the right eigenvalues to that order, yet its modes miss the residual equation by `1.0` (NSM): the
antisymmetric edge term is first order in the mode.

Not run: the whole workspace suite and the whole `rcfd-fem` suite (the lead's gate); only the files that cover `follower.rs`.

**Decision: done** by the registered rule. Every check met in the one registered run, seven of seven mutations caught, no
amendment, L0, N1, N2 and S1 met.

**Remaining questions** (none changes a default):

1. The zero-shift pivot test (open question 1): options (a) none, as now; (b) the ADR 0402 test on the symmetric part; (c) a
   pivot report from `EnvelopeLu`. Recommend (a) until a state near a critical point needs a clearer refusal message.
2. A flutter search about nonlinear states (the onset of a complex modal pair along a Newton path, Beck's column on a slender
   bar): the driver refuses states above the Euler load (ADR 0580), so the onset at `20.05 E I/L^2` cannot be reached by load
   control. Options: (a) a follow-up with an arc-length or a displacement-controlled path whose tangent is unsymmetric-capable;
   (b) nothing. Recommend (b) until a use needs flutter of a pre-deformed (not straight) state; the straight column is covered by
   ADR 0580.
3. `CentrifugalFollower::add_full` is the default (`add`): its tangent is symmetric, so the full path takes it unchanged. No
   action recommended.

## For the lead: roadmap entries

> Proposed status: done.

> Structural solver, advanced tier. Follow-up of F-671 and F-653 (ADR 0596, ADR 0580). DONE. ADR 0600, 2026-11-15: new follower::tangent_buckling_factors_full_with, opt-in beside the unchanged symmetric functions: the buckling pencil (K_T + K_c(u, lambda) + mu (K_sigma + K_c(u, 1) - K_c(u, 0))) x = 0 about a nonlinear state with the full (unsymmetric) tangents of the contributions (Contribution::add_full; for a follower pressure -K_p), complex factors of smallest modulus by the pencil solver of ADR 0580, with the backward errors of ADR 0587; tangent_modes_full (ADR 0580) is the modal path about the state and is checked by the same measure. Set-up: Full Hex8 neo-Hookean cantilever 2 x 0.5 x 0.4 m (8 x 2 x 2), follower end pressure 9e3 Pa (Beck's load, free-edged: antisymmetry of K_p on the free unknowns 2.0), unsymmetric Newton state at 0.27 of the dead Euler load of the mesh (below it; the driver refuses an indefinite tangent). Against the fourth-order central difference of the nonlinear residual with the follower force (complex directions by parts), vector measure max |D4_r[x] + mu (K_sigma x + D4_g[x])| / max |D4_r[x]|: buckling pairs (three complex pairs and a real factor) 3.1e-9, modes 4.3e-9 (limit 1e-6); backward errors at most 8.4e-13 and 9.7e-13 (limit 2e-10); at zero load the factors equal buckling_factors_full in every bit (limit 1e-10). Negative controls: the symmetric part through the full path 1.05, the symmetric tangent_modes 1.02, every contribution tangent at u = 0 2.4e-3, the load-factor derivative dropped 5.4, the rest pairs at the state 6.6e-3. Seven of seven mutations caught; the existing follower, F-653, F-664, F-671, F-677 tests and the baseline hashes pass. Not covered: a flutter search along a nonlinear path (states above the Euler load are not reachable by the driver), shells and beams, one bar and load level.

> F-671 note, proposed addition: the full unsymmetric path about a state, which ADR 0596 D1 left to a follow-up, is follower::tangent_buckling_factors_full_with (ADR 0600, 2026-11-15).
