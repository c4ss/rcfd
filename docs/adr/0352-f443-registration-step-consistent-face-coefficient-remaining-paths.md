# ADR 0352: F-443 registration: the step-consistent face coefficient on the remaining paths

Status: **REGISTERED 2026-11-12**, written before any code of F-443 and before any run of its checks. Not yet accepted by
the maintainer. Before this text nothing was compiled and no test was run: the code and documents under "What was read"
were read and the arithmetic under "Arithmetic before this registration" was done by hand. Results go in the addendum at
the end; nothing above the addendum is changed after a run.

Parents: [ADR 0245](0245-f344-registration-bdf2-step-remainder.md) (F-344: the opt-in
`PimpleSettings::rhie_chow_step_consistent`, called **C** below, its derivation in amendment 1, the check C3, question 3,
from which this item comes, and amendment 3 on ADR 0076's seams), [ADR 0076](0076-wave35a-transient-seams-and-relaxation.md)
(F-G removed; the seam floor), [ADR 0214](0214-f299-registration-piso-instability-at-large-cell-diffusion-number-across-a-thin-gap.md)
and [ADR 0223](0223-f301-registration-consistent-piso-correctors-moving-mesh-lag-estimate-case-key-and-benchmarks.md) (the
consistent correctors, their body-force and periodic extensions and their case key), [ADR 0258](0258-f322-registration-consistent-correctors-one-settings-value-and-extended-combinations.md)
(F-322: `ConsistentCorrectors`, `consistent_flags`, X1 to X3), [ADR 0347](0347-f445-registration-overset-prescribed-fringe-flux.md)
(F-445: the overset step's prescribed fringe flux, its composition with C), [ADR 0257](0257-f259-registration-tree-wiring-of-prism-and-incompressible-layering-and-moving-characteristic-faces.md)
and [ADR 0266](0266-f350-registration-strong-body-flow-coupling-case-key.md) read 6 (an optional table keeps every case
text and hash). Roadmap item F-443 (depends on F-344).

Number 0352, branch `f443-step-consistent-remaining-paths` from main 51e3bb3c, worktree `w49i-work/f443`, given by the
lead. House rules: `w49i-work/AGENT_RULES.md` (limits fixed here and not changed after a run; a miss becomes an ignored
`*_known_miss` with the limit intact; a defective set-up is corrected only by an amendment committed before the corrected
run; each registered run is made once; defaults bit-identical; no hash table or golden edited; no new crate, no unsafe).
Clean-room: the only algorithm is C, derived in ADR 0245 amendment 1 from first principles; its extension below is
derived here. Names recalled from memory there (Rhie and Chow 1983; Cubero and Fueyo 2007; Pascau 2011) are not used for
any formula.

## What F-443 asks

The acceptance of `docs/roadmap.json`, binding in full:

> rhie_chow_step_consistent acts with the consistent correctors and on PimpleSolver::step_ale_with_walls (defaults
> bit-identical); with a body force a hydrostatic column stays at rest to 1e-10 and the steady solution of a buoyant
> cavity is the same to 1e-9 for two step sizes; the case tree has a key for it; the check C3 of ADR 0245 holds on each
> path, or the ADR names why not with a number.

Clauses: (a1) C acts with the consistent correctors; (a2) C acts on `step_ale_with_walls`; (a3) defaults bit-identical;
(a4) with a body force a hydrostatic column stays at rest to `1e-10`; (a5) the steady solution of a buoyant cavity is the
same to `1e-9` for two step sizes; (a6) a case-tree key; (a7) C3 of ADR 0245 holds on each path, or the ADR names why
not with a number.

**Reading (registered here).** "Each path" is every path of the transient step that C now acts on or is named for: the
consistent correctors (a1), `step_ale_with_walls` (a2) and the body-force path (a5 is C3 with a body force); and the paths
ADR 0245's field documentation names as not covered: the fractional step, coupled (periodic) faces, and the library's
other moving-mesh step `step_sliding`. For these three this ADR names why not with a number. The overset step is not
touched (ADR 0347 composes with C as it is). "The same to `1e-9`" for the buoyant cavity is measured relative to the
largest speed of the run (C3's lid velocity is 1, so C3's measure is the same relative measure); the absolute number is
printed too. "Stays at rest" is measured as the largest cell speed.

## What was read (main 51e3bb3c, read only)

1. ADR 0245 in full (registration, amendments 1 to 3, addenda 1 and 2, the decision record); the head of ADR 0076; ADR
   0347 lines on the composition with C (D1 "Composition", X); ADR 0266 read 5 and 6.
2. `crates/rcfd-fv/src/pimple.rs`: `PimpleSettings` (the consistent fields, `consistent`, `moving_mesh_lag`,
   `rhie_chow_step_consistent`), `step_with` in full (lines 1294 to 1977), `consistent_rau`, `consistent_split`,
   `consistent_non_orth` (2397 to 2501), `ddt_history_weights`; `pimple/face_consistent.rs` in full;
   `pimple/moving.rs` in full (`AleStep`, `step_ale_with_walls`, `step_sliding`, `replace_mesh`); the head of
   `pimple/fractional.rs` and its face coefficient (`gamma_p = vec![dte; ..]`, line 211); `simple.rs` `BodyForce`;
   `energy.rs` `boussinesq_force`, `BuoyantPimple`; `ale.rs` `move_mesh`, `set_follow_conditions`.
3. Tests: `bdf2_remainder_f344.rs` (`settings`, `Checks`, `cavity_run`, C3), `ale_gcl.rs` (G2, G3: the translating
   cavity), `ale_flows.rs` G6, `transient_buoyancy.rs` (the cavity and the stratified column), the E-part of
   `sliding_interface_f250.rs` (`pair`, `tc_solver`), `rcfd-api/tests/consistent_correctors_case_f301.rs` (C1 to C4).
4. `crates/rcfd-api/src/schema.rs` (the optional table `solver.consistent_correctors`), `sim.rs` (the transient
   settings, lines 1966 to 2031; the located errors of `solver.consistent_correctors.enabled`, 2346 to 2364),
   `dynmesh.rs` (the tree's dynamic mesh calls `step_ale_with_walls` and `replace_mesh` only; no `step_sliding`, no
   overset, no immersed solid in `rcfd-api`).

What the code read shows:

- In `step_with` the gate of C is `s.rhie_chow_step_consistent && !cf.enabled && self.ale.is_none()` (ADR 0245
  amendment 3). C changes two things: the face coefficient `gamma_p` (harmonic in place of linear) and the added flux
  `add_lagged_gradient`. `gamma_p` then feeds the carry-over, the body-force face term (internal faces), the pressure
  equation and the flux correction (through `gamma_j`), and the jump faces.
- With the consistent correctors (`r_t = Some`), the pressure equation and the flux correction take `gamma_t =
  interp(rAtU)`, and `consistent_split` adds the compensation `dr_f snGrad(p_prev) |S|` with `dr_f =
  face_interpolate(rAtU - rAU)`; `consistent_non_orth` adds `dr_f k . grad(p_prev)_f` with the same `dr_f`. The cell
  velocity is `HbyA - (rAU - rAtU) G p_prev - rAtU G p_new + rAU b`.
- On a moving mesh (`self.ale = Some`) the carry-over is `gamma_p / dt * a.defect` (the old defect scaled by
  `interp(V0 / V1)`), the time scheme is implicit Euler, `a.add_wall_flux` adds `gamma |S| dp/dn` on moving walls
  (boundary faces only). `step_ale`, `step_ale_with_walls` and `step_sliding` all set `self.ale` and call `step`.
- The fractional step has the uniform face coefficient `dte` and no carry-over.

## Arithmetic before this registration (disclosed)

**D1. C with the consistent correctors.** Write `Gamma = harmonic(rAU)` and `gamma_t = interp(rAtU)`. A consistent
corrector with C forms, on an internal face,

`phi = interp(HbyA) . S + (Gamma / dt) K - [interp(rAU x) - Gamma interp(x)] . S + Gamma b_f + (gamma_t - Gamma) snGrad(p_prev) |S| - gamma_t snGrad(p_new) |S|`,

with `x = G p_prev - b` (the added flux of C), that is: the compensation of the consistent split takes `dr_f = gamma_t
- Gamma` face by face in place of `interp(rAtU - rAU)`. With `p_new = p_prev = p` (a converged corrector loop) this is
`interp(HbyA) . S + (Gamma / dt) K - [interp(rAU x) - Gamma interp(x)] . S + Gamma b_f - Gamma snGrad(p) |S|`, the flux
of C without the consistent correctors, term for term; and the cell velocity is `HbyA - rAU G p + rAU b`, as there. So
the converged step is C's, and ADR 0245's defect equation `(c0 d - c1 d^n + c2 d^{n-1}) / dt + a_f d = interp(G p - b) .
S + b_f - snGrad(p) |S|` holds: the steady defect does not depend on `dt`. The non-orthogonal compensation (ADR 0258 X1)
takes the same `dr_f`, so its converged part is `-Gamma k . grad(p)_f`, again C's. Had `dr_f = interp(rAtU - rAU)` been
kept, the converged face term would be `-interp(rAU) snGrad(p)` against the carry-over's `Gamma`: the steady defect
would be `[interp(x) - (interp(rAU) / Gamma) snGrad p] / a_f`, and `interp(rAU) / Gamma` depends on `dt` (mutation MM1).

On a boundary face both coefficients are the owner's value (`harmonic_face` and `face_interpolate` return
`cells[owner]`), so `gamma_t - Gamma = rAtU_o - rAU_o`, the same two numbers subtracted as `face_interpolate(rAtU -
rAU)` there: every boundary and coupled face of the split, including the pair mean `coupled_gamma(dr_f)` of ADR 0223 E2,
is bit for bit the default's (check P1). `gamma_t` itself is kept linear: it enters only the corrector iteration, not
the converged state, and keeping it leaves ADR 0214's stability argument as it is.

**D2. C on `step_ale_with_walls`.** The moving step solves `(V1 u - V0 u^n) / dt + A u - H = -V1 G p + V1 b`, so `a_P /
V1 = 1 / dt + a` and `rAU = V1 / a_P`; the carry-over is `(gamma / dt) D0` with `D0 = interp(V0 / V1) (phi^n -
interp(u^n) . S^n)`. With C the converged defect obeys `d = Gamma R + (Gamma / dt) D0`, and `1 / Gamma = interp(a_P / V1)
= 1 / dt + a_f`, so `(d - D0) / dt + a_f d = R`: the step size enters only through the Euler difference. On a mesh whose
cells keep their volumes (a rigid translation) a state steady in the mesh frame has `D0 = d^n = d`, so `a_f d = R`
whatever `dt`. Nothing in the derivation of ADR 0245 needs the mesh at rest; the moving-wall flux is on boundary faces,
where `Gamma` is the default's coefficient. So the gate's `self.ale.is_none()` is dropped.

`step_sliding` takes the same branch, with a remapped old defect on faces that change between steps; the derivation
holds for the faces that persist and says nothing for the remapped ones, and no check of this item is built on a sliding
interface. **Restriction, taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation):
`step_sliding` keeps the default step** (it clears the flag for its own step and restores it, so the settings a caller
reads are unchanged); check S1 shows it bit for bit.

