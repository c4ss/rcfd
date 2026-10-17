# ADR 0055: Wave 29A, frozen-rotor interface library and checks, mixing-plane library, guarded secant outlet update (F-126, F-200)

Status: accepted. Everything under "Decisions" and "Registered checks" was written before any code of this wave and before any run
of this wave. Limits are not changed, widened, replaced or deleted after a run. A miss is reported as measured and the assertion
moves to an ignored `*_known_miss` test with the registered limit intact; the cause is diagnosed, not tuned away. A test set-up
defect gets a NEW corrected test and the old one stays as a record. Addenda (exact numbers, deviations) may be appended at the end.

Clean-room: first principles and published literature only. No vendor code, text or installs, nothing from
vendor material. Waves 29B/29C/29D work in the same tree: this wave adds new files (`rcfd-fv/src/rotor_interface.rs`,
`rcfd-fv/src/mixing_plane.rs`, `rcfd-fv/tests/...`, `rcfd-api/tests/rotor_interface.rs`) and small hunks in shared files
(`rcfd-fv/src/lib.rs`, `outlets.rs`, `rcfd-api` schema and bc_cat for the one new enum value, docs), committed through a temporary
index. The baseline hashes and all existing tests are unchanged by construction (re-verified at the end).

## Audit (HEAD 67dc5d3)

* F-031/ADR 0008 already deliver a steady frozen-rotor zone: `sources.<name>.type = "mrf"` in the ABSOLUTE-velocity formulation.
  In that formulation the frame change at a zone interface is the identity for the solved variable (the absolute velocity is
  continuous) and the convective flux only gets the offset `(Omega x r) . S` on faces INSIDE the zone, which vanishes on a
  surface of revolution. The interface machinery is the planar join of F-114 (`annulus_3d_interface`); curved (cylindrical)
  interfaces are not covered. Checks M1-M7 of ADR 0008 exist (Taylor-Couette across the interface, through-flow, flux closure).
* So the solver already has a frozen rotor. What is missing for F-126 is (i) an explicit, tested velocity transformation
  between frames (needed whenever fields are reported or exchanged in a relative frame, and for the mixing plane), (ii) interface
  diagnostics (flux closure in absolute and relative form, surface-of-revolution defect) as a library, (iii) a check that the
  interface does not corrupt a solution (frame invariance across the interface), and (iv) the mixing plane, which does not exist.
* F-200 already has `mass_flow`, `total_pressure_inlet` (direction, swirl), `outflow` weights, `pressure_outlet` backflow options,
  `vent`, `opening`, `radial_equilibrium_outlet`, `target_mass_flow_outlet` (bernoulli) with the checks of ADR 0022 (A1-A8, B1-B4,
  C1-C5, D1-D3, E2, F1/F2, T1). Their closed-form checks therefore already exist (mass-flow flux 1.4e-16, total-pressure direction,
  backflow); this wave does NOT re-implement them. The open items of the F-200 note that are small and testable are the
  F1 secant miss (guarded secant, below). Compressible/coupled support, time-varying mass flow and lid-pipe meshes stay open.

## Decisions

D1 Frames (`rotor_interface::RotorFrame { origin, axis (unit), omega }`, right-handed rotation, same convention as
`MrfZone`). Frame velocity `u_f(x) = omega axis x (x - origin)`. Relative velocity `w = u - u_f(x)`, absolute `u = w + u_f(x)`.
Frame change from frame `a` to frame `b` of a velocity given relative to `a`: `w_b = w_a + (u_fa - u_fb)(x)`. All are exact
vector sums (error is round-off). Frame flux at a face: `(u_fa - u_fb) . S`.

D2 Frozen-rotor interface (`rotor_interface::FrozenRotorInterface`): the internal faces whose owner is in zone mask A and
neighbour in zone mask B (the join of F-114 produces ordinary internal faces). It stores the face list, the frame-flux jump
`(u_fa - u_fb) . S` per face, the worst relative non-axisymmetry (jump / (|Omega_a - Omega_b| r |S|)), and evaluates
the closure of a flux field: net flux, sum of absolute flux, and the relative-frame flux `phi - u_f . S`. A non-zero jump
(interface not a surface of revolution about the axis) is a located error because then the frozen-rotor fluxes of the two
sides do not close. The solver is not changed: the frozen-rotor interface of the solver is the absolute-formulation one of ADR 0008.

