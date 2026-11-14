# ADR 0488: F-551 registration: radiation reactance and surface pressure of a baffled plane by analytic face integrals of the singular part 1/R of the Rayleigh kernel

Status: **REGISTERED 2026-11-14, written before any code of F-551 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-551 was written, compiled or run in the repository: the files listed under "What was
read" were read, and two numpy scripts were run in the scratch folder (disclosed below, their numbers quoted as estimates).
The bit record DB1 is a test file written and run on the base after this commit and before any library line. Results go in
the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-551 (deps F-418), the follow-up proposed by [ADR 0468](0468-f418-registration-radiated-power-rayleigh-integral.md)
(F-418: `rcfd_post::solid_acoustic`, its remaining question 2). Structural solver, advanced tier
([ADR 0310](0310-structural-solver-advanced-tier-plan.md)). Number 0488 given by the lead. Branch `f551-work` from fb7c5d94,
worktree `w49i-work/f551`, scratch `w49i-work/f551-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a missed limit becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the hash tables or of any golden file; existing tests not edited; no new
dependency, no unsafe code, the crate graph unchanged; clean-room. Every choice below that the maintainer would normally be
asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).

## What F-551 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. The surface pressure of a baffled plane by the Rayleigh integral with the singular part 1/R integrated analytically over each face; the radiation impedance of a baffled circular piston equal to rho c S (1 - J1(2 k a)/(k a) + i H1(2 k a)/(k a)) within 1 percent in resistance and reactance at k a = 0.1, 0.5, 2 and 10 with faces of a tenth of a wavelength; the resistance equal to the power of F-418 within 1e-6; the low-frequency reactance giving the added mass 8 rho a^3/3 within 2 percent at k a = 0.1.

Clauses: **a1** the surface pressure of a baffled plane by the Rayleigh integral, the singular part `1/R` integrated
analytically over each face; **a2** the radiation impedance of the baffled circular piston within 1 percent in resistance
**and** in reactance (each separately) at `k a = 0.1, 0.5, 2, 10`, faces of a tenth of a wavelength; **a3** the resistance
equal to the power of F-418 within 1e-6; **a4** at `k a = 0.1` the reactance gives the added mass `8 rho a^3/3` within 2
percent. The brief of the lead adds: register the mesh rule (faces per radius) and probe the geometric discretisation error
before registering the 1 and 2 percent limits; every existing result of `solid_acoustic` keeps its bits (a default-bits test at
the base before any library change); `baseline_hashes` unchanged; no one-ulp probe ratio on long sums; 1e-11 for sums of about
1e3 terms.

## What was read (fb7c5d94, read only)

ADR 0468 whole (closed forms 1 to 6, the design, the registered meshes, amendment 1 and addendum 1 with the E1 probe miss);
`crates/rcfd-post/src/solid_acoustic.rs` whole (`VibratingSurface`, `face_points` on the bilinear map with a triangle as a
collapsed quadrilateral, `BaffledPlane::new`, `pressure` with the near-field splitting, `far_field`, `power` by the
`sin(kR)/R` double sum over the quadrature points with `s(0) = k`); `crates/rcfd-post/tests/sa_solid_acoustic.rs` (the disk
mesh `disk`, `mesh_of`, `piston`, `j1` by the 400-point midpoint rule, the `Checks` helper) and
`sa_solid_acoustic_default_bits.rs` (the digest pattern); the F-418 section of `docs/reference-data.md`;
`w49i-work/f418-scratch/closed_forms.py` and its log (the impedance with J1 and the Struve H1 verified by the chord-length
density of the disk to 3e-15 .. 3.4e-13; `H1(x) = (2x/pi) int_0^1 sqrt(1 - t^2) sin(x t) dt` checked against its series to
6e-15). The reference functions of F-418 are reused (J1 by its integral representation; H1 by the integral above, now in the
test by 200-point Gauss-Legendre as in that script).

Published sources, cited from memory, **not read** for this work (every formula used is derived below from first principles):
D. R. Wilton, S. M. Rao, A. W. Glisson, D. H. Schaubert, O. M. Al-Bundak and C. M. Butler, "Potential integrals for uniform and
linear source distributions on polygonal and polyhedral domains", IEEE Trans. Antennas Propag. AP-32 (1984) 276-281 (the
edge-sum form of the integral of `1/R` over a polygon); Kinsler, Frey, Coppens and Sanders, *Fundamentals of Acoustics*, and
Morse and Ingard, *Theoretical Acoustics*, chapter 7 (the piston impedance with `J1` and `H1`, the added mass `8 rho a^3/3`);
Pierce, *Acoustics*, chapter 5 (the Rayleigh integral; the mass loading of a piston). The potential of a uniform disk at an
in-plane point, `int_disk dS/R = 4 a E(r/a)` (`E` the complete elliptic integral of the second kind, modulus `r/a`), is from
memory and checked below at its two ends.

### Arithmetic done before this registration (disclosed)

Two numpy scripts, no rcfd code: `w49i-work/f551-scratch/estimate.py` (log `estimate.log`) implements the planned scheme below
on the planned meshes (the same Gauss points as F-418, the analytic face integrals, the subtracted remainder) and the analytic
checks; `estimate2.py` (log `estimate2.log`) implements the F-418 near-field pressure (order 3, `near` 4, `max_depth` 8) for
check S2. The first launch of `estimate.py` ran Python with `-I`, which hid numpy: it stopped at the import before computing
anything; it was then run once. `estimate2.py` was run once. Their numbers are quoted as estimates in the table of checks.

## Derivation

Convention of ADR 0468: complex peak amplitudes, `x(t) = Re(X e^{i omega t})`, `k = omega/c`. For a point `x` on the plane the
Rayleigh integral is

    P(x) = (i omega rho/(2 pi)) int_S V_n(y) e^{-i k R}/R dS(y),   R = |x - y|,

with an integrable singularity at `y = x` (the single-layer potential is continuous onto the plane).

1. **Split of the kernel.** `e^{-i k R}/R = cos(k R)/R - i sin(k R)/R`. The part `sin(k R)/R = k sinc(k R)` is smooth (F-418);
   `cos(k R)/R = 1/R + (cos(k R) - 1)/R`, the second term `O(k^2 R)` and bounded. With the subtraction of the velocity at `x`:

       int V(y) cos(kR)/R dS = V(x) int_S dS/R + int_S (V(y) cos(kR) - V(x))/R dS.

   The second integrand is bounded (for a bilinear `V` it is `grad V . (y - x)/R + O(R)`, direction-dependent at `y = x`, zero
   for a uniform `V`). So

       P(x) = (i omega rho/(2 pi)) [ V(x) sum_f I_f(x) + sum_j w_j (V_j cos(k R_j) - V(x))/R_j - i sum_j w_j V_j s(R_j) ],

   `I_f(x) = int_face f dS/R` analytic, the sums over the quadrature points `(y_j, w_j, V_j)` of `BaffledPlane` (F-418: Gauss
   order `n` per direction on each face), the bounded term set to 0 and `s(0) = k` at a coincident point (`R_j = 0`).
2. **Analytic `int_polygon dS/R` for `x` in the plane of the polygon (derived).** In polar coordinates about `x` the integral
   is `int rho(theta) d theta`, `rho` the distance to the boundary. One edge on a line at signed distance `d` from `x` (positive
   when `x` is on the inner side), tangent coordinate `s` from `s-` to `s+` along it: with `s = d tan phi`, `rho = d/cos phi`,
   `int d/cos phi d phi = d ln(sec phi + tan phi)`, so the edge gives `d ln((R+ + s+)/(R- + s-))`, `R± = sqrt(d^2 + s±^2)`; a
   polygon is the sum over its edges (the signed `d` makes the angular pieces of an outside point cancel). Cancellation in
   `R + s` for `s < 0` is avoided by `R + s = d^2/(R - s)`. An edge whose line passes through `x` (`d = 0`) contributes 0 (the
   limit `d ln(1/d) -> 0`); an edge of zero length (the repeated node of a triangle in F-418's collapsed form) is skipped. The
   polygon normal `n_f` is Newell's sum `sum p_i x p_{i+1}`, `t` the unit edge tangent, the outward in-plane normal `t x n_f`, so
   `d = (p_a - x) . (t x n_f)` and the result does not depend on the orientation of the face. `x` off the plane is taken as its
   projection (`d` and `s` are in-plane). This is the in-plane case of the edge-sum form of Wilton et al. 1984 (cited from
   memory). Checks of the derivation (numpy): square of side `L` at its centre `4 L ln(1 + sqrt 2)`, at a corner `2 L ln(1 +
   sqrt 2)`; rectangle `a x b` at a corner `a ln((b + sqrt(a^2+b^2))/a) + b ln((a + sqrt(a^2+b^2))/b)` (so at the mid-edge of the
   square twice that with `a = L/2`, `b = L`); right triangle of legs `L` at the right angle `sqrt 2 L ln(1 + sqrt 2)` (only the
   hypotenuse contributes, `d = L/sqrt 2`, `s = ±L/sqrt 2`). Each by hand from the edge sum; numpy equal to 4.4e-16 or better.
3. **Impedance.** The complex power `Pi = (1/2) int_S P conj(V_n) dS`, its real part the radiated power `W` of F-418 (ADR 0468
   closed form 5). The radiation impedance referred to the mean-square normal velocity, `Z = 2 Pi/<|V_n|^2>`, `<|V_n|^2> = (1/S)
   int |V_n|^2 dS`; for a rigid piston (`V_n = V` uniform) `Z = F/V`, `F = int P dS` the force on it. With 1, `Z/(rho c S) = (k/(2
   pi S)) (int int sin(kR)/R + i int int cos(kR)/R)` for the piston, the closed form of ADR 0468 closed form 6 (verified there by
   independent quadrature). In the discrete form the resistance part is `(rho c k/(4 pi)) sum_i sum_j w_i w_j Re(V_i conj V_j)
   s(R_ij)`: the double sum of F-418's `power` over the same points, so clause a3 holds to rounding (the imaginary part of
   `V_j conj V_i` against the symmetric `cos` kernel cancels in the sum; the subtracted and analytic terms are real multiples of
   `|V_i|^2`).
4. **Added mass.** For small `x`, `H1(x) = 2 x^2/(3 pi) + O(x^4)`, so `X1 = H1(2 k a)/(k a) -> 8 k a/(3 pi)` and `Im Z = rho c pi
   a^2 8 k a/(3 pi) = omega 8 rho a^3/3`: the added mass `M = Im Z/omega -> 8 rho a^3/3`. At `k a = 0.1` the exact `Im Z/omega`
   is 2.66e-3 below `8 rho a^3/3` (numpy: `H1(0.2)/0.1 = 0.0846565` against `0.8/(3 pi) = 0.0848826`); that physics share is
   inside the 2 percent and not separated. Static check: `int int_disk dS dS/R = 16 a^3/3` follows (`(k/(2 pi S)) 16 a^3/3 = 8 k
   a/(3 pi)`).
5. **Centre pressure (for check S1).** The on-axis pressure of ADR 0468 closed form 3 at `z = 0`: `P(0) = rho c V (1 - e^{-i k
   a})`. **Continuity (for S2).** By the Euler equation `dP/dz = -i omega rho V_n` on the plane, so `P(x + h n) = P(x) - i omega
   rho V_n(x) h + O(h^2)`.
6. **Disk potential (for G1).** `int_disk dS/R = 4 a E(r/a)` at an in-plane point at radius `r <= a`: at `r = 0` it is `2 pi a
   = 4 a E(0)`; at `r = a`, polar about the rim point, `rho = 2 a cos theta`, `int 2 a cos theta = 4 a = 4 a E(1)`.

## Design

New code only in `crates/rcfd-post/src/solid_acoustic.rs`, additive: no existing function, struct field or default changes, so
every existing result keeps its bits (DB1). No new dependency, no unsafe code.

- `pub fn polygon_inverse_distance(polygon: &[Vec3], x: Vec3) -> Result<f64, String>`: derivation 2. Refuses fewer than 3
  vertices, values not finite, a polygon of zero area (Newell normal zero).
- `BaffledPlane::quadrature_points(&self) -> &[(Vec3, f64, Phasor)]`: the points `(y_j, w_j, V_n,j)` already held.
- `BaffledPlane::inverse_distance_potential(&self, face, u, v) -> Result<f64>`: `sum_f I_f(x)` at the point `x` of face
  `face` at parameters `(u, v)` in `[-1, 1]^2` (the bilinear map of F-418), each face by derivation 2 over its own nodes.
- `BaffledPlane::surface_pressure(&self, face, u, v, rho, c, omega) -> Result<Phasor>`: formula of derivation 1 at that point,
  `V(x)` the bilinear interpolant of the nodal velocity there, normal component on the plane normal.
- `BaffledPlane::surface_pressures(&self, rho, c, omega) -> Result<Vec<Phasor>>`: the same at every quadrature point (`V(x) =
  V_n,i`; the point itself is the coincident `j = i`).
- `BaffledPlane::radiation_impedance(&self, rho, c, omega) -> Result<RadiationImpedance>` with `RadiationImpedance {
  complex_power: Phasor (W, real part the radiated power), area, mean_square_velocity, impedance: Phasor (Z = 2 Pi/<|V_n|^2>,
  N s/m), normalised: Phasor (Z/(rho c area)), added_mass (Im Z/omega, kg) }`; refused when the mean-square velocity is zero.
  Sums in a fixed order, single-threaded (deterministic).

## Registered set-ups and the mesh rule

Air `rho = 1.2`, `c = 340`; piston `a = 0.1 m`, `V = 1e-3 m/s` real along `z`; the polar disk mesh of ADR 0468 (`disk(a, n_r,
n_t)`, plane `z = 0`), Gauss order 3 (the default `RayleighOptions`). **Mesh rule (registered):** faces of at most a tenth of a
wavelength radially and along the rim, **and** at least `n_r = 4` rings (faces per radius) and at least `n_t = 64` rim
segments, the last two for the geometry (the inscribed polygon's area deficit `(2 pi/n_t)^2/6 = 1.6e-3`, the reactance scaling
as the area to the power 3/2 at low `k a`). That is ADR 0468's table, reused unchanged: `n_r x n_t = 4 x 64` (256 faces, 2304
points) at `k a = 0.1, 0.5, 2`, and `16 x 100` (1600 faces, 14400 points, faces 0.995 and 1.000 of `lambda/10`) at `k a = 10`.
The probe of the geometric discretisation error (`estimate.py`) is the column "Estimate" of Z1, Z2, M1 below: at `k a <= 2`
it is the polygon (resistance about twice the area deficit at low `k a`, the force scaling with the area squared there); a
refinement at `k a = 10` to `20 x 128` halves the reactance error (2.7e-3 to 1.4e-3), so the registered mesh is not at a
resolution floor. The reference is the true disk (`S = pi a^2` in `rho c S (...)`), so the polygon error is counted, not
divided out.

Before the library code, `crates/rcfd-post/tests/sa_radiation_impedance_default_bits.rs` (API of fb7c5d94 only) prints
digests of the existing `solid_acoustic` results on the `4 x 64` piston with a complex velocity `V (1 + x/a) + 0.3 i V` (ERP,
area, level; deviation, diameter, normal, origin and five Rayleigh pressures, near and far; five far fields and directivities;
the power at `k a = 0.1, 0.5, 2` with its ERP, efficiency and level, and `power_from_far_field(16, 32)`) and is committed with
them as constants.

## Registered checks (limits fixed now)

Tests in `crates/rcfd-post/tests/sa_radiation_impedance.rs` and `sa_radiation_impedance_default_bits.rs`, release profile.
"rel" is `|measured/reference - 1|`. J1 by the 400-point midpoint rule of the integral representation (as F-418), H1 by the
200-point Gauss-Legendre rule of `(2x/pi) int_0^{pi/2} cos^2 p sin(x sin p) dp` (as `closed_forms.py`), `E(m)` by the
400-point Gauss-Legendre rule of `int_0^{pi/2} sqrt(1 - m^2 sin^2 p) dp`; the tests' Gauss-Legendre nodes by Newton on the
Legendre recurrence. No one-ulp probe is registered (lesson of ADR 0468): the limits of the analytic checks were probed in
numpy (estimates) and sit at least 200 times above them.

| Id | Clause | Check | Limit | Estimate |
|---|---|---|---|---|
| A1 | a1 (analytic face integral) | `polygon_inverse_distance` of the square `L = 0.37` at its centre vs `4 L ln(1 + sqrt 2)`, at a corner vs `2 L ln(1 + sqrt 2)`, at the mid-edge vs `2 (a ln((b + sqrt(a^2+b^2))/a) + b ln((a + sqrt(a^2+b^2))/b))`, `a = L/2`, `b = L`; right triangle of legs `L` at the right angle vs `sqrt 2 L ln(1 + sqrt 2)` (largest rel) | `<= 1e-13` | 4.4e-16 |
| A2 | a1 | square, point outside at `(2 L, L/2)`, vs a 24 x 24 Gauss-Legendre tensor rule (smooth integrand); a general quadrilateral `(0,0), (0.3,-0.05), (0.35,0.25), (-0.02,0.2)` at the inner point `(0.1, 0.07)` vs Duffy coordinates on the four triangles from the point, 40 x 40 Gauss each (largest rel) | `<= 1e-11` (sums of 576 and 6400 terms) | 1.1e-15, 1.6e-15 |
| A3 | a1 | the square in reversed vertex order at its centre: rel vs forward; the right triangle as a quadrilateral with its last node repeated: bits equal to the triangle | `<= 1e-13`; bits | 2.2e-16; 0 |
| G1 | a1 (geometry) | `inverse_distance_potential` of the `4 x 64` and `16 x 100` disks at the ring nodes on `phi = 0` (`r = j a/4`, `j = 0..4`; `r = j a/16`, `j = 0, 4, 8, 12, 16`) vs `4 a E(r/a)` (largest rel) | `<= 5e-3` | 2.7e-3 (rim of `4 x 64`), 1.2e-3 |
| S1 | a1 | `surface_pressure` at the centre (face 0, corner `(-1, -1)`), uniform piston, the four `k a` of Z on their meshes: `|P - rho c V (1 - e^{-i k a})|/|rho c V (1 - e^{-i k a})|` (largest) | `<= 5e-3` | 8.0e-4, 8.1e-4, 9.5e-4, 1.7e-3 |
| S2 | a1 (non-uniform velocity) | `4 x 64`, `k a = 2`, velocity `V (1 + x/a)` along z: at node 0 (face 0, `(-1, -1)`) and node 65 (`r = a/2`, `phi = 0`; face 64 at `(1, -1)`, where it is not the first corner): `|P_F418(x + h n) - (P_s(x) - i omega rho V_n(x) h)|/|P_s(x)|`, `h = a/100`, `P_F418` the existing `pressure` with default options (largest) | `<= 1e-3` | 6.9e-5, 5.9e-5 |
| Z1 | a2 | resistance: rel of `Re Z/(rho c pi a^2)` vs `1 - J1(2 k a)/(k a)`, `k a = 0.1, 0.5, 2, 10` (each) | `<= 1e-2` | 3.2e-3, 3.1e-3, 2.2e-3, 5.5e-4 |
| Z2 | a2 | reactance: rel of `Im Z/(rho c pi a^2)` vs `H1(2 k a)/(k a)`, the same four (each) | `<= 1e-2` | 2.3e-3, 2.2e-3, 5.6e-5, 2.7e-3 |
| Z3 | a3 | rel of `Re Z` vs `2 W/V^2`, `W` = `BaffledPlane::power` (F-418) of the same plane, the four `k a` | `<= 1e-6` | 1.1e-14 |
| M1 | a4 | `k a = 0.1`: rel of `added_mass` vs `8 rho a^3/3` | `<= 2e-2` | 5.0e-3 |
| H1 | (order) | `k a = 2`: the faces in reverse order: `|Z_rev - Z|/|Z|` | `<= 1e-10` | rounding |
| NC1 | (negative control: mesh rule) | `disk(a, 2, 16)` at `k a = 0.1` (fewer rings and rim segments than the rule): rel of the reactance as Z2 | must be `> 1e-2` | 3.8e-2 |
| NC2 | (negative control: analytic part) | naive collocation on the registered `k a = 0.1` and `2` meshes, computed in the test from `quadrature_points`: `(k/(2 pi pi a^2)) sum_i sum_{j != i} w_i w_j cos(k R_ij)/R_ij` (the coincident point dropped, no analytic integral) vs `H1(2 k a)/(k a)` | must be `> 1e-2` (each) | 1.8e-2, 5.0e-2 |
| NC3 | (negative control: S2 sensitivity) | S2 without the Taylor step `- i omega rho V_n h` | must be `> 5e-3` (each) | 1.2e-2, 1.5e-2 |
| N1 | (refusals) | never a panic, each an error: `polygon_inverse_distance` with 2 vertices, a NaN vertex, a NaN point, three collinear vertices; `surface_pressure` and `inverse_distance_potential` with a face index out of range, `u` or `v` outside `[-1, 1]` or NaN; `surface_pressure`, `surface_pressures`, `radiation_impedance` with `rho`, `c` or `omega` not positive; `radiation_impedance` of a surface at rest | holds | holds |
| T1 | (cost) | the `k a = 10` impedance (14400 points) in release: wall time printed; above 120 s the check becomes an ignored heavy record with a fast test of the same path | printed | about 5 to 10 s |
| DB1 | (defaults) | the digests of `sa_radiation_impedance_default_bits.rs` equal its constants printed at fb7c5d94 | `==` | holds |
| DB2 | (defaults) | the existing `sa_solid_acoustic` (one ignored known miss) and `sa_solid_acoustic_default_bits` tests pass unedited | pass | holds |

Mutations (on a scratch copy of the library, one at a time, own `CARGO_TARGET_DIR`; each must be caught by at least one
registered check):

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | the analytic term `V(x) sum_f I_f(x)` dropped from the surface pressure | S1, S2, Z2, M1 |
| MU2 | in-plane normal `n_f x t` instead of `t x n_f` (sign of `d`) | A1, G1, Z2 |
| MU3 | `ln(A-/A+)` instead of `ln(A+/A-)` | A1, G1 |
| MU4 | the subtraction `- V(x)/R_j` left out of the bounded remainder | S1, Z2, M1 |
| MU5 | `s(0) = 0` instead of `k` in the surface pressure | Z3 |
| MU6 | the factor 1/2 of the complex power dropped | Z1, Z2, Z3, M1 |
| MU7 | the `d = 0` guard removed (NaN at a node on an edge line) | G1, S1 |
| MU8 | `V(x)` in `surface_pressure` taken as the velocity of the face's first node | S2 |

## Decision rule

**done**: every check met at its registered run, every negative control as registered, every mutation caught, no amendment
after a run. **partial**: a check missed (recorded as `*_known_miss` with its limit), a negative control not as registered, a
mutation not caught, or a check met only after an amendment made after its run. **todo**: the module does not build or the
piston runs cannot be made.

## Limits (what is not covered)

- The analytic integral is the in-plane case (the surface pressure on a plane radiator); field points off the plane keep
  F-418's near-field splitting. No analytic integral of the linear part of the velocity (it is subtracted and integrated as a
  bounded remainder), no curved or unbaffled radiator, no boundary-element solve.
- The impedance is referred to the mean-square normal velocity of the surface (for a rigid piston the force over the velocity);
  no modal impedance matrix between several velocity fields.
- Cost `O(P^2 + P F)` (points `P`, faces `F`): 14400 points take seconds; large surfaces need a fast method.
- The light-fluid limit only: the added mass is reported, not fed back into a structural solve.
- References are closed forms (derived here or in ADR 0468); no measured data.

## Amendment 1 (2026-11-14, written after the registered run of commit 0546e941 and the mutation runs, before any corrected run)

The registered run (`cargo test --release -p rcfd-post --test sa_radiation_impedance -- --test-threads=1 --nocapture`, log
`f551-scratch/run_checks.log`) met every limit and every negative control as registered. The mutation runs (`mutate.py`, logs
`MU*.log`) caught MU1 to MU6 and MU8, but **MU7 (the `d = 0` guard removed) was not caught by a registered check**, although
it turns the corner, mid-edge and right-angle values of A1, the triangle of A3, the node potentials of G1 and the centre
pressures of S1 into NaN (printed in `MU7.log`). Diagnosis: a defect of the test set-up, not of the library. The tests take the
largest error with `f64::max`, which drops a NaN operand, so `worst` stayed at the finite cases (A1 reported 2.2e-16, S1 0);
A3 compared the bits of two NaNs as equal. Checks compared one by one (`value <= limit`) treat NaN as a miss and are not
affected (Z1 to Z3, M1, H1, NC1, NC2, N1).

Correction, registered now with the limits unchanged: a copy `crates/rcfd-post/tests/sa_radiation_impedance_amend1.rs` of the
tests `a1_a3_analytic_polygon_integrals`, `g1_disk_potential` and `s1_s2_surface_pressure` in which the aggregation propagates
NaN (a NaN is the largest error, so a miss) and every value must also be finite (A3's bit comparison requires a finite value);
the same set-ups, limits and the bounds of NC3. The three original tests stay as ignored records (the attribute
`#[ignore]` with the reason, nothing else edited). The corrected run is made once on the library of 0546e941 (unchanged), then
MU7 is run once against the corrected file on the scratch copy. Per the decision rule F-551 cannot be **done** (a mutation not
caught by a registered check; checks met after an amendment made after their run).

