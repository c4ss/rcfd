# ADR 0610: F-682 registration: the interaction integral on curved crack fronts (circular and elliptical), with the correction terms of the auxiliary fields

Status: **REGISTERED 2026-11-15, written before any code of F-682 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-682 was written, compiled or run: the files under "What was read" were
read, and the arithmetic below was done by hand and with plain Python scripts (numpy only; the correction terms against central
differences, the potential of the penny slip, the closed forms, element counts; no rcfd code, no finite-element run; disclosed,
scripts in `w49i-work/f682-scratch/py`). Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-682 (dep F-632), the follow-up proposed by
[ADR 0605](0605-f632-registration-domain-integral-on-an-elliptical-front-and-mode-separation.md) (its remaining question 2).
Number 0610 given by the lead. Branch `f682-work` from the local main tip d8b6f3f5 (which holds F-632, ADR 0605), worktree
`w49i-work/f682`, scratch `w49i-work/f682-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored `*_known_miss`
test with the limit intact and the measured number); each registered run made once; existing tests not edited; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no
unsafe code; clean-room. The roadmap files are not edited (the entries go in "For the lead"). Every choice below that the
maintainer would normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the
recommendation).

## What F-682 asks

The acceptance, binding:

> Every limit not stated here is registered in the ADR before the run. The interaction integral on circular and elliptical fronts with the correction terms of auxiliary fields that are not exact there; a penny-shaped crack under inclined tension with K_I, K_II and K_III along the front within 3 percent of the closed forms, and the straight-front results of ADR 0605 unchanged.

Clauses: **c0** every limit registered here; **c1** an interaction integral on circular fronts and **c2** on elliptical fronts,
each with the correction terms of the auxiliary fields; **c3** a penny-shaped crack under inclined tension, `K_I`, `K_II` and
`K_III` along the front within 3 % of the closed forms; **c4** the straight-front results of ADR 0605 unchanged. From the brief:
**c5** the closed forms verified independently before registering, the ADR saying how each was verified (`nu -> 0`, the energy
release against the existing domain integral, the circumferential average); **c6** if the half-symmetric penny model cannot carry
shear, a full model or a stated scope; **c7** heavy tests `#[ignore]` with fast companions; **c8** (house rule 3) opt-in, existing
bits unchanged.

## What was read (no run)

`AGENT_RULES.md`; the brief; ADR 0605 in full (D1 to D7, arithmetic, checks, mutations, addendum 1; in particular D5: the
interaction integral of a straight front, "the fields of a straight front are exact solutions ..., on a curved one they are
not", and the end-face term of the front mean); ADR 0547 D2 to D8 and addendum 1 (the penny meshes PF and PH and their
accuracy: PF ring 2 `1.27e-2`, PH ring 3 `2.24e-3`); `docs/reference-data.md` sections F-429 and F-632;
`crates/rcfd-fem/src/fracture.rs` at d8b6f3f5 (all of it); `crates/rcfd-fem/tests/sa_fracture_modes.rs` (all: the helpers, the
plate set-ups, the `k_ref_station` hat average); `element.rs` (`PointGeometry`: position, shape values `n`, gradients);
`mesh.rs` (`face_incidence`, `face_nodes`, `add_surface_set`, `FaceRef`); `hex20.rs` (`traction_loads`, `traction_field_loads`:
consistent nodal forces of a position-dependent traction on Hex20 faces); `model.rs` (`Load`, `Constraint::displacement`).
`rcfd-api`, `rcfd-cli` and `rcfd-mcp` do not call `rcfd_fem::fracture` (grep at d8b6f3f5), so no case result and no golden can
change.

The papers named below (Gosz and Moran 2002; Walters, Paulino and Dodds 2005; Kassir and Sih 1966; the Tada handbook) are
cited **from memory**: no copy is in the repository or available to me. Nothing below depends on their content: the correction
terms are derived here from the divergence theorem and verified against central differences (arithmetic 1), and the closed forms
of the penny under shear are derived from the 2-D closed forms of ADR 0605 by a Fourier reduction and a potential integral
verified numerically (arithmetic 2).

## Design

### D1. The curved interaction integral and its correction terms (c1, c2), derived

ADR 0605 D5 uses `M = int_V P_jk q_k,j dV` with `P_jk = sigma_ij H_ik + sigma^aux_ij u_i,k - sigma_ij eps^aux_ij delta_jk`, `H`
the auxiliary displacement gradient. By the divergence theorem over the ring domain `V` (outer boundary `q = 0`, the crack
faces, the end faces, and a tube of vanishing radius about the front),

