# ADR 0468: F-418 registration: radiated sound of a vibrating surface, equivalent radiated power and the Rayleigh integral of a baffled plane

Status: **REGISTERED 2026-11-14, written before any code of F-418 and before any run of its checks.** Not yet accepted by the
maintainer. Results go in the addendum at the end; nothing above the addendum is changed after a run.

Parents: the design stub [ADR 0321](0321-radiated-sound-of-a-vibrating-surface.md) (SA-11) and the advanced-tier plan
[ADR 0310](0310-structural-solver-advanced-tier-plan.md). Builds on [ADR 0440](0440-f408-registration-advanced-tier-scaffold.md)
(F-408: the declared module `rcfd_post::solid_acoustic`, which this item fills), ADR 0391 (F-399: `HarmonicResponse`, the
convention `x(t) = Re((re + i im) e^{i omega t})`), the direct harmonic solve of ADR 0315 (returns the same `HarmonicResponse`),
[ADR 0066](0066-wave32b-aeroacoustics.md) (`rcfd_post::acoustics`: band numbering and the observer-sphere quadrature) and
F-136 (ADR 0013, ADR 0097: the FW-H integral). Roadmap item F-418 (deps F-408, F-136, F-399). Size M. Number 0468 given by the
lead. Branch `f418-work` from main `9aab52ae`, worktree `w49i-work/f418`, scratch `w49i-work/f418-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here before the code and the runs; each registered run made once; a
missed limit becomes an ignored `*_known_miss` test with the limit intact; defaults bit-identical; no edit of
`baseline_hashes.rs`, the hash tables or a golden file; existing tests are not edited; no new dependency, no unsafe code;
clean-room. Every choice below that the maintainer would normally be asked about is taken on the maintainer's standing
instruction of 2026-11-11 (take the recommendation).

## What F-418 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. Equivalent radiated power: a rigid piston gives rho c S v^2/2 within 1e-12 and a plate mode sin sin gives rho c A v0^2/8 within 0.5 percent. Rayleigh integral of a circular piston of radius a: on-axis pressure 2 rho c v |sin(k (sqrt(z^2 + a^2) - z)/2)| within 1 percent for z from a to 20 a at k a = 2 and 10 with faces of a tenth of a wavelength; far-field directivity |2 J1(k a sin theta)/(k a sin theta)| within 1 percent; radiated power over the equivalent radiated power equal to 1 - J1(2 k a)/(k a) within 1 percent at k a = 0.5, 2 and 10, and at k a = 0.1 the power rho c k^2 (S v)^2/(4 pi) within 2 percent. Levels: 1e-12 W gives 0 dB and the third-octave sums equal the narrow-band total within 1e-10. A surface that is not plane within the registered tolerance is refused by the Rayleigh integral with its deviation; the equivalent radiated power accepts any surface and its report says that it assumes a radiation efficiency of one.

Clauses: **a1** ERP of a rigid piston `rho c S v^2/2` within 1e-12; **a2** ERP of a plate mode `sin sin` equal to
`rho c A v0^2/8` within 0.5 percent; **a3** on-axis pressure of the circular piston within 1 percent, `z` from `a` to `20 a`,
`k a = 2` and `10`, faces of a tenth of a wavelength; **a4** far-field directivity within 1 percent; **a5** `W/ERP = 1 -
J1(2 k a)/(k a)` within 1 percent at `k a = 0.5, 2, 10`; **a6** at `k a = 0.1`, `W = rho c k^2 (S v)^2/(4 pi)` within 2 percent;
**a7** `1e-12 W` gives 0 dB; **a8** third-octave sums equal the narrow-band total within 1e-10; **a9** a surface not plane within
the registered tolerance is refused by the Rayleigh integral with its deviation; **a10** the ERP accepts any surface and its report
says that it assumes a radiation efficiency of one.

## What was read (main 9aab52ae, read only; no Rust run)

ADR 0321 (the stub) and the F-418, F-408, F-136 and F-399 roadmap entries; `crates/rcfd-post/src/solid_acoustic.rs` (the stub of
ADR 0440: only `PENDING = Some("ADR 0321 (F-418)")`); `crates/rcfd-post/src/solid_aerodamp.rs` (how the sibling rcfd-post item
retired its `PENDING`: set to `None`); `crates/rcfd-post/src/acoustics.rs` (ADR 0066: `level_from_ms`, `band_index`,
`nominal_frequency`, `third_octave_bands`, `sphere_quadrature`, private `gauss_legendre`); the list of `crates/rcfd-post/src`;
`crates/rcfd-fem/src/harmonic.rs` (`HarmonicResponse`: `omega`, `re`, `im`, one value per equation, `#[non_exhaustive]`),
`harmonic_direct.rs` (returns `HarmonicResponse`), `model.rs` (`DofMap::equation`, `SolidModel::mesh`, `dofs`), `mesh.rs`
(`FaceRef`, `surface_set`, `face_nodes`, `nodes`, the face maps of Tri3, Quad4, Tri6, Quad8); `crates/rcfd-fem/tests/sa_scaffold.rs`
(the X1 tables). The X1 tables list rcfd-fem and rcfd-linalg modules only (rcfd-fem cannot depend on rcfd-post), and no test
under `crates/*/tests` names `solid_acoustic::PENDING`; so no scaffold row covers this item and the standing decision on
`sa_scaffold.rs` (ADR 0291 pattern) has nothing to apply: the file is not edited. `PENDING` becomes `None`, as ADR 0446 did.

