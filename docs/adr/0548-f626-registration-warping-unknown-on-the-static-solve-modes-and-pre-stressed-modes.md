# ADR 0548: F-626 registration: the warping unknown on the static solve, the modes and the pre-stressed modes (and torsion in the buckling pre-stress)

Status: **REGISTERED 2026-11-15, written before any code of F-626 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-626 was written, compiled or run in the repository: the files listed under "What was
read" were read, and the arithmetic below was done by hand and evaluated with two plain numpy scripts in the scratch folder
(`w49i-work/f626-scratch/arith.py`, log `arith.log`: the closed forms, the torque split of an indeterminate beam, a cubic-Hermite
Euler-Vlasov model of the lateral-torsional problem with torque, the torsional frequencies; `arith2.py`, log `arith2.log`: the
pre-stressed modes of the same model; no rcfd code; disclosed, their numbers quoted below). The bit record B0 of the default
paths and of the ADR 0546 path is a test file written and run on the base after this commit and before any library code (D8).
Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-626 (deps F-598), structural solver, advanced tier, the follow-up named by
[ADR 0546](0546-f598-registration-warping-degree-of-freedom-for-beam2-and-i-section-lateral-torsional-buckling.md) (its
question Q2: the warping unknown on the static solve, the modes and the pre-stressed modes, lifting the refusal RF8 of torsion in
the pre-stress). Number 0548 given by the lead. Branch `f626-work` from e906faa8, worktree `w49i-work/f626`, scratch
`w49i-work/f626-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test edited;
no new dependency, no unsafe code, the crate graph unchanged; clean-room. Every choice below that the maintainer would normally
be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation), except the open
questions at the end.

## What F-626 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. The static twist of a cantilever I beam under end torque
> with the root warping held equals (T/(G J))(L - tanh(kL)/k) within 1e-3 at 16 elements, and the lateral-torsional buckling
> factor of a beam whose reference load includes torsion uses the warping static solve; every result without the option
> bit-identical.

The brief adds: cover the modes and the pre-stressed modes (the torsional frequencies of a warping beam against a closed form
derived or cited), registered before the run.

Clauses: **a0** every limit registered here; **a1** the static twist of the cantilever I beam with the root warping held within
`1e-3` of the closed form at 16 elements (on the model's static solve with the warping unknown, not on a hand assembly as ADR 0546
E6); **a2** the buckling factor of a beam whose reference load includes torsion uses the warping static solve (the pre-stress
comes from it; RF8 of ADR 0546 is lifted for that pre-stress); **a3** every result without the option bit-identical; **b1** the
modes with the warping unknown (torsional frequencies against a closed form); **b2** the pre-stressed modes with the warping
unknown (the same with an axial pre-stress, and with a torque pre-stress from the warping static solve).

### Readings, registered

- **"Uses the warping static solve"**: the pre-stress of the buckling (and of the pre-stressed modes) is made from the static
  solution of the 14-unknown model ([`WarpingPrestress::from_static`], D2): the end forces of each warping element are its
  14-unknown local forces, so its torque is the total torque `G J phi' - E C_w phi'''` of the warping model. In a statically
  determinate beam that torque equals the Saint-Venant one by equilibrium; the check therefore uses a statically indeterminate
  beam (twist held at both ends, warping held at one end, torque at an interior point), where the torque split between the two
  sides depends on `E C_w` (arithmetic 2: 874.2/-125.8 of 1000 against the Saint-Venant 750/-250), and checks that the factor
  is that of the closed-form split and not that of the Saint-Venant split (T2, T3).
- **"Opt-in"**: new functions of `rcfd_fem::beam_warping` (D7). The ADR 0546 functions `buckling_factors` and `matrices` keep
  their behaviour and their refusal RF8 (B0, T6). No case-file key (ADR 0546 Q1, F-625).
- **"Cantilever I beam"**: the I section of ADR 0546 (`doubly_symmetric_i(0.1, 0.01, 0.18, 0.006, 5/6)`, steel `E = 2e11`, `nu
  = 0.3`, `rho = 7850`), `L = 3` along `x`, 16 Beam2 elements, the root node with all six components held and its warping held
  (`Warping::restrain`), a nodal moment `T = 1000` about `x` at the tip; "static twist" the rotation component about the beam
  axis at the tip.
- **"Bit-identical"**: the record B0 (D8) on the base and after; no existing function changes behaviour (D7: one `pub(crate)`
  method added to `geometric.rs`, the ADR 0546 code refactored so that its two public functions run the same arithmetic).

## What was read (no run)

- `AGENT_RULES.md`; the brief; ADR 0546 (whole); ADR 0508 D1 (the second-order energy of the moment and torque terms) and D3
  (the end forces); ADR 0473 D1 (Wagner's `N I_p/A`).
- `crates/rcfd-fem/src`: `beam_warping.rs` (whole), `beam.rs` (`local_stiffness`, `local_mass`, `local_end_forces`,
  `end_resultants`, `to_global`), `geometric.rs` (`Prestress`, `prestressed_modes`), `geometric_struct.rs` (`beam_end_forces`),
  `solve.rs` (`solve_static_report`, `sparse_direct`), `constraint.rs` (`reduce`, `reactions`), `mass.rs` (`consistent_with`),
  `modal.rs` (`ModalOptions`, `ModalResult`), `model.rs` (`dof_map`, `Load`).
- Tests: `sa_beam_warping_f598.rs` (the set-ups), `sa_beam_warping_f598_bits.rs` (the digest pattern).

## Design

### D1. The static solve with the warping unknown

`solve_static(model, warping, threads) -> WarpingStaticResult`. The stiffness `K_aug = embed(K) + sum_e T^T (K14_l - embed(K12_l))
T` of ADR 0546 D5, `K` the model's `stiffness_with_materials` (the matrix of `solve::solve_static`); the load `f_aug` the model's
`nodal_load_vector` on the six-unknown equations and zero on the warping equations (no bimoment load; Limits); the prescribed
equations of the model (any value, as the base static solve) and the restrained warping equations (`chi = 0`); the free rigid
motions refused as the base (`check_rigid_body_motions`); `reduce`; the sparse `L D L^T` after minimum degree
(`SparseLdlt::factor`) with the pivot rule of ADR 0275 (`d_i <= 1e-10 a_ii` refused as `FemError::SingularStiffness`, the
component `6` naming the warping unknown of a node); the reactions `K_aug u - f_aug` at the prescribed and restrained equations
(`constraint::reactions`). The result has the displacement on all `n + n_w` equations (the model's `DofMap` order, then the
warping unknowns of `WarpingDofs`), the reactions, the numbering and the relative residual; `six_unknown()` is the first `n`
values (a displacement of the model's `DofMap`). Refusals: RF1 to RF4 and RF6 of ADR 0546 D4 (the same keys).

### D2. The pre-stress of the warping static solve

`WarpingPrestress` (new type) wraps a `Prestress` and says where it comes from:

- `WarpingPrestress::from_static(model, warping, result, wagner)`: the stress points of `Prestress::from_static(model,
  SolidResult::new(model.dof_map(), result.six_unknown()))` (the axial force of a beam reads only `u`, which the warping unknown
  does not change), and the end forces of every Beam2 element: outside the warping blocks `beam::local_end_forces` of the six
  unknowns (the forces of `Prestress::with_beam_resultants`); in a warping block the 14 local forces `f14 = K14_l T u_e` of the
  element's 14 unknowns (D2 of ADR 0546), kept as the twelve `f12[i] = f14[index_of(i)]` (the bimoments `f14[6]`, `f14[13]` are
  not kept: the bimoment geometric term has the coefficient `beta_omega = int omega (y^2 + z^2) dA/C_w`, zero for the doubly
  symmetric section; Limits). Set on the `Prestress` by a new `pub(crate)` method `Prestress::with_beam_end_forces(forces,
  wagner)` in `geometric.rs` (no existing function changed). The result's numbering must be that of `warping` on `model`
  (`WarpingDofs` equal, key `"warping"`). It records the blocks and sections of `warping`.
- `WarpingPrestress::torsion_free(prestress)`: any `Prestress` (ADR 0546's input); RF8 applies where it is used.
- `WarpingPrestress::with_torsion(prestress)`: a `Prestress` whose end forces the caller states are those of the warping model
  (a closed form; the reference of T2); RF8 is not applied. Documented as unchecked.
- `scaled(factor)`, `prestress()`.

### D3. Buckling with the pre-stress of D2

`buckling_factors_with(model, prestress: &WarpingPrestress, warping, options)`: the steps of ADR 0546 `buckling_factors`
(RF1 to RF9) on the matrices of ADR 0546 D5, except that RF8 (a torque on a warping element) is applied only to a
`torsion_free` pre-stress. **RF10**: a `from_static` pre-stress whose recorded blocks and sections differ from `warping`'s
(key `"warping"`). The ADR 0546 `buckling_factors(model, prestress, warping, options)` is unchanged (it equals
`buckling_factors_with` of `torsion_free(prestress)` operation for operation; B0 records its bits).

### D4. The mass of the warping element (first principles)

The 12-unknown consistent mass of ADR 0417 has the torsion pair `rho I_p L/6 [2, 1; 1, 2]` (`I_p = I_y + I_z`, the polar moment
about the centroid, which is the shear centre here) on the linear twist. The 14-unknown local mass is that matrix embedded by the
index map with the torsion pair replaced by the kinetic energy `(1/2) int rho I_p phi_dot^2 dx` on the Hermite twist: `rho I_p
L/420 [156, 22L, 54, -13L; 22L, 4L^2, 13L, -3L^2; 54, 13L, 156, -22L; -13L, -3L^2, -22L, 4L^2]` on `(phi_0, chi_0, phi_1,
chi_1)` (the integrals `int H_i H_j dx`, arithmetic 3). The axial and bending entries are bit for bit those of
`beam::local_mass`. **No warping inertia** `(1/2) int rho C_w phi_dot'^2 dx` (Limits; the closed form C3 neglects it too, as the
classical theory of Timoshenko and Gere, from memory). `local_mass(length, section, material)` (14 x 14), `element_mass`
(global, `to_global` of ADR 0546 D2); `M_aug = embed(M) + sum_e T^T (M14_l - embed(M12_l)) T`, `M` the model's
`consistent_with`.

