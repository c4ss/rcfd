---
type: source
tags: [tutorial, validation]
sources: []
origin: docs/tutorials/cavity.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Tutorial: run a lid-driven cavity and compare it with published data

**Origin:** [docs/tutorials/cavity.md](../../docs/tutorials/cavity.md) at `origin/main` 2e94e5ef
**Kind:** tutorial (executed by tests)

## Summary

The Re 100 lid-driven cavity on 32 x 32 cells with SIMPLEC and `linear_upwind`: run, sample the vertical centreline, compare by eye with Ghia, Ghia and Shin (1982), sweep Re with an operating map, optionally refine.

## Key points

- "The comparison on this page is by eye: the tutorial test runs the commands but does not check the velocities against the reference values." The quantitative comparison is in `crates/rcfd-api/tests/validation_cavity_ghia.rs`.
- Reference values in `cases/cavity/reference/ghia1982_u_centerline.csv`; citation and the 2 % tolerance in the [[benchmark-descriptor]] `cases/cavity/case.json`.
- Observation when written: ux = -0.207 at y = 0.5 against the reference -0.20581 ("a rounded observation", not checked).
- `rcfd sample ... --line 0.5,0,0:0.5,1,0 --points 33` writes columns `s, x, y, z, ux, uy, inside`.
- `rcfd map cavity_re100.toml --axis physics.nu=0.01,0.005,0.0025` sweeps Re 100/200/400; each point restarts from the previous converged solution; `re_sweep/map.csv` one row per point.
- Refinement to 64 x 64 runs only in the slow test (`run-slow`).

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[executable-tutorials]], [[benchmark-descriptor]], [[report]]
