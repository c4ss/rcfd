# EXP-ROT: how the FAN-01 rotor is modelled

*   **Status:** Proposed. Design only. No code has been written and nothing has been run. The limits are registered in
    [ADR 0183](../adr/0183-exp-rot-registration.md).
*   **Date:** 2026-11-04
*   **Roadmap:** F-276, milestone EXP-ROT (requires M-0). Parent: goal G7 and Technical Plan Part 2 of
    `docs/designs/unblock-roadmap-fan01-fea.md` (branch `m0-gate-roadmap`, not merged when this was written).

## Overview

EXP-ROT is the experiment that decides how M-FAN-A1 models the FAN-01 rotor. It has three parts:

1. Arm 1 rotates the ADR 0086 meshes rigidly through `crates/rcfd-mesh/src/motion.rs` and time-marches laminar PISO
   in the absolute frame. It asks whether rigid rotation removes the spurious outlet angular momentum that the
   rotating-frame solve leaves in the empty passage and that grows with refinement.
2. Arm 2 runs the bladed passage with ADR 0085 arm C: the coupled pressure-velocity solver, rotationally periodic
   seams, and the rotation term as implicit cross-component blocks. It asks whether the limiter limit cycle stays.
3. Check H runs B2 time-accurately with SST in the rotating frame. It asks whether the pocket behind the hub trailing
   edge is physically unsteady. This is question 1 of ADR 0181 (job 49R, branch `job49r-fan01-hub-mesh`), which
   recommends a report-only time-accurate check on B2.

ADR 0183 is the registration. It holds every number. This document explains the plan, what must be built, what it
costs, and how each result maps to the build of A1.

The parent design says the laminar runs of arm 1 need no new code. That turns out to be wrong, and this design departs
from it. Three small prerequisites are needed (P-A, P-B, P-C), and arm 2 needs a rebuild of about 1 000 to 1 800 lines.
The code ADR 0085 wrote for arm C was removed, was never committed, and has not been found on disk.

## The problem

What is known about FAN-01 on the C-grid family of [ADR 0086](../adr/0086-wave37c-fan01-solution-verification.md)
(empty passages E3 / E2 / E1 with 5 040 / 13 882 / 38 688 cells, bladed B3 / B2 / B1 with 4 920 / 13 627 / 38 184):

- **Spurious angular momentum.** In the empty passage the exact outlet angular-momentum flux L is zero. The
  rotating-frame solve (SST, SIMPLEC, Barth-Jespersen `linear_upwind`) gives -0.0095, 0.0596 and 0.1060 N m, so L grows
  with refinement, and the passive duct gains 4.5 to 9.2 Pa of total pressure.
  [ADR 0082](../adr/0082-wave36c-rotating-frame-angular-momentum-source.md) located a first-order part of this error on
  the H-slit mesh: upwind convection of the rigid part `-Omega x r` of the relative velocity. The growth under the
  limited scheme on the C-grid is undiagnosed ([`milestone-fan.md`](../milestone-fan.md)).
- **The absolute-velocity formulation.** It removes the source where it converges. ADR 0082's rotor passage (F-RP) gave
  -1.4e-5 N m on the H-slit M1. [ADR 0085](../adr/0085-wave37b-rotor-passage-robustness.md) made F-RP converge on M0 and
  M1 with a pseudo-time step or with the coupled solver (arm C), but neither converged on M2 in 60 minutes. All of that
  code was removed. Arm C used `upwind` and ran only on the empty H-slit passage.
- **The bladed limit cycle.** The steady iteration of the bladed passage does not converge. It cycles: at the blade-root
  leading edge on B3, and at the hub trailing edge (z 0.1716) on B2.
  [ADR 0110](../adr/0110-wave43c-fan01-rotor-passage-non-convergence-diagnosis.md) falsified SST, the seam gradient, the
  pressure-velocity coupling and the explicit Coriolis source as causes. Jobs 49K to 49R (ADRs 0174, 0175, 0177 to 0181,
  unmerged, not yet accepted by the maintainer) tried masks, relaxation, a first-order fallback, three TVD limiters and a
  mesh change. None converged B2. ADR 0181 found that the B2 site is not a geometric outlier. It is a near-stagnant,
  partly reversed corner pocket that oscillates at the order of its own speed. That raises the question of whether a
  steady state exists there at all.

