---
type: source
tags: [adr-thread, structural, fem, fatigue, acoustics, radiation]
sources: []
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: structural solver, high-cycle fatigue and radiated sound of a vibrating surface

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (registrations with result addenda), 6 ADRs, all 2026-11-14. Later follow-ups (Benasciutti-Tovo F-590, mutual radiation impedance F-579) are in the other half of the category.

Two post-processing families on top of the dynamic results of [[rcfd-fem]]: `rcfd_fem::fatigue` (stress-life curves, mean-stress corrections, rainflow counting, linear and narrow-band damage, then broadband Dirlik damage from spectral moments), and the radiated sound of a vibrating structural surface (equivalent radiated power and the Rayleigh integral of a baffled plane, then the radiation reactance), which lives in `rcfd_post::solid_acoustic` ([[doc-acoustics]]).

## Current state

- **Fatigue:** F-413 (ADR 0449) done: Basquin, power and log-log table curves with an endurance limit, Goodman, Gerber and Soderberg corrections (a mean at or above the strength refused, no compressive credit by default), the closed-form von Mises amplitude of a harmonic tensor, rainflow and Miner damage. F-534 (ADR 0453) added the rainflow residue guard and the Gerber compressive-mean message (done). Broadband Dirlik damage from `m0, m1, m2, m4` (F-566, ADR 0497) is partial by one uncaught mutation (M10) and amended checks; the moments $C_1$, $C_4$ (F-589, ADR 0516) are done.
- **Case-file wiring:** a `[fatigue]` section with outputs `damage_rate` and `life` (F-605) was **approved** by the maintainer on 2026-11-18 under the additive-golden-edit class rule and "may be issued" (ADR 0516, answers section); F-605 is still todo in the roadmap.
- **Radiated sound:** F-418 (ADR 0468) partial: every limit met, but E1 at $k a = 10$ missed its one-ulp probe condition. F-551 (ADR 0488, radiation reactance by analytic face integrals of `1/R`) partial: mutation MU7 (NaN guard) not caught; its sources are "cited from memory, not re-read" (roadmap gap).

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0449](../../docs/adr/0449-f413-registration-stress-life-rainflow-miner.md) | 2026-11-14 | registration (hub) | F-413 | S-N curves, mean-stress corrections, harmonic amplitude, rainflow, linear and narrow-band damage | done | |
| [0453](../../docs/adr/0453-f534-registration-rainflow-residue-guard-gerber-message.md) | 2026-11-14 | registration | F-534 | Rainflow residue guard, Gerber compressive-mean message | done | |
| [0468](../../docs/adr/0468-f418-registration-radiated-power-rayleigh-integral.md) | 2026-11-14 | registration | F-418 | ERP and the Rayleigh integral of a baffled plane | partial; E1 one-ulp probe 751 not 1000 | 0488 |
| [0488](../../docs/adr/0488-f551-registration-radiation-impedance-analytic-self-face-integrals.md) | 2026-11-14 | registration | F-551 | Radiation reactance and surface pressure by analytic face integrals of `1/R` | partial; MU7 not caught | (F-579, other half) |
| [0497](../../docs/adr/0497-f566-registration-broadband-fatigue-dirlik-from-spectral-moments.md) | 2026-11-14 | registration | F-566 | Dirlik damage rate from m0, m1, m2, m4 | ADR done by its rule; item partial (M10, amended C2, C6) | |
| [0516](../../docs/adr/0516-f589-registration-first-and-fourth-spectral-moments-of-a-random-stress-component-for-broadband-fatigue.md) | 2026-11-14 | registration | F-589 | First and fourth spectral moments of a random stress component | done; answers of 2026-11-18 approve F-605 | |

## Key numbers and limits

- F-418 gap: "Check E1 at k a = 10 met its 1e-12 limit but not the registered one-ulp probe condition (limit 751, not 1000, times the one-ulp change; rounding of a 14400-term sum)" ([[roadmap-structural-solver]]).
- F-551 (ADR 0488): "a2 by Z1 and Z2, largest 3.2e-3 against 1e-2; a3 by Z3, 8.0e-14 against 1e-6; a4 by M1, 5.0e-3 against 2e-2".
- F-566 (ADR 0497): the recalled form of the Dirlik coefficients is corroborated; "counted damage within 6.2 percent of a" synthetic record.

## Open questions

- ADR 0449 Q1 (wiring into case file, CLI and API) became F-605, approved 2026-11-18, not built at this snapshot.
- F-566: "The ADR proposes done; the maintainer's call".
- F-551: sources cited from memory, not re-read (a clean-room evidence point; see [[sources-read-addendum]]).

## Second half of the category (ADRs 0524 and 0538, job p3-structural-b)

### Current state (second half)

- **Benasciutti-Tovo** broadband damage rate of a Gaussian stationary stress from `m0, m1, m2, m4` as a second method beside Dirlik (F-590, ADR 0524, partial: C1 and C1b recorded as ignored known-miss tests). The roadmap note records the paper as "D. Benasciutti and R. Tovo, Int. J. Fatigue 27 (2005), cited from memory, the fitted weight b unverified".
- **Mutual radiation impedance** matrix of a baffled plane and the light-fluid added mass, damping and frequency shift of structural modes (F-579, ADR 0538, done: every check met in the single registered run, no amendment; Y1 and Y2 at `8.8e-16` against `1e-10`).
- Fracture parameters (J-integral, stress intensity, crack-growth life) are on [[adr-thread-structural-fracture]].

### ADRs (second half)

| ADR | Date | Kind | Item | Question | Outcome | Superseded by / followed by |
|---|---|---|---|---|---|---|
| [ADR 0524](../../docs/adr/0524-f590-registration-benasciutti-tovo-broadband-damage-rate.md) | 2026-11-14 | registration | F-590 (partial) | Benasciutti-Tovo broadband fatigue damage rate as a second method | partial: C1, C1b known misses | — |
| [ADR 0538](../../docs/adr/0538-f579-registration-mutual-radiation-impedance-and-light-fluid-modes.md) | 2026-11-14 | registration | F-579 (done) | Mutual radiation impedance of a baffled plane; light-fluid added mass, damping, frequency shift | done | — |

### Open questions (second half)

- The Benasciutti-Tovo citation "from memory" with an unverified fitted weight (ADR 0524 roadmap note): to be checked against the paper.

## Related

- [[aeroacoustics]], [[rcfd-post]], [[rcfd-fem]], [[structural-solver]], [[roadmap-structural-solver]]
- [[adr-thread-structural-dynamics-harmonic]]
