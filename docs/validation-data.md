# Public validation-data pack (F-125)

Open benchmark data for validating rcfd, with fetch scripts, SHA-256 checksums and licence records.
This page is a lookup record for anyone checking where a dataset comes from, under what licence, and how to fetch and verify it.
It also records the published sources read for the fan work. Read it in this order: the pack layout and rules, then the unresolved
gaps and the findings made while pinning, then the rotating-machinery and small-fan source surveys, and last the provenance table of
every dataset. In the source surveys, READ means a document or data file was opened and the relevant content read, seen means only a
citation, record page or search snippet was seen, NOT read means it was not opened, and UNVERIFIED marks a value not checked against a
primary text. For the benchmark values and constants typed into tests, and how each was checked, read
[reference data](reference-data.md) instead; for the later re-check of values marked unverified there, see the
[verification report](verification-report.md).

Everything lives in `validation-data/`:

- `manifest.json`: datasets, upstream URLs (pinned to a commit or tag where the host allows it),
  SHA-256 and size of every file, licence record (SPDX id, statement, URL, attribution, caveats),
  citation, and the roadmap case it validates. In-repo reference tables (`cases/*/reference`) are listed
  with `local` paths and checksums too.
- `fetch.py`: standard-library Python. `list`, `fetch [id]` (downloads into `validation-data/data/<id>/`,
  git-ignored, and verifies every checksum; a mismatch is an error and nothing is overwritten),
  `verify [id]` (offline), `pin` (maintainer: recompute the checksums after editing the manifest).
- `rcfd data list|verify|provenance`: the same manifest from the CLI, offline. `rcfd cases list` shows
  the data behind each case. `provenance` prints the table below.

Rules
- CI and `cargo test` never use the network and never need the downloaded files. The tests
  (`crates/rcfd-io/tests/datapack.rs`, `crates/rcfd-cli/tests/data.rs`) check the manifest structure,
  that every dataset has a licence record, citation and checksums, that in-repo tables match their
  checksums, that sources are public hosts, and that nothing mentions a proprietary install.
- Data come from public upstream sources only; copies of datasets distributed with commercial software
  are never used.
- Files under GPL (the OpenFOAM damBreak set-up) are fetched on the user's machine and never vendored.
- Cross-check done when the pack was pinned (2026-10-13): the independent SHA-256 implementation in Rust
  (`rcfd-io::datapack`) agrees with Python's hashlib on all 32 listed files (`rcfd data verify`).

Unresolved
- `hannover-compressor`: no public upstream with an open licence was found; the entry is kept with the reason
  so the gap is visible. Rotor 37 is represented by the NASA TP-1337 report (geometry and overall
  performance); detailed LDV data (AGARD-AR-355) are not fetched.
- GRI-Mech 3.0 from Berkeley carries no explicit licence on its page (citation requested); its legal
  status is recorded as unverified. The Cantera copy sits in a BSD-3-Clause repository.

Findings while pinning
- The TMR CFL3D SST skin-friction file `FlatPlate/SST/cf_plate_sstv.dat` gives Cf(x = 0.97) of about 2.691e-3
  (finest grid; 2.6909e-3 interpolated from the two bracketing rows), while `tests/flat_plate_sst.rs` quotes
  0.00268299 (273 x 193 grid from the TMR page). The rcfd value 0.002663 is -0.7 % against the
  page value and -1.0 % against the pinned finest-grid file. The test and its 5 % tolerance are unchanged.

## Rotating machinery sources (wave 34D, ADR 0075)

Searched and read on 2026-10-19 for the fan milestone (docs/milestone-fan.md) and the F-126 acceptance. "Read" means the document or data file
was opened and the relevant content read; "seen" means only a citation, a record page summary or a search snippet. The solver that would run
these cases: incompressible SIMPLE/SIMPLEC with `rotating_frame` (or `mrf`, refused on the fan sector mesh), rotational-periodic sector, mixing
plane, k-omega SST; mesh generators `rcfd_mesh::fan` (NACA 4-digit or double-circular-arc sections on cylinders, cylindrical hub, cylindrical
shroud with a tip gap; H-slit or C-grid passage). The compressible solver is explicit Euler without rotation, viscosity or turbulence.

Ranking (criteria fixed in ADR 0075 before reading: incompressible regime, geometry public and representable, measured quantity defined,
licence): 1. FAN-01; all others not runnable as they are.

**1. FAN-01, FAU / EAA low-pressure axial fan** (rank 1, dataset `w34d-fan01-fau-eaa-axial-fan`).
- Sources. Zenodo record "FAN 01 Benchmark: Low Pressure Axial Fan in a Short Duct", doi:10.5281/zenodo.20037409 (v5), licence CC BY 4.0:
  READ `characteristic_n1ug.mat` (all 20 points, checked by a second independent parse), `cad_model.igs` (blade surfaces), the LDA time-averaged
  `.mat` files and the rig description `LA_Case4_AxialFan_measurements.pdf`. Schoder, Junger, Kaltenbacher, Acta Acustica 4 (2020) 22 (CC BY 4.0),
  READ. Schoder and Czwielong, arXiv:2211.12014 (pp. 1-4), READ. Junger, PhD thesis TU Wien 2019 (doi:10.34726/hss.2019.40572, section 4.1), READ.
  Vourakis and Karlsson, round robin of the fan, FAN 2022 (CC BY 4.0), READ. Zenger, Junger, Kaltenbacher, Becker, SAE Technical Paper
  2016-01-1805 (the primary fan and rig paper): NOT read (SAE page only). Kroemer 2018 thesis: seen (catalogue page).
- Geometry: complete CAD (IGES and Parasolid) under CC BY 4.0. 9 blades, tip radius 247.5 mm, duct radius 250 mm (tip gap 2.5 mm), hub radius
  124 mm, zero skew, published as "NACA 4510" sections; the CAD sections (derived here, `validation-data/reference/w34d_fan01_cad_sections.csv`)
  have camber 0.0365-0.0376 at 0.52 chord and thickness 0.109-0.111 at 0.23 chord, chord 99 mm (hub) to 55 mm (tip), stagger 43.6 to 69.6 deg from
  the axis; the chords are 4-5 % shorter than the published design table (103 / 58 mm).
