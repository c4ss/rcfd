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

## Amendment 1 (2026-11-22): the missed W3 (b) becomes a known miss; written before the change of the test file

Written after the gate-side tests, the heavy record and M1, M2 and M3 were each run once, and before any change of the
test file. No limit, band, order range, mesh, step, window or run changes. The heavy record ran its three solves. Its
test `h1_...` passed (K4w, W1, S1). Its test `w2_...` failed on one check: W3 (b), the observed order of the three
meshes, 1.387 against [1.5, 2.5]. W3 (a), W3 (c) and the comparison of W2 passed (the numbers are in addendum 1).

By rule 2 the missed limit becomes an ignored test named `*_known_miss` with the limit intact. The change of
`crates/rcfd-api/tests/f869_cylinder_mesh_converged_drag.rs`, and nothing else:

- `w2_richardson_value_of_the_mesh_sequence_valid_and_in_the_band_heavy_record` keeps W3 (a), W3 (c) and W2 and no
  longer asserts W3 (b); its comment says that the value it compares is not a valid extrapolation by the registration;
- a new ignored test `w3b_observed_order_of_the_mesh_sequence_in_the_registered_range_known_miss` asserts W3 (b) with
  the same expression and the same range, and records the measured 1.387 in its comment;
- the comment of `h1_...` and the head of the file record the measured numbers;
- the gate-side tests, the functions `richardson`, `verdict`, `halves_hold`, `setup`, `advance`, `force`,
  `statistics`, `run`, `record` and `extrapolation`, and the constants are not touched. The function `verdict` still
  returns "partial, the order" for these numbers.

The heavy runs are not made a second time (rule 2, "each registered run is made once"): the three ignored tests after
the change are built, formatted and linted, and not run. The gate-side tests are run again after the change, because
the file they are in was changed; their code is the same.

## Addendum 1: results (2026-11-22)

All runs in the release profile, on the branch at 790ed327 (the test file as first committed; the solver is that of main
2bab1863), `CARGO_TARGET_DIR` under the scratch folder, beside the solves of other agents. Each registered run was made
once. Logs in the scratch folder: `gate_run1.log`, `heavy_record.log`, `m1_run.log`, `m2_run.log`, `m3_run.log`,
`lib_run.log`, `gate_run2.log`. No limit, band or range was changed.

### The sequence (BDF2, dt 0.025, far field 30 D, window of the last 8 whole periods)

| Mesh | C_D | In [1.314, 1.370] | St | C_L amplitude | C_L rms | Crossings | Window | C_D on W1, W2 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 90 x 70 | 1.32259 | yes (not a check) | 0.16169 | 0.3135 | 0.2216 | 17 | 110.3 to 159.7 | 1.32259, 1.32260 |
| 128 x 100 | 1.33285 | yes (not a check) | 0.16395 | 0.3283 | 0.2321 | 16 | 108.8 to 157.6 | 1.33285, 1.33286 |
| 181 x 141 (the fine value, W1) | 1.33900 | **yes** | 0.16486 | 0.3412 | 0.2412 | 16 | 108.0 to 156.5 | 1.33900, 1.33901 |

The sequence is monotone. The observed order (unequal ratios 1.42539 and 1.41203) is **1.387**, outside the registered
range [1.5, 2.5]. By the registration the extrapolation is therefore **not valid** and the mesh-converged drag is **not
shown**. The numbers the formulas give, printed as registered and not results: with the observed order 1.34901; by the
equal-ratio form of ADR 0761 1.34818 (order 1.479); at the formal order 2, 1.34519; on the half windows 1.34900 and
1.34902 (orders 1.388 and 1.387). GCI_fine is 0.935 %.

### Every check

| Id | Limit | Measured | Met |
| --- | --- | --- | --- |
| W1 | C_D on 181 x 141 in [1.314, 1.370] | 1.33900 (-0.22 % of 1.342) | yes |
| W2 | Richardson value in [1.314, 1.370] | 1.34901 passes the comparison, but the value is not a valid extrapolation (W3 b) | not shown |
| W3 (a) | monotone with a root | monotone, order 1.387 | yes |
| W3 (b) | observed order in [1.5, 2.5] | 1.387 | **no: missed** |
| W3 (c) | on each half window the fine value and the Richardson value in the band | 1.33900 and 1.33901; 1.34900 and 1.34902 | yes |
| S1 | GCI_fine(C_D) at most 3 % | 0.935 % | yes |
| K4w | more than 9 crossings, every run | 17, 16, 16 | yes |
| G1 | (i) relative error at most 1e-9; (ii) within 1e-4 of 1.37479, within 0.01 of 1.38 | (i) passed at 1e-9 (printed as 0.00000); (ii) 1.37483 (0.00004 off), 1.3774 (0.0026 off) | yes |
| N1 | the seven verdicts, and the limit of S1 in the decision | all as stated | yes |
| G3 | relative error at most 1e-3 | at most 1e-5 (the rms; the others print as 0.00000) | yes |
| G4 | 40 samples, finite, the guard silent | 40, finite, t = 1.000 in 3 s | yes |
| M1 | committed mutation (the Richardson step dropped): `g1_` fails | failed: G1 (i) limit off by 0.00595 at each order, G1 (ii) by 0.02232 | caught |
| M2 | the band's upper end as 1.380: `n1_` fails at (a) | failed at (a): "left: Done, right: Partial(OutsideBand)" | caught |
| M3 | the order range's lower end as 0.5: `n1_` fails at (c) | failed at (c): "left: Done, right: Partial(Order)" | caught |
| E | only the files of this work changed | `git diff --stat 2bab1863`: this ADR, the test file, its patch, the proposal file | yes |
| F | fmt, clippy, `--lib`, docs_site | `cargo fmt -p rcfd-api --check` clean; `cargo clippy -p rcfd-api --all-targets -- -D warnings` clean; `cargo test -p rcfd-api --release --lib` 159 passed, 4 ignored; docs_site given below | yes |
| P | the process check and the proposal check | given at the end of this addendum | see there |

