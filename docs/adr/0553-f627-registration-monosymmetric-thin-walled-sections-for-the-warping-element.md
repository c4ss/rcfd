# ADR 0553: F-627 registration: monosymmetric thin-walled sections for the warping element (shear-centre offset, the monosymmetry coefficient beta_x, the bimoment load)

Status: **REGISTERED 2026-11-15, written before any code of F-627 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-627 was written, compiled or run in the repository: the files listed under "What was
read" were read, and the arithmetic below was done by hand and evaluated with one plain numpy script in the scratch folder
(`w49i-work/f627-scratch/arith.py`, log `arith.log`: the section constants, the closed forms, and a cubic-Hermite Euler-Bernoulli
Vlasov model in shear-centre form of the lateral-torsional, the flexural-torsional column, the coupled vibration, the static
lateral-force and the bimoment problems, to estimate the discretisation error; no rcfd code; disclosed, its numbers quoted
below). Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-627 (deps F-598), structural solver, advanced tier, the follow-up named by
[ADR 0546](0546-f598-registration-warping-degree-of-freedom-for-beam2-and-i-section-lateral-torsional-buckling.md) (its question
Q3) and by [ADR 0548](0548-f626-registration-warping-unknown-on-the-static-solve-modes-and-pre-stressed-modes.md) (its question
Q3: the bimoment load and the bimoment pre-stress `beta_omega`). Number 0553 given by the lead. Branch `f627-work` from 89bdd903,
worktree `w49i-work/f627`, scratch `w49i-work/f627-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test edited;
no new dependency, no unsafe code, the crate graph unchanged; clean-room. Every choice below that the maintainer would normally
be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation), except the open
questions at the end.

## What F-627 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. The lateral-torsional buckling moment of a simply
> supported monosymmetric I under uniform moment equals the published closed form with beta_x within 2 percent at 16 elements;
> doubly symmetric results unchanged.

The roadmap note adds (from ADR 0548): the bimoment load and the bimoment pre-stress (`beta_omega`) belong to this item. The
brief adds: cite the closed form, mark citations from memory as such, derive `beta_x` and the shear-centre offset of the section.

Clauses: **a0** every limit registered here; **a1** the LTB moment of a simply supported monosymmetric I under uniform moment
within `2e-2` of the closed form with `beta_x` at 16 elements (both signs of the moment: the closed form has two roots, the
larger flange in compression and in tension); **a2** doubly symmetric results unchanged (bit for bit); **b1** the bimoment load;
**b2** the bimoment pre-stress (D6: zero for every section with one axis of symmetry, so the term is not added; registered with
its derivation).

### Readings, registered

- **"Monosymmetric I"**: two flanges of different size on one web, symmetric about the web axis (the section axis `z`, `e_3`),
  the flanges along the section axis `y` as in ADR 0546 D1. Flange 1 `b_1 x t_1` on the `+e_3` side, flange 2 `b_2 x t_2` on the
  `-e_3` side, web `h_w x t_w` between their inner faces. The section of the checks: `b_1 = 0.15`, `t_1 = 0.012`, `b_2 = 0.08`,
  `t_2 = 0.008`, `h_w = 0.2`, `t_w = 0.006` (flange 1 has `I_1/(I_1 + I_2) = 0.908` of the flange lateral inertia), steel `E =
  2e11`, `nu = 0.3`, `rho = 7850`, shear factor `5/6` both ways.
- **"Simply supported", "uniform moment"**: as ADR 0546 (fork supports `v = w = phi = 0` at both ends, `u = 0` at node 0,
  warping free; equal and opposite end moments `M_ref = 1000` about `e_2`), `L = 3`, 16 elements along `x`.
- **"The published closed form with beta_x"**: D3 (Trahair; Timoshenko and Gere; from memory, derived here from the energy).
- **"Doubly symmetric results unchanged"**: every path of a `WarpingSection` without a shear-centre offset (the constructors
  `new`, `saint_venant`, `doubly_symmetric_i`) runs the arithmetic of ADR 0546 and ADR 0548 operation for operation: the bit
  records of ADR 0508, 0546 and 0548 are unchanged (B0), and the test files of ADR 0546 and 0548 pass unchanged.
- **"Opt-in"**: the shear-centre offset is a new field of `WarpingSection`, zero unless set by `with_shear_centre` or by the new
  constructor `monosymmetric_i`; the bimoment load is a new `Warping::bimoment`. No case-file key (F-625).

## What was read (no run)

- `AGENT_RULES.md`; the brief; ADR 0546 (whole), ADR 0548 (whole); ADR 0508 D1 (the second-order energy, the convention of the
  Wagner coefficients) and D3 (end forces).
- `crates/rcfd-fem/src/beam_warping.rs` (whole), `beam.rs` (`local_mass`, `local_geometric_stiffness`, `second_order_rows`,
  `end_resultants`, `local_end_forces`), `geometric.rs` (`with_beam_end_forces`, `beam_terms`).
- Tests: `sa_beam_warping_f598.rs` (the set-ups, the `Checks` helper), `sa_beam_warping_f626.rs` (the set-ups).

## Design

### D1. Kinematics of the shear-centre offset (first principles)

Section axes `y`, `z` centroidal and principal; the shear centre `S` at `(y_0, z_0)` from the centroid `C`. The section rotates
by `phi` about the axis through `S` (Vlasov; Timoshenko and Gere ch. 5, Trahair ch. 2, from memory, not read): a point `P` at
`(y, z)` moves laterally by `v_P = v_s - (z - z_0) phi`, `w_P = w_s + (y - y_0) phi`, `(v_s, w_s)` the displacement of `S`. At the
centroid: `v_c = v_s + z_0 phi`, `w_c = w_s - y_0 phi`. The axial displacement is `u_P = u_c - y v_s' - z w_s' - omega_S phi'`
with `omega_S` the principal sectorial coordinate about `S` (`int omega_S dA = int y omega_S dA = int z omega_S dA = 0`; the last
two define `S`). Then the normal-strain energy is `(1/2) int (E A u_c'^2 + E I_z v_s''^2 + E I_y w_s''^2 + E C_w phi''^2) dx`:
**uncoupled in the shear-centre displacements** (with the centroid's, the bending and warping terms couple through `int y
omega_C dA != 0`). The transverse shear force acts through `S`, so the Timoshenko shear strains are those of `S`. Hence:

**The element of a section with an offset is the ADR 0546 element in shear-centre unknowns, transformed to the centroid
unknowns of the model's nodes**: `u_s = O u_c`, `O` the 14 x 14 identity with, at each node, `O[v][phi] = -z_0`, `O[w][phi] =
+y_0` (rows 1, 2 and 8, 9; columns 3 and 10). `K_c = O^T K_s O`, `K_s` = `local_stiffness(length, section, material, c_w)` of
ADR 0546. The nodes stay at the centroid (the axial force and nodal loads act there, as everywhere in the library). A transverse
nodal force at the centroid thereby carries the torque `M_S = M_C + z_0 F_y - y_0 F_z` about `S` (the moment of `F` about `S`).

**Kinetic energy**: `(1/2) int int rho (u_P_dot^2 + v_P_dot^2 + w_P_dot^2) dA dx` with `int y dA = int z dA = 0` is `(1/2) int
(rho A (u_c_dot^2 + v_c_dot^2 + w_c_dot^2) + rho I_p phi_dot^2) dx` (rotary terms as before), `I_p = I_y + I_z` about the
centroid: **the mass is exact in the centroid unknowns** and is not transformed (the ADR 0548 `local_mass` unchanged).

### D2. Second-order energy with the offset (first principles; the convention of ADR 0508 D1)

The second-order strain of a fibre is `(1/2)(v_P'^2 + w_P'^2)`; its work with `sigma = N/A + M_y z/I_y - M_z y/I_z` (ADR 0508's
convention: `M_y = int z sigma dA`, `M_z = -int y sigma dA`; the internal resultants of `end_resultants`) is, with `v_P' = v_c' -
z phi'`, `w_P' = w_c' + y phi'` and the centroidal integrals:

- `N`: `(N/2)(v_c'^2 + w_c'^2 + (I_p/A) phi'^2)` (the ADR 0473 terms with Wagner's `N I_p/A`, **in centroid unknowns**; in
  shear-centre unknowns it is `(N/2)((v_s' + z_0 phi')^2 + (w_s' - y_0 phi')^2 + (I_p/A) phi'^2)`, the classical `r_0^2 = I_p/A +
  y_0^2 + z_0^2` with the coupling `N z_0 v_s' phi'`).
- `M_y`: `-M_y v_c' phi' + (M_y/2) beta_y^C phi'^2`, `beta_y^C = int z (y^2 + z^2) dA/I_y` (the ADR 0546 coefficient). With
  `v_c' = v_s' + z_0 phi'`: `-M_y v_s' phi' + (M_y/2)(beta_y^C - 2 z_0) phi'^2`.
- `M_z`: `-M_z w_c' phi' - (M_z/2) beta_z^C phi'^2`, and with `w_c' = w_s' - y_0 phi'`: `-M_z w_s' phi' - (M_z/2)(beta_z^C - 2
  y_0) phi'^2`.

The moment terms of ADR 0508 D1 are written with the section rotations `theta` (the same for every point of the section), and
the Timoshenko shear strain is that of `S` (D1), so `theta_z -> v_s'`, `theta_y -> -w_s'` as the shear strain vanishes: they are
the moment terms in shear-centre unknowns, and the Wagner coefficients that go with them are **the coefficients about the shear
centre**: `beta_y^S = beta_y^C - 2 z_0`, `beta_z^S = beta_z^C - 2 y_0`. This is Trahair's monosymmetry coefficient `beta_x =
(1/I_x) int y (x^2 + y^2) dA - 2 y_0` (his axes: `x` major, `y` minor; from memory), in the axes here `beta_x = beta_y^S`.

Hence, with `G(N, ends, beta)` the ADR 0546 `local_geometric_stiffness` and `f_s` the end forces in shear-centre form (D4):
`K_sigma,c = G(N, None, [0, 0]) + O^T G(0, Some(f_s), beta^S) O`. The first term is the axial-force part in centroid unknowns
(it does not read `beta`); the second the moment, torque, shear-force and Wagner part in shear-centre unknowns. The shear-force
terms are taken in shear-centre form with the moment terms; no check below has a shear force in the pre-stress (Limits).

### D3. The closed form (Trahair, Flexural-Torsional Buckling of Structures, 1993, ch. 6; Timoshenko and Gere, Theory of Elastic Stability, 1961, ch. 6; from memory, derived)

Forks, warping free, uniform `M` (`M = M_y`): `v_s = a sin(k x)`, `phi = b sin(k x)`, `k = pi/L`, satisfy `v_s = v_s'' = phi =
phi'' = 0` at the ends. The energy of D1 and D2 per `L/4`: `P_y a^2 + (G J + E C_w k^2 + M beta_x) b^2 - 2 M a b` (times `k^2`),
`P_y = E I_z k^2`. Its determinant vanishes at `M^2 - beta_x P_y M - P_y (G J + E C_w k^2) = 0`:

`M = (beta_x P_y/2) +/- sqrt((beta_x P_y/2)^2 + P_y (G J + pi^2 E C_w/L^2))`,

the form `M_yz = (P_y beta_x/2)[1 +/- sqrt(1 + 4 (G J + pi^2 E C_w/L^2)/(P_y beta_x^2))]` of Trahair (from memory). For `beta_x =
0` it is the ADR 0546 closed form. The two roots are the two signs of the moment; the larger magnitude belongs to the larger
flange in compression (Trahair, from memory; here a consequence of the signs: in the convention of D2 `beta_x < 0` for the
larger flange on `+z`, and `M > 0` puts `+z` in tension). The mode: `a/b = M/P_y` (shear centre), so the centroid's `v_c/phi =
M/P_y + z_0`.

### D4. End forces and the refusal RF8 with an offset

The `Prestress` end forces stay those of the centroid unknowns (the work-conjugates of the model's unknowns, as everywhere: the
six-unknown `with_beam_resultants`, and `WarpingPrestress::from_static` with `f14 = K_c u` of D1). For an element with an offset,
`matrices` converts them to the shear-centre form before RF8 and before D2: `f = O^{-T} f_c`, which changes the torque only:
`T_s = T_c + z_0 V_y - y_0 V_z` at each end (the moment about `S`). RF8 (a torque in a `torsion_free` pre-stress) is tested on
`T_s`: a six-unknown pre-stress with a lateral shear force on an offset section is refused (it misses the torsion the offset
makes).

### D5. Section constructor `WarpingSection::monosymmetric_i(b_1, t_1, b_2, t_2, h_w, t_w, shear)` (derived)

Distances from the mid-plane of flange 2 (positive towards flange 1): `h_c = h_w + (t_1 + t_2)/2` (between flange mid-planes),
the web centre `t_2/2 + h_w/2`, the centroid `z_bar = (A_1 h_c + A_w (t_2/2 + h_w/2))/A`, `A_k = b_k t_k`, `A_w = h_w t_w`, `A = A_1 +
A_2 + A_w`. The three rectangles exactly: `I_y` (parallel axes about the centroid), `I_z = t_1 b_1^3/12 + t_2 b_2^3/12 + h_w
t_w^3/12`, `J = (b_1 t_1^3 + b_2 t_2^3 + h_w t_w^3)/3` (thin-walled), `beta_y^C = int z (y^2 + z^2) dA/I_y` integrated over each
rectangle (`int_rect z (y^2 + z^2) dA = (b^3/12)(z_b^2 - z_a^2)/2 + b (z_b^4 - z_a^4)/4`). Thin-walled shear centre and warping
constant (the web on the line of `S`, its warping neglected as ADR 0546 D1; Timoshenko and Gere, Trahair, from memory, derived):
with the flange inertias `I_k = t_k b_k^3/12` and `S` at `h_1`, `h_2` from the flange mid-planes, the principal sectorial
coordinate is `-h_1 y` on flange 1 and `+h_2 y` on flange 2, zero on the web; `int y omega dA = 0` gives `I_1 h_1 = I_2 h_2`, so
`h_2 = h_c I_1/(I_1 + I_2)` and `C_w = I_1 h_1^2 + I_2 h_2^2 = I_1 I_2 h_c^2/(I_1 + I_2)` (equal flanges: `I_f h_c^2/2 = t_f b_f^3
h_c^2/24`, ADR 0546 D1). Shear centre `(y_0, z_0) = (0, h_2 - z_bar)`, `beta_z^C = 0` by symmetry. Refuses a dimension that is
not positive and finite (key the dimension's name).

Values (arithmetic 1): `A = 3.64e-3`, `I_y = 2.6931510037e-5`, `I_z = 3.7199333333e-6`, `J = 1.1445333333e-7`, `C_w =
1.3670248453e-8`, `z_bar = 0.13813186813`, `z_0 = 5.2580303363e-2`, `beta_y^C = -4.7431546579e-2`, **`beta_x = beta_y^S =
-1.5259215331e-1`** (flange 1 at `z = +0.071868`). The approximation `|beta_x| ~ 0.9 h_c (2 rho_f - 1)(1 - I_z/I_y)^2` of
Kitipornchai and Trahair (from memory) gives `0.1146`: it is an approximation fitted to rolled sections, this section's web is
heavy (a third of the area), it is not used.

### D6. Bimoment load and bimoment pre-stress

- **Load**: `Warping::bimoment(node_set, value)` adds `value` to the warping equation of every node of the set in
  `solve_static` (the generalised force conjugate to `chi = phi'`: its work is `B chi`). Refused: an unknown set (`UnknownSet`),
  a node without a warping unknown (key `"warping"`), a non-finite value (key `"bimoment"`). The other entries ignore it (a
  buckling or modal analysis has no load vector).
- **Pre-stress**: the second-order term of a bimoment `B` is `(1/2) beta_omega B phi'^2`, `beta_omega = int omega_S (y^2 + z^2)
  dA/C_w` (Trahair, from memory; the work of the warping normal stress `-B omega/C_w` on the fibre strain of D2, `int omega_S
  v_P'^2` etc. reduce to it by the principal conditions). For a section with an axis of symmetry the principal sectorial
  coordinate about `S` (on that axis) is antisymmetric under the reflection (it reverses the sense of the rotation), `y^2 + z^2`
  is symmetric, so **`beta_omega = 0` for every monosymmetric and doubly symmetric section**, the only sections the
  constructors make. The term is not added; `WarpingSection::new(...).with_shear_centre(...)` for a section without symmetry
  (where `beta_omega != 0`) is documented as not covered (Limits; taken on the standing instruction: recommendation (a) of Q2).

### D7. API (in `rcfd_fem::beam_warping`)

`WarpingSection` gains the private field `shear_centre: [f64; 2]` (zero in `new`, `saint_venant`, `doubly_symmetric_i`):
`with_shear_centre(self, y_0, z_0) -> Result<WarpingSection>` (key `"shear_centre"` for a non-finite value), `shear_centre()`,
`wagner_about_shear_centre()` (`[beta_y - 2 z_0, beta_z - 2 y_0]`), `monosymmetric_i` (D5). `wagner()` keeps returning the
centroidal pair. New local functions `section_local_stiffness(length, section, material, &WarpingSection)` (D1) and
`section_local_geometric_stiffness(length, section, material, axial_force, end_forces_centroid, &WarpingSection)` (D2 with D4's
conversion). With a zero offset both return exactly `local_stiffness(.., c_w)` and `local_geometric_stiffness(.., wagner)` of ADR
0546 (the same call, not a transformed copy). `matrices`, `buckling_factors(_with)`, `solve_static`, `from_static`,
`modal_analysis`, `prestressed_modes` use them for every warping element. `Warping::bimoment` (D6).

### D8. Files

Changed: `src/beam_warping.rs` only. New test: `tests/sa_beam_warping_f627.rs`. Not touched: `beam.rs`, `geometric.rs`,
`buckling.rs` and every other source file. Docs: none unless a topic page has a beam-warping section (ADR 0548 found none).

## Arithmetic done before the code (disclosed; `arith.log`)

1. Section: D5's values. Equal flanges (`0.1, 0.01, 0.1, 0.01, 0.18, 0.006`): `z_0 = 1.4e-17`, `beta^C = 0`, `C_w = A = I_y = I_z =
   J` those of ADR 0546 (`C_w = 1.5041666667e-8`).
2. LTB, `L = 3`: `P_y = 8.15872676e5`, `G J + E C_w k^2 = 1.18023235e4`; roots **`M_+ = 5.39587954e4`** (flange 1, the larger, in
   tension) and **`M_- = -1.78454564e5`** (flange 1 in compression), ratio 3.31. Wrong forms: `beta_x = 0`: `9.81284530e4`;
   `beta^C` in place of `beta_x` (no shear-centre shift): `8.06688424e4`, `-1.19366945e5`. `L = 1.5`: `1.11379879e5`,
   `-6.09362953e5`; `L = 6`: `3.12441934e4`, `-6.23681355e4`. Euler-Bernoulli Hermite model in shear-centre form: 16 elements
   `8.8e-7`, `1.7e-6` (8 elements `1.4e-5`, `2.7e-5`); `L = 1.5`, `6`: below `2e-6`. The rcfd element's shear-flexible lateral plane
   lowers `P_y` by `E I_z k^2/(kappa G A) = 3.5e-3` at `L = 3` (`1.4e-2` at 1.5, `8.7e-4` at 6), so `M` by about half of it at
   most; ADR 0546 measured `+5.9e-4` where it estimated `-1e-3`. **Estimates `3e-3` (L = 3), `8e-3` (1.5), `1e-3` (6).** Mode at
   mid-span: `v_c/phi = 1.18716598e-1` (`M > 0`), `-1.66148144e-1` (`M < 0`); FE (E-B) `7e-7`.
3. Flexural-torsional column (axial compression `P`): `(P_y - P)(G J + E C_w k^2 - P r_0^2) - P^2 z_0^2 = 0`, `r_0^2 = I_p/A +
   z_0^2` (Timoshenko and Gere ch. 5, from memory; derived from the energy of D2 with `v_s = a sin`, `phi = b sin`): **`P_1 =
   6.09657056e5`**; `P_y = 8.159e5`, the torsional load about the centroid `1.40e6`, the strong-axis Euler load `5.9e6`. FE (E-B)
   16 elements `1.5e-6`. Estimate with shear flexibility `3e-3`.
4. Coupled lateral-torsional vibration, forks, `m = 1`: `det([E I_z k^4, 0; 0, G J k^2 + E C_w k^4] - omega^2 rho [A, A z_0; A z_0, I_p
   + A z_0^2]) = 0` (shear-centre unknowns, the centroid mass of D1): **`omega^2 = 2.33976263e4`, `7.19848243e4`** (uncoupled:
   lateral `3.13e4`, torsion `5.38e4`). FE (E-B) 16 elements `1.5e-6`, `1.1e-6`; the next is `2.34e5`. Rotary inertia `r_z^2 k^2 =
   1.1e-3` and shear `3.5e-3`: estimate `3e-3`.
5. Static cantilever, `L = 3`, root held (six components and warping), tip force `F = 1000` along `e_2` at the centroid: torque
   about `S` `T = z_0 F = 52.580303`, `G J = 8.804103e3`, `E C_w = 2.734050e3`, `k = 1.794483`, `kL = 5.38345`: **`phi(L) = (T/(G J))(L
   - tanh(kL)/k) = 1.45887735e-2`**; `v_s(L) = F L^3/(3 E I_z) + F L/(kappa G A) = 1.21098481e-2`, **`v_c(L) = v_s(L) + z_0 phi(L) =
   1.28769303e-2`**. Hermite, 16 elements, `phi`: `-2.0e-6`.
6. Tip bimoment `B = 100`, the same cantilever (root twist and warping held): `G J psi - E C_w psi'' = 0` (`psi = phi'`, no
   torque), `psi(0) = 0`, `E C_w psi'(L) = B`: `psi = B sinh(kx)/(E C_w k cosh(kL))`; **`phi(L) = (B/(G J))(1 - 1/cosh(kL)) =
   1.12540293e-2`**, **`chi(L) = B tanh(kL)/(E C_w k) = 2.03814903e-2`**, the root bimoment reaction `K u - f = -B/cosh(kL) =
   -9.18372061e-1`, the root torque reaction 0; the centroid moves laterally by `z_0 phi(L)`. Hermite 16 elements: `-4.5e-7`,
   `-8.8e-6`, `+4.8e-5`; twist reaction `3e-12`.

