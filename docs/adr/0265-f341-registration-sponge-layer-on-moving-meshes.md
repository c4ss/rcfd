# ADR 0265: F-341 registration: the sponge layer of the compressible solver on moving meshes (the frame of the layer, its memory on a moving cell, the source in the ALE step, the mesh replacement)

Status: **REGISTERED 2026-11-12**, written before any code of F-341 and before any run of its checks. Results go in an
addendum; nothing above the addendum is changed after a run. Not yet accepted by the maintainer.

Parents: [ADR 0237](0237-f331-registration-sponge-layer-of-the-compressible-solver.md) (F-331: the plain and the matched
form, the memory `Q`, the tube check A0, decision D5 and question 5: `step_ale` and `replace_mesh` refuse a sponge),
[ADR 0248](0248-f340-registration-matched-sponge-shift-corner-and-damping-along-the-layer.md) (F-340: the shifted matched
form with its memories `q`, `r`, `m` and the rates of a second layer), [ADR 0207](0207-f258-registration-compressible-ale-boundaries-and-mesh-speed-time-step.md)
(F-258: characteristic conditions on moving faces, `max_dt_ale`, the deforming set-up U, the bit records R1 to R4),
[ADR 0243](0243-f332-registration-mass-spring-damper-wall.md) (F-332: the mass-spring-damper wall, refused on moving meshes,
D6), [ADR 0090](0090-wave38c-dynamic-mesh-layering.md) (the ALE form of `step_ale`, D2). Roadmap item F-341 (depends on
F-331 and F-258).

Number: the lead gave 0265. Branch `f341-sponge-moving-mesh` from main 65667301, worktree `w49i-work/f341`.

House rules (the campaign rules of 2026-11-12): limits are fixed here and are not changed after a run; a missed limit
becomes an ignored `*_known_miss` test with the limit intact and the measured number recorded, and is diagnosed, not tuned
away; a defective set-up gets a corrected test by an amendment committed before the corrected run, the old test staying
as an ignored record; each registered run is made once; parameters are fixed here from a derivation; defaults stay
bit-identical; `baseline_hashes.rs`, the hash tables and the goldens are not touched; the tests of ADR 0207, 0233, 0237,
0243 and 0248 are not edited; no new third-party crate, no unsafe code; unsigned commits without trailers; no push.

Clean-room: the design is derived below from the integral form of the conservation laws on a moving control volume and
from the equations of ADR 0237 and ADR 0248. Papers are cited from memory where named; none was opened. No vendor code or
documentation was used.

## What was read and done before this text (disclosed)

- **Read:** the F-341 entry of `docs/roadmap.json`; ADR 0237 in full; ADR 0248 (decisions, checks, addendum headings, the
  roadmap note); ADR 0243 D6; ADR 0207 (reading, model M1 to M5, the set-ups T1 to T3 and the measured values of its
  addendum); `crates/rcfd-fv/src/sponge.rs` in full; `crates/rcfd-fv/src/compressible.rs` lines 1 to 1275 (the solver,
  `step_ale`, `replace_mesh`, `step_sponged`, `step_sponged_shifted`, `characteristic_face_state`);
  `crates/rcfd-fv/tests/sponge_f331.rs` (the tube, the measure, U1, U2, S2, B2); lines 1 to 260 of
  `crates/rcfd-fv/tests/compressible_ale_f258.rs` (the translating mesh and the motion of set-up U); the S3 test of
  `sponge_f340.rs` (lines 905 to 930); the sponge sections of `docs/acoustics.md`.
- **Searched:** the tests and documents for the text of the two refusals (only `is_err()` is asserted; no test matches the
  message).
