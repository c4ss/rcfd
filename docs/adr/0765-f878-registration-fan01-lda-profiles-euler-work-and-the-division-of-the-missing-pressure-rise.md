# ADR 0765: F-878 registration: the FAN-01 rotor against the measured LDA profiles, the Euler work and the division of the missing pressure rise

Status: **REGISTERED 2026-11-22, written before any code of F-878 and before any run of its checks.** Before this text
no test of F-878 was written, no solver was run and no simulated profile was seen. What was done: reading (below), one
reading script over `docs/roadmap.json`, the fetch of the published record of the measurement, the arithmetic on the
measured data alone (section "Arithmetic done beforehand"), and one build and run of the test binary `docs_site`, to
check the links of this file. Results go in addenda at the end; nothing above them is changed after a run.

It answers the brief `docs/adr/briefs/f878-fan01-lda-profiles.md` and follows `docs/process/agent-rules.md`, version 5.
Branch `f878-fan01-lda-profiles` from main 2bab1863; scratch `w49i-work/f878-fan01-lda-profiles-scratch`. No solver
source is changed by this work: it adds one test file, a reference fragment, records and text.

Size (agent): partial; 0; 4; 2; 400000

The expected outcome is partial, and the registration says why before the run: velocity profiles fix the work of the
rotor and the kinetic energy that leaves it, but not its losses, and without the losses the missing pressure rise can
be divided only between bounds (section "The rule that divides"). The measured data alone already give those bounds a
width of 94 Pa, against 52 Pa to be divided. What the run adds is where the simulated rotor lies between them, the
comparison at each radius, and the division under one stated condition.

## What is asked

Roadmap item F-878 (read by script from `docs/roadmap.json`). Its acceptance: "Every limit not stated here is registered
in the ADR before the run. The circumferentially averaged meridional and circumferential velocity of the FAN-01
design-point solve at the measured plane behind the trailing edge is compared with the LDA profiles of the published
record, which a page of the repository records with their citation, at each measured radius, within a band registered
before the comparison; the Euler work of the measured and of the simulated profile is reported with the measured shaft
power, and the report states what part of the miss of the pressure rise of F-870 is the rotor's and what part the
installation's. A miss is recorded as a miss with its numbers."

The brief's five tasks: the measured profiles, traced; the planes, the average, the bands, the Euler work integral and
the rule of division, registered before the comparison; the comparison with the recorded state of
[ADR 0762](0762-f870-registration-fan01-design-point-state-band-and-ranked-gap.md); what the limit cycle and the missed
mesh gates do to it; the result held as a test.

## What was read

Whole: the brief; `AGENTS.md`; `docs/process/agent-rules.md` (version 5);
[ADR 0762](0762-f870-registration-fan01-design-point-state-band-and-ranked-gap.md);
`crates/rcfd-api/tests/f870_fan01_design_point.rs`; `docs/reference-data.d/0762-F-870.part`;
`docs/roadmap/proposals/0762-F-870.json`; `docs/process/tolerances.md` (the table);
`validation-data/reference/w34d_fan01_cad_sections.csv`. Of `docs/validation-data.md`: lines 60 to 145. Of
`docs/roadmap.json`, by script: F-870, F-878 and F-880 in full. Of the source, to learn what a test can read from a
solve and no more: the field list of `Simulation` and its accessors `velocity()` and `pressure()` in
`crates/rcfd-api/src/sim.rs`, the accessor list of `crates/rcfd-mesh/src/mesh.rs`, the struct `FvGeometry`, the lines of
`crates/rcfd-fv/src/sources.rs` that say the centrifugal term is part of the frame source, and lines 340 to 440 of
`crates/rcfd-api/tests/fan01_gci_w37c.rs`. Of
[ADR 0086](0086-wave37c-fan01-solution-verification.md): the lines of the bladed balance and of the empty passage that
ADR 0762 cites (found by keyword), not the whole.

