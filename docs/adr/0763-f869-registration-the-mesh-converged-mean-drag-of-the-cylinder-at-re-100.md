# ADR 0763: F-869 registration: the mesh-converged mean drag of the cylinder at Re 100

Status: **REGISTERED 2026-11-22, written before any code of F-869 under this registration and before any run of its
checks.** Before this text no solver was run and no test of the cylinder was run by this work. What was done: reading,
the arithmetic below (a Python script on recorded numbers and cell counts), and one build and run of the test binary
`docs_site` (to check the links of this file). Results go in addenda at the end; nothing above them is changed after a
run.

It answers the brief `docs/adr/briefs/f869-cylinder-mesh-converged-drag.md` and follows `docs/process/agent-rules.md`,
version 5. Branch `f869-cylinder-mesh-converged-drag` from main 2bab1863, scratch
`w49i-work/f869-cylinder-mesh-converged-drag-scratch`. It is a second registration under F-869 (rule 13): it adds no
acceptance clause, and it does not edit
[ADR 0761](0761-f869-registration-the-mean-drag-of-the-cylinder-at-re-100-against-the-published-spread.md), whose band,
tests and known miss V2b stay as they are.

## What is asked

The acceptance of F-869 (`docs/roadmap.json`, read by script): "Every limit not stated here is registered in the ADR
before the run. The mean drag coefficient of the laminar flow past a circular cylinder at Re 100 is within the band of
the published values recorded in docs/reference-data.md ("Cylinder wake"), by the checks K1 to K4 of ADR 0060 or their
registered successors, on current main, in tests that the full gate runs; what the band is taken from and its width are
stated."

The maintainer's answer of 2026-11-22 in ADR 0761 (question 1, option (a)): F-869 stays partial until a mesh sequence at
dt 0.025 with the far field at 30 D or more shows the fine value and the Richardson value in the same band. The brief
asks for: the band unchanged; the three meshes, one far-field distance, dt 0.025, the averaging window and how its start
is chosen; the Richardson procedure with its observed order and the condition under which the extrapolation is not
valid, and what is concluded then; the run with the mean drag, the Strouhal number and the lift amplitude on each mesh,
the observed order, the Richardson value and the GCI; a heavy ignored record of the form ADR 0761 used; a negative
control or mutation on the new check. No solver source changes.

## What was read

- `AGENTS.md`; `docs/process/agent-rules.md` (version 5) whole; the brief whole; ADR 0761 whole;
  `crates/rcfd-api/tests/f869_cylinder_re100_mean_drag.rs` whole and its mutation patch.
- `crates/rcfd-api/tests/validation_common/mod.rs` whole (`observed_order`, `gci_fine`, `Checks`);
  [`docs/process/tolerances.md`](../process/tolerances.md) whole; `docs/reference-data.d/0761-F-869.part` whole;
  `docs/roadmap/proposals/0761-F-869.json`; the entry F-869 of `docs/roadmap.json` (by script);
  `docs/adr/briefs/README.md` lines 1 to 30; `scripts/process_report.py` lines 418 to 523 and 705 to 746.
- `crates/rcfd-mesh/src/gen.rs`: `cylinder_ogrid` (lines 546 to 591) and the lines that a search for "Expansion"
  gives (15, 52, 87). [ADR 0060](0060-wave30b-incompressible-validation.md) lines 186 to 200 (the sequence V5 and the
  definition of the observed order and the GCI) and the lines that a search for "GCI" gives.
  [ADR 0069](0069-wave33b-transient-accuracy-coupling.md): only the lines that a search for "BDF2" gives among its
  first 119 (the summary, line 17, and the order checks, lines 111 to 119). Of
  `crates/rcfd-api/tests/validation_cylinder_re100_w31.rs` the lines that a search for "181" gives (3, 123, 193 to
  201). [ADR 0062](0062-wave31a-solver-defects.md) was not opened; what is said of it below is what ADR 0761 records.
- No paper was fetched and no new published value is used: the band and its sources are those of ADR 0761, recorded in
  `docs/reference-data.d/0761-F-869.part`. This registration adds no reference fragment.