## Addendum 1: results (2026-11-14)

Commits: fd35fc68 (registration, alone), b3950d33 (the DB1 bit record, constants printed at fb7c5d94 before any library line),
0546e941 (library and checks), 0f790e6a (amendment 1), ad2e18c7 (the three NaN-blind tests kept as ignored records, the
NaN-propagating copy), 2d649cc0 (`cargo fmt`, layout only; module docs; `docs/acoustics.md` and `docs/reference-data.md`).
The registered run (`cargo test --release -p rcfd-post --test sa_radiation_impedance -- --test-threads=1 --nocapture`, commit
0546e941, log `f551-scratch/run_checks.log`) was made once; the corrected run of amendment 1 (`--test
sa_radiation_impedance_amend1`, commit ad2e18c7, the library unchanged, log `run_amend1.log`) once. After the layout-only
`cargo fmt` the same tests printed every number again unchanged (`run_after_fmt.log`). Release profile throughout. Every
measured number equals its estimate of `estimate.py` or `estimate2.py` to the digits printed there.

| Id | Limit | Measured | Met |
|---|---|---|---|
| A1 square centre, corner, mid-edge; right triangle at the right angle (largest rel) | `<= 1e-13` | 4.4e-16 | yes |
| A2 outside point vs Gauss 24 x 24; general quadrilateral vs Duffy 40 x 40 | `<= 1e-11` | 4.2e-15 | yes |
| A3 reversed orientation; collapsed triangle (bits) | `<= 1e-13`; bits | 0; equal | yes |
| G1 disk potential vs `4 a E(r/a)` at the ring nodes (`4 x 64`; `16 x 100`) | `<= 5e-3` | 8.0e-4 .. 2.69e-3 (rim); 3.3e-4 .. 1.17e-3 | yes |
| S1 centre pressure vs `rho c V (1 - e^{-i k a})`, `k a = 0.1, 0.5, 2, 10` | `<= 5e-3` | 8.0e-4, 8.1e-4, 9.5e-4, 1.72e-3 | yes |
| S2 non-uniform velocity: surface pressure with the Taylor step vs the F-418 pressure at `h = a/100` | `<= 1e-3` | 6.9e-5, 5.9e-5 | yes |
| Z1 resistance vs `1 - J1(2ka)/(ka)` (`rho c pi a^2`), `k a = 0.1, 0.5, 2, 10` | `<= 1e-2` each | 3.21e-3, 3.14e-3, 2.17e-3, 5.5e-4 | yes |
| Z2 reactance vs `H1(2ka)/(ka)`, the same | `<= 1e-2` each | 2.31e-3, 2.20e-3, 5.6e-5, 2.73e-3 | yes |
| Z3 `Re Z` vs `2 W/V^2` of F-418's `power` | `<= 1e-6` | 1.4e-14, 3.6e-15, 3.1e-15, 8.0e-14 | yes |
| M1 `k a = 0.1` added mass vs `8 rho a^3/3` | `<= 2e-2` | 4.97e-3 (of which 2.66e-3 the physics at `k a = 0.1`) | yes |
| H1 `k a = 2` faces reversed | `<= 1e-10` | 9.5e-15 | yes |
| NC1 `disk(a, 2, 16)` reactance at `k a = 0.1` | must be `> 1e-2` | 3.77e-2 | as registered |
| NC2 naive collocation reactance, `k a = 0.1`, 2 | must be `> 1e-2` | 1.80e-2, 4.98e-2 | as registered |
| NC3 S2 without the Taylor step (smallest) | must be `> 5e-3` | 1.19e-2 | as registered |
| N1 refusals | holds | 18 of 18, no panic | yes |
| T1 `k a = 10` impedance wall time | printed | 6.6 s | (record) |
| DB1 four digests of the existing results | `==` | 4 of 4 | yes |
| DB2 `sa_solid_acoustic` (11, 1 ignored known miss), `sa_solid_acoustic_default_bits` (1), unedited | pass | pass | yes |