### D5. Modes and pre-stressed modes

`modal_analysis(model, warping, options: &ModalOptions)` solves `K_aug x = omega^2 M_aug x`; `prestressed_modes(model,
prestress: &WarpingPrestress, warping, options)` solves `(K_aug + K_sigma,aug) x = omega^2 M_aug x` (`K_aug`, `K_sigma,aug` of
ADR 0546 D5 with the pre-stress of D2; RF8 as D3; RF10). The steps of `geometric::prestressed_modes` on the elimination path: a
nonzero prescribed value refused (key `"constraint"`); the model's prescribed and the restrained warping equations eliminated;
`rigid_free` from `check_rigid_body_motions`; the shift `options.shift` or `Shift::Auto` with a free rigid motion and
`Shift::Value(0)` otherwise; a mass diagonal `<= 0` refused (key `"density"`, naming the warping unknown of a node);
`lowest_eigenpairs` with the options; `coordinate_order` refused (key `"factorisation"`); the transformation path refused (RF4).
`WarpingModalResult`: eigenvalues, frequencies, modes on all equations, residuals, iterations, shift, Sturm check, unknowns,
`rigid_free`, the numbering.

### D6. Mixed models, refusals

As ADR 0546 D4 (RF1 to RF9 with the same keys), and RF10 (D3). A model without warping blocks is not taken (RF1): the default
paths are the model's own functions.