`int_(tube) P_jk q_k m_j dS = int_V P_jk q_k,j dV + int_V P_jk,j q_k dV - int_(faces) P_jk q_k m_j dS`

(`m` the normal out of the tube; the sign convention of ADR 0547 D3, where `J_bar = int P q,j dV` for `P,j = 0`). As the tube
shrinks the left side tends to `int_front q_k(s) [lim oint P_jk m_j dGamma] ds = int_front phi_j(s) M_loc(s) ds`, where the local
limit is the plane-strain-plus-antiplane value `M_loc = 2 (K_I K_I^aux + K_II K_II^aux)/E' + K_III K_III^aux/mu` of ADR 0605 D5
whenever the auxiliary field near each front point tends to the Williams field of that point. So the station value is

`M_j = (int_V P_jk q_k,j dV + int_V P_jk,j q_k dV) / int phi_j ds` (crack-face term zero, below),

the first integral being ADR 0605's and the second the **correction**, zero when the auxiliary field is an exact solution.

**The auxiliary field on a curved front.** For a point `x` near a smooth plane front (in the crack plane of normal `n`): its
*foot* `F` is the nearest point of the front curve in the plane; `N` the in-plane outward normal of the curve at `F` (the
advance direction), `b1 = N`, `b2 = n`, `b3 = N x n` (the frame of ADR 0605, `e3 = e x n`); `x1 = (x - F) . N`, `x2 = (x - x0) . n`
(`x0` the first front node), `r, theta` polar in `(x1, x2)`; `kappa > 0` the curvature of the curve at `F`; `h = 1 + kappa x1`.
The auxiliary gradient, strain and stress are those of ADR 0605 rotated into the frame of the foot,

`H = R G_loc(x1, x2) R^T`, `eps^aux = sym H`, `sigma^aux = C eps^aux = R sigma_loc R^T`, `R = [b1 | b2 | b3]`,

with `G_loc` the Williams gradient of `near_tip_gradient` (unchanged). This is the "incompatible" choice (Gosz and Moran 2002;
Walters, Paulino and Dodds 2005; from memory): `H` is not the gradient of a displacement field and `sigma^aux` is not
equilibrated when `kappa != 0`; but `sigma^aux m = 0` on the crack faces exactly (the faces are the crack plane, `m = +/- n = +/-
b2`, and `sigma_loc` is traction-free at `theta = +/- pi`), and near the front `H -> R G_loc R^T` of the foot. (The alternative,
the true gradient of `R(s) u_loc`, is compatible but its stress has a normal traction `lambda kappa u_1^loc/h` on the crack faces
for mode II, which would add a face integral; not taken.)

**The divergence.** With the actual field in equilibrium without body force (`sigma_ij,j = 0`, as ADR 0547) and the same
isotropic `C` for both fields,

`P_jk,j = sigma^aux_ij,j u_i,k + sigma_ij (H_ik,j - eps^aux_ij,k)`

(the terms `sigma^aux_ij u_i,kj = sigma^aux_ij eps_ij,k` and `sigma_ij,k eps^aux_ij = eps_ij,k sigma^aux_ij` cancel by the
symmetry of `C`). Both remaining terms need the derivatives of the rotated fields. In the curvilinear coordinates `(s, x1, x2)`
(`s` arc length of the foot) `x = F(s) + x1 N(s) + x2 n`, `dx/ds = h T`, `T = dF/ds = n x N = -b3`, so `ds/dx = T/h`,
`dx1/dx = N`, `dx2/dx = n`. Along `b1` and `b2` the local components change only through `G_loc(x1, x2)`; along `b3` (`dl`)
the foot moves by `ds = -dl/h` and the frame turns: `dN/ds = kappa T` gives `db1/dl = (kappa/h) b3`, `db3/dl = -(kappa/h) b1`,
`db2/dl = 0`, so for any rotated field `A = R A_loc R^T`, its derivative along `b3` in local components is

`D3 A = -(1/h) (Omega A_loc - A_loc Omega)`, `Omega_13 = kappa`, `Omega_31 = -kappa` (others 0).

In the local frame (all tensors and `q` rotated by `R^T`; `G = G_loc`, `sa = sigma_loc^aux`, `s = R^T sigma R`, `g = R^T grad u R`,
`ql = R^T q`):

