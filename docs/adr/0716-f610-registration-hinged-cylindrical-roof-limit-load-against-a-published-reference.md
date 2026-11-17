# ADR 0716: F-610 registration: the hinged cylindrical roof limit load against a published reference

Status: **REGISTERED 2026-11-17, written before any code of F-610 and before any run of its checks.** Written by the
implementation agent on branch `f610-work` (from 80281ea9), worktree `w49i-work/f610`, scratch `w49i-work/f610-scratch`. Not
yet accepted by the maintainer. Every choice below that the maintainer would normally be asked about is taken on the
maintainer's standing instruction of 2026-11-11 (take the recommendation) and is marked "(standing instruction)".

Roadmap item F-610 (deps F-585), the follow-up proposed by ADR 0522 (its remaining question Q1, option a). The dep is met: ADR
0522 put Shell4 and Shell3 with the corotated element of ADR 0490 into `arclength::trace`, and recorded the quarter roof
models and the Hex20 sequence. Test-only work: no library line changes. House rules (`w49i-work/AGENT_RULES.md`) apply: limits
fixed here and not changed after the run, each run made once, no golden or hash table edit, ADR 0522 and its test file
`crates/rcfd-fem/tests/sa_corot_shell_dyn.rs` not edited.

## What F-610 asks

> Every limit not stated here is registered in the ADR before the run. The tabulated limit load of the hinged cylindrical roof of Sze, Liu and Lo (2004) for t = 12.7 recorded in docs/reference-data.md from the paper; the Shell4 and Shell3 quarter models of ADR 0522 traced through the limit point by arc length within 2 percent of it; the Hex20 sequence of ADR 0522 kept as a record of the singular hinge.

Clauses: (c1) the reference value recorded from the paper; (c2) Shell4 and Shell3 quarters through the limit point within 2
percent of it; (c3) the Hex20 sequence kept as a record.

## What was read (disclosed)

`AGENT_RULES.md`; the brief; ADR 0522 in full (D5, amendments 3 and 4, addendum 1, its roadmap entries) and the roof part of
`crates/rcfd-fem/tests/sa_corot_shell_dyn.rs` (lines 1 to 340 and 465 to 482: `roof_shell`, `roof_hex20`, `vertex`,
`roof_limit`, `a2`, the two roof records and the fast roof test); the root choice in `crates/rcfd-fem/src/arclength.rs`
(lines 197 to 252); the F-585 section of `docs/reference-data.md`; the form of ADRs 0572, 0573 and 0714.

**The paper was read** (not a secondary source): K. Y. Sze, X. H. Liu and S. H. Lo, "Popular benchmark problems for geometric
nonlinear analysis of shells", Finite Elements in Analysis and Design 40 (2004) 1551-1569, the authors' copy in the HKU
Scholars Hub, `https://hub.hku.hk/bitstream/10722/54269/2/92945.pdf` (record `https://hub.hku.hk/handle/10722/54269`), 25 pages,
2710426 bytes, SHA-256 `8eaeb6904eccc3d3f6c260c9564f51c64d6375ad3fb6e952178bdedff68d0bce`. How it was read: `curl` and the
web fetch tool were refused by the site's bot check (HTTP 403); the PDF was opened in the user's Chrome on the record page and
its text taken from the page content streams (inflated with the browser's own `DecompressionStream`, the strings of the text
operators read in order; no external script). Only text was seen: the drawings of the figures and any equation set as
graphics were not. Notes with the full Table 9a as read: `f610-scratch/source-notes.txt`. What was read of it:

- Section 3.8 "Hinged cylindrical isotropic and laminated roofs": the roof under a central pinching force; isotropic and two
  laminates at two thicknesses; the reference solutions by the Riks method in ABAQUS (the paper's reference is itself a
  finite-element solution); a quarter modelled by symmetry; "Tables 9a to 9f list the data points"; with the Riks method a
  solution cannot be placed at a preset load level, so the tabulated loads are the points the path happened to reach.
- Figure 9a (its text): `R = 2540, L = 254`, `theta = 0.1 radian`, `h = 12.7 or 6.35`, `Pmax = 3000`, isotropic `E = 3102.75`,
  `nu = 0.3`; caption "Hinged cylindrical roof subjected to a central pinching force"; the labels `L` (twice), `hinged` (twice),
  `free` (twice), `C`. Units are not named ("unit thick"); consistent units, read here as N and mm as in ADR 0522.
