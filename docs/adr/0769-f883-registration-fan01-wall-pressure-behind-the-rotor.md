# ADR 0769: F-883 registration: FAN-01, a mean static pressure behind the rotor from the wall-pressure data of the record

Status: **REGISTERED 2026-11-22, written before any code of F-883 and before any run of its checks.** Before this text
no test or script of F-883 that reads the wall-pressure data was written, the file that holds them (`mic_kulite.tar`)
was not fetched, and no value of it was seen. What was done: reading (below), one reading script over
`docs/roadmap.json`, the fetch of the record's metadata, its description and its README (checksums below), and the
arithmetic on the committed LDA transcription of ADR 0765 alone (section "Arithmetic done beforehand"). Results go in
addenda at the end; nothing above them is changed after a run.

It answers the brief [`docs/adr/briefs/f883-fan01-wall-pressure.md`](briefs/f883-fan01-wall-pressure.md) and follows
[`docs/process/agent-rules.md`](../process/agent-rules.md), version 5. Branch `f883-fan01-wall-pressure` from main
21054b68; scratch `w49i-work/f883-fan01-wall-pressure-scratch`. No solver source is changed and no solve is run: the
work adds one test file, a transcription with the script that wrote it, a reference fragment, a manifest entry and text.

Size (agent): done; 0; 0; 0; 250000

## What is asked

Roadmap item F-883 (read by script from `docs/roadmap.json`). Its acceptance: "Every limit not stated here is
registered in the ADR before the reading. The wall-pressure data of the FAN-01 record (Zenodo
doi:10.5281/zenodo.23943494, the file named in ADR 0765) are fetched, checksummed and read for a time-mean static
pressure on the casing or hub behind the rotor at the design point, with its station, its uncertainty as the record
states it, and the page or file it comes from; if one is there, the identity of ADR 0765 is evaluated with it and the
rotor's share of the missing pressure rise is stated with its band; if none is there, that is recorded as the result."

The brief's four tasks: fetch and checksum, with a reference fragment; read, with the design point and the mean
registered before; if a pressure is there, the identity of
[ADR 0765](0765-f878-registration-fan01-lda-profiles-euler-work-and-the-division-of-the-missing-pressure-rise.md) with
it and the rotor's share with its band; a fast test over the committed transcription.

## What was read

Whole: the brief; `AGENTS.md`; `docs/process/agent-rules.md` (version 5); `docs/reference-data.d/0765-F-878.part`;
of ADR 0765 the registration (sections "The measured profiles, traced" to "What is not covered"), amendment 1 and
addendum 1 with the answers of the maintainer; of `docs/validation-data.md` lines 78 to 152 (FAN-01); the FAN-01 entry
of `validation-data/manifest.json` and the checks of `crates/rcfd-io/tests/datapack.rs` and
`crates/rcfd-io/src/datapack.rs` (lines 25 to 210) that a manifest entry must pass; the table of
[`docs/process/tolerances.md`](../process/tolerances.md); `docs/roadmap/proposals/0765-F-878.json` (the form);
`docs/investigations/f878-fan01-lda-profiles/lda_profiles.csv` and the summary lines of `analysis.txt` beside it. Of
`docs/roadmap.json`, by script: F-870, F-882 and F-883.

The record, fetched on 2026-11-22 from zenodo.org through its public API: the metadata of record 23943494, the
description `LA_Case4_AxialFan_measurements.pdf` and `README.md`. The description was read as text (`pdftotext
-layout`), whole; of its figures only captions and axis labels come through.

| File | Bytes | MD5 (equal to the record's) | SHA-256 |
| --- | --- | --- | --- |
| `LA_Case4_AxialFan_measurements.pdf` | 1 639 807 | 056fc203852d6186a583f3ade8f2603b | 547d17de2d3c8ab025624e17d3d61abd8253fedacd77cf079cd1805c8a76e8a6 |
| `README.md` | 6 527 | 7162aa176fe9b1ed73d9ef8f00a74fa3 | 7e58f996b82e5df32dbd5cf55228789407c3eed8ec70101e9c6b235598035161 |
| `mic_kulite.tar` (the data; not fetched yet) | 460 728 320 (metadata) | 1002be297aa2ac2123f3928bf7f5d905 (metadata) | to be recorded |

The two documents are the same bytes ADR 0765 read (equal checksums).

## What the record says of the wall-pressure data (description; before any reading of the data)

* **Instrument** (Table 2): differential pressure transducers, type XCS-093-1psi D; 30 s at 48 kHz, synchronous
  with the seven microphones. Nothing else: no range used, no calibration, no accuracy, no zero, no statement of an
  uncertainty of the mean or of the fluctuations.
* **The quantity** (Appendix A.2, Table A2): `kulite_pressure_in_Pa` in `mic_kulite_n1ug_1_4.h5` / `.mat`, 15 rows of
  N = 1 464 320 samples (30.507 s), unit Pa, described as "wall pressure, differential to the chamber pressure, mean
  value included"; row n is transducer n (n = 1 to 14); row 0 holds no wall-pressure signal (Appendix B, item 1: its
  rms is 0.16 Pa against 11 to 184 Pa in rows 1 to 14, its mean about 0).