- **Arithmetic, one scratch Python script, no rcfd code** (`w49i-work/f341-scratch/decay_model.py`, not committed): the
  scalar recursion of DR4 below, a uniform offset with the rate 2 on a cell whose volume changes linearly in time by
  `A k t` (`A = 0.05`, `k` = 4.44, -4.44, 2.0, the range of `d_x s + d_y s` of the deformation of set-up G below), the
  step 0.01 and 0.008, to `t = 1`, in three variants of the volume that multiplies the source. Result, relative error of
  the offset against `exp(-2 t)`: the stage volume of DR4 `-4.4e-7`, `-1.1e-6`, `-5.6e-7` (step 0.01) and `-2.3e-7`,
  `-5.5e-7`, `-2.8e-7` (0.008); the volume of the start of the step for every stage `2.0e-3`, `-2.5e-3`, `9.5e-4`; the
  volume of the end of the step `-2.0e-3`, `2.5e-3`, `-9.5e-4`; a static cell `-6.8e-7` (ADR 0237 U2 measured `6.84e-7`).
- **By hand:** DR1 to DR7 and the set-up numbers below.
- Nothing was compiled and no test was run before this text.

## What F-341 asks

Acceptance (binding): "With a sponge, a uniform reference state stays to round-off under `step_ale` on a deforming mesh;
the ADR 0237 tube check on a uniformly translating mesh reflects less than 1 percent; without a sponge `step_ale` is
bit-identical." The job adds: derive the frame of the target state and of the profile when the mesh moves, the meaning of
the matched form's memory on a moving cell, the interaction of the geometric conservation law of `step_ale` with the
damping term, and what `replace_mesh` does with the memory; the F-258 bit records and the tests of ADR 0233, 0237, 0243 and
0248 unedited.

**Reading (registered here).**

1. "Round-off" is `1e-12` relative, the reading of ADR 0207 (reading 2) and the limit of ADR 0237 U1.
2. "A deforming mesh" is the motion of ADR 0207 set-up U (an interior bump, a dilation, a rotation and a translation, every
   boundary face moving normal to itself), run to `t = 2`.
3. "The ADR 0237 tube check on a uniformly translating mesh" is the tube of ADR 0237 A0 (grids, pulse, layer, probe, the
   measure against a reference run on an extended grid, `t_end = 1.6`) with every point of both grids moving at the
   constant velocity `V = (-0.2, 0, 0)` (DR6), with the boundary behind the layer the Thompson boundary (as A0) and, in
   place of the rigid wall of A0s and P0 (a rigid wall that moves at `V` is a piston and pushes the gas), the pressure
   release `PressureOutlet { p0 }`, which reflects fully (`R = -1`) and does not disturb the gas at rest when it moves.
   All four sponge variants (plain, matched, shifted) and a control without sponge are run.
4. "Without a sponge `step_ale` is bit-identical": every run that does not set a sponge produces the bits it produced
   before (the F-258 records R1 to R4 and every existing test pass unedited), and, further, a sponge whose rates are all
   zero, a sponge set and cleared, and the new frame setting without a sponge give the same bits (B2).

## Derivations

Notation: the cell `c` with the volume `V_c(t)`, moving with the mesh; `w` the mesh velocity, `w_n` its normal component on
a face; `U` the lab-frame conserved state; `S(U, Q)` the sponge source per unit volume of ADR 0237 D1 (plain, matched) and
ADR 0248 DR14 (shifted); `sigma` the rate; `Q` the memory (and `r`, `m` in the shifted form).

**DR1. The frame of the target state.** The plain source `-sigma (U - U_ref)` is covariant under a Galilean change of
frame `x' = x - V t` when `U_ref` transforms like `U`: with `m' = m - rho V`, `E' = E - m.V + rho |V|^2/2`, the source of
`m'` is `-sigma (m - m_ref) + sigma V (rho - rho_ref) = -sigma (m' - m'_ref)`, and the energy likewise (the map from `U` to
`U'` is linear in `U`). The solver's variables are lab-frame variables also in `step_ale` (ADR 0090 D2: the flux is
`F.n - w_n U` with `U` in the lab frame). So the target state is a **lab-frame state**: it is the state of the gas far from
the region of interest, in the frame where it is given (for a body moving through still air: at rest), whatever the mesh
does. This needs no change: `Sponge::reference` is used as it is. The matched forms need a reference at rest (ADR 0237
D1): at rest in the lab frame.

**DR2. The frame of the profile: attached to the mesh.** Two choices exist.

