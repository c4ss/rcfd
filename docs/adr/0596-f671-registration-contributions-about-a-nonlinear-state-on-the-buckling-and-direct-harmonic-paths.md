# ADR 0596: F-671 registration: contributions about a nonlinear state on the buckling and direct harmonic paths

Status: **REGISTERED 2026-11-15, written before any code of F-671 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-671 was written and nothing of F-671 was compiled or run: the files
under "What was read" were read and the arithmetic below was done by hand (no script, no probe, no rcfd call). Measured numbers
quoted below come from the addenda of ADR 0480 and ADR 0590, not from a new run. Results go in the addenda at the end; nothing
above the addenda is changed after a run.

Roadmap item F-671 (dep F-669), structural solver, advanced tier, the follow-up proposed by
[ADR 0590](0590-f669-registration-buckling-and-harmonic-response-about-a-nonlinear-hyperelastic-state.md) (its remaining
question 2). Number 0596 given by the lead. Branch `f671-work` from the local main tip 8da47021 (which holds F-665, F-669),
worktree `w49i-work/f671`, scratch `w49i-work/f671-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number); each registered run made once; existing tests not edited;
defaults bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new
dependency, no unsafe code; clean-room. The roadmap files are not edited (the entries go in "For the lead"). Every choice below
that the maintainer would normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the
recommendation).

## What F-671 asks

The acceptance, binding:

> Every limit not stated here is registered in the ADR before the run. tangent_buckling_factors and DirectHarmonic::about_state take the tangents of the driver's contributions (a follower pressure) at the state; their factors and responses agree with central differences of the nonlinear residual including the follower force within 1e-6, and with no contribution given they equal the paths of ADR 0590 in every bit.

Clauses: **a0** every limit registered here; **a1** the buckling path about a state takes the tangents of the driver's
contributions (a follower pressure) at the state; **a2** the same for the direct harmonic path; **a3** the buckling factors agree
with central differences of the nonlinear residual including the follower force within `1e-6`; **a4** the harmonic responses
agree likewise within `1e-6`; **a5** with no contribution given both equal the paths of ADR 0590 in every bit.

## What was read

`AGENT_RULES.md`; the brief; ADR 0590 in full (D1 to D4, the arithmetic, the checks, the addendum: MU6 caught only by the set-up
check S0, the lesson that every check that can catch a mutation is listed, set-up checks included); ADR 0480 (D1, D3, arithmetic
3: the antisymmetric part of a pressure's load stiffness is an edge term `p int (delta u x Delta u) . dx`, zero on the quarter
ring whose edges lie on the symmetry planes and the held plane; S2 `3.4e-16`; B1 `3.0104 E I/R^3`); ADR 0560 (D1, D2, D4: the
load-factor derivative of a contribution's force, `add_full`, `NewtonOptions::unsymmetric`); ADR 0580 and 0587 (the complex
pencil solver for the full unsymmetric load stiffness, `buckling_factors_full` and `tangent_modes_full` at rest, the backward
error); ADR 0572/0573 (the "For the lead" form). Sources at 8da47021: `crates/rcfd-fem/src/follower.rs` (module doc;
`FollowerPressure` `force`, `load_stiffness`, `symmetric_negative`, `face_pressure` (linear in the load factor), the `Contribution`
impl; `CentrifugalFollower` (linear in the load factor); `buckling_factors` 480-; `tangent_modes` 501-583 (how contributions are
added: `c.add(u, load_factor, scratch, triplets)`, errors keyed `"contribution"`, `add_triplets`); `tangent_buckling_factors`
597-668), `crates/rcfd-fem/src/buckling.rs` (`factors_with` 158-: the extra triplets added to `K_sigma` by `add_triplets`),
`crates/rcfd-fem/src/harmonic_direct.rs` (module doc; the `DirectHarmonic` fields; `about_state` 791-870; `finish` 875-1046;
`value` 1084-1107: the Rayleigh stiffness `k_total (+ geometric)`; the factor `ComplexSkylineLdlt`, symmetric),
`crates/rcfd-fem/src/nonlinear.rs` (`Contribution` 1349-1405, `add_triplets` 1424-, `NewtonOptions::unsymmetric`, `solve_with`,
`NonlinearResult`), `crates/rcfd-linalg/src/` (listing: `skyline_lu.rs` real unsymmetric; no complex unsymmetric factor),
`crates/rcfd-fem/tests/sa_follower.rs` (`ring_model`, `buckling_ring`, B1 to B3, S2), `crates/rcfd-fem/tests/
sa_hyper_state_paths_f669.rs` (set-ups, `State::d4`, `harmonic_error`, `buckling_row`).

## Audit

- `tangent_buckling_factors` and `DirectHarmonic::about_state` (ADR 0590) take no contribution: `K_T` is `nonlinear::tangent`
  alone. `tangent_modes` takes contributions (ADR 0480 D7), so the pattern exists.
- Their signatures are public and used by the ADR 0590 tests, which are not edited. So the contributions come through two new
  entry points (D1, D2), and the two ADR 0590 functions become calls of them with no contribution (clause a5 by construction,
  checked by E0 and E1 below). Reading of the acceptance's "tangent_buckling_factors and DirectHarmonic::about_state take":
  these two paths, by their `_with` forms. Taken on the maintainer's standing instruction of 2026-11-11 (take the
  recommendation).

## Design

### D1. Buckling about a state with contributions (`follower.rs`, additive)

```text
pub fn tangent_buckling_factors_with(model, prescribed: &[Prescribed], u: &[f64], states: &[ElementState],
                                     load_factor: f64, contributions: &[&dyn Contribution],
                                     prestress: &Prestress, options: &BucklingOptions) -> Result<BucklingResult>
