---
type: source
tags: [reference-data, fan-and-validation]
sources: []
origin: docs/reference-data.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Reference data: ADR 0760 (F-868): Ladson's NACA 0012 lift, the 120-grit and 180-grit zones and the scatter of the three zones

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "ADR 0760 (F-868): Ladson's NACA 0012 lift, the 120-grit and 180-grit zones and the scatter of the three zones" (lines 2410-2426) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

ADR 0760 (F-868): Ladson NACA 0012 maximum lift for the 120-grit and 180-grit zones and the three-zone scatter.

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


Source: `CLCD_Ladson_expdata.dat` of the NASA Turbulence Modeling Resource page https://tmbwg.github.io/turbmodels/naca0012_val.html, at the pinned address of [validation-data.md](../../docs/validation-data.md) (section `naca0012-tmr`), fetched and read on 2026-11-22: 1343 bytes, sha256 `78cd2f6aa4968e80f44cbf6c96f699bd9c6e45681d958ec528a10cf72ed23357`, equal to the recorded value. File header: "Data from Ladson, NASA TM 4074, 1988", "Re=6 million, with transition tripped", "M=0.15". The report NASA TM 4074 itself was not read. The file states no uncertainty. The 80-grit zone is the stored subset `validation-data/reference/w31c_naca0012_ladson1988_80grit_cl_cd.csv` (the row "NACA 0012 force and Cp data" above).

| Zone | Rows as (alpha in degrees, cl), read from the file | Status |
| --- | --- | --- |
| "120 grit" (18 rows) | (-4.01, -0.4466), (-2.12, -0.2425), (-0.01, -0.0120), (0.01, -0.0122), (2.15, 0.2236), (4.11, 0.4397), (6.01, 0.6487), (8.08, 0.8701), (10.10, 1.0775), (11.23, 1.1849), (12.13, 1.2720), (13.26, 1.3699), (14.30, 1.4571), (15.27, 1.5280), (16.16, 1.5838), (17.24, 1.6347), (18.18, 1.1886), (19.25, 1.1888) | read; held as constants in `crates/rcfd-api/tests/naca0012_lift_curve_f868.rs` |
| "180 grit" (18 rows) | (-3.99, -0.4363), (-1.98, -0.2213), (-0.03, -0.0115), (0.04, -0.0013), (2.00, 0.2213), (4.06, 0.4365), (6.09, 0.6558), (8.09, 0.8689), (10.18, 1.0809), (11.13, 1.1731), (12.10, 1.2644), (13.31, 1.3676), (14.08, 1.4316), (15.24, 1.5169), (16.33, 1.5855), (17.13, 1.6219), (18.21, 1.0104), (19.27, 1.0664) | read; held as constants in the same test file |
| Scatter of the three zones | each zone interpolated linearly in alpha; largest less smallest cl of the three at 0, 2, 4, 6, 8, 10, 12, 13, 14, 15, 16, 17 degrees: 0.0050, 0.0142, 0.0030, 0.0028, 0.0058, 0.0086, 0.0101, 0.0090, 0.0130, 0.0145, 0.0175, 0.0177; at 18 degrees (beyond the maximum lift) 0.2635 | derived from the file (ADR 0760, check S1); the value taken is 0.018 |



## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
- [[naca0012]]
