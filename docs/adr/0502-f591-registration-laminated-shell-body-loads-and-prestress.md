# ADR 0502: F-591 registration: laminated Shell4 under body loads and pre-stress (gravity, the centrifugal load and the nodal body force on a layup, the geometric stiffness and the spin-softening mass of a layup: buckling and rotating parts)

Status: **REGISTERED 2026-11-14, written before any code of F-591 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-591 was written, compiled or run in the repository: the files listed under "What was
read" were read, and the arithmetic below was done by hand and evaluated with a plain numpy prototype in the scratch folder
(`w49i-work/f591-scratch/probe_f591.py`, `probe_f591.log`; no rcfd code; disclosed, its numbers quoted below). The bit record
B0 of the default paths is a test file written and run on the base after this commit and before any library code (D8). Results
go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-591 (deps F-428, F-420), the follow-up proposed by [ADR 0496](0496-f428-registration-laminated-shell-layup-ply-stresses-failure.md)
("For the lead: roadmap entries"). Number 0502 given by the lead. Branch `f591-work`, worktree `w49i-work/f591`, scratch
`w49i-work/f591-scratch`. Base 70348e9f (the head of the F-428 work).

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe code, the crate graph
unchanged; no existing test edited except by the records-and-copies scheme of ADR 0291 (D9); clean-room.

## What F-591 asks

The acceptance (proposed in ADR 0496 and given verbatim by the brief), binding:

> Every limit not stated here is registered in the ADR before the run. Gravity and the centrifugal load on a Layup block take the consistent load int N_i (sum rho_k t_k) b dA and the moment of the first mass moment; a one-ply isotropic layup gives the bits of its Shell twin for every such load; a rotating [0/90]s strip clamped on the axis has the radial membrane force rho_m Omega^2 (L^2 - r^2)/2 within 1e-8 and the flap frequency of the spinning strip within 2 percent of the Southwell form with the layup's D11; the buckling load of a simply supported [0/90]s plate under uniaxial compression is pi^2 (D11 + 2 (D12 + 2 D66) + D22)/a^2 within 2 percent with 16 x 16 elements.

The brief adds: (b1) check the closed forms by hand here (units of the membrane force; a registered Southwell coefficient for a
strip clamped at the axis, derived, with its method; the critical half-wave number of the orthotropic plate and its `a/b`);
(b2) keep the one-isotropic-ply route of ADR 0496 D3 for every new load; (b3) the corotated Newton path keeps refusing a layup
(`shell_corot.rs` not touched); (b4) no case-file key (a proposal in the open questions); (b5) do not edit the files of F-576
(viscoelastic time stepping, the nonlinear driver), F-387 (the fan-blade demonstration test on Tet10, `docs/structural-solver.md`)
and F-564 (`rotor.rs`); (b6) defaults keep their bits, `baseline_hashes` unchanged, a default-bits test at the base before any
library change; (b7) every aggregated error finite; probe before registering a tight limit; (b8) the fan target: a 120 mm
axial fan whose blade may be a glass-filled moulded layup shell that must take the centrifugal load and the spin stiffening.

Clauses: (a0) every limit registered here; (a1) gravity and the centrifugal load on a Layup block are the consistent load `int
N_i m0 b dA`, `m0 = sum rho_k t_k`, plus the moment of the first mass moment `m1 = sum rho_k t_k zbar_k`; (a2) one isotropic
ply gives the bits of its Shell twin for every such load (gravity, centrifugal, nodal body force, and through them the
pre-stress, the geometric stiffness and the spin-softening mass); (a3) the rotating `[0/90]s` strip clamped on the axis: the
radial membrane force within `1e-8`; (a4) its flap frequency within `2e-2` of the Southwell form with `D11`; (a5) the SS
`[0/90]s` plate under uniaxial compression, 16 x 16, within `2e-2`.

Readings, registered:

- **Units of (a3)** (b1). `rho_m` is the mass per unit area `m0 = sum rho_k t_k` (kg/m^2); the membrane force `N_r` is a force
  per unit width (N/m). Equilibrium of the part of a strip of width `W` beyond the radius `r`: `W N_r(r) = int_r^L m0 W Omega^2
  s ds`, so `N_r = m0 Omega^2 (L^2 - r^2)/2`; units `kg/m^2 1/s^2 m^2 = N/m`. The acceptance's `rho_m` is therefore `m0`.
- **"Has the radial membrane force ... within 1e-8"** (a3). A bilinear element has a membrane force that is constant in `x` per
  element (`N_x` linear in the element; its centre value is its average), so no bilinear mesh gives the quadratic `N_r(r)`
  pointwise to `1e-8`: at the element centre `r_c` the element value differs from `N_r(r_c)` by `m0 Omega^2 h^2/24` (arithmetic
  2). The registered quantity is the one the element gives exactly (arithmetic 2): for each column of elements `x_j <= x <=
  x_{j+1}` (width `h`), the average over the column of the element-centre `N_xx` of `shell::stresses` against the column
  average of the closed form, `m0 Omega^2 (L^2 - r_c^2 - h^2/12)/2`; the error of each column over the root value `m0 Omega^2
  L^2/2`, the largest over the columns, `<= 1e-8`.
