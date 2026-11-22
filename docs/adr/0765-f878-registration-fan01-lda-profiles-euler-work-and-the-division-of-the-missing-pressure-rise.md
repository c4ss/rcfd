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

## Addendum 1: results

Written 2026-11-22, after the runs. Commits on the branch, from main 2bab1863: 2d5f94d9 (this registration, alone),
2a008ec9 (amendment 1, before any registered run and before the commit of the test file), 6f7bb332 (the test file, the
transcription of the record and its scripts), 8a2ea1f7 (the mutation patch, the reference-data fragment, the paragraph
of `docs/validation-data.md`), 3f458137 (the recorded state, the logs of the three runs and the analysis), and the
commit of this addendum with the proposal file, the acceptance test kept as a known miss and the log of the fourth run.
The three registered solves were made from 6f7bb332; their logs are `B1.log`, `B2.log` and `B3.log` in
`docs/investigations/f878-fan01-lda-profiles/`, with `analyse.py`, which forms every number below from them, and its
output `analysis.txt`. The test file forms the verdicts a second time, in Rust, from the recorded constants; the two
agree.

### The sentence this work was to deliver

**Of the missing 52.47 Pa it cannot be told from the measured velocity profiles what part is the rotor's and what part
the installation's.** By the registered rule the rotor's share lies between -34.4 Pa and +59.1 Pa (bands 23.8 and 25.0
Pa), and the installation's between -6.6 Pa and +86.9 Pa; neither end is on one side of half the missing pressure. The
reason is the one the registration named: an LDA measures no pressure, so the loss of the measured rotor is not known.

What the profiles do show, with numbers: the simulated rotor does the work of the measured one and leaves the same
kinetic energy behind. Over the measured span its Euler head is 226.86 Pa against 220.02 Pa measured, and the kinetic
energy that leaves it 102.28 Pa against 99.20 Pa. So **if the measured rotor loses what the simulated one does, the
rotor's share is -3.8 Pa and the installation's 56.2 Pa, +-31.1 Pa**: the missing pressure is not in the work of the
rotor or in its outflow velocity. It is in what the registration called `Rec - L`, recovery behind the plane less loss
before it: +5.6 Pa on the rig, -50.6 Pa in the solve (12.3 Pa recovered, 62.9 Pa lost). Whether the rig recovers more
or its rotor loses less, the velocities cannot say.

### The comparison at each radius (C1, C2)

FAN-01 design point, B1 (38 184 cells) at 8000 iterations, x = 90 mm; velocities in m/s. The band is `U_num + S + T` as
registered; `u_meas` is zero because no source states one.