- Operating data: 1486 rpm (papers; not in the files), design 1.4 m^3/s, measured characteristic of 20 points from 0.20 to 2.08 m^3/s
  (`validation-data/reference/w34d_fan01_characteristic_n1ug.csv`): total-to-static pressure rise of the installation (inlet-chamber depression
  against ambient; ISO 5801 inlet chamber, bellmouth flow measurement) and shaft power (the file column says N m, the values are W). Design point
  1.4032 m^3/s, 126.46 Pa, 333.30 W, efficiency 0.532. Stall onset about 1.1 m^3/s. No density, no ambient state, no FAU uncertainty statement in
  anything read; the round robin quotes +-3 Pa and 1.5 % flow and agrees with the FAU curve within about 3 %. LDA profiles 20 mm and 90 (or 110) mm from
  a datum that the sources read do not settle.
- Representable: the rotor, hub cylinder and duct are; the bellmouth, the finite blunt hub, the 3 mm root fillet, struts, motor and diffuser are not;
  the measured pressure rise includes them. Status after ADR 0075: see the addendum there (the registered attempt was NOT RUNNABLE). Diagnosis of
  the attempt-3 miss: ADR 0078 (mainly numerical, spurious swirl of the upwind relative-frame discretisation; Delta p_ts is not a rotor-only quantity).

**2. NASA Low-Speed Axial Compressor (LSAC)**: Wasserbauer, Weaver, Senyitko, NASA TM-4635 (1995), NTRS 19950016775, READ (summary, facility,
Table I: 61 m/s tip speed, 980 rpm, 39 rotor blades, tip radius 61.0 cm, hub 48.8 cm, tip clearance 0.17 cm, pressure ratio 1.042). No blade
coordinates, no profile family and no performance map in the TM; a "publicly available" geometry package is mentioned elsewhere (seen) and was not
found. Four stages with IGV and stators. Not runnable.

**3. DTMB 4119 propeller**: READ only through arXiv:2104.13363 (Tables 1-3: 3 blades, D = 0.305 m, NACA 66 (mod) thickness, a = 0.8 mean line (printed
"a = 0.08"), radial distributions of chord, pitch, thickness and camber; K_T, K_Q at J = 0.833 from Jessup 1984/1989 as quoted there). The DTNSRDC
reports were not read. Open propeller, no shroud, section family not in the generator. Not runnable as is.

**4. NASA Rotor 37** (dataset `nasa-rotor37`, NASA TP-1337, READ): design 17 188.7 rpm, tip speed 454 m/s, 36 blades, rotor pressure ratio 2.106,
inlet relative Mach 1.48 (tip) to 1.13 (hub), multiple-circular-arc sections, blade manufacturing coordinates in Appendix C (12 sections), overall
performance Table V (100 % speed peak efficiency 0.876 at 20.74 kg/s, rotor pressure ratio 2.056); no tip clearance in the report. Transonic: not
runnable.

**5. NASA Rotor 67**: Strazisar et al., NASA TP-2879 (1989), design text pp. 1-12 READ: 22 blades, 16 043 rpm, tip relative Mach 1.38, pressure ratio
1.63, coordinates on surfaces of revolution (Table I), contoured hub. Transonic: not runnable.

**Not fans / not read**: Daily and Nece (1960) enclosed rotating disk (ASME, paywalled; only secondary snippets seen, coefficients UNVERIFIED);
ERCOFTAC classic collection rotating cases (list read; no rotor-stator cavity case found in it); Dring et al. UTRC turbine rig (snippets only).

What a fan validation with these tools needs (from the FAN-01 attempt, ADR 0075): blade input with more than two stations (added in wave 34D,
`mesh.fan.station_*`); a passage mesh whose periodic seams follow the blade at high hub solidity (the C-grid wake line leaves the straight-seam
sector at the FAN-01 hub); a turbulence wall treatment that accepts moving walls (a stationary casing is a moving wall in the rotating frame; fixed
for k-omega SST in wave 34D); an inlet nozzle / finite hub / diffuser model or a rig-independent measured quantity to compare with.

## Small-fan sources (computer-fan scale)

Searched and read on 2026-10-26 (wave 46E, part G1 of `docs/fan-project-breakdown.md`). Target: computer fans, axial or centrifugal,
40 to 140 mm, 1 000 to 5 000 rpm, tip speed 5 to 25 m/s, chord Reynolds number 2e4 to 2e5, large tip gap, high-impedance system. Labels as
above: READ (opened and the relevant content read), seen (citation, record page or search snippet only), NOT read, UNVERIFIED (a value
not checked against a primary text). Numbers below are quoted from the text of the sources or derived here (said so); values that exist
only in figures were not read off the plots. No data tables are copied into the repository; nothing was added to `manifest.json`.
Reynolds numbers derived here use nu = 1.5e-5 m^2/s. The generator these sources are judged against is `rcfd_mesh::fan` as of main
3175951: one NACA 4-digit or double-circular-arc family per rotor, sections on cylinders given at radial stations (chord, stagger, camber,
thickness, sweep, lean), cylindrical hub, cylindrical shroud with a tip gap, no stator row, no import of measured section coordinates.

Ranking criteria (written after reading, not before): (1) regime match (size, tip speed, chord Reynolds number, tip gap); (2) complete
open geometry; (3) fan curve, noise and vibration from the same fan; (4) licence; (5) representable by `rcfd_mesh::fan`.

**Verdict: no open source is adequate as the first computer-fan reference case.** The only source with geometry, fan curve, noise and
vibration from one fan on public terms is the NASA Quiet Space Fan (rank 1), and it is a ducted rotor-stator ventilation fan at about
50 m/s tip speed with a 0.23 mm tip gap (about 0.9 % of chord, against 2.9 % for the NASA 80 mm cooling fan of rank 2), outside the
computer-fan regime on tip speed, configuration and tip leakage. The only source inside
the regime (rank 2) publishes no geometry. G2 therefore needs a fan measured by the user, following the
[fan measurement protocol](fan-measurement-protocol.md). Rank 1 is usable to exercise the small-fan tool chain (geometry import, curve, tones,
imbalance) against real data, stated as not computer-fan validation.

