---
type: source
tags: [acoustics, fw-h, fan, noise, how-and-why]
sources: []
origin: docs/acoustics.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: stable
---

# Aeroacoustics for fan noise (wave 32B, ADR 0066)

**Origin:** [docs/acoustics.md](../../docs/acoustics.md) at `origin/main` 2e94e5ef
**Kind:** how-and-why page plus reference (606 lines)

## Summary

The library chain that produces a fan-noise estimate from a CFD run: time-domain FW-H for moving impermeable surfaces, a Gutin-type rotor tonal estimate, three steady-RANS broadband proxies, noise metrics, and file I/O. Later library additions: impedance and quiet boundaries, mass-spring-damper wall, sponge layers, a linear acoustic wave solver with duct modes, and radiated sound of a vibrating structure. FW-H is verified against closed forms; W5a/W5b MISSED; broadband proxies NOT validated; "No comparison with a measured fan was made." F-243 stays partial.

## Key points

- **Chain limits:** subsonic surfaces only, no quadrupole, free field, uniform mean flow only, no installation effects; a steady RANS run gives no tonal pressure history.
- **FW-H:** Formulation 1A (Brentner and Farassat); impermeable sets `u = v`, `rho' = 0`; Newton retarded time, 4-point Lagrange, fourth-order time derivatives; medium-frame evaluation for uniform stream. W5a/W5b (impermeable sphere in flow) MISSED (7.7e-2, 6.7e-2 vs 1e-2, 1e-3): test construction error proportional to U; kept as `*_known_miss`; W5d 4.5e-6.
- **Gutin-type tones:** derived from the dipole far field, checked against FW-H (errors <= 4e-5); "The original Gutin (1936) formula was not read".
- **Broadband proxies (NOT validated):** Proudman (alpha = 0.1, secondary source), Curle (needs `A_c`, `f_c`), trailing edge (`K_TE` placeholder). "Until then these numbers are not predictions."
- **Metrics:** SPL, OASPL, A weighting (IEC 61672-1 analytic formula, constants from secondary sources), Parseval spectra, 1/3-octave bands, sphere sound power; `rcfd-spectrum` v1 JSON.
- **Compressible boundaries** (library only): `CompBc::Impedance` (F-243, ADR 0233), matched and Thompson quiet boundaries (quiet only near normal incidence: 7 percent at 30 deg, 17 at 45), `MsdWall` (F-332, ADR 0243).
- **Sponge layers** (F-331, ADR 0237): plain and matched forms; shifted / corner / along (F-340, ADR 0248); on moving meshes (F-341, ADR 0265; F-407, ADR 0344).
- **Acoustic wave solver and duct modes** (F-333, ADR 0351): `acoustic_wave.rs`, `duct_modes.rs`; circular ducts (F-453, ADR 0366; counter-rotating mode 1.2e-5 vs 1e-6, known miss); driven from a sampled surface (F-452, ADR 0372); returning-wave split (F-478, ADR 0378). Not coupled to a CFD run; not in a case file or the CLI.
- **Radiated sound of a vibrating structure** (F-418, ADR 0468; F-551, ADR 0488; F-579, ADR 0538): ERP, baffled-plane Rayleigh integral, radiation impedance, light-fluid modes.
- **CLI:** `rcfd fwh series.json --observer x,y,z ...`; input `rcfd-surface-series` v1; exit codes 0/1/2/3.

## Contradictions with the wiki

- Within the page: the limits of the impedance-boundary section say "No wave-equation solver and no modal export exist", while a later section documents the acoustic wave solver and the `rcfd-duct-modes` export (F-333, ADR 0351). The page's own introduction says those were "added later". A stale line inside the repo doc.

## Pages this source informs

- [[fw-h]]
- [[aeroacoustics]]
- [[rcfd-post]]
- [[rcfd-fv]]
- [[rcfd-cli]]
- [[known-miss]]
- [[unverified]]
