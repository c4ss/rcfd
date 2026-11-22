---
type: source
tags: [adr-thread, particles, lagrangian, wall-film, erosion, droplets, evaporation, breakup, two-way-coupling]
sources: [doc-reference-data-f253-wall-film, doc-reference-data-f254-particle-wall, doc-reference-data-f255-particle-dispersion, doc-reference-data-f245-tab-breakup]
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: particle wall models, turbulent dispersion and droplet feedback

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (registrations, a preset decided against the recommendation, decision records), 8 ADRs, 2026-11-05 to 2026-11-12.

Two lines extend the Lagrangian particle library ([[lagrangian-particles]], [[rcfd-particles]]). **Walls:** a wall film with the Nusselt thickness and mass conservation (F-253, ADR 0192), then gas buoyancy and gas shear on it (F-292, ADR 0206); erosion and rough-wall rebound (F-254, ADR 0194) with a named steel preset of the erosion constants (ADR 0197). **Carrier coupling:** dispersion from the turbulence model's fields and particle surface reactions (F-255, ADR 0208); droplet evaporation by the d-squared law and TAB breakup (F-245, ADR 0224), their mass, heat and momentum in the exchange record (F-323, ADR 0230), and that feedback given to the carrier in the transient two-way path (F-326, ADR 0348).

## Current state

- **Done:** F-245, F-323, F-326 ([[roadmap-multiphase-and-particles]]). **Partial, every registered check met, waiting for the maintainer's close:** F-253 (ADR 0192 Q1), F-254 (ADR 0194 Q1), F-255 (ADR 0208 Q1), F-292 (ADR 0206 Q1). Decision records of 2026-11-06: "the maintainer closed none of F-253, F-254 and F-259 on 2026-11-06".
- **Maintainer choices of 2026-11-06:** gas buoyancy $(\rho_l - \rho_g)\, g_t$ and a shear-driven film closure became the new item F-292 (ADR 0192 Q2, Q3; the recommendations were different); the erosion constants get a named preset "against the recommendation" (ADR 0194 Q3, built in ADR 0197 as `particles.erosion.material = "dnv_o501_2007_steel"`; explicit `k`, `n`, `density` override it, the report and `particle_erosion.csv` record its name). The shadow-effect distribution of Sommerfeld and Huber (1999) "waits for the paper ... from the maintainer"; until then the realizability truncation W4 stays the model (ADR 0194 Q2).
- **Droplet feedback:** opt-in `Tracker::enable_droplet_exchange()` beside an unchanged `enable_coupling()` (ADR 0230 D1); the parcels are the same bit for bit with or without the record. F-326 gives the vapour species, momentum and heat to the carrier of a case on the transient two-way path; the steady path with the tables stays refused (ADR 0348 Q1), and the vapour is a dilute source at constant density until F-252 (Q2).
- **Not delivered:** splash or rebound at impact (F-284); wall friction and particle rotation; the surface reaction has no case keys; RSM, SA and LES are refused for dispersion (F-255 gap).

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0192](../../docs/adr/0192-f253-registration-particle-wall-film.md) | 2026-11-05 | registration + decision record | F-253 | Particle wall film (Nusselt thickness, mass conservation) | partial; every check met; Q2, Q3 became F-292 | 0206 |
| [0194](../../docs/adr/0194-f254-registration-particle-wall-interaction.md) | 2026-11-06 | registration + decision record | F-254 | Erosion and rough-wall rebound | partial; every check met; shadow effect waits for the paper | 0197 |
| [0197](../../docs/adr/0197-f254-named-steel-preset-of-the-erosion-constants.md) | 2026-11-06 | follow-up (maintainer decision) | F-254 | A named steel preset of the erosion constants | built; steel only (Q2 (a)) | |
| [0206](../../docs/adr/0206-f292-registration-gas-buoyancy-and-gas-shear-in-the-particle-wall-film.md) | 2026-11-06 | registration | F-292 | Gas buoyancy and gas shear in the film | partial; every check met; E2 refuses turbulent carriers | |
| [0208](../../docs/adr/0208-f255-registration-particle-dispersion-from-turbulence-model-fields-and-particle-surface-reactions.md) | 2026-11-07 | registration | F-255 | Dispersion from k-epsilon/SST fields; surface reactions | partial; every check met; reaction in the library only | |
| [0224](../../docs/adr/0224-f245-registration-droplet-evaporation-and-breakup.md) | 2026-11-08 | registration | F-245 | d-squared evaporation and TAB breakup in the library | done; no case-tree wiring (Q2 (b)) | 0230 |
| [0230](../../docs/adr/0230-f323-registration-droplet-feedback-and-exchange-record.md) | 2026-11-12 | registration | F-323 | Droplet mass, heat and momentum in the exchange record | done by the rule; no flow solver consumes the record yet | 0348 |
| [0348](../../docs/adr/0348-f326-registration-droplet-feedback-in-the-transient-two-way-path.md) | 2026-11-12 | registration | F-326 | Droplet feedback in the transient two-way path | done by the rule; no hash table, golden or test edited | |

## Key numbers and limits

- ADR 0192 Q2: gas buoyancy "moves the film thickness by about 4e-4 for water in air".
- ADR 0197 Q2: titanium (`2.0e-9`, `2.6`, 4500) and the two GRP rows are quoted in the docs but are not presets; the GRP rows need the brittle angle function, not delivered.
- ADR 0230 Q2: the normal momentum of a breakup is booked as a residual, "`1.45e-3` of the drag impulse in the spray after 80 breakups".
- ADR 0208 Q2: key `particles.dispersion_fields = "constant" | "turbulence_model"` (opt-in).

## Open questions

- ADR 0192 Q1, ADR 0194 Q1, ADR 0206 Q1, ADR 0208 Q1: marking F-253, F-254, F-292, F-255 done (maintainer).
- ADR 0194 Q2: the Sommerfeld and Huber (1999) shadow-effect distribution (needs the paper).
- ADR 0206 Q2 to Q6, ADR 0208 Q1 to Q10: as named in the roadmap gaps (shear source, counter-current films, RSM/SA/LES, `C_mu` of the eddy length, reaction law, burnt-out parcels, molar masses).
- ADR 0230 Q3: a temperature-dependent `L_v` (with F-252). ADR 0348 Q3, Q4: one record method or two; species undershoot.

## Related

- Predecessors (added by p3-records-a): one-way tracking in the new crate `rcfd-particles` (ADR 0028), the two-way exchange library, discrete random walk and injection options (ADR 0057), and the two-way coupling in the solver loops (ADR 0059); see [[adr-thread-vof-multiphase-particles]].
- [[lagrangian-particles]], [[rcfd-particles]], [[doc-particles]], [[doc-case-schema-particles]], [[k-epsilon]], [[k-omega-sst]]
- [[roadmap-multiphase-and-particles]]