**1. NASA Quiet Space Fan (QSF), spacecraft cabin ventilation fan** (rank 1; best available, regime mismatch).
- Sources. Stephens, Goodman, Buehrle, Mirhashemi, Koch, Shook, Sutliff, Allen, Matty, "Highlights of Aeroacoustic Tests of a Metal
  Spacecraft Cabin Ventilation Fan Prototype", NASA/TM-20220012622 (Nov 2022), https://ntrs.nasa.gov/citations/20220012622: READ (text;
  figures not read numerically), with supplements SUPPL1 (assembly drawing, READ), SUPPL2 (blade and endwall coordinates, READ and parsed
  here), SUPPL4 (16 dimensioned part drawings, listed, not read), SUPPL3 (CREO solid model, 90 MB, NOT downloaded). Koch et al.,
  "Spacecraft Cabin Ventilation Fan Research at NASA", NTRS 20240004675 (2024): READ. Stephens and Koch, "Quiet Spacecraft Cabin
  Ventilation Fan: Aerodynamic Measurements Results", NOISE-CON 2023, NTRS 20230003206: READ. Stephens, Goodman, Shook, Koch, "...
  Vibration Measurements Results", NOISE-CON 2023, NTRS 20230003228: READ (text). Sutliff, "... Acoustic Measurements Results",
  NOISE-CON 2023 presentation, NTRS 20230006061: READ (slide text). Tweedt, "Aerodynamic Design and Computational Analysis of a Spacecraft
  Cabin Ventilation Fan", NASA/CR-2010-216329, NTRS 20110002741: READ (design sections). Seen only: TM-20230001811 (test set-up, downloaded,
  not read beyond the abstract), Koch et al. TM-2012-217692 (plastic prototype acoustics), Mirhashemi NOISE-CON 2023 (wake hot wire),
  Butler et al. 2023 (stall simulations).
- Licence: NTRS distribution PUBLIC, copyright determination GOV_PUBLIC_USE_PERMITTED, no third-party material flagged (same record type
  as `nasa-rotor37`). Geometry and reports are downloadable; no measured data file is published (the aerodynamic paper reports about
  150 GB acquired; curves are figures).
- Fan: axial rotor-stator stage, 9 rotor blades, 11 stator vanes, flow-path diameter 88.9 mm (3.50 in), rotor tip clearance 0.23 mm
  (0.009 in), design 12 000 rpm, rotor tip speed 50.2 m/s (inlet) and 52.7 m/s (exit) (Tweedt; that is a rotor tip radius of 40.0 mm at
the leading edge and 42.0 mm at the trailing edge, a rotor of about 80 to 84 mm in an 88.9 mm duct), rotor chord Reynolds number about
  75 000 and stator about 60 000 (Tweedt), specific speed in the mixed-flow range with a 30 deg hub slope at the rotor inlet and a 22 %
  annulus area contraction through the rotor (Tweedt). Spinner, 3 aft struts, long exit diffuser, inlet bellmouth on the rig.
