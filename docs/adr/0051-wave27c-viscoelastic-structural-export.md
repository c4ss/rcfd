# ADR 0051: Wave 27C, viscoelastic constitutive models with log-conformation, format registry and one-way structural load export (F-135, F-237)

Status: accepted. Everything under "Decisions" and "Registered checks" was written before any code of this wave and before any run
of this wave. Limits are not changed, widened, replaced or deleted after a run. A miss is reported as measured and the assertion
moves to an ignored `*_known_miss` test with the registered limit intact; the cause is diagnosed, not tuned away. A test set-up
defect gets a NEW corrected test and the old one stays as a record. Addenda (exact numbers, deviations) may be appended at the end.

Clean-room: first principles and published literature only. Public pages read on the web for the bulk-data card layouts (page text
treated as data). No vendor code, text or installs, nothing from vendor material. Waves 27A (VOF) and 27B (solver
robustness) work in the same tree: this wave adds new files (`rcfd-fv/src/viscoelastic.rs`, `rcfd-io/src/{registry,nastran}.rs`,
`rcfd-post/src/structural.rs`, tests) and only one-line hunks in shared `lib.rs` files, `reference-data.md`, `interchange-formats.md`,
`case-schema.md`, `roadmap.json`, committed through a temporary index. No existing solver path is edited, so the baseline hashes and
all existing tests are unchanged by construction (re-verified at the end).

## Audit (HEAD bc792a5)

* `PimpleSolver` (PISO / PIMPLE) takes an explicit momentum source through `source_force: Option<BodyForce>` (cell accelerations and
  face values `a_f . S_f`, `BodyForce::from_cells` interpolates linearly), applied face-consistently in four places (ADR 0045). The
  kinematic viscosity `settings.nu` is implicit. Transient PISO/PIMPLE is conditionally stable for large `nu dt / dy^2` with few
  correctors (ADR 0046): the checks use `n_correctors = 4` and small `dt`.
* No viscoelastic model, no symmetric-tensor eigen-decomposition and no tensor transport exist in `rcfd-fv`.
* `rcfd-io` has independent writers/readers per format with no registry and no list of what is supported; no finite-element deck
  writer. `rcfd-post::remap` transfers cell fields between volume meshes (`least_squares` exact for linear fields,
  `conservative_overlap` conserves the integral on box cells). No surface-to-surface transfer exists.
* Case tree: nothing is added for either item (see "Not delivered").

## Decisions (F-135)

D1 Models in conformation form. Velocity gradient `L_ij = d u_i / d x_j`, `C` the symmetric positive definite conformation tensor
(equilibrium `C = I`), relaxation time `lambda`, polymer viscosity `eta_p`, `G = eta_p / lambda`. Upper-convected derivative
`C^ = dC/dt + u.grad C - L C - C L^T`. Stress `tau_p = G (g(C) )` below.

| Model | `C^ =` | `tau_p =` |
| --- | --- | --- |
| Oldroyd-B | `-(C - I)/lambda` | `G (C - I)` |
| Giesekus (alpha) | `-[(C - I) + alpha (C - I)^2]/lambda` | `G (C - I)` |
| Simplified linear PTT (epsilon) | `-(1 + epsilon tr(C - I)) (C - I)/lambda` | `G (C - I)` |
| FENE-P (L2), `f = (L2 - 3)/(L2 - tr C)` | `-(f C - I)/lambda` | `G (f C - I)` |

The first three follow from the stress forms `tau + lambda tau^ + (alpha lambda/eta_p) tau^2 = 2 eta_p D` etc. using
`(C - I)^ = C^ + 2 D` (derived by hand, checked by the homogeneous tests: the Giesekus and PTT shear closed forms below come out
of the same algebra). The FENE-P normalisation (`f = 1` at equilibrium, `L2 > 3`) is the common normalised dumbbell form, recalled
from the literature and NOT checked against a source in this wave (marked unverified); the checks verify the code against its own
derivation.

D2 Every model has the form `C^ = -(1/lambda) P(C)` with `P` a polynomial/rational function of `C` alone (commutes with `C`).

