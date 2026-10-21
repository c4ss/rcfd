# Partials audit, group 10

This is a read-only audit of ten roadmap items with status `partial`. It started at HEAD b858302 on 2026-10-21. Each item is graded
against its `acceptance` field in docs/roadmap.json only. Wider scope from the title or from a "SCOPE (merged review)" addition is listed
as "scope beyond acceptance". That scope does not lower the grade.

Grades: DONE = the acceptance text, as written, is met by a passing non-ignored (or reproducibly run) test with numeric limits. PARTIAL =
a real tested slice exists, and the unmet clause is named. TODO = nothing usable. OVERSTATED = the roadmap note claims more than the tests
show.

How the runs were made: Windows 11, `cargo test --release -p <crate> --test <name> -- --include-ignored` (or `--ignored <filter>` for
the heavy ones) with a private target directory, on a machine shared with other agents' cargo jobs, so wall times are noisy. "Known misses
fail as recorded" means the `#[ignore]`d known-miss or superseded tests fail, as their notes say. Heavy runs NOT repeated (recorded
addendum numbers were read instead): the wall-resolved backward-facing step `step_sst.rs` (three to six 8000-iteration runs), the ADR 0070
three-mesh flat plate `validation_w33c_sst_plate.rs` (about 30 minutes for the finest mesh), the ADR 0070 full-channel Karman-measure run
(58532 iterations), the ADR 0074 TMR-grid plate runs `w34c_sst_plate_hypotheses.rs`, and `c3_vent_reversed_k0_long_run_observation`.

## Summary table

| id | title (short) | grade | evidence run or read | residual gap (against acceptance) | size | blockers | one wave | user decision |
|---|---|---|---|---|---|---|---|---|
| F-043 | k-omega SST / BSL, low-Re wall treatment | PARTIAL (1 of 3 validations) | ran rcfd-turb `channel_sst` 2/2, `validation_w33c_sst_wall_layer` 2 pass (3 heavy ignored, not run), `flat_plate_sst` (Cf(0.97) -0.7 % vs TMR; White known miss fails as recorded, -7.36 %), `flat_plate_bsl_sst` B1 and B3 pass; read ADR 0015 B5/B5b, ADR 0070, ADR 0074 | BFS: x_r/h 6.554 (SST) and 5.956 (BSL) against [6.16, 6.36], y+ 2 runs and SST y+ 0.5 not converged (recorded, not re-run); NACA0012: never attempted | L (M for NACA0012 alone) | none hard: TMR NACA0012 data is already fetched (validation-data/data/naca0012-tmr); BFS needs a diagnosis of the model spread | no (yes for NACA0012 alone) | yes (BFS reference: experiment window or published SST results) |
| F-062 | Compressible BCs (total, far-field, non-reflecting) | PARTIAL (1 of 2 clauses) | ran rcfd-fv `compressible_bc` (A and B pass, sigma 0.02 and NACA placeholder known misses fail as recorded), the heavy nozzle check C (pass, 184 s), rcfd-api `compressible_bc_case` 3/3 | NACA0012 transonic Cp within published scatter: not attempted, no reference chosen | M-L | missing reference data (transonic NACA0012 Cp with scatter); airfoil mesh for the density-based solver | no | yes (choose the transonic dataset and band) |
| F-101 | GPU acceleration | DONE (see below) | built rcfd-gpu with `--features cuda` and ran the ignored `f1_gpu_vs_one_thread` (RTX 5080): 5000211 unknowns, CPU 1 thread 1.818 s, GPU one-shot 0.159 s (11.4x), resident 0.060 s (30.4x), 37 CG iterations each | none as written; the test asserts only GPU < CPU (1x), not 3x; flux assembly on the device and the SCOPE list are beyond the acceptance | S | none | yes | no |
| F-118 | Fluent .msh/.cas import | PARTIAL | ran rcfd-io `fluent_public` (real elbow.msh present, SHA-256 matches, pass), rcfd-io lib `fluent` 5/5, rcfd-cli `fluent` 2/2 | no 3D mesh is put through the mesh quality check: the only 3D import is a 2-cell hand-written sample; binary sections tested only on files written by the tests | S (quality check on a 3D sample); open-ended for a public 3D file | missing reference data (a public 3D or binary Fluent mesh) for the stronger reading | yes | no |
| F-130 | Radiation extensions | PARTIAL (1 of 2 clauses) | ran rcfd-api `radiation` 5/5 (R1 worst 2.6e-9, R2 0.410 %, R3, R4, R5) | non-grey band model and tabulated emissivity: nothing built; clause 1 is met only in the P1 sense (see below) | M-L | missing reference data (a verifiable WSGG coefficient set or emissivity table) | no | yes (which emissivity table) |
| F-200 | Boundary-condition catalogue | PARTIAL | ran rcfd-fv `bc_catalogue` 6/6, rcfd-api `bc_catalogue` 22 pass, 8 known misses fail as recorded (388 s) | "Hagen-Poiseuille pipe" and "swirling pipe" are annular ducts (A3 0.398 %, E2 within 3 %); no solid circular pipe with a mass-flow inlet; E2 and C2/C3/D1/D2 pass only under the post-hoc settled-field criterion | S-M | none (a solid pipe mesh `pipe_mesh` / `pipe_sweep` now exists; its own Poiseuille check needs non-orthogonal correctors) | yes | yes (accept the annulus as the pipe, or require a solid pipe) |
| F-210 | Fractional-step time advancement | DONE | ran rcfd-fv `fractional_step` 14 pass, 2 known misses fail as recorded; rcfd-api `fractional_tree` 4/4 | none as written (Taylor-Green KE error 2.30e-3 vs 0.02; Ghia u 3.0e-3, v 9.0e-3 vs 0.02) | - | - | - | no (a stale ignore to lift, below) |
| F-222 | Pollutant post-processing (NOx, soot) | PARTIAL (0 of 2 clauses) | ran rcfd-reactor `slices` k10: closed form and corrected quasi-steady check pass (6.8e-7), registered set-up known miss fails as recorded; read ADR 0027 | no flame, no comparison with published Zeldovich data, no piloted-jet NO trend | L | F-054 (no variable-density flame); missing reference data (flame NO data, TNF piloted flame); UNVERIFIED rate constants | no | yes (choose the flame datasets) |
| F-232 | Surface diagnostics, repair, wrapping | PARTIAL (1 of 3 clauses) | ran rcfd-mesh `surface_diag` 6 pass, the superseded `s3_repair_outcomes` fails as recorded | no public defective surface set; no self-intersection or spike detection; no shrink-wrap | L | missing reference data (public defective and leaky models) | no | yes (choose the public set) |
| F-243 | Acoustic boundaries and sources | PARTIAL (0 of 2 clauses) | read ADR 0066 and the source; grep for impedance or quiet boundaries (none in the acoustics scope); F-062's `b_no_spurious_reflection` re-run (above) | impedance tube not built; no quiet-boundary test with the 1 % limit | M (impedance); S (quiet boundary via the F-062 sigma = 0 outlet) | F-062 boundary exists; none for the S part | yes for the quiet boundary, no for both | no |