- Geometry (SUPPL2, derived here; coordinates in inches, 91 points LE to TE by 21 sections per surface, pressure and suction side paired by
  point index, unwrapped at each section's mean radius, so the values are approximate): rotor chord 23.7 mm at the hub to 28.1 mm at the tip
  (hub 16 % shorter; Tweedt states 15 %), stagger from the axis about 39 deg (hub) to 54 deg (tip), maximum thickness about 0.11 to 0.08 of
  chord (Tweedt: 10 % to 8 %), camber about 0.12 to 0.04 of chord (the surfaces extend 0.005 in beyond each endwall, README); stator
chord 29.1 mm constant (Tweedt
  1.15 in), thickness 0.10. Section definition (Tweedt): polynomial camber-angle and thickness distributions joined at 35 % chord, laid out
  on conic surfaces; the rotor camber-angle distribution is double circular arc, the thickness is not.
- Measured (text values): 3.48 in. water (867 Pa) at 150.6 cfm at 12 000 rpm, design goal 3.64 in. water (907 Pa) at 150 cfm; wide open
  0.52 in. water (130 Pa) at 220.4 cfm; peak 4.0 in. water (996 Pa) at 120.7 cfm; maps at 8 000, 10 000, 12 000 rpm (figures). Pressure
  rise between static taps on each side of the fan; flow from an ISO 5801 bellmouth wall tap, cross-checked against a pitot-static probe;
  transducer ranges 0-1 and 0-15 in. water; no uncertainty statement in anything read. Efficiency (figure) is isentropic power over
  electrical input to the controller (motor and controller included; "mechanical work" assumes 90 % motor efficiency): 75 % at design;
  design-point current 1.73 A at 48 V. AMCA International repeated the curve (AMCA 210-16) and sound power (AMCA 300-14, reverberant room)
  at 8 000 to 12 000 rpm: 71 dBA at design point. In-duct 72-microphone array: tones at 1 BPF (1 800 Hz) and 4 BPF (7 200 Hz), a
  non-blade-rate tone at the m = 1 cut-on frequency 2 270 Hz; in-duct PWL labels 64.2 dB (inlet, run 031) and 61.7 dB (exhaust, run 069)
  against 73.4 dB for a commercial fan of similar size. Vibration: rotor assembly 191 g, residual unbalance tolerance 0.762 g mm (grade
  G 5 at 1 257 rad/s), measured peak casing velocity just under 4 mm/s at shaft rate (2021 rotor), about 1 mm/s after the 2022 redesign
  (2023, unthrottled); first blade bending mode 7 430 Hz, lowest stator mode about 2 800 Hz.
- Representable: not as it stands (conic sections, contoured hub and shroud, stator row, polynomial thickness). A NACA 4-digit or
  circular-arc fit on cylinders would be an approximation whose geometric error must be reported. The coordinates can be used directly by a
  future coordinate-import path. The vibration paper states the inputs of the E1 imbalance force (2021 rotor assembly mass, unbalance
  tolerance, speed); comparing that force with the measured casing velocity needs the mount dynamics (E2), and NASA attribute the 2021
  vibration to assembly repeatability (bushing, motor centring) as well as balance.

**2. NASA 80 mm electronics cooling fan** (rank 2; inside the regime, no geometry).
- Sources. Van Zante, Koch, Wernet, Podboy, NASA/TM-2006-214448 (Part 1: aerodynamic performance), NTRS 20060048234: READ (text; figures
  not read numerically). Koch, Van Zante, Wernet, Podboy, NASA/TM-2006-214450 (Part 2: source identification), NTRS 20060045853: READ.
  Licence: NTRS GOV_PUBLIC_USE_PERMITTED. No data files.
- Fan: commercial spaceflight-qualified 80 mm axial fan (maker not named), 5 blades, 4 downstream struts, 80 x 80 x 32 mm case, duct
  diameter 77 mm, tip radius 37 mm, casing radius 38 mm, tip clearance 1 mm, chord 34 mm (clearance 2.9 % of chord), solidity 0.86,
  3 300 rpm, tip speed 12.8 m/s, BPF about 275 Hz; chord Reynolds number about 3e4 at the tip (derived). Hub: Part 1 table gives radius
  26 mm (hub/tip 0.70), Part 2 table gives diameter 42 mm; the two disagree.
- Measured: fan curve and A-weighted sound power at 3 300 rpm per ISO 10302 on an automated plenum with a 19-microphone array (ISO 3744),
  with and without a 1-diameter inlet duct (figures; text: stall plateau near 42 m^3/h, useful range 40 to 50 m^3/h, free delivery
  50 m^3/h); third-octave and narrowband spectra at free delivery (figures); 2D/3D PIV of the inlet (free delivery, mild stall) and hot-wire
  rotor wakes (free delivery). No uncertainty statement.
- Representable: no; blade geometry not published (sections, stagger and twist unknown). Usable only as a plausibility check of the
  regime (tip-gap blockage, inlet distortion from the square frame, stall plateau), not as a validation case.

**3. OpenCeFaDB, KIT generic centrifugal fan** (rank 3; open CAD and curves, centrifugal, no noise, larger than the target).
- Sources. Zenodo records by Probst and Pritz (KIT), all CC BY 4.0: "Generic Centrifugal Fan CAD File", doi:10.5281/zenodo.17871736
  (2025-12-09; `metadata.ttl` READ, `cefa_asm_v1.igs` 14.3 MB NOT opened); fan-curve datasets at 600, 800, 1 000, 1 200 rpm (Dataset 1 and 2,
  e.g. doi:10.5281/zenodo.18349134, 28 runs of HDF5 + RDF each; one run's RDF metadata READ, HDF5 values NOT read); catalog
  doi:10.5281/zenodo.18349358 and Python package (seen). Probst, Pritz, Bauer, "Towards a FAIR centrifugal fan database for off-design
  validation of CFD simulations", FAN 2022, doi:10.26083/tuprints-00021695 (CC BY 4.0): READ. Pritz, Walter, Gabi, FAN 2018 (rig and design
  point): NOT read.
- Fan (CAD metadata): backward-curved, 9 blades, D1 138 mm, D2 306 mm, D3 325 mm, b1 = b2 58.5 mm, blade angles 30.2 deg (inlet) and
  40.7 deg (outlet), spiral casing; design 25 1/s (1 500 rpm), flow coefficient 0.071, pressure coefficient 0.958, total efficiency 0.85
  (design values). Tip speed 9.6 to 19.2 m/s over 600 to 1 200 rpm (derived), inside the target range although D2 is not.
- Measured quantities per run (metadata): volume flow rate (orifice per ISO 5167), static and total pressure differences across the fan,
  fan torque, rotational speed, inlet/outlet/ambient temperature, humidity, atmospheric pressure, air density, each with a recorded
  standard deviation; PIV planned in the rotor and volute (FAN 2022 paper). Instrument uncertainty not stated in what was read. No noise.
- Representable: no (A3, the centrifugal generator, is missing). The best open candidate for a centrifugal reference once A3 exists:
  torque gives a true shaft-power route that the axial sources lack.

**4. Tutar and Cam, small axial fan with NACA 4-digit blades** (rank 4; representable table, experiment not read).
- Source. M. Tutar, J. B. Cam, "Computational Design of an Energy-Efficient Small Axial-Flow Fan Using Staggered Blades with Winglets",
  Int. J. Turbomach. Propuls. Power 10 (2025) 1, doi:10.3390/ijtpp10010001, CC BY 4.0: READ. Data availability: on request. The experiment
  is Lin and Tsai, "An integrated study of the design method for small axial-flow fans, based on the airfoil theory", Proc. IMechE Part C
  225(4) (2011) 885-895, doi:10.1243/09544062JMES2231: abstract READ (Crossref), full text NOT read; its data are UNVERIFIED.
- Fan: 7 blades, rotor diameter 120 mm, hub 50 mm, 2 000 rpm (tip speed 12.6 m/s, derived), NACA 4420 at the hub to 4406 at the tip,
  chord 18.6 to 41.6 mm, setting angle 39.9 to 36.0 deg (Table 1); chord Reynolds number about 7e3 (hub) to 3.6e4 (tip) at 60 cfm
  (derived). CFD (RNG k-epsilon, MRF) against Lin and Tsai's static pressure-flow curve at 2 000 rpm (figure; best efficiency near 60 cfm).
- Representable: yes in form (NACA 4-digit on stations), but the table is not self-consistent (11 radii, 10 segment numbers; only 3.9 deg
  setting-angle change over a radius ratio of 2.5, reference of the setting angle not defined), the computational domain has no shroud or
  tip gap, and the paper cites Lin and Tsai as "Int. J. Adv. Eng. Innov. Technol. 2010" while Crossref gives Proc. IMechE C 2011. Not usable
  until the original geometry and curve are read.

**5. FAN-02, enclosed centrifugal fan (FAU/TU Graz)** (rank 5; rich noise and vibration, not small, impeller data not yet in the record).
- Sources. Zenodo "FAN-02 Benchmark: Enclosed Centrifugal Fan with Duct System", doi:10.5281/zenodo.17909944 (2025-12-12, CC BY 4.0):
  record READ; its 11 files are housing STEP models and sensor positions only. Czwielong et al., "An Overview of the Benchmark Case for the
  Aeroacoustics and Structural Acoustics of an Enclosed Centrifugal Fan", Int. J. Turbomach. Propuls. Power 11 (2026) 10,
  doi:10.3390/ijtpp11010010: READ; licence CC BY-NC-ND (not CC BY).
- Fan: 12 backward-curved blades, outer diameter 400 mm, constant 5 mm thickness, 28 mm channel width, 1 800 rpm, tip speed 37.7 m/s,
  design 0.28 m^3/s and 700 Pa, BPF 360 Hz. Described data: inflow hot wire, PIV at the tongue, fan curve (ISO 5801 nozzle) with torque at
  three points, blade-surface pressure telemetry, housing modal and operational laser vibrometry, sound intensity, 8 free-field
  microphones. Impeller geometry and measured data were not in the record version read.

**Not usable for validation (read).**
- Dozolme, Metwally, Marchal, "Electronics Cooling Fan Noise Prediction", THERMINIC 2006, arXiv:0709.1876 (arXiv default licence): READ.
  No geometry, fan not named; "experimental" values are catalogue data (OMRON pages, NOT read); compares a computed surface acoustic power
  (Proudman model) with catalogue levels, which are not the same quantity.
- matutani/cooling-fan (GitHub; Sunaga, Kondo, Matsutani, IEEE Micro 43(6) 2023, NOT read): README READ. Vibration spectra 1-512 Hz of
  8, 12 and 14 cm fans at three speeds, normal and deliberately unbalanced; no licence file (cite only), no sensor, units or calibration
  stated.
- Not small fans: Mo and Choi, Appl. Sci. 10 (2020) 5432 (automotive fan, radii given in mm in the text and cm in Table 1; about 360 mm
  diameter; licence not checked); Zhang et al., Appl. Sci. 16 (2026) 2227 (train fan, 665 mm, CFD only); Assaf, Gras, Ferhat, Int. J.
  Turbomach. Propuls. Power 11 (2026) 17 (excavator fan, CC BY-NC-ND); Szelka et al., Energies 16 (2023) 542 (mining fan, hub 0.48 m).
- Open-licence computer-fan geometry: one search (Printables, Thingiverse, MakerWorld, GitHub) found parametric PC-fan grilles and a
  propeller generator but no CC BY 80-140 mm rotor; geometry without measured data cannot validate in any case.

**Seen only, NOT read (UNVERIFIED):** L. Huang, "Characterizing computer cooling fan noise", J. Acoust. Soc. Am. 114 (2003) 3189,
doi:10.1121/1.1624074; Quin and Grimes, Reynolds-number effect on micro axial fans, J. Fluids Eng. 130 (2008); Zhou and Yang, axial fan for
electronic devices, IEEE Trans. Compon. Packag. Technol. 33 (2010) 287; Trinity College Dublin (Persoons group) electronics-cooling fan
aeroacoustics records (repository pages returned 403); Fitzgerald and Lauchle, J. Acoust. Soc. Am. 76 (1984) 158.

**Measurement standards (procedure reference only; no text copied).**
- Fan performance: ISO 5801 (2017 edition cited by NASA and KIT) and ANSI/AMCA 210-16 = ANSI/ASHRAE 51-16 (used by AMCA for the QSF):
  NOT read. Open descriptions READ: the QSF rig (ISO 5801 bellmouth, wall tap about one inch downstream of the bellmouth curvature,
  throttle cone at the duct exit, static taps either side of the fan) and the KIT rig (ISO 5167 orifice for flow, pressures upstream and
  downstream of the fan).
- Small-fan noise: ISO 10302-1:2024 (second edition; airborne noise of small air-moving devices as a function of flow rate and static
  pressure on a plenum, including a limit on the plenum's flow) seen via the ISO catalogue summary, NOT read; ISO 10302-2 (structure-borne
  vibration) title only. ECMA-74, 22nd edition (Dec 2025), free download: READ in part; it references ISO 10302-1/-2, gives the prominent
  discrete tone method (tone-to-noise ratio, prominence ratio, Annex D) and states a reproducibility standard deviation of about 1.5 dB for
  A-weighted sound power in reverberation rooms and in free field over a reflecting plane. In-duct ISO 5136, ANSI/ASHRAE 68 and
  reverberant-room AMCA 300-14: cited by NASA, NOT read.
- Balance: ISO 21940-11 balance grade G = e omega (as used in the QSF vibration paper; the relation itself is first principles).

Inconsistencies found while reading: QSF design flow 0.0709 m^3/s in the report bodies but 0.709 m^3/s in the abstracts of NTRS 20240005078
and 20120014314; Koch et al. 2024 states the measured pressure rise as "3.478 psia" and "3.487 psia" where inches of water are meant; QSF
in-duct PWL values are figure labels; the QSF vibration paper's residual unbalance for fan 2 at the front plane (0.0014 oz-in, about
1.0 g mm) exceeds the stated 0.762 g mm tolerance; NASA 80 mm fan hub size differs between Parts 1 and 2; Tutar and Cam Table 1 numbering and citation
as above; Mo and Choi mm/cm; the KIT FAN 2022 paper planned a first release for mid-2022, the Zenodo data records are dated January 2026.