### D7. Files

Changed: `src/beam_warping.rs` (the new functions; `matrices` and `buckling_factors` refactored onto a shared internal function
with the same arithmetic), `src/geometric.rs` (one `pub(crate)` method `with_beam_end_forces`, no existing line changed in
behaviour). New tests: `tests/sa_beam_warping_f626.rs`, `tests/sa_beam_warping_f626_bits.rs`. Not touched: `buckling.rs`,
`nonlinear.rs`, `nl_transient*`, `shell3.rs`, `solve.rs`, `modal.rs`, `mass.rs`. Docs: `docs/structural-solver.md` (a
paragraph) if it has a beam section.

### D8. The bit record B0

`tests/sa_beam_warping_f626_bits.rs`, written and run on the base (e906faa8 plus this ADR, no library line changed): digests
(FNV-1a 64 of ADR 0291) of the plain static displacement of the I cantilever under end torque and a transverse force, the plain
modes (eigenvalues and modes, 4) of the fork I beam, its plain pre-stressed modes under axial tension, the ADR 0546 element
matrices (`beam_warping::local_stiffness`, `local_geometric_stiffness` with end forces), the ADR 0546 `matrices` (stiffness and
geometric values) and `buckling_factors` (factors and modes) of the uniform-moment I beam. It runs unchanged after the change.

## Arithmetic done before the code (disclosed)

1. **Section** (`arith.log`): `A = 3.08e-3`, `I_y = 2.098267e-5`, `I_z = 1.669907e-6`, `J = 7.962667e-8`, `C_w = 1.504167e-8`,
   `I_p = 2.265257e-5`, `G J = 6125.128`, `E C_w = 3008.333`, `k = sqrt(G J/(E C_w)) = 1.426904 /m`, `kL = 4.2807`.