| r / 250 mm | `c_x` LDA | B1 | B2 | B3 | band | B1 - LDA | | `c_t` LDA | B1 | B2 | B3 | band | B1 - LDA | |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 0.532 | 10.11 | 11.269 | 10.981 | 10.754 | 0.890 | +1.159 | **MISS** | 9.77 | 8.156 | 8.226 | 9.028 | 0.251 | -1.614 | **MISS** |
| 0.552 | 10.66 | 11.345 | 11.065 | 10.880 | 0.924 | +0.685 | within | 8.91 | 7.810 | 8.023 | 8.824 | 0.737 | -1.100 | **MISS** |
| 0.572 | 10.88 | 11.446 | 11.212 | 10.842 | 0.718 | +0.566 | within | 8.56 | 7.669 | 7.825 | 8.743 | 0.478 | -0.891 | **MISS** |
| 0.592 | 10.91 | 11.407 | 10.986 | 10.801 | 1.329 | +0.497 | within | 8.25 | 7.657 | 7.445 | 8.764 | 0.672 | -0.593 | within |
| 0.612 | 10.98 | 11.232 | 11.038 | 10.891 | 0.604 | +0.252 | within | 7.94 | 7.433 | 7.387 | 8.826 | 0.152 | -0.507 | **MISS** |
| 0.632 | 11.13 | 11.246 | 11.083 | 10.672 | 0.509 | +0.116 | within | 7.60 | 7.409 | 7.378 | 6.898 | 0.119 | -0.191 | **MISS** |
| 0.652 | 11.23 | 10.885 | 10.534 | 10.692 | 1.075 | -0.345 | within | 7.31 | 7.269 | 7.236 | 6.906 | 0.122 | -0.041 | within |
| 0.672 | 11.25 | 10.924 | 10.549 | 10.716 | 1.151 | -0.326 | within | 7.13 | 7.224 | 7.243 | 6.914 | 0.076 | +0.094 | **MISS** |
| 0.692 | 11.36 | 10.558 | 10.545 | 10.732 | 0.063 | -0.802 | **MISS** | 6.76 | 7.124 | 7.236 | 6.944 | 0.362 | +0.364 | **MISS** |
| 0.712 | 11.47 | 10.588 | 10.481 | 10.739 | 0.340 | -0.882 | **MISS** | 6.48 | 7.074 | 6.598 | 6.949 | 1.467 | +0.594 | within |
| 0.732 | 11.56 | 10.535 | 10.481 | 10.742 | 0.170 | -1.025 | **MISS** | 6.21 | 6.661 | 6.590 | 6.949 | 0.238 | +0.451 | **MISS** |
| 0.752 | 11.62 | 10.551 | 10.487 | 10.670 | 0.198 | -1.069 | **MISS** | 5.94 | 6.643 | 6.580 | 6.080 | 0.200 | +0.703 | **MISS** |
| 0.772 | 11.72 | 10.744 | 10.488 | 10.673 | 0.791 | -0.976 | **MISS** | 5.64 | 6.048 | 6.569 | 6.074 | 1.594 | +0.408 | within |
| 0.792 | 11.83 | 10.755 | 11.013 | 10.679 | 0.798 | -1.075 | **MISS** | 5.31 | 6.028 | 5.603 | 6.067 | 1.307 | +0.718 | within |
| 0.812 | 11.96 | 11.179 | 11.013 | 10.696 | 0.523 | -0.781 | **MISS** | 4.99 | 5.432 | 5.586 | 6.058 | 0.473 | +0.442 | within |
| 0.832 | 12.02 | 11.159 | 11.009 | 10.691 | 0.470 | -0.861 | **MISS** | 4.70 | 5.429 | 5.598 | 6.049 | 0.524 | +0.729 | **MISS** |
| 0.852 | 11.94 | 11.607 | 11.257 | 10.703 | 1.102 | -0.333 | within | 4.55 | 5.109 | 5.202 | 6.044 | 0.299 | +0.559 | **MISS** |
| 0.872 | 11.54 | 11.583 | 11.248 | 8.440 | 1.044 | +0.043 | within | 4.52 | 5.110 | 5.223 | 6.490 | 0.351 | +0.590 | **MISS** |
| 0.892 | 10.37 | 11.579 | 11.257 | 8.484 | 0.998 | +1.209 | **MISS** | 5.02 | 5.099 | 5.236 | 6.503 | 0.417 | +0.079 | within |
| 0.912 | 8.42 | 11.203 | 8.542 | 8.519 | 8.157 | +2.783 | within | 5.93 | 6.158 | 7.665 | 6.502 | 4.621 | +0.228 | within |
| 0.932 | 5.97 | 11.193 | 8.523 | 8.569 | 8.176 | +5.223 | within | 6.96 | 6.205 | 7.676 | 6.500 | 4.504 | -0.755 | within |
| 0.952 | 4.43 | 6.818 | 8.501 | 8.626 | 5.187 | +2.388 | within | 7.99 | 9.802 | 7.689 | 6.489 | 6.475 | +1.812 | within |
| 0.960 | 4.06 | 6.813 | 8.490 | 8.662 | 5.167 | +2.753 | within | 8.27 | 9.810 | 7.702 | 6.493 | 6.456 | +1.540 | within |
| 0.968 | 3.88 | 1.659 | 0.874 | 8.688 | 2.405 | -2.221 | within | 8.64 | 10.547 | 9.191 | 6.484 | 4.191 | +1.907 | within |
| 0.976 | 3.87 | -0.254 | 0.794 | 8.710 | 3.232 | -4.124 | **MISS** | 8.78 | 10.133 | 9.177 | 6.477 | 2.982 | +1.353 | within |
| 0.984 | 3.57 | -0.975 | -0.659 | 3.199 | 1.003 | -4.545 | **MISS** | 8.43 | 9.797 | 8.563 | 6.913 | 3.821 | +1.367 | within |

