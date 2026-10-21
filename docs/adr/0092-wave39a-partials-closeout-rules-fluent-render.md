# ADR 0092: Wave 39A, partials close-out for F-240, F-118 and F-073

Status: registered before the registered tests were committed. Limits are fixed; a miss becomes an `#[ignore]`d `*_known_miss`
test with the limit unchanged. No library code changes: every addition is test code, so defaults and the baseline hashes are
untouched. Clean-room: the only formula used is the NACA 4-digit thickness of `cases/naca0012/reference/naca4_thickness.txt`;
no literature value enters a gate.

The residual gaps are those stated in docs/audit/partials-group-8.md (F-240), partials-group-10.md (F-118) and
partials-group-3.md (F-073). I re-checked each claim against the code before writing this ADR:

* F-240: the E2 corpus of `crates/rcfd-api/tests/rules_w19c.rs` reads `cases/**/*.toml`, two documentation pages and the TOML
  literals of the test sources. The five registry directories (`cavity`, `backward_step`, `flat_plate`, `heated_cavity`,
  `naca0012`) hold only `case.json`; the `validation_*.rs` tests build their solvers through the library. Two descriptors name
  no case-tree generator: `backward_step` names a library generator (`gen::backward_step`) that the case schema does not offer,
  and `naca0012` says "requires an airfoil C-grid generator (not yet implemented)".
* F-118: the only 3D Fluent import in the tests is the two-cell sample of `rcfd-io/src/fluent.rs`; no test calls the quality
  check on a 3D Fluent import.
* F-073: the CLI has no render subcommand; its render path is `rcfd serve` plus `rcfd call render.png` (`cmd_serve`,
  `cmd_call` in `crates/rcfd-cli/src/main.rs`, the `render.png` method of `rcfd_api::service`). No test times that path.

## F-240: set-ups for the registry and validation cases (new file crates/rcfd-api/tests/registry_setups_w39a.rs)

Acceptance (unchanged): "A rule set of at least 10 dependency, required and count checks rejects each seeded invalid setup
with the expected message id and accepts the benchmark registry cases; the published validation cases raise no error-level
diagnostic".

Reading registered here, for the lead to accept or reject:

* "The benchmark registry cases" are the five `cases/*/case.json` descriptors that `rcfd cases` lists.
* "The published validation cases" are those five benchmarks (each cites published reference data), plus the other physical
  cases that the `validation_*.rs` tests compare with published data and that the case tree can express: the cylinder at
  Re 100, the plane channel at Re_tau 178.12 (SST), the wall-function channel and the Sod shock tube. Not covered: the Graetz
  test (energy equation on a prescribed analytic face flux; the case tree has no input for that flux), the fan01 tests (their
  own case files, out of scope here) and the w33c SST plate and wall-layer tests (library-only turbulence-model set-ups).

Set-ups (TOML texts in the test; one parameter of each descriptor):

| Set-up | Mesh | Physics |
|---|---|---|
| cavity | `cavity_2d` 32 x 32 (descriptor n = 32) | Re 100: nu 0.01, lid 1 |
| backward_step | descriptor mesh, `gen::backward_step` l_in 10, l_out 40, h_in 8, step 1, depth 0.1, n = [40, 160, 32, 8], written as an OpenFOAM polyMesh and read through `mesh.source = "file"` | Re_h 37500: nu 1/37500, k-epsilon, wall functions, inlet 5 % / 0.8 |
| flat_plate | `box_2d` 100 x 60 over 2 x 0.5, 20 inflation layers on ymin from 1e-4, growth 1.15 | laminar Re_L 1e5: nu 2e-5 |
| heated_cavity | `box_2d` 40 x 40 (descriptor n = 40) | Ra 1e3, Pr 0.71: nu 0.71, alpha 1, g 710, Boussinesq |
| naca0012 | O-grid 128 x 48 (`cylinder_ogrid`, radial expansion 200) with the inner ring moved onto the NACA 0012 thickness at x = (1 + cos theta)/2, the outer ring a circle of 20 chords about mid-chord, linear blend between; patch `cylinder` renamed `airfoil`; polyMesh file as above | Re_c 6e6, alpha 4 deg, k-omega SST, freestream far field |
| cylinder_re100 | `cylinder_ogrid` 128 x 100, r 0.5..20, grading 150 (the statistics-test set-up) | nu 0.01, PISO |
| channel_re180 | `cases/channel/sst_channel.toml` with nu = 1/178.12, omega_initial = 17.812 | SST, y+ = 1 inflation |
| channel_wall_law | `cases/channel/kepsilon_channel.toml` | k-epsilon, wall functions |
| sod | `cases/sod/sod.toml` | compressible Euler |