- **Table 9a**, "Deflections for the 12.7 unit thick isotropic hinged cylindrical shell (computed by using 16 x 16 S4R elements
  and Riks method)", columns `P/Pmax` and `-W_C` (the deflection of the loaded point C), 36 points. The rising branch ends with
  `(0.7246, 9.339)`, `(0.7412, 10.358)`, `(0.7421, 11.293)`, then `(0.7286, 12.141)`, `(0.7023, 12.903)` and on down to
  `(0.1729, 19.852)`, then up again to `(1.0835, 29.374)`.

**The registered reference.** The tabulated limit load is the largest tabulated `P/Pmax` before the snap, `0.7421`, times
`Pmax = 3000`: **`P_ref = 2226.3` N** (the whole roof, the central point load). Arithmetic 1 (by a one-line Python evaluation,
disclosed): the parabola through the three tabulated points around it, `(10.358, 0.7412)`, `(11.293, 0.7421)`, `(12.141,
0.7286)`, has its vertex at `W_C = 10.876`, `P/Pmax = 0.74374`, `2231.2` N, `2.2e-3` above `P_ref`: the tabulated point lies a
few `1e-3` below the peak of the paper's own path. `P_ref` is the registered value (it is what the paper tabulates); the
vertex value is a record.

**Geometry.** The figure text gives `L = 254` and two `L` labels; ADR 0522 models straight edges of `508` (a quarter of axial
length `254`). Read as the half-length per label (`2L = 508`), consistent with ADR 0522; the drawing itself was not seen, so this
reading is an assumption, supported by the secondary text of arXiv:2107.12483 (Section 3.3, read for this purpose only: the
same panel with `L = 508 mm`, `R = 2540 mm`, `0.1 rad`, `12.7 mm`) and noted as such.

**Known before the registration (disclosed).** ADR 0522 addendum 1 measured, on the same models with the same extraction,
Shell4 `16 x 16` `2222.4` N, Shell4 `8 x 8` `2223.6` N, Shell3 `16 x 16` `2221.3` N (apex deflection at the limit about `10.8`).
Against `P_ref` these are `-1.75e-3`, `-1.21e-3`, `-2.25e-3` (arithmetic 2). The outcome of the 2-percent clause is therefore
expected from records; this registration fixes the reference and reruns the same models once against it. The reference value
was not chosen with these numbers in view: it is the largest tabulated `P/Pmax` of Table 9a, a rule stated before the
arithmetic 2 was done, and the alternative (the vertex, arithmetic 1) is recorded beside it.

## Design

### D1. The models and the extraction (unchanged from ADR 0522 D5)

The quarter roof of ADR 0522 D5, copied into a new test file (a test file cannot import another): `R = 2540`, quarter axial
length `254`, half-angle `0.1` rad, `t = 12.7`, `E = 3102.75`, `nu = 0.3`; node `(i, j)` at `s = 0.1 i/n`, `y = 254 j/n`,
mid-surface `(R sin s, y, R cos s - R)`; held: on `s = 0` `u_x, theta_y, theta_z`; on `y = 0` `u_y, theta_x, theta_z`; on the
hinge `s = 0.1` `u_x, u_y, u_z` (rotations free); `Load::nodal_force (0, 0, -250)` on the apex (a quarter of `P_0 = 1000` N).
Shell4 `n x n`; Shell3 cuts each rectangle along its diagonal from the first to the third corner.

**The limit-load extraction** (registered here, as ADR 0522 D5): `trace` with the load scale `c = |u_lin|^2` (`u_lin` from
`solve_static` of the same model), the length `Ds = 0.03 sqrt(2 c)`, at most 300 points, `NewtonOptions::new(1)` (5 halvings as in ADR
0522 D5); stop at the first point whose load factor is at most `0.8 lambda_max` (`lambda_max > 0` the largest load factor of an
earlier point). `P_lim = P_0 lambda_v`, `lambda_v` the vertex of the parabola through `(w, lambda)` of the point of the largest
load factor and its two neighbours (`w = -u_z` of the apex); if the parabola is not concave or the point is first or last, the
largest load factor (recorded which). The point load on the quarter is a quarter of the whole roof's load, so `P_lim` is the
whole-roof load, the quantity of Table 9a.