Counts: DONE 2, PARTIAL 8, OVERSTATED 0, TODO 0.

## Items not DONE, and the DONE items with a caveat

**F-043 (PARTIAL).** The flat plate and the channel reproduce. The default channel test gives U_c+ 20.457 (+1.6 %) and U_b+ 18.167 (+4.4 %)
at y+ 1.00, and U_b+ 16.920 (-2.8 %) at y+ 28. The flat plate (140 x 110, y+ 0.5, 1989 iterations) gives Cf(0.97) = 0.002663, -0.7 %
against the TMR page value, and B1 -0.80 / -1.04 / -1.18 % against CFL3D at x = 0.5, 0.97, 1.5 (limit 3 %). BSL (B3) is within 1.6 %
of CFL3D. The White 5 % band still misses at -7.36 %. ADR 0070 and ADR 0074 show this is a property of the published model, not a defect:
the TMR codes sit the same way against White. The ADR 0070 P2a to P2e plate checks pass as recorded, and the H6c TMR-grid sequence
reaches order 1.07 (recorded, not re-run). Two of the three named validations are not met. First, the backward-facing step (ADR 0015
B5b, recorded) gives x_r/h 6.554 (SST) and 5.956 (BSL) on the y+ ~ 1 mesh, against the experimental window [6.16, 6.36]. The y+ 2 runs
of both models and the SST y+ 0.5 run did not converge, so "y+~1 meshes stable" holds on the plate and in the channel but not on the step.
Second, NACA0012 was never attempted. The TMR NACA0012 data (Ladson, Gregory, Abbott, McCroskey and the TMR grid points) is already on
disk in validation-data/data/naca0012-tmr, so missing data does not block it. A wall-resolved airfoil mesh is the main work, and
rcfd-mesh has C-grid generators from the fan work. The acceptance wording "validation passes" is unmeasurable without limits. Suggested
rewording: "flat-plate Cf within 3 % of TMR CFL3D; BFS x_r/h within <band> of the published SST result for the TMR step; NACA0012 CL and
CP at alpha 0, 10, 15 within <band> of the TMR SST results; every y+ ~ 1 run converges". The SST and BSL spread (6.55 and 5.96 against
6.26) is larger than the window, so comparing with published SST results rather than the experiment is a user decision. Citations:
Menter (1994, 2003) were not read (TMR transcription only), and the 6.26 +- 0.10 window comes from the TMR page.