D3 Mixing plane (`mixing_plane::MixingPlane`), constant density (kinematic pressure `p/rho`). Radial bands
`[r_0 < r_1 < ... < r_B]` about the axis (explicit edges, or uniform from min/max). A face belongs to the band containing its
centre radius `r_f` (lower edge inclusive, upper edge exclusive, the last band includes its upper edge); a face outside all
bands is a located error. Donor face data: area vector `S_f` (oriented in the flow direction), centre `x_f`, ABSOLUTE velocity
`u_f` (relative-frame data is converted with D1 first), kinematic pressure `p_f`, scalars `h_f` (e.g. total enthalpy). With
`phi_f = u_f . S_f`, band quantities (sums over the donor faces of band `b`):

* volume flux `Q_b = sum phi_f`, normal `n_b = sum S_f / |sum S_f|`, area `A_b = sum S_f . n_b`;
* specific angular momentum `Gamma_b = sum phi_f r_f u_theta,f / Q_b` (flux-weighted; `r u_theta = (x_f - o) x u_f . axis`),
  flux-weighted radial velocity `U_r,b = sum phi_f u_r,f / Q_b`, flux-weighted scalars `H_b = sum phi_f h_f / Q_b`;
* normal momentum flux + pressure force `F_b = sum (phi_f u_n,f + p_f S_f . n_b)` with `u_n,f = u_f . n_b`.

State passed to a receiving face `g` of band `b` (receiver areas `A'_b = sum S'_g . n_b`, so that mass is conserved to
round-off even if the receiving discretisation tiles the band differently):

* normal velocity `u_n = Q_b / A'_b` (uniform in the band, so that `sum u_n S'_g . n_b = Q_b`: MASS conserved exactly);
* swirl `u_theta(r_g) = Gamma_b / r_g` (so `sum phi'_g r_g u_theta = Gamma_b sum phi'_g = Gamma_b Q_b`: ANGULAR MOMENTUM flux
  conserved exactly for any receiving geometry; exact reproduction of a free vortex);
* radial velocity `U_r,b`; scalar `H_b` (flux of the scalar conserved exactly);
* for the upstream side (pressure passed back): mixed-out static pressure `p_b = (F_b - Q_b u_n)/A'_b` (so
  `sum(phi u_n + p S.n) = F_b`: NORMAL MOMENTUM conserved exactly).

Choice and justification: mass, angular momentum (the quantity the torque on a blade row is the difference of, Euler's
turbine equation), normal momentum (the usual "mixed-out" constraint, gives the pressure rise of the mixing) and flux of the
passive/energy-like scalar are conserved. The mechanical energy flux `sum phi (p + |u|^2/2)` is NOT conserved because mixing
is dissipative; it is reported as a mixing loss `E_before - E_after` (kinetic part of the swirl and normal velocity included).
Derived result (uniform pressure, positive normal velocities, `u = U + d`, `<d>=0` area-averaged): loss
`= (A/2)(U sigma^2 + m3) >= 0` with `sigma^2 = <d^2>`, `m3 = <d^3>`, because `u >= 0` gives `<d^2 (d + U)> >= 0`. Total enthalpy
is conserved as a flux-weighted scalar (energy equation of a steady adiabatic mixing without work); for the compressible
mixed-out state (ideal gas, solve the 1-D conservation for the uniform state) this is NOT implemented: constant density only.
In a rotating receiving frame, `u_theta` above is absolute; the relative value is `u_theta - omega r` (D1). Rothalpy
conservation across a frame change is a statement about the relative flow and is not implemented.

Radial bands and flux-weighted swirl have an inherent first-order band-resolution error for a profile that is not
`Gamma/r` inside a band (stated limit in X4). A band with `|Q_b| <= 1e-9 sum |phi_f|` (flux cancellation, reversed flow) is a
located error (band index) because the flux weighting is singular; no silent area-weight fallback.

D4 Not implemented (stated now): solver-coupled mixing plane (the exchange every outer iteration between an outlet patch
and an inlet patch of the segregated kernel; needs a per-iteration boundary hook in `sim.rs`, other agents edit that file this
wave), so no case-tree keys for the mixing plane or for the frozen-rotor library are added (no natural place: the interface of
the solver is the mesh join + `sources.mrf`, which already has keys); the downstream-velocity constraint, flow-regime options,
sector periodicity of a stage, curved/cylindrical interfaces, transient sliding, compressible mixed-out state.
No published turbomachine (Rotor 37, Hannover compressor) is run or compared: NOT ATTEMPTED.

