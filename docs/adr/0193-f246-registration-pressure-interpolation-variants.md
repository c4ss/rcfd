# ADR 0193: F-246 registration: pressure-interpolation variants of the steady SIMPLE kernel

Status: **REGISTERED 2026-11-06**, written before any code of F-246. Not yet accepted by the maintainer. Nothing was run before
this text; it rests on reading the code of main 54ec840. Results go in the addendum at the end.

Parents: [ADR 0016](0016-wave13-numerics.md) D2 and its F-208 manufactured problem (the meshes G, G3, K, W, S and the rule that
pressure-interpolation variants need an incompressible manufactured-solution harness), [ADR 0098](0098-wave40b-taylor-couette-converged-stretched-third-level-fast-piston.md)
M5 (the third level n = 64), [ADR 0190](0190-f256-registration-case-tree-wiring-of-the-gradient-options.md) (F-256: the
case-tree checks G1 to G4 that options added later wire under), [ADR 0069](0069-wave33b-transient-accuracy-coupling.md) (the
source construction of an incompressible manufactured solution in a closed box), [ADR 0050](0050-wave27b-robustness-adaptive-stepping.md)
D1 and [ADR 0084](0084-wave37a-sheared-mesh-simplec-linearisation.md) (the kernel's treatments of strongly non-orthogonal
meshes). Roadmap item F-246, split from F-208 on 2026-10-21; its case-tree part was split off as F-256 on 2026-10-22.

Number: 0192 is the highest on main; 0191 is reserved for F-259 and 0194 for F-254. No branch holds a 0193.

House rules ([ADR 0130](0130-wave48a-overview-and-run-record-contract.md)):

- limits are fixed here, before the code and the runs, and are not changed after a run;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact, and the cause is diagnosed, not tuned away;
- a defective set-up gets a new corrected test (by an amendment written before the corrected run), and the old one stays as an
  ignored record;
- the default stays bit-identical: every new code path is opt-in, `crates/rcfd-api/tests/baseline_hashes.rs` and the H1 record
  R0 are not re-recorded, and the new case key uses `hash_omit_default`;
- no new third-party crate, no commit trailers.

Clean-room: every formula below is derived here from a Taylor expansion or from the momentum balance of a fluid at rest. No
published source is quoted for a formula, and no vendor code, text or install was used.

## What F-246 asks

The roadmap entry: pressure-interpolation variants; the set of variants is named in the ADR before the work. On an
incompressible manufactured solution (per-cell momentum source) on the F-208 skewed, warped and stretched meshes at n = 16, 32,
64, each variant's finest-pair L2 order is at least 1.7 for velocity and at least a pressure limit registered in the ADR, and on
the near-orthogonal meshes no variant raises the error over the default by more than 1.10x (the F-208 M3 limit). A
body-force-weighted variant, if named, cuts the spurious velocity of a quiescent stably stratified Boussinesq box on the skewed
mesh by at least 10x against the default, below an absolute bound registered in the ADR. Out of scope: new gradient schemes,
limiter changes, a separate warped-face correction, compressible-solver interpolation, automatic option selection by mesh
quality, case-tree wiring (F-256).

## What the code read shows (main 54ec840, nothing run)

1. **Where the kernel turns cell pressures into gradients** (`rcfd_fv::simple::SimpleSolver::iterate`):
   - `grad_p`, the Green-Gauss cell gradient of the old pressure (`gradient::green_gauss(.., 0)`), is the momentum source
     `-V grad p`, the SIMPLEC term `-(rAU - rAtU) grad p` of `HbyA`, and the old-pressure gradient of `velocity_correction =
     face_flux`;
   - `grad_corr`, the Green-Gauss gradient of the new pressure, corrects the cell velocity `u = HbyA - rAtU grad p`;
   - three least-squares gradients of the pressure form the explicit non-orthogonal part `k . grad p_f` of the face pressure
     gradient when `non_orth_correctors > 0`: in `pressure::solve_pressure_parts_src_k` (the pressure equation, recomputed in
     every corrector pass), in `pressure::corrected_flux` (the flux correction) and in `simplec_non_orth_flux_correction`;
   - the face flux uses the compact difference `lap_f (p_N - p_P)` with `lap_f = |S|^2 / (S . d)` for the rest.
2. **Face values of the Green-Gauss gradient.** Internal faces take `w p_P + (1 - w) p_N`, the value at the point
   `x_f' = w x_P + (1 - w) x_N` where the centre line meets the face (the weight `w` comes from the projected distances).
   Boundary faces take the pressure-condition values (`gradient_wall_values`): on a face whose normal velocity is prescribed the
   condition is `dp/dn = g_n`, extrapolated as `p_P + dn g_n` with `dn` the normal distance. `g_n` is zero without a body
   force; with one it is `ratio a_f . n` (`ratio = rAU / rAtU` of the previous iteration, 1 for SIMPLE), so that the
   boundary flux of the pressure equation cancels the body-force flux at the wall.
3. **Body forces** (`BodyForce`): cell accelerations `a_c` and face values `a_f . S_f`. `uniform`, `from_scalar` (Boussinesq)
   and `from_cells` give internal faces the linear interpolation of the cell values; `from_scalar` gives boundary faces the
   boundary value of the scalar. The face values enter the flux as `rAU_f a_f . S_f` on every non-empty face (balanced-force
   treatment). `wall_pressure_trapezoid` (rotating-frame, acceleration, actuator-disk and Lorentz sources, ADR 0071) replaces
   the wall values of item 2 by values that make the wall-normal cell gradient equal the cell body force.
4. **`gradient::green_gauss(geom, cells, bvals, 1)`** already adds the face-centre correction
   `(w grad_P + (1 - w) grad_N) . (x_f - x_f')` with the gradients of a first pass; the kernel calls it with 0 passes.
