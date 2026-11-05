# ADR 0192: F-253 registration: the particle wall film (Nusselt thickness, mass conservation)

Status: **REGISTERED 2026-11-05**, written before any code of F-253. Not yet accepted by the maintainer. The only code
run before this text was the two record tests of check D (R1, R2): they use the code of main as it is and were run on it
to make the records below. Results go in the addendum at the end.

Parents: [ADR 0028](0028-wave18c-particles.md) (F-056, the tracker), [ADR 0057](0057-wave29c-lagrangian-extensions.md)
(F-128, the extensions), [ADR 0059](0059-wave30a-wiring-particles-mixing-plane.md) (two-way coupling in the solver loops),
[ADR 0155](0155-job48u-rotate-the-mesh-instead-of-the-flow.md) (optional objects in the case tree, the record R0 of
`case_hashes_w48u.rs`), [ADR 0156](0156-job48v-adaptive-pressure-correctors.md) D6 (`hash_omit_default`),
[ADR 0137](0137-linux-gate-platform-values.md) (platform values). Roadmap item F-253,
split from F-245 by the user on 2026-10-22.

Read at main 201ea17. Number: 0188 is the highest on main; 0189 is on the branch `f274-design-evals`; 0190 and 0191 are
reserved for F-256 and F-259. No branch holds a 0192.

House rules ([ADR 0130](0130-wave48a-overview-and-run-record-contract.md), ADR 0028):

- limits are fixed here, before the code and the runs;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact, and the cause is diagnosed, not tuned away;
- a defective set-up gets a new corrected test, and the old one stays as an ignored record;
- every new code path is opt-in, and runs without the film are unchanged bit for bit;
- a new case key uses `hash_omit_default`, and `case_hashes_w48u.rs` H1 stays green;
- neither `crates/rcfd-api/tests/baseline_hashes.rs` nor the record R0 is re-recorded;
- no new third-party crate, no commit trailers.

Clean-room: the film model is derived below from the momentum balance of a thin liquid layer. The two published
thresholds it uses carry the status words of ADR 0028: "verified (secondary)" means found with the same content in
published secondary sources read during this work; "derived" means derived here. No vendor code, text or install was used.

## What F-253 asks

The roadmap entry: a laminar film fed by impinging parcels on an inclined wall reaches the Nusselt thickness derived in the
ADR, `delta = (3 mu Gamma / (rho^2 g sin(theta)))^(1/3)` with `Gamma` the mass flow per unit width, within 5 percent at a
station away from the feed, at a film Reynolds number inside the laminar range registered in the ADR; film + airborne +
escaped parcel mass is conserved to 1e-10 relative. Out of scope: comparison with measured data, particle reactions,
discrete-element coupling.

Dependencies. F-128 is done. F-056 is partial only because it has no cyclone separation case. F-253 uses no cyclone, so
this text reads the partial F-056 as no block. That is a reading of the dependency, not a decision.

## What the code read shows (main 201ea17, nothing run)

1. **Wall actions.** `rcfd_particles::tracker` has `WallAction::{Reflect, Stick, Escape}`. A parcel that reaches a `Stick`
   face becomes `State::Deposited` and keeps its mass and position; nothing else records the hit. Actions are per boundary
   face, set from the patch kind and the per-patch overrides in `Tracker::new`.
2. **Extensions.** Dispersion, heat transfer and the exchange record are switched on by `Tracker::set_dispersion`,
   `set_thermal` and `enable_coupling`, and live beside the parcels (`Aux`, `Ctx`). ADR 0057 kept the struct literals of
   `Particle`, `Flow`, `Options` and `InjectorSpec` valid on purpose, and tests build them by literal.
3. **States.** `State` has four values (0 to 3). `rcfd_api::particle_run` names them in an exhaustive `match`
   (`state_name`) and sums the mass by state into a four-entry array indexed by `state as usize` (`report`). The VTP writer
   prints `state as i32`.
4. **Gravity.** `rcfd_api::particles::options` passes `physics.gravity` to the tracker when `particles.gravity` is true and
   a zero vector otherwise.