2. **Closed forms, statics.** (C1) Cantilever, `E C_w phi'''' - G J phi'' = 0`, `phi(0) = phi'(0) = 0`, `phi''(L) = 0` (free
   bimoment), `G J phi' - E C_w phi''' = T`: `phi' = (T/(G J))(1 - cosh(k(L - x))/cosh(kL))`, so `phi(L) = (T/(G J))(L -
   tanh(kL)/k) = 0.37541254` (T = 1000) and the root bimoment `B(0) = -E C_w phi''(0) = -T tanh(kL)/k`, `|B(0)| = 700.5501`. With
   the root warping free `phi(L) = T L/(G J) = 0.48978567`. 16 Hermite elements (the element of ADR 0546): `-1.07e-6` (8
   elements `-1.7e-5`), root free `-4e-13`, `|B(0)|` `+3.5e-6`. (C2) Interior torque: `phi(0) = phi'(0) = 0`, `phi(L) = phi''(L) =
   0`, torque `T` at `a = 0.75` (node 4 of 16), continuity of `phi`, `phi'`, `phi''` at `a`, the torque jumps by `T`: the 8 x 8
   system of the exponential solutions gives the internal torques `T_L = 874.16254908`, `T_R = -125.83745092` (per `T = 1000`);
   Saint-Venant alone `750`, `-250` (`-14.2` percent, `+98.7` percent). 16 Hermite elements: `+2.0e-7` (left), `-1.4e-6`
   (right); 8 elements `2.9e-6`, `-2.0e-5`.
3. **Hermite mass**: `int H_i H_j dx` closed (the standard cubic-Hermite consistent mass); `int` of the rigid twist (`phi_0 =
   phi_1 = 1`, `chi = 0`) is `L`, so its kinetic matrix product is `rho I_p L`.
4. **Lateral-torsional buckling with torque** (`arith.py` part 3: `v`, `w`, `phi` cubic Hermite, Euler-Bernoulli, the moment
   terms `-(1/2) M_y (theta_z phi' - phi theta_z')` and the torque terms `-(1/2) T (theta_y theta_z' - theta_z theta_y')` of ADR
   0508 D1, `theta_z = v'`, `theta_y = -w'`; fork supports `v = w = phi = 0` at both ends, warping held at `x = 0` and free at `x
   = L`, 16 elements, uniform `M_y = 1000`, the torques of C2 times `T_ref`): `M` only `lambda = 74.368`. With `T_ref = 4e4`:
   `lambda = 62.3156` with the closed-form split, the 16-element Hermite split `-5.5e-8` from it, the Saint-Venant split
   `+2.58e-2`; the torque lowers `lambda` by `16.2` percent; `-T_ref` gives the same `lambda` (to the last digit: the torque
   couples `v` and `w`, the moment `v` and `phi`, so `lambda` is even in `T`). `T_ref = 1e4`: Saint-Venant `+1.3e-3` only (too
   small to separate); `8e4`: `+7.8e-2` but the torque would dominate (`-42` percent). **`T_ref = 4e4` registered.**
5. **Torsional frequencies** (C3; first principles: `E C_w phi'''' - G J phi'' + rho I_p phi_tt = 0` with `phi = sin(k_m x)
   e^{i omega t}`, `k_m = m pi/L`, fork supports with the warping free, `phi = phi'' = 0`, satisfied by the sine): `omega_m^2 = (G J
   k_m^2 + E C_w k_m^4)/(rho I_p)`; with an axial force `N` (tension positive) `G J -> G J + N I_p/A` (Wagner's term of ADR 0473
   D1). `L = 3`: `omega^2 = 58118.116`, `476609.95`, `1987887.97` (38.37, 109.88, 224.40 Hz); 16 Hermite elements with the D4
   mass: `+7.2e-7`, `+2.2e-5`, `+1.37e-4`; 8 elements `+1.2e-5`, `+3.6e-4`, `+2.1e-3`; `C_w = 0`: `1.9e-9`, the warping term
   raises `omega_1^2` by `1.5386`. Warping held at both ends: `omega_1^2 = 151151` (2.60 of the free value), below the Rayleigh
   bound of the clamped mode `1 - cos(2 pi x/L)`, `158870`. The torsional buckling load `N_T = -(G J + E C_w k_1^2) A/I_p =
   -1.2814e6` (the weak-axis Euler load `3.66e5` is lower: compression is tested on the torsion-only model only); `N = 0.5 |N_T|`:
   ratio `1.5`, `omega_1^2 = 87177.17`, FE `+4.8e-7`; `N = -0.5 |N_T|`: ratio `0.5`, `29059.06`, FE `+1.4e-6`. The full fork beam
   (Euler, pinned both planes): weak bending `16612`, then torsion `58118`, then strong bending `208730` (Timoshenko lowers it
   by about 2 percent), weak `m = 2` `265789`: the torsional mode is the second; with `N = 0.5 |N_T|` the weak bending moves to
   about `45672` and torsion to `87177`, still the second.
