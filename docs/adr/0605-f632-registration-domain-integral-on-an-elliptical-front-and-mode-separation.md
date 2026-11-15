# ADR 0605: F-632 registration: the domain integral on an elliptical front, and the separation of K_I, K_II and K_III

Status: **REGISTERED 2026-11-15, written before any code of F-632 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-632 was written, compiled or run: the files under "What was read" were
read, and the arithmetic below was done by hand and with plain Python scripts (closed forms, a potential integral, the near-tip
fields, one Hex20 quadrature probe and element counts; no rcfd code, no finite-element run; disclosed, scripts in
`w49i-work/f632-scratch/py`). Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-632 (dep F-429), the follow-up proposed by
[ADR 0547](0547-f429-registration-fracture-parameters-domain-integral-stress-intensity-and-growth-life.md) (its mutation M12, inner
rings left out of the integral, was caught by no check; its limit "no interaction integral"). Number 0605 given by the lead.
Branch `f632-work` from the local main tip 2ab2c4d8, worktree `w49i-work/f632`, scratch `w49i-work/f632-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored `*_known_miss`
test with the limit intact and the measured number); each registered run made once; existing tests not edited; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no
unsafe code; clean-room. The roadmap files are not edited (the entries go in "For the lead"). Every choice below that the
maintainer would normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the
recommendation).

## What F-632 asks

The acceptance, binding:

> Every limit not stated here is registered in the ADR before the run. An embedded elliptical crack whose K varies along the front against the closed form of Irwin within a registered limit, with a check that catches leaving the inner rings out of the tapered integral; an interaction integral separating K_I, K_II and K_III, the inclined centre crack within 2 percent of each closed form.

Clauses: **c0** every limit registered here; **c1** an embedded elliptical crack, K along the front against Irwin's closed form
within a registered limit; **c2** a check that catches the inner rings left out of the integral (F-429 M12); **c3** an
interaction integral giving K_I, K_II and K_III; **c4** the inclined centre crack: K_I and K_II each within 2 % of its closed
form; **c5** (from the brief) K_III by an out-of-plane shear case with its own closed form, within 2 % (the acceptance names
"each closed form"; K_III has none in the inclined in-plane case, where it is zero, so it gets the antiplane case and the same
2 %); **c6** (brief) the closed forms derived or verified before registering, with provenance; **c7** (brief) heavy tests
`#[ignore]` with fast companions; **c8** (brief, house rule 3) opt-in, existing bits unchanged.

## What was read (no run)

`AGENT_RULES.md`; the brief; ADR 0547 in full (D1 to D9, arithmetic 1 to 9, checks, mutations, addendum 1 with the M12
diagnosis); `docs/reference-data.md` section F-429; `crates/rcfd-fem/src/fracture.rs` at 2ab2c4d8 (all of it: `CrackFront`,
`FrontShape`, `FrontKind`, `FrontGeometry` and its `advance`, `tangent`, `param`, `distance`, `hat`, `hat_integral`; `rings`,
`check_boundary`, `check_loads`, `element_elastic`, `j_integral`, `plane_strain_k`, `opening_k`, `gauss_legendre`,
`paris_cycles`, `graded`, `centre_crack_plate`, `penny_crack_cylinder`); `crates/rcfd-fem/tests/sa_fracture.rs` (the helpers
`Checks`, `rebuild`, `plate`, `plate_model`, `solve_plate`, `penny_model`, `coarse_penny`, the heavy PH test);
`hex20::traction_loads` (traction vector times the face area at each face node); `model.rs` (`add_material`,
`material_indices` refusing overlaps, `dof_map`); ADR 0601 (form). `rcfd-api`, `rcfd-cli` and `rcfd-mcp` do not call
`rcfd_fem::fracture` (grep at 2ab2c4d8), so no case result and no golden can change.

On the citations of ADR 0547 that the brief asks to verify (Tada, Paris and Irwin; deLorenzi 1982): no copy of either is in the
repository or available to me, so the citations cannot be verified here and stay "from memory" (open question 3 of ADR 0547
remains open). Nothing below depends on them: every formula used is derived here or in ADR 0547, and the references of the checks
are closed forms whose derivation or numerical verification is given (arithmetic 1 to 4).

## Design

### D1. The elliptical front

`FrontShape::Elliptical { centre, major_axis, a, b }` and `CrackFront::elliptical(nodes, normal, centre, major_axis, a, b)`: the
ellipse `X^2/a^2 + Y^2/b^2 = 1` in the crack plane, `X = d . u`, `Y = d . v`, `d = x - centre`, `u` the unit major axis, `v = n x
u`. Validation (in `CrackFront::check`, key `"crack front"`): `a > b > 0` finite; `|u . n| <= 1e-6` (u normalised after the
test); every front node within `FRONT_TOLERANCE a` of the plane and within `FRONT_TOLERANCE a` of the ellipse by the first-order
distance `|g - 1| / |grad g|`, `g = sqrt(X^2/a^2 + Y^2/b^2)` (message containing "ellipse"); the parameters strictly increasing
(the existing test and message).