The published record of the measurement, fetched on 2026-11-22 from zenodo.org through its public API and read (next
section). Not read: Kromer's thesis and SAE 2016-01-1805 (not fetched; F-877 gives them to a person), the notebook of
the record (fetched, not opened), the other FAN-01 ADRs and tests (known through ADR 0762's table).

## The measured profiles, traced (task 1)

**The record.** "FAN-01 Benchmark: Low-pressure axial fan in a short duct", version v6, Kroemer, Junger, Czwielong,
Heidegger, Kaltenbacher, Schoder, Becker, Zenodo, doi:10.5281/zenodo.23943494 (concept doi:10.5281/zenodo.8418340),
published 2026-11-04, licence CC BY 4.0. Fetched: the record's metadata, `lda_data.zip`, `README.md`,
`LA_Case4_AxialFan_measurements.pdf` (the description, 19 pages) and the notebook. Each file's MD5 equals the checksum
the record states.

| File | Bytes | MD5 (equal to the record's) | SHA-256 |
| --- | --- | --- | --- |
| `lda_data.zip` | 750 500 | 4cafb1602e81dc18f1681e6facd495a0 | 3c527f49b02e9e1b80718915a79468381e927dfb8239ba68f7a33d575df4f707 |
| `LA_Case4_AxialFan_measurements.pdf` | 1 639 807 | 056fc203852d6186a583f3ade8f2603b | 547d17de2d3c8ab025624e17d3d61abd8253fedacd77cf079cd1805c8a76e8a6 |
| `README.md` | 6 527 | 7162aa176fe9b1ed73d9ef8f00a74fa3 | 7e58f996b82e5df32dbd5cf55228789407c3eed8ec70101e9c6b235598035161 |
| `lda_data/time_averaged/n1ug_1_4_time_averaged_pressure_side.mat` | 2 428 | (inside the archive) | 61d0054ee9680f920def32e3bc55bc3dbc4311d92fe67a60351c2499f340fcff |
| `lda_data/time_averaged/n1ug_1_4_time_averaged_suction_side.mat` | 2 460 | | 4b1ec6659288941ee24dc45edbeedfee6dfc30343e7a46285bc5271520a3243b |
| `lda_data/ensemble_averaged/n1ug_1_4_ensemble_averaged_pressure_side.mat` | 192 620 | | 4d1a2aa7a20c30c0b8d299040c5b5de77252f1065539c72501079af0d4a293cb |
| `lda_data/ensemble_averaged/n1ug_1_4_ensemble_averaged_suction_side.mat` | 185 132 | | 08538c6e97c7653db791963fc2b34bbd7cd006e7d361638654c32c37cadbe32f |

The description was read as text (`pdftotext -layout`): whole, except that of its figures only the captions and axis
labels come through. The four `.mat` files were read with a reader of the MAT level-5 format written for this work
(standard library only; the `.h5` copies were not read, the machine has no HDF5 reader).

**What the record holds** (description, Table 2, Table 4 and Appendix A.4; the files themselves):

| | |
| --- | --- |
| Instrument | two-component LDA (Dantec 2D FiberFlow, BSA P80); 12 minutes or 2.5e6 samples per position |
| Operating point | one: the design point, 1.4 m^3/s (file names `n1ug_1_4`); the speed 1486 per minute |
| Planes | two: x = 20 mm ("suction side") and x = 90 mm ("pressure side"), x from the upstream face of the hub in flow direction (`x_in_mm` of the files holds 20 and 90) |
| Radii | 26 on one radial line: `radius_norm` = r / 250 mm from 0.532 to 0.952 in steps of 0.020, then 0.960, 0.968, 0.976, 0.984: r = 133 to 246 mm. The annulus is 124 to 250 mm; the 9 mm at the hub and the 4 mm at the casing are not measured |
| Components | three: `ux` (the record calls it meridional; it is the axial component), `uphi` (circumferential), `ur` (radial), each as mean and rms, in m/s, stored to 0.01 m/s |
| Time-averaged | 26 values per plane and component |
| Phase-resolved | "ensemble-averaged": 26 radii times 180 bins of 2 degrees of the rotor angle, means, rms and `k`. The phase between bin and rotor is not part of the record (description, section on the coordinate system) |

Table 4 of the description places the x = 90 mm line 2.2 mm behind the trailing edge at r = 133 mm and 27.2 mm behind it
at r = 246 mm, and the x = 20 mm line 5.0 mm ahead of the leading edge at r = 133 mm and 21.5 mm at r = 246 mm.

**The uncertainty of the LDA is not stated** in the description, the README or the files: no accuracy of the velocity,
no statement on velocity-bias correction or on the weighting of the time average, no seeding statement. The band below
therefore has no measurement part, as ADR 0762's had none.

**The datum is settled.** `docs/validation-data.md` says: "LDA profiles 20 mm and 90 (or 110) mm from a datum that the
sources read do not settle." The v6 description settles it in three places: the section on the coordinate system (the
origin is on the rotor axis in the flat upstream face of the hub disk, and `x_in_mm` of the LDA files is measured from
it), Table 4 (the two lines, as above), and its table of errata (Appendix C), which lists "located 110 mm behind the
hub" of the scripts of versions 2 to 5 as an error whose correct reading is 90 mm downstream of the upstream hub face,
and says the drawing of the LDA positions in the SAE paper (its Fig. 7) is not to scale. One reading is carried, x = 90
mm. What stays open is smaller and is carried as a number: the place of that plane in the frame of the mesh (next).

**The plane in the mesh.** The mesh of ADR 0762 puts the leading edge of the root section at z = 0.1 m and the other
sections by the column `le_x_mm` of `w34d_fan01_cad_sections.csv`, whose frame is that of the derivation of wave 34D,
not of the record. The two frames are tied by the blade edges that the description gives in the record's frame (Table 3,
"CAD"): leading edge 24.0 mm at r = 130 mm and 41.5 mm at r = 245 mm, trailing edge 89.3 mm and 62.8 mm. The CSV gives,
at the same radii, leading edge -21.10 and -2.43 mm, and trailing edge (`le_x + chord cos(stagger)`) 44.10 and 16.99 mm.
The four offsets are 45.10, 43.93, 45.20 and 45.81 mm. The two definitions of an edge differ (the CSV's is the end of
the chord line, the description's the axial extreme of the surface), and the tip pair differs most. Registered: the
offset is **45.15 mm**, the mean of the two values at r = 130 mm, where the plane is nearest to the blade; it agrees with
the root (21.6 mm in Table 3 against -23.7 mm in the CSV: 45.3 mm). Its uncertainty is registered as **+-1.3 mm**, the
largest distance of the four offsets from it (45.15 - 43.93 = 1.22, rounded up). So `z_mesh = 0.1 m + (x - 21.45 mm)`:
the planes are at z = 0.16855 m (x = 90) and z = 0.09855 m (x = 20), and the comparison is repeated at x = 88.7 and 91.3
mm for the band.

**The transcription.** The 26 values of each time-averaged quantity, and the per-radius bin means of the ensemble files
that the integrals below need, go into `docs/investigations/f878-fan01-lda-profiles/lda_profiles.csv` with the script
that wrote it, and the values the test uses are constants of the test file, checked against that CSV by the test. The
citation and checksums go into `docs/reference-data.d/0765-F-878.part`. `validation-data/manifest.json` is not edited
by this work; whether the transcription becomes a file of the data pack is left to the lead.

## Arithmetic done beforehand (the measured data alone)

By `lda_measured.py` (scratch folder; committed with the results). With rho = 1.2 kg/m^3 (assumed, as in ADR 0075 and
ADR 0762: the density of the measurement is not known, F-877), omega = 2 pi 1486 / 60 = 155.61 1/s, over the measured
span 133 to 246 mm (90.88 per cent of the annulus area), trapezoid rule on the 26 radii:

| | from the time-averaged files (products of the means) | from the ensemble files (bin means of the products) |
| --- | --- | --- |
| volume flow through the measured span, x = 20 / x = 90 | 1.3221 / 1.3525 m^3/s | 1.3005 / 1.3529 m^3/s |
| angular-momentum flux at x = 90 times omega | 299.20 W | 297.66 W |
| the same at x = 20 | -36.22 W | -33.55 W |
| Euler head at x = 90, `e_m` | 221.22 Pa | 220.02 Pa |
| kinetic-energy flux per volume flow at x = 90, `q_m` | 97.13 Pa | 99.20 Pa |
| `e_m - q_m` | 124.09 Pa | 120.82 Pa |

Beside them: the measured shaft power 333.30 W, which is 237.53 Pa per design volume flow, and the measured pressure
rise 126.46 Pa. The bin means of the ensemble files equal the time-averaged files within 0.03 m/s (`ux`) and 0.09 m/s
(`uphi`) at x = 90, and within 0.33 and 0.06 m/s at x = 20.

Two things are read from this before any run. First, the measured profile at x = 20 carries a mean swirl against the
rotation (from +0.39 m/s at the hub to -2.05 m/s near the casing), worth -34 to -36 W. A fluid that comes from a
chamber at rest has no angular momentum, and nothing between the chamber and that plane can give it any except wall
friction, so this is not used as an inlet term of the work (design below); it is reported. Second, `e_m - q_m`, the
static pressure a loss-free rotor would give at the plane, is 121 to 124 Pa, which is the measured pressure rise within
6 Pa: on the rig, what the installation recovers behind the plane is as large as what the rotor loses.

## The design, from first principles

**Notation.** `c_x`, `c_t`, `c_r`: axial, circumferential and radial velocity in the frame at rest, `c_t` positive in
the direction of rotation. An overbar is the circumferential mean at fixed x and r. All "per volume flow" quantities
are in Pa.

**The circumferential average of the solve.** The solve is steady in the frame that turns with the rotor, with the
relative velocity `w` as its unknown; the absolute velocity is `c = w + omega e_z x r` (the case text sets the inlet
to `w = -omega e_z x r + u_in e_z`). A probe at rest at (x, r) sees, in time, the relative field pass by at uniform
angular speed, so its time average is the average over the angle at fixed x and r, each angle with equal weight. The
discretisation, fixed now:

* at each (x, r): 360 points, equally spaced over one pitch of 40 degrees, `theta_k = (k + 1/2) (2 pi / 9) / 360`;
* each point is located in the mesh: it is turned by each of the nine multiples of the pitch, and the cell is taken
  for which the largest signed distance of the point from the planes of the cell's faces (face centre, outward unit
  normal) is smallest. If that distance is above 0.1 mm the point is not in the fluid (it lies in the blade); it is
  left out of the mean, and the number of such points is reported for each radius;