D3 Log-conformation (Fattal and Kupferman 2004): `Theta = log C = R diag(ln c_i) R^T`. In the eigenbasis (`M = R^T L R`,
`Cd = M diag(c) + diag(c) M^T - P(diag c)/lambda`) the exact chain-rule form is used,
`dTheta/dt = R [ divided_difference(ln, c_i, c_j) * Cd_ij ] R^T`, with `dd(ln, a, b) = (ln a - ln b)/(a - b)` and `1/a` for
`a = b`. This equals the Fattal-Kupferman `Omega Theta - Theta Omega + 2B + relaxation` form (diagonal: `2 M_ii - p(c_i)/(lambda c_i)`,
off-diagonal: `(c_j M_ij + c_i M_ji)(ln c_i - ln c_j)/(c_i - c_j)`), but is regular at repeated eigenvalues, e.g. at the start from
`C = I` where the Omega formula is 0/0. Eigen-decomposition: cyclic Jacobi for symmetric 3x3 (tolerance 1e-15 of the off-diagonal
norm). `C = exp(Theta)` is positive definite for any finite `Theta`, which is the point of the formulation.

D4 Uncoupled library `viscoelastic::{Model, Formulation, step_homogeneous}`: `Sym3` stored as 3x3 arrays, RK4 sub-stepping of the
local ODE (conformation or log-conformation), models/parameters validated with located errors (`Result<_, String>` naming the
argument), steady-shear/extension helpers (closed forms) used by the checks.

D5 Coupled solver `viscoelastic::ViscoelasticSolver`, wrapping `PimpleSolver` on the cell-centred mesh. One step: (1) velocity
gradient `L` per cell from the Green-Gauss gradient of the velocity components (boundary values of the velocity BCs); (2) the stress
carrier (`C` or `Theta`, 6 components per cell) is advanced by first-order upwind transport on the step flux `phi` followed by the
local ODE of D4 on `dt` (operator splitting, first order in time); boundary and periodic faces use the owner/partner value
(zero-gradient at walls); (3) momentum: STABILISATION CHOICE (both-sides-diffusion type, explicit/implicit split): the solver
`nu` is `nu_s + nu_p` (`nu_p = eta_p/rho`, rho = 1: kinematic units) implicit, and the explicit source is
`S = div(tau_p) - nu_p lap(u)` evaluated from the lagged `(tau_p, u)`, `div(tau_p)` by Gauss with linear face interpolation of
`tau_p` (boundary faces: owner value), `lap(u)` with the same compact operator as the implicit term (`lap_coeff`, no-slip wall
values). The same viscous operator therefore appears on both sides, the elliptic part of the system is present in the implicit
matrix at full polymer viscosity, and `S` vanishes in a Newtonian-like state; this is the standard remedy for the stress-velocity
decoupling at high Weissenberg number. The force goes into `source_force` (cell and linearly interpolated face values: the
face-consistent form of ADR 0045); (4) `PimpleSolver::step`. Coupling is lagged by one step (first order in time). Not claimed:
unconditional stability, 4:1 contraction, inflow/outflow stress boundary conditions, wall stress BCs other than zero gradient,
second-order transport.
With `eta_p = 0` no source is set and the run is the plain solver.

D6 No case-tree key is added: wiring into `Simulation` needs the shared `sim.rs`/`schema.rs`, edited concurrently by other
agents; a key that is accepted but not wired would be misleading. Not delivered (stated).

## Decisions (F-237)

D7 `rcfd-io::registry`: `FormatInfo { name, extensions, direction, status, conformance }`; `all()` lists every format; `Supported`
entries carry a `fn() -> Result<(), String>` conformance case that builds a tiny mesh plus fields in memory (or a temporary
directory), writes with the crate writer and re-reads or validates with an in-registry reader/parser that is independent of the writer
code path (round trip through the crate reader where one exists: STL, Tecplot, OpenFOAM, Plot3D, Nastran deck; hand parsers for the
write-only formats VTU/VTP/legacy VTK/EnSight/XDMF; embedded tiny samples for the read-only Gmsh and Fluent). `NotSupported`
entries (CGNS, HDF5 container) carry a reason and no case. `run_all()` returns per-format pass/fail with the format name in every
error.

D8 Deck form: Nastran-style bulk data (`GRID`, `CQUAD4`, `CTRIA3`, `CHEXA`, `PLOAD4`, `TEMP`) in free field (comma separated)
format with full double precision, so a round trip is exact. Cards (read from public pyNastran documentation, positions only):
`PLOAD4: SID, EID, P1, P2, P3, P4, THRU/G1, EID2/G3; continuation CID, N1, N2, N3, SORL, LDIR` (shell form: pressures at the corners,
blank P2..P4 mean uniform P1; "blank continuation means normal to the face"); `TEMP: SID, G1, T1, G2, T2, G3, T3`. Not confirmed
by a page read and therefore UNVERIFIED: the sign convention of PLOAD4 (written here so that a positive value is the fluid pressure
pressing on the structure, opposite to the outward element normal of an outward-oriented shell), the fixed 8-character small-field
form (not written; the free-field form is documented in public Nastran manuals, recalled), `CQUAD4`/`CHEXA` node order and
`GRID`/`CHEXA` field positions (recalled), and whether any solver accepts these files (cannot be tested here).
Reader: parses exactly the written subset, `line N` located errors.