5. **The case tree.** An optional object (`Schema::optional`, ADR 0155) stays out of the data until a value inside it is
   set, so adding one changes no case's data, written text or settings hash. `hash_omit_default` also leaves it out of the
   hash when it is written with its defaults. `case_hashes_w48u.rs` H1 compares every case file and every TOML block of
   `docs/` with R0; a TOML block is keyed by the FNV hash of its text.
6. **The mesh.** Boundary face area vectors point out of the domain. `box_mesh` makes six wall patches.
   `rcfd_mesh::edit::Affine::rotation` is a right-handed rotation, and `transform_mesh` applies it.

## The model

**M1. The film profile (derived).** A layer of liquid of density `rho_l`, dynamic viscosity `mu_l` and thickness `h` lies on
a plane wall with unit normal `n` (out of the fluid domain, into the wall). Gravity `g` has the tangential part
`g_t = g - (g . n) n`. For steady, fully developed laminar flow with no slip at the wall (`y = 0`) and no shear at the free
surface (`y = h`, gas at rest), the momentum balance along `g_t` is `mu_l u'' = -rho_l |g_t|`, so

    u(y) = (rho_l |g_t| / mu_l) (h y - y^2/2)
    u_bar = (1/h) int_0^h u dy = rho_l |g_t| h^2 / (3 mu_l)
    Gamma = rho_l u_bar h = rho_l^2 |g_t| h^3 / (3 mu_l)
    h_N(Gamma) = (3 mu_l Gamma / (rho_l^2 |g_t|))^(1/3)

On a plane inclined at `theta` to the horizontal, `|g_t| = g sin(theta)`, which gives the roadmap formula. The
hydrostatic pressure of the gas would replace `rho_l |g_t|` by `(rho_l - rho_g) |g_t|` in the momentum balance and thicken
the film by the factor `(1 - rho_g/rho_l)^(-1/3)` (4.0e-4 for water in air at 1.2 kg/m^3). It is left out, so that the
model's steady state is the roadmap formula exactly; the parcels keep their buoyancy (ADR 0028 D2). Open question Q2.

**M2. Lubrication (derived).** Where the thickness changes slowly along the wall, each face of the film moves with the
depth-averaged velocity of M1 at its own thickness, in the direction of its own `g_t`:

    u_bar_f = rho_l h_f^2 g_t,f / (3 mu_l)

Left out: film inertia (and with it the waves of a real film), the gradient of the hydrostatic pressure of the normal
gravity component, surface tension, shear and pressure gradients from the gas. All of these vanish where the thickness is
uniform and the gas is at rest, so the steady film far from the feed is M1's.

**M3. Film variables.** The film lives on the faces of the film patches, which are wall patches named in the settings.
Face `f` has the film mass `M_f >= 0`, its area `A_f`, its unit normal `n_f` from the area vector, its centre, and the
thickness `h_f = M_f / (rho_l A_f)`. The film liquid is the parcel liquid: `rho_l` is the parcel density. Mass balance
of a face: `dM_f/dt = (inflow) - (outflow) + S_f`, with `S_f` the mass of the parcels it absorbs.

**M4. Edges.** An edge of a film face is a pair of consecutive points of its point list. An edge held by two film faces
is an inner edge; an edge held by one is an open edge, the border of the film; an edge held by more than two is an error
(a located error in the case tree). For face `f` and its edge `e` from point `P` to point `Q`: the length is
`L_e = |Q - P|`, and the in-plane unit normal is `m_fe = (Q - P) x n_f / |(Q - P) x n_f|`, turned to point away from the
face centre.

**M5. Donor-cell flux.** The mass that leaves face `f` through edge `e` per unit time is

    Phi_fe = (M_f / A_f) L_e max(0, u_bar_f . m_fe) = rho_l h_f L_e max(0, u_bar_f . m_fe)

Across an inner edge it goes to the other face. Across an open edge it leaves the film and is booked as drained. Nothing
enters through an open edge. Each transfer is one number, debited from one face and credited to the other face or to the
drained total, so mass is conserved to round-off.