* the value at the point is the value of that cell (the finite-volume cell mean, no interpolation): the pressure, and
  the cylindrical components of `w` formed at the cell centre, which a rotation of the sector leaves unchanged;
  `c_t = w_t + omega r` with r the radius of the point, so the part of the frame is exact;
* the circumferential mean is the arithmetic mean over the points in the fluid.

**The compared profiles.** At each of the 26 measured radii at x = 90 mm: the circumferential mean of `c_x` against
`ux_mean` of the time-averaged file, and of `c_t` against `uphi_mean`. The record's "meridional" velocity is its axial
component, and the hub and casing of the solve are cylinders, so the axial component is compared. The same is reported
at x = 20 mm, and `c_r` at both planes, without a clause of the acceptance on them.

**The Euler work integral.** For a control volume from the chamber, where the fluid is at rest, to the plane behind
the rotor, the moment of momentum gives the torque of the rotor on the fluid, less the friction torque of the walls at
rest, as the flux of angular momentum through the plane: `T = rho int c_x r c_t dA`. So, over the measured span,

`P_E = rho omega int_{r_1}^{r_26} mean(c_x c_t) r 2 pi r dr`, `Q_c = int mean(c_x) 2 pi r dr`, `e = P_E / Q_c`,

by the trapezoid rule on the 26 measured radii, for the measurement and the solve alike. `mean(c_x c_t)` has two
readings and both are reported: the product of the means ("profile": the time-averaged file, and the circumferential
means of the solve) and the mean of the products ("pointwise": the bin mean over the 180 bins of the ensemble file,
and the mean over the 360 points of the solve). The pointwise reading is the flux of the mean and blade-periodic flow
and is the one the division uses; the turbulent part `mean(c_x' c_t')` is in neither (the record gives rms values, no
correlation). The inlet term is zero by the argument above and is not subtracted; the same integral at x = 20 mm is
reported beside it for both, where it should be zero.