* **The stations** (Table 4, Appendix A.2): one axial line on the duct wall, r = 250 mm, x = 10 n - 33 mm, so
  transducers 1 to 14 at x = -23 to 107 mm; circumferential position not dimensioned. The mapping of rows to
  transducers "is inferred, not documented"; the description gives four positions of an earlier source that agree
  with it (transducers 2, 7, 9, 13 at x = -13, 37, 57, 97 mm). No transducer is on the hub.
* **The operating point** (section "Fan and operating point"): "All time series (wall pressure, microphones, array)
  were recorded at the design point 1.4 m^3/s"; the file name carries `n1ug_1_4`, as the LDA files do. The volume
  flow and the chamber pressure of the run of the time series are not given.
* **The rotor** (Table 3): leading edge x = 21.6 (root) to 41.6 mm (tip), trailing edge 92.1 (root; CAD 89.3 at r =
  130 mm) to 62.8 mm (tip); rear hub face 104.5 mm; duct/diffuser joint 145.0 mm. On the casing, transducers 10 to 14
  (x = 67 to 107 mm) are behind the blade tips; 12 and 13 (x = 87 and 97 mm) enclose the LDA plane x = 90 mm.
* **The reference.** The chamber is the ISO 5801 inlet chamber from which the fan draws, its air at rest; its static
  pressure is `p_amb - p_ts`, and it is the reference of `p_ts` (126.46 Pa at the design point). So a mean of
  `kulite_pressure_in_Pa` is, as the record states it, `p_wall - p_chamber`: the static pressure at the casing against
  the total pressure of the air at rest ahead of the fan. That is the reference of `Pi` in ADR 0765 ("the static
  pressure at the plane against the inlet total pressure").

So the description says a time-mean is in the file and what it is referred to. Whether it is there and usable is
what the reading decides, by the rules below.

## The reading (fixed now)

**The design point** is the operating point of the file `mic_kulite/mic_kulite_n1ug_1_4.mat` (or its `.h5` copy) of
`mic_kulite.tar`, which the record states to be 1.4 m^3/s, the point of the LDA files and of the characteristic's
1.4032 m^3/s and 126.46 Pa. No other file is read for it. That the run of the time series and the run of the
characteristic were at the same flow is the record's statement and is not tested (no flow of the time series run is
in the record).

**The mean** of row n is the arithmetic mean of all N samples of `kulite_pressure_in_Pa` row n, in Pa, as stored. No
filter, no detrending, no removal of samples. Beside it, from the same samples: the standard deviation about the
mean, the root of the mean square, and the means of 30 consecutive batches of 48 000 samples (1 s, 24.8 revolutions
each; the last 24 320 samples are in the mean but in no batch).

**The statistical uncertainty of a mean**, `u_stat`, is twice the standard error formed from the 30 batch means
(sample standard deviation of the batch means over sqrt(30)): first principles, the central limit theorem over
batches long against the period of the rotor; no row of the tolerance table covers it.

**The file and its reader.** The `.mat` file is read with a reader of the MAT level-5 format written for this work
(standard library and numpy; the reader of ADR 0765, `mat5.py`, is the starting point). If the `.mat` file is not in
a form that reader can read (a MAT 7.3 file is HDF5) and no HDF5 reader is on the machine, the data "cannot be read
as registered" (decision rule). The HDF5 attributes `units` and `description` are read if the `.h5` copy can be read;
otherwise not.