6. **Pre-stressed modes with torque** (`arith2.log`, the part-4 beam with masses `rho A` on `v`, `w` and `rho I_p` on `phi`):
   at 30 times the reference load (0.48 of its buckling factor) the lowest four `omega^2` with the Hermite split are within
   `5.2e-8` of the closed-form split; with the Saint-Venant split `+1.30e-2`, `+5.5e-4`, `+1.79e-2`, `-4.5e-3`.

## Checks (limits fixed now)

Every aggregated error is asserted finite before it is compared (a non-finite value is a miss). Beams of the I section of
arithmetic 1, `L = 3`, 16 elements along `x` unless stated.

| id | check | limit | estimate |
|---|---|---|---|
| B0 | the bit record of D8, base and after | equal (bit) | exact |
| S1 | **acceptance a1**: cantilever, root held (six components and warping), `T = 1000` at the tip, `solve_static`: `|phi(L)/C1 - 1|` | `<= 1e-3` | `1.1e-6` |
| S2 | the same, root warping free: against `T L/(G J)` | `<= 1e-10` | `4e-13` |
| S3 | S1's reaction at the restrained warping unknown of the root: `||B|/(T tanh(kL)/k) - 1|` | `<= 1e-4` | `3.5e-6` |
| S4 | S1 at 8 elements | `<= 1e-3` | `1.7e-5` |
| S5 | S1 along `(1, 1, 0)/sqrt 2` (orientation rotated with it, the torque about the axis): the tip rotation about the axis against S1's | `<= 1e-9` relative | `1e-13` |
| S6 | a torsion-free load on S1's model (tip force `(1e4, 200, -300)`, tip moment `(0, 50, -20)`): `max|u_six - u_plain|/max|u_plain|` against `solve::solve_static`; `max|chi| <= 1e-12 max|u_plain|` | `<= 1e-10`; strict | `1e-14`; `0` |
| S7 | S1's twist reaction at the root (component 3 of node 0) against `-T` | `<= 1e-10` relative | `1e-13` |
| T1 | indeterminate twist: twist held at both ends (and the fork supports), warping held at `x = 0`, torque `T = 1000` at node 4: the torque `f[9]` of each warping element of `WarpingPrestress::from_static` against C2 (`T_L` on elements 0 to 3, `T_R` on 4 to 15) | `<= 1e-4` relative, max | `1.4e-6` |
| T2 | **acceptance a2**: the T1 beam with end moments `-/+M_ref = 1000` about `e_2` and the torque `T_ref = 4e4` at node 4, warping held at `x = 0` in the static solve and the buckling: `lambda` of `buckling_factors_with(from_static)` against `lambda` of `with_torsion` of the hand pre-stress whose torques are C2 times `T_ref` (the plain static displacement with its twist replaced by the twist whose element torques are C2, through `Prestress::with_beam_resultants`) | `<= 1e-5` relative | `6e-8` |
| T3 | negative control: `lambda` of the Saint-Venant pre-stress (the plain static solve, `with_beam_resultants`, through `with_torsion`) against T2's hand `lambda` | `> 1e-2` | `2.6e-2` |
| T4 | the torque acts: `lambda(T2)/lambda(M only, from_static) - 1` | `< -0.1` | `-0.162` |
| T5 | `-T_ref`: `|lambda(+T)/lambda(-T) - 1|` | `<= 1e-8` | `0` |
| T6 | RF8 kept: ADR 0546 `buckling_factors` with the plain pre-stress of T3 refused (key `"warping"`); `buckling_factors_with` of `torsion_free` of it refused (key `"warping"`) | exact | |
| T7 | torsion-free: the ADR 0546 A1 beam (fork, warping free, uniform moment): `buckling_factors_with(from_static)` against ADR 0546 `buckling_factors` (the plain pre-stress) | `<= 1e-9` relative | `1e-12` |
| F1 | **b1**: torsion-only model (components 0, 1, 2, 4, 5 held at every node, the twist at both ends, warping free), `modal_analysis`: `omega_m^2` against C3, `m = 1, 2, 3` | `<= 1e-4`, `1e-3`, `5e-3` | `7.2e-7`, `2.2e-5`, `1.4e-4` |
| F2 | F1 at 8 elements, `m = 1` | `<= 1e-3` | `1.2e-5` |
| F3 | F1 with `saint_venant()`: `omega_1^2` against `G J k_1^2/(rho I_p)`; and against the plain `modal::modal_analysis` (linear twist) of the same model | `<= 1e-4`; `<= 1e-2` | `2e-9`; `3e-3` |
| F4 | the full fork beam (`u = 0` at node 0, `v = w = phi = 0` at both ends, warping free), 4 modes: mode 2 against C3 `m = 1`; modes 1 and 3 outside `(1 -/+ 0.1)` of it | `<= 1e-4`; strict | `1e-6`; `0.29`, `3.5` |
| F5 | F1 with the warping held at both ends: `1.2 omega_1^2(free) < omega_1^2 < 158870 (1 + 1e-2)` | strict | `151151` |
| F6 | element mass: the twist block of `local_mass` (`L = 0.7`) against D4's hand matrix; the other entries bit-equal to the embedded `beam::local_mass`; the rigid twist `x^T M x` against `rho I_p L` | `<= 1e-12` relative; exact; `<= 1e-12` | `1e-16`; exact; `1e-16` |
| F7 | frame invariance of `element_mass` along `(1, 1, 0)/sqrt 2` against `Q M Q^T` of the axis-`x` element | `<= 1e-12 max|M|` | `1e-16` |
| P1 | **b2**: the F1 model with the pre-stress of `WarpingPrestress::from_static` of an axial tip force `N = 0.5 |N_T|` (static model: the fork beam), `prestressed_modes`: `omega_1^2` against C3 with `N`; the same with `N = -0.5 |N_T|` | `<= 1e-4` each | `4.8e-7`, `1.4e-6` |
| P2 | the F4 full beam with `N = 0.5 |N_T|`: mode 2 against C3 with `N` | `<= 1e-4` | `1e-6` |
| P3 | `prestressed_modes` with a zero pre-stress (`Prestress::uniform` zero, `torsion_free`) against `modal_analysis`, F1 model, 3 modes | `<= 1e-12` relative | `0` |
| PT1 | **b2 with torque**: the full T2 beam (warping held at `x = 0`), the `from_static` pre-stress times 30, `prestressed_modes` (4 modes): the lowest three against the T2 hand pre-stress times 30 | `<= 1e-5` relative each | `5e-8` |
| PT2 | negative control: mode 1 with the Saint-Venant pre-stress times 30 against the hand | `> 5e-3` | `1.3e-2` |
| PT3 | `prestressed_modes` with `torsion_free` of the plain T3 pre-stress refused (RF8, key `"warping"`) | exact | |
| R | refusals on the new entries, key each: `solve_static` with a non-Beam2 block (RF1), a node shared with a non-warping element (RF2), an elastic support (RF4), an unknown restrained set (`UnknownSet`, RF6); `from_static` with a result of another numbering (`"warping"`); `buckling_factors_with` of a `from_static` pre-stress of other sections (RF10); `modal_analysis` with `coordinate_order` (`"factorisation"`), with a nonzero prescribed value (`"constraint"`), on a massless model (`"density"`) | exact | |

