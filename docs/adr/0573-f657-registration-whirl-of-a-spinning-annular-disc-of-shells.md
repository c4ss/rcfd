# ADR 0573: F-657 registration: whirl of a spinning annular disc of shells (the rotary block of the shell Coriolis matrix)

Status: **REGISTERED 2026-11-15, written before any code of F-657 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-657 was written. The files under "What was read" were read, one
entry-path probe was run on the unchanged library (disclosed below: it printed only `Ok` or the error of each call, no
frequency), and the reference numbers below were computed by short scripts of the reference alone (no rcfd call;
`f657-scratch/reference.py`, `ref_speeds.py`, `kirchhoff.py`, `rf3.py`, `rf3cc.py`, disclosed). Results go in the addenda
at the end; nothing above the addenda is changed after a run.

Roadmap item F-657 (dep F-654), the follow-up proposed by [ADR 0569](0569-f654-registration-independent-whirl-reference-for-spinning-shells.md)
(its remaining question 2: the rotary block `m2 W^T [a x] W` of the shell Coriolis matrix of
[ADR 0561](0561-f647-registration-shells-on-the-rotating-cyclic-paths-and-the-full-rotor.md) D2 is zero on the band of ADR 0569
and has no independent whirl reference). Number 0573 given by the lead. Branch `f657-work` from 61033836, worktree
`w49i-work/f657`, scratch `w49i-work/f657-scratch`. This is a test-only item unless a defect is found.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored `*_known_miss`
test with the limit intact and the measured number); each registered run made once; defaults bit-identical; no edit of an
existing test, of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe
code; clean-room. `docs/ROADMAP.md` and `docs/roadmap.json` are not edited (the entries go in "For the lead"). Every choice
below that the maintainer would normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the
recommendation).

Two lessons of ADR 0569 are applied before the run, as the brief asks: (1) the sign of the whirl number of the backward root is
derived here for the negative root of the travelling-wave ansatz (D3, arithmetic 3), not carried over from the forward one; (2)
the no-Coriolis negative control's limit is set from the dense solver's rounding `c n eps omega_max` (ADR 0569 amendment 1), not
from `1e-10`.

## What F-657 asks

The acceptance, binding:

> Every limit not stated here is registered in the ADR before the run. The transverse modes of a flat annular plate clamped at its inner radius and spinning about its normal, from RotatingSector::whirl_modes with Shell4 and Shell3, against a semi-analytical reference derived in the ADR (the Mindlin plate with rotary inertia and the centrifugal membrane stress of the disc, solved per nodal diameter to 1e-8): frequencies at three speeds up to half the first frequency within the registered discretisation error at two meshes, the observed order within 0.5 of the expected one, and the rotating-frame split of each nodal-diameter pair against the reference within the registered limit; models without shells keep their bits.

Clauses: **a0** every limit registered here; **a1** the transverse modes of a flat annular plate clamped at its inner radius,
spinning about its normal, from `RotatingSector::whirl_modes` with Shell4 and with Shell3; **a2** against a semi-analytical
reference derived here (D3: the Mindlin plate with rotary inertia and the centrifugal membrane stress of the disc), solved per
nodal diameter to `1e-8`; **a3** three speeds up to half the first frequency; **a4** within the registered discretisation error at
two meshes; **a5** the observed order within 0.5 of the expected one; **a6** the rotating-frame split of each nodal-diameter pair
against the reference within the registered limit; **a7** models without shells keep their bits.

## What was read

`AGENT_RULES.md`; the brief; the F-657, F-654 and F-647 entries of `docs/roadmap.json`; ADR 0569 (all of it: D1 to D6, amendment
1, addendum, remaining questions); ADR 0561 D2 (the Coriolis matrix of the director `u + z theta x e3`); sources at 61033836:
`rotating.rs` (`RotatingOptions`, `RotatingPart::new`: the static solve of the centrifugal load at 1 rad/s with the model's
constraints, `Prestress::from_static`; `operator`: `K + Omega^2 (s K_sigma,1 - c M_perp)`), `rotating/whirl.rs`
(`RotatingSector::whirl_modes`, `whirl_at`: `G = 2 Omega C` on the doubled pair, the dense solver, `whirl_number` `kappa = 2 re^T C
im / (re^T M re + im^T M im)`, `Whirl::of` with `WHIRL_TOLERANCE = 1e-6`, the wave sense `tau`), `geometric.rs`
(`assemble_geometric`: without the opt-in of ADR 0508 the shell geometric stiffness acts on the three translations only),
`geometric_struct.rs` (`element_geometric_stiffness_with`), `shell.rs` (`SHEAR_FACTOR = 5/6`, `homogeneous_section`: `D` of plane
stress, `kappa G t`; `local_mass`: `rho t` on the translations, `rho t^3 / 12` on the three rotations; `local_geometric_stiffness`;
`rotary_softening_factor`, `local_spin_softening_mass`; `coriolis_factors`, `local_coriolis`, `model_coriolis`), `shell3.rs`
(`local_mass`, `stabilised_shear` `t^2 / (t^2 + alpha h^2)`, `SHEAR_STABILISATION = 0.1`, `local_coriolis` using
`coriolis_factors`), `tests/sa_ring_whirl_shells.rs` and `tests/sa_rotating_cyclic_shells.rs` (the helpers and the clamp pattern;
copied where used, not edited), `tests/si_shell_rotating_disc.rs` (the free-bore disc of ADR 0439; its closed form is not the one
needed here, the bore is clamped).

**Entry-path probe (disclosed).** A temporary test file (`tests/zz_probe_f657.rs`, deleted after the run, never committed) built
the D2 sector at both meshes (Shell4 and Shell3, clamped bore) and printed only `Ok` or the error of each call on the unchanged
library: `RotatingSector::new` with `coriolis(true)` and `stress_stiffening` off and on (the static centrifugal solve of the
clamped sector): `Ok` for every kind and mesh; `whirl_modes(k, 1000)` for `k = 1, 2, 3`: `Ok` with modes in every case. No
frequency was printed.

