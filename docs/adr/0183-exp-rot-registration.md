# ADR 0183: EXP-ROT (F-276) registration: rigid rotation against the rotating frame on FAN-01, the coupled solver with the rotation implicit, and the hub trailing-edge pocket in time

Status: **REGISTERED 2026-11-04** (design only). No code of this experiment has been written and no run of it has been
made.
The arms, prerequisites, construction checks, limits, predictions, reading rules, decision mapping, budgets and stop rules
are fixed here. Amendments follow this text, each written before what it covers. Results go in an addendum. Not yet
accepted by the maintainer.

Design: [`docs/designs/exp-rot.md`](../designs/exp-rot.md). Parent design and milestone (both on the unmerged branch
`m0-gate-roadmap` when this was written): `docs/designs/unblock-roadmap-fan01-fea.md` goal G7 and Technical Plan Part 2,
ADR 0182 (`docs/adr/0182-tiered-gate-and-nightly.md`), milestone EXP-ROT in `docs/milestones.json` (requires M-0, item
F-276). Parents in this tree: [ADR 0082](0082-wave36c-rotating-frame-angular-momentum-source.md),
[ADR 0085](0085-wave37b-rotor-passage-robustness.md), [ADR 0086](0086-wave37c-fan01-solution-verification.md),
[ADR 0033](0033-wave20b-dynamic-mesh.md), [ADR 0076](0076-wave35a-transient-seams-and-relaxation.md),
[ADR 0110](0110-wave43c-fan01-rotor-passage-non-convergence-diagnosis.md),
[ADR 0113](0113-wave44c-mrf-cyclic-seams.md), [`docs/dynamic-mesh.md`](../dynamic-mesh.md),
[`docs/milestone-fan.md`](../milestone-fan.md). FAN-01 jobs 49K to 49R (ADRs 0174, 0175, 0177 to 0181) are on the unmerged
branch `job49r-fan01-hub-mesh` and are cited by number; none of them is accepted by the maintainer, so their
recommendations are used here as recommendations.

House rules ([ADR 0130](0130-wave48a-overview-and-run-record-contract.md), ADR 0181): limits are fixed before the
registered runs; a missed limit or a falsified prediction becomes an ignored `*_known_miss` test with the limit intact; a
defective set-up is corrected by an amendment written before the corrected run, with a new test, and the old one stays
ignored; every new code path is opt-in and the defaults stay bit-identical; `crates/rcfd-api/tests/baseline_hashes.rs` is
not re-recorded; each registered run is made once and nothing is re-run to pick a result; no commit trailers. Clean-room:
first principles, our own ADRs, and the published FAN-01 data of ADR 0075. No vendor code or documentation.

## Context

G7 asks two questions, and ADR 0181 question 1 adds a third:

1. **Q-ROT.** Does rigid rotation of the mesh in the absolute frame remove the spurious outlet angular momentum of the
   empty passage? ADR 0086 measured it in the relative frame (`rotating_frame`, SST, SIMPLEC, Barth-Jespersen
   `linear_upwind`) on the C-grid family E3 / E2 / E1 (5 040 / 13 882 / 38 688 cells): L = -0.009518, 0.059609, 0.105977
   N m, exact 0, growing with refinement (E1 stopped by the wall clock). ADR 0082 located a first-order part of it (upwind
   convection of the rigid part of the relative velocity).
2. **Q-LC.** Does the limiter limit cycle of the bladed passage stay when the rotation term is treated differently? The
   design names ADR 0085 arm C: the coupled pressure-velocity solver, rotationally periodic seams, the rotation term as
   implicit cross-component blocks, on the absolute-velocity rotor passage F-RP of ADR 0082.
3. **Q-POCKET** (ADR 0181 question 1). Is the near-stagnant pocket behind the hub trailing edge of B2 (z 0.1716, cells
   c7, c57, c107, c157, c621, c1235) physically unsteady? ADR 0181 recommends a report-only time-accurate check on B2.

Code read for this registration (read only, main at edb4839):

- `crates/rcfd-mesh/src/motion.rs` gives rigid poses and GCL-exact swept volumes. `crates/rcfd-api/src/dynmesh.rs` runs a
  prescribed rotation (`rotation_angle_expression`) for `piso` / `pimple`, Euler only, and refuses turbulence and
  momentum sources. It requires at least one moving patch, and the following patches default to the moving ones.
  `crates/rcfd-fv/src/ale.rs` gives a following wall the mesh's tangential velocity plus a normal part that carries the
  swept volume; any other wall keeps its own condition.
- `solver.linear_upwind_limiter = "barth_jespersen"` is refused for every algorithm except `simple` / `simplec`
  (`sim.rs`, the check after the engine is built). The `tvd_*` names map to `ConvScheme::Tvd` for every incompressible
  algorithm (`conv_scheme` in `sim.rs`); their behaviour on the moving-mesh step was not checked.
- `rotating_frame` and `mrf` sources are refused with `piso` / `pimple` (`crates/rcfd-api/src/sources.rs`).
- `CoupledSolver::with_solids` refuses periodic patches. With `algorithm = "coupled"`, `sim.rs` refuses turbulence and
  sources ("closed-domain laminar flow only"). `coupled.rs` has no limiter.
- F-RP and the arm C code were never committed: `git log --all -S rotor_passage -- crates` is empty. The patches named in
  ADR 0085 (`w37b_frp_and_arm_c_removed.patch`) were not found in a bounded search of the session scratch folders and
  `E:/projects/cfd` on 2026-11-04. They are treated as lost.
- ADR 0113: the faceted hub and shroud of `axial_fan_sector` miss a surface of revolution by up to 1.84e-2 (hub) and
  4.5e-3 (shroud) of `Omega r |S|`. A rigidly rotating face of that patch therefore sweeps a volume of that relative size.

Fixed values (ADR 0075 / 0086): Omega = 155.614 rad/s about +z through the origin, 9 blades, r_hub 0.124 m, r_tip
0.2475 m, shroud 0.250 m, axial extent 0.4 m (inlet z = 0), U_in = 9.478 m/s (Q = 1.4032 m^3/s), nu = 1.5e-5 m^2/s,
rho = 1.2 (assumed). One revolution takes 40.377 ms, one blade passage 4.486 ms and one flow-through 42.20 ms. Full-annulus
values are 9 times the sector values.

## Decisions

**D1. Three parts.** Arm 1 answers Q-ROT (laminar, absolute frame, rigid rotation). Check H answers Q-POCKET (SST,
relative frame, time-accurate, static mesh). Arm 2 answers Q-LC (coupled solver, F-RP, rotation implicit). Each verdict
stands on its own. The decision mapping (D9) combines them.

**D2. Departure from the design, stated now.** The design says the laminar runs of arm 1 need no new code. The code read
above shows that they do. The hub and shroud of the empty passage are at rest, and the case tree cannot run a rigid
motion without a following patch. A plain `fixed` 0 wall on a rotating faceted patch leaves a relative (ALE) flux equal to
the swept volume. The control and the arm also need one convection scheme, and Barth-Jespersen is not on the PISO path.
Prerequisites P-A and P-B (D3) close these gaps.

**D3. Prerequisites** (written and checked before any registered run; opt-in, default bit-identical; sizes are estimates,
basis in the design):

| Id | What | For | Size |
|---|---|---|---|
| P-A | Barth-Jespersen limiter of `linear_upwind` on the `piso` / `pimple` momentum convection, including the moving-mesh step, using the same `gradient::Limiter::BarthJespersen` on the same least-squares gradient as `SimpleSolver`; the existing key is accepted for `piso` / `pimple` | arm 1, check H | S |
| P-B | A wall at rest on a rigidly moving mesh: face velocity `(m_f / abs(S_f)) n_f` (no tangential part; the normal part carries the swept volume, as the following wall of ADR 0033 D3 does); a rigid prescribed motion is allowed with no following patch | arm 1 | S |
| P-C | `rotating_frame` with `pimple` on a static mesh: the frame acceleration (centrifugal and Coriolis) through the ADR 0045 transient source hook, re-evaluated every outer iteration; the casing `moving_wall` with `wall_velocity_projection = "tangential"` on that path; the seam body-force term with the coupled mean `rAU` (ADR 0076 Y5) | check H | M |
| R-1 to R-6 | Arm 2 rebuild (D7) | arm 2 | L in total |