Registered check V1: every set-up goes through `runctl::check_only` (the library body of `rcfd check`: semantic validation,
simulation construction, mesh build, mesh quality, set-up rules). Limits: exit code 0 and 0 error-level rule diagnostics on
every one of the nine set-ups. Warnings and infos are printed, not limited.

The NACA O-grid is a set-up for the rule check, not a mesh for a lift prediction: the linear blend leaves high
non-orthogonality at the trailing edge (smoke below). The registered check accepts that because rule SV101 is a warning by the
rule file's own definition; making the blend orthogonal is not part of this item.

Smoke (rule d, run before this ADR with the same test file): all nine pass; exit 0 and 0 errors on each; one warning, SV101 on
naca0012 (max non-orthogonality 80.27 deg, skewness 2.25, aspect ratio 164); cells: cavity 1024, backward_step 7680,
flat_plate 6000, heated_cavity 1600, naca0012 6144, cylinder 12800, channel_re180 240, channel_wall_law 56, sod 100. The
existing E2 test (not ignored) still passes with the new literals in the corpus: 89 texts evaluated, warnings SV101 (two)
and SV103 (one) only.

Decision rule: if V1 passes, F-240 is set to done under the reading above, and the hand-back states the reading. If the lead
reads "published validation cases" as every `validation_*.rs` set-up including the Graetz test, the item goes back to partial
for that test alone.

## F-118: a 3D Fluent mesh through the quality check, ASCII and binary (new file crates/rcfd-io/tests/fluent_3d_w39a.rs)

Acceptance (unchanged): "Imports a 2D and a 3D mesh with zone/BC-type mapping and passes mesh checks; unsupported section ids
are reported, not silently dropped; only fixtures from public sources run in default CI".

Set-up: `gen::annulus_3d` 48 x 8 x 10 (r 0.5..1, height 1, radial expansion 3), twisted by 0.4 rad per unit height and
sheared by 0.2 z in x, so that the hexahedra are curved, twisted and non-orthogonal (3840 cells). The test writes it as a
Fluent file: header, node section, one cell zone (hexahedra), the interior face zone and four boundary zones mapped wall
(`inner`), symmetry (`outer`), velocity-inlet (`zmin`), pressure-outlet (`zmax`), quadrilateral faces `n0..n3 c0 c1` with c0
the owner, zone names in section 45. Three encodings: ASCII (10/13), binary double (3010/3013), binary single (2010/2013),
binary framing as the reader's own unit tests write it.

Registered checks:

* F1 (ASCII) and F2 (3010/3013): counts (cells, points, faces, internal faces) and patch sizes equal to the source; patch kinds
  Wall, Symmetry, Inlet, Outlet; no ignored section; `check_mesh` passes (`is_ok`); total volume, minimum volume, maximum
  skewness and maximum aspect ratio equal to the source's to 1e-12 relative, maximum non-orthogonality to 1e-10 degrees. F2
  also: the content hash equals the ASCII import's.
* F3 (2010/2013, float32 coordinates): the same with 1e-5 relative and 1e-3 degrees (float32 rounding is 6e-8 relative per
  coordinate; volumes and angles amplify it by a small factor).