## Provenance table (generated by `rcfd data provenance`)

## Lid-driven cavity centreline velocities (Ghia, Ghia and Shin 1982) (`cavity-ghia1982`)

- status: available
- validates case: `cavity`
- citation: U. Ghia, K. N. Ghia, C. T. Shin, J. Comput. Phys. 48 (1982) 387-411, doi:10.1016/0021-9991(82)90058-4
- licence: LicenseRef-published-numeric-data (Numerical values from a published paper, transcribed; no copyright claimed on the numbers), https://doi.org/10.1016/0021-9991(82)90058-4
- attribution: Ghia, Ghia and Shin (1982)
- licence notes: In-repo tables (cases/cavity/reference) cross-checked against secondary copies, see docs/reference-data.md. No upstream download: the paper's tables are not distributed as a file.
  - `cases/cavity/reference/ghia1982_u_centerline.csv` sha256 `2c815e4837de8802b90c075eede15592cb0f4c6c7911734271d722a000a9e175` (541 bytes), in this repository
  - `cases/cavity/reference/ghia1982_v_centerline_Re100.csv` sha256 `0d2e9c29d25bf8de06442f47c4f9b11475bcb08a1b81e2cd9db40d941ca82408` (254 bytes), in this repository

## Differentially heated square cavity (de Vahl Davis 1983) (`heated-cavity-devahldavis1983`)