**D3. A hydrostatic column.** With a uniform `b` and the pressure `p = b . x + c`, the Green-Gauss gradient of the
linear `p` is exact (the fixed-gradient boundary value `p_P + dn (b . n)` of a fixed-velocity face is exact for linear
`p`), so `x = G p - b = 0` in every cell (to rounding); C's added flux vanishes, the body-force face term `Gamma b_f`
balances `Gamma snGrad(p) |S|` face by face, and `u = 0` is a fixed point of the C step. The default has the same fixed
point (its coefficient is `interp(rAU)`). Started from `p = 0` the step must find the hydrostatic pressure; the
corrector and outer loops then converge to the fixed point; each pass reduces the velocity by about `H / a_P`, with
`nu dt / h^2 = 0.1` here about `0.4 / 1.4 = 0.29`, six passes a step: `0.29^6 = 6e-4` per step; from `u ~ b dt = 0.1`
after the first predictor, below `1e-10` after four steps. Where the check can fail: a face term with a coefficient
other than the pressure term's (MM5: `(interp(rAU) - Gamma) b_f`, relative size `(Delta rAU / rAU)^2 ~ 1e-4` at the
walls, where the diagonals are 5 and 4 `nu / h^2`: a flux of about `1e-4 rAU g |S| = 1e-4 x 0.0095 x 9.81 x 0.1 = 1e-6`)
or an added flux with `x = G p` (MM4: `[Gamma - interp(rAU)] b . S`, the same size). Both are four orders above the
limit.

