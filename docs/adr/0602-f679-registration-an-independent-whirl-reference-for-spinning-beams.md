# ADR 0602: F-679 registration: an independent whirl reference for spinning beams (the free spinning ring of Beam2)

Status: **REGISTERED 2026-11-15, written before any code of F-679 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-679 was written. The files under "What was read" were read, two
entry-path probes were run on the unchanged library (disclosed below: they printed only `Ok` or the error of each call, no
number; the probe file was deleted and never committed), and the reference and estimate numbers below were computed by short
scripts that make no rcfd call (`f679-scratch/reference.py`, `polygon.py`, `tables.py`, `predsum.py`, `mutsum.py`; disclosed).
Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-679 (dep F-675), the follow-up proposed by
[ADR 0598](0598-f675-registration-rotating-paths-on-a-cyclic-sector-of-beams.md) (its "Not covered": an independent whirl
reference for beams; its mutation X5, the global axis components in place of the local ones, escaped the sector-against-ring
check because both stay cyclically symmetric). Number 0602 given by the lead. Branch `f679-work` from the local main tip
43092983 (which holds F-672 `CyclicOptions::beams` and F-675's beam Coriolis matrix and `RotatingOptions::beams`), worktree
`w49i-work/f679`, scratch `w49i-work/f679-scratch`. This is a test-only item unless a defect is found.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored `*_known_miss`
test with the limit intact and the measured number); each registered run made once; existing tests not edited; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no
unsafe code; clean-room. The roadmap files are not edited (the entries go in "For the lead"). Every choice below that the
maintainer would normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the
recommendation).

Two lessons of ADR 0573 are applied before the run: (1) the sign of the whirl number and the wave sense of each root are derived
here for the forward and the backward root separately (D3.4, arithmetic 1), not carried over; (2) the no-Coriolis negative
control's limit is the dense solver's rounding `10 n_q eps omega_max / omega_lo`, not a fixed number. The lesson of ADR 0598
(X5) is applied by an oblique axis whose global components differ from the local ones of every element (D2), and by a check
against numbers that do not come from the library (D3, D7).

## What F-679 asks

The acceptance, binding:

> Every limit not stated here is registered in the ADR before the run. The whirl frequencies and the forward and backward split of a spinning ring of Beam2 (a free circular ring rotating about its axis, in-plane and out-of-plane modes per nodal diameter) from RotatingSector::whirl_modes with CyclicOptions::beams against a closed form derived or cited in the ADR, at three speeds up to half the first frequency: within the registered discretisation error at two meshes, the observed order within 0.5 of the expected one, and the split of each nodal-diameter pair within 1e-2 relative; the defaults bit-identical.

Clauses: **b0** every limit registered here; **b1** a free circular ring of Beam2 rotating about its axis, in-plane and
out-of-plane modes per nodal diameter, from `RotatingSector::whirl_modes` with `CyclicOptions::beams`; **b2** against a closed
form derived (or cited) here; **b3** three speeds up to half the first frequency; **b4** the whirl frequencies within the
registered discretisation error at two meshes; **b5** the observed order within 0.5 of the expected one; **b6** the forward and
backward split of each nodal-diameter pair within `1e-2` relative; **b7** the defaults bit-identical.

## What was read

`AGENT_RULES.md`; the brief; ADR 0598 (all: D1 the opt-in routes, D2 the Coriolis matrix of the section kinematics `u + theta x
s`, `b = rho (I_z a_2 e_2 + I_y a_3 e_3)`, the X5 escape); ADR 0573 (all: the reference in the rotating frame, the whirl-number
signs, the split, order and no-Coriolis checks, the mutation-site lesson of MU3); ADR 0595 (the sector of beams, the oblique
axis, the cyclic option). Sources at 43092983: `beam.rs` (`frame`: `e_1` the chord, `e_3 = e_1 x v / |e_1 x v|`, `e_2 = e_3 x
e_1`; `plane_stiffness`, `plane_shape_coefficients`, `plane_functions`, `plane_mass`, `local_stiffness`, `local_mass`: rotary
inertia `rho I_z` with `theta_z`, `rho I_y` with `theta_y`, `rho (I_y + I_z)` in torsion, the x-z plane with `psi = -theta_y`;
`local_geometric_stiffness`: `N int (u'^2 + v'^2 + w'^2) + N (I_y + I_z) / A int theta_x'^2`; `axial_force`: `E A e_1 . (u_1 -
u_0) / L`; `field_rows`; `local_spin_softening_mass`: `M - int (rho A (a . u)^2 + rho I_z (theta . (e_2 x a))^2 + rho I_y (theta
. (e_3 x a))^2)`; `local_coriolis`, `element_coriolis`; `centrifugal_load_vector`), `geometric_struct.rs`
(`prestress_from_displacement`: one point per beam, `(N / A) e_1 e_1^T`), `geometric.rs` (`Prestress`), `rotating.rs`
(`RotatingPart::new`: the static solve of the centrifugal load at 1 rad/s with the model's constraints; `with_unit_prestress`;
`check_model`: a free body is allowed; `operator`; `RotatingSector::new`, `with_unit_prestress`), `rotating/whirl.rs`
(`Whirl::of`, `WHIRL_TOLERANCE = 1e-6`, `WhirlMode` with `whirl` and `wave`, `whirl_number`, the sector `whirl_at`: `G = 2 Omega
C`, the dense solver, the wave sense `tau = 2 (a_r^T M b_i - a_i^T M b_r) / (...)`), `solve.rs` (`solve_static`, the
transformation path for linear and cyclic constraints), `mpc.rs` (`uses_transformation`), `model.rs` (`Constraint::linear`,
`LinearTerm`), the tests `sa_rotating_cyclic_beams_f675.rs` and `sa_disc_whirl_shells.rs` (helpers copied where used, not
edited).