- status: available
- validates case: `heated_cavity`
- citation: G. de Vahl Davis, Int. J. Numer. Meth. Fluids 3 (1983) 249-264, doi:10.1002/fld.1650030305
- licence: LicenseRef-published-numeric-data (Numerical values from a published paper, transcribed; no copyright claimed on the numbers), https://doi.org/10.1002/fld.1650030305
- attribution: de Vahl Davis (1983)
- licence notes: In-repo table (cases/heated_cavity/reference), verified against secondary copies in docs/reference-data.md.
  - `cases/heated_cavity/reference/devahldavis1983.csv` sha256 `b84b03aa48c3a963f0bc4211a6052b06861fbd587d3bf7d15ff9eacf79fe5a81` (161 bytes), in this repository

## Zero-pressure-gradient flat plate: validation data, CFL3D SST result, coarse grid (NASA TMR 2DZP) (`flat-plate-tmr`)

- status: available
- validates case: `flat_plate`
- citation: NASA Turbulence Modeling Resource, 2D Zero Pressure Gradient Flat Plate Verification and Validation, https://tmbwg.github.io/turbmodels/flatplate.html
- licence: CC0-1.0 (Creative Commons CC0 1.0 (repository LICENSE of TMBWG/turbmodels)), https://github.com/TMBWG/turbmodels/blob/main/LICENSE
- attribution: NASA Turbulence Modeling Resource (TMBWG), https://tmbwg.github.io/turbmodels/
- licence notes: Experimental data inside the files originate from the cited papers; the TMR page states they are public, provided as is.
  - `cf_K-S.dat` sha256 `bafbaf7fa7969a945a5ad0935ca90c320ab72f14ba5a08474abffa61356bba2d` (4224 bytes), https://raw.githubusercontent.com/TMBWG/turbmodels/20a39f549dbff988a6accc421ac84dafc4487695/FlatPlate_validation/cf_K-S.dat
  - `cf_as_function_of_x.dat` sha256 `97d01033550468819a0f081fee37fc1836ff12a4ad8318f0a80cdc847625a8e2` (183312 bytes), https://raw.githubusercontent.com/TMBWG/turbmodels/20a39f549dbff988a6accc421ac84dafc4487695/FlatPlate_validation/cf_as_function_of_x.dat
  - `u+y+.dat` sha256 `314dc0b5e4aa666ee88ac507eeda8cbeaacc1279057d10430748231264277bcb` (137104 bytes), https://raw.githubusercontent.com/TMBWG/turbmodels/20a39f549dbff988a6accc421ac84dafc4487695/FlatPlate_validation/u+y+.dat
  - `cf_plate_sstv.dat` sha256 `a56bd2d16818456af015b13839f0a991b4b85bfa6c18242e7c331a2c7a4d40e8` (32816 bytes), https://raw.githubusercontent.com/TMBWG/turbmodels/20a39f549dbff988a6accc421ac84dafc4487695/FlatPlate/SST/cf_plate_sstv.dat
  - `flatplate_clust2_4levelsdown_35x25.p2dfmt` sha256 `41c409f49afca8affaafd4a0a49c6ba934eca9007da502fe0c20afbd091500b0` (42622 bytes), https://raw.githubusercontent.com/TMBWG/turbmodels/20a39f549dbff988a6accc421ac84dafc4487695/FlatPlate/Grids/flatplate_clust2_4levelsdown_35x25.p2dfmt

## Backward-facing step: Driver and Seegmiller (1985) experiment (NASA TMR 2DBFS) (`backward-step-tmr`)

- status: available
- validates case: `backward_step`
- citation: D. M. Driver, H. L. Seegmiller, AIAA J. 23 (1985) 163-171, doi:10.2514/3.8890; data via NASA TMR https://tmbwg.github.io/turbmodels/backstep_val.html
- licence: CC0-1.0 (Creative Commons CC0 1.0 (repository LICENSE of TMBWG/turbmodels)), https://github.com/TMBWG/turbmodels/blob/main/LICENSE
- attribution: NASA Turbulence Modeling Resource (TMBWG), https://tmbwg.github.io/turbmodels/
- licence notes: Experimental data inside the files originate from the cited papers; the TMR page states they are public, provided as is.
  - `cf.exp.dat` sha256 `cd9b6434b7956bc58e859de6af654cd7f891e25303babf1dd8633d6c58f74276` (592 bytes), https://raw.githubusercontent.com/TMBWG/turbmodels/20a39f549dbff988a6accc421ac84dafc4487695/Backstep_validation/cf.exp.dat
  - `cp.expnew.dat` sha256 `2b3ec9f8d8025b958d6ddcf6767daaf3c93083d45ef350a8cf9c4b46f93a882a` (2087 bytes), https://raw.githubusercontent.com/TMBWG/turbmodels/20a39f549dbff988a6accc421ac84dafc4487695/Backstep_validation/cp.expnew.dat
  - `profiles.exp.dat` sha256 `23e8b44db52bfc36c66af6b743a4080c34f0165eb213a3db29c0914e12cf598d` (10341 bytes), https://raw.githubusercontent.com/TMBWG/turbmodels/20a39f549dbff988a6accc421ac84dafc4487695/Backstep_validation/profiles.exp.dat