**The station** is x = 90 mm on the casing, the LDA plane of ADR 0765 where `Pi_s`, `e_m` and `q_m` are formed. The
wall pressure there is the linear interpolation between transducers 12 and 13,
`p_w(90) = 0.7 p_12 + 0.3 p_13`, and its batch means are formed the same way from the batch means of rows 12 and 13
(the rows are synchronous). The term of the place, `u_x`, is half the range of the interpolated value over x = 88.7
to 91.3 mm, the +-1.3 mm of the plane in ADR 0765: `u_x = 0.13 abs(p_13 - p_12)`.

**From the casing to the mean over the span.** `Pi` of ADR 0765 is a flux-weighted mean over the measured span, not a
wall value. A swirling flow has a higher static pressure at the casing than inside; the time-mean radial momentum
equation of an axisymmetric mean flow, inviscid outside the wall layer, written with the time-mean products `<.>`
(mean and fluctuation together),

`dp/dr = rho (<c_t^2> - <c_r^2>) / r - rho d<c_r^2>/dr - rho d<c_x c_r>/dx`,

integrated from r to the wall R = 250 mm, where `<c_r^2>` is zero, gives

`p_w - p(r) = int_r^R rho (<c_t^2> - <c_r^2>) / r' dr' + rho <c_r^2>(r) - int_r^R rho d<c_x c_r>/dx dr'`.

The last term needs an axial derivative, and the record has one plane behind the rotor; it is left out and listed as
not covered. The rest is measured at x = 90 mm: `<c^2> = mean^2 + rms^2` of the time-averaged file (columns
`uphi_mean`, `uphi_rms`, `ur_mean`, `ur_rms` of `lda_profiles.csv` of ADR 0765). The integral is the trapezoid rule
on the 26 measured radii, and from r = 246 mm to the wall the integrand is taken to fall linearly to zero (the end
piece `0.5 f(r_26) (R - r_26)`, with `f = rho (<c_t^2> - <c_r^2>) / r`). Then

`G(r_i) = p_w - p(r_i)`, `Gbar = int ux_mean G 2 pi r dr / int ux_mean 2 pi r dr` (trapezoid, the 26 radii),

`Pi_m = p_w(90) - Gbar`.

Its band, added as ADR 0765 adds:

* `u_stat` and `u_x` as above;
* `u_gap = 0.5 f(r_26) (R - r_26)`, the end piece itself (the integrand between 246 mm and the wall lies between zero
  and its value at 246 mm);
* `u_read = abs(Gbar - Gbar_0)`, with `Gbar_0` formed from the means alone (no rms): the whole effect of the
  fluctuation terms, because the files do not say whether the rms is about the time mean or about the bin means;
* `u_meas`: the uncertainty of the wall pressure as the record states it. **None is stated in the description or the
  README.** If the data file states one, it is that; otherwise `u_meas = 0`, said so, not assumed.

`U_m = u_stat + u_x + u_gap + u_read + u_meas`.

## Arithmetic done beforehand (the LDA transcription alone)

By `pre_lda.py` (scratch) over `docs/investigations/f878-fan01-lda-profiles/lda_profiles.csv`, rho = 1.2 kg/m^3 as in
ADR 0765:

| | `Gbar` [Pa] | `G` at r = 133 mm [Pa] | `G` at r = 246 mm [Pa] | end piece [Pa] |
| --- | --- | --- | --- | --- |
| means and rms (the reading) | 19.932 | 37.674 | 21.495 | 0.712 |
| means alone (`Gbar_0`) | 14.198 | 36.778 | 1.494 | 0.687 |

So `Pi_m = p_w(90) - 19.93 Pa`, with `u_gap = 0.71 Pa` and `u_read = 5.73 Pa` before any wall value is seen. The
volume flow of the weighting is 1.35249 m^3/s, that of ADR 0765.

## The identity, with the measured pressure (fixed now)

From ADR 0765, unchanged: `dp_ts = Pi + Rec`, `Pi = e - q - L`, for the rig (m) and the solve (s); `D = 126.46 -
73.99 = 52.47 Pa`, `D / 2 = 26.24 Pa`. With `Pi_m` measured:

* **the rotor's share** `R = Pi_m - Pi_s`, with `Pi_s = 61.709 Pa` (B1, the recorded state of ADR 0765, over the
  measured span at x = 90 mm);
* **the installation's share** `I = D - R`;
* its band `U = U_m + U_s`, with `U_s = 21.683 Pa`, the band of `Pi_s` of ADR 0765 (`U_num` 21.280, `S` 0.0008, `T`
  0.008, `u_cov` 0.394);
* beside them, the terms of the rig that the measured pressure gives: `L_m = (e_m - q_m) - Pi_m` with `e_m - q_m =
  120.819 Pa` (pointwise reading of ADR 0765), and `Rec_m = 126.46 - Pi_m`.