```

The eigenproblem `(K_T(u) + K_c(u, lambda*) + mu (K_sigma(S_ref) + K_c'(u))) x = 0`:

- `K_T(u)` the driver's tangent at the state (as ADR 0590 D1);
- `K_c(u, lambda*)` the sum of the contributions' tangents at `(u, load_factor = lambda*)`, the triplets of `Contribution::add`
  (as `tangent_modes` and the Newton driver take them), added to `K_T` by `add_triplets`;
- `K_c'(u) = K_c(u, 1) - K_c(u, 0)`, the derivative of the contributions' tangents with the load factor at fixed `u`, added to
  `K_sigma` of the given pre-stress by `add_triplets` (as `factors_with` adds its extra triplets).

Derivation (first principles, extending ADR 0590 D1). The residual along the path is `r(u, lambda) = f_int(u) - lambda f_dead -
c(u, lambda)` with `c` the contributions' forces (`Contribution::add` adds `-c` to the force and the tangent `-dc/du`). Its
tangent is `K(u, lambda) = K_T(u) + K_c(u, lambda)`. The linearised (one-point) estimate about the state `lambda*` takes
`K(u*, lambda* + mu) ~ K(u*, lambda*) + mu dK/dlambda`, with `dK_T/dlambda` replaced by `K_sigma(S_ref)` (the geometric stiffness
of the stress increment of the reference load, as ADR 0590) and the explicit load-factor dependence of the contributions'
tangents at fixed `u` given exactly by `dK_c/dlambda`. Both contributions of the crate are linear in the load factor
(`FollowerPressure::face_pressure` is `lambda p`; `CentrifugalFollower` scales by `lambda Omega^2`), so `dK_c/dlambda = K_c(u, 1)
- K_c(u, 0)` exactly; the doc states this requirement for any other contribution (a contribution nonlinear in the load factor
gets the secant over `[0, 1]`). At `u = 0`, `lambda* = 0` with a follower pressure: `K_c(0, 0) = 0` (pressure `0`) and `K_c'(0)
= -K_p,sym(0, 1)`, so the problem is that of `follower::buckling_factors` (ADR 0480 D3), up to `K_T(0)` against `point_stiffness`
(B0c). With `contributions` empty nothing is added and the operations are those of ADR 0590 D1 in the same order (a5).

**The unsymmetric part (registered reading).** The tangent of `Contribution::add` for a follower pressure is its symmetric part
`-(K_p + K_p^T)/2` (ADR 0480 D1). D1 keeps it: the pencil stays symmetric and `lowest_buckling_factors` solves it (the path of
ADR 0590 unchanged). Reading: where the antisymmetric part couples only held unknowns (a closed surface, or edges on symmetry
planes with the in-plane normal component held; ADR 0480 arithmetic 3) the reduced load stiffness is symmetric and the factors
are those of the full linearised problem; elsewhere (a free-edged follower load, Beck's column) they are the divergence factors
of the symmetric (conservative) approximation, and flutter is not represented: the complex pencil path of ADR 0580
(`buckling_factors_full`, `tangent_modes_full` with `add_full`) exists at rest; about a nonlinear state it is proposed as a
follow-up. Why not the full load stiffness here: the symmetric eigensolver refuses an unsymmetric pencil; the pencil solver gives
complex values and would be a different path from ADR 0590's (whose bits a5 asks to keep). The measure a3 (D3) is the Rayleigh
quotient along each mode, which a quadratic form evaluates with the symmetric part only; so a3 holds for any follower set-up, and
the registered checks of a3 are made on the conservative ring, where the symmetric reading is the full problem; the free-edged
bar is run as records (RN). Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).

Contribution errors are mapped as `tangent_modes` maps them (key `"contribution"`, the contribution's name in the reason). The
old `tangent_buckling_factors(model, prescribed, u, states, prestress, options)` becomes
`tangent_buckling_factors_with(model, prescribed, u, states, 0.0, &[], prestress, options)`.

### D2. Direct harmonic about a state with contributions (`harmonic_direct.rs`, additive)

```text
impl DirectHarmonic {
    pub fn about_state_with(model, prescribed: &[Prescribed], u: &[f64], states: &[ElementState], load_factor: f64,
                            contributions: &[&dyn Contribution], options: &DirectHarmonicOptions) -> Result<DirectHarmonic>
}
```

`A(omega) = K - omega^2 M + i omega (alpha M + beta K + C_d)`, `K = K_T(u) + K_c(u, lambda*)` (the contributions' tangents of
`Contribution::add` at `(u, load_factor)` added to the tangent by `add_triplets` before the pattern test, so a triplet off the
mass pattern is refused with key `"pattern"`). One undamped part `K`, `k_total = K`: the Rayleigh stiffness is the tangent of the
residual, contributions included, as `new` puts the pre-stress `K_sigma` in its Rayleigh stiffness (ADR 0543 D5). Everything
else is ADR 0590 D2 (the checks and refusals, `finish`, the `stress` refusal). The factor of the plain path is the complex
`LDL^T` (symmetric), so the symmetric part of the follower load stiffness is the only form it can take; the reading is D1's
(exact on the conservative ring; the symmetric approximation on a free-edged load, record RNh). The old `about_state(model,
prescribed, u, states, options)` becomes `about_state_with(model, prescribed, u, states, 0.0, &[], options)`.

### D3. The central-difference measures (registered)

`D4[v]` as ADR 0590 D3 (`v` scaled to `max |v_k| = 1`, `a = 1e-4` m), now of the residual including the follower force:
`r(w) = internal_force(model, w, states) - fp.force(w, lambda*)` (no dead load in the set-ups of the checks), and of its
load-factor derivative `g(w) = dr/dlambda = -fp.force(w, 1)`.

- **Buckling (BFD, a3)**: for each mode `xhat`, `mu_FD,i = -xhat . D4_r[xhat] / (xhat^T K_sigma xhat + xhat . D4_g[xhat])`.
- **Harmonic (HFD, a4)**: `e_H` of ADR 0590 D3 with `KX` from `D4_r` (complex moduli, free equations, relative to `max |KX|`).

Both the internal force of the SVK material (Green-Lagrange strain quadratic in `u`, stress linear in it, `B(u)` linear: a cubic
in `u`) and the follower force (`x_u x x_v` with `x` linear in `u`: a quadratic) are polynomials of degree `<= 3` along any line,
for which `D4` is exact: on the ring only rounding remains (arithmetic 2, 3).

### D4. Set-ups (registered)

- **Ring** (conservative): ADR 0480's quarter ring with a thicker section: `R = 1`, `T = 0.1`, `BW = 0.1` m, 1 x 24 x 1 Hex20
  (`hex_box([R - T/2, 0, 0], [T, pi/2, BW], [1, 24, 1])` mapped by `(r, theta, z) -> (r cos theta, r sin theta, z)`,
  `upgrade_to_hex20_mapped`), `Material::isotropic_elastic(2e11, 0, 7850, 0)` (SVK in the driver), held: `ymin` component 1,
  `ymax` component 0, `all` component 2 (`ring_model` of `sa_follower.rs` with `T`, `BW` changed). Why `T = 0.1` and not
  ADR 0480's `0.02`: the residual measure of the harmonic response divides the direct solve's backward error, `~eps ||A|| ||X||`,
  by the bending-scale `|KX|`; their ratio grows like `(R/T)^2 (h/T)` (arithmetic 3), `~7.5e3` at `T = 0.02` and `~65` at `0.1`.
- **Ring load**: `FollowerPressure::new(&ring, "xmax", P_R)`, `P_R = 2.5e6` Pa on the outer face (external, presses on the solid),
  given to `nonlinear::solve_with` as the only contribution, `NewtonOptions::new(4)` (the symmetric tangent is exact here). The
  state `u*` at `lambda* = 1` with its states. **Pre-stress**: `Prestress::from_static` of the linear static solve of a copy with
  `Load::pressure("xmax", P_R)` (dead), the reference of ADR 0480 B1.
- **Ring buckling**: `BucklingOptions::new(2)`.
- **Ring harmonic**: `HarmonicLoad::real(F)`, `F_x = 1e3` N on each node of `ymin` (radial at `theta = 0`); `threads(1)`;
  `omega_1` the lowest circular frequency of `tangent_modes(ring, &[], u*, states, 1.0, &[&fp], ModalOptions::new(2))` at the
  state and of `modal_analysis(ring, ModalOptions::new(2))` at rest; `omega_a = 0.5 omega_1` undamped, `omega_b = 1.5 omega_1`
  with `rayleigh(1.0, 1e-5)`.
- **Bar F** (free-edged, records RN): ADR 0590's Full Hex8 bar `[0, 2] x [0, 0.5] x [0, 0.5]`, 8 x 2 x 2, `neo_hookean(1e6, 1e7)`,
  density 1000, held on `xmin`, no dead load; `FollowerPressure::new(&bar, "xmax", 9e3)` (`2.25e3` N, the compression of ADR 0590
  Bar C as a follower end load), `NewtonOptions::new(10).unsymmetric(true)` (the free edges make the symmetric tangent inexact;
  ADR 0560 D4). Pre-stress of the dead `Load::pressure("xmax", 9e3)`. Buckling `BucklingOptions::new(2)`; harmonic at `0.5
  omega_1` (from `tangent_modes` with the contribution), `F_z = 100` N on each node of `xmax`.
- **ADR 0590 set-ups** (E0, E1): its Bar C NH (buckling) and Bar S NH (harmonic at `omega_a`, `omega_b`), as in
  `sa_hyper_state_paths_f669.rs`.

### D5. Files

`crates/rcfd-fem/src/follower.rs` (D1: the new function, the old one a call of it, module doc), `crates/rcfd-fem/src/
harmonic_direct.rs` (D2: the new constructor, the old one a call of it, module doc), the new test
`crates/rcfd-fem/tests/sa_state_contributions_f671.rs`, this ADR.

## Arithmetic (disclosed, by hand)

1. **Ring state and factors.** `E I/R^3 = 2e11 * 0.1^4 / 12 = 1.667e6` Pa. Thin-ring follower `p_cr = 3 E I/R^3 = 5.0e6` less
   `R/R_o` (ADR 0480 arithmetic 3, `-5` percent at `T = 0.1`) and some percent for the thick section: `~4.7e6`. `P_R = 2.5e6` is
   `~0.53` of it: the circular state is stable, Newton converges (exact symmetric tangent, ADR 0480 S2). Hoop stress `P_R R/T =
   2.5e7` Pa, strain `eps_h = 1.25e-4`. In units of `E I/R^3` and the `n = 2` amplitude (ADR 0480 arithmetic 3): bending `(n^2 -
   1)^2 = 9`, follower load form `-(n^2 - 1) p = -3 p`, dead form `-(n^2 - 1)^2/n^2 p = -2.25 p`, so `K_sigma` gives `-2.25` and
   `-K_p,sym` gives `-0.75` per unit `p`. With `p* = 1.5`: `K_T + K_c ~ 9 - 4.5 = 4.5`, `K_sigma + K_c' ~ -3`: `mu_state ~ 1.5`
   (the thick-ring reductions lower it to `~0.9..1.4`); `n = 4`: `225 - 22.5` against `15`: `~13`. Two positive factors expected.
2. **BFD rounding (ring).** Internal-force rounding per entry: stress `2.5e7` Pa, `B ~ 15` /m, point volume `~2.4e-5` m^3: terms
   `~9e3` N, `~50` of them: `eps_r ~ 5e-11` N. `xhat . r` over `888` equations: `<= 4.5e-8`; `D4` multiplies by `18/(12 a) =
   1.5e4`: `6.8e-4`. `Pi'' = xhat^T K xhat ~ omega_s^2 xhat^T M xhat` with `omega_s^2 ~ 7e4` (rest `omega^2 = 7.2 E I/(rho A R^4)
   = 1.5e5`, times `1 - 0.53`) and `xhat^T M xhat ~ 60` kg (quarter mass `123` kg, half of it at unit amplitude): `4.2e6`. Relative
   `1.6e-10`; the denominator is of the same order (`|mu| ~ 1`), its follower part exact up to rounding (quadratic force). The
   Ritz value equals the Rayleigh quotient of its vector (ADR 0590 arithmetic 2). Estimate **`<= 1e-9`**.
3. **HFD (ring).** `D4` rounding of an entry `1.5 eps_r / a = 7.5e-7` N per unit of `s`; `max |KX|` for a unit `n = 2`-like
   response `>= omega_s^2 m_node ~ 7e4 * 0.25 = 1.8e4` N: `4e-11`. The direct solve's backward error `~eps max_i sum_j |A_ij|
   |X_j|`, `|A_ij| <= E T ~ 2e10`, `~30` entries per row: `6e-5` N, `3e-9` relative (a factor `10` of growth: `3e-8`). The
   Rayleigh factor `|1 + i omega beta| <= 1.00002`. Estimate **`<= 3e-8`**. At `T = 0.02` the same bound is `~2e-6`: the reason
   of D4's section.
4. **B0c, H0c (ring at zero load against the linear paths with the follower).** If `K_T(0)` of the Hex20 SVK ring equals
   `point_stiffness` (`stiffness_with`) in every bit, the results are bit-equal: `0` (ADR 0590 found that for Hex8; for Hex20 it is
   not known). Otherwise an entry rounding `eps` amplified by `sum |K_ij| |x_i| |x_j| / x^T K x ~ ||K|| / (bending scale) ~ 3e7`:
   `<= 3e-9` in a factor, and by `||A|| / sigma_min(A) ~ 2e7` in a response at `0.5 omega_1` or `1.5 omega_1` (`2e-9`). Limits
   `1e-6` (B0c) and `1e-7` (H0c).
5. **P1c.** `K_T(u*) + K_c(u*, 1) ~ K + K_sigma(S*) - K_p,sym(0, 1)` with `S* ~ S_ref`, so `1 + mu_state ~ mu_lin` of
   `follower::buckling_factors`; the remainder is the pre-buckling geometry change and the change of `K_p` with the state, `c
   eps_h` with `c <= 10`: `<= 1.3e-3`. Limit `1e-2`.
6. **E0, E1.** With `contributions` empty the new functions run the operations of ADR 0590 D1, D2 in the same order (D1, D2): `0`
   bit differences. E1 is the same comparison against the code before the change (a hash of the bits of the results of the ADR
   0590 calls on the registration commit and on the final commit).
7. **Negative controls.** NBc: the factors at `u*` without the contribution (`K_T(u*)` and `K_sigma` alone): arithmetic 1, `9 -
   2.25 * 1.5 = 5.6` against `2.25`: `mu ~ 2.5` against `~1.5`: `~0.4..0.7`; limit `>= 1e-2`. NB2c: BFD's measure without
   `D4_g` in the denominator, `3/2.25 - 1 = 0.33`; limit `>= 1e-2`. NHc: `about_state` without the contribution at `u*` against
   `about_state_with`, the response at `omega_a`: the `n = 2` stiffness `5.6` against `4.5` (`25` percent), near `0.5 omega_1` the
   response by `~0.3`; limit `>= 1e-2`. NH2c: `e_H` of that response against the residual with the follower: `~0.2`; limit `>=
   1e-2`.
8. **Bar F (records).** ADR 0590 Bar C's compression as a follower end load; the state is straight by symmetry; the unsymmetric
   Newton converges quadratically (ADR 0560). The Rayleigh measure RNq is the identity of D1's reading, its rounding as ADR 0590
   BFD (`<= 6e-8`); the vector measure RNv `||D4_r[x] + mu (K_sigma x + D4_g[x])||_F / ||D4_r[x]||_F` shows the antisymmetric part
   (the edge term `~p h` against the bending scale, of order `0.1`) on top of the eigenvector error (the backward residual `1e-10`
   times `||K|| / |Kx|`); RNa `max |K_p - K_p^T|_FF / max |K_p|_FF` at `u*`, order `1`; RNh `e_H` of the harmonic response of the
   symmetric approximation: order of the antisymmetric part's share, `~1e-2..1e-1`. Records only: the reading is stated, not a
   limit.

## Checks (limits fixed now)

Every measure is required finite and every call `Ok` before it is compared (otherwise a miss). Relative differences of lists are
position by position (ascending), worst over the list.

| Id | Clause | What | Limit | Estimate |
|---|---|---|---|---|
| S0r | set-up | nonlinear solve of the ring under the follower pressure converges | `Ok` | arithmetic 1 |
| BT0r | set-up | `tangent_buckling_factors_with` at `u*` with `[&fp]`, at `u = 0` (`lambda = 0`) with `[&fp]`, without contributions at `u*`, and `follower::buckling_factors`: `Ok`, 2 factors each, positive, residuals `<= 1e-10`, not exhausted | `Ok` | arithmetic 1 |
| BFD | a1, a3 | `max_i |mu_FD,i - mu_i| / |mu_i|` at `u*` (D3) | `<= 1e-6` | arithmetic 2: `<= 1e-9` |
| B0c | a1 | `tangent_buckling_factors_with` at zero load with `[&fp]` against `follower::buckling_factors` (same pre-stress, options) | `<= 1e-6` | arithmetic 4: `0` or `<= 3e-9` |
| P1c | a1 (physics) | `|(1 + mu_1,state) - mu_1,lin| / mu_1,lin`, `mu_lin` of `follower::buckling_factors` | `<= 1e-2` | arithmetic 5: `<= 1.3e-3` |
| NBc | negative | factors at `u*` without contributions against with, worst of 2 | `>= 1e-2` | `~0.4` |
| NB2c | negative | `max_i |mu_FD,i(no D4_g) - mu_i| / |mu_i|` | `>= 1e-2` | `~0.33` |
| BFVr | record | the vector measure of RNv on the ring (eigenvector error only) | record | |
| HT0r | set-up | `about_state_with` at `u*` (`[&fp]`) and at `u = 0` (`[&fp]`, `lambda = 0`), `about_state` at `u*`, `DirectHarmonic::new`: `Ok`; responses `Ok` | `Ok` | |
| HFD | a2, a4 | `e_H` of `about_state_with`'s response at `u*`, at `omega_a` (undamped) and `omega_b` (Rayleigh) | `<= 1e-6` | arithmetic 3: `<= 3e-8` |
| H0c | a2 | `about_state_with` at zero load with `[&fp]` against `DirectHarmonic::new` at the rest `omega_a`, `omega_b`: `max_k |X_k - X'_k| / max_k |X'_k|` | `<= 1e-7` | arithmetic 4: `0` or `<= 2e-9` |
| NHc | negative | `about_state` (no contribution) at `u*` against `about_state_with`, at `omega_a` | `>= 1e-2` | `~0.3` |
| NH2c | negative | `e_H` of that response, residual with the follower | `>= 1e-2` | `~0.2` |
| E0 | a5 | ADR 0590 Bar C NH: `tangent_buckling_factors_with(.., 0.0, &[], ..)` against `tangent_buckling_factors` (factors, modes, residuals); Bar S NH: `about_state_with(.., 0.0, &[], ..)` against `about_state` (responses at `omega_a`, `omega_b`): bit differences | `0` | arithmetic 6 |
| E1 | a5 | a hash of the bits of the same ADR 0590 results, on the registration commit (before any library change; the hash test copied alone into a `git archive`) and on the final commit | equal | arithmetic 6 |
| S0b | record set-up | nonlinear solve of Bar F (unsymmetric Newton) | record | arithmetic 8 |
| RNq | record | Bar F: BFD's Rayleigh measure | record | `<= 6e-8` |
| RNv | record | Bar F: the vector measure | record | `~0.1` |
| RNa | record | Bar F: reduced antisymmetry of `K_p` at `u*` | record | `~1` |
| RNh | record | Bar F: `e_H` of `about_state_with`'s response at `0.5 omega_1` | record | `~1e-2..1e-1` |
| B1 | defaults | `cargo test -p rcfd-api --release --test baseline_hashes`; `cargo test -p rcfd-fem --release` of `sa_hyper_state_paths_f669`, `sa_follower`, `sa_follower_default_bits`, `sa_harmonic_direct`, `sa_harmonic_direct_bm`, `sa_harmonic_direct_bm_default_bits`, `sa_harmonic_direct_tp`, `sa_harmonic_direct_tp_default_bits` | pass | |
| S1 | build | `cargo fmt -p rcfd-fem --check`; `cargo clippy -p rcfd-fem --all-targets -- -D warnings` | clean | |

Tests: `ring_buckling_about_the_pressurised_state` (S0r, BT0r, BFD, B0c, P1c, NBc, NB2c, BFVr), `ring_harmonic_about_the_
pressurised_state` (S0r, HT0r, HFD, H0c, NHc, NH2c), `no_contribution_equals_the_adr_0590_paths` (E0, and prints the E1 hash),
`free_edged_bar_records` (S0b, RNq, RNv, RNa, RNh; records only: a failed call is printed as the record and the test does
not fail). Each estimated well under a minute in release (888 equations on the ring;
`D4` four force evaluations per vector).

### Audit of existing tests

No existing test is edited. `tangent_buckling_factors` and `about_state` become one-line calls of the new functions with no
contribution: their operations are unchanged (E0, E1; the ADR 0590 tests in B1 cover them). `buckling.rs` and the code of
`finish` are not changed. No refusal is lifted.

## Planned mutations (on a scratch copy: a `git archive` of the test commit, its own `CARGO_TARGET_DIR`, one at a time; the new test file run for each)

Every check that can catch each mutation is listed (the lesson of ADR 0590 MU6), set-up checks included.

| Id | Mutation | Can be caught by | Expected not to move |
|---|---|---|---|
| MA | D1: the contributions' tangents at the state not added to `K_T` | BFD, P1c, BT0r (pivot or positivity), NBc (then `0`, a miss) | B0c (zero at load zero), HFD, E0 |
| MB | D1: `K_c'` not added to `K_sigma` | BFD, B0c, P1c, BT0r, NBc | HFD, E0 |
| MC | D1: `K_c'` evaluated at `u = 0` in place of `u*` | BFD (arithmetic: `eps_h` times the follower share `0.25`: `~3e-5`), P1c | B0c, HFD, E0 |
| MD | D2: the contributions' tangents not added | HFD (both), NHc (then `0`, a miss), HT0r | H0c, BFD, E0 |
| ME | D2: the Rayleigh stiffness `k_total` the material tangent alone (the part keeps the contribution) | HFD at `omega_b` | HFD at `omega_a`, H0c, E0 |
| MF | `follower.rs` `symmetric_negative`: `-0.5 *` to `-0.25 *` (the driver and both paths) | S0r (Newton with half the load stiffness), BFD, HFD, B0c, H0c, P1c, BT0r, HT0r | E0 |

A mutation is caught if any registered check (set-up checks included) misses. No mutation is declared equivalent.

## Decision rule

- **done**: every check of the table met (records excluded), MA to MF caught (each by at least one check of its "can be caught
  by" column), B1 and S1 met.
- **partial**: any of BFD, HFD, E0, E1 missed (recorded as a `*_known_miss` test, diagnosed), or B0c, H0c, P1c or a negative
  control missed, or a mutation not caught, or a check met only after a post-run amendment.
- **todo**: the set-ups do not run.

## Limits (what is not covered)

The follower pressure on Hex20 SVK (the ring) for the registered checks; contributions other than the follower pressure
(`CentrifugalFollower` is linear in the load factor and passes through the same code, not checked); hyperelastic materials only in
the records of Bar F; the symmetric part of the load stiffness only (D1 reading: flutter about a state not represented; the full
unsymmetric path about a state is a follow-up); the harmonic path's other refusals as ADR 0590; contributions nonlinear in the load
factor get the secant of `[0, 1]` (none exists); the plain path only; shells and beams not covered (the driver takes no follower
on them here).

## Amendment 1 (2026-11-15, after run 1 and the mutations, before the run of the check it adds)

Run 1 met every registered check, but mutation MC (`K_c'` evaluated at `u = 0`) was not caught: its results on the ring are equal
to the unmutated ones in every bit. Diagnosis: on the ring every free unknown is in-plane (`u_z` held everywhere), and the entries
of the symmetric part of the pressure's load stiffness between in-plane components `i, j` are `epsilon_ijz` times the `z`
component of `x_u` or `x_v`; on the outer face `x_v = (0, 0, BW)`-like and `x_u` in-plane, and an in-plane displacement changes
neither `z` component. So the ring's follower tangent on its free unknowns does not depend on the state, and no ring check can
see where it is evaluated (the same fact makes B0c and H0c exactly `0` and P1c small). The free-edged Bar F record RNq moved under
MC from `2.6e-12` to `8.3e-5`. Amendment: a new test `free_edged_bar_rayleigh_check` registers **RNq2**, Bar F's Rayleigh measure
(the set-up of D4, the measure of RNq), limit `<= 1e-6` (estimate of arithmetic 8, `<= 6e-8`). Disclosed: its value on the
unmutated code is already known from the record RNq (`2.6e-12`); the check is added for the coverage of MC, not as a blind test.
The new test is run once on its commit and MC is run again on a `git archive` of that commit. By the registered rule the item
stays **partial** whatever RNq2 gives (a mutation caught only after a post-run amendment). Run 1's tests and records stay as they
are. Also disclosed: arithmetic 1 used the ring's `E I` with the width `BW` for a load per unit length, while `P_R` is a pressure;
the critical pressure is `3 E T^3/(12 R^3) = 5e7` Pa, so the state is at `~0.05` of it, not `0.53` (the factors are `~19`, not
`~1`). The checks' limits do not depend on it; the negative controls NHc and NH2c come out a factor `~10` below their estimates
(still above their limits).