5. **The pressure functions are shared.** `solve_pressure_parts*`, `corrected_flux` and `boundary_values_pbc` are also used by the
   transient kernel (`pimple.rs`) and other solvers.
6. **The case tree** builds the steady kernel in `Simulation::new` (`sim.rs`, `Engine::Steady`, algorithms `simple` and
   `simplec` with constant density) and gives it, in every step, the sum of the uniform `physics.body_force`, the Boussinesq
   force of the energy equation (`energy.buoyancy`), a momentum UDF, the forces of `sources` (all types except `porous`) and the
   two-way particle force; `solver.bulk_velocity` adds a uniform driving force inside the kernel. With
   `solver.frozen_flow = true` the flow is not solved.
7. **Defaults of the tree:** `algorithm = "simplec"`, `alpha_u = 0.9`, `alpha_p = 1.0`, `non_orthogonal_correctors = 0`.
8. **The meshes of F-208** (`crates/rcfd-fv/tests/mms_poor_meshes.rs`): images of the unit cube `box_mesh([n; 3])` under
   G (identity), G3 (`x' = x + 0.015 sin(2 pi y)`, `y' = y + 0.015 sin(2 pi x)`), K (`x' = x + 0.7 y + 0.3 z`, `y' = y + 0.3 z`),
   W (each coordinate plus `0.1` times a product of two sines) and S (`Grading::Expansion(50)` in y). K is the affine image of a
   uniform grid: its cells are parallelepipeds, every internal face centre is the midpoint of the centre line (`w = 1/2`,
   `x_f' = x_f`) and `z' = z`. S and G are orthogonal with `x_f' = x_f`. Only W and G3 have `x_f' != x_f`.

## The variants (the set named before the work)