Negative controls: T3 and PT2 (the Saint-Venant split gives a different factor and frequency: the check sees the source of the
pre-stress), T4 (the torque acts), S2 (the restraint acts), F3 (`C_w = 0` gives the Saint-Venant frequency), F5 (direction of
the warping restraint), T6 and PT3 (RF8 kept for the other pre-stresses).

## Planned mutations (on a scratch copy, one at a time, own `CARGO_TARGET_DIR`)

| id | mutation | expected caught by |
|---|---|---|
| Y1 | the restrained warping equations not eliminated in `solve_static` | S1, S3 |
| Y2 | `from_static` takes the 12-unknown end forces (`local_end_forces` of the six unknowns) for the warping elements | T1, T2, PT1 |
| Y3 | `from_static` drops the torque (`f12[3] = f12[9] = 0`) | T1, T2, T4, PT1 |
| Y4 | the mass difference `M14 - M12` not added (the linear torsion mass kept, no mass on `chi`) | F1, F6, R (density) or a refusal |
| Y5 | the sign of `H_4` flipped in the Hermite mass | F1, F6 |
| Y6 | `prestressed_modes` without the geometric stiffness | P1, P2, PT1 |
| Y7 | RF8 not applied to `torsion_free` | T6, PT3 |
| Y8 | RF10 removed | R |
| Y9 | the stiffness difference `K14 - K12` not added in `solve_static` | S1, S2 (a singular-stiffness refusal or a wrong twist) |
| Y10 | the reactions computed without the warping equations | S3 |
| Y11 | the load vector's warping rows set to `1` (a spurious bimoment) | S1, S2, S6 |

