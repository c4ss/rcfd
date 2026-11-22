# ADR 0766: F-879 registration: FAN-01, check H of ADR 0183, the hub trailing-edge pocket in time

Status: **REGISTERED 2026-11-22, written before any code of F-879 and before any run of its checks.** Before this text no
test, harness or solver run of F-879 was written or made. What was done: reading (below), one reading script over
`docs/roadmap.json`, and the arithmetic of this page. Results go in addenda at the end; nothing above them is changed
after a run.

It answers the brief [`docs/adr/briefs/f879-fan01-check-h.md`](briefs/f879-fan01-check-h.md) and follows
[`docs/process/agent-rules.md`](../process/agent-rules.md), version 5. Branch `f879-fan01-check-h` from main 2bab1863;
scratch `w49i-work/f879-fan01-check-h-scratch`. No solver source is changed by this work: it adds one test file, record
files and text.

Size (agent): done; 0; 2; 2; 420000

The expected outcome is a class, with the outcome "does not decide" a real possibility: the registered run starts from a
uniform state and gives the start-up three flow-through times only (ADR 0183 D5), and the rule returns NOT EVALUABLE for a
window that still drifts.

## What is asked

Roadmap item F-879. Its acceptance: "Every limit not stated here is registered in the ADR before the run. The check H of
ADR 0183 is run as registered there, a time-accurate k-omega SST run of the FAN-01 passage from the stalled steady state,
and classes the reversed-flow pocket at the hub trailing edge as steady, physical unsteadiness or numerical by the rule
registered before the run, with the time step, the time run and the measured amplitude and frequency of torque and
pressure rise; the report states what the class means for the convergence rule of F-870. A run that does not decide is
recorded as not deciding."

The one sentence this work must be able to write (the brief): the pocket is steady, physically unsteady or numerical, by
the rule registered before the run, or the run does not decide.

## What was read