**Confocal coordinates.** With `c = sqrt(a^2 - b^2)` and the foci `(+/- c, 0)`, a point of the plane has the elliptic
coordinates `(mu, nu)`, `X = c cosh mu cos nu`, `Y = c sinh mu sin nu`: `cosh mu = (d_- + d_+)/(2 c)` (`d_+/-` the distances to
the foci) and `nu = atan2(Y cosh mu, X sinh mu)`. The front is `mu = mu0 = atanh(b/a)`, and on it `nu` is the parametric angle
`phi` of `X = a cos phi`, `Y = b sin phi` (first principles: `c cosh mu0 = a`, `c sinh mu0 = b`). The lines `nu = const` are the
confocal hyperbolas, orthogonal to every ellipse `mu = const`. So:

* **parameter** `s(x) = nu(x)`: the hats `phi_j` of ADR 0547 D3 are linear in `nu` between the stations (the existing `hat`);
  off the front a node takes the hat of the hyperbola through it;
* **advance direction** `e(x) = unit(dx/dmu) = unit(X tanh^2 mu u + Y v)` (the outward normal of the confocal ellipse through
  `x`; `dx/dmu = (X tanh mu, Y coth mu)`, scaled by `tanh mu > 0`); on the front it is the in-plane outward normal; on the mirror
  planes `X = 0` and `Y = 0` it lies in the plane exactly (no `cos(pi/2)` rounding: the formula uses `X` and `Y` themselves);
* **tangent** `n x e(x)` (for the boundary test of ADR 0547 D4: the faces of the mirror planes `X = 0` and `Y = 0` are end faces);
  a point with `mu <= 0` (on the focal segment) has no direction and is refused, which only a ring reaching the focal segment
  can produce;