- *Fixed in space.* `sigma = sigma(x)` with `x` the current cell centre, and the memory a field of space: its equation
  `dQ/dt = p - p_ref` holds at a fixed point, so on a moving cell `d(V Q)/dt = V (p - p_ref) + sum_f Q_f W_f / dt` (the
  memory crosses the moving faces as a field at rest crosses a moving control surface; `W_f` the swept volume). It needs
  the layer's geometry at every step (the `Sponge` of ADR 0237 holds only one rate per cell index) and a transport of every
  memory with an upwind face value, which smears the memory by a numerical diffusion of about `h |w| / 2`.
- *Attached to the mesh.* `sigma_c` and the memories belong to the cell and move with it; the layer is the set of cells that
  had a rate when the sponge was set. It needs no new data: the rates and the memories of ADR 0237 and ADR 0248 are
  already per cell.

The attached frame is built, for three reasons: (i) a domain whose outer boundary moves (a whole mesh that translates with
a body, the translating tube of the acceptance) keeps its layer in front of its boundary only if the layer moves with the
boundary; with a layer fixed in space the boundary runs out of the layer or into it (in the tube with `V = -0.2` the layer
behind the probe would shrink from 0.2 to 0.08 by `t = 0.6`, when the pulse reaches the end, and vanish at `t = 1.0`; at
0.08 its two-way attenuation is `exp(-2 I (0.08/0.2)^3) = 0.41` instead of `1e-6`); (ii) on a deforming mesh whose outer boundary stays (the usual case: the mesh moves near a body), the cells of an
outer layer hardly move, and the two frames coincide up to the displacement of those cells; (iii) no transport of the
memory, so no numerical diffusion of it. The fixed-in-space frame is not built (question Q1).

The rates of the second layer of the shifted form (ADR 0248, `sponge_rate2`, evaluated at the cell centres when the sponge
is set) are attached in the same way. The `normal` of the matched forms is a fixed lab-frame vector; a mesh whose layer
rotates is outside what the matched forms are derived for (limit L3).

**DR3. The memory on a moving cell.** In the attached frame the memory of a cell is the time integral of `p_c - p_ref`
along the path of the cell, `Q_c(t) = integral from t_set to t of (p_c - p_ref) dt'`: the derivative following the mesh,
`(d/dt + w.grad) Q = p - p_ref`. In the frame of a layer that moves with the mesh this is exactly the memory of ADR 0237
DR4, so the per-cell ordinary differential equation of `step_sponged` is kept unchanged: the memory is intensive (not
multiplied by the volume), it does not cross faces, and it is advanced with the stage weights of the state. The same holds
for `r` and `m` of the shifted form (`dr/dt = q - a r`, `dm/dt = (m_U - m_ref) - a m`). A cell carries its memory with it;
in the attached frame it does not move through the layer, the layer moves with it.

**DR4. The source in the ALE step and the geometric conservation law.** The integral form on the moving cell is
`d(V U)/dt = -sum_f (F.n - w_n U) |S_f| + V S(U, Q)`. `step_ale` (ADR 0090 D2) integrates `Z = V U` with the SSP-RK stages
`Z_s = a Z^n + b (Z_p + dt R(U_p))`, `U_s = Z_s / V_s`, `V_s = V^n + c_s dv` with `dv = sum_f W_f`, `(a, b, c_s)` = `(0, 1, 1)`,
`(3/4, 1/4, 1/2)`, `(1/3, 2/3, 1)` for SSP-RK3 (and the first one, or the first two with `(1/2, 1/2, 1)`, for Euler and RK2).
The volume `V_s` is the result of the same stage combination applied to `dV/dt = dv/dt`; it is the volume of the stage
state `U_s` at the stage time (`t^n + c_s dt`). The source enters as

    Z_s = a Z^n + b (Z_p + dt [R(U_p) + V_p S(U_p, Q_p)]),    V_p = V^n + c_p dv,

`c_p` the `c` of the stage that produced `U_p` (0 for `U^n`). Consequences:

