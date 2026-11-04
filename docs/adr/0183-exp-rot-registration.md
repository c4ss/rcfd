# ADR 0183: EXP-ROT (F-270) registration: rigid rotation against the rotating frame on FAN-01, the coupled solver with the rotation implicit, and the hub trailing-edge pocket in time

Status: **REGISTERED 2026-11-04** (design only). No code of this experiment has been written and no run of it has been
made.
The arms, prerequisites, construction checks, limits, predictions, reading rules, decision mapping, budgets and stop rules
are fixed here. Amendments follow this text, each written before what it covers. Results go in an addendum. Not yet
accepted by the maintainer.

Design: [`docs/designs/exp-rot.md`](../designs/exp-rot.md). Parent design and milestone (both on the unmerged branch
`m0-gate-roadmap` when this was written): `docs/designs/unblock-roadmap-fan01-fea.md` goal G7 and Technical Plan Part 2,
ADR 0182 (`docs/adr/0182-tiered-gate-and-nightly.md`), milestone EXP-ROT in `docs/milestones.json` (requires M-0, item
F-270). Parents in this tree: [ADR 0082](0082-wave36c-rotating-frame-angular-momentum-source.md),
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
  (`sim.rs`, the check after the engine is built). The `tvd_*` schemes are accepted on every incompressible path.
- `rotating_frame` and `mrf` sources are refused with `piso` / `pimple` (`crates/rcfd-api/src/sources.rs`).
- `CoupledSolver::with_solids` refuses periodic patches. With `algorithm = "coupled"`, `sim.rs` refuses turbulence and
  sources ("closed-domain laminar flow only"). `coupled.rs` has no limiter.
- F-RP and the arm C code were never committed: `git log --all -S rotor_passage -- crates` is empty. The patches named in
  ADR 0085 (`w37b_frp_and_arm_c_removed.patch`) were not found in a bounded search of the session scratch folders and
  `E:/projects/cfd` on 2026-11-04. They are treated as lost.
- ADR 0113: the faceted hub and shroud of `axial_fan_sector` miss a surface of revolution by up to 1.84e-2 (hub) and
  4.5e-3 (shroud) of `Omega r |S|`. A rigidly rotating face of that patch therefore sweeps a volume of that relative size.

Fixed values (ADR 0075 / 0086): Omega = 155.614 rad/s about +z through the origin, 9 blades, r_hub 0.124 m, r_tip
0.2475 m, shroud 0.250 m, axial extent 0.4 m (inlet z = 0), U_in = 9.478 m/s (Q = 1.4032 m^3/s), nu = 1.5e-5 m^2/s, rho = 1.2
(assumed). One revolution takes 40.377 ms, one blade passage 4.486 ms and one flow-through 42.20 ms. Full-annulus
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
| **R1(h)** control | relative, `rotating_frame` (Omega, centrifugal), SIMPLEC, steady: the ADR 0086 case text with `[turbulence] model = "laminar"` and the boundary turbulence lines removed | ADR 0086 empty set-up (hub and shroud `moving_wall` at -Omega, projection tangential) | E3, E2, E1 |
| **Y1(h)** arm | absolute, `dynamic_mesh` rigid, `rotation_angle_expression = "155.614 [1/s] * t"`, axis z, origin 0, PISO, Euler | hub and shroud at rest (P-B) | E3, E2, E1 |
| **I1(h)** inertial control | absolute, no mesh motion, PISO, Euler, otherwise Y1 | hub and shroud `fixed` 0 | E3, E2, E1 |
| **Y1b(h)** bladed, report only | as Y1 | hub and blade follow the mesh; shroud at rest (P-B) | B3, B2 |

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

**D8. Concurrency and order.** At most two registered runs at a time. Order: R1 family (records committed), then Y1 and
I1 families, then Y1(E2) at dt/2 (A1-T), then Y1b; H(dt), then H(dt/2); arm 2 last, after its rebuild. Records:
JSON per run under `docs/investigations/exp-rot/`, written to an absolute path.

**D9. Decision mapping.** The rows marked "design" are the design's decision rule. The other rows are not covered by
the design; they go to the maintainer as questions 3 and 6, and this ADR does not decide them.

| Outcome | Then | Source |
|---|---|---|
| Q-ROT **YES** | M-FAN-A1 is built on rigid rotation, time-marched, which needs F-271 | design |
| Q-ROT **NO** | the cause is not the rotating-frame term; A1 stays on a rotating-frame solve and its design reopens the diagnosis | design |
| Q-LC **STAYS** in arm 2 and the pocket is not PHYSICAL in H | the blocker is the mesh or the scheme, not the rotor model; A1's design starts from the M-FAN-B mesh | design (the "both arms" clause read as below) |
| Q-ROT **NOT REPRODUCED** or **NOT EVALUABLE** | open question 3 | this ADR |
| H **PHYSICAL** | open question 6 (cycle means as the reported values; steady convergence not expected) | ADR 0181 Q1 reading |

The design's "the limit cycle stays in both arms" cannot be read from Y1b: laminar flow at a chord Reynolds number of
about 1.4e5 is expected to be unsteady behind a trailing edge, so a Y1b oscillation is not evidence of a limit cycle.
This registration reads the clause from arm 2 (STAYS) and check H (not PHYSICAL). Open question 4 asks the maintainer to
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