## The band, unchanged

**B = [1.314, 1.370]**, the band registered in ADR 0761 ("The band, registered"): the smallest and the largest of the
eleven published time-averaged drag coefficients at Re 100 of arXiv:2303.09262v1 Table 1 (block "Re = 100") and
arXiv:1401.6513v2 Table 3, without the row "(Present 32 x 16)". Width 0.056, middle 1.342. Nothing is added to it and
it is not changed after a run (rule 2).

## Disclosure of prior knowledge

Known to me when this was written: every number of ADR 0761, among them, with BDF2 and the far field at 20 D unless
said, C_D 1.31674, 1.33879, 1.35247 on 64 x 50, 90 x 70, 128 x 100 at dt 0.05; 1.34511 on 128 x 100 at dt 0.025; 1.33884
on 128 x 100 at dt 0.05 with the far field at 30 D; the Richardson value 1.37479 at the observed order 1.38. Not known:
any result with BDF2 on 181 x 141; any result at dt 0.025 with the far field at 30 D; any result of this registration's
three runs. Every row W below is blind in that sense.

ADR 0761 says its arithmetic "suggests about 1.351" for the mesh-converged drag. That number is not a result. It was
not used to choose the meshes, the far-field distance, the step, the window, the order range or anything else here; the
choices are derived in the next section from the geometry of the mesh and from rows of the tolerance table. The
estimates in the table of checks are there because rule 14 asks for them; they are made from the records above and
they decide nothing.

## Design

No solver source changes. One new test file, `crates/rcfd-api/tests/f869_cylinder_mesh_converged_drag.rs` (header: ADR
0763, F-869). `f869_cylinder_re100_mean_drag.rs` and its patch are not edited.

**The set-up** is that of ADR 0761, copied: `cylinder_ogrid(n_theta, n_r, 0.5, r_out, Expansion(150), 0.1)`, D = 1, U =
1, nu = 0.01, `PimpleSettings::piso` with `time_scheme = Bdf2`, linear upwind, `n_outer = 2`, `amg_pressure`, `alpha_u =
alpha_p = 0.9`, the cylinder a no-slip wall, the far field `Freestream`, the same initial disturbance, the same blow-up
guard. Changed against ADR 0761, and nothing else: the far-field radius, the time step and the meshes.

**The far field: r_out = 30** (30 D with D = 1), one value. The maintainer's answer names 30 D or more. 30 D is taken
because it is the one larger distance with records to read the result against (R2 of ADR 0761, 1.33884 on 128 x 100 at
dt 0.05; step (iii) of ADR 0060 as ADR 0761 records it), and because the grading is fixed, so a larger radius coarsens
every mesh further (next paragraph). A radius of 30 D is more than the upstream and the lateral extent of the domain
that source A of ADR 0761 uses for its own value (16 D each, [-16 D, 48 D] x [-16 D, 16 D]) and less than its
downstream extent (48 D). Whether the drag still moves beyond 30 D is not covered. Taken on the maintainer's standing
instruction of 2026-11-11 (take the recommendation).

**The time step: dt = 0.025** on every mesh, as the maintainer's answer names it.

**The three meshes: 90 x 70, 128 x 100, 181 x 141** (n_theta x n_r; 6300, 12800 and 25521 cells). Derivation.
`Grading::Expansion(150)` is the ratio of the last to the first radial cell (`gen.rs` line 52; the cells are made by
`geometric(n, len, r)`, line 87), so with n_r fixed the radial cells are a geometric sequence whose every member is
proportional to the radial length r_out - 0.5. Moving the far field from 20 D to 30 D therefore makes every radial
cell larger by 29.5 / 19.5 = 1.513 at the same n_r, a little more than one refinement step of the sequence (sqrt 2 =
1.414). The sequence of ADR 0761 (64 x 50, 90 x 70, 128 x 100) is therefore moved one step finer, which gives back its
radial spacing to within 1.513 / 1.414 = 1.07 (first radial cell: 0.00646 on 128 x 100 at 20 D, 0.00696 on 181 x 141
at 30 D, by the formula of a geometric sequence) and makes the tangential spacing at the wall finer by 1.41. The three
meshes are those of the records (ADR 0060; 181 x 141 in `validation_cylinder_re100_w31.rs`); no mesh is new. Taken on
the maintainer's standing instruction of 2026-11-11 (take the recommendation).