- *A uniform state equal to the reference.* `R(U0) = U0 dv/dt` (ADR 0207 M4: the physical fluxes of a uniform state sum
  to zero on a closed cell and `sum_f w_n |S_f| = dv/dt`), and `S(U_ref, Q) = 0` for the plain form; for the matched forms
  `S` is zero when `U = U_ref` and `grad Q = 0`, and `Q` stays zero because `p - p_ref = 0`. So `Z_s = V_s U0` and the state
  stays `U_ref` up to round-off (clause 1), for any deformation, any rate and any of the three forms. The round-off
  `delta` of the GCL in `U` gives a source of `sigma delta`, which the relaxation damps: it does not grow.
- *A uniform offset with a uniform rate.* With `Y = U - U_ref` uniform, `R(U_p) = U_p dv/dt` and the stage is the SSP-RK
  stage of `Z' = (V'/V - sigma) Z`, `Z = V Y`, with `V` linear in time within the step, whose exact solution is `Y(t) =
  Y(0) exp(-sigma t)` independent of the motion. SSP-RK3 is third order for this non-autonomous equation; the scratch model
  gives `2e-7` to `1.1e-6` relative at `t = 1` (G1 below). If the source were multiplied by `V^n` (or `V^{n+1}`) in every
  stage, the decay rate would be `sigma V^n / V(t)` and the error `2.5e-3` (the model): the stage volume is what makes the
  damping consistent with the GCL.
- The source is evaluated with the geometry of the new mesh (the face area vectors and volumes that `add_source` uses for
  the gradient of `Q`), as `step_ale` evaluates every flux on the new geometry; for a gradient of a field this is the
  Green-Gauss gradient of that geometry.

**DR5. The matched forms on a mesh that moves: what is and is not matched.** In the frame of an attached layer that moves
at a constant `V`, the gas at rest in the lab has the velocity `-V`. In one dimension (normal incidence) the matched forms
are the plain form (the memory enters only through its gradient along the layer, which is zero), and the plain form with
any rate `sigma(x, t)` keeps the two characteristics `w+- = p' +- rho0 c u'` uncoupled, also with a mean flow `U0`:
`(d_t + (U0 +- c) d_x) w+- = -sigma w+-`. So the layer reflects nothing at normal incidence on a translating mesh, and a
wave crossing it twice is attenuated by

    exp(-I/(c + U0) - I/(c - U0)) = exp(-2 I / (c (1 - M^2))),    M = U0/c,    I = integral of sigma over the layer,

slightly more than at rest. At oblique incidence the construction of ADR 0237 DR4 (a stretching of the normal coordinate
for a medium at rest) does not hold with a mean flow: the convective term mixes the time and the normal derivative, so the
layer is not matched; the mismatch is of order `M` (not derived further here), and for a mean flow along the layer the
matched layer of the Euler equations can have growing modes (Tam, Auriault and Cambuli 1998; Hu 2001; from memory). No
oblique check on a moving mesh is registered (limit L2).

