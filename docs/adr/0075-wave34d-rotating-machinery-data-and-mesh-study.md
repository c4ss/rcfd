# ADR 0075: Wave 34D, published rotating-machinery data, the F-126 acceptance attempt, and a registered mesh study of the rotating-frame checks

Status: accepted. Everything under "Registered" was written and committed before any of the new test code was written or run and before any
solver run on a published rotating-machinery case. Limits are not changed, widened or deleted after a run; a miss becomes an ignored
`*_known_miss` test with the limit intact, a defective set-up gets a NEW corrected test and the old one stays ignored. Baseline hashes are never
re-recorded. Clean-room: first principles and published literature only; every reference value is cited, anything recalled from memory is marked
UNVERIFIED. Parent: ADR 0071 (wave 33D, addendum: F-126 not attempted, C2/C3 finest-mesh limits registered without a mesh study and missed,
C4 spurious radial/axial speed missed), ADR 0055/0059 (F-126 frozen rotor / mixing plane), ADR 0065/0068 (fan sector meshes), `docs/milestone-fan.md`.

## Registered

### Part A: reading published rotating-machinery data

For each source found the record (in `docs/validation-data.md` and, where it is a dataset, `validation-data/manifest.json`) states: full citation;
URL; what was ACTUALLY READ (the document or page opened and the relevant content read) versus only SEEN CITED (a reference in another paper, a
search snippet; never upgraded to "read"); geometry completeness (which geometric data are public, where); operating-condition data and measured
performance numbers actually read (table or figure; a value read off a plot is marked as such with its reading uncertainty); licence or terms of
use; whether the existing solver (incompressible SIMPLE/SIMPLEC with `rotating_frame` / `mrf`, mixing plane, rotational-periodic sector, SST) and
the `rcfd_mesh::fan` generators could in principle run it, and if not, why.

Vendoring rule: a file is added to the manifest with an upstream URL and checksum only when its licence explicitly permits redistribution or it is a
US-government work marked for public use; otherwise the dataset is listed with no files and status `unresolved` (as `hannover-compressor`).
`validation-data/data/` is git-ignored: downloaded documents are never committed. Numbers transcribed from a document into an in-repo CSV carry
the citation, the table/figure and the page.

Candidate ranking, criteria fixed now (applied in this order; a candidate failing (1) or (2) is "not runnable as is"):
1. Regime: incompressible (relative tip Mach number below 0.3), because the only rotating-frame flow solver is the incompressible pressure-based
   one; the compressible solver is explicit Euler without rotation, viscosity or turbulence.
2. Geometry public and representable: blade sections on cylinders, NACA 4-digit or double circular arc, cylindrical hub of constant radius, a
   shroud with a radial tip gap, no stator / bellmouth / spinner that the published performance depends on and the generator cannot build.
3. Measured quantity defined: which pressure rise (total-to-static, total-to-total, static-to-static), between which planes, how efficiency is
   defined, numeric values readable with a stated or estimable uncertainty.
4. Licence permits use of the numbers (published numbers may always be compared with; vendoring follows the rule above).
Ties: fewer blade rows, then simpler geometry.

### Part B: F-126 acceptance attempt on the best-fitting case

F-126 acceptance (roadmap, restated in ADR 0071): a public low-speed compressor or rotor case reproducing the overall pressure rise (pressure ratio)
and efficiency within published scatter. Outcome classes, fixed now, exactly one is reported:
* NOT RUNNABLE: no candidate passes criteria (1) and (2), or the generator rejects the published geometry (located error, fold), or a quantity the
  comparison needs cannot be computed. The report states exactly what would be needed.
* NOT CONVERGED: the run reaches its iteration cap without the registered convergence criterion, or diverges / stops on the divergence guard.
* MISSED: converged, at least one registered limit exceeded.
* MET: converged and every registered limit met.
Rules fixed now: the case is chosen by the ranking above after reading the sources; the numeric limits, the operating point(s), the compared
quantities, the turbulence treatment and the mesh are registered in a dated amendment of this ADR BEFORE any solver run on that case. The mesh is the
coarsest meaningful one by a rule fixed in that amendment (not by trying meshes). The run is done once per registered set-up. A published fan rebuilt
with the generator and compared with measurement is a validation result of that one comparison; no fan performance claim is made beyond it, and none
from any generator-only fan.

