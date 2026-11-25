# ADR 0762: F-870 registration: FAN-01 at its design point, the state, the band and the ranked gap

Status: **REGISTERED 2026-11-22, written before any code of F-870 and before any run of its checks.** Before this text
no test, harness or solver run of F-870 was written or made, and no cargo command was run. What was done: reading
(below), one reading script over `docs/roadmap.json`, the arithmetic of the band (`band.py` in the scratch folder), and
fetches of public sources for the uncertainty of the measurement. Results go in addenda at the end; nothing above them
is changed after a run.

It answers the brief `docs/adr/briefs/f870-fan01-design-point.md` and follows `docs/process/agent-rules.md`, version 5.
Branch `f870-fan01-design-point` from main 13b0937d; scratch `w49i-work/f870-fan01-design-point-scratch`. No solver
source is changed by this work: it adds one test file, records and text.

Size (agent): partial; 0; 3; 4; 700000

The expected outcome is a miss. One part of the band could not be registered, because the uncertainty of the
measurement is not in any source that could be read (section "The measurement and its uncertainty"); by the decision
rule below that alone ends this registration as "stopped and reported", whatever the run gives. The run, the ranked gap
and the test are delivered all the same, because none of them needs that number, and the registration says in advance
how the result is read without it.

## What is asked

Roadmap item F-870, a headline item of the milestone `fan`
([ADR 0757](0757-f867-registration-the-fan-milestone-as-the-whole-design-goal.md)). Its acceptance: "Every limit not
stated here is registered in the ADR before the run. The total-to-static pressure rise and the shaft power of the FAN-01
rotor at its design point (1.4032 m^3/s; measured 126.46 Pa and 333.30 W, as recorded in docs/validation-data.md) are
each within a band registered before the run from the measurement's stated uncertainty and a mesh-convergence estimate,
on a mesh that meets the registered quality gates, with the solve converged by its registered rule; a miss is recorded
with its number and is not a pass."

The brief's five tasks: the state of the FAN-01 diagnoses in one table; the band, registered before any run; one run of
the design point on current main with the best registered set-up; the gap, ranked; the state held in a test.

## What was read

Whole: the brief; `AGENTS.md`; `docs/process/agent-rules.md` (version 5); `docs/milestone-fan.md`;
`crates/rcfd-api/tests/fan01_validation.rs`; `docs/validation-data.md` lines 1 to 140 (the pack and section
"Rotating machinery sources"); [ADR 0181](0181-job49r-a-mesh-change-at-the-fan01-hub-trailing-edge-site.md);
[ADR 0757](0757-f867-registration-the-fan-milestone-as-the-whole-design-goal.md); `docs/process/tolerances.md`;
`docs/test-process.md` sections 7.4 and 7.5; `validation-data/reference/w34d_fan01_characteristic_n1ug.csv`. Of
`crates/rcfd-api/tests/fan01_gci_w37c.rs`: lines 1 to 240, 480 to 680 and 790 to 815 (the case, the run, the cap rule).
Of [ADR 0183](0183-exp-rot-registration.md): lines 1 to 36, the outline, check H and the decision record. Of
`docs/roadmap.json`, by script: F-126 and F-870 in full, and id, status, title, gap and milestones of the items whose
title, acceptance or gap matches a keyword of the fan gap (rotating frame, MRF, C-grid, stall, limit cycle, diffuser and
others).

Read by a second agent of this session on my instruction, in full, and reported to me with file and line for every
number: ADR 0075, 0078, 0086, 0110, 0111, 0170, 0171, 0172, 0174 to 0180, the leading comments, test names, ignore
reasons and asserts of the fourteen files `crates/rcfd-api/tests/fan01_*.rs`, and the gate lines of `docs/meshing.md`.
I checked these of its numbers against the files myself: the B1 / B2 / B3 torques and times of ADR 0170 (lines 251, 277
to 279), the quality table and the run table of ADR 0086 (lines 51 to 57, 209 to 216), the arm table of ADR 0078 (lines
398 to 412), the arm table of ADR 0178 (lines 187 to 191), and the gates of `docs/meshing.md` (lines 254 to 259, 344 to
348). The other numbers of the state table below rest on that agent's reading.

Not read: ADR 0065, 0079, 0082, 0083, 0085, 0087, 0113 (known through `docs/milestone-fan.md` only); the body of ADR
0183 beyond the parts named; `docs/fan-project-breakdown.md`; the solver source.

## The measurement and its uncertainty

**The measurement** is traced to a source that was read. `docs/validation-data.md` (section "Rotating machinery
sources", 1. FAN-01) records it from `characteristic_n1ug.mat` of the Zenodo record doi:10.5281/zenodo.20037409 (v5, CC
BY 4.0), transcribed in `validation-data/reference/w34d_fan01_characteristic_n1ug.csv`, row 13: 1.40324 m^3/s,
126.45658 Pa, 333.30090 W; the speed 1486 rpm is from the papers. The shaft torque that the tests compare is 333.30 /
(2 pi 1486 / 60) = 2.14184 N m.

**The uncertainty is not recorded.** The page says: "no density, no ambient state, no FAU uncertainty statement in
anything read; the round robin quotes +-3 Pa and 1.5 % flow and agrees with the FAU curve within about 3 %." The brief
asks me to find it in the published source and not to assume one. Fetched and read on 2026-11-22 (text extracted with
`pdftotext`, kept in the scratch folder under `sources/`); the statements are given in my words, with their place:

| Source | Read | What it states on uncertainty, density and friction |
| --- | --- | --- |
| Zenodo record doi:10.5281/zenodo.23943494 (v6 of the FAN-01 record, published 2026-11-04; concept doi:10.5281/zenodo.8418340; CC BY 4.0): `README.md` and the description `LA_Case4_AxialFan_measurements.pdf`, "Data set description, fan N1UG, version v6" (19 pages) | whole | No uncertainty of pressure, flow, torque, power or speed; no density, no ambient state. New against v5, section "Test rig": "The bearing friction was measured with the fan removed and subtracted." Table A1 gives `P_shaft_in_W` as the shaft power with that offset subtracted (the variable was renamed from `P_shaft_in_Nm`). Appendix A.1 gives the design point as 1.403 m^3/s, 126.46 Pa and efficiency 0.532. The pressure is ambient minus the static pressure in the chamber. Table 1: 150 Pa design, 126.5 Pa measured; rotor speed from the trigger 1486.08 per minute. The README says no measured value changed in v6. The description names Kromer's thesis as the detailed account of the rig and the measurements |
| The v4/v5 description of the same record (7 pages) | whole | No uncertainty, no density. Torque and speed came from a torque meter between motor and fan |
| Vourakis and Karlsson, "A round robin test of a low-pressure axial fan", FAN 2022, doi:10.26083/tuprints-00021703 (copy of research.chalmers.se, publication 549788) | the rig, the aerodynamic results and the conclusions (lines 60 to 250 and 340 to 360 of the extracted text) | The +-3 Pa and 1.5 % of flow are named as the measurement error of the rig of that paper (section on the aerodynamic results, under its Figure 4), for three 3-D printed copies of the fan; for the accuracy of that rig the paper refers to two other papers. Against the FAU curve its largest deviation of the pressure is 3 %, in the stall region below the design point. For power it gives no number: its efficiencies lie below the reference, and it says its torque readings were less certain than expected |
| Schoder and Czwielong, "Dataset FAN-01: Revisiting the EAA Benchmark for a low-pressure axial fan", arXiv:2211.12014 | searched by keyword in the extracted text (uncertainty, accuracy, density, torque, temperature); the passage on the characteristic read | No uncertainty, no density. It gives 126.5 Pa and 53 % at the design point and says the design pressure is not reached because of losses the design did not count, such as the tip flow |
| Czwielong, Kromer, Chaitanya, Becker, ICA 2019 paper 001182 (the same FAU chamber, another fan) | searched by keyword | No uncertainty |
| Kromer, "Sound emission of low-pressure axial fans under distorted inflow conditions", FAU University Press 2018, doi:10.25593/978-3-96147-089-1 | **could not be read**: the repository open.fau.de answered the fetch with an automated-access check, which I did not go around | unknown; the v6 description points to it for the rig |
| Zenger, Junger, Kaltenbacher, Becker, SAE Technical Paper 2016-01-1805 | **not read** (not open) | unknown |