**DR6. The tube on a translating mesh.** Every point moves as `x(t) = x(0) + V t`, `V = (-0.2, 0, 0)` (the outlet speed of
ADR 0207 T3: a relative Mach number 0.2). Relative to the mesh the gas at rest moves at `+0.2`; the pulse runs at 1.2, a
returning wave at 0.8. The layer of ADR 0237 D7 (`I = sigma_max L / 3 = ln(1e6)/2 = 6.908`) attenuates a wave that crosses
it twice by `exp(-2 x 6.908 / 0.96) = 5.6e-7` (DR5). Timing in mesh coordinates: the pulse (`x = 0.5`) reaches the layer
front (1.0) at `t = 0.42` and the end (1.2) at `0.58`; what the end returns reaches the probe (0.8975) at `0.58 + 0.3025/0.8
= 0.96`, plus a tail of `5 w / 0.8 = 0.25`: 1.21, inside `t_end = 1.6`. The reference grid's end (3.0) is reached at `2.5/1.2
= 2.08 > 1.6`. The boundary `xmin` (`Fixed(ambient)`, moving out of the gas) and the boundary behind the layer see the gas
at rest as a uniform state: by ADR 0207 M3 and M4 a moving `Fixed`, `NonReflecting` or `PressureOutlet` face in a uniform
gas that matches its data returns that gas, so the motion alone sends no wave. The pressure release with the gas leaving
through it at `M = 0.2` relative to the face holds `p = p0` on the face: `R = -1` in linear theory (ADR 0207 SN measured
1.000 on a moving pressure outlet). Both runs of a pair move in the same way, and the probe is the cell with the centre
0.8975 at `t = 0` in both, so the measure of ADR 0237 cancels the ALE error of the incident pulse as it cancels the
scheme's loss at rest.

**DR7. The mesh replacement.** `replace_mesh` installs a mesh of any topology and a state that the caller has already
mapped to it; the solver has no map from the old cells to the new. The three choices:

- *remap*: needs that map (or the layer's geometry and a conservative interpolation of the memories), which the solver does
  not have;
- *reset* (new rates from somewhere, memories at zero): a matched layer with a wave inside it and its memory at zero is the
  start that ADR 0237 amendment 1 found to ring at 4 to 9 percent in a rigid duct; a silent reset would do that at every
  layer event;
- *refuse*: what the solver does today (ADR 0237 D5).

Registered: **refuse**, unchanged, in both frames. The caller who has the map does the replacement explicitly: clear the
sponge, `replace_mesh`, set a sponge built on the new mesh, and set the memories mapped by the caller with
`set_sponge_memory` or `set_sponge_memories` (or leave them at zero, knowingly). S4 checks that this sequence works.

## Decisions

- **E1. Opt-in frame.** `rcfd_fv::sponge::SpongeFrame { StaticMesh, AttachedToMesh }`, `#[default] StaticMesh`;
  `CompressibleSolver::set_sponge_frame(SpongeFrame)` and `sponge_frame()`. The frame is a setting of the solver, kept
  across `set_sponge` calls. With `StaticMesh` (the default) `step_ale` refuses a sponge as today; with `AttachedToMesh` it
  takes the sponged ALE step (E2). Why an opt-in and not a lifted refusal: the tests S2 of ADR 0237 and S3 of ADR 0248
  assert that `step_ale` refuses a sponge and are not edited; and a layer whose frame was not chosen should not move
  silently. Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).
- **E2. The sponged ALE step.** A separate private method `step_ale_sponged`, called from `step_ale` when a sponge is set and
  the frame is `AttachedToMesh`. It repeats the geometric statements of `step_ale` (swept volumes, new geometry, `wn`), adds
  `V_p S(U_p, Q_p)` (DR4) to the undivided residual of each stage in the cells with a positive rate (or a positive rate of
  the second layer), and advances the memories with the stage weights (DR3). It keeps `wn` for `mesh_speeds` as
  `step_ale` does. Without a sponge `step_ale` runs its existing statements unchanged, so the default is bit-identical by
  construction; with a sponge whose rates are all zero no residual is touched, so the state is bit-identical to the run
  without sponge (B2).
- **E3. `replace_mesh`** refuses a sponge in both frames (DR7); its message names the explicit sequence.
- **E4. Not changed:** `step`, `step_sponged`, `step_sponged_shifted`, `max_dt`, `max_dt_ale`, `flux_sum`, `sponge.rs`'s
  sources, `compressible_bc.rs`; the mass-spring-damper wall stays refused on moving meshes and together with a sponge
  (ADR 0243 D6); `max_dt_ale` does not know the sponge (`rate dt` well below 2.5, as at rest).
- **E5. No case-schema wiring**, no file of `rcfd-api` edited (the sponge itself is library-only, F-331).

Code: `crates/rcfd-fv/src/sponge.rs` (the enum), `crates/rcfd-fv/src/compressible.rs` (a field, two methods, the branch in
`step_ale`, `step_ale_sponged`, the message of `replace_mesh`), new `crates/rcfd-fv/tests/sponge_f341.rs`,
`docs/acoustics.md` (prose). No existing test file is edited.

## Checks and limits (registered)

Common: `gamma = 1.4`; `CompSettings::default()` (HLLC, MUSCL van Leer, SSP-RK3, CFL 0.4); the frame `AttachedToMesh`
unless a check says otherwise; a moving run takes `dt = max_dt_ale()` (lockstep runs: the smallest over the runs) and the
mesh at `time + dt` built with `rcfd_mesh::motion::moved_mesh`, unless a check says otherwise.

### Clause 1: a uniform reference state on a deforming mesh