Likewise the kinetic energy that leaves the rotor, per volume flow, `q = rho int mean((c_x^2 + c_t^2 + c_r^2) c_x / 2)
2 pi r dr / Q_c`, in the same two readings, and for the solve the static pressure at the plane against the inlet total
pressure, `Pi_s = rho (int mean(p c_x) 2 pi r dr / Q_c - p_in - u_in^2 / 2)`, with `p_in` and `u_in` as ADR 0762 defines
its pressure rise (the solve carries the centrifugal term in the frame source, so `p` is the kinematic static
pressure).

For the solve the same quantities are also formed over the whole annulus (126 rings of 1 mm from 124 to 250 mm, midpoint
rule, points not in the fluid counted as zero flux), for the check V5 and for the coverage term below.

**The band at each radius.** `band(r) = U_num(r) + S(r) + T(r) + u_meas`, added, not combined in quadrature, as ADR
0762 registered its band:

* `U_num(r) = 1.25 abs(f_B2(r) - f_B1(r)) / (r_21 - 1)`, `r_21 = (38184 / 13627)^(1/3) = 1.40981`: the estimate of ADR
  0762 from the mesh family of ADR 0086 (the error of the fine value for `f(h) = f_0 + C h^p`, with the factor of safety
  1.25), with the order fixed at the cap `p = 1` of that registration. An observed order is not formed radius by
  radius: three samples of limit cycles at one point do not carry one. B3 is reported beside it;
* `S(r)`: the spread over the cycle, half the range of the B1 value over the iterations 7000, 7250, 7500, 7750 and
  8000. The compared value is that of iteration 8000, the recorded state;
* `T(r)`: the place of the plane, half the range of the B1 value over x = 88.7, 90 and 91.3 mm;
* `u_meas = 0`: no source that was read states an uncertainty of the LDA. Said so, not assumed.

The formula is fixed here; its numbers come from the runs and do not depend on the measured values. A radius is within
the band iff `abs(f_B1(r) - f_LDA(r)) <= band(r)`.

## The rule that divides the missing pressure rise (fixed now)