So: the stated uncertainty of the FAU measurement of pressure, flow and shaft power is in nothing that could be read,
and neither is the air density of the measurement. The one published scatter that was read (+-3 Pa and 1.5 % flow, and
agreement of the pressure curve within 3 %) belongs to another rig and other copies of the fan, and there is no such
number for the power at all. I do not take it as the uncertainty of the measurement: ADR 0075 used it for F-126, whose
acceptance says "within published scatter"; the acceptance of F-870 says "the measurement's stated uncertainty". The
row "Agreement with a measured case" of `docs/process/tolerances.md` reads "to be set by the maintainer per measured
case". This is a question that rule 6 leaves open (it needs a text that could not be obtained); it is the first open
question of this work.

One thing the reading removes: the page and ADRs 0075, 0078 and 0086 list bearing loss in the measured shaft power as
an unknown. The v6 description says it was measured and subtracted. The facts above go into
`docs/reference-data.d/0762-F-870.part` and the FAN-01 paragraph of `docs/validation-data.md` with the results.

## The state of the diagnoses (task 1; read, not rerun)

Torques before ADR 0170 are molecular moments; from ADR 0170 on the `moment` report uses the effective wall shear. On
the same fields the three C-grid meshes give 1.849759 / 1.967401 / 2.038428 N m molecular and 1.918189 / 2.028692 /
2.098164 N m effective. Measured: 2.142 N m (333.30 W), 126.46 Pa. "Cap rule" is the rule of ADR 0075 and ADR 0086:
8000 iterations, each functional changed by less than 0.5 % over the last 1000, mass balance 1e-3. Limits of
`docs/milestone-fan.md`: (a) the rotating-frame discretisation on bladed sectors, (b) the missed mesh quality gates,
(c) the residual stall. "Held" says what a test on main asserts; everything else is prose or a record file.

| Record | Tried | Design-point result | Limits (a), (b), (c) | Held on main |
| --- | --- | --- | --- | --- |
| [ADR 0075](0075-wave34d-rotating-machinery-data-and-mesh-study.md), 2026-10-19 | H-slit 13 332 cells, SST, `rotating_frame`; `linear_upwind` diverged, `upwind` ran | 1.320 N m molecular (-38 %), 1.3471 effective (-37.1 %), -3.8 Pa; cap rule | (a) MRF refused on the sector; (b) 79.1 deg, skewness 2.02, aspect 944; (c) not seen | `fan01_validation.rs`, `f126_..._known_miss`, ignored: asserts the F-126 limits, not the numbers; numbers in the comment |
| [ADR 0078](0078-wave35c-fan01-deficit-diagnosis.md), 2026-10-20 | the same case, one factor at a time, 22 arms | minmod 1.6800 N m / +27.4 Pa; in-plane refined 27 004 cells 1.8144 / +42.6; both (arm KN) 2.0163 molecular (-5.9 %) / +76.2 Pa | (a) confirmed and located: 2.061 N m of outlet angular momentum (168 % of the wall torque) without a wall source; first-order convection of the rigid swirl. Casing, outlet type and place, tip-gap count each under 2 % of the torque gap; outlet type moves the pressure rise by 8.2 Pa | `fan01_diagnosis_w35c.rs`, all arms ignored, numbers printed only: prose |
| [ADR 0086](0086-wave37c-fan01-solution-verification.md) | C-grid `camber_seam_h`, 4 920 / 13 627 / 38 184 cells (B3, B2, B1), SST, `linear_upwind` with Barth-Jespersen, `rotating_frame` | 1.850 / 1.967 / 2.038 N m molecular, 46.8 / 64.2 / 74.0 Pa; cap rule; order 1.51 and 1.74; bands +-10.6 % and +-40 % | (a) narrowed for the bladed balance, but the empty-passage swirl grows with refinement (0.106 N m) and the bands are "not shown credible"; (b) 78.4 / 79.5 / 83.4 deg; (c) first seen: residuals stay at 1e-3 | `fan01_gci_w37c.rs`: `b_m_b_p_bladed_studies_are_valid`, ignored, asserts the class "VALID" only; `s1_family_builds_and_is_accepted` runs in the gate (meshes build): numbers are prose |
| [ADR 0110](0110-wave43c-fan01-rotor-passage-non-convergence-diagnosis.md) | B3 and the empty passage, switches at iteration 1000, frozen limiter | default: 1.8493 N m, 46.64 Pa, a sample of a cycle; `upwind` converges: 1.335 N m, -4.0 Pa | (c) confirmed as a limit cycle of the limiter, not slow convergence; the cycle cells are at 26 to 28 deg, away from the 83 deg faces, so (b) is not its cause | `fan01_stall_w43c.rs`: arms ignored, printed only; two hook tests in the gate: prose |
| [ADR 0111](0111-wave44a-fan01-frozen-limiter-linearisation-and-relaxed-deferred-correction.md) | linearisation of the frozen map on B3; relaxed deferred correction on the empty passages | no new functional | (c) not attributed; the stop criterion is not a defect | ignored, printed only; some tests deleted: prose |
| [ADR 0170](0170-job49g-moment-reports-follow-the-effective-wall-shear-default.md), 2026-10-30 | the fields of ADR 0075 and 0086 read with the effective moment | H-slit 1.3471 N m (-37.1 %); B3 / B2 / B1 1.918189 / 2.028692 / 2.098164 N m, order 1.393, band +-0.2119 N m; B1 is 2.0 % below the measured torque; pressure rise unchanged | the reporting of wall shear was worth 2.1 % (H-slit) and 2.9 to 3.7 % (C-grid) of torque; (a) the B1 balance opens to -0.059 N m | comments and ignore reasons of the two files above: prose |
| [ADR 0174](0174-job49k-the-49h-wall-cell-metric-on-the-fan01-meshes.md) | a wall-cell metric on the six meshes, no solve | none | (b) removed as the cause of (c): the stall cells are unremarkable (20th to 30th percentile) | `fan01_wall_metric_w49k.rs`, five tests in the gate hold the metric values, not a functional |
| [ADR 0175](0175-job49l-where-the-fan01-stall-is-driven-in-the-nonlinear-iteration.md) | B3, a mask on the high-order convection correction | 1.9177 N m at 1000, a cycle sample | (c) narrowed: the convection deferred correction carries the cycle; the leading-edge root is sufficient, not necessary | `fan01_stall_locus_w49l.rs`, arms ignored; record files |
| [ADR 0176](0176-job49m-fan01-blade-passage-as-a-3d-example-project.md) | B3 with `upwind` as an example project | 1.4023 N m, 35 % below the measured torque | none | `rcfd-mcp` `example_projects_w48h.rs`: the sign of the torque in the gate; the 5 % comparison ignored |
| [ADR 0177](0177-job49n-under-relaxed-deferred-correction-on-the-bladed-fan01-passage.md) | B3, relaxed deferred correction 0.25 and 0.1 | 1.9210 N m, 47.0 Pa, not converged | (c) the leading-edge cycle goes, a hub cycle grows | ignored known misses; record files |
| [ADR 0178](0178-job49o-the-fan01-hub-cycle-with-the-leading-edge-driver-held-off.md) | the same plus first order in 322 hand-picked hub cells | **1.9144 N m, 45.75 Pa on B3 with `is_converged()` true**, the only converged bladed state above first order | (c) narrowed to two drivers; not a fix (hand-picked cells) | ignored; verdicts asserted, numbers in record files |
| [ADR 0179](0179-job49p-an-automatic-first-order-fallback-for-the-fan01-stall-cells.md) | an automatic selector of first-order cells, B3 and B2 | B3 1.9105 N m / 44.1 Pa; B2 1.9723 / 63.5; none converged | (c) not removed; the first-order share moves the B2 torque by 2.9 % | ignored known misses; record files |
| [ADR 0180](0180-job49q-a-face-bounded-second-order-convection-scheme-for-the-fan01-stall.md) | minmod, van Albada, van Leer switched in at 1000, B3 and B2 | B2: 2.040 to 2.059 N m (-3.9 to -4.7 %), 60.0 to 60.8 Pa (-52 %); cycle samples | (c) independent of the limiter family; all cycle at the same hub cells on B2 | ignored known misses; record files |
| [ADR 0181](0181-job49r-a-mesh-change-at-the-fan01-hub-trailing-edge-site.md), 2026-10-31 | a radial grading without the stack-to-middle jump, B3 and B2 | B2 2.0771 N m (-3.0 %), 69.1 Pa (-45 %); cycle samples; outside its registered band | (c) the site is the hub trailing edge, a near-stagnant pocket, not a mesh outlier; the cycle stays | `fan01_hub_mesh_w49r.rs`: mesh hashes and the key in the gate; arms ignored |
| [ADR 0183](0183-exp-rot-registration.md), 2026-11-04 | registered, not run: rigid rotation of the mesh against the rotating frame on the empty passage; the pocket in time (check H) | none | check H became runnable with F-286 ([ADR 0199](0199-f286-registration-transient-k-omega-sst-with-pimple.md)) and has not been run | no test |