Reported rows. **R1**: the Strouhal number is in K1's unchanged window [0.16005, 0.16995] on all three meshes (0.16486
on the fine mesh, -0.08 % of 0.165). The rms of the lift coefficient on the fine mesh, 0.2412, is above the published
0.222 to 0.236 of source A of ADR 0761 by 2.2 % of its upper end; the amplitude, 0.3412, is above the 0.31 and 0.28 of
source B. Both still rise with the mesh. No limit was registered for either. **R2**: as in the paragraph above the
table. The value on 128 x 100 here, 1.33285, against the records of ADR 0761 on the same mesh: 1.34511 at dt 0.025 and
20 D, 1.33884 at dt 0.05 and 30 D.

### What the gate runs and what is a heavy record

- The gate runs `g1_`, `n1_`, `g3_` and `g4_` of the new file (3 s together) and, unchanged, the gate-side tests of ADR
  0761. They hold the arithmetic of the procedure, the decision, the statistics, and that the set-up at 30 D runs.
- The heavy record `h1_...` (ignored) carries W1, S1 and K4w; `w2_...` (ignored) carries W3 (a), W3 (c) and W2's
  comparison; the three solves took 349 s, 1297 s and 1614 s, 3260 s together. No gate runs them.
- The known miss `w3b_observed_order_of_the_mesh_sequence_in_the_registered_range_known_miss` (ignored) carries W3 (b)
  with its range intact (amendment 1). The known miss V2b of ADR 0761 is untouched.

### Reading of the miss of W3 (b)

Arithmetic on the numbers above; none of it is a check, nothing was run for it, and the cause is not diagnosed.

- The order is the same as on the sequence of ADR 0761 (1.38 at dt 0.05 and 20 D, one mesh step coarser). Halving the
  step, widening the domain and refining by one step left it where it was. The expectation of the registration, that
  the time error held the order down, is therefore not supported: the low order looks like a property of the spatial
  discretisation of this case on these meshes. What it is was not looked for (candidates that were not examined: the
  strongly graded O-grid, the wall force integral, the far-field condition). No blow-up and no defect of the solver
  appeared, and nothing in the solver was changed.
- How far the conclusion "in the band" hangs on the order, for the fine pair 1.33285 and 1.33900: the Richardson step
  is 0.00615 / (1.41203^p - 1), which gives 1.35393 at order 1, 1.34902 at 1.387, 1.34807 at 1.5, 1.34519 at 2 and
  1.34349 at 2.5. It reaches the upper end 1.370 only for an order under 0.52. So every order from 1 to 2.5 puts the
  extrapolated drag between 1.343 and 1.354, inside the band; this is an observation and not the registered check,
  which asked for the order to be in its range first.
- Against the miss V2b of ADR 0761 (1.37479 at dt 0.05 and 20 D): the same procedure on this sequence gives 1.349. The
  smaller step and the wider domain go with the difference, as ADR 0761's arithmetic supposed; the two were changed
  together here and are not separated.

### Deviations

- M1 was made as `f3 + 0.0 * (f3 - f2) / (...)` in place of a bare `f3`, so that the mutated file builds without
  warnings of unused arguments; the effect is the registered one (`f_h` returns f3).
- The gate-side tests were run a second time after amendment 1 (`gate_run2.log`, 4 passed), as the amendment says.
- A first attempt to write this file was refused because the session stood in another worktree; the session was
  switched into this worktree and nothing was written elsewhere.

### What was not run

- The three ignored tests as they stand after amendment 1 (built, formatted and linted only; their runs are those of
  the heavy record before the split).
- The whole workspace suite, the other integration tests of `rcfd-api` (among them the tests of ADR 0761, whose file is
  unchanged), and `baseline_hashes` (no solver file was touched).
- A fourth mesh, a far field beyond 30 D, an extrapolation in the time step at 30 D, any diagnosis run for the order.
- The lead's application of the committed mutation patch (checked here with `git apply --check` on the final tree, and
  M1 was run from the same diff on the scratch copy).