**D4. The buoyant cavity.** `BuoyantPimple` advances the temperature on the previous flux and then the flow with the
Boussinesq force of the new temperature (`energy.rs`); a state where nothing changes between steps satisfies the
steady energy equation, which has no `dt`, and the flow step's steady equations, whose only `dt` is C3's face term. So
with C the steady state should not depend on `dt`. Ra `1e3`, Pr `0.71` in the units of `transient_buoyancy.rs` (`nu =
Pr`, `kappa = 1`, `g = Ra Pr`): de Vahl Davis's `u_max` at Ra `1e3` is about `3.65` (from memory; used only for a
Courant estimate): on 20 by 20 cells `h = 0.05`, at `dt = 0.004` the Courant number is `3.65 x 0.004 / 0.05 = 0.29`, the
cell Peclet number of the central scheme `3.65 x 0.05 / 0.71 = 0.26`. `a = 4 nu / h^2 = 1136`, `a dt = 4.5` and `1.1`
for `dt = 0.004` and `0.001`: the range in which `a dt / (c0 + a dt)` bends (ADR 0245), so the default's steady state
should differ between the two steps. Time to steady: the slowest decay is thermal, rate about `2 pi^2 kappa = 20`;
`t = 8` gives `e^{-160}`, ample, even at a rate a tenth of that (`e^{-16} = 1e-7` of the start-up velocity, about
`4e-7` relative to the start-up transient; that is the risk named in the limits section). Ra `1e3` and not `1e4`: a
fourfold smaller Courant number at the same step and a faster approach to steady; the dependence C removes is set by
`a dt`, not by Ra.

**D5. The fractional step: why not.** Its face flux is `interp(u*) . S + dte (interp(G p^n) . S - snGrad(p^n) |S|)` with
one coefficient `dte` on every face: the harmonic and the linear face value of a uniform field are the same, and the
commutator `interp(dte x) - dte interp(x)` is zero. C is the identity there by construction, so the step is left as it
is and the flag is not read. Its steady face defect is `dt R(p)`, of first order in `dt`, and it has no carry-over to
make consistent: a step-independent form of the fractional step would need a term C does not have. The number: C3 run
with the fractional step (printed, F0) and the flag on against off bit for bit (F1).

**D6. Coupled faces: why not.** ADR 0245 amendment 3 P0 shows C bit-inert on periodic and rotational faces at the
operator level; the seam floor is ADR 0076's (R2b on SECT-M `0.578` with C, `0.538` without, against `[1.9, 2.3]`), made
by the seam terms of F-G that C leaves out (ADR 0245 Q4). Nothing here changes coupled faces; P1 checks that the
consistent split with C keeps them bit for bit.

**D7. Cost.** The cavity of C3 on 20 by 20 cells: ADR 0245 runs four such runs (2500 steps of `0.04` and `0.01` each
twice) in a gate-run test, so about a few milliseconds a step. K1, K2: four runs each plus one reference, about a
minute each. H1: four runs of 100 steps of 200 cells, seconds. B1, B2: 2000 and 8000 steps with the energy equation,
six runs; above two minutes, so ignored as a heavy record with H1 as the fast test of the same code path (the body-force
terms of C). F: two fractional runs and two more for F1. S1: three sliding steps of the 2112-cell pair, twice.

## Design (registered)

Library (`rcfd-fv`), all behind `rhie_chow_step_consistent` (default `false`):

- **L1** (`pimple.rs`, `step_with`): the gate becomes `s.rhie_chow_step_consistent` (no `!cf.enabled`, no
  `self.ale.is_none()`). With the gate on and `r_t = Some`, the face values `dr_f = gamma_t - gamma_p` (face by face) are
  passed to `consistent_split` and `consistent_non_orth` through a new `Option<&[f64]>` argument; `None` keeps the
  present `face_interpolate(rAtU - rAU)`, so with the gate off every statement is the present one (the call sites pass
  `None` unless the gate is on). The pressure-lag estimate (`lag_map`) is not changed (it is a linearised estimate;
  limit L4).
- **L2** (`pimple/face_consistent.rs`): `consistent_face_difference(gamma_t, gamma_p) -> Vec<f64>` (the face values of
  D1) and its unit test P1.
- **L3** (`pimple/moving.rs`, `step_sliding`): the flag is cleared for the step and restored afterwards (D2).
- **L4**: the documentation of the field says where C acts and where not.

Case tree (`rcfd-api`):