Mutations (scratch copy `f551-scratch/copy` of the tree of 0546e941, own target `target-copy`, `mutate.py`, logs `MU*.log`):

| Id | Caught by |
|---|---|
| MU1 analytic term dropped | S1, S2, Z2, M1 |
| MU2 sign of `d` | A1, A2, G1, S1, S2, Z2, M1 |
| MU3 `ln(A-/A+)` | A1, A2, G1, S1, S2, Z2, M1 |
| MU4 subtraction `- V(x)/R` left out | S1, S2, Z2, M1 |
| MU5 `s(0) = 0` | Z3 (alone, as registered) |
| MU6 factor 1/2 dropped | Z1, Z2, Z3, M1 |
| MU7 `d = 0` guard removed | **not caught** by the registered tests (NaN dropped by `f64::max`; amendment 1); caught by the corrected copy (`MU7_amend1.log`): A1, A3, G1, S1, S2, NC3 |
| MU8 velocity of the first node | S2 (alone, as registered) |

Run besides: `cargo test -p rcfd-post --release` (59 test binaries, all ok; at 0546e941, the library code unchanged since,
log `run_post_all.log`); `cargo test -p rcfd-api --release --test baseline_hashes` and `cargo test -p rcfd-cli --release
--test docs_site` (log `run_gates.log`: 3 passed, 1 ignored as before; 2 passed); `cargo fmt -p rcfd-post --check` clean;
`cargo clippy -p rcfd-post --all-targets --release -- -D warnings` clean. Not run: the whole workspace (the lead runs the
gate). In the library only `crates/rcfd-post/src/solid_acoustic.rs` changed (module docs, lines 12 to 16; new code from line
798 on); no shared solver file.