## Checks (limits fixed now)

Every aggregated error is asserted finite before it is compared (a non-finite value is a miss). The monosymmetric section of the
readings, `L = 3`, 16 elements along `x` unless stated. "Flange 1 in compression" is decided in the test from the static solution
of the loaded model: the fibre strain `z_1 dtheta_y/dx` at flange 1 (`z_1 = +0.071868`, `u_P = z theta_y`) by the difference of
the end rotations, negative = compression (independent of the moment sign convention).

| id | check | limit | estimate |
|---|---|---|---|
| B0 | the bit records `sa_geometric_struct_f558_bits.rs`, `sa_beam_warping_f598_bits.rs`, `sa_beam_warping_f626_bits.rs` run unchanged after the change; `sa_beam_warping_f598.rs` and `sa_beam_warping_f626.rs` pass unchanged | equal (bit); pass | exact |
| D1 | `monosymmetric_i` against the values of D5 (`A, I_y, I_z, J, C_w, z_0, beta_y^C, beta_x`; `y_0 = beta_z^C = 0` exactly) | `<= 1e-9` relative each | `1e-11` (printed digits) |
| D2 | equal flanges: `monosymmetric_i(0.1, 0.01, 0.1, 0.01, 0.18, 0.006)` against `doubly_symmetric_i(0.1, 0.01, 0.18, 0.006)`: `A, I_y, I_z, J, C_w`; `|z_0|`, `|beta_y^C|` | `<= 1e-12` relative; `<= 1e-12` m | `1e-16`; `1e-17` |
| D3 | the D2 equal-flange section on the A1 set-up against the ADR 0546 factor of `doubly_symmetric_i` | `<= 1e-9` relative | `1e-13` |
| A1 | **acceptance**, flange 1 in compression: `|M_FE/M_- - 1|` (`|M_-| = 1.78454564e5`) | `<= 2e-2` | `3e-3` |
| A2 | **acceptance**, flange 1 in tension: `|M_FE/M_+ - 1|` (`5.39587954e4`) | `<= 2e-2` | `3e-3` |
| A3 | the sign assignment: the load case of A1 (the larger `|M|`) is the one with flange 1 in compression | holds | |
| A4 | A1 and A2 at 8 elements | `<= 2e-2` each | `3e-3` |
| A5 | A1 and A2 at `L = 1.5` and `L = 6` (16 elements) | `<= 2e-2` each | `8e-3`; `1e-3` |
| A6 | the beam along global `y` (orientation `z`, the permuted supports of ADR 0546 A5), both signs, against axis `x` | `<= 1e-6` relative | `1e-12` |
| A7 | the pre-stress of the warping static solve (`WarpingPrestress::from_static`, `buckling_factors_with`) against the six-unknown pre-stress (`buckling_factors`), both signs | `<= 1e-9` relative | `1e-12` |
| A8 | the buckling mode at mid-span: `v/phi` (components 1 and 3 of node 8) against `M/P_y + z_0` with the FE `M`, both signs | `<= 1e-2` relative | `3e-3` |
| N1 | negative controls: the A1 and A2 FE values against the `beta_x = 0` form (`9.81284530e4`) and against the `beta^C` form (`1.19366945e5`, `8.06688424e4`) | `> 0.3`; `> 0.15` each | `0.45, 0.82`; `0.33, 0.49` |
| C1 | flexural-torsional column: axial compressive tip force, the six-unknown pre-stress (`Prestress::from_static`, no end forces), `buckling_factors`: against `P_1 = 6.09657056e5`; and against `P_y` (the uncoupled weak-axis load) | `<= 1e-2`; `> 0.1` | `3e-3`; `0.25` |
| V1 | lateral-torsional vibration (components 0, 2, 4 held at every node, forks, warping free), `modal_analysis`, 3 modes: the lowest two against arithmetic 4 | `<= 1e-2` each | `3e-3` |
| S1 | static cantilever of arithmetic 5, `solve_static`: `phi(L)` (signed) against `1.45887735e-2`; `v(L)` against `1.28769303e-2` | `<= 1e-3`; `<= 1e-3` | `2e-6`; `1e-5` |
| S2 | S1 with the tip force along `e_3` (through `C` and `S`): `|phi(L)| <= 1e-14 |w(L)|`, every warping unknown `<= 1e-14 |w(L)|/L` | strict | `0` |
| W1 | bimoment of arithmetic 6 (`Warping::bimoment`): `phi(L)`, `chi(L)` signed against the closed forms; the root warping reaction against `-B/cosh(kL)`; the lateral tip displacement against `z_0 phi(L)` | `<= 1e-3` each | `5e-7`, `9e-6`, `5e-5`, `5e-7` |
| W2 | W1's root twist reaction | `<= 1e-9 B k` | `3e-12 B` |
| E1 | element: `section_local_stiffness` with the offset `(0.01, 0.04)` (`L = 0.7`, the monosymmetric `BeamSection`): entries outside rows and columns 3, 10 bit-equal to `local_stiffness`; rows and columns 3, 10 against the dense `O^T K O` of the test | exact; `<= 1e-12 max|K|` | exact; `1e-16` |
| E2 | geometric: uniform `M_y = 3`, `M_z = 2` (end forces in equilibrium, `N = 0`), `beta^C = (0.7, -0.4)`, offset `(0.01, 0.04)`: `section_local_geometric_stiffness` minus `local_geometric_stiffness(beta^C)`: the twist block against `(-2 z_0 M_y + 2 y_0 M_z)/(30 L)` times the first matrix of ADR 0546 D2, the other entries zero | `<= 1e-12` relative | `1e-16` |
| E3 | `N = -5e3`, no end forces, the offset: `section_local_geometric_stiffness` bit-equal to `local_geometric_stiffness` (the axial part in centroid unknowns) | exact | exact |
| E4 | zero offset (`WarpingSection::new(c_w, 0.7, -0.4)`): `section_local_stiffness` and `section_local_geometric_stiffness` (with end forces including shear and torque) bit-equal to the ADR 0546 functions | exact | exact |
| R | refusals: `with_shear_centre` non-finite (`"shear_centre"`); `monosymmetric_i` with a non-positive dimension (its name, two cases); `bimoment` on an unknown set (`UnknownSet`), on a node without a warping unknown (`"warping"`), non-finite (`"bimoment"`); RF8 on the offset: the six-unknown pre-stress of a lateral tip force (centroid torque zero, shear-centre torque `z_0 V`) refused by `buckling_factors` (`"warping"`) while the same with `doubly_symmetric_i` is accepted | exact | |