- **"The flap frequency of the spinning strip within 2 percent of the Southwell form with the layup's D11"** (a4). Flap: the
  motion along the rotation axis (the strip in the plane perpendicular to the axis, its normal along the axis: the fan blade's
  flap). The Southwell form `omega_S^2 = omega_0^2 + S Omega^2` with `omega_0 = beta_1^2 sqrt(D11/(m0 L^4))`, `beta_1 =
  1.8751040687` (the clamped-free beam), the closed form (not the computed rest frequency), and the Southwell coefficient `S =
  1.1933364` of arithmetic 3. "With the layup's D11": the strip in cylindrical bending, the rotation about the strip's radial
  axis `theta_x = 0` at every node, so that the bending stiffness per width is `D11` exactly (no anticlastic curvature; without
  it a free strip of this width lies between `D11 - D12^2/D22 = 0.955 D11` and `D11` depending on `W^2/(L t)`, a 2.3 percent band
  in the frequency, arithmetic 3). The flap frequency is that of `RotatingPart::modes` (stress stiffening and spin softening on,
  the pre-stress of the centrifugal load at 1 rad/s, F-396/F-420) of the mode of the four lowest with the largest kinetic share
  in `u_z`. Registered speeds `Omega = 0.5 omega_0` and `omega_0` (`omega_0 = 535.0652 rad/s`, about 5100 rpm: a fan's range);
  each within `2e-2`.
- **"The buckling load of a simply supported [0/90]s plate under uniaxial compression"** (a5) (b1). Square, `a = b = 1 m`,
  `t = 0.01 m` (`t/a = 0.01`, the plate of ADR 0496 PL1), the 0-degree plies along the load `x`; the set-up of ADR 0473 P1 (`w
  = 0` on the four edges, rotations free, `u_x = 0` on `x = 0`, `u_y = 0` at the node `(0, 0)`, the consistent edge force of
  `N_ref = 1000 N/m` in `-x` on `x = a`, the pre-stress from the static solve); the lowest positive factor of
  `buckling::factors` times `N_ref` against `pi^2 (D11 + 2 (D12 + 2 D66) + D22)/a^2` with the hand `D` of ADR 0496 arithmetic 3.
  Arithmetic 4 shows `m = 1` is the critical half-wave number for these `D`.
- **"The moment of the first mass moment"** (a1): per unit area the body force `rho(z) b` on the director `u + z theta x n`
  does the work `m0 b . u + m1 b . (theta x n) = m0 b . u + m1 theta . (n x b)`: the nodal moments `m1 int N_i (n x b) dA`. `b` is
  the acceleration at the mid-surface point (gravity `g`; centrifugal `Omega^2 P (x - p)`), as for the homogeneous shell (ADR
  0435 D2: the body force at the projected mid-surface); its variation through the thickness is not part of the load (Limits).
- **"For every such load"** (a2): gravity, the centrifugal load, the nodal body-force field; and, since the layup's pre-stress,
  geometric stiffness and spin-softening mass are made by this item, the static solve under them, `prestress_from_static`,
  `buckling::factors` and `RotatingPart` (its unit pre-stress, `K_sigma,1`, `M_perp`, the modes at a speed). A Layup block's
  own material is not used (ADR 0496 D7): the twin is the `Shell { t }` block of the ply's material.

## What was read (no run)

- `AGENT_RULES.md`; the brief; ADR 0496 (whole: D3 the route, D5 the layup mass, D7 the paths that refuse a layup, the RX2
  checks, the addendum and its proposed follow-up), ADR 0473 (F-420: D2 to D6, the checks P1, S1, R1 to R5, E1, the addendum
  P1 `3.30e-3`), ADR 0435 (the shell body loads, cited by the code), ADR 0403 (the Southwell coefficient `1.193336`, quoted by
  ADR 0473; not reopened).
- `crates/rcfd-fem/src`: `loads.rs` (`nodal_load_vector`, `add_body_load_with`, `add_shell_body_load`, `densities`,
  `perpendicular`), `shell.rs` (`local_mass`, `local_mass_layup`, `local_geometric_stiffness`, `element_geometric_stiffness`,
  `rotary_softening_factor`, `local_spin_softening_mass`, `element_spin_softening_mass`, `body_load_vector_at`,
  `model_element_mass`, `model_thermal_load_vector`, `shell_thickness`, `thickness_or_error`), `laminate.rs` (`Laminate`,
  `laminate`, `single_isotropic`), `geometric_struct.rs` (whole), `rotating.rs` (lines 1 to 520: `spin_softening_mass`,
  `RotatingPart`, `unit_centrifugal_model`, `check_model`), `buckling.rs` (the result type, `check_model`), `geometric.rs` (its
  dispatch to `geometric_struct`), `mass.rs` (the shell dispatch).
- Tests: `sa_laminate.rs` (the helpers, PL1, MS1, RX2), `sa_laminate_default_bits.rs` (the digest pattern),
  `sa_geometric_struct.rs` (the plate set-up of P1, the checks helper), `sa_scaffold.rs` (`entries` and R3c: run without a
  pre-stress, so E7 and E8 do not reach a layup; unaffected).

## D0. What exists and what refuses

After ADR 0496 a Layup block on Shell4 is taken by the stiffness, the mass (`local_mass_layup` with `m0, m1, m2`), the thermal
load, the pressure, `stresses` and `layup_response`. It is refused, naming `laminate::FOLLOW_UP`, by every path that reads one
thickness through `shell::thickness_or_error`: `loads.rs::add_shell_body_load` (gravity, centrifugal, nodal body force),
`geometric_struct::prestress_from_displacement` (`(1/t) N` of a shell), `geometric_struct::element_geometric_stiffness` and
`element_spin_softening_mass` (buckling, pre-stressed modes, the rotating part), and the Shell3 functions (a Shell3 Layup block is
refused earlier by `check_shell_model`); the corotated Newton path refuses it in `shell_corot::check_model`.

## Design

### D1. The layup body load (a1; first principles)

The consistent nodal load of the director kinematics `u(z) = u + z theta x e_3` under the body force `rho(z) b(x)`, `b` the
acceleration at the mid-surface point `x`: `delta W = int_A (m0 b . delta u + m1 b . (delta theta x e_3)) dA`. In the facet's
local axes, `b_l = (e_1 . b, e_2 . b, e_3 . b)`, `delta theta x e_3 = (delta theta_y, -delta theta_x, 0)`, so per node `i`

`f_i = m0 int N_i b_l dA` on the translations, `m_i = m1 int N_i (-b_l2, b_l1, 0) dA` on the rotations `(theta_x, theta_y,
theta_z)`,

by the 2 x 2 rule at the points of the projected facet (as `body_load_vector_at`), carried to the nodes by `T^T`. The sign
agrees with the mass coupling of ADR 0496 D5 (`+m1` between `u_x` and `theta_y`, `-m1` between `u_y` and `theta_x`): the load of
a uniform acceleration is the consistent mass times the rigid acceleration (check G1). `shell::layup_body_load_vector_at(coords,
m0, m1, b)` with `b(N, x)` the acceleration.

In `loads.rs` the Shell4/Shell3 rule becomes one of two kinds:

- **per mass** (gravity, centrifugal): a function `(rho, N, x) -> rho-proportional force per unit volume`, written with the
  same operations as today: gravity `[rho g_0, rho g_1, rho g_2]`, centrifugal `s = rho omega^2`, `[s r_0, s r_1, s r_2]`. A
  `Shell` block calls it with `rho[e]` (the block material's density, as today: the same operations on the same values, so the
  same bits) and `body_load_vector_at(coords, t, ...)` as today. A Layup block of **one isotropic ply** (D3 route) calls it
  with the ply's density and `body_load_vector_at(coords, t_ply, ...)`: the call of its `Shell` twin. Any **other layup**: the
  acceleration is the rule at `rho = 1` (`1 g_k` and `(1 omega^2) r_k` are exact) and the load is D1 with the laminate's `m0`,
  `m1`.
- **per volume** (the nodal body-force field, N/m^3): unchanged for `Shell`; for a Layup block `body_load_vector_at(coords,
  t_total, ...)`, `t_total` the ply sum (one isotropic ply: its thickness, the twin's call). The first moment of the volume
  about the mid-plane is zero (the mid-plane is at the half-thickness, ADR 0496 D2), so no moment.

### D2. The pre-stress and the geometric stiffness of a layup

The geometric stiffness of a shell takes the membrane resultant `N` only (ADR 0473 D2): it does not depend on the layup. The
pre-stress of a structural model stores `(1/t) N` and the element takes `N = t (stress)` back (`membrane_resultant`); for a
Layup block both use the total thickness `t_total` (`shell::membrane_thickness(mesh, e)`: `Shell` its thickness, a Shell4
Layup block one isotropic ply's thickness or else `laminate(plies).thickness`, otherwise `thickness_or_error` as today). With
the layup resultants of `shell::stresses` (ADR 0496 D2, the bending coupling `B kappa` included) the buckling and pre-stressed
paths take a layup with no other change. One isotropic ply: the same thickness and the same `stresses` (the D3 route of ADR
0496), so the same bits.

### D3. The spin-softening mass of a layup (first principles; the kinetic energy of ADR 0473 D3 with the mass moments)

`M_perp` is the kinetic energy of the motion perpendicular to the axis `a`, `P = I - a a^T`, of `u + z w`, `w = theta x e_3 = W
theta`, `W = [[0, 1, 0], [-1, 0, 0], [0, 0, 0]]` (local): `int rho |P (u + z w)|^2 dz = m0 u^T P u + 2 m1 u^T P W theta + m2
theta^T W^T P W theta`, and `W^T P W = R` (`rotary_softening_factor`; by expansion `|w|^2 - (a . w)^2`). Per pair of nodes, `s
= det N_a N_b` at the 2 x 2 points: `m0 s P` on the translations, `m2 s R` on the rotations, `m1 s (P W)` between the
translations of `a` and the rotations of `b` and its transpose, `(P W)[c][0] = -P[c][1]`, `(P W)[c][1] = P[c][0]`, `(P W)[c][2]
= 0`; the drilling rotation takes no term (as ADR 0473 D3). For `P = I` the coupling is that of `local_mass_layup`.
`shell::local_spin_softening_mass_layup(f, m0, m1, m2, a)`, `shell::element_spin_softening_mass_layup(coords, m0, m1, m2, axis)`,
and `shell::model_spin_softening_mass(mesh, e, coords, material, axis)`: `Shell` as today, one isotropic ply
`element_spin_softening_mass(coords, t, ply density, axis)` (the twin's call), any other layup the layup function.

### D4. The paths

- `loads.rs`: D1 (the shell hook of `add_body_load_with` takes the kind of rule; the beam and solid parts unchanged).
- `geometric_struct.rs`: `prestress_from_displacement` and `element_geometric_stiffness` (Shell4) use `membrane_thickness`;
  `element_spin_softening_mass` (Shell4) calls `shell::model_spin_softening_mass`. Shell3 lines unchanged.
- `thickness_or_error` keeps its Layup refusal for the paths that still need one thickness (the Shell3 functions; no Shell4
  path calls it for a layup after this item). `shell_corot::check_model` keeps refusing (b3).
- Nothing in `rotating.rs`, `buckling.rs`, `geometric.rs`, `mass.rs` (they reach the layup through the functions above).
- Doc comments: `mesh.rs` (the Layup property: the paths that take it), `loads.rs` (module header), `laminate.rs` (header).

### D5. Files

`crates/rcfd-fem/src/shell.rs` (D1 to D3 functions), `loads.rs` (D1), `geometric_struct.rs` (D2, D3 dispatch), `mesh.rs` and
`laminate.rs` (doc comments only). Tests: `crates/rcfd-fem/tests/sa_laminate_body.rs` (the checks),
`sa_laminate_body_default_bits.rs` (B0), the copy of D9 in `sa_laminate.rs`. No file of F-576, F-387, F-564; no edit of
`shell_corot.rs`, `rotating.rs`, `docs/structural-solver.md`.

## Arithmetic done before the code (disclosed)

Ply data of ADR 0496 (glass-filled moulded plastic, registered there, not measured): `E1 = 10 GPa`, `E2 = 4 GPa`, `nu12 =
0.35`, `G12 = G13 = 1.6 GPa`, `G23 = 1.4 GPa`, `rho = 1400 kg/m^3`; `Q11 = 1.0515247108e10`, `Q12 = 1.4721345952e9`, `Q22 =
4.2060988433e9`, `Q66 = 1.6e9 Pa` (probe). `[0/90]s` of four equal plies: `D11 = t^3 (7 Q11 + Q22)/96`, `D22 = t^3 (Q11 + 7
Q22)/96`, `D12 = Q12 t^3/12`, `D66 = Q66 t^3/12` (ADR 0496 arithmetic 3, checked there by L4).

1. **The consistent layup load is exact for these checks.** (G1) A uniform acceleration `g`: `sum_j M_ij g = m0 int N_i (sum_j
   N_j) g + m1 (coupling) = ` the D1 load, by the partition of unity, with the same 2 x 2 rule in the mass and the load; on a
   flat facet `T` maps a nodal translation to the local translation (no offset), so `f = M d_g`, `d_g` the nodal vector with
   `g` on every translation and zero rotations, to rounding. (C1) The centrifugal acceleration `Omega^2 P (x - p)` is affine in
   `x` and `x = sum N_j x_j` on the facet, so it equals `sum N_j Omega^2 P (x_j - p)` at every point: `f = M d_c`, `d_c` the nodal
   accelerations. (G2) The resultant force of gravity is `m0 A g` and its moment about the origin `int (x x m0 g) + m1 (n x g) dA
   = m0 A (x_cm x g)`, `x_cm = c + (m1/m0) n` the mass centre of the layup plate (`c` the centroid of the mid-surface): the
   "moment of the first mass moment". (C2) The resultant force of the centrifugal load: `Omega^2 m0 A P (c - p)` (affine
   integrand). (NB1) A uniform nodal body force `b` per volume: force `t_total A b`, moment `t_total A (c x b)`.
   Registered plate: `1 x 0.5 m`, 4 x 2 elements, rotated by `0.4 rad` about `(1, 1, 1)/sqrt 3` and moved by `(0.3, -0.2,
   0.5)`; the layup of two plies of `1 mm` at 0 and 90 degrees with the densities `1400` and `2800` (the data of ADR 0496 MS1):
   `m0 = 4.2 kg/m^2`, `m1 = 7.0e-4 kg/m`, `m1/m0 = 1.6667e-4 m`, `m2 = 1400 (1e-3 2.5e-7 + 1e-9/12) + 2800 (...) = 1.4e-6 kg`
   (by hand: `sum rho_k (t_k zbar_k^2 + t_k^3/12)` with `zbar = -+5e-4`, `t_k^3/12 = 8.33e-11`: `4200 (2.5e-10 + 8.33e-11) =
   1.4e-6`). `g = (1, -2, -9.81) m/s^2`; centrifugal axis `(0.2, 0.3, 1)` (normalised), origin `(0.1, 0.05, -0.1)`, `Omega = 50
   rad/s`; nodal body force `b = (100, 200, -300) N/m^3` at every node. The share of the first-moment term in the gravity
   moment is `|m1 (n x g)| / |m0 x_cm x g|` of order `m1/(m0 |c|) ~ 2e-4` (the negative control NG: the moment without it misses
   G2 by more than `1e-6`).
2. **The radial membrane force of the column** (a3). Virtual displacement `delta u = (phi(x), 0, 0)`, `phi = 0` for `x <= x_j`,
   `(x - x_j)/h` on the column, `1` beyond: in the bilinear space, its strain is `delta eps_xx = 1/h` on the column and zero
   elsewhere, no other strain (`delta eps_yy = delta gamma_xy = 0`, no curvature, no transverse shear, the drilling term `theta_z
   - (v_x - u_y)/2` of `delta` zero). The discrete equilibrium gives `(1/h) sum_col int N_xx dA = int m0 Omega^2 x phi dA` (the
   consistent load: the integrand is quadratic per element in `x`, the 2 x 2 rule exact). On a rectangular element `N_xx = A11
   u_x + A12 v_y + A16 (...)` plus `B11 kappa_x + ...`: `u_x` of the bilinear field is linear in `y`, `v_y` linear in `x`, so
   `N_xx` is linear in the element and its 2 x 2 integral is the area times the centre value; the `B` part is rounding (a
   symmetric layup, `|B| <= 1e-12 |A| t`, ADR 0496 L1) and the membrane of a Layup block is the bilinear one (no ANDES term).
   Hence the column mean of the centre values is `m0 Omega^2 [(L^2 - x_{j+1}^2)/2 + (1/h) int_0^h (x_j + s) s ds] = m0 Omega^2
   (L^2 - x_j^2 - x_j h - h^2/3)/2 = m0 Omega^2 (L^2 - r_c^2 - h^2/12)/2` (expanded by hand; `r_c = x_j + h/2`): the column average
   of the closed form, exactly, whatever the width effects (the Poisson coupling with the clamped root, the transverse
   acceleration `Omega^2 y`). The limit `1e-8` of the acceptance is rounding of a 1230-unknown solve. Registered strip: `L = 0.1
   m` (radial, `x in [0, L]`), `W = 0.02 m` (`y in [-W/2, W/2]`), `t = 2 mm` (four plies of `0.5 mm`, `[0/90/90/0]`, 0 along
   `x`), 40 x 4 Shell4 in the plane `z = 0`, axis `z` through the origin (on the root's mid-point), the root `x = 0` clamped in
   its six unknowns, `theta_x = 0` at every node (the reading of a4; no effect on the in-plane static state). `m0 = 2.8
   kg/m^2`; at `Omega = omega_0` the root value `m0 Omega^2 L^2/2 = 4008.1 N/m` (stress `2.0 MPa`, linear). The FE values are
   taken from `shell::stresses` of the static solve of the model with the centrifugal load at `Omega = omega_0`.
3. **The Southwell coefficient and the flap frequency** (a4) (b1). Strip of arithmetic 2: `D11 = 6.4844024 N m`, `D12 =
   0.9814231`, `D22 = 3.3298283`, `D66 = 1.0666667` (probe), `omega_0 = beta_1^2 sqrt(D11/(m0 L^4)) = 535.0652 rad/s`, `f_0 =
   85.1583 Hz`. Southwell coefficient, method: the first-order term of `omega^2(Omega^2)` at `Omega = 0`, by the Hellmann-Feynman
   theorem the Rayleigh quotient of the centrifugal geometric stiffness on the exact rest mode, `S = int_0^1 (1 - xi^2)/2
   phi'^2 dxi / int_0^1 phi^2 dxi` with `phi` the clamped-free mode (`beta_1 = 1.8751040687`): **`S = 1.1933364`** (probe, 200-point
   Gauss; ADR 0403 arithmetic 4 gives `1.193336`, ADR 0473 R3 met it). The alternative suggested by the brief, Rayleigh with the
   static deflection shape of a uniform load `xi^2 (6 - 4 xi + xi^2)`, gives `S = 1.1731` (and `lambda_0^2 = 3.5301` against
   `3.5160`): not used, an approximation of the same quotient (the two Southwell forms differ by `4.6e-3` in frequency at
   `Omega = omega_0`). The Southwell form is itself the linearisation: an Euler-Bernoulli rotating cantilever (Hermite elements,
   the exact `N(r)`, probe, 40 and 200 elements agree to `1e-8`) lies below it by `3.4e-4` in frequency at `Omega = 0.5 omega_0`
   and by `3.0e-3` at `omega_0`. The shell's own differences from that beam: transverse shear `D11/(H11 L^2) = 2.6e-4` (`H11 =
   (5/6) 1.5e9 t`), rotary inertia `(t^2/12) int phi'^2/int phi^2/L^2 = 1.5e-4` (both lower the frequency, a fraction of each),
   the element-constant pre-stress (`1.3e-5` and `2.9e-5` in the 40-element beam, probe), MITC4 at 40 elements along (of order
   `1e-4` for the first mode), and the width effects of the transverse acceleration (`N_yy` of order `m0 Omega^2 W^2/8`, its
   geometric stiffness on a `w` that is uniform across: negligible). Expected `|f/f_S - 1|` about `5e-4` at `0.5 omega_0` and
   `3.5e-3` at `omega_0`; the limit `2e-2` is the acceptance's. Southwell values: `f_S = f_0 sqrt(1 + S/4) = 97.03 Hz` and
   `f_0 sqrt(1 + S) = 126.12 Hz`. Negative control NF1: without the stress stiffening the flap frequency at `omega_0` is `32`
   percent below `f_S` (probe: `sqrt(1/(1 + S)) - 1 = -0.325`; the spin softening of the flap is only the rotary part).
   Why `theta_x = 0`: the narrow free strip's bending stiffness `D11 - D12^2/D22 = 6.1951 = 0.9554 D11` (probe) would put the rest
   frequency `2.3` percent below the D11 form, outside the acceptance, and a strip of `W^2/(L t) = 2` lies between the two limits
   by an amount not computed here.
4. **The orthotropic plate's critical mode** (a5) (b1). Specially orthotropic SS plate under `N_x` (Kirchhoff, first
   principles, the double sine `w = sin(m pi x/a) sin(n pi y/b)`): `N_x = pi^2 [D11 (m/a)^2 + 2 (D12 + 2 D66) (n/b)^2 + D22 n^4 a^2
   /(m^2 b^4)]`, least at `n = 1`. With `a = b = 1`, `t = 0.01`: `D11 = 810.5503`, `D22 = 416.2285`, `D12 = 122.6779`, `D66 =
   133.3333 N m`, `2 (D12 + 2 D66) = 778.0225`: `m = 1` gives `pi^2 2005.468 = 19793.18 N/m`, `m = 2` gives `pi^2 (4 D11 + 778.02
   + D22/4) = 40711.60 N/m`, `m = 3` `80140.1 N/m` (probe); `(m, n) = (1, 2)`: `pi^2 (D11 + 4 (778.02) + 16 D22) ~ 1.04e5`. The
   switch from `m = 1` to `m = 2` is at `a/b = (4 D11/D22)^(1/4)` (`m^2 (m+1)^2 = (a/b)^4 D22/D11` at `m = 1`, `(a/b)^4 = 4 D11/D22
   = 7.79`, `a/b = 1.67`): the square plate is well inside the `m = 1` range. Estimate of the miss: ADR 0473 P1 measured `3.30e-3`
   for the isotropic plate of the same mesh and supports; the Mindlin lowering `pi^2 den/(4 H) = 4.0e-4` (probe, `H = (5/6) 1.5e9
   t`): expected below `1e-2`. Negative control NBK: the form with `D12 + D66` (`17161.3 N/m`) is `15` percent away. The second
   factor against `m = 2` (supplementary BK2): the mesh has 8 elements per half-wave, about four times the first mode's error,
   expected below `2e-2`.
5. **The spin-softening identities** (D3). (SS1) Over the three global axes `sum_k P_k = 2 I` and `sum_k R(e_k) = diag(2, 2,
   0)` (local; `R` depends on `a` through its local components, and an orthonormal triple stays orthonormal), so `sum_k
   M_perp(e_k) = 2 M` except the drilling rotary entries, where it is zero: on a flat plate in the plane `z = 0` the drilling
   rotation is the global `theta_z` (component 5), and the entries `(theta_z(i), theta_z(j))` of `M` are `m2 s`. (SS2) A rigid
   rotation `u = omega x (X - p)`, `theta = omega`, is in the element's fields (`X = x + z n`; `sum N_j omega x (x_j - p) = omega x
   (x - p)`, `z theta x n = z omega x n`), so `q^T M_perp q = int_A [m0 |P (omega x (x - p))|^2 + 2 m1 P (omega x (x - p)) .
   (omega x n) + m2 |P (omega x n)|^2] dA` exactly (the integrand biquadratic in the element coordinates: the 2 x 2 rule and the
   test's 3 x 3 rule both exact). Registered: the flat `1 x 0.5` plate (4 x 2) in the plane `z = 0` for SS1, the rotated plate
   of arithmetic 1 for SS2 with `omega = (0.3, -0.5, 0.8)`, `p = (0.1, 0.05, -0.1)`, the axis of arithmetic 1; `m0, m1, m2` of
   the two-density layup computed in the test from the plies.

## Checks (limits fixed now)

Every aggregated error is the maximum of finite values; a non-finite value is a miss of its check (b7). Relative errors are
`|x/x_ref - 1|` unless stated. Eigen-solves: `ResidualTest::Backward`, tolerance `1e-12` (the options of ADR 0473 for strips and
plates).

| id | clause | quantity | limit | estimate |
|---|---|---|---|---|
| BB1 | a2 | the warped 3 x 3 plate of ADR 0496 BT2 (one steel ply of `t = 0.01` at `0.3 rad`, the block material aluminium) against its `Shell { 0.01 }` steel twin: the nodal load vectors of gravity, of the centrifugal load (the axis and origin of arithmetic 1, `Omega = 50`) and of a nodal body-force field (`b_j` varying by node), and the static displacements and reactions under the three together | bits equal | D1: the route |
| BB2 | a2 | the same plate under an in-plane end load on `x = 1` (`-x`): the pre-stress values of `prestress_from_static` and the buckling factors and modes (`buckling::factors`, 2) | bits equal | D2 |
| BB3 | a2 | the same plate as a rotating part about the axis of arithmetic 1 through the origin of arithmetic 1: `unit_prestress`, `unit_geometric_stiffness` values, `spin_softening` values, and the frequencies of `modes(100 rad/s)` (4 modes) | bits equal | D2, D3 |
| G1 | a1 | the gravity load vector of the two-density layup on the rotated plate against `M d_g` (`M = consistent`) | max abs difference / max abs entry `<= 1e-12` | arithmetic 1: rounding |
| G2 | a1 | its resultant force against `m0 A g` and its resultant moment about the origin `sum (x_i x F_i + M_i)` against `m0 A (x_cm x g)` | max component difference / norm of the reference, each `<= 1e-12` | arithmetic 1 |
| C1 | a1 | the centrifugal load vector of that layup plate against `M d_c` | as G1, `<= 1e-12` | arithmetic 1 |
| C2 | a1 | its resultant force against `Omega^2 m0 A P (c - p)` | as G2, `<= 1e-12` | arithmetic 1 |
| NB1 | (a2) | the uniform nodal body force on that layup plate: resultant force against `t_total A b`, resultant moment against `t_total A (c x b)` | as G2, `<= 1e-12` | arithmetic 1 |
| MF1 | a3 | the strip at `Omega = omega_0`: per column the mean of the element-centre `N_xx` against `m0 Omega^2 (L^2 - r_c^2 - h^2/12)/2`, the largest difference over the columns over `m0 Omega^2 L^2/2` | `<= 1e-8` | arithmetic 2: rounding |
| MF2 | (a3) | the strip: the sum of the reactions along `x` against `-m0 W Omega^2 L^2/2` | relative `<= 1e-10` | equilibrium |
| F0 | (a4) | the strip's rest flap frequency (`RotatingPart::modes(0)`) against `f_0 = 85.1583 Hz` | `<= 1e-2` | arithmetic 3: below `2e-3` |
| F1 | a4 | the flap frequency at `Omega = 0.5 omega_0` and at `omega_0` against the Southwell form `f_0 sqrt(1 + S (Omega/omega_0)^2)`, `S = 1.1933364` | `<= 2e-2` each | arithmetic 3: `5e-4`, `3.5e-3` |
| F2 | (a4) | the first-order slope `RotatingPart::slope(phi_0)` of the rest flap mode (`d omega^2/d Omega^2`, stiffening minus softening) against `S`: `|slope / S - 1|` | `<= 1e-2` | arithmetic 3: below `2e-3` |
| SS1 | (D3) | the flat two-density layup plate: `sum_k M_perp(e_k)` against `2 M` with the drilling rotary entries zero | max abs difference / max abs entry of `M` `<= 1e-12` | arithmetic 5 |
| SS2 | (D3) | the rotated layup plate: `q^T M_perp q` of the rigid rotation against the integral of arithmetic 5 | relative `<= 1e-12` | arithmetic 5 |
| BK1 | a5 | the SS `[0/90]s` plate, 16 x 16, `a = b = 1`, `t = 0.01`: `lambda N_ref` against `pi^2 (D11 + 2 (D12 + 2 D66) + D22)/a^2 = 19793.18 N/m` | `<= 2e-2` | arithmetic 4: below `1e-2` |
| BK2 | (a5) | its second factor against the `m = 2` load `40711.60 N/m` | `<= 5e-2` | arithmetic 4: below `2e-2` |
| BK3 | (a5) | the first buckling mode: `w` at the interior nodes of the line `y = a/2` of one sign, and at those of `x = a/2` of one sign (one half-wave each way, `m = n = 1`) | holds | arithmetic 4 |
| RX3 | (b3) | `nonlinear::solve` of a Shell4 Layup model (two orthotropic plies) still refused naming `laminate::FOLLOW_UP` | refused | D4 |
| RX4 | D9 | the copy `rx2_paths_that_refuse_a_layup_after_f591`: gravity on the layup solves (`Ok`), the buckling of the layup gives `Ok`, `nonlinear::solve` refused as before | as stated | D9 |

Negative controls (each must show the check is not vacuous):

| id | control | expected |
|---|---|---|
| NG | G2 with the moment reference without the `m1` term (`m0 A (c x g)`) | the measured moment differs from it by more than `1e-6` (relative as G2) |
| NF1 | F1 at `omega_0` with `stress_stiffening(false)` | `|f/f_S - 1| > 0.2` (probe `0.325`) |
| NBK | BK1 against the form with `D12 + D66` | `> 5e-2` away (probe `15.3` percent) |
| NS | SS2 with the coupling term `2 m1 ...` left out of the reference | differs by more than `1e-9` relative |
| B0 | the bit record of D8 | the base digests, unchanged |
| B0' | ADR 0496's `sa_laminate_default_bits.rs` | unchanged |
| B1 | `baseline_hashes` of `rcfd-api` | passes unchanged |

NS: the coupling term of SS2 is of order `2 m1 |omega|^2 |x - p|/(m0 |omega|^2 |x - p|^2) ~ 2 (1.7e-4)/0.7 = 5e-4` of the total
(by hand), far above the control's `1e-9`.

## Planned mutations (on a scratch copy, one at a time)

| id | mutation | expected to be caught by |
|---|---|---|
| MU1 | the layup body load's moment with `-m1` | G1, G2 |
| MU2 | the layup body load's force with `rho_block t_total` for `m0` | G1, G2, C1, C2 |
| MU3 | the D1 route removed (one isotropic ply through the layup body load) | BB1 |
| MU4 | the spin-softening coupling `m1` dropped | SS1, SS2 |
| MU5 | the spin-softening coupling without the projection (`W` for `P W`) | SS1, SS2 |
| MU6 | `membrane_thickness` of a layup giving the first ply's thickness in `element_geometric_stiffness` only | BK1, F1, F2 |
| MU7 | the nodal body force on a layup with the first ply's thickness | NB1 |
| MU8 | the centrifugal acceleration of a layup evaluated with the block's density (`rule(rho[e])` times `m0`) | C1, C2, MF1 |
| MU9 | the D3 route removed for the spin softening (one isotropic ply through the layup function) | BB3 |
| MU10 | the spin-softening rotary term with the homogeneous `rho_block t^3/12` for `m2` | SS1, SS2 |

## Decision rule

- **done**: every check of the table met, every negative control as expected, every planned mutation caught by at least one
  check, B0, B0' and B1 unchanged, the existing tests of `rcfd-fem` passing (with the records of D9), fmt and clippy clean.
- **partial**: B0, B0', B1 unchanged and BB1 to BB3 met, with any other miss diagnosed and recorded as a `*_known_miss` test.
- **todo**: otherwise (in particular any change of the default bits).

## D8. The bit record (B0, b6)

`crates/rcfd-fem/tests/sa_laminate_body_default_bits.rs`, written and run on the base after this commit and before any library
code, its digests printed by that run and written into the file in the same commit (the pattern of ADR 0496 D9), FNV-1a 64 over
the bits of: (1) the gravity, centrifugal and nodal body-force load vectors of the warped Shell4 plate (a `Shell` block) and
(2) of its Shell3 twin, (3) the same three of a Hex8 box, (4) the gravity and centrifugal load vectors of a Beam2 cantilever,
(5) the static solve of the Shell4 plate under the three loads, (6) `prestress_from_static` values and the buckling factors of
the Shell4 plate and of the Shell3 plate under an in-plane end load, (7) `RotatingPart` of the Shell4 plate and of the Shell3
plate (the unit pre-stress, `K_sigma,1`, `M_perp` values and the frequencies at a speed), (8) `spin_softening_mass` of the Hex8
box. B0 must give the same digests after the change.

## D9. Existing tests superseded by design (ADR 0291 records and copies)

`sa_laminate.rs::rx2_paths_that_refuse_a_layup` (ADR 0496 RX2) fails by design: gravity and the buckling of a layup are taken.
It gets `#[ignore = "record: ..."]` (the attribute only; the body unchanged) and the copy `rx2_paths_that_refuse_a_layup_after_f591`
with the same models: gravity `solve_static` is `Ok`; the buckling chain (`solve_static`, `prestress_from_static`, `factors`)
is `Ok`; `nonlinear::solve` refused naming `laminate::FOLLOW_UP` as before (RX4). If the whole-crate run shows another existing
test that fails by design, it is handled the same way by an amendment written before its corrected run.

## Limits (not covered)

- The body force is the acceleration at the mid-surface point times the mass moments `m0`, `m1`: its variation through the
  thickness (the centrifugal `Omega^2 P n z`, giving a force `m1 Omega^2 P n` and a rotary term with `m2`) is not part of the
  load, as for the homogeneous shell (relative size `t/r`).
- The geometric stiffness is that of the membrane resultants on the translations (ADR 0473 D2): no `t^2/12`-type rotation terms,
  no transverse-shear pre-stress; a layup's pre-stress is its centre resultant per element.
- The spin-softening mass has no drilling term (ADR 0473 D3); the Coriolis, whirl and cyclic-sector paths are not part of this
  item (they keep what they take).
- Shell3 still refuses a layup; the corotated Newton path still refuses it (b3); the transient integrator refuses shells as
  before.
- The flap check is on a strip in cylindrical bending (`theta_x = 0`); a free strip's anticlastic stiffness is not checked.
- No case-file key (b4).

## Open questions taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. **The thickness of a layup's pre-stress.** Options: (a) store `(1/t_total) N` and take `N` back with `t_total`; (b) change
   `Prestress` to hold resultants. Taken (a): no change of the pre-stress layout; the geometric stiffness depends on `N` only.
2. **The body force through the thickness.** Options: (a) the mid-surface acceleration with `m0`, `m1` (the acceptance's form);
   (b) also the `z` variation of the centrifugal acceleration. Taken (a); (b) a follow-up only if a thick hub-region layup needs it.
3. **The Southwell coefficient.** Taken: the exact first-order value `1.1933364` (rest-mode quotient), not the static-shape
   Rayleigh value `1.1731`.

Not taken (open, for the maintainer): **the case-file key** (b4; it changes the case schema and its golden). Proposal, as ADR
0496 open question 1 (b): a `[[layup]]` table referenced by name from a shell block; gravity, `[[centrifugal]]` and the rotating
analysis then take the block as they take a `Shell` block, with no new key of their own. Recommendation: that form.
