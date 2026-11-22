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
