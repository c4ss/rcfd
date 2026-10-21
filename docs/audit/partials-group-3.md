# Partials audit, group 3

This is a read-only audit of ten roadmap items whose status is `partial` (HEAD 04716a7, 2026-10-21). Each item is graded
only against its `acceptance` field in docs/roadmap.json. Wider scope, from the title or from a "SCOPE (merged review)"
addition, is listed separately as "scope beyond acceptance". It does not lower the grade.

Grades:

- DONE: the acceptance text, as written, is met by a passing test with numeric limits. The test is either non-ignored or
  was re-run here as an ignored test.
- PARTIAL: a real tested slice exists. The unmet clause is named.
- TODO: nothing usable exists.
- OVERSTATED: the roadmap note claims more than the tests show.

How the runs were made: Windows 11, `cargo test --release -p <crate> --test <name> -- --include-ignored --nocapture`, with a
private target directory, on a machine shared with other agents' cargo jobs, so wall times are noisy. "Known misses FAIL"
means that the `#[ignore]`d `*_known_miss` tests fail with the values their notes record. None of them failed in any other way.

Not re-run:

- a full workspace build or test;
- the rcfd-reactor GRI-Mech PSR blowout test `r7_gri_psr_blowout_trends` (ignored; heavy);
- the F-203 known misses `c2_kepsilon_standard_y30_known_miss` and the two `c2_kepsilon_enhanced_*_known_miss` tests;
- the F-106 ignored exploration and controller known-miss tests in `pseudo_control.rs` and `step_laminar.rs`;
- the F-133 R-G/R-H scan known misses (about 9 minutes each).

## Summary table

| id | title (short) | grade | evidence run or read | residual gap (against acceptance) | size | blockers | one wave | user decision |
|---|---|---|---|---|---|---|---|---|
| F-004 | ADRs: state tree, field storage, parallel strategy | PARTIAL | read docs/adr/0001-0003 (each "Status: accepted", on main since the initial import); no test applies | "reviewed": no record of a human review | S | user (the review is a human act) | yes, once the user reviews | yes |
| F-046 | RSM (LRR/SSG) and gamma-family transition | PARTIAL | ran rcfd-turb `rsm` (8 pass; known miss c2d fails) and rcfd-api `rsm_tree` (5 pass; known miss c6 closed-form fails); grep shows no transition or intermittency code; read ADR 0032 | no transition model, so there is no T3A result; the RSM refuses walls, so the swirl benchmark has not been run | L | RSM wall treatment; T3A reference data not in the repo; the swirl benchmark is unnamed | no | yes (name the swirl case; split the item) |
| F-073 | Headless PNG renderer | PARTIAL | ran rcfd-post `render_pixels` 3/3 (r2: 1M-cell render 0.903 s, PNG encode 0.053 s, limit 2 s) | the timing is for the library call only; the CLI has no render subcommand (only `rcfd serve` + `rcfd call render.png`), and no CLI path is timed at 1M cells | S | none | yes | no (or reword to "library") |
| F-106 | Pseudo-transient steady solver | PARTIAL | ran rcfd-fv `pseudo` 3/3, `pseudo_control` 3 pass (12 ignored), `step_laminar` 2 pass, rcfd-api lib `pseudo_transient_modes_are_selectable_from_the_settings_tree` (pass); read docs/bfs-convergence-findings.md, ADR 0023 | the acceptance premise is false: plain SIMPLEC 0.95 converges on the cavity (442 its); the only kept pseudo step test is Re_h 100, where every variant converges; Re_h 400 does not converge with pseudo | S (reword plus one kept step test) | user decision on rewording | yes | yes |
| F-123 | Turbomachinery post-processing | PARTIAL | ran rcfd-post `turbo` 8 pass (known miss t2 polygon fails); grep finds no use of `rcfd_post::turbo` outside its tests and no independent script; read ADR 0054 | no solved pump or compressor case; no comparison with an independent script; no case-file or CLI export hook | M | a converged rotating-machine solution (fan milestone, F-126); whether FAN-01 counts as "pump or compressor" | no | yes |
| F-133 | Blade-passage mesher | PARTIAL | ran rcfd-mesh `cgrid_w35d` p1_p8_fan01 (pass) and `p2_fan01_quality_gates_known_miss` (fails: 83.26 deg, skewness 3.978, aspect 227.2 against 70 / 2.0 / 300); read ADR 0065, 0079, 0083, 0087 | a public blade (FAN-01, CC BY) now yields a structured C-grid passage mesh, but its angle quality misses the project's own gates; "acceptable" has no number in the acceptance; Rotor 37 is not transcribed | L | mesh-topology work (ADR 0087: the index-offset topology has not been tried) | no | yes (put numbers in the acceptance) |
| F-203 | Near-wall treatment catalogue | DONE | ran rcfd-turb `near_wall_catalogue` (ignored heavy tests): k-epsilon Scalable y+ 30 -4.01 / -3.10 %, y+ 1 -4.56 / -3.13 % (limit 5 %); rough k-epsilon -1.01 / +0.25 / +6.34 % (limit 10 %); SST y+ 30 -0.51 / -0.69 % | none against the text, which names no model; scope beyond: SST y+ 1 -8.44 % and SST rough -22 % miss, Enhanced does not converge, no non-equilibrium wall functions | - | - | - | yes (accept "one treatment per clause" or name the models) |
| F-213 | Electric potential and Joule heating | DONE | ran rcfd-fv `electric` 7/7 (H1 Ohm bar phi 1.4e-15, current 1.1e-14; H4 parabola peak-normalised 2.5e-3 at N=20, order 2.000) and rcfd-api `electric_case` 4/4 (tree path, same values) | none; scope beyond: solved-flow coupling, magnetic induction | - | - | - | no |
| F-225 | Reactor networks and mechanism analysis | PARTIAL | ran rcfd-reactor `slices` (K11a \|S+1\| 6.7e-13, K11b 1e-11; known miss K10 fails); read ADR 0027 | no published methane-air PSR-PFR comparison (blowout tau, CO profile), no reference sensitivity ordering; scope: no shock tube, no flame-speed library | M | missing reference data (an unnamed "public validation set"); GRI-Mech licence unverified | no (until data is chosen) | yes (name the dataset) |
| F-235 | Sweep, thin-volume and multizone hex meshing | DONE | ran rcfd-mesh `sweep` 9 pass (known miss s8 annulus skewness fails); rcfd-api `pipe_sweep` s10c pass (centreline 0.1337 %, limit 1 %), s11 pass; `pipe_sweep_compensation` r1_s10d pass (8 layers, 0.1337 %) | none; caveats: 8 layers need the opt-in `solver.simplec_non_orthogonal_compensation`, and the default settings still give 3.33 % (s10 known miss) | - | - | - | no |