**The best recorded values at the design point.** Torque: B1, 2.098164 N m effective (326.50 W, 2.0 % below the
measurement), with 73.99 Pa (41.5 % below), on the C-grid of 38 184 cells with SST, limited `linear_upwind` and the
relative-velocity rotating frame; "converged" by the cap rule only, with `res_u` at 1.2e-3, so by ADR 0110 a sample of
a limit cycle. Pressure rise: arm KN of ADR 0078, 76.2 Pa on the H-slit of 27 004 cells with minmod; its torque 2.0163
N m is molecular. No second-order run of the bladed passage has reached the residual tolerance.

**What no test on main holds.** No test on main asserts a design-point pressure rise or torque of the second-order
set-up. The gate holds that the meshes build, the sign of a first-order torque, and mesh hashes. Every functional above
is prose, a comment or a record file. Task 5 closes that.

## The set-up of the run (fixed now)

The best registered set-up of the diagnoses is B1 of ADR 0086: it has the best recorded torque, the highest pressure
rise of the C-grid family, and it is the mesh of the mesh-convergence estimate. The case text is that of
`fan01_gci_w37c.rs` (`mesh_toml` and `case_toml`), copied into the new test file without a change of value:

* mesh `axial_fan_sector`, `topology = "c_grid"`, `c_grid_construction = "camber_seam_h"`, counts (n_up, n_chord,
  n_down, n_side, n_nose, n_span, n_gap) = (12, 24, 12, 15, 6, 21, 10), `n_core = 1`, `front_gap = 0.3`, `n_bl = 2`,
  `growth = 1.2`, first cell by `y_plus = 30.0`, the 16 sections of `w34d_fan01_cad_sections.csv`; 38 184 cells
  expected;
* 1486 rpm, 1.4032 m^3/s, `rho = 1.2` (assumed, as in ADR 0075; see open question 2), `nu = 1.5e-5`;
* `rotating_frame` with `centrifugal = true`, k-omega SST, the casing a moving wall, SIMPLEC, `linear_upwind` with
  `linear_upwind_limiter = "barth_jespersen"`, `tolerance_u = tolerance_p = 1e-5`, 8000 iterations at most;
* functionals as in ADR 0075: torque `-9 (m_blade + m_hub)` (effective moment, the default since ADR 0170), shaft power
  torque times `2 pi 1486 / 60`, pressure rise `rho (p_out - (p_in + u_in^2 / 2))` with `u_in = Q / A`.

One difference from that harness: no wall-clock stop. Two other heavy solves share the machine; ADR 0170 measured 46.4
minutes for B1 alone and ADR 0086 58.8 minutes on a shared machine, so I expect one to three hours and register no
limit on time. The stops that stay: the steady divergence guard, a non-finite residual, continuity above 1e6.

B2 (8, 17, 8, 11, 4, 15, 7) and B3 (6, 12, 6, 8, 3, 10, 5) are run once each to 8000 iterations with the same text, so
that the mesh-convergence estimate can be shown on current main (check R1). The three runs are started together as
three processes. Each registered run is made once.

## The band (task 2; fixed now, before any run)

**Form.** For each of the two quantities `f` (pressure rise, shaft power): met iff `abs(f_B1 - f_measured) <= U_num +
u_meas`, where `U_num` is the mesh-convergence estimate and `u_meas` the stated uncertainty of the measurement. The two
are added, not combined in quadrature: neither is a standard deviation of a known distribution, and the sum is the
larger band.

**The mesh-convergence estimate, `U_num`.** Fixed now from the recorded triplets (B1, B2, B3) of ADR 0170 for the
torque and ADR 0086 for the pressure rise, and not recomputed after the run. Derivation: if the functional on a mesh of
spacing `h` is `f(h) = f_0 + C h^p`, then with the refinement ratio `r = h_2 / h_1` the error of the fine value is
`f_1 - f_0 = (f_2 - f_1) / (r^p - 1)`, and `p` follows from three meshes. The estimate is that error with the order
capped at 1 (`p_band = min(p, 1)`) and times a factor of safety 1.25, as ADR 0086 registered it:
`U_num = 1.25 abs(f_2 - f_1) / (r_21^p_band - 1)`, `r_21 = (38184 / 13627)^(1/3) = 1.40981`. The factor 1.25 is
Roache's, in the three-mesh form of Celik et al. (2008), as ADR 0086 cites them. Citation from memory: no source could
be read (ADR 0086 itself marks both as recalled, not re-read); what is used is the formula derived above, and the factor
is taken as ADR 0086 registered it. Arithmetic (`band.py`): torque 2.098164 / 2.028692 / 1.918189 N m gives `p` 1.3928
and `U_num` 0.21190 N m, which is 32.98 W; pressure rise 73.988 / 64.233 / 46.778 Pa gives `p` 1.7378 and `U_num`
29.75 Pa. These agree with ADR 0170 (0.2119 N m) and ADR 0086 (29.76 Pa).

| Quantity | Measured | `U_num` | `u_meas` |
| --- | --- | --- | --- |
| total-to-static pressure rise | 126.46 Pa | **29.75 Pa** (23.5 % of the measured value) | not registered: no stated uncertainty in a source that was read |
| shaft power | 333.30 W | **32.98 W** (9.9 %) | not registered: the same |

**How the result is read without `u_meas`.** The check is evaluated with `u_meas = 0`, the narrowest band the form
allows, and beside it the addendum states, for each quantity, the `u_meas` that would be needed for the result to be
inside. Nothing is called a pass on this registration: with `u_meas` open, "done" is not an outcome it can return
(decision rule). When a stated uncertainty has been read, a new registration adds it to this form; it will then be
registered with this result known, which this text says now so that it is seen then.

**What this band is and is not.** It is wide. ADR 0086 found its bands "not shown credible" (the empty-passage check
failed), its family does not refine the wall-normal direction, and ADR 0110 showed that the three values are samples of
limit cycles. I register it because the acceptance asks for a mesh-convergence estimate and this is the one the
repository has; a result inside it would be a weak statement, and the gates and the convergence rule below are what
keep a cycle sample on a missed mesh from being read as a pass.

**Mesh quality gates** (the registered gates of the fan passage mesh, `docs/meshing.md`, section on `rcfd_mesh::fan`,
"Quality (registered gates are missed)", set-up A, and the gap of F-133), on the mesh of the run, by
`rcfd_mesh::quality::check_mesh`: largest non-orthogonality **<= 70 deg**, largest skewness **<= 2.0**, largest aspect
ratio **<= 300**.

**Convergence rule.** The solve is converged iff `Simulation::is_converged()` is true within 8000 iterations with
`tolerance_u = tolerance_p = 1e-5` (the tolerance ADR 0086 registered). The cap rule of ADR 0075 and ADR 0086 is
reported beside it and does **not** count as converged for F-870: ADR 0110 showed that the states that meet it are
samples of a limit cycle.

## The test that holds the state (task 5; fixed now)

One new file, `crates/rcfd-api/tests/f870_fan01_design_point.rs` (header: ADR 0762, F-870), with one function
`run_design_point(counts, cap)` that every solving test below calls: the same path for the heavy record and the fast
test.

* Run by the gate (not ignored):
  * `f870_reference_and_band`: the design row of the CSV equals 1.4032 m^3/s, 126.46 Pa, 333.30 W to the printed
    digits; the band arithmetic above reproduces 0.21190 N m and 29.75 Pa; the estimate recovers the exact error of a
    power law.
  * `f870_b1_mesh_gates_as_recorded`: builds the B1 mesh and asserts its three quality numbers against the recorded
    ones (ADR 0086: 83.37 deg, 3.765, 252.4) within 0.5 % and the verdict of each gate as recorded (missed, missed,
    met), so that a mesh that starts to meet a gate, or gets worse, fails the test.
  * `f870_b3_short_run_holds_the_state`: B3 for 300 iterations; torque and pressure rise at iteration 300 within the
    tolerance below of the values recorded from its first run.
