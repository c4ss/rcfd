---
type: source
tags: [tutorial, cases]
sources: []
origin: docs/tutorials/gallery.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Tutorial: example gallery

**Origin:** [docs/tutorials/gallery.md](../../docs/tutorials/gallery.md) at `origin/main` 2e94e5ef
**Kind:** tutorial / catalogue (executed by tests)

## Summary

The ready-made cases in `cases/`: what each shows and how far it is checked. The default test runs `rcfd check` on every case file; full runs are `run-slow` blocks. "A check does not solve the case: it shows that the case sets up, not that its result is right."

## Key points

- Two kinds of files: `*.toml` runnable [[case-file]]s, and `case.json` [[benchmark-descriptor]]s ("`rcfd run` refuses them"); `rcfd cases --dir cases` lists descriptors (`cavity`, `naca0012`, ...).
- Benchmarks: `backward_step`, `cavity` (Ghia 1982), `channel`, `flat_plate`, `heated_cavity`, `naca0012`.
- Runnable: `cavity/adapt_re100.toml` (576 base cells), four channel cases (all expected to succeed in the slow test), `packed_bed.toml` (stops at its 1700-iteration cap, exit 5), `rotating_channel.toml`, `sod.toml`, `wedge.toml`.
- Axial fan passages `axial_fan_sector.toml` (13332 cells) and `axial_fan_cgrid.toml` (11536 cells; `rcfd check` warns SV101, non-orthogonality over 70 degrees) are "laminar smoke cases for the geometry, the mesh and the rotating-frame plumbing, **not** predictions of fan performance"; 200 iterations do not converge (exit 5).

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[executable-tutorials]], [[benchmark-descriptor]], [[setup-rules]], [[fan-01]], [[exit-codes]]
