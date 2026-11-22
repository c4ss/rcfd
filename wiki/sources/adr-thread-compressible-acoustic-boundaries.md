---
type: source
tags: [adr-thread, compressible, acoustics, impedance, sponge-layer, ale]
sources: []
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: acoustic boundaries and sponge layers of the compressible solver

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (six registrations with result addenda), 6 ADRs, all 2026-11-12.

F-243's acceptance asked for an impedance boundary of `CompressibleSolver`; its title also named a wave-equation solver with sponge layers and modal export, which became items of their own (see [[adr-thread-acoustic-wave-solver]]). This thread builds, in the library: the real impedance boundary `CompBc::Impedance { p, z }` (F-243, ADR 0233), an opt-in sponge with a plain and a matched form (F-331, ADR 0237), a time-domain mass-spring-damper wall `MsdWall { p, r, m, k }` (F-332, ADR 0243), a matched form with memory decay, damping along the layer and corners (F-340, ADR 0248), and the sponge on moving meshes (F-341, ADR 0265; F-407, ADR 0344). Each meets its acceptance in the library; none can yet be selected from a case file. See [[aeroacoustics]], [[doc-acoustics]].

## Current state

- **Library only.** F-243, F-331, F-332 are partial because "Case-schema wiring is not done (a new key needs an approved edit of case_hashes_w48u.rs)"; the wiring item is F-330. The maintainer approved F-330's case keys on 2026-11-18 "under the class rule for additive golden edits" (ADR 0233 answers). The MSD wall's wiring is recommended as "an item of its own after F-330" (ADR 0243 Q1 (b)).
- **Done:** F-341 (sponge on moving meshes, opt-in `SpongeFrame::AttachedToMesh`) and, by its own proposal, F-407 (`SpongeFrame::AttachedWithLabMemory`); the roadmap holds F-407 partial ("Registered check G3 missed (8.92e-3 against 1e-12) because of a set-up defect, and it passed only as G3b after amendment 1").
- **F-340 partial** "for good" unless the maintainer replaces its clause 1: set-up D "stays at 3.7e-2, 6.1e-2, 8.6e-2 at 30, 45, 60 degrees with the shift ... against 1e-2, and no matched layer damps it" (roadmap gap).
- **Defaults held** (maintainer, 2026-11-18): `reflection_value` of `nonreflecting` stays `0.02` (ADR 0233 Q3: it "reflects 46 percent of a short pulse", ADR 0006 known miss); the default `shift` in the case wiring stays open, recommendation `0.05 c/L` (ADR 0248 Q2); the lab-frame memory as the behaviour of `AttachedToMesh` for the matched forms (F-446) is held.
- **Documented limits:** the F-341 attached memory is unstable with a mesh motion along the layer (ADR 0344); use the plain form on rotor meshes until an oblique and a tangential-motion check exist (ADR 0265 Q3).

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0233](../../docs/adr/0233-f243-registration-impedance-and-quiet-boundary.md) | 2026-11-12 | registration | F-243 | Impedance and quiet boundary against closed forms | partial; acceptance met in the library; wiring open (approved 2026-11-18 as F-330) | 0237, 0243 |
| [0237](../../docs/adr/0237-f331-registration-sponge-layer-of-the-compressible-solver.md) | 2026-11-12 | registration (one amendment) | F-331 | Sponge layer, plain and matched, 1 percent at 0, 30, 45, 60 degrees | partial; A0 `9.2e-7`, A30b to A60b `1.2e-4`, `2.8e-4`, `1.2e-3`; A30 to A60 defective set-up | 0248, 0265 |
| [0243](../../docs/adr/0243-f332-registration-mass-spring-damper-wall.md) | 2026-11-12 | registration | F-332 | Mass-spring-damper wall, reflection in magnitude and phase | partial; worst `2.9e-4` magnitude (limit `2e-2`), `2.4e-4` rad phase (limit `1.93e-2`); wiring open | |
| [0248](../../docs/adr/0248-f340-registration-matched-sponge-shift-corner-and-damping-along-the-layer.md) | 2026-11-12 | registration | F-340 | Memory decay (shift), corner of two layers, damping along the layer | partial; clauses 2 to 5 met (isolated pulse at 60 degrees `8.1e-4`); clause 1 missed, as expected (DR12) | |
| [0265](../../docs/adr/0265-f341-registration-sponge-layer-on-moving-meshes.md) | 2026-11-12 | registration (one amendment) | F-341 | Sponge in the ALE step, frame of the layer | done; U1 at `3e-15`; T0 `9.6e-7` to `9.8e-7`; NCr a known miss, NCr2 not blind | 0344 |
| [0344](../../docs/adr/0344-f407-registration-matched-sponge-moving-along-the-layer-and-rotating.md) | 2026-11-12 | registration (one amendment) | F-407 | Matched sponge moving along the layer; plain sponge on a rotating mesh | proposed done (roadmap partial); LR30, LR45 `1.6e-4`, `3.5e-4` (limit `1e-2`); attached frame `7.2e-3`, `3.2e-2`; RR `2.8e-3` | |

## Key numbers and limits

- ADR 0233 D2: `z == 0.0` returns `pressure_outlet` bit for bit; a supersonic outflow returns the interior state.
- ADR 0237 D1: plain form $S = -\mathrm{rate}\,(U - U_{ref})$ on the five conserved variables; matched form only for a reference at rest. Recommended keys `sponge_thickness`, `sponge_form`, `sponge_reflection` (default `1e-6`) (Q1 (a)).
- ADR 0243 Q3: oblique incidence not checked; closed form $(Z\cos - \rho c)/(Z\cos + \rho c)$.
- ADR 0248 Q1: hiding the ringing would need "about twice the rate integral of the D7 layer, so about 80 cells".

## Open questions

- ADR 0237 Q2: 60 degrees with an isolated pulse (accept A60b with X1 and X45, or one half-space run of "about 3 hours").
- ADR 0243 Q2: the wall together with a sponge (refused now; merge the step paths inside the wiring item).
- ADR 0248 Q1: keep clause 1 (F-340 partial for good), replace it, or drop it.
- ADR 0265 Q1 / ADR 0344 Q2: a layer fixed in space, "only when a case needs it". ADR 0265 Q2: the frame key (`frame = "mesh"`) with the sponge's wiring.

## Related

- Predecessor (added by p3-records-a): the first characteristic boundaries of the density-based Euler solver (`TotalInlet`, `PressureOutlet`, `NonReflecting`, `FarField`) are ADR 0006 (F-062), where "Only normally incident waves are treated as non-reflecting; oblique incidence reflects"; see [[adr-thread-early-physics-models]].
- [[aeroacoustics]], [[doc-acoustics]], [[doc-case-schema-compressible]], [[doc-reference-data-compressible-bcs]], [[dynamic-mesh]]
- [[adr-thread-acoustic-wave-solver]], [[adr-thread-wet-steam]], [[roadmap-compressible-flow-and-acoustics]]