* Heavy records (`#[ignore]`, run once by me, in one process so that they share one solve):
  * `f870_b1_design_point_state_record`: B1 to 8000 iterations; torque and pressure rise within the tolerance below of
    the recorded values, and the verdict of each acceptance clause (band for each quantity, each gate, convergence)
    equal to the recorded verdict. It fails if the result moves either way, and it fails if a missed clause starts to
    be met.
  * `f870_b1_design_point_acceptance`: asserts the acceptance of F-870 itself, with the limits of this registration
    intact. If it misses, it is renamed `..._known_miss` with the measured numbers in its ignore reason (rule 2).
  * `f870_family_run`: one of B1, B2, B3 by an environment variable, printing the outcome (the three registered runs).

**The tolerance of the state** (both the heavy record and the fast test): **1 %** of the recorded value, for torque and
for pressure rise. Source: the cap rule of ADR 0075 and ADR 0086 defines a functional of this case only to 0.5 % (its
change over the last 1000 iterations); the tolerance is twice that, for two samples each defined to 0.5 %. The solver is
deterministic (ADR 0181 reproduced B3 and B2 to the printed digits), so an unchanged main gives a difference of zero;
anything above 1 % is a change of the solver, of the mesh or of the set-up. The recorded values are those of the first
run of each test on current main and are written into the file after that run; the tolerance and the form are fixed
here.

Which is which, in the words of `docs/test-process.md`: the three tests of the first group run in the full gate and in
the per-merge selection; the B1 solve is a heavy record that the gate lists as ignored.

## Table of checks

Every clause of the acceptance first. "Estimate" is what I expect from the records above; "margin" is the limit divided
by the estimate (for a distance) or the estimate's side of the limit.

| Id | Check | Limit | Source of the limit | Estimate | Margin | Kind |
| --- | --- | --- | --- | --- | --- | --- |
| A1 | pressure rise of B1 against the measurement: `abs(dp_ts - 126.46)` | `<= 29.75 Pa + u_meas`, evaluated with `u_meas = 0` | acceptance of F-870 (band from a mesh-convergence estimate and the stated uncertainty); `U_num` from ADR 0086 as derived above | 52.5 Pa (73.99 Pa) | 0.57: expected MISS | validation |
| A2 | shaft power of B1 against the measurement: `abs(P - 333.30)` | `<= 32.98 W + u_meas`, evaluated with `u_meas = 0` | the same; `U_num` from ADR 0170 | 6.8 W (326.50 W) | 4.9: expected inside the numerical part | validation |
| A3a | largest non-orthogonality of the B1 mesh | `<= 70 deg` | `docs/meshing.md`, registered gates of the fan passage mesh (set-up A); gap of F-133 | 83.37 deg | 0.84: expected MISS | solution verification |
| A3b | largest skewness of the B1 mesh | `<= 2.0` | the same | 3.765 | 0.53: expected MISS | solution verification |
| A3c | largest aspect ratio of the B1 mesh | `<= 300` | the same | 252.4 | 1.19: expected met | solution verification |
| A4 | the B1 solve converged: `is_converged()` at 1e-5 within 8000 iterations | true | acceptance of F-870 ("converged by its registered rule"); tolerance of ADR 0086 | `res_u` 1.2e-3 at 8000 | 0.008: expected MISS | solution verification |
| A5 | a miss is recorded as a miss: `f870_b1_design_point_acceptance` fails on every missed clause and is kept as `..._known_miss` with its numbers; the state record asserts the verdicts | as stated | acceptance of F-870 (last clause); rule 2 | as stated | n/a | validation |
| R1 | B1, B2, B3 on current main against the triplets of ADR 0170 and ADR 0086 | each torque and pressure rise within 1 % | twice the 0.5 % of the cap rule (ADR 0075, ADR 0086) | 0 (the same fields), unless main's solver changed since 2026-10-30 | n/a | solution verification |
| R2 | the cap rule on B1, reported beside A4 | none (report) | - | met, as in ADR 0170 | n/a | solution verification |
| R3 | the estimate `U_num` recomputed from the three runs of current main, reported beside the registered one | none (report; the registered band is not changed) | - | equal to the registered values if R1 gives 0 | n/a | solution verification |
| S1 | `f870_b3_short_run_holds_the_state`: B3 at iteration 300 against its first run | 1 % each | as R1 | 0 | n/a | unchanged default output |
| S2 | `f870_reference_and_band`: CSV row; band arithmetic; power-law recovery | printed digits (5e-5, 5e-3, 5e-3); 1e-4 relative on `U_num`; 1e-9 relative on the power law | the CSV; this registration; closed form (`docs/process/tolerances.md`, "Agreement with a closed form") | exact | n/a | code verification |
| S3 | `f870_b1_mesh_gates_as_recorded` | 0.5 % on each number; verdicts as recorded | ADR 0086 lines 51 to 57 (the numbers are printed to four digits) | 0 | n/a | unchanged default output |
| D1 | defaults: no file under `crates/*/src` is changed | 0 files | rule 3 | 0 | n/a | unchanged default output |
| P | `cargo fmt -p rcfd-api --check`; `cargo clippy -p rcfd-api --all-targets -- -D warnings`; `cargo test -p rcfd-cli --release --test docs_site`; `python scripts/roadmap_proposals.py check`; `python scripts/process_report.py --check` | clean; passes; no failure but a generated file not current | rule 7 | as stated | n/a | code verification |

A1 and A2 are the rows of kind validation. The limits of R1, S1 and S3 are tolerances of a state record; their source
is the cap rule's 0.5 % and the printed precision of the record, not a new estimate of mine.

## Negative controls and planned mutations

Run on a scratch copy of the tree (`git archive` of the branch into the scratch folder, its own target folder), each
against the fast tests; the heavy record is not rerun for a mutation.

| Id | Mutation | Must be caught by |
| --- | --- | --- |
| M1 | solver: the Coriolis factor of `RotatingFrame::accel` (`crates/rcfd-fv/src/sources.rs`) from -2.0 to -1.9 | `f870_b3_short_run_holds_the_state` |
| M2 | test set-up: the speed from 1486 to 1501 rpm (1 %) | `f870_b3_short_run_holds_the_state` (the torque moves by about 2 %) |
| M3 | test: the factor of safety 1.25 to 3 | `f870_reference_and_band` |
| M4 | test: `n_span` of B1 from 21 to 20 | `f870_b1_mesh_gates_as_recorded` (cell count and numbers) |
| M5 | test: the gate for non-orthogonality from 70 to 85 deg (a widened gate) | `f870_b1_mesh_gates_as_recorded` (the verdict "missed" no longer holds) |
| K1 | negative control: the B3 short run made twice | equal to the last printed digit (the tolerance is not hiding noise) |

M1 is committed as `crates/rcfd-api/tests/f870_fan01_design_point.mutation.patch` with the first line
`# expect-fail: cargo test -p rcfd-api --release --test f870_fan01_design_point f870_b3_short_run_holds_the_state`.

## The ranked gap (task 4): what the addendum will hold

For each cause still standing: the evidence for its share with the number a diagnosis gives; what would have to be
built or changed; the roadmap item that carries it or the words that no item does; my estimate of its size. The causes
to be ranked are fixed now, from the reading, so that the run does not choose them: the compared quantity (a rotor-only
passage against an installation value with nozzle and diffuser); the limit cycle of the limited second-order scheme
and the question of ADR 0181 and ADR 0183 whether a steady state exists at the hub trailing edge; the mesh gates; the
resolution (in-plane, radial and wall-normal) and the credibility of the mesh-convergence estimate; the rotating-frame
discretisation (spurious swirl) and the absolute-velocity form; the unknowns of the measurement (uncertainty, density);
the geometry (chords 4 to 5 % shorter than the published table, no root fillet, NACA thickness law). Follow-ups are
proposed with their acceptance; none is made here.

## Decision rule

* **Done** only if pressure rise and shaft power are both within the registered band (A1, A2, with a registered
  `u_meas`) on a mesh that meets the registered gates (A3a to A3c) with a converged solve (A4). With `u_meas` not
  registered, this registration cannot return done.
* **Partial** if the band, the run, the ranked gap and the test are delivered and the result misses, with the numbers.
* **Stopped and reported** if the measurement or its uncertainty cannot be traced to a source that was read.

The third applies already: the uncertainty is not traced. I deliver the run, the ranked gap and the test as for
partial, and report the verdict as the rule gives it, with what was delivered beside it. The roadmap has no status for
"stopped"; the status I propose is for the lead to weigh (rule 9 entries in the addendum).

## What is not covered

