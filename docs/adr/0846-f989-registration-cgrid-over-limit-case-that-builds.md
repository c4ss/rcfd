# ADR 0846: F-989 registration, an over-limit C-grid fan case that builds, with a small build of the leading-edge fan first

REGISTERED 2026-11-28, written before any code of F-989 under this registration and before any run of its checks.

Written by Claude Opus 5.5 (`claude-opus-5-5`), as a background agent of the lead. It answers the brief
`docs/adr/briefs/f989-cgrid-over-limit.md` ([the brief](briefs/f989-cgrid-over-limit.md)) and follows
`docs/process/agent-rules.md`, version 5 ([the rules](../process/agent-rules.md)). Branch `f989-cgrid-over-limit` at the
brief's commit ecac9eec (equal to `origin/main`). Scratch folder `E:/projects/cfd/w49i-work/f989-cgrid-over-limit-scratch`.
It is a further registration under F-989 (rule 13): it adds no acceptance clause; it decides the case
[ADR 0844](0844-f989-registration-case-facts-estimate-for-mesh-files.md) left not decided (its Q2, answered (a) by the lead
on the maintainer's standing instruction of 2026-11-26).

## The item

F-989's acceptance (read from `docs/roadmap.json` by a script): "Every limit not stated here is registered in the ADR before
the run. mesh_preview::estimate gives the cell count of a mesh file (each format the engine imports) and of every generator
that has no estimate, read without building the mesh; each count equals the built count over a registered set of cases,
checked by two routes; case.facts with a limit refuses by the estimate for each, within the time and memory limits of ADR
0812, with the reply of FactsError::TooLarge."

Its gap after ADR 0844: the over-limit `axial_fan_sector` C-grid case is not decided (H11 and H11b of ADR 0844 folded at
the leading-edge fan, so no reference build was timed).

## What was read

AGENTS.md, the rules and the brief, in full. ADR 0844 in full (its set H1 to H11, the C-grid formula, amendment 2, the
results and Q2). Its test file `crates/rcfd-api/tests/case_facts_estimate_files_adr0844.rs` in full. From ADR 0844's scratch
folder: `logs/heavy.log` lines 190 to 213 (H11) and `logs/heavy-a2.log` lines 1 to 28 (H11b), `heavy.sh` and the sampler
`sample-peak.ps1`. [ADR 0812](0812-f823-registration-case-facts-refuses-by-the-counts-estimate.md), the rows E2 and E3 of
its table (lines 148 and 149) and amendment 2 (the sampler read once after exit). `crates/rcfd-api/src/mesh_preview.rs`
lines 170 to 205 (`fan_cells`, the C-grid count). `crates/rcfd-api/src/facts.rs` lines 368 to 396 (`case_facts`: the
refusal by `cell_estimate` at 388 to 392, the build at 393). `crates/rcfd-api/src/fan_case.rs` by search: the schema counts
(lines 59 to 86: `n_span`, `n_gap` at least 1, no upper bound; `c_grid_construction` default `pursuit`), `cgrid_spec_of`
(258 to 290) and the dispatch (296 to 311). `crates/rcfd-mesh/src/fan_cgrid.rs`: `capped_cells` and `graded_two_sided`
(266 to 356), `build_layer`'s head (1098 to 1130), `folded_quad` (1850 to 1903), `build_cgrid_mesh_with` (1952 to 2200:
the input checks, the radii, the construction configs, the per-layer build and the fold check at 2175 to 2183), the
assembly's tangle and volume checks (2366 to 2400). `cases/axial_fan_cgrid/axial_fan_cgrid.toml` in full. The
[tolerance table](../process/tolerances.md), rows "Same discrete problem by two routes", "Run time or memory, as a budget",
"Bit-identity of default output". About 130,000 bytes.

## Why H11 and H11b folded, and why raising only `n_span` and `n_gap` should not (first principles, from the code read)

The fold check (`fan_cgrid.rs` 2175) is planar: `folded_quad` (1850 to 1903) takes each radial layer `k` and asks that
every quad of the C block (and with smoothing the F block) has a positive area in the `(z, r θ)` plane of that layer. Each
layer's planar grid is built by `build_layer(fan, spec, lay, k, r, h1, ..)` (1098), whose `k` is used only in the text of
its errors (line 1108); its grid depends on the radius `r` (through `fan.station_at(r)` and the pitch `r · 2π / n_blades`)
and on the planar controls `n_up`, `n_chord`, `n_down`, `n_side`, `n_nose`, `n_core`, `front_gap`, `n_bl`, `growth` and the
first height `h1`. The default construction (`pursuit`, `fan_case.rs` 297 to 311) makes one attempt, without smoothing.
H11 and H11b changed the planar controls from the shipped case (`n_up` 40, `n_chord` 60, `n_down` 40, `n_side` 70,
`n_core` 4, and `n_nose` 10 or 4) and both folded at layer 0, the hub (`r = 0.1`), in the C block (line 110 row 34; line
39 row 12). The shipped case (ADR 0844's G10: `n_up` 8, `n_chord` 16, `n_down` 8, `n_side` 10, `n_nose` 4, `n_core` 1,
`front_gap` 0.3) builds at every one of its 22 layers, `r` from 0.1 to 0.31.