### Part C: registered mesh study of the rotating-frame checks (replaces the ADR 0071 finest-mesh limits)

Disclosure, fixed now: the ADR 0071 max-norm errors on the three meshes h, h/2, h/4 are KNOWN (Taylor-Couette 9.76e-2, 2.61e-2, 6.76e-3 V_i, orders
1.90, 1.95; Ekman against the infinite-depth spiral 4.04e-2, 1.30e-2, 3.58e-3 U_g, orders 1.64, 1.86; the `rotating_frame` spurious speed of the
C2a runs 7.9e-2, 2.4e-2, 6.4e-3 V_i). An order window on that triple passes by construction and is not counted as evidence. The blind part of this
study is therefore a FOURTH mesh h/8 that has never been run, and the Richardson analysis is made on the triple (h/2, h/4, h/8). Spacing halves in
every direction in which the solution varies (the azimuthal and radial directions of the annulus, the wall-normal direction of the Ekman box); the
direction(s) of invariance keep their cells (annulus z: 2 cells between symmetry planes; Ekman x and z: 2 cells, periodic).

Reference solutions (exact for the solved problem):
* Taylor-Couette (unchanged from ADR 0071, exact for the finite annulus): `u_theta = A r + B/r`, `A = -W_i r_i^2/(r_o^2 - r_i^2)`,
  `B = W_i r_i^2 r_o^2/(r_o^2 - r_i^2)`.
* Ekman layer, CORRECTED to the finite height of the box. ADR 0071 compared with the infinite-depth spiral; the box has the velocity fixed to
  `(U_g, 0, 0)` at `y = H = 8 delta`, where the infinite-depth solution is still off by about `e^{-8} = 3.4e-4 U_g` (about 10 % of the N = 64
  error, comparable to the discretisation error at N = 128). Derivation: with `V = (u - U_g) + i w`, ADR 0071 gives `V'' = lambda^2 V`,
  `lambda^2 = -2 i Omega/nu`, `lambda = (1 - i)/delta`, `delta = sqrt(nu/Omega)`. The conditions are `V(0) = -U_g` (no slip) and `V(H) = 0` (the
  fixed top). The solution of the two-point problem is `V(y) = -U_g sinh(lambda (H - y)) / sinh(lambda H)` (it satisfies the ODE, `V(0) = -U_g`,
  `V(H) = 0`; as `H -> inf` it tends to `-U_g exp(-lambda y)`, the ADR 0071 spiral). Then `u = U_g + Re V`, `w = Im V`.

Functionals (for Richardson extrapolation; max norms cannot be extrapolated):
* R1 (full annulus, API, `rotating_frame` at `Omega = W_i`, the C2a corrected set-up): `Q = sum_c u_theta,abs(c) V_c / (2 pi r_c lz)`, which
  approximates `int_{r_i}^{r_o} u_theta dr`; exact `Q_ex = A (r_o^2 - r_i^2)/2 + B ln(r_o/r_i)`. The polygonal cells add their own `O(h^2)` quadrature
  error (stated, part of the measured error).
* R2 (60-degree periodic sector, fv level, the C2b set-up): the seam volume flux `m_lo = sum of phi over theta_min` (the C5a quantity), exact
  `depth * int (u_theta - Omega r) dr`.
* R3 (Ekman box, fv level, the C3b set-up): the two transports per unit width `T_u = sum_c (u_c - U_g) dy`, `T_w = sum_c w_c dy` (averaged over the
  2 x 2 columns), exact `int_0^H V dy = -U_g (cosh(lambda H) - 1) / (lambda sinh(lambda H))`, `T_u = Re`, `T_w = Im` of it.