The decision this experiment feeds is in the parent design. If rigid rotation removes the spurious angular momentum, A1
is built on rigid rotation, time-marched (which needs F-277, SST on moving meshes). If not, A1 stays on a rotating-frame
solve. If the limit cycle stays whatever the treatment of the rotation, the blocker is the mesh or the scheme, and A1
starts from the CAD mesh of M-FAN-B.

## Goals

*   **EG1 (Q-ROT, arm 1).** On E3, E2 and E1, compare L under rigid rotation (Y1) with L under the rotating frame (R1),
    with the same laminar physics and the same convection scheme. A stationary-mesh run (I1) gives the inertial
    discretisation floor, and the same case on steady SIMPLEC (I1S) shows how much the algorithm alone moves L. The
    verdict is YES, NO, NOT REPRODUCED or NOT EVALUABLE by the rules of ADR 0183.
*   **EG2 (arm 1, bladed).** Run B3 and B2 rigidly rotating. Report whether they settle, the angular-momentum balance,
    and the wall time per revolution. The last number sizes the 4-hour budget per operating point of G8.
*   **EG3 (Q-POCKET, check H).** Classify the B2 hub pocket under time-accurate SST as STEADY, PHYSICAL, NUMERICAL or
    NOT EVALUABLE, from two runs at dt and dt/2.
*   **EG4 (Q-LC, arm 2).** On B3 and B2, apply the ADR 0181 limits C1 to C3 to the coupled solver with F-RP and the
    rotation implicit (Y2). A SIMPLEC control with F-RP (Y2S) attributes the result.
*   **EG5.** Every limit, prediction and stop rule is written in ADR 0183 before any code or run of the experiment.

## Non-goals

*   SST on moving meshes (F-277), sliding interfaces (F-250), and the CAD blade mesh (M-FAN-B).
*   Grading against the measured fan. Torque and Delta p_ts are reported against the ADR 0181 references, not graded.
*   Any change of a default. Every prerequisite is opt-in, and `baseline_hashes.rs` is not re-recorded.
*   Deciding the fate of the 49K to 49R branches (open question 1).
*   The upwind scheme in arm 1. ADR 0082 and ADR 0085 already answered the first-order mechanism.

## How the result is used

The mapping (ADR 0183 D9) keeps the parent design's three rules. It adds two outcomes the parent design does not
cover, and those are left to the maintainer:

| Outcome | Then |
|---|---|
| Q-ROT YES | A1 on rigid rotation, time-marched; F-277 moves into A1 (parent design) |
| Q-ROT NO | A1 stays on a rotating-frame solve; its design reopens the diagnosis (parent design) |
| Q-LC STAYS and H STEADY or NUMERICAL | the blocker is the mesh or the scheme; A1 starts from the M-FAN-B mesh (parent design) |
| Q-ROT NOT REPRODUCED or NOT EVALUABLE | open question 3 |
| H PHYSICAL | open question 6 |
| H NOT EVALUABLE or not run | the third row is not read and does not fire |

The parent rule says "the limit cycle stays in both arms". Arm 1 is laminar, and laminar flow at a chord Reynolds
number of about 1.4e5 is expected to oscillate behind a trailing edge whatever the rotor model. So the bladed arm-1 runs
cannot show a limit cycle. ADR 0183 reads the clause from arm 2 and check H instead (open question 4).

## Technical plan

### 1. What exists, and the departure from the parent design

Read on main at edb4839 (no code written):

| Need | State | Consequence |
|---|---|---|
| Rigid prescribed rotation, PISO / PIMPLE, Euler | exists (`dynmesh.rs`, `motion.rs`, `ale.rs`, [ADR 0033](../adr/0033-wave20b-dynamic-mesh.md)) | arm 1 runs on it |
| Turbulence or sources on a moving mesh | refused | arm 1 is laminar and has no frame source, which is the point of the arm |
| A wall at rest on a rotating mesh | none: a following wall takes the mesh's tangential velocity; a `fixed` 0 wall leaves a relative flux equal to the swept volume of its faceted faces (up to 1.84e-2 of `Omega r abs(S)` at the hub, [ADR 0113](../adr/0113-wave44c-mrf-cyclic-seams.md)) | P-B |
| A rigid motion with no following patch | refused (at least one moving patch, and the following patches default to it) | P-B |
| Barth-Jespersen on PISO / PIMPLE | refused (`sim.rs`: "the limiter applies only to ... simple or simplec") | P-A |
| `rotating_frame` on PIMPLE | refused (`sources.rs`: steady algorithms only) | P-C |
| Coupled solver with periodic seams, sources, SST, a limiter | refused or absent (`coupled.rs`, `sim.rs`) | arm 2 rebuild |
| F-RP and arm C code | removed in ADR 0082 / 0085; `git log --all -S rotor_passage -- crates` is empty; the patches were not found on disk | arm 2 rebuild from the ADR text |