**Entry-path probes (disclosed).** A temporary test file (`tests/zz_probe_f679.rs`, deleted, never committed) built the free
ring sector of D2 (`N = 24`, 4 elements, `CyclicOptions::new().shells(true).beams(true)`) and printed only `Ok` or the error of
each call: (1) `RotatingSector::new` with `coriolis(true)` and the default stress stiffening: `Err(SingularStiffness { node: 0,
component: 2, .. })` (the static solve of the centrifugal load of a free ring meets the two rigid motions of index 0); with
`stress_stiffening(false)`: `Ok`, and `whirl_modes(2, 10)`, `whirl_modes(3, 10)`: `Ok(Modes)`. (2) A copy of the sector with
two `Constraint::linear` at its node 1 (the tangential and the axial components zero, D4) and `Load::centrifugal(axis, origin,
1)`: `solve_static` `Ok`, `Prestress::from_static` `Ok`, `RotatingSector::with_unit_prestress` on the free sector `Ok`,
`whirl_modes(2, 10)` and `(3, 10)` `Ok(Modes)`; a copy with nodal springs instead: `solve_static` `Ok` (not used). No frequency
or other number was printed.

## Design

### D1. Why the free spinning ring, and what it tests

A free circular ring spinning about its own axis is the classical gyroscopic ring (Bryan 1890, cited from memory): in the frame
turning with it the in-plane flexural wave of `n` nodal diameters splits into a forward and a backward wave by the Coriolis force
on the **translations** (the block `rho A [a x]` of ADR 0598 D2), a split `4 n Omega / (n^2 + 1)` in the thin inextensional limit
(derived in D3.3), independent of the stiffness. The out-of-plane wave (translation along the axis, twist and out-of-plane
bending rotation) is untouched by the translation block (`a x u = 0` for `u` along `a`) and splits only by the **section's rotary
block** `[b x]` with `b = rho I_z a_2 e_2`, which couples the twist `theta_1` to the out-of-plane bending rotation `theta_3`
(D3.1): a small split, `~ 1e-3` of the frequency here (arithmetic 1), carried by that block alone. So the two families test the
two blocks of the beam Coriolis matrix separately, against a reference that is not the library, which is what ADR 0598's
sector-against-ring check could not do (X5).

The section is placed with `e_2` along the axis at every element (orientation `v = a`, D2), so the axis's local components are
`(0, 1, 0)` while its global components are `(1, 2, 2) / 3`: a mutation that puts the global components in the local slot (X5)
changes the physics of every element.

### D2. Geometry, material and meshes (fixed now)

