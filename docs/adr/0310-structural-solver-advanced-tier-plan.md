# ADR 0310: Structural solver, advanced tier: scope, architecture additions and the layout of ADRs 0311 to 0339

Status: **PROPOSED 2026-11-12** (planning only; no solver code written, nothing run). Written at main 1eab76ec on the branch
`structural-advanced-plan`. Not yet accepted by the maintainer. The decisions marked A1 to A10 in section 5 are open and are the
maintainer's; everything else is a proposal that the implementation agents' registrations may refine.

The structural solver is planned in three tiers. The basic tier is [ADR 0270](0270-structural-solver-basic-tier-plan.md) (linear
statics and modes at rest, roadmap items F-370 to F-387), the intermediate tier is [ADR
0290](0290-structural-solver-intermediate-tier-plan.md) (rotating blades, elements that bend, dynamics, the first two-way
coupling, F-388 to F-406). Both are plans: no line of `rcfd-fem` exists. This document plans the advanced tier on top of them as
twenty-nine ADRs, 0311 to 0339, each one work package for a single implementation agent, each with a stub file here with status
PLANNED; the registration with its limits is written by the implementation agent before any code (`AGENT_RULES.md` rule 1).

**This is the last tier.** No later planner exists, so what is not in the list of section 3 is not in the structural roadmap.
Section 1.3 names everything that was considered and left out, with the reason, in two groups: what has no acceptance route in
this repository, and what has one but did not fit under the cap of twenty-nine.

It is for the maintainer (sections 1.3 and 5 first), for the lead who enters the roadmap items (section 7), and for the
implementation agents (sections 2 and 3).

Rules and parents:

* The house rules of `w49i-work/AGENT_RULES.md` apply to every work package: registration before code with limits fixed in
  advance, known-miss records, defaults bit-identical, clean-room, no new third-party crate, no unsafe code, no golden or
  hash-table edit without the maintainer, no edit of an existing test.