### 2. Prerequisites for arm 1 and check H

Each is opt-in, keeps the defaults bit-identical (ADR 0183 V0), and is removed if it fails its checks (the ADR 0085
effort bound). The sizes are estimates. No line count exists for any earlier version of this code.

- **P-A, Barth-Jespersen on the transient path (S, about 50 to 120 lines plus tests).** The existing key
  `solver.linear_upwind_limiter = "barth_jespersen"` is accepted for `piso` and `pimple`, the moving-mesh step included.
  It applies the same `gradient::Limiter::BarthJespersen` to the same least-squares gradient as `SimpleSolver` does
  ([ADR 0081](../adr/0081-wave36b-solver-robustness-follow-ups.md) part L), in the convection term only. Basis: on the
  steady path this is one solver field and one gradient multiplier, and the PIMPLE convection already calls the same
  operator with a gradient. With P-A, the arm, its rotating-frame control and check H all use the scheme of every FAN-01
  result since ADR 0086. Without it, the control and the arm would differ in scheme as well as in frame, which is the
  "mixes the scheme and the formulation" caveat of ADR 0085 F1.
- **P-B, a wall at rest on a rigidly moving mesh (S, about 40 to 100 lines plus tests).** The face velocity is
  `(m_f / abs(S_f)) n_f`: no tangential part (the wall is at rest), and a normal part that carries the swept volume (no
  relative flux through the moving face). This is the following wall of ADR 0033 D3 with the tangential part removed. The
  same change lets a rigid motion run with no following patch. Check V5 runs it on an aligned annulus sector, where no
  swirl may appear.
