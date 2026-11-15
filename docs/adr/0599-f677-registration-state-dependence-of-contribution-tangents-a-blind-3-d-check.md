# ADR 0599: F-677 registration: state dependence of contribution tangents about a nonlinear state, a blind 3-D check

Status: **REGISTERED 2026-11-15, written before any code of F-677 in the repository and before any run of its checks.** Not yet
accepted by the maintainer. Before this text no library or test code of F-677 was written in the worktree. As the brief asks
("choose and verify the set-up ... both before the main run"; "check [the load-level arithmetic] numerically before
registering"), one set-up probe was written and run in the scratch folder only (`w49i-work/f677-scratch/probe`, a `git archive`
of the base commit d3d178e9 with one added test file `probe_f677.rs`; logs `probe1.log`, `probe_final.log`). It measured set-up
quantities only (the linear follower buckling factor at rest for the load level, Newton convergence, the antisymmetry and the
state dependence of the follower load stiffness, the latter also along the rest mode); it did not call
`tangent_buckling_factors_with`, `DirectHarmonic::about_state_with`, any central difference of the residual or any negative
control of the table. Its numbers are disclosed in "Set-up probe" and the set-up checks SCt, SDt, SDq whose values it already
measured are marked "known". Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-677 (dep F-671), structural solver, advanced tier, the follow-up proposed by
[ADR 0596](0596-f671-registration-contributions-about-a-nonlinear-state-on-the-buckling-and-direct-harmonic-paths.md) (its
remaining question 1, option (b)). Number 0599 given by the lead. Branch `f677-work` from the local main tip d3d178e9 (which
holds F-671's `tangent_buckling_factors_with` and `DirectHarmonic::about_state_with`), worktree `w49i-work/f677`, scratch
`w49i-work/f677-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number); each registered run made once; existing tests not edited;
defaults bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new
dependency, no unsafe code; clean-room. The roadmap files are not edited (the entries go in "For the lead"). Every choice below
that the maintainer would normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the
recommendation).

## What F-677 asks

The acceptance, binding:

> Every limit not stated here is registered in the ADR before the run. On a conservative 3-D follower set-up whose follower tangent between free unknowns changes with the state (a pressurised box or shell segment with symmetry planes and out-of-plane motion), tangent_buckling_factors_with and DirectHarmonic::about_state_with agree with central differences of the nonlinear residual including the follower force within 1e-6 (vector measure for the response), and evaluating the contribution tangents at zero displacement in place of the state is caught by a registered check.

Clauses: **a0** every limit registered here; **a1** the set-up is 3-D and conservative (verified: the antisymmetric part of the
follower load stiffness vanishes on the free unknowns); **a2** its follower tangent between free unknowns changes with the state
(verified, lower bound); **a3** `tangent_buckling_factors_with` agrees with the central differences of the nonlinear residual
including the follower force within `1e-6`; **a4** `DirectHarmonic::about_state_with` agrees likewise within `1e-6`, by the
vector measure; **a5** evaluating the contribution tangents at zero displacement in place of the state is caught by a registered
check.

The brief adds: blind (the set-up chosen and verified for a1 and a2 before the main run, with registered limits), test-only if
possible, existing bits unchanged.

## What was read

`AGENT_RULES.md`; the brief; ADR 0596 in full (D1 to D4, the checks, amendment 1: MC bit-equal on the plane ring because every
free unknown is in-plane and the in-plane entries of the symmetric follower tangent depend only on the `z` components of `x_u`,
`x_v`; the pressure-per-length slip of its arithmetic 1; the addendum; its remaining question 1); ADR 0590 D3 (the `D4` measures);
ADR 0480 D1, arithmetic 3 (the antisymmetric part of a pressure's load stiffness is an edge term `p oint (delta u x Delta u) .
dx`); ADR 0560 D4; ADR 0572/0573 (the "For the lead" form). Sources at d3d178e9: `crates/rcfd-fem/src/follower.rs`
(`FollowerPressure::face_force`, `face_stiffness`, `force`, `load_stiffness`, `symmetric_negative`, the `Contribution` impl;
`contribution_triplets`, `tangent_buckling_factors_with` 638-), `crates/rcfd-fem/src/harmonic_direct.rs` (`about_state_with`
811-), `crates/rcfd-fem/tests/sa_state_contributions_f671.rs` (all of it: the helpers reused here).

## Design (test-only)

No library file is changed: both entry points exist (ADR 0596 D1, D2). F-677 is a new test file
`crates/rcfd-fem/tests/sa_state_contributions_f677.rs` and this ADR. The defaults are bit-identical by construction (check L0:
no file under `crates/*/src` differs from d3d178e9).

### D1. Why the plane ring could not see the state, and what a 3-D set-up needs

The follower force of a face is `f_a = -p int N_a (x_u x x_v) du dv` (`face_force`), its load stiffness `K_p` (`face_stiffness`)
has entries `-p int N_a (N_b,u (e_j x x_v) + N_b,v (x_u x e_j))_i`. For a pair of components `(i, j)` the entry is a component of
`x_v` or `x_u` along the third axis `k` (`epsilon_ijk`). On ADR 0596's ring every free unknown was in-plane (`u_z` held
everywhere): only `(i, j) = (x, y)` entries were used, which take the `z` components of `x_u`, `x_v`, and an in-plane state does
not change those. A set-up sees the state if some free pair `(i, j)` takes a component of `x_u` or `x_v` that the state changes:
here free `u_z` (pairs `(x, z)`, `(y, z)` take the in-plane components of `x_u` (circumferential tangent), which the hoop
contraction changes by `-eps_h`) and meridional rotation near a clamped end (`x_v` (axial tangent) gains a radial component).

### D2. The set-up: a quarter tube under external follower pressure (registered)

A shell segment of a 3-D solid with symmetry planes and out-of-plane (axial, meridional) motion:

- Mesh: `hex_box([R - T/2, 0, 0], [T, pi/2, L], [1, 12, 8])` mapped by `(r, theta, z) -> (r cos theta, r sin theta, z)`,
  `upgrade_to_hex20_mapped` (Hex20), `R = 1`, `T = 0.15`, `L = 4` m (the half-length of a tube of length `8 R`; `z = 0` its
  mid-plane). 2325 equations, 1993 free.
- Material: `Material::isotropic_elastic(2e11, 0.3, 7850, 0)` (SVK in the driver).
- Held: `ymin` (`theta = 0`, the plane `y = 0`) component 1; `ymax` (`theta = pi/2`, the plane `x = 0`) component 0; `zmin` (the
  mid-plane `z = 0`) component 2; `zmax` (`z = L`) all three (clamped end).
- Load: `FollowerPressure::new(&tube, "xmax", P_T)` on the outer face `r = R + T/2` (external), `P_T = 1.75e8` Pa, the only
  contribution, `NewtonOptions::new(8)` (the symmetric tangent is exact for a conservative set-up). The state `u*` at
  `lambda* = 1`. **Pre-stress** `Prestress::from_static` of the linear static solve of a copy with the dead `Load::pressure("xmax",
  P_T)`.
- **Why conservative (a1).** The edges of the loaded face are: `theta = 0` (on the plane `y = 0`, `u_y` held: the free
  components and the edge tangent `dx` (axial) lie in that plane), `theta = pi/2` (likewise in `x = 0`), `z = 0` (on the plane
  `z = 0`, `u_z` held, `dx` circumferential in it) and `z = L` (all held). On each, `delta u`, `Delta u` and `dx` lie in one
  plane, so `det[delta u, Delta u, dx] = 0`: the edge term of ADR 0480 vanishes on the free unknowns at any state that keeps the
  held components zero. Verified numerically (SCt).
- **Buckling**: `BucklingOptions::new(2)`. With the quarter's symmetry only even circumferential wave numbers exist.
- **Harmonic**: `HarmonicLoad::real(F)`, `F_x = 1e3` N on each free `x` equation of the nodes of `ymin` (radial at `theta = 0`);
  `threads(1)`; `omega_1` the lowest circular frequency of `tangent_modes(tube, &[], u*, states, 1.0, &[&fp], ModalOptions::new(2))`
  at the state and of `modal_analysis(tube, ModalOptions::new(2))` at rest; `omega_a = 0.5 omega_1` undamped, `omega_b = 1.5
  omega_1` with `rayleigh(1.0, 1e-5)` (ADR 0596 D4).
- **Zero-displacement tangent (a5, in the test).** A test contribution `AtZero(&fp)` whose `add(u, lambda, force, tangent)` is
  `fp.add(0, lambda, force, tangent)`: through `tangent_buckling_factors_with` and `about_state_with` it gives exactly the
  operations of the library with every contribution tangent evaluated at `u = 0` in place of the state (the mutation MC3 and MH
  below, as a permanent registered check, NMb and NMh). Its force is not read by either path.
- **Non-conservative control (NSc).** The same tube with `zmin` not held (the loaded face's `z = 0` edge free; `zmax` clamped
  keeps it free of rigid motion).

The steel is far beyond yield at this pressure (hoop stress `~1.25e9` Pa): the material is the driver's elastic SVK and the
check is numerical; the load level is set by the geometry (D3), and any modulus gives the same strains at the same fraction of
the critical pressure.

### D3. The measures (as ADR 0596 D3, registered)

`D4[v]` the fourth-order central difference along `v` (`v` scaled to `max |v_k| = 1`, `a = 1e-4` m) of the residual
`r(w) = internal_force(tube, w, states) - fp.force(w, 1)` and of its load-factor derivative `g(w) = -fp.force(w, 1)`.

- **BFD** (a3, Rayleigh): for each mode `xhat`, `mu_FD,i = -xhat . D4_r[xhat] / (xhat^T K_sigma xhat + xhat . D4_g[xhat])`;
  `max_i |mu_FD,i - mu_i| / |mu_i|`.
- **BFV** (a3, vector): `max_i max_k,free |D4_r[xhat] + mu_i (K_sigma xhat + D4_g[xhat])|_k / max_k,free |D4_r[xhat]|_k`. On a
  conservative set-up the symmetric pencil is the full linearised problem, so this vector measure is limited only by the
  eigenvector error (ADR 0596 BFVr `8.5e-7` on the ring; Bar F, free-edged, `0.92`).
- **HFD** (a4, vector): `e_H` of ADR 0590 D3 / ADR 0596 D3, `max_k,free |(K X)_FD + i omega beta (K X)_FD - omega^2 M X + i
  omega alpha M X - F|_k / max_k,free |(K X)_FD|_k` with `(K X)_FD` from `D4_r` (complex moduli).
- **SCt**: `max_ij,free |K_p,ij - K_p,ji| / max_ij,free |K_p,ij|` of `fp.load_stiffness(w, 1)` at `w = u*` and `w = 0`.
- **SDt**: `max_ij,free |S_ij(u*) - S_ij(0)| / max_ij,free |S_ij(u*)|`, `S = (K_p + K_p^T)/2` at load factor 1.
- **SDq**: `|x0^T (S(u*) - S(0)) x0| / |x0^T (K_sigma - S(0)) x0|`, `x0` the lowest mode of `follower::buckling_factors` at rest
  under `P_T` (the first-order relative change of a buckling factor when the load-factor derivative is evaluated at `0`).

Both forces are polynomials of degree `<= 3` along a line (SVK internal force cubic, follower force quadratic), for which `D4` is
exact: only rounding remains.

## Set-up probe (disclosed; run before this registration, set-up quantities only)

The probe (`probe_f677.rs`, scratch) built the tube for a few geometries, solved the linear follower buckling problem at rest at
`p0 = 1e7` Pa, set the load at a fraction of `p_cr,lin = mu_1 p0`, solved the nonlinear state and measured SCt, SDt, SDq. All
with `nu = 0.3`, 12 x 8 elements, `NewtonOptions::new(8)` (every solve converged):

| `T` | `L` | load / `p_cr,lin` | `p_cr,lin` (Pa) | SCt at `u*`, at 0 | SDt | SDq | follower share `-x0^T S x0 / x0^T (K_sigma - S) x0` |
|---|---|---|---|---|---|---|---|
| 0.1 | 2 | 0.5 | `2.92e8` | `3.6e-15`, `5.0e-15` | `1.4e-2` | `4.9e-5` | `-0.037` |
| 0.1 | 4 | 0.5 | `9.55e7` | `2.8e-15`, `5.0e-15` | `3.8e-3` | `6.4e-5` | `0.148` |
| 0.1 | 6 | 0.5 | `6.54e7` | `3.2e-15`, `3.7e-15` | `2.0e-3` | `2.5e-5` | `0.160` |
| 0.15 | 4 | 0.5 | `2.53e8` | `3.9e-15`, `5.2e-15` | `6.2e-3` | `1.2e-4` | `0.106` |
| 0.15 | 4 | 0.7 | `2.53e8` | `2.2e-15`, `5.0e-15` | `8.7e-3` | `1.7e-4` | `0.106` |
| 0.2 | 4 | 0.7 | `5.35e8` | `3.6e-15`, `5.0e-15` | `1.3e-2` | `2.9e-4` | `0.065` |
| 0.2 | 4 | 0.5 | `5.35e8` | `2.5e-15`, `5.1e-15` | `9.3e-3` | `2.1e-4` | `0.065` |

Choice: `T = 0.15`, `L = 4`, the load `0.69 p_cr,lin`: SDq grows with `T/R` and with the load (the strain at a fraction of
`p_cr` scales like `(T/R)^2`), the follower share falls with `T/R`; `T/R = 0.15` keeps a share of `0.1` and a factor-of-2 margin
below the critical load. **Final probe at the registered set-up** (`P_T = 1.75e8`, `probe_final.log`): linear factors at `p0`
`25.304, 85.178` (`p_cr,lin = 2.530e8`, so `P_T = 0.692 p_cr,lin`); Newton converged in 8 steps; max `|u|` per component
`6.02e-3, 6.02e-3, 1.05e-3` m (axial motion present); SCt `1.9e-15` at `u*`, `4.9e-15` at `0`; SDt `8.63e-3`; SDq `1.69e-4`;
follower share `0.106`.

## Arithmetic (disclosed; by hand, after the probe)

1. **Load level.** A long tube (plane strain) under follower external pressure: `p_cr = 3 D / R^3` with `D = E T^3 / (12 (1 -
   nu^2))` (`n = 2`): `3 * 2e11 * 3.375e-3 / (12 * 0.91) = 1.85e8` Pa per unit mid-surface; on the outer face `R_o / R = 1.075`:
   `~1.72e8`. The clamped length `8 R` (an axial half-wave), the coarse circumferential mesh and the Hex20 through-thickness
   stiffening raise it; the probe's `2.53e8` is `1.47` times the long-tube value, plausible. `P_T = 1.75e8` is `0.69` of it
   (checked numerically: `P_T / p_cr,lin = 0.692`). Hoop strain `eps_h = P_T R_o / (E T) = 1.75e8 * 1.075 / 3e10 = 6.3e-3`,
   radial displacement `~R eps_h = 6.3e-3` m against the probe's `6.02e-3` (the clamp and Poisson reduce it). Hoop stress
   `P_T R_o / T = 1.25e9` Pa (D2: elastic, numerical). This arithmetic is a pressure, not a load per length (the slip of ADR 0596
   arithmetic 1 is not repeated: `P_T` is in Pa on the face, `D` per unit length of the mid-surface, `p_cr` in Pa).
2. **Factors.** `1 + mu_state ~ mu_lin = 1 / 0.692 = 1.445`: `mu_1,state ~ 0.45`; the second factor `~85.2 / 17.5 - 1 = 3.9`
   (if the order is kept). Two positive factors expected.
3. **BFD rounding.** As ADR 0596 arithmetic 2 (measured there `2.0e-11` against an estimate `1e-9`); the stresses are `~50` times
   the ring's but so is the bending scale `xhat^T K xhat` (`(T/R)^2` and `E`-scaling cancel in the ratio). Estimate **`<= 1e-9`**.
4. **BFV.** The eigenvector error at the residual test `1e-10` times `||K|| / |K x|` (membrane over bending, `~(R/T)^2 (R/h)`
   `~ 44 * 8 = 350`) and the solver's own growth: `~1e-7..1e-5` (ring `8.5e-7`). Limit `1e-4`.
5. **HFD.** As ADR 0596 arithmetic 3 (measured `1.7e-11`, `8.1e-12`): estimate **`<= 3e-8`**.
6. **B0c, H0c.** At `u = 0`, `lambda = 0` the code paths are those of `follower::buckling_factors` and `DirectHarmonic::new` up to
   `K_T(0)` against the linear stiffness; ADR 0596 found them bit-equal on Hex20 SVK: `0` (otherwise `<= 3e-9`). Limits `1e-6`,
   `1e-7`.
7. **P1c.** `|(1 + mu_1,state) - mu_1,lin| / mu_1,lin`: the pre-buckling geometry change and the change of the stress stiffness
   along the path, `c eps_h` with `c ~ 1..5`: `6e-3..3e-2`. Limit `1e-1`.
8. **The zero-displacement tangents (a5).** Let `A = K_T(u*) - S(u*)` (the state tangent with the contribution, load factor 1)
   and `B = K_sigma - S(u*)` (the increment; `K_c' = -S`), `A x = -mu B x`, `Delta S = S(u*) - S(0)`. Evaluated at `u = 0`: `A_0
   = A + Delta S`, `B_0 = B + Delta S`. First order: `delta mu / mu = -(1 + mu) x^T Delta S x / (mu x^T B x)`. With SDq `1.7e-4`
   along the rest mode as the value of `|x^T Delta S x / x^T B x|`: MC (`B_0` only) `~1.7e-4`; MC2 (`A_0` only, `/ mu`)
   `~3.8e-4`; MC3 / NMb (both, `(1 + mu)/mu`) `~5.5e-4` (signs may make MC2 and MC partly cancel in MC3; their ratio `1 : mu`
   keeps the sum `>= 1.7e-4` in magnitude unless the mode differs: estimate `1e-4..1e-3`). BFD measures the difference of the
   mutated factor and the Rayleigh quotient of the true residual along the mutated mode, which to first order is that change.
   Limit of NMb `>= 1e-5` (a factor `10` above BFD's acceptance limit, a factor `10..50` below the estimate).
   Harmonic (MH / NMh): the residual of the mutated response is `Delta S X` (the true tangent less the mutated one, times the
   response), relative to `max |K X|`; along the response's dominant shape `|x^T Delta S x / x^T A x| = SDq / mu ~ 3.8e-4`; the
   max-entry vector measure can be smaller where `Delta S` lives (near the clamp and along the edges) and `X` is small: estimate
   `1e-5..4e-4`. Limit of NMh `>= 2e-6` (a factor 2 above HFD's limit: the requirement is only that the registered measure would
   catch it).
9. **Negative controls.** NBc: without contributions `A = K_T`, `B = K_sigma`; the follower share `0.1` in `B` and `S/A ~
   0.1/0.45` in `A`: `~0.2..0.3`; limit `>= 1e-2`. NB2c (BFD without `D4_g`): the share `~0.1`; limit `>= 1e-2`. NHc
   (`about_state` without the contribution against `about_state_with`, `omega_a`): the stiffness along the response by `~0.2`,
   at `0.5 omega_1` the response by `~0.25`; limit `>= 1e-2`. NH2c (`e_H` of that response, residual with the follower):
   `~0.2`; limit `>= 1e-2`. NSc (antisymmetry with the `z = 0` edge free): the edge term against the face terms, order `1` (ADR
   0596 RNa `2.0`); limit `>= 1e-1`.

## Checks (limits fixed now)

Every measure is required finite and every call `Ok` before it is compared (otherwise a miss). Relative differences of lists
are position by position (ascending), worst over the list. "Known" marks a value measured by the set-up probe before this text.

| Id | Clause | What | Limit | Estimate |
|---|---|---|---|---|
| S0t | set-up | nonlinear solve of the tube under `P_T` converges (`NewtonOptions::new(8)`) | `Ok` | known: 8 steps |
| SCt | a1 | conservativeness: SCt at `u*` and at `0`, worst | `<= 1e-12` | known: `1.9e-15`, `4.9e-15` |
| SDt | a2 | state dependence, matrix | `>= 1e-3` | known: `8.6e-3` |
| SDq | a2 | state dependence along the rest mode | `>= 1e-5` | known: `1.7e-4` |
| NSc | negative (a1) | SCt at `0` with `zmin` not held | `>= 1e-1` | arithmetic 9: `~1` |
| BT0t | set-up | `tangent_buckling_factors_with` at `u*` with `[&fp]`, at `u = 0` (`lambda = 0`) with `[&fp]`, without contributions at `u*`, with `[&AtZero(&fp)]` at `u*`, and `follower::buckling_factors`: `Ok`, 2 factors each, positive, residuals `<= 1e-10`, not exhausted | `Ok` | arithmetic 2 |
| BFD | a3 | Rayleigh measure at `u*` | `<= 1e-6` | arithmetic 3: `<= 1e-9` |
| BFV | a3 | vector measure at `u*` | `<= 1e-4` | arithmetic 4: `1e-7..1e-5` |
| B0c | a3 (zero load) | factors at zero load with `[&fp]` against `follower::buckling_factors` | `<= 1e-6` | arithmetic 6: `0` |
| P1c | physics | `|(1 + mu_1,state) - mu_1,lin| / mu_1,lin` | `<= 1e-1` | arithmetic 7: `6e-3..3e-2` |
| NBc | negative | factors without contributions against with, worst of 2 | `>= 1e-2` | `~0.2` |
| NB2c | negative | BFD without `D4_g` | `>= 1e-2` | `~0.1` |
| NMb | a5 | BFD of the factors with `AtZero(&fp)` (the true residual's Rayleigh quotient along their modes) | `>= 1e-5` | arithmetic 8: `1e-4..1e-3` |
| HT0t | set-up | `about_state_with` at `u*` (`[&fp]`, both frequencies; `[&AtZero(&fp)]`, both), at `u = 0` (`[&fp]`, `lambda = 0`, both), `about_state` at `u*` (`omega_a`), `DirectHarmonic::new` (both), the modes: `Ok` | `Ok` | |
| HFD | a4 | `e_H` of `about_state_with` at `u*`, `omega_a` (undamped) and `omega_b` (Rayleigh) | `<= 1e-6` | arithmetic 5: `<= 3e-8` |
| H0c | a4 (zero load) | `about_state_with` at zero load against `DirectHarmonic::new`, both frequencies | `<= 1e-7` | arithmetic 6: `0` |
| NHc | negative | `about_state` (no contribution) against `about_state_with`, `omega_a` | `>= 1e-2` | `~0.25` |
| NH2c | negative | `e_H` of that response | `>= 1e-2` | `~0.2` |
| NMh | a5 | `e_H` of `about_state_with` with `AtZero(&fp)`, at `omega_a` (undamped) and at `omega_b` (Rayleigh), each | `>= 2e-6` each | arithmetic 8: `1e-5..4e-4` |
| L0 | defaults | `git diff --stat d3d178e9 -- crates/*/src` empty (no library change) | empty | |
| S1 | build | `cargo fmt -p rcfd-fem --check`; `cargo clippy -p rcfd-fem --all-targets -- -D warnings` | clean | |

Tests (release, `--nocapture --test-threads=1`): `tube_setup_is_conservative_and_state_dependent` (S0t, SCt, SDt, SDq, NSc),
`tube_buckling_about_the_pressurised_state` (S0t, BT0t, BFD, BFV, B0c, P1c, NBc, NB2c, NMb),
`tube_harmonic_about_the_pressurised_state` (S0t, HT0t, HFD, H0c, NHc, NH2c, NMh). Each estimated under a minute (2325
equations, dense helper matrices of `2325^2`, `D4` four force evaluations per vector).

Because no library file changes, the baseline hashes and the ADR 0590/0596 tests are not affected (L0); they are not re-run as
part of this item (the lead's gate runs them).

## Planned mutations (on a scratch copy: a `git archive` of the test commit, its own `CARGO_TARGET_DIR`, one at a time; the new test file run for each)

Every check that can catch each mutation is listed (ADR 0590 MU6), set-up checks included.

| Id | Mutation (library) | Can be caught by | Expected not to move |
|---|---|---|---|
| MA | `tangent_buckling_factors_with`: the contributions' tangents at the state not added to `K_T` | BFD, BFV, P1c, BT0t, NBc, NMb | SCt, SDt, SDq, HFD, B0c |
| MB | `K_c'` not added to `K_sigma` | BFD, BFV, B0c, P1c, BT0t, NBc, NB2c, NMb | SCt, HFD |
| MC | `K_c'` evaluated at `u = 0` (`contribution_triplets(.., zero, 1.0, ..)` and `(.., zero, 0.0, ..)`) | BFD (arithmetic 8: `~1.7e-4`), BFV, NMb | B0c, HFD, SCt |
| MC2 | the tangent at the state `K_c(u, lambda*)` evaluated at `u = 0` | BFD (`~3.8e-4`), BFV, NMb | B0c, HFD |
| MC3 | both (every contribution tangent at `u = 0`, buckling; the acceptance's mutation) | BFD (`~5.5e-4`), BFV; NMb then measures the same bits | B0c, HFD |
| MD | `about_state_with`: the contributions' tangents not added | HFD (both), NHc (then `0`), NMh, HT0t | H0c, BFD |
| ME | `about_state_with`: the Rayleigh stiffness without the contribution | HFD at `omega_b`, NMh at `omega_b` | HFD at `omega_a`, BFD |
| MH | `about_state_with`: the contributions' tangents at `u = 0` (the acceptance's mutation, harmonic) | HFD (both; arithmetic 8: `1e-5..4e-4`), NMh | H0c, BFD |
| MF | `symmetric_negative`: `-0.5 *` to `-0.25 *` | S0t, BFD, BFV, HFD, B0c, H0c, P1c, BT0t, HT0t | SCt (`load_stiffness` is not changed) |

A mutation is caught if any registered check (set-up checks included) misses. No mutation is declared equivalent.

## Decision rule

- **done**: every check of the table met, MA to MF caught (each by at least one check of its "can be caught by" column), L0 and
  S1 met, and a5 met both in the test (NMb, NMh) and by the mutations MC3 and MH.
- **partial**: any of BFD, BFV, HFD, NMb, NMh, SCt, SDt, SDq missed (recorded as a `*_known_miss` test, diagnosed), or another
  check missed, or a mutation not caught, or a check met only after a post-run amendment.
- **todo**: the set-up does not run.

## Limits (what is not covered)

The follower pressure on Hex20 SVK (one tube, one load level, one mesh) only; hyperelastic materials, shells and beams not
covered; `CentrifugalFollower` not checked; the symmetric part of the load stiffness only (a conservative set-up, where it is the
full problem: on free-edged loads the reading of ADR 0596 D1 and its records stand); the full unsymmetric path about a state (the
flutter follow-up of ADR 0596) not covered; the set-up quantities SCt, SDt, SDq were measured before this registration (disclosed,
"known"), the tangent paths' checks were not.

## Addendum 1: results (2026-11-15)

Commits: 135c6b36 (registration), 5636a8c3 (the test file, run 1), and this addendum. Each registered run was made once, in
release, with `--nocapture --test-threads=1`. Logs are in `w49i-work/f677-scratch`:

- the set-up probe made before the registration (`probe1.log`, `probe_final.log`; disclosed above);
- run 1, the whole new file on 5636a8c3 (`run1.log`);
- the mutations (`mutations.log`, `tools/mutate.py`), one at a time on a `git archive` of 5636a8c3 with its own target, the new
  test file run for each;
- S1 (`clippy.log`).

No library file was changed (L0): `git diff --stat d3d178e9 -- crates/*/src` is empty. The only new code file is
`crates/rcfd-fem/tests/sa_state_contributions_f677.rs`.

| Id | Measured | Limit | Met |
|---|---|---|---|
| S0t | converged in 8 steps, max `|u|` per component `6.02e-3, 6.02e-3, 1.05e-3` m | `Ok` | yes |
| SCt (known) | `1.9e-15` at `u*`, `4.9e-15` at `0` | `<= 1e-12` | yes |
| SDt (known) | `8.63e-3` | `>= 1e-3` | yes |
| SDq (known) | `1.69e-4` | `>= 1e-5` | yes |
| NSc | `0.561` (`z = 0` edge free) | `>= 1e-1` | yes |
| BT0t | all five calls `Ok`, 2 positive factors each, 12 to 15 iterations, worst residual `8.4e-11`, none exhausted; factors with the follower at the state `0.45081, 3.88454`; at zero load `1.44594, 4.86734`; without contributions `0.61730, 3.54434`; with `AtZero` `0.45106, 3.88457`; `follower::buckling_factors` `1.44594, 4.86734` | `Ok` | yes |
| BFD | `8.4e-13` | `<= 1e-6` | yes |
| BFV | `4.5e-7` | `<= 1e-4` | yes |
| B0c | `0` (equal in every bit) | `<= 1e-6` | yes |
| P1c | `3.4e-3` | `<= 1e-1` | yes |
| NBc | `0.369` | `>= 1e-2` | yes |
| NB2c | `0.118` | `>= 1e-2` | yes |
| NMb | `5.43e-4` | `>= 1e-5` | yes |
| HT0t | all nine responses and both modal solves `Ok`; frequencies at the state `64.02, 231.62` Hz, at rest `114.59, 252.41` Hz | `Ok` | yes |
| HFD | `3.5e-10` (`omega_a`), `1.7e-10` (`omega_b`) | `<= 1e-6` | yes |
| H0c | `0` at both frequencies (equal in every bit) | `<= 1e-7` | yes |
| NHc | `0.231` | `>= 1e-2` | yes |
| NH2c | `0.651` | `>= 1e-2` | yes |
| NMh | `1.32e-3` (`omega_a`), `6.60e-4` (`omega_b`) | `>= 2e-6` each | yes |
| L0 | empty | empty | yes |
| S1 | `cargo fmt -p rcfd-fem --check` clean; `cargo clippy -p rcfd-fem --all-targets -- -D warnings` clean (on 5636a8c3) | clean | yes |

**Mutations** (`mutations.log`):

| Id | Caught by (measured) | Listed as possible |
|---|---|---|
| MA (the tangent at the state not added) | BFD `0.18`, BFV `0.72` | BFD, BFV, P1c, BT0t, NBc, NMb |
| MB (`K_c'` not added) | BFD `0.10`, BFV `0.40`, B0c `0.12`, NB2c `5.9e-13` | BFD, BFV, B0c, P1c, BT0t, NBc, NB2c, NMb |
| MC (`K_c'` at `u = 0`) | BFD `1.69e-4` (estimate `1.7e-4`), BFV `6.0e-4` | BFD, BFV, NMb |
| MC2 (`K_c(u, lambda*)` at `u = 0`) | BFD `3.74e-4` (estimate `3.8e-4`), BFV `1.3e-3` | BFD, BFV, NMb |
| MC3 (every contribution tangent at `u = 0`, buckling) | BFD `5.43e-4` (estimate `5.5e-4`; equal to run 1's NMb in every printed digit, as D2 predicts), BFV `1.9e-3` | BFD, BFV |
| MD (the contributions not added, harmonic) | HFD `0.65`, `0.49`; NHc `0` | HFD, NHc, NMh, HT0t |
| ME (the Rayleigh stiffness without the contribution) | HFD at `omega_b` `2.6e-3` (`omega_a` still met) | HFD at `omega_b`, NMh at `omega_b` |
| MH (the contribution tangents at `u = 0`, harmonic) | HFD `1.32e-3`, `6.60e-4` (equal to run 1's NMh) | HFD, NMh |
| MF (`symmetric_negative` halved) | BFD `0.15`, BFV `0.57`, HFD `0.39`, `0.20`; S0t still converged; B0c and H0c `0` (both sides take the same halved tangent) | S0t, BFD, BFV, HFD, B0c, H0c, P1c, BT0t, HT0t |

All nine were caught, each by a check in its listed column. No check listed as "expected not to move" moved. SCt and SDt kept
their run-1 values under every mutation. SDq moved only under MF, from `1.691e-4` to `1.706e-4`, because the rest mode of
`follower::buckling_factors` takes the halved tangent.

**Deviations.** None from the registration. BFD and HFD came out 2 to 3 orders below their estimates, because the rounding
bounds are worst cases (as in ADR 0596). The first-order estimates of arithmetic 8 for MC, MC2 and MC3 were within 2 percent.
NMh came out at the upper end of its estimated range.

**What this adds to ADR 0596.** On the plane ring the follower tangent between free unknowns did not depend on the state, so MC
gave bit-equal results there. On the quarter tube (free `u_z`, a clamped end) the tangent does depend on the state: SDt
`8.6e-3`, SDq `1.7e-4`. The set-up is conservative: SCt is `4.9e-15`, against `0.56` with one loaded edge freed. The vector
measure of the buckling modes (`4.5e-7`) confirms that the symmetric pencil is the full linearised problem there. Evaluating the
tangents at zero displacement is caught blind, both by a permanent test (NMb, NMh) and by the source mutations.

Not run: the whole workspace suite (the lead's gate). The baseline hashes and the ADR 0590/0596 tests were not re-run because no
library file changed.

**Decision: done** by the registered rule. Every check was met. Nine of nine mutations were caught by listed checks. L0 and S1
were met, and a5 was met both in the test (NMb, NMh) and by MC3 and MH. No amendment was made.

**Remaining questions** (none changes a default):

1. The status of F-671. ADR 0596 stayed partial only because MC was caught by a check added after the run. Options: (a) leave
   F-671 partial and let F-677 carry the blind coverage; (b) move F-671 to done with a note pointing to ADR 0599. Recommend (b):
   this item closes, blind, the gap that F-671 disclosed.
2. Hyperelastic and larger-strain coverage. On steel the state dependence along the modes is `~1.7e-4`; it scales like the
   strain at a fixed fraction of `p_cr`, `~(T/R)^2`. Options: (a) a follow-up on a neo-Hookean set-up with large rotations; (b)
   nothing. Recommend (b) until a use needs it: the code path does not depend on the material, and the catch margin here is
   more than two orders of magnitude.

## For the lead: roadmap entries

> Proposed status: done.

> Structural solver, advanced tier. Follow-up of F-671 (ADR 0596). DONE. ADR 0599, 2026-11-15, test only (no library file changed): a blind 3-D check that tangent_buckling_factors_with and DirectHarmonic::about_state_with take the follower tangent at the state. Set-up: quarter tube (Hex20 SVK, R = 1, T = 0.15, half-length 4 m), symmetry planes x = 0, y = 0, z = 0, clamped end, external follower pressure 1.75e8 Pa (0.69 of the linear follower critical pressure). Before the run the set-up was checked for conservativeness (the antisymmetric part of the load stiffness on the free unknowns is 4.9e-15, against 0.56 with one loaded edge freed) and for state dependence (the symmetric follower tangent at the state differs from the one at u = 0 by 8.6e-3 entry-wise and 1.7e-4 along the rest mode). The factors agree with the fourth-order central difference of the nonlinear residual with the follower force to 8.4e-13 (Rayleigh, limit 1e-6) and 4.5e-7 (vector, limit 1e-4); the harmonic responses agree to 3.5e-10 undamped and 1.7e-10 with Rayleigh damping (vector, limit 1e-6); at zero load both equal follower::buckling_factors and DirectHarmonic::new in every bit. Evaluating every contribution tangent at zero displacement in place of the state is caught by registered checks, both by a permanent test contribution (5.4e-4 buckling, 1.3e-3 and 6.6e-4 harmonic, against limits >= 1e-5 and >= 2e-6) and by source mutations; nine of nine mutations caught. Not covered: hyperelastic materials, shells and beams, the full unsymmetric tangent about a state.

> F-671 note, proposed addition (ADR 0599 remaining question 1): the state dependence of the follower tangent, which the plane ring could not see, is checked blind by F-677 (ADR 0599, 2026-11-15); recommended status done.