## NACA 0012 airfoil: experimental Cl/Cd/Cp (Abbott, Ladson, Gregory, McCroskey) and surface points (NASA TMR) (`naca0012-tmr`)

- status: available
- validates case: `naca0012`
- citation: NASA Turbulence Modeling Resource, 2D NACA 0012 Airfoil Validation, https://tmbwg.github.io/turbmodels/naca0012_val.html
- licence: CC0-1.0 (Creative Commons CC0 1.0 (repository LICENSE of TMBWG/turbmodels)), https://github.com/TMBWG/turbmodels/blob/main/LICENSE
- attribution: NASA Turbulence Modeling Resource (TMBWG), https://tmbwg.github.io/turbmodels/
- licence notes: Experimental data inside the files originate from the cited papers; the TMR page states they are public, provided as is.
  - `n0012points_superbig_clust_fix.dat` sha256 `74e147db4fa052be7ab79465a756c55b39b1786bfd7141a86257e89586728ae1` (824246 bytes), https://raw.githubusercontent.com/TMBWG/turbmodels/20a39f549dbff988a6accc421ac84dafc4487695/NACA0012_grids/n0012points_superbig_clust_fix.dat
  - `0012.abbottdata.cd.dat` sha256 `21321fe22883672c344f3cd1369c69cdaf045ed8496cd0a976b741645c79e3d9` (569 bytes), https://raw.githubusercontent.com/TMBWG/turbmodels/20a39f549dbff988a6accc421ac84dafc4487695/NACA0012_validation/0012.abbottdata.cd.dat
  - `0012.abbottdata.cl.dat` sha256 `3ee6a0c198f6402c9fc5fd8affc1115c1e87ff943b8b02775df86a7cfa3c55f3` (696 bytes), https://raw.githubusercontent.com/TMBWG/turbmodels/20a39f549dbff988a6accc421ac84dafc4487695/NACA0012_validation/0012.abbottdata.cl.dat
  - `0012.mccroskeydata.cl.dat` sha256 `bf5848c235cedffef4e307f5cb52be1e7146bd5e480c202824a59d6d9754924e` (250 bytes), https://raw.githubusercontent.com/TMBWG/turbmodels/20a39f549dbff988a6accc421ac84dafc4487695/NACA0012_validation/0012.mccroskeydata.cl.dat
  - `CLCD_Ladson_expdata.dat` sha256 `78cd2f6aa4968e80f44cbf6c96f699bd9c6e45681d958ec528a10cf72ed23357` (1343 bytes), https://raw.githubusercontent.com/TMBWG/turbmodels/20a39f549dbff988a6accc421ac84dafc4487695/NACA0012_validation/CLCD_Ladson_expdata.dat
  - `CL_Gregory_expdata.dat` sha256 `0b7381a45412de50fcf0e0dddf67f1e8e35ba1a278bd4734fb7f16ad08df1f74` (308 bytes), https://raw.githubusercontent.com/TMBWG/turbmodels/20a39f549dbff988a6accc421ac84dafc4487695/NACA0012_validation/CL_Gregory_expdata.dat
  - `CP_Gregory_expdata.dat` sha256 `38d8474bc67661c3cef5e4cd9eeddb95d4a436d06abc1f58a8518066f510647c` (1608 bytes), https://raw.githubusercontent.com/TMBWG/turbmodels/20a39f549dbff988a6accc421ac84dafc4487695/NACA0012_validation/CP_Gregory_expdata.dat
  - `CP_Ladson.dat` sha256 `547e022634a360cd7697813543348cd447ee0038cdbc8f00a5296c7822ec7af8` (6331 bytes), https://raw.githubusercontent.com/TMBWG/turbmodels/20a39f549dbff988a6accc421ac84dafc4487695/NACA0012_validation/CP_Ladson.dat

## damBreak laminar two-phase set-up (OpenFOAM-11 tutorial files, geometry and initial conditions) (`dambreak-openfoam`)

- status: available
- citation: OpenFOAM Foundation, OpenFOAM-11, tutorials/incompressibleVoF/damBreakLaminar (set-up after Koshizuka and Oka 1996 / Martin and Moyce 1952 style dam break)
- licence: GPL-3.0-only (GNU General Public License v3 (COPYING of OpenFOAM-11)), https://github.com/OpenFOAM/OpenFOAM-11/blob/master/COPYING
- attribution: OpenFOAM Foundation
- licence notes: Fetched on the user's machine only and never vendored into this repository. Use the files as set-up data (geometry, dimensions, initial water column); no OpenFOAM code is linked.
  - `blockMeshDict` sha256 `d2a5a377a20df7fe5bdb888088c2927990077febd7741e546a4fdcbb89100a3c` (2069 bytes), https://raw.githubusercontent.com/OpenFOAM/OpenFOAM-11/9cbf94f3339dd49286633b303c4155fd10ea8200/tutorials/incompressibleVoF/damBreakLaminar/damBreak/system/blockMeshDict
  - `setFieldsDict` sha256 `c9813a4cbf891088709ea0c5379e8455e35d0fa62af760e6fc3d8c53db788ca1` (904 bytes), https://raw.githubusercontent.com/OpenFOAM/OpenFOAM-11/9cbf94f3339dd49286633b303c4155fd10ea8200/tutorials/incompressibleVoF/damBreakLaminar/damBreak/system/setFieldsDict
  - `controlDict` sha256 `fc946771fd8c25bfe3de13bd8aab8102cc847c6fad7dbb7689313d12f4f7a0ad` (1151 bytes), https://raw.githubusercontent.com/OpenFOAM/OpenFOAM-11/9cbf94f3339dd49286633b303c4155fd10ea8200/tutorials/incompressibleVoF/damBreakLaminar/damBreak/system/controlDict
  - `g` sha256 `d6377d97d8c0631d625ed54486e9abb142eda889f66f309e2fc74a1521a9a56b` (748 bytes), https://raw.githubusercontent.com/OpenFOAM/OpenFOAM-11/9cbf94f3339dd49286633b303c4155fd10ea8200/tutorials/incompressibleVoF/damBreakLaminar/damBreak/constant/g
  - `physicalProperties.water` sha256 `de9ad8fa8ef86531c963bd771db62b14495a95debced2484f8c8b4f072e5bb33` (762 bytes), https://raw.githubusercontent.com/OpenFOAM/OpenFOAM-11/9cbf94f3339dd49286633b303c4155fd10ea8200/tutorials/incompressibleVoF/damBreakLaminar/damBreak/constant/physicalProperties.water
  - `physicalProperties.air` sha256 `25f849fbff0b2d861318a65a60d714805d3a134ca62ac0a50dc45faae87a9769` (760 bytes), https://raw.githubusercontent.com/OpenFOAM/OpenFOAM-11/9cbf94f3339dd49286633b303c4155fd10ea8200/tutorials/incompressibleVoF/damBreakLaminar/damBreak/constant/physicalProperties.air