`D = 126.46 - 73.99 = 52.47 Pa` is missing (ADR 0762). Write, for the rig (index m) and the solve (index s) alike, the
pressure rise as what the rotor delivers as static pressure at the plane plus what happens behind it:

`dp_ts = Pi + Rec`, `Pi = e - q - L`,

with `e` the Euler work, `q` the kinetic energy that leaves through the plane, `L` the loss of total pressure between
the chamber and the plane (rotor and inlet), and `Rec` the change of static pressure from the plane to the place of
the measurement: on the rig the hub end, the diffuser and the exit to ambient, in the solve the rest of the annulus to
its outlet. This is the energy balance of the mean flow, per volume flow, and it defines `L` and `Rec`. Then

* **the rotor's share** is `R = Pi_m - Pi_s = (e_m - e_s) - (q_m - q_s) - (L_m - L_s)`,
* **the installation's share** is `I = Rec_m - Rec_s = D - R`.

For the solve every term is formed from the field. For the rig `e_m` and `q_m` are measured and **`L_m` is not**: an
LDA measures no pressure. First principles bound it: `L_m >= 0`, and since no kinetic energy can be recovered that is
not there, `Rec_m <= q_m`, which is `L_m <= e_m - 126.46 Pa`. So

`R_lo = 126.46 - q_m - Pi_s <= R <= R_hi = e_m - q_m - Pi_s`,

and with the measured numbers above (pointwise reading) `R_lo = 27.26 Pa - Pi_s` and `R_hi = 120.82 Pa - Pi_s`. Each
end carries a band `U = U_num(Pi_s) + S(Pi_s) + T(Pi_s) + u_cov + u_data`: the first three as for the profiles, formed
for the scalar; `u_cov = abs(Pi_s over the whole annulus - Pi_s over the measured span)` of B1, for the 9.1 per cent of
the annulus that is not measured; `u_data` the difference of the two readings of the measured term, 2.07 Pa for `R_lo`
(`q_m`) and 3.27 Pa for `R_hi` (`e_m - q_m`).

**The verdict**, with `D / 2 = 26.24 Pa`:

* if `R_hi + U < D / 2`: the larger part is **the installation's**; the shares are given as the intervals
  `R in [R_lo - U, R_hi + U]` and `I = D - R`;
* if `R_lo - U > D / 2`: the larger part is **the rotor's**; the shares are given as the intervals;
* otherwise **it cannot be told** from the velocity profiles, and the reason is the unknown loss of the measured
  rotor; the interval is stated all the same.

The interval is 93.56 Pa wide before any band, wider than `D`. So the division can be given only if the solve puts
`Pi_s` near an end: above about 95 Pa or below about 1 Pa. I expect 50 to 75 Pa (the pressure rise of the solve at its
outlet is 73.99 Pa), and so "cannot be told".

**The conditional division**, reported beside the verdict and never in its place: if the measured rotor loses what the
simulated one does (`L_m = L_s`, a condition that nothing in the record tests), then
`R_eq = (e_m - q_m) - (e_s - q_s) = 120.82 Pa - (e_s - q_s)` and `I_eq = D - R_eq`, with the band
`U_eq = U_num + S + T + u_cov` of `(e_s - q_s)` plus 3.27 Pa. This number is what the comparison of the profiles can
say on its own: how much of the missing pressure is in the work of the rotor and in the kinetic energy it leaves
behind.

Also reported, without a rule: the three terms of the solve (`L_s = e_s - q_s - Pi_s`, `Rec_s = 73.99 - Pi_s`); the
bound `I <= q_m - Rec_s`, the most an outlet model could add; and `Rec_m - L_m = 126.46 - (e_m - q_m)`.

The density enters `D`, `e`, `q` and `Pi` alike; one per cent of density is one per cent of each.

## The runs (fixed now)

The fields of the three solves of ADR 0762 were not kept (its test prints functionals and writes no field), so the
solves are made again, with the case text of `f870_fan01_design_point.rs` copied into the new test file without a
change of value: B3 (4 920 cells), B2 (13 627) and B1 (38 184), each once to 8000 iterations, the profiles taken at the
five iterations named above. Check R1 holds that these are the recorded states. Then the heavy record of B1 is run once
more with its recorded constants, as ADR 0762 did (its deviation 3), which is the fourth heavy run of the Size line. No
limit on wall-clock time is registered; the stops are those of ADR 0762 (the steady divergence guard, a non-finite
residual, continuity above 1e6).

## The test that holds it (task 5; fixed now)

One new file, `crates/rcfd-api/tests/f878_fan01_lda_profiles.rs` (header: ADR 0765, F-878). The file
`f870_fan01_design_point.rs` is not changed.