Reuse of F-136: `fwh.rs` integrates the FW-H surface terms over a time history and needs the surface pressure for the
impermeable form, which is not known without an acoustic solve (the stub's reason); it does not fit and is not used. Reused from
`acoustics.rs` (not changed): `band_index` and `nominal_frequency` (the base-10 third-octave numbering) for the power bands,
`P_REF` is not used (power levels are re 1e-12 W), and `sphere_quadrature` for a far-field power that cross-checks the
double-sum power (check S1).

### Arithmetic done before this registration (disclosed)

Two numpy scripts, run once each, no rcfd code: `w49i-work/f418-scratch/closed_forms.py` (log `closed_forms.log`) and
`estimate.py` (log `estimate.log`), the second implementing the planned discretisation below on the registered meshes. The
first run of `closed_forms.py` had an error in the script (the on-axis radial integrand lacked the `1/R` of the kernel; diff
4e-2); corrected and re-run, the log is the corrected run. `estimate.py` failed once on a Python syntax error before computing
anything and was then run once. Their numbers are quoted as estimates in the table of checks.

## Closed forms (derived here; sources cited from memory)

Convention: every amplitude is a complex peak amplitude with `x(t) = Re(X e^{i omega t})`, as `HarmonicResponse`; `k = omega/c`.
Time-averaged power of a peak amplitude carries the factor 1/2: `<p v> = Re(P conj V)/2`; with RMS amplitudes the same power is
`Re(P_rms conj V_rms)` with no 1/2. Every power below is time-averaged, in watts.

1. **Equivalent radiated power** (the radiation efficiency taken as one: each element radiates as a plane wave, `p = rho c v_n`):
   `ERP = (rho c/2) int_S |V_n|^2 dS` (peak), `= rho c int_S v_n,rms^2 dS` (RMS). Rigid piston of area `S`: `rho c S v^2/2`.
   Plate mode `V_n = v0 sin(m pi x/Lx) sin(n pi y/Ly)` on `Lx Ly = A`: `int sin^2 sin^2 = A/4`, so `ERP = rho c A v0^2/8` (the
   roadmap form, correct with peak `v0`).
2. **Rayleigh integral** (baffled plane, `e^{i omega t}`): the free-space monopole of volume velocity `Q` gives `p = i omega rho Q
   e^{-i k r}/(4 pi r)`; the rigid baffle doubles it (image), so `P(x) = (i omega rho/(2 pi)) int_S V_n(y) e^{-i k R}/R dS(y)`,
   `R = |x - y|`, for `x` in front of the plane.
3. **On-axis pressure of a circular piston**: `int_0^a e^{-i k sqrt(z^2+r^2)}/sqrt(z^2+r^2) 2 pi r dr = (2 pi/(i k)) (e^{-i k z} -
   e^{-i k sqrt(z^2+a^2)})`, hence `P(z) = rho c V (e^{-i k z} - e^{-i k sqrt(z^2 + a^2)})` and `|P| = 2 rho c |V| |sin(k
   (sqrt(z^2+a^2) - z)/2)|` (the roadmap form, correct; checked numerically, diff 6e-16).
4. **Far field**: `|y| << r` gives `P ~ F(r_hat) e^{-i k r}/r`, `F = (i omega rho/(2 pi)) int_S V_n e^{i k r_hat . y} dS`; for the
   piston `int_disk e^{i k sin(theta) x} dS = pi a^2 2 J1(u)/u`, `u = k a sin theta`, so the directivity `|F(theta)|/|F(0)| =
   |2 J1(u)/u|` (the roadmap form, correct; checked numerically to 1.3e-15).
5. **Radiated power** from 2: `W = (1/2) Re int_S P conj(V_n) dS = (omega rho/(4 pi)) int int Re(V_n(x) conj V_n(y)) sin(k R)/R
   dS dS = (rho c k/(4 pi)) int int Re(V conj V) sin(k R)/R` (the antisymmetric imaginary part of `V(y) conj V(x)` integrates to zero
   against the symmetric kernel, and `Re(i e^{-i k R}) = sin k R`). The kernel `sin(k R)/R = k sinc(k R)` is smooth (a power series
   in `R^2`), so no singular integral is needed for the power. Low frequency: `sin(k R)/R -> k`, `W = rho c k^2 |Q|^2/(4 pi)`,
   `Q = int V_n dS`: the baffled monopole (the roadmap form at `k a = 0.1`, correct). Two baffled point sources `Q1`, `Q2` at spacing
   `d`: `W = rho c k^2 (|Q1|^2 + |Q2|^2 + 2 Re(Q1 conj Q2) sin(k d)/(k d))/(4 pi)` (the baffled monopole pair; antiphase, the baffled
   dipole). The same `W` follows from the far field: `W = int_hemisphere |F|^2/(2 rho c) dOmega = (1/(4 rho c)) int_sphere |F|^2
   dOmega` (for `y` in the plane `F` is symmetric about it), using `(1/(4 pi)) int_sphere e^{i k r_hat . (y_i - y_j)} = sinc(k R_ij)`.
6. **Radiation impedance of the baffled piston** (`e^{i omega t}`): `Z = F_piston/V = rho c S (R1 + i X1)`, `R1 = 1 - J1(2 k
   a)/(k a)`, `X1 = H1(2 k a)/(k a)`, `H1` the Struve function. Verified here, not assumed: `Z/(rho c S) = (i k S/(2 pi)) E[e^{-i k
   R}/R]` with `R` the distance of two uniform points of the disk, whose density is `p(s) = (4 s/(pi a^2)) (arccos(s/2a) - (s/2a)
   sqrt(1 - s^2/4a^2))`; 400-point Gauss quadrature of that integral equals the closed form with J1 (integral representation, checked
   against its power series to 1e-15) and H1 (the integral `H1(x) = (2 x/pi) int_0^1 sqrt(1-t^2) sin(x t) dt`, checked against its
   series to 6e-15) to 3e-15 (`k a = 0.1`) .. 3.4e-13 (`k a = 10`). Hence `W/ERP = R1 = 1 - J1(2 k a)/(k a)` (the roadmap form,
   correct); the reactance is not part of F-418 (it needs the singular `cos(k R)/R` kernel; listed under limits). At `k a = 0.1` the
   low-frequency formula differs from `R1` by `(k a)^2/6 = 1.67e-3` (computed: 1.668e-3).

No roadmap formula was found wrong. Sources, from memory and **not read** for this work: Rayleigh, *The Theory of Sound*, vol. 2,
sections 278 and following (the baffled-plane integral and the piston); Kinsler, Frey, Coppens and Sanders, *Fundamentals of
Acoustics*, the chapter on radiation (piston on-axis pressure, directivity, impedance with `J1` and `H1`); Pierce, *Acoustics: An
Introduction to Its Physical Principles and Applications*, chapter 5 (Rayleigh integral, baffled piston); Morse and Ingard,
*Theoretical Acoustics*, chapter 7 (radiation from plane surfaces); and the elemental-radiator form of the power (the
`sin(kR)/R` resistance matrix) as used in active structural acoustic control (Elliott and Johnson, J. Acoust. Soc. Am. 94 (1993),
from memory, not read). Everything used is derived above from the monopole and the image principle.

## Design

New code only in `crates/rcfd-post/src/solid_acoustic.rs` (filling the stub; `PENDING = None`). Nothing else in the crate changes;
`acoustics.rs` is called, not edited. No new dependency (a small private complex type; rcfd-post does not depend on rcfd-linalg
and none is added).

- `Phasor { re, im }`: a complex peak amplitude, `abs()`.
- `VibratingSurface`: nodes (`Vec3`), faces of 3 or 4 nodes (Tri3, Quad4; a triangle is the bilinear quadrilateral with its last
  node repeated), nodal complex velocity vectors `[Phasor; 3]`. `new(nodes, faces, velocity)` validates (face sizes, indices,
  lengths, finiteness, positive face area). `from_displacement(model, set, omega, re, im)` takes the faces of a surface set of a
  `SolidModel` and the nodal displacement of the equations (`DofMap::equation(node, c)`, components 0..2; a node without the
  equation has zero displacement) and sets `V = i omega U` (`re_V = -omega im_U`, `im_V = omega re_U`); faces of 6 or 8 nodes are
  refused with an error naming the face and its node count. `from_response(model, set, &HarmonicResponse)` forwards `omega`, `re`,
  `im`. Works for the responses of ADR 0391 and ADR 0315 alike.
- Quadrature on a face: Gauss-Legendre of order `n` per direction on the bilinear map of `[-1, 1]^2` (nodes computed by Newton on
  the Legendre recurrence), weight `w_u w_v |x_u x x_v|`; velocity interpolated with the same bilinear functions (on a collapsed
  quadrilateral this reproduces the linear interpolant of the triangle exactly).
- `equivalent_radiated_power(surface, rho, c) -> ErpReport { power, level_db, area, note }`: `(rho c/2) sum w |V . n|^2` with the
  local unit normal at each point and order 3 (exact for the interpolant on a parallelogram: degree 3 per direction). Any surface
  is accepted (curved, warped, any orientation). `note` = "equivalent radiated power: assumes a radiation efficiency of one (rho c
  |v_n|^2/2 per unit area, peak amplitudes); it over-predicts below coincidence and for surfaces small against the wavelength".