- **T** an optional table `[solver.rhie_chow]` (`Schema::optional`, as `solver.consistent_correctors`) with one key
  `step_consistent` (boolean, default `false`, `hash_omit_default`); the table itself `hash_omit_default`. `sim.rs`
  sets `s.rhie_chow_step_consistent = get_b(case, "solver.rhie_chow.step_consistent")` for the transient PISO/PIMPLE
  engine. Located errors at `solver.rhie_chow.step_consistent` when it is `true` with: a solver other than the
  incompressible transient `piso` / `pimple` (steady, VOF, compressible, variable density); `solver.time_advance =
  "fractional_step"` (D5: no effect there); lumped components (their face jumps: C changes the jump faces' coefficient,
  and no check covers it). By ADR 0266 read 6 an absent optional table stays out of every case's data, texts and hash, so
  no hash table, golden or existing TOML block changes; K0 checks that reading. `docs/case-schema.md` gets a section in
  prose (no TOML block). If K0 fails, the table is taken out, the library part stays, and the schema step is reported
  as the maintainer's.

## Set-ups (registered)

PS is ADR 0245's `settings(nu)`: `PimpleSettings::piso(nu)`, central convection, momentum and pressure solves to `1e-12`
relative, `1e-15` absolute, 3000 iterations; plus the time scheme and `n_outer` named. "C" is `rhie_chow_step_consistent
= true`; "CC" the consistent correctors (`consistent_correctors = true`, and `consistent_body_force = true` where a body
force acts). The new test file is `crates/rcfd-fv/tests/step_consistent_paths_f443.rs`; helpers are copied from the
files named (not edited).

- **CAV**: ADR 0245 C3: `cavity_2d(20, 1.0)`, `nu = 0.1`, lid `(1, 0, 0)`, walls at rest, PS, BDF2, `n_outer = 3`, from
  rest to `t = 20` in steps of `0.04` and `0.01`.