## Design

### D1. Why the disc, and what it tests

For a flat plate spinning about its normal `n` the transverse motion `w` is along `n`, so the translation block `m0 [a x]` of the
shell Coriolis matrix does not act on it (`n x w n = 0`), `m1 = 0` for a homogeneous shell, and the only gyroscopic term of the
transverse modes is the rotary block `m2 W^T [a x] W`, `m2 = rho t^3 / 12`: the Coriolis force on the in-plane motion `z d` of the
fibres (`d = theta x e3`, the fibre's in-plane displacement per unit `z`). In the thin (Kirchhoff) limit this term vanishes to
order `(t k)^2` and the transverse modes do not split in the rotating frame (Lamb and Southwell 1921, cited from memory; it
follows from D3 with `m2 -> 0`). A moderately thick plate (`t / (b - a) = 1/12`) is taken so that the split is large against
the dense solver's rounding (arithmetic 5) and is carried entirely by the rotary block: this is the block that ADR 0569's band
could not test.

On a flat plate the transverse block (`w`, the two bending rotations) and the in-plane block (the in-plane translations and the
drilling rotation) are uncoupled in every matrix of the whirl problem (stiffness of a homogeneous facet, the mass, the spin
softening `P = I - n n^T` and the rotary factor, the geometric stiffness per translation component, the Coriolis blocks): the
coupling is the rounding of the facet axes (D5, TR).

### D2. Geometry, material and meshes (fixed now)

Axis `n = (1, 2, 2) / 3` through `o = (0.3, -0.2, 0.5)`, in-plane basis `e_a = (2, -1, 0) / sqrt 5`, `e_b = n x e_a` (as ADR 0569
D2). Inner radius `a = 0.04` m, outer `b = 0.1` m, thickness `t = 5e-3` m, steel `E = 2e11` Pa, `nu = 0.3`, `rho = 7850` kg/m^3,
`BlockProperty::shell(t)`. `N = 48` sectors. Node `(i, j)` of a sector at `o + r_j (cos phi_i e_a + sin phi_i e_b)`, `r_j = a +
(b - a) j / n_r`, `phi_i = 2 pi i / (N n_c)`, `i = 0 .. n_c`, `j = 0 .. n_r`; quads `(i, j), (i + 1, j), (i + 1, j + 1), (i, j +
1)` (Shell4), or each quad cut into two Shell3 along the diagonal from its first node (as ADR 0569). Node sets `low` (`i = 0`),
`high` (`i = n_c`) and `clamp` (`j = 0`), the six components of `clamp` zero (`Constraint::displacement`), `Constraint::cyclic_with
("low", "high", n, o, 48, CyclicOptions::new().shells(true))`, the material on `all`. All nodes lie in one plane: the facets are
planar trapezoids.

Two meshes, a uniform refinement by 2 in both directions: **coarse** `n_c = 1`, `n_r = 8`; **fine** `n_c = 2`, `n_r = 16`. The
element angle `Delta = 2 pi / (N n_c)` (coarse `0.1309`, fine `0.0654`). Harmonic indices **`k = 1, 2, 3`** (nodal diameters 1, 2
and 3, each a pair). Reduced unknowns of a pair: `2 x 6 n_c n_r` (96 coarse, 384 fine), below `MAX_WHIRL_UNKNOWNS = 1500`.

### D3. The reference (first principles): the Mindlin annular plate in the rotating frame, per nodal diameter

Polar coordinates `(r, theta)` about `n` in the plate's plane, `theta` increasing in the sense of the rotation; `w` the transverse
displacement, `d = d_r e_r + d_theta e_theta` the in-plane displacement per unit `z` of the fibre (so the in-plane displacement
is `z d`). Strains of the Mindlin plate: curvatures `k_rr = d_r,r`, `k_tt = (d_r + d_theta,theta) / r`, twist `chi = d_r,theta /
r + d_theta,r - d_theta / r`; shear `g_r = d_r + w_,r`, `g_t = d_theta + w_,theta / r`. Strain energy per unit area `(D/2)[k_rr^2
+ k_tt^2 + 2 nu k_rr k_tt + ((1 - nu)/2) chi^2] + (kappa_s G t / 2)(g_r^2 + g_t^2)`, `D = E t^3 / (12 (1 - nu^2))`, `kappa_s =
5/6`, `G = E / (2 (1 + nu))` (the shells' section: `homogeneous_section`). The membrane pre-stress at 1 rad/s, `N_r(r)`,
`N_t(r)`, adds `(Omega^2 / 2)(N_r w_,r^2 + N_t (w_,theta / r)^2)` (the geometric stiffness of ADR 0473 D2 on the transverse
translation; the rotations take none without the opt-in of ADR 0508, so the reference takes none either).

Kinetic energy in the frame turning with `Omega` about `n` (as ADR 0569 D3, integrated through the thickness with `int rho dz = rho
t`, `int rho z dz = 0`, `int rho z^2 dz = I = rho t^3 / 12`): the part quadratic in the motion is `(rho t / 2) w_t^2 + (I / 2)
|d_t|^2 + Omega I d_t . (n x d) + (Omega^2 / 2) I |d|^2`; the translation's Coriolis and spin-softening terms vanish for `w n`
(D1). With `n x e_r = e_theta`, `n x e_theta = -e_r`: `d_t . (n x d) = d_theta,t d_r - d_r,t d_theta`. Lagrange: `I d_r,tt - 2
Omega I d_theta,t - Omega^2 I d_r + dU/dd_r = 0`, `I d_theta,tt + 2 Omega I d_r,t - Omega^2 I d_theta + dU/dd_theta = 0`, `rho t
w_tt + dU/dw = 0` (the same structure as the ring of ADR 0569 D3, with `w -> d_r`, `v -> d_theta`, `m -> I`).

The travelling wave `w = W(r) cos(phi)`, `d_r = R(r) cos(phi)`, `d_theta = T(r) sin(phi)`, `phi = n theta - omega t` (`omega > 0`
travels in the sense of the rotation): every strain is `cos phi` (`k_rr = R'`, `k_tt = (R + n T)/r`, `g_r = R + W'`) or `sin phi`
(`chi = T' - (T + n R)/r`, `g_t = T - n W / r`), so for `n >= 1` the `theta` integrals give the same factor `pi` to every term
and the problem is real and one-dimensional on `[a, b]`:

`A(omega) x = 0`, `A(omega) = K_e - omega^2 M + 2 Omega omega G`, `K_e = K + Omega^2 (s K_sigma - c M_I)`,

with the quadratic forms (weight `r dr`): `x^T K x = int {D [R'^2 + ((R + nT)/r)^2 + 2 nu R'(R + nT)/r + ((1 - nu)/2)(T' - (T +
nR)/r)^2] + kappa_s G t [(R + W')^2 + (T - nW/r)^2]}`, `x^T K_sigma x = int [N_r W'^2 + N_t n^2 W^2 / r^2]`, `x^T M x = int [rho t
W^2 + I (R^2 + T^2)]`, `x^T M_I x = int I (R^2 + T^2)`, `x^T G x = 2 int I R T`; `s = 1` with stress stiffening, `c = 1` (spin
softening, on in both sets). Clamped at `r = a` (`W = R = T = 0`), free at `r = b` (natural). Positive roots `omega` are forward
waves, negative roots backward waves; `omega_f` the smallest positive root, `omega_b` the magnitude of the negative root of the
same (first radial) family, the rotating-frame split `omega_b - omega_f`.

**The plane-stress pre-stress (closed form, derived).** Equilibrium `(r sigma_r)' - sigma_t + rho Omega^2 r^2 = 0` with
Hooke's law of plane stress gives `u'' + u'/r - u/r^2 = -(1 - nu^2) rho Omega^2 r / E`, so `u = C1 r + C2 / r - alpha r^3`,
`alpha = (1 - nu^2) rho / (8 E)` at 1 rad/s; `sigma_r = (E / (1 - nu^2))[(1 + nu) C1 - (1 - nu) C2 / r^2 - (3 + nu) alpha r^2]`,
`sigma_t = (E / (1 - nu^2))[(1 + nu) C1 + (1 - nu) C2 / r^2 - (1 + 3 nu) alpha r^2]`. `u(a) = 0` gives `C2 = alpha a^4 - C1 a^2`,
`sigma_r(b) = 0` gives `C1 = alpha [(3 + nu) b^4 + (1 - nu) a^4] / [(1 + nu) b^2 + (1 - nu) a^2]`. `N_r = t sigma_r`, `N_t = t
sigma_t`. (The library's static solve is of the same linear problem, `K u = f` with the centrifugal load on the undeformed
geometry and the clamp; its pre-stress is discretised, and its error is part of the FE error of set F.)

**Numerics (solved to `1e-8`).** A p-version Galerkin discretisation in `r`: `n_e` equal elements, Lagrange polynomials of degree
`p` on the Gauss-Lobatto points for each of `W, R, T`, Gauss-Legendre quadrature of `p + 4` points (the integrands are rational in
`r`, analytic on `[a, b]`). The registered reference is `(p, n_e) = (8, 8)` (189 unknowns); the check RF1 compares it with `(10,
12)`. For each root, the fixed point `omega = root of x^T A(omega) x = 0 of the lowest eigenvector x of (K_e + 2 Omega omega G) x
= mu M x`: from `omega_0 = 0`, at each step a Cholesky factor of `K_e + 2 Omega omega G` (positive definite: arithmetic 4), four
inverse iterations on `x`, then `omega` the root of `-m omega^2 + g omega + k = 0` (`k = x^T K_e x`, `m = x^T M x`, `g = 2 Omega
x^T G x`) of the sign wanted, until `|delta omega| <= 1e-13 omega` or 100 steps. The fixed point is contracting (its slope is of
order `kappa Omega / omega`, about `1e-3`), and the root of the quotient is stationary at the eigenvector (the matrix is
symmetric), so the root converges quadratically in the error of `x`. The determinant changes sign at each root and the count of
negative eigenvalues of `A` increases by one at each root away from `0` on either side (for any fixed `x` the quadratic in
`omega` has one positive and one negative root, `k, m > 0`), so the lowest eigenvector gives the first root of each sign.

**The whirl number of the reference (lesson 1).** The library's `kappa = Im(phi^H C phi) / phi^H M phi` with `u = Re(phi e^{i
omega_L t})`, `omega_L > 0`. A positive root `omega_s = omega_L`: `cos(n theta - omega_L t) = Re(e^{-i n theta} e^{i omega_L t})`,
`sin(n theta - omega_L t) = Re(i e^{-i n theta} e^{i omega_L t})`, so `phi_r = R e^{-i n theta}`, `phi_t = i T e^{-i n theta}`. A
negative root `omega_s = -omega_L`: `phi_r = R e^{i n theta}`, `phi_t = -i T e^{i n theta}`. With `C` on the director `I (n x)`:
`phi^H C phi = I int [conj(phi_r)(-phi_t) + conj(phi_t) phi_r]` = `-2 i I R T` (forward), `+2 i I R T` (backward) per unit area
and angle. Hence **`kappa_f = -x_f^T G x_f / x_f^T M x_f`, `kappa_b = +x_b^T G x_b / x_b^T M x_b`**, each at its own eigenvector
(arithmetic 3 repeats it on the ring of ADR 0569 to confirm that it reproduces that ADR's amended signs).

### D4. The two effect sets and the speeds

- **Set S** (Coriolis and spin softening, no stress stiffening): `RotatingOptions::new(2).coriolis(true).stress_stiffening(false)`,
  `RotatingSector::new`. Reference `s = 0`, `c = 1`.
- **Set F** (all three): `RotatingOptions::new(2).coriolis(true)`, `RotatingSector::new` (the library's static centrifugal solve
  of the clamped sector; the probe). Reference `s = 1`, `c = 1` with the closed-form pre-stress.

`omega_1` the lowest reference frequency at rest over `n = 0 .. 3` (`n = 0`: the same forms with `Omega = 0`; arithmetic 1: it is
`n = 0`, `6842.79` rad/s); `Omega_j = (j / 6) omega_1`, `j = 1, 2, 3` (up to half the first frequency, a3).

### D5. The measures

At each kind, mesh, set, speed and `k`: `whirl_modes(k, Omega_j)` (dense solver, the default), its modes ascending; `lo`, `hi`
the first two, `omega_3` the third. Against the reference `omega_f < omega_b` (arithmetic 2: the forward root is the lower in
every case, `kappa_f > 0`):

- `e_lo = omega_lo / omega_f - 1`, `e_hi = omega_hi / omega_b - 1`;
- the split `d = (omega_hi - omega_lo) / (omega_b - omega_f) - 1`;
- the observed order `p = log2(|e_coarse| / |e_fine|)` of each frequency;
- the whirl numbers `kappa_lo / kappa_f - 1`, `kappa_hi / kappa_b - 1` (D3's signs);
- direction and wave sense: `lo` `Whirl::Forward` with `tau >= 0.99`, `hi` `Whirl::Backward` with `tau <= -0.99`, and `omega_3 >=
  1.5 omega_b`;
- transversality: for `lo` and `hi`, `sqrt(sum |u_perp|^2 / sum (u . n)^2)` over the translations of every node, both halves
  of the doubled pair, real and imaginary parts (`u_perp = u - (u . n) n`).

### D6. The discretisation error expected (the estimate behind WF)

Both kinds interpolate `w` and the rotations linearly and tie (Shell4) or stabilise (Shell3) the shear. As ADR 0569 D6, a field
of wave number `k` on elements of length `h` has `omega_h / omega = 1 + (k h)^2 / 8 + O(h^4)` with tied shear and consistent
mass: expected **order 2**. Here two directions: radially the first-radial (clamped-free) shape, an effective `k_r (b - a) =
1.875` (the cantilever's root, cited from memory, an estimate only), `k_r h_r = 1.875 / n_r`; circumferentially `k_theta h_theta
= (n / r)(r Delta) = n Delta` at every radius. The inscribed polygons of the bore and the rim shrink the annulus by the area
factor `1 - Delta^2 / 6`, a similarity scaling of `1 - Delta^2 / 12` in length, raising a plate frequency by `Delta^2 / 6`. The
estimate, all energy taken in each direction (an upper estimate):

`e_est(k, mesh) = [(1.875 / n_r)^2 + (k Delta)^2] / 8 + Delta^2 / 6`

(coarse `1.19e-2`, `1.83e-2`, `2.91e-2` for `k = 1, 2, 3`; fine `2.98e-3`, `4.58e-3`, `7.27e-3`). The pre-stress of set F is the
library's linear-membrane solve, its centroid resultant second-order accurate: another `O(h^2)` term of the same size. Shell3's
stabilisation `t^2 / (t^2 + alpha h^2)` multiplies the shear compliance by `1 + alpha h^2 / t^2`, which lowers the frequency by
about `alpha (k h)^2 / (12 (1 - nu) kappa_s) = 0.014 (k h)^2` (`h` the longest edge), exactly `O(h^2)` (the compliance is linear
in `h^2`); with the diagonal cut, a constant not derived here. Hence Shell3's doubled limit constant. The reference itself is
the model the shells discretise (Mindlin, `kappa_s = 5/6`, rotary inertia, plane stress, the same pre-stress terms), so there is
no model error beyond the geometry.

The split is `2 kappa Omega` at first order; its relative error is that of `kappa`, the ratio of the rotary-inertia form `int I R
T` to the mass, of the same interpolation order; registered at twice the frequency limit's constant (SP).

## Checks (limits fixed now)

| Id | Clause | Check | Limit | Estimate behind it |
|---|---|---|---|---|
| RF1 | a2 | the reference at `(p, n_e) = (8, 8)` against `(10, 12)`: every `omega_f`, `omega_b` of every case (two sets, three speeds, `k = 1, 2, 3`) and `omega_1` | `<= 1e-9` relative; the split `<= 1e-6` relative | arithmetic 1: `6e-11` at rest |
| RF2 | a2 | the Bessel functions of the RF3 closed form: the Wronskians `J_n Y_n' - J_n' Y_n - 2 / (pi x)` and `I_n K_n' - I_n' K_n + 1 / x`, `n = 0 .. 3`, `x = 0.5, 1.5, 3, 4` | `<= 1e-12` absolute (times `x`) | series to rounding (arithmetic 6: `<= 3e-14`) |
| RF3a | a2 | the reference's bending part (its polar kinematics) against the Kirchhoff closed form of the annulus clamped at both radii (Bessel functions; D7): `Omega = 0`, `(p, n_e) = (16, 8)`, the Richardson value `(4 omega(t/2) - omega(t)) / 3` at `t = 1e-3` over `omega_K`, `n = 0 .. 3` | `<= 2e-5` relative | the Mindlin correction is `O(t^2)` (`2.2e-3` at `t = 1e-3`), the extrapolation leaves `O(t^4)`, its square `5e-6`; arithmetic 6: `1.8e-6 .. 5.2e-6` |
| RF3b | a2 | as RF3a with the free outer edge of the item (clamped-free), `n = 0` | `<= 1e-6` relative | arithmetic 6: `1.3e-8` (for `n >= 1` a free edge has the Mindlin boundary layer of the twisting moment, a correction of order `t`, and the two-point extrapolation does not apply: arithmetic 6) |
| RF4 | a2 | the pre-stress closed form: `u(a)`, `sigma_r(b)` and the equilibrium residual by central differences (step `1e-5 (b - a)`) at 11 radii, over `rho b^2` | `<= 1e-6` | rounding over the step, `O(h^2)` of the difference `~ 1e-10` |
| WF4 | a1, a2, a4 | Shell4, both meshes, both sets, the three speeds, `k = 1, 2, 3`: `|e_lo|`, `|e_hi|` | `<= 2 e_est(k, mesh)` | D6 |
| WF3 | a1, a2, a4 | Shell3, as WF4 | `<= 4 e_est(k, mesh)` | D6, constant not derived |
| OD4, OD3 | a5 | the observed order `p` of each of the 36 frequencies (two sets, three speeds, three `k`, two members) per kind | `|p - 2| <= 0.5` | D6: order 2 |
| SP4, SP3 | a6 | the split of each pair, both meshes, both sets, the three speeds, `k = 1, 2, 3`: `|d|` | `<= 4 e_est(k, mesh)` | D6 |
| KA4, KA3 | a1 | the whirl numbers of the pair, the same cases: `|kappa / kappa_ref - 1|` with D3's signs | `<= 4 e_est(k, mesh)` | as SP: the split is `2 kappa Omega` at first order |
| DI4, DI3 | a1 | the same cases: `lo` forward with `tau >= 0.99`, `hi` backward with `tau <= -0.99`, `omega_3 >= 1.5 omega_b` | as stated | D3 and arithmetic 2: the forward root is the lower; the next transverse root of each `k` is `>= 4.8` times the first at rest (arithmetic 1); the in-plane modes are an estimate (`~ sqrt(E / rho) / (b - a)`, `8e4` rad/s) |
| TR4, TR3 | a1 | the same cases: the in-plane over the axial translation of `lo` and `hi` (D5) | `<= 1e-6` | D1: uncoupled up to the rounding of the facet axes, `~ 1e-15` |
| NC1 | a6 | set S **without** Coriolis (`coriolis(false)`), coarse, `Omega_3`, `k = 1, 2, 3`, each kind: `omega_hi / omega_lo - 1` | `<= 10 n_q eps omega_max / omega_lo` (`n_q` the reduced unknowns, `omega_max` the largest of `frequencies_all`, `eps = 2.2e-16`) | ADR 0569 amendment 1: 30 to 350 times `eps omega_max / omega_lo` at `n_q = 96 .. 144`, that is at most `3.7 n_q`; with `G` the smallest split is `>= 1e-3` of `omega` (arithmetic 2), far above |
| B1 | a7 | every existing rcfd-fem test passes unchanged (release) | pass | no library line changes (test-only item) |
| B2 | a7 | `cargo test -p rcfd-api --release --test baseline_hashes`; `cargo test -p rcfd-cli --release --test docs_site` | pass | as B1 |

### D7. The Kirchhoff closed form of RF3 (derived; the Bessel series cited from memory)

The thin plate `D nabla^4 w = rho t omega^2 w`, `w = W(r) cos n theta`, `k^4 = rho t omega^2 / D`: `W = c1 J_n(kr) + c2 Y_n(kr)
+ c3 I_n(kr) + c4 K_n(kr)`; clamped at `a` (`W = W' = 0`), free at `b`: `M_r = -D [W'' + nu (W'/r - n^2 W / r^2)] = 0` and the
Kirchhoff shear `V_r = Q_r + (1/r) dM_rt/dtheta = -D [W''' + W''/r - (1 + (2 - nu) n^2) W'/r^2 + (3 - nu) n^2 W / r^3] = 0`
(derived from `M_rt = -D (1 - nu) d/dr (w_theta / r)` and `Q_r = -D d(nabla^2 w)/dr`). The second and third derivatives follow
from Bessel's equation (`W'' = -W'/r + n^2 W / r^2 -+ k^2 W`, `-` for `J, Y`, `+` for `I, K`, and its derivative), the first from
`Z_n' = Z_{n-1} - (n/x) Z_n` (`J, Y, I`; `J_{-1} = -J_1`, `Y_{-1} = -Y_1`, `I_{-1} = I_1`) and `K_n' = -K_{n-1} - (n/x) K_n`
(`K_{-1} = K_1`). `J_n`, `I_n` by their power series, `Y_n` and `K_n` by the series with the logarithm and the digamma function
(Abramowitz and Stegun 9.1.11 and 9.6.11, cited from memory; RF2 checks them through their Wronskians, which are exact). The
frequency parameter `lambda = k b` is the root of the 4 x 4 determinant, bracketed on a grid of 500 steps over `[1, 6]` and
bisected 100 times; `omega_K = (lambda / b)^2 sqrt(D / (rho t))`. Clamped at both radii (RF3a) the rows at `b` are `W = W' = 0`. The reference of
RF3 is the D3 code with the outer node's three unknowns also fixed (RF3a) or free (RF3b), at `(p, n_e) = (16, 8)` (a thin plate
locks a low-degree Mindlin discretisation, arithmetic 6).

## Negative controls

NC1 (the split vanishes without the Coriolis matrix: the split of SP is the Coriolis term's). TR (no coupling of the transverse
and in-plane blocks). RF2 to RF4 are the controls of the reference code.

## Mutations (on a scratch copy, own `CARGO_TARGET_DIR`)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | the rotary block `W^T [a x] W` of `coriolis_factors` zero (`shell.rs`, used by both kinds) | SP (`d = -1`), KA, DI, both kinds |
| MU2 | the rotary block negated | DI (directions and wave senses flip), both kinds |
| MU3 | the sector whirl with `G = Omega C` in place of `2 Omega C` (`whirl_at`) | SP (`d = -0.5`), both kinds |
| MU4 | the Shell4 Coriolis `m2` halved (`model_coriolis`, homogeneous closure) | SP4, KA4 |
| MU5 | the Shell3 Coriolis rotary scalar `mr` halved (`shell3::local_coriolis`) | SP3, KA3 |
| MU6 | the Shell4 geometric stiffness without the local normal component (`c = 2` dropped in `local_geometric_stiffness`) | WF4 (set F) |

No mutation is declared equivalent. Not mutated, because no registered check resolves it: the rotary spin softening `I Omega^2
|d|^2` and the rotary inertia of the mass (their effect on the frequencies is `~ (t k)^2 / 12 x (Omega / omega)^2 <= 1e-3`, below
the WF limits; Limits).

## Decision rule

**done**: every check RF1 to B2 met, every mutation caught. **partial**: any registered check missed (recorded as a
`*_known_miss`), or any mutation uncaught, or a check met only after a post-run amendment. **todo**: WF and SP not run.

## Arithmetic (disclosed)

1. Reference at rest (`f657-scratch/reference.py`, the D3 code in Python, `(p, n_e) = (8, 8)` against `(10, 12)`): `n = 0 .. 4`
   first roots `6842.791415`, `6889.770357`, `7853.541182`, `11205.557565`, `17192.033217` rad/s (second radial roots `42892`,
   `44159`, `48036`, `54695`, `64263`), the two discretisations agreeing within `6e-11`. So `omega_1 = 6842.7914` rad/s (`n =
   0`) and the speeds `1140.465`, `2280.930`, `3421.396` rad/s. In the Kirchhoff parameter `lambda^2 = omega b^2 sqrt(rho t / D)`
   the `n = 0` root is `8.960`, against the thin plate's `9.0206` (D7): the Mindlin plate is `0.67 %` lower at `t / (b - a) =
   1/12`, as expected.
2. Reference at the speeds (`f657-scratch/ref_speeds.py`, the fixed point of D3): `(omega_f, omega_b)` rad/s, split over `omega_f`,
   `kappa_f`:

   | Set | `j` | `n = 1` | `n = 2` | `n = 3` |
   |---|---|---|---|---|
   | S | 1 | 6887.9012, 6891.2028; 4.8e-4; 1.4476e-3 | 7849.9239, 7856.6314; 8.5e-4; 2.9409e-3 | 11200.0752, 11210.4999; 9.3e-4; 4.5708e-3 |
   | S | 2 | 6885.5951, 6892.1985; 9.6e-4 | 7845.7794, 7859.1947; 1.7e-3 | 11194.0526, 11214.9023; 1.9e-3 |
   | S | 3 | 6882.8520, 6892.7574; 1.4e-3 | 7841.1074, 7861.2308; 2.6e-3 | 11187.4894, 11218.7647; 2.8e-3 |
   | F | 1 | 7059.7111, 7063.0099; 4.7e-4 | 8051.2851, 8057.9897; 8.3e-4 | 11400.9524, 11411.3768; 9.1e-4 |
   | F | 2 | 7548.8379, 7555.4192; 8.7e-4 | 8622.8615, 8636.2538; 1.6e-3 | 11977.3490, 11998.1962; 1.7e-3 |
   | F | 3 | 8299.0873, 8308.9203; 1.2e-3 | 9499.9468, 9519.9952; 2.1e-3 | 12883.0738, 12914.3411; 2.4e-3 |

   In every case the forward root is the lower, `kappa_f > 0` and `kappa_b < 0` (D3's signs: `kappa_b = x^T G x / x^T M x` with
   `x^T G x = 2 int I R T < 0`), `|kappa| = 1.44e-3, 2.94e-3, 4.57e-3` for `n = 1, 2, 3`, and `2 kappa Omega` equals the split to
   `3e-4`. The two discretisations agree within `3.3e-11` (frequencies) and `6.8e-8` (splits). The splits, `3.3` to `31.3` rad/s,
   are carried by the rotary block alone. The fixed point reached `1e-14` in 5 to 18 steps in some cases and stalled at rounding
   (below the RF1 agreement) in others; the Rust code stops at `1e-13` or 100 steps.
3. The whirl numbers' signs, checked on the ring of ADR 0569: its forward `kappa = -2 W V / (W^2 + V^2)` and amended backward `+2 W
   V / (W^2 + V^2)` are D3's formulas with `(W, V) -> (R, T)` and `m -> I`; `x^T G x = 2 int I R T` here plays `2 m W V` there.
4. `K_e + 2 Omega omega G` is positive definite along the iteration and at the fixed point: its lowest eigenvalue relative to `M`
   is `mu_1 = omega^2 > 0` at the root, and the rotary terms `2 Omega omega I` are `< 1e-7` of the shear stiffness `kappa_s G t`
   per unit `I` (`kappa_s G t / I = 3.9e12` s^-2 against `2 Omega omega <= 9e7`).
5. Rounding against the split: the shells' largest frequency is that of the rotations, `~ sqrt(kappa_s G t / I) = 2e6` rad/s
   (an estimate); with ADR 0569's observed `<= 350 eps omega_max`, the dense solver's error is `<= 2e-7` rad/s, `6e-8` of the
   smallest split. NC1's limit `10 n_q eps omega_max / omega_lo` is `~ 1e-10` at `n_q = 96`.
6. RF2 and RF3 (`f657-scratch/kirchhoff.py`, `rf3.py`, `rf3cc.py`): the Wronskians of the series (after correcting `J_0' = -J_1`,
   `Y_0' = -Y_1` in the script) are `<= 2.7e-14`. Kirchhoff `lambda^2`: clamped-free `9.020565`, `9.115493`, `10.464518`,
   `14.963070` (`n = 0 .. 3`); clamped-clamped `61.872213`, `62.995986`, `66.671687`, `73.630155`. The D3 code at `(16, 8)`,
   `t = 1e-3` and `5e-4` (at `(8, 8)` a thin plate locks: `n = 3` changes by `4.5e-6` between `(8, 8)` and `(10, 12)` at `t =
   1e-3`; at `(16, 8)` against `(20, 8)` by `< 1.5e-9`): clamped-clamped, the ratio to Kirchhoff `-2.15e-3` and `-5.40e-4` (a
   ratio of 3.99: `O(t^2)`), the Richardson value `1.8e-6, 2.3e-6, 3.5e-6, 5.2e-6` off; clamped-free `n = 0`: `-2.73e-4`, `-6.82e-5`,
   Richardson `1.3e-8`; clamped-free `n >= 1`: the ratios fall by `~ 3` per halving, not 4 (the Richardson value is `6.8e-5` to
   `4.2e-4` off): the free edge's Mindlin boundary layer, an `O(t)` term; hence RF3b only at `n = 0`.
7. D6: `(1.875 / n_r)^2` coarse `0.0549`, fine `0.0137`; `(k Delta)^2` coarse `0.0171`, `0.0685`, `0.154`, fine a quarter;
   `Delta^2 / 6` coarse `2.86e-3`, fine `7.1e-4`.

## Limits (what is not covered)

- The rotary spin softening and the rotary inertia of the mass are present in the reference and the shells but are not resolved
  by WF (their effect is below the discretisation error); only the Coriolis rotary block is tested to the SP limit.
- The opt-in `t^2/12` rotation terms of the shell geometric stiffness (ADR 0508) are not on (the default rotating path does not
  take them); the reference matches the default.
- The layup coupling `m1` (zero for a homogeneous shell), `nu`-dependent anticlastic effects of free bands, the stationary frame,
  the sparse whirl solvers, the split modes and whirl state with shells, the Shell4 ANDES membrane, warped facets.
- `n = 0` (not a pair) enters only through `omega_1`.

## Addendum 1: results (2026-11-15)

No library change: the item is test-only and no defect of the library was found. Test file
`crates/rcfd-fem/tests/sa_disc_whirl_shells.rs` (the D3 reference in Rust with RF1 to RF4, the two disc tests, NC1), 59.5 s in
release for its four tests. No existing test edited, no golden or hash table touched. Run 1 (tests at 8697741b, library unchanged
from 61033836, log `f657-scratch/run1.log`) is the only registered run; every check was met in it, so there is no amendment.

| Id | Limit | Measured | Met |
|---|---|---|---|
| RF1 reference `(8, 8)` against `(10, 12)` | `1e-9`; split `1e-6` | `4.9e-11`; split `4.3e-8` | yes |
| RF2 Wronskians | `1e-12` | `3.5e-14` | yes |
| RF3a clamped-clamped Kirchhoff limit, `n = 0 .. 3` | `2e-5` | `1.8e-6`, `2.3e-6`, `3.5e-6`, `5.2e-6` | yes |
| RF3b clamped-free Kirchhoff limit, `n = 0` | `1e-6` | `3.5e-8` | yes |
| RF4 pre-stress closed form | `1e-6` | `5.9e-11` | yes |
| WF4 Shell4, coarse | `2 e_est` = `2.4e-2` / `3.7e-2` / `5.8e-2` (`k = 1, 2, 3`) | `5.3e-3 .. 7.5e-3` / `6.5e-3 .. 9.4e-3` / `1.02e-2 .. 1.29e-2` | yes |
| WF4 Shell4, fine | `5.9e-3` / `9.1e-3` / `1.45e-2` | `1.4e-3 .. 2.0e-3` / `1.9e-3 .. 2.7e-3` / `3.0e-3 .. 3.8e-3` | yes |
| WF3 Shell3, coarse | `4 e_est` = `4.7e-2` / `7.3e-2` / `1.16e-1` | `4.5e-3 .. 6.5e-3` / `8.5e-3 .. 1.14e-2` / `1.37e-2 .. 1.62e-2` | yes |
| WF3 Shell3, fine | `1.19e-2` / `1.83e-2` / `2.9e-2` | `1.2e-3 .. 1.8e-3` / `2.6e-3 .. 3.5e-3` / `4.2e-3 .. 5.1e-3` | yes |
| OD4 / OD3 observed order (36 each) | `abs(p - 2) <= 0.5` | `p = 1.77 .. 1.93` / `1.66 .. 1.87` | yes |
| SP4 split, coarse / fine | `4 e_est` (coarse `4.7e-2 .. 1.16e-1`, fine `1.19e-2 .. 2.9e-2`) | coarse `1.5e-3` (`k = 1`), `6.2e-3` (`k = 2`), `1.53e-2` (`k = 3`); fine `3.5e-4`, `1.31e-3`, `3.2e-3` | yes |
| SP3 split, coarse / fine | as SP4 | coarse `4.9e-4`, `5.3e-3`, `1.34e-2`; fine `1.5e-5`, `7.7e-4`, `2.07e-3` | yes |
| KA4 / KA3 whirl numbers (D3's signs) | `4 e_est` | equal to SP to two digits (worst `1.53e-2` / `1.34e-2`, coarse `k = 3`) | yes |
| DI4 / DI3 | forward `tau >= 0.99`, backward `tau <= -0.99`, `omega_3 >= 1.5 omega_b` | every case `Forward` `tau = 1.000000`, `Backward` `tau = -1.000000`; `omega_3 / omega_b >= 4.39` | yes |
| TR4 / TR3 in-plane over axial | `1e-6` | `<= 1.27e-8` / `<= 1.25e-8` | yes |
| NC1 no split without `G` | `10 n_q eps omega_max / omega_lo` (`4.6e-11 .. 8.6e-11`; `omega_max = 2.45e6` rad/s for Shell4 at `k = 1`) | `5.6e-14 .. 1.2e-12` | yes |
| B1 every rcfd-fem test (release) | pass | 1153 passed, 0 failed, 309 ignored (including the four new tests) | yes |
| B2 `baseline_hashes`; `docs_site` | pass | 3 passed, 1 ignored; 2 passed | yes |

Every frequency error is positive (the facets are stiff; Shell3's stabilisation lowers its error below Shell4's, as D6 expected),
and `e / e_est` is `0.44 .. 0.63` for Shell4 on the coarse mesh: the upper estimate of D6 holds with a factor 2 to spare. The
observed orders are a little below 2 (`1.66` worst, Shell3): the coarse mesh is not yet fully asymptotic (`(k Delta)^2 = 0.15` at
`k = 3`). The rotary block of the shell Coriolis matrix reproduces the reference's split, `3.3` to `31.3` rad/s (`5e-4` to
`2.8e-3` of the frequency; in the thin limit it would vanish), within `3.2e-3` on the fine mesh for both kinds, and the split
error halves to a quarter on refinement at every `k`. TR at `1e-8` is above the `1e-15` the registration guessed for the facet
axes; it is the eigenvector accuracy of the dense solver on the in-plane part (an error of order `eps omega_max / gap`), still two
orders below the limit; noted, no consequence.

`cargo fmt -p rcfd-fem --check` clean; `cargo clippy -p rcfd-fem --release --all-targets -- -D warnings` clean.

Mutations on a `git archive` copy of 8697741b with its own target (`f657-scratch/mut`, `mutate.py`, `runmut.sh`, logs
`mut_MU*.log`), the two disc tests each:

| Id | Caught by |
|---|---|
| MU1 rotary block zero | SP (`d = -1`), KA, DI (`Neutral`), OD, both kinds |
| MU2 rotary block negated | DI (directions and wave senses flip), both kinds, every case |
| MU3 sector `G = Omega C` | SP (`d = -0.50`), both kinds |
| MU4 Shell4 Coriolis `m2` halved | SP4 (`-0.50`), KA4 |
| MU5 Shell3 Coriolis `mr` halved | SP3 (`-0.50`), KA3 |
| MU6 Shell4 geometric stiffness without the normal component | WF4 (set F, up to `0.17`), OD4 (set F) |

All six caught, each by the checks registered for it (MU1 also by OD). Disclosed: the first application of MU3 by the script
put the factor in `RotatingPart::whirl_at` (the whole-part path, the first `fn whirl_at` of `whirl.rs`), which the sector tests
do not use, and nothing moved (`mut_MU3_wrong_site.log`); the script was corrected to the registered site, the sector's
`whirl_at`, and that run is the one in the table. This was a defect of the mutation script, not a change of any check.

**Decision by the registered rule: done.** Every check RF1 to B2 was met in the one registered run, with no amendment, and every
mutation was caught. Every clause of the acceptance is met: the transverse forward and backward whirl of a flat annular plate
clamped at its bore and spinning about its normal, Shell4 and Shell3 on a 48-sector cyclic sector about a tilted axis, nodal
diameters 1, 2 and 3, three speeds up to half the first frequency, with Coriolis and spin softening and with the library's own
centrifugal pre-stress, matches the semi-analytical Mindlin reference (solved to `5e-11`) within the registered discretisation
error at two meshes, with observed order `1.66 .. 1.93` (expected 2), and the rotating-frame split, carried by the rotary block
alone, within the registered limit (`<= 3.2e-3` on the fine mesh).

### Remaining questions

1. Status. Options: (a) done, as the rule gives; (b) partial until a third mesh shows the order closer to 2. Recommendation: (a);
   the order is within the registered band at every frequency and the errors are well inside their limits.
2. The rotary spin softening `I Omega^2 |d|^2` and the opt-in `t^2/12` rotation terms of the shell geometric stiffness (ADR 0508)
   are below the resolution of these checks (Limits). Options: (a) leave them to the uniform-field checks of ADR 0473/0508; (b) a
   follow-up with a thick plate at high speed and a Richardson-extrapolated comparison that resolves `1e-4`. Recommendation: (a);
   their effect on a fan blade's frequencies is of order `(t k)^2 (Omega / omega)^2 / 12`, small next to the stress stiffening.

## For the lead: roadmap entries

F-657 proposed status: **done**.

> Structural solver, advanced tier. Follow-up of F-654 (ADR 0569, 2026-11-15). DONE. ADR 0573, 2026-11-15: the whirl of a spinning annular disc of shells, test-only (no library change), the first independent reference of the rotary block m2 W^T [a x] W of the shell Coriolis matrix (zero on the band of ADR 0569). A flat annular plate (a = 0.04 m, b = 0.1 m, t = 5 mm, steel, nu = 0.3) clamped at its bore, on a 48-sector cyclic sector about a tilted axis, its transverse forward and backward whirl from RotatingSector::whirl_modes with Shell4 and Shell3, against a semi-analytical reference derived in the ADR: the Mindlin plate with rotary inertia in the rotating frame (Coriolis and spin softening of the fibres, the closed-form plane-stress centrifugal membrane stress of the clamped-free disc), a p-version Galerkin solve in r per nodal diameter, converged to 5e-11 and checked against the Kirchhoff closed form (Bessel functions) in the thin limit within 5.2e-6. Nodal diameters 1, 2, 3, speeds 1/6, 1/3 and 1/2 of the first frequency, without and with the library's own centrifugal pre-stress: frequencies within the registered discretisation error at two meshes (worst 1.6e-2 coarse, 5.1e-3 fine, limits 2 and 4 times the estimate), observed order 1.66 to 1.93 (expected 2), the rotating-frame split (3.3 to 31.3 rad/s, carried by the rotary block alone) within 1.5e-2 coarse and 3.2e-3 fine (limit 4 times the estimate), whirl numbers likewise, directions and wave senses exact, transverse and in-plane blocks uncoupled to 1.3e-8, no split without the Coriolis matrix (1.2e-12, within the dense solver's rounding limit); models without shells keep their bits (no library line changed; every rcfd-fem test, baseline_hashes, docs_site). Six of six mutations caught. Every check met in the one registered run. Not resolved by these checks: the rotary spin softening and the opt-in t^2/12 rotation terms of the shell geometric stiffness (below the discretisation error).

No follow-up is proposed (remaining question 2 recommends leaving the rotary spin softening and the ADR 0508 rotation terms to their uniform-field checks).