* **C1** `= (div sigma^aux) . (g ql)`: the in-plane part `d1 sa_a1 + d2 sa_a2 = 0` (Williams equilibrium), leaving `(D3 sa)_a3`:
  `div sigma^aux = (kappa/h) (sa_11 - sa_33, sa_21, 2 sa_13)`. (For the circle these are the cylindrical terms `(sigma_rr -
  sigma_phiphi)/r_c`, `sigma_rz/r_c`, `2 sigma_rphi/r_c` with `r_c = R h`: a check by hand.)
* **C2** `= s_ab (D_b H)_ac ql_c - s_ab (D_c eps^aux)_ab ql_c`: for `b, c` in `{1, 2}` the two parts cancel (`G_loc` is the true
  gradient of the 2-D Williams field and its third column is zero); the `c = 3, b in {1, 2}` part of the first term vanishes
  (`(G_loc)_a3 = 0` everywhere). What remains: `C2 = s_a3 (D3 H)_ac ql_c - ql_3 s_ab (D3 eps^aux)_ab`, i.e.

  `C2 = -(kappa/h) [s_13 (G ql)_3 - s_33 (G ql)_1 - (s_13 G_11 + s_23 G_21 + s_33 G_31) ql_3] + (ql_3/h) sum_ab (s Omega - Omega s)_ab eps^aux_ba`.

  (The last term is not zero: `s` is the actual stress and `eps^aux` the auxiliary strain, which do not commute. It is the term
  my first hand derivation dropped; arithmetic 1 found it, disclosed.)

The correction integral per station is `int_V (C1 + C2) q-weighted dV` with `q = sum_a N_a q_a` at the integration point (the
shape values of the element; `q_a = rho_a phi_j(s_a) e_a` as ADR 0547). For a straight front `kappa = 0` and both vanish.

**The crack faces and the end faces.** On the crack faces `q . m = 0`, `sigma m = 0` (traction-free) and `sigma^aux m = 0`
(above), so the face integral is zero. On an end face that is a mirror plane normal to the front (the x = 0 plane of D4), for the
mode I and II auxiliary fields `H_(b3, .) = 0` and `sigma^aux_(b3, b1/b2) = 0`, and `u_x = 0` on the plane: the end-face term is
zero; for mode III it is not (the end-face term of ADR 0605 D5): at a station whose hat reaches such a face `K_III` is a
record, not a check (D4). In a half-symmetric model (the half `+n` of the crack plane, symmetric loading) the mode-I auxiliary
field is symmetric about the crack plane and modes II and III antisymmetric; the full-model integral of an antisymmetric
integrand is zero, so `K_I` is twice the half-model value and `K_II = K_III = 0` exactly (returned as `0`).

**Foot and curvature.** Straight: the foot along the line, `kappa = 0`, the frame of ADR 0605 (`x1 = (x - x0) . e`). Circle
(centre `c` projected into the plane of `x0`, radius `R`): `N = unit(in-plane part of x - c)`, `x1 = rho - R`, `kappa = 1/R`,
`h = rho/R`; refused on the axis. Ellipse (`a > b`, axes `u, v = n x u`, in-plane `(X, Y)`): the foot `(a cos t, b sin t)` with
`t` the root of `f(t) = (a^2 - b^2) sin t cos t - a X sin t + b Y cos t` (the foot condition `(P - F) . F' = 0`) by Newton from
`t0 = atan2(a Y, b X)`, at most 60 steps, `|dt| <= 1e-14`; `N = unit(b cos t, a sin t)`, `kappa = a b/(a^2 sin^2 t + b^2 cos^2
t)^(3/2)`. Refused (key `"crack front"`): no convergence, `h <= 0` (a point beyond the centre of curvature), `r = 0`.

**The library.** In `crates/rcfd-fem/src/fracture.rs`, new items only:

* `FrontCurve::{Line, Circle, Ellipse}` and `tip_frame(&FrontCurve, x) -> Result<TipFrame>` (`foot`, `basis`, `x1`, `x2`,
  `curvature`);
* `curved_auxiliary(&FrontCurve, TipMode, x, mu, nu) -> Result<CurvedAuxiliary>` (the frame, `H`, `sigma^aux` global, and the
  local `G`, `sa`), and `CurvedAuxiliary::correction(&self, sigma, grad_u, q) -> f64` (`C1 + C2` above);
