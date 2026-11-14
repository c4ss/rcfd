# ADR 0473: F-420 registration: stress stiffening of beams and shells (geometric stiffness of Beam2, Shell4 and Shell3, their centrifugal load and spin-softening mass) through the existing buckling, pre-stressed modal and rotating-part paths

Status: **REGISTERED 2026-11-14, written before any code of F-420 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-420 was written, compiled or run in the repository: the files listed under "What was
read" were read, and the arithmetic below was done by hand and evaluated with plain numpy scripts in the scratch folder
(`w49i-work/f420-scratch`: `probe_beam.py`, `probe_rot.py`; dense matrices of the planar Timoshenko element of ADR 0417 written
again in numpy, no rcfd code; disclosed, their numbers quoted below). The bit record B1 of the default paths is a test file
written and run on the base after this commit and before any library code (D9). Results go in the addenda at the end; nothing
above the addenda is changed after a run.

Roadmap item F-420 (deps F-419, F-391, F-392, F-395, F-396), the package SA-13 of the advanced tier (plan
[ADR 0310](0310-structural-solver-advanced-tier-plan.md), stub [ADR 0323](0323-geometric-stiffness-of-beams-and-shells-rotating-shell-blade.md)).
Number 0473 given by the lead. Branch `f420-work` from local main 4abff5fe, worktree `w49i-work/f420`, scratch
`w49i-work/f420-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no
unsafe code, the crate graph unchanged; existing tests not edited, except that a test whose refusal this work retires is kept as
an ignored record citing ADR 0291 and continued in an `_after_f420` copy (the pattern of `sa_scaffold.rs`, named in the brief);
clean-room.

## What F-420 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. Beam: Euler loads pi^2 E I/L^2 (pinned) and pi^2 E I/(4 L^2) (clamped-free) with the registered shear correction within 0.1 percent at 8 elements; under tension w_n^2 = (n pi/L)^4 E I/(rho A) + (n pi/L)^2 T/(rho A) within 0.5 percent for n = 1, 2. Rotating radial beam of square section: w_flap^2 - w_lag^2 = Omega^2 within 1e-6 Omega^2 and the stiffening slope equal to the Rayleigh quotient of F-396 within 1 percent. Shell: a simply supported square plate buckles at 4 pi^2 D/a^2 per width in one direction and at 2 pi^2 D/a^2 under equal compression in both, within 2 percent at 16 x 16 Shell4 and 4 percent at 32 x 32 x 2 Shell3; a stretched strip has the tension frequencies above within 1 percent. Equivalence: a flat plate strip twisted by 30 degrees, thickness/chord 0.02, clamped at the root and rotating: the Campbell table of the first four modes as a shell equals that of the solid model of F-396 (Hex20) within 2 percent at the registered speeds. Models of solid elements only keep their bits.

The brief adds: (b1) extend the existing geometric-stiffness path to beams, Shell4 and Shell3, no parallel one; (b2) models of
solid elements only keep their bits and `baseline_hashes` stays unchanged; (b3) a case-file key is the maintainer's call
(recommended, not added); (b4) probe a quantity before a `1e-12`-class limit, `1e-11` for sums of about `1e3` terms; (b5) the
records-and-copies scheme of `sa_scaffold.rs` (the latest X1 copy is `x1_modules_and_pending_after_f430`); (b6) keep the changes
local to the geometric stiffness and the element files (F-415 works in the modal and rotating code at the same time).

Clauses: (a0) every limit registered here; (a1) the pinned Euler load with the registered shear correction within `1e-3` at 8
elements; (a2) the clamped-free one likewise; (a3) the beam under tension, `n = 1, 2`, within `5e-3`; (a4) the rotating radial
beam's flap-lag identity within `1e-6 Omega^2`; (a5) its stiffening slope against the Rayleigh quotient of F-396 within `1e-2`;
(a6) the simply supported square plate in one direction, Shell4 16 x 16 within `2e-2`, Shell3 32 x 32 x 2 within `4e-2`; (a7) the
same under equal biaxial compression; (a8) the stretched strip's tension frequencies within `1e-2`; (a9) the Campbell table of the
twisted strip as a shell against the Hex20 solid within `2e-2`; (a10) solid-only models keep their bits.

Readings, registered:

- **(a1, a2) "the registered shear correction":** Engesser's, `P_cr = P_E/(1 + P_E/(kappa G A))` with `P_E` the Euler load of
  the case (Timoshenko and Gere, Theory of Elastic Stability, the section on the effect of the shearing force; cited from memory,
  derived in arithmetic 2). It is the load of the Timoshenko beam whose geometric work is `P int w'^2` on the total slope `w'`
  (D1), which is the beam of this ADR; the Haringx form (work on the section rotation) is not used.
- **(a3) "the tension frequencies":** the Euler-Bernoulli formula of the acceptance with `T` the axial force of the pre-stress
  computed by a static solve; the beam is slender (`L/b = 200`) so that the shear and rotary-inertia terms of the Timoshenko
  element of ADR 0417 are below `3e-4` (arithmetic 3).
- **(a4) "w_flap^2 - w_lag^2 = Omega^2":** at `Omega = 0.2 omega_0`, `omega_0` the lowest frequency at rest (the F-396 reading of
  ADR 0403 A3), on a radial cantilever of square section clamped on the axis. Arithmetic 4 shows that with the kinematic spin
  softening of D3 the identity holds to `1 - f_rot` with `f_rot = r_g^2 int w'^2/int w^2` the flap mode's rotary share, the beam
  counterpart of ADR 0403's `f_x`; the literal `1e-6` is therefore checked on a beam slender enough that `f_rot < 1e-7` (`L/b =
  2000`, R1), and the identity in its corrected form `1 - f_rot,h` on the F-396 beam itself (`L/b = 20`, R4).
- **(a5) "the stiffening slope equal to the Rayleigh quotient of F-396":** as ADR 0403 A4 and arithmetic 4 there: `S = (omega_flap^2
  - omega_0^2)/Omega^2` at `Omega = 0.2 omega_0` against the discrete quotient `Q_h = phi^T K_sigma,1 phi / phi^T M phi` of the
  rest flap mode (`RotatingPart::stiffening_quotient`); its continuum value `1.193336` (ADR 0403 arithmetic 4, Southwell's
  coefficient of the first flap mode) is the reference of the supplementary check R3.
- **(a6, a7) "buckles at 4 pi^2 D/a^2 per width":** the lowest positive factor of `buckling::factors` times the reference
  membrane force per unit width (N/m) of a static edge load; `D = E t^3/(12 (1 - nu^2))`; simple support `w = 0` on the four
  edges, the rotations free (the soft support of a Mindlin plate; `t/a = 0.01`).
- **(a8) "a stretched strip has the tension frequencies above":** a plate strip of length `L`, width `b`, `nu = 0` (so that the
  strip's bending stiffness is `E I = D b` with no anticlastic ambiguity), pinned at both ends (`w = 0` on the two end edges),
  tensioned by a static end load `T`, against the formula of (a3) with `I = b t^3/12`, `A = b t`; both shells.
- **(a9) "the Campbell table of the first four modes as a shell equals that of the solid model of F-396 (Hex20)":** the same
  twisted strip as a Shell4 model and as a Hex20 model, each through `RotatingPart::campbell` (F-396) at the registered speeds;
  for each of the first four tracks (the tracks start at rest in ascending order and are followed by the modal assurance) and
  each speed, `|omega_shell/omega_solid - 1|`. Shell3 is run as the supplementary check E2.
- **(a10) "Models of solid elements only keep their bits":** every result of a model without Beam2, Shell4 or Shell3 elements on
  every path this work touches is bit-identical to the base (B1); models with beams or shells on paths that already took them
  (static, modal, the shell centrifugal load) keep their bits as well (B1).

## What was read (no run)

- `AGENT_RULES.md`; the brief; `docs/roadmap.json` and `docs/ROADMAP.md` (F-420); ADR 0310 (SA-13), ADR 0323 (the stub).
- ADR 0390 (F-395: `K_sigma`, `Prestress`, pre-stressed modes, buckling), ADR 0403 (F-396: the rotating part, spin softening,
  the flap-lag identity in 3-D `1 - 2 f_x`, `f_x = 9.683e-4`, Southwell's `1.193336`, the speeds), ADR 0417 (Beam2), ADR 0432
  and ADR 0435 (Shell4, its body loads), ADR 0454 (Shell3), ADR 0466 and ADR 0470 (the drilling membranes), ADR 0465 (F-547).
- `crates/rcfd-fem/src`: `geometric.rs`, `geometric_struct.rs` (the stub), `buckling.rs`, `rotating.rs` (lines 1 to 720),
  `modal.rs`, `mass.rs`, `assemble.rs`, `loads.rs`, `beam.rs`, `shell.rs`, `shell3.rs` (the facet, the stiffness, the mass, the
  transformation, the stresses), `element.rs` (the trait, `Pending`), `model.rs` (the entry checks), `hex20.rs` (the upgrade),
  `recover.rs` (the field).
- `crates/rcfd-fem/tests`: `si_rotating.rs`, `si_rotating_flap_lag.rs`, `si_geometric.rs` (C8: the trait method of Beam2 and
  Shell4 refuses), `sa_ortho_rotating_default_bits.rs` (the digest pattern), `sa_scaffold.rs` (X1), `si_beam.rs` and
  `si_beam_loads.rs` (the centrifugal load on a beam refused, rows that this work retires: D7).

## Arithmetic done before the code (disclosed)

1. **The beam's geometric stiffness (D1).** The second variation of the Green-Lagrange strain energy at the fixed axial stress
   `sigma = N/A`, with the section kinematics `u(X) = u_c + theta x s`, `s = y e_2 + z e_3`, is `N int (u_c'^2 + v'^2 + w'^2) dx
   + (N/A) int (theta_1'^2 I_p + theta_2'^2 I_y + theta_3'^2 I_z) dx` (cross terms vanish on a centroidal section). The torsion
   term (Wagner) is kept, as in the 3-D frame element of McGuire, Gallagher and Ziemian (Matrix Structural Analysis, the
   geometric stiffness with `F_x I_p/(A L)`; cited from memory, NOT read); the two bending-rotation terms are of relative size
   `r_g^2 (pi/L)^2` (`2e-3` at `L/b = 20`) and are left out, as in that element and in the classical beam-column; with them the
   continuum load would not be Engesser's (reading (a1)). Each lateral term uses the deflection functions of the
   interdependent element of ADR 0417 (cubic, depending on the shear parameter `Phi`), integrated by the four-point Gauss rule of
   `beam.rs` (exact: the integrand is of degree 4); the axial and torsion terms use the linear functions (`N/L [1 -1; -1 1]`,
   `N I_p/(A L) [1 -1; -1 1]`). At `Phi = 0` the lateral block is the classical `N/(30 L) [36, 3L, -36, 3L; 3L, 4L^2, -3L, -L^2;
   ...]` (U1).
2. **Engesser's load is the limit of this beam.** `Pi = 1/2 int (E I psi'^2 + kappa G A (w' - psi)^2 - P w'^2)`: the Euler
   equations give `psi' = (1 - P/(kappa G A)) w''` and `E I (1 - P/(kappa G A)) w'''' + P w'' = 0`; pinned, `w = sin(pi x/L)`:
   `P = P_E (1 - P/(kappa G A))`, so `P = P_E/(1 + P_E/(kappa G A))`. Clamped-free: the shear condition at the free end makes the
   integration constant zero, `psi = (1 - P/(kappa G A)) w'`, `w = 1 - cos(beta x)`, `beta L = pi/2`: the same form with `P_E =
   pi^2 E I/(4 L^2)`. Numpy (`probe_beam.py`, `L = 1`, `b = 0.1`, `E = 2e11`, `nu = 0.3`, `kappa = 5/6`; `P_E/(kappa G A) =
   2.566e-2` pinned, `6.42e-3` clamped-free): at 8 elements the factor is above Engesser's by `3.42e-4` (pinned) and `2.23e-5`
   (clamped-free); 4, 16, 32 elements: `1.71e-3, 8.0e-5, 2.0e-5` and `1.1e-4, 5.2e-6, 1.3e-6` (order 2: the shear-flexible
   functions); against the raw Euler loads `-2.47e-2` and `-6.35e-3` (the negative control N1).
3. **The beam under tension.** `L = 1`, `b = 0.005` (`L/b = 200`), 16 elements, `T = pi^2 E I/L^2` (the tension term equal to
   the bending term at `n = 1`): numpy `omega_h^2/omega^2 - 1 = -5.1e-5` (`n = 1`) and `-2.58e-4` (`n = 2`; 8 elements `-3.6e-5`,
   `1.4e-4`), the shear and rotary inertia of the Timoshenko element. Without the geometric stiffness `omega_1^2` is half the
   formula's (N2).
4. **The rotating radial beam.** Spin softening (D3) is the kinetic energy of the motion perpendicular to the axis, `M_perp =
   int rho |P u(X)|^2`, `P = I - a a^T`, with the section kinematics: `M_perp = M - M_aa`, `M_aa = int rho (a . u)^2 dA dx = int
   (rho A (a . u_c)^2 + rho I_z (theta . (e_2 x a))^2 + rho I_y (theta . (e_3 x a))^2) dx`. For the beam along `x`, axis `z`,
   `e_2 = y`, `e_3 = z`: the lag subspace `(v, theta_3)` has `M_perp = M` exactly, the flap subspace `(w, theta_2)` has `M_perp =`
   its rotary mass `rho I_y theta_2^2` (the section rotation moves points along `x`). With the same `K` and `K_sigma` in both
   planes (square section) `lambda_lag = mu - Omega^2` exactly and `lambda_flap = mu - Omega^2 f_rot + O(Omega^4)`, `f_rot =
   phi^T M_perp phi/phi^T M phi` of the flap mode: `(omega_flap^2 - omega_lag^2)/Omega^2 = 1 - f_rot`. Numpy (`probe_rot.py`,
   root on the axis, `nu = 0.3`, the axial force of each element its average, D4): `L/b = 20`, 20 elements: `f_rot = 9.616e-4`
   (ADR 0403's 3-D `f_x = 9.683e-4`), identity `1 - 9.595e-4`, against `1 - f_rot` within `2.1e-6`; `L/b = 2000`, 20 elements:
   `f_rot = 9.68e-8`, identity `1 - 9.6e-8` (within `7e-10` of `1 - f_rot`; at 40 elements numpy's general eigensolver shows
   `1.7e-7` of noise). Stiffening `S(0.2 omega_0)/Q_h - 1 = -1.32e-3` (`L/b = 20`), `-4.9e-4` (`L/b = 2000`; ADR 0403's
   Euler-Bernoulli `1.19278/1.193336 - 1 = -4.7e-4`, the second-order term). `Q_h/1.193336 - 1 = +5.1e-4` at `L/b = 2000`, 20
   elements, element-average force (`+8.7e-8` with the exact quadratic force: the `5e-4` is D4's average), `-1.7e-3` at `L/b =
   20` (the shear deformation lowers the quotient). With the softening off the identity is `1 - 1 = 0` to first order (N3), with
   the stiffening off `S = -f_rot` (N4).
5. **Centrifugal load on a beam (D5).** `b = rho A Omega^2 P (X - p)` is affine along the element: the consistent load of the
   linear distribution with the element's own deflection functions (four-point Gauss, exact for cubic times linear), the axial
   part with the linear functions; the section's distributed moment `m = -rho Omega^2 (a_2 I_z (e_2 x a) + a_3 I_y (e_3 x a))`
   (local components of `a`; from `int s x P s dA`) with the rotation functions. For a radial beam `m = 0`. The nodal displacements
   of the axial bar are exact for this load (1-D), so the element-average force `E A (u_1 - u_0)/L` is exact.
6. **The plates (a6, a7, a8): estimates, no prototype.** For `sin(pi x/a)` on 16 bilinear intervals, `k h = pi/16 = 0.196`, the
   difference quotient changes `int w_x^2` by `-(k h)^2/12 = -3.2e-3` and the linear interpolation across by a like amount; the
   MITC4 bending energy errs by the same order: the Shell4 factor is predicted within `1e-2` of the plate value at 16 x 16 (limit
   `2e-2`), and the Mindlin correction at `t/a = 0.01` is about `2 pi^2 D/(a^2 kappa G t) = 5.6e-4` (the plate counterpart of
   Engesser's, the factor lowered by that fraction). Shell3 at 32 x 32 x 2:
   linear `w` per triangle (half the size), MITC3 with its stabilisation; predicted within `2e-2` (limit `4e-2`), the margin the
   least certain of this ADR. The strips (`L = 1`, `b = 0.05`, `t = 0.005`, 40 elements along, 2 across): `(2 pi/40)^2/12 =
   2.1e-3` per term; predicted within `5e-3` (limit `1e-2`). Plate values: `a = 1`, `t = 0.01`, `E = 2e11`, `nu = 0.3`, `D =
   18315.0`; `4 pi^2 D/a^2 = 7.2305e5` N/m, the next mode `(2 + 1/2)^2 = 6.25` (uniaxial), `(1, 2)` at `5` (biaxial); the
   membrane factors of the in-plane terms of D2 are of order `E t/N_ref = 2e6`, far above.
7. **The twisted strip (a9).** Chord `c = 0.05`, `t = 0.001` (`t/c = 0.02`), span `L = 0.15`, root on the axis, twist `theta(r) =
   (pi/6) r/L` about the span. Steel `E = 2e11`, `nu = 0.3`, `rho = 7850`. Plate estimates: `omega_1F = 3.516/L^2 sqrt(D/(rho t)) =
   238.7` rad/s, `1T ~ (pi/(2L)) (2 t/c) sqrt(G/rho) = 1312`, `2F ~ 1496`, `2T ~ 3936`, `3F ~ 4188` (the fourth and fifth 6 percent
   apart: six modes are computed so that tracking has the fifth), edgewise `~ 1.2e4`. Speeds `0, 120, 240, 360` rad/s (about `0,
   0.5, 1, 1.5 omega_1F`); the largest stress `rho Omega^2 L^2/2 = 1.1e7` Pa. Meshes: Shell4 12 (chord) x 36 (span), square
   cells of `4.17` mm; Hex20 the same 12 x 36 with one element through the thickness (aspect `4.2`), 3267 nodes; Shell3 the
   Shell4 cells cut by the diagonal from the cell's first to its third node. The solid is the shell's mid-surface `S(r, s) = (r, s
   cos theta, s sin theta)` offset by `zeta n(r, s)`, `n` the unit normal of the mid-surface (`(-s theta', -sin theta, cos
   theta)` normalised), so that both have thickness `t` along the normal (an offset along the section's own normal would thin
   the edges by `(s theta')^2/2 = 3.8e-3`). Warp of the Shell4 facets `~ theta' dr ds/(4 sqrt(dr ds)) = 1.1e-3` (limit 0.05).
   Expected differences: the discretisations (MITC4 at 12 x 36, Hex20 at aspect 4.2) and Mindlin against 3-D theory
   (`(t/c)^2`), each below `1e-2`; the 2 percent is the acceptance's.

## Design

### D1. The geometric stiffness of a beam (`beam.rs`)

`beam::local_geometric_stiffness(length, section, material, axial_force) -> ElementMatrix` (12 x 12, local axes) of arithmetic
1, and `beam::element_geometric_stiffness(coords, section, orientation, material, stress)`: `N = A e_1 . sigma e_1` from the
point's stress (global Voigt, tensor shear), then `T^T K_l T` with the rotation of `beam.rs` (made `pub(crate)` there, no other
change). Other components of the stress are not read (no shear-force or moment terms: Limits).

### D2. The geometric stiffness of a shell (`shell.rs`, `shell3.rs`)

In the facet's local axes, `K[(a, i), (b, i)] = int grad N_a . N grad N_b dA`, `i = u, v, w`, `N` the 2 x 2 membrane resultant
`t (Lambda sigma Lambda^T)_{alpha beta}`; Shell4 with its bilinear functions and the 2 x 2 rule (on the projected facet), Shell3
with its constant gradients; the rotations take no term (Limits). `shell::local_geometric_stiffness(facet, n)`,
`shell::element_geometric_stiffness(coords, thickness, stress)` (then `T^T K T` with the facet's transformation, offsets
included), the same in `shell3`. The in-plane terms (`u`, `v`) are those of the solid's `delta_ij` form (ADR 0390 D1). The
functions are those of the default membrane also for a block with a drilling membrane (Limits).

### D3. The spin-softening mass of a beam and a shell

Arithmetic 4: `M_perp = int rho |P u(X)|^2` with the element's kinematics. Beam: `M_l - M_aa` (local), `M_aa` by the four-point
rule on the twelve-unknown rows of `u_c` (axial linear, lateral the deflection functions), `theta_1` (linear), `theta_2`,
`theta_3` (the rotation functions, with the signs of `beam.rs`), then `T^T (.) T`. Shell: `u(zeta) = u + zeta theta x n`; per
pair of nodes `m_ab (delta_ij - a_i a_j)` on the translations and `m^r_ab R` on the rotations, `R = [[1 - a_2^2, a_1 a_2, 0],
[a_1 a_2, 1 - a_1^2, 0], [0, 0, 0]]` in local axes (`a` in local components; `|theta x e_3|^2 - (a . (theta x e_3))^2`), `m_ab` and
`m^r_ab = rho t^3/12 int N_a N_b` the scalars of the element's own consistent mass (the drilling rotation's rotary mass of the
mass matrix has no kinematic counterpart and takes no softening); then the facet's `T^T (.) T`. `beam::element_spin_softening_mass`,
`shell::element_spin_softening_mass`, `shell3::element_spin_softening_mass`.

### D4. The pre-stress of a structural model (`geometric_struct.rs`, `geometric.rs`)

`Prestress` keeps its layout: a Beam2, Shell4 or Shell3 element has one point (the length of the placeholder's rule, so that
`check_fits` is unchanged), holding a stress (global Voigt, tensor shear): for a beam `(N/A) e_1 e_1^T`, `N = E A e_1 . (u_1 -
u_0)/L` the element-average axial force (exact for a bar, arithmetic 5); for a shell `(1/t) sum N_{alpha beta} e_alpha e_beta^T`
from `shell::stresses` at the centre (with its thermal strain subtracted). `geometric_struct::prestress_from_static(model,
result)`; `Prestress::from_static` calls it for a model with any Beam2, Shell4 or Shell3 element, and keeps its former path
otherwise. A model with both solid and structural elements is refused there (`NotYetSupported`, item `"pre-stress"`, package
`"a follow-up of ADR 0473"`); `Prestress::uniform` takes structural models (the stress as given; D1, D2 read its resultants).
A pre-stress constructor `pub(crate) Prestress::from_parts` is added in `geometric.rs`.

### D5. The centrifugal load on a beam (`beam.rs`, `loads.rs`)

`beam::centrifugal_load_vector(coords, section, orientation, material, axis, origin, omega2)` of arithmetic 5. In
`loads.rs::add_body_load_with` the beam hook returns either the uniform line load (gravity, unchanged: the same call of
`add_line_load`) or an element vector (the centrifugal load, added force then moment per node as `add_line_load` does). The
refusal of the centrifugal load on a beam is retired (D7); the nodal body-force field and the temperature on a beam still refuse.

### D6. The paths (`geometric.rs`, `buckling.rs`, `rotating.rs`)

- `geometric::assemble_geometric`: a Beam2, Shell4 or Shell3 element takes `geometric_struct::element_geometric_stiffness`
  (dispatch before the solid call); `geometric::geometric_stiffness`, `prestressed_modes` (both paths), `buckling::factors` (both
  paths), `rotating::RotatingPart` take models with beams and shells: their entry checks become the structural checks
  (`check_supported_with_materials`, or `check_elements_supported_with_structures` for `geometric_stiffness`), which for a model
  of solid elements only are the former checks (the beam and shell checks return at once without such blocks).
- `rotating::spin_softening_mass`: a Beam2, Shell4 or Shell3 element takes D3 (dispatch before the solid call).
- Nothing else in `rotating.rs` (the whirl and Coriolis paths keep the solid routines and refuse a beam or a shell through them;
  F-415 works there).

### D7. Records retired

`geometric_struct::PENDING` becomes `None`: `x1_modules_and_pending_after_f430` (sa_scaffold.rs) becomes an ignored record
citing ADR 0291 and continues as `x1_modules_and_pending_after_f420`. The rows "N2 centrifugal on a beam" (refused) of
`si_beam.rs::n2_n3_other_paths_and_invalid_data_after_transformation_landed` and `si_beam_loads.rs::n2_n3_other_loads_and_invalid_line_loads`
are retired the same way: each test becomes an ignored record and continues as an `_after_f420` copy in which that row checks
instead that the static solve takes the load and that the reactions balance the total centrifugal force (limit `1e-12`, as G1
of ADR 0422).

### D8. Names in the topic documentation

`docs/reference-data.md` gains the F-420 section (sources, cited from memory). No case-file key is added (Q1).

### D9. The bit record of the default (B1)

`crates/rcfd-fem/tests/sa_geometric_struct_bits.rs` uses only the API of the base 4abff5fe and is run once on the base (this ADR
committed, no library line changed) to print its digests (FNV-1a 64 of the `to_bits` of the values, the digest of ADR 0291),
which become its constants; after the change it runs unchanged and compares. Models: a Hex20 cantilever (static pre-stress and
buckling on the elimination path; uniform pre-stress, `geometric_stiffness`, pre-stressed modes), a Hex8 cantilever on the
transformation path (spring support; buckling, pre-stressed modes), the Hex20 radial beam of F-396 (unit pre-stress, `K_sigma,1`,
`M_perp`, modes at 0 and 150 rad/s, a Campbell table of three speeds), the centrifugal and gravity load vectors of a Shell4 and
a Shell3 plate and the gravity load of a beam (the shared `loads.rs` hook), the static solve of a beam with gravity.

## Checks (limits fixed now)

Common: `E = 2e11`, `rho = 7850`; beams `nu = 0.3`, `kappa = 5/6` both directions, square section `b`, torsion constant
`0.1406 b^4` (from memory; it enters no check), orientation `(0, 1, 0)`, along `x`. Eigen-solves with `ResidualTest::Backward`,
tolerance `1e-12` (beams, plates, strips) and `1e-10` (the twisted strip). Every value is printed.

| Id | Clause | Quantity | Limit | Estimate |
|---|---|---|---|---|
| B1 | a1 | pinned, `L = 1`, `b = 0.1`, 8 elements, planar (`u_z = theta_x = theta_y = 0` at every node; `u = v = 0` at `x = 0`, `v = 0` at `x = L`), end load `-1000 e_x` at `x = L`, pre-stress from the static solve: `|lambda P_ref/P_Eng - 1|` | `<= 1e-3` | `3.4e-4` |
| B2 | a2 | clamped-free, same beam, all six at `x = 0`: the same | `<= 1e-3` | `2.2e-5` |
| B3 | a3 | `L = 1`, `b = 0.005`, 16 elements, pinned planar, end load `T = pi^2 E I/L^2` at `x = L`, pre-stressed modes from the static solve: `|omega_n^2/formula - 1|`, `n = 1, 2` | `<= 5e-3` each | `5e-5`, `2.6e-4` |
| R1 | a4 | radial cantilever `L = 1`, `b = 5e-4`, 20 elements, clamped at `x = 0` on the axis `z` through the origin, `RotatingPart` (4 modes): at `Omega = 0.2 omega_0`, `|(omega_flap^2 - omega_lag^2)/Omega^2 - 1|`; flap the mode of the four with the largest kinetic share in `u_z`, lag in `u_y` | `<= 1e-6` | `1e-7` |
| R2 | a5 | the same beam: `|S/Q_h - 1|`, `S = (omega_flap^2(0.2 omega_0) - omega_0^2)/(0.2 omega_0)^2`, `Q_h` the stiffening quotient of the rest mode of lowest frequency | `<= 1e-2` | `4.9e-4` |
| R3 | supp. | the same: `|Q_h/1.193336 - 1|` | `<= 2e-3` | `5.1e-4` |
| R4 | supp. (a4 diag.) | the F-396 beam as Beam2: `L = 1`, `b = 0.05`, `nu = 0`, 20 elements: `|(omega_flap^2 - omega_lag^2)/Omega^2 - (1 - f_rot,h)|`, `f_rot,h = phi_f^T M_perp phi_f/phi_f^T M phi_f` of the flap mode at the speed | `<= 1e-5` | `2.1e-6` |
| R5 | supp. | R4's `f_rot,h` against ADR 0403's 3-D `f_x = 9.683e-4`: `|f_rot,h/9.683e-4 - 1|` | `<= 2e-2` | `7e-3` |
| P1 | a6 | square plate `a = 1`, `t = 0.01`, Shell4 16 x 16, `w = 0` on the edges, `u_x = 0` on `x = 0`, `u_y = 0` at the node `(0, 0)`, the consistent edge force of `N_ref = 1000` N/m in `-x` on `x = a`, pre-stress from the static solve: `|lambda N_ref/(4 pi^2 D/a^2) - 1|` | `<= 2e-2` | `< 1e-2` |
| P2 | a7 | the same, `u_y = 0` on `y = 0` instead of the node, `N_ref` in `-x` on `x = a` and in `-y` on `y = a`: against `2 pi^2 D/a^2` | `<= 2e-2` | `< 1e-2` |
| P3 | a6 | P1 on Shell3 32 x 32 x 2 | `<= 4e-2` | `< 2e-2` |
| P4 | a7 | P2 on Shell3 32 x 32 x 2 | `<= 4e-2` | `< 2e-2` |
| S1 | a8 | strip `L = 1`, `b = 0.05`, `t = 0.005`, `nu = 0`, Shell4 40 x 2, `w = 0` on the end edges, `u_x = 0` on `x = 0`, `u_y = 0` at one root node, `theta_x = 0` on the end edges (no torsion mechanism), end force `T = pi^2 E I/L^2` shared consistently on `x = L`: `|omega_n^2/formula - 1|`, `n = 1, 2` (the modes with the largest share in `u_z`) | `<= 1e-2` each | `< 5e-3` |
| S2 | a8 | S1 on Shell3 40 x 2 x 2 | `<= 1e-2` each | `< 5e-3` |
| E1 | a9 | the twisted strip of arithmetic 7, Shell4 12 x 36 against Hex20 12 x 36 x 1, `RotatingPart::campbell` with 6 modes at `0, 120, 240, 360` rad/s: `max |omega_shell/omega_solid - 1|` over tracks 1 to 4 and the four speeds | `<= 2e-2` | `< 1e-2` |
| E2 | supp. | E1 with Shell3 12 x 36 x 2 | `<= 4e-2` | `< 2e-2` |
| E3 | supp. | in E1 and E2, no loss of stiffness, and every MAC of tracks 1 to 4 with their previous mode `> 0.9` | each | `~1` |
| U1 | supp. | `beam::local_geometric_stiffness` at shear factor `1e15` (`Phi ~ 1e-14`) against the classical matrix and `N/L`, `N I_p/(A L)`: max entry difference / max entry | `<= 1e-10` | `1e-14` |
| U2 | supp. | element `K_sigma` of a beam, a Shell4 (warped by `2e-3`) and a Shell3 in general position with a general stress: `max |K t|/max |K|` over the three unit translations `t`; `max |K - K^T|` | `<= 1e-11`; `0` | rounding |
| U3 | supp. | Shell4 and Shell3 (flat, general position, uniform stress): `q^T K q` for the field `u = 0`, `w = g . x` along the normal (`q` the nodal values) against `A g^T N g`, relative | `<= 1e-11` | rounding |
| U4 | supp. | beam: `sum_k M_perp(e_k) = 2 M` (global axes, max difference / max `|M|`); shell: for the rigid translation `d`, `d^T M_perp(a) d = m (1 - (a . d)^2)`, and for the uniform rotation `theta_0` (no translation) `rho t^3 A/12 |P (theta_0 x n)|^2`, relative | `<= 1e-11` | rounding |
| U5 | supp. | the centrifugal load of a beam in general position: the sum of its nodal forces against `rho A Omega^2 L P(x_mid - p)`, relative; on a beam parallel to the axis against `line_load_vector` of the uniform load, max difference / max | `<= 1e-12` each | rounding |
| D1 | supp. (D7) | the `_after_f420` copies: centrifugal on a beam solves, reactions balance `rho A Omega^2 L P(x_mid - p)`, relative | `<= 1e-12` | rounding |
| B1b | a10 | every digest of `sa_geometric_struct_bits.rs` equal to the base's | equal | equal |
| RF1 | supp. | `Prestress::from_static` of a model with a Hex8 block and a Shell4 block (disjoint) is refused: `NotYetSupported` with item `"pre-stress"` | holds | holds |

Negative controls:

| Id | Quantity | Expected |
|---|---|---|
| N1 | B1 and B2 against the raw Euler loads: `|lambda P_ref/P_E - 1|` | `> 1e-3` each (`2.5e-2`, `6.4e-3`): the correction is resolved |
| N2 | B3 without the pre-stress (`modal_analysis`): `|omega_1^2/formula - 1|` | `> 5e-2` (`0.5`) |
| N3 | R1 with `spin_softening(false)`: `|(omega_flap^2 - omega_lag^2)/Omega^2|` | `< 1e-3` |
| N4 | R2 with `stress_stiffening(false)`: `|S|` | `< 1e-2` |
| N5 | P1 with the load reversed (tension): no positive factor (`factors` empty) | holds |
| N6 | E1 with the shell's `stress_stiffening(false)` at 360 rad/s: `|omega_shell/omega_solid - 1|` of track 1 | `> 5e-2` |

## Planned mutations (on a scratch copy, one at a time)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | the `w` (x-z plane) term of the beam's geometric stiffness dropped | R1 (the flap is in that plane), U2 |
| MU2 | the rotational part of the beam's `M_aa` dropped | U4 |
| MU3 | the shell's membrane resultant without the thickness (`sigma` for `t sigma`) | P1, P2, S1 |
| MU4 | the shell pre-stress written in local axes as if global | E1 |
| MU5 | the beam's centrifugal load with the node-0 value at both ends | R3 |
| MU6 | the structural elements given a zero geometric stiffness in `assemble_geometric` | B1 |
| MU7 | the shell's rotational spin softening dropped | U4 |
| MU8 | the drilling rotation given softening (`R[2][2] = 1`) | U4 |

## Decision rule

- **done:** every check above met (B1 to B3, R1 to R5, P1 to P4, S1, S2, E1 to E3, U1 to U5, D1, B1b, RF1), every negative
  control as expected, the existing tests of the touched crates passing (with the records of D7), fmt and clippy clean.
- **partial:** a clause of the acceptance missed or not run, with the miss recorded as a `*_known_miss` test and diagnosed; or a
  supplementary check missed.
- **todo:** the geometric stiffness of beams or shells not reaching the buckling path.

## Limits (not covered)

- The beam's geometric stiffness comes from its axial force only: no shear-force, bending-moment or Wagner-moment terms beyond
  the torsion term of arithmetic 1 (no lateral-torsional buckling), and the bending-rotation terms of the section are omitted.
- The shells' geometric stiffness comes from the membrane resultants on the translations only (no `t^2/12` terms on the
  rotations, no transverse-shear pre-stress), with the default membrane's functions also for the drilling membranes.
- The pre-stress is constant per element (the beam's average axial force, the shell's centre resultants): a varying pre-stress
  is resolved to `O(h^2)`.
- A model with both solid and structural elements: no pre-stress from a static result (refused; a uniform one is taken).
- The whirl and Coriolis analyses, the cyclic sector and the transient path with beams or shells are not part of this work.
- Small rotations: large untwist is not covered.

## Open questions taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

- **Q1. A case-file key.** Options: (a) none: the solid case file's buckling, modal-with-pre-stress and rotating analyses take
  beam and shell blocks as they are; (b) a key to switch the structural geometric stiffness off. Recommendation (a), not
  applied here (the case file is the maintainer's call; nothing is added).
- **Q2. The bending-rotation terms of the beam's geometric stiffness.** Options: (a) omit (classical; Engesser in the limit);
  (b) keep (the 3-D section's full second variation; neither Engesser nor Haringx). Taken (a).
- **Q3. The flap-lag identity.** The literal `1e-6` holds only where `f_rot < 1e-6`; taken: the literal check on a slender beam
  (R1) and the corrected identity on the F-396 beam (R4).

## Amendment 1 (2026-11-14, written after the library code and before any run of the checks)

One check added; nothing above is changed. The section moment of the beam's centrifugal load (arithmetic 5) is zero on the
radial beam of R1 to R5, so no check above sees it. **U6** (supp.): the beam of U5 in general position, the resultant moment
about the axis point `p` of its consistent nodal load, `sum_i ((x_i - p) x F_i + M_i)`, against the exact moment of the body
force, `rho Omega^2 (A int (x_c - p) x P (x_c - p) dx + L m)` (Simpson's rule, exact for the quadratic integrand; `m` the section
moment of arithmetic 5): max component difference over the largest component, limit `<= 1e-12`. The consistent load gives the
exact resultant because a rigid rotation (`u_c = dtheta x (x_c - p)` linear, `theta = dtheta` constant) is in the element's
fields. Mutation **MU9**: the sign of the section moment reversed; expected to be caught by U6.

## Addendum 1 (2026-11-14): results

Each registered run made once (`crates/rcfd-fem/tests/sa_geometric_struct.rs`, release build); every value is printed by the
test.

| Check | Measured | Limit | Result |
|---|---|---|---|
| B1 pinned, `P_h/P_Eng - 1` (`P_h` 1.60433e7) | 3.43e-4 | `1e-3` | met |
| B2 clamped-free (`P_h` 4.08621e6) | 2.23e-5 | `1e-3` | met |
| B3 n = 1, 2 | 5.15e-5, 2.58e-4 | `5e-3` | met |
| R1 slender beam, `(w_f^2 - w_l^2)/Omega^2 - 1` | 9.69e-8 | `1e-6` | met |
| R2 `S/Q_h - 1` (`S` 1.193358, `Q_h` 1.193947) | 4.93e-4 | `1e-2` | met |
| R3 `Q_h/1.193336 - 1` | 5.12e-4 | `2e-3` | met |
| R4 F-396 beam, identity against `1 - f_rot,h` (identity 0.999039) | 2.61e-9 | `1e-5` | met |
| R5 `f_rot,h/9.683e-4 - 1` (`f_rot,h` 9.6087e-4) | 7.68e-3 | `2e-2` | met |
| P1, P2 Shell4 16 x 16 | 3.30e-3, 3.30e-3 | `2e-2` | met |
| P3, P4 Shell3 32 x 32 x 2 | 4.77e-3, 4.77e-3 | `4e-2` | met |
| S1 Shell4 n = 1, 2 | 9.84e-4, 5.12e-3 | `1e-2` | met |
| S2 Shell3 n = 1, 2 | 8.58e-4, 4.31e-3 | `1e-2` | met |
| E1 Shell4 twisted strip against Hex20, tracks 1 to 4, four speeds | 5.92e-3 | `2e-2` | met |
| E2 Shell3 twisted strip | 4.05e-2 | `4e-2` | **missed** (known miss, below) |
| E3 no loss of stiffness, lowest MAC (Hex20, Shell4, Shell3) | 0.999712, 0.999709, 0.999708 | `> 0.9` | met |
| U1 beam `K_sigma` at `Phi ~ 0` | 2.1e-16 | `1e-10` | met |
| U2 `|K t|/|K|` beam, Shell4 (warped), Shell3; symmetry | 7.4e-18, 2.3e-16, 3.5e-16; exact | `1e-11`; exact | met |
| U3 Shell4, Shell3 | 2.2e-16, 0 | `1e-11` | met |
| U4 beam sum; shell translations; flat shell rotations | 3.2e-16; <= 2.2e-16; <= 2.2e-16 | `1e-11` | met |
| U5 force sum; parallel beam | 2.3e-16, 4.6e-16 | `1e-12` | met |
| U6 resultant moment | 1.8e-15 | `1e-12` | met |
| D1 `si_beam.rs`, `si_beam_loads.rs`, `si_shell_transformation.rs` copies | 1.4e-14, 7.5e-15, 0 | `1e-12` | met |
| B1b digests | equal | equal | met |
| RF1 mixed Hex8 + Shell4 static pre-stress | `NotYetSupported { item: "pre-stress", .. }` | refused | met |
| N1 B1, B2 against raw Euler | 2.47e-2, 6.35e-3 | `> 1e-3` | as expected |
| N2 B3 n = 1 without pre-stress | 0.500 | `> 5e-2` | as expected |
| N3 R1 without spin softening | 1.2e-13 | `< 1e-3` | as expected |
| N4 without stress stiffening, `|S|` | 9.7e-8 | `< 1e-2` | as expected |
| N5 P1 in tension | no positive factor | empty | as expected |
| N6 E1 without stress stiffening at 360 rad/s, track 1 | 0.494 | `> 5e-2` | as expected |

**Set-up correction (E1 to E3, before any comparison).** The registered map of the Hex20 strip, `p -> S(p[1], p[0]) + p[2] n`,
swaps the first two box axes and so inverts every element (the first run stopped at the mesh: `InvertedElement`, Jacobian
-2.2e-9). The test takes the thickness coordinate as `-p[2]`: the same set of points (the box is symmetric in `p[2]`), the
element orientation kept. No limit, speed or mesh changed.

**E2 known miss (diagnosed).** The worst ratio is on the third track (nearly speed-independent, 460.73 Hz Shell3 against 442.80
Hz Hex20 at rest) at rest, 4.050e-2, and 4.006e-2 at 360 rad/s; the other three tracks are within 3.8e-3. The miss is in the
Shell3 rest stiffness of that mode (Shell4 is within 5.3e-3 of Hex20 on the same track), which F-420 does not change: the speed
trend of the track (461.630/460.730 = 1.00195) follows the Hex20 one (443.852/442.797 = 1.00238). Recorded as the ignored
`e2_twisted_strip_shell3_known_miss`; the limit was not changed.

**Mutations** (scratch copy, one at a time, the whole test file):

| Id | Expected | Caught by (measured) |
|---|---|---|
| MU1 | R1, U2 | R1 (1.19), R2, R4, U1 (1.0), N3; U2 did not catch it (dropping a plane keeps `K t = 0`) |
| MU2 | U4 | U4 beam sum (2.7e-4) |
| MU3 | P1, P2, S1 | P1, P2 (0.99), P3, P4, S1, S2 (99.6), E1, E3 |
| MU4 | E1 | E1 (0.467), S2 (0.127) |
| MU5 | R3 | R3 (3.3e-2), U5 (1.16), U6 |
| MU6 | B1 | B1, B2 (no factor: test panics), B3 (0.50), R2, R3, S1, S2, E1, P1 to P4 |
| MU7 | U4 | U4 Shell4 flat rotation (1.0) |
| MU8 | U4 | U4 Shell3 and Shell4 flat rotation (0.64) |
| MU9 | U6 | U6 (5.0e-3) |

Every mutation was caught by its expected check except MU1 by U2 (caught by R1 as registered first).

**Decision (rule above): partial.** Every acceptance clause is met (B1, B2, B3, R1, R2, P1 to P4, S1, S2, E1, a10 by B1b);
the supplementary E2 is missed and recorded as a known miss. The existing tests of `rcfd-fem` pass with the records of D7.

Process notes: the brief asked for the F-420 entries of `docs/ROADMAP.md` and `docs/roadmap.json` to be updated on the branch,
which AGENT_RULES rule 9 leaves to the lead; the brief was followed. A fourth record was retired beyond the three of D7: the
N4 row "centrifugal on a beam (unchanged)" of `si_shell_transformation.rs::s3_n2_to_n4_refusals` (ADR 0432), now an ignored
record continued as `s3_n2_to_n4_refusals_after_f420` with the D1 balance.

## For the lead: roadmap entries

> F-420 follow-up: the Shell3 rest stiffness of the twisted strip's third mode is 4.05 percent above Hex20 (Shell4 0.53
> percent); a diagnosis of the Shell3 membrane-bending coupling on warped triangles (CST membrane, MITC3 shear) against the
> Hex20 strip, before any change to Shell3.
> F-420 follow-up: the pre-stress of a static result on a model with both solid and structural elements (refused now, RF1).
> F-420 follow-up: the bending-moment, shear-force and Wagner terms of the beam's geometric stiffness (lateral-torsional
> buckling) and the `t^2/12` rotation terms of the shells' geometric stiffness.