`c_x`: **14 of 26 radii within the band, 12 miss**; largest distance 5.22 m/s (r / 250 mm = 0.932, inside its band of
8.18 m/s), largest excess over the band 3.54 m/s (0.984). `c_t`: **14 of 26 within, 12 miss**; largest distance 1.91 m/s
(0.968, inside its band), largest excess over the band 1.36 m/s (0.532). No point of the 360 on any measured ring at x =
90 mm lies outside the fluid on B1 and B2.

How to read the band, which stands as registered. The value at a point is the mean of its cell, and the cells of B1
are thicker in the radial direction than the 5 mm between measured radii, so the simulated profile is a staircase:
neighbouring radii often carry the same value. Where a step of B1 and a step of B2 fall between the same two radii,
`U_num` is a few hundredths of a m/s and a difference of 0.2 m/s is a miss; where they do not, `U_num` is 4 to 8 m/s and
"within" says little (r / 250 mm from 0.912 to 0.968). The count of 14 is therefore a weak number in both directions.
What does not depend on it: between r / 250 mm = 0.69 and 0.83 the solve is 0.8 to 1.1 m/s low in `c_x`; at the hub
(0.532) it is 1.2 m/s high in `c_x` and 1.6 m/s low in `c_t`; and outside 0.97 it has no through-flow (`c_x` -0.25 and
-0.98 m/s at 0.976 and 0.984) where 3.9 and 3.6 m/s were measured, with 1.4 m/s more swirl. The measured fall of `c_x`
toward the casing starts at 0.87; in the solve it starts at 0.95 and is steeper.

### The Euler work (C3)

Over the measured span (133 to 246 mm), rho = 1.2 kg/m^3 assumed; "profile" is the product of the means, "pointwise" the
mean of the products (for the measurement: over the 180 bins).

| | volume flow through the span [m^3/s] | Euler work, profile / pointwise [W] | Euler head `e` [Pa] | kinetic energy `q` [Pa] | static pressure at the plane `Pi_s` [Pa] | loss `L_s` [Pa] | `Rec_s` [Pa] |
| --- | --- | --- | --- | --- | --- | --- | --- |
| measured | 1.3525 / 1.3529 | **299.20 / 297.66** | 221.22 / 220.02 | 97.13 / 99.20 | not measured | not measured | not measured |
| B1 | 1.3754 | **318.72 / 312.03** | 231.73 / 226.86 | 98.99 / 102.28 | 61.71 | 62.87 | 12.28 |
| B2 | 1.3314 | 312.96 / 308.25 | 235.06 / 231.53 | 94.50 / 98.04 | 54.73 | 78.75 | 9.50 |
| B3 | 1.3395 | 324.30 / 321.81 | 242.10 / 240.24 | 91.42 / 96.55 | 39.35 | 104.34 | 7.43 |

Beside them: the measured shaft power, **333.30 W** (237.53 Pa per design volume flow), and the torque of B1 times
omega, 326.50 W. The measured Euler work over 91 per cent of the annulus is 89 to 90 per cent of the measured shaft
power; the simulated one over the same span is 96 to 98 per cent of the simulated shaft power.

The same integral at x = 20 mm, which should be zero: measured -36.22 / -33.55 W, B1 +3.38 W. The measured profile
there carries a swirl against the rotation that grows to -2.05 m/s at the casing; the solve has none (0.0 to 0.15 m/s).
Not registered, my arithmetic after the run from the same files: the kinetic energy at x = 90 mm splits into 71.3 Pa
axial and 25.6 Pa swirl in the measurement and 72.1 and 26.7 Pa on B1 (profile reading).

### The division (C4)

`D` = 52.47 Pa, `D / 2` = 26.24 Pa. From B1: `Pi_s` = 61.71 Pa, `e_s - q_s` = 124.58 Pa.