**F-062 (PARTIAL).** Re-run: free-stream preservation and the face-state checks pass, the pressure outlet reflects 0.9412, and the
non-reflecting outlet at sigma = 0 reflects 0.0000 (limit 0.02). The registered sigma = 0.02 known miss fails as recorded. Nozzle C
passes in 136934 steps: Mach 5.38e-3, p/p0 5.73e-4, T/T0 1.81e-4, mass -4.94e-3, in/out 3.3e-7, all inside their limits. So the
"no spurious reflection test" clause is met as written. Caveat: the same test prints 0.0000 for the legacy `Fixed` (ambient state) and
`Extrapolate` conditions too. A 1-D normal-incidence pulse through a Riemann-solver boundary does not separate a good non-reflecting
treatment from plain extrapolation. An oblique-wave or 2-D vortex test would. The NACA0012 transonic clause is open. The vendored TMR
NACA0012 data is low-speed only, and no transonic reference with a scatter band is chosen (AGARD AR-138 or AR-211, or the
Vassberg-Jameson Euler values). The acceptance needs a named reference before it is measurable.

**F-101 (DONE, see the numbers).** The acceptance holds on the one machine with the device: the re-run at 5000211 unknowns (171^3, CG-Jacobi, tol 1e-6,
median of 5 after a warm-up) gives 11.4x one-shot, including matrix upload and download, and 30.4x resident, against one CPU thread. The
recorded runs gave 9.7x and 15.2x. Caveats. (1) The test's numeric limit is "GPU faster than one CPU thread", not 3x, so the roadmap
figure is measured but not asserted. Raising the assertion to 3x is S. (2) The CPU single-thread time varies with machine load (1.82 s
here, 1.94 s and 2.84 s recorded), so the speedup is noisy, though always well above 3x. (3) The test skips without a device, so default
CI never checks it. (4) The CG needs only 37 iterations at this tolerance, so the ratio mostly reflects SpMV and vector bandwidth. The
title's flux assembly on the device, the multi-GPU and other-vendor work, and the SCOPE list (energy, free surface, species, turbulence,
radiation on the device) are scope beyond the acceptance. They should become their own item if wanted.

**F-118 (PARTIAL).** The real Fluent-written 2D elbow mesh (fetched, SHA-256 a7a3c881...) imports with 918 cells, five named zones plus
`frontAndBack`, and passes `check_mesh`. The extruded volume equals the shoelace area times the thickness to 5.4e-16. Unknown sections
are reported (`(4, 1), (33, 2), (37, 1)` in the unit test), and the CLI prints "Fluent sections not read". The 2D Fluent channel runs a
laminar case. Unmet: "imports ... a 3D mesh ... and passes mesh checks". The only 3D import is a 2-cell hand-written sample that checks
counts, zones and volume, and no test calls the mesh quality check on it. Running `check_mesh` on a larger synthetic 3D file (ASCII and
binary) is S. The stronger reading, a real 3D or binary Fluent file, needs a public file that has not been found. HDF5 and gzip are title
scope beyond the acceptance.

**F-130 (PARTIAL).** Clause 1 is met in a narrow sense. R1 compares P1 with the analytic P1 solution (worst 2.6e-9, limit 1e-3), which is
a self-consistency check of the P1 discretisation. Only R2 compares with radiative transport, at one optical thickness (tau = 20, 0.410 %
from the Milne asymptote). The Milne-to-slab mapping was re-derived in the repo, not taken from a source (docs/reference-data.md). A
transport comparison at tau of order 1, for example the tabulated exact slab Psi of Heaslet and Warming, would need a non-P1 method such
as discrete ordinates (F-052), and its table is not on disk. Clause 2 (non-grey band model reproducing tabulated emissivity) has nothing
built. It needs a verifiable WSGG or band coefficient set and the emissivity table it should reproduce, and choosing them is the user's
call. Monte Carlo, solar load and six-flux are scope beyond the acceptance.

