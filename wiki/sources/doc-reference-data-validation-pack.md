---
type: source
tags: [reference-data, literature]
sources: []
origin: docs/reference-data.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Reference data: Public validation-data pack (F-125, fetched 2026-10-13)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Public validation-data pack (F-125, fetched 2026-10-13)" (lines 264-276) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

The public validation-data pack of F-125 as fetched on 2026-10-13: what each fetched file showed and what stayed unresolved.

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


Files are listed with checksums and licences in `validation-data/manifest.json` (see `docs/validation-data.md`).

- Flat plate Cf at x = 0.97: the pinned TMR file `FlatPlate/SST/cf_plate_sstv.dat` (CFL3D, SST) gives about 2.691e-3 (2.6949e-3 at x = 0.9601, 2.6909e-3 at x = 0.9701). `crates/rcfd-turb/tests/flat_plate_sst.rs` uses 0.00268299 (273 x 193 grid value from the TMR page). rcfd measures 0.002663: -0.7 % vs the page value, -1.0 % vs the file. Test tolerance (5 %) unchanged.
- Rotor 37 values from NASA TP-1337: only the report is stored; no numbers are transcribed yet. Unverified until they are.
- GRI-Mech 3.0 (Berkeley copy): licence status unverified (no licence text on the page). Cantera BSD copy preferred.
- OpenFOAM damBreak set-up files: GPL-3.0-only, fetched only, never vendored.
- Hannover compressor: no open upstream found; unresolved.


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
- [[validation-data-pack]]