* `interaction_integral_curved(model, displacement, front, ring, CurvedTerms) -> Result<Vec<ModeFactors>>`: straight, circular
  and elliptical fronts, full or half-symmetric; `CurvedTerms::Corrected` (D1) or `CurvedTerms::DomainOnly` (the first integral
  only, for comparison: not exact on a curved front; a record); everything else (rings, `q`, refusals of the loads, the boundary,
  the elements, one material) as `interaction_integral`;
* `penny_crack_half_cylinder(&PennyCrack) -> Result<CrackedHalfCylinder>` (D4).

`interaction_integral` (the straight path of ADR 0605) is **not changed**: it still refuses curved and half-symmetric fronts,
and the ADR 0605 checks run unchanged (c4, D5).

### D2. The closed forms of the penny under inclined tension (c3, c5), derived and verified

Remote uniaxial tension `sigma` along `d = (0, sin beta, cos beta)` on a penny crack of radius `a` in the plane `z = 0` (normal
`n = z`): the remote state `sigma d d` has on the crack plane the normal stress `sigma_nn = sigma cos^2 beta` and the shear
`tau = sigma_yz = sigma sin beta cos beta` (along `y`). Subtracting the uniform state leaves the faces loaded by `-sigma_nn` and
`-tau y`.

* **Mode I.** `K_I = 2 sigma_nn sqrt(a/pi)`, uniform (Sneddon; the reference of ADR 0547 PH1, measured there within `2.2e-3`).
* **Shear.** The front angle `theta` from `x` about `z`, `e = (cos theta, sin theta, 0)`, `b3 = e x n = (sin theta, -cos theta,
  0)`. The closed forms (Kassir and Sih 1966, Tada handbook, from memory) for a shear `tau` along `y`:

  `K_II = A_II sin theta`, `A_II = 4 tau sqrt(a/pi)/(2 - nu)`; `K_III = -A_III cos theta`, `A_III = 4 (1 - nu) tau sqrt(a/pi)/(2 - nu)`

  in the frame `(e, n, e x n)` of the code (signs: below).

