---
type: source
tags: [validation, data, fan]
sources: []
origin: docs/validation-data.md
origin_commit: 09647baa
created: 2026-11-22
updated: 2026-11-23
status: draft
---

# Public validation-data pack (F-125)

**Origin:** [docs/validation-data.md](../../docs/validation-data.md) at origin/main 09647baa (first read at 2e94e5ef; re-read at 5b96a1f8)
**Kind:** repo doc: dataset catalogue, licence and checksum record, and two fan-data surveys (wave 34D, wave 46E).

## Summary

Describes the `validation-data/` pack of F-125: a `manifest.json`, a `fetch.py` script and the CLI commands `rcfd data list|verify|provenance`. Datasets are fetched from their public sources and checked by SHA-256; GPL material (the OpenFOAM damBreak files) is fetched only, never vendored, and CI makes no network access. The doc also holds the survey of rotating-machinery sources (wave 34D, ADR 0075) that chose FAN-01, the small-fan survey (wave 46E, 2026-10-26) for the computer-fan target, and the generated provenance table with one section per dataset.

## Key points

- Pack datasets (provenance table): `cavity-ghia1982`, `heated-cavity-devahldavis1983`, `flat-plate-tmr` (NASA TMR 2DZP, CC0, pinned commit), `backward-step-tmr` (Driver and Seegmiller 1985, NASA TMR 2DBFS), `naca0012-tmr` (Abbott, Ladson, Gregory, McCroskey data and surface points), `dambreak-openfoam` (GPL, fetched only), `fluent-elbow` (Fluent TGrid 2D elbow mesh, 918 cells), `gri-mech-30`, `nasa-rotor37` (report only), `hannover-compressor` (unresolved).
- Unresolved items: the Hannover compressor cascade source and the GRI-Mech licence.
- Rotating-machinery survey (wave 34D, ADR 0075), five candidates:
  1. FAN-01 (FAU/EAA), Zenodo 10.5281/zenodo.20037409 v5, CC BY 4.0: 9 blades, tip radius 247.5 mm, duct 250 mm (gap 2.5 mm), hub 124 mm, "NACA 4510" sections, chord 99 to 55 mm, stagger 43.6 to 69.6 deg; 1486 rpm, design 1.4 m^3/s, 20 measured points 0.20 to 2.08 m^3/s, stall about 1.1 m^3/s. Zenodo v6 (10.5281/zenodo.23943494, 2026-11-04) changes no value. Diagnosis in ADR 0078. ~~LDA profiles 20 mm and 90 (or 110) mm from a datum that the sources read do not settle~~ (2e94e5ef); at 5b96a1f8 the doc says the sources read on 2026-10-19 did not settle the datum and "the v6 description settles it".
     - Re-read on 2026-11-22 for F-878 (ADR 0765), added at 5b96a1f8: `lda_data.zip` of v6 (MD5 4cafb1602e81dc18f1681e6facd495a0, equal to the record's) and the description, READ. The LDA data are two radial lines at the design point, x = 20 mm and x = 90 mm from the upstream face of the hub (the errata of Appendix C list the "110 mm behind the hub" of the scripts of v2 to v5 as an error), 26 radii from 133 to 246 mm, three components, time-averaged and in 180 bins of the rotor angle. The 90 mm line is 2.2 mm behind the trailing edge at r = 133 mm and 27.2 mm at r = 246 mm. "No uncertainty of the LDA is stated." The profiles are transcribed in `docs/investigations/f878-fan01-lda-profiles/lda_profiles.csv` ([[doc-reference-data-f878-fan01-lda-profiles]]).
     - Re-read on 2026-11-22 for F-883 (ADR 0769), added at 09647baa: `mic_kulite.tar` of v6 (MD5 1002be297aa2ac2123f3928bf7f5d905, equal to the record's), `mic_kulite_n1ug_1_4.mat` READ. Its `kulite_pressure_in_Pa` holds 14 casing transducers at x = -23 to 107 mm "differential to the chamber pressure, mean value included", at the design point, "with no stated uncertainty". The time mean at the casing at x = 90 mm is 113.90 Pa against the chamber; over the measured span, by the radial equilibrium of the LDA profiles, 93.96 Pa +- 7.50 Pa. Transcribed in `docs/investigations/f883-fan01-wall-pressure/kulite_means.csv` ([[doc-reference-data-f883-fan01-wall-pressure]]).
  2. LSAC (not runnable), 3. DTMB 4119, 4. NASA Rotor 37 (17188.7 rpm, pressure ratio 2.106), 5. Rotor 67.
- Main findings stated at the top: "The registered FAN-01 attempt was NOT RUNNABLE, and ADR 0078 diagnoses the attempt-3 miss as mainly numerical"; the pinned TMR skin-friction file gives Cf(x = 0.97) of about 2.691e-3 (2.6909e-3 interpolated) while `tests/flat_plate_sst.rs` quotes 0.00268299; rcfd 0.002663 is -0.7 % against the page value and -1.0 % against the file; "The test and its 5 % tolerance are unchanged."
- Survey labels: READ, seen, NOT read, UNVERIFIED.
- Pinning cross-check (2026-10-13): the Rust SHA-256 (`rcfd-io::datapack`) agrees with Python's hashlib on all 32 listed files.
- Small-fan survey (wave 46E, searched and read on 2026-10-26 as part G1 of the breakdown): "Verdict: no open source is adequate as the first computer-fan reference case", so part G2 needs a fan measured by the user. Target scale 40 to 140 mm, 1000 to 5000 rpm, tip speed 5 to 25 m/s, chord Re 2e4 to 2e5. Candidates:
  1. NASA Quiet Space Fan: 9 rotor / 11 stator blades, 88.9 mm, tip gap 0.23 mm, 12000 rpm, tip speed 50.2 m/s; 867 Pa at 150.6 cfm; 71 dBA; unbalance 0.762 g mm; 75 % efficiency.
  2. NASA 80 mm cooling fan (TM-2006-214448 / 214450): 5 blades, 3300 rpm, tip 12.8 m/s, gap 1 mm (2.9 % chord), no geometry; a hub diameter disagreement of 26 mm vs 42 mm.
  3. KIT OpenCeFaDB centrifugal fan (D2 306 mm).
  4. Tutar and Cam 120 mm fan: 7 blades, 2000 rpm, NACA 4420 to 4406, with an inconsistent table.
  5. FAN-02 centrifugal, 400 mm.
- Standards named for fan testing: ISO 5801, AMCA 210, ISO 10302-1, ECMA-74 (1.5 dB).
- Recorded inconsistencies in the sources (for example QSF 0.0709 vs 0.709 m^3/s).

## Contradictions with the wiki

> [!warning] Contradiction
> This doc records the small-fan survey ("part G1 of `docs/fan-project-breakdown.md`") as searched and read on 2026-10-26 in wave 46E, while [[doc-fan-project-breakdown]] (plan dated 2026-10-26) still marks G1 "missing". Within the docs (the plan table predates or ignores the survey).

## Pages this source informs

- [[validation-data-pack]], [[fan-01]], [[computer-fan-target]]
- [[lid-driven-cavity]], [[differentially-heated-cavity]], [[flat-plate]], [[backward-facing-step]], [[naca0012]]
- [[provenance]], [[doc-reference-data]]