Set-up U1: `box_2d(12, 8, 1.2, 0.8)`, the motion of ADR 0207 set-up U (the function `u_motion` of
`compressible_ale_f258.rs`, copied), to `t = 2` with the schedule of ADR 0207 (steps clipped to the end). Every cell starts
at the reference; all four patches `Fixed(reference)`. Layer `SpongeLayer { origin (0.6, 0, 0), normal x, thickness 0.6,
rate_max 20, exponent 2 }` (half of the cells; `rate dt <= 20 x 0.02 = 0.4`).

| id | form, reference | limit | estimate |
|---|---|---|---|
| U1p | plain, `rho 1.2, u (0.3, 0.1, 0), p 0.9` | drift `<= 1e-12` after every step | DR4; ADR 0207 U1 `1.9e-14`, ADR 0237 U1p `1e-16` |
| U1m | matched (normal x), `rho 1.2, p 0.9` at rest; also the memory `max |Q| / (p_ref t)` | `<= 1e-12` each | DR4 |
| U1s | shifted, `shift 0.25`, `along 0.05`, second layer `origin (0, 0.4, 0), normal y, thickness 0.4, rate_max 20, exponent 2` (a corner); also `max |q| / (p_ref t)`, `max |r| / (p_ref t^2)`, `max |m| / (rho_ref a_ref t)` | `<= 1e-12` each | DR4 |
| U1g (set-up) | steps of each run; the largest `|w_n|` of `mesh_speeds()` on each of the four patches | `>= 100`; `>= 0.02` each | ADR 0207 U2: 152, `0.12` to `0.40` |

Drift: `max over cells and components of |U_k - U_ref,k| / scale_k`, `scale = (rho_ref, rho_ref a_ref, rho_ref a_ref, rho_ref
a_ref, E_ref)` (ADR 0237 U1).

### Clause 2: the tube on a uniformly translating mesh

Set-up T (DR6): test `box_2d(240, 1, 1.2, 0.005)`, reference `box_2d(600, 1, 3.0, 0.005)`, both moving at `V = (-0.2, 0, 0)`
(points `p0 + V t`); the pulse of ADR 0237 (`eps g(x - 0.5)`, an exact isentropic simple wave running right in the gas at
rest); `xmin` `Fixed(ambient)`; the layer of ADR 0237 D7 with its front at `x = 1.0` at `t = 0` (`R0 = 1e-6`, `L = 0.2`,
exponent 2, `sigma_max = 103.616`); the reference's `xmax` Thompson (`NonReflecting { p0, 0 }`); probe: the cell with the
centre 0.8975 at `t = 0`; all runs in lockstep until `t >= 1.6`; window: all steps; `R` the measure of ADR 0237.

| id | form, boundary behind the layer | limit | estimate |
|---|---|---|---|
| T0m | matched, Thompson (A0 of ADR 0237 on the moving mesh) | `R <= 0.01` | ADR 0237 A0 `9.2e-7` at rest; DR5 |
| T0mr | matched, `PressureOutlet { p0 }` | `R <= 0.01` | DR6 `5.6e-7` plus the scheme; ADR 0237 A0s `1.1e-6` |
| T0pr | plain, `PressureOutlet { p0 }` | `R <= 0.01` | as T0mr (DR5: the same equations in one dimension) |
| T0sr | shifted (`shift 0.25`, `along 0`, one layer), `PressureOutlet { p0 }` | `R <= 0.01` | ADR 0248 A0 with the shift `1.2e-6` in front of a rigid wall |
| NCr | no sponge, `PressureOutlet { p0 }` (control) | `R >= 0.9` | `R = -1` (DR6); ADR 0207 SN `1.000` |

The acceptance clause is met when T0m, T0mr, T0pr and T0sr meet the limit and NCr holds.

### Clause 3: bit identity

- **B1.** Pass unedited, release: `cargo test -p rcfd-fv --lib compressible`, `--lib sponge`, and the test files
  `compressible_bc`, `compressible_ale_f258`, `compressible_ale_f258_records` (the F-258 bit records), `impedance_f243`,
  `sponge_f331`, `sponge_f340` (fast tests), `msd_wall_f332` (fast tests), `ale_gcl`, `ale_flows`; and
  `cargo test -p rcfd-api --release --test baseline_hashes` (a shared solver file is changed).
