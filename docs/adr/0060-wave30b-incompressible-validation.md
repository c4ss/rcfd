# ADR 0060: Wave 30B, validation of the incompressible laminar and turbulent solvers against read benchmark data (F-006, F-125)

Status: accepted. Everything under "Reference data read", "Case set-ups", "Mesh sequences" and "Registered checks" was written before
any run of the NEW tests of this wave. Limits are not changed, widened, replaced or deleted after a run. A miss is reported as
measured and the assertion moves to an ignored `*_known_miss` test with the registered limit intact; the cause is diagnosed (by
bisection: time step, mesh, domain size), never tuned away. A test set-up defect gets a NEW corrected test and the old one stays as a
record. Addenda (exact numbers, deviations, diagnosis findings) may be appended at the end without touching a registered limit.
Clean-room: published literature and public data only; no vendor code, binaries or outputs; nothing from vendor material.
No solver code is changed by this wave; if a defect is proven the work stops at the diagnosis (ignored known-miss test, addendum).

## Audit of what was validated before this wave (HEAD 9d06638)

| Benchmark | Existing test | Reference | Status |
|---|---|---|---|
| Lid-driven cavity | `rcfd-fv/tests/cavity.rs` | Ghia, Ghia and Shin (1982) u (Re 100/400/1000), v (Re 100), repo CSVs | Re 100 on 40x40 in the normal suite, Re 400 (80x80) and 1000 (128x128) ignored; one mesh each, 2 % of lid speed; no grid-convergence study |
| Laminar backward step, ER 2 | `rcfd-fv/tests/step_laminar.rs` | Erturk 11.834 and Gartling 12.20 (Re_D = 800), via arXiv:2507.16509 Table 2; Re_D 200/400 from that paper's own FV results | one mesh [16,320,32,32], ignored, 5 % window |
| Laminar flat plate (Blasius) | none (case file cites correlations only) | - | not validated |
| Turbulent flat plate | `rcfd-turb/tests/flat_plate_{sst,sa,bsl_sst}.rs` | NASA TMR CFL3D, White and power-law Cf | one known miss registered (SST vs White) |
| Turbulent channel | `rcfd-turb/tests/channel_{kepsilon,sst}.rs` | MKM DNS Re_tau 395 (values typed from a file read in wave 6a, then "UNVERIFIED, from memory" in a later table) | 10 % limits, one mesh each |
| Cylinder, Re 100 | `rcfd-fv/tests/cylinder.rs` | St 0.165 (Williamson) from arXiv:2303.09262 | one mesh and dt, ignored, 5 %, no drag, recorded St 0.1588 |

Gaps this wave closes: mesh sequences with an observed order and GCI for every benchmark, a laminar Blasius test, drag for the
cylinder, a Re_tau = 180 channel set-up whose reference table was read in this wave, and reference sources read in this wave.

## Disclosure of prior knowledge (limits are not blind for three cases)

Before this ADR was written the following results of EXISTING tests were known to the author: cavity Re 100 (n=40) max |u-Ghia| 0.0028
and max |v-Ghia| 0.0086; Re 400 (n=80) 0.0032; Re 1000 (n=128, TVD van Leer) 0.0044 (run once for timing, 94 s); backward step Re_h 400
x_r/h = 11.672 on [16,320,32,32] (from `docs/reference-data.md`); cylinder St = 0.1588 on 128x100 with dt 0.05 (same file, -3.8 % from
0.165). The limits below are justified from the reference scatter and an a-priori discretisation allowance, not from those numbers;
nevertheless the St limit (3 %) is stricter than the earlier 5 % and the earlier measurement lies outside it, which is deliberate: the
purpose is to find out whether the miss is discretisation (mesh, dt) or something else. The channel Re_tau = 178 and Blasius runs are
blind (no earlier measurement exists).

## Reference data read in this wave

Every number below was read as raw text (`curl` + `pdftotext -layout`, or the raw data file) on 2026-10-17; none is typed from memory.
Retrieval date for all entries: 2026-10-17.

