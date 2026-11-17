# ADR 0707: F-769 registration: the F-040 wall-function channel and the checks of its test file

Status: **REGISTERED 2026-11-17, written before any run of the checks of [ADR 0705](0705-f769-registration-adrs-for-the-five-feature-groups-that-no-adr-describes.md)
(F-769, group 2 of 5).** Not yet accepted by the maintainer. Results are in the addendum of ADR 0705.

**These tests predate this ADR.** `crates/rcfd-turb/tests/channel_wall_function.rs` was committed on 2026-10-13 in the first import of rcfd
(`f71f6e2c`), as the verification of F-040 (wave 5); no ADR registered its limits. ADR 0014 and ADR 0015 describe the turbulence models
built on the same wall constants, not this test. This ADR records what the file checks and which limits it carries, read from the code and
from the F-040 note of the roadmap, which records the measured numbers. It is not a decision taken before the test, and the roadmap
note records the measured deviations of W7 together with its 3 % limit; it does not record whether the limit was fixed before the run. No Rust file is
edited and no test is run for it (ADR 0705, Limits).

## The feature

F-040 ("wall distance and standard wall functions", acceptance: "y+ and wall distance verified on channel mesh; log law recovered in
developed flow") added to `rcfd-turb`: `wall_distance` (exact Euclidean distance from each cell centre to the nearest wall face),
`WallConstants` (kappa 0.41, E 9.8, Cmu 0.09; the equilibrium log law of Launder and Spalding 1974 with the switch to the viscous sublayer
at the y+ where the two laws meet, 11.5 for these constants), the wall eddy viscosity from the first-cell velocity
(`nut_wall_from_velocity`), `wall_y_plus`, `effective_viscosity` and the Prandtl mixing-length closure `l = kappa d`
(`MixingLength::nut_with_walls`). The F-040 note says the log law is recovered with a mixing-length interior, not yet with k-epsilon (F-042),
and that a DNS comparison is left to F-042.

## The channel

A periodic channel of half height h = 1, friction velocity u_tau = 1, nu = 1/395 (Re_tau = 395, `RE_TAU`), height 2, 4 cells along the flow;
the streamwise pressure gradient is the body acceleration `u_tau^2 / h = 1` (`BodyForce::uniform` [1,0,0]); walls `ymin` and `ymax`
fixed; SIMPLEC with `alpha_u = 0.9` and the opt-in `h_from_solution = true` (the default Jacobi-like H needs more than 8000 iterations;
the F-040 note records that the opt-in is about 6 times faster); the eddy viscosity is under-relaxed 0.5 per iteration (the plain Picard
map is only neutrally stable); at most 8000 iterations; the stop is a maximum velocity update below 1e-10 (the momentum residual of the
identically zero v component is round-off noise).

## Limits carried by the file

| id | test | kind | limit | source |
|---|---|---|---|---|
| W1 | `wall_distance_is_exact_on_the_channel_mesh` | numeric | on the 12-cell uniform channel mesh and on a 20-cell mesh graded symmetric 5.0 (3 x 20 cells, height 2), the wall distance equals `min(y, 2 - y)` to **1e-13** in every cell; the distance to the side wall `xmin` equals `x` to **1e-13**; an unknown patch name (`nope`) is an error | the asserts; the F-040 note ("equals min(y, 2h - y) to 1e-13 on uniform and graded channel meshes and equals x for a side wall") |
| W2 | `channel_recovers_the_log_law_and_the_wall_shear` (for `ny` = 8, 12, 16) | numeric | the run converges: max velocity update **below 1e-10** within the 8000 iterations | the assert on `ch.res` |
| W3 | same | numeric | the first-cell y+ is `y1 * 395` with `y1 = 1/ny`, that is 49.4, 32.9, 24.7 for the three meshes, and each is **above 11.6** (the three meshes are in the log layer, none in the sublayer) | the assert; the F-040 note lists the three values |
| W4 | same | numeric | the y+ reported by `wall_y_plus` on every wall face equals `y1 * 395` within a relative **5e-3** | the assert; the F-040 note says "within 0.5 %" |
| W5 | same | numeric | the wall shear from the wall function, `(nu + nut_w) u_P / y`, equals the imposed `u_tau^2 = 1` within **2e-3** (the momentum balance), on every wall face | the assert; the F-040 note says "within 2e-3" |
| W6 | same | numeric | the velocity of the first cell equals the log law `WallConstants::u_plus(y+)` within a relative **5e-3** | the assert |
| W7 | same | numeric | the interior profile, for cells with centre below mid-height and beyond the first cell, deviates from the solution of the mixing-length stress balance `(nu + l^2 S) S = 1 - y/h`, `l = 0.41 y`, integrated outward from the first cell by Simpson with 4000 intervals, by at most **0.03** relative. The recorded deviations are 2.1 %, 2.3 %, 2.4 % for ny 8, 12, 16; the file's comment says the offset is the second-order truncation error across the log layer between the first two cells of a uniform mesh (centre distances 1:3) and does not shrink with ny | the assert; the F-040 note |
| W8 | same | numeric | the flow is streamwise uniform: every cell of a column of 4 cells differs from the first of its column by less than **1e-7** | the assert |

W3 to W8 are in one test, so a failure of any of them fails `channel_recovers_the_log_law_and_the_wall_shear`; the table lists them
separately because each carries its own number.

## What the file does not check

* A comparison with the DNS profile of Re_tau 395 (F-042: the k-epsilon channel is in the shipped case `cases/channel/kepsilon_channel.toml`,
  see ADR 0708), k-epsilon, SST or Spalart-Allmaras wall treatment, scalable wall functions, rough walls, the first cell in the viscous
  sublayer (y+ below 11.6) or at y+ of about 1.
* Grid refinement: the three meshes are all uniform; W7 does not show convergence to the ODE solution (the F-040 note says it does not shrink).
* That `h_from_solution` is the better choice outside this flow; it is opt-in and default off.
* The mixing-length closure is not selectable from the case schema (F-040 note); only the library is checked.

## Addendum 1: corrections of rows (ADR 0717, 2026-11-17)

Appended by [ADR 0717](0717-f795-registration-corrections-and-a-reading-of-the-18-unread-limit-rows-of-adrs-0706-to-0710.md)
(F-795). Nothing above this heading was edited. For any reader, the corrected row below replaces the row of the same id in the
table above. The other rows of this ADR stand: W2 was read by ADR 0705, W3 to W5 by ADR 0715 and W6 to W8 by ADR 0717, which found
the three in agreement with the test file and the F-040 note.

**W1** (line 36). Wrong words: "the 12-cell uniform channel mesh" and "a 20-cell mesh". The test
(`crates/rcfd-turb/tests/channel_wall_function.rs`, lines 95 to 126) calls `run_channel(12, 1.0)`, which builds
`box_2d_graded(4, ny, 1.0, 2.0, ...)` (line 33): a uniform mesh of 4 x 12 cells, 48 cells, 12 of them across the height 2. The
second mesh (lines 107 to 115) has 3 x 20 cells, 60 cells, as the bracket of the row already says. The numbers 12 and 20 are the
cells across the height, not the cells of the mesh. The limits of the row are right. The asserts on `xmin` and on the unknown patch
name are made on the graded mesh, which the corrected row now says. Corrected row:

| id | test | kind | limit | source |
|---|---|---|---|---|
| W1 | `wall_distance_is_exact_on_the_channel_mesh` | numeric | on a uniform 4 x 12 mesh (48 cells, 12 across the height 2, the channel mesh of `run_channel(12, 1.0)`) and on a 3 x 20 mesh (60 cells, height 2) graded symmetric 5.0 across the height, the wall distance to `ymin` and `ymax` equals `min(y, 2 - y)` to **1e-13** in every cell; on the graded mesh the distance to the side wall `xmin` equals `x` to **1e-13**, and an unknown patch name (`nope`) is an error | the asserts; the F-040 note ("equals min(y, 2h-y) to 1e-13 on uniform and graded channel meshes and equals x for a side wall") |