**Pathology (P)**, from R1 only, evaluated before any Y1 record is read: **P-GROW** iff all three R1 runs converge,
`abs(L_R1(E1)) > abs(L_R1(E3))` and `abs(L_R1(E1)) >= 1e-2 N m`; otherwise **P-ABSENT**. P-GROW means the ADR 0086 growth
is reproduced laminar. The 1e-2 N m floor makes the A1-R limit (1 % of it) no smaller than the 1e-4 N m settle floor, so
a removal by two orders can be measured. ADR 0086 E1 had 0.106 N m.

| Id | Limit | Source |
|---|---|---|
| A1-R | `abs(L_Y1(E1)) <= 0.01 abs(L_R1(E1))` | ADR 0085 A2 fallback |
| A1-N | `abs(L_Y1(E1)) <= abs(L_Y1(E2)) <= abs(L_Y1(E3))`, and if all three exceed 1e-4 N m also `p = ln(e_E3 / e_E1) / ln(1.9727) >= 0.5` (r31 = (38 688 / 5 040)^(1/3)); if all three are at or below 1e-4 N m, "not measurable (floor)" and counted as met | ADR 0085 A4; ADR 0086 floor |
| A1-L | on every member `abs(L_Y1(h)) <= max(10 abs(L_I1(h)), 1e-4 N m)` | ADR 0085 A2 |
| A1-E | on every member `Delta p_tt(Y1(h)) <= 0` (a passive duct with walls at rest gains no total pressure) | ADR 0085 A3 |
| A1-T | time-step floor: Y1(E2) is also run at dt/2; if `abs(L_Y1(E2)) > 1e-4 N m` and L changes by more than 50 % between dt and dt/2, A1-N and A1-R are "not measurable (time-step floor)"; if it changes by more than 10 %, Y1(E3) and Y1(E1) are run once more at dt/2 and those runs count (the dt runs are reported) | ADR 0085 floor rule, applied to dt |

**Verdict Q-ROT** (exactly one):
- **YES**: P-GROW; every Y1 and I1 run settled; A1-R and A1-N met.
- **NO**: P-GROW; every Y1 and I1 run settled; A1-R or A1-N missed.
- **NOT REPRODUCED**: P-ABSENT. Y1 is still run and reported against A1-N, A1-L and A1-E. Those results say whether
  rigid rotation is clean on these meshes, but they do not say that it removes the ADR 0086 growth.
- **NOT EVALUABLE**: an R1, Y1 or I1 run diverged, was NOT SETTLED or NOT CONVERGED, hit its cap, or A1-T floored.

A1-L and A1-E do not enter the verdict. A miss of either is a known miss and is reported beside the verdict.

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

Class (exactly one):
- **STEADY**: `A <= 0.01 Omega r_hub` (0.193 m/s) in H(dt) and in H(dt/2).
- **PHYSICAL**: `A` above that in both runs; the window holds at least 5 periods (`n >= 6`) in both; at least 20 steps
  per period in H(dt); `f(dt/2) / f(dt)` in [0.9, 1.1]; `A(dt/2) / A(dt)` in [0.75, 1.33]; both runs TIME-ACCURATE.
- **NUMERICAL**: `A` above the threshold, and either the period in steps agrees within 10 % between H(dt) and H(dt/2), or
  the period is at most 4 steps.
- **NOT EVALUABLE**: anything else, including a budget hit, a divergence or a NOT TIME-ACCURATE run.

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
at the end against the ADR 0181 references, the five largest-`abs(du)` cells with tags, r and z, and the share of `sum abs(du)^2` in the hub
trailing-edge columns (ADR 0181 definition).

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
- SST in arm 1. It needs F-271 (k-omega SST on moving meshes), which `docs/dynamic-mesh.md` lists as missing.
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
   repeat arm 1 with SST as part of F-271's verification before A1 is graded. For NOT EVALUABLE, keep A1's frame open,
   and decide after one registered amendment that addresses the cause.
4. **Reading "the limit cycle stays in both arms" (D9).** Recommendation: read it from arm 2 and check H, not from the
   laminar Y1b, for the reason given under D9.
5. **The form of arm 2.** Options: (a) ADR 0085 arm C as the design names it (F-RP, coupled, rotation implicit; size L);
   (b) the relative frame on the coupled solver with Coriolis as implicit blocks (no F-RP; smaller; it separates the
   treatment of the rotation term from the change of formulation; untested anywhere). Recommendation: (a), as the
   design names it, with the Y2S control for attribution.
6. **H PHYSICAL.** Recommendation: report the window means of torque and Delta p_ts as the FAN-01 values (ADR 0180
   question 3) and stop steady-iteration fixes of the pocket. A1 is time-marched in any case if Q-ROT is YES.
7. **When arm 2 runs.** It is the costliest part (size L). Its answer changes A1 only when A1 stays on a steady
   rotating-frame solve. Options: (a) as the design says, both arms regardless; (b) build and run arm 2 only if Q-ROT is
   not YES. Recommendation: (b). With Q-ROT YES, A1 is time-marched on a moving mesh, and the "both arms" clause is read
   from check H.

## Addenda

To be added in this order: amendment 1 (V0 to V5, the smoke values, the caps, the dt fallback if used); the R1 records
and the P verdict, committed before any Y1 record is read; arm 1 results and the Q-ROT verdict; check H; arm 2 if it
runs; the decision under D9.