D5 Guarded secant (F-200 F1 miss): `PressureUpdate::GuardedSecant` (`pressure_update = "guarded_secant"`). Root cause of the F1
miss (ADR 0022 addendum): the secant slope is taken from consecutive iterates while the flow still spins up, so it is
contaminated, and the 1000x clip turns one spurious value into an overshoot. Guard: the secant slope is accepted only if the
last two iterates satisfy `|dm| <= 0.5 |e|` (the mass-flow change is not larger than half the present error, i.e. not
spin-up transient dominated) and the slope lies in `[0.1, 10]` times the dynamic-head estimate; otherwise the dynamic-head
(Bernoulli) slope is used. The old `secant` value is unchanged (bit-identical). The step is relaxed by the same `relax`.

## Registered checks (limits fixed before any run)

Common: kinematic quantities, `rho = 1` unless stated. "Rel." is relative to the stated scale.

### Frames and frozen-rotor interface (library, `rcfd-fv` unit tests and `rcfd-api/tests/rotor_interface.rs`)

R1 Rigid-body rotation (200 deterministic pseudo-random points in a box of half-size 2, axis through a non-origin point and
a non-unit axis input (normalised), `W = 3`): the absolute field `W axis x r`, transformed to a frame with `Omega = W`, is
zero: `max |w| <= 1e-14 W R` with `R = max r_perp`. For frames `a`, `b` with `Omega_a, Omega_b` the field relative to `a`,
`(W - Omega_a) axis x r`, transforms to `(W - Omega_b) axis x r` within `1e-14 W R`. Round trip `a -> b -> a` of arbitrary
vectors: `1e-14` relative to `|w| + W R`. Composition `a -> c` equals `a -> b -> c`: `1e-14`.
R2 Frame flux jump on a planar annulus face (normal along the axis): `|(u_fa - u_fb) . S| <= 1e-14 |Omega_a - Omega_b| R |S|`;
for a face in a plane through the axis (normal tangential) the jump equals the closed form `(Omega_a - Omega_b) r_c (n_theta . S)`
to `1e-14` relative; the constructor of `FrozenRotorInterface` rejects the second with a located error naming the face.
R3 `FrozenRotorInterface` on `annulus_3d_interface` (lower 8 x 3 cells, upper 11 x 4, 48 around): face count > 48 x 11 (non-conformal),
total interface area vs `(N/2) sin(2 pi/N)(r_o^2 - r_i^2)` (flat quads of the 48-gon rings) to `1e-12` relative, worst
non-axisymmetry `<= 1e-12`.
R4 Solver, uniform axial flow across a stationary/rotating pair (`annulus_3d_interface` lower 8 x 3 MRF `Omega = 5`, upper 11 x 4
stationary; walls `inner`/`outer` move axially with `u = (0,0,1)` through velocity expressions, so `u = (0,0,1)`, `p` uniform is the
exact solution; inlet `zmin` fixed `(0,0,1)`, outlet `zmax` zero-gradient + fixed pressure; SIMPLEC linear upwind): `max |u - (0,0,1)| <= 1e-6`
over all cells, pressure range `<= 1e-6`, inlet/outlet mass flow and interface flux equal to `1e-9` relative, relative-frame
interface flux equals the absolute one to `1e-12` relative.
R5 Frame invariance across the interface (laminar Taylor-Couette, `annulus_3d_interface` lower 16 x 2, upper 11 x 3,
`nu = 0.1`, `r_i = 0.5`, `r_o = 1`, inner wall `W_i = 1`, open ends zero-gradient/fixed pressure, `component_diagonal` as in ADR 0008 M4b):
three runs with the lower-zone MRF frame `Omega = 0, 0.5 W_i, W_i` (the upper block stationary in all), semi-analytic solution
`u_theta = A r + B/r` (`A = -1/3`, `B = 1/3`). Criteria (same families as ADR 0008): each run `max |u_theta - analytic| <= 2e-2 V_i`
(`V_i = W_i r_i = 0.5`); mutual difference of the three velocity fields `<= 1e-2 V_i` (the interface does not corrupt the
solution relative to the single-frame `Omega = 0` solve on the same mesh); interface net flux `<= 1e-8 V_i A_if`, sum of absolute interface flux `<= 1e-3 V_i A_if`.
R6 Located errors: `RotorFrame` with zero axis or non-finite omega/origin; `FrozenRotorInterface` with zone masks of wrong length,
no interface faces, identical masks; non-axisymmetric interface (R2); each names the quantity or face index.