* **normalisation** `int phi_j ds`: computed consistently with the discrete `q`, as the sum over the front edges (corner,
  mid-edge, corner) of `int_-1^1 (sum_a L_a(xi) phi_j(s_a)) |dx/dxi| dxi` with the quadratic Lagrange interpolation `L_a` of
  the three front nodes and `x(xi) = sum_a L_a x_a`, 8-point Gauss-Legendre (`gauss_legendre(8)`; the integrand is smooth,
  the rule's error below `1e-12` relative for these edges). Straight and circular fronts keep the existing closed-form
  `hat_integral` unchanged (their bits stay).
* `opening_k` refuses an elliptical front (key `"opening"`, message "straight or circular"): the opening path is not part of
  this item. (`distance` gets the first-order formula above for completeness; no path reaches it.)

The domain integral itself (`j_integral`) is unchanged: it calls `advance`, `param`, `hat`, `hat_integral` and the boundary
test, each extended by the elliptical case.

### D2. The check that catches the inner rings left out (c2): a uniform field

For a displacement that is linear in `x` the stress is uniform and `F_j,j = 0` holds with no singularity, so `int_V (sigma_ij
u_i,k - W delta_jk) q_k,j dV = P_jk int_V q_k,j dV = P_jk oint q_k n_j dS`, `P = sigma grad u - W I` constant. For the isotropic
expansion `u = eps (x - x_c)`: `sigma = 3 K_b eps I` (`K_b` the bulk modulus), `u_i,k = eps delta_ik`, `W = 9 K_b eps^2/2`, so `P
= p I` with `p = 3 K_b eps^2 - 9 K_b eps^2/2 = -3 K_b eps^2/2`, and `G_bar = p oint q . n dS`. On the boundary of the ring domain
`q = 0`, except on the crack plane (`q` in the plane, `q . n = 0`) and on the mirror planes (`q` in the plane: D1): **the
correct integral is exactly zero, for any front shape and any ring**. Discretely, `G_bar = p sum_a q_a . (sum_e int_e grad N_a
dV)`; with exact quadrature `sum_e int_e grad N_a dV = oint N_a n dS` (the Piola identity of the isoparametric map), so the
discrete value is zero to rounding when the stiffness rule integrates `adj(J) grad_xi N_a` exactly. Arithmetic 5 shows that the
3 x 3 x 3 rule does so on the curved elements of the builders (in-plane map, linear in z): the probe's error is `<= 7e-15`
relative to `h0^2`.

With the inner rings left out (M12), the integral is `p oint_(Gamma_in) q . n dS` over the inner boundary of ring k, `= -p int_in
div q dV`, and `div q = phi div e = phi kappa` (`kappa` the curvature of the confocal ellipse or circle; `e . grad phi = 0`). So
`J_M12 ~ -p kappa A_in`, `A_in` the cross-section of rings `1..k-1` (doubled in a half-symmetric model). For a straight front
`kappa = 0` (M12 not visible: ADR 0547's diagnosis), for the ellipse at `phi = 0` `kappa = a/b^2 = 4/a`.

The check **EU** prescribes the isotropic expansion (`eps = 1e-3`, `x_c` the origin) as the displacement vector (no solve) on the
coarse elliptical mesh and on the coarse penny mesh of ADR 0547 PF, and asks `eta = max_j |J_j| / (|p| h0) <= 1e-8` for rings 1,
2 and 3. Estimate: rounding, `~1e-12`. Under M12: ellipse ring 2 at `phi = 0`, `A_in = (2 h0/2)(2 h0)` (radial size `h0 b/a`,
D3), `eta ~ kappa A_in/h0 = (4/a)(2 h0^2)/h0 = 8 h0/a = 1.0` (`h0 = a/8`); at `phi = pi/2` `0.25`; penny ring 2 `4 h0/a = 0.5`. EU is the registered catcher
of c2, fast, independent of the accuracy of the solve. The heavy elliptical rows (EH2: rings 2 to 4 within 1 %) catch it too
(arithmetic 6).

### D3. The elliptical builder

`fracture::elliptical_crack_block(&EllipticalCrack)`: the penny mesh of ADR 0547 D8 (`penny_crack_cylinder` with radius `a`,
the same parameters: `R_p` the outer radius of the polar grid, `H`, `h0`, `n_u`, `n_theta`, core side `s`, `n_t`, `r_t`,
growths), every node mapped in the plane (z unchanged) by `M`:

* for `rho >= r_t (1 - 1e-12)` (`rho, theta` polar of the penny node): `mu = mu0 + ln(rho/a)`, `(x, y) = (c cosh mu X/rho, c sinh
  mu Y/rho)` (`X, Y` the penny coordinates): the circle `rho = a` goes to the crack front with `theta -> nu = phi`, the circles
  `rho` to the confocal ellipses, the rays to the hyperbolas;
* for `rho < r_t` (the core square and transition): the affine map `(X A_t/r_t, Y B_t/r_t)`, `A_t = c cosh mu_t`, `B_t = c sinh
  mu_t`, `mu_t = mu0 + ln(r_t/a)`, continuous with the first at `rho = r_t`.

Refused (key `"elliptical crack"`): `b >= a`, `mu_t <= 0` (`r_t <= a c/(a + b)`), and whatever the penny builder refuses. The map
is orientation-preserving (`d(x, y)/d(mu, nu) = c^2 (sinh^2 mu cos^2 nu + cosh^2 mu sin^2 nu) > 0`, `dmu/drho = 1/rho > 0`, the
affine factors positive), so the mesh validation (`det J > 0` at the nodes) is expected to pass; mid-edge nodes are mapped by
the same `M` (the front's mid-edge nodes at `nu` of mid angle exactly on the ellipse). `dmu/drho = 1/a` at the front: the radial
element size is `h0` at `phi = pi/2` and `h0 b/a` at `phi = 0`; z sizes are the penny's. The outer boundary is the ellipse
`mu0 + ln(R_p/a)` (for `R_p = 12 a`, `b = a/2`: semi-axes `9.02 a`, `8.98 a`). Returned `CrackedEllipse { mesh, front }` with the
penny's node sets (`all`, `ligament`, `x0`, `y0`) and surface set (`top`); `crack_front()` = elliptical about the origin, normal
`z`, major axis `x`, half-symmetric. Model as the penny: `u_z = 0` on `ligament`, `u_x = 0` on `x0`, `u_y = 0` on `y0`, `(0, 0,
sigma)` on `top`.

### D4. Irwin's closed form (c1, c6)

`K_I(phi) = sigma sqrt(pi b)/E(k) (sin^2 phi + (b/a)^2 cos^2 phi)^(1/4)`, `k^2 = 1 - b^2/a^2`, `E(k) = int_0^(pi/2) sqrt(1 - k^2
sin^2 t) dt` (Irwin 1962 from the solution of Green and Sneddon 1950; both from memory). Derivation and verification:

1. Green and Sneddon's opening is ellipsoidal, half-opening `w = w0 sqrt(1 - X^2/a^2 - Y^2/b^2)` with `w0 = 2 sigma b/(E' E(k))`,
   `E' = E/(1 - nu^2)` (from memory). Near the front, a distance `r` inward along the normal, `1 - f ~ r |grad f|`, `|grad f| =
   2 sqrt(X^2/a^4 + Y^2/b^4) = (2/b) (sin^2 phi + (b/a)^2 cos^2 phi)^(1/2)`. The local plane-strain half-opening `4 K (1 - nu^2)/E
   sqrt(r/(2 pi))` (ADR 0547 D6) then gives `K = w0 E' sqrt(pi)/(2 sqrt(b)) (sin^2 phi + (b/a)^2 cos^2 phi)^(1/4)`, which with `w0`
   is the formula (derived here).
2. The factor `w0 ∝ b/E(k)` is verified numerically (arithmetic 1): the normal stress on the crack of an opening `D(X, Y)` is a
   constant times the in-plane Laplacian of `phi(x) = int D(xi)/|x - xi| dA` (potential theory of the dislocation layer); for
   `D = sqrt(1 - f)` the probe gives `-Lap phi b/E(k) = 6.28318531` (`2 pi`) for `b/a = 1, 0.75, 0.5, 0.25` alike, so the uniform
   pressure needs `w0 ∝ b/E(k)` for every aspect ratio.
3. The constant is fixed by the limits: `b = a` (`E(0) = pi/2`) gives Sneddon's `K = 2 sigma sqrt(a/pi)` (the reference of ADR
   0547 PH1) and `w0 = 4 sigma a/(pi E')` (Sneddon's opening); `a -> infinity` (`E(1) = 1`) gives at `phi = pi/2` the tunnel crack
   `K = sigma sqrt(pi b)` and Westergaard's half-opening `2 sigma (1 - nu^2)/E sqrt(b^2 - x^2)` (ADR 0547 arithmetic 3).

For `b/a = 0.5`: `E(k) = 1.21105603`, `K(pi/2) = 0.82573 sigma sqrt(pi b)`, `K(0)/K(pi/2) = sqrt(b/a) = 0.70711`; with `a = 0.01`,
`sigma = 1e8`: `K(pi/2) = 1.03489e7`, `K(0) = 7.3178e6 Pa m^0.5`: K varies by 41 % along the front.

**The reference of a station.** A station's J is the hat average `int G phi_j ds / int phi_j ds`; for a varying `G` it differs
from the point value at the station by up to `1.9 %` (coarse, `n_theta = 4`) and `0.21 %` (fine, `n_theta = 12`) in K
(arithmetic 2). The checks therefore compare with `K_ref,j = sqrt(<K_Irwin^2>_j)`, the hat average over the exact ellipse (`ds =
sqrt(a^2 sin^2 nu + b^2 cos^2 nu) d nu`, hat linear in `nu`), computed in the test by composite Gauss-Legendre (16 points on each
side of the station, 64 panels), with the point values printed. `E(k)` is computed in the test by the composite Simpson rule
(4000 intervals, error `< 1e-13`). The finite body (`9 a` semi-axes, `H = 10 a`) is estimated to change K by about `(a/9a)^3 ~
1.4e-3` (estimate, as ADR 0547 arithmetic 7, not a reference).

### D5. The interaction integral (c3)

For two equilibrium fields of the same homogeneous linear elastic material, the J of their sum is `J + J^aux + M`, with the
interaction integral (the cross terms of the domain form of ADR 0547 D3, derived by the same divergence argument; the identity
`sigma_ij eps^aux_ij,k = sigma^aux_ij eps_ij,k` of a common isotropic material replaces `W_,k = sigma_ij u_i,jk`)

`M = int_V (sigma_ij u^aux_i,k + sigma^aux_ij u_i,k - W^int delta_jk) q_k,j dV`, `W^int = sigma_ij eps^aux_ij`

(Yau, Wang and Corten 1980; Moran and Shih 1987: from memory; derived). Near a point of a smooth front the field is locally
plane strain plus antiplane, `J = (K_I^2 + K_II^2)/E' + K_III^2/(2 mu)` (ADR 0547 D5, and arithmetic 3 for each mode), so `M =
2 (K_I K_I^aux + K_II K_II^aux)/E' + K_III K_III^aux/mu`. With the unit auxiliary field of one mode: `K_I = E' M_I/2`, `K_II = E'
M_II/2`, `K_III = mu M_III`.

**Auxiliary fields.** In the local frame of a straight front, `x1` along the advance `e`, `x2` along the normal `n`, `x3` along
`e x n`, origin the front line (`r`, `theta = atan2(x2, x1)` in `(-pi, pi]`, the crack faces at `theta = +/- pi`), plane strain,
`kappa = 3 - 4 nu` (Williams 1957, Irwin; Anderson chapter 2; from memory, then verified):

* I: `u1 = 1/(2 mu) sqrt(r/(2 pi)) cos(theta/2) (kappa - cos theta)`, `u2 = 1/(2 mu) sqrt(r/(2 pi)) sin(theta/2) (kappa - cos theta)`;
* II: `u1 = 1/(2 mu) sqrt(r/(2 pi)) sin(theta/2) (kappa + 2 + cos theta)`, `u2 = -1/(2 mu) sqrt(r/(2 pi)) cos(theta/2) (kappa - 2 +
  cos theta)`;
* III: `u3 = 2/mu sqrt(r/(2 pi)) sin(theta/2)`.

Only the displacement gradient is coded (analytic chain rule in `r, theta`); strain and stress follow by Hooke's law with the
element's `lambda, mu`, so compatibility is built in. Arithmetic 3 verified (Python, these formulas only): the gradient against
central differences of `u` (`1e-10`), equilibrium (`div sigma` `<= 5e-10` of `|sigma|/r`), traction-free faces (`1e-12`
relative), `sqrt(2 pi r) (sigma_22, sigma_12, sigma_23)` ahead `= (1, 0, 0)`, `(0, 1, 0)`, `(0, 0, 1)` for I, II, III, and Rice's
contour J of each unit field `= 1/E'`, `1/E'`, `1/(2 mu)` with cross terms `< 2e-13` (which fixes the factors of `K = E' M/2`, `K
= mu M`). The fields of a straight front are exact solutions of three-dimensional elasticity (independent of `x3`), so `M`
needs no correction terms; on a curved front they are not, and the interaction integral **refuses** circular and elliptical
fronts (key `"crack front"`, message containing "straight").

`fracture::interaction_integral(model, displacement, front, ring) -> Vec<ModeFactors>`; `ModeFactors { station, node, position,
s, k_i, k_ii, k_iii, weight }`. The rings, `q`, the boundary and load tests and the element tests are those of `j_integral`; per
integration point the local coordinates of the point, the three unit auxiliary gradients rotated to the global frame (`G =
R G_loc R^T`, `R = [e | n | e x n]`), and the three `M` sums. Refused besides: a half-symmetric front (modes II and III are
antisymmetric about the crack plane; doubling is wrong for them), a domain whose elements do not all have the `E, nu` of the
element of the first front node (key `"element"`: the auxiliary field and the conversion assume one material). `E', mu` of that
element convert `M`. The public `near_tip_gradient(mode, r, theta, mu, nu) -> (Vec3, [[f64; 3]; 3])` returns the unit field's
displacement and gradient in the local frame (`TipMode::{I, II, III}`), used by the library and checked by MA.

**The front mean.** In a one-layer plate with fields independent of `z`, the hat of an end station has `dphi/dz = -1/t`
throughout, and the term `P_(z k) q_k,z` integrates to a contribution of the end face that does not vanish for a field with
both in-plane and out-of-plane parts (for the interaction integral: an aux mode with a mode of the other kind, e.g. `sigma^aux_33
u_3,1` of the III case under the mode-I auxiliary field). The two stations' terms are equal and opposite, and the sum over the
stations has `sum_j q^(j) = rho e` with no `z` derivative. The plate checks therefore use the **front mean** `K_bar = sum_j K_j
w_j / sum_j w_j` (`w_j` = `weight`), which is exactly the integral with `q` uniform along the front; the per-station values are
printed (record). In the mode-I plate the term vanishes exactly (as in ADR 0547 arithmetic 4).

### D6. The inclined centre crack and the antiplane case (c4, c5, c6)

**Closed forms (infinite plate).** A remote uniform stress whose components in the crack frame are `sigma_nn`, `sigma_en`,
`sigma_ee`: subtracting the uniform state leaves the crack faces loaded by `-sigma_nn` and `-sigma_en` (`sigma_ee` leaves them
traction-free, so it gives no K). Mode I: `K_I = sigma_nn sqrt(pi a)` (Westergaard, ADR 0547 arithmetic 3). Mode II: `K_II =
sigma_en sqrt(pi a)` (the Westergaard function `Z = tau z/sqrt(z^2 - a^2)` of the same form, from memory; the FE check
verifies it). Under uniaxial tension `sigma` at the angle `beta` between the load and the crack normal: `sigma_nn = sigma cos^2
beta`, `sigma_en = sigma sin beta cos beta` (rotation of the stress tensor), so `K_I = sigma sqrt(pi a) cos^2 beta`, `K_II =
sigma sqrt(pi a) sin beta cos beta`. Mode III, remote `sigma_23 = tau` (derived): `w = (tau/mu) Im sqrt(z^2 - a^2)` is harmonic,
`-> tau y/mu` far away; on the crack `sqrt(x^2 - a^2)` is imaginary so `sigma_yz = tau Re(z/sqrt(z^2 - a^2)) = 0` (traction-free);
ahead `sigma_yz = tau x/sqrt(x^2 - a^2) ~ tau sqrt(a/(2 r))`: `K_III = tau sqrt(pi a)`.

**Inclined set-up `I`.** The plate `P` of ADR 0547 (`a = 0.01`, `W = 10 a`, `H = 20 a`, `t = 0.05 a`, `h0 = a/40`, `g = 1.4`,
`n_u = 4`, plane strain `u_z = 0` on `all`) rotated about `z` by `beta = 30 deg` (every node `x -> R x`): the crack at `30 deg` to
`x`, the load uniaxial along global `y`, applied as the tractions of the uniform state `sigma e_y (x) e_y` on the four sides,
`t = sigma (m . e_y) e_y` with `m` the rotated outward normal (top `sigma cos beta e_y`, bottom `-sigma cos beta e_y`, right `sigma
sin beta e_y`, left `-sigma sin beta e_y`), `sigma = 1e8`. The angle between the load and the crack normal is `beta`, so `K_I /
(sigma sqrt(pi a)) = 0.75`, `K_II / (sigma sqrt(pi a)) = 0.43301`. Supports: `u_x = u_y = 0` on `right_pin`, `u_y = 0` on
`left_pin` (the loads are self-equilibrated; the second support fixes only the rotation about the first, reachable by a rigid
rotation since `cos beta != 0`, so no reaction). Front: the right tip, straight, `n = (-sin beta, cos beta, 0)`, `e = (cos
beta, sin beta, 0)`. The rotation exercises the frame transformation (an unrotated set-up would not catch a transposed `R`).
`StaticSolver::Direct`, `refinement(2)` (as ADR 0547).

**Antiplane set-up `III`.** The plate `P` unrotated, `u_x = u_y = 0` on `all`, `u_z = 0` on `right_pin`; tractions `(0, 0, tau)` on
`top`, `(0, 0, -tau)` on `bottom`, `tau = 1e8`. With the in-plane components held, the discrete problem is the scalar one for
`w(x, y)`, exact in one layer (z-independent, as ADR 0547 arithmetic 4); antisymmetric about `y = 0`, consistent with the support.

**Mode-I set-up `T`.** The tension plate of ADR 0547 P (unrotated), for the consistency with J and the zero modes.

Finite size: mode I in the strip `W = 10 a` is `0.62 %` above the infinite plate (secant form, ADR 0547 arithmetic 1), and ADR
0547 measured the FE `0.16 %` below the secant form at ring 3, so about `+0.45 %` against the closed form; mode II and III in the
finite plate I estimate within `1 %` of the infinite plate (for III the periodic-strip form `sqrt((2 W/(pi a)) tan(pi a/(2 W)))
= 1.0041`, from memory; for II the same order as I; estimates, not references). The limit is the acceptance's 2 %.

### D7. Opt-in

Everything is in `crates/rcfd-fem/src/fracture.rs`: new variants `FrontShape::Elliptical` (the enum is `#[non_exhaustive]`) and
the private `FrontKind::Elliptical`, new functions and types; the existing paths of straight and circular fronts are not changed
(each new branch is taken only for the elliptical kind; `opening_k` gets one refusal of the elliptical kind before its
existing code). No other library file changes, no case-file key; `baseline_hashes` is run anyway. Tests in a new file
`crates/rcfd-fem/tests/sa_fracture_modes.rs`; `sa_fracture.rs` is not edited.

## Arithmetic done beforehand (disclosed)

1. **Green and Sneddon's factor** (`py/gs.py`): `phi` by the closed-form inner integral along each ray (`int sqrt(c0 - 2 c1 rho -
   c2 rho^2) d rho`) and 4000 angles, the Laplacian by the 5-point difference at the centre (`phi` is quadratic inside): `-Lap
   phi = 9.8696044, 11.5733614, 15.2185789, 26.9499068` for `b/a = 1, 0.75, 0.5, 0.25`; times `b/E(k)` all `6.28318531`.
2. **Irwin at the stations** (`py/irwin.py`): the table of D4; hat bias of the point value against the hat average `+1.88e-2`
   (coarse station 0), `+2.14e-3` (fine station 0); largest elsewhere smaller.
3. **Near-tip fields** (`py/williams.py`): as listed in D5.
4. **Inclined closed forms**: `cos^2 30 = 0.75`, `sin 30 cos 30 = 0.4330127`; `sigma sqrt(pi a) = 1.77245e7`.
5. **Quadrature of the Piola identity** (`py/quad.py`, numpy, a Hex20 of the map of D3 at `phi = 0` and `pi/2`, inside and
   outside the front, coarse and fine sizes): `max |int grad N_a dV (3^3) - (10^3)| <= 6.9e-15 h0^2`, i.e. exact to rounding.
6. **M12 magnitude** (`py/m12.py`): leading term `J_M12 - J ~ kappa int_(A_in) W dA` (inner rings, `q_k,j = phi kappa t_k t_j +
   ...`, `p_tt ~ -W`); `E' int W dA / K^2` over a rectangle of half-sizes `(d_r, d_z)` about the tip `= 0.521 d` (`d_r = d_z = d`)
   and `0.383 d` (`d_r = d/2`, `d_z = d`). Check against ADR 0547's diagnosis: the coarse penny ring 2 predicted `dK/K = 3.3 %`,
   observed `4 %`. Fine ellipse ring 3 at `phi = 0` (`kappa = 4/a`, `d = 2 h0 = a/16`): `dJ/J = 0.096`, `dK/K ~ 4.8 %`; at `phi =
   pi/2` `0.8 %`. Per ring step at `phi = 0` `dK ~ 2.4 %`: the spread of rings 2 to 4 under M12 is about `4.8 %`. Coarse ring 2 at
   `phi = 0`: `dK ~ 9.6 %`.
7. **Element counts** (`py/counts.py`): fine (`h0 = a/32`, `n_u = 4`, `n_theta = 12`, `s = 0.4 a`, `n_t = 2`, `r_t = 0.75 a`,
   `g_in = 1.25`, `g_out = 1.4`, `R_p = 12 a`, `H = 10 a`): 25 polar layers, 18 z layers, 6480 Hex20, about 86 000 unknowns (the
   PH penny had 96 267 and solved in 36 s: heavy record, `#[ignore]`); `mu_t = 0.2616`. Coarse (`h0 = a/8`, `n_u = 2`, `n_theta =
   4`, `s = 0.4 a`, `n_t = 1`, `r_t = 0.65 a`, `g_in = 1.5`, `g_out = 1.6`, same `R_p`, `H`): 600 Hex20, about 9 000 unknowns;
   `mu_t = 0.1185` (the inner ellipse `0.872 a x 0.103 a`, inside the crack, far from the rings).
8. **The radial advance (mutation M2)**: the angle between the radial direction and the normal of the ellipse at `phi = 45 deg`
   is `36.9 deg` (normal `(b, a)`, radial `(a, b)` directions): J scaled by about `cos 36.9 = 0.80`, K by `0.89`.

## Checks

Common: `E = 2e11`, `nu = 0.3`, `a = 0.01`, `sigma = tau = 1e8`. Relative errors `|v/v_ref - 1|`; every aggregated value asserted
finite (F1). Ellipse `b = a/2`; `EF`/`EU` on the coarse mesh, `EH` on the fine mesh (arithmetic 7); `StaticSolver::Direct` for
coarse, `SupernodalDirect` for fine. Plate rows at ring 3 unless stated, the front mean of D5.

| check | clause | set-up | limit | estimate |
|---|---|---|---|---|
| EH1 | c1 | fine ellipse (heavy, `#[ignore]`, run once alone): `plane_strain_k` of the ring-3 J at each of the 13 stations against `K_ref,j` (D4): the largest error | `<= 0.03` | 0.01 |
| EH2 | c1, c2 | fine: at each station `(max - min)/min` of the K of rings 2, 3, 4; the largest | `<= 0.01` | 0.003 |
| EH3 | c1 | fine: `K_0/K_12` (ring 3) against `K_ref,0/K_ref,12` | `<= 0.02` | 0.005 |
| EH4 | record | fine: rings 2 and 4 errors, point values of Irwin, mesh counts and solve time printed | none | |
| EF | c1, c7 | coarse ellipse (fast): ring-2 K at each of the 5 stations against `K_ref,j`: the largest error | `<= 0.08` | 0.03 |
| EU1 | c2 | coarse ellipse mesh, displacement `u = 1e-3 x` prescribed: `max_j |J_j|/(|p| h0)` for rings 1, 2, 3 | `<= 1e-8` | 1e-12 |
| EU2 | c2 | the same on the coarse penny mesh of ADR 0547 PF (circular front) | `<= 1e-8` | 1e-12 |
| MI1 | c4 | `I`: front-mean `K_I` against `sigma sqrt(pi a) cos^2 beta` | `<= 0.02` | 0.006 |
| MI2 | c4 | `I`: front-mean `K_II` against `sigma sqrt(pi a) sin beta cos beta` | `<= 0.02` | 0.01 |
| MI3 | c3 | `I`: `|K_III|/(sigma sqrt(pi a))` | `<= 1e-8` | 1e-14 |
| MI4 | c3 | `I`: front-mean J of `j_integral` against `(K_I^2 + K_II^2)/E'` | `<= 0.02` | 0.003 |
| MI5 | c3 | `I`: `(max - min)/max` of `K_I` and of `K_II` over rings 2, 3, 4; the larger | `<= 0.01` | 0.003 |
| M31 | c5 | `III`: front-mean `K_III` against `tau sqrt(pi a)` | `<= 0.02` | 0.006 |
| M32 | c3 | `III`: `max(|K_I|, |K_II|)/(tau sqrt(pi a))` | `<= 1e-8` | 1e-14 |
| M33 | c3 | `III`: `sqrt(2 mu J)` (front-mean J of `j_integral`) against `K_III` | `<= 0.01` | 0.003 |
| MT1 | c3 | `T`: front-mean `K_I` against `plane_strain_k` of the front-mean J | `<= 0.01` | 0.002 |
| MT2 | c3 | `T`: `max(|K_II|, |K_III|)/K_I` | `<= 1e-8` | 1e-13 |
| MR | record | `I`, `III`, `T`: per-station K values printed (the end-face term of D5) | none | |
| MA | D5 | `near_tip_gradient` for I, II, III at 4 points: gradient against central differences of the displacement (`h = 1e-6 r`) `<= 1e-6` relative; `div sigma` by central differences of the stress `<= 1e-5 |sigma|/r`; traction on `theta = +/- (pi - 1e-9)` `<= 1e-6` of `|sigma|`; `sqrt(2 pi r)` times `(sigma_22, sigma_12, sigma_23)` at `theta = 0` against the unit vector of the mode `<= 1e-12` | as stated | 1e-9, 1e-8, 1e-9, 1e-15 |
| R5 | D1, D3, D5 | refusals: `interaction_integral` with the coarse penny front (circular) and with a half-symmetric straight front ("straight", "half"), with a ring element of another material (key `"element"`); `CrackFront::elliptical` with `b >= a`, with the major axis not in the plane, with a front node moved off the ellipse by `1e-4 a` in `x` ("ellipse"), and the same moved by `1e-8 a` accepted; `opening_k` of an elliptical front; `elliptical_crack_block` with `b >= a` and with `r_t <= a c/(a + b)` | refused (each), the control accepted | |
| F1 | brief | every aggregated value above finite | finite | finite |

## Negative controls and mutations

Controls: R5's `1e-8 a` acceptance; EU at ring 1 (no inner rings, so M12 cannot change it: shows EU is not vacuous when it moves
at rings 2 and 3); MT2 and M32 (the separation puts nothing in the absent modes). Mutations, each run once on a scratch copy
(`f632-scratch/mut`, its own target dir), the fast tests of `sa_fracture_modes.rs` (EH not run for mutations):