## Decision rule

**done**: B0, S1 to S7, T1 to T7, F1 to F7, P1 to P3, PT1 to PT3 and R met, and every mutation caught. **partial**: any
registered check missed (a `*_known_miss` record), any mutation not caught, or any check met only after a post-run amendment.
**todo**: S1, T2 or B0 missed.

## Limits (not covered)

- No bimoment load and no bimoment pre-stress (`beta_omega` terms; zero for doubly symmetric sections); no distributed torque.
- No warping inertia `rho C_w` in the mass (D4).
- No shear-centre offset (monosymmetric sections: F-627), no warping continuity through kinks (RF3), no transformation path (RF4).
- The rotating part, the harmonic and transient paths and the nonlinear drivers do not take the warping unknown.
- Only the sparse direct solver on the static solve (no iterative solver, no refinement).
- No case-file key (F-625).

## Open questions (to be taken on the maintainer's standing instruction of 2026-11-11 when answered; none changes a default)

- **Q1. Warping inertia.** Options: (a) none (this item); (b) an opt-in `rho C_w` term. Recommendation (a) until a check needs
  it: it changes the torsional frequencies of short stocky beams only (`C_w k^2/I_p` = `7e-4` here for `m = 1`).
- **Q2. The warping unknown on the transient and harmonic paths.** Recommendation: a separate item when a case needs it.

## Addendum 1 (2026-11-15): results

Written after the one registered run of `sa_beam_warping_f626.rs` (log `f626-scratch/run1.log`, code commit 514522b1), the B0 run
after the change (`b0_after.log`) and the mutation runs (`mutations.log`, `mut_Y*.log`). The numbers are those the tests printed
(release). There was no amendment and no re-run.

### Results against the registered limits

| id | measured | limit | verdict |
|---|---|---|---|
| B0 | 11 digests bit-identical to the base record (and ADR 0546's 12 and ADR 0508's 16 also unchanged) | equal | met |
| S1 (acceptance a1) | `1.07e-6` (`phi(L) = 0.37541214` against `0.37541254`) | `1e-3` | met |
| S2 (root warping free) | `1.2e-13` | `1e-10` | met |
| S3 (root bimoment) | `3.5e-6` (`-700.5525` against `700.5501`) | `1e-4` | met |
| S4 (8 elements) | `1.7e-5` | `1e-3` | met |
| S5 (skew axis) | `5.7e-12` | `1e-9` | met |
| S6 (torsion-free load) | `1.4e-12`; `max|chi| = 0` | `1e-10`; strict | met |
| S7 (twist reaction) | `3.7e-15` | `1e-10` | met |
| T1 (torque split) | `1.39e-6` (874.16272/-125.83728 against 874.16255/-125.83745) | `1e-4` | met |
| T2 (acceptance a2) | `5.4e-8` (`lambda` 62.421729 against 62.421732) | `1e-5` | met |
| T3 (Saint-Venant split) | `2.55e-2` (64.0129) | `> 1e-2` | met |
| T4 (the torque acts) | `-0.161` (M only 74.417) | `< -0.1` | met |
| T5 (`-T_ref`) | `1.2e-15` | `1e-8` | met |
| T6 (RF8 kept) | both refused, key `"warping"` | exact | met |
| T7 (torsion-free against ADR 0546) | `1.1e-15` | `1e-9` | met |
| F1 (`m = 1, 2, 3`) | `7.2e-7`, `2.25e-5`, `1.37e-4` | `1e-4`, `1e-3`, `5e-3` | met |
| F2 (8 elements) | `1.16e-5` | `1e-3` | met |
| F3 (`C_w = 0`) | `1.9e-9`; `3.2e-3` against the plain linear-twist modes | `1e-4`; `1e-2` | met |
| F4 (full beam, mode 2) | `7.2e-7`; modes 1 and 3 at `0.285` and `3.49` of it | `1e-4`; strict | met |
| F5 (warping held at both ends) | `151151` (2.60 of free) | strict | met |
| F6 (element mass) | `2.8e-17`; bit-equal; `0` | `1e-12`; exact; `1e-12` | met |
| F7 (mass frame invariance) | `2.9e-16` | `1e-12` | met |
| P1 (`N = +/-0.5 |N_T|`) | `4.8e-7`, `1.44e-6` | `1e-4` each | met |
| P2 (full beam in tension) | `4.8e-7` | `1e-4` | met |
| P3 (zero pre-stress) | `0` | `1e-12` | met |
| PT1 (modes with torque) | `2.8e-8`, `1.0e-9`, `4.8e-8` | `1e-5` each | met |
| PT2 (Saint-Venant split) | `1.30e-2` | `> 5e-3` | met |
| PT3 (RF8 kept) | refused, key `"warping"` | exact | met |
| R (9 refusals) | each with its key | exact | met |