Grade counts: DONE 3 (F-203, F-213, F-235), PARTIAL 7, TODO 0, OVERSTATED 0. Two notes are stale in ways that matter:

- F-133: the note does not mention the FAN-01 C-grid waves 34A-37D.
- F-106: the note keeps a wave-5 narrative whose premise its own wave-6 entry refutes.

## Items not DONE

**F-004 (PARTIAL).** ADRs 0001-0003 are merged and marked "Status: accepted". The acceptance also asks for "reviewed", which is
a human act that leaves no trace in the repository. The note's ADR list is stale: 91 ADRs exist now. ADR 0003 says "MPI comes
later". ADR 0058 (distributed parallel) has since happened, so a reviewer should check whether ADR 0003 needs a superseded-by
line.

- Proposed rewording: "ADRs 0001-0003 accepted, each with a reviewer and date line".
- Closing it takes the user a few minutes. No agent can close it.

**F-046 (PARTIAL).** The RSM slice is real and tested: term invariants, homogeneous shear, decay ODEs, the semi-implicit update
and the momentum source, all at 1e-8 or tighter. The two known misses reproduce.

- Neither acceptance clause has any evidence. No transition or intermittency code exists in any crate, so no T3A run is
  possible.
- The RSM refuses walls, so no wall-bounded swirl case can run.
- "RSM swirl benchmark" names no case and no limit, so it cannot be measured. Name a published case, for example a swirling
  pipe or a confined swirl with a profile table, with a numeric limit.
- Split the item: RSM walls plus swirl (L), and gamma or gamma-Re_theta transition plus T3A (L).
- Unverified constants: LRR-IP C2 = 0.6, and Ce1, Ce2 and C_eps (docs/reference-data.md marks them "from memory"). The T3A
  data (ERCOFTAC) is not in validation-data.

**F-073 (PARTIAL, near done).** The library renders a 1M-cell 2-D mesh in 0.903 s here (1.033 s recorded), well under 2 s;
the PNG encode adds 0.053 s.

- The acceptance says "CLI renders". The CLI (crates/rcfd-cli/src/main.rs) has no render subcommand. The only CLI route is
  `rcfd serve` followed by `rcfd call render.png`, which needs a live session, and that route has no test at 1M cells.
- The note's "the criterion measured" is true for the library only.
- Close it by adding a `post`/`render` CLI path with one timed release test on a 1M-cell case (mesh load excluded or
  stated). The alternative is to reword the acceptance to "library render".