Whole: the brief; `AGENTS.md`; `docs/process/agent-rules.md` (version 5); [ADR 0183](0183-exp-rot-registration.md) (all
of it: the registration, amendments P-AB and P-C with their addenda, the decision record of 2026-11-06);
[ADR 0762](0762-f870-registration-fan01-design-point-state-band-and-ranked-gap.md) (all of it, with the lead's reading
and the maintainer's answers); `crates/rcfd-api/tests/f870_fan01_design_point.rs`; `docs/process/tolerances.md`.

In part: [ADR 0199](0199-f286-registration-transient-k-omega-sst-with-pimple.md) (the rows S6 and V3H, "Not in scope",
questions 4 and 5, the results of S6 and V3H, the paragraph "Check H of ADR 0183 is now runnable as registered", the
answers of 2026-11-18); `crates/rcfd-api/tests/f286_transient_sst.rs` lines 1 to 66, 155 to 182 and 455 to 640 (the
case text of H(dt), `h_text`, and the smoke); [ADR 0181](0181-job49r-a-mesh-change-at-the-fan01-hub-trailing-edge-site.md)
lines 55, 72 to 90 and the lines that carry the B2 references and question 1;
[ADR 0205](0205-f295-registration-one-diffusivity-per-periodic-seam-pair.md) lines 284 to 300 and 388 to 400 (question 2
and the answers); `crates/rcfd-fv/src/pimple.rs` lines 150 to 250, 1162 to 1202, 1775 to 1792, 2484 to 2600 (the outer
loop, its criterion and `OuterRecord`); `crates/rcfd-api/src/sim.rs` lines 4340 to 4470 (the `pimple` step and its
hook); `scripts/process_report.py` lines 415 to 522. Of `docs/roadmap.json`, by script: F-879, F-286, F-870 and F-878 in
full, and id, status, title and the head of the note of F-295, F-296 and F-302.

Not read: ADR 0175 to 0180 and 0086 beyond what ADR 0762 and ADR 0181 quote of them; `docs/milestone-fan.md`; the tests
`fan01_*.rs` other than a search of `fan01_hub_mesh_w49r.rs` for the site cells; ADR 0458 and ADR 0617 beyond the doc
comments of the settings they added, which are in `pimple.rs`.

No published source outside the repository is cited by this registration. The measures below are defined from first
principles (a half range, a count of zero crossings, a discrete Fourier sum) and the limits are those of ADR 0183.

## Check H as ADR 0183 registered it (restated)

From ADR 0183, D5 and the section "Check H (Q-POCKET; report only)".

**Set-up.** B2 only, the unchanged mesh of ADR 0181 (C-grid `camber_seam_h`, 13 627 cells). The bladed case text of ADR
0086 (k-omega SST, `rotating_frame` with the centrifugal part, Barth-Jespersen-limited `linear_upwind`, the walls and
inlet of ADR 0086) with `algorithm = "pimple"`, `n_outer = 5`, `time_scheme = "bdf2"`. This text is held on main by
check S6 of ADR 0199, the function `h_text` of `crates/rcfd-api/tests/f286_transient_sst.rs`; F-286 is done, and S6 and
the 200-step smoke V3H (finite, no guard trip, 1.315 s per step) are in ADR 0199.

**Start state.** The ADR 0086 initial state: uniform relative inflow, not the stalled SIMPLEC iterate. ADR 0183 gives
three reasons: the iterate is a cycle and not a solution; a hand-over across algorithms is untested code; a physical
oscillation does not depend on the start.

**Runs.** Two: H(dt) with `dt = T_c / 100 = 4.605e-5 s` (`T_c` = hub chord 0.099 m over the hub relative speed 21.50
m/s = 4.605 ms) and H(dt/2) with 2.3025e-5 s. Each runs 0.2532 s, six flow-through times of 42.20 ms, which is 5 498 and
10 996 steps and 6.27 rotor revolutions of 40.377 ms. The first 0.1266 s is start-up, the last 0.1266 s the window (3.14
revolutions): steps 2 750 to 5 498 of H(dt) and 5 499 to 10 996 of H(dt/2), the steps whose time is above 0.1266 s.

**Signal.** The relative speed `abs(w)` of the six B2 site cells c7, c57, c107, c157, c621 and c1235, every step of the
window. Amplitude `A`: the largest over the six cells of `(max - min) / 2`. Frequency `f = (n - 1) / (t_n - t_1)` from
the `n` upward zero crossings of the mean-removed signal of the cell with the largest `A`. A DFT peak is reported beside
it (bin width 7.9 Hz).

**Outer-iteration rule.** In at least 99 % of the window's steps the velocity change of the 5th outer iteration is at
most 1e-2 of that of the 1st. If not, the run is NOT TIME-ACCURATE.

**Class** (exactly one, tested in this order):

1. NOT EVALUABLE: a budget hit, a divergence, or a NOT TIME-ACCURATE run, in H(dt) or H(dt/2).
2. STEADY: `A <= 0.01 Omega r_hub` (0.193 m/s) in H(dt) and in H(dt/2).
3. PHYSICAL: `A` above that in both runs; at least five periods in the window (`n >= 6`) in both; at least 20 steps per
   period in H(dt); `f(dt/2) / f(dt)` in [0.9, 1.1]; `A(dt/2) / A(dt)` in [0.75, 1.33].
4. NUMERICAL: `A` above the threshold, and either the period in steps agrees within 10 % between H(dt) and H(dt/2), or
   the period is at most 4 steps.
5. NOT EVALUABLE: anything else.

**Reported beside the class, without a limit:** the time means of torque and of the pressure rise over the window
against the B2 cycle means of ADR 0181 (2.0308 N m, 63.860 Pa); the seam-pair flux sum; the five cells of largest
amplitude.

## What ADR 0183 leaves open, and where this registration departs from it (fixed now)

ADR 0183 is not edited. Each point below is registered here, before the run, with its reason.

**D-1. The eddy viscosity on the rotational seams: `pair`.** Both case texts get
`[turbulence.periodic_seam]` `diffusivity = "pair"`. Reason: ADR 0199 found that with the default (`owner`) the
diffusion of k and omega and the momentum face viscosity are not conservative across a periodic seam where the eddy
viscosity differs; F-295 (ADR 0205) built the opt-in remedy, and question 2 of ADR 0205 ("an amendment of ADR 0183 by
the EXP-ROT job, before H runs, that sets `diffusivity = "pair"` in both texts", its option (b), the recommendation) was
closed on its recommendation under the maintainer's decision of 2026-11-18; the roadmap note of F-296 records it. This
registration is that amendment in substance. The 200-step smoke of ADR 0199 ran the text without the key, so the changed
text gets its own short run before the registered runs (check S1).

**D-2. The outer-iteration rule is read on the pressure, from a record that exists.** `StepInfo` carries no change of an
outer iteration (ADR 0199, question 4, which recommended that the job of check H add a read-only record to the transient
solver). The brief forbids a change of solver source. What main has: `PimpleSettings::outer_pressure_tolerance` (F-536,
ADR 0458) makes the solver record, per step, `c = max abs(p_k - p_(k-1)) / max abs(p_1 - p^n)`, the pressure change of
an outer iteration over that of the step's first, in `PimpleSolver::outer_record`. Read in `pimple.rs`: with a negative
tolerance the criterion is never met (`final_next = outer_ratio <= tol`), every step runs exactly `n_outer` iterations
with the last one the final one, as without the setting, and the only additions are a copy of the pressure and the
record. The ratio is formed after every iteration but the last, so with `n_outer = 5` the recorded ratio is that of the
**4th** outer iteration. The rule of this registration:

> a step meets the rule iff `c_4 <= 1e-2`, or `max abs(p_4 - p_3) <= 0.0415 m^2/s^2` (kinematic pressure); the run is
> TIME-ACCURATE iff at least 99 % of the window's steps meet it.

The test sets `outer_pressure_tolerance = Some(-1.0)` on the built solver (`Simulation::pimple_solver_mut`), no case key.
Reasons. (i) Pressure for velocity: the corrector of an outer iteration changes the velocity by the gradient of the
pressure change times `rAU`, so both changes contract at the rate of the outer iteration; it is the quantity the
repository's own outer criterion uses. (ii) The 4th for the 5th: where the outer iteration contracts, the change of the
4th is no smaller than that of the 5th, so the rule is no weaker than the registered one there; where it does not
contract, the rule fails, as it should. (iii) The absolute floor: in a state that has become steady the denominator
`max abs(p_1 - p^n)` falls to the level of the linear-solver tolerance and the ratio loses its meaning, which would turn
the class STEADY into NOT EVALUABLE by construction. The floor is 1e-2 of the pressure scale of the STEADY threshold
itself, `w_hub x 0.01 Omega r_hub = 21.50 m/s x 0.193 m/s = 4.15 m^2/s^2`: an outer change below it cannot move a site
speed by more than about one hundredth of the threshold that separates the classes. The share of steps that meet the
ratio alone is reported beside the share that meet the rule, so the reader sees whether the floor decided. That the
switch leaves the solution bit for bit unchanged is check C2. This is a departure from the registered rule in the
quantity and in the iteration; it is the one departure that touches the validity test of the class, and the report says
so with the verdict.

**D-3. No wall-clock cap.** ADR 0183 fixes caps at three times the projected wall time of the smoke. The brief says to
register no limit on wall-clock time, because other heavy solves share the machine. So "a budget hit" cannot occur; a
divergence (a failure of the simulation, a guard trip, a non-finite field) stays a stop and gives NOT EVALUABLE. The two
runs are made once, as two processes started together (ADR 0183 D8 allows two registered runs at a time).

**D-4. Reports in the case text.** The text of `h_text` has no reports. Six are added, as in
`f870_fan01_design_point.rs`: the moments of blade and hub about z, the area averages of p on inlet and outlet, the mass
flows of inlet and outlet. Reports read the solution and do not enter it.

**D-5. What ADR 0183 leaves open in the measures.** (a) The time of an upward zero crossing is taken by linear
interpolation between the two steps around it. (b) "The period in steps" is `1 / (f dt)` of the run; it is defined only
with `n >= 2`. (c) "The period is at most 4 steps" is read as: in H(dt) or in H(dt/2). (d) The speed of a site cell is
the norm of the cell value of the relative velocity (`ux`, `uy`, `uz` of the rotating-frame run). (e) The threshold is
the printed 0.193 m/s. (f) The DFT peak is the largest modulus of the discrete Fourier sum of the mean-removed signal
over the window, zero frequency left out, at the multiples of 1 / (window length).

**D-6. The records.** ADR 0183 D8 names JSON files under `docs/investigations/exp-rot/`. The records of this work are
`docs/investigations/f879-fan01-check-h/H_dt.csv` and `H_dt2.csv`, one row per step of the whole run (step, time, the six
site speeds, the smallest axial relative velocity of the six cells, torque, pressure rise, the outer ratio and the first
pressure change of the step, the momentum residual, continuity), with the logs beside them.

**D-7. The start state and the words of the acceptance.** The acceptance of F-879 says "from the stalled steady state";
ADR 0183 D5 registers the start from the uniform initial state and says why the stalled iterate is not used. The two
cannot both hold. The acceptance's first clause is "run as registered there", the brief asks for "check H as ADR 0183
registered it" with its start state, and the case tree has no hand-over of a SIMPLEC state to `pimple` with SST. The run
starts from the uniform state, as registered; the difference in wording is put to the lead in the addendum and is not
decided here.

**What is not departed from:** the mesh, the model, the frame, the scheme and its limiter, the algorithm, `n_outer`, the
time scheme, both time steps, the run length, the window, the signal, the amplitude, the frequency, the threshold, the
order of the classes and every number in them. Stop rule 4 of ADR 0183 holds: after the two runs nothing more is run in
this part, no further arm, scheme, parameter or mesh.

## The three classes as numbers, and the outcome that does not decide (task 2; fixed now)

The class is carried by the signal ADR 0183 registered, the speed at the six site cells; torque and pressure rise are
measured with the same definitions and reported (below). The verdict of F-879 is one of four:

| Verdict of F-879 | Condition | What separates it, as a number |
| --- | --- | --- |
| **steady** | class STEADY of ADR 0183 | `A <= 0.193 m/s` in H(dt) and in H(dt/2), both TIME-ACCURATE. Reported as the decay: `A` of the max-`A` cell, of torque and of pressure rise in each of the six flow-through times of the run |
| **physically unsteady** | class PHYSICAL of ADR 0183, **and** persistence | `A > 0.193 m/s` in both; `n >= 6`; at least 20 steps per period in H(dt); `f(dt/2) / f(dt)` in [0.9, 1.1]; `A(dt/2) / A(dt)` in [0.75, 1.33]; and in both runs `A_2 / A_1 >= 0.75`, where `A_1` and `A_2` are the amplitudes of the max-`A` cell over the first and the second half of the window |
| **numerical** | class NUMERICAL of ADR 0183 | `A > 0.193 m/s`, not PHYSICAL, and the period in steps agrees within 10 % between the two runs (the frequency follows `1 / dt`, a dependence on the time step of a factor near 2 against the bound of 10 %), or the period is at most 4 steps |
| **does not decide** | class NOT EVALUABLE of ADR 0183 (first or last), or PHYSICAL without persistence | a divergence; a run that is not TIME-ACCURATE (D-2); `A` above the threshold in one run and not the other; fewer than six crossings; ratios outside every band; or an oscillation that passes PHYSICAL while its amplitude falls by more than a quarter across the window |

**Persistence** is the one condition added to ADR 0183, and it only narrows PHYSICAL. Reason: the brief asks that a
physical oscillation "persist", and the window begins after three flow-through times from an impulsive start, so a
decaying start-up oscillation could meet every clause of PHYSICAL. Its bound is the lower bound ADR 0183 registered for
an amplitude ratio (0.75), applied to the two halves of the window. The class by ADR 0183's rule alone is reported
beside the verdict.

**The scheme and the mesh.** The brief names, for a numerical cause, "a dependence on the time step, the scheme or the
mesh beyond a registered bound". Check H as registered tests the time step only: one mesh, one scheme, and stop rule 4
forbids a further arm. So "numerical" by this rule means a time-step-tied oscillation, and a verdict "physically
unsteady" is stable under a halved time step and not shown independent of scheme or mesh in time. For the steady
iteration ADR 0180 and ADR 0181 changed the limiter family and the mesh at the site and the cycle stayed; that is not
evidence in time, and the addendum will not present it as such.

**Torque and pressure rise** (the acceptance asks for their amplitude and frequency): for each run, over the window, the
mean, `A = (max - min) / 2`, `f` by the same crossing count and the DFT peak, of the shaft torque
`-9 (m_blade + m_hub)` and of the total-to-static pressure rise `rho (p_out - (p_in + u_in^2 / 2))` with `rho = 1.2`
(the definitions of `f870_fan01_design_point.rs`); the ratios of `A` and `f` between the two runs. The means are set
beside the measured 126.46 Pa and 333.30 W (2.14184 N m), beside the B2 values of ADR 0762 at iteration 8000 (2.028692
N m, 64.2329 Pa; B1: 2.098164 N m, 73.9881 Pa) and beside the B2 cycle means of ADR 0181. They are reported, with no
limit, and do not enter the class.

Also reported, no limit: whether the pocket is reversed in time (the smallest axial relative velocity of the six cells
over the window); the seam-pair flux sum every 50 steps; the five cells of largest `A`; the mass balance; wall seconds
per step.

## The tests (task 5; fixed now)

One new file, `crates/rcfd-api/tests/f879_fan01_check_h.rs` (header: ADR 0766, F-879). One function `run_h(dt, steps,
record)` builds the case and steps it; one pure function computes the measures and the class from two step series. Every
solving test calls the first, every classing test the second.

* Run by the gate (not ignored):
  * `f879_measures_and_classes_on_known_signals`: sampled sines of known amplitude and frequency give `A` and `f`; built
    pairs of series give each of the four verdicts, among them a pair whose frequency doubles with the halved step
    (numerical), a pair that decays (does not decide) and a pair that fails the outer rule (does not decide).
  * `f879_site_cells_are_at_the_hub_trailing_edge`: the B2 mesh has 13 627 cells and the six site cells lie where ADR
    0181 puts them.
  * `f879_short_run_holds_the_state`: 20 steps of H(dt) on the path of the heavy record, twice: with and without the
    outer record (C2), and against the values of its first run (S1).
  * `f879_class_from_the_committed_records`: reads the two committed record files, applies the rule and asserts the
    recorded verdict and its numbers. This is the test that holds the result in the gate.
* Heavy records (`#[ignore]`, run once by me, one process each): `f879_h_dt_record` and `f879_h_dt2_record`: the full
  run, written as a record file to the folder named by `F879_OUT`; where the committed record of the run exists, the
  measures of the new run are compared with it (R1); in the first run there is none and the test only completes.

The values of S1 and the verdict of G1 are those of the first run of each and are written into the file after that run;
the tolerance and the form are fixed here.

## Table of checks

Every clause of the acceptance first. Where the check is the measurement itself there is no number expected beforehand
and no margin.

| Id | Check | Limit | Source of the limit | Estimate | Margin | Kind |
| --- | --- | --- | --- | --- | --- | --- |
| A1 | check H is run as registered, with D-1 to D-7: H(dt) completes 5 498 steps and H(dt/2) 10 996, no failure, no guard trip, every field finite | both complete | acceptance of F-879 ("run as registered there"); ADR 0183 D5 | completes, by the 200-step smoke of ADR 0199 | n/a | solution verification |
| A2 | each run is TIME-ACCURATE by D-2 | at least 99 % of the window's steps | ADR 0183, check H, outer-iteration rule (99 %, 1e-2), as departed from in D-2 | not known before the run | n/a | solution verification |
| A3 | the class by ADR 0183's rule, and the verdict of F-879 by the table above | one of the four, by the registered numbers: 0.193 m/s; `n >= 6`; 20 steps per period; [0.9, 1.1]; [0.75, 1.33]; 10 %; 4 steps; persistence 0.75 | acceptance of F-879 ("by the rule registered before the run"); ADR 0183, check H, class; persistence from ADR 0183's amplitude bound | ADR 0183 PR5 predicted STEADY; I add no prediction | n/a | solution verification |
| A4 | the time step, the time run in revolutions, amplitude and frequency of torque and pressure rise, their means beside the measured and the steady values | reported | acceptance of F-879 | as stated | n/a | solution verification |
| A5 | the report says what the class means for the convergence rule of F-870 | stated in the addendum | acceptance of F-879 | as stated | n/a | solution verification |
| A6 | a run that does not decide is recorded as not deciding: the verdict "does not decide" is an outcome of the pure function and of `f879_class_from_the_committed_records` | as stated | acceptance of F-879 (last sentence) | as stated | n/a | solution verification |
| C1 | the six site cells on the B2 mesh of current main: radius of the cell centre and its z | 13 627 cells; r in [0.12425, 0.12565] m; z in [0.1716, 0.2432] m (behind the trailing edge by at most one axial hub chord, 0.1000 to 0.1716) | ADR 0181, "Where the site is" (r 0.1243 to 0.1256 to its printed digits; z 0.17160; first wake column) | inside | n/a | unchanged default output |
| C2 | the outer record leaves the solution unchanged: after 20 steps of H(dt), the six site speeds, torque and pressure rise with and without `outer_pressure_tolerance = Some(-1.0)` | equal bits | `docs/process/tolerances.md`, "Bit-identity of default output" | equal | n/a | unchanged default output |
| C3 | measures on known signals: `A` and `f` of a sampled sine (200 samples a period, 10.5 periods) | relative error `<= 1e-3` each | `docs/process/tolerances.md`, "Agreement with a closed form" (exact for the continuum only) | 1.2e-4 for `A` (`1 - cos(pi / 200)`), smaller for `f` | 8 | code verification |
| C4 | the four verdicts on built pairs of series, one pair per row of the table of verdicts and one per clause of "does not decide" named in the test | each verdict as built | this registration, the table of verdicts (the rule of ADR 0183) | as built | n/a | code verification |
| S1 | `f879_short_run_holds_the_state`: after 20 steps of H(dt) the time, the largest site speed, torque and pressure rise against the first run; 20 outer records; every field finite | time to 1e-12 relative; 1 % each | ADR 0762, "The tolerance of the state" (1 % for a held state of this passage); the solver is deterministic | 0 | n/a | unchanged default output |
| G1 | `f879_class_from_the_committed_records`: step counts 5 498 and 10 996, window steps 2 749 and 5 498, the verdict and the class as recorded, `A` and `f` of each run to the recorded digits | counts exact; verdict equal; 1e-6 relative on `A`, `f` and the means (the same file read twice) | `docs/process/tolerances.md`, "Same discrete problem by two routes" (`<= 1e-8` where only rounding separates; the record is printed to 9 digits, so 1e-6) | 0 | n/a | unchanged default output |
| R1 | a rerun of a heavy record against the committed record of that run: `A`, `f` and the means of the site signal, torque and pressure rise; the per-run conditions of the class | 1 % each; conditions equal | ADR 0762, "The tolerance of the state" | 0; not rerun by me (each registered run is made once) | n/a | unchanged default output |
| D1 | defaults: no file under `crates/*/src` is changed | 0 files | rule 3 | 0 | n/a | unchanged default output |
| P | `cargo fmt -p rcfd-api --check`; `cargo clippy -p rcfd-api --all-targets -- -D warnings`; `cargo test -p rcfd-cli --release --test docs_site`; `python scripts/roadmap_proposals.py check`; `python scripts/process_report.py --check` | clean; passes; no failure but a generated file not current | rule 7 | as stated | n/a | code verification |

No row is of kind validation: F-879 claims no agreement with a measured case. The measured 126.46 Pa and 333.30 W
(`docs/validation-data.md`, as ADR 0762 traces them) stand beside the time means as a comparison without a limit.

## Negative controls and planned mutations

Run on a scratch copy of the tree (`git archive` of the branch into the scratch folder, its own target folder), each
against the gate tests; the heavy records are not rerun for a mutation.

| Id | Mutation | Must be caught by |
| --- | --- | --- |
| M1 | solver: the Coriolis factor of `RotatingFrame::accel` (`crates/rcfd-fv/src/sources.rs`) from -2.0 to -1.9 | `f879_short_run_holds_the_state` |
| M2 | test set-up: the time step from 4.605e-5 to 4.7e-5 s | `f879_short_run_holds_the_state` (the time) |
| M3 | test: the STEADY threshold from 0.193 to 0.5 m/s | `f879_measures_and_classes_on_known_signals` |
| M4 | test: site cell 1235 replaced by 13000 | `f879_site_cells_are_at_the_hub_trailing_edge` |
| M5 | test: the frequency band [0.9, 1.1] widened to [0.4, 2.5] | `f879_measures_and_classes_on_known_signals` (the pair whose frequency doubles is then called physical) |
| M6 | record: the committed `H_dt.csv` cut to its first half | `f879_class_from_the_committed_records` |
| M7 | test: the persistence condition removed | `f879_measures_and_classes_on_known_signals` (the decaying pair) |
| K1 | negative control: the short run made twice | equal to the last printed digit |

The committed patch (`crates/rcfd-api/tests/f879_fan01_check_h.mutation.patch`) is M1, with the first line
`# expect-fail: cargo test -p rcfd-api --release --test f879_fan01_check_h f879_short_run_holds_the_state`. If the
20-step run does not catch M1 at 1 %, that is recorded as a mutation not caught, and the committed patch is M3 with its
own command.

## Decision rule

* **Done** only if the two runs are made and the rule gives the verdict steady, physically unsteady or numerical, with
  the halved time step.
* **Partial** if the two runs are made and the verdict is "does not decide", with the reason by the rule and what it
  would take.
* **Stopped and reported** if check H cannot be run as registered here on current main: a case text that is refused, a
  short run (S1) that fails, a divergence before the window, or a defect of the solver.

A defective set-up found before the registered runs is corrected by an amendment committed before the corrected run. A
class is not asserted without the registered evidence: the departures D-1 to D-7, above all D-2, are named with the
verdict.

## What the addendum will say about F-870 (task 4)

Fixed now, so that the result does not choose it: for each verdict, whether a steady state exists for the steady solve
of F-870 to converge to, and, if none does, what a rule on a time mean would have to state (the window in revolutions or
flow-through times, the rule by which the mean counts as settled, the time step) to be registered. That is a proposal to
the maintainer and no change is made here. The limits of what one mesh (B2, not the B1 of F-870) and one scheme can say
are stated with it.

## What is not covered

* The scheme and the mesh in time (above); B1, the mesh of F-870; any run from the stalled SIMPLEC state (D-7).
* The velocity change of the outer iterations, as ADR 0183 registered the rule (D-2).
* Arm 1 and arm 2 of ADR 0183, and its decision mapping D9: this work runs check H alone and maps nothing.
* A longer run, a third time step, another start: stop rule 4 of ADR 0183.
* Any agreement with the measurement: no band is registered and none is claimed.
* The existing tests are not edited, un-ignored or removed; `f870_fan01_design_point.rs` and `f286_transient_sst.rs` are
  read only.

## Open questions for the maintainer

To be completed in the addendum with options and a recommendation; not decided by this work: the convergence rule of
F-870 (a steady state, or a rule on a time mean).
