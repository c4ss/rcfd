---
type: source
tags: [reference-data, fan-and-validation, fan-01, wall-pressure]
sources: []
origin: docs/reference-data.md
origin_commit: 09647baa
created: 2026-11-23
updated: 2026-11-23
status: draft
---

# Reference data: F-883 the FAN-01 wall pressure behind the rotor: the record's casing transducers, their time means, and the identity of ADR 0765

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-883 the FAN-01 wall pressure behind the rotor: the record's casing transducers, their time means, and the identity of ADR 0765" (lines 2509-2523) at origin/main 09647baa. The doc is generated from `docs/reference-data.d/*.part` (this section from `docs/reference-data.d/0769-F-883.part`).
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

The wall-pressure data of the FAN-01 record (Zenodo v6, doi:10.5281/zenodo.23943494, CC BY 4.0) as fetched and read for ADR 0769 on 2026-11-22: `mic_kulite.tar` with its checksums, what `kulite_pressure_in_Pa` holds (14 casing transducers at r = 250 mm, x = 10 n - 33 mm from the upstream hub face, 30.5 s at 48 kHz, "differential to the chamber pressure, mean value included"), no stated uncertainty, the time means of the rows behind the blade tips (92.82 to 117.30 Pa) and the static pressure at the LDA plane x = 90 mm: 113.90 Pa at the casing and $\Pi_m$ = 93.96 Pa +- 7.50 Pa over the measured span. The means are computed in the repository from the file, not published values. The use of the value, the rotor's share of the missing pressure rise, is on [[adr-thread-fan01-design-point]] (ADR 0769, F-883).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Relative links are rebased to the repo.

Recorded 2026-11-22 for ADR 0769. The file was fetched that day from zenodo.org and read; nothing here is from memory. The transcription is `docs/investigations/f883-fan01-wall-pressure/kulite_means.csv`, written by `kulite_means.py` beside it; the test `f883_transcription_constants` of `crates/rcfd-api/tests/f883_fan01_wall_pressure.rs` holds its constants against that file. The page that records the case is [validation-data.md](../../docs/validation-data.md), section "Rotating machinery sources".

| Quantity | Source | Value and use |
|---|---|---|
| The record | as for F-878: "FAN-01 Benchmark: Low-pressure axial fan in a short duct", v6, Kroemer et al., Zenodo, doi:10.5281/zenodo.23943494, published 2026-11-04, CC BY 4.0 | `mic_kulite.tar`, 460 728 320 bytes, MD5 1002be297aa2ac2123f3928bf7f5d905 (equal to the record's), SHA-256 ce02ca09c4c2ec84da54a09ccb271b3012e190110ff95c9b44c412a7a7032862; inside it `mic_kulite/mic_kulite_n1ug_1_4.mat` (read, MAT level 5), SHA-256 79a2025fcacec78cf7d0a2942c128ec1f9e82b103bafb7f4d880580ed1507b1c, and `mic_kulite_n1ug_1_4.h5` (not read: no HDF5 reader on the machine), SHA-256 0e330c8a4874065b4a9b59e699b9f5955394e8f5421ab073a702ab1351028aca |
| What the data are | description, Table 2, Table 4, Appendix A.2 and Appendix B (read); the `.mat` file | `kulite_pressure_in_Pa`, 15 rows of 1 464 320 samples at 48 kHz (30.5 s), "wall pressure, differential to the chamber pressure, mean value included"; row n = transducer n on the casing (r = 250 mm) at x = 10 n - 33 mm from the upstream hub face, n = 1 to 14 (the mapping is inferred by the record); row 0 holds no signal; the design point, 1.4 m^3/s |
| Uncertainty of the wall pressure | description, README, the `.mat` file (read) | **not stated**: transducer type only (differential, 1 psi range); no calibration, accuracy or zero |
| Time means of the rows behind the blade tips (tip trailing edge x = 62.8 mm), Pa against the chamber | `kulite_means.csv` (computed here from the file, not published values) | x = 67: 92.82; 77: 104.81; 87: 112.77; 97: 116.54; 107: 117.30 (twice the standard error of the 30-s mean 0.5 to 0.7). Ahead of the blades, x = -23 to 17 mm: -49.83 to -39.49 |
| Static pressure at the casing at the LDA plane x = 90 mm, and over the measured span | ADR 0769 (linear interpolation; the casing-to-span correction of the radial equilibrium from the LDA profiles of F-878) | `p_w(90)` = 113.90 Pa; `Pi_m` = 93.96 Pa +- 7.50 Pa (no measurement part; the axial-derivative term of the radial equilibrium not in it) |

## Contradictions with the wiki

- None. The section closes what [[doc-reference-data-f878-fan01-lda-profiles]] and ADR 0765 left open ("an LDA measures no pressure, so the loss of the measured rotor is not known"): the record does hold a measured static pressure behind the rotor, on the casing. A newer reading, not a conflict.

## Pages this source informs

- [[doc-reference-data]] (hub)
- [[adr-thread-fan01-design-point]] (ADR 0769, F-883)
- [[fan-01]]
- [[validation-data-pack]]
- [[doc-reference-data-f878-fan01-lda-profiles]] (the LDA profiles of the same record, used for the radial-equilibrium correction)