**Meshes**: Shell4 and Shell3 `16 x 16` (the registered comparison, B1 and B2: the paper's own mesh size); Shell4 and Shell3
`8 x 8` (the fast test, B5).

### D2. The Hex20 record (c3)

The Hex20 sequence of ADR 0522 (`2213.9, 2184.5, 2167.5` N at `n = 4, 8, 16`, point load; `2203.6, 2186.4` N patch load) is
kept as a record: not rerun (each registered run is made once), its ignored tests in `sa_corot_shell_dyn.rs` left unedited, and
its values entered in the F-610 section of `docs/reference-data.md` beside the published value, with the ratio to `P_ref`
(arithmetic 3, by hand: `2167.5/2226.3 - 1 = -2.64e-2`, `2184.5/2226.3 - 1 = -1.88e-2`, `2213.9/2226.3 - 1 = -5.6e-3`), which
shows the solid falling away from the published value as it is refined, consistent with the singular hinge of ADR 0522's
diagnosis (not a proof of it).

### D3. Tests

A new file `crates/rcfd-fem/tests/sa_roof_published_f610.rs`:

- `b1_b2_roof_16_against_sze_2004` (ignored, the registered record run once; about 35 s by ADR 0522's wall times, ignored
  because the brief asks for heavy runs as records with a fast test): B1, B2, B3 on both traces, records.
- `b5_fast_roof_8_against_sze_2004` (regular): B5 (Shell4 and Shell3 `8 x 8`), B3 on both, records.
- `nc_roof_negative_controls` (regular): NC1 and NC2 on Shell4 `8 x 8`.

## Checks (limits fixed now)

| Id | Clause | Check | Limit | Estimate |
|---|---|---|---|---|
| R0 | c1 | `docs/reference-data.md` holds `P_ref = 0.7421 x 3000 = 2226.3` N with table, figure, URL, "the paper read"; the test's constant equals `0.7421 * 3000.0` | holds | holds |
| B1 | c2 | `|P_lim(Shell4 16)/P_ref - 1|` | `<= 2e-2` | `1.75e-3` (ADR 0522 record) |
| B2 | c2 | `|P_lim(Shell3 16)/P_ref - 1|` | `<= 2e-2` | `2.25e-3` (ADR 0522 record) |
| B3 | c2 | every trace stopped by the rule (not the cap), a negative pivot at a point after the point of `lambda_max` (through the limit point), `P_lim` finite | holds | holds (ADR 0522 A2) |
| B5 | c2 (fast) | `|P_lim(Shell4 8)/P_ref - 1|`, `|P_lim(Shell3 8)/P_ref - 1|` | each `<= 2e-2` | `1.2e-3`; Shell3 `8` not run before, within `1e-2` expected (the Shell3 `16` and Shell4 `8` differ from Shell4 `16` by `5e-4`) |
| H1 | c3 | `git diff 80281ea9 -- crates/rcfd-fem/tests/sa_corot_shell_dyn.rs docs/adr/0522-*` empty at the tip; the Hex20 values and their ratios to `P_ref` in `docs/reference-data.md` | holds | holds |
| NC1 | control | Shell4 `8` with `E x 1.05`: `|P_lim/P_ref - 1| > 2e-2` (the check bites on a 5 percent error), and `|P_lim(1.05 E)/(1.05 P_lim(E)) - 1| <= 1e-3` (static equilibrium of an elastic body scales with `E`; the path points differ through `Ds` and `c`, so only the vertex rule's discretisation remains) | `> 2e-2`; `<= 1e-3` | `4.9e-2`; `<= 1e-4` |
| NC2 | control | Shell4 `8` with the hinge rotations also held (clamped straight edges): `|P_lim/P_ref - 1| > 2e-2` | `> 2e-2` | well above `0.1` (a clamped shallow arch carries far more than a hinged one); not estimated more closely |

Records: `P_lim` of each mesh in N and over `P_ref`; over the vertex value `2231.2` N (arithmetic 1); the apex deflection at the
limit against Table 9a's `11.293` (and the vertex `10.876`); the deflections of each trace at the tabulated rising-branch
loads `P/Pmax = 0.1980, 0.3473, 0.4686, 0.5647, 0.6381, 0.6908, 0.7246` (linear interpolation in the trace) against Table 9a;
points, halvings, wall times; NC2's stop reason.

## Planned mutations (scratch copy by `git archive` of the tip, one at a time, the fast tests only)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | the reference constant `3000` written `3090` (a 3 percent wrong reference) | B5 |
| MU2 | the model thickness `12.7` written `12.0` | B5 |
| MU3 | the hinge rotations held in the main builder (clamped) | B5 |
| MU4 | `arclength.rs` root choice reversed (`score(roots[0]) < score(roots[1])`, ADR 0522 MU6) | B5 or B3 (the trace fails or doubles back) |
| MU5 | the vertex rule disabled (the largest point taken) | not expected (a change below `1e-3`) |

## Decision rule

**done** if R0, B1, B2, B3, B5 and H1 are met and NC1 and NC2 behave as registered; **partial** if B1 or B2 misses (a known
miss with a diagnosis) or a control misbehaves; **todo** if the reference could not be recorded (not the case: it was read).

## Limits (not covered)

- The reference is a 16 x 16 S4R finite-element solution of 2004, not an exact solution; its discretisation error is not known
  here (the paper calls `8 x 8` adequate at this thickness). Agreement within 2 percent of it does not bound the error of rcfd
  against the exact limit load more tightly than the reference's own error plus 2 percent.
- Only the isotropic `t = 12.7` roof; the `6.35` roof (snap-back) and the laminates of Tables 9b to 9f are not covered.
- The geometry reading `2L = 508` is from the figure text and a secondary source (above), not from the drawing.
- Only the limit load is checked; the path at the tabulated points is a record.

## Open questions

- **Q1** (the thin roof, `t = 6.35`, one of Tables 9d to 9f, not read here) as a follow-up: proposed in the addendum only if the run gives
  reason; recommendation: a follow-up, as it exercises the arc length through a snap-back, which this roof does not.

## Addendum 1 (2026-11-17): results

Runs (release, `--nocapture --test-threads 1`, logs in `f610-scratch`): the reference and the tests committed at 328ada56,
then the fast tests once (`run1-fast.log`, 19.6 s) and the registered record `b1_b2_roof_16_against_sze_2004` once with
`--ignored` (`run1-record.log`, 22.6 s). No amendment; no limit changed; nothing rerun. Afterwards, as regressions:
`rcfd-fem --lib` (15 passed), `sa_corot_shell_dyn` (5 passed, 10 ignored), `sa_arclength` (2 passed), `sa_arclength_sphere_f649`
(1 passed, 2 ignored), `docs_site` of `rcfd-cli` (2 passed) (`checks.log`). `baseline_hashes` not run: no library line changed.

| Id | Limit | Measured | Met |
|---|---|---|---|
| R0 `P_ref = 0.7421 x 3000` in `docs/reference-data.md` and the test | holds | `2226.3` N, Table 9a and Figure 9a of the paper read, URL and SHA-256 recorded | yes |
| B1 Shell4 `16 x 16` | `<= 2e-2` | `1.77e-3` (`2222.37` N, below) | yes |
| B2 Shell3 `16 x 16` | `<= 2e-2` | `2.27e-3` (`2221.26` N, below) | yes |
| B3 the four traces | holds | stopped by the rule after 208 or 209 points, no halving, a negative pivot after the limit, vertex fit on each | yes |
| B5 Shell4 `8 x 8`, Shell3 `8 x 8` (fast) | each `<= 2e-2` | `1.22e-3` (`2223.58` N), `3.23e-3` (`2219.10` N) | yes |
| H1 the Hex20 record kept | holds | `sa_corot_shell_dyn.rs`, ADR 0522 and `crates/rcfd-fem/src` unchanged against 80281ea9 (empty diff); values and ratios in `docs/reference-data.md` | yes |
| NC1 `E x 1.05` | `> 2e-2`; `<= 1e-3` | `4.87e-2`; `1.30e-7` | yes |
| NC2 clamped straight edges | `> 2e-2` | `1.75` (`6120` N at the cap of 300 points, no limit point reached, `w = 20.0`) | yes |

Records. Against the vertex value `2231.2` N (arithmetic 1): Shell4 `16` `-3.97e-3`, Shell3 `16` `-4.47e-3`. The apex
deflection at the limit: Shell4 `16` `10.862`, Shell3 `16` `10.831` (against the tabulated point's `11.293`: `-3.8e-2`, `-4.1e-2`;
against the vertex `10.876` of the tabulated points: `-1.3e-3`, `-4.2e-3`; the tabulated point is past the peak of its own
path). The rising branch at the seven tabulated loads, the deflection relative to Table 9a: Shell4 `16` from `-1.8e-3` to
`+1.0e-2`, Shell3 `16` from `-6.4e-3` to `+8.5e-3`, the largest at `P/Pmax = 0.7246` next to the limit, where the slope is
smallest; Shell4 `8` and Shell3 `8` within `1.1e-2` and `2.0e-2`. Wall times: Shell4 `16` 12.3 s, Shell3 `16` 10.3 s, each
`8 x 8` about 3.7 s.

### Mutations (scratch copies of 328ada56 by `git archive`, one at a time, the fast tests; `f610-scratch/mutate.py`, `mutations.log`, `mut-MU*.log`)

| Id | Caught by |
|---|---|
| MU1 `Pmax` 3090 | R0, B5 (`3.03e-2`, `3.23e-2`), NC1 (`1.82e-2`, not above `2e-2`) |
| MU2 `t = 12.0` | B5 (`1.02e-1`, `1.04e-1`) |
| MU3 hinge rotations held | B5 (`1.75`, `1.72`), B3, NC1 |
| MU4 `arclength.rs` root reversed | B5 and the controls (the trace fails) |
| MU5 vertex rule disabled | not caught (B5 `1.23e-3`, `3.24e-3`), as registered |

Four of the four mutations expected to be caught were caught; MU5 was not, as registered. Deviation: the first MU5 run reused
the library compiled for MU4 (the archive restored `arclength.rs` with its commit time, older than the mutated build, so cargo
did not rebuild it); that log is kept as `mut-MU5-stale-lib.log` and MU5 was run again with fresh times (`tar -m`,
`mutations-mu5-rerun.log`). MU1 to MU4 are unaffected (each changed a file after the previous build).

### Deviations

- None in the registered checks. The mutation set-up defect above (MU5 run twice, the first with a stale library).
- fmt (`cargo fmt -p rcfd-fem --check`) and clippy (`-p rcfd-fem --all-targets -D warnings`) clean.

### Decision

**done** by the registered rule: R0, B1, B2, B3, B5 and H1 met, NC1 and NC2 behave as registered. Clauses: (c1) met, the paper
itself read, Table 9a; (c2) met, Shell4 `1.8e-3` and Shell3 `2.3e-3` below the published limit load at `16 x 16`, and within
`3.3e-3` at `8 x 8`; (c3) met, the Hex20 sequence kept unedited and entered beside the published value (`-2.64e-2` at
`n = 16`, falling with refinement). The result agrees with what ADR 0522 recorded (disclosed in the registration).

### Remaining questions (recommendations for the lead)

- **Q1** (the thin roof, `t = 6.35`, with snap-back): a follow-up that traces it through the snap-back against the paper's
  table. Recommendation: propose it (below); it exercises the arc length where the load and the deflection both reverse, which
  the `12.7` roof does not.
- **Q2** (the record status of ADR 0522 A1): its known miss stays as it is (a defect of the registered solid reference); the
  roof clause of F-585 is now met against the published value by this ADR. Recommendation: the lead may move F-585 to done
  with a note pointing here, if the roadmap counts a clause met by a follow-up; otherwise leave F-585 partial.

## For the lead: roadmap entries

> Proposed status: done

> Structural solver, advanced tier. Follow-up of F-585 (ADR 0522). DONE. ADR 0716, 2026-11-17: test-only (no library line changed). The published limit load of the hinged cylindrical roof (R 2540, edges 508, 0.1 rad, t 12.7, E 3102.75, nu 0.3, central point load) read from Sze, Liu and Lo (2004), Table 9a (the authors' copy, HKU Scholars Hub; the paper itself, not a secondary source): the largest tabulated P/Pmax before the snap, 0.7421, Pmax 3000, so 2226.3 N, recorded in docs/reference-data.md with its table, figure and URL (the paper's reference is itself a 16 x 16 S4R Riks solution). The Shell4 and Shell3 quarter models of ADR 0522 traced through the limit point by arc length (the extraction of ADR 0522 D5): Shell4 16 x 16 2222.4 N (1.8e-3 below), Shell3 16 x 16 2221.3 N (2.3e-3 below), 8 x 8 within 3.3e-3 (limit 2 percent); the deflection of the rising branch within 1.1 percent of the table at 16 x 16 (a record). The Hex20 sequence of ADR 0522 kept unedited as the record of the singular hinge (2213.9, 2184.5, 2167.5 N at n 4, 8, 16, falling away from the published value). Negative controls: E x 1.05 gives 4.9e-2 and scales the limit load to 1.3e-7; clamped edges give no limit point. Four of four expected mutations caught.

> Follow-up proposed: Hinged cylindrical roof of thickness 6.35 through its snap-back against the published table. Deps F-610. Acceptance: Every limit not stated here is registered in the ADR before the run. The tabulated path of the isotropic hinged cylindrical roof of Sze, Liu and Lo (2004) for t = 6.35 recorded in docs/reference-data.md from the paper; the Shell4 and Shell3 quarter models traced by arc length through its limit points and its snap-back, the two limit loads within 2 percent of the tabulated values and the deflection of the rising branch at the tabulated loads within a registered limit.