| | value [Pa] | band [Pa] | of which `U_num`, `S`, `T`, `u_cov`, `u_data` [Pa] |
| --- | --- | --- | --- |
| `R_lo` (the measured rotor loses all it can) | -34.45 | 23.75 | 21.28, 0.0008, 0.008, 0.39, 2.07 |
| `R_hi` (the measured rotor loses nothing) | 59.11 | 24.95 | 21.28, 0.0008, 0.008, 0.39, 3.27 |
| `R_eq` (conditional: `L_m = L_s`) | -3.76 | 31.10 | 27.17, 0.0005, 0.33, 0.33, 3.27 |

`R_hi + U` = 84.06 Pa is not below 26.24 Pa and `R_lo - U` = -58.20 Pa is not above it: **the verdict is "it cannot be
told"**, as expected in the registration. The installation's share `I = D - R` is between -6.64 and 86.92 Pa without
the band (-31.59 and 110.67 Pa with it); its upper end is the registered bound `q_m - Rec_s`, the most an outlet model
could add. Conditional: `I_eq` = 56.23 Pa.

The terms the solve gives and the rig does not: the simulated rotor loses 62.87 Pa of total pressure before the plane,
27.7 per cent of its Euler head, and the annulus behind it recovers 12.28 Pa. On the rig `Rec_m - L_m` = 126.46 -
(220.02 - 99.20) = +5.64 Pa. If the measured rotor lost the 62.87 Pa of the simulated one, the rig would have to recover
68.5 Pa behind the plane, 69 per cent of all the kinetic energy that leaves the rotor, swirl included; if the rig
recovered only the 12.28 Pa of the solve, its rotor would lose 6.6 Pa. The truth is between, and no number here places
it.

### What the limit cycle and the meshes do to it (R2, task 4)

* **The limit cycle does nothing that can be seen at the plane.** Over the iterations 7000 to 8000 the profile values
  move by at most 0.0006 m/s (`S`), `Pi_s` by 0.0008 Pa and `e_s - q_s` by 0.0005 Pa. The cycle that keeps `res_u` at
  1.2e-3 is not in these quantities.
* **The place of the plane is a small term**: +-1.3 mm moves a profile value by at most 0.085 m/s, `Pi_s` by 0.008 Pa
  and `e_s - q_s` by 0.33 Pa.
* **The mesh is the whole band, and the solve is not mesh-converged in the terms that matter.** `Pi_s` is 39.35, 54.73
  and 61.71 Pa on B3, B2 and B1; the loss 104.34, 78.75 and 62.87 Pa; the Euler head 240.24, 231.53 and 226.86 Pa,
  falling toward the measured 220.02 Pa. Each refinement removes loss and excess work. The conditional share `R_eq` is
  -22.87, -12.67 and -3.76 Pa on the three meshes.
* **The verdict survives.** It is "cannot be told" with the band and without it, and with B2 in the place of B1
  (`R_lo` -27.47, `R_hi` 66.09 Pa). The conditional division does not survive as a number: +-31 Pa on 52 Pa, 27 Pa of it
  the mesh term.
* **The missed mesh gates.** The faces that miss them are at the lower seam corner of the tip layers (ADR 0762). The
  outermost radii are where the profiles are furthest from the measurement, but the tip gap is also where the model is
  coarsest (wall functions, the casing boundary layer of the rig not in the inlet), and nothing here separates the two.

### Every check

