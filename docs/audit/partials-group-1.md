# Partials audit, group 1

Read-only audit of ten roadmap items with status `partial` (HEAD 07ae2db, 2026-10-21). Each item is graded against its
`acceptance` field in docs/roadmap.json only. Wider scope from the title or a "SCOPE (merged review)" addition is listed
separately as "scope beyond acceptance". It does not lower the grade, but the paragraph proposes how to record it.

Grades: DONE = the acceptance text, as written, is met by a passing non-ignored test (or a reproducible script) with numeric
limits; PARTIAL = a real tested slice exists, with the unmet clause named; TODO = nothing usable; OVERSTATED = the roadmap
note claims more than the tests show.

How the runs were made: Windows 11, `cargo test --release -p <crate> --test <name> -- --include-ignored` with a private
target directory, on a machine shared with other agents' cargo jobs (so wall times and speed-ups are noisy). "Ignored records
FAIL" below means the `#[ignore]`d known-miss or superseded tests fail as their notes say. Not re-run: a full
`cargo build/test --workspace` on any OS, any Linux run, and any hosted CI.

## Summary table

| id | title (short) | grade | evidence run or read | residual gap (against acceptance) | size | blockers | one wave | user decision |
|---|---|---|---|---|---|---|---|---|
| F-001 | Cargo workspace layout | PARTIAL | ran rcfd-cli `workspace_graph` (3/3 pass); read docs/architecture.md and .github/workflows/ci.yml | no evidence of a hosted CI run on Windows and Linux | S | user: push and enable hosted CI (agents may not push) | no | yes |
| F-044 | Turbulence-model plugin interface, coefficients in settings | DONE | ran rcfd-api `turb_models` 8/8 plus ignored R2 (pass; switched vs fresh 0, 1.9e-11, 1.6e-11, 1.9e-12) | none; scope beyond: SST override, expression/UDF override, laminar switching | - | - | - | yes (rewrite acceptance or split the SCOPE addition) |
| F-063 | Dynamic mesh: rigid, smoothing, AMI | PARTIAL | ran rcfd-fv `ale_gcl` 4/4, `ale_flows` 4 pass plus ignored records, rcfd-api `dynamic_mesh` 5 pass plus 1 ignored known miss; read ADR 0033, ADR 0090 | no oscillating-cylinder case; rotating case (G6) does not assert mass conservation; whether "rotating toy" needs a sliding interface is undefined | M (S for the G6 mass assertion) | none for the cylinder; AMI (if required) is L | yes for the cylinder and G6; no for AMI | yes (whether AMI is required) |
| F-102 | Adaptive refinement and polyhedral conversion | PARTIAL | ran rcfd-mesh `adapt` 12/12, `polydual` 3/3, rcfd-fv `amr_solve` (A4, A4b pass, A4 known miss fails at 3.71); read ADR 0054 | adapted lid-driven cavity never run; the acceptance has no numeric limit | M | needs a solver-loop or test hook to adapt a SIMPLE run | yes | yes (reword with limits) |
| F-119 | Interchange formats STL, Tecplot, Plot3D, XDMF | DONE | ran rcfd-io `interchange` 17/17, `plot3d_public` 1/1 (sample present, not skipped); ran scripts/check_interchange.py with VTK 9.6.2: 20 PASS, 0 FAIL | none if VTK readers count as "third-party viewers"; scope beyond: HDF5 heavy data, TMR grids | - | - | - | yes (accept VTK readers as viewers) |
| F-131 | Surface sizing and remeshing from STL | PARTIAL | ran rcfd-mesh `surface_remesh` 8 pass plus 1 known miss, `tetmesh` p1 (pass); read ADR 0030 | no STL file in the pipeline (in-code generator); tet quality only `check_mesh().is_ok()`, measured min dihedral 3.3 deg, no limit | S (STL I/O plus limits); M if the quality needs raising | quality partly belongs to F-132 | yes | no |
| F-201 | Zone sources and fixed values | DONE | ran rcfd-api `zone_sources` 8/8 (E1b error 2.44e-4, order 2.000; E4b BDF2 orders 2.07/2.03; E3a deviation 0) | none; scope beyond: mass, momentum and turbulence sources, variable-density and compressible engines | - | - | - | yes (split the title scope into a new item) |
| F-211 | Advection and transient scheme additions | PARTIAL | ran rcfd-fv `schemes_wave13` 6 pass plus ignored records, rcfd-api `schemes_wave13` 2/2; read ADR 0016 | order of compressive(1.5) and compressive(2) shown only in a failing ignored test (orders 1.98/1.99 in band; the deferred-correction iteration stalls) | S | none | yes | no |
| F-223 | Stiff chemistry acceleration | OVERSTATED | ran rcfd-reactor `accel` 12 pass plus ignored records, rcfd-api `chemistry_accel` 4/4; read ADR 0053 | the "speed-up 118 for cell ISAT" claim is a speed-up on a wrong answer; no comparison against a published ignition delay | S (rewording) / L (published reference) | missing reference data (published GRI-Mech 3.0 delay) | yes if reworded | yes |
| F-233 | Mesh-quality measure catalogue | PARTIAL | ran rcfd-mesh `measures` 17 pass plus 1 superseded record, rcfd-api `mesh_quality` 1/1; read ADR 0018 | warp, ortho-skew and size change not asserted on a degenerate cell; the four older measures not asserted on the tetrahedra | S | none | yes | no |