D9 Pressure transfer (`rcfd-post::structural`): the wall pressure is a face field on a patch; the structural surface is a
tensor-product quad grid with a parametrisation `(u, v)` (planar `(x, y)`, cylinder `(theta, z)`). Conservative overlap weights
`w_ts = |s cap t| / |t|` in parameter space (the area element is constant in `(u, v)` for both planes and cylinders, so these are
the area-overlap weights): `p_t = sum_s w_ts p_s`. Weights of a target sum to 1, a uniform pressure is exact, `sum p A` is conserved
when both surfaces cover the same parameter rectangle and have equal parametric area density. Force/moment: element force
`p_t A_t n_t` with polygon-facet area vectors; transfer error of force and moment is then purely the geometric difference of the two
facet tessellations.
Temperature (volume field): `rcfd_post::remap::Remapper` onto the structural volume mesh (cell values), then nodal values by
averaging, over the cells around a node, `T_c + g_c . (x_node - x_c)` with `g_c` a least-squares gradient over the face neighbours
(exact for linear fields incl. boundary nodes).
Heat-rate loads (QBDY-type) and CGNS/HDF5 are NOT delivered.

## Registered checks (limits fixed before any run)

`rel(a,b) = |a-b|/|b|`. Time `t` in units of the relaxation time where stated.

F-135, uncoupled (RK4, `dt = lambda/200`, conformation unless stated; `C(0) = I`)
* V1 Oldroyd-B start-up of simple shear, closed forms `tau_xy = eta_p gdot (1 - e^{-s})`,
  `N1 = 2 eta_p lambda gdot^2 [1 - e^{-s}(1+s)]`, `s = t/lambda` (derived above): Wi = 0.5, 1, 5, t in [0, 10 lambda]: max relative
  error (against the value scale at that time, floor 1e-12) conformation <= 1e-9, log-conformation <= 1e-6.
* V2 Oldroyd-B steady shear after `40 lambda`, Wi = 0.1, 1, 10: `tau_xy/gdot = eta_p` rel <= 1e-8, `N1 = 2 eta_p lambda gdot^2` rel <= 1e-8
  (both formulations).
* V3 Steady shear closed forms: PTT `eta = eta_p/Y`, `Y^2 (Y - 1) = 2 epsilon Wi^2`, `N1 = 2 eta_p lambda gdot^2/Y^2` (epsilon = 0.25);
  FENE-P `eta = eta_p/f`, `L2 f^2 (f - 1) = 2 Wi^2`, `N1 = 2 eta_p lambda gdot^2/f^2` (L2 = 20); Giesekus (alpha = 0.2): ODE long
  time vs independent Newton solve of the three steady algebraic equations rel <= 1e-8, and vs the literature closed form
  `eta_p (1-f)^2/(1+(1-2a) f)`, `f = (1-chi)/(1+(1-2a) chi)`, `chi = (sqrt(1 + 16 a (1-a) Wi^2) - 1)/(8 a (1-a) Wi^2)` (recalled from
  memory, unverified) rel <= 1e-6. Wi = 0.1, 1, 10 (t = 80 lambda; ODE vs cubic root rel <= 1e-6 for both formulations).
* V4 Shear thinning: `eta/eta_p` strictly decreasing in Wi over Wi = 0.1, 0.3, 1, 3, 10, 30 for Giesekus, PTT, FENE-P; Oldroyd-B constant
  within 1e-8.
* V5 Uniaxial extension of Oldroyd-B (`L = diag(-e/2, -e/2, e)`, equivalent to the stated `C_zz` equation), `Wi_e = lambda e`: at 0.4
  `C_zz -> 1/(1 - 2 Wi_e) = 5`, `C_xx -> 1/(1 + Wi_e)` within 1e-6 (t = 100 lambda); at 0.5 the exact solution is the unbounded linear
  growth `C_zz = 1 + t/lambda`, rel <= 1e-8 for t up to 200 lambda (no steady state: the singularity); at 0.6 growth rate
  `(2 Wi_e - 1)/lambda`, `C_zz(t)` closed form rel <= 1e-8 (conformation and log-conformation).