| mutation | change | expected to be caught by |
|---|---|---|
| M1 | inner rings left out: `rings` returns only the elements of ring `k` (F-429 M12) | EU1, EU2 |
| M2 | elliptical advance direction radial, `unit(X u + Y v)` | EF |
| M3 | elliptical weight `int phi_j ds` replaced by `a (s_(j+1) - s_(j-1))/2` (the circle formula with radius `a`) | EF |
| M4 | the sign of the mode-II auxiliary `u2` flipped | MI2 (and MI4) |
| M5 | `W^int` dropped from the interaction integrand | MI1, MI2, M31, MT1 |
| M6 | `K = E M/2` (plane stress) for modes I and II | MI1, MT1 |
| M7 | the local frame `e3 = n x e` (K_III sign) | M31 |
| M8 | the rotation of the auxiliary gradient transposed (`R^T G R`) | MI1 or MI2 |
| M9 | the elliptical distance test disabled | R5 |
| M10 | mode-I auxiliary `u1` with `kappa + cos theta` | MA, MT1 |

## Decision rule

* **done**: EH1 to EH3, EF, EU1, EU2, MI1 to MI5, M31 to M33, MT1, MT2, MA, R5, F1 met, every mutation caught by at least one
  check, fmt and clippy clean, the crate's tests and `baseline_hashes` pass.
* **partial**: the code built and the fast checks run, but a registered check missed (a `*_known_miss` record) or EH not run,
  or a mutation not caught.
* **todo**: the elliptical front or the interaction integral not built.

## Limits (not covered)

* The interaction integral on straight fronts only (curved fronts need the correction terms of non-exact auxiliary fields);
  not on half-symmetric models; one material in the domain; linear elastic isotropic, no body force, no thermal strain, no
  crack-face traction (as ADR 0547).
* Elliptical fronts as a closed-form shape (centre, axes); no general curved front given by nodes only; mode I only on it (no
  mixed-mode elliptical case); `opening_k` not on elliptical fronts.
* The plate checks use the front mean; the per-station values of a mixed in-plane/out-of-plane field at the end stations of a
  layer include the end-face term (D5) and are records.
* No surface crack or free-surface corner (the front meets no free surface); no case-file key.
* The citations of D4 to D6 are from memory; each formula used is derived or verified numerically here.
