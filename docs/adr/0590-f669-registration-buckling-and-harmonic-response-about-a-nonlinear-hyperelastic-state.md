# ADR 0590: F-669 registration: buckling and harmonic response about a nonlinear hyperelastic state

Status: **REGISTERED 2026-11-15, written before any code of F-669 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-669 was written and nothing of F-669 was compiled or run: the files
under "What was read" were read and the arithmetic below was done by hand (no script, no probe, no rcfd call). Measured numbers
quoted below come from the addendum of ADR 0586 and ADR 0581, not from a new run. Results go in the addenda at the end; nothing
above the addenda is changed after a run.

Roadmap item F-669 (dep F-646), structural solver, advanced tier, the follow-up proposed by
[ADR 0586](0586-f646-registration-linear-paths-about-a-pre-stressed-hyperelastic-state.md) (its remaining questions 1 and 2).
Number 0590 given by the lead. Branch `f669-work` from the local main tip 06b66e35 (which holds F-644, F-645, F-646), worktree
`w49i-work/f669`, scratch `w49i-work/f669-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number); each registered run made once; existing tests not edited;
defaults bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new
dependency, no unsafe code; clean-room. The roadmap files are not edited (the entries go in "For the lead"). Every choice below
that the maintainer would normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the
recommendation).

## What F-669 asks

The acceptance, binding:

> Every limit not stated here is registered in the ADR before the run. The buckling factors and the direct harmonic response about a nonlinear static state of a hyperelastic model take the driver's tangent at that state; at a load of zero they equal those of the linear paths within 1e-10, and the BBar bar at K/G = 1e4 about a stretched state gives frequencies that agree with central differences of the nonlinear static residual within 1e-6.

Clauses: **a0** every limit registered here; **a1** the buckling factors about a nonlinear static state of a hyperelastic model
take the driver's tangent at that state; **a2** the direct harmonic response about such a state takes the driver's tangent;
**a3** at a load of zero both equal those of the linear paths (`buckling::factors`, `DirectHarmonic::new`) within `1e-10`;
**a4** the BBar bar at `K/G = 1e4` about a stretched state gives frequencies that agree with central differences of the
nonlinear static residual within `1e-6`.

## What was read

`AGENT_RULES.md`; the brief; ADR 0586 in full (D1 to D3, arithmetic 1 to 6, the checks, the addendum, remaining questions 1 and 2,
the follow-up text that became F-669); ADR 0581 (the diagnosis of the modal residual floor `c eps lambda_max / lambda_1`,
arithmetic 1 and 2: the frequency error at the stopping test `<= tau^2 kappa(M) mu / g`, option (i) `ModalOptions::tolerance(1e-8)`);
ADR 0555's list of linear call sites and D3 (the hyperelastic material on the linear paths through `linearised_elasticity`);
ADR 0572 and 0573 (the "For the lead" form). Sources at 06b66e35: `crates/rcfd-fem/src/follower.rs` (`buckling_factors` 475-487,
`tangent_modes` 496-583, `buckling_factors_full` 761-), `crates/rcfd-fem/src/buckling.rs` (`BucklingOptions`, `factors`,
`factors_with` 138-244: the model checks, zero-value prescribed equations, `check_rigid_body_motions`, `assemble_geometric`,
`point_stiffness`, `reduce`, the zero-shift pivot rule of ADR 0402, `lowest_buckling_factors` with `BucklingEigenOptions`, the
coordinate order), `crates/rcfd-fem/src/harmonic_direct.rs` (module doc; `DirectHarmonicOptions`; `DirectHarmonic` fields;
`new` 589-915: checks, damping per material, the stiffness parts, `geometric`, the dashpots, the transformation branch, the
free rows and the nested-dissection order; `coefficients`, `value` 954-977 (the Rayleigh stiffness `k_total + geometric`),
`stress` 998-), `crates/rcfd-fem/src/nonlinear.rs` (module doc on the F-bar Hex8 of ADR 0506; `evaluate_fbar` 484-590: `E_bar`,
the material at `E_bar`, the consistent tangent with the mean part of `M_q`; `check_driver_model` 868-886: the transformation
path and temperature loads refused; `initial_states`, `internal_force`, `tangent`, `add_triplets`), `crates/rcfd-fem/src/assemble.rs`
(`stiffness_with`, `point_stiffness`), `crates/rcfd-fem/src/mass.rs` (`consistent_with`), `crates/rcfd-fem/src/geometric.rs`
(`Prestress::from_static`, `geometric_stiffness`), `crates/rcfd-fem/src/harmonic.rs` (`HarmonicLoad::real`, `HarmonicResponse`),
`crates/rcfd-linalg/src/eigen.rs` (`lowest_buckling_factors`: Rayleigh-Ritz on `(A_r, K_r)` with `theta = 1/lambda`),
`crates/rcfd-fem/tests/sa_hyper_prestress_f646.rs` (set-ups, `Checks`, `directional`, `full_difference`),
`crates/rcfd-fem/tests/sa_hyper_paths_f644.rs` (the BBar block set-up `set_block_property(0, BlockProperty::solid(BBar))`).

## Audit: do the paths exist?

- **Buckling.** `buckling::factors` and `follower::buckling_factors` solve `(K + mu K_sigma) x = 0` with `K = point_stiffness`, the
  stiffness at rest (for the hyperelastic material `linearised_elasticity`, ADR 0555 D1), and `K_sigma` of a given pre-stress.
  No entry point takes `nonlinear::tangent` at a state. `follower::buckling_factors_full` also takes `point_stiffness`.
- **Direct harmonic.** `DirectHarmonic::new` takes `stiffness_with` (at rest) and optionally `K_sigma` of a pre-stress
  (`DirectHarmonicOptions::prestress`, ADR 0543 D5). No entry point takes the tangent at a state.
- **Modes.** `follower::tangent_modes` exists and takes the tangent (ADR 0586 D1).

So F-669 needs library code for a1 and a2 (not test-only); a4 is a set of checks on `tangent_modes` (existing).

## Design

### D1. Buckling about a state (`follower.rs`, additive)

```text
pub fn tangent_buckling_factors(model, prescribed: &[Prescribed], u: &[f64], states: &[ElementState],
                                prestress: &Prestress, options: &BucklingOptions) -> Result<BucklingResult>