* V6 Positive definiteness: log-conformation over `200 lambda` of a prescribed alternating flow (shear Wi = 10 for 5 lambda, planar
  extension Wi_e = 2 for 2 lambda, rest 3 lambda, repeating), RK4 `dt = 0.1 lambda`: minimum eigenvalue of `C` over the whole run
  > 1e-6 and every value finite. The plain conformation form is run on the same flow with forward Euler `dt = 2.5 lambda` and the
  minimum eigenvalue is REPORTED ONLY (it can be negative).
* V7 Jacobi eigen-decomposition: random symmetric matrices (deterministic LCG), `|R diag R^T - A|` <= 1e-13 relative, orthonormal `R` to 1e-14;
  `exp(log(A))` reproduces A to 1e-12 relative for SPD A.
* V8 Located errors: negative or non-finite `lambda`, `eta_p < 0`, `alpha` outside [0, 1], `epsilon < 0`, `L2 <= 3`, non-SPD initial `C` each
  returns `Err` whose text contains the parameter name.

F-135, coupled (2-D mesh `box_2d(4, ny, 1, 1, periodic_x = true)`, walls `ymin`/`ymax` no-slip, rho = 1, uniform axial acceleration `G`
through `source_force` of the plain-solver comparison; PISO with `n_correctors = 4`)
* P1 Start-up of plane Poiseuille flow, Oldroyd-B, `G = 8`, `eta_s = 0.2`, `eta_p = 0.8`, `lambda = 0.2`, channel height 1 (half-width 0.5),
  `ny = 40`, `dt = 5e-4`, `t` to 1.5: centre-line velocity (mean of the two centre cells) vs the modal series (D below) at 30 equally spaced
  times: `max |u_c - u_ref| / u_ss <= 2e-2`; both formulations. Error on `ny = 20` vs `ny = 40` at the same `dt`: ratio <= 0.75.
  The modal series (derived here): `u = sum_n a_n(t) cos(k_n y)`, `k_n = (2n-1) pi/(2 h)`, `lambda rho s^2 + (rho + lambda eta_s k^2) s +
  (eta_s + eta_p) k^2 = 0`, forcing coefficient `g_n = G 4 (-1)^{n+1}/(pi (2n-1))`, `a_n = g_n/(eta k^2) + A e^{s1 t} + B e^{s2 t}`, initial
  `a_n = 0`, `a_n' = g_n/rho` (complex roots handled). Cross-check of this derivation inside the test against an independent method-
  of-lines solution of the PDE pair `rho u_t = G + eta_s u_yy + tau_y`, `lambda tau_t = eta_p u_y - tau` (100 intervals, RK4 `dt = 5e-5`):
  `max |u_series - u_pde| / u_ss <= 2e-3` over the same times.
* P2 Steady Poiseuille, Oldroyd-B, `G = 8`, `eta_s = 0.1`, `eta_p = 0.9`, `lambda = 1`, `ny = 32`, `dt = 2e-3`, `t = 12`: velocity profile vs
  the Newtonian parabola with `eta_s + eta_p`: `max |u - u_N| / u_N,centre <= 1e-2`; wall shear stress total within 1e-2.
* P3 Shear-thinning steady Poiseuille (same parameters, `lambda = 1`): Giesekus alpha = 0.2, PTT epsilon = 0.25, FENE-P `L2 = 20`: profile vs
  the quadrature of `eta_s g + tau_xy(g) = G y'` (`y'` from the centre; `tau_xy(g)` the homogeneous steady-shear closed form of V3, `g`
  found by bisection, `u` by trapezoid on 4000 points): `max |u - u_ref|/u_ref,centre <= 2e-2`; and the profile is flatter than the
  Newtonian one (centre velocity smaller than the Newtonian total-viscosity value).
* P4 Contraction / cylinder benchmark: NOT attempted.
* P5 Newtonian limit: `eta_p = 0` run: velocity bit-identical (max diff exactly 0) to a `PimpleSolver` with `nu = eta_s`; `eta_p = 1e-10`
  within 1e-8 absolute of it after 200 steps of the P1 set-up.
* P6 Baseline hashes (`crates/rcfd-api/tests/baseline_hashes.rs`) pass unchanged; no existing test edited.

