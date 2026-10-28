# ADR 0116: Wave 45B, a sector generator whose walls are exact surfaces of revolution (MRF with cyclic seams through the case tree)

Status: **ACCEPTED (registered 2026-10-26)**. The limits and decision rules below were fixed before any registered test ran. The
house rules of ADRs 0110-0113 apply:
* a missed limit becomes an ignored `*_known_miss` test with the limit intact;
* a defective set-up gets a new corrected test, registered here as an amendment before it runs;
* defaults stay bit-identical, and `crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded.

Written at main e65b4ea (worktree `<scratch dir>`, branch `w45b`, target `<target dir>`). Clean-room: first
principles and our own ADRs only. Parents: ADRs 0065 (S4), 0109, 0113.

## User decisions (taken before this wave started)

* Scope **B0**, the generic exact sector (options B1 and B2 of the draft are not pursued).
* If G1-G4 miss, the generator function is kept as an unregistered library helper, so that the known-miss tests compile.
* No third attempt at frame flux on the blade walls (F-RP, ADRs 0082 and 0085) is drafted or attempted.

## What I already knew when registering (disclosed)

* The code was written before this registration, because the rule-d smoke needs it:
  * `rcfd_mesh::gen::annulus_3d_sector`;
  * the schema entry and the `sim.rs` dispatch;
  * the test file `crates/rcfd-api/tests/revolved_sector_w45b.rs` and the unit tests in `gen.rs`.
* Two things were run before registration:
  * the G6 hash print, at e65b4ea before any change, which is the reference the test asserts;
  * the ignored smoke `smoke_w45b`, whose results are below.
* No registered test (G1-G5, G7, N1, N2) was run before this ADR was committed.
* I checked the draft's code claims against the source:
  * `MrfZone::worst_non_axisymmetry` (`crates/rcfd-fv/src/sources.rs` 175-212) checks every face with exactly one side in the zone,
    wall patches included;
  * the `s4_absolute_formulation_mrf_is_refused_on_the_sector` comment (`crates/rcfd-api/tests/fan_sector.rs`) says the same;
  * `sim.rs` 2667 sets `coupled_offdiag_in_h1` from the MRF `cyclic_seams` inside `step()`;
  * the generator list is at `schema.rs` 78 and the dispatch near `sim.rs` 492;
  * for an MRF-only case, `Sources` leaves `wall_pressure_trapezoid` and the pair mean off (`sources.rs` 605-610). That is the same as
    the 44C library harness.
* The draft's parallelogram claim (fact 2 below) belongs to B1. I did not test it, and nothing here depends on it.
* FAN-01 numbers:
  * 1486 rpm = 155.61 rad/s;
  * U = Q / A = 1.4032 / (pi (0.25^2 - 0.124^2)) = 9.478 m/s.

## Context, and a correction to the wave-45 framing

The proposal was: "a fan-passage generator whose hub and shroud are surfaces of revolution, so the MRF zone can open on the fan passage".
The code gives two facts that limit what such a generator can deliver.

1. **The blade blocks MRF, whatever the hub and shroud are.**
   * `MrfZone::worst_non_axisymmetry` checks every face with exactly one side in the zone. That includes every wall patch of a zone
     cell, the `blade` patch among them. A blade surface is not a surface of revolution.
   * `crates/rcfd-api/tests/fan_sector.rs::s4_absolute_formulation_mrf_is_refused_on_the_sector` has documented this since ADR 0065:
     "the blade and seam faces are not surfaces of revolution".
   * ADR 0113 A2 / K6 moved the refusal from the seams to the hub, on the **empty** passage only (`include_blade = false`).
   * An MRF zone on a bladed passage needs the frame flux on the blade walls, so that the wall carries `phi = F_f` and `phi_rel = 0`.
     That is the F-RP route. It did not converge and was removed twice (ADRs 0082, 0085). No generator removes this blocker, and by
     user decision no third attempt is drafted.
2. **Exact hub/shroud faces and a body-fitted blade root cannot coexist on a conformal hex surface lattice.** This is a sketch for B1,
   not tested here.
   * A quad whose (theta, z) image is a parallelogram has a pi-rotation symmetry about the radial line through its centre. That
     gives `F_f = 0`.
   * A lattice made only of such quads is a translation net. A blade footprint between two lattice lines is then a theta-translate of
     itself: constant circumferential thickness, not the FAN-01 NACA root section.
   * The C-grid wraps around the nose, so it cannot be a translation net. The ADR 0113 smoke agrees: hub 4.23e-3 even with zero
     stagger, camber, sweep and lean.

**What this wave delivers:** an **empty** sector whose walls are exact surfaces of revolution, built through the case tree.
* It closes the gap ADR 0113 left open. No case-tree run had sent `cyclic_seams = true` through `Sources` and the `sim.rs` 2667 F1
  line, because no case-tree mesh reached it.
* It gives ADR 0117 an exact case-tree sector.
* **It does not open MRF on the bladed FAN-01 passage.** There the blade wall is still refused by `MrfZone::worst_non_axisymmetry`.

## Decisions (B0)

1. **D1** `rcfd_mesh::gen::annulus_3d_sector(n_theta, n_r, n_z, r_in, r_out, height, angle, grading)`.
   * Geometry: the hexahedra of `annulus_3d` (axes radial, theta, z) over `n_theta` chords of `angle / n_theta`, opened at theta = 0
     and theta = angle.
   * Patches: `inner`, `outer`, `zmin`, `zmax` (walls), and `theta_min` / `theta_max` (generic), made a rotationally cyclic pair by
     `set_periodic_rotation` (rotation `angle` about z, as in `annulus_sector`).
   * Errors: any count 0; not 0 < r_in < r_out; height <= 0; angle not in (0, 2 pi), NaN included.
   * Every face is planar: chord rectangles on inner/outer, planes elsewhere.
2. **D2** Generator name `annulus_3d_sector` in `schema.rs`, plus a new key `mesh.sector_angle` (degrees, default 0, schema range
   [0, 360]).
   * The generator uses `nx` (theta), `ny` (r), `nz`, `r_in`, `r_out`, `lz` and `grading` (expansion, as `annulus_3d`).
   * Dispatch in `sim.rs` after the `annulus_3d_cylindrical_interface` arm. It gives located errors:
     * `mesh.r_out` when r_out <= r_in;
     * `mesh.sector_angle` unless 0 < angle < 360 (so the key is required).
   * `mesh.sector_angle` set to a non-zero value on any other generator is a located error (`check_mesh_options`, the `r_interface`
     pattern).
3. **D3** No change to `axial_fan_sector`, `annulus_3d`, `annulus_sector`, `MrfZone`, `Sources`, the I/O or the solver. No default
   changes.

## Registered checks

Common set-up (FAN-01 empty-passage dimensions):
* r_in 0.124 m, r_out 0.250 m (r_tip 0.2475 + clearance 0.0025), lz 0.4 m, angle 40 deg (9 blades);
* counts 8 x 10 x 24 (theta x r x z), 1 920 cells;
* Omega = 1486 rpm = 155.61 rad/s about +z through the origin;
* U = Q / A = 9.478 m/s (ADR 0086).

* **G1, geometry against the analytic polygonal sector** (unit test `annulus_3d_sector_g1_polygonal_volume_and_areas`).
  * Uniform and radially graded (`Expansion(3.0)`).
  * Volume: the sum of (1/2) sin(Delta theta) (r_2^2 - r_1^2) Delta z, which telescopes to n_theta (1/2) sin(Delta theta)
    (r_out^2 - r_in^2) lz for any radial nodes.
  * Inner and outer areas: n_theta 2 r sin(Delta theta / 2) lz.
  * zmin and zmax: n_theta (1/2) sin(Delta theta) (r_out^2 - r_in^2).
  * theta_min and theta_max: (r_out - r_in) lz.
  * Limit: relative error <= 1e-12 each. Reason: the polygonal values are exact for planar facets.
  * Reported: sin(Delta theta) / Delta theta = 0.99873 for Delta theta = 5 deg.
* **G2, cyclic seams** (`g2_cyclic_seams_of_the_tree_sector`, on the `build_mesh` mesh of the G4 case).
  * `FvGeometry` accepts the pair.
  * Partner face centre `|R c_f - c_g| / r_out <= 1e-9`.
  * MRF pair mismatch `|F_f + F_g| / (|Omega| r |S|) <= 1e-12` (the 44C G limit).
  * Per-cell closure of the face-centre frame flux (zone all, seams on) `<= 1e-12`.
* **G3, the MRF zone opens through the case tree** (`g3_...`).
  * `type = "mrf"`, zone all, `cyclic_seams = true`: `Simulation::new` succeeds, and `worst_non_axisymmetry <= 1e-6` (the API limit).
  * Without the key: refused at `sources.rotor.zone`, with a message that contains "surface of revolution" and `'theta_min'` or
    `'theta_max'`.
* **G4, exact empty passage through the case tree** (`g4_...`).
  * Set-up:
    * zmin fixed (0, 0, U); inner and outer fixed (0, 0, U); zmax zero-gradient velocity with p fixed at 0;
    * nu 1.5e-5, rho 1, initial (0, 0, U);
    * MRF zone all at Omega with `cyclic_seams = true`;
    * SIMPLEC, `upwind`, other settings at the tree defaults.
  * Run: 300 calls of `Simulation::step()`. `step()` does not stop at the residual targets, so there is no early stop.
  * Limits: `max |u - (0, 0, U)| <= 1e-8 U` and `max |p| <= 1e-8 U^2` (ADR 0065 S1 / 0113 K4).
  * Also asserted: `SimpleSolver::coupled_offdiag_in_h1` is true after every step. This shows the F1 line is reached.
* **G5, solid-body rotation through the case tree, sector against the full annulus** (`g5_...`).
  * Meshes: `annulus_3d_sector` (common counts) against `annulus_3d` with nx 72, ny 10, nz 24 and the same radii and height. The
    sector cells are the annulus cells, matched by centre to 1e-9.
  * Physics:
    * inner and outer walls at W = Omega x r (velocity expressions); zmin and zmax symmetry;
    * MRF zone all at Omega; `cyclic_seams = true` on the sector only (the annulus has no cyclic faces);
    * fluid at rest initially;
    * nu = 0.49, so Re = Omega r_out (r_out - r_in) / nu = 10, the Reynolds number of ADR 0113 K1.
  * Solver: SIMPLEC, `central`, alpha_u 0.8, alpha_p 1.0, `tolerance_u = 1e-10`, `tolerance_p = 1e-9`, `Simulation::run(4000)`.
  * Limits:
    * both runs converged (the tree's residual targets met within 4000 iterations);
    * sector minus annulus velocity `<= L_DIFF W r_out`, with `L_DIFF = 4.3e-4` (ADR 0008 M3, as in ADR 0113).
  * Reported only: the sector against the analytic Omega x r, and the pressure difference (offset to cell 0) as a fraction of
    W^2 (r_out^2 - r_in^2) / 2. The library F1 level was 3.6e-9 (ADR 0113 K1).
  * Known differences between the tree and the 44C library harness:
    * the tree's `res_p` is the normalised pressure residual, not the library's continuity;
    * the linear-solver and non-orthogonal-corrector settings are the tree defaults.
  * Same as the library: trapezoid and pair mean off, component-diagonal correction on, F1 on with seams.
* **G6, defaults unchanged** (`g6_fan_meshes_unchanged`).
  * `Mesh::content_hash` (points, faces, owner, neighbour, patches, zones) of `build_mesh` is identical to the hashes recorded at
    e65b4ea before any change, for three meshes:
    * the default `axial_fan_sector` case: `8471bf9a1aef1dd4`;
    * the ADR 0086 C-grid F3 bladed mesh B3: `be64f588a252d0b4`;
    * its empty passage E3: `04fbabedbad41768`.
  * `fan_sector.rs` (s4 included), both `mrf_cyclic_seams_w44c.rs` files and `baseline_hashes.rs` pass unchanged in the gate.
* **G7, located errors.**
  * Case-tree test `g7_invalid_sector_settings_are_located`. Each of these is a diagnostic at the named key, from parsing or from
    `build_mesh`:
    * `sector_angle` 0, absent, 360 or 400: `mesh.sector_angle`;
    * `r_out` equal to or below `r_in`: `mesh.r_out`;
    * `nx`, `ny` or `nz` = 0: that key;
    * `sector_angle` on `annulus_3d`: `mesh.sector_angle`.
  * The valid base case gives no diagnostic.
  * Library: `annulus_3d_sector_g7_invalid_arguments` (each invalid argument is an error; 1 x 1 x 1 builds).

### Negative controls (run first; each must miss by at least 10x, otherwise its check is declared non-discriminating)

* **N1** is G4 at library level on the same tree mesh with `cyclic_seams = false`, called past the refusal.
  * Harness: as ADR 0113 K6, SIMPLEC library defaults.
  * Must give `max |u - (0, 0, U)| >= 1e-7 U`. ADR 0113 D4 measured 2.56 U on the bent box.
* **N2** is G1 with a deliberately wrong reference, the curved-sector volume (angle / 2) (r_out^2 - r_in^2) lz.
  * Must give a relative error `>= 1e-11` (expected about 1.27e-3).

### Status decision rule (registered now)

* The generator is kept iff G1-G4, G6 and G7 pass and N1, N2 discriminate.
* A miss of G1-G4:
  * the case-tree name and key are removed (rule b);
  * by user decision, `gen::annulus_3d_sector` stays as an unregistered library helper;
  * the missed tests stay as ignored `*_known_miss` tests with their limits, built through the helper.
* G5 is an acceptance check of the tree path. A G5 miss with G1-G4 passing:
  * keeps the generator;
  * records the tree/library difference as a finding;
  * keeps G5 as an ignored known miss.
  * A G5 run that fails to converge counts as a G5 miss, reported with the values it reached.
* G6 or G7 failing is a defect of this change. It is fixed before hand-back, with the limits unchanged.
* F-126 stays partial whatever happens.

## Smoke results before registration (rule d)

`smoke_w45b` (ignored, run on demand), one run at the common set-up:

* Construction:
  * `build_mesh` gives 1 920 cells;
  * `FvGeometry` accepts the cyclic pair;
  * `Simulation::new` accepts the G4 case with `cyclic_seams = true`.
* Face-level revolution value `|F_f| / (Omega r |S|)`, worst per patch: inner 1.3e-15 (192 faces), outer 8.0e-16 (192), zmin 0 (80),
  zmax 0 (80), theta_min 1.0 (240), theta_max 1.0 (240). The seams carry the full frame flux, as expected, and receive it with the key.
* 30 iterations, finiteness only:
  * G4 (tree): finite, `coupled_offdiag_in_h1` true after the steps;
  * N1 (library): finite;
  * G5 sector: finite, res_u 1.2e-2, res_p 9.5e-3 at iteration 30;
  * G5 annulus: finite, res_u 1.8e-2, res_p 6.4e-3.
  * Convergence to the G5 targets was not smoked; the decision rule covers a non-converged G5.
* No set-up was found defective.

## Out of scope

* MRF on any bladed passage (fact 1), and frame flux on blade walls (F-RP; no third attempt, by user decision).
* Changing the 1e-6 revolution limit.
* The OpenFOAM writer's missing cyclic rotation (ADR 0113).
* The cylindrical rotor/stator interface on a sector (ADRs 0109, 0112).
* Any change to `axial_fan_sector`.
* Accuracy of a fan solution.
* The coarse pre-solve (`initial.method = "coarse"`) does not list the new generator.

## Files

* `crates/rcfd-mesh/src/gen.rs`: the new function, plus unit tests G1/N2 and G7 (library part).
* `crates/rcfd-api/src/schema.rs`: generator list and doc, the `sector_angle` key.
* `crates/rcfd-api/src/sim.rs`: dispatch arm, `check_mesh_options` rule.
* New `crates/rcfd-api/tests/revolved_sector_w45b.rs`: G2-G7, N1 and the smoke.
* `docs/case-schema.md`, `docs/meshing.md`.

## Addendum: measured values (one run each, release build, on the pre-rebase commits 0abc8fe and its N2 split; rebased onto main 637f8d2 as ba4c648 and 497e28b)

Order: N2 and N1 first, then G1 and G7 (library), then G2-G7 (case tree).

| check | measured | limit | result |
| --- | --- | --- | --- |
| N2 G1 volume against the curved sector (uniform / graded) | 1.269e-3 / 1.269e-3 | >= 1e-11 | discriminates |
| N1 G4 without seam frame flux (library, tree mesh) | 6.38 U | >= 1e-7 U | discriminates |
| G1 volume (uniform / graded) | 2.0e-15 / 4.0e-16 | 1e-12 | pass |
| G1 inner, outer (both gradings) | 4.0e-16, 9.9e-16 | 1e-12 | pass |
| G1 zmin, zmax (both gradings) | 4.2e-16, 2.1e-16 | 1e-12 | pass |
| G1 theta_min, theta_max (uniform / graded) | 3.2e-15, 3.3e-15 / 1.1e-15, 6.9e-16 | 1e-12 | pass |
| G1 reported: sin(dt) / dt | 0.99873 | none | reported |
| G2 `FvGeometry` accepts the pair (1 920 cells) | accepted | accepted | pass |
| G2 partner face centre / r_out | 5.4e-16 | 1e-9 | pass |
| G2 pair mismatch | 1.8e-15 | 1e-12 | pass |
| G2 per-cell closure | 3.2e-16 | 1e-12 | pass |
| G3 with `cyclic_seams = true` | `Simulation::new` accepts; worst 1.8e-15 (a theta_max pair) | <= 1e-6 | pass |
| G3 without the key | refused at `sources.rotor.zone`, `patch 'theta_max'`, 1.00 | located, names a seam | pass |
| G4 tree, 300 steps: max u - (0, 0, U); max p | 5.6e-13 U; 3.0e-14 U^2 | 1e-8; 1e-8 | pass |
| G4 `coupled_offdiag_in_h1` after every step | true | true | pass |
| G5 sector / annulus converged | 349 / 316 iterations | targets met within 4000 | pass |
| G5 sector - annulus velocity | 1.4e-11 W r_out | 4.3e-4 | pass |
| G5 reported: sector against analytic; pressure difference | 3.0e-3 W r_out; 2.1e-10 of the range | none | reported |
| G6 content hashes (default, B3, E3) | equal to the e65b4ea values | equal | pass |
| G7 located errors (10 cases; library invalid arguments) | each at its key; each an error | located | pass |

Verdict: G1-G7 pass and N1, N2 discriminate, so by the decision rule the generator and its case-tree name are kept.

What this delivers:
* A case-tree mesh, `annulus_3d_sector`, on which an MRF zone with `cyclic_seams = true` is accepted.
* The positive path of ADR 0113 now runs through the case tree, through `Sources` and the `sim.rs` 2667 F1 line (G4 asserts the flag).
* The exact empty passage holds to 5.6e-13 U.
* The solid-body sector reproduces the full annulus to 1.4e-11 W r_out. The library F1 level of ADR 0113 K1 was 3.6e-9.

On the measured levels:
* The G4 level is above the library bent-box K4 value (1.1e-15) and far below its limit. The tree case differs: FAN-01 speeds,
  `upwind`, tree defaults. I did not look further.
* The G5 analytic error (3.0e-3 W r_out) is reported only. It is the discretisation error of both meshes, shared by sector and annulus.

What it does not deliver:
* MRF on the bladed FAN-01 passage. There the blade wall is refused by `MrfZone::worst_non_axisymmetry`, whatever the hub and shroud
  are. This generator has no blade.
* By user decision, no third F-RP (blade-wall frame flux) attempt is drafted.

Deviations from the registered plan:
* N2 was registered as part of the G1 description, and the first code computed it inside the G1 unit test. Before any registered run
  it was split into its own unit test `annulus_3d_sector_n2_curved_reference_misses`, so that it could run first as registered.
  The criterion is unchanged.
* The cargo filter for the first N2 run used `--exact` with the wrong module path and ran nothing. It was re-run with a name filter.
* Docs and the one-sentence F-126 roadmap note were written after the runs. They state the measured values only.

Unverified references: none. Every reference value is a closed form or an earlier ADR's measured value.

Status: F-126 stays partial. Its acceptance, a published compressor case, is not attempted.

## Final gate

The gate ran on a fresh `git archive` of a033720, the branch rebased onto main 637f8d2, in `<scratch dir>`. Stages ran
one at a time (CARGO_BUILD_JOBS=3, RUST_TEST_THREADS=3):
* `cargo fmt --check`: clean;
* `cargo clippy --workspace --all-targets -- -D warnings`: clean;
* `cargo test --workspace --release --no-fail-fast`: 2020 passed, 0 failed, 634 ignored.

The wave-44 baseline was 2010 / 0 / 633. This wave adds 10 passing tests (3 in `gen.rs`, 7 in `revolved_sector_w45b.rs`) and 1
ignored test (`smoke_w45b`). `baseline_hashes.rs`, `rules_w19c.rs`, `fan_sector.rs` and both `mrf_cyclic_seams_w44c.rs` files pass
unchanged. The only commit after the gated one adds this section (docs only).

## References

None outside our own ADRs. The polygonal volumes and areas, and the rotation identity `F_g = -F_f` (ADR 0113), are first principles.