```

The eigenproblem `(K_T(u, states) + mu K_sigma(prestress)) x = 0`, `K_T` = `nonlinear::tangent` (the driver's tangent at the
state: material part of `Material::stress` at each point's Green-Lagrange strain, or `E_bar` for a BBar block, plus the geometric
part of the state's stress), `K_sigma` = the geometric stiffness of the given pre-stress (the increment of the reference load).

Derivation (first principles). Along the equilibrium path `r(u(lambda), lambda) = f_int(u) - lambda f = 0` a critical state is
where `K_T(u(lambda))` becomes singular. Linearising the tangent's dependence on the load about a state `lambda*` by the
geometric stiffness of the stress increment of the reference load (the "linearised pre-buckling" or one-point estimate; Brendel
and Ramm, 1980, cited from memory, used here only as a name: the formula is derived here), `K_T(lambda* + mu) ~ K_T(lambda*) +
mu K_sigma(S_ref)`, and the factors `mu` are the further load factors at which it becomes singular. At `u = 0` (`lambda* = 0`)
`K_T(0)` is the stiffness at rest and the problem is `buckling::factors`' (clause a3). The pre-stress is the caller's: the test
uses `Prestress::from_static` of the linear static solve of the reference load (the linear path's pre-stress). The material's
initial-displacement and stress-stiffening terms of the derivative `dK_T/dlambda` beyond `K_sigma(S_ref)` are not included (the
linear path leaves them out too; this is the linearised estimate about the state).

Checks and reduction as `buckling::factors_with` on the plain path, with the driver's model check in place of the linear
one: `model.validate()`, `model.check_supported_nonlinear()`, `nonlinear::check_driver_model` (the transformation path and
temperature loads refused, so there is no transformation branch); the prescribed equations of the model and of `prescribed`
with zero values (the motions about the state hold them; their values are not used, as `tangent_modes`); `check_rigid_body_motions`
(refused if a rigid motion is free, as buckling); `modal::check_coordinate_order` with `uses_transformation = false`; the factor
count test; the zero-shift pivot rule of ADR 0402 on the reduced `K_T`; `lowest_buckling_factors` with the options as
`factors_with`; the result `BucklingResult` (its fields as `factors_with` fills them). Not offered: contributions (follower
loads) about the state; nonzero increments of the prescribed displacements.

### D2. Direct harmonic about a state (`harmonic_direct.rs`, additive plus a code motion)

```text
impl DirectHarmonic {
    pub fn about_state(model, prescribed: &[Prescribed], u: &[f64], states: &[ElementState],
                       options: &DirectHarmonicOptions) -> Result<DirectHarmonic>
}
```

`A(omega) = K_T - omega^2 M + i omega (alpha M + beta K_T + C_d)`, `K_T = nonlinear::tangent(model, u, states, threads)`, `M` the
consistent mass, the dashpots and the base motion as `new`. The model check is the driver's (`validate`,
`check_supported_nonlinear`, `check_driver_model`); the prescribed equations of the model and of `prescribed` are held with zero
increments (their values are not used; a `BaseMotion` gives a harmonic increment there). Refused (key in brackets): a loss factor
(`"loss factor"`: a per-material split of the tangent is not built), a pre-stress (`"prestress"`: the state's stress is in
`K_T`), a temperature (`"temperature"`), a viscoelastic material (`"material"`), a tangent whose pattern differs from the mass's
(`"pattern"`). The solver holds one stiffness part `(K_T, no damping)`, `k_total = K_T`, no `geometric`; `stress` of such a
solver is refused (key `"stress"`: the recovered stress is that of the material at rest). Everything after the assembly (the
dashpots, the free rows, the nested-dissection order) is the code of `new`, moved unchanged into a private function
`finish_plain` that `new` calls for its plain path and `about_state` calls (a code motion: the same operations in the same order,
so `new`'s bits are unchanged; B1 below). A new private field `about_state: bool` (false from `new`) carries the `stress` refusal.

### D3. The central-difference measure (registered)

The fourth-order central difference of the residual along a vector `v` (`v` scaled to `max |v_k| = 1`), step `a`:
`D4[v] = (8 (r(u + a v) - r(u - a v)) - (r(u + 2a v) - r(u - 2a v))) / (12 a)` (the Richardson combination of two central
differences; for a polynomial `g` of degree 5 its error is `-(a^4/30) g^(5)`; elementary). `a = 1e-4` m (arithmetic 2).

- **Modes (FB1, a4)**: `lambda_FD,i = phihat . D4[phihat] / (phihat^T M phihat)`, `f_FD,i = sqrt(lambda_FD,i)/(2 pi)`.
- **Buckling (BFD, a1)**: for each mode `xhat`, `mu_FD,i = -xhat . D4[xhat] / (xhat^T K_sigma xhat)`.
- **Harmonic (HFD, a2)**: for the response `X`, `KX = s_re D4[X_re/s_re] + i s_im D4[X_im/s_im]` (`s = max |.|`), the residual
  `q = KX (1 + i omega beta) + (i omega alpha - omega^2) M X - F`, `e_H = max_k |q_k| / max_k |KX_k|` (complex moduli).

Why fourth order and not ADR 0586's two-point difference: at `K/G = 1e4` the truncation of the two-point difference along a
bending mode is dominated by the bulk term (arithmetic 2), and no step keeps both its truncation and its rounding below `1e-6`
with a derived margin. ADR 0586's two-point measure with `a = 5e-6` is run on the BBar bar as a record (FB2). Taken on the
maintainer's standing instruction of 2026-11-11 (take the recommendation).

### D4. Set-ups (registered)

- **Energies**: NHc `neo_hookean(1e6, 1e7)`, MRc `mooney_rivlin(3.5e5, 1.5e5, 1e7)` (ADR 0586), NHi `neo_hookean(1e6, 1e10)`
  (`K/G = 1e4`, ADR 0581); density `1000`.
- **Mesh**: `hex_box([0, 0, 0], [2, 0.5, 0.5], [8, 2, 2])`, material on `all`, the three components held on `xmin`.
- **Bar S** (stretched, Full): `Load::nodal_force("xmax", [4e3, 0, 0])` (ADR 0586 Bar S). NHc and MRc.
- **Bar C** (compressed, Full): `Load::nodal_force("xmax", [-250, 0, 0])` (total `-2.25e3` N). NHc and MRc.
- **Bar B** (stretched, BBar block): Bar S's load with NHi and `set_block_property(0, BlockProperty::solid(BBar))`.
- **States**: `nonlinear::solve(&bar, &[], &[], &NewtonOptions::new(10))`, its displacement `u*` and states; at zero load `u = 0`,
  `initial_states`.
- **Residual**: `r(u) = internal_force(bar, u, states) - f_ext` (the dead load at factor 1 at `u*`, 0 at rest; it cancels in
  every difference).
- **Buckling**: `prestress_C = Prestress::from_static(&bar_C, &solve_static(&bar_C, &StaticOptions::default()))`,
  `BucklingOptions::new(4)` (backward test `1e-10`).
- **Harmonic**: `HarmonicLoad::real(F)` with `F_z = 100` N on each node of `xmax`; `threads(1)`. Frequencies from `omega_1`, the
  lowest circular frequency of `tangent_modes(bar_S, &[], u, states, 1.0, &[], ModalOptions::new(6))` at the state (of
  `modal_analysis(bar_S, ModalOptions::new(6))` at rest): `omega_a = 0.5 omega_1` undamped, `omega_b = 1.5 omega_1` with
  `rayleigh(0.5, 1e-3)`.
- **BBar modes**: `tangent_modes(bar_B, &[], u*, states, 1.0, &[], ModalOptions::new(6).tolerance(1e-8))` (ADR 0581 option (i)).

### D5. Files

`crates/rcfd-fem/src/follower.rs` (D1, additive), `crates/rcfd-fem/src/harmonic_direct.rs` (D2: additive, the code motion of
`finish_plain`, the field, the `stress` refusal, the module doc), the new test `crates/rcfd-fem/tests/sa_hyper_state_paths_f669.rs`,
this ADR.

## Arithmetic (disclosed, by hand)

1. **States.** Bar S as ADR 0586 (converged, stretch `1.0506` NH, measured there). Bar C: nominal stress `2.25e3 / 0.25 = 9e3` Pa,
   strain `9e3 / 2.9e6 = 3.1e-3`. Euler-Bernoulli load of the clamped-free bar `pi^2 E I / (4 L^2) = 9.87 * 2.9e6 * 5.21e-3 / 16
   = 9.3e3` N; the Full Hex8 mesh is stiffer in bending (ADR 0586's rest frequency `1.2607` Hz against the beam's `1.087`: a
   factor `1.35` in stiffness), so `P_cr ~ 1.25e4` N and `mu_lin ~ 5.6`; the state is at `~0.18` of it, on the stable straight
   branch (the mesh and the loads are symmetric). Newton converges (the tangent is exact; ADR 0586 KM). Bar B: the same load,
   `K/G = 1e4`: the bulk term holds `J - 1 ~ sigma / K ~ 1e-5`; the relative residual floor of Newton is the rounding of the
   pressure `K eps ~ 1e-6` Pa times the area `~0.06`: `~1e-7` N against `|f_ext| ~ 1.2e4`: `~1e-11`, below `1e-10`. Estimate:
   converged.
2. **D4 along a mode, truncation and rounding.** `g(t) = v . r(u + t v) = Pi'(t)`, `Pi` the potential along `v`. Bulk part (BBar,
   `F_bar`): per element `V (kappa/2) (Jbar - 1)^2`, `Jbar` the element mean of `det F`, cubic in `t`: `Jbar - 1 = j0 + j1 t + j2
   t^2 + j3 t^3`. Along a low mode of a locking-free element `j1 ~ 0` but `j2 = (1/2)((tr H)^2 - tr(H^2)) ~ theta^2` (a rotation
   `theta` changes the volume at second order), with the slope `theta <= 0.7` per unit tip amplitude. Two-point truncation
   `(a^2/6) g'''`, `g''' ~ 12 kappa V <j2^2> ~ 12 * 1e10 * 0.5 * 0.05 = 3e8` against `Pi'' = lambda phihat^T M phihat >= 4.7e3`
   (ADR 0586 arithmetic 2): `~1e4 a^2` relative (uncertain by 10): at `a = 5e-6`, `2.5e-7` (up to `2.5e-6`): the reason D3 takes
   D4. D4: the bulk part of `g` is a polynomial of degree 5, error `(a^4/30) g^(5)`, `g^(5) = 360 kappa V <j3^2>`, `j3 = det H
   <= 0.1`: `<= 360 * 1e10 * 0.5 * 1e-2 = 1.8e10`, relative `1.8e10 / 4.7e3 / 30 a^4 = 1.3e5 a^4`: at `a = 1e-4` `1.3e-11`, with a
   factor 100 for the deviatoric and pre-strain terms `1.3e-9`. Rounding: ADR 0586 arithmetic 2 bounds the rounding of `phihat .
   r` by `eps_g = 1.2e-8` (worst case, G-scale forces). The bulk pressure carries `kappa eps ~ 1e-6` Pa but enters `phihat . r`
   multiplied by `dJbar` along the mode (`<= V (|j1| + 4 a |j2|) ~ 1e-5` per element at `a = 1e-4`): `< 1e-12`, negligible. D4's
   rounding `(16 + 2) eps_g / (12 a) / Pi'' = 1.5 * 1.2e-8 / (1e-4 * 4.7e3) = 3.8e-8`. FB1 estimate in the frequency (half the
   eigenvalue's): **`<= 2e-8`**. Full rows (BFD): no bulk term at that scale; truncation `<= 180 a^4`-type (ADR 0586's `180 a^2`
   squared in kind), negligible; rounding with `Pi'' = xhat^T K_T xhat ~ 3e3` (the first bending stiffness `k_tip ~ 5.7e3` N/m
   times the factor `1.35` softened by `1 - 0.18`): `1.5 * 1.2e-8 / (1e-4 * 3e3) = 6e-8`. The Ritz value equals the Rayleigh
   quotient of its Ritz vector (Galerkin condition of the Rayleigh-Ritz step) up to rounding, so the convergence tolerance does
   not enter. BFD estimate **`<= 6e-8`**.