### Mixing plane (library unit tests)

X1 Circumferential/band average of an analytic field equals the closed form. Donor samples are generated by 3-point Gauss-Legendre in `r`
per radial cell (10 cells, bands = 5 bands of 2 cells over `r in [0.5, 1.0]`) and `N_theta = 16` equal sectors with area weights
`r dr d theta`; `f = 1 + 2 r + 3 r^2 + 0.5 cos(3 theta) + 0.2 sin(theta)`. Area-weighted band average of `f` equals `integral (1 + 2 r + 3 r^2) r dr / integral r dr` per band
to `1e-13` relative; the circumferential mean of `cos(k theta)` and `sin(k theta)` (`k = 1 .. 15`) is `<= 1e-14` (absolute).
X2 Mass conservation on a mismatched receiver: donor = the `zmax` patch of `annulus_3d(48, 16, ...)`, receiver = the `zmin` patch of an `annulus_3d` with
29 sectors and 11 radial layers (different layers and sector counts, 4 bands): donor field `u = (0, 0, 1 + 0.5 sin(2 theta) + 0.3 r)`,
the passed state on the receiver faces gives per-band flux equal to the donor's to `1e-13` relative, total `1e-13`.
X3 Uniform axial flow `u = (0,0,W = 2.5)`, `p = 7`, `h = 3` on the same pair: receiver `u_n = W`, `u_theta = 0`, `u_r = 0`, `p = 7`, `h = 3` each to `1e-13` relative (absolute `1e-13` for the zero components).
X4 Swirl, uniform axial `W = 2`, `u_theta = Omega_s r` with `Omega_s = 3` (solid body) and `u_theta = G_0/r`, `G_0 = 0.7` (free vortex), Gauss donor samples of X1 with 10 bands (one cell each, `Delta r = 0.05`):
donor band angular momentum flux equals `rho W Omega_s 2 pi (r_2^4 - r_1^4)/4` per band to `1e-12` relative; the angular momentum flux on the 29-sector mismatched receiver
(constructed from mesh patch faces) equals the donor's per band to `1e-12` relative (total `1e-12`); the free vortex is reproduced exactly:
`max |u_theta,receiver - G_0/r| <= 1e-12` (band widths irrelevant); the solid-body profile passed back has the bounded band-resolution error
`max |u_theta - Omega_s r| <= Omega_s Delta r (1 + Delta r/(2 r_min))` (derived from `Gamma_b = Omega_s (r_1^2 + r_2^2)/2`) with `Delta r = 0.05`, `r_min = 0.5` (= `0.1575`).
X5 Normal momentum / mixed-out pressure, two top-hat jets (area fractions `alpha = 0.3`, `1 - alpha` with `u_1 = 2`, `u_2 = 1`, uniform `p = 4`, one band): mixed-out
pressure `p_b = p + alpha (1 - alpha)(u_1 - u_2)^2` to `1e-13` relative, passed `u_n = alpha u_1 + (1 - alpha) u_2` to `1e-13`; normal momentum flux before and after equal to `1e-13`; the mixing
loss equals `(A/2) alpha (1 - alpha) (u_1 - u_2)^2 (U + (1 - 2 alpha)(u_1 - u_2))` to `1e-12` relative and is `>= 0`; for uniform flow the loss is `<= 1e-14`.
X6 Scalar (total enthalpy) flux conservation: `h = 2 + 3 r` with uniform axial flow on the mesh pair: per-band `sum phi h` donor vs receiver `1e-13` relative; the donor band mean equals the Gauss closed form
`(h_0 + h_1 (2/3)(r_2^3 - r_1^3)/(r_2^2 - r_1^2))` to `1e-13` relative (X1 samples).
X7 Mesh-geometry swirl (solid body, 48 x 16 patch donor): the donor band angular momentum from the face centres vs the closed form of X4 within `1e-2` relative (midpoint rule and flat-quad area deficit; the
conservation across the receiver stays `1e-12`).
X8 Located errors (each with a test): non-monotone/too-few band edges; non-finite data; mismatched array lengths; face outside all bands (names the face); zero-area donor or receiver band (names the band); cancelling band flux (names the band); zero axis.