Negative controls: N1 (the shear-centre shift of `beta` acts and the `beta` term acts), C1's second limit (the offset couples
the column), S2 (no coupling along the symmetry axis), A3 (direction), R (RF8 sees the shear-centre torque).

## Planned mutations (on a scratch copy, one at a time, own `CARGO_TARGET_DIR`)

| id | mutation | expected caught by |
|---|---|---|
| Z1 | `beta^S` replaced by `beta^C` in the element (no `-2 z_0`) | A1, A2, N1, E2 |
| Z2 | the shift with the wrong sign (`+2 z_0`) | A1, A2, E2 |
| Z3 | the transform `O` left out of the stiffness (`K_c = K_s`) | C1, V1, S1, A8, E1 |
| Z4 | the axial-force part transformed too (`O^T G(N, ends, beta^S) O`) | C1, E3 |
| Z5 | the end-force conversion of D4 left out | R (RF8 on the offset) |
| Z6 | the bimoment load ignored | W1 |
| Z7 | the bimoment load with the wrong sign | W1 |
| Z8 | the shear centre from the wrong flange (`h_2 = h_c I_2/(I_1 + I_2)`) | D1, A1, A2 |
| Z9 | `O` with the wrong sign (`v_s = v_c + z_0 phi`) | S1, A8, E1 |
| Z10 | the mass transformed as the stiffness (`O^T M O`) | V1 |