| Id | Measured | Met |
| --- | --- | --- |
| C1 | `c_x`: 14 of 26 radii within the band; 12 miss, largest excess 3.54 m/s | **no, a miss** |
| C2 | `c_t`: 14 of 26 radii within the band; 12 miss, largest excess 1.36 m/s | **no, a miss** |
| C3 | Euler work 299.20 / 297.66 W measured and 318.72 / 312.03 W simulated over the measured span, beside 333.30 W of measured shaft power | reported |
| C4 | verdict "cannot be told": `R` between -34.45 and 59.11 Pa, bands 23.75 and 24.95 Pa; conditional `R_eq` -3.76 Pa, `I_eq` 56.23 Pa, band 31.10 Pa | the rule returns "cannot be told": **no division** |
| C5 | the acceptance test ran once (the fourth run) and failed on its three clauses with the radii and the division printed; it is kept as `f878_b1_profiles_acceptance_known_miss`, ignored, with the numbers in its reason and its limits unchanged. The state record asserts the per-radius verdicts and the verdict of the division, and passed in the same run | yes |
| R1 | B1 2.098164 N m and 73.9881 Pa, B2 2.028692 and 64.2329, B3 1.918189 and 46.7776: equal to ADR 0762 to the printed digits | yes |
| R2 | above | reported |
| R3 | x = 20 mm: `c_x` within the band at 6 of 26 radii (the solve is flat at 10 m/s, the measurement falls from 11.95 at the hub to 6.53 m/s at the casing), `c_t` at 8 of 26 (the measured swirl against the rotation is not in the solve). `c_r` at x = 90 mm: measured +0.2 to +1.6 m/s outward of 0.57, solve -0.5 to -0.7 m/s up to 0.93 | reported |
| V1 | constants equal to the CSV; 299.196, 297.657 W; 221.218, 220.015, 97.129, 99.196 Pa; flows 1.35249, 1.35289, 1.32210, 1.30055 m^3/s | yes |
| V2 | largest relative error 9.1e-15 (as amendment 1 has the check) | yes |
| V3 | 0 points outside the fluid at x = 20 mm at every measured radius on B3, B2 and B1 (after the correction of the candidate list, amendment 1) | yes |
| V4 | 326.6175764075039 W against the same closed form | yes |
| V5 | B1, whole annulus at x = 90 mm: volume flow 1.4144 m^3/s, 0.80 % above 1.4032; Euler work 331.62 W (profile) and 323.85 W (pointwise) against 326.50 W of torque times omega, +1.6 % and -0.8 %. B2: +0.34 %, +4.7 % and +2.9 %. B3: +1.18 %, +15.3 % and +14.1 %. At x = 20 mm the flow is 2.4 % high on B1. The least share of fluid points on a ring is 0.989 (the blade root) | reported; inside my estimates on B1 |
| S1 | B3 at iteration 300: `e_s` 240.0486 Pa, `q_s` 96.4801 Pa, `Pi_s` 39.3569 Pa, `Q_c` 1.339620 m^3/s, equal in every run on the unmutated tree | yes |
| S2 | the verdicts formed in Rust from the recorded constants equal the recorded ones, and the six numbers of the division equal those of `analyse.py` to 0.005 Pa | yes |
| D1 | `git diff --name-only 2bab1863` names no file under `crates/*/src` and not `f870_fan01_design_point.rs` | yes |
| P | see "The checks of rule 7" below | see below |

### What is a gate test and what is a heavy record

All in `crates/rcfd-api/tests/f878_fan01_lda_profiles.rs`.

* Run with every `cargo test` (about 20 s together in release): `f878_lda_reference_and_measured_integrals` (the
  constants against the transcription, the measured integrals), `f878_sampler_closed_forms` (the circumferential mean
  and the trapezoid rule against closed forms, on the B3 mesh), `f878_recorded_verdicts_follow_from_the_recorded_profiles`
  (bands, per-radius verdicts and the division from the recorded profiles), `f878_b3_short_run_holds_the_profiles` (the
  case text and the code path of the heavy record on B3 for 300 iterations, four scalars within 1 %, either way).
* Ignored, heavy, run by name: `f878_b1_profiles_state_record` (about 1 h; fails if the solve is not the state of ADR
  0762, if a scalar or one of the 52 profile values moves by more than the tolerance either way, or if a verdict
  changes, so an improvement fails it too); `f878_b1_profiles_acceptance_known_miss` (the acceptance as a test; it fails
  today and its ignore reason carries the numbers); `f878_family_run` (B1, B2, B3 with all profiles printed).

### Mutations and negative controls

On a copy of the tree in the scratch folder with its own target folder. All six were caught by the test the
registration (M6: amendment 1) names.