### Guarded secant (F-200 F1/F2)

G1 The F1 case of ADR 0022 with `pressure_update = "guarded_secant"`: outlet mass flow error `<= 1e-4` relative in 6000 iterations (the limit originally registered for the secant),
converged, F2 slope error `<= 1 %`, imbalance `<= 1e-9`. The existing `bernoulli` and `secant` F1 tests and the baseline hashes are unchanged.
G2 `guarded_secant` is bit-identical to `secant` on a synthetic linear response where every secant slope passes the guard (unit test of `PressureUpdater`, 1e-14), and falls back to the Bernoulli slope when `|dm| > 0.5 |e|`.

## Order of work

1. This ADR committed. 2. `rotor_interface.rs` + R1-R3, R6 unit tests; `mixing_plane.rs` + X1-X8 unit tests. 3. `rcfd-api/tests/rotor_interface.rs` (R4, R5). 4. Guarded secant (G1, G2). 5. Docs
(`reference-data.md`, `roadmap.json`), addendum with the measured values and deviations, final gate on a `git archive` copy.

## Addendum 1 (written before any run of this wave): X7 band alignment

While writing the X7 test (before any code of this wave ran) it was noticed that the registered X7 uses the 10 bands of X4 (width
0.05) on a donor mesh with 16 radial layers (width 0.03125): a face is assigned to a band by its centre, so the layers do not
tile the bands and the per-band error would be of the order of the face width over the band width, which is a set-up defect
and not a property of the method. X7 is therefore evaluated with 8 bands of two layers each (edges 0.5, 0.5625, ..., 1.0); the
limit (`1e-2` relative per band) and the closed form (`W Omega_s 2 pi (r_2^4 - r_1^4)/4`) are unchanged. All other checks do
not depend on the alignment (conservation holds for any assignment; X1/X4 use Gauss donor samples whose cells tile the bands).

## Addendum 2 (written after the first unit run, before the corrected test is written): X3 miss and corrected X3b

First run (`cargo test -p rcfd-fv --lib`): X1, X2, X4, X5, X6, X7, X8, R1, R2, R3, R6 meet their limits. X3 as registered MISSES: the
worst deviation of the passed state of a uniform flow on the mismatched mesh pair is 3.7e-1 (limit 1e-13). Cause (diagnosed, not
tuned): the donor and the receiver band areas differ. Faces are assigned to a band by their centre (sharp assignment), the donor
(16 layers) and the receiver (11 layers) do not tile the four bands in the same way, and the flat-polygon rings of 48 and 29
sectors have different areas as well (`(N/2) sin(2 pi/N)`: 0.99715 vs 0.99283 of the ring area). The method conserves mass
exactly on the receiver (`u_n = Q/A'_b`), so a uniform flow `u` passes as `u A_b/A'_b`; the mixed-out pressure
`(F - Q u_n)/A'_b` is shifted likewise. No band-based method that conserves mass exactly on the receiver can pass a uniform flow
unchanged when the band areas differ. The registered X3 is therefore a set-up defect of the pair (the criterion presupposes equal band
areas). The registered X3 test stays as an ignored `*_known_miss` test with its limit intact. New, corrected checks:

* X3b (equal band areas, exact quadrature): donor Gauss samples (10 radial cells, 16 sectors) and a different receiver built the same way
  (15 radial cells, 23 sectors), 5 bands aligned with both (2 and 3 cells per band): the uniform flow `u = (0,0,2.5)`, `p = 7`, `h = 3`
  passes with every component within `1e-13` relative (absolute `1e-13` for the zero components).
* X3c (mesh pair, unequal areas): the passed normal velocity equals `W A_b / A'_b` per band to `1e-13` relative (an identity of the
  method), `u_theta = u_r = 0` to `1e-13` and `h` unchanged to `1e-13`; the library function `MixingPlane::band_areas` reports `A'_b`.
  The maximum area mismatch of the pair is printed, not limited.