Axis `a = (1, 2, 2) / 3` through `O = (0.3, -0.2, 0.5)`, in-plane basis `e_a = (2, -1, 0) / sqrt 5`, `e_b = a x e_a` (as ADR 0573
and 0598). Ring radius `R = 0.5` m. Steel `E = 2e11` Pa, `nu = 0.3`, `rho = 7850` kg/m^3 (`G = E / (2 (1 + nu))`). Rectangular
section of width `b = 0.03` m along the section axis y (`e_2`, the ring's axis) and height `h = 0.02` m along z (`e_3`, radial):
`A = b h = 6e-4` m^2, `I_y = b h^3 / 12 = 2e-8` m^4 (bending in the ring's plane, rotation `theta_2` about the axis), `I_z = h b^3
/ 12 = 4.5e-8` m^4 (bending out of the plane), torsion constant `J = 4.7e-8` m^4 (an input; the value of a 3:2 rectangle,
`0.196 b h^3`, from memory; the reference uses the same input, so its accuracy does not matter), shear factors `5/6` and `5/6`:
`BeamSection::new(A, I_y, I_z, J, 5/6, 5/6)`, orientation `v = a` for the block.

`N = 24` sectors; a sector has `n_c` equal Beam2 elements between the nodes `O + R (cos phi_i e_a + sin phi_i e_b)`, `phi_i = 2 pi
i / (N n_c)`, `i = 0 .. n_c` (so `e_1` is the chord in the sense of increasing `phi`, `e_3 = e_1 x a` points outwards, `e_2 = a`).
Node sets `low = {0}`, `high = {n_c}`, element set `all`, `Constraint::cyclic_with("low", "high", a, O, 24,
CyclicOptions::new().shells(true).beams(true))`, no other constraint. Two meshes, a uniform refinement by 2: **coarse** `n_c =
2` (48 elements on the ring, element angle `Delta = 0.1309`), **fine** `n_c = 4` (96 elements). Harmonic indices **`k = 2, 3`**
(nodal diameters 2 and 3, each a pair; reduced unknowns of a pair `12 n_c`: 24 and 48).

### D3. The reference (first principles): the spinning Timoshenko ring in the rotating frame, exact per nodal diameter

**D3.1 Kinematics and energies.** Arc length `s = R phi`, the moving frame `e_1 = e_phi`, `e_2 = a`, `e_3 = e_r` (right-handed:
`e_phi x a = e_r`), `de_phi / ds = -e_r / R`, `de_r / ds = e_phi / R`. The section moves by `u + theta x s_` (`s_ = y e_2 + z
e_3`), `u = u_1 e_1 + u_2 e_2 + u_3 e_3`, `theta = theta_1 e_1 + theta_2 e_2 + theta_3 e_3`. The derivatives along the ring in the
moving frame: `(du/ds)_1 = u_1' + u_3 / R`, `(du/ds)_2 = u_2'`, `(du/ds)_3 = u_3' - u_1 / R`, and the same for `theta`. The
strains of the shear-flexible curved beam, with the sign conventions of `local_stiffness` (x-y plane `gamma = v' - theta_z`, x-z
plane `gamma = w' + theta_y`): `eps = (du/ds)_1`, `gamma_2 = (du/ds)_2 - theta_3`, `gamma_3 = (du/ds)_3 + theta_2`, `kappa_i =
(dtheta/ds)_i`. Per unit length:

- strain energy `1/2 [E A eps^2 + k_y G A gamma_2^2 + k_z G A gamma_3^2 + G J kappa_1^2 + E I_y kappa_2^2 + E I_z kappa_3^2]`;
- kinetic energy `1/2 [rho A |u_t|^2 + rho (I_y + I_z) theta_1t^2 + rho I_y theta_2t^2 + rho I_z theta_3t^2]`;
- spin softening (ADR 0473 D3, the energy `1/2 Omega^2 int rho |a x w|^2`, `w = u + theta x s_`): with `a = e_2`, `a . (theta x
  s_) = -z theta_1`, so `M_perp` per unit length is `rho A (u_1^2 + u_3^2) + rho (I_z theta_1^2 + I_y theta_2^2 + I_z theta_3^2)`;
- Coriolis per unit speed (ADR 0598 D2, `q_t^T C q = int rho w_t . (a x w)`): `a x u = u_3 e_1 - u_1 e_3` gives `rho A (u_1t u_3 -
  u_3t u_1)`; `b = int rho s_ (s_ . a) = rho I_z e_2`, `b x theta = rho I_z (theta_3 e_1 - theta_1 e_3)` gives `rho I_z (theta_1t
  theta_3 - theta_3t theta_1)`;
- the pre-stress: a free ring under its centrifugal load `rho A Omega^2 R` (radial, on the undeformed geometry) has the hoop force
  `N = rho A Omega^2 R^2` (equilibrium of half the ring; the extensible linear ring expands uniformly, no bending), and the
  geometric stiffness of the axial force of ADR 0473 D1 (the derivative of the whole translation, and Wagner's torsion term) in the
  moving frame: `1/2 N [(du/ds)_1^2 + (du/ds)_2^2 + (du/ds)_3^2 + ((I_y + I_z) / A) (dtheta/ds)_1^2]`.

The in-plane fields `(u_1, u_3, theta_2)` and the out-of-plane fields `(u_2, theta_1, theta_3)` are uncoupled in every form.
Consistency: the rigid rotation about the axis (`u_1 = c`) has zero strain, geometric energy `N c^2 / (2 R^2) = rho A Omega^2 c^2 /
2` and spin softening `-rho A Omega^2 c^2 / 2`: zero in sum, as it must be for a free ring at equilibrium.

**D3.2 The exact solution per nodal diameter.** With `q = Q e^{i (n phi + lambda t)}` (`d/ds -> i n / R`) every form becomes a
Hermitian 6 x 6 matrix per unit length (`K`, `K_sigma` at 1 rad/s, `M`, `M_perp`) and `C` a real skew one, and the equations of
motion `M q_tt + 2 Omega C q_t + (K + Omega^2 (s K_sigma - c M_perp)) q = 0` (the operator of `RotatingPart::operator` and `G = 2
Omega C` of `whirl_at`) give

`H(lambda) Q = 0`, `H(lambda) = K_e - lambda^2 M + i lambda 2 Omega C`, `K_e = K + Omega^2 (s K_sigma - c M_perp)`,

a 3 x 3 Hermitian gyroscopic problem per family; `det H(lambda)` is real, a polynomial of degree 6 in `lambda` whose six real roots
are the whirl frequencies (closed form: the roots of the characteristic polynomial of the ring, no discretisation). `lambda > 0`
is a wave travelling in `-phi` (against the rotation: **backward**), `lambda < 0` one travelling in `+phi` (**forward**); the
rotating-frame frequency is `|lambda|`. `omega_f` and `omega_b` are the smallest `|lambda|` of each sign of the family (the
flexural branch; the extensional and shear branches are above `13 omega_b`, arithmetic 3).

**D3.3 The thin limit (derived; the published forms cited from memory, to verify).** In-plane, inextensional (`eps = 0`: `u_3 =
-R u_1'`), shear-rigid (`theta_2 = -(du/ds)_3`), without rotary inertia, with `u_1 = alpha(t) sin n phi + beta(t) cos n phi`: the
curvature is `kappa_2 = -(u_1''' R + u_1' / R)`, the geometric term `(N / 2R^2) (d^2 u_1 / d phi^2 + u_1)^2`; averaged over
`phi` the Lagrangian is, per `rho A / 4`, `(1 + n^2)(alpha_t^2 + beta_t^2) + 4 n Omega (alpha_t beta - beta_t alpha) - k (alpha^2 +
beta^2)`, `k = (1 + n^2) omega_0^2 + Omega^2 [(n^2 - 1)^2 - (n^2 + 1)]`, `omega_0^2 = E I_y n^2 (n^2 - 1)^2 / (rho A R^4 (n^2 +
1))`. Its roots are

`omega_{f,b} = [sqrt((1 + n^2)^2 omega_0^2 + n^2 (n^2 - 1)^2 Omega^2) -/+ 2 n Omega] / (1 + n^2)`,

the forward root the lower, the split `4 n Omega / (n^2 + 1)`. This is the published form of the spinning ring in the rotating
frame (as given, from memory, by Huang and Soedel 1987 and Endo et al. 1984; to verify; the derivation above is what is used).
Out-of-plane at rest, Euler-Bernoulli with the twist condensed (no rotary inertia): `omega^2 = (E I_z / (rho A R^4)) n^2 (n^2 -
1)^2 / (n^2 + E I_z / (G J))` (the classical out-of-plane ring formula, cited from memory, to verify; not derived here, it enters
only the check RF2, which verifies it numerically as the thin limit of D3.2). RF2 checks that D3.2 tends to both forms as the
section shrinks (`b, h -> t b, t h`, the speed scaled with the frequencies, `Omega = t Omega_3`).

**D3.4 The whirl number and the wave sense of the reference (lesson 1).** The library writes a mode `u = Re(phi e^{i omega_L t})`,
`omega_L > 0`, `kappa = Im(phi^H C phi) / phi^H M phi` (`> 0`: the orbit turns in the sense of the rotation). A backward root
`lambda > 0` is already of that form, `phi = Q e^{i n phi}`: `kappa_b = Im(Q^H C Q) / Q^H M Q`. A forward root `lambda < 0` is
`u = Re(conj(Q) e^{-i n phi} e^{i |lambda| t})`, `phi = conj(Q) e^{-i n phi}`: `kappa_f = -Im(Q^H C Q) / Q^H M Q`, each at its
own null vector `Q`. The wave sense: `tau = +1` forward, `-1` backward (the library's `wave`: `+1` travelling in the sense of the
rotation). A hand check on the thin in-plane forward wave (`u_1 = V sin(n phi - omega t)`, `u_3 = -n V cos(n phi - omega t)`): the
material point at `phi = 0` passes from `-e_r` (`t = 0`) to `-e_phi` (`omega t = pi / 2`), a positive turn about `a`, so
`kappa_f > 0`, as the formula gives (arithmetic 1: `kappa_f = +0.8002`, `kappa_b = -0.8001` at `n = 2`, near `+-2n / (n^2 + 1)
= +-0.8`).

**D3.5 Numerics (the Rust reference of the test).** Per family and root sign: `det H(lambda)` (the 3 x 3 complex determinant) is
positive at `lambda = 0` (`K_e` positive definite, arithmetic 2) and changes sign at the first root; the test scans `|lambda|`
upwards in steps of `omega_s / 1000`, `omega_s = sqrt(E I_y / (rho A R^4))`, to the first sign change and bisects to the
adjacent floating-point numbers. The null vector is the (bilinear) cross product of the two rows of `H` with the largest one.
RF1 checks the roots against the independent eigen-solution of the scripts (arithmetic 1: `numpy.linalg.eig` of the 6 x 6
linearisation, refined by the stationary root of the Hermitian quadratic quotient).

### D4. The rigid-body modes and the pre-stress of a free ring

A free ring has six rigid motions. On the cyclic sector they live in two harmonic indices: `k = 0` the translation along the axis
and the rotation about it; `k = 1` the two in-plane translations and the two rotations about in-plane axes. **The checks use `k
= 2, 3` only**, whose reduced problems contain no rigid motion (`K_eff` positive definite at every registered speed, arithmetic
2); `k = 0, 1` are not solved.

The pre-stress is the static solve of the centrifugal load at 1 rad/s under the index-0 rows, which for a free ring is singular
(probe 1: `RotatingSector::new` refuses with `SingularStiffness`; NC0). So for set F the pre-stress comes from a **copy** of the
sector with two linear constraints at its node 1, `t . u = 0` and `a . u = 0` (`t = -sin phi_1 e_a + cos phi_1 e_b`, the tangent at
node 1), and `Load::centrifugal(a, O, 1)`: `solve_static` and `Prestress::from_static` (the library's own path), passed to
`RotatingSector::with_unit_prestress` of the **free** sector. The two constraints remove exactly the two rigid motions of index
0, and they carry no reaction: the centrifugal load is orthogonal to both (it has no axial component and no moment about the
axis), so the singular free problem has solutions, and the one with the tangential and axial displacement of node 1 zero is the
constrained solution. Its stress is the free ring's. The check PR compares the hoop force of every element with the independent
polygon model's (D7), which solves the free problem by least squares.

### D5. The effect sets and the speeds

- **Set S** (Coriolis and spin softening, no stress stiffening): `RotatingSector::new(&free, &RotatingOptions::new(3)
  .coriolis(true).stress_stiffening(false))` (no static solve). Reference `s = 0`, `c = 1`.
- **Set F** (all three): `RotatingSector::with_unit_prestress(&free, prestress, &RotatingOptions::new(3).coriolis(true))`, the
  pre-stress of D4. Reference `s = 1`, `c = 1`, `N = rho A R^2` at 1 rad/s.

`omega_1` the lowest reference frequency at rest over `n = 2 .. 5` and both families (arithmetic 1: the in-plane `n = 2`,
`312.4344` rad/s); `Omega_j = (j / 6) omega_1`, `j = 1, 2, 3` (up to half the first frequency, b3). The dense whirl solver (the
default), 3 modes per harmonic, so 6 whirl modes of each pair.

### D6. The measures

At each mesh, set, speed and `k`: `whirl_modes(k, Omega_j)`, its modes ascending. The four lowest are sorted into families by the
axial fraction of their translations, `f_ax = sum (u . a)^2 / sum |u|^2` over every node, both halves of the doubled pair, real
and imaginary parts: in-plane if `f_ax < 0.5`, out-of-plane otherwise; exactly two of each (else a set-up failure). In each
family `lo < hi`, against the reference `omega_f < omega_b` (D3, the forward root the lower in both families, arithmetic 1):

- `e_lo = omega_lo / omega_f - 1`, `e_hi = omega_hi / omega_b - 1`;
- the split `d = (omega_hi - omega_lo) / (omega_b - omega_f) - 1`;
- the observed order `p = log2(|e_coarse| / |e_fine|)` of each frequency;
- the whirl numbers `kappa_lo / kappa_f - 1`, `kappa_hi / kappa_b - 1` (D3.4);
- direction and wave sense: `lo` `Whirl::Forward` with `tau >= 0.99`, `hi` `Whirl::Backward` with `tau <= -0.99`; the fifth mode
  `>= 5` times the fourth;
- family purity: `f_ax <= 1e-6` for the in-plane pair, `1 - f_ax <= 1e-6` for the out-of-plane pair.

### D7. The discretisation error expected (an independent polygon model, disclosed)

The library's ring is a polygon of straight elements, each the exact prismatic Timoshenko beam with the interdependent functions
(ADR 0417), whose interpolation error for a straight member is of order `h^4`. The faceting is of order `Delta^2`: the chord is
shorter than the arc by `Delta^2 / 24`, the centroid of the element's centrifugal load is at `R (1 - Delta^2 / 24)`, and the
polygon's static hoop force is `rho A R^2 (1 - Delta^2 / 6 + ...)` (arithmetic 4: `-2.8517e-3` at `Delta = 0.1309`, `Delta^2 / 6
= 2.856e-3`). So the expected order is **2**.

The constant is not derived by hand. It is estimated by `f679-scratch/polygon.py`, an independent implementation in numpy (no
rcfd call) of the same discretisation: straight two-node elements in the frame of ADR 0417, the interdependent deflection and
rotation functions written from their textbook form (`mu = 1 / (1 + Phi)`, deflection `mu (2 xi^3 - 3 xi^2 - Phi xi + 1 + Phi)`,
...; constant shear strain checked), stiffness, consistent mass with rotary inertia, the geometric stiffness of D3.1's terms per
element, the spin-softening mass and the Coriolis matrix of the section kinematics, the consistent centrifugal load, the axial
force `E A e_1 . (u_1 - u_0) / L`, assembled on one sector and reduced per nodal diameter by the Bloch relation (the last node is
`e^{i n alpha} R_alpha` times the first), the Hermitian quadratic problem solved by `numpy.linalg.eig` of its linearisation and each
root refined by inverse iteration with the stationary quadratic quotient. It takes the conventions (the frame, the sign of `psi`,
the three terms of the geometric stiffness, the projections of the spin-softening mass, `b`) from the documented formulas of ADR
0417, 0473 and 0598, so it is an independent implementation of the library's discretisation, not an independent model; the
independent model is D3, and the polygon model converges to D3 at order `2.000` (arithmetic 4), which also verifies D3 by a
second route.

Its errors against D3 (arithmetic 4) are all positive, coarse `1.09e-3 .. 4.05e-3`, fine `2.72e-4 .. 1.01e-3`, the observed order
`1.9997 .. 2.0067`; the split errors are `<= 1.87e-3` (coarse, out-of-plane) and `<= 4.6e-5` (in-plane: the thin split is
independent of the stiffness). The WF limit is twice the largest predicted error of its mesh, set and family (room for an
implementation difference of the same order, a lumping or quadrature choice); the check AG compares the library with the polygon
model directly, where both should agree to rounding.

## Arithmetic (disclosed)

1. **Reference** (`reference.py`, D3.2 by `numpy.linalg.eig` of the 6 x 6 linearisation and the stationary quotient; residual
   `||H Q|| / (||K_e Q|| + lambda^2 ||M Q|| + |lambda| ||G Q||) <= 3.6e-12`). At rest (`n = 2 .. 5`): in-plane `312.434404`,
   `882.474371`, `1688.831244`, `2724.547661` rad/s; out-of-plane `410.909316`, `1231.202081`, `2415.696331`, `3940.662551`. So
   `omega_1 = 312.434403687127` rad/s and `Omega_j = 52.072400614521`, `104.144801229042`, `156.217201843564` rad/s. At the speeds,
   `omega_f`, `omega_b` (rad/s), `kappa_f`, `kappa_b`:

   | Set | `j` | `n` | in-plane `omega_f`, `omega_b`; `kappa_f`, `kappa_b` | out-of-plane `omega_f`, `omega_b`; `kappa_f`, `kappa_b` |
   |---|---|---|---|---|
   | S | 1 | 2 | 269.204298684032, 352.531499597897; +0.8001548, -0.8000634 | 410.767082988726, 411.034577829776; +2.568876e-3, -2.568104e-3 |
   | S | 1 | 3 | 850.223792368553, 912.752862883168; +0.6005203, -0.6002899 | 1230.943439746718, 1231.449944582607; +4.864924e-3, -4.862010e-3 |
   | S | 2 | 2 | 222.794891003891, 389.445463492684; +0.8001807, -0.8000006 | 410.607868193025, 411.142874238668; +2.569340e-3, -2.567796e-3 |
   | S | 2 | 3 | 815.995801011409, 941.051385989798; +0.6006223, -0.6001632 | 1230.674013215345, 1231.687040338849; +4.866465e-3, -4.860637e-3 |
   | S | 3 | 2 | 173.055752657706, 423.022037357161; +0.8001915, -0.7999285 | 410.431658892268, 411.234208872855; +2.569857e-3, -2.567541e-3 |
   | S | 3 | 3 | 779.771634045630, 967.348621613690; +0.6007147, -0.6000299 | 1230.393791281521, 1231.913375598148; +4.868062e-3, -4.859319e-3 |
   | F | 1 | 2 | 276.949899625503, 360.278694082952; +0.8001712, -0.8000775 | 423.726183298182, 423.993685518972; +2.568959e-3, -2.568162e-3 |
   | F | 1 | 3 | 859.995136358259, 922.526329372697; +0.6005419, -0.6003090 | 1240.769518818326, 1241.276037663302; +4.865071e-3, -4.862133e-3 |
   | F | 2 | 2 | 253.140917571968, 419.804237434476; +0.8002508, -0.8000530 | 460.264295641277, 460.799360726951; +2.569717e-3, -2.567986e-3 |
   | F | 2 | 3 | 854.577229582325, 979.649792988307; +0.6007139, -0.6002347 | 1269.522730521359, 1270.535869719386; +4.867095e-3, -4.861083e-3 |
   | F | 3 | 2 | 239.288208400434, 489.297510006706; +0.8003583, -0.8000371 | 515.489496260866, 516.292245512028; +2.570791e-3, -2.567883e-3 |
   | F | 3 | 3 | 864.831854926126, 1052.466135853155; +0.6009312, -0.6001801 | 1316.194508499327, 1317.714471078663; +4.869578e-3, -4.860225e-3 |

   In every case the forward root is the lower and `kappa_f > 0 > kappa_b` in both families. The in-plane split equals the thin
   `4 n Omega / (n^2 + 1)` within `4e-4` (at `n = 2`, `Omega_1`: `83.3288` against `83.3158`); the out-of-plane split is `0.27` to
   `1.52` rad/s, `6e-4` to `1.6e-3` of the frequency. In set S at `Omega_3`, `n = 2`, the in-plane backward root (`423.0`) lies
   above the out-of-plane pair (`410.4`, `411.2`): the families are interleaved, hence the family sort of D6.
2. **`K_eff` at `k = 2, 3`.** `det H(0) = det K_e > 0` in every case (the scan of D3.5 starts on the positive side), and the
   polygon model's reduced `K_eff` has no negative eigenvalue at any registered case (its eigen-solution has only real roots).
3. **Separation.** Polygon model: the fifth `|lambda|` over the fourth `13.7 .. 33.1` at every case.
4. **Polygon model** (`polygon.py`, `tables.py`, `predsum.py`; refined roots): hoop force over `rho A R^2` minus 1:
   `-2.851712875399e-3` coarse, `-7.136922537857e-4` fine, equal in every element to `1e-14`. Errors against D3, the range over
   the speeds, `n` and both members:

   | Mesh | Set | in-plane `e` | in-plane max `|d|`, `|kappa|` | out-of-plane `e` | out-of-plane max `|d|`, `|kappa|` |
   |---|---|---|---|---|---|
   | coarse | S | `+1.66e-3 .. +4.05e-3` | `4.6e-5`, `5.4e-5` | `+1.43e-3 .. +1.65e-3` | `1.87e-3`, `1.87e-3` |
   | coarse | F | `+1.17e-3 .. +2.45e-3` | `4.5e-5`, `5.2e-5` | `+1.09e-3 .. +1.56e-3` | `1.82e-3`, `1.82e-3` |
   | fine | S | `+4.15e-4 .. +1.01e-3` | `7.8e-6`, `9.7e-6` | `+3.57e-4 .. +4.13e-4` | `4.67e-4`, `4.68e-4` |
   | fine | F | `+2.93e-4 .. +6.13e-4` | `7.5e-6`, `9.3e-6` | `+2.72e-4 .. +3.89e-4` | `4.55e-4`, `4.55e-4` |

   Observed order of the 48 frequencies `1.9997 .. 2.0067`. The coarse mesh's four lowest `|lambda|` per case (the AG table):

   | Set | `j` | `k` | four lowest, rad/s |
   |---|---|---|---|
   | S | 1 | 2 | 269.877221329041, 353.204884278410, 411.444733264837, 411.712727520844 |
   | S | 1 | 3 | 852.133234015595, 914.665141646339, 1232.703398668859, 1233.210615539384 |
   | S | 2 | 2 | 223.478093166842, 390.129622237774, 411.285265493855, 411.821270205628 |
   | S | 2 | 3 | 817.910490572004, 942.971789402309, 1232.433616368229, 1233.448067226008 |
   | S | 3 | 2 | 173.757361015913, 411.108801175707, 411.912848746281, 423.725161879525 |
   | S | 3 | 3 | 781.696137103931, 969.281794492800, 1232.153039068244, 1233.674758148412 |
   | F | 1 | 2 | 277.606374132585, 360.935622735102, 424.386111234562, 424.654099882103 |
   | F | 1 | 3 | 861.883303688418, 924.417315795607, 1242.513846364662, 1243.021062763019 |
   | F | 2 | 2 | 253.762145009690, 420.426358337451, 460.879675777895, 461.415635628995 |
   | F | 2 | 3 | 856.410018758138, 981.488151845963, 1271.221898263074, 1272.236345340532 |
   | F | 3 | 2 | 239.861473143049, 489.872077918155, 516.050892824875, 516.854789006668 |
   | F | 3 | 3 | 866.582637948845, 1054.225102212552, 1317.824847221716, 1319.346553541724 |

   The unrefined `eig` values differ from the refined ones by `<= 2e-11` relative.
5. **Thin limit (RF2)** (`tables.py`): D3.2 against D3.3 at `t = 1/8` and `1/16`, the ratio of the two differences `4.000` in every
   case (order `t^2`), the Richardson value `(4 d(t/2) - d(t)) / 3`: in-plane (`Omega = t Omega_3`) `-2.6e-10`, `-1.1e-10` (`n =
   2`, forward, backward), `-7.3e-10`, `-5.8e-10` (`n = 3`); out-of-plane at rest `-5.3e-10` (`n = 2`), `-3.3e-9` (`n = 3`). So D3
   reproduces both thin-ring forms in the limit, which verifies D3.3 and the cited out-of-plane form.
6. **Rounding.** The largest frequency of the reduced problems (polygon model) `3.56e5` rad/s coarse, `6.01e5` fine. With ADR
   0573's `<= 10 n_q eps omega_max` the dense solver's relative error at `omega_lo >= 173` rad/s is `<= 1.1e-10` (coarse, `n_q =
   24`) and `<= 3.7e-10` (fine): far below the smallest predicted error (`2.7e-4`) and the smallest split (`0.27` rad/s, a relative
   floor `<= 2e-7` on `d`). NC1's limit `10 n_q eps omega_max / omega_lo` is `~ 1e-10`.
7. **Mutation effects on the polygon model** (`mutsum.py`, coarse mesh, the largest over the cases; the catchers registered
   below): X5 `e` up to `0.25`, in-plane split `0.50`, out-of-plane split `50`, out-of-plane `kappa_f < 0`; X1 out-of-plane split
   `-1.00`; X2 out-of-plane split `-0.555`; X3 every sign of `kappa` reversed, splits unchanged; X4 every split `-0.50`, in-plane `e`
   `0.25`; X6 set F `e` `0.27` (in-plane), `0.20` (out-of-plane); X7 set F out-of-plane frequencies by `~ 2e-5` (below WF, above AG);
   X8 out-of-plane `e` `7.3e-2`; X9 out-of-plane frequencies by `~ 1e-5` (below WF, above AG).

## Checks (limits fixed now)

| Id | Clause | Check | Limit | Estimate behind it |
|---|---|---|---|---|
| RF1 | b2 | the Rust reference (D3.5) against the table of arithmetic 1: `omega_1` and every `omega_f`, `omega_b` (48) | `<= 1e-9` relative | two methods, rounding `~ 1e-12` (arithmetic 1) |
| RF2 | b2 | the Rust reference's thin limit (arithmetic 5): the ratio of the differences at `t = 1/8`, `1/16` within `[3.9, 4.1]`, and the Richardson value against D3.3, in-plane `n = 2, 3` forward and backward at `Omega = t Omega_3`, out-of-plane `n = 2, 3` at rest | ratio as stated; Richardson `<= 1e-7` | arithmetic 5: `<= 3.3e-9` |
| RF3 | b2 | the Rust reference's whirl numbers against arithmetic 1 (48), and their signs `kappa_f > 0 > kappa_b` | `<= 1e-6` relative; signs as stated | null vector of a 3 x 3 at a root accurate to `~ 1e-12` relative |
| PR | b1 (set-up of set F) | the hoop force `A e_1 . S e_1` of every element of the library's pre-stress (D4) over `rho A R^2`, minus 1, against arithmetic 4 | `<= 1e-8` absolute | the static solve's rounding (condition `~ 1e8`): `<= 1e-8` |
| WF | b1, b4 | both meshes, both sets, the three speeds, `k = 2, 3`, both families: `abs(e_lo)`, `abs(e_hi)` | `<= 2 e_max(mesh, set, family)`: coarse S in `8.1e-3`, S out `3.3e-3`, F in `4.9e-3`, F out `3.1e-3`; fine S in `2.03e-3`, S out `8.3e-4`, F in `1.23e-3`, F out `7.8e-4` | D7, arithmetic 4 (twice the largest predicted error) |
| OD | b5 | the observed order `p` of each of the 48 frequencies | `abs(p - 2) <= 0.5` | D7: order 2 (predicted `1.9997 .. 2.0067`) |
| SP | b6 | the split of each pair (two sets, three speeds, `k = 2, 3`, two families), both meshes: `abs(d)` | `<= 1e-2` | the acceptance; predicted `<= 1.87e-3` (arithmetic 4) |
| KA | b1 | the same cases: `abs(kappa / kappa_ref - 1)` with D3.4's signs | `<= 1e-2` | as SP: the split is `2 kappa Omega` at first order; predicted `<= 1.87e-3` |
| DI | b1 | the same cases: `lo` `Forward` with `tau >= 0.99`, `hi` `Backward` with `tau <= -0.99`; the fifth mode `>= 5` times the fourth | as stated | D3.4; arithmetic 3 (`>= 13.7`) |
| TR | b1 | the same cases: family purity (D6) | `<= 1e-6` | the families are uncoupled; ADR 0573 measured `1.3e-8` for its analogue |
| AG | b4 (sharpened) | coarse mesh, both sets, the three speeds, `k = 2, 3`: the four lowest `omega` of the library against the table of arithmetic 4 | `<= 1e-8` relative | the same discretisation; both solvers' rounding `<= 1.1e-10` (arithmetic 6) |
| NC1 | b6 | set S **without** Coriolis (`coriolis(false)`), coarse, `Omega_3`, `k = 2, 3`: per family `omega_hi / omega_lo - 1` | `<= 10 n_q eps omega_max / omega_lo` (`n_q` the reduced unknowns, `omega_max` the largest of `frequencies_all`) | ADR 0573 NC1; the split with `G` is `>= 6e-4` of `omega` |
| NC0 | D4 | `RotatingSector::new` on the free sector with the default stress stiffening | refused, `FemError::SingularStiffness` | probe 1 |
| B1 | b7 | no line under `crates/*/src` changed (`git diff --stat` of the branch against 43092983) | none | test-only item |
| B2 | b7 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | as B1 |

## Negative controls

NC1 (no split without the Coriolis matrix: the split of SP is the Coriolis term's). NC0 (the free ring's own static solve is
singular: the reason for D4). TR (the families do not couple). RF2 is the control of the reference.

## Mutations (on a scratch copy, own `CARGO_TARGET_DIR`)

| Id | Mutation (site) | Expected to be caught by |
|---|---|---|
| X5 | ADR 0598's X5: `element_coriolis` passes the axis's global components `a` to `local_coriolis` in place of the local `al` (`beam.rs`) | WF, SP, KA, DI (out-of-plane), AG |
| X1 | the rotary block zero: `b = 0` in `local_coriolis` | SP (out-of-plane `d = -1`), KA, DI (out-of-plane), AG |
| X2 | `I_y` and `I_z` exchanged in `b` (`local_coriolis`) | SP (out-of-plane `-0.555`), KA, AG |
| X3 | the beam Coriolis matrix negated (`local_coriolis` subtracts each Gauss term) | DI (both families), KA |
| X4 | the sector's whirl with `G = Omega C` in place of `2 Omega C` (`RotatingSector::whirl_at` in `rotating/whirl.rs`, the second `fn whirl_at` of the file) | SP (`-0.5`), WF, AG |
| X6 | the beam geometric stiffness without its lateral terms (`local_geometric_stiffness`: the two `place_plane` calls removed) | WF (set F), AG |
| X7 | Wagner's torsion term zero (`nt = 0` in `local_geometric_stiffness`) | AG (`~ 2e-5`, below WF) |
| X8 | the beam spin-softening mass without the axial projection of the translation (`ra = 0` in `local_spin_softening_mass`) | WF (out-of-plane), AG |
| X9 | the rotary part of the beam spin-softening mass not projected (`mz = my = 0` in `local_spin_softening_mass`) | AG (`~ 1e-5`, below WF) |

No mutation is declared equivalent. Each is applied by a script to a `git archive` copy of the test commit and the test file run
once per mutation.

## Decision rule

**done**: every check RF1 to B2 and NC0 met in the registered run, every mutation caught by at least one check. **partial**: any
registered check missed (recorded as a `*_known_miss`), a check met only after a post-run amendment, a check not run, or a
mutation not caught. **todo**: WF, OD and SP not run.

## Limits (what is not covered)

- `k = 0, 1` (the rigid motions of the free ring, D4) are not solved; the free ring's own centrifugal static solve is refused
  (NC0) and not changed; set F takes its pre-stress from a constrained copy through `with_unit_prestress`.
- One section shape and slenderness (`R / h = 25`); shear and rotary inertia are in both the reference and the library, but their
  separate effects are not isolated. Offset or non-principal sections are not representable (ADR 0598).
- The opt-in moment, shear and Wagner terms of ADR 0508 (`with_beam_resultants`) are not on (the default rotating path does not
  take them); the reference matches the default.
- The sparse whirl solvers, the split modes, the whirl state and `full_rotor` with beams; the stationary-frame frequencies.
- The full ring as a `RotatingPart` (ADR 0598 compared it with the sector); only the sector path is checked here.