R1. Lid-driven cavity, Ghia, Ghia and Shin (1982), J. Comput. Phys. 48, 387-411 (original not accessible).
 * arXiv:1807.04896 (https://arxiv.org/pdf/1807.04896) Table A-1, "Ghia, Table 1, p. 398" next to "Botella, Table 9, p. 426" (Botella and
   Peyret, Comput. Fluids 27 (1998) 421, spectral), Re = 1000, u along x = 0.5. Ghia / Botella at y = 0.9766 0.65928 / 0.6644227;
   0.9688 0.57492 / 0.5808359; 0.9609 0.51117 / 0.5169277; 0.9531 0.46604 / 0.4723329; 0.8516 0.33304 / 0.3372212; 0.7344 0.18719 /
   0.1886747; 0.6172 0.05702 / 0.0570178; 0.5 -0.06080 / -0.0620561; 0.4531 -0.10648 / -0.1081999; 0.2813 -0.27805 / -0.2803696; 0.1719
   -0.38289 / -0.3885691; 0.1016 -0.29730 / -0.3004561; 0.0703 -0.22220 / -0.2228955; 0.0625 -0.20196 / -0.2023300; 0.0547 -0.18109 /
   -0.1812881. Largest |Ghia - Botella| = 0.0063 (y = 0.9531); this is the evidence for how much of a deviation from Ghia is Ghia's own
   (129x129 grid) error. Stored in `validation-data/reference/cavity_re1000_ghia_botella_u.csv`.
 * Re = 100 and 400 u (17 rows), read from the raw CSV files `ghia_Re100_u_centerline.csv`, `ghia_Re400_u_centerline.csv` of
   github.com/PN-CourseWork/02689-AdvancedNumericalAlgorithmP3 (a student repository: a secondary copy): all 16 rows each equal, to the last
   digit, the in-repo `cases/cavity/reference/ghia1982_u_centerline.csv` (the y = 0.9766 row is absent from that copy). v at Re 100: in-repo
   table only (verified in wave 6a against two secondary copies). v at Re 400 and 1000: data not obtained (not used).
 * Ghia original paper: data not obtained.

R2. Backward-facing step, expansion ratio 2, Re_D = U_mean D_h / nu = 2 Re_h (https://arxiv.org/pdf/2507.16509 Table 2, a 2025 MHD paper
   that tabulates published values of x_r/h at Re = 800; the originals were not read): Rogers and Kwak 11.48, Barton 11.51, Erturk 11.834,
   Kim and Moin 11.90, Lee and Mateescu 12.00, Guj and Stella 12.05, Grigoriev and Dargush 12.18, Keskar and Lyn 12.19, Gartling 12.20, Gresho
   et al. 12.20: ten published values from 11.48 to 12.20 (mid-range 11.84, half-width 3.0 %). The same paper's Table 1 (its OWN finite
   volume results, 3.00, 4.95, 6.75, 8.10, ... at Re 100, 200, 300, 400) is NOT a published benchmark and is not a registered reference. Stored in
   `validation-data/reference/bfs_er2_re800_xr_literature.csv`. Armaly et al. (1983): tables/figures not obtained (the washington.edu copy
   timed out, the Cambridge page is paywalled); data not obtained, no check. Gartling (1990) original: not obtained. Reattachment values
   for Re_D < 800 from a published benchmark: data not obtained (arXiv:1806.10251 Fig. 10 is a graph only; the Fomin and Fomina 2017
   PDF holds Re >= 1000 tables and a graph). Set-up differences against the references: rcfd uses a 2 h long inlet channel with a
   parabolic profile and a 40 h outlet channel; the references' inlet placement differs in part (a search snippet quoting a GitHub pull
   request states that a channel upstream of the step shortens x_r, Barton 1997; not read, stated only as a possible set-up
   sensitivity, nothing is tuned to it).