| Id | What happened |
| --- | --- |
| M1 | Coriolis factor -2.0 to -1.9: B3 at iteration 300 gives `e_s` 235.45 Pa against 240.05 (-1.9 %) and `q_s` 94.49 against 96.48 (-2.1 %); `Pi_s` (-0.9 %) and `Q_c` (-0.08 %) stay inside. Caught. Committed as the mutation patch |
| M2 | `c_t = w_t`: the closed form fails with a relative error of 16.6. Caught |
| M3 | 12.02 to 12.20: the constant no longer equals the CSV. Caught |
| M4 | ring weight `2 pi`: the free-vortex closed form fails. Caught |
| M5 | factor of safety 10: every radius but one of `c_x` comes within the band, the recorded misses no longer hold. Caught |
| M6 | sign of the circumferential unit vector: the closed form fails with a relative error of 35.2. Caught |
| K1 | the B3 short run gave the same four values to the printed digits in each of its runs |

M1 shows the same weakness ADR 0762 found: at 1 % a 5 % error of the Coriolis term is caught by two of the four
scalars, by a factor of two, not by all.

### Deviations

1. Amendment 1: the registered value at a point (the relative circumferential velocity of the cell plus `omega r` of
   the point) put a sawtooth on the profile. Corrected to the cell's absolute velocity, committed before any registered
   run. I had seen B3 numbers at 300 iterations with the defective sampling; they are named in the amendment.
2. An error of my code in the list of candidate cells of the locator, found by V3 on its first run and corrected before
   the test file was committed; the registered method did not change.
3. M6 could not be applied as registered and was replaced (amendment 1).
4. The fourth heavy run ran the state record and the acceptance test in one process, on one solve. The acceptance test
   had its one run there.
5. To make the mutation patch with `git diff`, `crates/rcfd-fv/src/sources.rs` was changed in the worktree and restored
   in the same command; no solver source is changed in any commit.
6. The logs of the three runs are committed with every sampled ring (about 100 kB each), because the fields are not
   kept and the logs are the record the numbers are formed from.
7. The mutations were applied and run by hand on the scratch copy; the repository's mutation runner was not used.
8. The figures of the description were not read, only its text, tables and captions.

The fourth run (the test binary built from 3f458137, B1, 8000 iterations, 59.5 min; log `B1_confirm.log` in the investigation folder):
`f878_b1_profiles_state_record` passed, so the solve repeats the recorded state, the four scalars, the 52 profile
values and the verdicts within the tolerances; `f878_b1_profiles_acceptance` failed, as it must: `c_x` outside its band
at r / 250 mm = 0.532, 0.692 to 0.832 (eight radii), 0.892, 0.976 and 0.984; `c_t` at 0.532, 0.552, 0.572, 0.612, 0.632,
0.672, 0.692, 0.732, 0.752, 0.832, 0.852 and 0.872; the division "cannot be told". After that run the test was renamed
to the known miss and given its ignore reason, and the result was put in the sentence of the file header; nothing else
of the test file changed, and the renamed test was not run again.

### What was not run

* Nothing with a measurement uncertainty: none is stated for the LDA.
* No finer mesh than B1, no outlet model, no transient run, no solve on another set-up.
* The phase-resolved comparison and the turbulent part of the fluxes: not in the record in a usable form.
* The wall-pressure data of the record (`mic_kulite.tar`, 0.46 GB: 14 transducers in the casing from x = -23 to 107 mm)
  were not fetched. The description presents them as fluctuations; whether their mean is a calibrated static pressure
  it does not say. A static pressure behind the rotor is the one measurement that would close the division.
* The full workspace test run and the heavy tests of other files. Of the workspace only the new file's tests,
  `docs_site`, fmt and clippy of `rcfd-api` were run.
* Kromer's thesis and the SAE paper were not read (F-877).

### The checks of rule 7

On the final tree, with the target folder in the scratch folder:

* `cargo test -p rcfd-api --release --test f878_fan01_lda_profiles`: 4 passed, 0 failed, 3 ignored (the heavy record,
  the known miss, the family run), 18 s.
* `cargo fmt -p rcfd-api --check`: clean.
* `cargo clippy -p rcfd-api --all-targets -- -D warnings`: clean.
* `cargo test -p rcfd-cli --release --test docs_site`: passed (2 tests), with this addendum in the tree.
* `python scripts/roadmap_proposals.py check docs/roadmap/proposals/0765-F-878.json`: passes (F-878 todo to partial, gap
  set, one new item).