**F-200 (PARTIAL).** Re-run: A1 flux exactness 1.4e-16. A2 plane Poiseuille with a prescribed mass flow gives a dp/dx error of 0.498 %
(limit 1 %), which meets the channel half of clause 1. A3 annular Hagen-Poiseuille gives 0.398 %. E2 gives outlet pressure 0.408 %, u_theta
0.032 % and mid-length 2.669 % (limits 3 %). Mass imbalance is at most about 5e-13 everywhere. F1 bernoulli converges in 2093
iterations. The secant known miss diverges to NaN and the residual-criterion known misses fail as recorded. Three points stop a DONE.
(1) Both "pipe" clauses are run on an annulus. A3 is an annular duct, and E2 is an annulus with both walls in solid-body rotation, not a
pipe. A solid circular-pipe mesh now exists (rcfd-mesh `pipe_mesh`, case generator `pipe_sweep`, ADR 0047). Its body-force Poiseuille
check missed at 3.33 % with the registered settings and passes with non-orthogonal correctors on two layers. A mass-flow Hagen-Poiseuille
on it is S to M. (2) E2 and C2, C3, D1, D2 pass only under a post-hoc settled-field criterion. The registered residual criterion was not
met (the known misses reproduce). (3) "Every boundary kind" includes C5 (prevent-reverse-flow), which is trivially at rest, and no
compressible or coupled kinds exist, because the new kinds are rejected there. Suggested rewording: name the annulus explicitly if the user
accepts it.

**F-210 (DONE).** F1 Taylor-Green kinetic-energy error 2.303e-3 and L2 1.485e-3 (limits 0.02). The Euler order is 1.034. F6 Ghia Re 100
centreline errors are u 3.0e-3 and v 9.0e-3 (limit 0.02), from a transient fractional-step run marched to steady state, and the reference
is vendored at cases/cavity/reference/ghia1982_u_centerline.csv. The BDF2 periodic order (1.513, band 1.7 to 2.3) and the Kim-Moin ratio
(1.130 against 0.5) fail as recorded. Stale ignore: `f3_known_miss_spatial_order` is still `#[ignore]`d but now PASSES (ratio 3.954,
limit 3.0), as its own ignore text says. Under the ADR rules it should be lifted or kept with a note. The "transient lid-driven cavity" is
compared only at its steady end state. If a time history was meant, the acceptance needs a named transient reference. Suggested rewording:
"... marched to steady state agrees with Ghia et al.".

**F-222 (PARTIAL, 0 of 2 clauses).** Only the library rate function exists. The k10 test gives the closed form to 1e-12 and a quasi-steady
error of 6.8e-7 on the corrected set-up. The registered set-up known miss fails as recorded. No laminar premixed flame is computed, so no
Zeldovich rate is compared with published flame data, and no piloted-jet NO trend exists. Blockers: F-054 (constant density only), the
rate constants (UNVERIFIED, not bundled), and the absence of a chosen flame dataset. The acceptance is not measurable until the flame and
band are named.

**F-232 (PARTIAL, 1 of 3 clauses).** S2 detector counts are exact on generated defects. S3c repairs a one-triangle hole in the sphere and
the cube to the original volume (1e-12), and a six-triangle sphere hole to a closed manifold (F = 318, chi = 2) within 0.5 %. Orientation,
duplicate and degenerate repairs are tested on the sphere only. The cube is checked only for the single missing triangle, so "seeded-defect
cube ... removes all defects" is thinly covered. Unmet: exact counts on a public defective surface set (none chosen), self-intersection
and spike detection, and leak-tolerant wrapping with a 2 % volume check on a public leaky model. Data choice is the user's.

**F-243 (PARTIAL, 0 of 2 clauses).** The delivered FW-H, rotor-tone, spectra and proxy work (ADR 0066) is real but outside this acceptance,
and its tests were not re-run here. No impedance boundary and no acoustic quiet boundary exist (the only "sponge" in the source is the VOF
wave tank). The quiet-boundary clause is almost free: F-062's non-reflecting outlet at sigma = 0 already reflects 0.0000 of a 1-D pulse
in `b_no_spurious_reflection`. That test's limit is 0.02, not 1 %, and it has the discrimination caveat noted under F-062. A registered 1 %
test (normal and oblique incidence) is S. The impedance tube (a 1-D wave solver or the compressible solver with an impedance condition,
against the closed form R = (Z - rho c)/(Z + rho c)) is M.

## What was not re-run or not checked

- The heavy SST runs listed at the top (BFS, ADR 0070 three-mesh plate, full-channel Karman measure, ADR 0074 TMR-grid sequence).
- The F-243 FW-H and acoustics tests (rcfd-post), which are outside the acceptance.
- `c3_vent_reversed_k0_long_run_observation` (F-200).
- The other GPU checks of ADR 0038 (K1, K2, S1, S2, C1, M1, E1) and the `gpu_bench` table; only F1 was re-run.
- Any test in a hosted CI or on Linux.