`n_span` and `n_gap` enter only through the radii: `graded_two_sided` (302 to 356) puts `n_bl = 2` cells `h1 g^m`
(`h1 = 10⁻³`, `g = 1.2`) at each end of the span (length `r_tip − r_hub = 0.2`) and of the gap (`tip_clearance = 0.01`), and
between them `n − 4` cells that grow from `h1 g² = 1.44·10⁻³` with a cap, or are uniform when that first cell is not below
their mean (line 327). The radii are increasing in every case (positive sizes, summed). Raising `n_span` and `n_gap` keeps
every planar control of G10 and only adds layers at more radii inside the same range `[0.1, 0.31]`. A fold can then come
only from a radius between those G10 sampled where the planar grid folds; the blade's station varies smoothly with `r`
(linear chord, stagger, thickness, camber, sweep and lean between hub and tip in the shipped case), so none is expected. The
volume check (2394) asks positive volumes; with increasing radii and unfolded planar quads, the hexahedra between two layers
have positive volume.

This is checked in two steps before the over-limit build: S1, the planar controls and the `n_nose` and `front_gap` of the
over-limit case at G10's own layers (G10 itself, built in ADR 0844, built again here as the first check of this record),
and S2, the same planar controls with 20 times G10's layers, which puts the span and the gap in the same branch of
`graded_two_sided` as the over-limit case (uniform middle cells) and samples 421 radii. A negative control N1 shows the
check sees a fold where one is: H11b's planar controls at G10's 21 layers must fold at layer 0, as H11b did with 140 layers,
which also checks that the fold does not depend on `n_span` and `n_gap`.

## Arithmetic done beforehand

The C-grid count (ADR 0844, `fan_cgrid.rs` 76 to 88 and 2200 to 2270; `mesh_preview.rs` 185 to 191): per layer
`2 · (n_down + n_chord) · n_side + 2 · n_nose · n_up`, over `n_span + n_gap` layers, plus `n_core · n_chord` core cells in
each of the `n_gap` gap layers (with the blade). With G10's planar controls a layer has `2·24·10 + 2·4·8 = 544` cells and a
gap layer 16 more: `N(n_span, n_gap) = 544 · (n_span + n_gap) + 16 · n_gap`.

- S1 (G10): `544·21 + 16·7 = 11,424 + 112 = 11,536`.
- S2 (G10, `n_span` 280, `n_gap` 140): `544·420 + 16·140 = 228,480 + 2,240 = 230,720`.
- HC (G10, `n_span` 2440, `n_gap` 1220, the shipped ratio 2:1 kept; with that ratio `N = 824 · n_span`, over 2,000,000 from
  `n_span` 2428; 2440 is the next multiple of 20): `544·3660 + 16·1220 = 1,991,040 + 19,520 = 2,010,560`, over the
  default service limit 2,000,000.