**F-106 (PARTIAL).**

- Met:
  - The settings-tree clause is met: off/auto/local/physical modes, `pseudo_factor` and `pseudo_cfl` all run from the tree,
    and the lib test passes.
  - Pseudo-transient converges the Re 1000 cavity: local cfl 5 in 346 its, physical 547, auto 594.
- The comparative clause "where plain SIMPLE needs lower relaxation" is false after the wave-6 `h_from_solution` fix. The
  kept test asserts that plain SIMPLEC 0.95 converges (442 its).
- For the step, the only kept test is Re_h 100. Both variants converge there (SIMPLE 240 its, pseudo 135). The Re_h 200
  pseudo result (607 its) is only in an ignored exploration test, and Re_h 400 does not converge with pseudo.
- The automatic controllers (`pseudo_control = ser | ser2`) fail all their registered checks (12 ignored known misses).
- Proposed rewording: "cavity Re 1000 and BFS Re_h 200 converge to residual 1e-6 with alpha_u = 1 in local, physical and
  auto modes; modes and factor selectable in the tree".
- With that rewording, one kept Re_h 200 step test closes it (S).

**F-123 (PARTIAL).** The library delivers:

- constant-span and streamwise surfaces;
- pitch and mass-flow averages;
- head, pressure-ratio and efficiency formulas;
- CSV/HTML reports.

It is checked only against closed forms, at 1e-13. Nothing calls `rcfd_post::turbo` outside its own tests. No solved
turbomachine field has been post-processed, and no independent script exists.

- Blocker: a converged rotating-machine solution. FAN-01 is the only public machine in the repo and is still in the
  milestone loop.
- The user should decide whether a low-speed fan counts as "a public pump or compressor case".
- One part could be closed now (S-M): an independent numpy script on an exported surface from any rotating-frame solution,
  plus a CLI or case-file export hook.

**F-133 (PARTIAL; the note is stale).**

- What has changed since the note:
  - The note says there is no public blade. In fact the FAN-01 axial fan (CC BY 4.0, vendored in wave 34D) builds a closed,
    periodic, exact-stack C-grid passage mesh (p1_p8 pass).
- What is still missing:
  - Its quality misses the project gates. Measured here: non-orthogonality 83.26 deg, skewness 3.978, aspect 227.2 (limits
    70 / 2.0 / 300); p99 non-orthogonality is 70.35 deg.
  - Four waves of C-grid changes (ADRs 0079, 0083, 0087) have not cleared it. ADR 0087 says the index-offset topology has
    not been tried.
  - Rotor 37 numbers are not transcribed; validation-data holds only NASA TP-1337.
  - Curve-file input does not exist.
- Problems with the acceptance itself:
  - "Acceptable angle/expansion ratios" has no number.
  - The project gates on non-orthogonality, skewness and aspect ratio. None of the tests checks the expansion (volume)
    ratio the acceptance names.
- Proposed rewording: "FAN-01 (or Rotor 37) passage mesh with max non-orthogonality <= 70 deg, skewness <= 2, aspect <= 300
  and adjacent volume ratio <= N".
- Size: L.

**F-225 (PARTIAL).** The sensitivity ranking (|S+1| 6.7e-13) and the PSR-into-plug network (1e-11 against closed forms)
pass. The library also has a PSR blowout search (`psr_blowout`, F-112), but it is exercised only by an ignored GRI test that
was not re-run here.

- Missing: a published methane-air PSR-PFR case with its blowout residence time, CO profile and sensitivity ordering.
- "The public validation set" is unnamed, so the acceptance cannot be measured until a dataset is chosen.
- The GRI-Mech 3.0 licence is unverified (docs/reference-data.md).
- Scope beyond acceptance: shock tube and premixed flame-speed library, both absent.
- Size: M once a dataset is named.

**DONE items, for the record.**

- F-203: the numbers in the note reproduce to the printed digit.
  - The text names no model. k-epsilon Scalable meets both smooth clauses and k-epsilon Standard plus roughness meets the
    rough clause.
  - SST misses at y+ 1 (-8.44 %) and rough (-22 %). Non-equilibrium wall functions and a converging Enhanced treatment are
    title scope.
  - The Prandtl-Schlichting and fully-rough laws are confirmed from secondary sources only (docs/reference-data.md).
- F-213 meets both clauses, including through the case tree.
- F-235 meets both clauses: all-hex with exact layer counts, and a centreline error of 0.13 % against 1 %. The 8-layer pipe
  needs an opt-in compensation flag.