Counts: DONE 3, PARTIAL 6, OVERSTATED 1, TODO 0.

## Items not DONE

**F-001 (PARTIAL).** Measured: `workspace_graph` passes (crates exist, graph acyclic, layering matches docs/architecture.md).
The documentation and acyclicity clauses are met. The CI clause is not shown. `.github/workflows/ci.yml` defines fmt, clippy and
an ubuntu/windows test matrix. The local `origin/main` ref points at 5e300f9 (wave 32), so a push did happen at some point. The
GitHub Actions page returns 404 without authentication (private or absent), and `gh` is not installed, so whether a hosted run
exists is UNVERIFIED. The note's "never run on a hosted CI" is unconfirmed either way. I did not re-run the WSL Linux or full
workspace builds. Closing this needs the user to push and read the Actions result. The acceptance is reasonable.

**F-063 (PARTIAL).** Measured: GCL G1 and uniform-flow G2 pass. G6 (rigid rotation of an annulus) asserts velocity 3.3e-3 W r_o
(limit 2e-2) and pressure 5.0e-3 of the range (limit 5e-2). It prints continuity 4.1e-14 but never asserts it. T2 piston prints
continuity 3.5e-12, also unasserted. The note is stale in both directions:
- The `g4b_..._known_miss` record now passes (moving vs fixed mesh difference 1.84e-14), so the cyclic-face lag the note cites
  no longer reproduces.
- ADR 0090 (F-230) delivered the `InverseVolume` stiffness variant (S1-S3 met) and an ALE step for the compressible solver.
  The note still lists both as not done.

The piston-cell velocity miss (2.9e-4 vs 1e-6) still fails as recorded. Residual against the acceptance:
- (a) an oscillating cylinder case (rigid body in a smoothed mesh) with a registered mass-conservation limit;
- (b) a mass assertion on the rotating case (S);
- (c) a decision whether the "rotating toy case" must use a sliding interface (AMI is not implemented; L).

Proposed rewording: "Oscillating cylinder (smoothed mesh) and a rotating annulus (rigid mesh rotation) conserve mass to 1e-10
per step and satisfy the GCL to round-off; a sliding-interface rotor case is a separate item."

**F-102 (PARTIAL).** Measured: adaptation operators and indicators pass. On a manufactured tanh-front diffusion problem, A4b
gives 1330 cells against 4096 at E_a/E64 = 1.27, but the registered A4 known miss (3.71 against 3) still fails. The acceptance
names the lid-driven cavity, which has never been run on an adapted mesh. There is no case-file or solver-loop hook. The
acceptance has no number. Proposed rewording, reusing the ADR 0054 A4 limits: "Re 100 lid-driven cavity adapted from a 32x32
solution: centreline u and v within 3x the error of a uniform 128x128 solution against Ghia et al. with at most half its cells."
Size M, one wave. The Ghia reference values must be checked against the published table.

**F-131 (PARTIAL).** Measured: curvature sizing on sphere, torus and cylinder-in-box passes. `p1_pipeline_cylinder_in_box`
remeshes and tetrahedralizes, and asserts closed manifold, volume error at or below 1e-10, Euler 2 and `check_mesh().is_ok()`.
Unmet clauses:
- "a closed STL": the surface comes from the in-code `cyl_in_box()` generator and never goes through STL write/read.
- "passes quality checks": the tetrahedral quality is only printed. Measured min dihedral 3.34 deg, max 174.0 deg, 83 bad
  tetrahedra left, radius-edge fraction ok 0.927.

Closure:
- STL round trip in the test (S);
- registered quality limits, for example min dihedral and radius-edge (S to write; M if the mesh must be improved to meet
  them, which overlaps F-132).

