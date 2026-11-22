---
type: concept
tags: [structural, fem, nonlinear, continuation, stability]
sources: [adr-thread-structural-hyperelastic-and-follower, adr-thread-structural-incompatible-hex8-rule-l, adr-thread-structural-beams-shells, adr-thread-structural-materials-nonlinear]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Arc-length continuation

A path-following method for nonlinear equilibrium: the load factor becomes an unknown and each step is constrained by an arc length in load-displacement space, so a solution path can be followed through limit points (maxima of load, snap-through, snap-back) where load control fails.

## How rcfd uses it

- `arclength::trace` of [[rcfd-fem]] (ADR 0480, F-424) traces follower-pressure paths ([[adr-thread-structural-materials-nonlinear]]); `ArcLengthOptions::contribution_load` lets a reference load made only of follower pressure be traced (F-572, ADR 0560, done) ([[adr-thread-structural-hyperelastic-and-follower]]).
- Through the pressure maximum of the inflated hyperelastic sphere (F-649, ADR 0576, done) ([[adr-thread-structural-hyperelastic-and-follower]]).
- With corotated Shell4 and Shell3 (F-585, ADR 0522), checked against the published limit load of the hinged cylindrical roof, `2226.3 N` (F-610, ADR 0716) ([[adr-thread-structural-beams-shells]]).
- `arclength::trace_from` (opt-in) starts from a given converged state under prescribed-displacement loading (F-797, ADR 0726, partial) and passes the fold of rule L's `16^3` punch branch with warm-started local modes (F-845, ADR 0728, done) ([[adr-thread-structural-incompatible-hex8-rule-l]]).
- Related stability tools in the same chain: the indefinite-tangent Newton, a driver retry that reports the inertia of each accepted state (ADR 0694, ADR 0719), bisection of a bifurcation, a branch switch and a Moore-Spence extended system for the fold (ADR 0697, ADR 0700, ADR 0724) ([[adr-thread-structural-incompatible-hex8-rule-l]]).

## Related

- [[incompatible-modes-hex8]], [[hyperelastic-materials]], [[rcfd-fem]], [[structural-solver]]

## Open questions

- Whether `trace` accepts non-zero prescribed values by default (ADR 0726 Q3) ([[adr-thread-structural-incompatible-hex8-rule-l]]).
- Case-file keys for the continuation (ADR 0560 Q1) ([[adr-thread-structural-hyperelastic-and-follower]]).
- The thin roof `t = 6.35` with snap-back (ADR 0716 Q1) ([[adr-thread-structural-beams-shells]]).
