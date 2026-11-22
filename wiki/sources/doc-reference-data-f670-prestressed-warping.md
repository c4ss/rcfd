---
type: source
tags: [reference-data, structural]
sources: []
origin: docs/reference-data.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Reference data: F-670 load height on the pre-stressed warping modes and harmonic path (ADR 0594)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-670 load height on the pre-stressed warping modes and harmonic path (ADR 0594)" (lines 2259-2270) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-670 load height on pre-stressed warping modes (ADR 0594).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-670. Every reference is derived in ADR 0594 (D2, D3) and computed by a plain numpy script (`f670-scratch/arith.py`, `arith.log`), no rcfd code.

| Item | Status | Source |
|---|---|---|
| Kinetic energy of the lateral-torsional motion of a doubly symmetric beam, the continuum of the library's consistent mass: `(1/2) int (rho A v_t^2 + rho I_z v'_t^2 + rho I_p phi_t^2) dx`, no warping inertia | derived | ADR 0594 D2 (from `beam::local_mass`, `beam_warping::local_mass`) |
| Rayleigh-Ritz frequencies, forks, `L = 3`, the ADR 0588 doubly symmetric I, UDL `0.5 lambda_ref q_ref` (sine series `N = 81`, converged to `<= 1.8e-9` from `N = 41`): `omega = 111.798383170594` (top flange), `107.430207818881` (shear centre), `103.339440973516` rad/s (bottom flange); unloaded `omega_0^2 = 1.660191376344e4`; at `lambda_ref` the reference eigenvalue is `<= 4.2e-12 omega_0^2` | computed | ADR 0594 D3 |
| Timoshenko-Rayleigh pinned lateral frequency, mode 1, `kappa = 5/6`: `omega_T = 128.729211880906` rad/s | derived (2 x 2 frequency equation of `v = sin`, `psi = cos`) | ADR 0594 arithmetic 2 |
| Measured, 16 elements: `prestressed_modes` and the pre-stressed `WarpingHarmonic` resonance `8.1e-4` to `8.3e-4` below the references (the shear-flexible lateral plane); at the model's buckling load `|omega^2|/omega_0^2 <= 7.7e-13` (modes), `<= 2.8e-10` (harmonic); at `lambda_ref` `<= 3.2e-5` | computed | ADR 0594 addendum 1 |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
