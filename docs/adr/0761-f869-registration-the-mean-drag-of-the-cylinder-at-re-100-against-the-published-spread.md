# ADR 0761: F-869 registration: the mean drag of the cylinder at Re 100 against the published spread

Status: **REGISTERED 2026-11-22, written before any code of F-869 and before any run of its checks.**
Before this text no solver was run and no test of the cylinder was run by this work. What was done: reading, the fetch
and reading of two published papers, the arithmetic below, and one build of the test binary `docs_site` (to check the
links of this file). Results go in addenda at the end; nothing above them is changed after a run.

It answers the brief `docs/adr/briefs/f869-cylinder-re100-mean-drag.md` and follows `docs/process/agent-rules.md`,
version 5. Branch `f869-cylinder-re100-mean-drag` from main 13b0937d, scratch
`w49i-work/f869-cylinder-re100-mean-drag-scratch`. The records of the case are
[ADR 0060](0060-wave30b-incompressible-validation.md) (V5, the checks K1 to K4),
[ADR 0062](0062-wave31a-solver-defects.md) (item 3 and addendum 4, the separation of the time and mesh errors) and
[ADR 0069](0069-wave33b-transient-accuracy-coupling.md) (the one run with BDF2).

## What is asked

The acceptance of F-869 (`docs/roadmap.json`): "Every limit not stated here is registered in the ADR before the run. The
mean drag coefficient of the laminar flow past a circular cylinder at Re 100 is within the band of the published values
recorded in docs/reference-data.md ("Cylinder wake"), by the checks K1 to K4 of ADR 0060 or their registered successors,
on current main, in tests that the full gate runs; what the band is taken from and its width are stated."

The brief adds: what exists is read against the acceptance; the band is registered before any run; the run on current
main reports the Strouhal number and the lift amplitude beside the drag; the comparison becomes a test the gate runs, or
a heavy record with a fast test on the same path; a negative control or mutation fails for a drag that is wrong by more
than the band; no solver source changes.

## What was read

- `AGENTS.md`; `docs/process/agent-rules.md` (version 5) whole; the brief whole; ADR 0060 whole;
  `crates/rcfd-api/tests/validation_cylinder_re100.rs` whole; `validation_cylinder_re100_w31.rs` whole;
  `validation_cylinder_re100_w33b.rs` whole; the header (lines 1 to 60) of `w48t_cylinder_spokes.rs`;
  `crates/rcfd-api/tests/validation_common/mod.rs` lines 1 to 120.
- ADR 0062: item 3 (lines 103 to 139) and addendum 4 (lines 274 to 360). ADR 0069: the registration of the cylinder run
  (lines 176 to 194) and its result (lines 395 to 428). [ADR 0156](0156-job48v-adaptive-pressure-correctors.md): the
  table of results and the resolution sweep (lines 440 to 470). The rest of these three was not read.
- `docs/reference-data.md`: the section "Cylinder wake" (lines 200 to 207), the rows of wave 30B and wave 31C on the
  cylinder (lines 1271 and 1288), discrepancy 5 and the lines of wave 31A (1331 to 1339);
  `docs/reference-data.d/README.txt`; `validation-data/reference/cylinder_re100_literature.csv`.
- The entries F-869 and F-029 of `docs/roadmap.json` (by script); `docs/test-process.md` section 4 (the time limit of a
  test) and the head of section 7.4; `docs/process/tolerances.md`; `scripts/process_report.py` lines 440 to 520;
  [ADR 0759](0759-f866-registration-the-control-of-l1-decides-and-asserts-on-the-ten-listings.md) and
  `docs/roadmap/proposals/0759-F-866.json` for the form; `crates/rcfd-post/src/integrals.rs` lines 86 to 135.
- Two published papers, fetched on 2026-11-22 as PDF from arxiv.org and read as raw text (`pdftotext -layout`); what
  they print is in the next section.

## The published values, as read on 2026-11-22

**Source A.** S. Parvar, K. T. Iqbal, M. N. Ardekani, L. Brandt, O. Tammisola, "Elastoviscoplastic fluid flow past a
circular cylinder", arXiv:2303.09262v1 (16 March 2023), Table 1 ("Cylinder flow characteristics (the drag coefficient
CD, the rms of lift coefficient CLrms, Strouhal number St, ...)"), block "Re = 100", Newtonian fluid. The rows as
printed, in the order of the table:

| Row of Table 1 | C_D | C_L rms | St | What the row is |
| --- | --- | --- | --- | --- |
| Williamson [2] | - | - | 0.165 | quoted by the paper (experiment) |
| Kravchenko et al. [59] | 1.314 | 0.222 | 0.164 | quoted by the paper |
| Oliveira [29] | 1.370 | - | 0.167 | quoted by the paper |
| Sivakumar et al. [58] | 1.325 | 0.229 | 0.164 | quoted by the paper |
| Posdziech and Grundmann [3] | 1.325 | 0.228 | 0.164 | quoted by the paper |
| Mossaz et al. [10] | 1.328 | - | 0.164 | quoted by the paper |
| Qu et al. [6] | 1.319 | 0.225 | 0.165 | quoted by the paper |
| Constant et al. [5] | 1.37 | - | 0.165 | quoted by the paper |
| Peng et al. [36] | 1.361 | 0.235 | 0.165 | quoted by the paper |
| (Present 32 x 16) | 1.400 | 0.254 | 0.168 | the paper's own computation, domain 32 D by 16 D |
| (Present 64 x 32) | 1.359 | 0.236 | 0.164 | the paper's own computation, domain 64 D by 32 D |

The paper's text below the table says of its own two rows that "a domain size with 32D x 16D is too small and may
negatively affect the results, as shown in Table 1", and that the domain of 64 D by 32 D is used in its work (section
on validation, the paragraph after Table 1; its domain is given as [-16 D, 48 D] x [-16 D, 16 D]).

**Source B.** S.-K. Park, G. Jo, H. J. Choe, "Virtual Interpolation Point Method for Viscous Flows in Complex
Geometries", arXiv:1401.6513v2 (4 February 2014), Table 3 ("Simulation results for flow over a circular cylinder"),
rows at Re 100: "VIP method 100 1.328 +- 0.31 0.164" (the paper's own computation) and "H. Ding [13] 100 1.325 +- 0.28
0.164" (quoted by the paper). The text above the table defines C_D as the time-averaged drag coefficient and C_L as "the
amplitude of lift-coefficient fluctuations (maximum deviation from the time-averaged value)".

What kind of source this is, said plainly: nine of the eleven drag values are quotations of other papers by A and B;
those nine originals were not read (the page said so in waves 30B and 31C, and two searches of 2026-11-22 for open
copies of Qu et al. (2013) and Posdziech and Grundmann (2007) found only the bibliographic records). Two values are
primary: each is the own computation of the paper that was read (A: 1.359 on its 64 D by 32 D domain; B: 1.328). The
nine quoted rows are the rows of `validation-data/reference/cylinder_re100_literature.csv`, which is not changed (it is
in the data pack with a checksum).

These values go on the page as the fragment `docs/reference-data.d/0761-F-869.part` (rule 4), under the heading
"Cylinder wake: the mean drag coefficient at Re 100 (F-869, ADR 0761)", in the commit after this one and before any
run. The brief gives this work the section "Cylinder wake" of `docs/reference-data.md`; the page is generated and that
section lies in `0000-base.part`, which rule 4 and the README of the folder forbid an item to edit, so the fragment is
used and the old section stays as it is. This is a deviation from the brief's words, taken by the rules.

## The band, registered

**B = [1.314, 1.370]**: the smallest and the largest of the eleven published values of the time-averaged drag
coefficient at Re 100 that the two tables print for an unbounded two-dimensional flow, without the row "(Present 32 x
16)". It is taken from the spread of the published values and from nothing else: no allowance for discretisation is
added. Its width is 0.056, its middle 1.342, its half-width 2.09 % of the middle. The row (Present 32 x 16), 1.400, is
left out because its own paper says that its domain is too small; leaving it out makes the band narrower, not wider.
The two primary values lie inside the range of the nine quoted ones, so the band is the range of the CSV, 1.314 to
1.370, as ADR 0060 recorded it.

The band is not widened after a run (rule 2). The window of K2 in ADR 0060, "within 4 % of 1.342", [1.2883, 1.3957],
is this band widened by an allowance of 2 % for the discretisation; it stays the limit of K2 and is used below for the
coarse mesh only, under its own name.

## What exists, against the acceptance

**K1 to K4** (ADR 0060, V5; `validation_cylinder_re100.rs`; PISO with two outer iterations, first order in time, linear
upwind, O-grid with the far field at 20 D, to t = 160, statistics over the last 8 whole periods) on the sequence (64 x
50, dt 0.1), (90 x 70, dt 0.07), (128 x 100, dt 0.05):