- N1 (H11b's planar controls, `n_span` 14, `n_gap` 7): per layer `2·100·70 + 2·4·40 = 14,320`;
  `14,320·21 + 4·60·7 = 300,720 + 1,680 = 302,400` (the estimate; the build must fail).

The grading branch: in the span `l_mid = 0.2 − 2·(10⁻³ + 1.2·10⁻³) = 0.1956`; its mean middle cell is 0.0196 for G10 (10
middle cells: graded), 7.09·10⁻⁴ for S2 (276) and 8.03·10⁻⁵ for HC (2436), both below `1.44·10⁻³` (uniform). In the gap
`l_mid = 0.0056`: 1.87·10⁻³ for G10 (3: graded), 4.1·10⁻⁵ for S2 (136) and 4.6·10⁻⁶ for HC (1216) (uniform).

## The cases (fixed now)

| Id | Case | Count | Expected |
|---|---|---|---|
| S1 | `cases/axial_fan_cgrid/axial_fan_cgrid.toml` as shipped (`n_nose` 4, `front_gap` 0.3, read from the case) | 11,536 | builds |
| S2 | S1 with `mesh.fan.n_span` 280, `mesh.fan.n_gap` 140 | 230,720 | builds |
| N1 | S1 with `n_up` 40, `n_chord` 60, `n_down` 40, `n_side` 70, `n_core` 4 (H11b's planar controls) | 302,400 | the build fails, folded at layer 0 |
| HC | S1 with `mesh.fan.n_span` 2440, `mesh.fan.n_gap` 1220 | 2,010,560 | builds (heavy) |

## Tests (fixed now)

New file `crates/rcfd-api/tests/case_facts_estimate_cgrid_adr0846.rs` (header names ADR 0846 and F-989, rule 12). A new
file, not additions to ADR 0844's, so that its committed mutation patch has a stem of its own; ADR 0844's file is not edited.

- `s1_leading_edge_fan_small`: S1's `mesh.fan.n_nose` is 4 and `front_gap` 0.3; `sim::build_mesh` succeeds;
  `cell_estimate` equals the registered count (route B) and the built `n_cells()` (route A).
- `s2_dense_layers_small`: the same for S2, and `case_facts(S2, NotDefault, Some(n))` is `Ok` with `cells == n` and
  `Some(n − 1)` gives `TooLarge { cells: n, limit: n − 1 }`, `n` the built count.
- `n1_the_fold_is_planar` (negative control): `sim::build_mesh(N1)` fails with a diagnostic at `mesh.fan.front_gap` whose
  message contains "the line family is folded at layer 0"; `cell_estimate(N1)` is `Some(302_400)`.
- Heavy, ignored (each run once, rule 7): `rc_refusal_alone` (`case_facts(HC, NotDefault, Some(2_000_000))` timed, T_r,
  must be `TooLarge { cells: 2_010_560, limit: 2_000_000 }`) and `hc_refusal_and_build` (the same refusal timed, then
  `sim::build_mesh(HC)` timed, T_b, which must succeed with `n_cells() == 2_010_560`; asserts T_r ≤ 1 s and
  T_r / T_b ≤ 0.5; prints both), as ADR 0844's `rk_` and `hk_`.

## Table of checks

| Id | Clause | Check | Limit | Source of the limit | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|---|
| C1 | "each count equals the built count ... checked by two routes" (the leading-edge fan of the over-limit case's controls, small) | `s1_` | builds; estimate, registered count and built count equal | the acceptance; tolerance table row "Same discrete problem by two routes" (exact: integers) | equal | exact | code verification |
| C2 | as C1, at the over-limit case's grading branch | `s2_` | as C1; and refused at `n − 1` with `TooLarge { n, n − 1 }`, not at `n` | the acceptance | equal; passes | exact | code verification |
| C3 | the check sees a fold (negative control) | `n1_` | the build fails, folded at layer 0; estimate 302,400 | ADR 0844 amendment 2 (H11b's fold at layer 0) | passes | exact | code verification |
| C4 | "case.facts with a limit refuses by the estimate ... with the reply of FactsError::TooLarge", "each count equals the built count" (HC) | `rc_`, `hc_` | `TooLarge { 2,010,560, 2,000,000 }`; HC builds with 2,010,560 cells | the acceptance | passes | exact | code verification |
| C5 | "within the time limits of ADR 0812" | `hc_` | T_r ≤ 1 s and T_r / T_b ≤ 0.5 | ADR 0812 E2 (line 148); tolerance table row "Run time or memory, as a budget" | T_r 0.5 ms (H11b's refusal 0.35 ms); T_b 40 s (H10 22.6 s for 2.02 million H-grid cells; the C-grid's planar build per layer and polyhedral assembly add to it) | 2000; 40,000 (ratio 1.3·10⁻⁵) | unchanged default output (the row's kind) |
| C6 | "... and memory limits of ADR 0812" | peak working set of `rc_` alone (P_r) and `hc_` alone (P_b), by ADR 0812 amendment 2's sampler | P_r ≤ 200 MB and P_r / P_b ≤ 0.5 | ADR 0812 E3 (line 149); the same row | P_r 7 MB (H11b 7.0 MB); P_b 4 GB (ADR 0844's H-grid H10 3.9 GB) | 28; 280 | unchanged default output |
| K7 | existing replies unchanged | ADR 0844's file `case_facts_estimate_files_adr0844` (its fast tests) | passes as on main | rule 3; tolerance table row "Bit-identity of default output" | passes | exact | unchanged default output |
| BH | rule 3 | `cargo test -p rcfd-api --release --test baseline_hashes` | passes as on main | rule 3 | passes | exact | unchanged default output |
| R7 | rule 7 | `cargo fmt -p rcfd-api --check` (or `rustfmt --check` of the new file on os error 206); `cargo clippy -p rcfd-api --all-targets -- -D warnings`; `cargo test -p rcfd-api --release --lib`; the new file's fast tests | each exit 0 | rule 7 | passes | exact | code verification |
| DS | rule 7 | `cargo test -p rcfd-cli --release --test docs_site` | passes | rule 7 | passes | exact | code verification |
| P1 | rules 7, 9 | `python scripts/roadmap_proposals.py check docs/roadmap/proposals/0846-F-989.json`; `python scripts/process_report.py --check` | valid; clean but for a generated file only not current | rules 7, 9 | passes | exact | code verification |
| MP | rule 16, the lead's instruction | `git apply --check` of every committed `*.mutation.patch` of the repository at the final commit | each applies | rule 16 | each applies (no library file changes) | exact | code verification |
| A1 | files owned | `git diff --stat ecac9eec..HEAD` | the new test file and its patch, this ADR, the proposal file | the brief | as the limit | exact | code verification |

No library, generator or solver code changes: the count and the refusal are ADR 0844's code, unchanged.

### Set-up of the runs (fixed here)

Every cargo command with `CARGO_TARGET_DIR=E:\projects\cfd\w49i-work\f989-cgrid-over-limit-scratch\target` and
`CARGO_BUILD_JOBS=8`, release profile. The host CPU (mean `LoadPercentage` of `Win32_Processor`) is recorded before and
after each timed run, and the free physical memory before the heavy runs. Order: R7 (the fast tests `s1_`, then `s2_`, then
`n1_` give C1, C2 and C3: S1 first, the small build before anything over the limit), K7, BH. The heavy runs only if `s1_`
and `s2_` passed: `rc_refusal_alone` and then `hc_refusal_and_build`, each once as its own process of the test executable
(`--exact <name> --ignored --test-threads=1 --nocapture`) under ADR 0812 amendment 2's sampler (a copy of ADR 0844's
`sample-peak.ps1` in this record's scratch folder that also records the CPU after exit). If `s1_` or `s2_` fails, the heavy
runs are not made, C4 to C6 are not decided, and the case's parameters and the diagnostic are reported (no generator code
changes, the brief). If `hc_`'s build fails, C4 to C6 are recorded as not decided for HC, not as met.

## Negative controls and planned mutations

The negative control is N1 (`n1_`). Mutations, each once on a `git archive` copy of the code commit, by `cargo test
--release -p rcfd-api --test case_facts_estimate_cgrid_adr0846` (the fast tests), with a target folder of the scratch:

| Id | Mutation | Caught when |
|---|---|---|
| MU1 | committed: the C-grid's F-block cells `2 · n_nose · n_up` counted as `n_nose · n_up` (`mesh_preview.rs`, `fan_cells`) | `s1_` and `s2_` fail (estimate below the built count) |
| MU2 | the C-grid's core cells in every layer (`k0` ignored) | `s1_` and `s2_` fail |
| MU3 | the C-grid's gap layers left out of the estimate (`n_k = n_span` in the C-grid branch) | `s1_` and `s2_` fail |
| MU4 | the planar fold check of `build_cgrid_mesh_with` removed (`fan_cgrid.rs` line 2175, the `if let` never taken) | `n1_` fails |

MU1 is committed as `crates/rcfd-api/tests/case_facts_estimate_cgrid_adr0846.mutation.patch`, its first line
`# expect-fail: cargo test --release -p rcfd-api --target-dir target/mutation --test case_facts_estimate_cgrid_adr0846 s1_`.

## Size

Size (agent): done; 0; 1; 0; 130000

(The heavy run: the two processes `rc_` and `hc_`, one reference build of about 2 million cells.)

## Decision rule

**Done** for F-989 if the over-limit C-grid case HC builds, its estimate equals the built count (C4), its refusal is within
ADR 0812's limits (C5 and C6) and each mutation MU1 to MU4 is caught (the brief's rule); with C1 to C3, K7, BH, R7, DS, P1,
MP and A1 holding as the rules ask. **Partial** otherwise, with every miss or case not decided recorded. ADR 0844's checks
for the other 28 cases and ten over-limit cases stand as recorded there.

## What is not covered

A C-grid with other planar controls than the shipped case's over the limit (H11 and H11b fold at the hub; the reason is
ADR 0844 amendment 2's and is not a matter of the count). The `smoothed`, `camber_seam` and `camber_seam_h` constructions
over the limit (the estimate does not depend on them, the code read; ADR 0844's small cases cover the default). The time on
other hosts.

## Open questions for the maintainer

None at registration.