Stability of the finest run, argued and not known: BDF2 is recorded stable on 128 x 100 at dt 0.05 and 20 D (ADR 0069,
ADR 0761). Against that run the finest run here has the tangential cell smaller by 1.414, the radial cell larger by
1.07 and the step smaller by 2, so its Courant numbers are at most 0.71 of that run's for the same velocity field. A
blow-up fails the test at the guard and is not repeated (see the decision rule).

**The run and the averaging window**, the same on every mesh and the same as ADR 0761: the run goes to t = 160 (6400
steps); the velocity component v at the cell nearest to the probe (3, 0, 0.05) is recorded for t > 60 and the force on
the cylinder for t > 85. The start of the window is not chosen by hand and not from the drag: with the upward zero
crossings (linear interpolation) of the probe signal less its mean over t > 60, the window W is from the ninth-last
crossing to the last crossing, that is the last 8 whole shedding periods before t = 160. St = 8 / length of W. C_D is
the trapezoid mean over W of F_x / (0.5 U^2 D depth), with the force from
`rcfd_post::integrals::wall_force_and_moment`. The lift coefficient F_y / (0.5 U^2 D depth) gives over W its amplitude
(half of the largest less the smallest) and its rms about its mean. The two halves of W, W1 (the first 4 whole periods)
and W2 (the last 4), give two further means of the drag, used in condition (c) below.

**The Richardson procedure.** f1, f2, f3 are the mean drag over W on 90 x 70, 128 x 100, 181 x 141. The representative
cell size is h = N^(-1/2) with N the number of cells: the mesh is two-dimensional, the grading is the same on all
three, and both directions are refined by nearly the same factor (128/90 = 1.422 and 100/70 = 1.429; 181/128 = 1.414
and 141/100 = 1.410). The ratios are therefore r21 = sqrt(12800/6300) = 1.42539 and r32 = sqrt(25521/12800) = 1.41203,
not exactly sqrt 2, and the procedure is derived for unequal ratios from first principles: with the model f_i = f_0 + C
h_i^p of one leading error term,

    f2 - f3 = C h3^p (r32^p - 1),    f1 - f2 = C h3^p r32^p (r21^p - 1),

so the observed order p is the root of

    g(p) = r32^p (r21^p - 1) / (r32^p - 1) = (f1 - f2) / (f2 - f3),

found by bisection on [0.01, 20] (g is 1.031 at p = 0.01; for equal ratios r it is r^p and the root is the closed
form ln|(f1 - f2)/(f2 - f3)| / ln r of ADR 0060 and ADR 0761). The Richardson value is

    f_h = f3 + (f3 - f2) / (r32^p - 1)

with the observed p. GCI_fine = 1.25 |f3 - f2| / (|f3| (r32^p - 1)) with p limited to [1, 2] for the GCI, that is
`validation_common::gci_fine(f2, f3, p, r32)` (Roache's index with the safety factor 1.25, as ADR 0060 lines 196 to 198
define it).

The formal order of the sequence is 2: the linear-upwind face value phi_P + grad(phi)_P . (x_f - x_P) is exact for a
linear field, the diffusion term is the central difference, and the step is common to the three meshes, so the mesh
sequence sees the spatial order alone. The time error of dt 0.025 is common to the three values and is carried by f_h
as it is (ADR 0761 estimated it as about +0.0025 on 128 x 100 at 20 D); it is not removed and no extrapolation in the
step is made here.

**When the extrapolation is not valid.** It is valid only if all three hold:

- (a) the sequence is monotone: (f1 - f2)(f2 - f3) > 0, and the bisection has a root (the ratio of the differences is
  inside (g(0.01), g(20)));