Every measured value is within a factor of 3 of its estimate in the arithmetic, except S5 (`5.7e-12` against `1e-13`). S5 is the
rounding of the rotated frame through the sparse solve, 175 times below its limit.

### Mutations (scratch copy `f626-scratch/mut`, own target dir, one at a time)

| mutation | result | caught by |
|---|---|---|
| Y1 restraint not eliminated in the static solve | caught | S1, S3, S4, T1, T2, PT1 |
| Y2 12-unknown end forces for the warping elements | caught | T1, T2, T4, PT1 |
| Y3 torque dropped from the pre-stress | caught | T1, T2, T4, PT1 |
| Y4 mass difference not added | caught | F6, and every modal test refused (`"density"`, the warping unknown has no mass) |
| Y5 `H_4` sign flipped in the mass | caught | F1 to F4, F6, P1, P2 |
| Y6 pre-stressed modes without `K_sigma` | caught | P1, P2, PT2 |
| Y7 RF8 not applied to `torsion_free` | caught | T6, PT3 |
| Y8 RF10 removed | caught | R (RF10) |
| Y9 stiffness difference not added in the static solve | caught | S, T and PT tests (singular stiffness at the warping unknown) |
| Y10 reactions without the warping equations | caught | S3 |
| Y11 spurious bimoment load on the warping rows | caught | S1 to S4, S6, T1, T2, T5, T7, PT1 |

All eleven mutations are caught. Y6 was predicted to be caught by PT1 but is caught by PT2: without `K_sigma`, all three pre-stresses
give the same modes, so PT1 is met trivially and PT2 fails.

### Other runs

`cargo test -p rcfd-fem --release`: 244 test binaries and doc-tests, all pass, none failed (the ignored records stay ignored).
`cargo test -p rcfd-api --release --test baseline_hashes`: 3 passed, 1 ignored (unchanged). `cargo test -p rcfd-cli --release
--test docs_site`: 2 passed. `cargo clippy -p rcfd-fem --release --all-targets -- -D warnings`: clean. `cargo fmt -p rcfd-fem
--check`: clean. After the B0 commit, `cargo fmt` reformatted the B0 file `sa_beam_warping_f626_bits.rs` (whitespace only, same
digests). No documentation page was changed: `docs/structural-solver.md` has no beam section (D7).

### Decision

**Done** by the registered rule. Every registered check is met in the one run: B0, S1 to S7, T1 to T7, F1 to F7, P1 to P3, PT1 to
PT3 and R. Eleven of eleven mutations are caught. There was no amendment. The acceptance is met: the static twist of the
cantilever is `1.07e-6` of the closed form at 16 elements. The lateral-torsional factor with torsion in the reference load equals
the factor from the closed-form warping torque split to `5.4e-8`, and the Saint-Venant split gives a factor `2.6e-2` away. The
defaults and the ADR 0546 path are bit-identical (B0).

### Remaining questions

- Q1 (warping inertia `rho C_w`): (a) none; (b) an opt-in term. Recommendation (a) until a check needs it (`7e-4` of the first
  torsional frequency here).
- Q2 (the warping unknown on the transient and harmonic paths): recommendation, a separate item when a case needs it.
- Q3 (bimoment loads and the bimoment pre-stress, `beta_omega`): needed only with F-627 (monosymmetric sections). Recommendation:
  take them in F-627.
