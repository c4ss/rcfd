---
type: source
tags: [adr-thread, acoustics, wave-equation, duct-modes, fan-noise]
sources: []
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: the linear acoustic wave solver and duct modal export

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (five registrations with result addenda), 5 ADRs, all 2026-11-12.

F-333 (ADR 0351) built `AcousticWaveSolver` in `rcfd-fv/src/acoustic_wave.rs`: the linear acoustic equations about a uniform medium at rest, first order in `p` and `u`, on the polyhedral finite-volume mesh, with an exact linear Riemann flux and the existing `sponge::SpongeLayer`; the modal decomposition and export of hard-walled duct fields is in `rcfd-post` (`duct_modes.rs`). It is "the small-amplitude limit of the Euler equations of `CompressibleSolver`" with two unknowns instead of five, so it carries sound over long ducts cheaply. Follow-ups: a circular duct on an O-grid (F-453), conservation with a non-zero boundary mean (F-473), a drive from a sampled CFD surface (F-452) and the split of returning waves at the sampled section (F-478). All are library only. See [[aeroacoustics]].

## Current state

- **Done:** F-333, F-473, F-452, F-478 (each "library only"). **Partial:** F-453 ([[roadmap-compressible-flow-and-acoustics]]): the counter-rotating `(-1, 0)` content missed, "1.2e-5 against 1e-6 (start-up ramp of the drive)", and mutation MU3 escaped U8 (closed by F-473).
- **CFD coupling:** the wave solver can be driven by an `rcfd-surface-series` file, the format of the FW-H chain (F-452), and with two or more sampled sections the entering and returning waves are split before the drive (F-478, separation threshold `0.05`). See [[fw-h]].
- **Approved follow-ups (2026-11-18, additive golden edits):** the command `rcfd duct-modes` (F-451, ADR 0351 Q1 option (b)); an optional axial-velocity series in the `rcfd-surface-series` format (F-480, ADR 0378 Q1 option (b)).
- **Not wired:** no case section `[acoustics.wave]` (ADR 0351 Q1 (a), "when a fan case needs it"; ADR 0372 Q3).

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0351](../../docs/adr/0351-f333-registration-acoustic-wave-solver-and-modal-export.md) | 2026-11-12 | registration (hub) | F-333 | Linear wave solver, its sponge, duct modal export | done, library only; every mutation caught | 0366, 0372 |
| [0366](../../docs/adr/0366-f453-registration-circular-and-three-dimensional-ducts-through-the-wave-solver.md) | 2026-11-12 | registration (one amendment) | F-453 | Spinning and radial modes of a circular duct on an O-grid, with a matched layer | partial (Rule I); K10 `-2.58e-3`, K01 `-2.56e-3`, S10 `3.6e-5`, S01 `6.5e-5` (limits `1e-2`); SP10, SP10c missed; MU3 escaped U8 | 0371 |
| [0371](../../docs/adr/0371-f473-registration-wave-solver-conservation-with-a-non-zero-boundary-mean.md) | 2026-11-12 | registration | F-473 | Conservation of the integral of `p` with a non-zero boundary mean | done; closes U8's blind spot | |
| [0372](../../docs/adr/0372-f452-registration-wave-solver-driven-by-a-sampled-cfd-surface.md) | 2026-11-12 | registration | F-452 | Drive from a sampled CFD surface | done, library only; entering-wave assumption kept (Q1 (b)) | 0378 |
| [0378](../../docs/adr/0378-f478-registration-wave-drive-with-returning-waves-at-the-sampled-section.md) | 2026-11-12 | registration | F-478 | Returning waves at the sampled section | done, library only | |

## Key numbers and limits

- ADR 0366 D1: new generator `rcfd_mesh::gen::circular_duct_ogrid` (Cartesian core $[-a, a]^2$ plus four ring blocks, extruded along `z`; every face planar).
- ADR 0366 Q1: the SP checks stay a known miss (recommendation (c)); a clean counter-rotating amplitude only "if a later item needs ... a fan's modal content".
- ADR 0378 Q1: one section of pressure "carries no information on the split", so the residual is measured against a second section (`entering_residual`).

## Open questions

- ADR 0351 Q1 (a) / ADR 0372 Q3 / ADR 0378 Q3: a case key for the wave solver and its drive (needs approval).
- ADR 0371 Q2: a conservation check for a driven patch (recommendation none now).
- ADR 0372 Q2: name the face farthest from the plane in the off-plane error (one-line change for the wiring item).

## Related

- [[aeroacoustics]], [[fw-h]], [[doc-acoustics]], [[doc-reactor-and-acoustics]], [[rcfd-post]], [[rcfd-mesh]]
- [[adr-thread-compressible-acoustic-boundaries]], [[roadmap-compressible-flow-and-acoustics]]