- (b) the observed order is in **[1.5, 2.5]**: within 0.5 of the formal order 2, the row "Observed order of accuracy" of
  the [tolerance table](../process/tolerances.md);
- (c) the conclusion does not hang on the window: on W1 and on W2 alike, the fine value is in B, the three half-window
  means are monotone with a root, and their Richardson value is in B. This condition has no number of its own; its
  limits are the band.

What is concluded when it is not valid: the mesh-converged drag is **not shown**; the item stays partial with the fine
value, the three values, the observed order and the reason. In that case the number that the formula gives is still
printed, marked not valid, and beside it the value at the formal order 2, f3 + (f3 - f2)/(r32^2 - 1); neither is then
a result and neither is compared with the band for the decision. The order range is not widened after the run.

**What the gate runs and what is a heavy record.** The three runs are about 285 million cell-steps (ADR 0761's heavy
record: 194 million in 1897 s beside other solves), so they are a heavy record: expected 35 to 75 minutes; no limit on
time is registered.

- **Heavy record** (ignored, run once by me, the three runs one after the other in one process behind a `OnceLock`):
  `h1_fine_mesh_mean_drag_at_dt_0025_and_30d_in_the_band_heavy_record` carries W1, S1 and K4w and prints the reported
  rows; `w2_richardson_value_of_the_mesh_sequence_valid_and_in_the_band_heavy_record` carries W2 and W3. They are two
  tests from the start so that a miss of the one does not hide the other.
- **The gate runs** (not ignored, each a few seconds): `g1_...` (the procedure recovers a manufactured limit and order,
  and reproduces the record of ADR 0761), `n1_...` (the negative controls of the decision), `g3_...` (the window
  statistics on a signal with a closed form) and `g4_...` (the coarsest registered mesh, 90 x 70 at 30 D, advanced 40
  steps of dt 0.025 through the same `setup`, `advance` and `force` as the heavy record). The gate-side tests of ADR
  0761 (`g1_`, `g3_` of the other file) stay as they are and still hold the drag path on 64 x 50.

Said plainly: the comparison of the mesh-converged drag with the band is a heavy record that no gate runs; the gate
holds the arithmetic of the procedure, the decision and the statistics, and that the set-up at 30 D runs.

## Table of checks

Estimates, from the records of ADR 0761 and deciding nothing: 128 x 100 at dt 0.025 and 30 D from 1.34511 less the 1.01
% of R2, 1.3315; 90 x 70 from 1.33879 less the 0.0074 of the halved step less 1 %, 1.318; 181 x 141 from the
difference 0.0137 of the last pair of ADR 0761 divided by 1.4^1.38 to 1.4^2, 1.338 to 1.340. Margin: for a band, the
distance from its middle to its end divided by the distance of the estimate from the middle; for a one-sided limit, the
limit divided by the estimate.

