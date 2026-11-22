---
type: source
tags: [interface, case, chemistry]
sources: [doc-case-schema]
origin: docs/case-schema.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Case schema: finite-rate and turbulent combustion chemistry

**Origin:** [docs/case-schema.md](../../docs/case-schema.md), section "`chemistry` finite-rate and turbulent combustion chemistry (F-054, ADR 0027)", at `origin/main` 2e94e5ef. Hub: [[doc-case-schema]]
**Kind:** reference section

## Summary

Reactions between `species` mass fractions of a constant-density incompressible case, from a CHEMKIN-format mechanism (file or inline).

## Key points

- `model`: `off` (default), `finite_rate`, `eddy_dissipation` (needs `k_epsilon` or `k_omega_sst`), `finite_rate_eddy_dissipation`, `edc`. `density` and `heat_capacity` must equal `physics.rho` / `physics.cp`.
- Constants: Magnussen-Hjertager `eddy_dissipation_a` 4.0, `_b` 0.5 (UNVERIFIED); EDC `edc_c_xi` 2.1377, `edc_c_tau` 0.4082 (secondary source).
- `acceleration` (F-223, ADR 0053): `off`, `cluster`, `isat` (Pope 1997), transient `finite_rate` only.
- Output `chemistry.json` whenever `model` is not `off` (ADR 0037); the binary and the service give bit-identical fields to the in-process run.
- Transient: Lie splitting; steady: source form. Limits: constant density, no Strang splitting, no flamelet models, "no comparison with published flames (F-054 stays `partial`)". "No mechanism is shipped."

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[rcfd-reactor]], [[unverified]]