* `u_meas` and the density of the measurement (open questions 1 and 2).
* One operating point; the off-design point of `fan01_validation.rs` is not run.
* The wall-normal resolution is not in `U_num` (ADR 0086 did not refine the first cell or the stacks), and no
  measured y+ of the run is a gate.
* The empty-passage check of ADR 0086 is not repeated; the band stays "not shown credible" in ADR 0086's words.
* The rotor-only model: no nozzle, no finite hub, no struts, no diffuser. The measured pressure rise includes them.
* No fix, no solver change, no new mesh construction, no transient run. Check H of ADR 0183 is not run here.
* The H-slit set-up with minmod (arm KN of ADR 0078, the best recorded pressure rise) is not rerun: one set-up is run,
  and B1 is the one the mesh-convergence estimate belongs to.
* Kromer's thesis and the SAE paper were not read.
* The existing tests `fan01_validation.rs` and `fan01_gci_w37c.rs` are not edited, un-ignored or removed.

## Amendment 1 (2026-11-22, before the corrected run of S2 and before any registered solve): the tolerance of S2 on `U_num`

Written after the first run of the three gate tests of the new file (uncommitted; no heavy run has been made). That run
gave: S3 passed (B1 mesh 38 184 cells, 83.372 deg, 3.7647, 252.42); S1 printed its first values (B3 at iteration 300:
torque 1.918174 N m, 46.5819 Pa) and failed as written, because no value was recorded yet; S2 failed on one assertion.

The defect is in the set-up of S2, not in the band. The registration gives `U_num` to the printed digits, 0.21190 N m and
29.75 Pa, and asks S2 to reproduce them within 1e-4 relative. The arithmetic gives 0.2119034 N m and 29.75469 Pa: the
second differs from its own rounded value by 1.6e-4, so the registered tolerance is tighter than the rounding of the
number it checks. Corrected, fixed now: S2 asserts each `U_num` to half a unit of its last printed digit,
`abs(U - 0.21190) <= 5e-6` N m and `abs(U - 29.75) <= 5e-3` Pa. The band is unchanged: A1 and A2 use the registered
29.75 Pa and 0.21190 N m (32.98 W) as written. The other limits of S2 stay.

The first values of S1 are the record of the registration's words ("the recorded values are those of the first run of
each test on current main"): B3 at iteration 300, 1.918174 N m and 46.5819 Pa.

## Addendum 1: results

Written 2026-11-22, after the runs. Commits on the branch, from main 13b0937d: a2c58ff1 (this registration, alone),
a6566b21 (amendment 1, before the corrected run of S2 and before any solve), da68a8bf (the test file), df09d254 (the
mutation patch, the reference-data fragment, the line of `docs/validation-data.md`), 024ef792 (the recorded B1 state and
the acceptance test kept as a known miss), and the commit of this addendum with the proposal file and the logs. The
heavy runs were made from da68a8bf; their logs are `docs/investigations/f870-fan01-design-point/B1.log`, `B2.log` and
`B3.log`, and the arithmetic of the band is `band.py` beside them.

### The result against the band

FAN-01, design point, 1.4032 m^3/s at 1486 rpm, B1 (38 184 cells), 8000 iterations, 49.9 minutes.

| Quantity | Simulated | Measured | Distance | Registered band (numerical part; `u_meas` not registered) | Verdict |
| --- | --- | --- | --- | --- | --- |
| total-to-static pressure rise | 73.988 Pa | 126.46 Pa | 52.47 Pa (-41.5 %) | 29.75 Pa | **missed**; a measurement uncertainty of 22.72 Pa (18 % of the measured value) would be needed to close it |
| shaft power | 326.50 W (torque 2.098164 N m) | 333.30 W | 6.80 W (-2.0 %) | 32.98 W | inside the numerical part of the band; not a pass of the clause, because the band is not complete without `u_meas` |

### Every check

| Id | Measured | Met |
| --- | --- | --- |
| A1 | `abs(73.988 - 126.46)` = 52.472 Pa against 29.75 Pa | **no, a miss** |
| A2 | `abs(326.503 - 333.30)` = 6.797 W against 32.98 W | yes, for the numerical part only |
| A3a | largest non-orthogonality of B1 83.37 deg against 70 | **no, a miss** |
| A3b | largest skewness of B1 3.765 against 2.0 | **no, a miss** |
| A3c | largest aspect ratio of B1 252.4 against 300 | yes |
| A4 | `is_converged()` false at 8000 iterations: `res_u` 1.208e-3 and `res_p` 9.290e-4 against 1e-5 | **no, a miss** |
| A5 | the first heavy run of `f870_b1_design_point_acceptance` failed and named A1, A3a, A3b and A4; it is kept, ignored, as `f870_b1_design_point_acceptance_known_miss`, with the numbers in its reason; `f870_b1_design_point_state_record` asserts the six verdicts and the two values | yes |
| R1 | B1 2.098164 N m and 73.9881 Pa, B2 2.028692 and 64.2329, B3 1.918189 and 46.7776: each equal to the triplets of ADR 0170 and ADR 0086 to their printed digits (difference 0 against 1 %) | yes |
| R2 | the cap rule on B1: change over the last 1000 iterations 5.784e-6 N m (2.8e-4 %) and 9.747e-5 Pa (1.3e-4 %), mass balance 5.94e-10: met. Reported; it is not the registered convergence rule. B2 and B3 the same (`res_u` 1.003e-3 and 1.030e-3, cap rule met, 20.0 and 9.1 minutes) | report |
| R3 | `U_num` from the three runs of current main: 0.21190 N m and 29.75 Pa, the registered values | report |
| S1 | B3 at iteration 300: 1.918174 N m and 46.5819 Pa, equal to the recorded values in every run made (K1) | yes |
| S2 | passes with the tolerance of amendment 1 | yes, after amendment 1 |
| S3 | B1 mesh 38 184 cells, 83.372 deg, 3.7647, 252.42; verdicts missed, missed, met as recorded | yes |
| D1 | `git diff --name-only 13b0937d` names no file under `crates/*/src`; two files under `crates/rcfd-api/tests` | yes |
| P | see "The checks of rule 7" below | see below |

Mesh gates of the other two meshes, reported: B2 79.54 deg, 4.618, 204.0; B3 78.39 deg, 1.578, 168.9.

### What is a gate test and what is a heavy record

All in `crates/rcfd-api/tests/f870_fan01_design_point.rs`.

* Run with every `cargo test` (about 25 s together in release): `f870_reference_and_band` (the CSV row of the
  measurement and the arithmetic of the band), `f870_b1_mesh_gates_as_recorded` (the B1 mesh, its three numbers and
  their verdicts against the registered gates), `f870_b3_short_run_holds_the_state` (the same case text and the same
  code path as the heavy record, on B3 for 300 iterations, each value within 1 % of 1.918174 N m and 46.5819 Pa, either
  way).
* Ignored, heavy, run by name: `f870_b1_design_point_state_record` (50 minutes; fails if torque or pressure rise leave
  1 % of 2.098164 N m and 73.9881 Pa either way, or if any of the six verdicts changes, so an improvement fails it too
  and has to be recorded); `f870_b1_design_point_acceptance_known_miss` (the acceptance as a test; it fails today, and
  its ignore reason carries the numbers; it is the test that passes when F-870 is done); `f870_family_run` (B2 and B3
  for R1 and R3).

### Mutations and negative controls

Run on a copy of the tree in the scratch folder with its own target folder. All five were caught by the test named in
the registration.

| Id | What happened |
| --- | --- |
| M1 | Coriolis factor -2.0 to -1.9: B3 at iteration 300 gives 34.09 Pa against 46.58 (-27 %); torque 1.918645 against 1.918174 (0.02 %, inside its tolerance). Caught by the pressure rise. Committed as the mutation patch |
| M2 | 1501 rpm: 1.9586 N m (+2.1 %) and 49.49 Pa (+6.2 %). Caught |
| M3 | factor of safety 3: `f870_reference_and_band` fails. Caught |
| M4 | `n_span` 20: 36 960 cells against 38 184. Caught |
| M5 | gate 85 deg: the verdict assertion fails ("gate 0: 83.37 against 85"). Caught |
| K1 | the B3 short run gave 1.918174 N m and 46.5819 Pa in each of its runs on the unmutated tree |

M1 shows a weakness worth saying: at 1 % the torque alone would not have caught a 5 % error of the Coriolis term on
this short run; the pressure rise did.

### Deviations

1. Amendment 1: the tolerance of S2 on `U_num` was tighter than the rounding of the number it checks. Committed before
   the corrected run and before any solve. The band is unchanged.