* `python scripts/process_report.py --check`: 2 failures, both "generated file is not current" (`docs/adr/index.md`,
  `docs/reference-data.md`), which rule 9 leaves to the lead; no other failure.
* The mutation patch was applied by hand on the scratch copy and its named test failed there (M1 above); the
  repository's runner was not run on it.

### Size

Size (agent): partial; 0; 4; 2; 450000

The bytes read are an estimate.

### Decision by the registered rule

**Partial.** The comparison is made at every measured radius; the division cannot be given by the registered rule (C4),
and radii miss their bands (C1: 12, C2: 12). The measured profiles are traced to a source that was read, so the third
case does not apply.

## Open questions for the maintainer

1. **What this means for the compared quantity of F-870.** The result: the missing pressure rise is not in the work of
   the rotor or in its outflow velocity (the conditional share is -3.8 Pa +-31.1); it is in loss and recovery, which
   the record does not measure apart. Options: (a) keep the total-to-static pressure rise of the installation as the
   compared quantity; it then needs the outlet as on the rig and a rotor loss that is believed, and cannot be met by
   the rotor-only passage. (b) Add the rotor quantities that the record does measure, the Euler head and the kinetic
   energy over the measured span (220.02 and 99.20 Pa) and the profiles, as compared quantities of F-870 beside the
   pressure rise, each with its own band; the solve is 6.8 Pa and 3.1 Pa from them today, inside its mesh term, while
   12 of 26 radii miss the profile band. (c) Replace the pressure rise by them. I recommend (b): it keeps the headline
   honest (the pressure rise stays a miss until the outlet is modelled) and gives the rotor a measured quantity that
   does not depend on the rig. It is a change of the acceptance, so it is the maintainer's.
2. **F-880, the outlet as on the rig.** It was to be issued only if this work showed the missing pressure to be the
   installation's. By the rule it shows neither. Options: (a) issue F-880 now: with the outlet modelled, `Rec` is
   computed for the rig's geometry and the identity of this ADR then leaves the rotor's loss as the remainder, so it
   would close the division from the other side; it is a medium to large piece of mesher work with F-133 open. (b)
   First the cheaper piece proposed below: where the 62.87 Pa of simulated loss arise and what a finer mesh makes of
   them (the loss fell by 25.6 and 15.9 Pa in the last two refinements). (c) First have the wall-pressure data of the
   record looked at for a mean static pressure behind the rotor. I recommend (b), with (c) beside it as a short
   reading, and F-880 after: the installation can add at most 86.9 Pa and would have to recover 65 per cent of all
   kinetic energy behind the rotor to close the gap alone, while a loss that still falls with the mesh is the nearer
   unknown. This is an inference from the numbers above, not a result.

## For the lead: roadmap entries

> Status proposed for F-878: partial.

> Note entry: ADR 0765, 2026-11-22, partial: the measured LDA profiles are traced (Zenodo doi:10.5281/zenodo.23943494, lda_data.zip, checksums in docs/reference-data.d/0765-F-878.part) and their datum is settled (x = 90 mm from the upstream hub face). On B1 at x = 90 mm c_x is within its registered band at 14 of 26 radii and c_t at 14 of 26 (largest excess over the band 3.54 and 1.36 m/s). Over the measured span the Euler work is 297.66 W measured and 312.03 W simulated (shaft power 333.30 W), the Euler head 220.02 against 226.86 Pa, the kinetic energy leaving the rotor 99.20 against 102.28 Pa. By the registered rule the missing 52.47 Pa cannot be divided: the rotor's share lies between -34.4 and 59.1 Pa (bands 23.8 and 25.0 Pa), because the loss of the measured rotor is not measured; at equal rotor loss it would be -3.8 Pa and the installation's 56.2 Pa, +-31.1 Pa. Held by crates/rcfd-api/tests/f878_fan01_lda_profiles.rs: four gate tests and a heavy record of B1; six mutations caught.