## Fluent TGrid 2D elbow mesh (ASCII .msh, triangles, 918 cells) from the OpenFOAM-11 icoFoam elbow tutorial (`fluent-elbow-openfoam`)

- status: available
- citation: OpenFOAM Foundation, OpenFOAM-11, tutorials/legacy/incompressible/icoFoam/elbow/elbow.msh (a mesh written by Fluent TGrid 2.4.1 / PreBFC 4.3)
- licence: GPL-3.0-only (GNU General Public License v3 (COPYING of OpenFOAM-11)), https://github.com/OpenFOAM/OpenFOAM-11/blob/master/COPYING
- attribution: OpenFOAM Foundation
- licence notes: Fetched on the user's machine only and never vendored. Used as a real-world input for the Fluent .msh reader test, which skips when the file is absent.
  - `elbow.msh` sha256 `a7a3c88110d19f5534b5a225eb99102f1416d5113331c61ee47de1a416c299e3` (32114 bytes), https://raw.githubusercontent.com/OpenFOAM/OpenFOAM-11/9cbf94f3339dd49286633b303c4155fd10ea8200/tutorials/legacy/incompressible/icoFoam/elbow/elbow.msh

## GRI-Mech 3.0 methane combustion mechanism, thermo and transport data (`gri-mech-30`)

- status: available
- citation: G. P. Smith, D. M. Golden, M. Frenklach et al., GRI-Mech 3.0, http://combustion.berkeley.edu/gri-mech/version30/text30.html
- licence: LicenseRef-GRI-Mech-citation-requested (Distributed freely by the GRI-Mech project; the page asks to be cited and states no formal licence), http://combustion.berkeley.edu/gri-mech/version30/text30.html
- attribution: GRI-Mech 3.0 (Smith et al.)
- licence notes: UNVERIFIED legal status: no licence text on the page. The Cantera copy gri30.yaml sits in a BSD-3-Clause repository (Cantera License.txt) and is the preferred source.
  - `berkeley-grimech30.dat` sha256 `a7726208619d27b50e43b9fb6b27dc0a737003c049cc79e0a849bc75301cd763` (29134 bytes), http://combustion.berkeley.edu/gri-mech/version30/files30/grimech30.dat
  - `berkeley-thermo30.dat` sha256 `8d918b771aa965fe05e8c8f1f7e926a5326ebc90e7fc923ce129dc118b634dcc` (17346 bytes), http://combustion.berkeley.edu/gri-mech/version30/files30/thermo30.dat
  - `berkeley-transport.dat` sha256 `e2ef4437568311ad0ba6c2311a564966c9accb2b43ea3157b764c1e8febc5825` (8118 bytes), http://combustion.berkeley.edu/gri-mech/version30/files30/transport.dat
  - `gri30.yaml` sha256 `06650b1e0ee0012f6903d5328b1bb218cb6007d07f8ebe375d18f24811039345` (67813 bytes), https://raw.githubusercontent.com/cantera/cantera/v3.0.0/data/gri30.yaml
  - `cantera-License.txt` sha256 `2a51b2b912ef60b74559aa20b2b2799b4001c72c6e5a2c0887f3226f0dec1974` (1796 bytes), https://raw.githubusercontent.com/cantera/cantera/v3.0.0/License.txt

## NASA transonic compressor stage 37 (rotor 37): design and overall performance report (`nasa-rotor37`)

- status: available
- citation: L. Reid, R. D. Moore, NASA TP-1337 (1978), 'Design and overall performance of four highly loaded, high speed inlet stages for an advanced high-pressure-ratio core compressor', NTRS 19780025165
- licence: LicenseRef-NASA-public (NASA NTRS document, distribution PUBLIC, determination GOV_PUBLIC_USE_PERMITTED, no third-party material flagged), https://ntrs.nasa.gov/citations/19780025165
- attribution: NASA Lewis Research Center
- licence notes: Rotor 37 geometry and overall performance are in the stage 37 section. Detailed LDV data are in AGARD-AR-355 (not fetched).
  - `NASA-TP-1337.pdf` sha256 `5c2b9b2b1b701c9c5c14a167ad8bcea92af3c66fe452aaacfac4f691549c5157` (3813337 bytes), https://ntrs.nasa.gov/api/citations/19780025165/downloads/19780025165.pdf

## Leibniz University Hannover compressor cascade (`hannover-compressor`)

- status: unresolved
- citation: none found
- licence: NONE (unresolved), 
- unresolved: No public upstream with an open licence was found (search 2026-10-13: the only Hannover TFD compressor dataset located is CC-BY-NC and not open). Copies distributed with commercial software are not used. Add a file entry here once an upstream URL and licence are confirmed.