2. The case text of the new test is that of `fan01_gci_w37c.rs` without its report-only sections; R1 shows the values
   are the same to the printed digits.
3. `f870_b1_design_point_state_record` was run a second time after its values were recorded, to see the test pass with
   its constants: one more heavy solve than the three of the Size line. Its result is in the last line of this section.
4. In the first B1 run both heavy tests failed, as planned: the acceptance test on its four misses, the state record
   because no value was recorded yet. Neither failure was hidden; the values were recorded from that run (024ef792).
5. The registration says no cargo command was run before it. `cargo test -p rcfd-cli --release --test docs_site` was
   run on the registration's text before its commit, to check its links. No test of F-870 existed then.
6. The mutation patch was applied and run by hand on the scratch copy; the repository's mutation runner was not used.

Confirmation run of deviation 3: `f870_b1_design_point_state_record` passed (1 passed, 38.1 minutes): torque 2.098164 N m, pressure rise
73.9881 Pa, the six verdicts as recorded, every printed digit equal to the first run. Its log is
`docs/investigations/f870-fan01-design-point/B1_confirm.log`.

### What was not run

* Nothing with a registered `u_meas`: the uncertainty of the measurement is not traced, so the clauses A1 and A2 are
  evaluated on the numerical part alone.
* The off-design point, a fourth mesh, the empty-passage check of ADR 0086, check H of ADR 0183, the H-slit set-up with
  minmod: as said under "What is not covered".
* The comparison with the LDA profiles (rank 1 of the gap) is proposed, not made.
* The full workspace test run and the heavy tests of other files: not run. Of the workspace only the new file's tests,
  `docs_site`, fmt and clippy of `rcfd-api` were run.
* Kromer's thesis and the SAE paper were not read: the open-access server of the thesis answered an automated client
  with a check that I did not go around.

### The checks of rule 7

* `cargo fmt -p rcfd-api --check`: clean. `cargo clippy -p rcfd-api --all-targets -- -D warnings`: clean.
* `cargo test -p rcfd-cli --release --test docs_site`: 2 passed, with this addendum in the tree.
* `cargo test -p rcfd-api --release --test f870_fan01_design_point`: 3 passed, 3 ignored (the heavy records).
* `python scripts/roadmap_proposals.py check docs/roadmap/proposals/0762-F-870.json`: passes (F-870 todo to partial,
  gap set, four new items).
* `python scripts/process_report.py --check`: 2 failures, both "generated file is not current": `docs/adr/index.md` and
  `docs/reference-data.md`. Both are generated pages and are left to the lead (rule 9). No other failure.

### Size

Size (agent): partial; 0; 4; 4; 900000

Four heavy runs against three registered (deviation 3). The bytes read are an estimate.

### Decision by the registered rule

**Stopped and reported**: the uncertainty of the measurement cannot be traced to a source that was read, so the band of
the acceptance could not be registered in full. Beside it, as the rule says: the band's numerical part, the run, the
ranked gap and the test are delivered as for partial, and the result is a miss on four clauses (A1, A3a, A3b, A4) with
the numbers above. The roadmap has no status "stopped"; I propose **partial** and the lead weighs it.

### The gap, ranked (task 4)

What misses, in the order of the acceptance: the pressure rise (A1), the mesh gates (A3a, A3b) and the convergence
(A4); the shaft power (A2) is inside the numerical part of its band. The ranking below is by what each cause blocks and
by the evidence for its share. A number is given where a record gives one; where none does, that is said. Sizes are
mine: small is one registration without solver code, medium one registration with solver or mesher code, large more
than one.

**The shape of the miss, from the records.** The power agrees and the pressure does not. On B1 the rotor takes
326.5 W of the measured 333.3 W (-2.0 %) and raises the mass-averaged absolute total pressure by 157.25 Pa (ADR 0086,
table of the bladed balance, B1; prose, not rerun here), a total-to-total efficiency of 1.4032 x 157.25 / 326.5 = 0.68.
The compared total-to-static value, 73.99 Pa, is that total rise less everything that leaves the passage as velocity:
157.25 - 73.99 = 83.3 Pa, of which the axial dynamic pressure of the annulus is `rho/2 (Q/A)^2` = 0.6 x (1.4032 /
0.14804)^2 = 53.9 Pa and the outlet swirl energy 23.5 Pa (ADR 0086, the same table). The measurement is taken behind a
finite hub and a diffuser, to ambient. The miss of 52.5 Pa is 63 % of the 83.3 Pa that the rotor-only definition
discards. Mesh error does not close it on this family: the extrapolated pressure rise of ADR 0086 is 85.9 Pa, still
40.5 Pa (32 %) below the measurement, and its band [44, 104] Pa does not contain 126.46 Pa.

| Rank | Cause still standing | Blocks | Evidence for its share | What would have to be built or changed | Roadmap item | Size |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | **The compared quantity.** A rotor-only passage with a cylindrical hub through the outlet is compared with a pressure measured across the installation: inlet nozzle, a hub that ends 12 mm behind the blade root (rear hub face at x = 104.5 mm, root trailing edge at 92.1 mm), a diffuser to x = 286.3 mm, exit to ambient (v6 description of the record, table of axial positions) | A1 | The numbers above: 52.5 Pa missing, 83.3 Pa of outlet dynamic pressure discarded, of which 53.9 Pa axial. From first principles, the axial dynamic pressure falls from 53.9 Pa on the annulus to 30.6 Pa on the full duct area (0.19635 m^2) behind the hub, so an ideal recovery to the duct area alone is 23 Pa, and a sudden expansion recovers `rho u_2 (u_1 - u_2)` = 20.0 Pa; the diffuser adds to that. This is my arithmetic, not a diagnosis. Diagnoses: the outlet type alone moves the pressure rise by 8.2 Pa (ADR 0078, arm C3); ADR 0078 concluded that the pressure and efficiency limits "cannot be met in principle without modelling the installation or redefining the compared quantity"; the 40.5 Pa left by the extrapolation of ADR 0086. No record separates the installation from the rest of the 40.5 Pa | (i) Compare a quantity of the rotor alone: the LDA profiles of the record 2.2 mm behind the trailing edge (x = 90 mm, r 133 to 246 mm, 26 points; the v6 description settles the datum that `docs/validation-data.md` calls unsettled), meridional and circumferential velocity, and the Euler work from them. Post-processing of the existing solve, no solver or mesher code. (ii) Model the outlet: a hub that ends and a shroud contour (duct and diffuser) as surfaces of revolution in `axial_fan_sector`, the pressure taken as on the rig | no item. `docs/validation-data.md` names the need ("an inlet nozzle / finite hub / diffuser model or a rig-independent measured quantity"). F-133 carries the passage mesher but not hub or shroud contours of this generator | (i) small; (ii) medium to large |
| 2 | **No converged second-order solution.** The limited `linear_upwind` solve of the bladed passage ends in a limit cycle (ADR 0110), driven at the leading-edge root and at the hub trailing edge (ADR 0175, 0178, 0181) | A4; it also makes every functional of rows A1 and A2 a sample of a cycle, and blocks F-260 | Share of the value: small. The one converged mixed-order state on B3 (ADR 0178, first order in 322 hub cells) differs from the cycle sample by -0.2 % in torque (1.9144 against 1.9177 N m) and -2 % in pressure rise (45.75 against 46.7 Pa); the automatic selector moved the B2 torque by 2.9 % (ADR 0179); three TVD schemes move the B2 pressure rise by -6 % (ADR 0180). Share of the clause: all of it (`res_u` 1.2e-3 against 1e-5) | First the registered, unrun check H of ADR 0183: a time-accurate SST run on B2 from the stalled state, runnable since F-286 (ADR 0199), which classes the pocket as steady, physical or numerical. If physical, no steady state exists and the convergence rule of F-870 has to become a rule on a time mean (a maintainer decision, then an amendment of the rule); if numerical, a treatment of the deferred correction everywhere (ADR 0175 names it as the next candidate) | no item: ADR 0183 names F-276, which is not in `docs/roadmap.json`; ADR 0757 says of the stall diagnoses "no item has them as its subject" | small (check H is one registered run); the fix after it medium |
| 3 | **The mesh gates.** B1 has 83.37 deg against 70 and skewness 3.765 against 2.0 (aspect 252.4 against 300 is met); the worst faces are at the lower seam corner of the tip layers (`docs/milestone-fan.md`) | A3a, A3b | Share of the value: no record gives one. The stall cells are not at the bad faces (ADR 0110: 26 to 28 deg; ADR 0174). ADR 0087 moved the corner from 83 to 78 deg and was rejected; the later construction S gives 88.0 deg and 4.30 on FAN-01 (ADR 0692, R2), worse | A topology change at the tip (the index-offset topology of ADR 0087 was not tried), or a decision of the maintainer that the gate for this case is another number, registered before a run | F-133 (its gap names the gates); F-726, F-777, F-778 work on other blades | large |
| 4 | **Resolution, and whether the estimate can be believed.** | the band itself (A1, A2) | `U_num` is 40 % of the pressure rise and 10 % of the torque; the wall-normal direction is not refined (first cell 0.565 mm by the y+ 30 rule, two stack cells); ADR 0086's check of the estimate on the empty passage failed. B1 is 12.0 Pa and 0.113 N m below the extrapolated values | A fourth mesh with the first cell and the stacks refined too, and the empty-passage check repeated; worth little before rank 2 is settled, since each value is a cycle sample | no item of its own; part of F-870 | small to medium (compute, about 3 h for one finer mesh alone) |
| 5 | **The rotating-frame discretisation** (limit (a) of the milestone page): convection of the rigid swirl creates angular momentum | A1 and A2, by a few per cent | Empty passage on the B1 mesh: 0.106 N m of outlet angular momentum that should be zero (5 % of the measured torque), growing with refinement, and 4.5 to 9.2 Pa of total pressure created in an empty duct (ADR 0086); the bladed balance of B1 is open by -0.059 N m, 2.8 % of the torque (ADR 0170) | The absolute-velocity form on the bladed passage (built and removed in ADR 0082 and ADR 0085 because it did not converge), or rigid rotation of the mesh, arm 1 of ADR 0183 (registered, not run) | F-126 (its gap: "MRF on the bladed FAN-01 passage") | medium to large |
| 6 | **The unknowns of the measurement**: its stated uncertainty and the air density | the registration of the band, so "done" itself; A1 and A2 by the density | Both simulated quantities scale with the assumed 1.2 kg/m^3: one per cent of density is one per cent of each (0.74 Pa and 3.3 W). No source read gives either number | Reading Kromer's thesis (open access; the fetch was refused to an automated client) or the SAE paper | none; open question 1 | small (a person and a browser) |
| 7 | **Geometry and model**: CAD chords 4 to 5 % shorter than the published table, no 3 mm root fillet, NACA 4-digit thickness law (the CAD has its maximum at 0.23 chord), fully turbulent SST with wall functions at chord Reynolds numbers of 1.25e5 to 1.5e5 | A1, A2 | No record gives a number. The strip estimate of ADR 0078 exceeds the measured torque by 12 to 22 %, so the blade as modelled can absorb the power, and B1 does to 2 % | Not proposed before ranks 1 to 3 | F-046 (transition) is outside the milestone | - |

