---
type: source
tags: [adr-thread, acoustics, wave-equation, duct-modes, fan-noise]
sources: [doc-acoustics, doc-roadmap-json]
origin: docs/adr/ (ADR list below)
origin_commit: 709e302e
created: 2026-11-22
updated: 2026-11-24
status: draft
---

# ADR thread: the linear acoustic wave solver and duct modal export

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 709e302e (first written at 2e94e5ef; ADR 0783 added at 709e302e, the five earlier ADRs unchanged since).
**Kind:** ADR thread (six registrations with result addenda), 6 ADRs: five of 2026-11-12 and ADR 0783 of 2026-11-23.

F-333 (ADR 0351) built `AcousticWaveSolver` in `rcfd-fv/src/acoustic_wave.rs`: the linear acoustic equations about a uniform medium at rest, first order in `p` and `u`, on the polyhedral finite-volume mesh, with an exact linear Riemann flux and the existing `sponge::SpongeLayer`; the modal decomposition and export of hard-walled duct fields is in `rcfd-post` (`duct_modes.rs`). It is "the small-amplitude limit of the Euler equations of `CompressibleSolver`" with two unknowns instead of five, so it carries sound over long ducts cheaply. Follow-ups: a circular duct on an O-grid (F-453), conservation with a non-zero boundary mean (F-473), a drive from a sampled CFD surface (F-452) and the split of returning waves at the sampled section (F-478). ~~All are library only.~~ The wave solver and its drive are library only; the modal analysis of saved sections is reachable from the CLI since F-451 (`rcfd duct-modes`, ADR 0783). See [[aeroacoustics]].

## Current state

- **`rcfd duct-modes`** (F-451 done, ADR 0783, merged at 01726464 and pushed at 9c1d3817 on 2026-11-24): the command reads one `rcfd-surface-series` file per duct section and writes the `rcfd-duct-modes` export; one file gives the export of kind `section`, two or more the axial split into the waves towards `+x` and `-x` (kind `axial`). "The library needs no change": `crates/rcfd-post/src/duct_modes.rs` is not edited, the command is `crates/rcfd-cli/src/cmd_duct_modes.rs`. On the seven sections of the X1 field of ADR 0351 the export equals the library's text byte for byte (0 of 3337 as a file, 0 of 3338 on standard output, 0 of 3336 with another reference position; one section 0 of 2091). A face outside the duct and a series cut to 127 or 96 of its 128 samples are refused with exit 3, the file, the JSON path and the registered text. 5 of 5 mutations caught. Usage and limits: [[doc-acoustics]]; see [[rcfd-cli]], [[duct-acoustic-modes]].
- **Done:** F-333, F-473, F-452, F-478 (each "library only"). **Partial:** F-453 ([[roadmap-compressible-flow-and-acoustics]]): the counter-rotating `(-1, 0)` content missed, "1.2e-5 against 1e-6 (start-up ramp of the drive)", and mutation MU3 escaped U8 (closed by F-473).
- **CFD coupling:** the wave solver can be driven by an `rcfd-surface-series` file, the format of the FW-H chain (F-452), and with two or more sampled sections the entering and returning waves are split before the drive (F-478, separation threshold `0.05`). See [[fw-h]].
- **Approved follow-ups (2026-11-18, additive golden edits):** ~~the command `rcfd duct-modes` (F-451, ADR 0351 Q1 option (b))~~ (built, the bullet above); an optional axial-velocity series in the `rcfd-surface-series` format (F-480, ADR 0378 Q1 option (b)).
- **Not wired:** no case section `[acoustics.wave]` (ADR 0351 Q1 (a), "when a fan case needs it"; ADR 0372 Q3).
- ~~F-451 (`rcfd duct-modes`) was issued on 2026-11-23 by the brief `docs/adr/briefs/f451-duct-modes-command.md` (ADR 0783); its registration is not on main at 09647baa and the item is todo~~ (done at 709e302e, [[doc-roadmap-json]]).

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0351](../../docs/adr/0351-f333-registration-acoustic-wave-solver-and-modal-export.md) | 2026-11-12 | registration (hub) | F-333 | Linear wave solver, its sponge, duct modal export | done, library only; every mutation caught | 0366, 0372 |
| [0366](../../docs/adr/0366-f453-registration-circular-and-three-dimensional-ducts-through-the-wave-solver.md) | 2026-11-12 | registration (one amendment) | F-453 | Spinning and radial modes of a circular duct on an O-grid, with a matched layer | partial (Rule I); K10 `-2.58e-3`, K01 `-2.56e-3`, S10 `3.6e-5`, S01 `6.5e-5` (limits `1e-2`); SP10, SP10c missed; MU3 escaped U8 | 0371 |
| [0371](../../docs/adr/0371-f473-registration-wave-solver-conservation-with-a-non-zero-boundary-mean.md) | 2026-11-12 | registration | F-473 | Conservation of the integral of `p` with a non-zero boundary mean | done; closes U8's blind spot | |
| [0372](../../docs/adr/0372-f452-registration-wave-solver-driven-by-a-sampled-cfd-surface.md) | 2026-11-12 | registration | F-452 | Drive from a sampled CFD surface | done, library only; entering-wave assumption kept (Q1 (b)) | 0378 |
| [0378](../../docs/adr/0378-f478-registration-wave-drive-with-returning-waves-at-the-sampled-section.md) | 2026-11-12 | registration | F-478 | Returning waves at the sampled section | done, library only | |
| [0783](../../docs/adr/0783-f451-registration-the-rcfd-duct-modes-command.md) | 2026-11-23 | registration, addendum 1 results, the lead's reading (2026-11-24) | F-451 | The `rcfd duct-modes` command: arguments, output, the bit-for-bit check against the library, two located errors | done; no differing byte in four comparisons; exit 3 with the registered texts; the capability inventory re-recorded by the lead | |