## Decision rule

**done**: every registered check (B0, D1 to D3, A1 to A8, N1, C1, V1, S1, S2, W1, W2, E1 to E4, R) met in the one run and every
mutation caught. **partial**: any registered check missed (a `*_known_miss` record), any mutation not caught (unless declared
equivalent here, none is), or any check met only after a post-run amendment. **todo**: A1, A2 or B0 missed.

## Limits (not covered)

- Shear-force terms of the geometric stiffness with an offset are taken in shear-centre form (D2) without a check: no
  non-uniform moment and no transverse load in a buckling pre-stress of an offset section is checked (no exact closed form; the
  moment-gradient factors are approximations).
- Load height (a load above or below the shear centre) is not modelled (as ADR 0546).
- No bimoment pre-stress (D6: zero for every section with an axis of symmetry); a section without symmetry given through
  `with_shear_centre` would miss `beta_omega`.
- The section of `monosymmetric_i` is thin-walled for `C_w`, `J` and the shear centre (the web's warping neglected), exact
  rectangles for `A`, `I_y`, `I_z` and `beta^C`.
- No warping inertia (ADR 0548 Q1), no kinks (RF3), no transformation path (RF4), no case-file key (F-625), no harmonic or
  transient path (F-635).

## Open questions (to be taken on the maintainer's standing instruction of 2026-11-11 when answered; none changes a default)

- **Q1. Load height for transverse loads on monosymmetric beams.** Options: (a) none; (b) a load-height geometric term `-(1/2) q
  a phi^2`. Recommendation (b) as a separate item together with a non-uniform moment check.
- **Q2. Sections without symmetry** (`beta_omega != 0`, a principal-axis rotation). Options: (a) not offered (this item); (b) a
  general section from a thin-walled outline with the bimoment pre-stress. Recommendation (a) until a case needs it.

## Amendment 1 (2026-11-15): E2 corrected, written while writing the test and before any run of any check

No check has been run. While writing the test of E2 it became clear that its second clause is false by construction: the
section rotations `theta_y`, `theta_z` of the ADR 0508 moment terms are the interdependent rotation fields of the Timoshenko
element (`beam::second_order_rows`), which depend on the nodal translations `v`, `w` as well as on the nodal rotations (for
Euler-Bernoulli, `theta_z = v'` of the Hermite deflection). So the moment part `G` of D2 has entries in the rows of `v` and `w`,
and `O^T G O - G` is not zero outside the twist block: the difference `section_local_geometric_stiffness - local_geometric_stiffness`
is not the Wagner shift alone. The design (D1, D2) is unchanged; only the check was wrong.

Corrected **E2'** (limits fixed now, before any run):

- (a) `section_local_geometric_stiffness(L = 0.7, N = -5e3, f, ws)` with the offset `(0.01, 0.04)`, `beta^C = (0.7, -0.4)` and end
  forces of an element in equilibrium with shear forces, torque and moments (`V_y = 40`, `V_z = -25`, `T = 6`, `M_y(0) = 3`,
  `M_z(0) = 2`, the end moments from equilibrium) against the dense composition of the test: `local_geometric_stiffness(N, None,
  [0, 0]) + O^T local_geometric_stiffness(0, f_s, beta^S) O`, with the test's own `O` (D1's signs), its own `f_s` (`T_s = T_c + z_0
  V_y - y_0 V_z` at each end) and its own `beta^S = (0.7 - 2 * 0.04, -0.4 - 2 * 0.01)`: `max|diff|/max|hand| <= 1e-12`
  (estimate `1e-16`).