**The verdict**, the rule of ADR 0765 with the interval replaced by `R +- U`: if `R + U < D / 2` the larger part is
the installation's; if `R - U > D / 2` the larger part is the rotor's; otherwise it cannot be told, and the shares
are stated all the same. Before any wall value: the rotor's part is the larger if `Pi_m > 116 Pa` or so, the
installation's if `Pi_m < 60 Pa` or so (with `U_m` near 7 Pa); I have no basis for an estimate of `p_w(90)`.

**Consistency with the LDA.** ADR 0765 bounds `Pi_m` by first principles from the measured velocities: `L_m >= 0`
gives `Pi_m <= 120.819 Pa`, and `Rec_m <= q_m` gives `Pi_m >= 126.46 - 99.196 = 27.264 Pa`. The reading is consistent
if `27.264 - U_m <= Pi_m <= 120.819 + U_m`.

## Checks of the data before they are used (fixed now)

* **K1** (the record's own figures): row 0 has a standard deviation, or a root mean square, that rounds to 0.16 Pa and
  a mean whose magnitude is below 0.5 Pa ("about 0"); the smallest and the largest of rows 1 to 14 round to 11 and
  184 Pa, under the same one of the two readings of "rms", which is named. This tests the reader and the order of
  rows.
* **K2** (first principles: no work is done on the air before it reaches the blades, so ahead of them the static
  pressure at the wall is at most the total pressure of the chamber): the means of rows 1 to 5 (x = -23 to 17 mm,
  ahead of the root leading edge at 21.6 mm and 24.6 mm or more ahead of the tip leading edge) are each at most
  `+u_stat` of their own.
* **K3** (the reference): the mean of row 1 (x = -23 mm, in the duct behind the nozzle) is above -126.46 Pa. If the
  rows were referred to the ambient and not to the chamber, row 1 would lie below `-p_ts`; referred to the chamber it
  is `-(rho c^2 / 2 + loss)` with a mean duct velocity of 7.1 m/s there (1.4 m^3/s over r = 250 mm), far above.

If K2 or K3 fails, the means are not a static pressure against the chamber as the record states, and the result is
that the record holds no usable time-mean static pressure behind the rotor (the identity is not evaluated). If K1
fails the reader is wrong, and that is corrected (by an amendment if the method changes) before anything is used.

## The transcription and the test (fixed now)

* `docs/investigations/f883-fan01-wall-pressure/kulite_means.csv`: for rows 0 to 14, `row`, `x_mm` (empty for row 0),
  `n`, `mean`, `std`, `rms`, and the 30 batch means, in Pa, to 1e-6 Pa; written by `kulite_means.py` beside it, from
  the `.mat` file, with the checksums of the archive and of the file it read in its header. A file entry for it in
  the FAN-01 dataset of `validation-data/manifest.json` (local path, SHA-256, bytes).
* `docs/reference-data.d/0769-F-883.part`: the record, the file, its checksums, what it says, and the values used.
* `crates/rcfd-api/tests/f883_fan01_wall_pressure.rs` (header: ADR 0769, F-883), all run by the gate, no solve, no
  heavy record:
  * `f883_transcription_constants`: the constants of the test (means and batch means of rows 0, 1 to 5, 12, 13, the
    standard deviations and root mean squares of all rows) equal the CSV to its printed digits;
  * `f883_radial_equilibrium_closed_form`: the correction on a field whose integral the trapezoid rule gives exactly:
    `<c_t^2> = a r + k r`, `<c_r^2> = k r`, `ux_mean = b / r` on the 26 radii, for which
    `G(r) = rho a (r_26 - r) + 0.5 rho a (R - r_26) + rho k r` and `Gbar` is the mean of the linear `G` over the
    radii; relative error at most 1e-9;
  * `f883_reading_and_share`: K1 to K3 from the constants; `p_w(90)`, its `u_stat` and `u_x`; `Gbar`, `u_gap`,
    `u_read` from the LDA transcription of ADR 0765; `Pi_m`, `U_m`, `R`, `I`, `U`, `L_m`, `Rec_m`, the verdict and
    the consistency, each equal to the value the Python script gave (0.005 Pa) and the verdict equal.

## Table of checks

| Id | Check | Limit | Source of the limit | Estimate | Margin | Kind |
| --- | --- | --- | --- | --- | --- | --- |
| A1 | `mic_kulite.tar` fetched; its MD5 equals the record's; SHA-256 recorded, also of the file read inside it | equal | acceptance of F-883 ("fetched, checksummed"); the record's metadata | equal | n/a | code verification |
| A2 | a time-mean static pressure on the casing behind the rotor at the design point: station, mean, uncertainty as the record states it, file and place | present with all four, or recorded as absent | acceptance of F-883 | present at x = 87 and 97 mm; no stated uncertainty | n/a | validation |
| A3 | the identity evaluated with `Pi_m`: `R`, `I`, `U`, the verdict by the rule above | the rule above | acceptance of F-883 ("the rotor's share ... is stated with its band"); ADR 0765, the rule of division | no number | n/a | validation |
| B1 | `Pi_m` against the first-principles bounds of ADR 0765 | `27.264 - U_m <= Pi_m <= 120.819 + U_m` | ADR 0765, section "The rule that divides" (`L_m >= 0`, `Rec_m <= q_m`) | inside | n/a | validation |
| K1 | the figures of Appendix B, item 1 | as stated above | the record's description, Appendix B | met | n/a | code verification |
| K2 | means of rows 1 to 5 | each `<= u_stat` | first principles (energy equation ahead of the blades) | -20 to -60 Pa | n/a | validation |
| K3 | mean of row 1 | `> -126.46 Pa` | first principles with the record's reference (chamber) | about -30 to -60 Pa | about 2 | validation |
| V1 | `f883_transcription_constants` | equal to the printed digits (1e-6 Pa) | the transcription | exact | n/a | code verification |
| V2 | `f883_radial_equilibrium_closed_form` | relative error `<= 1e-9` | `docs/process/tolerances.md`, "Agreement with a closed form" (exact for the discrete problem) | 1e-15 | 1e6 | code verification |
| V3 | `f883_reading_and_share`: the Rust values equal the Python values | 0.005 Pa; verdict equal | `docs/process/tolerances.md`, "Same discrete problem by two routes", at the printed digits of this ADR | 1e-9 Pa | n/a | code verification |
| D1 | no file under `crates/*/src` changed; no solve | 0 files | rule 3; the brief | 0 | n/a | unchanged default output |
| P | `cargo fmt -p rcfd-api --check`; `cargo clippy -p rcfd-api --all-targets -- -D warnings`; `cargo test -p rcfd-cli --release --test docs_site`; `cargo test -p rcfd-io --release --test datapack`; `python scripts/roadmap_proposals.py check`; `python scripts/process_report.py --check` | clean; passes; no failure but a generated file not current | rule 7 | as stated | n/a | code verification |

A2, A3 and B1 are the rows against the measurement. A2 and A3 decide the item; B1 says whether the reading agrees
with the LDA of the same record.

## Negative controls and planned mutations

On a scratch copy of the tree (`git archive`, own target folder), each against the gate tests of the new file.

| Id | Mutation | Must be caught by |
| --- | --- | --- |
| M1 | the interpolation weights at x = 90 mm swapped (`0.3 p_12 + 0.7 p_13`) | `f883_reading_and_share` |
| M2 | the mean of row 12 in the constants changed by +1 Pa | `f883_transcription_constants` |
| M3 | the `1 / r` of the integrand of the radial equilibrium left out | `f883_radial_equilibrium_closed_form` |
| M4 | the term `rho <c_r^2>(r)` left out | `f883_radial_equilibrium_closed_form` |
| M5 | the sign of the correction (`Pi_m = p_w + Gbar`) | `f883_reading_and_share` |
| K0 | negative control: `kulite_means.py` run twice | equal bytes (SHA-256 of the CSV) |

M1 is committed as `crates/rcfd-api/tests/f883_fan01_wall_pressure.mutation.patch` with the first line
`# expect-fail: cargo test -p rcfd-api --release --test f883_fan01_wall_pressure f883_reading_and_share`.

## Decision rule

* **Done** if A1 is met and either (a) A2 is met, K1 to K3 hold, `Pi_m` is formed with `U_m` and A3 states `R`, `I`
  and `U` with the verdict, whatever the verdict, and B1 is met; or (b) the record is read and holds no time-mean
  static pressure on the casing or hub behind the rotor at the design point, or K2 or K3 shows the means are not a
  static pressure against the chamber: that is the result.
* **Partial** if the data cannot be read as registered (the file is not in the record, not in a form that can be read
  here, or K1 cannot be met), or if B1 misses: the share is then stated with its numbers and the miss recorded.

## What is not covered

* The uncertainty of the transducers, unless the data file states one (the description and README do not); vendor
  documentation of the transducer is not a source.
* The axial-derivative term of the radial equilibrium (`d<c_x c_r>/dx`): one plane behind the rotor; the plane is
  14.5 mm ahead of the rear hub face, where the flow turns. Its size is not known and it is not in the band.
* The circumferential position of the transducer line and the non-uniformity around the casing (one line only).
* The flow and the chamber pressure of the run of the time series; the density (F-877; rho = 1.2 kg/m^3 assumed for
  the LDA terms; the wall pressure is a pressure and needs none).
* The inferred mapping of rows to transducers, beyond the four positions the description checks it against; the
  value at x = 90 mm with the mapping moved by one row either way is reported, not banded.
* The fluctuations of the wall pressure and the microphone and array data. The loss budget of the solve (F-882) is
  another agent's.

Every link of this file was checked with `cargo test -p rcfd-cli --release --test docs_site` before its commit.

## Addendum 1: results

Written 2026-11-22, after the reading. Commits on the branch, from main 21054b68: d1d7e788 (this registration, alone,
before the data were fetched), 4d9f92ec (the test file, the transcription `kulite_means.csv` with `kulite_means.py`,
the script `wall_pressure_share.py` and its output `analysis.txt` in `docs/investigations/f883-fan01-wall-pressure/`,
the reference fragment, the manifest entry, the paragraph of `docs/validation-data.md`), cff4b861 (the mutation
patch), and the commit of this addendum with the proposal file. No amendment was needed.

### The sentence this work was to deliver

**The record gives a time-mean static pressure behind the rotor at the design point**: on the casing, against the
chamber, 112.77 Pa at x = 87 mm and 116.54 Pa at x = 97 mm (transducers 12 and 13 of `kulite_pressure_in_Pa` in
`mic_kulite/mic_kulite_n1ug_1_4.mat`), so 113.90 Pa at the LDA plane x = 90 mm; the record states no uncertainty for
it. Carried over the measured span by the radial equilibrium of the measured profiles, `Pi_m` = 93.96 Pa +- 7.50 Pa.
**With it the rotor's share of the missing 52.47 Pa is 32.26 Pa, band 29.19 Pa, and the installation's 20.21 Pa**: by
the rule of ADR 0765 it cannot be told which is the larger (`R - U` = 3.07 Pa is not above 26.24 Pa, `R + U` = 61.44
Pa is not below it). Of the band, 21.68 Pa is the solve's (its mesh term alone 21.28 Pa), 7.50 Pa the measurement's.

What the measured pressure does decide: the interval of ADR 0765 for the rotor's share (-34.4 to 59.1 Pa) closes to
32.3 +- 29.2 Pa, and the rig's terms are now measured: `L_m` = 26.85 Pa of rotor and inlet loss against 62.87 Pa in
the solve, and `Rec_m` = 32.50 Pa recovered behind the plane against 12.28 Pa in the solve. Both differ in the
direction that makes the missing pressure rise: the simulated rotor loses 36 Pa more and its annulus recovers 20 Pa
less.

### The reading (A1, A2, K1 to K3)

`mic_kulite.tar`: 460 728 320 bytes, MD5 1002be297aa2ac2123f3928bf7f5d905 (equal to the record's), SHA-256
ce02ca09c4c2ec84da54a09ccb271b3012e190110ff95c9b44c412a7a7032862. It holds `mic_kulite_n1ug_1_4.mat` (MAT level 5;
SHA-256 79a2025fcacec78cf7d0a2942c128ec1f9e82b103bafb7f4d880580ed1507b1c) and the `.h5` copy (SHA-256
0e330c8a4874065b4a9b59e699b9f5955394e8f5421ab073a702ab1351028aca; not read: no HDF5 reader). The `.mat` holds
`time_in_s` (1 464 320 x 1), `mic_pressure_in_Pa` (x 7), `kulite_pressure_in_Pa` (x 15) and `sample_freq_in_Hz`
(48000), all doubles; it carries no unit, description or uncertainty beyond the names.

| row | x [mm] | mean [Pa] | 2 s.e. [Pa] | std [Pa] |
| --- | --- | --- | --- | --- |
| 0 | - | 0.0038 | | 0.159 |
| 1 to 5 | -23 to 17 | -49.83, -47.80, -44.82, -42.58, -39.49 | 0.48 to 0.60 | 11.1 to 47.2 |
| 6 to 8 | 27 to 47 | -42.05, -53.22, -68.15 | | 76.5 to 184.1 |
| 9 | 57 | 26.50 | | 77.6 |
| 10 to 14 | 67 to 107 | 92.82, 104.81, 112.77, 116.54, 117.30 | 0.52 to 0.69 | 39.5 to 11.5 |

(every row in `analysis.txt`). K1 met under the reading "standard deviation about the mean": row 0 0.159 Pa, mean
0.004 Pa; rows 1 to 14 from 11.13 to 184.07 Pa (the root mean square gives 50.16 to 196.28 Pa and does not). K2 met:
rows 1 to 5 are -49.8 to -39.5 Pa. K3 met: row 1 is -49.83 Pa, above -126.46 Pa, so the reference is the chamber, as
the record states. On the casing the pressure rises through the blade row from -68 Pa at x = 47 mm to +93 Pa at 67 mm,
just behind the tip trailing edge (62.8 mm), and goes on rising slowly to 117 Pa at 107 mm.

### The identity (A3, B1)

| | value [Pa] | band terms [Pa] |
| --- | --- | --- |
| `p_w(90) = 0.7 p_12 + 0.3 p_13` | 113.8964 | `u_stat` 0.5674, `u_x` 0.4901 |
| `Gbar` (casing minus span mean) | 19.9318 | `u_gap` 0.7124, `u_read` 5.7339 (`Gbar_0` 14.1979), `u_meas` 0 (not stated) |
| `Pi_m` | 93.9646 | `U_m` 7.5038 |
| `R = Pi_m - Pi_s` (61.709) | 32.2556 | `U` = 7.5038 + 21.683 = 29.1868 |
| `I = D - R` | 20.2144 | the same |
| `L_m = 120.819 - Pi_m` | 26.8544 | |
| `Rec_m = 126.46 - Pi_m` | 32.4954 | |

Verdict: **cannot be told**. B1: `Pi_m` = 93.96 Pa lies inside 19.76 to 128.32 Pa: **met**; the reading agrees with
the energy balance of the LDA profiles (`L_m >= 0`, `Rec_m <= q_m`).

Reported, not banded (the inferred mapping): with every row moved by one transducer the value at x = 90 mm would be
107.20 Pa (rows 11 and 12) or 116.76 Pa (rows 13 and 14), `R` 25.6 or 35.1 Pa; the verdict is the same in both.

### Every check

| Id | Measured | Met |
| --- | --- | --- |
| A1 | MD5 equal to the record's; SHA-256 of the tar and of both files inside recorded | yes |
| A2 | casing, x = 87 and 97 mm (and 67 to 107 mm), the design point; 113.90 Pa at x = 90 mm against the chamber; uncertainty not stated by the record; file `mic_kulite_n1ug_1_4.mat`, variable `kulite_pressure_in_Pa`, rows 12 and 13 | yes |
| A3 | `R` 32.26 Pa, `I` 20.21 Pa, `U` 29.19 Pa; verdict "cannot be told" | stated |
| B1 | 93.96 Pa in [19.76, 128.32] Pa | yes |
| K1 | as above | yes |
| K2 | -49.83 to -39.49 Pa, each at most its `u_stat` | yes |
| K3 | -49.83 Pa > -126.46 Pa | yes |
| V1 | `f883_transcription_constants` | yes |
| V2 | `f883_radial_equilibrium_closed_form`, relative error at most 1e-9 | yes |
| V3 | `f883_reading_and_share`: every value within 0.005 Pa of the Python value (equal at the printed digits), verdict equal | yes |
| D1 | no file under `crates/*/src` in `git diff --name-only 21054b68`; no solve | yes |
| P | below | below |

### What is a gate test and what is a heavy record

All three tests of `crates/rcfd-api/tests/f883_fan01_wall_pressure.rs` run with every `cargo test` (under 1 s); none
is ignored and there is no heavy record. The 460 MB archive is not committed; the transcription (6 609 bytes) is, with
an entry in `validation-data/manifest.json` whose checksum the test `datapack` checks.

### Mutations and negative controls

On a copy of the tree in the scratch folder (`git archive` plus the new files, its own target folder), by a script.

| Id | Caught by |
| --- | --- |
| M1 weights swapped | `f883_reading_and_share`; committed as the mutation patch |
| M2 row 12 mean +1 Pa | `f883_transcription_constants` (and `f883_reading_and_share`) |
| M3 `1 / r` left out | `f883_radial_equilibrium_closed_form` (and `f883_reading_and_share`) |
| M4 `rho <c_r^2>(r)` left out | `f883_radial_equilibrium_closed_form` (and `f883_reading_and_share`) |
| M5 sign of the correction | `f883_reading_and_share` |
| K0 | `kulite_means.py` run twice: the same SHA-256, 06b92eb5f44bcebf5e448903a52fae4dab5b996971576dcf51f84fda6bcaf593 |

### Deviations

1. The reader uses the standard library only (`array`, `math.fsum`), not numpy: the registration allowed both, and
   `python -I` does not see the numpy of the user site. The method is unchanged.
2. The mutations were run by a script on the scratch copy; the repository's mutation runner was not used.

### What was not run

* The `.h5` copy of the data (no HDF5 reader); the description says it holds the same values.
* Anything for the term `d<c_x c_r>/dx` of the radial equilibrium; its size is not known and it is not in the band.
* No solve; nothing of F-882.

### The checks of rule 7

* `cargo test -p rcfd-api --release --test f883_fan01_wall_pressure`: 3 passed, 0 failed, 0 ignored.
* `cargo test -p rcfd-io --release --test datapack`: 4 passed (the manifest entry and its checksum).
* `cargo fmt -p rcfd-api --check`: clean. `cargo clippy -p rcfd-api --all-targets -- -D warnings`: clean.
* `cargo test -p rcfd-cli --release --test docs_site`: passed, with this addendum in the tree.
* `python scripts/roadmap_proposals.py check docs/roadmap/proposals/0769-F-883.json`: passes (F-883 todo to done).
* `python scripts/process_report.py --check`: run last; its result is in the report to the lead.

### Size

Size (agent): done; 0; 0; 1; 260000

One question for the maintainer arose (below). The bytes read are an estimate.

### Decision by the registered rule

**Done**, by case (a): A1 met; A2 met (a time-mean static pressure on the casing behind the rotor at the design
point, with its station, the record's silence on its uncertainty, and its file and place); K1 to K3 hold; `Pi_m` is
formed with `U_m`; A3 states `R`, `I` and `U` with the verdict; B1 is met. The verdict "cannot be told" is a result,
not a miss: the rule of done does not depend on which part is the larger.

## Open questions for the maintainer

1. **A measured pressure behind the rotor as a compared quantity of F-870.** The record gives one that the solve can
   be compared with directly, without the radial-equilibrium correction: the casing pressure against the inlet total
   pressure at x = 67 to 107 mm (92.8 to 117.3 Pa; 113.90 Pa at x = 90 mm). Options: (a) add the casing pressure at
   x = 90 mm (or at the five stations behind the tips) as a compared quantity of F-870 beside the Euler head and the
   kinetic energy, with a band registered before its run; (b) keep it as reference data only, used through `Pi_m` by
   F-882 and F-880. I recommend (a): it is a rotor-local measured pressure and tests the simulated loss directly; the
   solve would have to be sampled at the casing, which the tests of F-878 do not do yet. It changes an acceptance, so
   it is the maintainer's.

## For the lead: roadmap entries

> Status proposed for F-883: done.

> Note entry: ADR 0769, 2026-11-22, done: the record's wall pressure (mic_kulite.tar, MD5 equal to the record's, checksums in docs/reference-data.d/0769-F-883.part) gives a time-mean casing pressure against the chamber of 113.90 Pa at x = 90 mm (no stated uncertainty), 93.96 Pa +-7.50 Pa over the measured span by the radial equilibrium of the LDA profiles; with it the rotor's share of the missing 52.47 Pa is 32.26 Pa +-29.19 Pa (rig rotor loss 26.85 Pa against 62.87 Pa simulated), so by the rule of ADR 0765 it cannot be told which part is larger; held by crates/rcfd-api/tests/f883_fan01_wall_pressure.rs, three gate tests, five mutations caught.

The same entry is in `docs/roadmap/proposals/0769-F-883.json` (form 1 of ADR 0755, base 21054b68), checked with
`python scripts/roadmap_proposals.py check`. No follow-up is proposed; question 1 would extend the acceptance of
F-870. Generated pages not current on this work, left to the lead (rule 9): `docs/adr/index.md` (ADR 0769 is new) and
`docs/reference-data.md` (the fragment `docs/reference-data.d/0769-F-883.part` is new).