3. **HFD.** Per entry the rounding of `r` is `~1.8e-10` N (ADR 0586 arithmetic 2), D4's `1.5 * 1.8e-10 / a = 2.7e-6` N per unit
   of `s`; `max |K_T v|` for a unit bending-like `v` `>= 100` N (the tip forces of a unit tip deflection `~600` N per node; the
   inertia `omega^2 M v ~ 180` N): `<= 2.7e-8`; truncation negligible; the direct solve's backward error `~eps kappa(A) ~ 1e-16 *
   5e4`: `5e-12`; the factor `|1 + i omega beta| <= 1.0002`. Estimate **`<= 3e-8`**.
4. **Zero load (B0, H0).** At `u = 0` the driver's tangent and the linear stiffness were equal in every bit for Bar S (ADR 0586
   R1, modal path; the frequencies equal in every bit). If the matrices are bit-equal and the same eigensolver and factor see them,
   the results are bit-equal: `0`. Otherwise an entry rounding `eps` of `K` amplified at most by `lambda_max / lambda_1 ~ 4.6e4`
   (ADR 0586 arithmetic 5) gives `<= 5e-12` in a response at `0.5 omega_1` or `1.5 omega_1` (the distance to the resonance
   multiplies by at most `1 / |1 - 0.25|` and, damped, by `1/(2 zeta) ~ 20`: `1e-10` worst case), and the eigenvalue worst case
   `~1e-9` of ADR 0586 arithmetic 5 for the buckling factors (`5e-12` typical). Estimate **`~0`** (worst cases disclosed; the
   limit is the acceptance's).
5. **P1, the critical load from the state.** `K_T(u*)` holds `K_sigma(S*)` with `S* ~ S_ref` (factor 1), so `mu_state ~ mu_lin
   - 1`; the remainder is the pre-buckling nonlinearity at strain `3.1e-3` (the length and section changes, the tangent of the
   energy in compression): `|(1 + mu_state) - mu_lin| / mu_lin ~ c 3.1e-3` with `c` up to 5: `<= 1.6e-2`. Limit `5e-2`.
6. **Negative controls.** NB1 `mu_state` against `mu_lin`: `~1/mu_lin ~ 0.18`, limit `>= 1e-2`. NB2 BFD's measure against the
   rest stiffness `K_T(0)` along the same `xhat`: `xhat^T K_T(u*) xhat ~ xhat^T K xhat (1 - 1/mu_lin)`: `~0.2`, limit `>= 1e-2`.
   NH1 `about_state` at `u*` against `new` with `Prestress::from_static` of Bar S's linear solve (the linear pre-stress path):
   ADR 0586 N2 `6.75e-2` in the frequency, `~0.13` in the stiffness of the first pair, `~0.17` in the response at `0.5 omega_1`;
   limit `>= 1e-3`. NH2 HFD's measure on the response of `new` (at rest) at the stretched state: the stiffness differs by ADR 0586
   N1 (`0.69` in the frequency): `>= 0.5`; limit `>= 1e-2`. NBb FB1's measure against the Rayleigh quotients of the rest tangent
   of Bar B (as ADR 0586 N3, `0.30` there): limit `>= 1e-2`.
7. **KMb.** As ADR 0586 arithmetic 3 with the bulk term: the rounding of an entry of `r` `~1e-7` N (the pressure rounding times
   the area) over `2h = 2e-6`: `5e-2` against `max |K_T| ~ kappa h_e ~ 2.5e9`: `2e-11`; truncation `10 (h / h_e)^2 / 6 = 3e-11`.
   Estimate `<= 5e-11`, limit `1e-8`.
8. **TB0.** At `tau = 1e-8` (Relative) the floor `~1.3e-9` of ADR 0581 is below the tolerance; the stretched state raises
   `lambda_1`. Estimate converged; frequency error at the stop `<= 5e-13` (ADR 0581 arithmetic 2).

## Checks (limits fixed now)

Every measure is required finite and every call `Ok` before it is compared (otherwise a miss). Relative differences of lists are
position by position (ascending), worst over the list.

| Id | Clause | What | Limit | Estimate |
|---|---|---|---|---|
| S0 | set-up | nonlinear solves of Bar S, Bar C (NH, MR) and Bar B (NHi) converge | `Ok` | arithmetic 1 |
| BT0 | set-up | `tangent_buckling_factors` at `u*` of Bar C and `buckling::factors` of Bar C: `Ok`, 4 factors each, positive, residuals `<= 1e-10`, `exhausted` `None` | `Ok` | |
| BFD | a1 | `max_i |mu_FD,i - mu_i| / |mu_i|` at `u*` of Bar C, D3 with `a = 1e-4` (NH, MR) | `<= 1e-6` | arithmetic 2: `<= 6e-8` |
| B0 | a3 | `tangent_buckling_factors` at `u = 0` (load zero) against `buckling::factors`, same pre-stress and options (NH, MR) | `<= 1e-10` | arithmetic 4: `~0` |
| P1 | a1 (physics) | `|(1 + mu_1,state) - mu_1,lin| / mu_1,lin` (NH, MR) | `<= 5e-2` | arithmetic 5: `<= 1.6e-2` |
| NB1 | negative | `mu_state` against `mu_lin`, worst of 4 (NH, MR) | `>= 1e-2` | `~0.18` |
| NB2 | negative | `max_i |mu_FD,i - mu_0,i| / |mu_FD,i|`, `mu_0,i = -xhat^T K_T(0) xhat / xhat^T K_sigma xhat` (NH, MR) | `>= 1e-2` | `~0.2` |
| HT0 | set-up | `DirectHarmonic::about_state` at `u*` and at `u = 0`, `new` with and without the pre-stress: `Ok`; responses at `omega_a`, `omega_b` `Ok` | `Ok` | |
| HFD | a2 | `e_H` of `about_state`'s response at `u*` of Bar S, at `omega_a` and `omega_b` (NH, MR) | `<= 1e-6` | arithmetic 3: `<= 3e-8` |
| H0 | a3 | `about_state` at `u = 0` against `new` (Bar S, no pre-stress) at the rest `omega_a`, `omega_b`: `max_k |X_k - X'_k| / max_k |X'_k|` (NH, MR) | `<= 1e-10` | arithmetic 4: `~0` |
| NH1 | negative | `about_state` at `u*` against `new` with the linear pre-stress, same measure, at `omega_a` (NH) | `>= 1e-3` | `~0.17` |
| NH2 | negative | `e_H` of `new`'s response (rest, no pre-stress) at `omega_a`, the residual at `u*` (NH) | `>= 1e-2` | `>= 0.5` |
| HR | refusals | `about_state` with a loss factor, a pre-stress, a temperature: refused with the keys of D2; `stress` of an `about_state` solver refused (key `"stress"`) | refused | D2 |
| TB0 | set-up | BBar modes (D4): `Ok`, six frequencies, every residual `<= 1e-8` | `Ok` | arithmetic 8 |
| KMb | a4 (matrix) | `max |K_FD - K_T| / max |K_T|` of Bar B at `u*`, `h = 1e-6`, all 243 equations | `<= 1e-8` | arithmetic 7: `<= 5e-11` |
| FB1 | a4 | `max_i |f_FD,i - f_i| / f_i`, D3 with `a = 1e-4`, Bar B at `u*` | `<= 1e-6` | arithmetic 2: `<= 2e-8` |
| CB0 | a4 (control) | FB1's measure at rest (`u = 0`, `tangent_modes` at load zero, tolerance `1e-8`) | `<= 1e-6` | arithmetic 2 |
| NBb | negative | FB1's `f_FD` against the Rayleigh quotients of `K_T(0)` of Bar B along the same `phihat` | `>= 1e-2` | `~0.3` |
| FB2 | record | ADR 0586's two-point measure (`a = 5e-6`) on Bar B at `u*` | record | arithmetic 2: `~2.5e-7` (to `2.5e-6`) |
| RB | record | Bar B: `tangent_modes` at zero load against `modal_analysis` (both tolerance `1e-8`) | record | the F-bar at `u = 0` is the B-bar stiffness (ADR 0506) in a different operation order |
| B1 | defaults | `cargo test -p rcfd-api --release --test baseline_hashes`; `cargo test -p rcfd-fem --release` of `sa_harmonic_direct`, `sa_harmonic_direct_bm`, `sa_harmonic_direct_bm_default_bits`, `sa_harmonic_direct_tp`, `sa_harmonic_direct_tp_default_bits`, `sa_hyper_paths_f644`, `sa_follower`, `sa_follower_default_bits`, `sa_hyper_prestress_f646` | pass | D2 code motion |
| S1 | build | `cargo fmt -p rcfd-fem --check`; `cargo clippy -p rcfd-fem --all-targets -- -D warnings` | clean | |

Tests: `buckling_about_the_compressed_bar` (S0 Bar C, BT0, BFD, B0, P1, NB1, NB2), `harmonic_about_the_stretched_bar` (S0 Bar
S, HT0, HFD, H0, NH1, NH2), `refusals_of_about_state` (HR), `bbar_modes_about_the_stretched_bar` (S0 Bar B, TB0, KMb, FB1, CB0,
NBb, FB2, RB). Each estimated well under a minute in release (243 equations; KMb `486` force evaluations; D4 `4` per vector).

### Audit of existing tests

No existing test is edited. `buckling.rs` is not changed. `follower.rs` gets an additive function. `harmonic_direct.rs`: the code
motion of D2 leaves `new`'s operations in the same order; the existing harmonic tests listed in B1 and `baseline_hashes` cover
it. No refusal is lifted and no existing result can change.

## Planned mutations (on a scratch copy: a `git archive` of the test commit, its own `CARGO_TARGET_DIR`, one at a time; the new test file run for each)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | `tangent_buckling_factors`: the tangent at `(0, initial_states)` instead of `(u, states)` | BFD, P1; **not** B0 (and NB1 then misses, recorded) |
| MU2 | `tangent_buckling_factors`: `K_sigma` scaled by `2.0` | B0, BFD |
| MU3 | `about_state`: `stiffness_with(model)` in place of the tangent (parts and `k_total`) | HFD; not H0 |
| MU4 | `about_state`: `k_total` (the Rayleigh stiffness) the stiffness at rest, the part the tangent | HFD at `omega_b`; not H0 |
| MU5 | `mat_hyper.rs` `let v_ii = -2.0 * kappa * k.j * k.jm1;` to `0.0 * ...` (a tangent term that vanishes at rest; ADR 0586 MU1) | BFD, HFD, FB1, KMb; not B0, H0 |
| MU6 | `nonlinear.rs` `mean_coefficient += w * a2 * s_c / 3.0;` to `0.0 * ...` (the F-bar mean part, zero at rest) | FB1, KMb; not the Full checks, not CB0 |

A mutation is caught if any registered check misses or a set-up call fails; the "not" entries are expectations recorded for the
record. No mutation is declared equivalent.

## Decision rule

- **done**: every check of the table met (records excluded), MU1 to MU6 caught (each by at least one of its expected checks), B1
  and S1 met.
- **partial**: any of BFD, B0, HFD, H0, FB1, KMb missed (recorded as a `*_known_miss` test, diagnosed), or P1 or a negative
  control missed, or a mutation not caught, or a check met only after a post-run amendment.
- **todo**: the set-ups do not run.

## Limits (what is not covered)

Hex8 only (Full and BBar), one bar, one stretch, one compression; dead loads; no follower contributions about the state (D1, D2
do not take contributions); no nonzero increments of prescribed displacements; the harmonic path about a state takes no loss
factor, pre-stress, temperature or viscoelastic material and recovers no stress; the plain path only (the driver refuses the
transformation path); the buckling factors about a state are the linearised estimate (D1), not the nonlinear critical load (no
arc-length comparison); Beam2 and shell models about a hyperelastic state not checked (F-645's materials).

## Amendment 1 (2026-11-15, before any code was compiled or any check run)

`BucklingResult` has a private field (`dofs`), so D1 in `follower.rs` cannot build it. `crates/rcfd-fem/src/buckling.rs` gets one
additive `pub(crate)` constructor, `BucklingResult::from_parts`, with the fields `factors_with` fills; no existing line of
`buckling.rs` changes and `factors_with` does not call it. Nothing else of the registration changes.

## Addendum 1: results (2026-11-15)

Commits: 07d87ab6 (registration), 3ca044f0 (amendment 1), 0a6c52ff (library and tests, as in run 1), 82c41f4c (the text of three
refusal messages of `about_state` put on one logical line each: a backslash continuation had been written with the indentation
inside the string; text only, no check reads it, the keys are unchanged), and this addendum. Runs, each once, release,
`--nocapture --test-threads=1`, logs in `w49i-work/f669-scratch`: run 1, the whole new file on 0a6c52ff (`run1.log`); B1
(`b1_fem.log`, `b1_hashes.log`, on 82c41f4c); the mutations (`mutations.log`, `mutate.py`, on a `git archive` of 82c41f4c with
its own target, the new test file for each). Shared solver files changed: `follower.rs` (additive function and module doc),
`buckling.rs` (additive constructor), `harmonic_direct.rs` (additive constructor, field, `stress` refusal, module doc; the code of
`new` after its assembly wrapped in place as the private `finish`, its lines otherwise unchanged).

| Id | Measured | Limit | Met |
|---|---|---|---|
| S0 | Bar C NH 10 steps (stretch `0.99700`), MR 10 steps; Bar S NH `1.05061`, MR `1.05132`; Bar B NHi converged in 40 steps (halvings), stretch `1.05186` | `Ok` | yes |
| BT0 | all `Ok`, 4 positive factors, 18 iterations, worst residual `4.2e-11`, not exhausted; NH state `4.5360, 4.5360, 35.322, 35.322`, linear `5.4485, 5.4485, 35.809, 35.809` | `Ok` | yes |
| BFD | NH `3.1e-13`, MR `8.2e-14` | `<= 1e-6` | yes |
| B0 | `0` (NH, MR: equal in every bit) | `<= 1e-10` | yes |
| P1 | NH `1.61e-2`, MR `1.72e-2` | `<= 5e-2` | yes |
| NB1 | NH `0.167`, MR `0.166` | `>= 1e-2` | yes |
| NB2 | NH `0.202`, MR `0.200` | `>= 1e-2` | yes |
| HT0 | all six solvers and responses `Ok` (NH, MR); `omega_1` state `13.361` (NH), rest `7.921` rad/s | `Ok` | yes |
| HFD | NH `7.4e-11` (`omega_a`), `2.7e-11` (`omega_b`); MR `7.4e-11`, `1.9e-11` | `<= 1e-6` | yes |
| H0 | `0` at both frequencies (NH, MR: equal in every bit) | `<= 1e-10` | yes |
| NH1 | `0.153` | `>= 1e-3` | yes |
| NH2 | `0.949` | `>= 1e-2` | yes |
| HR | loss factor, pre-stress, temperature refused with their keys; `stress` refused (key `"stress"`) | refused | yes |
| TB0 | `Ok`, six frequencies `2.0416 .. 7.0525` Hz at the state (`1.1192 .. 7.0531` at rest), worst residual `7.5e-10` (state), `6.3e-9` (rest) | `<= 1e-8` | yes |
| KMb | `2.55e-12` | `<= 1e-8` | yes |
| FB1 | `5.3e-11` | `<= 1e-6` | yes |
| CB0 | `5.8e-11` | `<= 1e-6` | yes |
| NBb | `3.34` | `>= 1e-2` | yes |
| FB2 (record) | `1.29e-4` (two-point, `a = 5e-6`) | record | - |
| RB (record) | `1.03e-10` | record | - |
| B1 | `baseline_hashes` 3 passed (1 ignored); the nine `rcfd-fem` test files of B1 all passed | pass | yes |
| S1 | `cargo fmt -p rcfd-fem --check` clean; `cargo clippy -p rcfd-fem --all-targets -- -D warnings` clean | clean | yes |

**Mutations** (`mutations.log`; one at a time):

| Id | Caught by | Expected |
|---|---|---|
| MU1 (rest tangent in `tangent_buckling_factors`) | BFD `0.167`, P1 `0.184` (NH, MR); NB1 `0` (as recorded) | BFD, P1; B0 met, as expected |
| MU2 (`K_sigma` doubled) | B0 `0.5`, BFD `1.0`, P1 `0.40` | B0, BFD |
| MU3 (`stiffness_with` in `about_state`) | HFD `0.949`, `0.617` (NH), `0.950`, `0.608` (MR) | HFD; H0 met, as expected |
| MU4 (Rayleigh stiffness at rest) | HFD at `omega_b` `2.5e-2` (NH), `2.4e-2` (MR); `omega_a` met (undamped) | HFD at `omega_b`; H0 met |
| MU5 (`v_ii` zero) | KMb `3.9e-5`, FB1 `2.5e-2`, BFD `3.3e-3`, HFD `0.15 .. 0.17` | all four; B0, H0 met |
| MU6 (F-bar mean part zero) | **S0 of Bar B** (Newton fails at load step 3 after 5 halvings); the Full tests met | FB1, KMb (not reached) |

**Deviations and diagnoses.** No limit was missed and no post-run amendment was written. (1) B0 and H0 are exactly `0`: at `u = 0`
the driver's tangent equals the linear stiffness in every bit on Bar C and Bar S (as ADR 0586 R1), so the factors and the responses
are bit-equal. (2) BFD, HFD, FB1 and CB0 lie 2 to 3 orders below their estimates: the rounding bounds of arithmetic 2 and 3 are
worst-case. (3) FB2 (record, `1.3e-4`) confirms the reason of D3: ADR 0586's two-point measure at `K/G = 1e4` is dominated by the
bulk truncation (arithmetic 2 estimated `2.5e-7 .. 2.5e-6`; the measured value is 50 times the upper estimate, the mean `<j2^2>`
being larger than estimated); with it the acceptance's `1e-6` would have been missed, with D4 it is met by five orders. (4) Bar B
needed step halvings (40 steps) but converged. (5) MU6 was caught only by the set-up check S0 (Newton fails without the mean part
of the F-bar tangent, as ADR 0586's MU4 without the geometric part), not by one of its registered expected checks FB1 or KMb,
which the failed solve does not reach. Not run: the whole workspace suite (the lead's gate).

**Decision: partial** by the registered rule, on one item only: every check met in the one registered run, MU1 to MU5 caught by
expected checks, B1 and S1 met; MU6 was caught, but by S0 and not by one of its expected checks, and the rule asks for an expected
check.

**Remaining questions** (none changes a default):

1. MU6 and the rule. Options: (a) accept the item as done, reading S0 (a registered set-up check of the same test) as catching it,
   as ADR 0586 listed "or S0" for its analogous MU4; (b) keep partial and add a follow-up mutation of the F-bar tangent that
   vanishes at rest but keeps Newton converging (for instance the term `(2/9) s_c g_i g_j` alone). Recommend (a): the mutation
   was caught by a registered check of the same test, and the omission of "or S0" in the expectations was a drafting slip.
2. Contributions (follower loads) about the state on the new buckling and harmonic paths. Options: (a) a follow-up passing
   `&[&dyn Contribution]` as `tangent_modes` does, checked against central differences including the follower force; (b)
   nothing. Recommend (a), proposed below.
3. Loss factors about a state (a per-material split of the tangent). Options: (a) a follow-up assembling the tangent per material
   set; (b) leave them refused. Recommend (b) until a use asks for it.

## For the lead: roadmap entries

> Proposed status: partial.

> Structural solver, advanced tier. Follow-up of F-646 (ADR 0586). PARTIAL. ADR 0590, 2026-11-15: follower::tangent_buckling_factors, (K_T + mu K_sigma) x = 0 with the driver's tangent at a state and the geometric stiffness of a given reference pre-stress, and DirectHarmonic::about_state, K_T - omega^2 M + i omega (alpha M + beta K_T + C_d) (no loss factor, pre-stress, temperature or viscoelastic material; stress refused), both opt-in, the code of DirectHarmonic::new after its assembly shared unchanged (baseline hashes and the harmonic and follower tests pass). Compressed hyperelastic bar (Full Hex8 8 x 2 x 2, 2.25e3 N, neo-Hookean and Mooney-Rivlin): the factors about the state against the fourth-order central difference of the nonlinear static residual along each mode 3.1e-13 (limit 1e-6); at zero load equal to buckling::factors in every bit (limit 1e-10); 1 + mu_state against the linear factor 1.6e-2 (limit 5e-2). Stretched bar (3.6e4 N): the harmonic response about the state satisfies the central-difference residual equation within 7.4e-11 (limit 1e-6, undamped and with Rayleigh damping); at zero load equal to DirectHarmonic::new in every bit. BBar bar at K/G = 1e4, stretched: tangent_modes frequencies against the fourth-order central difference 5.3e-11 (limit 1e-6; ADR 0586's two-point difference gives 1.3e-4, a record), the tangent against column differences 2.6e-12. Negative controls 0.15 to 3.3. Six of six mutations caught, five by their expected checks; the dropped mean part of the F-bar tangent was caught only by the failed Newton solve of the set-up, hence partial by the registered rule (recommended to be read as done). Not covered: follower contributions and loss factors about a state, Hex20, Tet, shells and beams.

> Follow-up proposed: contributions about a nonlinear state on the buckling and direct harmonic paths. Deps F-669. Acceptance: Every limit not stated here is registered in the ADR before the run. tangent_buckling_factors and DirectHarmonic::about_state take the tangents of the driver's contributions (a follower pressure) at the state; their factors and responses agree with central differences of the nonlinear residual including the follower force within 1e-6, and with no contribution given they equal the paths of ADR 0590 in every bit.
