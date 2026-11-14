# ADR 0525: F-588 registration: the Hex8 formulations (B-bar, incompatible modes) on the remaining linear paths

Status: **REGISTERED 2026-11-14, written before any code of F-588 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-588 was written, compiled or run in the repository. The arithmetic under "Arithmetic
done before the code" was done with plain numpy (dense, no rcfd code) in the scratch folder `w49i-work/f588-scratch`
(`proto.py` copied from `f390-scratch`, `est588.py`, `est588b.py`, logs `est588.log`, `est588b.log`; disclosed, its numbers quoted
below). The bit record B1 of the default paths is a test file written and run on the base after this commit and before any
library code (D7). Results go in the addendum at the end; nothing above the addendum is changed after a run.

Roadmap item F-588 (deps F-390), follow-up 2 of [ADR 0493](0493-f390-registration-hex8-b-bar-and-incompatible-modes.md) (the
refusals of its D5). Number 0525 given by the lead. Branch `f588-work` from local main f168d994, worktree `w49i-work/f588`,
scratch `w49i-work/f588-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored `*_known_miss`
test with the limit intact and the measured number recorded); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe code; clean-room. The
brief overrides rule 9 for this item: the F-588 entries of `docs/ROADMAP.md` and `docs/roadmap.json` are updated on the branch in a
separate last commit (the svgs are not regenerated). Other agents edit F-604, F-592 and F-584: the hunks of this item in shared
files are small (listed in the addendum).

## What F-588 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. The refusals of ADR 0493 D5 lifted with consistent
> arithmetic: the free thermal expansion of the distorted F-374 patch stress-free within 1e-10 with both formulations; an
> orthotropic patch within 1e-10; Euler buckling of a one-layer Incompatible strip within registered limits; the damped modes and
> the direct harmonic solve with the formulations' stiffness (the viscoelastic shear and bulk split refused for Incompatible,
> whose condensed stiffness is not linear in D); the patch recovery with the formulations' strain; defaults bit-identical.

Clauses: a1 the temperature load; a2 per-point materials (orthotropic, tabulated); a3 the geometric stiffness (Euler buckling);
a4 the damped modes; a5 the direct harmonic solve; a6 the patch recovery; a7 the viscoelastic split refused for Incompatible;
a8 defaults bit-identical, no case-file key.

Readings fixed here. "Both formulations" is `BBar` and `Incompatible` of `SolidFormulation`. "The distorted F-374 patch" is the
patch box of ADR 0493 reading (a2) (origin `[0.1, -0.2, 0.3]`, lengths `[1.2, 1, 0.8]`, division `[4, 3, 3]`) with the interior
nodes moved by `0.25 h_k (2U - 1)`, SplitMix64 seed `0xF374_0275 + 1001` (the mesh of ADR 0493 PT2; the milder `0.1 h`, tag 11,
is used where stated). "Free" thermal expansion: the model has no support but six displacement components, fixed to zero at three
corners of the box (`A` the origin corner: three components; `B` the corner at `+L_x`: `u_y`, `u_z`; `C` the corner at `+L_y`:
`u_z`), which remove the rigid motions and no strain. "Stress-free" is measured against `s_0 = max |D| max |eps_th|`.
"Euler buckling of a one-layer strip": the clamped-free column `P_cr = pi^2 E I / (4 L^2)` with `I = h^4/12`, `L = 2`, `h = 0.1`
(a square section), `20 x 1 x 1` elements, the root face fixed in all three components, the axial compression a uniform traction
on the tip face, the load factor of `buckling::factors` with the pre-stress of the static result. "The formulations' stiffness"
of a path is the stiffness of `assemble_stiffness` (the element of `hex8x`) in every place where the path assembles an element
matrix. The nonlinear driver is out of scope (ADR 0506 and F-587).

## What was read (worktree at f168d994, read only)

ADR 0493 (D1 to D8, checks, addenda), `crates/rcfd-fem/src/hex8x.rs`, `assemble.rs` (`assemble_stiffness`, `point_stiffness`),
`recover.rs` (`compute_with`), `thermal.rs` (`add_thermal_load`), `mat_ortho.rs` (`MaterialPoints`, `element_stiffness`,
`element_thermal_load`, `thermal_strain`), `geometric.rs` (`assemble_geometric`, `Prestress`), `damped.rs` and `harmonic_direct.rs`
(every `element_for(..).stiffness` closure), `estimate.rs` (`prepare`, `element_samples`, `energy_error`), the tests
`si_hex8x.rs`, `si_hex8x_bits.rs`, `si_fbar.rs` (the copy `rf1_rf2_rf3_refusals_after_f587`), `docs/reference-data.md` (the F-390
entry), `AGENT_RULES.md`. No source outside the repository was read; the methods are cited as in ADR 0493.

## Arithmetic done before the code (disclosed)

Plain numpy, dense, element matrices as in ADR 0493's `proto.py`.

1. The thermal load of the formulations (derivation). With the thermal strain `eps_th,q` at the point `q` of the 2 x 2 x 2 rule
   (from the interpolated temperature change `dT_q = sum_b N_b(q) dT_b` and the material's expansion), the equivalent load of
   `BBar` is `f = sum_q w_q Bbar_q^T D_q eps_th,q` and of `Incompatible` the condensed load `f = f_u - K_ua K_aa^{-1} f_a`,
   `f_u = sum_q w_q B_q^T D_q eps_th,q`, `f_a = sum_q w_q G_q^T D_q eps_th,q`. The strain recovered at the points is
   `Bbar_q u` and `B_q u + G_q alpha` with `alpha = K_aa^{-1} (f_a - K_au u)` (the stationarity of the energy in `alpha`); the stress
   is `D_q (eps - eps_th,q)`. For a constant `eps_th` and the field `u = eps_th (x - x_A)`: `B_q u = eps_th`, `Bbar_q u = eps_th`
   (the mean of the dilatation of a linear field is its value), `K_au u = f_a` (the integral of `G^T D eps_th` against the same
   field), so `alpha = 0`, the stress is zero and `K u - f = 0`. Prototype (`est588b.py`, the distorted element of `est1.py`, isotropic
   `nu = 0.4999` and rotated-free orthotropic `D` with a full six-component `eps_th`): residual `|K u - f|_max / |f|_max` `4.2e-16`
   (BBar iso), `1.1e-15` (BBar orthotropic), `2.8e-16` (Incompatible iso), `4.1e-16` (Incompatible orthotropic); stress `|D (eps -
   eps_th)|_max / (|D| |eps_th|)` `9.0e-16`, `3.2e-16`, `7.2e-16`, `5.1e-17`.
2. The uniform thermal strain cannot see a wrong B-bar thermal load: `Bbar^T s = B^T s` for a deviatoric `s` and, for a uniform
   `s`, `int (Bbar - B) dV = 0`. Only a non-uniform `eps_th` does (TH4).
3. Per-point materials: the stiffness of both formulations takes `D_q` at the 8 points of the rule (`K_uu`, `K_ua`, `K_aa` use
   `D_q`; `BBar` sums `Bbar_q^T D_q Bbar_q`); symmetric, with the six rigid motions in the kernel (the strain is zero).
4. The additivity in `D` (the reason for the refusal of the viscoelastic split): prototype, the distorted element, `D_1` isotropic
   `nu = 0.3`, `D_2` orthotropic (identity axes, `E = (1.4e11, 1e10, 1e10)`, `nu_12 = nu_13 = 0.3`, `nu_23 = 0.4`, `g = (5e9, 3.5e9,
   5e9)` as `(g_12, g_13, g_23)`): `max |K(D_1 + D_2) - K(D_1) - K(D_2)| / max |K|` `2.5e-16` (Full), `1.7e-16` (BBar),
   `1.03e-2` (Incompatible).
5. Euler buckling of the one-layer strip (`est588.py`; the standard geometric stiffness `sum_q w_q g_a . S_q g_b I_3` with the
   stress of the formulation at the points, the stiffness of the formulation, the lowest factor of `K u = -lambda K_G u` by a
   dense Cholesky reduction, ratio `lambda P / P_cr`, the unit load): `20` elements, root face fully fixed:
   `nu = 0`: Full `1.49846`, BBar `1.07402`, Incompatible `0.99949`; `nu = 0.3`: Full `1.54202`, BBar `0.82617`, Incompatible
   `1.00306`. (`10` elements, not a check: Incompatible `1.00257`, `1.00974`.)
6. The patch test and the recovered stress of an orthotropic material are the same algebra as ADR 0493 arithmetic 5 with a
   constant `D`; the Taylor correction makes `K_ua` annihilate a constant-strain field for any `D`, and `Bbar u` is the field's
   strain for any `D`. Estimated at the rounding of ADR 0493's PT1, PT2 (`5e-16` to `8e-14`), times the condition of an `E` ratio of
   14: below `1e-12`.

Not computed beforehand: the damped-mode and harmonic numbers (closed forms, below), the thermal numbers beyond item 1.

## Design

### D1. `hex8x`: point data and the new entries

`element_stiffness`, `point_strains`, `internal_modes` keep their signatures and call the new entries with eight copies of the
one matrix (the same arithmetic in the same order, so their bits are unchanged). New public functions, each refusing a list of
other than eight matrices (`FieldLength`):

- `element_stiffness_at(f, coords, ds)`: `ds` the eight elasticity matrices at the points of the 2 x 2 x 2 rule in its order.
- `element_thermal_load(f, coords, ds, eth)`: the load of arithmetic 1, 24 entries node-major.
- `point_strains_at(f, coords, ds, eth, u)`: the strains at the eight points; with `eth` the modes of `Incompatible` are
  `alpha = K_aa^{-1} (f_a - K_au u)`; `eth = None` is `point_strains`.
- `internal_modes_at(coords, ds, eth, u)`.
- `strain_at(f, coords, d, u, xi)`: the strain at any point `xi` of the reference cube for a constant `D` and no thermal strain:
  `Full` `strain_at` of the element, `BBar` the B-bar matrix at `xi` (the mean gradients of the 2 x 2 x 2 rule, the point's own
  gradients), `Incompatible` `B(xi) u + G(xi) alpha` with `G(xi) = (det J_0 / det J(xi)) J_0^{-T} dP/dxi (xi)` and `alpha` of the
  2 x 2 x 2 rule. Used by the patch recovery (centroid and the 3 x 3 x 3 rule).
- `pub(crate)`: `element_data(material, points, e, coords, dte)` the eight `D_q` and (with a temperature) `eps_th,q` of an element
  at its stiffness points: for an orthotropic or tabulated material the `MaterialPoints::context` of ADR 0452 and
  `elasticity_at`, `mat_ortho::thermal_strain`; otherwise `elasticity_matrix()` and `thermal::thermal_strain` with the
  interpolated temperature. `element_stiffness_for(mesh, e, kind, coords, d)`: the formulation's stiffness for an opted Hex8,
  `element_for(kind).stiffness` otherwise (used by every closure of `damped` and `harmonic_direct`).
  `refuse_incompatible_viscoelastic(model, path)`.

`check_model` and `refuse_formulations` of ADR 0493 D5 are removed (no caller remains); the constants `FOLLOW_UP` and `PENDING`
stay.

### D2. The paths

- Temperature load (`thermal::add_thermal_load`): an opted Hex8 takes `element_thermal_load` with `element_data`. Other elements
  unchanged. The `check_model` call is removed.
- Per-point materials (`assemble::assemble_stiffness`): an opted element takes `element_stiffness_at` with `element_data` (the
  per-point matrices when the material is orthotropic or tabulated and the model's `MaterialPoints` exists); the `check_model`
  call is removed.
- Recovery (`IntegrationPointField::compute_with`): an opted element takes `point_strains_at` with the same data (so the strain is
  the formulation's, with the thermal modes); the stress and the thermal strain of the field are computed as before.
- Geometric stiffness (`geometric::assemble_geometric`): the refusal is removed. The element matrix is the standard
  `element_geometric_stiffness` with the stress of the formulation at the 8 points (the pre-stress of `Prestress::from_static` is
  the formulation's `D (eps - eps_th)`); the geometric stiffness takes the compatible gradient only (the gradient of the internal
  modes is not included; recorded as limit L3).
- Damped modes and direct harmonic solve: the closures that call `element_for(kind).stiffness(coords, d)` call
  `element_stiffness_for`. A model with an `Incompatible` block whose material is `Viscoelastic` is refused
  (`NotYetSupported { item: "formulation", variant: "Incompatible", location: "block b, <path> (the viscoelastic moduli ...)",
  package: FOLLOW_UP }`) by `damped_modes` (on the original model, before the storage substitution) and by `DirectHarmonic::new`:
  the stiffness of a split `G(w) 2 I_dev + K(w) 1 (x) 1` is a sum of two stiffnesses only if the stiffness is linear in `D`, which
  holds for `Full` and `BBar` (arithmetic 4). The brief names the split; the refusal here is of every viscoelastic material on an
  `Incompatible` block, because the equal-weight case is untested (a limit, L2). `BBar` takes every viscoelastic path.
- Patch recovery (`estimate.rs`): `prepare` no longer refuses; `element_samples` and `energy_error` call `hex8x::strain_at` for an
  opted element. A temperature field stays refused there (ADR 0478, unchanged), and the per-point materials too (no constant `D`).

No case-file key is added (the formulation is the block property of ADR 0291, set through the mesh).

### D3. Defaults

Anything that asks for no formulation takes the former code: each hunk is guarded by `opted(..)` (only a Hex8 of a block of a
formulation other than `Full`) or sits in a closure whose non-opted branch is the former call.

### D4. Test files (written after this registration)

`tests/si_hex8x_linear_bits.rs` (B1, written and run on the base before any library code), `tests/si_hex8x_linear.rs` (the
checks). `#[ignore = "record of ADR 0493 / ADR 0506 ... before ADR 0525 (F-588)"]` is added to `rf1_rf2_rf3_refusals_after_f587` of
`si_fbar.rs` (and nothing else of that file): its rows RF2 and RF3 assert the refusals of D5, which this item lifts; its copy
`rf1_rf2_rf3_refusals_after_f588` is in `si_hex8x_linear.rs` (RF1 kept, the lifted rows now "taken", the nonlinear rows as in the
f587 copy). `si_hex8x.rs::rf1_rf2_rf3_refusals` is already an ignored record.