### Size

Size (agent, actual): partial; 0; 1; 1; 105000

### Decision by the registered rule

**Partial.** W1 is met: the fine value, 1.33900 on 181 x 141 at dt 0.025 with the far field at 30 D, is in the band
[1.314, 1.370]. W3 does not hold: the observed order, 1.387, is outside [1.5, 2.5], so by the registration the
extrapolation is not valid and the mesh-converged drag is not shown, although the number the formula gives, 1.34901, is
in the band, as is the value at every order from 1 to 2.5. S1, K4w and the gate-side checks are met and the three
mutations are caught. What is missing: a mesh-converged value by an extrapolation that is valid, or the maintainer's
word that the registered order range is not the measure.

## Open questions for the maintainer

1. **The fine value and every extrapolated value are in the band, but the observed order of the sequence is 1.39, under
   the range [1.5, 2.5] that this registration set for a valid extrapolation. Is the item done?** Options: (a) the item
   stays partial; a further registration under F-869 (rule 13) adds a fourth mesh, 256 x 200 at the same step and far
   field, and registers before the run whether the mesh-converged drag is then read from the finest triple with the
   same range, or from the bound over a registered range of orders; (b) the item is done as it stands: the fine value
   is in the band, and the Richardson value is in the band for every order from 1 to 2.5 (1.343 to 1.354), with the
   known miss of the order kept as a record; (c) as (b), and a new item diagnoses why the drag converges at order 1.4
   on this case (the same order on two sequences). Recommendation: (c). The acceptance of F-869 is about the drag
   against the band, and that conclusion does not depend on the order within any range one would register; the low
   order is a finding about the discretisation and deserves its own item rather than holding the headline. It is not
   taken on the standing instruction because it changes how the registered rule of a headline item is read and adds an
   item (rule 6).

## For the lead: roadmap entries

Status proposed: partial.

> ADR 0763, 2026-11-22, partial: on the mesh sequence 90 x 70, 128 x 100, 181 x 141 with BDF2 at dt 0.025 and the far field at 30 D the mean drag is 1.32259, 1.33285, 1.33900 (St 0.16169, 0.16395, 0.16486; GCI 0.94 %), the fine value inside the band [1.314, 1.370], but the observed order is 1.387, outside the registered range [1.5, 2.5], so the extrapolation is not valid by the registration and the mesh-converged drag is not shown (known miss W3b); the formula gives 1.34901, and 1.343 to 1.354 for every order from 1 to 2.5, all inside the band.

> The drag is in the published band [1.314, 1.370] on the fine mesh of the registered sequence (1.33900 on 181 x 141, BDF2, dt 0.025, far field 30 D), and a Richardson value at any order from 1 to 2.5 is in it too (1.343 to 1.354). Missing: the observed order is 1.387, outside the registered [1.5, 2.5] (known miss W3b, ADR 0763), so the mesh-converged drag is not shown by a valid extrapolation; the maintainer decides whether that holds the item (ADR 0763, question 1).

No follow-up item is proposed here: whether one is entered is question 1. The same entries are in
`docs/roadmap/proposals/0763-F-869.json`.

For the lead, not roadmap: `docs/adr/index.md` is generated and not current after this work (the new ADR); it is left
to the lead (rule 9). No reference fragment was added, so `docs/reference-data.md` is not touched by this work.

### The process check (row P)

`python scripts/roadmap_proposals.py check docs/roadmap/proposals/0763-F-869.json`: "F-869: partial -> partial; gap set;
note + 549 characters", exit 0. `python scripts/process_report.py --check`: 1 failure, "generated file is not current:
docs/adr/index.md"; no failure of the registration. The page is left to the lead (rule 7, rule 9).
`cargo test -p rcfd-cli --release --test docs_site` on the tree with this addendum: 2 passed.

## The lead's reading (2026-11-22), at the merge

Merged at cc4a9c74 and pushed at 99b7daf1 (the report read at 11:50 UTC, 4 minutes of reading, proposed partial, written
partial). Checked by the lead before the merge: the registration is the first commit and alone; amendment 1 is
committed before the change of the test file it allows; the branch touches only the test file, its mutation patch, this
ADR and the proposal file; the three heavy tests are ignored with the reason and the command in the attribute, one of
them the known miss W3 (b). The push script ran the committed mutation and it failed as expected.

Status: **partial**, as proposed, applied from `docs/roadmap/proposals/0763-F-869.json` as the agent wrote it. The lead
reads the result as the agent does: the fine value and the extrapolated value at every order from 1 to 2.5 lie in the
band, and the order of 1.39 is the same on two sequences that differ in step, far field and mesh, so the time step was
not what held it down. The lead's recommendation on question 1 is (c), with the agent's reason: the acceptance of F-869
is the drag against the band, and the order is a finding about the discretisation. Question 1 is put to the maintainer
because it changes how a registered rule of a headline item is read and adds an item.