- (b) `wagner_about_shear_centre()` of that section against `(0.62, -0.42)`: `<= 1e-15` absolute each (estimate `1e-17`).

Mutations: Z1 and Z2 are now expected to be caught by A1, A2, N1 and E2' (b) or (a) depending on where the mutation is placed
(in `wagner_about_shear_centre`: (b); in the element: (a)). The original E2 is not run (it was found false before any run).

## Amendment 2 (2026-11-15): the eigensolver set-up of the A checks is defective; written after the first run of `sa_beam_warping_f627.rs` and before any corrected run

The first run (log `w49i-work/f627-scratch/run1.log`, code commit 9031e35a) ran every test once. Six of the seven test functions
finished, and every check in them met its limit (D1 to D3, C1, V1, S1, S2, W1, W2, E1, E2', E3, E4, R; numbers in addendum 1). The
seventh, `a_n_lateral_torsional_buckling_of_the_monosymmetric_i` (A1 to A8, N1), stopped at its second solve. That was the
`-M_ref` load case (flange 1 in compression) at 16 elements. `lowest_buckling_factors` refused it with `SturmMismatch`: "32
eigenvalues below 5.397979218577e9, but 3 were found". So no A or N1 value was measured; only the `+M_ref` factor `53.9798` was
printed before the stop.

Cause, read from the message and the solver code (`rcfd-linalg` `eigen.rs`, `lowest_buckling_factors`) with no further run:

- The bound `5.397979e9` is `1/(POSITIVE_CUT * tmax)` with `POSITIVE_CUT = 1e-8`, so `tmax = 1/53.98`. The largest `|theta|` of
  the subspace is the negative factor `-53.98`, the mirror of the `+M_ref` case.
- The spectrum of a monosymmetric beam is not symmetric in the sign of the load, unlike the doubly symmetric beams of ADR 0546
  and 0548. For `-M_ref` the negative factors are smaller in magnitude than the positive ones, so they dominate the subspace
  iteration.
- The default subspace of `BucklingOptions::new(3)` is `q = 11`. It held exactly three positive Ritz values, so `mp = positive
  = count`. The solver then takes its "exhausted" branch and counts at the far bound `1/cut`, where all 32 positive factors of the
  model lie, instead of between the third and fourth factors. The counts differ and the solver refuses.

This is a property of the registered eigensolver options on a load case with a sign-asymmetric spectrum, not of the element.
The registered limit was unreachable with the registered set-up. Changing `rcfd-linalg` (a shared solver file) is out of scope;
it is reported as a finding (Q3 below).

Corrected **A1' to A8', N1'**: the same models, loads, comparisons and limits as A1 to A8 and N1 (unchanged), with
`subspace_size = Some(40)` in the `BucklingOptions` of every LTB solve of the A test (`factors = 3` unchanged). The model has
about 110 free unknowns. With either sign about half of a 40-vector subspace is positive, so `positive > count`, and the Sturm
check is made between the third and fourth factors. Estimates unchanged. The first-run function stays as the ignored record
`a_n_run1_known_miss`, with the refusal recorded in its comment. The checks of the six finished functions are not re-run in the
corrected run (the corrected run is the A test alone). D3, which uses `+M_ref` only, is not affected.

By the decision rule, a check met only after a post-run amendment makes the item **partial**.

- **Q3 (new). The buckling eigensolver on sign-asymmetric spectra.** Options: (a) leave it, document that a load case whose
  negative factors are smaller in magnitude needs a larger subspace; (b) in `lowest_buckling_factors`, when `positive ==
  mp == count` and the subspace also holds negative Ritz values, enlarge the subspace or iterate on, instead of counting at
  `1/cut`. Recommendation (b), as a separate item in `rcfd-linalg` with its own registration (it touches a shared solver).