R3. Blasius flat plate. arXiv:2204.02215 (Lal and Martin, 2022, https://arxiv.org/pdf/2204.02215) Table 1 and the benchmark tables, 32-digit
   Taylor-series solution of f''' + (1/2) f f'' = 0, f(0) = f'(0) = 0, f'(inf) = 1, eta = y sqrt(U/(nu x)): f''(0) = 0.33205733621519629893718;
   f'(1) = 0.32978003124967, f'(2) = 0.62976573650239, f'(3) = 0.84604444365799, f'(4) = 0.95551822981069, f'(5) = 0.99154190016439;
   f(15) = 13.27921234247949718 so the displacement thickness is delta* = (15 - f(15)) sqrt(nu x / U) = 1.72078765752 sqrt(nu x/U). Derived
   (textbook algebra, tau_w = mu U sqrt(U/(nu x)) f''(0)): Cf = 2 f''(0) / sqrt(Re_x) = 0.664115 / sqrt(Re_x). Stored in
   `validation-data/reference/blasius_lal_martin_2022.csv`. (The older Jaffer/Schlichting reading of 0.664 is consistent.)

R4. Channel DNS, Moser, Kim and Mansour, Re_tau = 178.12 ("Re_tau = 180"): the published statistics file
   https://torroja.dmt.upm.es/turbdata/agard/chapter5/PCH10/chan180/profiles/chan180.means (header: "Authors: Moser, Kim & Mansour, DNS of
   Turbulent Channel Flow up to Re_tau=590, 1998", "Re_tau = 178.12", "Normalization: U_tau, h", 65 half-channel points), mirror of the
   original site. Read as raw text, trapezoid-integrated and interpolated linearly in y+ (computed before any run): U_c+ = 18.301 (file value at
   y = 1), U_b+ = integral_0^1 U dy = 15.6787 (Re_b = 2 U_b = 5585 in the nominal wall-unit normalisation, i.e. the well-known Re_b about
   5600), U+ at y+ = 5, 10, 20, 30, 50, 100, 150: 4.811, 8.522, 12.287, 13.868, 15.309, 17.147, 18.131. Derived friction coefficient
   (the correlations of Dean/Colebrook/Moody were not obtained and are not used): Cf = 2 / U_b+^2 = 0.008135 (based on U_b). Redistribution
   terms of the file are not stated; only the sampled numbers above and the two integrals are stored
   (`validation-data/reference/mkm_chan180_sampled.csv`), with the URL, not the file.

R5. Cylinder, Re = 100, unbounded 2-D flow (values: time-averaged drag C_D, Strouhal number St):
 * arXiv:2303.09262 Table 1 (https://arxiv.org/pdf/2303.09262), Newtonian rows: Kravchenko et al. 1.314 / 0.164; Oliveira 1.370 / 0.167;
   Sivakumar et al. 1.325 / 0.164; Posdziech and Grundmann 1.325 / 0.164; Mossaz et al. 1.328 / 0.164; Qu et al. 1.319 / 0.165; Constant et
   al. 1.37 / 0.165; Peng et al. 1.361 / 0.165; Williamson (experiment) St 0.165, no C_D.
 * arXiv:1401.6513 Table 3 (https://arxiv.org/pdf/1401.6513): Ding 1.325 / 0.164 (cited), the paper's own VIP result 1.328 / 0.164.
 Range: C_D 1.314 to 1.370 (mid 1.342, half-width 2.1 %), St 0.164 to 0.167 (mid-range 0.1655, half-width 0.9 %), experiment 0.165. Stored in
 `validation-data/reference/cylinder_re100_literature.csv`. Original papers not read.

## Case set-ups

All tests are new files in `crates/rcfd-api/tests/` (the crate already depends on rcfd-fv, rcfd-mesh, rcfd-turb and rcfd-post). No
`Symmetry` boundary is used where plug flow matters (the plate top is a `Freestream`, the channel is periodic with no-slip walls).

**V1 cavity** (`validation_cavity_ghia.rs`): unit square, lid speed 1, SIMPLEC, `rcfd_mesh::gen::cavity_2d`. Re 100: linear upwind;
Re 400: linear upwind; Re 1000: TVD van Leer (the schemes of the existing tests). Convergence to max change 1e-7. Centreline velocities
interpolated exactly as in `cavity.rs` (cell-line average, 4-point Lagrange) at the Ghia stations.

**V2 backward step** (`validation_bfs_step.rs`): ER 2, h = 1, 2 h inlet channel with the parabolic
mean-1 profile, 40 h outlet, Re_h = 400 (Re_D = 800), SIMPLEC, van Leer TVD, residual 1e-7, reattachment from the last negative-to-positive
crossing of u in the first cell row above the floor, exactly as in `step_laminar.rs`.

**V3 Blasius** (`validation_blasius_plate.rs`): U = 1, L = 1, nu = 1e-5 (Re_L = 1e5, laminar), plate wall on ymin from x = 0 (uniform inlet
at xmin, as in the TMR-style plate tests), top boundary at y = 0.3 (19 outlet boundary-layer thicknesses) a `Freestream` for u and p, outlet
zero-gradient u and fixed p = 0, linear upwind, SIMPLEC. Cf from the wall-face shear on the wall-cell values; profile and displacement
thickness taken from the cell column nearest x = 0.5.

**V4 channel Re_tau 178.12** (`validation_channel_re180.rs`): periodic channel of half height 1 (`box_2d_graded`, periodic in x), no-slip
walls, uniform body force 1 so u_tau = 1 exactly, nu = 1/178.12 (the DNS value, not the nominal 180), SIMPLEC, k-omega SST (wall-resolved
mesh) and standard k-epsilon (wall functions). Bulk velocity is volume-weighted.

**V5 cylinder** (`validation_cylinder_re100.rs`): `cylinder_ogrid`, D = 1, r_out = 20 (20 D; blockage-like error of the far field is a
known possible error source, cf. the domain test in the bisection list), U = 1, nu = 0.01, PISO (`PimpleSettings::piso`), linear upwind,
`Freestream` far field exactly as in `cylinder.rs`, same initial disturbance. Run to t = 160, statistics from t > 60. St from upward zero
crossings of v at the probe (3, 0, 0.05) over the last 8 periods (whole periods); C_D = F_x / (0.5 U^2 D depth) with depth 0.1 from
`rcfd_post::integrals::wall_force_and_moment` (pressure on the patch from `pressure_bc` / `boundary_values`), averaged over a whole number of
shedding periods (integer periods between zero crossings of the lift).

## Mesh sequences (refinement ratio 2 or sqrt 2, constant relative grading)

* V1 Re 100: n = 20, 40, 80. Re 400: n = 40, 80, 160. Re 1000: n = 32, 64, 128 (n = 256 is not run: about 10 min and more).
* V2: [nxu,nxd,nyi,nys] = [8,160,16,16], [16,320,32,32], [32,640,64,64] (cell width and height both halved).
* V3: nx = ny = 60, 120, 240; y grading `Expansion(60)` (last/first), x uniform.
* V4 SST: `Grading::Symmetric(40)` in y, ny = 40, 56, 80 (ratio sqrt 2), first-cell y+ below 1 on all; k-epsilon (wall functions, uniform
  mesh): ny = 8, 12, 16 (cell-centre y+ 22, 15, 11); for the wall-function model a grid-convergence index is not meaningful (the model
  depends on y+), only the spread of U_b+ over the meshes is reported and bounded.
* V5: (n_theta, n_r, dt) = (64, 50, 0.1), (90, 70, 0.07), (128, 100, 0.05) (ratio about sqrt 2 in all of h and dt), outer grading ratio 150.

Observed order and GCI (Roache, safety factor 1.25) are computed on a scalar functional of the solution itself, never on the deviation
from the reference: with f1 (coarse), f2, f3 (fine) and constant ratio r: p = ln|(f1-f2)/(f2-f3)| / ln r, GCI_fine = 1.25 |f3-f2| /
(f3 (r^p - 1)) with p limited to [1, 2] for the GCI. Functionals: V1 u_min on the vertical centreline (the extremum of the 4-point
interpolant over the sampled range) and the RMS over the Ghia stations of the difference between successive meshes; V2 x_r/h; V3
Cf sqrt(Re_x) at x = 0.5; V4 U_b+; V5 St and C_D.

## Registered checks (fixed before any run of the new tests)

Limits are on the FINEST mesh; coarser meshes are reported only. Justification is given with each group.

**V1 cavity.** Reference scatter: |Ghia - Botella| <= 0.0063 at Re 1000 (R1), i.e. Ghia carries about 0.006 of its own error at the
highest Re and less at lower Re (the lower the Re the smaller the error of a 129x129 solution). Allowance for the second-order scheme on
the finest mesh: 0.004 of the lid speed.
 * C1 Re 100 n = 80: max |u - Ghia| <= 0.010 and max |v - Ghia| <= 0.010 (all stations of the stored tables).
 * C2 Re 400 n = 160: max |u - Ghia| <= 0.010.
 * C3 Re 1000 n = 128: max |u - Ghia| <= 0.012 (0.0063 + 0.004 allowance, rounded up; the finest mesh of this test is half as fine as the
   129x129 of Ghia).
 * C4 Re 1000 n = 128: max |u - Botella| <= 0.012 (the more accurate spectral solution; same limit, so a pass against both is not an
   artefact of Ghia's error).
 * C5 grid convergence, every Re: observed order p of the successive-difference RMS >= 0.9 and GCI_fine of u_min <= 1.0 % of |u_min|.
 * C6 every run converges (max change below 1e-7 within 12000 iterations).

**V2 step.** Reference scatter: ten published values, 11.48 to 12.20, half-width 3.0 % about 11.84 (R2). Allowance 2 % for the finest-mesh
discretisation error and the difference of inlet treatment, giving +-5 % about 11.84, i.e. [11.248, 12.432] (this equals the 5 % of the
earlier test; it is not tightened because the inlet-channel difference could account for part of the spread).
 * S1 x_r/h on the finest mesh in [11.248, 12.432].
 * S2 GCI_fine of x_r/h <= 3 %, observed order p >= 0.9.
 * S3 convergence of every run to 1e-7.
Re_D < 800 and Armaly: data not obtained, no check.

**V3 Blasius.** The reference is exact (32 digits), so the whole limit is discretisation and leading-edge error. A second-order scheme
with at least 50 cells across the boundary layer at x = 0.25 on the finest mesh gives about 1 percent; the singular leading edge of a
uniform start and the finite outlet add a few tenths of percent at Re_x >= 2.5e4. Allowance 3 %.
 * B1 Cf sqrt(Re_x) within 3 % of 0.664115 at x = 0.25, 0.5, 0.75, 0.9 (Re_x 2.5e4 .. 9e4), finest mesh.
 * B2 u/U at x = 0.5 within 0.02 (absolute) of f'(eta) at eta = 1, 2, 3, 4, 5 (R3), finest mesh.
 * B3 displacement thickness delta* sqrt(U/(nu x)) at x = 0.5 within 3 % of 1.72079.
 * B4 grid convergence: observed order p of Cf sqrt(Re_x)(x = 0.5) >= 1.0 and GCI_fine <= 1.0 %.

**V4 channel.** Reference uncertainty: DNS statistical error well under 1 % in U+ (not quantified here, stated as an allowance); discretisation
on the wall-resolved SST mesh <= 1 %; model-form error is what is being measured, and is bounded by an engineering allowance of the size of
the repo's existing Re_tau = 395 bounds (10 %) tightened where the quantity is integral.
 * D1 (SST, finest) U_b+ within 5 % of 15.679 and C_f = 2/U_b+^2 within 10 % of 0.008135 (equivalent statements, listed separately).
 * D2 (SST, finest) U_c+ within 5 % of 18.301.
 * D3 (SST, finest) U+ at y+ = 10, 20, 30, 50, 100 within 8 % of 8.522, 12.287, 13.868, 15.309, 17.147.
 * D4 (SST) GCI_fine of U_b+ <= 1 %, p >= 1.0.
 * D5 (standard k-epsilon, wall functions, ny 8/12/16) U_b+ within 10 % of 15.679 and U_c+ within 10 % of 18.301 on every mesh; U_b+ spread
   (max-min)/mean over the three meshes <= 5 %.
 * D6 convergence (change below 1e-6) on every run.

**V5 cylinder.** Reference scatter: St 0.164 to 0.167, half-width 0.9 % about 0.1655, experiment 0.165; C_D 1.314 to 1.370, half-width
2.1 % about 1.342. Allowance for discretisation (time step included) on the finest mesh: 2 % for both.
 * K1 St within 3 % of 0.165, i.e. [0.16005, 0.16995].
 * K2 time-averaged C_D within 4 % of 1.342, i.e. [1.2883, 1.3957].
 * K3 grid convergence: GCI_fine of St <= 3 % and of C_D <= 3 %.
 * K4 sustained shedding detected (at least 9 zero crossings after t = 60) on every mesh.
 The tests of V5 are ignored (cost above the 5 minute guideline); command: `cargo test --release -p rcfd-api --test
 validation_cylinder_re100 -- --ignored`. If K1 misses, the registered bisection order is: (i) the same mesh with dt halved; (ii) the
 same dt with the next finer mesh; (iii) r_out = 30 D (far-field position); (iv) probe-position independence (a second probe at (2, 0, 0.1)).

Other fixed conventions: tests are named by the check (`c1_...`); known misses are ignored tests named `*_known_miss` with the limit text
unchanged; every printed table lists measured, limit and verdict.

## What is not validated (stated, not hidden)

Armaly et al. (1983) tables and the Gartling and Ghia originals (not read); Ghia v at Re 400/1000; backward-step x_r for Re_D < 800
from a published benchmark; the Moody/Colebrook/Dean pipe and channel friction correlations (not read; the friction check uses the
DNS-derived 2/U_b+^2); turbulent cylinder; time-resolved quantities of the channel.

## Addendum: measured results (2026-10-17)

All limits above are unchanged. Commands: `cargo test --release -p rcfd-api --test validation_<name>`; ignored tests with `-- --ignored`.

**V1 cavity (met except C5 at Re 1000).** Max |u - Ghia|: Re 100 n = 80 0.00449 (limit 0.010), |v - Ghia| 0.00915 (0.010); Re 400 n = 160 0.00184 (0.010);
Re 1000 n = 128 0.00443 against Ghia and 0.00539 against Botella (0.012). C5: observed order of the RMS 2.14, 1.98, 1.85; GCI_fine(u_min) 0.29 %,
0.19 %, 1.41 % (limit 1.0 %). MISS C5 at Re 1000 (ignored `c5_re1000_gci_known_miss`). Diagnosis: in the asymptotic range (p(u_min) 1.96), the
Richardson-extrapolated u_min -0.3895 is within 0.001 of Botella's -0.38857 at y = 0.1719; n = 128 is too coarse for a 1 % GCI at Re 1000
(u_min -0.32416, -0.37270, -0.38521). No solver defect. C6 converged on every run.

**V2 step (all met).** x_r/h = 11.2393, 11.6720, 11.7965 on 5248, 20992, 83968 cells (3640 iterations on the fine mesh, 2166 s): inside
[11.248, 12.432] (the coarse mesh alone would not be), -0.37 % from the mid-range 11.84; observed order 1.80, GCI_fine 0.53 % (limit 3 %).
Data not obtained: Armaly (1983), Re_D < 800 benchmark values, Gartling original.

**V3 Blasius (met with a corrected set-up).** SET-UP DEFECT: the registered plate starting right at the Dirichlet inlet missed everything
(Cf sqrt(Re_x) deviation 9.6 to 14.4 %, profile 0.076, B4 p = 0.64; ignored `b1_b4_plate_at_inlet_setup_defect_known_miss`, limits intact). Bisection:
independent of top boundary (open/slip/fixed), domain height, scheme, coupling, grading; it is the inlet/leading-edge interaction. The corrected test
adds a 0.5 long shear-free moving-wall belt upstream of the plate (no solver change). Corrected results on 60/120/240 cells across: Cf sqrt(Re_x)
within 0.27, 0.22, 0.46, 0.72 % at x = 0.25, 0.5, 0.75, 0.9 (limit 3 %); profile max |u/U - f'| 0.00228 (0.02); delta* + 0.013 %; order 1.11, GCI_fine 0.155 %
(limits 1.0, 1 %). DEVIATION decided after seeing a miss: the registered B3 integral over the whole column missed (9.5 %, ill-conditioned because the
free-stream displacement integral accumulates the outer-boundary error; ignored `b3_whole_column_displacement_thickness_known_miss`, limit 3 % intact);
the corrected measurement integrates to eta = 10 with the local edge velocity and meets the same 3 % limit. Both disclosed here.

**V4 channel Re_tau 178.** SST (wall resolved, ny 40/56/80, first-cell y+ 0.40/0.29/0.21): U_b+ 15.696, 15.542, 15.418 (DNS 15.679, finest +1.66 %,
limit 5 %); Cf 3.41 % (10 %); U_c+ 17.954 (1.9 %, 5 %); U+ at y+ 10, 20, 30, 50, 100 within 2.4, 5.8, 4.9, 2.0, 0.5 % (8 %). D6 met. MISS D4: observed
order 0.63 (>= 1.0) and GCI_fine 2.43 % (1 %), ignored `d4_sst_grid_convergence_known_miss`. Diagnosis: U_b+ keeps decreasing monotonically under
refinement: ny 24 16.066, 112 15.335, 160 15.274 (successive differences 0.37, 0.155, 0.124, 0.083, 0.061), so the sequence has not reached the
asymptotic range at ny = 80 and tends to about 15.1 to 15.2, i.e. the model's own mesh-converged value is about 3 % below the DNS; no defect proven.
k-epsilon with wall functions (ny 8/12/16, cell-centre y+ 22/15/11): U_b+ 16.638, 18.256, 15.115 (+6.1, +16.4, -3.6 %); the ny = 8 and 16 deviations and all
U_c+ points pass except ny = 12; MISS D5 ny = 12 U_b+ (0.164 > 0.10) and spread 18.8 % (> 5 %), ignored `d5_k_epsilon_wall_function_known_miss`. Diagnosis
(`diagnose_channel_sequences`): ny 4..32 gives U_b+ 16.72, 16.64, 16.59, 16.64, 17.01, 18.26 (ny 12), 16.43, 15.12, 13.37, 12.29, 11.10 (y+ 44 down to 5.6):
a bump near y+ 15 and a steady fall to -29 % when the first cell centre enters the buffer layer, i.e. a log-law wall function is used where the cell
centre is outside its range of validity (y+ below about 30); no mesh-converged value exists, no defect proven.

**V5 cylinder (ignored, 448 s).** 64x50 dt 0.1, 90x70 dt 0.07, 128x100 dt 0.05: St 0.14665, 0.15450, 0.15878 (fine -3.77 % of 0.165; K1 limit 3 %: MISS);
C_D 1.2514, 1.2873, 1.3100 (fine -2.39 % of 1.342, K2 limit 4 %: met); observed order St 1.75, C_D 1.33, GCI_fine St 4.05 %, C_D 3.67 % (K3 limit 3 %: MISS);
14, 15, 16 zero crossings (K4 met). Misses in ignored `k1_k3_strouhal_and_gci_known_miss`. Bisection in the registered order: (i) fine mesh with dt 0.025:
St 0.16175 (-1.97 % of 0.165, inside 3 %), C_D 1.3234; (iii) r_out = 30 at dt 0.05: St 0.15781, C_D 1.2961 (-0.6 %, -1.1 %, so the far field is not the cause).
Cause: temporal discretisation error of the first-order-in-time PISO step; mesh and dt were refined together, so the sequence is dominated by dt (halving
dt alone moves St by +1.9 %). Steps (ii) and (iv) were not run. No defect proven; the registered K1 and K3 stand as missed.

**Data pack.** Five datasets added to `validation-data/manifest.json` (`python validation-data/fetch.py verify` ok for all). `cargo test -p rcfd-cli --test data`
currently fails with "manifest: sod-exact-marzouk2020: case 'sod' does not exist in cases/", an entry added by wave 30C, not by this wave.

**Moved from unverified to verified:** see docs/reference-data.md, wave 30B section. Not obtained: Armaly (1983), Gartling and Ghia originals, Ghia v at Re 400/1000,
Moody/Colebrook/Dean, Williamson original.