- **B2 (new).** Set-up U of ADR 0207 with its four characteristic conditions (state `rho 1, u (0.3, 0.1, 0), p 1`; the
  conditions of `compressible_ale_f258.rs`), 30 steps of `step_ale` with the schedule: (i) nothing set; (ii) a plain sponge
  set and cleared; (iii) the frame `AttachedToMesh` set, no sponge; (iv) the frame set and a plain, a matched and a shifted
  (one layer, `shift 0.25`, `along 0.05`) sponge whose rates are all zero (reference at rest): the conserved state of every
  cell and `mesh_speeds()` are bit-identical (`to_bits`) between (i) and each other run. Control: the plain layer of U1
  with the reference at rest (`rho 1, p 1`) changes the state of the cells.

### G: the source in the ALE step (DR4)

Set-up G: `box_2d(20, 20, 1, 1)`, all four patches `Slip`; the boundary points fixed and the interior deformed,
`x(t) = x0 + 0.05 t sin(pi x0) sin(pi y0) (1, 1, 0)` (so the boundary faces do not move; `d_x s + d_y s` in `[-4.44,
4.44]`; the volume of a cell changes by up to 22 percent by `t = 1`); a fixed step `dt = 0.005`, 200 steps, to `t = 1`;
reference `rho 1, p0 = 1/1.4` at rest; start `rho 1.001`, `p 1.002 p0`, at rest; the rate 2 in every cell.

| id | check | limit | estimate |
|---|---|---|---|
| G1p | plain: `(U_k - U_ref,k)/(U_k(0) - U_ref,k)` against `exp(-2 t)` for mass and energy in every cell | relative `<= 1e-5` | DR4 and the model: `1e-7` to `6e-7` at this step |
| G1m | matched (normal x): as G1p; and `Q` of each cell against the trapezoid sum over the steps of the recorded `p - p_ref` of that cell | `<= 1e-5`; `<= 1e-4` | as G1p; ADR 0237 U2m `3.3e-5` (`dt` 0.01 there) |
| G1u | plain and matched: the largest momentum magnitude over the cells | `<= 1e-8` | the cells decay with RK errors that differ by up to `1e-6` relative (the model, `k` = +-4.44), a pressure non-uniformity of about `1e-10`, velocities of about `1e-10` |
| G2s | shifted (`shift 0.25`, `along 0`, one layer, normal x) on set-up G against the same sponge on the undeformed mesh with `step` and the same steps: the largest relative difference of `U_k - U_ref,k` (mass, energy), `q` and `r` over the cells (relative to the largest value of each on the undeformed mesh) | `<= 1e-5` | DR4: the uniform solution does not depend on the motion; RK error differences as G1p |

### S4: the frame setting and the refusals

| id | check | limit |
|---|---|---|
| S4a | the default frame is `StaticMesh`; with it `step_ale` refuses a plain, a matched and a shifted sponge and leaves the state, the time and the step count unchanged | error / unchanged |
| S4b | `AttachedToMesh`: `step_ale` accepts each form; `mesh_speeds()` is `Some` after it; `replace_mesh` refuses a sponge in both frames and leaves the solver unchanged; `set_sponge_frame` without a sponge is accepted and kept across `set_sponge(None)` and `set_sponge(Some)` | ok / error as stated |
| S4c | the explicit replacement (DR7): clear the sponge, `replace_mesh`, set a sponge built on the new mesh, `set_sponge_memory` with a given vector: accepted, the memory is that vector, and `step_ale` runs | ok |
| S4d | `step_ale` with a sponge in `AttachedToMesh` and a mesh of another topology: error; a mass-spring-damper wall and a sponge still cannot be set together | error |

## Mutations (registered; on a scratch copy, not committed)

Each is applied to `compressible.rs` in a `git archive` copy with its own target directory; then
`cargo test --release -p rcfd-fv --test sponge_f341` is run once (and the named existing test where stated).