* Run by the gate:
  * `f878_lda_reference_and_measured_integrals`: the constants of the measured profiles equal the transcription CSV;
    the integrals of the table above are reproduced from them.
  * `f878_sampler_closed_forms`: on the B3 mesh, without a solve: a field whose cylindrical components are constant is
    returned by the circumferential mean to rounding at every measured radius of both planes; no point is outside the
    fluid at x = 20 mm; the trapezoid rule returns the closed form of a free vortex.
  * `f878_recorded_verdicts_follow_from_the_recorded_profiles`: from the recorded B1 profiles, the recorded bands and
    the measured constants, the per-radius verdicts and the division are formed again and equal the recorded ones.
  * `f878_b3_short_run_holds_the_profiles`: B3 for 300 iterations on the path of the heavy record; `e_s`, `q_s`, `Pi_s`
    and `Q_c` at x = 90 mm within the tolerance below of the values of its first run.
* Heavy records (`#[ignore]`):
  * `f878_b1_profiles_state_record`: B1 to 8000 iterations; torque and pressure rise within 1 per cent of ADR 0762's
    record; `e_s`, `q_s`, `Pi_s`, `Q_c` and each of the 52 profile values within the tolerance below of the recorded
    ones; the recorded verdicts unchanged.
  * `f878_b1_profiles_acceptance`: every radius within its band for both components, and the division given. If it
    misses it is renamed `..._known_miss` with the numbers in its ignore reason (rule 2).
  * `f878_family_run`: one of B1, B2, B3 by an environment variable; writes the profiles at the five iterations.

**The tolerance of the state**: 1 per cent of the recorded value, and for a profile value 1 per cent or 0.01 m/s,
whichever is larger (0.01 m/s is the precision to which the record stores a velocity). Source: the tolerance of the
state of ADR 0762, twice the 0.5 per cent to which the cap rule of ADR 0075 and ADR 0086 defines a functional of this
case; the solver is deterministic, so an unchanged main gives zero.

## Table of checks

Every clause of the acceptance first. "Estimate" is what I expect; where I have no basis for a number I say so.