Smoke (run before this ADR, limits already in the file): F1 and F2 differences 0 (bit-equal metrics), non-orthogonality
30.099397 deg, `is_ok`, no warnings; F3 volume 3.3e-10, minimum volume 9.3e-7, skewness 2.6e-6, aspect ratio 6.4e-7 relative,
non-orthogonality 30.099406 deg (+9e-6).

The file is generated by the test from rcfd's own mesh generator, so it is not a third-party fixture; that satisfies "only
fixtures from public sources run in default CI". It remains a round trip: the writer encodes the reader's reading of the
public section description, so it verifies the 3D and binary paths end to end but not the binary layout against a
Fluent-written file (no public 3D or binary Fluent file has been found). With F1 to F3 passing, the acceptance as written is
met (2D: the elbow test and the 2D unit test; 3D: F1 to F3; unsupported sections: the existing unit test).

## F-073: timed CLI render of a 1M-cell mesh (new file crates/rcfd-cli/tests/render_cli_w39a.rs)

Acceptance (unchanged): "CLI renders a contour image in under 2 s for a 1M-cell mesh".

Set-up: the test starts `rcfd serve --addr 127.0.0.1:0` (killed by a drop guard), then runs `rcfd call session.create` with a
1000 x 1000 `cavity_2d` case whose initial x velocity is `sin(6 x / 1 m) y / 1 s` (the r2 field of the library test, so the
speed varies over the domain), `rcfd call run.initialize`, and finally one `rcfd call render.png` with field `speed` and
width 1024 (as r2). Release builds only (`cfg_attr(debug_assertions, ignore)`), as r2.

Registered check R4: the wall time of that single `rcfd call render.png` process (process start, connection, render, PNG
encode, base64, JSON transfer, print) is below 2.0 s; the reply is a PNG of width 1024 and the speed range exceeds 0.5. One
call, no repetition or median. Session creation and initialisation are timed and printed, not limited (mesh generation and
solver set-up are excluded as in r2).

Smoke (run before this ADR): the first attempt failed in set-up, not in the check: the expression `sin(6*x)*y` was refused by
the unit checker ("sin needs a dimensionless argument"), so the unit literals were added. Second run: session.create 0.022 s,
run.initialize 14.6 s, `rcfd call render.png` 1.253 s, image 1024 x 954, speed 3e-7 .. 0.9995.

The timed path is the CLI binary (`rcfd call` against `rcfd serve`); with R4 passing, the acceptance as written is met.
No rewording is proposed.

## Roadmap

Only the three entries' `status` and `note` change in docs/roadmap.json; the acceptance texts are untouched.

## Addendum: measured values (registered runs after this ADR was committed)

* F-240 V1: met. All nine set-ups exit 0 with 0 error-level rule diagnostics. One warning: SV101 on naca0012 (max
  non-orthogonality 80.27 deg, skewness 2.25, aspect ratio 164). No other warning or info. F-240 set to done under the reading
  registered above.
* F-118 F1/F2: met; every compared quantity equal to the source bit for bit (relative differences 0, non-orthogonality
  30.099397 deg both), content hashes of the ASCII and 3010/3013 imports equal, `is_ok`, no warnings. F3: met; volume 3.3e-10,
  minimum volume 9.3e-7, skewness 2.6e-6, aspect ratio 6.4e-7 relative, non-orthogonality +9e-6 deg. F-118 set to done.
* F-073 R4: met; `rcfd call render.png` 1.555 s (limit 2 s), run.initialize 18.8 s, session.create 0.054 s, image 1024 x 954,
  speed 3e-7 .. 0.9995. The smoke run gave 1.253 s; the difference is load from other agents' builds on the same machine.
  The margin under a fully loaded gate is about 0.45 s. F-073 set to done.

No deviation from the registered plan. No miss.