Meshes: R1 (nx, ny, nz) = (12, 4, 2), (24, 8, 2), (48, 16, 2), (96, 32, 2); R2 (n_theta, n_r) = (2, 4), (4, 8), (8, 16), (16, 32); R3 N = 16, 32, 64, 128.
Convergence (required on every mesh; a run that is not converged is a miss of the check): R1 `tolerance_u = tolerance_p = 1e-10`, cap 20000 iterations;
R2 `res_u < 1e-10` and continuity `< 1e-9`, cap 20000; R3 `max_du < 1e-11` and continuity `< 1e-9` (the C3b criterion), cap 400000.
Iteration-error check I1: after convergence on the two finest meshes, 500 further iterations change each functional by at most `1e-2 |f_{h/4} - f_{h/8}|`
and the max-norm error by at most `1e-2 e_{h/8}`.

Limits, the same for R1, R2 and R3 (for R3 for each of `T_u`, `T_w`), with `f1, f2, f3` the functional on h/2, h/4, h/8 and `f_ex` the exact value:
* L1 monotone convergence: `rho = (f2 - f3)/(f1 - f2)` lies in (0, 1); otherwise the functional is outside the asymptotic range and the check is a MISS (not
  reinterpreted).
* L2 observed order `p = log2(1/rho)` in [1.7, 2.3] (the formal order is 2: central/linear-upwind convection, linear interpolation, midpoint quadrature).
* L3 Richardson-extrapolated error against the exact value: `f_RE = f3 + (f3 - f2)/(2^p - 1)`, `|f_RE - f_ex| <= 0.2 |f3 - f_ex|` (extrapolation removes at least
  80 % of the finest-mesh error).
* L4 error estimate brackets the true error: the fine-grid convergence index `GCI = 1.25 |f3 - f2| / ((2^p - 1) |f3|)` satisfies `|f3 - f_ex| / |f_ex| <= GCI`.
Max norm `e_k = max over cells` of the ADR 0071 error measure (R3 now against the finite-height solution), meshes h ... h/8:
* L5 blind prediction of the h/8 error from the two finest known meshes: `e_pred = e_{h/4}^2 / e_{h/2}`, limit `|e_{h/8}/e_pred - 1| <= 0.2`. (For R3 `e_{h/2}`,
  `e_{h/4}` are re-measured against the finite-height solution in the same run; they are close to, not equal to, the ADR 0071 values.)
* L6 order of the finest pair `log2(e_{h/4}/e_{h/8})` in [1.7, 2.3]; every earlier pair `>= 1.4`.
* L7 the ADR 0071 finest-mesh limits restated on the mesh the study says is needed: `e_{h/8} <= 5e-3 V_i` (R1, R2) and `<= 2e-3 U_g` (R3).
The ADR 0071 tests `c2a_corrected_finest_error_known_miss`, `c2b_finest_error_known_miss`, `c3b_ekman_finest_error_known_miss` stay ignored and unchanged.
R3 also keeps the ADR 0071 spiral-amplitude criterion on h/8 (max |w| within 5 % of the analytic maximum, now of the finite-height solution).

R4, the spurious radial/axial speed of the C4 frame-invariance runs (the C4 corrected set-up: (a) inertial, (b) `mrf` `Omega = W_i`, (c) `rotating_frame`
`Omega = W_i`, (d) `rotating_frame` `0.5 W_i`, (e) `mrf` `0.5 W_i`) on the four R1 meshes. The exact value is zero, so the measured speed is itself the
error against the exact solution. Only the (48, 16, 2) values and the C2a `rotating_frame` sequence are known; the (a), (b), (d), (e) sequences have never
been run. Limits per run: L6 as above (finest pair in [1.7, 2.3], earlier pairs `>= 1.4`; a finest-mesh value below `1e-7 V_i` counts as an order pass).
Reported, no limit: the location (r, z) of the maximum on each mesh, and the mesh on which the ADR 0071 limit `1e-3 V_i` would be met if the
observed order holds. Convergence as R1. The ADR 0071 test `c4_corrected_spurious_speed_known_miss` stays ignored and unchanged.

Code policy: tests only. New files `crates/rcfd-api/tests/rotating_mesh_study.rs` (R1, R4) and `crates/rcfd-fv/tests/rotating_mesh_study.rs` (R2, R3);
the ADR 0071 test files are not edited. A miss stays as an ignored `*_known_miss` test with the limit unchanged.
