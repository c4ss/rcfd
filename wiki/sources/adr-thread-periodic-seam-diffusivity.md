---
type: source
tags: [adr-thread, turbulence, periodic, seam, diffusivity, fan]
sources: []
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: one diffusivity per periodic seam pair

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (three registrations with result addenda), 3 ADRs, 2026-11-06 to 2026-11-08.

On a periodic seam the two faces of a pair took their diffusivity from their own owner cell, so the two sides of one surface carried different values and the global budgets of k and omega did not close. F-295 (ADR 0205) gave both faces one value for SST under `turbulence.periodic_seam.diffusivity = "pair"`; F-296 (ADR 0212) extended the rule to the other models; F-302 (ADR 0227) to the property and user-function viscosities, with a `pimple` check. All three are done; the default (owner) is bit-identical. See [[cyclic-symmetry]], [[adr-thread-coupled-periodic-faces]].

## Current state

- **F-295, F-296, F-302 done** ([[roadmap-turbulence]]).
- **The rule** (ADR 0205 D1, generalised by ADR 0212 D1): for a pair $(f, g)$, $f < g$, $P$ the owner of $f$, $Q$ the owner of $g$, $w = dn_g / (dn_f + dn_g)$, both faces take "the formula the model uses on its internal faces, with the pair's weight $w$ in place of the internal weight and the cells $P$, $Q$ in place of owner and neighbour"; for SST $\Gamma_f = \Gamma_g = \nu + w\, a_P + (1 - w)\, a_Q$.
- **Scope:** SST momentum, k and omega (F-295); k-epsilon, SA, RSM, subgrid and mixing-length, rheology (F-296); property viscosity $\mu(w T_P + (1 - w) T_Q) / \rho$ and user-function viscosity evaluated once per pair at the centre of the lower-index face (F-302, Q1 recommendation (a)).
- **Not covered:** the energy-equation diffusivity on a seam (ADR 0227 Q2, leave until a case needs it); the RSM stress divergence on rotational seams (ADR 0212 Q1 (c), "only if the Reynolds stress model is ever run on a rotational seam").
- **Answers of 2026-11-18:** questions closed on their written recommendations by the lead (the general rule recorded in each ADR).

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0205](../../docs/adr/0205-f295-registration-one-diffusivity-per-periodic-seam-pair.md) | 2026-11-06 | registration | F-295 | One diffusivity per seam pair for SST | done; k and omega budgets close to `1e-14`; B2 fan passage pairs equal to the bit | 0212 |
| [0212](../../docs/adr/0212-f296-registration-one-diffusivity-per-periodic-seam-pair-for-the-other-models.md) | 2026-11-07 | registration (two addenda) | F-296 | The same for the other models | done; ADR 0205 S1 kept as an ignored record | 0227 |
| [0227](../../docs/adr/0227-f302-registration-pair-diffusivity-for-properties-and-user-function-viscosity.md) | 2026-11-08 | registration | F-302 | Property and user-function viscosities, and the `pimple` check | done | |

## Key numbers and limits

- ADR 0205: with `pair` the global k and omega budgets of the periodic box of ADR 0199 C1 "close to 1e-14 of the destruction (the owner rule misses them by 1.7e-7 to 6.5e-2)"; on the B2 fan passage "the pairs are equal to the bit where the owner rule differs by 13 to 27 percent".
- ADR 0205 Q1: low priority for the other models because "FAN-01 and check H run SST".
- ADR 0205 Q2: recommendation (b), an amendment of ADR 0183 so that check H sets `diffusivity = "pair"` before it runs.

## Open questions

- ADR 0212 Q3: `pair` for the `nuTilda` diffusivity of SA (differs where `nuTilda < 0` on a seam cell).
- ADR 0227 Q2, Q3: the energy diffusivity on a seam; `pair` for the property viscosity changes only rounding.

## Related

- [[cyclic-symmetry]], [[adr-thread-coupled-periodic-faces]], [[adr-thread-transient-rans]], [[fan-01]], [[fan-passage-mesh]], [[rcfd-turb]]
- [[doc-reference-data-f302-seam-diffusivity]], [[roadmap-turbulence]]