### D5. Names in the topic documentation

`docs/reference-data.md` gains an F-588 section (no source read; Euler's buckling load closed form; the methods as ADR 0493).

## Checks (limits fixed now)

Test parameters fixed here. Steel: `E = 2e11`, `rho = 7850`, `alpha = 1.2e-5`. Orthotropic "O": `E = (1.4e11, 1e10, 1e10)`, `nu =
(0.3, 0.3, 0.4)` as `(12, 13, 23)`, `g = (5e9, 5e9, 3.5e9)`, `expansion = (2e-6, 3e-5, 4e-5)`, axes `Fixed` the rotation `Rz(0.5)
Rx(0.3)` (rows the material directions). Temperatures: `T_ref = 293`, uniform `T = 353` (`dT = 60`) unless stated. Every aggregated
error is required finite before it is compared (a NaN misses).

| id | clause | check | limit | estimate behind it |
| --- | --- | --- | --- | --- |
| TH1 | a1 | the distorted patch (`0.25 h`, tag 1001), steel, uniform `dT = 60`, free (three corners, six components), `nu = 0.3` and `0.4999`, both formulations: `max |sigma| / (E alpha dT)` of `IntegrationPointField`; `max |u - alpha dT (x - x_A)| / (alpha dT L_max)` | `1e-10`; `1e-10` | arithmetic 1: `1e-15`; patch conditioning `8e-14` at `nu = 0.4999` (ADR 0493 PT2) |
| TH2 | a1, a2 | the same patch, material O (full thermal strain tensor in the rotated axes), uniform `dT = 60`, free, both: `max |sigma| / s_0`, `s_0 = 1.4e11 * 4e-5 * 60` | `1e-10` | arithmetic 1: `1e-15`, ratio 14 of the moduli: `< 1e-12` |
| TH3 | a1, a2 | the same patch, a tabulated steel (`Material::tabulated(steel(0.3), [200, 600], e = [2.0e11, 1.8e11], nu = [0.3, 0.3], expansion = [1.2e-5, 1.5e-5])`), uniform `T = 353`, free, both: `max |sigma| / (E_0 alpha_0 dT)` | `1e-10` | as TH1 |
| TH4a | a1 | the clamped patch (`0.25 h`, tag 1001, every boundary component fixed), `nu = 0.3`, a non-uniform `T_b = 293 + 80 U_b` (SplitMix64 seed `0xF588 + 7`), both: per element `max |sum_q w_q B_q^T sigma_q - (K_e u_e - f_e)| / max(|K_e u_e|, |f_e|)` with the test's own `B_q` (BBar: the matrices from the gradients; Incompatible: the compatible `B_q`), `K_e = element_stiffness`, `f_e = element_thermal_load` with the test's `eps_th,q`, `sigma_q` the field's stress | `1e-9` | RS3 of ADR 0493: `3e-12`; the identity is algebraic |
| TH4b | a1 | the same model, Incompatible: per element `|sum_q w_q G_q^T sigma_q| / sum_q w_q |G_q|^T |sigma_q|` with the test's own `G_q` (ADR 0493 `test_modes`) | `1e-9` | RS3: `6.7e-13` |
| PO1 | a2 | the patch (`0.1 h` tag 11 and `0.25 h` tag 1001), material O (no temperature), the boundary prescribed with the linear field of ADR 0493, both: interior error (relative to the largest field component) | `1e-10` (the acceptance) | arithmetic 6: `< 1e-12` |
| PO2 | a2 | the same: the recovered stress at every point against `D_O eps` (`D_O = elasticity_at(default context)`), relative to `max |D_O eps|`, both | `1e-10` | arithmetic 6 |
| PO3 | a2 | the distorted element of ADR 0493 (`distorted_element`): `element_stiffness_at(f, coords, [D; 8])` equals `element_stiffness(f, coords, D)` bit for bit, both; with `D_q = D_O (1 + 0.05 q)` (`q = 0..7`): `max |K - K^T| / max |K|`; the six rigid motions `max |K r| / (max |K| max |r|)` | exact; `1e-13`; `1e-11` | EV3 of ADR 0493: `3.7e-16`, `6.4e-16` |
| PP1a | a2 | one distorted Hex8, material O with `Cylindrical { origin: [-0.4, -0.3, 0], axis: [0, 0, 1] }` axes (the matrix differs from point to point), supports `A` (node 0: three), `B` (node 1: `u_y`, `u_z`), `C` (node 3: `u_z`), nodal forces `[1e3, -2e3, 3e3]` at node 6 and `[-1e3, 5e2, 8e2]` at node 7: `solve_static_with` against the test's dense reduction of `element_stiffness_at` with the test's `D_q` (from `MaterialPoints::context`, `elasticity_at`); `max |u - u_ref| / max |u_ref|`; and the field's strains of the element equal `point_strains_at(f, coords, D_q, None, u_e)` bit for bit; both | `1e-9`; exact | a single element, condition of the reduced `K` below `1e4`: `1e-12` |
| PP1b | a1, a2 | the same element, a tabulated steel as TH3 with the non-uniform nodal temperatures `[300, 320, 350, 310, 340, 380, 360, 330]`, no mechanical load: `solve_static_with` against the dense reduction of `K_e` (with `D_q = elasticity_at(context at the point)`) and `f_e = element_thermal_load` (with `eps_th,q = mat_ortho::thermal_strain`); error as PP1a; the field's strains equal `point_strains_at(f, coords, D_q, eps_th, u_e)` bit for bit; both | `1e-9`; exact | as PP1a |
| LIN1 | a7 | the distorted element, `D_1` steel `nu = 0.3`, `D_2` orthotropic identity axes (arithmetic 4): `max |K(D_1 + D_2) - K(D_1) - K(D_2)| / max |K|` for `BBar` | `1e-13` | arithmetic 4: `1.7e-16` |
| N-LIN | a7 | the same for `Incompatible` (the control: the condensed stiffness is not additive) | `> 1e-3` | arithmetic 4: `1.03e-2` |
| GB1 | a3 | the strip, Incompatible, `nu = 0`: `|lambda P / P_cr - 1|` | `0.01` | arithmetic 5: `5.1e-4` |
| GB2 | a3 | the same, `nu = 0.3` | `0.02` | arithmetic 5: `3.1e-3` |
| N-GB | a3 | the same mesh, default Full Hex8, `nu = 0` and `0.3`: `lambda P / P_cr` (the shear-locking control) | `> 1.3` | arithmetic 5: `1.498`, `1.542` |
| O-GB | a3 | the same, BBar, printed | observation | arithmetic 5: `1.074`, `0.826` |
| SA1 | a6 | the distorted element with a fixed pseudo-random `u` (SplitMix64 seed `0xF588 + 1`, `|u_i| <= 1e-3`): `strain_at(f, coords, D, u, xi_q)` at the eight points of the rule against `point_strains(f, coords, D, u)[q]`, relative to the largest strain, both | `1e-12` | the same algebra with a different loop order: rounding `1e-15` |
| ES1 | a6 | the patch of ADR 0493 PT2 (`0.25 h`, tag 1001, `nu = 0.3`) with the linear field's boundary solution, both: `patch recover` nodal stresses at the interior nodes against `D eps` relative to `max |D eps|`; `energy_error` estimate / solution norm | `1e-10`; `1e-9` | the strain at every sampling and estimate point is the field's strain to `5e-16` |
| ES2 | a6 | pure bending of the `8 x 2 x 2` box (`2 x 0.1 x 0.1`, `nu = 0.3`, root face clamped in `u_x`, `u_z` and `u_y` at a corner as ADR 0493's cantilever, the end couple `M = 10` as RS2), Incompatible: `recover` nodal stress at the interior nodes against `(M z'/I, 0, ...)` relative to `M h/(2 I)`; `energy_error` estimate / solution norm | `1e-8`; `1e-8` | RS2: `2.7e-12`; the exact stress is linear in `z` and the linear fit reproduces it |
| DM1 | a4 | the strip cantilever `10 x 1 x 1` (`2 x 0.1 x 0.1`, `nu = 0.3`, root face fixed), two element sets of five elements and the same steel, `loss_factor(a, Constant(0.02))` and `(b, ..)`, four modes with the modal tolerance `1e-12`: per mode `|eta_mode / 0.02 - 1|` with `eta_mode = Im(lambda^2) / Re(lambda^2)`, and `|(-Re lambda^2) / omega_modal^2 - 1|`, both | `1e-6`; `1e-6` | the loss stiffness is `0.02 K`, the projected problem diagonal: `1e-10` |
| DM2a | a4 | the same mesh, one set, BBar, a viscoelastic steel with one Prony term `(0.2, 0.2, tau = 1e-2)` (equal weights), `viscoelastic_reference(100, None)`: per mode `|eta_mode / (Im c / Re c) - 1|` and `|(-Re lambda^2) / (omega_0^2 Re c) - 1|`, `c = G*(100) / G_0` from `mat_visco::complex_moduli`, `omega_0` of `modal_analysis` of the model with the elastic base | `1e-6`; `1e-6` | `D*(w) = c D_0`, so `K* = c K_0` exactly: `1e-10` |
| DM2b | a4 | the same with `own_frequency` (`OwnFrequency::new(8)`): per mode `|lambda^2 / (-omega_0^2 c(omega_own)) - 1|` with `omega_own` of the record | `1e-6` | the frozen problem at `omega_own` is diagonal with `K^* = c K_0`: `1e-10` |
| DM2c | a4 | the same with unequal weights `(0.03, 0.01, 1e-2)` (the split): per mode `eta_mode` within `[min(G''/G', K''/K'), max(...)]` and `(-Re lambda^2) / omega_0^2` within `[min(G'/G_0, K'/K_0), max(...)]`, each with `1e-9` slack | holds | Rayleigh quotient between the extremes of `D''` and `D'` |
| DH1 | a5 | the DM1 model, `DirectHarmonicOptions::new().loss_factor(a, 0.02).loss_factor(b, 0.02)`, the tip traction of `tip_shear` as load, `omega = 0.1` rad/s, both: `max |u_h (1 + 0.02 i) - u_s| / max |u_s|` with `u_s = solve_static` | `1e-6` | the inertia term `(omega/omega_1)^2 = 3e-8` |
| DH2 | a5 | the same: the response stress `DirectHarmonic::stress` real part against the static `IntegrationPointField` stress, and the imaginary part, relative to `max |sigma_s|`, both | `1e-6`; `1e-6` | as DH1 |
| DH3a | a5 | BBar, the viscoelastic steel with equal weights `(0.2, 0.2, 1.0)`, `omega = 1e-6`: `max |u_h - u_s| / max |u_s|` with `u_s` of the elastic steel with the relaxed moduli `G_inf = 0.8 G_0`, `K_inf = 0.8 K_0` | `1e-5` | `Im G* / G_0 = w omega tau = 2e-7` |
| DH3b | a5 | the same with unequal weights `(0.3, 0.1, 1.0)` (the split), relaxed `G_inf = 0.7 G_0`, `K_inf = 0.9 K_0` | `1e-5` | `3e-7` |
| DH3c | a5 | DH3b: the response stress against the static field stress of the relaxed model, relative to `max |sigma_s|` | `1e-5` | `3e-7` |
| RF1 | D2 | the formulations on Tet4, Tet10, Hex20 refused as before; `Full` on Tet4 taken (the copy of ADR 0493 RF1) | exact | unchanged |
| RF2 | D2, a7 | with an `Incompatible` block and a viscoelastic material: `damped_modes` (with `viscoelastic_reference`) and `DirectHarmonic::new` refused (`NotYetSupported`, item "formulation", variant `Incompatible`, package `FOLLOW_UP`); with `BBar` both taken | exact | D2 |
| RF3 | D2 | the paths lifted are taken: with each formulation `solve_static` with a temperature load, `nodal_load_vector`, `IntegrationPointField::compute`, the orthotropic `solve_static`, `geometric_stiffness`, `damped_modes` (Rayleigh), `DirectHarmonic::new`, `estimate::recover` return `Ok`; `nonlinear::internal_force` and `tangent` as in the f587 copy (BBar `Ok`, Incompatible refused with key `"nonlinear"`) | exact | D2 |
| B1 | a8 | the digests of `si_hex8x_linear_bits` equal the base run | exact | D3 |
| B2 | a8 | `cargo test -p rcfd-fem --release`: every test passes (ignored records excepted), including `si_hex8x_bits`, the `sa_damped_*_default_bits`, the `sa_harmonic_visco*_default_bits`; no existing test edited but the one attribute of D4 | pass | D3 |
| B3 | a8 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | no fluid file touched |

### Negative controls

N-LIN and N-GB above. In addition: N-TH, TH1 with `Full` (the default path of the same harness) is stress-free to the same limit
(the harness is not the cause of a miss); estimate `1e-13`. N-ES, the recovered stress of ES2 for the default `Full` Hex8 of the
same mesh printed (the parasitic shear): observation.

## Planned mutations (on a scratch copy with its own target, one at a time)

| id | mutation | expected to be caught by |
| --- | --- | --- |
| MU1 | `element_thermal_load` of Incompatible drops the condensation of `f_a` (`f = f_u`) | TH1, TH2, TH3, TH4a, PP1b |
| MU2 | `point_strains_at` computes `alpha` without `f_a` (`alpha = -K_aa^{-1} K_au u`) | TH1, TH2, TH3, TH4b, PP1b |
| MU3 | the BBar thermal load takes `B` in place of `Bbar` | TH4a (arithmetic 2: not TH1 to TH3) |
| MU4 | the assembly takes `D` of the first point at all points | PP1a, PP1b |
| MU5 | `compute_with` passes `[D; 8]` and no thermal strain to `point_strains_at` | PP1a, PP1b, TH1 |
| MU6 | an opted element takes the Full thermal load (the `thermal.rs` dispatch dropped) | TH1, TH4a |
| MU7 | `assemble_geometric` returns a zero matrix for an opted block | GB1, GB2 |
| MU8 | the loss closure of `damped_modes` calls `element_for(..).stiffness` | DM1, DM2a |
| MU9 | the own-frequency parts of `damped_modes` call `element_for(..).stiffness` | DM2b |
| MU10 | the multi-set closure of `DirectHarmonic::new` calls `element_for(..).stiffness` | DH1, DH2 |
| MU11 | the viscoelastic-parts closure of `harmonic_direct` calls `element_for(..).stiffness` | DH3a, DH3b |
| MU12 | `estimate::element_samples` uses `element.strain_at` (the compatible strain) | ES2 |
| MU13 | `hex8x::strain_at` of BBar without the mean-dilatation correction | SA1 |
| MU14 | `refuse_incompatible_viscoelastic` returns `Ok` | RF2 |
| MU15 | a default (Full) element takes the BBar branch of the assembly | B1, B2 |

## Decision rule

- **done:** TH1 to TH4b, PO1 to PO3, PP1a, PP1b, LIN1, N-LIN, GB1, GB2, N-GB, SA1, ES1, ES2, DM1, DM2a to DM2c, DH1, DH2, DH3a to
  DH3c, RF1 to RF3, B1 to B3 met; the negative controls as stated; every mutation caught by a registered check; no amendment
  after a run.
- **partial:** a check missed (recorded as a `*_known_miss`) or not run, a mutation not caught, or a check met only after an
  amendment written after its run.
- **todo:** the paths do not build.

## Limits (not covered)

- L1. Linear elasticity only; the nonlinear driver is as ADR 0506 left it (BBar taken as F-bar, Incompatible refused). The
  quasi-static and nonlinear transient temperature histories (F-592) are not touched.
- L2. `Incompatible` with a viscoelastic material is refused on the damped modes and the direct harmonic solve entirely (the split
  is not linear in `D`; the equal-weight case, where the stiffness is a complex multiple of one real stiffness, is not checked).
- L3. The geometric stiffness of both formulations takes the compatible gradient with the stress of the formulation; the gradient
  of the internal modes (the consistent second variation of the enhanced element) is not included. The Euler load of the strip
  is checked (GB1, GB2) and the pre-stressed modes, the rotating stress stiffening and the cyclic paths use the same matrix
  without a check of their own. The BBar element keeps the shear locking in bending (O-GB: `1.074`, `0.826`); it is for nearly
  incompressible solids, not for thin bending.
- L4. The patch recovery covers a constant elasticity and no temperature field (ADR 0478): the per-point materials and a
  temperature there stay refused by their own existing messages.
- L5. A viscoelastic `BBar` is checked in the damped modes (equal weights exactly, unequal weights by bounds) and the direct solve
  (the static limit); no closed form of a distorted viscoelastic element is used.
- L6. The thermal load of `Incompatible` uses the interpolated temperature at the points; a temperature varying within the element
  is represented by its trilinear interpolation only, like every Hex8 thermal load.

## Open questions (no case-file key is added; the questions that would change a default are listed, not taken)

1. Is the Incompatible geometric stiffness worth the consistent enhanced-gradient term (L3)? Recommendation: only if a pre-stressed
   or buckling model on a one-layer strip needs better than the registered `1e-2`; the registered result is `5e-4`.
2. `Incompatible` with equal-weight viscoelastic materials (L2): recommendation: leave refused until a model needs it.

## Amendment 1 (2026-11-14, written and committed before the first run of ES2 and of any other check of the file)

Found while writing `tests/si_hex8x_linear.rs`, before any run of it. The ES2 row says the root face of the `8 x 2 x 2` box is
"clamped in `u_x`, `u_z` and `u_y` at a corner as ADR 0493's cantilever". That support is exact for the one-element-thick strip of
RS2 because the exact field of pure bending has `u_z(0, y, z) = -nu M (z'^2 - y'^2) / (2 E I) = 0` at the four corner nodes of
the face (`|y'| = |z'| = h/2`) and `u_y = 0` there too; with `2 x 2` elements the root face has edge-mid and centre nodes where
the exact `u_z` is not zero, so `u_z = 0` on the whole face would add a Saint-Venant disturbance of the order `nu` that decays
over about `h`, i.e. beyond `1e-8` at the interior nodes of elements of length `2.5 h`. The registered set-up is therefore
defective for this mesh (the limit `1e-8` was derived from RS2, where the field is exact). Corrected set-up, the limits unchanged
(`1e-8` for the recovered stress and `1e-8` for the estimate over the solution norm): the same mesh `8 x 2 x 2`, the same end
couple `M = 10` applied as the consistent nodal forces of the linear stress `sigma_xx = M (z - h/2) / I` on the tip face (the
force of the node `(j, k)` is the product of the 1D integrals `int N_j dy` and `int N_k sigma dz`), and the supports compatible
with the exact field: `u_x = 0` on the whole root face, `u_y = u_z = 0` at the centre node `(0, h/2, h/2)` and `u_y = 0` at
`(0, h/2, h)`, where the exact `u_y = -nu M y' z' / (E I)` and `u_z` vanish. Interior nodes are those off the six faces of the
box. No other check is changed. The negative control N-ES prints the same quantities for `Full`.

## Amendment 2 (2026-11-14, written and committed before the corrected run of RF3)

Run 1 of `si_hex8x_linear.rs` (log `f588-scratch/run1.log`) met every row but one: RF3 `estimate::recover` on the cantilever strip
`20 x 1 x 1` (the model of the f587 copy) returned `InvalidValue { key: "patch recovery", reason: "no patch can be solved
(ADR 0478 D3): 0 interior corner nodes, 0 short patches" }` for both formulations, an error that is not a formulation refusal:
the strip has no interior node, so the registered set-up cannot show that the path is taken. The refusals of ADR 0493 that the
row asserts lifted are not what failed (the message does not name a formulation), but the row as set up proves nothing, so it is
a defective set-up and is corrected, not counted as met. The old test stays as an ignored record (`rf1_rf2_rf3_refusals_after_f588`,
28 rows met, this row missed) and the corrected test `rf1_rf2_rf3_refusals_after_f588_amended` runs the same rows with the one
change that `estimate::recover` is called on a box `1 x 0.5 x 0.5` of `4 x 2 x 2` elements with the formulation and a zero
displacement (the sole row changed; the criterion is "returns Ok", unchanged).

## Amendment 3 (2026-11-14, written and committed before the first run of ES3)

Mutation MU12 (`estimate::element_samples` uses the compatible `element.strain_at` for an opted element) ran on the scratch copy
(`f588-scratch/mutations.log`) and was NOT caught by any registered check: `failed []`. Diagnosis from run 1: the negative control
N-ES (the same ES2 box with the default `Full` element) gives a recovered nodal stress error of `6.4e-13`, i.e. on the uniform box
the compatible strain recovers the linear bending stress as well, because the parasitic shear `k (x - x_c)` of the compatible
element at the 2 x 2 x 2 points is antisymmetric within each element and cancels in the least-squares fit over the symmetric patch
(the energy estimate does see it, `0.84`, but it goes through `energy_error`'s own strain call, not `element_samples`). The
registered ES2 therefore cannot discriminate MU12; this is a miss of the mutation, recorded as such (the decision rule gives
`partial`), and one added check is registered here before it is run: ES3, the ES2 box with the nodes mapped `x -> L (x / L)^1.5`
(axis-aligned elements of unequal length, so the incompatible element still represents bending exactly and the stress is still
`M z' / I`), same loads and supports. Limits: ES3a Incompatible recovered nodal stress `<= 1e-8` and `energy_error` estimate over
solution norm `<= 1e-8` (as ES2); ES3b control, `Full` on the same box, recovered nodal stress error `> 1e-4` (the control must
show that the compatible strain is not enough there; if it does not exceed, ES3 does not discriminate MU12 and MU12 stays
uncaught). MU12 is re-run against `si_hex8x_linear` with ES3 present; the other mutations are not re-run (their catches stand).

## Addendum 1: results

Run 1 (registered run, log `f588-scratch/run1.log`) and the amended re-runs, 2026-11-14. All limits are those registered above
and in Amendments 1 to 3. Every check met except where marked.

| check | limit | measured | met |
| --- | --- | --- | --- |
| N-TH (Full, stress / displacement) | 1e-13 | 1.7e-14 / 1.4e-14 | yes |
| TH1 BBar nu 0.3 | stress-free 1e-10 | 1.3e-14 | yes |
| TH1 BBar nu 0.4999 (stress / displacement) | 1e-10 | 2.3e-11 / 3.9e-11 | yes |
| TH1 Incompatible (nu 0.3; nu 0.4999 stress / displacement) | 1e-10 | 2.2e-14; 7.4e-11 / 7.4e-12 | yes |
| TH2 BBar / Incompatible | 1e-10 | 3.5e-15 / 7.3e-16 | yes |
| TH3 (two values) | 1e-10 | 7.3e-15, 7.0e-15 | yes |
| TH4a (BBar / Incompatible), TH4b | registered | 6.2e-16, 6.3e-16; 3.3e-16 | yes |
| PO1, PO2 | 1e-10 | at most 1.7e-14 | yes |
| PO3 | bit-equal, symmetry, rigid motions | bit-equal; 1.4e-16; 2.8e-16 | yes |
| PP1a, PP1b (u error, four values) | registered | 1.3e-14, 4.1e-15, 1.4e-15, 3.5e-15; field strains bit-equal | yes |
| LIN1 BBar | registered | 1.6e-16 | yes |
| N-LIN Incompatible (control, > 1e-3) | > 1e-3 | 7.6e-3 | yes |
| GB1 (Euler strip, Incompatible) | 1e-2 | 5.13e-4 | yes |
| GB2 | registered | 3.06e-3 | yes |
| N-GB Full (control) | outside | 1.498, 1.542 | yes |
| O-GB BBar (observation) | none | 1.074, 0.826 | observation |
| SA1 | bit equality | 0 | yes |
| ES1 | registered | 1.0e-15, 1.4e-15 | yes |
| ES2 (after Amendment 1) stress / estimate | 1e-8 | 1.76e-12 / 6.8e-12 | yes |
| N-ES Full (control) | outside | stress 6.4e-13, estimate / norm 0.84 | see below |
| DM1 | registered | about 1e-12 | yes |
| DM2a, DM2b | registered | about 3e-12, 1.0e-11 | yes |
| DM2c (bounds) | within bounds | mode at the shear bound; eta 1.5228e-2, ratio 0.985 | yes |
| DH1 (BBar / Incompatible) | registered | 3.0e-7 / 5.9e-7 | yes |
| DH2 real / imaginary | registered | 3.5e-7, 6.7e-7 / 6.9e-9, 1.3e-8 | yes |
| DH3a, DH3b, DH3c | registered | 2.5e-7, 4.3e-7, 7.4e-12 | yes |
| RF1, RF2 | message names variant and follow-up | met | yes |
| RF3 (estimate row) | refusal | first set-up defective; amended (Amendment 2), then met | met after amendment |
| ES3a (graded 1.5 power mesh) | 1e-8 | 8.2e-13 stress, 4.5e-12 estimate | yes |
| ES3b control (Full, > 1e-4) | > 1e-4 | 3.45e-13 | MISSED |
| B1 (`si_hex8x_linear_bits`, 18 digests), `si_hex8x`, `si_fbar` | bit-identical | pass | yes |

N-ES: the Full box recovers the stress to 6.4e-13 as well, so the compatible element does not show the parasitic shear in the
least-squares nodal stress on this box; ES3b missed for the same reason (3.45e-13 against `> 1e-4`). Both are recorded:
ES3b as the ignored test `es3_..._known_miss` with its limit intact.

### Deviations

- Amendment 1 (ES2 supports), Amendment 2 (RF3 on a box with interior nodes, the strip has none), Amendment 3 (ES3 added). Each
  was written and committed before the run it concerns. The RF3 row was run once with the defective set-up (missed), then
  amended; under the decision rule that is a check met only after an amendment written after its run.
- The expected-catch lists of the planned mutations were partly wrong (MU1, MU6, MU10, MU11 map to other checks); every
  mutation but MU12 is caught by some registered check.
- `rf1_rf2_rf3_refusals_after_f587` in `si_fbar.rs` is ignored as a record (ADR 0493 D5 and ADR 0506 refusals lifted here), and
  the first RF3 test is ignored as a record of the defective set-up.

### Mutations (scratch copy, own target, `f588-scratch/mutations.log`)

| id | caught by |
| --- | --- |
| MU1 | TH4a |
| MU2 | TH4a |
| MU3 | TH4a |
| MU4 | PP1 |
| MU5 | PP1, TH4a |
| MU6 | PP1 |
| MU7 | GB1, GB2 |
| MU8 | DM1 |
| MU9 | DM2 |
| MU10 | DH3 |
| MU11 | DH1, DH2 |
| MU12 | NOT CAUGHT (no check failed, including ES3) |
| MU13 | SA1 |
| MU14 | RF |
| MU15 | B1 (`si_hex8x_linear_bits` and `si_hex8x_bits`) |

MU12 (the estimate sampling with the compatible strain for an opted element) is not caught: on the axis-aligned bending boxes
the compatible strain recovers the same nodal stress, because the parasitic shear cancels in the symmetric least-squares fit.
That is a gap in the registered checks, not a defect found in the code.

### Not run

The whole workspace suite (the lead runs the gate). No baseline or golden file was touched.

### Decision (by the rule above): partial

Reasons: MU12 not caught; ES3b missed (known miss); the RF3 row met only after Amendment 2. Everything else in the acceptance is
met: thermal expansion stress-free (1.3e-14 to 7.4e-11 against 1e-10), orthotropic patch, Euler buckling within registered limits,
damped modes and direct harmonic with the formulations' stiffness, Incompatible viscoelastic refused, patch recovery with the
formulations' strain, defaults bit-identical.

### Open questions (recommendations, not taken)

1. A check that discriminates `element_samples`: a non-equilibrium quadratic displacement field on a distorted element, where the
   compatible and the formulation strain differ at the points. Recommendation: add it (small, one test); then MU12 is caught and
   the item can close.
2. L2, equal-weight viscoelastic `Incompatible`: leave refused until a model needs it.
3. L3, the consistent enhanced-gradient term in the geometric stiffness: not needed while GB1 holds at 5e-4.

## For the lead: roadmap entries

> F-588 PARTIAL. ADR 0525, 2026-11-14: the refusals of ADR 0493 D5 lifted in rcfd_fem::hex8x (element_stiffness_at, element_thermal_load, internal_modes_at, point_strains_at, strain_at) and its callers: free thermal expansion of the distorted F-374 patch stress-free to 1e-10 for BBar and Incompatible (7.4e-11 worst at nu 0.4999), orthotropic and per-point patches to 1.7e-14, Euler buckling of a one-layer Incompatible strip within 5.1e-4 (Full control 1.5), damped modes and direct harmonic solve with the formulations' stiffness (loss factors to 1.0e-11; viscoelastic BBar by bounds and the static limit), Incompatible with a viscoelastic material refused, patch recovery with the formulations' strain (ES2 1.8e-12). Defaults bit-identical (18 new digests, baseline_hashes unchanged). Missed: ES3b control (Full also recovers the bending box, 3.45e-13) and mutation MU12 (estimate sampling with the compatible strain) not caught by any check; RF3 set-up amended once (Amendment 2). Follow-ups (numbers left to the lead): a discriminating test of the estimate sampling; equal-weight viscoelastic Incompatible, only if a model needs it.

> Proposed follow-up (structural, low, number to be assigned by the lead): a non-equilibrium quadratic displacement test for `estimate::element_samples` with an opted Hex8; closes F-588.
