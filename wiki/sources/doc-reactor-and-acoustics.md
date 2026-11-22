---
type: source
tags: [reactor, chemistry, acoustics, fw-h, reference]
sources: []
origin: docs/reactor-and-acoustics.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: stable
---

# Reactor cases (`rcfd reactor`, F-112) and FW-H acoustics (`rcfd acoustics`, F-136)

**Origin:** [docs/reactor-and-acoustics.md](../../docs/reactor-and-acoustics.md) at `origin/main` 2e94e5ef
**Kind:** user reference page (93 lines)

## Summary

Two stand-alone commands with their own JSON input: a zero-dimensional chemical reactor and a far-field FW-H calculation from a permeable data surface in a medium at rest. Checks in `docs/adr/0013-reactor-and-fwh.md`. Exit codes: 3 invalid input, 1 I/O, 2 usage.

## Key points

- `rcfd reactor <case.json> [--out <file.csv>]`. Limits: ideal gas only, constant-area constant-pressure plug flow without pressure drop, no heat exchange model, no surface chemistry, **no 1D flame speed (so F-112 stays partial)**, no sensitivity analysis.
- Mechanism: CHEMKIN-II text (ELEMENTS, SPECIES, THERMO NASA-7, REACTIONS with units, third bodies, `LOW`/`TROE` falloff, `REV`, `DUPLICATE`); `SRI`, `PLOG`, `FORD`, `LT`, `HIGH`, `MOLECULES` rejected with a line number. No mechanism ships; `python validation-data/fetch.py fetch gri-mech-30` downloads GRI-Mech 3.0 (licence recorded as unverified).
- `reactor.type`: `constant_pressure`, `constant_volume`, `plug_flow`, `psr` (with optional `blowout` bisection), `equilibrium` (`TP`, `HP` default, `UV`). Ignition delay by max dT/dt (rise threshold 400 K default).
- Integrator: in-repo extrapolated linearly implicit Euler (order 6 columns, dense LU), about 80 RHS evaluations per step; "Large mechanisms (hundreds of species) would need sparse algebra."
- `rcfd acoustics <surface.json> --observer x,y,z ...`: FW-H formulation 1A from a permeable surface, medium at rest, no volume quadrupole. Input `rcfd-fwh-surface` v1; at least 8 samples; retarded times three samples inside the record.
- For moving blades, uniform flow, rotor tones and metrics see [[doc-acoustics]].

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[rcfd-reactor]]
- [[fw-h]]
- [[aeroacoustics]]
- [[rcfd-cli]]