**Verification (c5).** (i) *Fourier reduction.* For a slip `b(x, y)` on the plane `z = 0` in an infinite body, a single Fourier
mode `b^ e^(i k . x)` is a 2-D problem: plane strain in the plane of `k` and `z` for the component of `b^` along `k^`, antiplane
for the component along `k^_perp`. The 2-D closed forms of ADR 0605 D6 (mode II `K_II = tau sqrt(pi a)` with the slip `4 tau/E'
sqrt(a^2 - x^2)`; mode III `K_III = tau sqrt(pi a)` with the slip `2 tau/mu sqrt(a^2 - x^2)`; both measured there within
`6.2e-3` and `3.9e-3`) fix the symbols `E'/4 |k|` and `mu/2 |k|` (the same normalisation as Sneddon's opening, `E'/4 |k|`), so
the traction of a slip is `sigma_z = -|k| (E'/4 k^ k^ + mu/2 (I - k^ k^)) b^`. (ii) *The penny slip.* For `b = b0 sqrt(1 -
rho^2/a^2) y`, with `phi = int b/|x - x'| dA'` the potential of ADR 0605 D4 (`F^-1[b^/|k|] = phi/(2 pi)`), the traction is
`sigma_y = -b0 [mu/2 (-Lap phi) + (E'/4 - mu/2) (-phi_yy)]/(2 pi)`, `sigma_x = -b0 (E'/4 - mu/2) (-phi_xy)/(2 pi)`: uniform and
along `y` inside the disc iff `phi_xx = phi_yy = -pi^2/2` and `phi_xy = 0` there. Arithmetic 2 verifies this numerically at five
interior points (`phi_xx/(-pi^2/2) = phi_yy/(-pi^2/2) = 1.00000000`, `|phi_xy| <= 2e-11`), giving `sigma_y = -b0 mu pi (2 -
nu)/(8 (1 - nu) a)`. So `b0 = 8 (1 - nu) tau a/(pi (2 - nu) mu)`. (iii) *Near-front values.* Near the front the slip is `b0
sqrt(2 r/a)` along `y`; its component along `e` is the mode-II jump `4 (1 - nu) K_II/mu sqrt(r/(2 pi))` and along `e x n` the
mode-III jump `4 K_III/mu sqrt(r/(2 pi))` (ADR 0605 D5 fields, the jumps at `theta = +/- pi`), which gives `A_II` and `A_III`
above (arithmetic 2: `1.327505` and `0.929253` per unit `tau sqrt(a)` at `nu = 0.3`, equal to the formulas to `1e-15`). (iv)
*Signs.* `tau > 0` pulls the `+z` half towards `+y`, so the jump (upper minus lower, the upper face at `theta_local = +pi`) is
along `+y`: `K_II = A_II (e . y) = A_II sin theta`, `K_III = A_III ((e x n) . y) = -A_III cos theta`. (v) *`nu -> 0`*: `A_II =
A_III = 2 tau sqrt(a/pi)`, Sneddon's constant: at `nu = 0` the two symbols are equal and the slip problem is the opening problem
(consistent). (vi) *Energy (circumferential average).* `Delta U = 1/2 int tau b dA = pi tau b0 a^2/3 ∝ a^3`; `G_mean = d(Delta
U)/da/(2 pi a)` equals the front average of `K_II^2/E' + K_III^2/(2 mu)` (arithmetic 2: ratio `1 + 2e-11`). (vii) *Against the
existing domain integral (FE).* The checks PM6 and PM7 compare the ring-3 J of `j_integral` (ADR 0547, the independent energy
path) with `G = (K_I^2 + K_II^2)/E' + K_III^2/(2 mu)` of these closed forms, station by station and as the front average.

For `beta = 45 deg`, `nu = 0.3`: `K_I = 0.564190`, `A_II = 0.663752`, `A_III = 0.464627` (times `sigma sqrt(a)`).

**The reference of a station.** `M` is linear in `K`, so a station value is the hat average `int phi_j K ds / int phi_j ds` of
the closed form (not `sqrt(<K^2>)` as for J in ADR 0605 D4). The test computes it by Gauss-Legendre on each side of the station
(16 points on 64 panels, `ds = a dtheta`), and for J the hat average of `G`; the point values are printed.

### D3. The elliptical front (c2)

Mode I on the embedded elliptical crack of ADR 0605 (half-symmetric quarter block, `b = a/2`): `K_I` from
`interaction_integral_curved` against Irwin's form (ADR 0605 D4), the station reference now the *linear* hat average of
`K_Irwin` over the exact ellipse, and against `plane_strain_k` of `j_integral` on the same mesh and ring. Meshes: ADR 0605's
coarse (`EF`) and fine (`EH`) ellipses unchanged. (No mixed-mode elliptical case: no closed form is available to me that I can
verify; D7.)

### D4. The full model of the penny (c3, c6)

The half-symmetric penny of ADR 0547 cannot carry shear (modes II and III are antisymmetric about the crack plane). The
inclined load `d = (0, sin beta, cos beta)` is symmetric under the reflection `x -> -x`, so the model is the half `x >= 0` of the
whole cylinder, both sides of the crack, with `u_x = 0` on `x = 0`: `penny_crack_half_cylinder(&PennyCrack)` builds the 2-D
quarter of ADR 0547 D8 (the same nodes and quads, code duplicated, not shared, so that `penny_crack_cylinder` stays as it is),
reflects it in `y` (nodes on `y = 0` shared, the reflected quads reversed to keep the orientation), extrudes it over `-H <= z <=
H` with the levels of the penny mirrored, and doubles the nodes of the plane `z = 0` inside the crack (`rho < a (1 - 1e-9)`: the
lower face); the front nodes (`rho = a`) are shared. Hex8, then `upgrade_to_hex20` and the circumferential mid-edge nodes moved
onto their circles (as ADR 0547). Sets: element `all`; nodes `all`, `x0` (`x = 0`), `top_centre` `(0, 0, H)`, `bottom_centre`
`(0, 0, -H)`; surfaces `top`, `bottom`, `side` (the boundary faces at `z = +/- H` and `rho = R`). Front: the nodes at `z = 0`,
`rho = a`, from `theta = -pi/2` to `pi/2` (`2 n_theta` edges, `2 n_theta + 1` stations), `crack_front()` circular about the
origin, normal `z`, not half-symmetric. The parameter of ADR 0547 (`R atan2`, from the first node) runs from `0` to `pi R`,
inside `(-pi, pi]`.

Loads: the tractions of the uniform state `sigma d d`: `sigma cos beta d` on `top`, `-sigma cos beta d` on `bottom`, `sigma
(d . m) d` on `side` with `m = (x, y, 0)/rho` (`traction_field_loads`); none on `x = 0` (`d_x = 0`). Supports: `u_x = 0` on `x0`;
`u_y = u_z = 0` on `top_centre`, `u_y = 0` on `bottom_centre` (the three rigid motions left by the mirror; the loads are
self-equilibrated, so no reaction beyond the rounding of the consistent loads). `beta = 45 deg`, `sigma = 1e8`.

* **PM (heavy, `#[ignore]`):** `PennyCrack::new(a, 10 a, 10 a, a/16, 3, 8, 0.25 a, 1, 0.5 a, 1.4, 1.5)`, about 9 150 Hex20 and
  120 000 unknowns (arithmetic 3), `StaticSolver::SupernodalDirect`, rings 2 to 4, 17 stations.
* **PC (fast):** the PF spec of ADR 0547 (`a/8`, 2, 4, `0.25 a`, 1, `0.5 a`, 1.5, 1.6, `R = H = 10 a`), about 2 560 Hex20 and
  35 000 unknowns, supernodal, ring 2, 9 stations.

Finite body: `R = H = 10 a`; for the penny mode I ADR 0547 estimated about `(a/R)^3 = 1e-3`; the same order for shear (estimate).
Mesh error: PF (`a/8`) measured `1.3e-2` in mode I at ring 2, PH (`a/32`) `2.2e-3`; for `a/16` I estimate `5e-3` in mode I and
`1e-2` in II and III.

At the two end stations (`theta = +/- pi/2`, on the mirror plane) `K_III`'s closed form is zero and the computed value carries the
end-face term (D1): a record. `K_I` and `K_II` are checked at every station.

### D5. The straight front unchanged (c4)

`interaction_integral` is not edited; `tests/sa_fracture_modes.rs` and `tests/sa_fracture.rs` are not edited and are run. In
addition `interaction_integral_curved` on the inclined plate of ADR 0605 (`I`, straight front, `kappa = 0`) is compared with
`interaction_integral`: the corrections vanish identically and the frame is the same, so the two agree to rounding (MS).

### D6. Opt-in (c8)

Only new items in `fracture.rs` (the enum `FrontCurve` and `CurvedTerms` `#[non_exhaustive]`); no existing line of `fracture.rs`
is changed (the module documentation gains a bullet). No other library file changes; no case-file key. Tests in a new file
`crates/rcfd-fem/tests/sa_fracture_curved.rs`.

### D7. Limits (not covered)

* Plane cracks only (the front in a plane, the faces the plane): no non-planar front, no crack-face traction, no body force,
  no thermal strain, one isotropic material in the domain (as ADR 0605).
* Curved fronts given as a closed-form curve (circle, ellipse); no front given by nodes only (a spline front is a follow-up).
* Mixed mode checked against closed forms on the penny only; on the ellipse mode I only (no verified closed form of the
  elliptical crack under shear is available to me).
* `K_III` at stations on a mirror plane carries the end-face term (record); `K_I`, `K_II` there are exact by symmetry (D1).
* The penny model uses the `x`-mirror of the inclined load; a general load direction needs the whole cylinder, which the
  front parametrisation (a closed front, no end station) does not support (ADR 0547's `hat` has two ends).

## Arithmetic done beforehand (disclosed)

1. **The correction terms** (`py/curv.py`, numpy): the Williams gradient of ADR 0605 (ported), the frames of D1 for a circle
   (`a = 0.01`, foot angle 0.3) and an ellipse (`a = 0.01`, `b = a/2`, feet `t = 0.2` and `1.2`, Newton foot), four offsets
   `(x1, x2)` of `1e-4` to `8e-4` above and below the plane, the three modes, a random uniform actual field and a random `q`:
   `div_j P_jk q_k` by central differences (`h = 1e-5 r`) against `C1 + C2`. My first formula (without the last term of C2)
   missed by up to `7.8e-3` of the scale `max|sigma| max|H|/r`; with it the largest difference is `1.2e-10` of that scale, while
   the correction itself is `4e-3` to `0.17` of the scale (circle `1e-2` to `4.5e-2`, ellipse at `t = 0.2` up to `0.17`).
2. **The penny under shear** (`py/shear_penny.py`): `phi` by the closed-form ray integral of ADR 0605 `gs.py` (4000 rays) and its
   second differences (`h = 0.02`) at `(0, 0)`, `(0.3, 0.2)`, `(0.5, -0.4)`, `(-0.6, 0.1)`, `(0, 0.8)`: `phi_xx/(-pi^2/2) =
   phi_yy/(-pi^2/2) = 1.00000000`, `|phi_xy| <= 1.9e-11`; the slip traction `-0.95369777 b0 mu/a` equal to `-mu pi (2 - nu)/(8 (1
   - nu))`; `A_II`, `A_III` as in D2 (to `1e-15`); the energy average ratio `1.00000000002`; `nu -> 0`: both `2`.
3. **Element counts** (`py/counts.py`): coarse half cylinder 2 560 Hex20, about 35 000 unknowns; medium (PM) 9 152 Hex20, about
   120 000 unknowns (ADR 0605's 87 663-unknown ellipse solved in 16.5 s; PM is estimated at 30 s to 90 s on a shared machine).
4. **Hat bias** (by hand): for `K = A sin theta` and a hat of half-width `Delta`, the station value is `A sin theta_j (sin(Delta/
   2)/(Delta/2))^2`, `1 - Delta^2/12`: `0.64 %` for PM (`Delta = pi/16`), `2.6 %` for PC: the references are the hat averages.

## Checks

Common: `E = 2e11`, `nu = 0.3`, `a = 0.01`, `sigma = 1e8`, `beta = 45 deg`. `A_I = 2 sigma cos^2 beta sqrt(a/pi)`. Errors of
`K_I` relative; of `K_II` and `K_III` as `|K_j - K_ref,j| / A_mode` (the closed forms have zeros along the front) and, in PM4,
also relative where `|K_ref,j| >= A_mode/2`. Every aggregated value asserted finite (F1).

| check | clause | set-up | limit | estimate |
|---|---|---|---|---|
| PM1 | c1, c3 | PM ring 3: `K_I` at all 17 stations against `A_I`: the largest relative error | `<= 0.03` | 0.007 |
| PM2 | c1, c3 | PM ring 3: `K_II` at all stations, `|K - K_ref,j|/A_II` | `<= 0.03` | 0.012 |
| PM3 | c1, c3 | PM ring 3: `K_III` at the 15 interior stations, `|K - K_ref,j|/A_III` | `<= 0.03` | 0.012 |
| PM4 | c3 | PM ring 3: `K_II` and `K_III` relative to `K_ref,j` where `|K_ref,j| >= A/2` (interior for III) | `<= 0.03` | 0.015 |
| PM5 | c1 | PM: per station and mode (III interior), `(max - min)/A_mode` over rings 2, 3, 4 | `<= 0.01` | 0.003 |
| PM6 | c5 | PM: `j_integral` ring 3 at every station against the hat average of `G` of the closed forms | `<= 0.04` | 0.015 |
| PM7 | c5 | PM: the front mean (weights) of J against that of `G` | `<= 0.02` | 0.01 |
| PM8 | c1 | PM: J at the interior stations against `(K_I^2 + K_II^2)/E' + K_III^2/(2 mu)` of the ring-3 `K` | `<= 0.02` | 0.005 |
| PMR | record | PM: `CurvedTerms::DomainOnly` values and errors at ring 3 and their ring spread; end-station `K_III`; point values; counts and solve time | none | |
| PC1 | c7 | PC ring 2: `K_I` at all 9 stations against `A_I` | `<= 0.08` | 0.02 |
| PC2 | c7 | PC ring 2: `K_II`, all stations, `/A_II` | `<= 0.08` | 0.03 |
| PC3 | c7 | PC ring 2: `K_III`, the 7 interior stations, `/A_III` | `<= 0.08` | 0.03 |
| PC4 | c7, c5 | PC ring 2: J at every station against the hat average of `G` | `<= 0.10` | 0.04 |
| PCR | record | PC: `DomainOnly` values | none | |
| EC1 | c2 | coarse ellipse (ADR 0605 EF mesh, half-symmetric) ring 2: `K_I` against `plane_strain_k` of the ring-2 J, every station | `<= 0.02` | 0.006 |
| EC2 | c2 | the same: `K_I` against the linear hat average of Irwin's form | `<= 0.08` | 0.035 |
| EC3 | D1 | the same: `K_II = K_III = 0` exactly (half-symmetric) | `== 0` | 0 |
| EH1 | c2 | fine ellipse (ADR 0605 EH mesh; heavy, `#[ignore]`) ring 3: `K_I` against the linear hat average of Irwin, every station | `<= 0.03` | 0.01 |
| EH2 | c2 | fine: `K_I` against `plane_strain_k` of the ring-3 J, every station | `<= 0.01` | 0.003 |
| EH3 | c2 | fine: `(max - min)/min` of `K_I` over rings 2, 3, 4, every station | `<= 0.01` | 0.003 |
| PQ1 | c1 | coarse quarter penny (ADR 0547 PF, half-symmetric, mode I) ring 2: `K_I` against `2 sigma sqrt(a/pi)`, every station | `<= 0.10` | 0.02 |
| PQ2 | c1 | the same against `plane_strain_k` of the ring-2 J | `<= 0.02` | 0.005 |
| MC1 | D1 | no solve: `CurvedAuxiliary::correction` against central differences (`h = 1e-5 r`) of `div_j P_jk q_k` built from `curved_auxiliary` at x +/- h: the points, field and `q` of arithmetic 1 (circle and ellipse, 3 modes, 12 points): `|diff|/(max|sigma| max|H|/r)` | `<= 1e-7` | 1e-10 |
| MC2 | D1 | `tip_frame` at the MC points: `|x - foot - x1 b1 - x2 b2|/a` and `|(x - foot) . b3|/a`, and the foot on the curve within `1e-12 a` | `<= 1e-12` | 1e-16 |
| MC3 | D1 | the size of the correction at the MC points (`|C1 + C2|/scale`, printed; the maximum at least `1e-3` shows the check is not vacuous) | `>= 1e-3` | 0.17 |
| MS | c4 | the inclined plate of ADR 0605 (`I`, its set-up copied), rings 2 to 4: `interaction_integral_curved` (Corrected and DomainOnly) against `interaction_integral`, `max |Delta K|/(sigma sqrt(pi a))` over stations, modes and rings | `<= 1e-12` | 1e-15 |
| R6 | D1, D4 | refusals: `tip_frame` of a point on the circle's axis ("axis"); of the ellipse centre ("centre of curvature"); `penny_crack_half_cylinder` with `n_theta = 3` ("penny crack"); `interaction_integral` (unchanged) of the half-cylinder's circular front ("straight") | refused (each) | |
| F1 | brief | every aggregated value finite | finite | finite |

The ADR 0605 checks (`sa_fracture_modes.rs`, every fast test and its heavy record EH) and ADR 0547's fast tests
(`sa_fracture.rs`) are run unchanged (c4).

## Negative controls and mutations

Controls: MC3 (the correction is not negligible where MC1 tests it); PMR and PCR (what the domain term alone gives); MS (the
corrections vanish exactly on a straight front). Mutations, each run once on a scratch copy (`f682-scratch/mut`, its own target
dir), the fast tests of `sa_fracture_curved.rs` (PM and EH not run for mutations):

| mutation | change | expected to be caught by |
|---|---|---|
| X1 | C1 (`div sigma^aux` term) dropped | MC1 |
| X2 | the first part of C2 dropped | MC1 |
| X3 | the last part of C2 (the rotated auxiliary strain against the actual stress) dropped | MC1 |
| X4 | the sign of `kappa` in the corrections flipped | MC1 |
| X5 | `h = 1` | MC1 |
| X6 | ellipse foot by the confocal angle (no Newton) | MC2 (and MC1) |
| X7 | ellipse curvature `1/a` | MC1 |
| X8 | the factor 2 of `K_I` in a half-symmetric model dropped | EC1, EC2, PQ1, PQ2 |
| X9 | the auxiliary frame fixed at the first front node (no per-point frame) | PC1 to PC3 |
| X10 | `b3 = n x e` in the curved path | PC3 |
| X11 | the line curvature `1/L` instead of 0 | MS |

## Decision rule

* **done**: PM1 to PM8, PC1 to PC4, EC1 to EC3, EH1 to EH3, PQ1, PQ2, MC1 to MC3, MS, R6, F1 met; every mutation caught by at
  least one check; the ADR 0605 and ADR 0547 tests pass unchanged; fmt and clippy clean; `baseline_hashes` passes.
* **partial**: the code built and the fast checks run, but a registered check missed (a `*_known_miss` record), or PM or EH not
  run, or a mutation not caught.
* **todo**: the curved interaction integral not built.

## Limits (not covered)

As D7. Also: the citations (Gosz and Moran 2002, Walters, Paulino and Dodds 2005, Kassir and Sih 1966, Tada) are from memory;
nothing used depends on them (D1 derived and verified by arithmetic 1; D2 derived from the 2-D forms of ADR 0605 and verified by
arithmetic 2 and by PM6, PM7).