Effort bound (ADR 0085): a prerequisite that fails its bit-identity check (V0) or the construction checks of its first
user is removed. The part that needs it is reported NOT RUNNABLE with the reason.

**D4. Arm 1 runs** (the empty family E3, E2, E1 and the bladed B3, B2 of ADR 0086, construction `camber_seam_h`, laminar,
convection `linear_upwind` with `barth_jespersen` in every run):

| Run | Frame and algorithm | Walls | Mesh |
|---|---|---|---|
| R1(h) control | relative, `rotating_frame` (Omega, centrifugal), SIMPLEC, steady: the ADR 0086 case text with `[turbulence] model = "laminar"` and the boundary turbulence lines removed | ADR 0086 empty set-up (hub and shroud `moving_wall` at -Omega, projection tangential) | E3, E2, E1 |
| Y1(h) arm | absolute, `dynamic_mesh` rigid, `rotation_angle_expression = "155.614 [1/s] * t"`, axis z, origin 0, PISO, Euler | hub and shroud at rest (P-B) | E3, E2, E1 |
| I1(h) inertial control | absolute, no mesh motion, PISO, Euler, otherwise Y1 | hub and shroud `fixed` 0 | E3, E2, E1 |
| I1S(h) algorithm control, report only | absolute, no mesh motion, no source, steady SIMPLEC, otherwise R1 (no code needed) | hub and shroud `fixed` 0 | E3, E2, E1 |
| Y1b(h) bladed, report only | as Y1 | hub and blade follow the mesh; shroud at rest (P-B) | B3, B2 |

Inlet `fixed` (0, 0, 9.478) in the absolute runs, outlet fixed static pressure 0 with zero-gradient velocity, initial
velocity (0, 0, 9.478). R1 keeps the ADR 0086 inlet and initial state, which are the same flow in the relative frame.
Every other key stays at the case-tree default. Exact solution in the empty passage: axial, axisymmetric, with zero
absolute swirl, for any nu and in either frame. The PISO seam coupling is lagged within a step (ADR 0033), but the settled
empty-passage state is steady at every cell of the moving mesh (`c_x = c_y = 0`, `c_z(r, z)`), so the lag vanishes
there. The bladed state is not steady in Cartesian components, so Y1b is report only.

Time step `Omega dt = 1 deg` (dt = 1.12156e-4 s, 360 steps per revolution); I1 uses the same dt. Run length: until
settled (below), at most 15 revolutions (5 400 steps). Fallback, used at most once: if the smoke (V3) of Y1(E3) or
Y1b(B3) diverges, amendment 1 sets `Omega dt = 0.5 deg` for every arm-1 run before any registered run. If it diverges
again, arm 1 is NOT RUNNABLE at this cost.

**D5. Check H** (report only; B2 only, the unchanged mesh of ADR 0181): the ADR 0086 bladed case text (SST,
`rotating_frame`, Barth-Jespersen `linear_upwind`, ADR 0086 walls and inlet) with `algorithm = "pimple"`, `n_outer = 5`,
`time_scheme = "bdf2"`, from the ADR 0086 initial state. Two runs: **H(dt)** with dt = T_c / 100 = 4.605e-5 s
(T_c = hub chord 0.099 m / hub relative speed 21.50 m/s = 4.605 ms) and **H(dt/2)**. Each runs 0.2532 s (6 flow-through
times). The first 0.1266 s is start-up and the last 0.1266 s is the window. It starts from the uniform initial state, not
from the stalled SIMPLEC iterate, for three reasons. The iterate is a cycle, not a solution (ADR 0110). A hand-over
across algorithms is untested code. A physical oscillation does not depend on the start.

**D6. Functionals** (full annulus; ADR 0075 / 0086 definitions): outlet absolute angular-momentum flux
`L = 9 rho sum_outlet (x c_y - y c_x)_f phi_f`; outlet swirl energy `rho <c_theta^2 / 2>_m`; Delta p_tt (mass-averaged
absolute total pressure, outlet minus inlet); torque `M = -9 (moment of the fluid on blade + hub)` about z; the
angular-momentum excess `L - (torque of all walls on the fluid)`, exact 0 in every run. On the rotating mesh, `phi_f` on
the outlet is the absolute flux. The outlet plane turns in itself, so its mesh flux is zero (V2).

**D7. Arm 2 rebuild and runs.** What must be rebuilt (none of it is in the repository):

| Item | Content (ADR 0082 / 0085 specification) | Size |
|---|---|---|
| R-1 F-RP | `sources.<name>.rotor_passage` for `mrf`: frame flux `(Omega x r_f).S_f` also on boundary faces of zone cells; per-cell frame-flux closure `<= 1e-10` replacing the surface-of-revolution check (and so the ADR 0113 refusal of `axial_fan_sector`); turbulence on the relative flux | S-M |
| R-2 coupled seams | rotationally periodic pairs in `coupled.rs`: pressure implicit across the pair in the gradient, continuity and Rhie-Chow rows, the partner velocity rotated in the continuity rows, momentum through lagged partner coefficients | M |
| R-3 flux offset | the F-RP frame flux in the coupled convection | XS |
| R-4 implicit rotation | `-Omega x c` as cross-component blocks `M_xy = -Omega V`, `M_yx = +Omega V` seen by GMRES; Rhie-Chow body-force term lagged | S-M |
| R-5 wiring | `sim.rs`, `turb.rs`, `sources.rs`: SST and `mrf` sources with `algorithm = "coupled"`, only with `rotor_passage = true` | S |
| R-6 limiter (new, not in ADR 0085) | Barth-Jespersen `linear_upwind` in the coupled convection. ADR 0085 arm C used `upwind`, and unlimited `linear_upwind` trips the guard within 7 to 27 iterations on these meshes (ADR 0110) | S |

ADR 0085 arm C converged only on the empty H-slit passage (M0 476, M1 567 iterations, `upwind`, SST); it did not converge
on M2 within 60 minutes. It has never run bladed, on the C-grid, or with a limiter.

Runs (B3 and B2, ADR 0086 bladed set-up with F-RP absolute conditions as in ADR 0085 F1: hub and blade
`velocity_expression_x/y = -Omega y, Omega x`, shroud `fixed` 0; SST; Barth-Jespersen `linear_upwind`; 3000 iterations
from iteration 1, the ADR 0181 protocol): **Y2(h)** coupled with R-1 to R-6; **Y2S(h)** the same F-RP case on SIMPLEC
(rotation explicit, as ADR 0082), the attribution control. References: the ADR 0181 step-1 unchanged-mesh SIMPLEC runs
(rotating frame), reproduced in the same process (V1).

**D8. Concurrency and order.** At most two registered runs at a time. Order: R1 and I1S families (records committed),
then Y1 and I1 families, then Y1(E2) at dt/2 (A1-T), then Y1b; H(dt), then H(dt/2); arm 2 last, after its rebuild.
Records: JSON per run under `docs/investigations/exp-rot/`, written to an absolute path.

**D9. Decision mapping.** The rows marked "design" are the design's decision rule. The other rows are not covered by
the design; they go to the maintainer as questions 3 and 6, and this ADR does not decide them.

| Outcome | Then | Source |
|---|---|---|
| Q-ROT **YES** | M-FAN-A1 is built on rigid rotation, time-marched, which needs F-277 | design |
| Q-ROT **NO** | the cause is not the rotating-frame term; A1 stays on a rotating-frame solve and its design reopens the diagnosis | design |
| Q-LC **STAYS** in arm 2 and H is **STEADY** or **NUMERICAL** | the blocker is the mesh or the scheme, not the rotor model; A1's design starts from the M-FAN-B mesh | design (the "both arms" clause read as below) |
| Q-ROT **NOT REPRODUCED** or **NOT EVALUABLE** | open question 3 | this ADR |
| H **PHYSICAL** | open question 6 (cycle means as the reported values; steady convergence not expected) | ADR 0181 Q1 reading |
| H **NOT EVALUABLE** (or not run) | the clause of row 3 is not read, so row 3 does not fire | this ADR |

