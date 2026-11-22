---
type: source
tags: [overview]
sources: []
origin: README.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: stable
---

# README.md

**Origin:** [README.md](../../README.md) at `origin/main` 2e94e5ef
**Kind:** repo front page

## Summary

What rcfd is (clean-room AGPL-3.0 finite-volume CFD suite in Rust: CLI solver, solver service, MCP server, desktop app), its main limits, how to build, test and start the Studio, and the licence.

## Key points

- Clean-room: algorithms from published literature or first principles only; vendor docs consulted in a few recorded cases (mostly contents pages; some theory pages to check model constants, now marked UNVERIFIED). See [[clean-room]], [[provenance]].
- Main gaps listed: FAN-01 steady rotating-frame solve misses its operating point (diagnosis open); structural solver new and unvalidated; overset not available from a case or the app (library has a first coupling, F-134, ADR 0219); Studio has no installer.
- Sliding-mesh transient solves **run from a case file** for the full-annulus rotor-stator generator and its sector form (`[mesh.sliding]`, F-303, ADR 0637; F-711, ADR 0646; laminar piso/pimple); planar sliding is library-only.
- Toolchain: Rust 1.90+ (CI type-checks on 1.90, builds and tests on 1.99); CUDA optional; Node.js 22.12+ for the Studio; Python 3.9+ optional (gate script, data fetch).
- Full test suite ~1 h release on 16 threads; the [[gate]] (`python scripts/gate_test.py`) runs on `main` in CI, not before each merge; a merge is first checked by the tests its changes select.
- Studio start: build `rcfd-cli`, `rcfd-runner`, `rcfd-mcp` release, then `npm ci && npm run build` in `apps/studio-web`, `RCFD_STUDIO_RCFD=... npm start`.
- `example-projects/` (Windows runs) and `example-projects-linux/` (same projects, Linux runs): `wind-tunnel`, `quick-start` (2D), `axial-fan` (3D blade passage, first-order, view only).
- Licence: AGPL-3.0 plus a commercial licence; third-party data in `validation-data/` and `cases/*/reference/` keep their own terms.

## Contradictions with the wiki

- Sliding mesh: README says transient sliding-mesh solves run from a case file (F-303, F-711), but [[doc-roadmap-overview]] 'What does not work yet' says they are not available. Flagged on [[overview]].

## Pages this source informs

- [[overview]]
- [[clean-room]]
- [[provenance]]
- [[gate]]
- [[studio]]
- [[fan-01]]
