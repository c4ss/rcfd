---
type: source
tags: [reference-data, wave-section]
sources: []
origin: docs/reference-data.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Reference data: Wave 30A two-way particle coupling and mixing plane in the solver loops (F-128, F-126, ADR 0059)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Wave 30A two-way particle coupling and mixing plane in the solver loops (F-128, F-126, ADR 0059)" (lines 1240-1255) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Wave 30A two-way particle coupling and mixing plane references (ADR 0059).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read in this wave and no web page was fetched. Every reference below is derived from first principles or recalled from memory (unverified). No vendor code, binary or install was used. No published particle-laden jet, no spray, no published turbomachine (Rotor 37, Hannover compressor, any pump or fan) was run or compared: NOT ATTEMPTED.

| Item | Status | Note |
|------|--------|------|
| Momentum given to the carrier is minus the momentum the parcels receive from it; the sum is conserved | derived | Newton's third law on the drag impulse; the record is conservative to round-off (J2 5.7e-16) |
| Steady plug: force on the carrier per unit area balances the pressure drop, `F = mdot_p (v_L - v0)`, with `v = U - (U - v0) e^{-t/tau}`, `x = x0 + U t - (U - v0) tau (1 - e^{-t/tau})` | derived | uniform stream cannot change by continuity; PLUG-B measured F 1.8e-5 and pressure drop 7.6e-4 off it (limits 1e-6 miss, 1e-3 met) |
| Free-vortex plus uniform axial flow `u = W z + (Gamma/r) theta`, `p = p0 - Gamma^2/(2 r^2)` as an exact steady Navier-Stokes solution | derived | curl u = 0 so the viscous term vanishes; convection balances the radial pressure gradient; the MRF terms cancel for any axisymmetric field, so the field is exact for any rotor speed |
| Mass flux and angular momentum flux conservation across the plane per band | derived | `u_n = Q/A'` and `u_theta = Gamma/r` by construction (identity checks, 1e-14), so MP2 and MP3 test the wiring, not the physics |
| Mixing loss non-negative for uniform pressure and non-negative normal velocity | derived | as ADR 0055; measured 11.8 percent of the energy flux for a 30 percent cosine axial distortion |
| Ranz and Marshall (1952) correlation, Rosin-Rammler distribution, mixing-plane literature (Denton) | unverified | as in the wave 29A and 29C sections; not re-read |
| Pressure error 6.85e-2 P at the hub wall against the exact centripetal gradient | measured | the plane-free control gives 1.13e-1, 6.62e-2, 4.24e-2 P for 4, 8, 16 radial layers (roughly first order); attributed, as an unisolated order-of-magnitude estimate, to the zero-gradient wall pressure; not compared with any other code |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
