---
type: source
tags: [interface, case, moving-mesh]
sources: [doc-case-schema]
origin: docs/case-schema.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Case schema: dynamic mesh

**Origin:** [docs/case-schema.md](../../docs/case-schema.md), section "Wave 20B: `dynamic_mesh` (F-063, F-229, ADR 0033)", at `origin/main` 2e94e5ef. Hub: [[doc-case-schema]]
**Kind:** reference section

## Summary

`dynamic_mesh` moves the mesh in incompressible laminar `piso`/`pimple` runs (`euler`) and, since F-259 (ADR 0191), in transient compressible runs; prescribed or six-DOF motion; rigid, smoothing or layering deformation. Inert while `enabled = false`.

## Key points

- `motion` `prescribed`/`six_dof`; `deformation` `rigid`/`smoothing`/`layering`; `moving_patches`, `follow_patches`, `fixed_patches`, `rest_patches` (ADR 0183 P-B); displacement and rotation expressions of `t`.
- Six-DOF: `mass`, `inertia`, `rotation` `locked`/`free`/`about_axis`/`hinge`, `integrator` `rk4`/`newmark`.
- Strong coupling `[dynamic_mesh.six_dof.coupling] scheme = "strong"` (F-350, ADR 0266): the loose coupling "is unstable when the added mass of the fluid exceeds about twice the body's mass" (growth `sqrt(m_a/(2M))` per step, ADR 0250 D3).
- `wall_pressure = "prescribed_gradient"` became the default for the body force (F-358) and for force reports (`[dynamic_mesh.force_reports]`, F-440, ADR 0342) by ADR 0346, "approved by the maintainer on 2026-11-12"; this moved the result of every six-DOF case that does not write the key (piston `omega_m/omega` `1.001307` to `1.000264`).
- Layering: `box_3d`, piston `xmax`, merge below 0.4 and split above 1.6 of `lx / nx` (ADR 0090 thresholds, no key).
- Not implemented together: turbulence, energy, scalars, particles, sources, rheology, statistics, restart.

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[report]], [[case-file]]