**Which one or two pieces of work move the headline, and in what order.** The pressure clause cannot be met by any
change of scheme or mesh while a rotor-only passage is compared with an installation value: the repository's own
extrapolation leaves 40 Pa. So the first piece is rank 1, in its cheap form first: the comparison with the LDA profiles
behind the rotor says, from the solve that exists, whether the rotor's work is right, and so whether the 52.5 Pa is
installation or rotor; the hub end and diffuser then follow only if it is installation. The second piece is rank 2,
check H of ADR 0183: one registered run that decides whether the convergence clause of F-870 can be met by a steady
solve at all. Rank 6 costs a person a few minutes and should be done before either, because without it no band can be
registered in full. Ranks 3, 4 and 5 do not move the headline until 1 and 2 are answered.

## Open questions for the maintainer

1. **The uncertainty of the measurement.** No source read states it (the record at Zenodo in versions 5 and 6 with its
   description, the benchmark paper arXiv:2211.12014, the two conference papers named under "What was read"). Options:
   (a) a person opens Kromer's thesis (doi:10.25593/978-3-96147-089-1, open access) and the SAE paper 2016-01-1805 in
   a browser and records the stated uncertainty of pressure, flow, torque and the density, with the page; (b) the
   maintainer sets the band of this measured case, as `docs/process/tolerances.md` allows ("to be set by the maintainer
   per measured case"); (c) the scatter between the laboratories of the round robin is taken as the uncertainty, which
   is another quantity than the one the acceptance names. I recommend (a), and (b) only if the thesis does not state
   it. Until one of them, F-870 cannot be done by its own words.
2. **The air density of the measurement.** The repository assumes 1.2 kg/m^3; no source read gives it, and both compared
   quantities scale with it. Options: (a) read it with question 1; (b) keep 1.2 and carry one per cent of density
   (0.74 Pa, 3.3 W) in `u_meas`. I recommend (a).
3. **Which follow-ups, in what order.** Four are proposed below. I recommend this order: the reading of question 1; the
   comparison with the LDA profiles behind the rotor (small, no solver code, says whether the 52.5 Pa is rotor or
   installation); check H of ADR 0183 (one registered run, decides whether a steady solve can converge at all); the
   hub end and diffuser in the passage generator only if the LDA comparison says installation. The mesh gates stay
   with F-133 and no new item is proposed for them.
4. **The compared quantity and the convergence rule.** If the LDA comparison shows the rotor's work is right, the
   headline of F-870 compares a rotor-only model with an installation value and cannot be met without the outlet
   model: is the outlet model wanted for the milestone, or does the headline become a rotor quantity? And if check H
   classes the hub pocket as physical, no steady state exists: is a rule on a time mean acceptable for F-870? Both are
   changes of the acceptance, so they are the maintainer's. I recommend deciding neither before the two cheap runs of
   question 3 have given their answers.

## For the lead: roadmap entries

> Status proposed for F-870: partial.

> Note entry: ADR 0762, 2026-11-22, partial, and by its own rule stopped on the band: the stated uncertainty of the FAN-01 measurement is in no source that could be read, so the band is its numerical part alone (29.75 Pa, 32.98 W from the B1/B2/B3 family of ADR 0086, factor of safety 1.25). On main 13b0937d, B1 (38 184 cells, 8000 iterations): pressure rise 73.99 Pa against 126.46 Pa (-41.5 %, distance 52.47 Pa, a miss), shaft power 326.50 W against 333.30 W (-2.0 %, inside), non-orthogonality 83.37 deg against 70 and skewness 3.765 against 2.0 (misses), res_u 1.2e-3 against 1e-5 (a miss). The state is held by crates/rcfd-api/tests/f870_fan01_design_point.rs: three gate tests (a B3 short run on the heavy record's path, 1 % either way) and a heavy record of B1; five mutations caught. The ranked gap and four questions are in the ADR.

> Gap entry: Missing for done: the stated uncertainty and density of the measurement (not traced; no full band can be registered); the pressure rise (73.99 Pa against 126.46 Pa, band 29.75 Pa), where a rotor-only passage is compared with an installation value; the mesh gates of B1 (83.37 deg, skewness 3.765), with F-133; a converged solve (limit cycle, res_u 1.2e-3). Shaft power is within 2.0 %.

> Follow-up (proposed, open question 1): "FAN-01 measurement: its stated uncertainty and air density, read from the thesis of the measurement and recorded", deps F-870; acceptance: Every limit not stated here is registered in the ADR before the run. The uncertainty of the pressure rise, the volume flow and the torque of the FAN-01 measurement and the air density of the measurement are recorded in docs/validation-data.md, each with its source, page and the date it was read, from a published source that was read; or, where the source does not state one, the page says so and names the band the maintainer sets for this measured case. The band of F-870 is then registered in full by an amendment before any run that uses it.

> Follow-up (proposed, open question 3): "FAN-01 rotor alone: the solved field against the measured LDA profiles behind the trailing edge and the Euler work from them", deps F-870; acceptance: Every limit not stated here is registered in the ADR before the run. The circumferentially averaged meridional and circumferential velocity of the FAN-01 design-point solve at the measured plane behind the trailing edge is compared with the LDA profiles of the published record, which a page of the repository records with their citation, at each measured radius, within a band registered before the comparison; the Euler work of the measured and of the simulated profile is reported with the measured shaft power, and the report states what part of the miss of the pressure rise of F-870 is the rotor's and what part the installation's. A miss is recorded as a miss with its numbers.

> Follow-up (proposed, open question 3): "FAN-01 hub pocket: check H of ADR 0183, a time-accurate SST run that classes it as steady, physical or numerical", deps F-870, F-286; acceptance: Every limit not stated here is registered in the ADR before the run. The check H of ADR 0183 is run as registered there, a time-accurate k-omega SST run of the FAN-01 passage from the stalled steady state, and classes the reversed-flow pocket at the hub trailing edge as steady, physical unsteadiness or numerical by the rule registered before the run, with the time step, the time run and the measured amplitude and frequency of torque and pressure rise; the report states what the class means for the convergence rule of F-870. A run that does not decide is recorded as not deciding.

> Follow-up (proposed, open question 4): "FAN-01 outlet as on the rig: a hub that ends and the diffuser contour in the passage generator, the pressure rise taken as measured", deps new:fan01-lda, F-133; acceptance: Every limit not stated here is registered in the ADR before the run. The FAN-01 passage is generated with the hub ending at its published axial position and the duct and diffuser contour of the published rig as surfaces of revolution, within the registered mesh gates, and the total-to-static pressure rise is taken between the stations of the measurement; the design point is run and reported against the band of F-870, with the change against the rotor-only passage of ADR 0762 as a number. A miss is recorded as a miss.

The same entries are in `docs/roadmap/proposals/0762-F-870.json` (form 1 of ADR 0755, base 13b0937d), checked with
`python scripts/roadmap_proposals.py check`.

Generated pages not current on this work, left to the lead (rule 9): `docs/adr/index.md` (ADR 0762 is new) and
`docs/reference-data.md` (the fragment `docs/reference-data.d/0762-F-870.part` is new).

## The lead's reading (2026-11-22), after the merge

The report was read when it came (2026-11-22, 06:18 UTC); the merge waited 94 minutes for the left-out runs of the two
merges before it, which build from the same tree. Merged at 3a241b6d and pushed at 06e7cbc8 (ledger line of 3a241b6d).
Checked against the branch: the registration is the first commit and alone; amendment 1 was committed before the test
file; no solver source and no existing test was changed; the acceptance test is an ignored known miss with its numbers
and its limits intact, as the repository holds known misses.

Status: **partial**. By the agent's registered rule the work stopped on the band, because the measurement's stated
uncertainty and the air density are in no source that could be read; the run, the test that holds the state and the
ranked gap are delivered all the same, and the roadmap shows that as partial with the gap saying what is missing. The
agent did not go around the automated-access check of the thesis's site, which is right.

The proposal file was applied without its four follow-ups. Whether they are entered, and in what order, is a question of
scope and priority, and questions 1 to 4 above are put to the maintainer with the agent's recommendations. Question 1
needs a person: the thesis has to be opened in a browser and the uncertainty and density read from it.

Read with the result, for the next reader: that 63 per cent of the missing pressure rise is the outlet dynamic pressure
of a rotor-only passage is the agent's inference from the numbers and from ADR 0086's extrapolation; no run of this work
shows it. The comparison with the measured profiles behind the rotor is the run that would.

## Answers of the maintainer (2026-11-22)

The lead put the open questions of ADR 0760, 0761 and 0762 to the maintainer on 2026-11-22 as seven questions, each with the
agent's recommendation, which the lead shares, and asked whether to take all seven. The maintainer answered:
"Yes, all seven".

For this ADR:

1. Option (a): a person opens the thesis of the measurement and the SAE paper in a browser and records the stated
   uncertainty, with the page. Roadmap item F-877. It is not issued to an agent: the site of the thesis refuses automated
   access, and the reading waits for the maintainer or a person he names.
2. Option (a): the air density is read with it (F-877).
3. The four follow-ups are entered in the recommended order: F-877 (the reading), F-878 (the measured profiles behind
   the rotor against the solve), F-879 (check H of ADR 0183), F-880 (the outlet as on the rig). F-878 and F-879 are
   issued, as ADR 0765 and ADR 0766. F-880 is entered and not issued: it is issued only if F-878 shows that the missing
   pressure rise is the installation's.
4. Neither the compared quantity nor the convergence rule of F-870 is decided now; both are put again when F-878 and
   F-879 have reported.

## Question 4 answered (2026-11-22)

Question 4 above, the compared quantity and the convergence rule of F-870, was put again after F-878 and F-879 reported,
as their questions. The maintainer's answers of 2026-11-22 are recorded in ADR 0765 (question 1: the rotor quantities
the record measures are compared beside the pressure rise) and ADR 0766 (question 2: the convergence rule waits on
F-885).

## Sources read (2026-11-25)

F-886; the registration, the points and the limits are in
[ADR 0796](0796-f886-registration-three-formulas-read-against-their-open-sources.md). Nothing above is changed; the band of F-870 and
its result are as they were.

| citation | status | where | finding |
|---|---|---|---|
| Celik et al. (2008), the three-mesh form: the order from three meshes, the extrapolated value and the fine-grid index with the factor 1.25 | read | I. B. Celik, U. Ghia, P. J. Roache and C. J. Freitas, "Procedure for Estimation and Reporting of Uncertainty Due to Discretization in CFD Applications", in the file "Journal of Fluids Engineering Editorial Policy", 15 pages, https://www.asme.org/wwwasmeorg/media/resourcefiles/shop/journals/jfenumaccuracy.pdf (SHA-256 3e22d96c..., the same bytes as the copy of qa12.asme.org): Eq. 1 (p. 3 of the procedure), Eq. 3 (p. 4), Eqs. 4 to 7 (p. 5), Table 1 (p. 7), read from the page images. It is the publisher's policy copy; the journal's page (J. Fluids Eng. 130, 078001, with Coleman and Raad as further authors) was not opened | agrees, with the cap of the order, which is this repository's |
| Roache, the factor 1.25 | read, of a second source | the same, Eq. 7 (Roache is one of its authors); Roache's own papers were not read | 1.25 agrees |

Terms of use of the copy: it states none (searched: copyright, licence, permission, rights), so no copy is kept and no text is
reproduced; it does not forbid being cited and opens without a sign-in, a payment or a check.

Term by term (`crates/rcfd-api/tests/f870_fan01_design_point.rs`, `FS` and `mesh_convergence_estimate`):

| Source | Code | Agrees |
|---|---|---|
| Eq. 1: `h = (sum of volumes/N)^(1/3)`; step 3: `r21 = h2/h1` | `r21 = (N1/N2)^(1/3)` | agrees, with equal total volume of the three meshes, which the cell counts assume |
| Step 2: `r` "desirable" above 1.3, from experience | 1.40981 and 1.40436 | inside |
| Eq. 3a to 3c: `p = abs(ln abs(e32/e21) + q(p))/ln r21`, `q = ln((r21^p - s)/(r32^p - s))`, `s = sign(e32/e21)` | the same, fixed point | yes; seven triplets, one root each, worst 1.6e-14 (G1, limit 1e-8). The source starts from the first term, the code from 1 and stops after 500 passes without a flag: no difference at these triplets |
| Eq. 4: `phi_ext = (r21^p phi_1 - phi_2)/(r21^p - 1)` | not in this function (the error it implies is the half-width without the factor and the cap) | not used by the band |
| Eqs. 5 and 7: `GCI = 1.25 abs((phi_1 - phi_2)/phi_1)/(r21^p - 1)`, relative | `FS abs(e21)/(r21^min(p, 1) - 1)`, `FS = 1.25`: `abs(phi_1)` times it | yes for `p <= 1`, worst 1.8e-14 (G2, limit 1e-8); **no for `p > 1`**: the cap, below |
| the factor 1.25 for three meshes, at any `p` | the same here; `fan01_gci_w37c.rs` takes 3 for `p` outside [0.9, 2.2] | agrees; the rule of ADR 0086 is not in the source |
| before any estimate, residuals down by at least three orders of magnitude (p. 2 of the procedure) | not a term of the function | not compared; ADR 0110 and check A4 above record what the runs did |

**The finding.** The cap `min(p, 1)` is in no equation of the source. At the recorded triplets the registered half-width is
wider than Eq. 7 times `abs(phi_1)`: torque 0.21190 against 0.14156 N m (factor 1.4969, `p` 1.3928), pressure rise 29.755 against
14.935 Pa (factor 1.9923, `p` 1.7379); the capped value is `1.25 abs(e21)/(r21 - 1)` to 5.1e-16 (G3). The registration above says that
the order is capped, so the reading changes no statement of it except that the cap is not Celik's. The worked columns
of Table 1 come out to the printed digits: p 1.53, 0.75 and 1.51 (code 1.5340, 0.7519, 1.5077), phi_ext 6.1685, 10.8801 and 6.0269,
GCI 2.2, 1.1 and 0.5 % (G4, G4b). Recorded and not repaired.