* Parents: ADR 0270 (section 1.3 is the first cut of this tier) and ADR 0290 (section 1.2 lists what it moved here, section 1.3 is
  its cut, section 2.5 the interfaces this tier builds on); [fan project breakdown](../fan-project-breakdown.md) items E2 (rotor
  dynamics), E3 (unsteady blade loads as forcing) and E5 (blade and housing response, FSI), and its target list ("vibration:
  bearing loads and housing response", noise, material); [fan milestone](../milestone-fan.md) ("The CFD solver does not see blade
  material").
* What the tier connects to: the co-simulation participant and the coupled run of ADR 0303, 0304 and 0307; `rcfd-post::rotordyn`
  (ADR 0120, the rigid rotor on two bearings); `rcfd-post::acoustics` (ADR 0066); `rcfd-dist` (ADR 0058) and `rcfd-gpu` (ADR
  0038); the parallel strategy of ADR 0003.

## What was read (disclosed)

* **Read:** `AGENT_RULES.md`; ADR 0290 in full; ADR 0270 sections 1, 2 and 4 to 7 and the headings of its section 3; the stubs of
  ADR 0299 and 0308 (for the stub style); `docs/milestone-fan.md` and `docs/fan-project-breakdown.md` (the long rows of the
  capability table cut at 600 characters); the calls, the mapping and the limits sections of `docs/cosimulation-protocol.md`; ADR
  0003 (parallel strategy; the brief's "ADR 0004" is the GUI decision), the head of ADR 0058 and the head and the gaps list of
  `docs/gpu.md`; the public signatures of `rcfd-dist` (all files), `rcfd-gpu/src/lib.rs`, `rcfd-linalg/src/backend.rs` (head),
  `rcfd-post/src/{rotordyn.rs, acoustics.rs, fwh.rs}` and the head comment of `rotordyn.rs`; the dependencies in the `Cargo.toml`
  of `rcfd-post`, `rcfd-dist`, `rcfd-gpu`, `rcfd-linalg` and `rcfd-api`; the layer table of
  `crates/rcfd-cli/tests/workspace_graph.rs`; titles and statuses of the roadmap items named below; a search of
  `docs/reference-data.md` and `docs/validation-data.md` for the flag benchmark, fatigue data and shell benchmarks (none found).
* **Not read:** the other stubs of the two tiers (their text is repeated in the plans, which were read); the bodies of ADR 0250,
  0252, 0256 and 0261 (ADR 0290's findings on them are taken over); ADR 0263 (not on main); function bodies anywhere;
  `rcfd-mesh::partition` beyond two constructor signatures; the service protocol. Nothing was built or run.
* **Findings that shape the plan:**
  1. Nothing of the two earlier tiers exists as code. Every file name of `rcfd-fem` below is read from two plans; the scaffold
     package (ADR 0311) is where a mismatch surfaces first.
  2. The repository holds no material datum of a polymer (stiffness against temperature, relaxation, creep, stress-life curve,
     damping) and no fibre-orientation field; `docs/fan-project-breakdown.md` limit 1 says the same of every measured fan
     quantity. Material models can be verified against closed forms; a life, a creep growth or a damped amplitude of the real fan
     cannot be validated here. This divides section 1 into "planned" and "not planned".
  3. The fan's flow side is still not validated (F-126 not met) and has no moving wall in a rotating passage. A flutter margin of
     the fan has therefore no acceptance route; the machinery that computes aerodynamic damping has one, on closed forms (ADR
     0324).
  4. `rcfd-post::rotordyn` is a rigid rotor on two isotropic bearings written in complex coordinates. A flexible rotor is a new
     model beside it; the join is a conversion and a stiff-shaft limit check, not an edit of that file.
  5. `rcfd-dist` has in-process ranks only (MPI is F-100, todo) and builds its decomposition from a finite-volume mesh; `rcfd-gpu`
     runs CG with Jacobi or Chebyshev on a CSR matrix. Both can take a stiffness matrix as they are. `rcfd-fem` is planned in
     layer 2 beside them, so the glue must sit above: in `rcfd-api`, which already depends on `rcfd-gpu`.
  6. The impermeable FW-H surface of `rcfd-post::fwh` needs the surface pressure of the sound field, which a structural solve does
     not have. The radiated sound of a housing is therefore planned as equivalent radiated power and the Rayleigh integral, not
     through F-136.
  7. The MCP tools of ADR 0308 take a solid case file. New analyses reach the assistant through the file (ADR 0338) with no new
     tool, so no package of this tier needs a golden edit. One package needs the maintainer for another reason: it needs published
     values that the repository lacks (ADR 0337).

## 1. Scope of the advanced tier

### 1.1 Advanced tier (this plan)

Ordered by what the fan goal needs first: the material of a plastic blade, then its life under blade-passing forcing, then the
rotor and the housing, then the general mechanics, then scale.

* **Polymer blade materials:** orthotropic elasticity with material axes and orthotropic expansion, properties tabulated against
  temperature (0312); linear viscoelasticity by a Prony series, in time and as a complex modulus (0313); creep (0314).
* **Life under blade-passing forcing:** a direct harmonic solve with material loss factors and discrete dampers (0315);
  stress-life fatigue with mean-stress corrections, cycle counting and linear damage (0316); random response (0320).
* **Rotation:** an eigensolver for damped and gyroscopic problems (0317); the Coriolis term in the rotating frame, closing
  decision N5 of ADR 0290 (0318); a flexible rotor on bearings in the stationary frame with forward and backward whirl, joined to
  the rigid model (0319).
* **Housing and noise:** a triangular shell (0322); stress stiffening of beams and shells and the Campbell table of a shell blade
  (0323); radiated sound of a vibrating surface (0321).
* **Flutter machinery:** aerodynamic damping and added mass of a mode by the energy method, on cases with a closed form (0324).
* **Nonlinear mechanics:** J2 plasticity (0325), nonlinear transient dynamics (0326), follower pressure, arc-length continuation
  and modes about a nonlinear state (0327), hyperelasticity (0328).
* **Assemblies:** tied non-matching meshes, joints and the cyclic extensions (0329); frictionless contact with interference fits
  (0330); substructuring (0333).
* **Composites and cracks:** laminated shells with ply failure indices (0331); fracture parameters (0332).
* **Scale and quality:** a sparse factorisation with a fill-reducing ordering (0334); patch stress recovery with an error estimate
  (0335); elasticity on the ranks of `rcfd-dist` and on the device (0336).
* **Public benchmark:** the structural tests and the steady coupled case of the flag benchmark, blocked on the maintainer (0337).
* **Entry point and demonstration:** the advanced analyses in the solid case file (0338); a polymer fan blade and a frame (0339).
* **Verification:** closed forms and equivalences, as in the two earlier tiers. One public benchmark (0337). No vendor manual, no
  paid standard, no measured datum.

### 1.2 Departures from the first cuts of ADR 0270 section 1.3 and ADR 0290 section 1.3, and why

1. **The order is the fan's, not the textbook's.** Orthotropy, temperature, viscoelasticity, creep, the direct harmonic solve and
   fatigue come before plasticity and contact. ADR 0290 named J2 plasticity "first"; here it is ADR 0325. The driver interface it
   needs is prepared by the scaffold, so the order costs nothing.
2. **Orthotropy and temperature-dependent properties share one package** (0312), and **fatigue is one package** (0316) that holds
   curves, corrections, counting and damage: each half alone is an S, and section 6 gives the splits.
3. **Fatigue and fracture are verified, not validated.** The checks are textbook closed forms (the hole in a plate, the cracked
   plate, the penny-shaped crack, Basquin and Miner arithmetic, the damage of a narrow-band process). The paid standards that
   define rainflow counting or assessment procedures are not used.
4. **Flutter is split.** The energy-method machinery is planned on closed forms (0324); the margin of the rotating fan passage is
   not planned (section 1.3, group A).
5. **The Coriolis term is planned twice,** because it is two models: the rotating frame of a solid blade or disc (0318) and the
   stationary frame of a shaft on bearings (0319). Cyclic symmetry with the Coriolis term is left out.
6. **Radiated sound is added** (0321). Neither earlier cut named it; the fan target does ("housing response", noise).
7. **Distributed and device solves are one package** (0336), limited to what the two crates offer today.
8. **A scaffold package is added** (0311), as in the two earlier tiers.
9. **Dropped to stay within twenty-nine:** frictional contact, explicit dynamics, topology optimisation, an iterative eigensolver,
   wedge and pyramid elements, a mixed displacement-pressure element, response spectra, heat conduction in the solid, several
   interfaces and two-sided shells in a coupled run, mesh adaptation, the protocol transport. Each is in section 1.3, group B,
   with what would verify it.

### 1.3 Not planned

**Group A: no acceptance route in this repository.** These need measured data, a paid standard or a flow capability that does not
exist. An ADR for any of them could be written but not accepted.

| Item | Why it cannot be accepted here |
|---|---|
| Flutter margin of the fan (aerodynamic damping of a blade mode on the rotating passage) | The rotating passage is not validated (F-126 not met, FAN-01 misses), it has no moving wall in the rotating frame, and no measured damping exists. ADR 0324 delivers the method on closed forms; running it on the fan would produce a number with nothing to check it against |
| Validated life, creep growth or damped amplitude of a real fan blade | No stress-life curve, creep constant, relaxation spectrum or damping ratio of any blade polymer is in the repository; manufacturers' data are proprietary. The models are planned; their inputs are assumed (decision A1) |
| Fibre orientation of a moulded blade (the material axes of ADR 0312 from a mould-filling simulation) | Needs an injection-moulding solver or measured orientation tensors; neither exists here |
| Fatigue assessment by code or guideline (weld classes, notch-sensitivity rules, the standard rainflow procedure as a conformance claim) | The documents are paid standards; house rule 4 forbids working from them |
| Validated sound level of the housing | Needs a measured fan (fan breakdown G2, missing). ADR 0321 gives radiated power with its stated assumption |
| Bearing coefficients of a small fan (sleeve or ball bearing stiffness and damping as predictions) | Research-grade tribology with no data here; ADR 0319 takes them as inputs, as `rotordyn` does |
| The unsteady cases FSI2 and FSI3 of the flag benchmark | Large mesh motion and thousands of coupled steps on a flow side that is first order in time; public values exist, but no realistic route to meeting them within a package |
| Progressive damage of laminates, delamination, crack growth with remeshing, elastic-plastic fracture | No closed form beyond the stationary cases of ADR 0331 and 0332; public benchmark values are not in the repository |
| Mistuned bladed discs | Needs measured blade-to-blade scatter to mean anything; the reduced blades of ADR 0333 would be its basis |

**Group B: an acceptance route exists; left out by the cap and by fan priority.** Each line names the closed form that would
verify it, so that a later item can be written from it. The first two are the ones I would add first (decision A6).

| Item | What would verify it | Why it is out |
|---|---|---|
| Explicit dynamics (central differences, lumped mass, stable step) | A stress wave in a bar is reproduced exactly at Courant number 1; step limit 2/w_max; energy balance. Size S | No fan use (drop or impact of a blade) |
| Frictional contact (Coulomb) | Stick and slip of a block at F = mu N; torque capacity 2 pi mu p r^2 L of the interference fit of ADR 0330. Size M | The fan needs the fit pressure, which ADR 0330 gives |
| Topology optimisation (density method, compliance) | Sensitivities against differences; the convex case with penalty exponent 1, whose optimum is closed-form. Size M | It is optimiser work (category of F-095, F-249); it needs from the solver only compliance and its sensitivity |
| Iterative eigensolver on the multigrid | Agreement with the factorisation-based solver; closed-form bar and plate spectra | With ADR 0334 the factorisation reaches the fan's models; the multigrid of ADR 0305 does not cover shells |
| Wedge and pyramid elements | Patch test, volume, the closed forms of ADR 0280 | No mesh source of the repository produces them for a solid (ADR 0272 refuses them) |
| Mixed displacement-pressure element | Lame's cylinder at nu = 0.5 exactly | Needs an indefinite solve; B-bar (ADR 0293) and the mean-dilatation element (ADR 0328) cover the planned materials |
| Response spectra (shock) | Two-unknown closed forms of the combination rules | No fan use; random response is planned (ADR 0320) |
| Heat conduction in the solid, coupled thermo-mechanics | Thermal stress of a cylinder with a logarithmic temperature | The temperature field comes from the flow solver's conjugate heat transfer through the deck (F-384) and the thermal strain of F-380 |
| Several interfaces, shells wetted on both sides, heat across the interface in a coupled run | A plate between two fluid columns (sum of two added masses) | The fan path uses solid blades with one closed wetted surface; the protocol is "one wall and two participants" |
| The transport of the protocol (the calls over the JSON-RPC service) | Bit identity with the in-process run | Coupling and service work, not structural (ADR 0290 said so); it should be an item next to F-139 and F-085 |
| Mesh adaptation driven by the error estimate | Error halves at the predicted rate on the manufactured solution | Needs a mesher that takes a size field; the estimate itself is planned (ADR 0335) |
| Mortar ties and contact, large sliding, self-contact, contact in dynamics | The patch test across a non-matching interface | Size; ADR 0329 and 0330 state what the one-sided method costs |
| Cyclic symmetry with the Coriolis term or with damping | Sector against full rotor | The full rotor route exists (ADR 0318) |
| Large rotations of beams and shells (nonlinear statics of a shell blade), finite-strain plasticity, strain-life fatigue, critical-plane criteria | Elastica for the beam; no cheap closed form for the others | Nonlinear statics stays on solid elements (ADR 0300); the rest has no fan need |
| Fully coupled structure and acoustic fluid (finite or boundary elements of the air) | Pulsating sphere | ADR 0321 covers what air needs; water would need this |
| MPI transport, multi-device and device assembly, multigrid on ranks or device | Bit identity and scaling records | F-100 and the gaps of F-101; not structural |

## 2. Architecture additions

### 2.1 Where the code lives: no new crate

```text
rcfd-linalg  gains: complex.rs, cskyline.rs (complex symmetric L D L^T), eigen_general.rs (general real eigenproblem,
                    projected quadratic problem), sparse_chol.rs (ordering and sparse L D L^T)
rcfd-fem   gains: twenty-four modules (section 2.2); additions to model.rs, material.rs, mesh.rs, element.rs, shell.rs,
                    nonlinear.rs (scaffold only), cyclic.rs (0329), rotating.rs (0323, if needed), solve.rs (0334)
rcfd-post    gains: solid_acoustic.rs, solid_aerodamp.rs, solid_rotor.rs
rcfd-api     gains: solid_parallel.rs and the dependency on rcfd-dist (0336); additions to solid_case.rs (0338)
rcfd-cli, rcfd-mcp, rcfd-fv, rcfd-dist, rcfd-gpu: unchanged
```

The graph stays acyclic and the layer table of `workspace_graph.rs` needs no line: `rcfd-api` (layer 5) may depend on `rcfd-dist`
(layer 2).

### 2.2 New modules

Declared once by ADR 0311 as stubs, so that no later package edits a `lib.rs` (the one exception is `rcfd-api`, where ADR 0336
adds its own line and runs alone on that file).

| Crate | Module | Content | Filled by |
|---|---|---|---|
| `rcfd-linalg` | `complex` | complex number type and vector helpers | 0311 |
| `rcfd-linalg` | `cskyline` | complex symmetric factorisation on the envelope | 0315 |
| `rcfd-linalg` | `eigen_general` | dense general eigenproblem; quadratic problem on a subspace | 0317 |
| `rcfd-linalg` | `sparse_chol` | minimum degree, elimination tree, sparse L D L^T with inertia | 0334 |
| `rcfd-fem` | `mat_ortho`, `mat_table` | orthotropic elasticity and axes; properties against temperature | 0312 |
| `rcfd-fem` | `mat_visco` | Prony series, shift, complex modulus | 0313 |
| `rcfd-fem` | `mat_creep` | power-law creep | 0314 |
| `rcfd-fem` | `harmonic_direct` | direct harmonic solve, loss factors, dampers, base motion | 0315 |
| `rcfd-fem` | `fatigue` | stress-life, corrections, counting, damage | 0316 |
| `rcfd-fem` | `damped` | damped and complex modes | 0317 |
| `rcfd-fem` | `gyro` | gyroscopic matrix in the rotating frame, split Campbell table | 0318 |
| `rcfd-fem` | `rotor` | shaft, discs, bearings, whirl, unbalance response | 0319 |
| `rcfd-fem` | `random` | spectral response, root-mean-square stress, spectral moments | 0320 |
| `rcfd-fem` | `shell3` | three-node shell | 0322 |
| `rcfd-fem` | `geometric_struct` | geometric stiffness of beams and shells | 0323 |
| `rcfd-fem` | `mat_j2` | J2 plasticity | 0325 |
| `rcfd-fem` | `nl_transient` | nonlinear time integration | 0326 |
| `rcfd-fem` | `follower`, `arclength` | follower pressure; continuation | 0327 |
| `rcfd-fem` | `mat_hyper`, `nonlinear_md` | hyperelastic energies; mean-dilatation element | 0328 |
| `rcfd-fem` | `tie` | node-to-surface search, ties, joints | 0329 |
| `rcfd-fem` | `contact` | frictionless contact | 0330 |
| `rcfd-fem` | `laminate` | layup stiffness, ply stresses, failure indices | 0331 |
| `rcfd-fem` | `fracture` | domain integral, stress intensity, growth life | 0332 |
| `rcfd-fem` | `cms` | condensation and component modes | 0333 |
| `rcfd-fem` | `estimate` | patch recovery and error estimate | 0335 |
| `rcfd-post` | `solid_acoustic` | equivalent radiated power, Rayleigh integral | 0321 |
| `rcfd-post` | `solid_aerodamp` | prescribed modal motion of the wall, work per cycle | 0324 |
| `rcfd-post` | `solid_rotor` | conversion to and from `rotordyn`, discs from a solid model | 0319 |

### 2.3 Data model additions (plain data, readable from a file)

* **`Material`** gains the variants `Orthotropic` (nine constants, expansion, an axes rule: fixed triad, cylindrical about an
  axis, or a triad per element), `Tabulated` (any elastic variant with tables against temperature), `Viscoelastic` (Prony terms,
  shift constants), `Creep`, `J2`, `Hyperelastic`. Elastic variants may carry a loss factor (constant or a table against
  frequency), used by the harmonic and damped-mode analyses only.
* **The constitutive call takes a point context:** temperature, time increment and time, and the material axes at the point. The
  default context reproduces today's call bit for bit.
* **State layout:** each material declares how many numbers it keeps per integration point (zero for the elastic ones). The state
  is one array per element block, committed and rolled back by the driver as in ADR 0300, and it is part of the saved state of the
  integrators.
* **`ElementKind`** gains `Shell3`. **Shell property** gains `Layup { plies }` beside `thickness`; the shell asks one
  section-stiffness function for its membrane, coupling, bending and shear stiffness.
* **Newton driver:** a list of extra contributions, each with a residual and a tangent (contact, follower loads), and a
  step-control hook (arc-length). Both empty by default.
* **`Constraint`** gains `Tie { fine, coarse, tolerance }`; the cyclic pair gains a matching rule (`node`, `surface`).
* **Analyses stay functions with their own settings and results**, as in ADR 0290 section 2.3: `harmonic_direct::response`,
  `damped::modes`, `gyro::campbell`, `rotor::whirl` and `rotor::unbalance_response`, `random::response`, `fatigue::assess`,
  `nl_transient::Integrator`, `arclength::trace`, `cms::reduce`, `fracture::j_integral`, `estimate::energy_error`.
* **Complex numbers:** `rcfd_linalg::complex::C64`. `rcfd-post::rotordyn` keeps its own type (that file is not edited);
  `solid_rotor` converts.

### 2.4 What must change in the earlier tiers' designs, said plainly

1. **The constitutive call (request to ADR 0273 and ADR 0300, before they are written).** ADR 0270 fixed it as `stress(strain,
   state) -> (stress, tangent)`. Temperature dependence, creep, viscoelasticity and material axes need more input than a strain.
   If the call takes a context structure built by a constructor from the start, this tier adds fields; if not, ADR 0311 adds a
   second entry point that the old one forwards to, and every element routine must call the new one. The request is cheap now and
   awkward later (decision A3).
2. **Material evaluation per integration point.** If the basic elements build one elasticity matrix per element, tables against
   temperature and curved material axes are wrong inside large elements. The elements should ask the material at each point; for a
   constant isotropic material the result must keep its bits.
3. **The Newton driver of ADR 0300 must be open.** Contact, follower loads and arc-length all add to its residual, tangent or step
   control. ADR 0311 gives it the two extension points; if the driver is written as one closed function, that is a restructuring
   whose bit identity is the scaffold's hardest clause. ADR 0300's stub already says materials plug in "without changing the
   driver"; it should say the same of extra contributions.
4. **The shell of ADR 0295 must take its section stiffness from one function,** not integrate a homogeneous thickness in line; the
   laminate (ADR 0331) replaces that function.
5. **The transient integrator of ADR 0301** is linear with one factorisation. The viscoelastic material fits it only if a history
   force can be added per step; ADR 0313 says what it does otherwise. The nonlinear integrator (ADR 0326) is a second type that
   shares the saved state.
6. **The rotating analysis of ADR 0299** stays the default; the Coriolis term is opt-in (ADR 0318) and decision N5 of ADR 0290
   keeps its answer for the default. If `rotating.rs` computes the softening mass for solid elements only, ADR 0323 edits it.
7. **The eigensolver of ADR 0283 and the factorisation of ADR 0274** are not replaced. The general eigenproblem, the complex
   factorisation and the sparse factorisation are new files beside them. The sparse factorisation is opt-in (decision A4).
8. **The mesh validation of ADR 0271** must allow coincident nodes when a model declares a crack (ADR 0332) and must not merge
   them.
9. **Nothing changes in `rcfd-fv::cosim`, `rcfd-dist`, `rcfd-gpu` or `rcfd-post::rotordyn`.**

### 2.5 Interfaces and the case files

* **Case file.** ADR 0338 extends the solid case file with the fan-relevant materials, analyses and result requests. The other
  packages are library-only unless merged when ADR 0338 starts; the user page says which.
* **No new tool, no new subcommand, no flow case key.** The tools of ADR 0308 run any solid case file. The coupled run of ADR 0307
  takes whatever solid file it is given; a nonlinear participant in a coupled run is not planned beyond the flag's steady case.
* **Parallel solves** are reached through `rcfd_api::solid_parallel`, not through the solid case file, until a decision on the
  keys (decision A5).
* **Units and conventions** are those of ADR 0270 section 2.2. Loss factor, damping ratio and logarithmic decrement are three
  different numbers; every result names which one it is.

## 3. The ADR list

Twenty-nine work packages, ADR 0311 to 0339. Each is sized for one implementation agent working alone under `AGENT_RULES.md`: S is
about a session, M a few hours; nothing is L (where a package is at the upper end of M, its entry says how to split it). Each has
a stub file in this directory. The acceptance texts are drafts in the roadmap's form; the implementation agent's registration
fixes every limit before its code. SA-n is the placeholder id of the roadmap entry of ADR 0310 + n. Every citation is from memory
unless said otherwise; every recalled constant is marked for recomputation.

| ADR | Package | Title | Size | Depends on |
|---|---|---|---|---|
| [0311](0311-advanced-scaffold-modules-material-context-driver-hooks.md) | SA-1 | Scaffold of the advanced tier: module layout, material variants with a point context and a state layout, extension points of the Newton driver | M | F-372, F-388, F-397 |
| [0312](0312-orthotropic-elasticity-and-temperature-dependent-properties.md) | SA-2 | Orthotropic elasticity with material axes, orthotropic expansion, and temperature-dependent properties from tables | M | SA-1, F-372, F-374, F-376, F-380 |
| [0313](0313-linear-viscoelasticity-prony-series.md) | SA-3 | Linear viscoelasticity: Prony series in time, complex modulus in frequency, time-temperature shift | M | SA-1, F-397, F-398 |
| [0314](0314-creep-power-law-implicit.md) | SA-4 | Creep: power law with time hardening, implicit integration with a consistent tangent | S | SA-1, F-397 |
| [0315](0315-direct-harmonic-solve-complex-factorisation-structural-damping.md) | SA-5 | Direct harmonic solve: complex symmetric factorisation, structural (hysteretic) damping, discrete dampers, base excitation | M | SA-1, F-373, F-374, F-383, F-399 |
| [0316](0316-high-cycle-fatigue-stress-life-rainflow-miner.md) | SA-6 | High-cycle fatigue: stress-life curves, mean-stress corrections, harmonic and counted histories, linear damage | M | SA-1, F-376, F-399 |
| [0317](0317-damped-eigenproblem-general-dense-eigensolver-complex-modes.md) | SA-7 | Damped and complex modes: a dense eigensolver for general real matrices and the quadratic eigenproblem on a modal subspace | M | SA-1, F-382, F-383 |
| [0318](0318-coriolis-term-rotating-frame-campbell.md) | SA-8 | Coriolis term of a rotating part: gyroscopic matrix in the rotating frame and the Campbell table with split modes | M | SA-7, F-396 |
| [0319](0319-flexible-rotor-on-bearings-stationary-frame.md) | SA-9 | Flexible rotor on bearings in the stationary frame: whirl, critical speeds, unbalance response, joined to the rigid-rotor model | M | SA-7, F-391, F-393 |
| [0320](0320-random-vibration-by-modal-superposition.md) | SA-10 | Random vibration: response to force and base-acceleration spectra by modal superposition, root-mean-square stress | S | SA-1, F-399 |
| [0321](0321-radiated-sound-of-a-vibrating-surface.md) | SA-11 | Radiated sound of a vibrating surface: equivalent radiated power and the Rayleigh integral of a baffled plane | S | SA-1, F-136, F-399 |
| [0322](0322-triangular-shell-element.md) | SA-12 | Three-node shell element (flat facet, assumed shear strain) for triangulated surfaces | M | SA-1, F-391, F-392 |
| [0323](0323-geometric-stiffness-of-beams-and-shells-rotating-shell-blade.md) | SA-13 | Stress stiffening of beams and shells: buckling, pre-stressed modes and the Campbell table of a shell blade | M | SA-12, F-391, F-392, F-395, F-396 |
| [0324](0324-aerodynamic-damping-by-the-energy-method.md) | SA-14 | Aerodynamic damping and added mass of a mode by prescribed motion of the wall (energy method) | M | SA-1, F-362, F-383, F-400, F-401 |
| [0325](0325-j2-plasticity-radial-return.md) | SA-15 | J2 plasticity with isotropic and kinematic hardening: radial return and consistent tangent | M | SA-1, F-389, F-397 |
| [0326](0326-nonlinear-transient-dynamics.md) | SA-16 | Nonlinear transient dynamics: Newton iteration inside the Newmark and generalised-alpha step | M | SA-1, F-397, F-398 |
| [0327](0327-follower-pressure-arc-length-modes-about-a-nonlinear-state.md) | SA-17 | Follower pressure, arc-length continuation, and modes about a nonlinear equilibrium | M | SA-1, F-395, F-396, F-397 |
| [0328](0328-hyperelasticity-nearly-incompressible.md) | SA-18 | Hyperelastic materials (neo-Hookean, Mooney-Rivlin) with a mean-dilatation element for near incompressibility | M | SA-1, F-390, F-397 |
| [0329](0329-tied-non-matching-meshes-joints-cyclic-extensions.md) | SA-19 | Tied non-matching meshes, shell and beam joints to solids, non-matching cyclic boundaries and nodes on the axis | M | SA-1, F-391, F-392, F-393, F-394 |
| [0330](0330-frictionless-contact.md) | SA-20 | Frictionless contact: node-to-surface with an augmented Lagrangian, rigid and deformable targets, interference fits | M | SA-19, F-397 |
| [0331](0331-laminated-shell-and-ply-failure-criteria.md) | SA-21 | Laminated shell: layup stiffness, ply stresses and failure indices | M | SA-2, F-392 |
| [0332](0332-fracture-parameters-domain-integral.md) | SA-22 | Fracture parameters: J-integral by the domain form, stress intensity factor, crack-growth life | M | SA-1, F-374, F-376, F-389 |
| [0333](0333-substructuring-and-component-mode-synthesis.md) | SA-23 | Substructuring: static condensation and fixed-interface component mode synthesis (Craig-Bampton) | M | SA-1, F-382, F-383, F-393 |
| [0334](0334-sparse-cholesky-with-fill-reducing-ordering.md) | SA-24 | Sparse L D L^T with a fill-reducing ordering, and its use by the static, buckling and modal solves | M | SA-1, F-373, F-374, F-382, F-402 |
| [0335](0335-stress-recovery-by-patches-and-error-estimate.md) | SA-25 | Stress recovery by element patches and an energy-norm error estimate | S | SA-1, F-376, F-378 |
| [0336](0336-elasticity-on-ranks-and-on-the-device.md) | SA-26 | Elasticity solves on the ranks of rcfd-dist and on the CUDA device | M | F-016, F-101, F-374, F-381 |
| [0337](0337-flag-benchmark-structural-tests-and-steady-coupled-case.md) | SA-27 | Public coupled benchmark (the elastic flag behind a cylinder): structural tests CSM1 to CSM3 and the steady coupled case FSI1 (needs the maintainer: reference values) | M | SA-16, F-385, F-397, F-401 |
| [0338](0338-solid-case-file-advanced-analyses.md) | SA-28 | Solid case file: the materials, analyses and results of the advanced tier | M | SA-2, SA-3, SA-4, SA-5, SA-6, SA-8, SA-10, SA-11, F-403 |
| [0339](0339-plastic-fan-blade-and-housing-demonstration.md) | SA-29 | Demonstration on the 120 mm fan: a polymer blade (orthotropy, temperature, creep, fatigue at a Campbell crossing) and the radiated power of the frame | M | SA-2, SA-4, SA-5, SA-6, SA-8, SA-11, SA-28, F-406 |

### ADR 0311 (SA-1): Scaffold of the advanced tier: module layout, material variants with a point context and a state layout, extension points of the Newton driver

**Goal.** Prepare the shared files once, so that every later package of the tier fills a file of its own. Declare the new modules
of `rcfd-fem`, `rcfd-linalg` and `rcfd-post` (ADR 0310 section 2.2) as stubs. Add the material variants (orthotropic, tabulated,
viscoelastic, creep, J2, hyperelastic) as data that is refused until its package lands; give the constitutive call a point context
(temperature, time increment, material axes at the point) and each material a declared state size per integration point, with
commit and roll-back unchanged from ADR 0300. Add the element kind `Shell3`, the shell property variant with a layup, and one
section-stiffness hook in the shell so that a laminate replaces the thickness integral without editing the element. Give the
Newton driver two extension points: a list of extra contributions (residual and tangent: contact, follower loads) and a
step-control hook (arc-length). Add a complex number type to `rcfd-linalg`. No analysis numerics.

**Dependencies.** none in this list; roadmap items: F-372, F-388, F-397.

**Acceptance (draft).** The modules, material variants, point context, state layout, element kind, layup property,
section-stiffness hook and the two driver extension points of ADR 0310 section 2 exist; every test of the basic and intermediate
tiers (the sb_ and si_ files) compiles and passes unedited, and a registered set of models (patch test, cantilever, bar modes, the
elastica of F-397, the free vibration of F-398, a Shell4 plate) gives displacements, stresses, frequencies and histories with the
same bits as before the change; with an empty contribution list and no step hook the Newton driver takes the same iterations with
the same residuals bit for bit; the default point context (no temperature, no time increment, global axes) reproduces the
isotropic stress and tangent bit for bit; each new material variant, the element kind Shell3 and the layup property are refused by
every solve with an error naming the variant and the set, never ignored; the complex type satisfies (a b) conj(b) = a |b|^2 and
a/a = 1 within 1e-15 on a registered sample and prints and parses losslessly; the baseline hashes are unchanged.

**Checked against.** No physics: the reference is the two earlier tiers (bit identity) and the refusals. The models for a
declare-once scaffold are ADR 0271 and ADR 0291.

**Files.** `crates/rcfd-fem/src/{lib.rs, model.rs, material.rs, mesh.rs, element.rs, shell.rs, nonlinear.rs}` (additions only),
one-line stubs of the modules of ADR 0310 section 2.2; `crates/rcfd-linalg/src/lib.rs` (four `pub mod` lines), `complex.rs`, stubs
`cskyline.rs`, `eigen_general.rs`, `sparse_chol.rs`; `crates/rcfd-post/src/lib.rs` (three `pub mod` lines), stubs
`solid_acoustic.rs`, `solid_aerodamp.rs`, `solid_rotor.rs`; `crates/rcfd-fem/tests/sa_scaffold.rs`.

**Size.** M. **What could make it miss.** The earlier tiers are plans. If the constitutive call of ADR 0273 has a fixed argument
list and the tests call it directly, the context goes into a second entry point with a forwarding default (ADR 0310 section 2.4,
point 1). If the Newton driver of ADR 0300 is one closed function, the extension points mean restructuring it, and the bit
identity of its iterations is then the hard clause. It edits more shared files than ADR 0291 did, hence M.

### ADR 0312 (SA-2): Orthotropic elasticity with material axes, orthotropic expansion, and temperature-dependent properties from tables

**Goal.** The material of a moulded, fibre-filled blade is not isotropic and softens with temperature. Add linear orthotropic
elasticity from nine constants with material axes per element set (fixed axes, cylindrical about an axis, or one triad per element
as data), orthotropic thermal expansion, transversely isotropic input as a shorthand, and properties tabulated against temperature
(piecewise linear E, nu, G, alpha, evaluated at the integration point from the nodal temperature field of ADR 0281, the thermal
strain being the integral of alpha from the reference temperature).

**Dependencies.** ADR 0311; roadmap items: SA-1, F-372, F-374, F-376, F-380.

**Acceptance (draft).** Orthotropy: with isotropic constants the elasticity matrix equals the isotropic one within 1e-13 relative;
constants that are not positive definite are refused naming the constant. Off-axis tension of a block with the material axes
turned by 15, 30, 45 and 60 degrees about z: axial strain sigma/E_x with 1/E_x = c^4/E1 + s^4/E2 + (1/G12 - 2 nu12/E1) s^2 c^2 and
the shear-extension ratio from the rotated compliance within 1e-10; rotating the model and the axes together leaves the strain
energy unchanged within 1e-12. Cantilever of slenderness 20 with E1 along the span and E2 = E1/10: tip deflection P L^3/(3 E1 I) +
P L/(kappa G13 A) within 1 percent. Free heating gives the strains alpha_i dT in material axes with zero stress, full restraint
the stress -D alpha dT, both within 1e-10. Cylindrical axes: a tube b/a = 2 with E_theta/E_r = 4 under internal pressure in plane
stress has the bore hoop stress of the polar-orthotropic closed form (powers r^(k-1) and r^(-k-1), k = sqrt(E_theta/E_r), derived
in the registration) within 1 percent. Tables: a bar whose temperature and so its modulus vary linearly from E0 to E1 along it
stretches under an end force by F L ln(E1/E0)/(A (E1 - E0)) within 0.1 percent with 20 elements; with alpha = a0 + a1 T the free
expansion is a0 (T - Tref) + a1 (T^2 - Tref^2)/2 within 1e-10; a constant table gives the bits of the constant material; a
temperature outside a table is refused with the element and the temperature.

**Checked against.** Closed forms: the rotated compliance of an orthotropic sheet (tensor rotation; Jones, Mechanics of Composite
Materials, from memory); the polar-orthotropic tube (Lekhnitskii, Theory of Elasticity of an Anisotropic Body; re-derived from
equilibrium and compatibility in the registration); the bar with a varying modulus (one integral).

**Files.** `crates/rcfd-fem/src/mat_ortho.rs`, `crates/rcfd-fem/src/mat_table.rs`, `crates/rcfd-fem/tests/sa_ortho.rs`,
`crates/rcfd-fem/tests/sa_mat_table.rs`.

**Size.** M. **What could make it miss.** If the element routines of the basic tier evaluate the material once per element,
properties that vary inside an element need the routine opened (the scaffold's point context is meant to prevent that). The
constants of a real moulded blade and its fibre directions are not in the repository: they are inputs (decision A1). Split if
needed: orthotropy first, tables second.

### ADR 0313 (SA-3): Linear viscoelasticity: Prony series in time, complex modulus in frequency, time-temperature shift

**Goal.** Polymers relax and damp. Add the generalised Maxwell solid (Prony series on the shear and, optionally, the bulk modulus)
with a recursive update of one internal stress per branch and integration point, exact for a strain that is linear in the step;
the time-temperature shift (WLF or Arrhenius constants as inputs); and the storage and loss moduli as functions of frequency,
which is the material damping that the direct harmonic solve (ADR 0315) and the damped modes (ADR 0317) take.

**Dependencies.** ADR 0311; roadmap items: SA-1, F-397, F-398.

**Acceptance (draft).** Relaxation of one element after a strain step: sigma(t) = eps0 (E_inf + sum E_i exp(-t/tau_i)) within
1e-10 at every step end for three branches. Creep of the standard solid under constant stress: eps(t) = sigma0 (1/E_inf - (1/E_inf
- 1/E_0) exp(-t/tau_c)), tau_c = tau E_0/E_inf, within 0.1 percent at 20 steps per tau with observed order >= 1.9. The complex
modulus function returns E' = E_inf + sum E_i w^2 tau_i^2/(1 + w^2 tau_i^2) and E'' = sum E_i w tau_i/(1 + w^2 tau_i^2) within
1e-13, and a strain cycled in time dissipates pi eps0^2 E'' per cycle and volume within 1 percent at 200 steps per cycle. Shift:
the relaxation curve at a temperature with shift factor a_T equals the reference curve at t/a_T within 1e-10. Free vibration of a
bar of the standard solid with a loss factor near 0.05 at its first frequency: logarithmic decrement pi E''/E' within 3 percent. A
material without branches gives the bits of the elastic one; the internal stresses are saved and restored bit for bit with the
integrator state; a non-positive relaxation time or modulus is refused with the key.

**Checked against.** Closed forms of the Maxwell and standard linear solid (one ordinary differential equation each); the
recursive update is the exact integral of the hereditary law for piecewise-linear strain (Taylor, Pister and Goudreau, Int. J.
Numer. Meth. Engng 2 (1970) 45; Simo and Hughes, Computational Inelasticity, chapter 10; from memory, derived in the
registration).

**Files.** `crates/rcfd-fem/src/mat_visco.rs`, `crates/rcfd-fem/tests/sa_visco.rs`.

**Size.** M. **What could make it miss.** With a fixed step the effective stiffness is constant, so the linear integrator of ADR
0301 can carry the history as a load; if its interface cannot take a history force the dynamic clause runs through the nonlinear
driver quasi-statically and the vibration clause is not run (partial). No Prony series of any polymer is in the repository
(decision A1).

### ADR 0314 (SA-4): Creep: power law with time hardening, implicit integration with a consistent tangent

**Goal.** A plastic blade under centrifugal load grows slowly towards the housing. Add deviatoric creep with the rate A sigma_e^n
t^m (Norton for m = 0; an Arrhenius factor in temperature as input), integrated implicitly at the integration point with its
consistent tangent, driven by the Newton driver of ADR 0300 over time steps limited by a registered creep-strain increment.

**Dependencies.** ADR 0311; roadmap items: SA-1, F-397.

**Acceptance (draft).** The tangent agrees with central differences within 1e-6. Uniaxial constant stress: creep strain A sigma^n
t within 1e-8 for m = 0 and A sigma^n t^(m+1)/(m+1) within 0.1 percent for m = -0.5. Relaxation at fixed strain, n = 3 and 5:
sigma(t) = (sigma0^(1-n) + (n - 1) E A t)^(1/(1-n)) within 0.5 percent at the registered step count, observed order >= 0.9. Thick
cylinder b/a = 2 in plane strain under internal pressure, n = 4: the stress starts at Lame within 1 percent and tends to the
stationary field sigma_r = -p ((b/r)^(2/n) - 1)/((b/a)^(2/n) - 1), sigma_theta = p ((2 - n)/n (b/r)^(2/n) + 1)/((b/a)^(2/n) - 1)
within 2 percent after the registered redistribution time. Radial bar clamped on the axis of rotation, n = 3: the tip creeps at
the rate A times the integral of (rho Omega^2 (L^2 - r^2)/2)^n over the length within 0.5 percent (the stress is statically
determinate). A step whose creep-strain increment exceeds the registered limit is halved; a non-converged local iteration is an
error with the element.

**Checked against.** Closed forms: uniaxial integrals of the creep law; stationary creep of a cylinder, which is nonlinear
elasticity with exponent 1/n (Bailey; Finnie and Heller, Creep of Engineering Materials; from memory, re-derived in the
registration and equal to Lame at n = 1); the rotating bar by one integral.

**Files.** `crates/rcfd-fem/src/mat_creep.rs`, `crates/rcfd-fem/tests/sa_creep.rs`.

**Size.** S. **What could make it miss.** The redistribution time of the cylinder is an estimate (ratio of elastic strain to creep
rate). Primary creep of polymers is often better fitted by other laws; only this one is offered. No creep constant of any polymer
is in the repository.

### ADR 0315 (SA-5): Direct harmonic solve: complex symmetric factorisation, structural (hysteretic) damping, discrete dampers, base excitation

**Goal.** Mode superposition (ADR 0302) cannot take damping that differs between materials or depends on frequency. Add the direct
solve of (K (1 + i eta) - w^2 M + i w C) u = f per frequency: a complex symmetric L D L^T on the envelope in `rcfd-linalg`, loss
factors per material (constant, or a table of storage modulus and loss factor against frequency, which is how the complex modulus
of ADR 0313 enters), Rayleigh and discrete viscous dampers, pre-stress as an added stiffness, harmonic base motion, a sweep with
amplitude and phase of displacement and stress.

**Dependencies.** ADR 0311; roadmap items: SA-1, F-373, F-374, F-383, F-399.

**Acceptance (draft).** rcfd-linalg: a random complex symmetric diagonally dominant matrix of order 200 is solved to a residual of
1e-12 relative; a real matrix gives the real factorisation's solution within 1e-13; a pivot below the registered threshold is
refused with the equation. Fixed-free bar, nu = 0, loss factor 0.02, tip force: tip response F tan(k L)/(E* A k) with E* = E (1 +
i eta) and k = w sqrt(rho/E*), amplitude and phase within 0.5 percent at 0.5, 1.0 and 1.5 times the first frequency. With Rayleigh
damping the direct solve equals the mode superposition of F-399 with all modes within 1e-8 on a small model. Tip dashpot c on the
same bar: response F/(E A k cot(k L) + i w c) within 0.5 percent, and with c = A sqrt(E rho) the amplitude is F |sin(k L)|/(E A k)
(no resonance) within 0.5 percent at five frequencies. Base motion of the clamped end, undamped: tip amplitude 1/cos(k L) of the
base within 0.5 percent at half the first frequency. A table of modulus against frequency reproduces the constant-modulus result
when constant (1e-12). An undamped solve at a resonance is refused with the frequency; results are bit-identical for 1 and 8
threads.

**Checked against.** Closed forms: the wave equation of a bar with a complex modulus, with an end dashpot and with a moving base
(one line each). The factorisation is Gaussian elimination in symmetric form with complex numbers (no pivoting; stable here
because the damping makes the imaginary part definite: argued in the registration; Higham, Accuracy and Stability of Numerical
Algorithms, on complex symmetric systems, from memory).

**Files.** `crates/rcfd-linalg/src/cskyline.rs`, `crates/rcfd-fem/src/harmonic_direct.rs`,
`crates/rcfd-linalg/tests/sa_cskyline.rs`, `crates/rcfd-fem/tests/sa_harmonic_direct.rs`.

**Size.** M. **What could make it miss.** Without pivoting an undamped system close to a resonance loses digits: the refusal
threshold is a registered estimate. One factorisation per frequency: a sweep of a large model is slow (recorded). Constraints by
the transformation of ADR 0296 must carry complex right-hand sides.

### ADR 0316 (SA-6): High-cycle fatigue: stress-life curves, mean-stress corrections, harmonic and counted histories, linear damage

**Goal.** Turn stresses into life. From a static (mean) stress field and a harmonic stress field (blade-passing response on the
centrifugal state) or a stress history: stress-life curves (Basquin, or a table in log-log, with an optional endurance limit),
mean-stress corrections (Goodman, Gerber, Soderberg), equivalent stresses for proportional loading (signed von Mises, largest
principal; the amplitude of a harmonic tensor over the phase), rainflow counting, the Palmgren-Miner sum, the narrow-band damage
of a random stress from its spectral moments, and fields of life, damage rate and safety factor.

**Dependencies.** ADR 0311; roadmap items: SA-1, F-376, F-399.

**Acceptance (draft).** Curves and corrections reproduce N = (sigma_a/sigma_f)^(1/b)/2, sigma_a/(1 - sigma_m/sigma_u) (Goodman),
sigma_a/(1 - (sigma_m/sigma_u)^2) (Gerber) and Soderberg within 1e-12; a mean stress at or above the strength is refused. Harmonic
amplitude: for a normal and a shear component 90 degrees out of phase the von Mises amplitude equals the closed-form maximum over
the phase within 1e-10. Cantilever with a static and a harmonic tip force: the largest damage lies on the root surface and the
life there equals the hand value from 6 P L/(b h^2) within the registered factor that a 2 percent stress error allows. Plate with
a small circular hole (radius/width 0.05) under remote harmonic tension: stress amplitude at the hole edge within 3 percent of 3
sigma (Kirsch) and the life the Basquin life of that stress. Counting: n cycles of constant amplitude give n cycles of that range;
a large cycle carrying m small ripples gives 1 and m; a periodic history started at its maximum closes with no residual; a history
and its mirror image give the same counts; two load blocks give n1/N1 + n2/N2 within 1e-12. Narrow band: the damage rate nu0
(sqrt(2) s)^m Gamma(1 + m/2)/C within 1e-12 as a function, and the counted damage of a synthesised narrow-band history (registered
seed, 1e5 cycles) within 5 percent of it.

**Checked against.** Closed forms and definitions: Basquin (1910), Goodman, Gerber, Soderberg, Miner (1945); the hole in a plate
(Kirsch, in Timoshenko and Goodier); the damage of a Rayleigh distribution of peaks (Miles, 1954; one integral). Counting is
defined in the registration by the closing of hysteresis loops (Matsuishi and Endo, 1968; Downing and Socie, Int. J. Fatigue 4
(1982) 31; from memory) and checked on constructed histories; the standard that prints a worked example (ASTM E1049) is paid and
is not used.

**Files.** `crates/rcfd-fem/src/fatigue.rs`, `crates/rcfd-fem/tests/sa_fatigue.rs`.

**Size.** M. **What could make it miss.** No stress-life curve of any polymer is in the repository, and such curves depend on
temperature, frequency and fibre direction: every life is as good as its input (decision A1). Non-proportional multiaxial loading
(critical plane), strain-life and notch-sensitivity factors are not included. The stress at a root fillet needs a converged mesh
(ADR 0335 gives the estimate).

### ADR 0317 (SA-7): Damped and complex modes: a dense eigensolver for general real matrices and the quadratic eigenproblem on a modal subspace

**Goal.** Everything with a skew or damping matrix needs eigenvalues that the symmetric solver of ADR 0283 cannot give. Add to
`rcfd-linalg` the eigenvalues and vectors of a dense real general matrix (balancing, Hessenberg reduction, shifted QR), and on it
the quadratic problem (lambda^2 M + lambda (C + G) + K) x = 0 projected on a set of undamped modes (2m by 2m). In `rcfd-fem`:
damped modes with Rayleigh damping, discrete dampers and loss factors, giving decay rates, damped frequencies, damping ratios and
complex shapes. The gyroscopic packages (ADR 0318, 0319) use the same routine.

**Dependencies.** ADR 0311; roadmap items: SA-1, F-382, F-383.

**Acceptance (draft).** rcfd-linalg: the companion matrix of a polynomial with registered real and complex roots returns them
within 1e-10; a random matrix of order 50 has residuals |A v - lambda v| <= 1e-10 |A|; a symmetric matrix reproduces the Jacobi
values of F-382 within 1e-10; a Jordan block is returned with its eigenvalue within 1e-6 and a report, not a failure; no
convergence within the registered sweeps is an error. One oscillator: lambda = -zeta w +/- i w sqrt(1 - zeta^2) within 1e-12.
Rayleigh damping a M + b K on a solid model: every mode has the ratio (a/w_j + b w_j)/2 within 1e-8. Skew coupling: a two-unknown
system with G = g [[0, 1], [-1, 0]] has the closed-form split frequencies within 1e-12. Fixed-free bar with a tip dashpot c below
A sqrt(E rho): lambda_j = (c0/L) (-atanh(c/(A sqrt(E rho))) + i (2 j - 1) pi/2), the first three within 2 percent on 40 modes; at
c above that value no oscillating root is returned and the result says so. Damping ratios are -Re(lambda)/|lambda|; conjugate
pairs are reported once.

**Checked against.** Closed forms: the damped oscillator; proportional damping; the bar with a boundary dashpot (wave equation
with the condition E A u_x = -c u_t, giving tanh(lambda L/c0) = -A sqrt(E rho)/c). Method: Francis's QR iteration on a Hessenberg
matrix (Golub and Van Loan, Matrix Computations, chapter 7; from memory; checked by the residual test, not taken on trust).

**Files.** `crates/rcfd-linalg/src/eigen_general.rs`, `crates/rcfd-fem/src/damped.rs`,
`crates/rcfd-linalg/tests/sa_eigen_general.rs`, `crates/rcfd-fem/tests/sa_damped.rs`.

**Size.** M. **What could make it miss.** A boundary dashpot converges slowly in undamped modes (the 2 percent is a draft;
residual vectors may be needed). The QR iteration has known hard cases; the test matrices are registered, not chosen after a run.
Large models are reached only through the modal subspace, never directly.

### ADR 0318 (SA-8): Coriolis term of a rotating part: gyroscopic matrix in the rotating frame and the Campbell table with split modes

**Goal.** ADR 0299 neglects the Coriolis coupling (decision N5 of ADR 0290). Add the skew matrix G = 2 Omega int rho N^T [e x] N
dV of solid elements and the modes of lambda^2 M + lambda G + K_eff(Omega) = 0 on the modal subspace (ADR 0317), opt-in beside the
existing table: frequencies over speed with the forward and backward members of each split pair, tracked, with the engine-order
crossings. The default table keeps its bits.

**Dependencies.** ADR 0317; roadmap items: SA-7, F-396.

**Acceptance (draft).** G is skew within 1e-13 of its norm and x^T G x = 0 within 1e-12. Without the option the table has the bits
of F-396; with it at zero speed the frequencies equal those of F-396 within 1e-10. A mass on isotropic springs in the plane of
rotation (two unknowns): frequencies |w0 - Omega| and w0 + Omega within 1e-10; the same as a stiff block on elastic supports
within 1e-3. Thin ring spinning about its axis (thickness/radius 0.05, nu = 0, full model): the two travelling waves of in-plane
mode n differ in frequency by 4 n Omega/(n^2 + 1) within 2 percent for n = 2 and 3 at Omega = 0.1 w_2. Radial beam of square
section: the flap frequency equals that of F-396 within 1e-6 relative and the lag frequency moves by the registered second-order
estimate within 20 percent of that estimate. Crossings of split modes are tracked without exchange (modal assurance on the complex
shapes above 0.9). A speed at which an eigenvalue has a positive real part is reported as unstable with the speed.

**Checked against.** Closed forms: the plane oscillator in a rotating frame; the split of the travelling waves of an inextensional
ring (Bryan, Proc. Camb. Phil. Soc. 7 (1890) 101; recalled as the factor 4 n/(n^2 + 1), re-derived in the registration from the
ring's kinetic energy in the rotating frame). The matrix follows from the relative-acceleration term 2 Omega x v (first
principles).

**Files.** `crates/rcfd-fem/src/gyro.rs`, `crates/rcfd-fem/tests/sa_gyro.rs`.

**Size.** M. **What could make it miss.** The ring needs the full 360 degree model: cyclic symmetry with the Coriolis term
(complex sector problem with a skew part) is not included, so a bladed disc gets the term only as a full rotor. Bryan's factor is
recalled. The lag estimate of the beam is a perturbation result with a wide draft limit.

### ADR 0319 (SA-9): Flexible rotor on bearings in the stationary frame: whirl, critical speeds, unbalance response, joined to the rigid-rotor model

**Goal.** Fan breakdown item E2 has a rigid rotor on two bearings (`rcfd-post::rotordyn`, ADR 0120). Add the flexible one: a shaft
of beam elements with rotary inertia and the gyroscopic matrix of a spinning beam, rigid discs (mass, polar and transverse
inertia; from a solid model's mass matrix for the hub with blades), bearings with stiffness and damping matrices including
cross-coupling, and from them the whirl frequencies over speed with forward and backward labels (Campbell table in the stationary
frame), critical speeds, stability, and the response to unbalance. `rcfd-post::solid_rotor` converts to and from the rigid model's
data.

**Dependencies.** ADR 0317; roadmap items: SA-7, F-391, F-393.

**Acceptance (draft).** At zero speed the frequencies equal those of the beam model of F-391 within 1e-10. Disc on a light shaft
between two bearings: critical speed sqrt(k/m) and unbalance amplitude e r^2/sqrt((1 - r^2)^2 + (2 zeta r)^2) within 1e-3 at r =
0.5, 1 and 2. In the stiff-shaft limit the critical speeds and the response equal rotordyn::critical_speeds and rotordyn::response
for the same rotor within 1e-6. Uniform shaft on pins with rotary inertia, 20 elements: forward and backward whirl of modes 1 and
2 equal the roots of E I k^4 - rho A w^2 - rho I k^2 w^2 +/- 2 rho I k^2 Omega w = 0, k = n pi/L, within 0.5 percent at three
speeds. Overhung disc on a massless cantilever shaft: the four whirl speeds equal the roots of the two-unknown closed form
(derived in the registration) within 0.5 percent. Anisotropic bearings split the critical speed into sqrt(kx/m) and sqrt(ky/m)
within 1e-3. A cross-coupled stiffness k_c on a damped disc is stable below c w_n and unstable above, the threshold within 1
percent. Forward and backward are labelled by the sense of the orbit; an unstable speed is reported, never dropped.

**Checked against.** Closed forms: the Jeffcott rotor with and without cross-coupling; the spinning beam with rotary inertia (from
its kinetic energy); the overhung disc (Stodola; two-unknown flexibility model). The repository's rigid model (ADR 0120) is the
reference of the stiff-shaft limit. Element matrices: Nelson and McVaugh, J. Eng. Ind. 98 (1976) 593 (from memory; re-derived).

**Files.** `crates/rcfd-fem/src/rotor.rs`, `crates/rcfd-post/src/solid_rotor.rs`, `crates/rcfd-fem/tests/sa_rotor.rs`,
`crates/rcfd-post/tests/sa_solid_rotor.rs`.

**Size.** M. **What could make it miss.** Only the head of `rotordyn.rs` was read (rigid rotor, isotropic bearings, complex
coordinates); if its types do not map onto general bearings the join covers the isotropic case only. Bearing stiffness and damping
of a sleeve or ball bearing of a small fan are inputs: none is in the repository. Blade flexibility is not coupled to the shaft in
this package (discs are rigid).

### ADR 0320 (SA-10): Random vibration: response to force and base-acceleration spectra by modal superposition, root-mean-square stress

**Goal.** Broadband aerodynamic loading is a spectrum, not a harmonic. From the modes and the modal transfer functions of ADR
0302: the response spectral density at chosen nodes for a load pattern with a force spectrum (or a few patterns with a
cross-spectral matrix) and for a base acceleration spectrum; mean squares by integration on a grid refined at the peaks;
root-mean-square displacement, stress components and von Mises stress; spectral moments and the zero-crossing rate, which the
narrow-band fatigue estimate of ADR 0316 takes.

**Dependencies.** ADR 0311; roadmap items: SA-1, F-399.

**Acceptance (draft).** One mode under a flat one-sided force spectrum G: mean-square displacement (pi/2) f_n Q G/k^2 with Q =
1/(2 zeta), and under a flat base acceleration spectrum the mean-square absolute acceleration (pi/2) f_n Q G (for Q well above 1;
the exact integral is computed in the registration), both within 0.5 percent at zeta = 0.02 on the registered grid, observed
convergence under grid refinement; zero-crossing rate f_n within 1 percent. Two well separated modes: the mean square is the sum
within 1 percent. A band-limited spectrum equals the numerical integral registered beforehand within 1e-6. For a uniaxial stress
state the root-mean-square von Mises stress equals the root-mean-square stress within 1e-10. Two fully correlated patterns equal
one summed pattern within 1e-12; uncorrelated ones add in mean square. A spectrum with a negative value or a non-increasing
frequency list is refused with the entry.

**Checked against.** Closed form: the integral of |H|^2 of a single oscillator over frequency (residues; Crandall and Mark, Random
Vibration in Mechanical Systems; Miles, J. Aeronaut. Sci. 21 (1954) 753; from memory, derived). The root-mean-square von Mises
stress as a quadratic form of the stress covariance (first principles).

**Files.** `crates/rcfd-fem/src/random.rs`, `crates/rcfd-fem/tests/sa_random.rs`.

**Size.** S. **What could make it miss.** No force spectrum of the fan exists (broadband loads need a scale-resolving flow run);
the package is machinery. Response spectra (shock) are not included (ADR 0310, not planned).

### ADR 0321 (SA-11): Radiated sound of a vibrating surface: equivalent radiated power and the Rayleigh integral of a baffled plane

**Goal.** The housing's vibration is heard. From the harmonic velocity of a surface set (ADR 0302 or 0315): the equivalent
radiated power (rho c/2) sum A |v_n|^2 for any surface, as level spectra through `rcfd-post::acoustics`; and for a plane radiator
in a baffle the Rayleigh integral: pressure at field points, directivity, radiated power and radiation efficiency. No acoustic
field solve and no reaction of the air on the structure.

**Dependencies.** ADR 0311; roadmap items: SA-1, F-136, F-399.

**Acceptance (draft).** Equivalent radiated power: a rigid piston gives rho c S v^2/2 within 1e-12 and a plate mode sin sin gives
rho c A v0^2/8 within 0.5 percent. Rayleigh integral of a circular piston of radius a: on-axis pressure 2 rho c v |sin(k (sqrt(z^2
+ a^2) - z)/2)| within 1 percent for z from a to 20 a at k a = 2 and 10 with faces of a tenth of a wavelength; far-field
directivity |2 J1(k a sin theta)/(k a sin theta)| within 1 percent; radiated power over the equivalent radiated power equal to 1 -
J1(2 k a)/(k a) within 1 percent at k a = 0.5, 2 and 10, and at k a = 0.1 the power rho c k^2 (S v)^2/(4 pi) within 2 percent.
Levels: 1e-12 W gives 0 dB and the third-octave sums equal the narrow-band total within 1e-10. A surface that is not plane within
the registered tolerance is refused by the Rayleigh integral with its deviation; the equivalent radiated power accepts any surface
and its report says that it assumes a radiation efficiency of one.

**Checked against.** Closed forms of the baffled circular piston: on-axis pressure, directivity and radiation resistance
(Rayleigh, The Theory of Sound, section 278 and following; Kinsler and Frey, Fundamentals of Acoustics; from memory, derivable
from the Rayleigh integral). Levels and bands are those of `rcfd-post::acoustics` (ADR 0066).

**Files.** `crates/rcfd-post/src/solid_acoustic.rs`, `crates/rcfd-post/tests/sa_solid_acoustic.rs`.

**Size.** S. **What could make it miss.** A fan frame is not a baffled plane: for it only the equivalent radiated power is
available, which over-predicts below coincidence. Feeding the surface to the FW-H integral of F-136 was considered and left out:
the impermeable form needs the surface pressure of the radiated field, which is not known without an acoustic solve.

### ADR 0322 (SA-12): Three-node shell element (flat facet, assumed shear strain) for triangulated surfaces

**Goal.** The surface meshers of the repository produce triangles (decision N6 of ADR 0290). Add a three-node shell with six
unknowns per node on the conventions of ADR 0295: constant-strain membrane, Reissner-Mindlin bending with an assumed transverse
shear strain that is isotropic in the element and free of locking, the same drilling treatment, consistent mass, pressure and body
loads, resultants and surface stresses, and mixed meshes with the four-node shell.

**Dependencies.** ADR 0311; roadmap items: SA-1, F-391, F-392.

**Acceptance (draft).** Membrane patch test and constant-curvature bending patch test on an irregular flat mesh, and on a mesh
mixing Shell3 and Shell4, within 1e-8. Simply supported square plate under uniform load, 32 x 32 squares each cut in two, nu =
0.3: centre deflection within 2 percent of the Navier series at thickness/side 0.01 and within 3 percent at 0.001, and within 1
percent of each other for the two diagonal directions. Cantilever strip, nu = 0: tip deflection within 1 percent. Pinched ring
strip, 32 elements per quarter: loaded diameter within 3 percent of (pi/4 - 2/pi) P R^3/(E I). Cylinder under internal pressure:
radial displacement p R^2/(E t) within 1 percent. First frequency of the simply supported plate within 2 percent. A free plate has
six zero frequencies below 1e-6 of the seventh; a single element has exactly six zero eigenvalues; a triangle with an angle below
the registered limit is refused with the element.

**Checked against.** The closed forms of ADR 0295 (Navier series, Castigliano ring, membrane cylinder, plate frequencies). Method:
the three-node element with mixed interpolation of the shear strain (Lee and Bathe, Comput. Struct. 82 (2004) 945; from memory;
the tying is re-derived and checked by the patch and isotropy tests).

**Files.** `crates/rcfd-fem/src/shell3.rs`, `crates/rcfd-fem/tests/sa_shell3.rs`.

**Size.** M. **What could make it miss.** Triangles converge more slowly than quadrilaterals: every percentage is a draft. The
assumed-strain triangle is recalled, not derived here; if it shows a spurious mode the fallback is the discrete-Kirchhoff triangle
(thin plates only) by an amendment before the run.

### ADR 0323 (SA-13): Stress stiffening of beams and shells: buckling, pre-stressed modes and the Campbell table of a shell blade

**Goal.** ADR 0298 gives geometric stiffness to solid elements only, so a blade meshed as a shell has no Campbell table. Add the
geometric stiffness of the beam (from its axial force) and of both shells (from the membrane resultants), their centrifugal load
and spin-softening mass, and pass them through the existing pre-stressed modal analysis, buckling and rotating-part analysis.

**Dependencies.** ADR 0322; roadmap items: SA-12, F-391, F-392, F-395, F-396.

**Acceptance (draft).** Beam: Euler loads pi^2 E I/L^2 (pinned) and pi^2 E I/(4 L^2) (clamped-free) with the registered shear
correction within 0.1 percent at 8 elements; under tension w_n^2 = (n pi/L)^4 E I/(rho A) + (n pi/L)^2 T/(rho A) within 0.5
percent for n = 1, 2. Rotating radial beam of square section: w_flap^2 - w_lag^2 = Omega^2 within 1e-6 Omega^2 and the stiffening
slope equal to the Rayleigh quotient of F-396 within 1 percent. Shell: a simply supported square plate buckles at 4 pi^2 D/a^2 per
width in one direction and at 2 pi^2 D/a^2 under equal compression in both, within 2 percent at 16 x 16 Shell4 and 4 percent at 32
x 32 x 2 Shell3; a stretched strip has the tension frequencies above within 1 percent. Equivalence: a flat plate strip twisted by
30 degrees, thickness/chord 0.02, clamped at the root and rotating: the Campbell table of the first four modes as a shell equals
that of the solid model of F-396 (Hex20) within 2 percent at the registered speeds. Models of solid elements only keep their bits.

**Checked against.** Closed forms: Euler and Engesser; the beam and the strip under tension; buckling of the simply supported
plate (Timoshenko and Gere, Theory of Elastic Stability; derivable from the Navier series with in-plane forces); the flap-lag
identity and the perturbation slope of ADR 0299. The solid model is the reference of the equivalence.

**Files.** `crates/rcfd-fem/src/geometric_struct.rs`, `crates/rcfd-fem/src/rotating.rs` (only if its softening mass assumes
solid elements), `crates/rcfd-fem/tests/sa_geometric_struct.rs`.

**Size.** M. **What could make it miss.** At the upper end of M; split: beams first, shells second. The equivalence compares two
discretisations of a thin plate, both with their own error: the 2 percent is a draft. Large untwist is not covered (ADR 0327 gives
modes about a nonlinear state for solids only).

### ADR 0324 (SA-14): Aerodynamic damping and added mass of a mode by prescribed motion of the wall (energy method)

**Goal.** A flutter margin is the sign and size of the work the fluid does on a vibrating mode. Add the machinery: move the fluid
wall in a structural mode shape at a given amplitude and frequency (through the interface weights of F-362 and the mesh-motion
helper of ADR 0304), run the flow solver over registered cycles, and from the interface force history extract the in-phase part
(added mass or stiffness) and the quadrature part (work per cycle, aerodynamic damping ratio of the mode). Checked where the fluid
force has a closed form; the fan passage is not part of the acceptance (ADR 0310, not planned).

**Dependencies.** ADR 0311; roadmap items: SA-1, F-362, F-383, F-400, F-401.

**Acceptance (draft).** Force harmonics: for a synthetic force history with known in-phase and quadrature parts the extraction
returns them within 1e-10, and the work per cycle equals pi q0 times the quadrature part within 1e-10. Piston on a column of
incompressible fluid with an open end (the case of F-401): added mass rho_f A L_f within 2 percent and a damping ratio below the
registered viscous and time-step estimate. Piston driving a plane channel of gap h and length L_f: force per piston velocity equal
to A times the oscillating-channel impedance i w rho L_f/(1 - tanh(kappa)/kappa), kappa = (h/2) sqrt(i w/nu), real and imaginary
parts within 3 percent at Womersley numbers 1 and 5 after the registered extrapolation in the time step, and the low-frequency
limit 12 mu L_f/h^2 within 3 percent. The damping ratio of the bar on the open column from this method and from the decay of the
coupled run of F-401 are both recorded. A history that has not become periodic within the registered tolerance is refused with the
cycle-to-cycle change.

**Checked against.** Closed forms: plug flow of an incompressible column; laminar oscillating flow between plates (the
plane-channel form of Womersley's solution; one ordinary differential equation, derived in the registration). The energy method is
the work integral of the interface force over a cycle (Carta, J. Eng. Power 89 (1967) 419, from memory; first principles).

**Files.** `crates/rcfd-post/src/solid_aerodamp.rs`, `crates/rcfd-post/tests/sa_aerodamp.rs`.

**Size.** M. **What could make it miss.** The flow side is implicit Euler: its phase error turns about w dt/2 of the inertial
force into false damping, which at 200 steps per period is of the size of the physical damping at the higher Womersley number; the
registration must fix the step sizes and the extrapolation from that estimate before any run. Long runs (heavy record with a short
fast variant). A wall moving in its own plane is not covered by the protocol's tests.

### ADR 0325 (SA-15): J2 plasticity with isotropic and kinematic hardening: radial return and consistent tangent

**Goal.** The first material with history, moved here by decision N1 of ADR 0290. Small-strain von Mises plasticity with linear
isotropic and linear kinematic hardening (and a tabulated isotropic curve), integrated by radial return with the consistent
tangent, on the Newton driver and the state slot of ADR 0300; plastic strain and back stress as results.

**Dependencies.** ADR 0311; roadmap items: SA-1, F-389, F-397.

**Acceptance (draft).** The tangent agrees with central differences within 1e-6 in the elastic and in the plastic branch. Uniaxial
bar, bilinear: the stress follows sigma_y + (E H/(E + H)) (eps - sigma_y/E) within 1e-10 in one step and in 100; on reversal the
bar yields again at -sigma_max with isotropic and at sigma_max - 2 sigma_y with kinematic hardening within 1e-10. Thick sphere b/a
= 2, perfectly plastic, internal pressure: the pressure at plastic radius c equals 2 sigma_y ln(c/a) + (2 sigma_y/3)(1 - c^3/b^3)
within 1 percent at c = 1.25 a and 1.5 a, the solve converges at 0.98 of the limit pressure 2 sigma_y ln(b/a), and the residual
hoop stress at the bore after unloading equals the closed form within 2 percent. Beam of rectangular section in pure bending:
M/M_y = 1.5 - 0.5 (kappa_y/kappa)^2 within 1 percent up to kappa = 5 kappa_y. The last three residuals of each step show an order
>= 1.8; a halved step restores the state bit for bit; a non-positive yield stress and a softening curve are refused with the key.

**Checked against.** Closed forms: the bilinear bar; the partly plastic sphere, where Tresca and von Mises coincide (Hill, The
Mathematical Theory of Plasticity, 1950, chapter 5; from memory, re-derived from equilibrium and the yield condition); the
elastic-plastic moment of a rectangle. Method: Simo and Taylor, Comput. Meth. Appl. Mech. Engng 48 (1985) 101; Simo and Hughes,
Computational Inelasticity (from memory; the tangent is checked by differences).

**Files.** `crates/rcfd-fem/src/mat_j2.rs`, `crates/rcfd-fem/tests/sa_j2.rs`.

**Size.** M. **What could make it miss.** Isochoric plastic flow locks fully integrated elements: the checks use Hex20, and
whether the B-bar element of ADR 0293 is offered to the nonlinear driver is not known. Small strains only (the measure of the
total Lagrangian driver, valid for large rotation with small strain). Yield data of polymers are rate and temperature dependent:
not modelled.

### ADR 0326 (SA-16): Nonlinear transient dynamics: Newton iteration inside the Newmark and generalised-alpha step

**Goal.** Time integration of M a + C v + f_int(u) = f(t) for the total Lagrangian solids and the materials with history: the
schemes and the state of ADR 0301 with a Newton iteration per step on the driver of ADR 0300, step halving on divergence, energy
and momentum reports, and save and restore bit for bit so that a nonlinear solid can stand behind the participant interface.

**Dependencies.** ADR 0311; roadmap items: SA-1, F-397, F-398.

**Acceptance (draft).** At a load factor of 1e-6 the history equals the linear one of F-398 within 1e-9 relative. A stiff slender
bar pinned at one end and released from the horizontal under gravity swings with the period 2 pi sqrt(2 L/(3 g)) (2/pi) K(sin 45
degrees) (factor 1.1803, recomputed in the registration) within 1 percent, the total energy constant within 1e-3 of its swing over
five periods at the registered step. A free spinning elastic bar keeps its linear momentum within 1e-10 and its angular momentum
within 1e-6 relative over ten turns. Large axial oscillation of a bar element with nu = 0 from the stretch 1.2: period equal to
the quadrature of the energy integral (computed in the registration) within 0.5 percent. With the spectral radius 1 the
generalised-alpha result equals Newmark within 1e-10. A run split by save and restore has the bits of the unsplit run, with a
material state included. A step that does not converge after the registered halvings is an error with the time.

**Checked against.** Closed forms: the compound pendulum at large amplitude (complete elliptic integral); conservation of momentum
of a free body; the one-unknown oscillator with the St. Venant-Kirchhoff bar force (energy integral). Methods as ADR 0300 and ADR
0301.

**Files.** `crates/rcfd-fem/src/nl_transient.rs`, `crates/rcfd-fem/tests/sa_nl_transient.rs`.

**Size.** M. **What could make it miss.** The trapezoidal rule does not conserve energy in nonlinear problems and can blow up at
large steps: the 1e-3 is a draft tied to the registered step. A pin on a solid mesh is a line of constrained nodes. No
energy-momentum scheme, no contact in dynamics. The public beam tests of the flag benchmark are in ADR 0337, not here.

### ADR 0327 (SA-17): Follower pressure, arc-length continuation, and modes about a nonlinear equilibrium

**Goal.** Three additions to nonlinear statics, each through the scaffold's driver extension points: pressure that follows the
deformed surface, with its load stiffness; continuation along the load-displacement path with a spherical arc-length constraint,
through limit points; and the modal analysis with the tangent stiffness of a converged nonlinear state (for a rotating part: the
pre-stress including untwist) in place of the linear pre-stress of ADR 0298.

**Dependencies.** ADR 0311; roadmap items: SA-1, F-395, F-396, F-397.

**Acceptance (draft).** Follower pressure: the load stiffness agrees with central differences of the load within 1e-6. Thin ring
(thickness/radius 0.02, nu = 0) under internal pressure up to the stretch 1.5: p = E t (lambda^2 - 1)/(2 R) within 1 percent, the
dead pressure on the same mesh being the registered control (off by the factor lambda). Linear buckling of the ring under external
pressure with the load stiffness: 3 E I/R^3 within 2 percent; without it the registered control 4 E I/R^3 within 3 percent.
Arc-length: a two-bar truss of solid bars (nu = 0, height h, bar length L0) is followed to the apex displacement 2.2 h with P = E
A (h - w)(2 h w - w^2)/L0^3 within 1e-6 at the registered points and the limit load 2 E A h^3/(3 sqrt(3) L0^3) within 0.5 percent,
load control failing at the first limit point (control). Modes about a state: a clamped-clamped beam stretched by an end
displacement has the tension frequencies of F-395 within 1 percent, and at a small stretch the tangent modes equal the linear
pre-stressed modes within 1e-4; for the rotating radial beam at 0.2 w_0 they equal F-396 within 1e-4, and the difference at higher
speed is recorded.

**Checked against.** Closed forms: equilibrium of a thin ring in the deformed state; buckling of a ring under hydrostatic and
under constant-direction pressure (Timoshenko and Gere; the two factors 3 and 4 are recalled and re-derived); the two-bar truss
with Green-Lagrange strain (one line). Methods: Riks, Int. J. Solids Struct. 15 (1979) 529; Crisfield, Comput. Struct. 13 (1981)
55; the load stiffness of a pressure (Hibbitt, 1979); all from memory, derived in the registration.

**Files.** `crates/rcfd-fem/src/follower.rs`, `crates/rcfd-fem/src/arclength.rs`, `crates/rcfd-fem/tests/sa_follower.rs`,
`crates/rcfd-fem/tests/sa_arclength.rs`.

**Size.** M. **What could make it miss.** At the upper end of M; split: arc-length as its own S package. The load stiffness is
unsymmetric on a free edge; the symmetric part is used and the registration must say when that is exact. The two buckling factors
of the ring are recalled.

### ADR 0328 (SA-18): Hyperelastic materials (neo-Hookean, Mooney-Rivlin) with a mean-dilatation element for near incompressibility

**Goal.** Rubber mounts and soft gaskets: finite-strain elasticity from a strain-energy function. Compressible neo-Hookean and
Mooney-Rivlin forms with a volumetric penalty, second Piola-Kirchhoff stress and material tangent on the total Lagrangian driver,
and a mean-dilatation formulation of Hex8 (constant pressure and volume ratio per element) so that a bulk modulus thousands of
times the shear modulus does not lock.

**Dependencies.** ADR 0311; roadmap items: SA-1, F-390, F-397.

**Acceptance (draft).** The tangent agrees with central differences within 1e-6; a rotation of the deformed state leaves the
energy unchanged within 1e-10; at a strain of 1e-6 the stress equals linear elasticity with the same shear and bulk modulus within
1e-6 relative. With bulk/shear = 1e4: uniaxial nominal stress mu (lambda - lambda^-2) (neo-Hookean) and 2 (C1 + C2/lambda)(lambda
- lambda^-2) (Mooney-Rivlin) for stretches 0.7 to 3, equibiaxial mu (lambda - lambda^-5), simple shear tau = mu gamma with the
normal stress difference mu gamma^2, each within 0.1 percent after the registered compressibility correction. Thick spherical
shell B/A = 2, neo-Hookean, inflated to the inner stretch 1.5: pressure (mu/2) ((1/l_b^4 + 4/l_b) - (1/l_a^4 + 4/l_a)) with l_b^3
= 1 + (A/B)^3 (l_a^3 - 1) (recomputed by quadrature in the registration) within 1 percent with the mean-dilatation element, the
standard Hex8 on the same mesh being the registered locking control. A non-positive modulus is refused; an inverted element is an
error with the element.

**Checked against.** Closed forms of incompressible rubber elasticity: homogeneous deformations and the inflated sphere (Rivlin;
Green and Zerna, Theoretical Elasticity; Ogden, Non-linear Elastic Deformations; from memory, derivable from the strain energy).
Method: the three-field mean-dilatation element (Simo, Taylor and Pister, Comput. Meth. Appl. Mech. Engng 51 (1985) 177; from
memory).

**Files.** `crates/rcfd-fem/src/mat_hyper.rs`, `crates/rcfd-fem/src/nonlinear_md.rs`, `crates/rcfd-fem/tests/sa_hyper.rs`.

**Size.** M. **What could make it miss.** If the driver of ADR 0300 fixes the element's internal force to the standard
displacement form, the mean-dilatation element needs its own element routine (the scaffold's entry points allow it). Tetrahedra
are not given a locking-free form. A true mixed displacement-pressure element is not planned (ADR 0310).

### ADR 0329 (SA-19): Tied non-matching meshes, shell and beam joints to solids, non-matching cyclic boundaries and nodes on the axis

**Goal.** Parts meshed apart must be joined. A node-to-surface search (bucket grid, projection onto the face) that turns each node
of the fine side into a linear constraint on the face it lies on (ADR 0296); the joints of a shell edge and of a beam end to a
solid, where the rotation follows the solid's surface nodes; and the same interpolation for cyclic boundaries that do not match,
with the rule for nodes on the axis by harmonic index. This is bonded contact; the search is reused by ADR 0330.

**Dependencies.** ADR 0311; roadmap items: SA-1, F-391, F-392, F-393, F-394.

**Acceptance (draft).** With matching meshes the tied model equals the merged one within 1e-10. Flat non-matching interfaces 2:3
and 3:5 under uniaxial stress: far-end displacement within 0.1 percent and interface stress within the registered 3 percent (a
one-sided node-to-surface tie does not pass the patch test exactly), the error falling with an observed order >= 1 under
refinement. A cantilever cut in two and tied 2:3 (Tet10 to Hex20): tip deflection within 1 percent of the closed form. The
constraint forces of a tie have zero resultant and moment within 1e-10. A node farther from every face than the registered
tolerance is refused with its id. Joints: a beam continuing a solid bar and a shell continuing a solid plate give the tip
deflection of the one-piece cantilever within 1 and 2 percent. Cyclic: the ring sector of F-394 with a 2:3 boundary has its
frequencies within 0.5 percent of the matching sector; a solid disc with nodes on the axis, spinning (index 0): centre stress (3 +
nu) rho Omega^2 b^2/8 within 1 percent; a rod on the axis at index 1 has the bending frequency of the beam within 1 percent, and
at index 2 its axis nodes are held.

**Checked against.** The merged or matching model is the reference of the equivalences; closed forms: the cantilever, the rotating
solid disc in plane stress (Timoshenko and Goodier), the beam frequency. That a one-pass tie fails the patch test is known (Taylor
and Papadopoulos, 1991, from memory); the limit is registered from an estimate of the gap error.

**Files.** `crates/rcfd-fem/src/tie.rs`, `crates/rcfd-fem/src/cyclic.rs` (additions), `crates/rcfd-fem/tests/sa_tie.rs`.

**Size.** M. **What could make it miss.** At the upper end of M; split: ties first, joints and cyclic extensions second. Curved
interfaces leave gaps and overlaps that a mortar method would treat properly; mortar ties are not planned. Many constraints fill
the reduced matrix (reported).

### ADR 0330 (SA-20): Frictionless contact: node-to-surface with an augmented Lagrangian, rigid and deformable targets, interference fits

**Goal.** Unilateral contact as an extra contribution to the Newton driver: the search of ADR 0329 repeated as the state moves, a
gap per node of the contactor against a deformable surface or a rigid plane, cylinder or sphere, a penalty with augmentation of
the multipliers until the registered penetration is met, an initial gap or interference, and contact pressure as a result.

**Dependencies.** ADR 0329; roadmap items: SA-19, F-397.

**Acceptance (draft).** A bar pushed against a rigid stop across a gap g: the piecewise-linear force-displacement closed form
within 1e-8, the penetration after augmentation below 1e-8 L. Two stacked blocks with matching meshes under pressure p: uniform
stress and contact pressure p within 1e-8; pulled apart they separate with no tensile contact force (every multiplier >= 0).
Elastic sphere on a rigid plane: contact radius (3 F R/(4 E*))^(1/3), peak pressure 3 F/(2 pi a^2) and approach a^2/R within 5, 5
and 3 percent at a/R = 0.1 with at least ten nodes across the contact radius. Interference fit of a ring (outer radius b) on a
solid shaft (radius a) of the same material with radial interference d, plane stress: contact pressure E d (b^2 - a^2)/(2 a b^2)
within 2 percent, and the hoop stress of the ring that of Lame under that pressure within 2 percent. The contact tangent agrees
with central differences within 1e-6 in a fixed active set; the last residuals of a step with a settled active set show an order
>= 1.8; an active set that cycles beyond the registered count is an error with the nodes.

**Checked against.** Closed forms: springs with a gap; Hertz's sphere on a plane (Johnson, Contact Mechanics, chapter 4; from
memory; derivable); the shrink fit from Lame's cylinder (one line, re-derived in the registration). Method: penalty with
augmentation of the multipliers (Simo and Laursen, Comput. Struct. 42 (1992) 97; Wriggers, Computational Contact Mechanics; from
memory).

**Files.** `crates/rcfd-fem/src/contact.rs`, `crates/rcfd-fem/tests/sa_contact.rs`.

**Size.** M. **What could make it miss.** Hertz with node-to-surface contact converges slowly and quadratic elements give uneven
nodal contact forces: the percentages are drafts and the check uses linear hexahedra. Friction, self-contact and contact in
dynamics are not planned.

### ADR 0331 (SA-21): Laminated shell: layup stiffness, ply stresses and failure indices

**Goal.** A composite or skin-core blade as a layered shell: plies with a material (ADR 0312), a thickness and an angle; the
membrane, coupling and bending stiffness and the transverse shear stiffness of the layup through the shell's section hook; thermal
resultants; stresses and strains at the top and bottom of each ply in its material axes; failure indices and reserve factors by
the maximum-stress, maximum-strain, Tsai-Hill and Tsai-Wu criteria.

**Dependencies.** ADR 0312; roadmap items: SA-2, F-392.

**Acceptance (draft).** One isotropic ply gives the bits of the Shell4 of F-392. Layup stiffness: a symmetric layup has no
coupling (|B| <= 1e-12 |A| t); [0/90]s has A11 = A22 = t (Q11 + Q22)/2; [0/+60/-60]s and [0/45/-45/90]s have A11 = A22, A66 = (A11
- A12)/2 and A16 = A26 = 0, all within 1e-12 relative. Simply supported square plate of a cross-ply symmetric layup under a
sinusoidal load: centre deflection q0 a^4/(pi^4 (D11 + 2 (D12 + 2 D66) + D22)) within 1 percent at thickness/side 0.01 with 16 x
16 elements. Unsymmetric [0/90] strip heated uniformly: curvature equal to the closed form of the two-ply laminate (solved by hand
in the registration) within 0.5 percent with zero resultants. Off-axis coupon at 15, 30, 45 and 60 degrees: ply stresses sigma
c^2, sigma s^2 and -sigma s c within 1e-10, Tsai-Hill failure stress (c^4/X^2 + (1/S^2 - 1/X^2) s^2 c^2 + s^4/Y^2)^(-1/2) within
1e-10, and Tsai-Wu equal to the tensile and compressive strengths on the axes within 1e-12. A layup with a non-positive thickness
or an unknown material is refused with the ply.

**Checked against.** Classical lamination theory and its closed forms (Jones, Mechanics of Composite Materials; Reddy, Mechanics
of Laminated Composite Plates and Shells, for the Navier solution; from memory, all derivable); the criteria are algebraic
definitions (Tsai and Wu, J. Compos. Mater. 5 (1971) 58).

**Files.** `crates/rcfd-fem/src/laminate.rs`, `crates/rcfd-fem/tests/sa_laminate.rs`.

**Size.** M. **What could make it miss.** The transverse shear stiffness of a layup needs a correction factor that depends on the
layup (registered rule; thin plates in the checks). No interlaminar stress, no delamination, no progressive failure. Strengths and
the fibre pattern of a moulded blade are inputs that the repository does not have. Shell3 takes the layup only if ADR 0322 is
merged at the start.

### ADR 0332 (SA-22): Fracture parameters: J-integral by the domain form, stress intensity factor, crack-growth life

**Goal.** Given a crack in the mesh (a slit of doubled nodes with a straight front), the energy release rate along the front by
the equivalent domain integral over rings of elements, the mode-I stress intensity factor from it and from the opening
displacement behind the front, and the number of cycles for a crack to grow between two lengths by the Paris law. A builder makes
a plate with a slit. Stationary cracks in linear elasticity only.

**Dependencies.** ADR 0311; roadmap items: SA-1, F-374, F-376, F-389.

**Acceptance (draft).** Centre crack of half-length a in a plate of half-width W = 10 a under tension, plane strain (one layer,
held in z), Hex20: K_I from J within 2 percent of sigma sqrt(pi a) sqrt(sec(pi a/(2 W))), the values of rings 2 to 4 within 1
percent of each other, and J equal to the change of potential energy between two models with half-lengths a +/- 0.02 a within 2
percent. K_I from the opening displacement within 5 percent. Penny-shaped crack of radius a in a cylinder of radius 10 a under
tension (heavy record): K_I = 2 sigma sqrt(a/pi) within 3 percent, uniform along the front within 2 percent. Under pure shear
loading of the centre crack the mode-I factor from the opening is zero within 1e-8 of the tension value. Growth: the cycles from
a0 to a1 by the Paris law with K = d sigma sqrt(pi a) equal the closed-form integral for m = 3 and the logarithm for m = 2 within
1e-6. A front that is not straight within the registered tolerance and a ring that reaches the boundary are refused.

**Checked against.** Closed forms: the centre crack in an infinite plate with the secant width correction (Feddersen's form, an
approximation whose error at this ratio is below the limit: stated and bounded in the registration), the penny-shaped crack
(Sneddon, 1946), the identity J = -dPi/da. Method: Li, Shih and Needleman, Eng. Fract. Mech. 21 (1985) 405; Shih, Moran and
Nakamura, Int. J. Fract. 30 (1986) 79 (from memory; checked by path independence and the energy difference).

**Files.** `crates/rcfd-fem/src/fracture.rs` (with the slit builder), `crates/rcfd-fem/tests/sa_fracture.rs`.

**Size.** M. **What could make it miss.** The mesh validation of the basic tier may refuse coincident nodes, which a slit needs.
Curved fronts, mixed-mode separation, crack growth with remeshing and elastic-plastic fracture are not planned. Toughness and
growth constants are inputs.

### ADR 0333 (SA-23): Substructuring: static condensation and fixed-interface component mode synthesis (Craig-Bampton)

**Goal.** Reduce a part to its interface: static condensation (Guyan) and the Craig-Bampton reduction (constraint modes plus a
number of fixed-interface modes) written as a transformation on the machinery of ADR 0296; reduced stiffness and mass as plain
data that can be saved and assembled with other parts and with ordinary elements; recovery of the interior displacements and
stresses. A housing with a rotor, or a blade row from one reduced blade, is the use.

**Dependencies.** ADR 0311; roadmap items: SA-1, F-382, F-383, F-393.

**Acceptance (draft).** Statics: the condensed model gives the retained displacements and, after recovery, the interior ones and
the stresses of the full model within 1e-10. Keeping every fixed-interface mode reproduces all frequencies of the full model
within 1e-8 (small model). A bar of two substructures with five modes each: the first three frequencies of the assembly within 0.1
percent of the full model and not below it (1e-10), the error falling monotonically with the number of modes. The reduced matrices
are symmetric within 1e-13 and the reduced mass is positive definite; a free substructure keeps six zero frequencies below 1e-6 of
the seventh. Harmonic response of the assembly equals the full model within 1 percent up to half the lowest omitted
fixed-interface frequency. A reduced part written to its file and read back gives the same bits. An interface set with no nodes
and a mode count above the interior size are refused.

**Checked against.** The full model is the reference. The reduction is a Rayleigh-Ritz transformation, so the frequencies are
upper bounds (first principles). Guyan, AIAA J. 3 (1965) 380; Craig and Bampton, AIAA J. 6 (1968) 1313 (from memory).

**Files.** `crates/rcfd-fem/src/cms.rs`, `crates/rcfd-fem/tests/sa_cms.rs`.

**Size.** M. **What could make it miss.** The reduced matrices are dense in the interface unknowns: a large interface defeats the
purpose (reported). Free-interface methods, reduction with damping or gyroscopic terms and mistuning studies on reduced blades are
not included.

### ADR 0334 (SA-24): Sparse L D L^T with a fill-reducing ordering, and its use by the static, buckling and modal solves

**Goal.** The envelope solver of ADR 0274 stops at a few times 1e4 unknowns in three dimensions. Add to `rcfd-linalg` a minimum
degree ordering on the quotient graph, the elimination tree, a symbolic and a numeric L D L^T with the count of negative pivots
(inertia), and solves for many right-hand sides; selectable in the static solve and accepted by the eigensolvers as the
factorisation. The default solver is unchanged.

**Dependencies.** ADR 0311; roadmap items: SA-1, F-373, F-374, F-382, F-402.

**Acceptance (draft).** Five-point grid 100 x 100 and seven-point grid 30^3: residual below 1e-12 relative, and on the 30^3 grid
at most half the factor entries of the envelope after reverse Cuthill-McKee (both counts recorded). On a solid model the solution
equals the envelope solver's within 1e-10 and the modal frequencies through either factorisation agree within 1e-9. Inertia: for
the tridiagonal (2, -1) of order 200 and three shifts the number of negative pivots of K - s I equals the number of eigenvalues 4
sin^2(j pi/(2 (n + 1))) below s. A matrix that is not positive definite is reported with the equation when positive definiteness
was requested. Factor and solution are bit-identical for 1 and 8 threads. Heavy record: a clamped cube of about 3e5 unknowns is
solved, time and memory recorded beside the envelope's estimate. The default solver's results keep their bits.

**Checked against.** The residual, the envelope solver and Sylvester's law of inertia are the references. Methods: George and Liu,
Computer Solution of Large Sparse Positive Definite Systems (minimum degree, elimination tree); Liu, ACM Trans. Math. Softw. 11
(1985) 141; Davis, Direct Methods for Sparse Linear Systems (from memory; the algorithm is checked by the residual and the fill
count).

**Files.** `crates/rcfd-linalg/src/sparse_chol.rs`, `crates/rcfd-fem/src/solve.rs` (the choice),
`crates/rcfd-linalg/tests/sa_sparse_chol.rs`, `crates/rcfd-fem/tests/sa_sparse_solve.rs`.

**Size.** M. **What could make it miss.** A plain minimum degree without supernodes is slow beyond about 1e6 unknowns and its fill
is worse than nested dissection: the factor-of-two clause is a draft. If the eigensolver of ADR 0283 is written against the
envelope type, one function is appended to `eigen.rs` (then this package must not run beside another that edits it).

### ADR 0335 (SA-25): Stress recovery by element patches and an energy-norm error estimate

**Goal.** How far is the stress from converged? Recover nodal stresses by a least-squares fit of the integration-point values over
the patch of elements around each node, and from the difference to the raw stress an element and a global error in the energy
norm, a relative error, and a refinement indicator per element as data. No mesh is changed.

**Dependencies.** ADR 0311; roadmap items: SA-1, F-376, F-378.

**Acceptance (draft).** A linear displacement field gives the exact stress and a zero estimate within 1e-10. On the manufactured
solution of F-378 for Hex8, Tet10 and Hex20: the effectivity index (estimate over true error in the energy norm) lies in [0.8,
1.2] on the three finest meshes and its distance from 1 does not grow under refinement; the recovered stress of Hex8 converges in
L2 at an observed order >= 1.7 where the raw stress shows >= 0.9. Plate with a small circular hole under tension: the recovered
peak stress is within 2 percent of 3 sigma on the mesh on which the averaged nodal stress of F-376 is recorded, and the tenth of
the elements with the largest indicator contains the elements at the hole edge. Boundary and corner nodes take the patch of the
nearest interior node by the registered rule; the result is bit-identical for 1 and 8 threads; a patch with too few points is
reported, not solved.

**Checked against.** The manufactured solution of ADR 0279 (true error known) and Kirsch's hole. Method: Zienkiewicz and Zhu, Int.
J. Numer. Meth. Engng 33 (1992) 1331 and 1365 (from memory; the effectivity is measured, not assumed).

**Files.** `crates/rcfd-fem/src/estimate.rs`, `crates/rcfd-fem/tests/sa_estimate.rs`.

**Size.** S. **What could make it miss.** Superconvergence of the patch recovery is not guaranteed on tetrahedra: the effectivity
band is a draft. Shells and beams are not covered. Adaptation is not planned (ADR 0310).

### ADR 0336 (SA-26): Elasticity solves on the ranks of rcfd-dist and on the CUDA device

**Goal.** Use the two parallel layers the repository has for the static solve of a solid: the stiffness matrix split by nodes over
the ranks of `rcfd-dist` (ADR 0058: in-process ranks only) and solved by its distributed CG with the block-Jacobi preconditioner;
and the CG of `rcfd-gpu` (ADR 0038) with Jacobi or Chebyshev on the device, chosen by the existing device setting. Assembly stays
serial. The glue lives in `rcfd-api`, which gains the dependency on `rcfd-dist`; the default solver is untouched.

**Dependencies.** none in this list; roadmap items: F-016, F-101, F-374, F-381.

**Acceptance (draft).** Ranks: with one rank the iteration counts and the solution have the bits of the serial CG of F-381 with
the same preconditioner; with 2, 4 and 8 ranks the solution of a clamped cube (about 4e4 unknowns) and of a cantilever of Tet10
equals the serial one within 1e-8 relative at a residual of 1e-10, in at most twice the iterations of one rank (counts recorded);
repeated runs with the same rank count give the same bits; the three unknowns of a node are never split; a rank that fails or
times out ends the solve with a located error. Scaling is recorded, not asserted. Device: with a usable device the device CG
equals the CPU CG with the same preconditioner within 1e-9 on a cube of at least 3e5 unknowns, times recorded; `auto` below the
threshold runs on the CPU; without a device `cuda` is a located error and `auto` falls back with the reason recorded, and the
device clauses are reported as not run. The default solver's results and the baseline hashes are unchanged.

**Checked against.** The serial solution is the reference (as ADR 0058 and ADR 0038 did for the flow matrices). Preconditioned CG
and block-Jacobi as additive Schwarz without overlap (Saad, Iterative Methods for Sparse Linear Systems).

**Files.** `crates/rcfd-api/Cargo.toml` (dependency on `rcfd-dist`), `crates/rcfd-api/src/solid_parallel.rs`,
`crates/rcfd-api/src/lib.rs` (one `pub mod` line), `crates/rcfd-api/tests/sa_solid_parallel.rs`.

**Size.** M. **What could make it miss.** `rcfd_mesh::partition::Decomposition` is built from a finite-volume mesh (cells); a
solid needs a split of nodes, so the package may have to build the rank rows itself (`DistCsr::from_rows`). Jacobi on elasticity
converges slowly: the device is unlikely to pay below about 1e6 unknowns (recorded, not asserted). No device on the machine: the
device clauses are not run and the item is partial. A real transport (MPI) is F-100 and is not structural work.

### ADR 0337 (SA-27): Public coupled benchmark (the elastic flag behind a cylinder): structural tests CSM1 to CSM3 and the steady coupled case FSI1 (needs the maintainer: reference values)

**Goal.** The one public benchmark that the earlier tiers named (Turek and Hron, 2006). Its structural part: the flag alone under
gravity, St. Venant-Kirchhoff, plane strain, static (CSM1, CSM2) and dynamic (CSM3). Its steady coupled part: the flag in channel
flow at Reynolds number 20 (FSI1), run to a steady state with the static participant, after the flow-only case with a rigid flag
(CFD1) as a gate. The published values must first be recorded in `docs/reference-data.md` from the public paper: house rule 6 does
not let an implementation agent take a data set the repository lacks, so this needs the maintainer (decision A2).

**Dependencies.** ADR 0326; roadmap items: SA-16, F-385, F-397, F-401.

**Acceptance (draft).** NEEDS THE MAINTAINER'S APPROVAL to record the published values in docs/reference-data.md before it starts;
no limit below uses a value recalled from memory. Structure alone (plane strain, one element thick): the tip displacement of CSM1
and CSM2 within 1 percent of the published values, both components; CSM3 (released under gravity, undamped): mean, amplitude and
frequency of both tip components within 2 percent, on two meshes with the change recorded. Flow alone with a rigid flag (CFD1):
drag within 2 percent and lift within 10 percent of the published values (gate; if it misses, the coupled case is not run and the
item is partial). Coupled steady case FSI1: drag within 2 percent, tip displacement in x within 5 percent and in y within 10
percent, lift within 10 percent, with the coupling converged to 1e-6 in the final state and the iteration counts recorded. The
unsteady cases FSI2 and FSI3 are not run.

**Checked against.** Turek and Hron, Proposal for numerical benchmarking of fluid-structure interaction between an elastic object
and laminar incompressible flow, in Fluid-Structure Interaction, Lecture Notes in Computational Science and Engineering 53,
Springer (2006) 371; public as a report of TU Dortmund. Not read for this plan; the values are not quoted here on purpose.

**Files.** `docs/reference-data.md` (rows, after approval), `crates/rcfd-fem/tests/sa_flag_csm.rs`,
`crates/rcfd-post/tests/sa_flag_fsi1.rs`, a case folder under `cases/` for the channel mesh.

**Size.** M. **What could make it miss.** The riskiest package of the tier. A two-dimensional body-fitted mesh of the cylinder
with the flag from the repository's generators is not known to exist. The lift of FSI1 is a small number that the paper's own
participants scatter on. The flow side's accuracy on CFD1 is untested here. FSI2 and FSI3 need large mesh motion and a nonlinear
participant over thousands of coupled steps: not planned.

### ADR 0338 (SA-28): Solid case file: the materials, analyses and results of the advanced tier

**Goal.** Make the fan-relevant part of the tier usable without writing Rust, and through the tools of ADR 0308 without a new
tool: the solid case file gains the material variants (orthotropic with axes, tables against temperature, Prony series, creep),
the analysis kinds direct harmonic, rotating modal with the Coriolis term, creep over time, random response, and the result
requests fatigue (curve, correction, mean and alternating load cases) and radiated power. Every key is optional; files of the
earlier tiers run as before.

**Dependencies.** ADR 0312, ADR 0313, ADR 0314, ADR 0315, ADR 0316, ADR 0318, ADR 0320, ADR 0321; roadmap items: SA-2, SA-3, SA-4,
SA-5, SA-6, SA-8, SA-10, SA-11, F-403.

**Acceptance (draft).** Each new analysis run from a file gives the bits of the same model run by library calls (limit 1e-12,
expected identical), and one closed-form check of each library package passes from files: the off-axis block (ADR 0312), the
relaxation curve (ADR 0313), the rotating creeping bar (ADR 0314), the bar with a loss factor (ADR 0315), the cantilever life (ADR
0316), the plane oscillator's split (ADR 0318), the single-mode mean square (ADR 0320), the piston's equivalent radiated power
(ADR 0321). An unknown key, a material that lacks a required constant, a fatigue request without a mean or an alternating case, a
table that is not increasing and an analysis that names a missing set are each refused with the key and the line. Every solid case
file of the tests of the earlier tiers gives the same report and the same .vtu bytes as before. No flow case key, CLI help text,
MCP tool list, hash table or golden changes (the existing schema, CLI and tool-list tests pass unedited).

**Checked against.** The closed forms of the library packages, re-run through the file, and the bit comparison with library calls.
ADR 0287 and ADR 0306 are the models.

**Files.** `crates/rcfd-api/src/solid_case.rs`, `crates/rcfd-api/tests/sa_solid_case.rs`, `docs/structural-solver.md` (the file
format section).

**Size.** M. **What could make it miss.** Eight dependencies: if one ends partial its keys are left out and this package is
partial with it. The other packages of the tier (plasticity, contact, laminates, fracture, substructures, rotor, nonlinear
dynamics) are parsed and checked only if merged when this one starts; what is left over is library-only and the page says so.

### ADR 0339 (SA-29): Demonstration on the 120 mm fan: a polymer blade (orthotropy, temperature, creep, fatigue at a Campbell crossing) and the radiated power of the frame

**Goal.** Bring the tier to what fan breakdown item E5 asks of a material choice: the blade of ADR 0309 with an orthotropic,
temperature-dependent polymer (assumed constants, named as assumed), its Campbell table with the Coriolis term, the creep growth
of the tip towards the housing at speed, the fatigue safety factor at a Campbell crossing under a unit blade-passing pressure
harmonic on the centrifugal mean stress, and a simplified frame (a square plate with the rotor opening on four mounts) driven by
the rotor's unbalance and blade-passing forces, with its equivalent radiated power. Consistency and mesh change only: nothing is
validated.

**Dependencies.** ADR 0338; roadmap items: SA-2, SA-4, SA-5, SA-6, SA-8, SA-11, SA-28, F-406.

**Acceptance (draft).** Blade from solid case files. With isotropic constants and no temperature the Campbell table has the bits
of F-406. A uniform temperature at which the table gives 0.8 E scales every frequency at rest by sqrt(0.8) within 1e-8. For the
orthotropic blade the change of each squared frequency at rest for a 1 percent change of E1 equals the Rayleigh-quotient
sensitivity within 1 percent. With the Coriolis term the first four frequencies at the maximum speed differ from those without by
less than the registered bound, both recorded. Creep at the maximum speed over the registered time: tip growth monotone, its
initial rate within the registered factor of the one-dimensional estimate, change between two meshes below 5 percent. Fatigue at a
registered crossing: the safety-factor field is written, the largest alternating stress changes by less than 10 percent between
two meshes and lies in the same region, and doubling the pressure harmonic changes the Goodman factor as the formula says within
1e-8. Frame: the equivalent radiated power spectrum at the rotation and blade-passing frequencies is written and equals (rho c/2)
sum A |v_n|^2 recomputed from the output within 1e-10. The page docs/structural-solver.md lists every assumed input and states
that no material datum, life, creep growth or sound level is validated against a measurement.

**Checked against.** Consistency only: the earlier demonstration (bits), the scaling of frequencies with the modulus, first-order
perturbation, mesh change. No reference value exists for the blade or the frame (docs/fan-project-breakdown.md, part G; decision
A1).

**Files.** `crates/rcfd-api/tests/sa_fan_polymer.rs`, `docs/structural-solver.md`, the E5 status text of
`docs/fan-project-breakdown.md`.

**Size.** M. **What could make it miss.** It inherits the risks of ADR 0288 and ADR 0309 (the blade mesh). The repository has no
frame geometry (fan breakdown A1 is partial), so the frame is a stand-in. Every engineering number scales with assumed material
data.

## 4. Critical path and parallel work

Dependencies inside the tier (an arrow means "must be merged before"):

```text
0311 -> 0312 -> 0331
0311 -> 0313, 0314, 0315, 0316 ------------------------> 0338 -> 0339
0311 -> 0317 -> 0318 ----------------------------------> 0338 -> 0339
        0317 -> 0319
0311 -> 0320, 0321 ------------------------------------> 0338
0311 -> 0322 -> 0323
0311 -> 0324
0311 -> 0325, 0327, 0328, 0332, 0333, 0334, 0335
0311 -> 0326 -> 0337 (maintainer)
0311 -> 0329 -> 0330
0336 (needs only the basic tier and the two parallel crates)
```

From the earlier tiers: 0311 needs F-372, F-388 and F-397 merged, so the tier starts when the intermediate tier's waves 1 to 3 are
merged; 0322, 0329 and 0331 need F-392 (the shell, wave 4 there), 0324 and 0337 need F-401 (the coupled piston), 0338 needs F-403,
0339 needs F-406.

**Critical paths:**

* the fan (five packages): 0311 -> 0317 -> 0318 -> 0338 -> 0339. The first life number of a blade under harmonic load exists after
  0316, in wave 2; the first Campbell table with the Coriolis term after 0318.
* the public benchmark (three packages and a decision): 0311 -> 0326 -> 0337.
* assemblies: 0311 -> 0329 -> 0330.

**Waves.** Within a wave no two packages edit the same file; each wave starts when the packages it depends on are merged.

| Wave | Packages in parallel | Files, by package |
|---|---|---|
| 1 | 0311, 0336 | 0311: `rcfd-fem` `lib.rs`, `model.rs`, `material.rs`, `mesh.rs`, `element.rs`, `shell.rs`, `nonlinear.rs`, stubs; `rcfd-linalg` `lib.rs`, `complex.rs`, stubs; `rcfd-post` `lib.rs`, stubs. 0336: `rcfd-api` `Cargo.toml`, `lib.rs`, `solid_parallel.rs` |
| 2 | 0312, 0313, 0314, 0315, 0316, 0317 | `mat_ortho.rs`, `mat_table.rs`; `mat_visco.rs`; `mat_creep.rs`; `rcfd-linalg` `cskyline.rs`, `harmonic_direct.rs`; `fatigue.rs`; `rcfd-linalg` `eigen_general.rs`, `damped.rs` |
| 3 | 0318, 0319, 0320, 0321, 0322, 0324 | `gyro.rs`; `rotor.rs`, `rcfd-post` `solid_rotor.rs`; `random.rs`; `rcfd-post` `solid_acoustic.rs`; `shell3.rs`; `rcfd-post` `solid_aerodamp.rs` |
| 4 | 0323, 0325, 0326, 0329, 0331, 0338 | `geometric_struct.rs`, `rotating.rs`; `mat_j2.rs`; `nl_transient.rs`; `tie.rs`, `cyclic.rs`; `laminate.rs`; `rcfd-api` `solid_case.rs`, `docs/structural-solver.md` |
| 5 | 0327, 0328, 0330, 0332, 0333, 0334, 0339 | `follower.rs`, `arclength.rs`; `mat_hyper.rs`, `nonlinear_md.rs`; `contact.rs`; `fracture.rs`; `cms.rs`; `rcfd-linalg` `sparse_chol.rs`, `rcfd-fem` `solve.rs`; test file, `docs/structural-solver.md`, `docs/fan-project-breakdown.md` |
| 6 | 0335, 0337 (once approved) | `estimate.rs`; `docs/reference-data.md`, test files, a case folder |

Notes on the table:

* Waves 2 to 5 are six or seven builds at once on a shared machine. If that is too many, the fan path goes first in each: 0315,
  0316, 0317 and 0312 in wave 2; 0318 and 0321 in wave 3; 0338 in wave 4; 0339 in wave 5.
* 0325, 0327, 0328, 0332, 0333, 0334 and 0335 depend only on the scaffold and the earlier tiers; they are placed late by priority,
  not by dependency, and any of them can move forward into a free slot. 0335 in particular is small and independent.
* 0334 edits `rcfd-fem/src/solve.rs`, which ADR 0305 (F-402) also edits: F-402 is a dependency for that reason only.
* 0338 and 0339 both write to `docs/structural-solver.md`, in different waves.
* 0331 gives Shell3 a layup only if 0322 is merged when it starts; 0338 takes the packages that are merged when it starts.
* Each package adds its own test files (`sa_*.rs`); none edits another's, or an `sb_` or `si_` file.
* Tests-only findings, as in the earlier tiers: a defect found in a file the package does not own is recorded and reported; the
  fix is a separate change by the lead's choice.

## 5. For the maintainer: decisions

Each with options and a recommendation. None is taken here.

* **A1. Material data of the blade.** The tier models orthotropy, temperature dependence, relaxation, creep and fatigue, and the
  repository has no datum for any polymer. Options: (a) every material constant is an input; the demonstration (0339) uses assumed
  values, each named as assumed on the user page; (b) the maintainer names a polymer and supplies a public data sheet or measured
  values for `docs/reference-data.md`, and 0339 uses them; (c) hold 0339 until data exist. Recommendation: (a) now, (b) welcome at
  any time; with (a) no life or creep number of the tier may be quoted as a prediction.
* **A2. The flag benchmark's values (ADR 0337).** House rule 6 does not let an agent take a paper the repository lacks. Options:
  (a) approve that the implementer of 0337 fetches the public report, records the values with the source in
  `docs/reference-data.md` and commits that before its registration, as wave 31C did for the cavity tables; (b) the maintainer
  adds the values; (c) drop 0337: the tier then has no public benchmark. Recommendation: (a).
* **A3. The constitutive call.** Section 2.4, points 1 to 4, are requests to packages of the earlier tiers that are not written
  yet (ADR 0273, 0295, 0300). Options: (a) pass them on now; (b) accept the second entry point and the restructuring in ADR 0311.
  Recommendation: (a).
* **A4. The sparse factorisation as the default.** Options: (a) opt-in, the envelope solver stays the default and every existing
  result keeps its bits; (b) the default above a size threshold once 0334 is accepted, which changes the bits of large solid
  results (no hash table covers them today). Recommendation: (a), and (b) as a later decision with numbers.
* **A5. Where the parallel glue lives.** Options: (a) `rcfd-api` gains a dependency on `rcfd-dist` and a module `solid_parallel`,
  library only; (b) `rcfd-fem` depends on `rcfd-dist` and `rcfd-gpu` (same layer: the graph test may refuse it); (c) keys in the
  solid case file as well. Recommendation: (a) now; (c) after the first measured numbers.
* **A6. The cuts of section 1.3, group B.** Twenty-nine numbers were available. Options: (a) as proposed; (b) swap in explicit
  dynamics (S) or frictional contact (M) for hyperelasticity (0328) or fracture (0332), which have the least fan use among the
  planned; (c) raise the cap. Recommendation: (a).
* **A7. Items for other categories.** The protocol transport (coupling and service) and topology optimisation (optimiser) are
  named in section 1.3 with their checks. Options: (a) the lead adds them as items of those categories from that text; (b) leave
  them out. Recommendation: (a) for the transport, since the co-simulation page lists it as its first gap.
* **A8. The fan's flutter margin.** Not planned (section 1.3, group A). Options: (a) as proposed, revisited when F-126 is met and
  a moving wall exists in the rotating passage; (b) a recorded run of ADR 0324's method on the passage without limits, as a
  demonstration. Recommendation: (a); (b) would publish a damping number that nothing supports.
* **A9. The frame.** The repository has no frame or strut geometry (fan breakdown A1 partial). Options: (a) 0339 uses a stand-in
  plate with the rotor opening; (b) 0339 waits for A1. Recommendation: (a), stated on the page.
* **A10. Roadmap placement.** The existing category "Structural solver", family "Solver", phase 9, as the two earlier tiers. 0336
  could sit with F-100 and F-101 instead; I do not argue for it, because its acceptance is about solids.

## 6. What I am unsure of

* **Three tiers of plans on no code.** File names, the constitutive call, the driver and the shell's section integral are read
  from ADR 0270 and 0290. The scaffold (0311) is the most exposed package; I sized it M for that reason.
* **Numbers.** Every limit in section 3 is a draft from standard theory or an estimate; nothing was run. The ones I trust least:
  every percentage of the Hertz check (0330) and of the tie's patch error (0329); the effectivity band on tetrahedra (0335); the
  factor of two in fill (0334) and in rank iterations (0336); the 3 percent of the channel impedance against a first-order flow
  solver (0324); the 2 percent of the bar with a boundary dashpot on a modal subspace (0317); the triangle shell's percentages
  (0322); the shell against solid Campbell table (0323); all of 0337.
* **Recalled formulas,** each marked for re-derivation in its stub: Bryan's split 4 n Omega/(n^2 + 1) (0318); the ring buckling
  factors 3 and 4 (0327); the stationary creep field of a cylinder (0314); the inflated neo-Hookean sphere (0328); the secant
  width correction of the cracked plate (0332); the whirl equation of the spinning beam (0319); the elliptic factor 1.1803 of the
  pendulum (0326); the mean-square response formulas (0320); the piston's radiation efficiency (0321). Derived here by hand and
  believed right: the interference-fit pressure (0330), the two-bar truss (0327), the thin ring under follower pressure (0327),
  the bar with a tip dashpot in both the harmonic and the eigenvalue form (0315, 0317), the low-frequency limit of the channel
  impedance (0324). All citations are from memory.
* **Which triangular shell.** The assumed-strain triangle of 0322 is recalled, not derived; the stub names the fallback.
* **The Newton driver's extension points** (0311): whether one design serves contact, follower loads and arc-length was thought
  through only on paper.
* **Contact convergence** (0330): active-set cycling on curved surfaces is the usual failure and has no closed-form guard.
* **`rcfd-dist` with nodes** (0336): the decomposition type is built from cells; I read two signatures, not the body.
* **Whether the machine has a CUDA device** for the device clauses of 0336.
* **The flag benchmark** (0337): whether any generator of the repository meshes the channel with the cylinder and the flag, and
  how the flow solver does on the flow-only gate.
* **Sizes.** At the upper end of M, with their splits: 0312 (orthotropy; tables), 0323 (beams; shells), 0327 (follower and tangent
  modes; arc-length), 0329 (ties; joints and cyclic), 0316 (curves and corrections; counting and random damage). If all five
  split, the tier needs thirty-four numbers, not twenty-nine.
* **Whether the order is right for the maintainer.** I put the general mechanics (plasticity, contact) behind the polymer and
  vibration work because the fan does not need them; a maintainer who wants a general-purpose solver first would reverse waves 2
  and 4.

## 7. For the lead: roadmap entries

Category for the roadmap graphic: the existing **"Structural solver"**, family **"Solver"** (the row of `CATEGORIES` in
`scripts/roadmap_progress.py` that holds 370 to 406; the new ids are appended to it). Placeholder ids SA-1 to SA-29 for ADR 0311
to 0339, to be renumbered; status todo for all; phase 9 as the earlier tiers (A10). Dependencies are given as SA ids or F ids. The
existing items they depend on: F-016, F-101 and F-136 (done), F-362 (todo), and the two earlier tiers F-370 to F-406 (todo). SA-27
is blocked on decision A2 and nothing depends on it. Not planned, for the lead to record where the roadmap overview speaks of the
structural solver: the items of section 1.3. Suggested edits of existing notes, for the lead to word: fan breakdown E5 may name
this plan as the route to the material dimension once SA-6 is done; the "Limits and gaps" of `docs/cosimulation-protocol.md` keeps
its transport line (decision A7).

Each acceptance and each note below is one quoted paragraph on a single line.

### Entries

**SA-1** (ADR 0311), status todo, deps: F-372, F-388, F-397

- title: Scaffold of the advanced tier: module layout, material variants with a point context and a state layout, extension points of the Newton driver
- acceptance:

> Every limit not stated here is registered in the ADR before the run. The modules, material variants, point context, state layout, element kind, layup property, section-stiffness hook and the two driver extension points of ADR 0310 section 2 exist; every test of the basic and intermediate tiers (the sb_ and si_ files) compiles and passes unedited, and a registered set of models (patch test, cantilever, bar modes, the elastica of F-397, the free vibration of F-398, a Shell4 plate) gives displacements, stresses, frequencies and histories with the same bits as before the change; with an empty contribution list and no step hook the Newton driver takes the same iterations with the same residuals bit for bit; the default point context (no temperature, no time increment, global axes) reproduces the isotropic stress and tangent bit for bit; each new material variant, the element kind Shell3 and the layup property are refused by every solve with an error naming the variant and the set, never ignored; the complex type satisfies (a b) conj(b) = a |b|^2 and a/a = 1 within 1e-15 on a registered sample and prints and parses losslessly; the baseline hashes are unchanged.

- note:

> Structural solver, advanced tier (plan ADR 0310, 2026-11-12). Size M. Stub ADR 0311; the registration is written by the implementation agent before any code.

**SA-2** (ADR 0312), status todo, deps: SA-1, F-372, F-374, F-376, F-380

- title: Orthotropic elasticity with material axes, orthotropic expansion, and temperature-dependent properties from tables
- acceptance:

> Every limit not stated here is registered in the ADR before the run. Orthotropy: with isotropic constants the elasticity matrix equals the isotropic one within 1e-13 relative; constants that are not positive definite are refused naming the constant. Off-axis tension of a block with the material axes turned by 15, 30, 45 and 60 degrees about z: axial strain sigma/E_x with 1/E_x = c^4/E1 + s^4/E2 + (1/G12 - 2 nu12/E1) s^2 c^2 and the shear-extension ratio from the rotated compliance within 1e-10; rotating the model and the axes together leaves the strain energy unchanged within 1e-12. Cantilever of slenderness 20 with E1 along the span and E2 = E1/10: tip deflection P L^3/(3 E1 I) + P L/(kappa G13 A) within 1 percent. Free heating gives the strains alpha_i dT in material axes with zero stress, full restraint the stress -D alpha dT, both within 1e-10. Cylindrical axes: a tube b/a = 2 with E_theta/E_r = 4 under internal pressure in plane stress has the bore hoop stress of the polar-orthotropic closed form (powers r^(k-1) and r^(-k-1), k = sqrt(E_theta/E_r), derived in the registration) within 1 percent. Tables: a bar whose temperature and so its modulus vary linearly from E0 to E1 along it stretches under an end force by F L ln(E1/E0)/(A (E1 - E0)) within 0.1 percent with 20 elements; with alpha = a0 + a1 T the free expansion is a0 (T - Tref) + a1 (T^2 - Tref^2)/2 within 1e-10; a constant table gives the bits of the constant material; a temperature outside a table is refused with the element and the temperature.

- note:

> Structural solver, advanced tier (plan ADR 0310, 2026-11-12). Size M. Stub ADR 0312; the registration is written by the implementation agent before any code.

**SA-3** (ADR 0313), status todo, deps: SA-1, F-397, F-398

- title: Linear viscoelasticity: Prony series in time, complex modulus in frequency, time-temperature shift
- acceptance:

> Every limit not stated here is registered in the ADR before the run. Relaxation of one element after a strain step: sigma(t) = eps0 (E_inf + sum E_i exp(-t/tau_i)) within 1e-10 at every step end for three branches. Creep of the standard solid under constant stress: eps(t) = sigma0 (1/E_inf - (1/E_inf - 1/E_0) exp(-t/tau_c)), tau_c = tau E_0/E_inf, within 0.1 percent at 20 steps per tau with observed order >= 1.9. The complex modulus function returns E' = E_inf + sum E_i w^2 tau_i^2/(1 + w^2 tau_i^2) and E'' = sum E_i w tau_i/(1 + w^2 tau_i^2) within 1e-13, and a strain cycled in time dissipates pi eps0^2 E'' per cycle and volume within 1 percent at 200 steps per cycle. Shift: the relaxation curve at a temperature with shift factor a_T equals the reference curve at t/a_T within 1e-10. Free vibration of a bar of the standard solid with a loss factor near 0.05 at its first frequency: logarithmic decrement pi E''/E' within 3 percent. A material without branches gives the bits of the elastic one; the internal stresses are saved and restored bit for bit with the integrator state; a non-positive relaxation time or modulus is refused with the key.

- note:

> Structural solver, advanced tier (plan ADR 0310, 2026-11-12). Size M. Stub ADR 0313; the registration is written by the implementation agent before any code.

**SA-4** (ADR 0314), status todo, deps: SA-1, F-397

- title: Creep: power law with time hardening, implicit integration with a consistent tangent
- acceptance:

> Every limit not stated here is registered in the ADR before the run. The tangent agrees with central differences within 1e-6. Uniaxial constant stress: creep strain A sigma^n t within 1e-8 for m = 0 and A sigma^n t^(m+1)/(m+1) within 0.1 percent for m = -0.5. Relaxation at fixed strain, n = 3 and 5: sigma(t) = (sigma0^(1-n) + (n - 1) E A t)^(1/(1-n)) within 0.5 percent at the registered step count, observed order >= 0.9. Thick cylinder b/a = 2 in plane strain under internal pressure, n = 4: the stress starts at Lame within 1 percent and tends to the stationary field sigma_r = -p ((b/r)^(2/n) - 1)/((b/a)^(2/n) - 1), sigma_theta = p ((2 - n)/n (b/r)^(2/n) + 1)/((b/a)^(2/n) - 1) within 2 percent after the registered redistribution time. Radial bar clamped on the axis of rotation, n = 3: the tip creeps at the rate A times the integral of (rho Omega^2 (L^2 - r^2)/2)^n over the length within 0.5 percent (the stress is statically determinate). A step whose creep-strain increment exceeds the registered limit is halved; a non-converged local iteration is an error with the element.

- note:

> Structural solver, advanced tier (plan ADR 0310, 2026-11-12). Size S. Stub ADR 0314; the registration is written by the implementation agent before any code.

**SA-5** (ADR 0315), status todo, deps: SA-1, F-373, F-374, F-383, F-399

- title: Direct harmonic solve: complex symmetric factorisation, structural (hysteretic) damping, discrete dampers, base excitation
- acceptance:

> Every limit not stated here is registered in the ADR before the run. rcfd-linalg: a random complex symmetric diagonally dominant matrix of order 200 is solved to a residual of 1e-12 relative; a real matrix gives the real factorisation's solution within 1e-13; a pivot below the registered threshold is refused with the equation. Fixed-free bar, nu = 0, loss factor 0.02, tip force: tip response F tan(k L)/(E* A k) with E* = E (1 + i eta) and k = w sqrt(rho/E*), amplitude and phase within 0.5 percent at 0.5, 1.0 and 1.5 times the first frequency. With Rayleigh damping the direct solve equals the mode superposition of F-399 with all modes within 1e-8 on a small model. Tip dashpot c on the same bar: response F/(E A k cot(k L) + i w c) within 0.5 percent, and with c = A sqrt(E rho) the amplitude is F |sin(k L)|/(E A k) (no resonance) within 0.5 percent at five frequencies. Base motion of the clamped end, undamped: tip amplitude 1/cos(k L) of the base within 0.5 percent at half the first frequency. A table of modulus against frequency reproduces the constant-modulus result when constant (1e-12). An undamped solve at a resonance is refused with the frequency; results are bit-identical for 1 and 8 threads.

- note:

> Structural solver, advanced tier (plan ADR 0310, 2026-11-12). Size M. Stub ADR 0315; the registration is written by the implementation agent before any code.

**SA-6** (ADR 0316), status todo, deps: SA-1, F-376, F-399

- title: High-cycle fatigue: stress-life curves, mean-stress corrections, harmonic and counted histories, linear damage
- acceptance:

> Every limit not stated here is registered in the ADR before the run. Curves and corrections reproduce N = (sigma_a/sigma_f)^(1/b)/2, sigma_a/(1 - sigma_m/sigma_u) (Goodman), sigma_a/(1 - (sigma_m/sigma_u)^2) (Gerber) and Soderberg within 1e-12; a mean stress at or above the strength is refused. Harmonic amplitude: for a normal and a shear component 90 degrees out of phase the von Mises amplitude equals the closed-form maximum over the phase within 1e-10. Cantilever with a static and a harmonic tip force: the largest damage lies on the root surface and the life there equals the hand value from 6 P L/(b h^2) within the registered factor that a 2 percent stress error allows. Plate with a small circular hole (radius/width 0.05) under remote harmonic tension: stress amplitude at the hole edge within 3 percent of 3 sigma (Kirsch) and the life the Basquin life of that stress. Counting: n cycles of constant amplitude give n cycles of that range; a large cycle carrying m small ripples gives 1 and m; a periodic history started at its maximum closes with no residual; a history and its mirror image give the same counts; two load blocks give n1/N1 + n2/N2 within 1e-12. Narrow band: the damage rate nu0 (sqrt(2) s)^m Gamma(1 + m/2)/C within 1e-12 as a function, and the counted damage of a synthesised narrow-band history (registered seed, 1e5 cycles) within 5 percent of it.

- note:

> Structural solver, advanced tier (plan ADR 0310, 2026-11-12). Size M. Stub ADR 0316; the registration is written by the implementation agent before any code.

**SA-7** (ADR 0317), status todo, deps: SA-1, F-382, F-383

- title: Damped and complex modes: a dense eigensolver for general real matrices and the quadratic eigenproblem on a modal subspace
- acceptance:

> Every limit not stated here is registered in the ADR before the run. rcfd-linalg: the companion matrix of a polynomial with registered real and complex roots returns them within 1e-10; a random matrix of order 50 has residuals |A v - lambda v| <= 1e-10 |A|; a symmetric matrix reproduces the Jacobi values of F-382 within 1e-10; a Jordan block is returned with its eigenvalue within 1e-6 and a report, not a failure; no convergence within the registered sweeps is an error. One oscillator: lambda = -zeta w +/- i w sqrt(1 - zeta^2) within 1e-12. Rayleigh damping a M + b K on a solid model: every mode has the ratio (a/w_j + b w_j)/2 within 1e-8. Skew coupling: a two-unknown system with G = g [[0, 1], [-1, 0]] has the closed-form split frequencies within 1e-12. Fixed-free bar with a tip dashpot c below A sqrt(E rho): lambda_j = (c0/L) (-atanh(c/(A sqrt(E rho))) + i (2 j - 1) pi/2), the first three within 2 percent on 40 modes; at c above that value no oscillating root is returned and the result says so. Damping ratios are -Re(lambda)/|lambda|; conjugate pairs are reported once.

- note:

> Structural solver, advanced tier (plan ADR 0310, 2026-11-12). Size M. Stub ADR 0317; the registration is written by the implementation agent before any code.

**SA-8** (ADR 0318), status todo, deps: SA-7, F-396

- title: Coriolis term of a rotating part: gyroscopic matrix in the rotating frame and the Campbell table with split modes
- acceptance:

> Every limit not stated here is registered in the ADR before the run. G is skew within 1e-13 of its norm and x^T G x = 0 within 1e-12. Without the option the table has the bits of F-396; with it at zero speed the frequencies equal those of F-396 within 1e-10. A mass on isotropic springs in the plane of rotation (two unknowns): frequencies |w0 - Omega| and w0 + Omega within 1e-10; the same as a stiff block on elastic supports within 1e-3. Thin ring spinning about its axis (thickness/radius 0.05, nu = 0, full model): the two travelling waves of in-plane mode n differ in frequency by 4 n Omega/(n^2 + 1) within 2 percent for n = 2 and 3 at Omega = 0.1 w_2. Radial beam of square section: the flap frequency equals that of F-396 within 1e-6 relative and the lag frequency moves by the registered second-order estimate within 20 percent of that estimate. Crossings of split modes are tracked without exchange (modal assurance on the complex shapes above 0.9). A speed at which an eigenvalue has a positive real part is reported as unstable with the speed.

- note:

> Structural solver, advanced tier (plan ADR 0310, 2026-11-12). Size M. Stub ADR 0318; the registration is written by the implementation agent before any code.

**SA-9** (ADR 0319), status todo, deps: SA-7, F-391, F-393

- title: Flexible rotor on bearings in the stationary frame: whirl, critical speeds, unbalance response, joined to the rigid-rotor model
- acceptance:

> Every limit not stated here is registered in the ADR before the run. At zero speed the frequencies equal those of the beam model of F-391 within 1e-10. Disc on a light shaft between two bearings: critical speed sqrt(k/m) and unbalance amplitude e r^2/sqrt((1 - r^2)^2 + (2 zeta r)^2) within 1e-3 at r = 0.5, 1 and 2. In the stiff-shaft limit the critical speeds and the response equal rotordyn::critical_speeds and rotordyn::response for the same rotor within 1e-6. Uniform shaft on pins with rotary inertia, 20 elements: forward and backward whirl of modes 1 and 2 equal the roots of E I k^4 - rho A w^2 - rho I k^2 w^2 +/- 2 rho I k^2 Omega w = 0, k = n pi/L, within 0.5 percent at three speeds. Overhung disc on a massless cantilever shaft: the four whirl speeds equal the roots of the two-unknown closed form (derived in the registration) within 0.5 percent. Anisotropic bearings split the critical speed into sqrt(kx/m) and sqrt(ky/m) within 1e-3. A cross-coupled stiffness k_c on a damped disc is stable below c w_n and unstable above, the threshold within 1 percent. Forward and backward are labelled by the sense of the orbit; an unstable speed is reported, never dropped.

- note:

> Structural solver, advanced tier (plan ADR 0310, 2026-11-12). Size M. Stub ADR 0319; the registration is written by the implementation agent before any code.

**SA-10** (ADR 0320), status todo, deps: SA-1, F-399

- title: Random vibration: response to force and base-acceleration spectra by modal superposition, root-mean-square stress
- acceptance:

> Every limit not stated here is registered in the ADR before the run. One mode under a flat one-sided force spectrum G: mean-square displacement (pi/2) f_n Q G/k^2 with Q = 1/(2 zeta), and under a flat base acceleration spectrum the mean-square absolute acceleration (pi/2) f_n Q G (for Q well above 1; the exact integral is computed in the registration), both within 0.5 percent at zeta = 0.02 on the registered grid, observed convergence under grid refinement; zero-crossing rate f_n within 1 percent. Two well separated modes: the mean square is the sum within 1 percent. A band-limited spectrum equals the numerical integral registered beforehand within 1e-6. For a uniaxial stress state the root-mean-square von Mises stress equals the root-mean-square stress within 1e-10. Two fully correlated patterns equal one summed pattern within 1e-12; uncorrelated ones add in mean square. A spectrum with a negative value or a non-increasing frequency list is refused with the entry.

- note:

> Structural solver, advanced tier (plan ADR 0310, 2026-11-12). Size S. Stub ADR 0320; the registration is written by the implementation agent before any code.

**SA-11** (ADR 0321), status todo, deps: SA-1, F-136, F-399

- title: Radiated sound of a vibrating surface: equivalent radiated power and the Rayleigh integral of a baffled plane
- acceptance:

> Every limit not stated here is registered in the ADR before the run. Equivalent radiated power: a rigid piston gives rho c S v^2/2 within 1e-12 and a plate mode sin sin gives rho c A v0^2/8 within 0.5 percent. Rayleigh integral of a circular piston of radius a: on-axis pressure 2 rho c v |sin(k (sqrt(z^2 + a^2) - z)/2)| within 1 percent for z from a to 20 a at k a = 2 and 10 with faces of a tenth of a wavelength; far-field directivity |2 J1(k a sin theta)/(k a sin theta)| within 1 percent; radiated power over the equivalent radiated power equal to 1 - J1(2 k a)/(k a) within 1 percent at k a = 0.5, 2 and 10, and at k a = 0.1 the power rho c k^2 (S v)^2/(4 pi) within 2 percent. Levels: 1e-12 W gives 0 dB and the third-octave sums equal the narrow-band total within 1e-10. A surface that is not plane within the registered tolerance is refused by the Rayleigh integral with its deviation; the equivalent radiated power accepts any surface and its report says that it assumes a radiation efficiency of one.

- note:

> Structural solver, advanced tier (plan ADR 0310, 2026-11-12). Size S. Stub ADR 0321; the registration is written by the implementation agent before any code.

**SA-12** (ADR 0322), status todo, deps: SA-1, F-391, F-392

- title: Three-node shell element (flat facet, assumed shear strain) for triangulated surfaces
- acceptance:

> Every limit not stated here is registered in the ADR before the run. Membrane patch test and constant-curvature bending patch test on an irregular flat mesh, and on a mesh mixing Shell3 and Shell4, within 1e-8. Simply supported square plate under uniform load, 32 x 32 squares each cut in two, nu = 0.3: centre deflection within 2 percent of the Navier series at thickness/side 0.01 and within 3 percent at 0.001, and within 1 percent of each other for the two diagonal directions. Cantilever strip, nu = 0: tip deflection within 1 percent. Pinched ring strip, 32 elements per quarter: loaded diameter within 3 percent of (pi/4 - 2/pi) P R^3/(E I). Cylinder under internal pressure: radial displacement p R^2/(E t) within 1 percent. First frequency of the simply supported plate within 2 percent. A free plate has six zero frequencies below 1e-6 of the seventh; a single element has exactly six zero eigenvalues; a triangle with an angle below the registered limit is refused with the element.

- note:

> Structural solver, advanced tier (plan ADR 0310, 2026-11-12). Size M. Stub ADR 0322; the registration is written by the implementation agent before any code.

**SA-13** (ADR 0323), status todo, deps: SA-12, F-391, F-392, F-395, F-396

- title: Stress stiffening of beams and shells: buckling, pre-stressed modes and the Campbell table of a shell blade
- acceptance:

> Every limit not stated here is registered in the ADR before the run. Beam: Euler loads pi^2 E I/L^2 (pinned) and pi^2 E I/(4 L^2) (clamped-free) with the registered shear correction within 0.1 percent at 8 elements; under tension w_n^2 = (n pi/L)^4 E I/(rho A) + (n pi/L)^2 T/(rho A) within 0.5 percent for n = 1, 2. Rotating radial beam of square section: w_flap^2 - w_lag^2 = Omega^2 within 1e-6 Omega^2 and the stiffening slope equal to the Rayleigh quotient of F-396 within 1 percent. Shell: a simply supported square plate buckles at 4 pi^2 D/a^2 per width in one direction and at 2 pi^2 D/a^2 under equal compression in both, within 2 percent at 16 x 16 Shell4 and 4 percent at 32 x 32 x 2 Shell3; a stretched strip has the tension frequencies above within 1 percent. Equivalence: a flat plate strip twisted by 30 degrees, thickness/chord 0.02, clamped at the root and rotating: the Campbell table of the first four modes as a shell equals that of the solid model of F-396 (Hex20) within 2 percent at the registered speeds. Models of solid elements only keep their bits.

- note:

> Structural solver, advanced tier (plan ADR 0310, 2026-11-12). Size M. Stub ADR 0323; the registration is written by the implementation agent before any code.

**SA-14** (ADR 0324), status todo, deps: SA-1, F-362, F-383, F-400, F-401

- title: Aerodynamic damping and added mass of a mode by prescribed motion of the wall (energy method)
- acceptance:

> Every limit not stated here is registered in the ADR before the run. Force harmonics: for a synthetic force history with known in-phase and quadrature parts the extraction returns them within 1e-10, and the work per cycle equals pi q0 times the quadrature part within 1e-10. Piston on a column of incompressible fluid with an open end (the case of F-401): added mass rho_f A L_f within 2 percent and a damping ratio below the registered viscous and time-step estimate. Piston driving a plane channel of gap h and length L_f: force per piston velocity equal to A times the oscillating-channel impedance i w rho L_f/(1 - tanh(kappa)/kappa), kappa = (h/2) sqrt(i w/nu), real and imaginary parts within 3 percent at Womersley numbers 1 and 5 after the registered extrapolation in the time step, and the low-frequency limit 12 mu L_f/h^2 within 3 percent. The damping ratio of the bar on the open column from this method and from the decay of the coupled run of F-401 are both recorded. A history that has not become periodic within the registered tolerance is refused with the cycle-to-cycle change.

- note:

> Structural solver, advanced tier (plan ADR 0310, 2026-11-12). Size M. Stub ADR 0324; the registration is written by the implementation agent before any code.

**SA-15** (ADR 0325), status todo, deps: SA-1, F-389, F-397

- title: J2 plasticity with isotropic and kinematic hardening: radial return and consistent tangent
- acceptance:

> Every limit not stated here is registered in the ADR before the run. The tangent agrees with central differences within 1e-6 in the elastic and in the plastic branch. Uniaxial bar, bilinear: the stress follows sigma_y + (E H/(E + H)) (eps - sigma_y/E) within 1e-10 in one step and in 100; on reversal the bar yields again at -sigma_max with isotropic and at sigma_max - 2 sigma_y with kinematic hardening within 1e-10. Thick sphere b/a = 2, perfectly plastic, internal pressure: the pressure at plastic radius c equals 2 sigma_y ln(c/a) + (2 sigma_y/3)(1 - c^3/b^3) within 1 percent at c = 1.25 a and 1.5 a, the solve converges at 0.98 of the limit pressure 2 sigma_y ln(b/a), and the residual hoop stress at the bore after unloading equals the closed form within 2 percent. Beam of rectangular section in pure bending: M/M_y = 1.5 - 0.5 (kappa_y/kappa)^2 within 1 percent up to kappa = 5 kappa_y. The last three residuals of each step show an order >= 1.8; a halved step restores the state bit for bit; a non-positive yield stress and a softening curve are refused with the key.

- note:

> Structural solver, advanced tier (plan ADR 0310, 2026-11-12). Size M. Stub ADR 0325; the registration is written by the implementation agent before any code.

**SA-16** (ADR 0326), status todo, deps: SA-1, F-397, F-398

- title: Nonlinear transient dynamics: Newton iteration inside the Newmark and generalised-alpha step
- acceptance:

> Every limit not stated here is registered in the ADR before the run. At a load factor of 1e-6 the history equals the linear one of F-398 within 1e-9 relative. A stiff slender bar pinned at one end and released from the horizontal under gravity swings with the period 2 pi sqrt(2 L/(3 g)) (2/pi) K(sin 45 degrees) (factor 1.1803, recomputed in the registration) within 1 percent, the total energy constant within 1e-3 of its swing over five periods at the registered step. A free spinning elastic bar keeps its linear momentum within 1e-10 and its angular momentum within 1e-6 relative over ten turns. Large axial oscillation of a bar element with nu = 0 from the stretch 1.2: period equal to the quadrature of the energy integral (computed in the registration) within 0.5 percent. With the spectral radius 1 the generalised-alpha result equals Newmark within 1e-10. A run split by save and restore has the bits of the unsplit run, with a material state included. A step that does not converge after the registered halvings is an error with the time.

- note:

> Structural solver, advanced tier (plan ADR 0310, 2026-11-12). Size M. Stub ADR 0326; the registration is written by the implementation agent before any code.

**SA-17** (ADR 0327), status todo, deps: SA-1, F-395, F-396, F-397

- title: Follower pressure, arc-length continuation, and modes about a nonlinear equilibrium
- acceptance:

> Every limit not stated here is registered in the ADR before the run. Follower pressure: the load stiffness agrees with central differences of the load within 1e-6. Thin ring (thickness/radius 0.02, nu = 0) under internal pressure up to the stretch 1.5: p = E t (lambda^2 - 1)/(2 R) within 1 percent, the dead pressure on the same mesh being the registered control (off by the factor lambda). Linear buckling of the ring under external pressure with the load stiffness: 3 E I/R^3 within 2 percent; without it the registered control 4 E I/R^3 within 3 percent. Arc-length: a two-bar truss of solid bars (nu = 0, height h, bar length L0) is followed to the apex displacement 2.2 h with P = E A (h - w)(2 h w - w^2)/L0^3 within 1e-6 at the registered points and the limit load 2 E A h^3/(3 sqrt(3) L0^3) within 0.5 percent, load control failing at the first limit point (control). Modes about a state: a clamped-clamped beam stretched by an end displacement has the tension frequencies of F-395 within 1 percent, and at a small stretch the tangent modes equal the linear pre-stressed modes within 1e-4; for the rotating radial beam at 0.2 w_0 they equal F-396 within 1e-4, and the difference at higher speed is recorded.

- note:

> Structural solver, advanced tier (plan ADR 0310, 2026-11-12). Size M. Stub ADR 0327; the registration is written by the implementation agent before any code.

**SA-18** (ADR 0328), status todo, deps: SA-1, F-390, F-397

- title: Hyperelastic materials (neo-Hookean, Mooney-Rivlin) with a mean-dilatation element for near incompressibility
- acceptance:

> Every limit not stated here is registered in the ADR before the run. The tangent agrees with central differences within 1e-6; a rotation of the deformed state leaves the energy unchanged within 1e-10; at a strain of 1e-6 the stress equals linear elasticity with the same shear and bulk modulus within 1e-6 relative. With bulk/shear = 1e4: uniaxial nominal stress mu (lambda - lambda^-2) (neo-Hookean) and 2 (C1 + C2/lambda)(lambda - lambda^-2) (Mooney-Rivlin) for stretches 0.7 to 3, equibiaxial mu (lambda - lambda^-5), simple shear tau = mu gamma with the normal stress difference mu gamma^2, each within 0.1 percent after the registered compressibility correction. Thick spherical shell B/A = 2, neo-Hookean, inflated to the inner stretch 1.5: pressure (mu/2) ((1/l_b^4 + 4/l_b) - (1/l_a^4 + 4/l_a)) with l_b^3 = 1 + (A/B)^3 (l_a^3 - 1) (recomputed by quadrature in the registration) within 1 percent with the mean-dilatation element, the standard Hex8 on the same mesh being the registered locking control. A non-positive modulus is refused; an inverted element is an error with the element.

- note:

> Structural solver, advanced tier (plan ADR 0310, 2026-11-12). Size M. Stub ADR 0328; the registration is written by the implementation agent before any code.

**SA-19** (ADR 0329), status todo, deps: SA-1, F-391, F-392, F-393, F-394

- title: Tied non-matching meshes, shell and beam joints to solids, non-matching cyclic boundaries and nodes on the axis
- acceptance:

> Every limit not stated here is registered in the ADR before the run. With matching meshes the tied model equals the merged one within 1e-10. Flat non-matching interfaces 2:3 and 3:5 under uniaxial stress: far-end displacement within 0.1 percent and interface stress within the registered 3 percent (a one-sided node-to-surface tie does not pass the patch test exactly), the error falling with an observed order >= 1 under refinement. A cantilever cut in two and tied 2:3 (Tet10 to Hex20): tip deflection within 1 percent of the closed form. The constraint forces of a tie have zero resultant and moment within 1e-10. A node farther from every face than the registered tolerance is refused with its id. Joints: a beam continuing a solid bar and a shell continuing a solid plate give the tip deflection of the one-piece cantilever within 1 and 2 percent. Cyclic: the ring sector of F-394 with a 2:3 boundary has its frequencies within 0.5 percent of the matching sector; a solid disc with nodes on the axis, spinning (index 0): centre stress (3 + nu) rho Omega^2 b^2/8 within 1 percent; a rod on the axis at index 1 has the bending frequency of the beam within 1 percent, and at index 2 its axis nodes are held.

- note:

> Structural solver, advanced tier (plan ADR 0310, 2026-11-12). Size M. Stub ADR 0329; the registration is written by the implementation agent before any code.

**SA-20** (ADR 0330), status todo, deps: SA-19, F-397

- title: Frictionless contact: node-to-surface with an augmented Lagrangian, rigid and deformable targets, interference fits
- acceptance:

> Every limit not stated here is registered in the ADR before the run. A bar pushed against a rigid stop across a gap g: the piecewise-linear force-displacement closed form within 1e-8, the penetration after augmentation below 1e-8 L. Two stacked blocks with matching meshes under pressure p: uniform stress and contact pressure p within 1e-8; pulled apart they separate with no tensile contact force (every multiplier >= 0). Elastic sphere on a rigid plane: contact radius (3 F R/(4 E*))^(1/3), peak pressure 3 F/(2 pi a^2) and approach a^2/R within 5, 5 and 3 percent at a/R = 0.1 with at least ten nodes across the contact radius. Interference fit of a ring (outer radius b) on a solid shaft (radius a) of the same material with radial interference d, plane stress: contact pressure E d (b^2 - a^2)/(2 a b^2) within 2 percent, and the hoop stress of the ring that of Lame under that pressure within 2 percent. The contact tangent agrees with central differences within 1e-6 in a fixed active set; the last residuals of a step with a settled active set show an order >= 1.8; an active set that cycles beyond the registered count is an error with the nodes.

- note:

> Structural solver, advanced tier (plan ADR 0310, 2026-11-12). Size M. Stub ADR 0330; the registration is written by the implementation agent before any code.

**SA-21** (ADR 0331), status todo, deps: SA-2, F-392

- title: Laminated shell: layup stiffness, ply stresses and failure indices
- acceptance:

> Every limit not stated here is registered in the ADR before the run. One isotropic ply gives the bits of the Shell4 of F-392. Layup stiffness: a symmetric layup has no coupling (|B| <= 1e-12 |A| t); [0/90]s has A11 = A22 = t (Q11 + Q22)/2; [0/+60/-60]s and [0/45/-45/90]s have A11 = A22, A66 = (A11 - A12)/2 and A16 = A26 = 0, all within 1e-12 relative. Simply supported square plate of a cross-ply symmetric layup under a sinusoidal load: centre deflection q0 a^4/(pi^4 (D11 + 2 (D12 + 2 D66) + D22)) within 1 percent at thickness/side 0.01 with 16 x 16 elements. Unsymmetric [0/90] strip heated uniformly: curvature equal to the closed form of the two-ply laminate (solved by hand in the registration) within 0.5 percent with zero resultants. Off-axis coupon at 15, 30, 45 and 60 degrees: ply stresses sigma c^2, sigma s^2 and -sigma s c within 1e-10, Tsai-Hill failure stress (c^4/X^2 + (1/S^2 - 1/X^2) s^2 c^2 + s^4/Y^2)^(-1/2) within 1e-10, and Tsai-Wu equal to the tensile and compressive strengths on the axes within 1e-12. A layup with a non-positive thickness or an unknown material is refused with the ply.

- note:

> Structural solver, advanced tier (plan ADR 0310, 2026-11-12). Size M. Stub ADR 0331; the registration is written by the implementation agent before any code.

**SA-22** (ADR 0332), status todo, deps: SA-1, F-374, F-376, F-389

- title: Fracture parameters: J-integral by the domain form, stress intensity factor, crack-growth life
- acceptance:

> Every limit not stated here is registered in the ADR before the run. Centre crack of half-length a in a plate of half-width W = 10 a under tension, plane strain (one layer, held in z), Hex20: K_I from J within 2 percent of sigma sqrt(pi a) sqrt(sec(pi a/(2 W))), the values of rings 2 to 4 within 1 percent of each other, and J equal to the change of potential energy between two models with half-lengths a +/- 0.02 a within 2 percent. K_I from the opening displacement within 5 percent. Penny-shaped crack of radius a in a cylinder of radius 10 a under tension (heavy record): K_I = 2 sigma sqrt(a/pi) within 3 percent, uniform along the front within 2 percent. Under pure shear loading of the centre crack the mode-I factor from the opening is zero within 1e-8 of the tension value. Growth: the cycles from a0 to a1 by the Paris law with K = d sigma sqrt(pi a) equal the closed-form integral for m = 3 and the logarithm for m = 2 within 1e-6. A front that is not straight within the registered tolerance and a ring that reaches the boundary are refused.

- note:

> Structural solver, advanced tier (plan ADR 0310, 2026-11-12). Size M. Stub ADR 0332; the registration is written by the implementation agent before any code.

**SA-23** (ADR 0333), status todo, deps: SA-1, F-382, F-383, F-393

- title: Substructuring: static condensation and fixed-interface component mode synthesis (Craig-Bampton)
- acceptance:

> Every limit not stated here is registered in the ADR before the run. Statics: the condensed model gives the retained displacements and, after recovery, the interior ones and the stresses of the full model within 1e-10. Keeping every fixed-interface mode reproduces all frequencies of the full model within 1e-8 (small model). A bar of two substructures with five modes each: the first three frequencies of the assembly within 0.1 percent of the full model and not below it (1e-10), the error falling monotonically with the number of modes. The reduced matrices are symmetric within 1e-13 and the reduced mass is positive definite; a free substructure keeps six zero frequencies below 1e-6 of the seventh. Harmonic response of the assembly equals the full model within 1 percent up to half the lowest omitted fixed-interface frequency. A reduced part written to its file and read back gives the same bits. An interface set with no nodes and a mode count above the interior size are refused.

- note:

> Structural solver, advanced tier (plan ADR 0310, 2026-11-12). Size M. Stub ADR 0333; the registration is written by the implementation agent before any code.

**SA-24** (ADR 0334), status todo, deps: SA-1, F-373, F-374, F-382, F-402

- title: Sparse L D L^T with a fill-reducing ordering, and its use by the static, buckling and modal solves
- acceptance:

> Every limit not stated here is registered in the ADR before the run. Five-point grid 100 x 100 and seven-point grid 30^3: residual below 1e-12 relative, and on the 30^3 grid at most half the factor entries of the envelope after reverse Cuthill-McKee (both counts recorded). On a solid model the solution equals the envelope solver's within 1e-10 and the modal frequencies through either factorisation agree within 1e-9. Inertia: for the tridiagonal (2, -1) of order 200 and three shifts the number of negative pivots of K - s I equals the number of eigenvalues 4 sin^2(j pi/(2 (n + 1))) below s. A matrix that is not positive definite is reported with the equation when positive definiteness was requested. Factor and solution are bit-identical for 1 and 8 threads. Heavy record: a clamped cube of about 3e5 unknowns is solved, time and memory recorded beside the envelope's estimate. The default solver's results keep their bits.

- note:

> Structural solver, advanced tier (plan ADR 0310, 2026-11-12). Size M. Stub ADR 0334; the registration is written by the implementation agent before any code.

**SA-25** (ADR 0335), status todo, deps: SA-1, F-376, F-378

- title: Stress recovery by element patches and an energy-norm error estimate
- acceptance:

> Every limit not stated here is registered in the ADR before the run. A linear displacement field gives the exact stress and a zero estimate within 1e-10. On the manufactured solution of F-378 for Hex8, Tet10 and Hex20: the effectivity index (estimate over true error in the energy norm) lies in [0.8, 1.2] on the three finest meshes and its distance from 1 does not grow under refinement; the recovered stress of Hex8 converges in L2 at an observed order >= 1.7 where the raw stress shows >= 0.9. Plate with a small circular hole under tension: the recovered peak stress is within 2 percent of 3 sigma on the mesh on which the averaged nodal stress of F-376 is recorded, and the tenth of the elements with the largest indicator contains the elements at the hole edge. Boundary and corner nodes take the patch of the nearest interior node by the registered rule; the result is bit-identical for 1 and 8 threads; a patch with too few points is reported, not solved.

- note:

> Structural solver, advanced tier (plan ADR 0310, 2026-11-12). Size S. Stub ADR 0335; the registration is written by the implementation agent before any code.

**SA-26** (ADR 0336), status todo, deps: F-016, F-101, F-374, F-381

- title: Elasticity solves on the ranks of rcfd-dist and on the CUDA device
- acceptance:

> Every limit not stated here is registered in the ADR before the run. Ranks: with one rank the iteration counts and the solution have the bits of the serial CG of F-381 with the same preconditioner; with 2, 4 and 8 ranks the solution of a clamped cube (about 4e4 unknowns) and of a cantilever of Tet10 equals the serial one within 1e-8 relative at a residual of 1e-10, in at most twice the iterations of one rank (counts recorded); repeated runs with the same rank count give the same bits; the three unknowns of a node are never split; a rank that fails or times out ends the solve with a located error. Scaling is recorded, not asserted. Device: with a usable device the device CG equals the CPU CG with the same preconditioner within 1e-9 on a cube of at least 3e5 unknowns, times recorded; `auto` below the threshold runs on the CPU; without a device `cuda` is a located error and `auto` falls back with the reason recorded, and the device clauses are reported as not run. The default solver's results and the baseline hashes are unchanged.

- note:

> Structural solver, advanced tier (plan ADR 0310, 2026-11-12). Size M. Stub ADR 0336; the registration is written by the implementation agent before any code. rcfd-api gains a dependency on rcfd-dist (decision A5 of ADR 0310).

**SA-27** (ADR 0337), status todo, deps: SA-16, F-385, F-397, F-401

- title: Public coupled benchmark (the elastic flag behind a cylinder): structural tests CSM1 to CSM3 and the steady coupled case FSI1 (needs the maintainer: reference values)
- acceptance:

> Every limit not stated here is registered in the ADR before the run. NEEDS THE MAINTAINER'S APPROVAL to record the published values in docs/reference-data.md before it starts; no limit below uses a value recalled from memory. Structure alone (plane strain, one element thick): the tip displacement of CSM1 and CSM2 within 1 percent of the published values, both components; CSM3 (released under gravity, undamped): mean, amplitude and frequency of both tip components within 2 percent, on two meshes with the change recorded. Flow alone with a rigid flag (CFD1): drag within 2 percent and lift within 10 percent of the published values (gate; if it misses, the coupled case is not run and the item is partial). Coupled steady case FSI1: drag within 2 percent, tip displacement in x within 5 percent and in y within 10 percent, lift within 10 percent, with the coupling converged to 1e-6 in the final state and the iteration counts recorded. The unsteady cases FSI2 and FSI3 are not run.

- note:

> Structural solver, advanced tier (plan ADR 0310, 2026-11-12). Size M. Stub ADR 0337; the registration is written by the implementation agent before any code. Blocked until the maintainer approves recording the published values of the benchmark in docs/reference-data.md (decision A2 of ADR 0310); nothing depends on it.

**SA-28** (ADR 0338), status todo, deps: SA-2, SA-3, SA-4, SA-5, SA-6, SA-8, SA-10, SA-11, F-403

- title: Solid case file: the materials, analyses and results of the advanced tier
- acceptance:

> Every limit not stated here is registered in the ADR before the run. Each new analysis run from a file gives the bits of the same model run by library calls (limit 1e-12, expected identical), and one closed-form check of each library package passes from files: the off-axis block (ADR 0312), the relaxation curve (ADR 0313), the rotating creeping bar (ADR 0314), the bar with a loss factor (ADR 0315), the cantilever life (ADR 0316), the plane oscillator's split (ADR 0318), the single-mode mean square (ADR 0320), the piston's equivalent radiated power (ADR 0321). An unknown key, a material that lacks a required constant, a fatigue request without a mean or an alternating case, a table that is not increasing and an analysis that names a missing set are each refused with the key and the line. Every solid case file of the tests of the earlier tiers gives the same report and the same .vtu bytes as before. No flow case key, CLI help text, MCP tool list, hash table or golden changes (the existing schema, CLI and tool-list tests pass unedited).

- note:

> Structural solver, advanced tier (plan ADR 0310, 2026-11-12). Size M. Stub ADR 0338; the registration is written by the implementation agent before any code.

**SA-29** (ADR 0339), status todo, deps: SA-2, SA-4, SA-5, SA-6, SA-8, SA-11, SA-28, F-406

- title: Demonstration on the 120 mm fan: a polymer blade (orthotropy, temperature, creep, fatigue at a Campbell crossing) and the radiated power of the frame
- acceptance:

> Every limit not stated here is registered in the ADR before the run. Blade from solid case files. With isotropic constants and no temperature the Campbell table has the bits of F-406. A uniform temperature at which the table gives 0.8 E scales every frequency at rest by sqrt(0.8) within 1e-8. For the orthotropic blade the change of each squared frequency at rest for a 1 percent change of E1 equals the Rayleigh-quotient sensitivity within 1 percent. With the Coriolis term the first four frequencies at the maximum speed differ from those without by less than the registered bound, both recorded. Creep at the maximum speed over the registered time: tip growth monotone, its initial rate within the registered factor of the one-dimensional estimate, change between two meshes below 5 percent. Fatigue at a registered crossing: the safety-factor field is written, the largest alternating stress changes by less than 10 percent between two meshes and lies in the same region, and doubling the pressure harmonic changes the Goodman factor as the formula says within 1e-8. Frame: the equivalent radiated power spectrum at the rotation and blade-passing frequencies is written and equals (rho c/2) sum A |v_n|^2 recomputed from the output within 1e-10. The page docs/structural-solver.md lists every assumed input and states that no material datum, life, creep growth or sound level is validated against a measurement.

- note:

> Structural solver, advanced tier (plan ADR 0310, 2026-11-12). Size M. Stub ADR 0339; the registration is written by the implementation agent before any code.

## Lead's note (2026-11-12): roadmap ids

The entries were added to docs/roadmap.json with these ids: SA-1 = F-408, SA-2 = F-409, SA-3 = F-410, SA-4 = F-411, SA-5 = F-412, SA-6 = F-413, SA-7 = F-414, SA-8 = F-415, SA-9 = F-416, SA-10 = F-417, SA-11 = F-418, SA-12 = F-419, SA-13 = F-420, SA-14 = F-421, SA-15 = F-422, SA-16 = F-423, SA-17 = F-424, SA-18 = F-425, SA-19 = F-426, SA-20 = F-427, SA-21 = F-428, SA-22 = F-429, SA-23 = F-430, SA-24 = F-431, SA-25 = F-432, SA-26 = F-433, SA-27 = F-434, SA-28 = F-435, SA-29 = F-436, in the category "Structural solver".