- `power_level_db(w) -> Result<Option<f64>>`: `10 log10(w/W_REF)`, `W_REF = 1e-12` W; `None` for zero, error for negative or not
  finite. `PowerSpectrum::new(freq, power)` (frequencies positive and strictly increasing, powers finite and not negative), `total`,
  `level_db`, `third_octave_bands(f_min, f_max) -> Vec<PowerBand>`: every line assigned by `acoustics::band_index` to its band,
  bands from the one holding `f_min` to the one holding `f_max`, edges and labels as `acoustics::third_octave_bands`.
  `erp_spectrum(model, set, responses, rho, c)`: one line per response at `omega/(2 pi)`.
- `RayleighOptions { plane_tolerance: 1e-6, order: 3, near: 4.0, max_depth: 8 }` (builder setters, validated). `BaffledPlane::new
  (surface, options)`: plane normal `n` = the normalised sum of the face area vectors (refused if its length is below half the
  total area: faces not consistently oriented), origin `p0` = the area-weighted centroid of the quadrature points, deviation =
  the largest `|(x_node - p0) . n|`, diameter `D` = twice the largest `|x_node - p0|`; refused when `deviation > plane_tolerance D`
  with the error "Rayleigh integral: the surface is not plane: deviation {dev:e} m from its mean plane ({dev/D:e} of its diameter),
  above the tolerance {tol:e}". The normal velocity is `V . n` with the plane normal.
