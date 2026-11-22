---
type: concept
tags: [structural, rotordynamics, method]
sources: [adr-thread-structural-rotordynamics, doc-reference-data-f415-coriolis-campbell, doc-reference-data-f555-cyclic-campbell]
builds_on: [rotordynamics-gyroscopic-effects, modal-analysis]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Campbell diagram (Campbell table)

The natural frequencies of a rotating part plotted (or tabulated) against its speed, with modes tracked from speed to speed; crossings with engine-order lines $n \Omega$ mark possible resonances. With Coriolis coupling each mode splits into forward and backward whirl ([[adr-thread-structural-rotordynamics]]).

## How rcfd uses it

- F-396 (ADR 0403): modes of a rotating part with stress stiffening and spin softening, mode tracking and a Campbell table; Coriolis neglected; partial ([[adr-thread-structural-rotordynamics]]).
- F-499 (ADR 0411): gyroscopic matrix and forward / backward whirl in the Campbell table. Free vibration in the rotating frame $M u_{tt} + 2 \Omega C u_t + K_{\mathrm{eff}} u = 0$, $C$ skew ([[doc-reference-data-f415-coriolis-campbell]]).
- F-415 (ADR 0472): split Campbell table on the modal subspace; F-555 (ADR 0481) the same for a cyclic sector by index ([[doc-reference-data-f555-cyclic-campbell]]).
- F-564 (ADR 0501): damped critical speeds as unbalance-response peaks and checked Campbell crossings of the flexible rotor on bearings ([[adr-thread-structural-rotordynamics]]).
- Not done: a Campbell diagram of the fan blade (F-406, todo) ([[roadmap-structural-solver]]).

## Builds on

- [[rotordynamics-gyroscopic-effects]]: whirl, gyroscopic coupling, critical speeds.
- [[modal-analysis]]: natural frequencies and mode shapes.

## Related

- [[cyclic-symmetry]], [[rcfd-fem]], [[structural-solver]], [[fan-01]]
