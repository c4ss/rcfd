---
type: concept
tags: [structural, fem, rotor, method]
sources: [adr-thread-structural-modal-buckling-cyclic, adr-thread-structural-rotordynamics, doc-reference-data-f555-cyclic-campbell]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Cyclic symmetry

A rotor made of N identical sectors is solved on one sector: the boundary unknowns of the high side are tied to the low side rotated by `2 pi / N`, and modes are found per harmonic index `k` (Bloch-Floquet on the sector), `k = 0` and `k = N/2` single, the others doubled ([[adr-thread-structural-modal-buckling-cyclic]]).

## How rcfd uses it

- `rcfd_fem::cyclic` (F-394, ADR 0386): `cyclic::boundary` matches the low and high surface sets node for node under the rotation, `MATCH_TOLERANCE 1e-8` of the largest radius; unmatched or doubly used nodes are refused by name. The `k = 0` rows `u_H = R u_L` are rows of the [[mpc-transformation]]; `modal_analysis` of a cyclic model therefore gives the `k = 0` result (taken as documented behaviour, ADR 0386 Q1) ([[adr-thread-structural-modal-buckling-cyclic]]).
- Sector against full model: frequencies within 3.96e-14 (limit 1e-8) ([[adr-thread-structural-modal-buckling-cyclic]]).
- Extended to harmonic loads and rigid sets at `k > 0` (F-489), cylindrical material axes (F-544), pre-stressed modes with per-point materials (F-547) ([[adr-thread-structural-modal-buckling-cyclic]]), tied non-matching cyclic boundaries and nodes on the axis (F-426) ([[adr-thread-structural-beams-shells]]).
- Rotating sectors: modes by harmonic family and the [[campbell-diagram]] (F-500), whirl and stability per index (F-507, F-511), the split Campbell table on the modal subspace (F-555) ([[adr-thread-structural-rotordynamics]], [[doc-reference-data-f555-cyclic-campbell]]).
- The demonstration on a bladed disc (F-406) is not started ([[roadmap-structural-solver]]).

## Related

- [[rcfd-fem]], [[structural-solver]], [[campbell-diagram]], [[mpc-transformation]]

## Open questions

- Shells and Beam2 in the cyclic constraint are follow-ups in the other half of the structural ADRs (F-628, F-672).