**M6. Time stepping.** Explicit Euler on the face masses. In a sub-step of length `tau`, every transfer is
`T_fe = Phi_fe tau`, computed from the masses at the start of the sub-step; then all transfers are applied in face order.
With the outflow rate `r_f = sum_e L_e max(0, u_bar_f . m_fe) / A_f` [1/s], the sub-step is

    tau = min(rest of the step, C_film / (3 max_f r_f)),   C_film = 0.5

and it is the whole rest when that rest is within 1e-12 of it (as the tracker's sub-steps, ADR 0028 D3), or when no face
has an outflow. The factor 3 is the kinematic wave speed: `Phi` grows as `M^3`, so `dPhi/dM = 3 Phi/M`. With it the
Courant number of the wave is at most `C_film`, and a face loses at most `C_film/3 = 1/6` of its mass in a sub-step, so
no mass becomes negative. The film is advanced once per `Tracker::advance`, over the whole step, after the parcels and
the injections of that step.

**M7. What the steady state shows (derived).** At a steady state, the outflow of every face equals its inflow plus its
source. Down a uniform incline, a face below the feed with no source then carries `Gamma` per unit width, so its
thickness is `h_N(Gamma)` exactly, on any mesh and for any time step. The checks below therefore verify the chain
(parcel, absorption, transport, approach to the steady state) and the conservation, not the closure M1; the closure is
checked by A1 and the residual A2. The feed face receives the mass of its parcels before its transfer of the step is
computed, so at the end of a step it holds `M_N - S dt` (an offset of order `dt`); only faces below the feed are compared.

**M8. Absorption.** A parcel that reaches a face of a film patch is absorbed in the same `advance`: its mass
`n rho_p pi d^3 / 6` is added to that face's `M_f` at the hit, its state becomes `State::Film` (4), it keeps the hit
position and is not tracked again. No splash, rebound or breakup. The parcel's momentum and heat do not enter the film
(the closure M2 is algebraic). The film never acts on the carrier and never moves a parcel.

**M9. Film Reynolds number and the laminar range.** `Re_f = 4 Gamma / mu_l`, with `Gamma = rho_l h |u_bar|` the local mass
flow per unit width (the falling-film and film-condensation definition).

- The registered laminar range is `0 < Re_f <= 30`, the laminar wave-free film. Status: verified (secondary). Two
  university lecture notes on film condensation, found by a web search on 2026-11-05, give `Re = 4 Gamma / mu_l` with the
  laminar wave-free film below about 30, wavy laminar from about 30 to 1800 and turbulent above. The original texts they
  draw on were not accessed. The model is not checked outside this range and does not refuse it; the run report gives the
  largest face `Re_f`.
- A flat film on a plane inclined at `theta` is linearly unstable to long waves above a threshold. Status: verified
  (secondary). Thompson, Tseluiko and Papageorgiou, "Falling liquid films with blowing and suction", J. Fluid Mech. 2015
  (arXiv:1506.01579), state `R > R_c = (5/4) cot(theta)` with `R = rho h U_s / mu` and the Nusselt surface speed
  `U_s = rho g h^2 sin(theta) / (2 mu)`. A second search result gives the same threshold as `(5/6) cot(theta)` for a
  Reynolds number on the mean velocity. The two agree because `U_s = (3/2) u_bar`. The originals (Benjamin 1957, Yih 1963)
  were not accessed.
- In `Re_f`: `R = (3/2) Gamma / mu`, so `Re_f = (8/3) R`, and the threshold is `Re_f,c = (10/3) cot(theta)`, 5.774 at
  `theta = 30` degrees.
- The acceptance case runs at `Re_f = 2`: inside the laminar range and below 5.774, so the real film it stands for is
  linearly stable. The model has no inertia and cannot show the instability in any case.

## Settings and code

**S1. Library (`rcfd-particles`).** A new module `film`:

- `FilmSpec {patches, density, viscosity, gravity}`;
- `Film`, with the face masses, the drained and absorbed totals, the smallest face mass seen in any sub-step, and read
  access to faces, areas, centres, normals, thickness, velocity and `Re_f`;
- the closed forms `film_velocity` (M2) and `nusselt_thickness` (M1).

`Tracker::set_film(spec)` builds the film and marks its faces `WallAction::Film`. It refuses an unknown patch, a patch that
is not a wall, a patch listed twice and an edge held by more than two film faces. `Tracker::film()` and `film_mut()` give
access. `State::Film = 4` is added. `Tracker::fingerprint` appends the face masses and the drained total only when a film
is set. No field is added to `Particle`, `Flow`, `Options` or `InjectorSpec`.

**S2. Case tree.** The optional object `particles.film` (`.optional().hash_omit_default()`), absent from the data until
set:

| Key | Default | Meaning |
| --- | --- | --- |
| `enabled` | `false` | parcels that reach a film patch join a wall film |
| `patches` | `[]` | names of the wall patches that carry the film |
| `viscosity` | `-1` | dynamic viscosity of the film liquid [Pa s], must be set (> 0) |

The film liquid density is `particles.density`, and the film gravity is the tracker's gravity. Located errors, checked
without the mesh:

- `particles.film.enabled` when `particles.enabled` is false, or with `particles.two_way` or `particles.heat_transfer`
  (the booking of absorbed parcels in the exchange is not defined; not delivered);
- `particles.film.patches` empty, a name listed twice, or a patch that also has a `particles.walls.<patch>` entry;
- `particles.film.viscosity` not above 0;
- `particles.gravity` false, or `physics.gravity` zero (the film is driven by gravity alone).

Checked with the mesh, by `build_tracker`: `particles.film.patches` naming an unknown patch, a patch that is not a wall,
or a non-manifold patch set. The keys of `walls.<patch>.interaction` and `wall_interaction` keep their values; no value is
added to them.

**S3. Run path.** Only when the film is on: `particle_report.json` gains `counts.film`, `mass_by_state.film` and an object
`film` {`faces`, `held`, `drained`, `absorbed`, `max_thickness`, `max_reynolds`}, and the run writes
`particle_film.csv` with one row per film face (`face,patch,x,y,z,area,thickness,mass,ux,uy,uz`). The VTP state 4 and the
track-log state `film` appear only when a parcel is absorbed. Without a film every output is unchanged (R2).

## Registered checks

All tests run in release. Library checks are in `crates/rcfd-particles/tests/wall_film.rs`; case-tree and run-path checks
in `crates/rcfd-api/tests/wall_film_f253.rs`. "Relative" is `|a - b| / |b|`.

**Geometry G (A and B).** `box_mesh([10, 4, 2], [0.05, 0.02, 0.01])`, rotated by 30 degrees about the y axis through the
origin, so the patch `zmin` is a plane inclined at 30 degrees with the local x direction going down. World gravity
`(0, 0, -9.81)`. The film is on `zmin`, with `rho_l = 1000` and `mu_l = 1e-3`. Columns `j = 0..3` (local y centres 2.5,
7.5, 12.5, 17.5 mm), rows `i = 0..9` (local x centres `2.5 + 5 i` mm). Design flow `Gamma = 5e-4` kg/(m s), so
`Re_f = 2`, `h_N = 6.737272e-5` m, `u_bar_N = 7.421401e-3` m/s, and the wave Courant number at a step of 0.01 s is
`3 u_bar_N 0.01 / 0.005 = 0.0445`.

**A. The model (library).**

- **A1 closed forms.** `nusselt_thickness` of `Gamma` gives back `Gamma = rho_l u_bar h` through `film_velocity` to 1e-14
  relative; `|g_t|` of every `zmin` face equals `9.81 sin(30 deg)` to 1e-14 relative, and `u_bar` has no component across
  the slope beyond 1e-14 of its size.
- **A2 residual.** Every `zmin` face holds `M = rho_l h_N A_f`, there is no source, one `advance` of 0.01 s: the faces of
  rows 1 to 9 keep their mass to 1e-13 relative; row 0 loses `Phi dt` to row 1 to 1e-13 relative; held + drained equals the
  initial mass to 1e-14 relative.
- **A3 approach.** An empty film; at the start of every step a mass `S dt` is added to each row-0 face, with
  `S = Gamma x 5 mm = 2.5e-6` kg/s and `dt = 0.01` s, for 2000 steps (20 s). At 20 s, every face of rows 1 to 9 is within
  1e-10 relative of `h_N`; added = held + drained to 1e-12 relative; the smallest face mass over all sub-steps is `>= 0`;
  every step takes one sub-step. Prediction: a Python model of the same scheme on one column with these numbers (not in
  the repository) reaches 5e-15 everywhere at 15 s, and 1.8e-7 at row 9 at 10 s. 20 s is the registered time.

**B. The acceptance case (library).** Geometry G, carrier at rest (`u = 0`, `rho_g = 1.2`, `mu_g = 1.8e-5`), drag `none`,
analytic integrator, cell-centre interpolation. Four point injectors, one per column, at local `x = 2.5e-3 - 2e-3 tan(30
deg) = 1.3453e-3` m, `y = y_j`, `z = 2e-3` m, rotated to world, from rest, rate 730 parcels/s, `d = 1e-4` m, density 1000,
`mass_flow = 2.5e-6` kg/s each (equal-mass parcels, `Gamma` times the 5 mm column width), injecting from 0 to 20 s. A
parcel falls vertically for 0.0217 s and lands at the centre of the row-0 face of its column. Flow steps of 0.01 s, 2000
steps. The window is `[18, 20]` s: 200 steps, 20 whole periods of the feed (73 parcels per 0.1 s). The station is row 6
(local x 32.5 mm), six rows below the feed and three above the drain edge. The reference is
`h_acc = (3 mu_l Gamma / (rho_l^2 g sin(theta)))^(1/3)` with `Gamma = 2.5e-6 / 5e-3`, `g = 9.81`, `theta = 30` degrees.

- **B1 (acceptance).** Every station face is within 5 percent of `h_acc`, both as the average over the window and at 20 s.
- **B2 (consistency).** The window average of every station face is within 1e-6 relative of `h_acc`. Prediction (the
  Python model with the arrival counts of the 730/s trains and the fall time): 2e-16 for the window average and 5e-10 at
  20 s.
- **B3 (acceptance).** At the end of every step, `|injected - (active + deposited + escaped + stuck + held + drained)| <=
  1e-10 injected`. Masses are `n rho_p pi d^3 / 6` per parcel; held is `sum M_f`; drained is the booked total; the masses
  of the absorbed parcels are not counted again.
- **B4.** Absorbed (the masses of the parcels in state 4) equals held + drained to 1e-10 relative.
- **B5.** `Re_f = 4 Gamma / mu_l = 2` is asserted inside `(0, 30]` and below `(10/3) cot(30 deg)`; the largest face
  `Re_f` at 20 s is at most 30.
- **B6.** No parcel ends deposited, escaped or stuck; the smallest face mass over all sub-steps is `>= 0`.

**C. Case tree and run path.**

- **C1 located errors.** Each invalid input of S2 gives a diagnostic at the path stated there, from `Case::semantic_errors`
  or from `Simulation::new` (mesh-dependent ones); a valid film case gives none.
- **C2 tree equals library.** Case K: `box_2d` with `nx = 10`, `ny = 2`, `lx = 0.05`, `ly = 0.01` (depth 5 mm); frozen
  flow at rest; `physics.gravity = [4.905, -8.495709211125343, 0]` (9.81 at 30 degrees from `-y`, so `ymin` is the incline);
  the film on `ymin` (`viscosity = 1e-3`, `particles.density = 1000`); one point injection at
  `(1.3453e-3, 2e-3, 2.5e-3)`, rate 730/s, `diameter = 1e-4`, `mass_flow = 2.5e-6`, to 20 s; `particles.dt = 0.01`, drag
  `none`, integration `analytic`. `track_frozen`, the run path and a tracker built by hand from the same inputs give the
  same bits: fingerprint, face masses, drained.
- **C3 (acceptance in the tree).** The row-6 face of K (x centre 32.5 mm) is within 5 percent of `h_acc` at 20 s, with
  gravity tilted instead of the mesh.
- **C4 outputs.** The film fields of `particle_report.json` equal the tracker's values; `counts.film` is the number of
  absorbed parcels; `particle_film.csv` has one row per film face, and its thickness column reads back `M / (rho_l A)` to
  1e-15 relative.

**D. Defaults.**

- **D1 R1.** FNV-1a 64 of the fingerprint, exchange record and parcel temperatures of a tracker without a film (the R1
  scenario in `wall_film.rs`: two injections with Rosin-Rammler and uniform sizes, a cone, a mass flow, dispersion, heat
  transfer, the exchange record, stick, escape and reflecting walls, Schiller-Naumann and RK4, 150 steps; on main it
  injects 220 parcels and ends with 108 active, 58 deposited, 53 escaped and 1 stuck). Recorded on main 201ea17 (Windows):
  `e56421d672c5454f`. It must stay equal on Windows. It is printed elsewhere, because `exp` may differ
  in the last bits between platforms (ADR 0137).
- **D2 R2.** FNV-1a 64 of the names and bytes of the particle output files of a run without a film (the R2 case in
  `wall_film_f253.rs`: piso, a point injection, the track log, numbered VTP files). Recorded on main 201ea17 (Windows):
  `ce9df10b3a885c96` (over `particle_report.json`, `particle_tracks.csv`, `particles.vtp` and `particles_10.vtp` to
  `particles_40.vtp`). It must stay equal on Windows and is printed elsewhere.
- **D3.** `case_hashes_w48u.rs` H1, `baseline_hashes.rs` and the existing particle suites pass unchanged; no
  `DEFAULT_KEYS` entry is added (the film object is optional).
- **D4.** A case without `particles.film` has no `film` key in its data; a case that writes `[particles.film]` with
  `enabled = false` and nothing else has the settings hash of the case without it.

All the tests above take a few seconds in release.

## Not delivered

Gas shear and gas pressure gradients on the film, the hydrostatic and surface-tension terms, film inertia and waves,
splash, rebound and breakup at impact, film separation, dripping and stripping at edges and overhangs, evaporation and
heat transfer of the film, any effect of the film on the carrier (mass, momentum, blockage), the film together with
`two_way` or `heat_transfer`, the film state in a restart, a comparison with measured data.

## Open questions and choices for the maintainer

- **Q1. Status of F-253.** If every registered check is met, the acceptance is met on the branch. Marking F-253 done is
  the maintainer's decision; the roadmap says partial until then.
- **Q2. Gas buoyancy in the film (M1).** Options: (a) leave it out, as registered, so the steady state is the roadmap
  formula; (b) use `(rho_l - rho_g) g_t`, which matches the parcels' buoyancy and moves the film thickness by about 4e-4
  for water in air. Recommendation: (a); revisit if a film in a dense gas is wanted.
- **Q3. Gas shear.** Options: (a) a new item for a shear-driven closure, checked against `h = (2 mu_l Gamma / (rho_l
  tau_g))^(1/2)` for a film driven by shear alone; (b) nothing for now. Recommendation: (b) until a case needs it.
- **Q4. Impact regimes.** Splash and rebound at a film are not in F-253. Options: (a) add them to F-254 (wall
  interaction); (b) a new item. Recommendation: (a).

## Amendment 1 (2026-11-05): the set-up of case K

Written before the corrected run. The library checks A, B and D1 had run (their results go in the addendum); no check of C
had measured anything.

The first run of C1, C2-C3 and C4 stopped at the set-up. `Simulation::new` refuses case K as registered with
`solver.frozen_flow: a prescribed flow field is for the energy equation: set energy.enabled = true`. C1 failed at its
precondition (the valid K has no diagnostic), and C2-C3 and C4 at building the simulation. This is a defect of the
registered case text, not of the code under test: the PT14 frozen-flow case of ADR 0028 has the same two lines for the
same reason.

Correction: case K gains `[energy] enabled = true` and `[initial] temperature = 300.0`, as PT14 has. Nothing else
changes: the mesh, the gravity, the particles, the film, the limits and the checks stay as registered. The energy
equation runs on the frozen field at rest and does not reach the particles or the film (`heat_transfer` is off). The three
tests on the registered K stay as ignored records (`*_k_setup_defect`), and the corrected tests are new.

## Addendum

To be written after the runs.