F-237
* R1 Registry: at least 10 supported formats, unique names; each supported conformance case returns `Ok`; CGNS and HDF5 are listed
  `NotSupported` with a non-empty reason, `conformance_for("cgns")` is an error naming it; a deliberately corrupted conformance input
  (test hook with tampered bytes) returns `Err` containing the format name for VTU, STL, Tecplot, EnSight and the deck.
* R2 Deck round trip: write then parse: node ids/coordinates/connectivity/pressure and temperature values bit-identical; located error
  `line N` for a malformed card, an unknown card, a missing field, a non-numeric field, duplicate id.
* R3 Uniform pressure `p0 = 3.7e5` mapped plane 8x8 -> 5x7 and tube 96x24 -> 64x20: every element value within 4 eps relative of `p0`.
* R4 Plane patch (fluid `box_mesh([8,1,8])` face `zmax`, `p = 1 + 2x + 3y`) -> shell grid 5x7 (and 13x11, non nested): `sum p A` and the
  force vector within 1e-12 relative; moment about the origin within 5e-3 of `S L` (`S = sum |p| A`, `L` the patch size). Tube (fluid
  `annulus_3d(96, 1, 24, r_in 0.02, 0.05, 1)` patch `outer`, `p = 1e5 (1 + 0.5 cos theta + 0.3 z)`) -> shell tube 64 x 20 of equal radius:
  force vector error `|dF| <= 1e-3 S` (the stated acceptance of F-237), moment about the axis midpoint `|dM| <= 5e-3 S R`.
* R5 Temperature: `T = 1 + 2x - y + 3z`, fluid box `4^3` -> structural `8^3` (nested), `6^3`, `5x7x3` with `least_squares` and
  `conservative_overlap`: nodal values within 1e-11 absolute (including corner nodes); `conservative_overlap` conserves `int T dV` within
  1e-12 relative.
* R6 Located errors (message names the offending item): unknown patch, pressure array length mismatch, NaN pressure, zero-area target
  element, parameter rectangles that do not intersect (zero total weight), structural element referencing a missing node, fewer than
  2 grid lines.
* R7 Third-party solver acceptance: not testable, not claimed. The acceptance sentence about a public CGNS round trip is not met (no CGNS).

## Not attempted / limits of the claim

* F-135: contraction/cylinder benchmarks (P4), case tree wiring (D6), inflow/outflow stress BCs, second-order stress transport, 3-D
  coupled runs, Giesekus/PTT/FENE-P start-up analytic solutions (only homogeneous and steady viscometric references), unconditional
  stability at high Weissenberg number. The item stays `partial`.
* F-237: CGNS (and therefore the CGNS acceptance), third-party solver acceptance, heat-rate loads, case tree, curved surfaces other than
  a tensor-product parametrisation, solid-face `PLOAD4`. The item stays `partial`.

## Addendum A: measured results, misses and deviations (after the runs)

Registered limits are unchanged. Tests: `crates/rcfd-fv/tests/w27c_viscoelastic_homog.rs`, `w27c_viscoelastic_channel.rs`,
`crates/rcfd-io/tests/w27c_registry.rs`, `crates/rcfd-post/tests/w27c_structural.rs`.

### F-135 measured against limit

| Check | Measured | Limit |
|-------|----------|-------|
| V1 Oldroyd-B start-up, corrected (viscometric scale), conformation / log | 6.0e-12 / 4.7e-10 | 1e-9 / 1e-6 |
| V1 as registered (relative to the instantaneous value), conformation | 8.359e-9 | 1e-9 (MISS, ignored `known_miss`) |
| V2 steady shear, Oldroyd-B | ~1e-14 | 1e-8 |
| V3 PTT and FENE-P steady shear closed forms | 1e-14 .. 1e-12 | 1e-6 |
| V3 Giesekus, derived implicit form (ODE vs Newton 2.6e-14) | passes | 1e-6 |
| V3 Giesekus recalled literature formula | 5.1e-3 at Wi 0.1 | 1e-6 (MISS, ignored `known_miss`) |
| V4 shear thinning monotone | strictly decreasing for all three | - |
| V5 uniaxial extension Wi_e 0.4 / 0.5 / 0.6 | 8.2e-9 abs / 1.4e-13 (log 8.2e-11) / 5e-13 | per ADR |
| V6 log-conformation min eigenvalue | 0.1727 | > 1e-6 (plain explicit Euler at dt = 2.5 lambda: -9e33, report only) |
| V7 eigen / log reconstruction | 2.4e-15 | per ADR |
| P1 modal series vs independent 1-D PDE | 1.0e-4 | 2e-3 |
| P1 Poiseuille start-up vs series, ny 40 / ratio 20 -> 40 | 6.57e-3 / 0.664 | 2e-2 / 0.75 |
| P2 steady Oldroyd-B vs Newtonian (eta_s + eta_p) | 6.5e-3, wall shear rel 4.1e-6 | 1e-2 |
| P3 shear thinning profiles (corrected test): Giesekus / PTT / FENE-P | 2.49e-3 / 4.44e-3 / 6.89e-3 | 2e-2 |
| P5 Newtonian limit | eta_p = 0: 0 (bit-identical); eta_p = 1e-10: 1.65e-11 | 1e-8 |
| P6 baseline hashes | unchanged, passing | - |
| P4 contraction / cylinder | NOT ATTEMPTED | - |