The torus volume known miss (4.18 vs 3 percent) still fails as recorded. Literature attributions are UNVERIFIED (note).

**F-211 (PARTIAL, narrow).** Measured: B1 blend 0/1 differences exactly 0; O1c orders 2.0000 for central, blended(1),
gamma(0.2) and bounded_central and 0.98-1.003 for the first-order members; W1b square wave: every limited scheme stays inside the
bounds (largest signed excursion -3.1e-5 for gamma; negative means inside; limit 1e-3), while the central control overshoots
0.147. T1 temporal orders and T2 bounded BDF2 (overshoot 1.2e-9 vs
2.5e-2) also pass. Decision: I count only passing tests. compressive(1.5) and compressive(2) reach orders 1.98/1.99 only inside
`o1c_known_miss_compressive_convergence`, which fails its convergence rule (limit cycle of the deferred correction). So their
"nominal order" clause is unmet. Closure (S): a newly registered test that measures the order of those two schemes with a
convergence rule suited to a limit cycle (or a direct non-linear solve), with the old record kept. Literature references are
UNVERIFIED (docs/reference-data.md).

**F-223 (OVERSTATED).** Measured:
- DRG: static DRG on GRI-Mech 3.0 H2/air keeps delays within 3.6e-6 of the direct run at about 9x wall speed-up (D2). Dynamic
  DRG gives delay 1.58e-3 relative at about 5.7x (D4).
- ISAT, 0D H2/air ensemble (I4): delay difference 1.7e-3 but wall speed-up 0.22, a slowdown.
- The roadmap note's "speed-up 118 for cell ISAT on repeated states" comes from I5 (measured 115-126 here). In that run the
  ISAT cells never ignited: 0 against 17 cells above 1500 K, profile error 1222 K. It is a speed-up on a wrong answer.

ADR 0053 states the I4 slowdown honestly; the roadmap note omits it. Second gap: "published fuel/mechanism pair reproduced
within 5 percent" is met only against the same mechanism integrated directly. A published GRI-Mech 3.0 ignition delay was not
available (reference-data), so this is blocked by missing reference data. User decision, with proposed rewording: "accelerated
0D ignition delay of a published mechanism (GRI-Mech 3.0 H2/air) within 5 percent of its direct integration, with a measured
speed-up above 1". DRG already meets that wording. Known misses I3 (1.27e-3 vs 1e-3), I4 drift (2.4e-9 vs 1e-9) and I5 still
fail as recorded. Speed-ups were measured on a shared machine.

**F-233 (PARTIAL).** Measured: all closed-form checks pass (unit cube, right-corner and regular tetrahedra, sheared and warped
hexahedra, size change, invariance). The note's self-assessment matches the code:
- `q2a_sliver_hexahedron` (the only degenerate cell) asserts edge ratio, angles and inverse orthogonality, but not warp,
  ortho-skew or size change;
- `q2d` uses a hand-built face list, not a cell;
- the four existing measures are not asserted on the tetrahedra.

Closure (S): extend `q2a` and the tetrahedron tests with the missing closed forms. Attributions are UNVERIFIED (note).

## DONE items: notes

- **F-044.** K1 SA cb1 +20 percent changes max nut by 0.153. K2 k-epsilon c2 +10 percent changes it by 0.287. Explicit
  defaults are bit-identical. R1 shows the flow and iteration counter kept across switches. R2 (ignored, about 58 s in release)
  passes, and its numbers match the note. Scope beyond the acceptance: SST coefficients rejected when non-default;
  expression/UDF overrides; laminar switching. Either reword the acceptance to include them or move them to a new item.
- **F-119.** VTK 9.6.2 readers (`vtkSTLReader`, `vtkTecplotReader`, `vtkXdmfReader`) reproduce cell counts, volumes and cell
  data `p` for hex, tet, prism and a mixed hex/pyramid/tet/prism set (20 PASS, 0 FAIL; GLB SKIP is not this item). The blunt-fin
  grid is read as 40x32x32 with VTK's bounds. The third-party check is a manual script, not CI. Scope beyond: HDF5 heavy data;
  the TMR flat-plate and bump grids of the SCOPE addition. The sample's SHA-256 in docs/reference-data.md was not re-checked by me.
- **F-201.** All three clauses are met by passing tests. The two superseded ignored records (`e1_...`, `e5_...`) also pass
  now. Scope beyond: the title's "every transported quantity" (mass, momentum by expression, turbulence quantities,
  compressible). Propose a new item for those and marking F-201 done.