| Id | Check | Limit | Source of the limit | Estimate | Margin | Kind |
| --- | --- | --- | --- | --- | --- | --- |
| C1 | `c_x` of B1 at x = 90 mm against `ux_mean`, at each of the 26 radii | `abs(difference) <= band(r)` at every radius | acceptance of F-878 (a band registered before the comparison); the band's form from ADR 0762 and ADR 0086 as derived above | misses expected at the outer radii, where the measured profile falls from 12.0 to 3.6 m/s; no number | below 1 at some radii: expected MISS | validation |
| C2 | `c_t` of B1 at x = 90 mm against `uphi_mean`, at each of the 26 radii | the same | the same | the same expectation | below 1 at some radii: expected MISS | validation |
| C3 | the Euler work of the measured and of the simulated profile, both readings, beside the measured shaft power | none (report) | acceptance of F-878 ("is reported") | measured 299.20 and 297.66 W over the measured span; simulated about 300 W (the torque of B1 is 326.50 W over the whole annulus) | n/a | validation |
| C4 | the division: the verdict by the rule above, with `R_lo`, `R_hi`, `U`, and the conditional `R_eq`, `I_eq`, `U_eq` | the rule above | acceptance of F-878 ("states what part ... is the rotor's and what part the installation's"); the brief (a rule with the condition under which it cannot be divided) | "cannot be told" (`Pi_s` 50 to 75 Pa) | n/a | validation |
| C5 | a miss is recorded as a miss: the acceptance test fails on every missed clause and is kept as a known miss with its numbers; the state record asserts the verdicts | as stated | acceptance of F-878 (last sentence); rule 2 | as stated | n/a | validation |
| R1 | the three solves are the recorded states: torque and pressure rise of B1, B2, B3 at 8000 iterations against ADR 0762 (2.098164 N m and 73.9881 Pa; 2.028692 and 64.2329; 1.918189 and 46.7776) | 1 % each | ADR 0762, the tolerance of the state | 0 | n/a | unchanged default output |
| R2 | what the limit cycle and the meshes do: `S(r)` and `U_num(r)` at each radius, and `S`, `U_num`, `T`, `u_cov` of `Pi_s` and `e_s - q_s`; whether the verdict of C4 is the same with the band and without it, and on B2 | none (report) | the brief, task 4 | `S` small (the functionals move by 3e-4 % over 1000 iterations); `U_num` the largest term | n/a | solution verification |
| R3 | the same comparison at x = 20 mm, and `c_r` at both planes | none (report) | - | no number | n/a | validation |
| V1 | the measured constants equal the CSV; the integrals of the table above from the constants | each value equal as printed (5e-3 m/s); 299.20 and 297.66 W within 5e-3 W; 221.22, 220.02, 97.13, 99.20 Pa within 5e-3 Pa | the record as transcribed; this registration, to its printed digits | exact | n/a | code verification |
| V2 | the circumferential mean of a field with constant cylindrical components (1.7, -2.3, 0.4) m/s in the turning frame, B3 mesh, 26 radii, both planes | relative error `<= 1e-9` on `c_x`, `c_r` and `c_t - omega r` | `docs/process/tolerances.md`, "Agreement with a closed form" (exact for the discrete problem) | 1e-15 | 1e6 | code verification |
| V3 | points outside the fluid at x = 20 mm (ahead of the blade), B3, B2 and B1, at every measured radius | 0 | closed form: the plane does not cut the blade (Table 3 of the description: leading edge at 21.6 mm and more) | 0 | n/a | code verification |
| V4 | trapezoid rule on the 26 radii for `c_x = a`, `c_t = G / r`: `P_E = rho omega a G pi (r_26^2 - r_1^2)` | relative error `<= 1e-9` | the same row of the tolerance table (the integrand is linear in r, so the rule is exact) | 1e-15 | 1e6 | code verification |
| V5 | the whole-annulus integrals of the solve against its own functionals, B1: volume flow at x = 90 mm against 1.4032 m^3/s; `rho omega int c_x r c_t dA` against the torque times omega (326.50 W), both readings | none (report): no row of the tolerance table covers a balance of a flow case, and I set none of my own | - | flow within 2 %; power within 10 % (ADR 0170: the balance of B1 is open by 2.8 %; the plane cuts the blade root below r = 131 mm) | n/a | solution verification |
| S1 | `f878_b3_short_run_holds_the_profiles` | 1 % each | ADR 0762, the tolerance of the state | 0 | n/a | unchanged default output |
| S2 | `f878_recorded_verdicts_follow_from_the_recorded_profiles` | verdicts equal | this registration | equal | n/a | code verification |
| D1 | defaults: no file under `crates/*/src` is changed; `f870_fan01_design_point.rs` is not changed | 0 files | rule 3; the brief | 0 | n/a | unchanged default output |
| P | `cargo fmt -p rcfd-api --check`; `cargo clippy -p rcfd-api --all-targets -- -D warnings`; `cargo test -p rcfd-cli --release --test docs_site`; `python scripts/roadmap_proposals.py check`; `python scripts/process_report.py --check` | clean; passes; no failure but a generated file not current | rule 7 | as stated | n/a | code verification |

C1 and C2 are the rows of kind validation that carry a limit. V5 has no limit because I found no source for one; it is
reported so that a reader can judge the sampling, and a result that reads as a defect of the sampling stops the
comparison until an amendment corrects it.

## Negative controls and planned mutations

On a scratch copy of the tree (`git archive` of the branch, its own target folder), each against the gate tests; no
heavy record is rerun for a mutation.

| Id | Mutation | Must be caught by |
| --- | --- | --- |
| M1 | solver: the Coriolis factor of `RotatingFrame::accel` (`crates/rcfd-fv/src/sources.rs`) from -2.0 to -1.9 | `f878_b3_short_run_holds_the_profiles` |
| M2 | test: the absolute circumferential velocity without the part of the frame (`c_t = w_t`) | `f878_sampler_closed_forms` |
| M3 | test: one measured constant, `ux_mean` at r / 250 mm = 0.832, from 12.02 to 12.20 | `f878_lda_reference_and_measured_integrals` |
| M4 | test: the ring weight of the trapezoid rule, `2 pi r` to `2 pi` | `f878_sampler_closed_forms` |
| M5 | test: the factor of safety of the band from 1.25 to 10 (a widened band) | `f878_recorded_verdicts_follow_from_the_recorded_profiles` (a recorded miss no longer holds), if the result has a miss; if every radius is within the band at 1.25, the mutation is 1.25 to 0.01 instead |
| M6 | test: the sign of the rotation of a point into the sector | `f878_sampler_closed_forms` (points are not found, or V2 fails) |
| K1 | negative control: the B3 short run made twice | equal to the last printed digit |

M1 is committed as `crates/rcfd-api/tests/f878_fan01_lda_profiles.mutation.patch` with the first line
`# expect-fail: cargo test -p rcfd-api --release --test f878_fan01_lda_profiles f878_b3_short_run_holds_the_profiles`.

## Decision rule

* **Done** only if every one of the 26 radii is within its band for `c_x` and for `c_t` (C1, C2) and the division is
  given with numbers by the rule above (C4: the first or the second case of the verdict).