Notation: `P` the owner, `N` the neighbour, `d = x_N - x_P`, `S` the face area vector out of `P`, `w` the owner weight,
`x_f' = w x_P + (1 - w) x_N`, `lap_f = |S|^2 / (S . d)`, `k_f = S - lap_f d`. `f_X` is the body acceleration at the centre of
cell `X` (`BodyForce::cell`). On an internal face `f_f = w f_P + (1 - w) f_N`. On a boundary face `b` of cell `P` with unit
normal `n` and area `A`: `g_b = (a_b . S_b) / A` (the stored face value) and `f_b = f_P + (g_b - f_P . n) n` (the stored normal
component, the owner's tangential component).

**V0 `linear`** (the default, unchanged): as in item 1 and 2.

**V1 `skew_corrected`.** The Green-Gauss face value of an internal face is moved from `x_f'` to the face centre with the
interpolated cell gradient, `p_f = w p_P + (1 - w) p_N + (w g_P + (1 - w) g_N) . (x_f - x_f')`, `g` the gradient of a first
Green-Gauss pass: the first-order Taylor expansion of `p` about `x_f'`, exact for linear `p` on any mesh. Boundary faces keep the
pressure-condition values. It acts on `grad_p` and `grad_corr` (it is `green_gauss(.., 1)`, item 4); the least-squares gradients
are exact for linear fields already and are unchanged. It is a pressure-interpolation variant in the sense of the item: it changes
the face pressure the existing Green-Gauss scheme sums, not the scheme. It is identically V0 where `x_f' = x_f`, so on G, K and S
it changes nothing beyond rounding, and no benefit can be claimed there; it acts on W and G3.

**V2 `body_force_weighted`.** Derivation. Let `Pi` be a local potential of the body force, `grad Pi = f`, and write
`p = q + Pi`. For a fluid at rest under a conservative force, `q` is constant, so any interpolation of `q` is exact where an
interpolation of `p` is not. V2 applies the V0 discretisation to `q` and adds `Pi` back. The potential differences are line
integrals of `f` along straight segments, taken with the trapezoid rule (exact when `f` is linear along the segment):
`Pi(x_f) - Pi(x_X) = 1/2 (f_X + f_face) . (x_f - x_X)`, `Pi(x_N) - Pi(x_P) = 1/2 (f_P + f_N) . d`. The rules:

- **V2-a** (Green-Gauss, internal faces): `p_f = w (p_P + 1/2 (f_P + f_f) . (x_f - x_P)) + (1 - w) (p_N + 1/2 (f_N + f_f) . (x_f - x_N))`.
- **V2-b** (Green-Gauss and least squares, boundary faces whose normal velocity is prescribed and that are not periodic):
  `p_b = p_P + 1/2 (f_P + f_b) . (x_b - x_P)`. It replaces the extrapolation `p_P + dn g_n` of item 2 (and the
  `wall_pressure_trapezoid` values, in the library) in the cell gradients only; the pressure equation keeps its wall condition.
  Other boundary faces keep the pressure-condition values.
- **V2-c** (the least-squares gradients of the non-orthogonal corrections, item 1): `grad p_P = f_P + LS_P(e)`, `LS` the existing
  weighted least-squares fit (weights `1/|delta|^2`) of the reduced increments `e = (p_N - p_P) - 1/2 (f_P + f_N) . d` on
  internal faces and `e = (p_b - p_P) - 1/2 (f_P + f_b) . (x_b - x_P)` on boundary faces, `p_b` the V2-b value where V2-b applies
  and the pressure-condition value elsewhere.
- **V2-d** (body-force part of the internal face flux): `rAU_f [lap_f 1/2 (f_P + f_N) . d + k_f . f_f]`, the `k_f` term only when
  `non_orth_correctors > 0`, in place of `rAU_f a_f . S`. It is the discrete form of `rAU_f f . S` that matches the face pressure
  gradient `lap_f (p_N - p_P) + k_f . grad p_f` term by term. Boundary faces keep `rAU_f a_f . S`. The stored internal face
  values `a_f . S` are not read by V2.
- Without a body force (no `body_force` and no driver) V2 takes the V0 path, so it equals V0 bit for bit (check L0).
- Read with care: V2 assumes the pressure balances the body force. With a body force that the pressure does not balance (the
  manufactured source below), V2-a and V2-b carry an `O(h) |f - grad p|` error in the face pressure where V0 carries `O(h^2)`.

**Fixed-point property of V2 (derived here; checked by B).** Let `f` be linear and conservative (`f = grad Pi`, `Pi` quadratic),
let every internal face have `x_f' = x_f`, every cell be a parallelepiped, and `f_b = f(x_b)` on every boundary face whose normal
velocity is prescribed. Then `u = 0`, `p = Pi + c` is a fixed point of the SIMPLE iteration with V2, and of the SIMPLEC iteration
when `non_orth_correctors = 0` or `simplec_non_orth_compensation` is on:

1. At `u = 0` the momentum matrix has no source besides `V (f_P - grad p_P)`; every V2-a and V2-b face value is `Pi(x_f) + c`
   (trapezoid exact), and Green-Gauss with exact face-centre values of a quadratic on a parallelepiped gives the exact centroid
   gradient (the midpoint errors of opposite faces are equal and cancel), so `grad p_P = f_P`, the predictor is zero and
   `HbyA = rAU f_P`.
2. Every reduced increment of V2-c is zero, so the corrector gradient is `f_P` and its face interpolation is `f_f`.
3. Internal faces: the flux `rAU_f [lap_f dPi + k_f . f_f] - rAtU_f [lap_f (p_N - p_P) + k_f . f_f]` vanishes for SIMPLE
   (`rAU = rAtU`); for SIMPLEC with 0 correctors the compensation `(rAtU - rAU)_f lap_f (p_N - p_P)` closes it; with correctors,
   the compensation of ADR 0050 D1 closes the `k_f` part.
4. Walls: the flux is the prescribed zero; the pressure equation's wall term `rAtU_f |S| ratio g_b = rAU_P g_b |S|` cancels the
   wall body-force flux `rAU_P a_b . S` (both owner values; `ratio` is lagged and equal at the fixed point).
5. The velocity correction gives `u = rAU f_P - rAU grad p_P = 0`.

V0 has no such fixed point on K: on the side walls the exact increment from the cell centre to the face centre is 0 (both at the
same height), but V0 extrapolates `dn g_n` with `g_n = ratio f . n != 0` (the side-wall normals are tilted), an `O(h |f|)` face
error and an `O(|f|)` cell-gradient error in every side-wall cell; with 0 correctors the internal faces also lack `k_f . f_f`.

**Named and not in the set:** a momentum-coefficient-weighted interpolation and a staggered-pressure interpolation (neither is a
poor-mesh treatment; Q7).

## Choices of this registration

**K1. Library (`rcfd-fv`).** A public enum `PressureInterpolation { Linear (default), SkewCorrected, BodyForceWeighted }` and a
public field `SimpleSolver::pressure_interpolation` (`Linear` from `new`); `SimpleSettings` is not changed (struct literals).
V1 and V2 act only in `SimpleSolver::iterate` and its private helpers. The shared pressure functions (item 5) get new entry
points that take an optional gradient function or precomputed gradient; the existing functions keep their arithmetic and call
the new ones with `None` (or the gradient they computed before). The transient kernel and every other solver are unchanged.

**K2. Case key** `solver.pressure_interpolation`: enum `linear` (default), `skew_corrected`, `body_force_weighted`,
`.hash_omit_default()`. `Simulation::new` sets it on the steady kernel. This follows the brief and ADR 0190 (options added later
wire under F-256's checks), although the roadmap entry of F-246 lists case-tree wiring as out of scope (Q6).

**K3. Located errors** (`Diagnostic` at `solver.pressure_interpolation`; first applicable rule wins):

- **R1 (values).** The schema refuses any other value.
- **R2 (no kernel takes it).** A non-default value when the built engine is not the steady constant-density kernel (algorithm
  `coupled`, `piso`, `pimple`, `compressible`, variable density, VOF), or with `solver.frozen_flow = true`.
- **R3 (nothing to weight).** `body_force_weighted` when the steady kernel can receive no body force: `physics.body_force` zero,
  no energy equation with `energy.buoyancy` and a non-zero `physics.gravity`, no momentum UDF, no source of a type other than
  `porous`, `solver.bulk_velocity` zero and no two-way particle coupling.
- **R4 (two wall treatments).** `body_force_weighted` with a source of type `rotating_frame`, `acceleration`, `actuator_disk` or
  `lorentz` (the kernel's `wall_pressure_trapezoid` wall values of ADR 0071 for these; Q3).

**K4. Not wired, stated now.** The transient kernel (PISO/PIMPLE, fractional step), the coupled solver, variable density,
compressible, VOF (Q2); no gradient scheme and no limiter is changed; no automatic selection.

**K5. Separable commits.** (1) this text; (2) `rcfd-fv` (K1) with the library tests; (3) `rcfd-api` (K2, K3) with the tree tests
and the H1 edit; (4) documents and roadmap; (5) the addendum.

## Open questions

**Q1. Name and place of the key.** (a) One enum `solver.pressure_interpolation` (K2); (b) one boolean per variant. Recommendation
(a), reversible before any case uses it. Proceeding on (a).

**Q2. The transient kernel.** (a) Steady kernel only (K4), the transient kernel a later item with its own checks; (b) extend now.
Recommendation (a): F-246's checks are steady. Proceeding on (a).

**Q3. V2 with the `wall_pressure_trapezoid` sources.** (a) Refuse in the tree (R4), the library lets V2-b win; (b) allow, with V2-b
winning; (c) allow, with the trapezoid winning. Recommendation (a) until a rotating-frame check exists. Proceeding on (a).

**Q4. Readings of the acceptance.** (a) "Each variant" is V1 and V2, the new members; V0 is measured and reported as the reference
and as a check of the harness, without a limit. Alternative: V0 also bound. (b) The 1.10x clause binds the velocity L2 error at
n = 32 (the F-208 M3 resolution) on G and G3 (G2 missed its precondition in ADR 0016); the pressure ratio is reported.
Alternative: bind the pressure too. Proceeding on both readings as stated; they are readings, not decisions.

**Q5. The default.** V0 stays the default; no variant becomes the default by this item.

**Q6. Case-tree wiring.** (a) Wire now under F-256's checks (K2, K3, T1 to T4), as the brief asks; (b) library only, as the
roadmap's out-of-scope line reads. Proceeding on (a).

**Q7. Further variants** (momentum-coefficient weighting, staggered pressure): (a) leave out; (b) a new item. Recommendation (a).

## Acceptance checks and limits (fixed here, before any code)

### Library: `crates/rcfd-fv/tests/pressure_interp_f246.rs`

**P (preconditions, measured, set-up).** At n = 16, as F-208's M0b: G3 maximum non-orthogonality at most 12 degrees; K at least
25; W at least 20 with face non-planarity at least 1e-4; S aspect ratio at least 10. W's maximum non-orthogonality is also printed
at n = 32 and 64. For B: on K at n = 16 the largest `|x_f - x_f'| / |d|` over internal faces is at most 1e-12 (item 8; the
fixed-point property needs it); on G3 and W at n = 16 it is at least 1e-6 (so that V1 acts there). A miss is a set-up defect.

**Manufactured solution (M).** Physical coordinates; every velocity component is independent of its own coordinate, so
`div u = 0` exactly:

- `u = sin(1.1 y + 0.3) cos(0.9 z + 0.2)`, `v = cos(1.2 x + 0.1) sin(0.8 z + 0.4)`, `w = sin(0.7 x + 0.5) cos(1.3 y + 0.2)`;
- kinematic pressure `p = cos(0.9 x + 0.3) sin(1.2 y + 0.1) cos(0.8 z + 0.5)`; `nu = 0.1`;
- source (acceleration) `f = (u . grad) u - nu lap u + grad p`, analytic.

Data given to the kernel:

- `body_force`: cell values `f(x_c)`; internal face values the linear interpolation of the cell values dotted with `S` (as
  `BodyForce::from_cells`), so that V0 and V2 see the same data; boundary face values `(grad p(x_b) . S)` (ADR 0069: the kernel's
  no-slip pressure condition is `dp/dn = a_b . n`, and the true wall pressure gradient keeps it consistent with the viscous
  term). V2 therefore acts on a source the pressure does not balance (see the caution above).
- Velocity: fixed value on every boundary face, `u_b = u(x_b) - (E / A_b) n_b` with `E = sum_b u(x_b) . S_b` and
  `A_b = sum_b |S_b|` over all boundary faces. The box is closed and its pressure level is pinned in one cell, so a net boundary
  flux would act as a point source there; the midpoint rule leaves `E = O(h^2)` on K's tilted and W's non-planar faces, and the
  uniform normal shift (`O(h^2)`, second order) removes it. Measured precondition: `|sum_b u_b . S_b| <= 1e-13 sum_b |u_b . S_b|`.
- Initial fields: `u(x_c)` and `p(x_c)` at the cell centres (the converged discrete solution does not depend on them).

Kernel set-up: SIMPLE, `alpha_u = 0.7`, `alpha_p = 0.3`, `ConvScheme::Central` (cell Peclet number at most 0.6 at n = 16),
`non_orth_correctors = 1` (the secondary fluxes of the momentum Laplacian, the pressure equation and the flux correction need
it on K and W, whose non-orthogonality does not fall with h), momentum and pressure `tol_abs = 1e-300`, the other controls as
`SimpleSettings::new`. SIMPLE with `alpha_p < 1` is chosen over SIMPLEC because W reaches about 61 degrees and SIMPLEC at
`alpha_p = 1` diverges from 55 to 60 degrees (ADR 0084).

Stop rule: from iteration 100 on, with `delta_k` the largest velocity change of iteration `k` and
`rho = (delta_k / delta_(k-50))^(1/50)`, the run has converged when `rho < 1` and `delta_k rho / (1 - rho) <= 1e-10 U`
(`U` the largest exact speed at the cell centres), an estimate of the remaining iteration error. Cap 30000 iterations; a run that
reaches the cap is not converged and every check that uses it is NOT MET.

Errors: `e_u = sqrt(sum_c V_c |u_c - u(x_c)|^2 / sum_c V_c)`; `e_p` the same for `p_c - mean(p) - (p(x_c) - mean(p_exact))`,
means volume-weighted. Order of a pair: `log2(e(n) / e(2n))`.

Set-up-defect rule: if V0 does not converge under the stop rule on any mesh at n = 16 or 32, the set-up is defective; an
amendment fixes it before any error of V1 or V2 is computed.

- **M1 (binding).** On K, W and S, for V1 and V2: the order of the pair (32, 64) is at least **1.7** for `e_u` and at least
  **1.2** for `e_p`. The pressure limit: the variant with the weakest wall treatment (V2 under the unbalanced source) has an
  `O(h)` face-pressure error in the wall cells, which drives an `O(h)` pressure error confined to the wall-adjacent layer
  (volume fraction `O(h)`), an L2 order of 1.5; the 0.3 allowance of the velocity limit (1.7 for a design order of 2) gives 1.2.
  V0's orders are reported.
- **M2 (binding, near-orthogonal).** On G and G3 at n = 32: `e_u(V1) / e_u(V0) <= 1.10` and `e_u(V2) / e_u(V0) <= 1.10`. The
  pressure ratios are reported.
- **M3 (reported).** The orders of the pair (16, 32), the ratios `e(V) / e(V0)` on K, W and S at n = 64, the iterations.
- **Heavy rule.** Each (mesh, variant) sequence n = 16, 32, 64 is its own test. A test whose release run on this machine takes
  more than 15 minutes is marked `#[ignore = "heavy: ..."]` and run by name in release; its numbers are recorded in the addendum
  and the roadmap names the heavy tests. The levels n = 16 and 32 of every mesh and variant also run in a non-ignored test
  (M3), as do M2, B, L0 and P.

**B (stratified box, binding).** Mesh K at n = 16, every wall fixed zero velocity. Temperature frozen at the conduction profile
`T = z` on the cell centres and boundary faces (the vertical coordinate is K's unmapped z, so horizontal planes stay planar with
normal `z`; a solved temperature on K's tilted adiabatic side walls would not stay linear, and then `curl(beta g T) != 0`
drives a real flow, not a spurious one). Force `rcfd_fv::energy::boussinesq_force` with `beta = 1`, `g = (0, 0, -10)`,
`t_ref = 0.5`, so `a = 10 (T - 0.5) z`, stably stratified. `nu = 0.1`; velocity scale `U_b = |g| beta dT H^2 / nu = 100`
(`dT = 1`, `H = 1`). Pressure starts at 0. Two kernel set-ups, each run with V0 and with V2 for 4000 iterations:

- B-a: SIMPLEC, `alpha_u = 0.9`, `alpha_p = 1.0` (the tree defaults), `non_orth_correctors = 0`;
- B-b: SIMPLE, `alpha_u = 0.7`, `alpha_p = 0.3`, `non_orth_correctors = 1`;
- the other settings as `SimpleSettings::new`.

`s` is the largest cell speed `max_c |u_c|` after the last iteration. For each set-up:

- **B0 (teeth, precondition).** `s(V0) >= 1e-7 U_b`.
- **B1.** `s(V2) <= s(V0) / 10` (the roadmap's 10x).
- **B2.** `s(V2) <= 1e-8 U_b` (`1e-6`; above the linear-solver floor, which is about `1e-11` here).
- **B3 (converged, precondition).** The largest velocity change of the last iteration is at most `1e-12 U_b` in both runs; if
  not, the set-up's checks are NOT MET.
- Reported: the largest face speed `|phi_f| / |S_f|` of both runs.

**L0 (V2 without a body force is V0).** A lid-driven cavity on K at n = 8 (lid `zmax` at `(1, 0, 0)`, `nu = 0.1`, SIMPLEC tree
defaults, `non_orth_correctors = 1`), 50 iterations with V0 and with V2: every cell value of `u` and `p` equal bit for bit.

### Case tree: `crates/rcfd-api/tests/pressure_interp_f246.rs`

Set-up W: `wedge_2d`, `nx = ny = 16`, `lx = ly = 1`, `ramp_start = 0`, `ramp_angle = 30`; lid `ymax` `velocity_value = [1, 0, 0]`,
the other patches fixed zero; `physics.nu = 0.1`, `physics.body_force = [0, -10, 0]`; `solver.algorithm = "simplec"`,
`convection = "linear_upwind"`, `max_iterations = 60`, `tolerance_u = tolerance_p = 1e-14`. C1 with
`non_orthogonal_correctors = 0`, C2 with 1. Sets: V1 (`skew_corrected`), V2 (`body_force_weighted`).

- **T0 (precondition).** On W the largest `|x_f - x_f'| / |d|` is at least 1e-6 and the maximum non-orthogonality at least 20
  degrees.
- **T1 (located errors).** Refused at `solver.pressure_interpolation`: R1 `"momentum_weighted"` (parse); R2 V1 with
  `algorithm = "pimple"`, V1 with `algorithm = "coupled"`, V1 on a frozen-flow energy case; R3 V2 on C1 without
  `physics.body_force`, and the same with a `porous` source only; R4 V2 on C1 with an `acceleration` source on zone `all`.
  Accepted (controls): V1 on C1 without a body force, V2 on C1, V2 on a buoyant case (energy with `buoyancy` and gravity, no
  `physics.body_force`).
- **T2 (tree equals library, the acceptance's 1e-12).** For C1 and C2, the control (no key, library `Linear`) and each set: the
  largest absolute difference over all cells of `u_x`, `u_y` and `p` between the tree after 60 iterations and a library run
  (`rcfd_fv` only: the tree's geometry, the same fields and boundary conditions, `SimpleSolver` set up as the tree sets it, the
  variant set on its field, 60 `iterate` calls) is at most **1e-12**.
- **T3 (teeth).** Each set's tree fields differ from the default tree run by more than **1e-9**.
- **T4 (defaults bit-identical).** (i) C1 and C2 with `pressure_interpolation = "linear"` written have the settings content hash
  of the case without it and the same fields bit for bit; (ii) `baseline_hashes.rs` passes unedited; (iii)
  `case_hashes_w48u.rs` H1 passes with the one registered edit H2.
- **H2 (the one edit of an existing test).** `case_hashes_w48u.rs`: `DEFAULT_KEYS` gains `("/solver", "pressure_interpolation",
  "\"linear\"")`, its C4 loop gains one non-default copy (`"skew_corrected"`), its module comment names this ADR. R0 is not
  re-recorded.
- **Can fail.** One mutation, restored afterwards and compared byte for byte with the kept copy: the line of K2 that sets the
  variant on the steady kernel removed; then T2 for V2 on C1 must fail. `scripts/limit_audit.py` on the new tests.
- **Gate.** fmt per package, clippy `-D warnings`, every test in release on a `git archive` copy, one process per test, three
  jobs: no failure; a flaky failure is rerun alone and reported with its name.

## Not in F-246

New gradient schemes, limiter changes, a separate warped-face correction, the transient kernel and the other solvers (Q2),
automatic selection by mesh quality, momentum-weighted and staggered interpolation (Q7), a default change (Q5).

## Where to go next

- [ADR 0016](0016-wave13-numerics.md) D2 for the meshes and the scalar manufactured problem of F-208.
- [ADR 0190](0190-f256-registration-case-tree-wiring-of-the-gradient-options.md) for the wiring checks this item follows.
- [Case schema](../case-schema.md) for the key.

## Amendment 1 (2026-11-06): the warped mesh W at n = 32, a supplementary warped mesh W2, and the set-up of B-a

Written after the first runs and before any corrected run. Run before this text: P, L0, T0 to T4, H1 and `baseline_hashes.rs`;
B as registered; V0 on K and S at n = 16 and 32 (n = 64 still running) and on W at n = 16 and 32; and the diagnosis runs below
(V0 only, convergence only, `crates/rcfd-fv/tests/pressure_interp_f246_diag.rs`, ignored tests). No error of V1 or V2 had been
computed. No limit is changed.

**Finding 1: V0 diverges on W from n = 32.** Under the registered set-up V0 converges on W at n = 16 (218 iterations) and
diverges at n = 32: the largest velocity change grows about 1.8 times per iteration from the first iteration and is not finite
at iteration 939. The growth rate stays about the same with 2 or 0 correctors, with upwind or linear-upwind convection instead
of central, with `alpha_u / alpha_p` = 0.5 / 0.2 and 0.2 / 0.1, with pseudo-time steps (local, CFL 1), with the H operator of the
previous iterate, with `velocity_correction = face_flux`, with SIMPLEC (`alpha_p = 0.7`, ADR 0050 compensation, `face_flux`), and
with `nu` = 1 and 0.01. The largest error sits at a box edge (cell centre near x = y = 0.97). The geometry is sound: no cell
centre lies on the wrong side of a face, the weights lie in [0.475, 0.525], and the maximum non-orthogonality is 63.5 degrees at
n = 32 (61.3 at 16, 64.1 at 64). The same harness converges on K and S at n = 16 and 32 and on W at n = 16.

Reading: since neither relaxation nor viscosity changes the rate, the growing mode lives in the step from `HbyA` to the corrected
velocity, `u = HbyA - rAU grad p` with `p` from the compact face Laplacian; on this mesh the Green-Gauss cell gradient and the
compact Laplacian disagree enough that the step amplifies. This is a property of the default kernel on F-208's W from n = 32, not
a defect of the harness. The defaults stay bit-identical (L0, T4, `baseline_hashes.rs` and H1 passed), so the kernel is the one of
main.

**Rule applied.** The set-up-defect rule called for an amendment before any V1 or V2 error; the defect is in the default kernel,
not in the set-up, so:

- **W stays binding as registered.** M1 on W binds V1 and V2 unchanged (orders of the pair (32, 64) at least 1.7 and 1.2, every
  run converged). V0 on W is reported (it diverges). V1 changes the Green-Gauss face values, the part of the step the reading
  above points at, so whether V1 converges on W is measured and reported as it comes; no benefit is claimed in advance. A variant
  that diverges on W is a miss of M1, moved to an ignored `*_known_miss` test with the limits intact.
- **M3 on W** no longer asserts that V0 converges; the divergence is recorded.
- **W2, supplementary.** The F-208 map with amplitude `a` instead of 0.1. Selection rule (convergence only): the largest `a` of
  {0.07, 0.05} for which V0 converges under the stop rule at n = 16 and 32 and its largest velocity change falls over the first 200
  iterations at n = 64. Measured: `a = 0.07` converges at 16 (183 iterations) and 32 (591), and at 64 the change falls from
  3.0e-5 (iteration 3) to 1.3e-7 (iteration 200); `a = 0.05` likewise. So **W2 = W with amplitude 0.07** (45.1 degrees at n = 16).
  Preconditions at n = 16, measured in P: maximum non-orthogonality at least 20 degrees, non-planarity at least 1e-4, largest
  `|x_f - x_f'| / |d|` at least 1e-6. W2 carries the M1 limits (1.7, 1.2) for V1 and V2 and V0 reported, and its levels 16 and 32
  run in M3 (V0 must converge there). W2 is not the roadmap's warped mesh; whether it may stand in for W is Q8.

**Finding 2: B-a as registered does not converge.** After 4000 iterations both runs of B-a (SIMPLEC, `alpha_u = 0.9`,
`alpha_p = 1`, no correctors) are not finite, so B3 fails and B-a is NOT MET as registered. Diagnosis on V0 (box on K at n = 16,
400 iterations): the change falls to 5e-2 by iteration 10 and then grows (3.8e27 at 400), the period-two pressure mode of SIMPLEC
at `alpha_p = 1` on non-orthogonal cells (ADR 0084). With `velocity_correction = face_flux` it falls to 3e-17 by iteration 320;
with `alpha_p = 0.7` to 3e-17; SIMPLE with `alpha_p = 0.3` and no corrector grows slowly. B-b (SIMPLE, one corrector) met every
check.

- **B-a' (corrected):** B-a with `reconstructed_correction = true` (the case key `velocity_correction = "face_flux"`, ADR 0084's
  remedy for that mode), which keeps the tree defaults `alpha_u = 0.9`, `alpha_p = 1`. B0 to B3 unchanged. The fixed point of V2
  survives: at `p_new = p_old` the reconstruction term is zero, and with SIMPLEC
  `u = HbyA - rAtU grad p_old = rAU f_P - (rAU - rAtU) grad p - rAtU grad p = rAU (f_P - grad p) = 0`.
- The registered B-a test stays as an ignored record.

**Heavy rule applied.** V0 on K at n = 64 has run for more than 23 minutes without finishing, and every n = 64 run is of that size,
so every M1 sequence test (n = 16, 32, 64) is heavy: ignored, run by name in release, numbers in the addendum. The levels 16 and
32 run in M3 (non-ignored) on K, W, W2 and S.

**Q8. W2.** (a) W stays the acceptance's warped mesh and W2 is supplementary evidence only; (b) accept W2 as the warped mesh of
F-246's acceptance, since the default kernel cannot be converged on F-208's W from n = 32. Recommendation (a): (b) changes the
acceptance after the fact. Proceeding on (a); a variant that misses on W makes F-246 partial.

**Disclosed, no change:** L0's lid cavity (SIMPLEC, `alpha_u = 0.9`, one corrector, no compensation) does not converge: its
largest speed is 301 after 50 iterations, the axial mode of ADR 0050 D1. The bit comparison still holds; its control "the lid
drives a flow" is met by that growth, not by a converged flow.

## Addendum 1: what was built, and the results (2026-11-06)

Branch `f246-pressure-interp`, after this text and amendment 1. Not merged.

**Built as registered.** K1 to K5 as written. `rcfd-fv`: the module `pressure_interp` (`PressureInterpolation`,
`BodyForceWeighting` with V2-a to V2-d), the field `SimpleSolver::pressure_interpolation` (`Linear` from `new`), one helper in
`SimpleSolver` that forms the two Green-Gauss pressure gradients (`Linear` calls `green_gauss(.., 0)` with the same wall values
as before), `gradient::least_squares_increments` (the existing fit applied to given increments), and two new entry points of the
shared pressure functions, `pressure::solve_pressure_parts_grad` and `pressure::corrected_flux_with`, which the old functions call
with their old gradient, so their arithmetic is unchanged. `rcfd-api`: the key in `schema.rs` (`hash_omit_default`),
`sim::pressure_interpolation` and `check_pressure_interpolation` (R2 to R4, after the energy, source and particle states are
built), one line in `Simulation::new` that sets the variant, and `Sources::has_force`. No Studio file changed. Existing test
edited: `case_hashes_w48u.rs` (H2 only). New tests: `crates/rcfd-fv/tests/pressure_interp_f246.rs`,
`crates/rcfd-fv/tests/pressure_interp_f246_diag.rs` (the ignored diagnosis tests of amendment 1) and
`crates/rcfd-api/tests/pressure_interp_f246.rs`.

**Differences found while building.**

1. T1's registered R2 cases for `pimple` and `coupled` were written on C1, which carries `physics.body_force`; `piso` and `pimple`
   refuse that key first (at `physics.body_force`), so the first run of T1 failed there. The registered text names no body force
   for R2, so both R2 cases drop it. T1 has one control more than registered: the `pimple` case without the key is accepted, so
   R2's refusal is caused by the key.
2. The registered can-fail mutation stops T2 on C1 at V1 (the first row: tree vs library 4.968e-3, T3 change 0). V2's row was
   checked by the same mutation with the rows taken in reverse order (tree vs library 3.756e-1, T3 change 0); both files were
   restored and compared byte for byte with the kept copies.
3. `limit_audit.py` flagged the silent `return` of an unknown set-up in the diagnosis tests; it panics now. No hit in the other
   new tests.
4. B-b ran for about 14 minutes next to B-a (4000 iterations of SIMPLE with a corrector on 4096 cells); the registered settings
   were kept.

**Results.** Errors are L2 over the cells; orders are `log2(e(n) / e(2n))`; "it." counts SIMPLE iterations to the stop rule.

| Check | Result |
|---|---|
| P | at n = 16: G3 10.50 degrees (at most 12), K 38.31 (at least 25), W 61.27 with non-planarity 2.306e-2, S aspect ratio 13.40; W at n = 32 and 64: 63.51 and 64.09 degrees; largest `\|x_f - x_f'\| / \|d\|`: K 1.549e-14 (at most 1e-12), G3 9.173e-3, W 8.422e-2 (at least 1e-6); W2: 45.08 degrees, non-planarity 5.877e-3, 5.842e-2 |
| M (harness) | net boundary flux at most 2.5e-15 of the gross flux 2.3 to 3.1 on every mesh (limit 1e-13 relative) |
| M1 K | V1: velocity 2.176, pressure 1.491 (pair (32, 64); limits 1.7, 1.2), the same numbers as V0 to five digits (V1 changes nothing beyond rounding on K); V2: 2.235, 1.444. All runs converged (1911 to 1923 it. at n = 64). **Met** |
| M1 S | V1: velocity 1.735, pressure 1.756, the same numbers as V0 to five digits; V2: 1.966, 1.318 (1841 to 1867 it. at n = 64). **Met** |
| M1 W | V1 and V2 diverge at n = 32 (not finite at it. 1168 and 1215), as V0 does (939). **Not met** (known misses `m1_w_v1_known_miss`, `m1_w_v2_known_miss`) |
| M1 W2 (supplementary) | V1: velocity 1.999, pressure 1.745; V2: 1.999, 1.648 (2034 to 2037 it. at n = 64). **Met** |
| M2 | at n = 32, `e_u(V) / e_u(V0)` (limit 1.10): V1 1.0000 on G and 1.0015 on G3, **met**; V2 1.3422 on G and 1.1500 on G3, **not met** (known miss `m2_near_orthogonal_meshes_v2_known_miss`). Pressure ratios (reported): V1 1.0000, 0.9995; V2 2.660, 2.244 |
| M3 (orders of the pair (16, 32), velocity / pressure) | K: V0 and V1 2.234 / 1.457, V2 2.279 / 1.456; S: V0 and V1 1.337 / 1.660, V2 2.063 / 1.002; W2: V0 1.975 / 1.754, V1 1.973 / 1.744, V2 1.980 / 1.655; W: every variant diverges at n = 32 |
| M3 ratios at n = 64 (reported) | `e(V2) / e(V0)`: K velocity 1.076, pressure 1.001; S 1.151, 4.375. V1 equals V0 on K |
| B-a (record) | SIMPLEC without correctors: V0 and V2 not finite after 4000 it.; B3 failed, NOT MET as registered (amendment 1) |
| B-a' | SIMPLEC with `face_flux`: largest cell speed V0 1.618e-2 (1.6e-4 `U_b`), V2 3.742e-12 (3.7e-14 `U_b`); ratio 4.3e9 (limit 10), V2 below 1e-6 (limit), V0 above 1e-5 (teeth floor); last change 2.8e-17 in both (limit 1e-10). **Met** |
| B-b | SIMPLE with one corrector: V0 4.553e-3 (4.6e-5 `U_b`), V2 1.034e-11 (1.0e-13 `U_b`); ratio 4.4e8; last change 1.5e-14 and 1.1e-13. **Met** |
| B (reported) | largest face speed: B-a' V0 8.40e-3, V2 4.83e-12; B-b V0 2.32e-3, V2 1.02e-11 |
| L0 | 0 of 512 cells differ in a bit of `u` or `p` (limit 0); the cavity does not converge (amendment 1, disclosed) |
| T0 | wedge W: 29.641 degrees, largest `\|x_f - x_f'\| / \|d\|` 1.760e-2 |
| T1 | refused at `solver.pressure_interpolation`, each with its message: R1 `"momentum_weighted"` (schema); R2 `pimple`, `coupled`, frozen flow; R3 no body force, a `porous` source only; R4 an `acceleration` source. Accepted: V1 without a body force, V2 with `physics.body_force`, V2 on a buoyant case, `pimple` without the key |
| T2 | tree vs library, largest absolute difference over `u_x`, `u_y`, `p` (limit 1e-12): C1 and C2, the control and both variants, 0 each |
| T3 | change against the default tree run (limit > 1e-9): C1 V1 4.968e-3, V2 3.756e-1; C2 V1 2.738e-1, V2 2.950 |
| T4 | (i) settings hash unchanged with the key written at its default (C1 `8129287614d0e713`, C2 `82487320653bc7f2`), 0 of 768 values differ in a bit; (ii) `baseline_hashes.rs` unedited: 3 passed, 1 ignored; (iii) H1 with H2: 41 recorded sources, 0 differ; C4: the copy with `pressure_interpolation = "skew_corrected"` differs from R0 |
| Can fail | the line of K2 that sets the variant removed: T2 C1 V1 4.968e-3 and V2 3.756e-1 (tree vs library), T3 change 0; `sim.rs` restored and equal byte for byte to the kept copy |
| Audit | `scripts/limit_audit.py`: no hit in the new tests after difference 3 |

**Time.** On this machine, shared with other agents' gates: a sequence test (n = 16, 32, 64) took 84 to 141 minutes (5048 to
8445 s), most of it the n = 64 run (4933 to 8261 s); B-a' 1.7 minutes, M2 14 minutes and M3 on W 20 minutes when run alone.

**Which code produced the heavy numbers.** The heavy M1 tests are not run by the gate. V1 and V2 on K, S and W2, and V0 on W2,
ran from the test binary of 4d901bc; V0 on K and S from the binary of 51a639f (before amendment 1; its `mms` harness is the same).
The library code (`rcfd-fv/src`) has not changed since 51a639f; the later commits touched only test files and documents.

**Reading.**

- `skew_corrected` is the default wherever the centre line meets the face at its centre (G, K, S): the same errors to five
  digits. Where it acts (G3, W2) it changes the velocity error by at most 0.5 percent and the pressure error by at most 3 percent
  (lower on W2). It does not stop the divergence on W. No benefit is claimed.
- `body_force_weighted` does what it was built for: a fluid at rest under a linear conservative body force stays at rest to
  1e-11 on the skewed mesh, where the default makes a spurious flow of 1e-2. With a body force the pressure does not balance (the
  manufactured source) it costs accuracy: 8 to 34 percent in velocity, and in pressure up to 4.4 times (1.0 on K, 2.2 to 4.4 on
  G, G3 and S), with the orders still above the limits on K, S and W2.
- The default kernel diverges on F-208's warped mesh W from n = 32 with every variant. The amendment's reading (the step from
  `HbyA` to the corrected velocity amplifies on that mesh) is not proven here; it is the open question Q9 below.

**Q9 (new). The divergence on W.** The roadmap dropped a separate warped-face correction "unless A shows a warped-mesh residual".
A shows more than a residual: the steady kernel cannot be converged on F-208's W from n = 32 (63.5 degrees) with any of the
variants, any relaxation, viscosity, convection scheme, corrector count, pseudo-time or `face_flux`. Options: (a) record it and
leave it (poor meshes beyond about 60 degrees are out of the kernel's range); (b) open an item to diagnose and fix the kernel on
W (for example a consistent gradient in the velocity correction); (c) reopen the dropped warped-face item. Recommendation (b): the
evidence points at the pressure-velocity step, not at the face gradient. No decision is taken here.

**Questions for the maintainer.** Q1 to Q8 stand as registered and amended, each proceeding on its reversible choice; Q9 is new.
F-246 is partial: M1 on W and M2 for `body_force_weighted` are not met.
