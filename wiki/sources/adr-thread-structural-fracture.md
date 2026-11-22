---
type: source
tags: [adr-thread, structural, fem, fracture, j-integral, stress-intensity]
sources: []
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: structural solver, fracture parameters (J-integral, stress intensity factors, crack growth)

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (registrations with result addenda), 4 ADRs, all dated 2026-11-15 in the ADR index. Job p3-structural-b. High-cycle fatigue (S-N curves, rainflow, Dirlik, Benasciutti-Tovo) is on [[adr-thread-structural-fatigue-and-radiated-sound]].

`rcfd_fem::fracture` in [[rcfd-fem]]: the J-integral by the equivalent domain integral on rings of elements about a crack front, the plane-strain stress intensity `K = sqrt(J E/(1 - nu^2))`, the Paris crack-growth life, then the interaction integral that separates `K_I`, `K_II` and `K_III` on straight, circular, elliptical and closed fronts (plan ADR 0310, stub ADR 0332).

## Current state

- **Domain integral and growth life** (F-429, [ADR 0547](../../docs/adr/0547-f429-registration-fracture-parameters-domain-integral-stress-intensity-and-growth-life.md), partial: one mutation not caught, every acceptance clause met): `CrackFront` (straight or circular), `j_integral`, `plane_strain_k`, `opening_k`, `paris_cycles`, the builders `centre_crack_plate` (Hex20 graded towards the tips; quarter-point elements are refused by the mesh validation) and `penny_crack_cylinder`.
- **Elliptical front and mode separation** (F-632, ADR 0605, done): `CrackFront::elliptical`, `elliptical_crack_block`, `interaction_integral` giving `K_I`, `K_II`, `K_III`.
- **Curved fronts** (F-682, ADR 0610, partial: EC1 missed, the lead may take done): `interaction_integral_curved` with the correction terms of the auxiliary fields derived in closed form in the local frame.
- **Closed fronts and mirror planes** (F-689, ADR 0618): `CrackFront::closed` (periodic fronts without end stations) and `CrackFront::mirror_plane` (`K_III = 0` at stations on a mirror plane). Addendum 2 meets every check and calls F-689 done; the roadmap lists it done.
- References are closed forms (the secant form of the centre crack, penny and elliptical cracks); no measured crack is compared. No case-file key or output for fracture is named in these ADRs.

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by / followed by |
|---|---|---|---|---|---|---|
| [ADR 0547](../../docs/adr/0547-f429-registration-fracture-parameters-domain-integral-stress-intensity-and-growth-life.md) | 2026-11-15 | registration | F-429 (partial) | J-integral by the domain form, stress intensity, crack-growth life | partial: one mutation not caught | ADR 0605 |
| [ADR 0605](../../docs/adr/0605-f632-registration-domain-integral-on-an-elliptical-front-and-mode-separation.md) | 2026-11-15 | registration | F-632 (done) | Domain integral on an elliptical front; separation of K_I, K_II, K_III | done | ADR 0610 |
| [ADR 0610](../../docs/adr/0610-f682-registration-interaction-integral-on-curved-crack-fronts.md) | 2026-11-15 | registration | F-682 (partial) | Interaction integral on curved (circular, elliptical) fronts | partial: EC1 missed | ADR 0618 |
| [ADR 0618](../../docs/adr/0618-f689-registration-interaction-integral-on-closed-fronts-and-mirror-plane-stations.md) | 2026-11-15 | registration | F-689 (done) | Interaction integral on closed fronts and at mirror-plane stations | done (after addendum 2) | — |

## Key numbers and limits

- Centre crack `W = 10 a`, plane strain, Hex20: `K` from `J` of rings 2-4 within `2.1e-3` of the secant form (limit `2e-2`), rings within `1.2e-3` of each other (ADR 0547).
- Elliptical front: EH1 `8.4e-3` against `3e-2` with `K` varying by 41 % along the front; MI1, MI2 and M31 within 2 % (ADR 0605).
- Curved fronts: PM1 to PM4 within 3 % (largest `1.5e-3`) (ADR 0610).

## Open questions

- ADR 0610 question 1: whether EC1's diagnosed miss lets the lead take F-682 as done (the roadmap lists F-682 partial at the snapshot).

## Related

- [[rcfd-fem]], [[structural-solver]], [[roadmap-structural-solver]]
- [[adr-thread-structural-fatigue-and-radiated-sound]], [[adr-thread-structural-verification-elements]]