Deviations: (1) MU7 not caught by a registered check (amendment 1; a test defect, the library unchanged). (2) The first launch
of `estimate.py` with `python -I` stopped at the numpy import (disclosed in the registration).

**Decision by the registered rule: partial.** Every check and every negative control met at the registered run, and every
clause of the acceptance met (a1 by S1, S2, G1 and A1 to A3; a2 by Z1 and Z2, largest 3.2e-3 against 1e-2; a3 by Z3,
8.0e-14 against 1e-6; a4 by M1, 5.0e-3 against 2e-2), but one registered mutation (MU7) was not caught by the registered
tests, and the corrected checks were met only after an amendment made after their run.

Remaining questions:
1. NaN in the aggregation of test errors. Options: (a) leave it to each item; (b) a shared NaN-propagating helper in the test
   support of each crate; (c) a registration rule that every aggregated check also requires finite values. Recommendation:
   (c) for later registrations (no shared code needed); the corrected copy here does both.
2. Several velocity fields (a mutual radiation impedance matrix; the added mass and radiation damping of structural modes).
   Options: a follow-up item (below); or leave out. Recommendation: the optional follow-up, only if a light-fluid modal
   correction is wanted for the fan.

### For the lead: roadmap entries

F-551, status proposed **partial**, new note:

> ADR 0488, 2026-11-14: rcfd-post solid_acoustic: BaffledPlane::surface_pressure and surface_pressures (the Rayleigh integral on the plane, the singular part 1/R integrated analytically over each face by the in-plane edge sum polygon_inverse_distance, the velocity at the point subtracted and the bounded rest by the quadrature points of F-418), radiation_impedance (complex power, Z = 2 Pi/<|V_n|^2>, Z/(rho c S), added mass Im Z/omega), quadrature_points; all additive, the existing results bit-identical (four digests). Baffled piston on the meshes of ADR 0468 (4 rings x 64 rim segments, 16 x 100 at k a = 10; faces of a tenth of a wavelength or finer): resistance within 3.2e-3, 3.1e-3, 2.2e-3, 5.5e-4 and reactance within 2.3e-3, 2.2e-3, 5.6e-5, 2.7e-3 of rho c pi a^2 (1 - J1(2ka)/(ka) + i H1(2ka)/(ka)) at k a = 0.1, 0.5, 2, 10 (the inscribed polygon dominates); resistance equal to the F-418 power to 8.0e-14; added mass at k a = 0.1 within 5.0e-3 of 8 rho a^3/3; analytic face integrals equal to closed forms to 4.4e-16; disk potential within 2.7e-3 of 4 a E(r/a); centre pressure within 1.7e-3 of rho c V (1 - e^{-i k a}); a non-uniform velocity continuous with the F-418 field pressure to 6.9e-5; negative controls (coarse mesh 3.8e-2, naive collocation 1.8e-2 and 5.0e-2) as registered. Partial because one of eight mutations (the guard of an edge through the point, giving NaN) was not caught by the registered tests (their largest-error aggregation dropped NaN); caught after amendment 1 by a NaN-propagating copy. Not covered: unbaffled or curved radiators, a mutual impedance matrix of several velocity fields, feedback of the added mass into the structure.

Follow-up item (proposed, optional), title "Mutual radiation impedance matrix of a baffled plane and the light-fluid added mass and damping of structural modes", deps F-551:

> Every limit not stated here is registered in the ADR before the run. The radiation impedance matrix Z_mn = (1/2) int P[V_m] conj(V_n) dS of several velocity fields of a baffled plane by the surface pressure of F-551, symmetric within a registered tolerance, its diagonal equal to the impedance of F-551 within 1e-10; for two small baffled pistons the mutual term equal to (i omega rho/(2 pi)) S1 S2 e^{-i k d}/d within 1 percent at k d = 1 and 3; the modal added mass and radiation damping of the first modes of a baffled clamped plate reported, and the shift of its first frequency in a light fluid compared with the first-order estimate from the added mass within a registered limit.