| Id | Check | Limit | Source of the limit | Estimate | Margin | Kind |
| --- | --- | --- | --- | --- | --- | --- |
| W1 | acceptance, "the mean drag coefficient ... is within the band of the published values"; the maintainer's answer, "the fine value": mean C_D on 181 x 141, far field 30 D, dt 0.025, BDF2 (heavy record) | in [1.314, 1.370] | ADR 0761, "The band, registered" (arXiv:2303.09262 Table 1, Re = 100, and arXiv:1401.6513 Table 3); the acceptance of F-869 | 1.338 to 1.340 | 7 (wide) | validation |
| W2 | the maintainer's answer, "the Richardson value in the same band": f_h of the three meshes (heavy record) | in [1.314, 1.370] | the same band; the answer of the maintainer of 2026-11-22 in ADR 0761 | 1.345 to 1.356 | 2.0 | validation |
| W3 | the extrapolation is valid: (a) monotone with a root; (b) observed order in [1.5, 2.5]; (c) on each half window the fine value and the Richardson value in [1.314, 1.370] (heavy record) | holds | (b): `docs/process/tolerances.md`, row "Observed order of accuracy" (within 0.5 of the formal order); (a) and (c): the brief, task 1 ("the condition under which the extrapolation is not valid"), with the band of W1 as the only number | order 1.4 to 2.0: (b) may miss | 1.0 for (b), which may miss | solution verification |
| S1 | K3's drag half on this sequence: GCI_fine of C_D (heavy record) | at most 3 % | ADR 0060, K3 | 0.8 to 1.3 % | 2.3 | solution verification |
| K4w | sustained shedding in each of the three runs | more than 9 upward zero crossings after t = 60 | ADR 0060, K4 | 16 | 1.6 | solution verification |
| G1 | the procedure (gate, pure): (i) for f_i = f_0 + C h_i^p on the registered cell counts, p = 1.5, 2 and 2.5, it returns f_0 and p; (ii) with both ratios sqrt 2 and the recorded triple 1.31674, 1.33879, 1.35247 it returns the record of ADR 0761 | (i) relative error at most 1e-9 for each; (ii) within 1e-4 of 1.37479 and within 0.01 of 1.38 | (i) `docs/process/tolerances.md`, row "Agreement with a closed form" (exact for the discrete problem); (ii) ADR 0761, addendum 1, row V2 (b), whose inputs are printed to five decimals | (i) 1e-13; (ii) 1.37483 and 1.377 by the arithmetic done beforehand | (i) 1e4 (wide); (ii) 2.5 | code verification |
| N1 | negative controls of the decision (gate, pure), each a manufactured or recorded triple put through the same function `verdict` as the heavy record: (a) f_0 = 1.375 at order 2 with the fine value in the band gives "partial", not "done"; (b) a non-monotone triple gives "not valid"; (c) order 1.2 with f_0 in the band gives "not valid"; (d) the triple of ADR 0761 with its ratios does not give "done"; (e) a fine value of 1.380 gives "stopped"; (f) f_0 = 1.345 at order 2 gives "done"; (g) a half window whose Richardson value is 1.375 gives "not valid" | every verdict as stated | the brief, task 4 ("a negative control or mutation on the new check") and its decision rule | holds | n/a | code verification (negative control) |
| G3 | the window statistics (gate, pure) on v = sin(2 pi 0.165 t), F_x = 0.05 (1.34 + 0.01 sin(4 pi 0.165 t)), F_y = 0.05 x 0.33 sin(2 pi 0.165 t + 0.7), sampled with dt 0.025 as a run records them: St, mean C_D on W, W1 and W2, the amplitude and the rms of C_L against 0.165, 1.34, 0.33 and 0.33/sqrt 2 | relative error at most 1e-3 each; 8 whole periods in W and 4 in each half | `docs/process/tolerances.md`, row "Agreement with a closed form" (exact only for the continuum) | 1e-4 | 10 (wide) | code verification |
| G4 | the same code path as the heavy record (gate): 90 x 70 at 30 D, 40 steps of dt 0.025 through `setup`, `advance`, `force` | 40 samples, every value finite, the guard silent | rule 7 of `docs/process/agent-rules.md` ("a fast test on the same code path") | holds | n/a | code verification |
| M1 | committed mutation: the Richardson step dropped in the test file (`f_h` returns f3) | `g1_` fails | rule 16 of `docs/process/agent-rules.md`; the brief, task 4 | fails | n/a | code verification (negative control) |
| M2 | mutation: the upper end of the band 1.370 written as 1.380 in the test file | `n1_` fails at (a) | the brief, task 4 | fails | n/a | code verification (negative control) |
| M3 | mutation: the lower end of the order range 1.5 written as 0.5 in the test file | `n1_` fails at (c) | the brief, task 4 | fails | n/a | code verification (negative control) |
| R1 | reported on each mesh: St (and its place in K1's window [0.16005, 0.16995]), the amplitude and the rms of the lift coefficient, the crossings, the window, the means on W1 and W2 | none (reported) | the brief, task 2 | St 0.163 to 0.165, rms 0.23 to 0.24 | n/a | validation (reported without a limit) |
| R2 | reported: the observed order; f_h; GCI_fine; f_h and the order by the equal-ratio form of ADR 0761 (r = sqrt 2); the value at the formal order 2; the Richardson values of the two half windows | none (reported; the decision uses W1, W2, W3 and S1 only) | the brief, task 2 | see W2, W3 | n/a | solution verification (reported without a limit) |
| E | `git diff --stat 2bab1863` names this ADR, the new test file, its mutation patch and the proposal file only | no other file | the brief ("Files you own"; "Change no solver source") | holds | n/a | unchanged default output |
| F | `cargo fmt -p rcfd-api --check`; `cargo clippy -p rcfd-api --all-targets -- -D warnings`; `cargo test -p rcfd-api --release --lib`; `cargo test -p rcfd-cli --release --test docs_site` | clean; clean; passes; passes | rule 7 of `docs/process/agent-rules.md` | clean | n/a | code verification |
| P | `python scripts/process_report.py --check` and `python scripts/roadmap_proposals.py check` on the last commit | no failure of this work | rules 7 and 9 of `docs/process/agent-rules.md` | none, but generated pages not current | n/a | code verification |

W3 (b) is the row most likely to miss, and that is said now: the one BDF2 mesh sequence on record (ADR 0761, dt 0.05, 20
D) had the observed order 1.38, under the range. The step is halved here, which removes three quarters of a
second-order time error from each value, and the meshes are one step finer; whether that brings the order into [1.5,
2.5] is not known. Nothing is tuned to it.

## Negative control and mutations

- **N1** runs in the gate: seven triples through `verdict`, the function that the heavy record uses, each with the
  verdict it must give; six of the seven must not give "done".
- **M1** (committed, `crates/rcfd-api/tests/f869_cylinder_mesh_converged_drag.mutation.patch`, first line
  `# expect-fail: cargo test --release -p rcfd-api --test f869_cylinder_mesh_converged_drag g1_`): the Richardson step
  dropped. Made with `git diff` on a scratch copy; the tree of this work never holds the change.
- **M2** and **M3**: the band's upper end and the order range's lower end widened in the test file.

All three on a scratch copy of the tree at the last code commit, release, each run once; what failed and its message
are recorded. They touch the test file only, so they are run against the gate-side tests and need no solve.

## Size

Size (agent): done; 0; 1; 0; 110000

The one heavy run is the heavy record (three solves in a row, both ignored tests in one process).

## Decision rule

- **done** only if W1 and W2 are met and W3 holds (the extrapolation is valid), S1 and K4w are met, G1, N1, G3 and G4
  pass, M1, M2 and M3 are caught, and E, F and P hold.
- **partial**, with the numbers, if W1 is met and any of these holds: W2 is missed (the Richardson value is outside the
  band: a second known miss with its number); W3 does not hold (the extrapolation is not valid: the mesh-converged drag
  is not shown, and the reason is stated: not monotone, the order, or the window); S1 is missed; a gate-side check is
  missed; a mutation survives; a check was not run.
- **stopped and reported** if W1 is missed (the fine value itself leaves the band, with its number), if a run blows up
  at the guard or shows no sustained shedding (K4w), or if a defect of the solver appears. Nothing is changed in the
  solver and no run is repeated.
- **todo** if the heavy record gives no number.

A run that misses is not repeated and no limit is changed (rule 2). A missed W2 or W3 is kept as an ignored
`*_known_miss` test with its limit and its number, by an amendment written before the test file is changed; the known
miss V2b of ADR 0761 stays as it is.

## What is not covered

- A far field beyond 30 D, and an extrapolation in the time step at 30 D: f_h carries the time error of dt 0.025.
- A mesh finer than 181 x 141, a fourth mesh, another grading; the unequal refinement of the two directions beyond what
  h = N^(-1/2) takes up.
- The default first-order time step (the maintainer's answer to question 2 of ADR 0761: the comparison stays with the
  option `time_scheme = Bdf2`).
- The Strouhal number and the lift as checks: they are reported (R1). The Strouhal number is F-029's acceptance.
- The nine originals behind the quoted values of the band, and any measured drag (as ADR 0761 says).
- The case-file path (`Simulation`): the test drives `PimpleSolver` directly, as ADR 0761 does.