The design's "the limit cycle stays in both arms" cannot be read from Y1b: laminar flow at a chord Reynolds number of
about 1.4e5 is expected to be unsteady behind a trailing edge, so a Y1b oscillation is not evidence of a limit cycle.
This registration reads the clause from arm 2 (STAYS) and check H (STEADY or NUMERICAL; a NOT EVALUABLE H gives no
reading). Open question 4 asks the maintainer to
confirm this reading.

## Registered limits and checks (fixed now)

### Validity and construction checks (a failure is a defective set-up: amendment before any registered run)

| Id | Check | Limit (source) |
|---|---|---|
| V0 | defaults unchanged after P-A, P-B, P-C and R-1 to R-6: `baseline_hashes.rs` unchanged; `fan01_stall_w43c.rs` non-ignored tests; the dynamic-mesh tests of `crates/rcfd-api/tests/dynamic_mesh.rs` and `crates/rcfd-fv/tests/ale_flows.rs`; the coupled lid-cavity hashes (12 x 12, nu 0.01, 20 iterations, `upwind` and `linear_upwind`) recorded on the main of the implementation before any coupled code is written (the ADR 0085 values were recorded on feb015f and are not reused) | bit-identical |
| V1 | ADR 0181 step-1 references reproduce in arm 2's process: B3 1.089e-3 / 4.87e-4, 1.9176 N m, 46.704 Pa, 5.742 m/s; B2 1.004e-3 / 6.23e-4, 2.0308 N m, 63.860 Pa, 8.248 m/s | printed digits |
| V2 | arm 1 construction, every mesh, steps 1 to 360 of the smoke: GCL `max_c abs(sum_f W_f - (V1 - V0)) <= 1e-12 mean(V)`; continuity (`StepInfo::continuity`) `<= 1e-9`; total volume `abs(V(t) - V0) <= 1e-12 V0`; seam pairs at step 1 `abs(m_f + m_g) <= 1e-12 Omega r abs(S)`; inlet and outlet mesh flux `<= 1e-12 Omega r abs(S)` per face; hub and shroud `max abs(m_f) / (Omega r abs(S))` reported (expected near ADR 0113's 1.84e-2 / 4.5e-3) | ADR 0101 (i), (ii); ADR 0085 A1 form |
| V3 | smoke: one revolution of Y1(E3) and Y1b(B3), 50 iterations of R1(E3), 200 steps of H(dt), 30 iterations of Y2(B3) and Y2S(B3): finite, no guard trip; wall time per step or iteration recorded | finite |
| V4 | arm 2 construction (ADR 0082 / 0085 A1): F-RP closure `<= 1e-10` on B3 and B2; seam pairs at iteration 1 `abs(phi_rel,f + phi_rel,g) <= 1e-12 Omega r abs(S)`; 30-iteration smoke of Y2(B3) with finite continuity | as stated |
| V5 | P-B: an annulus sector (`annulus_3d`, cells aligned with theta, rotational seams) with walls at rest and axial inflow, rigidly rotating for one revolution, keeps `max abs(c_theta)` at most `max(10 C0, 1e-9 U)`, where `C0` is the same quantity with the mesh at rest | the A1-L form |

If V4 fails, arm 2 is NOT RUNNABLE, and its code is removed (ADR 0085 effort bound).

### Arm 1 (Q-ROT)

**Settled** (Y1, I1, Y1b): revolution means of L (Y1b: torque and L) of two consecutive revolutions differ by at most
`max(0.5 % abs(value), 1e-4 N m)`, and `abs(m_in + m_out) <= 1e-3 abs(m_in)`. This is the ADR 0086 cap rule with one
revolution in place of 1000 iterations. The value is the mean of the last revolution, and its max - min spread is
reported. A run not settled at 15 revolutions is NOT SETTLED.

**R1 convergence**: the ADR 0086 rules unchanged (residual tolerance 1e-5, or at the 8000 cap a last-1000 change of L at
most `max(0.5 % abs(L), 1e-4 N m)` and of the swirl energy at most `max(0.5 %, 0.01 Pa)`, mass balance 1e-3). The wall
clock is 90 minutes instead of ADR 0086's 60, because ADR 0086 E1 missed 60 minutes by 82 iterations. Anything else is
NOT CONVERGED.

**Pathology (P)**, from R1 only, evaluated before any Y1 record is read: P is defined only when all three R1 runs
converge; otherwise P is **NOT EVALUABLE**. **P-GROW** iff `abs(L_R1(E1)) > abs(L_R1(E3))` and
`abs(L_R1(E1)) >= 1e-2 N m`; otherwise **P-ABSENT**. P-GROW means the ADR 0086 growth
is reproduced laminar. The 1e-2 N m floor makes the A1-R limit (1 % of it) no smaller than the 1e-4 N m settle floor, so
a removal by two orders can be measured. ADR 0086 E1 had 0.106 N m.

| Id | Limit | Source |
|---|---|---|
| A1-R | `abs(L_Y1(E1)) <= 0.01 abs(L_R1(E1))` | ADR 0085 A2 fallback |
| A1-N | `abs(L_Y1(E1)) <= abs(L_Y1(E2)) <= abs(L_Y1(E3))`, and if all three exceed 1e-4 N m also `p = ln(e_E3 / e_E1) / ln(1.9727) >= 0.5` (r31 = (38 688 / 5 040)^(1/3)); if all three are at or below 1e-4 N m, "not measurable (floor)" and counted as met | ADR 0085 A4; ADR 0086 floor |
| A1-L | on every member `abs(L_Y1(h)) <= max(10 abs(L_I1(h)), 1e-4 N m)` | ADR 0085 A2 |
| A1-E | on every member `Delta p_tt(Y1(h)) <= 0` (a passive duct with walls at rest gains no total pressure) | ADR 0085 A3 |
| A1-T | time-step floor: Y1(E2) is also run at dt/2; if `abs(L_Y1(E2)) > 1e-4 N m` and L changes by more than 50 % between dt and dt/2, A1-N and A1-R are "not measurable (time-step floor)"; if it changes by more than 10 %, Y1(E3) and Y1(E1) are run once more at dt/2 and those runs count (the dt runs are reported) | ADR 0085 floor rule, applied to dt |

**Verdict Q-ROT** (exactly one; the classes are tested in this order and the first that applies is the verdict):
1. **NOT EVALUABLE**: P is NOT EVALUABLE; or a Y1 or I1 run diverged, was NOT SETTLED or hit its cap; or A1-T floored
   (A1-R and A1-N then neither met nor missed).
2. **NOT REPRODUCED**: P-ABSENT. Y1 is still run and reported against A1-N, A1-L and A1-E. Those results say whether
   rigid rotation is clean on these meshes, but they do not say that it removes the ADR 0086 growth.
3. **YES**: P-GROW; A1-R and A1-N met.
4. **NO**: P-GROW; A1-R or A1-N missed.

A1-L and A1-E do not enter the verdict. A miss of either is a known miss and is reported beside the verdict.

**Algorithm confound.** Y1 runs on PISO and R1 on steady SIMPLEC, so A1-R compares the frame and the algorithm together
(ADR 0085 used one algorithm for each arm and its control; A1-L does, Y1 against I1). I1S, the inertial case on R1's
algorithm, is reported beside I1. If on some member `abs(L_I1S)` and `abs(L_I1)` differ by more than a factor of 10 and
either exceeds 1e-4 N m, the verdict (YES or NO) is labelled **ALGORITHM-CONFOUNDED**: on that member the algorithm alone
moves L by more than the margin of A1-L.

**Y1b (report only, no limit)**: settled or not; torque, L, the angular-momentum excess and Delta p_ts (revolution means
and spreads); the five cells with the largest velocity amplitude over the last revolution, with generator tags; the
amplitude at the ADR 0181 site cells; wall seconds per revolution. The last number feeds the 4-hour budget of G8.

### Check H (Q-POCKET; report only)

Signal: the relative speed `abs(w)` of the six B2 site cells (c7, c57, c107, c157, c621, c1235), every step of the
window. Amplitude `A` = the largest over the six cells of `(max - min) / 2`. Frequency `f = (n - 1) / (t_n - t_1)`
from the `n` upward zero crossings of the mean-removed signal of the cell with the largest `A`. A DFT peak is reported
beside it; at a window of 0.1266 s its bin width (7.9 Hz) is too coarse for the 10 % test below.

Outer-iteration rule: in at least 99 % of the window's steps, the velocity change of the 5th outer iteration is at most
1e-2 of the 1st. If not, the run is NOT TIME-ACCURATE.

Class (exactly one; tested in this order):
- **NOT EVALUABLE** (tested first): a budget hit, a divergence, or a NOT TIME-ACCURATE run in H(dt) or H(dt/2).
- **STEADY**: `A <= 0.01 Omega r_hub` (0.193 m/s) in H(dt) and in H(dt/2).
- **PHYSICAL**: `A` above that in both runs; the window holds at least 5 periods (`n >= 6`) in both; at least 20 steps
  per period in H(dt); `f(dt/2) / f(dt)` in [0.9, 1.1]; `A(dt/2) / A(dt)` in [0.75, 1.33].
- **NUMERICAL**: `A` above the threshold, and either the period in steps agrees within 10 % between H(dt) and H(dt/2), or
  the period is at most 4 steps.
- **NOT EVALUABLE**: anything else.

Reported beside the class, without a limit: the time means of torque and Delta p_ts over the window against the ADR 0181
B2 cycle means (2.0308 N m, 63.860 Pa); the seam-pair flux sum (the ADR 0076 seam body-force risk); the five
largest-amplitude cells.

### Arm 2 (Q-LC)

The ADR 0181 limits, per run, on B3 and B2:

| Id | Limit |
|---|---|
| C1 | `res_u` and `res_p` both below 1e-5 at some iteration in 1001-3000 |
| C2 | `is_converged()` true at iteration 3000 |
| C3 | `abs(du)_2` at iteration 3000 at most 0.01 of the reference stall's `abs(du)_2` at iteration 1000 (B3 5.742 m/s, B2 8.248 m/s) |

**Verdict Q-LC** for Y2: **GONE** iff C1-C3 hold on B3 and B2; **STAYS** iff C1-C3 are all missed on B3 and on B2;
**MIXED** otherwise; **NOT RUNNABLE** under V4. Y2S gets the same limits. Attribution, reported without a rule: Y2 GONE
with Y2S not GONE points at the coupled solve with the rotation implicit; both GONE points at the formulation; Y2 STAYS
says that none of the three changes matters on these meshes. Reported per run, without a limit: torque and Delta p_ts
at the end against the ADR 0181 references, the five largest-`abs(du)` cells with tags, r and z, and the share of
`sum abs(du)^2` in the hub trailing-edge columns (ADR 0181 definition).

## Predictions (mine; moderate confidence)

| Id | Prediction |
|---|---|
| PR1 | P-GROW: the laminar R1 family reproduces the growth of L (it comes from the scheme and frame on this family, not from SST) |
| PR2 | Q-ROT YES, with `abs(L_Y1)` at or below 1e-4 N m on all three members |
| PR3 | A1-E met (Y1 creates no total pressure), where ADR 0086 E had +4.5 to +9.2 Pa |
| PR4 | Y1b(B2) does not settle (laminar unsteadiness behind the trailing edges) |
| PR5 | H is STEADY: the SST pocket has a steady state in time, so the SIMPLEC cycle is an iteration property |
| PR6 | Q-LC STAYS: ADR 0110 falsified the explicit Coriolis source as the cause, and ADR 0180 found every limiter cycling at the same B2 hub cells |

## Budgets and stop rules

Estimates now, from ADR 0086 (SIMPLEC 0.044 / 0.108 / 0.280 s per iteration on E3 / E2 / E1, 0.048 / 0.107 / 0.301 on
B3 / B2 / B1) and ADR 0085 (coupled 0.3 / 0.8 / 3.6 s per iteration at 4 320 / 13 524 / 47 040 cells). A PISO step
including the mesh move is assumed to cost about two SIMPLEC iterations. The caps are fixed in amendment 1 as three times
the V3-projected wall time of the registered length. No physical parameter (dt, run length, window, iteration count)
changes with the smoke, except the single dt fallback of D4.

| Runs | Estimate per run |
|---|---|
| R1 E3 / E2 / E1 | 5 / 15 / 45 min |
| I1S E3 / E2 / E1 | 5 / 15 / 45 min |
| Y1, I1 E3 / E2 / E1 (15 revolutions at most) | 10 / 25 / 65 min |
| Y1b B3 / B2 | 10 / 25 min |
| H(dt) / H(dt/2) (5 498 / 10 996 steps, 5 outer) | 50 / 100 min |
| Y2 B3 / B2 (3000 iterations) | 18 / 40 min |
| Y2S B3 / B2 | 2 / 5 min |

Stop rules:
1. A run that hits its cap stops, writes its record with `budget_hit = true`, and counts as NOT SETTLED, NOT CONVERGED or
   NOT EVALUABLE. It is not re-run without an amendment.
2. A prerequisite that fails V0 or its user's construction check is removed, and its part is NOT RUNNABLE (D3).
3. Arm 1 smoke divergence: one dt fallback (D4), then NOT RUNNABLE.
4. After all registered runs of a part, nothing more is run in that part: no further arm, scheme, parameter, relaxation
   or mesh (ADR 0181 rule 6).
5. If every part is NOT RUNNABLE or NOT EVALUABLE, stop and report.

## Known-miss policy

Each registered limit has a test. A missed limit becomes an ignored `*_known_miss` test with the limit unchanged. A
falsified prediction is reported in the addendum, with no test. A set-up found defective before its run is corrected by
an amendment; the corrected run gets a new test and the old one stays ignored. Long runs are ignored tests run one per
process; construction, bit-identity and V5 run in the gate when they are fast. Prerequisite code that is kept is opt-in,
and its key is documented in `docs/case-schema.md`. Code of a part that is NOT RUNNABLE is removed.

## Not run (and why)

- An `upwind` pair in arm 1. ADR 0082 and ADR 0085 already showed that the absolute-velocity formulation removes the
  first-order upwind source where it converges. The open item is the growth under the limited scheme.
- SST in arm 1. It needs F-277 (k-omega SST on moving meshes), which `docs/dynamic-mesh.md` lists as missing.
- The finest bladed mesh B1 (38 184 cells) in Y1b, check H and arm 2. ADR 0181 used B3 and B2, and B1 would cost about
  three times B2.
- Local pseudo-time on B2 (`pseudo_mode = "local"`, no code). A converged result would only show that a steady state
  exists; it cannot show whether the flow leaves that state in time.
- The relative-frame coupled solver with implicit Coriolis, as arm 2. It is a smaller rebuild, but it is not the arm the
  design names (open question 5).

## Open questions for the maintainer (each with a recommendation)

1. **The FAN-01 jobs in flight (49K to 49R).** The parent design leaves open whether they finish, freeze, or are taken
   over. Recommendation: EXP-ROT takes over ADR 0181 question 1 (check H). No new steady-iteration scheme or mesh job
   until EXP-ROT reports. Merging those branches is a separate decision.
2. **Prerequisites P-A and P-B (D2).** Options: (a) build both, as registered; (b) drop P-B and give the empty passage a
   hub that follows the mesh, reading the angular-momentum excess instead of L (no wall code, but the parent set-up and
   the exact-zero L are lost); (c) drop P-A and use `tvd_van_albada` in every arm-1 run and in H (no limiter code; ADR
   0180 found it cycling at the same B2 hub cells, but the ADR 0086 growth was never measured with it); (d) `upwind`
   everywhere (no code; it answers only ADR 0082's first-order mechanism and damps the pocket). Recommendation: (a). One
   limiter wiring serves arm 1 and check H. The control and the arm then use the scheme of every FAN-01 result since
   ADR 0086.
3. **Q-ROT NOT REPRODUCED or NOT EVALUABLE.** The design maps only YES and NO. Recommendation: for NOT REPRODUCED, build
   A1 on rigid rotation provisionally if Y1 meets A1-N, A1-L and A1-E (the formulation is clean on these meshes), and
   repeat arm 1 with SST as part of F-277's verification before A1 is graded. For NOT EVALUABLE, keep A1's frame open,
   and decide after one registered amendment that addresses the cause.
4. **Reading "the limit cycle stays in both arms" (D9).** Recommendation: read it as arm 2 STAYS and H STEADY or
   NUMERICAL, not from the laminar Y1b, for the reason given under D9. A NOT EVALUABLE H gives no reading.
5. **The form of arm 2.** Options: (a) ADR 0085 arm C as the design names it (F-RP, coupled, rotation implicit; size L);
   (b) the relative frame on the coupled solver with Coriolis as implicit blocks (no F-RP; smaller; it separates the
   treatment of the rotation term from the change of formulation; untested anywhere). Recommendation: (a), as the
   design names it, with the Y2S control for attribution.
6. **H PHYSICAL.** Recommendation: report the window means of torque and Delta p_ts as the FAN-01 values (ADR 0180
   question 3) and stop steady-iteration fixes of the pocket. A1 is time-marched in any case if Q-ROT is YES.
7. **When arm 2 runs.** It is the costliest part (size L). Its answer changes A1 only when A1 stays on a steady
   rotating-frame solve. Options: (a) as the design says, both arms regardless; (b) build and run arm 2 only if Q-ROT is
   not YES. Recommendation: (b). With Q-ROT YES, A1 is time-marched on a moving mesh, and the "both arms" clause is read
   from check H alone (STEADY or NUMERICAL; not read if H is NOT EVALUABLE).

## Addenda

To be added in this order: amendment 1 (V0 to V5, the smoke values, the caps, the dt fallback if used); the R1 records
and the P verdict, committed before any Y1 record is read; arm 1 results and the Q-ROT verdict; check H; arm 2 if it
runs; the decision under D9.

### Amendment P-AB (2026-11-05): the set-up of P-A and P-B and of their checks, written before their code

Job EXP-ROT P-A/P-B, branch `exprot-pab` from main 7c960bb. Open question 2 is answered by the maintainer's loop
instruction with its recommendation (a): build P-A and P-B. This amendment makes the D3 rows and V5 specific enough to
code and test. It changes no limit of this ADR, and no part of the experiment (arms, runs, functionals, verdicts) is run
by this job. P-C and arm 2 are separate jobs.

**P-A as built.** `PimpleSettings` gets `linear_upwind_limiter: Option<gradient::Limiter>` (`None` in `piso()` and
`pimple()`). With `Some(kind)` and `convection = LinearUpwind`, the momentum predictor of `PimpleSolver::step_with`
multiplies the least-squares gradient of each velocity component by `gradient::limiter(geom, cells, bvals, grad, kind)`,
where `cells`, `bvals` and `grad` are the convected cell values, the boundary values and the unlimited gradient that the
convection already uses. The limited gradient goes to the convection term only; the Laplacian's non-orthogonal
correction keeps the unlimited gradient (as `SimpleSolver`, ADR 0081 part L). The moving-mesh step (`step_ale*`) runs
through the same code on the new geometry, so it is covered. The fractional step has its own convection and does not
use the field. Case tree: `solver.linear_upwind_limiter = "barth_jespersen"` is accepted with `linear_upwind` for
`piso` and `pimple` (constant density, with or without `dynamic_mesh` and turbulence). It stays a located error at the
key with any other scheme, with `solver.time_advance = "fractional_step"`, and with every other engine (coupled,
compressible, variable density, VOF). The line of ADR 0081 L0c that asserts the refusal for `pimple` is superseded by
D3. That case of the test `l0c_the_key_outside_its_scope_is_a_located_error` becomes `piso` with the fractional step.
The other lines stay. Turbulence, scalar and energy equations are not limited.

**P-B as built.** New key `dynamic_mesh.rest_patches` (list of patch names, default `[]`, with
`Schema::hash_omit_default` as ADR 0156 D6 and the ADR 0181 addendum F1). Each face of a listed patch gets the velocity
`u_b = (m_f / abs(S_f)) n_f` every step, with `m_f` the mesh flux and `S_f`, `n_f` the area vector and unit normal of
the new mesh. So `u_b . S_f = m_f` (no relative flux) and the tangential part is zero. The pressure condition is the
fixed-flux one of the following walls (ADR 0033 addendum, deviations): `dp/dn = -n . (u_b,new - u_b,old) / dt`.
At step 1 the old value is the initial wall value 0, so a face that sweeps volume gets a one-step start-up term
`-(m_f / abs(S_f)) / dt`. This is the impulsive start of the facets, as for a following wall whose initial value differs
from the mesh velocity. The library function is new (`ale::set_rest_conditions`); `follow_velocity` and
`set_follow_conditions` are not changed. Static checks, located: an unknown name, or a name that is also in
`moving_patches` or `follow_patches`, at `dynamic_mesh.rest_patches`; a non-empty list with `deformation = "smoothing"`
at `dynamic_mesh.rest_patches` (D3 registers a rigidly moving mesh only); a rest patch whose `velocity` is not `"fixed"`,
whose `velocity_value` is not zero, or that has a velocity expression, at that boundary key. `moving_patches` may be
empty when `motion = "prescribed"` and `deformation = "rigid"` (D3: "a rigid prescribed motion is allowed with no
following patch"); with `six_dof` or `smoothing` it stays required. With `rest_patches` empty every code path is the
one of today.

**Settings hash and written text.** The content hash of every existing case is unchanged by F1's flag. The text
`Case::to_toml` / `Case::to_json_string` write gains `rest_patches = []`, so ADR 0155 H1 (`case_hashes_w48u.rs`) gets
the ADR 0181 addendum F2 treatment: the normalisation also removes `dynamic_mesh.rest_patches` when (and only when) it
is the empty list; `DEFAULT_KEYS` holds JSON defaults instead of strings; C2 also asserts that every recorded valid
source holds `[]`; C4 also asserts that a copy with `rest_patches = ["inner"]` differs from R0. R0 and
`baseline_hashes.rs` are not edited or re-recorded. No fenced TOML block of `docs/` is edited.

**V5 set-up (a deviation from the literal row, for the maintainer).** The row says "axial inflow". With a velocity
inlet and walls with no slip the developing layers carry a radial velocity `c_r`. A radial vector's Cartesian
components turn with each cell of the rotating mesh. The Euler time term and the convection by the tangential mesh flux
cancel that turn only to `O(Delta theta^2, (Omega dt)^2)`, which leaves a swirl source of order
`Omega c_r (Delta theta^2 - (Omega dt)^2) / 6` per cell. That is a property of convecting Cartesian components on a
rotating mesh, not of the wall condition, and it is far above `1e-9 U` for any `c_r` above round-off. On the static
mesh C0 is round-off (mirror symmetry of each cell), so the limit is `1e-9 U`, and V5 would then fail for a reason that
D3's effort bound would charge to P-B. So V5 is run where `c_r = 0` exactly: an axially periodic sector with an axial
plug flow that decays through Stokes layers at the walls at rest (exact solution `c = c_z(r, t) e_z`). The literal
inflow variant is kept as V5-U, report only. The same mechanism bears on D4's premise that the settled empty passage
has `c_x = c_y = 0` at every cell: the laminar layers of the empty passage also carry `c_r`. This job does not change D4.

On `annulus_3d_sector` every wall face is a planar chord rectangle, symmetric under the half turn about its radial
centre line, so its swept volume under rotation about z is zero to round-off. V5 therefore tests "at rest against
following" and the run with no following patch, not the normal part. PB-1 tests the normal part on faces that sweep
volume.

**Checks (limits fixed now; gate tests unless marked).** Library constants: `Omega = 155.614` rad/s,
`U = 9.478` m/s, `nu = 1.5e-5` m^2/s, `dt = (pi/180)/Omega = 1.12158e-4` s (`Omega dt = 1` deg, 360 steps a
revolution), PISO (1 outer, 2 correctors), Euler, `linear_upwind` with Barth-Jespersen (P-A), momentum and pressure
tolerances `tol_rel = 1e-13`, `tol_abs = 1e-16` (ADR 0033 strict tests).

| Id | Set-up | Limit |
|---|---|---|
| V5 | `annulus_3d_sector(8, 8, 2, 0.124, 0.25, 0.05, 40 deg)` with `zmin` / `zmax` made a translational cyclic pair; `inner` and `outer` at rest (P-B), no following patch; initial `u = (0, 0, U)`, `p = 0`; one revolution rigidly about z through the origin, and the same 360 steps with the mesh at rest (C0). Quantity: `max` over steps and cells of `abs(c_theta)`, `c_theta = (x u_y - y u_x) / r` at the cell centre of that step | `max abs(c_theta) <= max(10 C0, 1e-9 U)` (the registered limit) |
| V5-a | the V5 rotating run | every step: continuity (`StepInfo::continuity`) `<= 1e-9`, GCL residual `<= 1e-12 mean(V)`; step 1: seam pairs `abs(m_f + m_g) <= 1e-12 Omega r_f abs(S_f)` (V2 forms). Reported: `max abs(m_f) / (Omega r_f abs(S_f))` on `inner` and `outer` |
| V5-N | negative control: the V5 rotating run with `inner` and `outer` following the mesh (`set_follow_conditions`) | `max abs(c_theta)` above the V5 limit, or the divergence guard stops the run (either shows that V5 can fail) |
| V5-U | report only, ignored test: z not periodic, `annulus_3d_sector(8, 8, 8, 0.124, 0.25, 0.2, 40 deg)`, `zmin` fixed `(0, 0, U)`, `zmax` pressure 0 with zero-gradient velocity; static and rotating | none; `max abs(c_theta)`, `max abs(c_r)` of both runs reported |
| PB-1 | through the case tree: `annulus_3d` 24 x 4 x 2, `r_in 0.124`, `r_out 0.25`, `lz 0.05`, inlet `zmin` fixed `(0, 0, U)`, outlet `zmax` pressure 0; `rest_patches = ["inner", "outer"]`, `moving_patches = []`, rigid prescribed rotation `155.614 [1/s] * t` about z through `(0.01, 0, 0)` (off the annulus axis, so the wall faces sweep volume), the constants above, 3 steps; `m_f` from `motion::swept_volumes` of the meshes before and after each step | every step, every rest face: `abs(phi_f - m_f) <= 1e-12 Omega r_out abs(S_f)` and `abs(u_b - (u_b . n) n) <= 1e-12 Omega r_out`; non-vacuity: `max abs(m_f) / (Omega r_out abs(S_f)) >= 1e-3` on the rest faces |
| PB-1N | negative control: PB-1 with `rest_patches = []` (plain `fixed` 0 walls; the rigid motion with no following patch) | `max abs(phi_f - m_f) >= 1e-3 Omega r_out abs(S_f)` on `inner` and `outer` (PB-1's limit is missed) |
| PB-2 | case tree, static checks | each located error above at its path; `moving_patches = []` accepted with a rigid prescribed motion and refused with `smoothing` and with `six_dof` |
| PB-3 | case tree, hash | key absent and `rest_patches = []`: equal settings value and content hash; `["inner"]`: a different hash, written by `to_toml`, and the written text parses back to the same value and hash |
| PA-0 | case tree, `cavity_2d` 20 x 20, `nu 0.01`, `linear_upwind`, 20 steps of `piso` and of `pimple` | the key absent and `"none"`: bit-identical `ux`, `uy`, `p` and flux |
| PA-1 | case tree, scope | accepted: `piso` and `pimple` with `linear_upwind`, and `piso` with `dynamic_mesh`; located at `solver.linear_upwind_limiter`: `piso` with `upwind`, `piso` with `time_advance = "fractional_step"`, `coupled` |
| PA-2 | library: `box_2d(100, 2, 1, 0.02)` periodic in y, `nu = 1e-6`, `u = (1, h(x), 0)` with `h = 1` for cell centres in `[0.1, 0.3)` and 0 elsewhere, `xmin` fixed `(1, 0, 0)`, `xmax` zero-gradient velocity and pressure 0; `dt = 0.005` (Courant 0.5), 80 steps; overshoot `O = max` over steps and cells of `max(u_y - 1, -u_y, 0)`; once unlimited (`O_LU`), once with P-A (`O_BJ`) | `O_LU >= 1e-3` (non-vacuity) and `O_BJ <= 0.5 O_LU` |
| PA-3 | library: the ADR 0033 G3 cavity (8 x 8, `nu 0.05`, 20 steps of 0.02, frame translating with `(0.6, 0.2, 0)`) with `linear_upwind` and P-A | `max abs(u_moving - V - u_fixed) <= 1e-9`; non-vacuity: at least one cell and component with a limiter factor below 1 in the final fixed-mesh state (count reported) |

V0 for P-A and P-B: `baseline_hashes.rs` unchanged and passing; the non-ignored tests of `fan01_stall_w43c.rs`,
`crates/rcfd-api/tests/dynamic_mesh.rs`, `crates/rcfd-fv/tests/ale_flows.rs` and `ale_gcl.rs`, `w36b_limiter.rs` (L0c as
amended above) and `case_hashes_w48u.rs` (H1 as amended above) pass. The coupled lid-cavity hashes of V0 belong to arm 2
(not applicable here).

A miss is reported with its number and kept as an ignored `*_known_miss` test with the limit intact. A miss of V5, V5-a
or PB-1 is a failed construction check of P-B, and D3's effort bound then applies; a miss of PA-2 or PA-3 is reported
beside P-A. Results follow in the next addendum.

### Addendum: results of amendment P-AB (2026-11-05)

Code in 9c2a7e0 (amendment d35759b before it). Files: `crates/rcfd-fv/src/pimple.rs` (P-A), `crates/rcfd-fv/src/ale.rs`
(P-B, `rest_velocity`, `set_rest_conditions`), `crates/rcfd-api/src/dynmesh.rs`, `sim.rs`, `schema.rs` (keys and
checks), tests `crates/rcfd-fv/tests/exp_rot_pab.rs` and `crates/rcfd-api/tests/exp_rot_pab.rs`, the amended
`case_hashes_w48u.rs` (H1) and `w36b_limiter.rs` (L0c), `docs/case-schema.md` (the lines of the two keys) and
`docs/dynamic-mesh.md`. Every check was run once, in release; nothing was re-run to pick a result.

| Id | Measured | Limit | Result |
|---|---|---|---|
| V5 | C0 = 0 exactly (the static run has no radial or tangential velocity); rotating `max abs(c_theta)` 1.213e-12 m/s (1.28e-13 U); `max abs(c_r)` 1.25e-12 m/s; 360 of 360 steps | `max(10 C0, 1e-9 U)` = 9.478e-9 m/s | met |
| V5-a | continuity max 4.16e-12; GCL max 6.27e-16 mean(V); seam pairs at step 1 3.65e-15 Omega r abs(S); wall sweep `max abs(m_f)` 2.10e-14 Omega r abs(S) (reported: round-off, as the amendment derived for chord faces) | 1e-9; 1e-12; 1e-12 | met |
| V5-N | following walls: `max abs(c_theta)` 8.32 m/s; the guard did not stop the run (360 steps) | above 9.478e-9 m/s, or a guard stop | met (V5 can fail) |
| V5-U | report only. At rest: `max abs(c_theta)` 8.53e-5 m/s, `max abs(c_r)` 1.76e-3 m/s; rotating: `max abs(c_theta)` 1.417e-4 m/s (1.50e-5 U), `max abs(c_r)` 1.68e-3 m/s; continuity max 3.9e-14 | none | reported |
| PB-1 | steps 1 to 3: `max abs(phi_f - m_f)` 1.38e-17 Omega r_out abs(S_f); tangential part at most 1.72e-17 Omega r_out; `max abs(m_f)` 3.97e-2 Omega r_out abs(S_f) | 1e-12; 1e-12; non-vacuity >= 1e-3 | met |
| PB-1N | plain `fixed` 0 walls: `max abs(phi_f - m_f)` 3.97e-2 Omega r_out abs(S_f) | >= 1e-3 | met (PB-1 can fail) |
| PB-2 | every registered located error at its path; `moving_patches = []` accepted with a rigid prescribed motion, refused with `smoothing` and `six_dof` | as registered | met |
| PB-3 | key absent and `[]`: equal settings value and content hash (07d69e902b96415d on the PB-1 case); `["inner", "outer"]`: 500fb527083c7fe5, written and round-tripped | as registered | met |
| PA-0 | `piso` and `pimple`: key absent and `"none"` bit-identical in `ux`, `uy`, `p` and flux after 20 steps | bit-identical | met |
| PA-1 | accepted for `piso`, `pimple` (and the limited run differs from the unlimited one in both) and `piso` with `dynamic_mesh`; located at the key for `piso` with `upwind`, with the fractional step, and `coupled` | as registered | met |
| PA-2 | overshoot unlimited 0.1134, limited 0 (the limited top hat stays within [0, 1] on every step) | `O_LU >= 1e-3`, `O_BJ <= 0.5 O_LU` | met |
| PA-3 | `max abs(u_moving - V - u_fixed)` 9.16e-16; 37 of 128 (cell, component) pairs with a limiter factor below 1 | 1e-9; at least 1 | met |
| H1 | 41 recorded sources, 0 differ after the extended normalisation; C3: 30 of 30 valid sources differ raw in the text columns only; C4: the copy with `rest_patches = ["inner"]` differs from R0 | as the ADR 0181 addendum F2 form | met |

**V5-U and the premise of the V5 deviation.** The amendment expected C0 to be round-off for the inflow variant (mirror
symmetry of each cell). It is not: the static sector with a velocity inlet has `max abs(c_theta)` 8.53e-5 m/s, about
5 % of its `max abs(c_r)`. A likely cause, not proven here, is the seam coupling, which PISO lags within a step (D4):
the lag turns a radial velocity at the rotational seams into a tangential error, and the maximum over the steps
includes the start-up. The rotating run has 1.66 times C0, so the literal inflow variant would also have met the
registered V5 limit (`10 C0` = 8.53e-4 m/s). The amendment's reason for the deviation therefore did not apply on this
mesh. The registered V5 (axially periodic, `c_r = 0`) stays the check of record, and it is the stricter one
(`1e-9 U` against `10 C0`). These two numbers do not separate the rotating-mesh swirl source that the amendment
derived from the seam effect; the rotating run exceeds the static one by 5.6e-5 m/s (5.9e-6 U). If the seam lag is the
cause, the static empty passage I1 (which has seams) carries an L floor of its own; A1-L compares Y1 against that
floor.

V0: `baseline_hashes.rs` unchanged and passing (not re-recorded); the full release test suites of `rcfd-fv`,
`rcfd-mesh` and `rcfd-api` were run on 9c2a7e0, including the non-ignored tests of `fan01_stall_w43c.rs`,
`dynamic_mesh.rs`, `ale_flows.rs`, `ale_gcl.rs`, `w36b_limiter.rs` and `case_hashes_w48u.rs`: 1557 passed, 0 failed, 622 ignored
(`cargo test -p rcfd-fv -p rcfd-mesh -p rcfd-api --release`).
`cargo fmt --check` and `cargo clippy --all-targets -D warnings` are clean on the three crates.

No limit was missed, so no `*_known_miss` test was added. P-A and P-B are kept (opt-in, defaults bit-identical). D3's
first-user checks (V2, V3 of arm 1) belong to the experiment and were not run.

### Amendment P-C (2026-11-05): the set-up of P-C and of its checks, written before its code

Job EXP-ROT P-C, branch `exprot-pc` from main 4e84c11. The D3 row of P-C names three items and the design (section 2)
a fourth, conditional one. This amendment makes them specific enough to code and test. It changes no limit of this ADR,
and no part of the experiment (arms, runs, functionals, verdicts) is run by this job. Arm 2 is a separate job.

**What exists (read on 4e84c11, no run).** The ADR 0045 hook already evaluates every `sources.<n>` once per outer
iteration on `piso` / `pimple` (`sim.rs`, `Sources::evaluate` inside the `step_with` hook). `RotatingFrame::add_force`
gives the centrifugal part analytically at cell and face centres and the Coriolis part from the velocity it is given
(boundary faces: the boundary velocity). Only `sources::check` refuses `rotating_frame` with `piso` / `pimple`. The
transient kernel has the opt-in `PimpleSettings::coupled_body_pair_mean` (ADR 0080 F-BP: the body-force face term of
every periodic face uses the coupled pair mean of `rAU`, the ADR 0076 Y5 term), which nothing in the API switches on. The
steady wall-pressure treatment of the rotating frame (`SimpleSolver::wall_pressure_trapezoid`, wave 10: the wall value
that enters the Green-Gauss gradient of a fixed-velocity wall cell is chosen so that the wall-normal gradient equals the
cell-centre body force) is not on the transient path (ADR 0045 D1: not ported). A `moving_wall` with
`wall_velocity_projection = "tangential"` builds the same fixed-value condition for every algorithm (`wall_motion.rs`).

**P-C as built.**
- `sources::check`: `rotating_frame` is accepted with `pimple` (constant density, iterative time advance). It stays a
  located error at `sources.<n>.type` with `piso` (D3 names `pimple`; one evaluation per step is a lagged Coriolis
  term), with `solver.time_advance = "fractional_step"` (existing rule), and with every other engine. `mrf` stays refused
  with `piso` and `pimple`. A moving mesh stays refused (the existing located error at `dynamic_mesh.enabled`, momentum
  sources), so P-C is the static mesh only. The line of ADR 0045 T6 (`transient_sources.rs`,
  `t6_unsupported_combinations_get_located_errors`) that asserts the refusal of `("pimple", "rotating_frame")` is
  superseded by D3; that case becomes `("piso", "rotating_frame")`. The other lines stay.
- Frame acceleration: no new hook code. The existing hook calls `Sources::evaluate` at the start of every outer
  iteration with the current iterate, so the Coriolis part is re-evaluated every outer iteration (and converges to the
  implicit term with the outer iterations).
- `Sources` gets a predicate that is true only when a `rotating_frame` source is configured. On the `pimple` engine it
  switches on `PimpleSettings::coupled_body_pair_mean` (the D3 seam term) and the new
  `PimpleSettings::wall_pressure_trapezoid` (below). Both are set once when the solver is built.
  `Sources::wall_pressure_trapezoid` (true also for acceleration, actuator disk and Lorentz sources) is not used for this,
  so every transient run with those sources stays bit-identical.
- Transient wall pressure (the design's conditional item): `PimpleSettings::wall_pressure_trapezoid` (default false).
  With a body force, the boundary values of the two Green-Gauss pressure gradients of `step_with` (momentum predictor and
  velocity correction) are the steady `gradient_wall_values` construction, moved to a crate-level function that the
  steady solver calls unchanged. The SIMPLEC wall correction of the steady path is not ported (PIMPLE has no `rAtU`).
  The fixed-flux pressure condition, the face fluxes and the pressure equation are unchanged.
- Casing `moving_wall` with the tangential projection: no new code; checked on this path (PC-4).
- No case key is added or changed. The settings value and content hash of every case are unchanged, and
  `case_hashes_w48u.rs` (H1) and `baseline_hashes.rs` are not edited. `Simulation::pimple_solver` / `pimple_solver_mut`
  (diagnostics and tests, as `simple_solver` / `simple_solver_mut`) are added.

**Gap that P-C does not close: check H needs transient SST.** D5 registers H with k-omega SST. `turb.rs` refuses
`k_omega_sst` (and every transported RANS model) with `piso` / `pimple`, and ADR 0136 lists URANS as missing. D3 does not
list it. After P-C the case tree runs `rotating_frame` with `pimple` laminar only, so check H as registered stays NOT
RUNNABLE, and D3's first-user construction check of P-C (V3, the 200-step smoke of H(dt)) cannot be run. This amendment
does not decide it. Open question 8 for the maintainer (the ADR has no recommendation, and none is given here): (a) a
further prerequisite, k-omega SST with `pimple` on a static mesh (time derivative in the k and omega equations, updated
within the outer iterations), registered by its own amendment before code; (b) H laminar, which changes D5 (D9 expects
laminar flow behind a trailing edge at a chord Reynolds number of 1.4e5 to be unsteady, so the reading of H would
change); (c) H NOT RUNNABLE, so row 3 of D9 does not fire.

**Checks (limits fixed now; gate tests unless marked).** Every run is made once, in release.

| Id | Set-up | Limit |
|---|---|---|
| PC-1 | case tree: `box_2d` 8 x 8, unit square, doubly periodic, `nu = rho = 1`, initial `u = (1, 0, 0)`, `rotating_frame` `angular_velocity = 0.5` about z, `centrifugal = false`; `pimple`, `euler`, `n_outer = 30`, `alpha_u = alpha_p = 1`, `upwind`, `dt = 0.1` (`a = 2 Omega dt = 0.1`), momentum and pressure `rel_tol = 1e-12`; 50 steps. Reference: the implicit-Euler recursion `u' = (u + a v)/(1 + a^2)`, `v' = (v - a u)/(1 + a^2)` | every step and cell: `abs(u - u_ref) <= 1e-9` (vector norm) and `abs(p) <= 1e-9` (the ADR 0045 T1 limits) |
| PC-1b | PC-1 with `n_outer = 1` | every step and cell within `1e-9` of the explicit recursion `u' = u + a v`, `v' = v - a u` (the source is evaluated from the iterate at the start of the outer iteration) |
| PC-1N | negative control: the PC-1b run against the implicit recursion | `max abs(u - u_ref) >= 1e-3` (PC-1's limit is missed) |
| PC-2 | case tree: the wave-9 rest state (`sources.rs` `rest_text(24)`: closed unit box 24 x 24, walls `fixed` 0, `nu 0.05`, `rotating_frame` `angular_velocity = 3` about `(0.5, 0.5, 0)`, centrifugal on) on `pimple`, `euler`, `n_outer = 2`, `n_correctors = 2`, `upwind`, `dt = 0.05` (`nu dt / dy^2 = 1.44`, below the ADR 0045 bound), 400 steps (t = 20 s, one viscous time `L^2 / nu`), from rest | final state: speed `<= 1e-3 Omega R_max` and pressure error `<= 1e-2` of the range (the wave-9 criterion of the steady test); the maximum over the steps is reported |
| PC-2N | PC-2 with `wall_pressure_trapezoid` switched off after the build | at least one PC-2 limit missed |
| PC-2b | case tree: the wave-9 rotating channel (`sources.rs` `ROT_CHANNEL`: `box_2d` 4 x 40, periodic in x, plates at y = 0 and 1, `nu 0.1`, Coriolis only, `angular_velocity = 2`) with the drive as a `sources.drive` acceleration `(1, 0, 0)` (`physics.body_force` is steady-only), `pimple`, `euler`, `n_outer = 2`, `upwind`, `dt = 0.01` (`nu dt / dy^2 = 1.6`), 3000 steps (t = 30 s, three viscous times `h^2 / nu`; the slowest start-up mode decays by `exp(-3 pi^2)`), from rest | final state: profile error `<= 1 %` of the centre-line velocity, and the transverse pressure difference within `1 %` of `-2 Omega` times the integral of the parabola (the registered wave-9 limits) |
| PC-3 | case tree: `annulus_3d_sector` 8 x 6 x 8, `r_in 0.5`, `r_out 1.0`, `lz 0.5`, 40 deg, `nu 0.01`, `rotating_frame` `angular_velocity = 2`, centrifugal on; inlet `zmin` fixed `(2 y + 0.2 x, -2 x + 0.2 y, 1 + y)` m/s (relative frame; not axisymmetric and with a radial part, so the two sides of a seam differ); outlet `zmax` pressure 0, zero-gradient velocity; `inner` fixed 0; `outer` `moving_wall` at rate `-2` with the tangential projection; `pimple`, `bdf2`, `n_outer = 3`, `n_correctors = 2`, `linear_upwind` with `barth_jespersen`, `dt = 0.05`, 10 steps. Quantity: `max abs(phi_f + phi_g) / max abs(phi_f)` over the `theta_min` / `theta_max` pairs | every step `<= 1e-8` (ADR 0080 P1) |
| PC-3N | negative control: PC-3 with `coupled_body_pair_mean` switched off after the build | above `1e-8` at some step |
| PC-4 | case tree: the ADR 0086 empty passage E3 (the `fan01_gci_w37c.rs` F3 counts, `camber_seam_h`, no blade), laminar (the `[turbulence]` block and the boundary turbulence lines removed), `rotating_frame` at Omega, hub and shroud `moving_wall` at `-Omega` with the tangential projection, the inlet and initial state of ADR 0086, `pimple`, `bdf2`, `n_outer = 5`, `linear_upwind` with `barth_jespersen`, `dt = 4.605e-5` s (D5's dt), 10 steps | every step, every hub and shroud face: `abs(phi_f) <= 1e-12 Omega r_f abs(S_f)`; non-vacuity: `max abs((Omega e_z x r_f) . n_f) / (Omega r_f) >= 1e-3` on those faces (ADR 0113: 1.84e-2 / 4.5e-3); every step finite with no guard trip. Reported: the seam pair sums (the PC-3 quantity), continuity, wall seconds per step |
| PC-4N | negative control: PC-4 with hub and shroud `velocity = "fixed"` and `velocity_expression_x/y = Omega y, -Omega x` (the same rigid wall velocity, not projected) | `max abs(phi_f) / (Omega r_f abs(S_f)) >= 1e-3` on hub and shroud (PC-4's limit is missed) |
| PC-5 | case tree, scope | accepted: `rotating_frame` with `pimple`; located at `sources.<n>.type`: `rotating_frame` with `piso`, `mrf` with `pimple` and with `piso`, `rotating_frame` with `pimple` and `time_advance = "fractional_step"`; located at `dynamic_mesh.enabled`: `rotating_frame` with `pimple` and a dynamic mesh; located at `turbulence.model`: `k_omega_sst` with `pimple` and `rotating_frame` (the gap above, recorded) |
| PC-6 | case tree, the flags of the built `pimple` solver | `rotating_frame` (alone, and with an acceleration): both flags true; no source, `acceleration`, `porous`, `actuator_disk`, `lorentz`: both false |
| PC-0b | scratch (not committed): bit hash of `ux`, `uy`, `p` and the flux on a `git archive` of 4e84c11 and on the final tree, separate target directories, built one after the other: steady SIMPLEC `rest_text(24)` and `ROT_CHANNEL` (200 iterations each); `pimple` with an acceleration, a porous, an actuator-disk and a Lorentz source (`plates_text` of `transient_sources.rs`, 20 steps) | bit-identical |

V0 for P-C: `baseline_hashes.rs` unchanged and passing; the non-ignored tests of `fan01_stall_w43c.rs`, `dynamic_mesh.rs`,
`ale_flows.rs`, `ale_gcl.rs`, `w36b_limiter.rs`, `case_hashes_w48u.rs` (untouched), `exp_rot_pab.rs`, `sources.rs`,
`rotating_foundation.rs`, `mrf.rs`, `transient_sources.rs` (T6 as amended above), `w35a_seam.rs` and the `rcfd-fv`
fingerprint tests pass; the full release suites of `rcfd-fv` and `rcfd-api`.

**Classes.** V0, PC-0b, PC-1, PC-1b, PC-2b, PC-3, PC-4, PC-5 and PC-6 are the construction checks of P-C: a miss is a
failed construction check, and D3's effort bound applies (P-C removed; H NOT RUNNABLE for that reason too). PC-2 and
PC-2N decide the transient wall pressure only: it is kept if PC-2 meets and PC-2N misses. If PC-2N meets, the treatment
is not needed on the transient path and is removed, and PC-2 is recorded on the run without it. If PC-2 misses, the
treatment is removed and PC-2 is an ignored `*_known_miss` test with the limit intact. A negative control (PC-1N, PC-2N,
PC-3N, PC-4N) that does not fire is a defective set-up of that control, corrected by an amendment before any re-run; it
is never counted as a pass. A missed limit is reported with its number and kept as an ignored `*_known_miss` test with
the limit intact. Results follow in the next addendum.