## Key numbers and limits

- ADR 0783, the refusals (exit 3, `<path>: invalid value at '<JSON path>': <reason>`, nothing written): a face is outside the duct beyond `1e-6` of the duct size ($L$ is $\max(a, b)$ or the radius); the periods covered are not whole beyond `1e-6` of their number; `rho0` or `c0` differing from the first file's by more than `1e-9` relative; a face normal with $|n_x| < 1 - 10^{-6}$; a uniform flow, a rotating surface, faces of one file not on one plane, more modes than faces. Exit codes as `rcfd fwh`: 0, 1 I/O, 2 usage, 3 invalid input.
- ADR 0783, the modes fitted: all with a cut-off wavenumber up to `--k-max`, by default up to $k = 2\pi f/c_0$ (the propagating modes); the axial wavenumber is the closed form, not fitted.
- ADR 0783, the "CLI help golden" of the acceptance: "No file under `crates/rcfd-cli/tests` holds the help text"; the file that records the help is the Studio's `apps/studio-web/capabilities/inventory.json`. The help gains six lines; by the repository's parser `rcfd --help` goes from 210 to 217 entries, "7 added, 0 removed, 0 changed" (one command, six options). The agent edited none of the three files (rule 3); the lead regenerated the inventory and mapped the command in `coverage.json` under F-809, unplanned, in the merge commit ([[doc-studio-coverage]], [[adr-thread-studio-electron]]).
- ADR 0366 D1: new generator `rcfd_mesh::gen::circular_duct_ogrid` (Cartesian core $[-a, a]^2$ plus four ring blocks, extruded along `z`; every face planar).
- ADR 0366 Q1: the SP checks stay a known miss (recommendation (c)); a clean counter-rotating amplitude only "if a later item needs ... a fan's modal content".
- ADR 0378 Q1: one section of pressure "carries no information on the split", so the residual is measured against a second section (`entering_residual`).

## Open questions

- ADR 0783 has no open question. Not covered by it: a duct axis other than `+x`, a pressure table in a CSV file (`pressure_csv`), more than one frequency per run, a fitted axial wavenumber, a medium with flow, an MCP tool or a Studio control for the command, a case key.
- ADR 0351 Q1 (a) / ADR 0372 Q3 / ADR 0378 Q3: a case key for the wave solver and its drive (needs approval).
- ADR 0371 Q2: a conservation check for a driven patch (recommendation none now).
- ADR 0372 Q2: name the face farthest from the plane in the off-plane error (one-line change for the wiring item).

## Related

- [[aeroacoustics]], [[fw-h]], [[doc-acoustics]], [[doc-reactor-and-acoustics]], [[rcfd-post]], [[rcfd-mesh]], [[rcfd-cli]], [[duct-acoustic-modes]]
- [[adr-thread-compressible-acoustic-boundaries]], [[roadmap-compressible-flow-and-acoustics]]