| Check | Registered in ADR 0060 | Recorded (ADR 0060 addendum) | Verdict |
| --- | --- | --- | --- |
| K1 | St within 3 % of 0.165 | 0.15878, -3.77 % | missed |
| K2 | C_D within 4 % of 1.342 | 1.3100, -2.39 % | met |
| K3 | GCI_fine of St and of C_D at most 3 % | St 4.05 %, C_D 3.67 % | missed, both |
| K4 | at least 9 zero crossings on every mesh | 14, 15, 16 | met |

The mean drag on the three meshes was 1.2514, 1.2873, 1.3100. All three tests of the file are ignored: K2 and K4 as
heavy, K1 and K3 as the known miss, the third as a diagnosis. No gate runs any of them.

**Does the known miss touch the drag?** K1 is the Strouhal number alone. K3 is two limits, and one of them is the grid
convergence index of the drag, 3.67 % against 3 %: so the known miss does touch the drag, in its convergence evidence,
and does not touch the comparison of the drag with the published values (K2, met). ADR 0062 found the cause: mesh and
time step were refined together and the step is first order in time, so the sequence is dominated by the time step. Its
corrected sequence at the common dt 0.025 (`k_corrected_sequence_common_dt_0025`, ignored, passed once) gave C_D
1.32341, GCI_fine(C_D) 0.96 % at the observed order 2.02, and St 0.16175.

**The later tests.** w31 (ADR 0062) is the matrix of meshes and time steps; its drag values with the first-order step:

| Mesh | dt 0.1 | dt 0.05 | dt 0.025 | dt 0.0125 |
| --- | --- | --- | --- | --- |
| 64 x 50 | 1.25143 | 1.27744 | 1.29283 | |
| 90 x 70 | 1.27313 | 1.29803 | 1.31325 | |
| 128 x 100 | 1.28844 | 1.30996 | 1.32341 | 1.33150 |
| 181 x 141 | | unstable | 1.33013 | |

Its extrapolations: dt to zero on 128 x 100 gives 1.3437; h to zero at dt 0.025 gives 1.3335 and 1.3433 (two
triples); the fitted model of both gives 1.357; the far field at 30 D in place of 20 D lowers C_D by 1.1 % (ADR 0060,
1.2961 against 1.3100). w33b (ADR 0069) ran 128 x 100 at dt 0.05 once with `time_scheme = Bdf2`: St 0.16480, C_D
1.35247, 16 crossings. w48t ([ADR 0154](0154-job48t-cylinder-ogrid-pressure-spokes.md), ADR 0156) is about pressure
spokes on the example case; its sweep with adaptive correctors recorded mean C_D 1.3237, 1.3488, 1.3550 on 64 x 48,
128 x 96, 256 x 256 of another set-up (grading 100, Courant-matched steps) and registered no comparison with published
drag. All tests of w31 and w33b are ignored.

**Against the band B**, from these records and before any run of mine:

| Record | C_D | In B = [1.314, 1.370] |
| --- | --- | --- |
| 128 x 100, dt 0.05, first-order step (K2's point, the default time scheme) | 1.30996 | no, 0.31 % under the lower end |
| 128 x 100, dt 0.025, first-order step | 1.32341 | yes |
| 128 x 100, dt 0.0125, first-order step | 1.33150 | yes |
| 181 x 141, dt 0.025, first-order step | 1.33013 | yes |
| 128 x 100, dt 0.05, BDF2 | 1.35247 | yes |

So the acceptance is not met today for three separate reasons: no gate runs a comparison of the drag; the registered
comparison (K2) has a window wider than the published spread; and at K2's own point the drag with the default first-order
step is outside the spread itself.

## Disclosure of prior knowledge

The limits below are not blind. Every number of the last section was known to me when the band, the mesh, the time step
and the time scheme were fixed, among them 1.35247 for the very point that V1 compares. What was not known: any result
with BDF2 on 64 x 50 or 90 x 70, at dt 0.025, or with the far field at 30 D, so G1, N1, V2, S1, S2 and R2 are blind;
and whether main of today reproduces the numbers of October (the solver file `pimple.rs` has changed since, last in
F-771 and F-757). The band is the published spread with nothing added, and the choices are derived as follows and not
searched: the mesh is the finest of the registered sequence of ADR 0060; the time scheme is the second-order one because
ADR 0062 measured the first-order step's error in the drag as 2.4 % at this point, more than the half-width of the band;
the time step is the one of ADR 0060's fine point and of ADR 0069's run.

## Design

No solver source changes. One new test file, `crates/rcfd-api/tests/f869_cylinder_re100_mean_drag.rs` (header: ADR 0761,
F-869), with the set-up of `validation_cylinder_re100_w33b.rs` copied: `cylinder_ogrid(n_theta, n_r, 0.5, r_out,
Expansion(150), 0.1)`, D = 1, U = 1, nu = 0.01, `PimpleSettings::piso` with `time_scheme = Bdf2`, linear upwind,
`n_outer = 2`, `amg_pressure`, `alpha_u = alpha_p = 0.9`, the cylinder a no-slip wall, the far field `Freestream`, the
same initial disturbance, the blow-up guard of w31. BDF2 is an option of `PimpleSettings` on main; the default stays
the first-order step and nothing of a default changes (rule 3).

**The measured quantities**, the same in every run: the run goes to t = 160; v at the probe (3, 0, 0.05) is recorded
for t > 60 and the force on the cylinder for t > 85; the window is the last 8 whole periods between upward zero
crossings of the probe signal (as K1 to K4); St = 8 / window; C_D is the trapezoid mean over the window of F_x / (0.5
U^2 D depth) from `rcfd_post::integrals::wall_force_and_moment` with the pressure of `pressure_bc` and
`boundary_values`; the pressure part alone is averaged in the same way (for N1); the lift coefficient F_y / (0.5 U^2 D
depth) gives, over the window, its amplitude (half of the largest less the smallest) and its rms about its mean. A
blow-up (the guard panics) fails the test that holds the run; there is no second run.

**Why a heavy record with a fast test, and not the whole comparison in the gate.** The comparison with the band needs
the fine mesh: on 64 x 50 the first-order records are 2.3 to 2.9 % under the fine values, more than the half-width of the
band. One run of 128 x 100 to t = 160 took 4 to 15 minutes in the records (ADR 0062, ADR 0069), above the two minutes of
rule 7, and the mesh sequence and the time-step pair multiply it. So:

- **The gate runs** (not ignored) `g1_coarse_mesh_mean_drag_within_the_k2_window_and_the_pressure_part_alone_fails`:
  one run of 64 x 50 at dt 0.05, through the same functions `setup`, `run` and `mean_drag` as the heavy record, with G1,
  G2 and N1 below; and the pure test `g3_the_band_is_the_recorded_spread_and_rejects_a_drag_outside_it`. Expected time
  of the run: about 80 s alone on this machine (ADR 0060: 448 s for 60.5 million cell-steps; this run has 10.2 million),
  more beside the two other solves; no limit on time is registered.
- **The heavy record** (ignored, run once by me) is
  `h1_fine_mesh_mean_drag_in_the_published_band_heavy_record`: the runs 64 x 50, 90 x 70 and 128 x 100 at dt 0.05, 128
  x 100 at dt 0.025, and 128 x 100 at dt 0.05 with the far field at 30 D, one after the other in one process, with V1,
  V2, S1, S2, K4h and the reported rows. Expected 25 to 65 minutes alone.

What the gate then holds, said plainly: that the drag path (set-up, solver with BDF2, force integral, averaging) gives
on the coarse mesh a mean drag inside the published spread widened by the 2 % discretisation allowance of K2, and
rejects a drag without its viscous part. The comparison with the band B itself is carried by the heavy record, which no
gate runs.

**The mesh convergence of V2 and S1.** With f1, f2, f3 the mean drag on 64 x 50, 90 x 70, 128 x 100 at the common dt
0.05 and r = sqrt 2: observed order p = ln|(f1 - f2)/(f2 - f3)| / ln r, limited to [1, 2] as in
`validation_common::gci_fine`; GCI_fine = 1.25 |f3 - f2| / (|f3| (r^p - 1)) (Roache, as ADR 0060); the Richardson value
f_h = f3 + (f3 - f2)/(r^p - 1). If the three values are not monotone, f_h counts as not obtained and V2 as missed. For
the time step, with g the value on 128 x 100 at dt 0.025 and the formal order 2 of BDF2: GCI_t = 1.25 |g - f3| / (|g|
(2^2 - 1)).

**The existing tests.** `validation_cylinder_re100.rs`, `_w31.rs` and `_w33b.rs` are not edited, not un-ignored and not
removed; `k1_k3_strouhal_and_gci_known_miss` stays the known miss it is. The tests of this registration are the
registered successors of K2, K3 (its drag half) and K4 for F-869, with BDF2 and a common time step; K1 has no successor
here (the Strouhal number is F-029's acceptance) and St is reported against K1's unchanged window [0.16005, 0.16995]
without a limit.

## Table of checks

Estimates: for V1 the record of ADR 0069; for the blind rows, the first-order matrix of ADR 0062 moved by the difference
BDF2 made at the fine point (1.35247 - 1.30996 = 0.0425). Margin: the distance from the middle of the limit's interval
to its end, divided by the distance of the estimate from that middle; for a one-sided limit, the limit divided by the
estimate.

| Id | Check | Limit | Source of the limit | Estimate | Margin | Kind |
| --- | --- | --- | --- | --- | --- | --- |
| V1 | acceptance, "the mean drag coefficient ... is within the band of the published values": mean C_D on 128 x 100, dt 0.05, BDF2 (heavy record) | in [1.314, 1.370] | arXiv:2303.09262 Table 1 (Re = 100) and arXiv:1401.6513 Table 3, the smallest and the largest of the eleven values; the acceptance of F-869 | 1.352 | 2.8 | validation |
| V2 | the same drag does not leave the band with a smaller step or a finer mesh: (a) mean C_D on 128 x 100 at dt 0.025; (b) the Richardson value f_h of the three meshes at dt 0.05 (heavy record) | each in [1.314, 1.370] | the same two tables; the decision rule of the brief ("partial if ... the result depends on a mesh") | (a) 1.345 to 1.36; (b) 1.36 to 1.372 | 1.1 for (b), which may miss; 1.6 for (a) | validation |
| S1 | K3's drag half on a common time step: GCI_fine of C_D on 64 x 50, 90 x 70, 128 x 100 at dt 0.05 (heavy record) | at most 3 % | ADR 0060, K3 | 1.0 % (ADR 0062: 0.96 % at dt 0.025) | 3.0 | solution verification |
| S2 | the time step: GCI_t of C_D from dt 0.05 and 0.025 on 128 x 100 (heavy record) | at most 3 % | ADR 0060, K3 | 0.3 % | 10 (wide) | solution verification |
| K4h | sustained shedding in every run of the heavy record | more than 9 zero crossings after t = 60 | ADR 0060, K4 | 14 to 17 | 1.5 | solution verification |
| G1 | acceptance, "in tests that the full gate runs": mean C_D on 64 x 50, dt 0.05, BDF2 (gate) | in [1.2883, 1.3957] | ADR 0060, K2 (the published spread and 2 % for the discretisation) | 1.32 | 2.4 | validation |
| G2 | sustained shedding in the gate's run | more than 9 zero crossings after t = 60 | ADR 0060, K4 | 14 | 1.5 | solution verification |
| G3 | the constants of the band in the test equal the smallest and the largest C_D of `cylinder_re100_literature.csv`; the band check rejects 1.314 - 0.0561 and 1.370 + 0.0561 and accepts both ends (gate, pure) | holds | this registration, "The band, registered" | holds | n/a | code verification |
| N1 | negative control in the gate's run: the pressure part of the drag alone, a drag that lacks the wall shear | (a) fails G1's window; (b) is under the full drag by at least 0.056, the width of the band | the brief, task 5 ("fails for a drag that is wrong by more than the band"); the row of G1 | under the full drag by 0.3 (expectation, no source read) | 5 (wide) | validation (negative control) |
| A1 | acceptance, "what the band is taken from and its width are stated": this file, "The band, registered", and the fragment of the page | present in both | the acceptance of F-869 | present | n/a | code verification (reading) |
| M1 | committed mutation: the viscous part of the patch force set to zero in `crates/rcfd-post/src/integrals.rs` | G1 fails | rule 16 of `docs/process/agent-rules.md`; the brief, task 5 | fails (C_D about 1.0) | n/a | code verification (negative control) |
| M2 | mutation: the reference area of the drag coefficient doubled in the test (`0.5 * DEPTH` to `DEPTH`) | G1 fails | the brief, task 5 | fails (C_D about 0.66) | n/a | code verification (negative control) |
| R1 | reported beside the drag, every run: St (and its place in K1's window [0.16005, 0.16995]), the amplitude and the rms of the lift coefficient (beside the published rms 0.222 to 0.236 of source A and amplitudes 0.28 and 0.31 of source B) | none (reported) | the brief, task 3 ("with the Strouhal number and the lift amplitude beside it for the record") | St 0.165, rms 0.23 | n/a | validation (reported without a limit) |
| R2 | reported: the far field at 30 D, 128 x 100, dt 0.05, mean C_D and its place in B | none (reported) | ADR 0060, bisection step (iii) | 1.1 % under V1's value | n/a | solution verification (reported without a limit) |
| R3 | reported: the existing ignored `k1_k4_...` and `k1_k3_..._known_miss` of `validation_cylinder_re100.rs`, run once on current main, unchanged, in one process | none (reported; K2 and the known miss keep their limits) | the brief, task 1 ("what the mean drag coefficient is on current main on each mesh") | 1.2514, 1.2873, 1.3100; K2 met; K1 and K3 missed as recorded | n/a | validation (reported without a limit) |
| E | `git diff --stat 13b0937d` names this ADR, the fragment, the test file, its mutation patch and the proposal file only | no other file | the brief ("Files you own"; "Change no solver source") | holds | n/a | unchanged default output |
| F | `cargo fmt -p rcfd-api --check`; `cargo clippy -p rcfd-api --all-targets -- -D warnings`; `cargo test -p rcfd-api --release --lib`; `cargo test -p rcfd-cli --release --test docs_site` | clean; clean; passes; passes | rule 7 of `docs/process/agent-rules.md` | clean | n/a | code verification |
| P | `python scripts/process_report.py --check` and `python scripts/roadmap_proposals.py check` on the last commit | no failure of this work | rules 7 and 9 of `docs/process/agent-rules.md` | none, but generated pages not current | n/a | code verification |

V2 (b) is the row most likely to miss, and that is said now: the first-order records put the mesh error of the fine
mesh at 0.010 to 0.020, and 1.35247 plus that is 1.362 to 1.372 against the upper end 1.370. The far field at 20 D
raises the drag by about 1 % against 30 D (ADR 0060), which pushes the same way; R2 is run so that a miss of V2 can be
read. Nothing is tuned to it.

## Negative control and mutations

- **N1** runs in the gate with G1, on the same run, at no further cost: the mean of the pressure part alone must fail
  the window of G1 and lie under the full drag by at least the width of the band.
- **M1** (committed, `crates/rcfd-api/tests/f869_cylinder_re100_mean_drag.mutation.patch`, first line
  `# expect-fail: cargo test --release -p rcfd-api --test f869_cylinder_re100_mean_drag g1_`): the viscous force of
  `force_and_moment` zeroed. Made with `git diff` on a scratch copy; the tree of this work never holds the change.
- **M2**: the reference area doubled in the test's `mean_drag`.

Both on a scratch copy of the tree at the last code commit, release, each run once; what failed and its message are
recorded.

## Size

Size (agent): done; 0; 2; 0; 120000

The two heavy runs are the heavy record (five solves in a row) and R3 (the existing sequence, about 7 minutes alone).

## Decision rule

- **done** only if V1, V2, S1, S2, K4h, G1, G2, G3 and N1 are met, A1 holds, M1 and M2 are caught, and E, F and P hold.
- **partial** if V1 is met and any of these holds, with what is missing stated: V2 is missed (the drag is in the band on
  the registered mesh and step, and leaves it with a smaller step or in the Richardson value: the result depends on the
  mesh); S1 or S2 is missed (the convergence evidence is a miss); the gate-side check is not in place or G1, G2, G3 or N1
  is missed; a mutation survives; a check was not run.
- **stopped and reported** if V1 is missed: the drag misses the band, with its number; or if a run blows up, or a defect
  of the solver appears. Nothing is changed in the solver.
- **todo** if no run of the heavy record gives a number.

A run that misses is not repeated and no limit is changed (rule 2); a miss is kept as an ignored `*_known_miss` test with
its limit and its number.

## What is not covered

- The nine originals behind the quoted values, and any measured drag: every published value here is a computation, so V1
  is agreement between codes, with the spread of the codes as the band; no experiment's drag was found in a source that
  could be read.
- The default first-order time step: at 128 x 100 and dt 0.05 its recorded drag, 1.30996, is outside the band. F-869 is
  shown with the option `time_scheme = Bdf2`, not with the default. R3 reports the default's numbers of today.
- The Strouhal number as a check (F-029's acceptance, and the known miss K1, which stays).
- A finer mesh than 128 x 100, a far field beyond 30 D, three-dimensional effects, another Reynolds number.
- The case-file path (`Simulation`, the example project): the test drives `PimpleSolver` directly, as K1 to K4 do.