| id | mutation | expected to be caught by |
|---|---|---|
| MV1 | the source multiplied by `V^n` in every stage | G1p, G1m (model: `2e-3`), G2s |
| MV2 | the source added per unit volume to the undivided residual (the factor `V_p` dropped) | G1p, G1m, T0mr, T0pr, T0sr |
| MV3 | the memories not advanced in the ALE step | G1m (memory), G2s |
| MV4 | the source dropped in the ALE step (the refusal lifted, nothing added) | G1p, G1m, T0mr, T0pr, T0sr (the release end alone: `R ~ 1`) |
| MV5 | the frame ignored: `step_ale` takes the sponged step in the default frame | S4a; the S2 test of `sponge_f331` and the S3 test of `sponge_f340` |
| MV6 | `r` of the shifted form not advanced in the ALE step (`q` and `m` are) | G2s |
| MV7 | the sign of the source in the ALE step | G1p, G1m, T0mr |

A mutation that no check catches is reported as surviving.

## Decision rule (registered)

- **Acceptance met** when U1p, U1m and U1s meet `1e-12` with U1g held (clause 1); T0m, T0mr, T0pr and T0sr meet 1 percent
  with NCr held (clause 2); B1 and B2 hold (clause 3).
- **done** (proposed): the acceptance met, G1p, G1m, G1u, G2s and S4 met, and every mutation caught. The acceptance asks for
  library behaviour only; the sponge itself is reachable from Rust only (F-331 partial for its wiring), which is said in the
  note.
- **partial**: a clause of the acceptance missed or not run, or a G or S4 check missed (the source would not be shown to be
  integrated consistently with the GCL), or a mutation surviving; the miss named with its number and diagnosis.
- **todo**: the sponged ALE step is not usable (a T0 run unstable or not finite).

## Limits

- L1. Attached frame only: a layer that should stay in space while cells move through it (a sliding or rotating region
  inside the layer) is not built (Q1).
- L2. The matched forms on a moving mesh are derived for a medium at rest in the frame of the layer; with a mesh velocity
  `w` the medium moves at `-w` relative to the layer. Matched at normal incidence (DR5); not matched at oblique incidence
  (order `|w|/c`, not quantified) and, for a velocity along the layer, possibly unstable (literature, from memory); no
  oblique or tangential-motion check.
- L3. The `normal` of the matched forms is a fixed lab vector; a rotating mesh rotates the attached rates but not the
  normal: the matched forms are not for a layer that rotates. The plain form has no normal and is not affected.
- L4. On a deforming mesh the attached profile deforms with the cells; for the matched forms a profile that is no longer a
  function of the normal coordinate alone is a mismatch (small where the layer cells hardly move).
- L5. `replace_mesh` refuses a sponge (DR7); a remap of the memories is the caller's.
- L6. Not with a mass-spring-damper wall (ADR 0243 D6, unchanged); explicit source, `max_dt_ale` does not know the rate;
  library only; two-dimensional grids checked.

## Sources

- The ALE form and its discrete GCL: ADR 0090 D2, ADR 0033, ADR 0207 M4 (this repository).
- The sponge forms: ADR 0237 DR1, DR2, DR4 and ADR 0248 DR14 (this repository); M. Israeli, S. A. Orszag, J. Comput. Phys.
  41 (1981); J.-P. Berenger, J. Comput. Phys. 114 (1994); F. Q. Hu, J. Comput. Phys. 173 (2001) (matched layer with a mean
  flow); C. K. W. Tam, L. Auriault, F. Cambuli, J. Comput. Phys. 144 (1998) (instability of the matched layer with a mean
  flow). The papers are cited from memory; none was opened, and nothing used here depends on them beyond the statement of
  limit L2.

## Open questions (with recommendations)

- Q1. A layer fixed in space (DR2, first choice): the layer's geometry kept in the sponge, the rates re-evaluated at every
  step, the memories transported across the swept volumes. Recommendation: only when a case needs it (a sliding or
  rotating region inside the layer).
- Q2. The frame setting in the case schema, with the sponge's wiring (F-331 question 1). Recommendation: with that wiring,
  as a key `frame = "mesh"` beside the layer, default "static" (refused on moving meshes); not taken here (approval).