- `pressure(x, rho, c, omega)`: formula 2 by the face quadrature; a face (or sub-face) whose centroid is closer to `x` than `near`
  times its diameter is split into four in its parameter square, recursively to `max_depth`; `x` must be in front of the plane by
  more than `plane_tolerance D` (else refused).
- `far_field(direction, rho, c, omega)`: `F` of formula 4 relative to `p0`, the same points as the power (no subdivision);
  `directivity(direction, omega)` = `|F(direction)|/|F(n)|` (refused if `F(n) = 0`).
- `power(rho, c, omega) -> RadiatedPower { power, erp, efficiency, level_db }`: formula 5 as the double sum over all quadrature
  points `(rho c k/(4 pi)) sum_i sum_j w_i w_j Re(V_i conj V_j) s(R_ij)`, `s(R) = sin(k R)/R`, `s(0) = k`, upper triangle doubled
  plus the diagonal, fixed order (deterministic, single-threaded); `erp` by the ERP above, `efficiency = power/erp`.
  `power_from_far_field(rho, c, omega, n_theta, n_phi)`: `(1/(4 rho c)) sum_d w_d |F(d)|^2` over `acoustics::sphere_quadrature`.

## Registered set-ups

Air `rho = 1.2 kg/m^3`, `c = 340 m/s` (as F-136); piston radius `a = 0.1 m`, normal velocity `V = 1e-3 m/s` (peak, real), `k =
(k a)/a`, `lambda = 2 pi/k`. **Polar disk mesh** `disk(a, n_r, n_t)`: centre node and rings `r_j = a j/n_r` of `n_t` nodes at
angles `2 pi m/n_t`; triangles (centre, ring 1) and quadrilaterals between rings; plane `z = 0`, normal `+z`. Fixed numbers (from
faces of at most a tenth of a wavelength radially and along the rim, at least 4 rings and at least 64 rim segments, the last to
keep the polygon's area deficit `(2 pi/n_t)^2/6` at 1.6e-3):

| `k a` | `n_r` | `n_t` | faces | radial width / (lambda/10) | rim arc / (lambda/10) |
|---|---|---|---|---|---|
| 0.1, 0.5, 2 | 4 | 64 | 256 | 0.040, 0.199, 0.796 | 0.016, 0.078, 0.312 |
| 10 | 16 | 100 | 1600 | 0.995 | 1.000 |

On-axis points: 40 values `z = a 20^(i/39)`, `i = 0..39` (log-spaced, `a` to `20 a` inclusive). Directions: `theta = 0, 1, ..,
90` degrees, at `phi = 0` and `phi = pi/n_t` (between two rim nodes). Plate: `Lx = 0.4`, `Ly = 0.3` m, 40 x 30 Quad4, nodal
velocity `v0 sin(m pi x/Lx) sin(n pi y/Ly)` along `z`, `v0 = 1e-3`, modes (1,1) and (2,3). Two pistons: radius `b = 0.01 m`, `k b =
0.05`, `disk(b, 2, 16)` each, centres `(-d/2, 0)` and `(d/2, 0)`, `k d = 1` and `3`, velocities `V` and `+V` or `-V`.

Before the library code, `crates/rcfd-post/tests/sa_solid_acoustic_default_bits.rs` (API of main 9aab52ae only) prints digests of
the `acoustics` paths that this item calls (`band_index` and `nominal_frequency` over bands -30..20, `third_octave_bands` of a
synthetic spectrum, `sphere_quadrature(48, 96)`, `level_from_ms`) and is committed with them as constants.

## Registered checks (limits fixed now)

Tests in `crates/rcfd-post/tests/sa_solid_acoustic.rs` and `sa_solid_acoustic_default_bits.rs`, release profile. "rel" is
`|measured/reference - 1|`. J1 in the tests: the integral representation `(1/pi) int_0^pi cos(t - x sin t) dt` by the 400-point
midpoint rule (spectrally accurate for the periodic integrand), self-checked in J0 against the power series. One-ulp probes: for
each check with a limit of 1e-10 or smaller the test also evaluates the set-up with one input moved by one ulp (the velocity `V`
and one node coordinate) and prints the relative change of the result; the limit must be at least 1e3 times that change (the
probe is part of the check). Expected change: 2.2e-16 to 4.4e-16 (relative condition 1 or 2).

| Id | Clause | Check | Limit | Estimate |
|---|---|---|---|---|
| J0 | (tool) | test J1 (integral) vs power series at `x = 0.2, 1, 4` | `<= 1e-12` abs | 1e-15 |
| E1 | a1 | rigid piston, meshes `k a = 2` and `10`: rel of `ERP` vs `rho c S_h V^2/2`, `S_h = (n_t/2) a^2 sin(2 pi/n_t)` (the polygon's area, in the test); ulp probe | `<= 1e-12` | 4.4e-16 |
| E2 | a2 | plate mode (1,1), 40 x 30: rel of `ERP` vs `rho c A v0^2/8` | `<= 5e-3` | 2.85e-3 (the interpolant's deficit) |
| E3 | a2 (accuracy) | modes (1,1) and (2,3): rel of `ERP` vs `rho c A v0^2/8 f`, `f = ((2 + cos(m pi/40))/3) ((2 + cos(n pi/30))/3)` (the exact ERP of the bilinear interpolant); ulp probe | `<= 1e-12` | 2.2e-16 |
| E4 | a10 | a curved surface (a quarter cylinder, radius 0.1, 12 x 6 Quad4, radial velocity): ERP accepted, finite and positive; the report note of it and of E1 contains "radiation efficiency of one" | holds | holds |
| P1 | a3 | `k a = 2` and `10`, the 40 on-axis points: largest rel of `|P|` vs `2 rho c V |sin(k (sqrt(z^2+a^2) - z)/2)|` (no zero of the closed form in the range: its smallest value is 0.025 of `2 rho c V` at `k a = 2`, 0.125 at `k a = 10`) | `<= 1e-2` | 1.6e-3, 6.5e-4 |
| P2 | a3 (phase) | same points: largest `|P - rho c V (e^{-i k z} - e^{-i k sqrt(z^2+a^2)})|/|closed|` | `<= 1e-2` | 1.6e-3, 1.3e-3 |
| D1 | a4 | `k a = 2` and `10`, the 182 directions: largest `abs(directivity - |2 J1(u)/u|)`, `u = k a sin theta` (an error as a fraction of the on-axis value: at `k a = 10` the closed form has zeros at `u = 3.83, 7.02`, where a relative error is undefined; "within 1 percent" read as 1 percent of the on-axis value) | `<= 1e-2` | 5.7e-4, 3.2e-4 |
| W1 | a5 | `k a = 0.5, 2, 10` (and 0.1 recorded with the same limit): rel of `power/erp` vs `1 - J1(2 k a)/(k a)` | `<= 1e-2` | 1.5e-3, 5.7e-4, 1.1e-4 (1.6e-3) |
| W2 | a6 | `k a = 0.1`: rel of `power` vs `rho c k^2 (pi a^2 V)^2/(4 pi)` (the circle's area) | `<= 2e-2` | 4.9e-3 |
| W3 | (monopole, dipole) | two pistons, `k d = 1, 3`, in phase and antiphase: rel of `power` vs `rho c k^2 Q^2 (2 +- 2 sin(k d)/(k d))/(4 pi)`, `Q = S_h V` of one piston | `<= 5e-3` | 4.9e-4 |
| S1 | (reuse, consistency) | `k a = 2` and `10`: rel of `power_from_far_field(48, 96)` vs `power` | `<= 1e-9` | 3.6e-15, 1.7e-13 |
| L1 | a7 | `power_level_db(1e-12)`: `abs(L)`; ulp probe | `<= 1e-12` | 0 exactly |
| L2 | a7 | `power_level_db(1e-2)` vs 100 dB; zero gives `None`, negative and NaN give errors | `<= 1e-12`; holds | 0; holds |
| B1 | a8 | ERP spectrum of the plate mode (1,1) at 301 frequencies log-spaced 20 Hz to 10 kHz, amplitude `v0/sqrt(1 + ((f - 800)/200)^2)`: rel of the sum of the bands (20 Hz to 10 kHz) vs `total`; ulp probe | `<= 1e-10` | 1e-15 |
| B2 | a8 | every band's power equals the sum of the lines with `band_index` equal to its `n` (bits); a single line at 1000 Hz lands in band 0 (nominal 1000) only; band edges and labels equal those of `acoustics::third_octave_bands` (bits) | exact | holds |
| N1 | a9 | `k a = 2` disk with the centre node raised by `delta = 1e-3 a`: `BaffledPlane::new` refused; the message names "not plane" and a deviation (parsed) within `[0.9 delta, delta]` (the plane's origin rises by about `delta/48`); raised by `1e-9 a` accepted; the quarter cylinder of E4 refused | holds | holds |
| N2 | a9 | field points on the plane (`z = 0`) and behind it refused; refusals of bad input never panic: face of 5 nodes, index out of range, velocity length differs, NaN velocity, zero-area face, `rho`, `c`, `omega` not positive, empty surface, inconsistent orientation, `order = 0` | holds | holds |
| X1 | (input) | a Hex8 block 3 x 2 x 1 (steel, clamped base, a surface set of its 6 top faces), the direct harmonic solve (ADR 0315) under a top pressure at one frequency: rel of the ERP of `from_response` vs the test's own formula `(rho c/2) omega^2 sum_faces (A/36) sum_ab M_ab Re(U_a conj U_b)` (bilinear mass `M` = [4 2 1 2; 2 4 2 1; 1 2 4 2; 2 1 2 4] in face order, rectangular faces, `U` the z displacement); ulp probe | `<= 1e-12` | 1e-15 |
| X2 | (input) | `from_displacement` with `re = U`, `im = 0`: every nodal velocity equals `(0, omega U)` per component (bits); Quad8 faces (a Hex20 surface set) refused naming 8 nodes; unknown set refused; `erp_spectrum` of three responses equals the three ERPs (bits) | exact | holds |
| H1 | (order) | `k a = 10` power with the faces given in reverse order: rel vs the forward order | `<= 1e-12` | 1e-14 |
| Z1 | (scaffold) | `solid_acoustic::PENDING == None` | holds | holds |
| DB1 | (defaults) | the digests of `sa_solid_acoustic_default_bits.rs` equal its constants printed at main 9aab52ae | `==` | holds |
| DB2 | (defaults) | the existing `acoustics_metrics`, `fwh`, `fwh_impermeable`, `noise_proxies` tests pass unedited | pass | holds |

Mutations (on a scratch copy of the library, one at a time; each must be caught by at least one registered check):

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | Rayleigh factor `1/(4 pi)` instead of `1/(2 pi)` (no baffle) | P1, P2 |
| MU2 | `e^{+i k R}` in the pressure | P2 |
| MU3 | ERP without the factor 1/2 | E1, E2, E3 |
| MU4 | power kernel `sin(k R)/(k R)` (k dropped) | W1, W2, W3 |
| MU5 | power diagonal `s(0) = 0` instead of `k` | W1, W2, W3 |
| MU6 | planarity test removed | N1 |
| MU7 | band assignment `band_index(f) + 1` | B2 |
| MU8 | velocity at a quadrature point taken from the first node of the face | E3 |
| MU9 | `from_displacement` without the factor `i omega` (`V = U`) | X1, X2 |

## Decision rule

**done**: every check met at its registered run, every mutation caught, no amendment after a run. **partial**: a check missed
(recorded as `*_known_miss` with its limit), a mutation not caught by a registered check, or a check met only after an
amendment made after its run. **todo**: the module does not build or the piston runs cannot be made.

## Limits (what is not covered)

- No acoustic field solve and no reaction of the air on the structure (light fluid); the radiation reactance (`H1` term, the
  singular `cos(kR)/R` kernel) and the surface pressure are not computed.
- The Rayleigh integral only for a plane radiator in an infinite rigid baffle; a fan frame or housing gets only the ERP, which
  over-predicts below coincidence and for small radiators (efficiency one). No unbaffled plate, no Helmholtz/Kirchhoff integral, no
  boundary-element solve.
- The double-sum power costs `O(P^2)` in the number of quadrature points (`P = 14400` at `k a = 10`); large surfaces at high
  frequency would need a far-field or fast method.
- Faces of 3 and 4 nodes only; the quadratic faces of Tri6 and Quad8 are refused (not interpolated with their own functions).
- Near-field points closer than the subdivision resolves (`max_depth`) lose accuracy; points on the plane are refused.
- The references are closed forms derived here; no measured radiation data.

## Amendment 1 (2026-11-14, written after the registered run of commit 0ed86258 and before any corrected run)

The registered run (`cargo test --release -p rcfd-post --test sa_solid_acoustic -- --test-threads=1`, log
`f418-scratch/run_checks.log`) met every limit, but test `e1_e4_equivalent_radiated_power` failed on the ulp-probe condition of
**E1 at `k a = 10`**: E1 itself 3.73e-14 (limit 1e-12, met), the one-ulp change 1.332e-15, so the limit is 751 times the change,
not the registered 1e3. Diagnosis: the registration expected a one-ulp change of 2.2e-16 to 4.4e-16 (relative condition 1 or 2),
but at `k a = 10` the ERP is a sequential sum of 14400 rounded terms; moving a rim node by one ulp reshuffles the rounding of that
sum, and the change measured is that rounding noise (about 6 ulp), not the condition of the formula. The same noise explains E1
at 3.7e-14 against the numpy estimate 4.4e-16 (pairwise against sequential summation). This is a miss of E1 as registered (the
probe is part of the check): the test stays as `e1_e4_equivalent_radiated_power_known_miss` (ignored, limits and probe rule
intact, the numbers in its reason). Nothing is re-tuned: no corrected E1 at `k a = 10` is introduced.

So that the checks that were met stay in the default suite, a copy `e1_e4_after_amendment_1` runs the same set-ups with the same
limits and the same probe rule for E1 at `k a = 2`, E2, E3 and E4, and prints (does not assert) the probe of E1 at `k a = 10`.
Per the decision rule F-418 cannot be **done**.