* **Partial** if the comparison is made at every measured radius and the division cannot be given by the rule, or a
  radius misses its band; the numbers, the interval and the conditional division are reported.
* **Stopped and reported** if the measured profiles cannot be traced to a source that was read. That is already
  excluded: they are traced, with checksums, above.

The datum is settled by the description, so the brief's case "the datum stays open and the two readings disagree" does
not arise; the +-1.3 mm of the mesh frame is in the band.

## What is not covered

* The uncertainty of the LDA (not stated in anything read) and the density of the measurement (F-877).
* The loss of the measured rotor: no pressure was measured behind it. This is what keeps the strict division open.
* The turbulent part of the fluxes, `mean(c_x' c_t')`: not in the record.
* The 9.1 per cent of the annulus outside the measured span, except through `u_cov`.
* The phase-resolved comparison (the blade-to-blade field against the 180 bins): the phase between bin and rotor is
  not in the record; only bin means are used.
* The off-design points: the record has LDA at the design point only.
* A converged solve and a mesh within its gates: the limits of ADR 0762 stand; R2 says what they do to this result.
* The inference of ADR 0762 that 63 per cent of the missing pressure is outlet dynamic pressure is not tested as such;
  `q_s`, `Rec_s` and the bound `q_m - Rec_s` are the numbers this work puts beside it.
* No outlet model, no solver change, no new mesh. Check H (F-879) is another agent's.
* The `.h5` copies of the LDA files were not read; the description says they hold the same values as the `.mat` files.

## Amendment 1 (2026-11-22, before any registered run and before the commit of the test file): the value at a point is the cell's absolute velocity

Written after the first runs of the gate tests of the new file, which is not committed yet. No heavy run has been made
and no profile of B1 or B2 has been seen. What those runs gave: V1 passed. S1 and S2 failed as they must while no value
is recorded. V3 failed on its first run (118 and 105 points "not in the fluid" at the two outermost radii at x = 20 mm
on B3): an error of my code, not of the method. The list of cells that are tried for a ring left out cells whose flat
faces reach nearer to the axis than their corner points do; the list now takes the bound of the cell's convex hull. The
registered method (the cell with the smallest largest distance from its face planes) is unchanged, and V3 then gave 0
at every radius.

The defect that needs this amendment is in the registered design. It says: the value at a point is the cylindrical
components of the relative velocity `w` at the cell centre, and "`c_t = w_t + omega r` with r the radius of the point,
so the part of the frame is exact". That is wrong for a cell that is not thin in the radial direction. The relative
circumferential velocity is close to `-omega r` plus a slowly varying part, so it varies across a cell by `omega` times
the cell's radial extent; holding it constant in the cell and adding `omega r` of the point puts a sawtooth of that size
on the profile. The B3 short run (300 iterations, printed, not recorded) showed it: `c_t` at x = 90 mm rose by 0.78 m/s
per 5 mm within a layer of cells, which is `omega` times 5 mm, and fell by 4 to 6 m/s at each layer boundary (for
instance 10.64 m/s at r = 153 mm and 4.86 m/s at r = 158 mm). The quantity that is smooth across a cell is the absolute
velocity.

Corrected, fixed now: **the value at a point is the absolute velocity of its cell**, `c = w + omega e_z x x_c` formed at
the cell centre, in cylindrical components at the cell centre: `c_t = w_t + omega r_c`. This is the cell mean of the
absolute velocity, since the frame part is linear in position. Everything else of the design stands: the points, the
location, the means, the integrals, the bands and the rule of division.

V2 changes with it: the field of the check has constant cylindrical components (1.7, -2.3, 0.4) m/s of the **absolute**
velocity (`w = c - omega e_z x x_c` at each cell centre), and the circumferential mean must return `c_x`, `c_t` and
`c_r` with a relative error of at most 1e-9, at every measured radius of both planes on B3. Mutation M2 becomes: the
part of the frame left out (`c_t = w_t`), caught by `f878_sampler_closed_forms`, as registered.

What I had seen when I wrote this: the B3 values at iteration 300 with the defective sampling (`e_s` 243.0 Pa, `q_s`
98.1 Pa, `Pi_s` 39.4 Pa, whole-annulus Euler power 343 to 347 W against a torque power of 298.5 W). No rule, band or
limit is changed by this amendment, and none of those numbers is recorded.

M6 of the table of mutations cannot be applied as written: a point is turned by all nine multiples of the pitch, so
there is no sign of a rotation to change. It is replaced by: the sign of the circumferential unit vector in the
decomposition of a cell's velocity, caught by `f878_sampler_closed_forms`.
