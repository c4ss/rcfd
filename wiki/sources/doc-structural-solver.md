---
type: source
tags: [structural, fem, fan]
sources: []
origin: docs/structural-solver.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: stable
---

# The structural solver

**Origin:** [docs/structural-solver.md](../../docs/structural-solver.md) at `origin/main` 2e94e5ef
**Kind:** user page plus hand-written capability summary and generated "What was built" list (495 lines)

## Summary

The finite-element structural solver (`rcfd-fem`): scope, how to run it, the fan-blade demonstration (F-387, ADR 0500) and a summary of the "Structural solver" roadmap line. "Early-stage software. Nothing in it is validated against a measured blade or a measured fan." Checks are closed forms, discretisation identities and mesh convergence.

## Key points

- Reading list for agents: this page, `crates/rcfd-fem/src/lib.rs`, the roadmap item; tier plans ADR 0270 (93 KB), 0290 (115 KB), 0310 (164 KB) are reference, read only your item's section.
- **Basic tier** (ADR 0270) in the code; five items partial by a missed check (F-378, F-381, F-383, F-384, F-386): linear elastostatics and modal analysis, Tet4/Hex8/Tet10 (Hex20), meshes from box builders / FV meshes / tet mesher / Gmsh, loads (forces, traction, pressure, gravity, centrifugal, body force, thermal strain), direct (envelope or sparse `L D L^T`) and PCG solves, stresses, consistent mass, `.vtu` output and the solid case file.
- "There is no CLI command and no MCP tool for the structural solver yet": used from Rust or the solid case file.
- **Shell pressure on curved meshes** (ADR 0550, 0567, 0579): error `~ c h^2 / (R t)`; opt-ins `Load::shell_pressure_angle_weighted`, `shell_pressure_curvature`, `shell_pressure_curvature_with(.., Jet)`.
- **Fan-blade demonstration** (`crates/rcfd-fem/tests/sb_fan_blade.rs`): one blade of `ComputerFan::default()` ("an illustration of the parameter schema and not a real fan"), Tet10, nominal PBT-like E = 2.5 GPa, nu = 0.38, 1300 kg/m^3, 2000 rpm + 50 Pa. First frequency 188.8 to 186.1 Hz coarse to fine (1.5 percent); reactions balance to about 1e-11. Not validated; linear and at rest. Library blade mesher `fan_blade_tet4/tet10`, `computer_fan_blade_tet10` (ADR 0505); case-file keys proposed in ADR 0505 "wait for the maintainer".
- **Roadmap summary as of 2026-11-18:** 307 items, 152 done, 129 partial, 26 todo; drafted by Claude from `docs/roadmap.json`, "not regenerated". Areas: elements, beams/shells, materials (orthotropic, viscoelastic, creep, hyperelastic, J2), loads and substructures, modes/buckling/harmonic/random, rotating parts (Coriolis, whirl, Campbell), nonlinear statics/dynamics, contact/fatigue/fracture, linear solvers (incl. `rcfd-dist` ranks and CUDA, F-433), coupling.
- **Not built:** no CLI / MCP (F-405), no `[solid]` table in the flow case (F-404), no fan-rotor demonstration (F-406, F-436), no public coupled benchmark (F-434).
- The generated block (`scripts/process_report.py --write`) lists **154** done items.

## Contradictions with the wiki

- [[doc-product]] says a structural solver "does not exist"; this page documents a substantial library (already flagged on [[overview]]).
- Within the page: hand-written summary says 152 done of 307 (as of 2026-11-18); the generated list says 154 done. Different dates of the two parts, not a conflict of claims.

## Pages this source informs

- [[structural-solver]]
- [[rcfd-fem]]
- [[rcfd-post]]
- [[cosimulation-protocol]]
- [[roadmap-structural-solver]]