- **P-C, `rotating_frame` with PIMPLE on a static mesh (M, about 200 to 400 lines plus tests).** The refusal is lifted for
  `pimple`. The frame acceleration (centrifugal and Coriolis) goes through the transient source hook of
  [ADR 0045](../adr/0045-wave25c-transient-momentum-sources.md) and is re-evaluated every outer iteration. The casing
  `moving_wall` with the tangential projection is checked on that path. The seam body-force term uses the coupled mean
  `rAU`: [ADR 0076](../adr/0076-wave35a-transient-seams-and-relaxation.md) Y5 measured order 2.04 on its rotational
  sector with this term, against 1.72 without it (both with that wave's fix set K, since removed). It named the missing
  term as an open item for any transient rotating-frame run of a sector. The rotating-frame wall-pressure treatment of
  the steady path (F-031 note, `wall_pressure_trapezoid`) is checked on the transient path too. If it is missing there,
  it is added under the same key.

### 3. Arm 1: rigid rotation, laminar, absolute frame

**Runs** (ADR 0183 D4). On each of E3, E2 and E1:
- R1, the control: the ADR 0086 empty-passage case unchanged except `turbulence.model = "laminar"`. It is relative
  frame, steady SIMPLEC, Barth-Jespersen.
- Y1, the arm: the same mesh in the absolute frame, rotating at `Omega t` about z, PISO with Euler, walls at rest
  (P-B), inlet (0, 0, 9.478) m/s.
- I1, the inertial control: Y1 with the mesh at rest.
- I1S, the algorithm control (report only, no code): I1 on R1's steady SIMPLEC.

The bladed Y1b runs on B3 and B2, with the hub and blade following the mesh and the shroud at rest. It is report
only.

**Why laminar is enough for the empty passage.** The exact solution is axial and axisymmetric, with no swirl, for any
viscosity and in either frame. Any L is discretisation error, laminar or not. The open risk is that the growth in
ADR 0086 comes from SST and does not appear laminar. R1 runs first and decides that (P-GROW against P-ABSENT) before any
Y1 record is read.

**Why PISO is enough for the empty passage.** ADR 0033 lags the seam coupling inside a step. In the settled empty
passage every cell of the rotating mesh keeps its value (`c_x = c_y = 0`, `c_z(r, z)`), so the lag vanishes at the state
that is measured. The bladed state rotates in Cartesian components, so its seam lag and its Euler time error do not
vanish. That is one reason Y1b is report only.

**Time step and length.** `Omega dt = 1 deg` (dt = 1.12156e-4 s, 360 steps per revolution). The relative speed reaches
about 40 m/s at the shroud. Core cells are 5 to 15 mm, so the Courant number there is of order 1. It is higher in the
thin wall cells, which PISO's implicit momentum step tolerates for a settled target. A revolution takes 40.4 ms and a
flow-through 42.2 ms. A run stops when two consecutive revolution means of L agree within the ADR 0086 cap rule, and
after 15 revolutions at most. One fallback to 0.5 deg is registered for a smoke that diverges. A dt/2 repeat on E2 tests
the time-step floor, using ADR 0085's 10 % / 50 % rule applied to dt.

**Reading.** The rules are ADR 0085's, inherited:
- A1-R: Y1 is two orders below R1 on E1.
- A1-N: Y1 does not grow with refinement (monotone, and order at least 0.5 when above the 1e-4 N m floor).
- A1-L: Y1 is within ten times the inertial control.
- A1-E: Y1 creates no total pressure.

YES needs P-GROW with A1-R and A1-N. A1-L and A1-E are recorded and become known misses if missed. This follows ADR 0085,
which used the inertial control and the energy check as acceptance companions.

Y1 runs on PISO and R1 on steady SIMPLEC, so A1-R compares the frame and the algorithm together. ADR 0085 kept one
algorithm per arm and control; here only A1-L (Y1 against I1) does. I1S is the cheap way to see the algorithm's share. If
I1S and I1 differ by more than a factor of 10 on a member (above the 1e-4 N m floor), the verdict is labelled
ALGORITHM-CONFOUNDED.

### 4. Check H: the hub pocket in time

ADR 0181 recommends a time-accurate PIMPLE run on B2 to see whether the pocket keeps oscillating at a physical
frequency. ADR 0183 D5 registers it:
- the ADR 0086 bladed case (SST, rotating frame, Barth-Jespersen) on B2 with PIMPLE, 5 outer iterations and BDF2;
- dt = T_c / 100, with T_c = 4.605 ms the time the hub relative flow (21.5 m/s) takes to pass the 99 mm hub chord, and a
  second run at dt/2;
- 6 flow-through times, the second half of which is the measurement window.

It starts from the uniform initial state. The stalled SIMPLEC iterate is not a solution, and a hand-over across
algorithms would be untested code.

The class comes from the relative speed of the six ADR 0181 site cells:
- **STEADY**: the amplitude stays below 1 % of the hub speed `Omega r_hub` (0.193 m/s).
- **PHYSICAL**: the frequency (from zero crossings) and the amplitude agree between dt and dt/2 within 10 % and 25 %,
  with at least 5 periods in the window and 20 steps per period.
- **NUMERICAL**: the period is fixed in steps, not in seconds.
- **NOT EVALUABLE**: anything else.

The classes are tested in a fixed order, with the NOT EVALUABLE conditions (budget hit, divergence, a run that is not
time-accurate) first, so every outcome has exactly one class. The Q-ROT verdict has the same precedence.

An outer-iteration rule guards the time accuracy. The window means of torque and Delta p_ts are reported against the
ADR 0181 cycle means. They are the values ADR 0180 question 3 asks for if the pocket is unsteady.

H runs on a static mesh with SST. It does not depend on arm 1, so it can run while the arm-1 prerequisites are built
(P-A is shared).

### 5. Arm 2: the coupled solver with the rotation implicit

**What must be rebuilt.** None of it is in the repository. The specification is ADR 0082 (F-RP) and ADR 0085 (arm C),
plus one new item:

| Item | Content | Estimate |
|---|---|---|
| R-1 F-RP | `rotor_passage` for `mrf`: frame flux on the boundary faces of zone cells, the per-cell closure check (1e-10) in place of the surface-of-revolution check (which today refuses `axial_fan_sector`, ADR 0113), turbulence on the relative flux | S-M, 150-300 lines |
| R-2 coupled seams | rotational periodic pairs in the gradient, continuity and Rhie-Chow rows, with lagged momentum partner coefficients | M, 200-350 lines |
| R-3 flux offset | the frame flux in the coupled convection | XS, 20-40 lines |
| R-4 implicit rotation | cross-component blocks `M_xy = -Omega V`, `M_yx = +Omega V` in the GMRES operator; Rhie-Chow body-force term lagged | S-M, 80-150 lines |
| R-5 wiring | SST and `mrf` sources accepted with `algorithm = "coupled"` (today "closed-domain laminar flow only"), only with `rotor_passage = true` | S, 80-150 lines |
| R-6 limiter (new) | Barth-Jespersen `linear_upwind` in the coupled convection; ADR 0085 used `upwind`, and the unlimited scheme trips the guard within 7 to 27 iterations on these meshes (ADR 0110) | S, 40-80 lines |
| Tests | V0 (bit-identity, including coupled lid-cavity hashes re-recorded on the then-current main), V4 (closure, seam pairs, 30-iteration smoke), the registered runs | 400-700 lines |

Total: about 1 000 to 1 800 lines, size L. Basis: the item descriptions in ADRs 0082 and 0085, the 874 lines of
`coupled.rs` today, and the record that ADR 0085 wrote R-1 to R-5 in one wave alongside its diagnostics. No line count
was recorded, so the range is an estimate. R-6 has no precedent.

**Runs.** Y2 (coupled, R-1 to R-6) and Y2S (F-RP on SIMPLEC, rotation explicit) run on B3 and B2, 3000
iterations from iteration 1, with the ADR 0181 limits C1 to C3. Y2S exists for attribution, because Y2 changes three
things at once: the formulation, the solver, and the implicitness of the rotation term.
- Y2 GONE with Y2S not GONE points at the coupled solve.
- Both GONE points at the formulation.
- Y2 STAYS says none of the three matters on these meshes.

ADR 0110's falsification of the explicit Coriolis source, and ADR 0180's finding that every limiter cycles at the same
B2 hub cells, predict STAYS.

**When it runs.** Arm 2 is the costliest part. Its answer changes A1 only if A1 stays on a steady rotating-frame solve.
Open question 7 recommends building it only if Q-ROT is not YES. The parent design runs it regardless.

### 6. Harness, records and order

- One test file, `crates/rcfd-api/tests/exp_rot.rs` (proposed name). It holds construction checks and bit-identity tests
  that are not ignored when fast. Each registered run is an ignored test run alone, one per process.
- A driver, `scripts/exp_rot.py` (proposed name). It queues the registered runs at most two at a time, in the order of
  ADR 0183 D8, and launches the built test binary directly. It reads no solver output except the JSON records.
- One JSON record per run under `docs/investigations/exp-rot/`, written to an absolute path (ADR 0180 deviation 1).
- The R1 records and the P verdict are committed before any Y1 record is read. Each later family is committed before the
  next one runs.
- Amendment 1 records the smoke values and fixes the time caps at three times the projected run time. No physical
  parameter changes with the smoke, except the single registered dt fallback.

**Cost estimates** (machine time; from ADR 0086 SIMPLEC and ADR 0085 coupled per-iteration costs; a PISO step taken as
two SIMPLEC iterations):

| Part | Runs | Estimate |
|---|---|---|
| Arm 1 | R1 x3, I1S x3, Y1 x3, I1 x3, Y1(E2) at dt/2, Y1b x2 | about 7 h |
| Check H | H(dt), H(dt/2) | about 2.5 h |
| Arm 2 | Y2 x2, Y2S x2, references | about 1.1 h |

That is about 10.5 machine-hours, or about 5 to 6 hours of wall time at two runs at a time, plus the build and gate
time of the prerequisites. Arm 1 and check H change a core crate (`rcfd-fv`), so their gate runs the rcfd-fv heavy tests
(the core-crate rule of ADR 0182 D1).

## Alternatives considered

### A. Arm 1 with `upwind`, no code

*   Pros: no prerequisite; both paths accept it; it is the scheme of ADR 0082's mechanism.
*   Cons: ADR 0082 and ADR 0085 already showed that the absolute formulation removes the first-order source. The open
    item is the growth under the limited scheme, and `upwind` damps the hub pocket.
*   **Rejected** in favour of P-A. Kept as option (d) of open question 2.

### B. `tvd_van_albada` in arm 1 and check H, no limiter code

*   Pros: the case tree maps it to the TVD convection for every incompressible algorithm today (its behaviour on the
    moving-mesh step was not checked). ADR 0180 found it cycling at the same B2 hub cells as Barth-Jespersen.
*   Cons: the ADR 0086 growth was never measured with it, so R1 would have to establish a new pathology first.
*   **Not chosen.** Option (c) of open question 2.

### C. A hub that follows the mesh in the empty passage, no wall code

*   Pros: the case tree can express it today.
*   Cons: it is not the ADR 0086 set-up. The exact L is no longer zero, so the measure becomes the angular-momentum
    excess, which depends on how the wall torque is integrated.
*   **Not chosen.** Option (b) of open question 2.

### D. Local pseudo-time on B2 instead of check H

*   Pros: no code (`pseudo_mode = "local"`).
*   Cons: it can show only that a steady state exists, not whether the flow leaves it in time.
*   **Not chosen.**

### E. The relative frame on the coupled solver with implicit Coriolis, as arm 2

*   Pros: no F-RP. It separates the treatment of the rotation term from the change of formulation.
*   Cons: it is not the arm the parent design names, and it has never run anywhere.
*   **Not chosen.** Open question 5.

### F. Wait for F-277 and run arm 1 with SST

*   Pros: the same turbulence model as the stall.
*   Cons: F-277 is unsized, and EXP-ROT is what decides whether A1 needs it.
*   **Rejected.** The laminar arm answers whether the formulation itself is clean. A turbulent repeat is recommended
    inside F-277's verification if the laminar control does not reproduce the growth (open question 3).

## Risks

| Risk | Effect | Mitigation |
|---|---|---|
| Laminar R1 does not reproduce the growth | Q-ROT NOT REPRODUCED | registered outcome; open question 3 |
| PISO diverges on the C-grid (non-orthogonality up to 83 deg) | arm 1 not runnable | V3 smoke; one dt fallback; then NOT RUNNABLE |
| The transient transposed-stress term (variable viscosity, ADR 0032) skips rotationally periodic seams (`crates/rcfd-fv/src/pimple.rs`, a named limitation) | with SST, check H differs from the steady stall at the seams (arm 1 is laminar with constant nu, so the term is zero there) | reported; the steady path's treatment of that term at rotational seams was not checked for this design |
| Facets of the hub and shroud are not surfaces of revolution | swept volume at walls at rest | P-B carries it; V2 reports it; V5 checks P-B on an aligned mesh |
| A pocket period longer than about 25 ms | fewer than 5 periods in the H window, NOT EVALUABLE | stated now; a longer window needs an amendment before the run |
| The O(dt) seam body-force mismatch of ADR 0076 | H reads NUMERICAL for the wrong reason | the pair mean is part of P-C; the seam-pair flux is reported |
| F-RP's closure check (1e-10) is exact only for planar faces, and the C-grid faces are warped | V4 fails, arm 2 NOT RUNNABLE | registered outcome; the closure is measured on B3 and B2 before any arm-2 run |
| Arm 2 rebuild larger than estimated | cost | effort bound (V4); open question 7 |
| A cost overrun on E1 or H(dt/2) | runs stop at the cap | caps fixed in amendment 1 from the smoke; nothing is re-run without an amendment |

## Open questions

ADR 0183 lists them, each with a recommendation. In short:
1. The 49K to 49R jobs: EXP-ROT takes over ADR 0181 question 1. No new steady-iteration job until EXP-ROT reports.
2. Prerequisites: build P-A and P-B as registered.
3. Q-ROT NOT REPRODUCED: build A1 on rigid rotation provisionally if Y1 is clean, and repeat arm 1 with SST in F-277's
   verification. NOT EVALUABLE: one registered amendment, then decide.
4. "Both arms": read the clause as arm 2 STAYS and H STEADY or NUMERICAL, not from the laminar bladed runs.
5. Arm 2's form: ADR 0085 arm C as the parent design names it, with the Y2S control.
6. H PHYSICAL: report the window means as the FAN-01 values, and stop steady-iteration fixes of the pocket.
7. Arm 2 only if Q-ROT is not YES. The clause is then read from H alone, and not at all if H is NOT EVALUABLE.

These are recommendations, not decisions. The parent design's own rules are used where it gives them and are marked as
such.

## References

Recalled, not re-read (UNVERIFIED). No limit depends on them.
- P. D. Thomas and C. K. Lombard, "Geometric conservation law and its application to flow computations on moving grids",
  AIAA Journal 17 (10), 1979.
- R. I. Issa, "Solution of the implicitly discretised fluid flow equations by operator-splitting", Journal of
  Computational Physics 62, 1986.
- T. J. Barth and D. C. Jespersen, "The design and application of upwind schemes on unstructured meshes", AIAA paper
  89-0366, 1989.
- I. B. Celik et al., "Procedure for estimation and reporting of uncertainty due to discretization in CFD applications",
  Journal of Fluids Engineering 130, 078001, 2008 (as cited in ADR 0086).