- **CAVT** (the translating cavity, `ale_gcl.rs` G3's frame): CAV's mesh, the points moved to `x + V t_n` with `V =
  (0.6, 0.2, 0)` every step through `move_mesh`, `set_follow_conditions` on `walls` (the returned wall gradient handed
  to `step_ale_with_walls`), the lid `FixedValue(V + (1, 0, 0))`, the walls and the initial field `V`; PS, implicit Euler
  (the moving step's only scheme), `n_outer = 3`, `t = 20` in steps of `0.04` and `0.01`; compared: `u - V`. **CAVE**:
  CAV with implicit Euler on the fixed mesh (`step`), `dt = 0.01`.
- **COL** (hydrostatic column): `box_2d(10, 20, 1.0, 2.0, false, false)`, the four sides fixed at `u = 0`, `nu = 0.1`,
  `BodyForce::uniform(g)` with `g = (0, -9.81, 0)`, PS, BDF2, `n_outer = 3`, `dt = 0.01`, 100 steps. COL-0: started from
  `u = 0`, `p = 0`. COL-H: started from `u = 0`, `p = g . x` (cell centres).
- **BUO** (buoyant cavity): `transient_buoyancy.rs`'s `cavity` set-up at Ra `1e3`, Pr `0.71`, on `box_2d_graded(20, 20,
  1, 1, Uniform)`, with the flow settings PS (`nu = 0.71`), BDF2, `n_outer = 3` (in place of that file's vanLeer, two
  correctors, AMG), the energy solve to `1e-12` relative, `1e-300` absolute, 5000 iterations, its convection
  `ConvScheme::Central`; from rest, `T = 0.5`, to `t = 8` in steps of `0.004` and `0.001`.
- **FR** (fractional step): CAV with `fractional = Some(FractionalSettings::default())` and implicit Euler.
- **SL** (sliding): `sliding_interface_f250.rs`'s `pair(120)` with `tc_solver(.., nu = 1, nonorth = 2)` (copied), three
  sliding steps of `dt = 0.005` at `omega = 1`, no wall conditions beyond the initial ones.

## Registered checks

### A. Acceptance

| Id | Clause | Check | Limit |
| --- | --- | --- | --- |
| K1 | a1, a7 | CAV with C and CC: the largest difference of the cell velocities, `dt = 0.04` against `0.01` | `<= 1e-9` |
| K1n (negative control) | | CAV with CC, without C: the same | `>= 1e-6` |
| K1c | a1 | CAV with C and CC at `dt = 0.01` against CAV with C (default correctors) at `dt = 0.01` (both converged steady states are C's, D1) | `<= 1e-9` |
| K1d | | continuity, every step of the K1 runs | `<= 1e-9` |
| K2 | a2, a7 | CAVT with C: the largest difference of `u - V`, `dt = 0.04` against `0.01` | `<= 1e-9` |
| K2n (negative control) | | CAVT without C: the same | `>= 1e-6` |
| K2c | a2 | CAVT with C at `0.01` (`u - V`) against CAVE with C at `0.01` | `<= 1e-9` |
| K2d | | continuity, every step of the K2 runs | `<= 1e-9` |
| H1a | a4 | COL-0 with C: the largest cell speed after step 100 | `<= 1e-10` |
| H1b | a4 | COL-H with C: the largest cell speed over every step | `<= 1e-10` |
| H1c | a4, a1 | COL-0 and COL-H with C and CC (`consistent_body_force`): the two numbers of H1a and H1b | `<= 1e-10` each |
| B1 | a5, a7 | BUO with C: the largest difference of the cell velocities, `dt = 0.004` against `0.001`, over the largest speed of the `0.001` run | `<= 1e-9` |
| B1n (negative control) | | BUO without C: the same | `>= 1e-6` |
| B2 | a5, a1 | BUO with C and CC (`consistent_body_force`): as B1 | `<= 1e-9` |
| B3 | | continuity, every step of the B1 and B2 runs | `<= 1e-9` |
| E0 | a3 | with the flag off, unedited: the non-ignored tests of `rcfd-fv` `--lib`, `bdf2_remainder_f344`, `seam_no_harm_f344` (non-ignored), `w33b_default_bits`, `w33b_transient_order`, `w34b_lag`, `consistent_correctors_f301`, `consistent_defaults_f301`, `consistent_value_f322`, `consistent_extensions_f322`, `w48v_corrector_control`, `piso_thin_gap_f299`, `ale_gcl`, `ale_flows`, `sliding_interface_f250`, `fractional_step`, `transient_buoyancy`, `overset_f338`, `overset_f445` (ADR 0347), `overset_f134`; `rcfd-api`: `baseline_hashes`, `case_hashes_w48u`, `w48v_case_hashes`, `consistent_defaults_case_f301`, `consistent_correctors_case_f301` (release) | pass |
| T1 | a6 | a case (the committed `cylinder-re100.toml` example, PISO) with `solver.rhie_chow.step_consistent = true`: the built transient solver's settings have the flag set and are otherwise those of the case without the key (their `Debug` text with the flag reset is equal); after 16 steps the fields `p`, `ux`, `uy` differ from the run without the key | equal; differ |
| T2 | a6 | located errors at `solver.rhie_chow.step_consistent` for the steady quick-start cavity, the cylinder with `fractional_step`, the low-Mach box (`LOW_MACH`) and the lumped channel (`LUMPED`) of `consistent_correctors_case_f301.rs` (each builds without the key) | an error at that path, each |
| T3 | a6 | the cylinder case without the table and with `step_consistent = false`: equal `content_hash`; with `true` a different hash; the written TOML and JSON of the case without the table do not name `rhie_chow` | equal; differ; not named |
| T4 | a6 | the tree's dynamic mesh (`PISTON` of `consistent_correctors_case_f301.rs`) with the key: builds, 10 steps without a stop, fields differ from the run without | each |
| K0 | a6, a3 | with the table in the schema, unedited: `rcfd-api` `baseline_hashes`, `case_hashes_w48u`, `w48v_case_hashes`, `physics_schema`, `rules_w19c`, `consistent_correctors_case_f301`, `consistent_defaults_case_f301`; `rcfd-mcp` `default_f1` | pass |

### B. The paths not taken (a7: "names why not with a number")

| Id | Check | Limit |
| --- | --- | --- |
| F1 | FR, `dt = 0.04`, 100 steps, with the flag against without: `u`, `p`, `phi` | bit for bit |
| F0 | FR with `dt = 0.04` and `0.01` to `t = 20`: the largest difference of the cell velocities (the fractional step's own step dependence, D5) | printed, no limit |
| S1 | SL with the flag against without: `u`, `p`, `phi` after each of the three steps; the flag is set again after the steps | bit for bit; set |
| P1 | unit test in `face_consistent.rs`, on `box_2d(8, 8, 1, 1)` periodic both ways and on `annulus_sector(8, 8, 0.5, 1, pi/3, Uniform, 0.1)` (ADR 0245 P0's meshes) with varying positive `rAU`, `rAtU = 1.3 rAU (1 + 0.1 sin c)`: `consistent_face_difference(interp(rAtU), harmonic(rAU))` against `face_interpolate(rAtU - rAU)` on every boundary face, and `coupled_gamma` of both on every coupled face | bit for bit |
| P1n (negative control) | the same on the internal faces | differ by `>= 1e-6` relative on some face |

Printed for the record, no limit: the change of the velocity over the last step of every steady run; B1's and B2's
absolute differences and the temperature difference; COL's numbers without C; K2 and CAVT with CC and C together.

### Mutations (scratch copy `w49i-work/f443-scratch`, not committed)

Each applied alone to a `git archive` copy of the final tree by an exact replacement that must occur once; the named
test is run once and must fail; the file is restored and compared byte for byte.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MM1 | `step_with`: the override is not passed (`None` in place of the face differences) | K1 |
| MM2 | the gate keeps `!cf.enabled` | K1 |
| MM3 | the gate keeps `self.ale.is_none()` | K2 |
| MM4 | `add_lagged_gradient`: the body term not subtracted (`x = G p_lag`) | H1b |
| MM5 | the body-force face term of internal faces takes `face_interpolate(rAU)` in place of `gamma_p` when C is on | H1b |
| MM6 | `step_sliding`: the flag is not cleared | S1 |
| MM7 | `sim.rs`: the key is not read (the assignment removed) | T1 |
| MM8 | `consistent_face_difference`: boundary faces take `0` | P1 |

`scripts/limit_audit.py` is run on the new test files and every hit is read.

## Decision rule

- **done**: K1, K1c, K2, K2c, H1a, H1b, H1c, B1, B2 and the continuity rows met; T1 to T4 and K0 met; E0 passes;
  F1, S1, P1 met; the negative controls held; every mutation failed its check.
- **partial**: the library part met as above but the key could not be added without a hash-table or golden edit (then
  that step is the maintainer's), or a check of A missed with a diagnosed cause (a `*_known_miss`), or a mutation was
  not caught, or E0 could not be shown in full.
- **todo**: C does not act on the consistent correctors or on `step_ale_with_walls` with a met K1 or K2.

A negative control that does not hold makes its check void (not met), and the item is partial.

## Open choices taken

Each taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation); none is a decision the
maintainer made in person; none changes a default.

- **O1. `gamma_t` with C.** (a) keep `interp(rAtU)` and pass `gamma_t - Gamma` to the split; (b) take
  `harmonic(rAtU)`. Taken (a): the converged state is C's either way (D1); (a) leaves ADR 0214's corrector iteration as
  it is.
- **O2. `step_sliding`.** (a) keep the default step there (cleared and restored, S1); (b) let C act unchecked. Taken
  (a).
- **O3. The key's form.** (a) an optional table `[solver.rhie_chow] step_consistent`; (b) a plain key in `solver`
  (needs `DEFAULT_KEYS` of H1, ADR 0266 read 6). Taken (a).
- **O4. The key with lumped components.** (a) a located error; (b) accept. Taken (a): no check covers C on jump faces.

## Limits of this registration

- L1. Uniform Cartesian meshes, one viscosity per set-up, central convection, fixed-velocity walls, implicit Euler on the
  moving mesh (the only scheme it has), a rigid translation (no deforming cells, no wall acceleration beyond rounding).
- L2. Coupled faces keep ADR 0076's seam terms (D6); the fractional step and `step_sliding` are not changed (D5, D2).
- L3. The overset step is not touched; ADR 0347's composition with C stands as measured there.
- L4. The pressure-lag estimate and the adaptive corrector count read the default face coefficient (ADR 0245 left them
  so; with C their estimate is of a slightly different operator).
- L5. Face jumps with C (library) are as F-344 left them; the key refuses them (O4).
- L6. B1/B2 assume BUO is steady at `t = 8` to the measure; if it is not, the miss is diagnosed by the printed change
  over the last step, not by a longer run.

## Open questions

Listed with options and a recommendation in the addendum, after the results.

## Addendum 1: results (2026-11-12)

Measured against the limits registered above (`1f9f393f`); none was changed. The code and the tests were committed compiled and
not run (`db22e388`); every registered run was then made once, release, Windows, this worktree's target directory (K1, K2, B1 and
B2 ran as four processes at the same time; B1 and B2 are `#[ignore]` heavy records and were started with `--ignored`). Logs in
`E:/projects/cfd/w49i-work/f443-scratch` (`p1.log`, `h1_.log`, `f1_.log`, `s1_.log`, `k1_.log`, `k2_.log`, `b1_.log`, `b2_.log`,
`t.log`, `k0-api.log`, `k0-mcp.log`, `e0.log`, `f443-MM*.log`, `mut-summary.log`).

**Said plainly.** C now acts with the consistent correctors and on `step_ale` / `step_ale_with_walls`, and with a body force; on
each of these paths the steady solution is the same for two step sizes to `1e-13` to `5e-13` (relative `2e-14` to `4e-14` for
the buoyant cavity), where the same runs without C differ by `3.1e-4` to `3.75e-4` (relative `6.0e-5` in the buoyant cavity). A
hydrostatic column stays at rest to `2e-14` and, started from zero pressure, reaches rest to `1.6e-12`. The case tree has the key
`[solver.rhie_chow] step_consistent`, an optional table: no hash table, golden or existing case text moved. Every check met,
every control held, every mutation caught; the defaults are bit for bit.

### What was built

- `crates/rcfd-fv/src/pimple.rs` (final numbering): 139 to 143 (the field's documentation); 1672 to 1674 (the gate is
  `s.rhie_chow_step_consistent`, in place of main's line 1671); 1744 to 1754 (the face differences `dr_c` with C and the
  consistent correctors, passed to `consistent_split` and `consistent_non_orth`, in place of main's lines 1742 and 1745); 2073
  and 2076 (the pressure-lag map passes `None`); 2416 to 2438 and 2482 to 2501 (the `Option<&[f64]>` argument `dr_face` of
  `consistent_split` and `consistent_non_orth`; `None` computes `face_interpolate(rAtU - rAU)` as before).
- `crates/rcfd-fv/src/pimple/face_consistent.rs`: `consistent_face_difference` (68 to 74), the unit test P1 (258 to 298).
- `crates/rcfd-fv/src/pimple/moving.rs`: `step_sliding` clears the flag for its step and restores it (241 to 245).
- `crates/rcfd-api/src/schema.rs` 1071 to 1084 (the optional table `solver.rhie_chow`); `sim.rs` 2008 to 2009 (the key read
  into the transient settings), 2367 to 2381 (its located errors). `docs/case-schema.md`: a prose section.
- Tests: `crates/rcfd-fv/tests/step_consistent_paths_f443.rs` (K1, K2, H1, F1, S1 gate-run; B1, B2 ignored, heavy);
  `crates/rcfd-api/tests/rhie_chow_case_f443.rs` (T1 to T4). No existing test, ignored record, hash table or golden was edited;
  `overset.rs` is not touched.

### Checks

| Check | Limit | Measured | Verdict |
| --- | --- | --- | --- |
| **K1** CAV with C and CC, `0.04` against `0.01` | `<= 1e-9` | `4.67e-13` (change over the last step `1.1e-16`, `9.5e-14`) | met |
| K1n (control) CAV with CC without C | `>= 1e-6` | `3.750e-4` | held |
| K1c C with CC against C alone, `dt = 0.01` | `<= 1e-9` | `9.24e-14` | met |
| K1d continuity | `<= 1e-9` | `2.76e-12` | met |
| **K2** CAVT with C (`step_ale_with_walls`), `0.04` against `0.01`, `u - V` | `<= 1e-9` | `2.20e-13` (change `2.2e-13`, `4.8e-13`) | met |
| K2n (control) CAVT without C | `>= 1e-6` | `3.062e-4` | held |
| K2c CAVT against CAVE, both C, Euler, `0.01` | `<= 1e-9` | `3.45e-13` | met |
| K2d continuity | `<= 1e-9` | `4.73e-12` | met |
| **H1a** COL-0 with C, speed after step 100 | `<= 1e-10` | `1.63e-12` | met |
| **H1b** COL-H with C, speed over every step | `<= 1e-10` | `2.10e-14` | met |
| **H1c** COL-0, COL-H with C and CC | `<= 1e-10` each | `2.80e-13`; `2.09e-14` | met |
| **B1** BUO with C, `0.004` against `0.001`, relative | `<= 1e-9` | `2.06e-14` (absolute `7.7e-14`, largest speed `3.7095`, temperature `5.3e-15`, change `4.4e-16`, `2.9e-14`) | met |
| B1n (control) BUO without C | `>= 1e-6` | `5.955e-5` (absolute `2.21e-4`, temperature `1.98e-7`) | held |
| **B2** BUO with C and CC | `<= 1e-9` | `3.96e-14` (absolute `1.47e-13`) | met |
| B3 continuity of B1, B2 | `<= 1e-9` | `2.88e-12`; `3.16e-12` | met |
| F1 fractional step, flag on against off, 100 steps | bit for bit | equal | met |
| F0 (printed) fractional step's steady cavity, `0.04` against `0.01` | none | `8.69e-2` | printed |
| S1 `step_sliding` flag on against off, three steps; flag after | bit for bit; set | equal each step; set | met |
| P1 periodic box (32 coupled faces), sector (16) | bit for bit | boundary and coupled values equal | met |
| P1n internal faces | `>= 1e-6` relative | `0.524`; `0.530` | held |
| T1 the key sets the option, nothing else; acts | equal; differ | flag set, other settings equal; `p`, `ux`, `uy` differ after 16 steps | met |
| T2 located errors (steady, fractional step, low Mach, lumped) | an error at the key | at `solver.rhie_chow.step_consistent`, each | met |
| T3 hash, text | equal; differ; not named | `6ac54a8827aedab0` without and with `false`, `9d1cd84220b96c9b` with `true`; TOML and JSON do not name it | met |
| T4 dynamic mesh with the key | builds, runs, differs | yes | met |
| K0 `rcfd-api` `baseline_hashes` 3 (1 ignored), `case_hashes_w48u` 1, `w48v_case_hashes` (its one test is ignored), `physics_schema` 11, `rules_w19c` 8 (1 ignored), `consistent_correctors_case_f301` 4 (8 ignored), `consistent_defaults_case_f301` 3; `rcfd-mcp` `default_f1` 2 | pass | all passed, no file edited | met |
| E0 `rcfd-fv` (non-ignored): `--lib` 104 (6 ignored; P1, G1, P0 among them), `ale_flows` 5, `ale_gcl` 4, `bdf2_remainder_f344` 4, `consistent_correctors_f301` 12, `consistent_defaults_f301` 6, `consistent_extensions_f322` 8, `consistent_value_f322` 5, `fractional_step` 13, `overset_f134` 12, `overset_f338` 9, `overset_f445` 9, `piso_thin_gap_f299` 18, `seam_no_harm_f344` 6, `sliding_interface_f250` 14, `transient_buoyancy` 3, `w33b_default_bits` 1, `w33b_transient_order` 6, `w34b_lag` 3, `w48v_corrector_control` 5 | pass | all passed | met |

Printed: COL without C: `1.87e-12` after step 100 from `p = 0`, `2.25e-14` over every step from the hydrostatic pressure (the
default keeps the column at rest too, as D3 says). CAVT with C and CC: `7.3e-13` between the step sizes, `1.1e-13` against C
alone. Run times (four processes at once): K1 62 s, K2 83 s, B1 137 s, B2 88 s; H1, F1, S1 under 3 s.

**Why not, with the numbers (a7).** The fractional step: C would change nothing there (its face coefficient is uniform; F1 bit
for bit), and its own steady cavity differs by `8.69e-2` between `dt = 0.04` and `0.01`: a defect `dt R(p)` of first order and no
carry-over, a different term from the one C removes (D5). Coupled faces: P1 shows the consistent compensation with C bit for bit
the default's on every periodic and rotational face; the seam floor stays ADR 0076's (R2b `0.578` with C, ADR 0245 addendum 2).
`step_sliding`: keeps the default step (S1); the remapped old defect on faces that change is not covered by D2's derivation.

### Mutations

Applied alone to a `git archive` copy of `db22e388` (`f443-scratch/mut`, its own target directory) by `f443-mut-run.py` (an exact
replacement that occurs once), the named test run once, the file restored and compared byte for byte.

| Id | Must fail | Result |
| --- | --- | --- |
| MM1 the face differences not passed (`None`) | K1 | failed: K1 `3.81e-4`, K1c `3.65e-4` |
| MM2 the gate keeps `!cf.enabled` | K1 | failed: K1 `3.75e-4`, K1c `8.24e-4` |
| MM3 the gate keeps `self.ale.is_none()` | K2 | failed: K2 `3.06e-4`, K2c `9.56e-4` |
| MM4 the body term not subtracted in `add_lagged_gradient` | H1b | failed: `9.59e-5` (H1a, H1c the same) |
| MM5 the internal body-force face term with `face_interpolate(rAU)` | H1b | failed: `9.59e-5` |
| MM6 `step_sliding` does not clear the flag | S1 | failed: not bit for bit |
| MM7 `sim.rs` does not read the key | T1 | failed: the flag is not set |
| MM8 `consistent_face_difference` gives 0 on boundary faces | P1 | failed |

Deviation in MM8: the function sees only face arrays, so the mutation zeroes the last quarter of the faces (on P1's meshes
these are boundary faces: empty and coupled) instead of "boundary faces" exactly. MM1 is larger than D1 estimated (`3.8e-4`
against a guessed `1e-6` to `1e-5`): with the linear coefficient in the converged pressure term the steady defect carries the
whole ratio `interp(rAU) / Gamma`, which is the default's dependence again. MM4 and MM5 give the same number: both leave
`(Gamma - interp(rAU)) b . S` on the internal faces, with opposite signs. `scripts/limit_audit.py` on the two new test files:
0 hits.

### Deviations and disclosures

- The rcfd-fv tests of E0 ran in one `cargo test` process with the default test threads while the mutation runs built and ran in
  the scratch copy; nothing was re-run.
- B1 and B2 are ignored heavy records (registered); H1 is the fast test of the body-force path of C and K1 of the consistent path.
- D3's guess of four steps to rest from `p = 0` was not measured; only the registered number after step 100 was.

### Not run

The whole workspace suite, the gate, Linux; `rcfd-cli`'s tests; the ignored tests of every file named; the immersed-solid file
(`immersed_solid_f219`: C is not reached there with the flag off, and no line it runs changed except the gate, whose conjunction
with the flag off is false as before).

### After the tests

`cargo fmt -p rcfd-fv --check`, `cargo fmt -p rcfd-api --check`: clean. `cargo clippy -p rcfd-fv -p rcfd-api --all-targets --
-D warnings`: clean.

### Decision by the registered rule

**Done.** K1, K1c, K2, K2c, H1a to H1c, B1, B2 and the continuity rows met; T1 to T4 and K0 met; E0 passed; F1, S1, P1 met; the
controls held; all eight mutations failed their checks. The key needed no hash-table or golden edit (ADR 0266 read 6 confirmed
by K0 and T3).

### Open questions (each with options and a recommendation)

- **Q1. C by default** (ADR 0245 Q5, unchanged): it would change every transient result. Open for the maintainer; not taken.
  What this item adds: C is now defined on every path the case tree's transient incompressible runs take except lumped
  components.
- **Q2. The fractional step's own step dependence** (`8.69e-2` in the steady cavity between `dt = 0.04` and `0.01`). (a) a
  follow-up item that registers a step-consistent face flux for the projection (a carried defect, as the PISO step has); (b)
  leave it: the fractional step is a non-iterative step for transients, not for steady states. Recommendation (a), proposed
  below; the number is large against the PISO step's `3.7e-4`.
- **Q3. `step_sliding` with C.** (a) a follow-up when a sliding case needs a step-independent steady state, with a C3 check
  on the sliding pair; (b) not now. Recommendation (b).
- **Q4. The key with lumped components (O4).** (a) keep the located error; (b) derive and check C on jump faces. Recommendation
  (a) until a lumped case needs it.
- **Q5. The pressure-lag estimate with C** (L4): it reads the default face coefficient. (a) leave; (b) pass the coefficient.
  Recommendation (a): the estimate is a stability guide, and C changes the coefficient by the ratio of a harmonic to a linear
  mean (at most `0.53` relative on P1's strongly varying test field, about `1e-4` on the meshes run here).

Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation), not decisions the maintainer made in
person: O1 to O4 of the registration.

### For the lead: roadmap entries

No roadmap file was edited. **F-443**, proposed status: `done`. Proposed note:

> From ADR 0245 question 3, recommendation (a), taken on the maintainer's standing instruction of 2026-11-11. ADR 0352, 2026-11-12: DONE. PimpleSettings::rhie_chow_step_consistent (ADR 0245's harmonic face coefficient and lagged-gradient commutator) now acts with the consistent correctors (their compensation takes interp(rAtU) - Gamma face by face, so a converged step is C's) and on step_ale / step_ale_with_walls (the gate's moving-mesh and consistent exclusions dropped); step_sliding keeps the default step (bit for bit) and the fractional step is unchanged (its face coefficient is uniform, so C would change nothing; bit for bit). Defaults bit for bit (the rcfd-fv suites touching the changed lines and baseline_hashes, case_hashes_w48u unedited). C3 of ADR 0245 on each path: consistent correctors 4.7e-13 between dt 0.04 and 0.01 (3.75e-4 without C), translating cavity through step_ale_with_walls 2.2e-13 (3.06e-4 without), buoyant cavity at Ra 1e3 2.1e-14 relative between dt 0.004 and 0.001 (6.0e-5 without), 4.0e-14 with the consistent correctors too; a hydrostatic column stays at rest to 2.1e-14 and reaches rest from zero pressure to 1.6e-12 (limit 1e-10). Why not on the other paths: the fractional step's own steady cavity differs by 8.7e-2 between the two steps (a first-order defect dt R(p) without a carry-over, not C's term); coupled faces bit-inert (P1), seam floor of ADR 0076 unchanged. Case key: the optional table [solver.rhie_chow] step_consistent (default false; absent from every case's text and hash), located errors with steady, compressible, variable-density and VOF runs, the fractional step and lumped components; acts on the tree's dynamic mesh. Eight mutations caught. Follow-up proposed for the fractional step (ADR 0352 Q2).

**F-344**: in its note, the sentence "The opt-in does not act with the consistent correctors, on the library's moving-mesh steps, in the fractional step, on coupled faces; no case-tree key." of addendum 1 (if still present) should read:

> Since ADR 0352 (F-443) the opt-in also acts with the consistent correctors and on step_ale / step_ale_with_walls and has the case key [solver.rhie_chow] step_consistent; step_sliding, the fractional step and coupled faces keep the default.

Proposed follow-up item (the number is the lead's):

- **Fractional step: a step-consistent face flux for the projection.** Depends on F-443. Acceptance:

> The steady lid-driven cavity of ADR 0245 C3 with solver.time_advance = "fractional_step" (8.7e-2 between dt = 0.04 and 0.01, ADR 0352 F0) is traced to its face term with a number, and either made the same to 1e-9 for the two step sizes by an opt-in (defaults bit-identical) or the ADR names why not with a number.

  Note:

> From ADR 0352 question 2, recommendation (a); proposed by the F-443 addendum, not yet taken. The fractional step's face flux interp(u*).S + dt (interp(grad p^n).S - snGrad(p^n)|S|) has no carried defect, so its steady face defect is dt R(p).