> Gap entry: Missing for done: the division of the missing 52.47 Pa. Velocity profiles do not fix the loss of the measured rotor, so the rotor's share is only bounded (-34.4 to 59.1 Pa, bands 24 to 25 Pa); needed is a measured static pressure behind the rotor or the outlet as on the rig (F-880). And c_x misses its band at 12 of 26 radii, c_t at 12 (worst at the hub and the tip; the radial layers of B1 are coarser than the measured spacing).

> Follow-up (proposed, open question 2; it diagnoses the miss, so by rule 13 it may instead be a registration under F-878): "FAN-01 rotor: the loss of total pressure of the simulated rotor up to the measured plane, where it arises and what a finer mesh makes of it", deps F-878; acceptance: Every limit not stated here is registered in the ADR before the run. The loss of total pressure of the FAN-01 design-point solve between the inlet and the measured plane of ADR 0765 (62.87 Pa on B1, 78.75 Pa on B2, 104.34 Pa on B3) is divided, by a rule registered before the run, into the parts that arise at the blade surfaces, at the hub and casing, in the tip gap and in the interior of the passage, and is recomputed on one mesh finer than B1 that also refines the wall-normal direction; the report states the loss the family extrapolates to with its band and what that leaves of the missing pressure rise of F-870 for the installation by the identity of ADR 0765. A run that does not decide is recorded as not deciding.

The same entries are in `docs/roadmap/proposals/0765-F-878.json` (form 1 of ADR 0755, base 2bab1863), checked with
`python scripts/roadmap_proposals.py check`.

Generated pages not current on this work, left to the lead (rule 9): `docs/adr/index.md` (ADR 0765 is new) and
`docs/reference-data.md` (the fragment `docs/reference-data.d/0765-F-878.part` is new). `validation-data/manifest.json`
does not list the transcription `docs/investigations/f878-fan01-lda-profiles/lda_profiles.csv`; whether it becomes a
file of the data pack is the lead's.

## The lead's reading (2026-11-22), at the merge

Merged at 7531a4ed (the report read at 13:07 UTC, 6 minutes of reading, proposed partial, written partial). Checked by
the lead before the merge: the registration is the first commit and alone, and amendment 1 is committed before any
registered run; no commit of the branch touches a solver source (the transient edit of `crates/rcfd-fv/src/sources.rs`
to make the mutation patch was restored in the same command, as the addendum says, and no commit carries it); the heavy
tests are ignored with the reason in the attribute, one of them the known miss of the acceptance. Noted, not held
against the merge: about 400 kB of run logs are committed under `docs/investigations/`, the mutations were run by hand
and not by the repository's runner (the push script ran the committed M1), and `validation-data/manifest.json` does not
list the transcription CSV.

Status: **partial**, as proposed, from a copy of `docs/roadmap/proposals/0765-F-878.json` without its follow-up. The
lead reads the result as the agent does: the measured Euler work and the outflow velocity are close to the simulated
ones, so the missing pressure rise of F-870 lies in loss or recovery and not in the work put in, and velocity profiles
alone cannot divide it between the rotor and the installation. The simulated loss falls from 104 to 63 Pa from B3 to B1
and is not converged, which makes the loss budget on a finer mesh the next step that can decide. Questions 1 and 2 are
put to the maintainer, the follow-up with question 2.

## Answers of the maintainer (2026-11-22)

The lead put questions 1 and 2 above to the maintainer on 2026-11-22, with the question of ADR 0764 and three of ADR
0766, each with the agent's recommendation, which the lead shares, and asked whether to take all six. The maintainer answered: "Yes, all six".

1. Option (b): the Euler head and the kinetic energy leaving the rotor over the measured span and the profiles of c_x and
   c_t at the measured plane are compared quantities of F-870 beside the pressure rise and the shaft power, each with a
   band registered before its run; the pressure rise stays a compared quantity and its miss stays a miss. The acceptance
   of F-870 is extended by a sentence that says so. This also answers question 4 of ADR 0762 as to the compared
   quantity; the convergence rule is answered in ADR 0766.
2. Options (b) and (c): first the loss budget of the simulated rotor on a finer mesh, the follow-up of the proposal
   file, entered as F-882 with the title, dependencies and acceptance written there; beside it a short reading of the
   wall-pressure data of the record for a mean static pressure behind the rotor, entered as F-883. F-880 waits on both.
