---
type: source
tags: [acoustics, fw-h, fan, noise, how-and-why]
sources: []
origin: docs/acoustics.md
origin_commit: 709e302e
created: 2026-11-22
updated: 2026-11-24
status: stable
---

# Aeroacoustics for fan noise (wave 32B, ADR 0066)

**Origin:** [docs/acoustics.md](../../docs/acoustics.md) at `origin/main` 709e302e (first read at 2e94e5ef; the section on `rcfd duct-modes` added by F-451, ADR 0783)
**Kind:** how-and-why page plus reference (650 lines; 606 at 2e94e5ef)

## Summary

The library chain that produces a fan-noise estimate from a CFD run: time-domain FW-H for moving impermeable surfaces, a Gutin-type rotor tonal estimate, three steady-RANS broadband proxies, noise metrics, and file I/O. Later library additions: impedance and quiet boundaries, mass-spring-damper wall, sponge layers, a linear acoustic wave solver with duct modes, and radiated sound of a vibrating structure. FW-H is verified against closed forms; W5a/W5b MISSED; broadband proxies NOT validated; "No comparison with a measured fan was made." F-243 stays partial.

## Key points

- **Chain limits:** subsonic surfaces only, no quadrupole, free field, uniform mean flow only, no installation effects; a steady RANS run gives no tonal pressure history.
- **FW-H:** Formulation 1A (Brentner and Farassat); impermeable sets $u = v$, $\rho' = 0$; Newton retarded time, 4-point Lagrange, fourth-order time derivatives; medium-frame evaluation for uniform stream. W5a/W5b (impermeable sphere in flow) MISSED (7.7e-2, 6.7e-2 vs 1e-2, 1e-3): test construction error proportional to U; kept as `*_known_miss`; W5d 4.5e-6.
- **Gutin-type tones:** derived from the dipole far field, checked against FW-H (errors <= 4e-5); "The original Gutin (1936) formula was not read".
- **Broadband proxies (NOT validated):** Proudman (alpha = 0.1, secondary source), Curle (needs $A_c$, $f_c$), trailing edge ($K_{TE}$ placeholder). "Until then these numbers are not predictions."
- **Metrics:** SPL, OASPL, A weighting (IEC 61672-1 analytic formula, constants from secondary sources), Parseval spectra, 1/3-octave bands, sphere sound power; `rcfd-spectrum` v1 JSON.
- **Compressible boundaries** (library only): `CompBc::Impedance` (F-243, ADR 0233), matched and Thompson quiet boundaries (quiet only near normal incidence: 7 percent at 30 deg, 17 at 45), `MsdWall` (F-332, ADR 0243).
- **Sponge layers** (F-331, ADR 0237): plain and matched forms; shifted / corner / along (F-340, ADR 0248); on moving meshes (F-341, ADR 0265; F-407, ADR 0344).
- **Acoustic wave solver and duct modes** (F-333, ADR 0351): `acoustic_wave.rs`, `duct_modes.rs`; circular ducts (F-453, ADR 0366; counter-rotating mode 1.2e-5 vs 1e-6, known miss); driven from a sampled surface (F-452, ADR 0372); returning-wave split (F-478, ADR 0378). Not coupled to a CFD run; ~~not in a case file or the CLI~~ at 709e302e "the wave solver is not reachable from a case file or the CLI. The modal analysis of saved sections is, by the command below."
- **`rcfd duct-modes`** (F-451, ADR 0783; added at 709e302e): `rcfd duct-modes s0.json [s1.json ...] --duct rect:a,b|circ:radius --frequency hz [--k-max k] [--x-ref x] [--origin y,z] [--out modes.json]`. Each file is an `rcfd-surface-series` document of one duct section (inline pressure; `pressure_csv` is not read); the duct axis is `+x`; the section coordinates of a face are $(y, z)$ less `--origin`. Each series must cover a whole number of periods of the one frequency analysed; `rho0` and `c0` are read from the first file. Modes fitted: those with a cut-off wavenumber up to `--k-max`, by default up to $k = 2\pi f/c_0$; "A section needs at least as many faces as modes." One file gives `"kind": "section"`; two or more at different `x` give per mode the waves towards `+x` and `-x` at `--x-ref` and their powers (`"kind": "axial"`). `residual` "says how much of the sampled field the fitted modes do not represent, in pascals". Exit codes 0, 1 I/O, 2 usage, 3 invalid input; a file that cannot be analysed is refused with its name, the JSON path and the reason, and nothing is written. Checked: on the synthetic rectangular field (seven sections, eight modes) "the command's export equals the library's byte for byte". "Limits: the axis is `+x` only; one frequency per run; the axial wavenumber is the closed form, not fitted; no flow."
- **Radiated sound of a vibrating structure** (F-418, ADR 0468; F-551, ADR 0488; F-579, ADR 0538): ERP, baffled-plane Rayleigh integral, radiation impedance, light-fluid modes.
- **CLI:** `rcfd fwh series.json --observer x,y,z ...`; input `rcfd-surface-series` v1; exit codes 0/1/2/3. Since 709e302e also `rcfd duct-modes` (`crates/rcfd-cli/src/cmd_duct_modes.rs`).

## Contradictions with the wiki

- Within the page: the limits of the impedance-boundary section say "No wave-equation solver and no modal export exist", while a later section documents the acoustic wave solver and the `rcfd-duct-modes` export (F-333, ADR 0351). The page's own introduction says those were "added later". A stale line inside the repo doc.

## Pages this source informs

- [[fw-h]]
- [[aeroacoustics]]
- [[duct-acoustic-modes]]
- [[adr-thread-acoustic-wave-solver]]
- [[rcfd-post]]
- [[rcfd-fv]]
- [[rcfd-cli]]
- [[known-miss]]
- [[unverified]]