Log-conformation and conformation formulations give identical P1 numbers (the stress there is smooth); the log-conformation
advantage is shown by V6 only.

### Deviations and misses

1. V1 metric defect (set-up defect in the registered metric): the relative error against the instantaneous value is
   dominated by rounding in C_xx - C_yy at tiny t (10x smaller dt does not improve it). The registered test is kept as the
   ignored `v1_..._known_miss`; a new `..._corrected` test uses the viscometric scale with the same dt and limits.
2. Giesekus closed form recalled from the literature (chi form) is wrong (5.1e-3 relative eta error at Wi = 0.1). Kept as an
   ignored known-miss test; the closed form derived in this wave (f = -A_yy) is used and agrees with the ODE to 2.6e-14.
3. P3 registered claim "centre velocity smaller than Newtonian" has the wrong sign (shear thinning raises the centre
   velocity). The old test is ignored; `p3_..._corrected` asserts u_c > Newtonian and u(h/2)/u_c > 0.75, plus the closed-form
   profile error limit.
4. First coupled runs used zero-gradient velocity walls (set-up defect, centre u ~ 96); fixed in place to no-slip before any
   valid run.
5. Coupling is first order in time and Green-Gauss gradients on the stress carrier are used; documented limits, not misses.
6. FENE-P normalisation recalled and unverified.

### F-237 measured against limit

| Check | Measured | Limit |
|-------|----------|-------|
| R1 registry | 12 supported cases pass, cgns/hdf5 NotSupported, tamper detected | per ADR |
| R2 deck round trip, located errors | bit exact, `line N` errors | per ADR |
| R3 uniform pressure, plane 8x8 -> 5x7, 13x11, 8x8 and tube 96x24 -> 64x20 | 0 relative | 4 eps |
| R4 plane force 5x7 / 13x11 | 2.5e-16 / 1.3e-16 relative | 1e-12 |
| R4 plane moment 5x7 / 13x11 | 2.4e-3 / 6.4e-4 of S L | 5e-3 |
| R4 tube force |dF|/S | 1.55e-4 | 1e-3 |
| R4 tube moment |dM|/(S R) | 1.8e-15 | 5e-3 |
| R5 linear T nodal error, LeastSquares / ConservativeOverlap (8^3, 6^3, 5x7x3) | <= 2.4e-14 / <= 3.6e-15 | 1e-11 |
| R5 integral of T conserved | <= 1.5e-16 | 1e-12 |
| R6 located errors | all pass | per ADR |
| R7 third-party solver acceptance | not testable, not claimed | - |

Deviations: (a) the first R3 run gave 4.7e-15 against 4 eps (8.9e-16): the normalised overlap weights sum to 1 only up to
rounding, so averaging p directly gives p (1 + O(n eps)). Root-cause fix in the mapping (shifted average about a reference
value) before any recorded pass; the limit is unchanged and now met with 0. (b) The plane fluid mesh is `box_mesh([8,8,1])` with
patch `zmax` (the ADR text wrote [8,1,8]; same geometry, axes named differently). (c) The tube moment is about the axis
midpoint (0,0,0.5).

### Not delivered

CGNS and HDF5 (the "public CGNS example round trip" acceptance is not met), heat-rate loads and the heat-rate part of the
F-237 acceptance, case-tree keys for both items (D6), planar contraction / cylinder benchmark, stress inflow/outflow BCs,
second-order stress transport, 3-D viscoelastic flow, third-party solver acceptance. Both roadmap items stay `partial`.
Unverified references: FENE-P normalisation, recalled Giesekus formula (found wrong), GRID / element node orders / CHEXA
continuation / free-field syntax / PLOAD4 sign (PLOAD4 and TEMP layouts were read from pyNastran documentation).
