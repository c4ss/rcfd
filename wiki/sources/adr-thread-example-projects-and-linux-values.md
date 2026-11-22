---
type: source
tags: [adr-thread, example-projects, studio, linux, platform-values, mesh-transform, naca0012, fan-01]
sources: [adr-thread-wave48-egui-studio, adr-thread-naca0012-sa-drag]
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: the shipped example projects, the Linux-built trees and the rotated aerofoil cases (wave 48H to 49M)

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (registrations with result addenda), 6 ADRs, 2026-10-28 to 2026-10-30; none has an item in its file name.

The Studio showed nothing on first open, "because a new project is empty". ADR 0142 (48H) added two finished example projects, `wind-tunnel` and `quick-start`, under `example-projects/`, with committed runs, a regeneration script and a staleness test. The first Linux gate then failed on them, because the cylinder generator's last bits differ per platform: the maintainer chose "ship Linux-built stores too, not a tolerance and not a known miss" (ADR 0150), a second tree `example-projects-linux/` built by the same script in Docker. ADR 0155 (48U) added `[mesh.transform]` and two rotated NACA 0012 cases, ADR 0157 (48W) merged them into both trees, and ADR 0176 (49M) added the 3D project `axial-fan`. The Linux expected values of the gate itself are ADR 0137 (on [[adr-thread-daily-gate-and-edit-lock]]). See [[example-projects]].

## Current state

- **Three projects in two trees** at 2e94e5ef: `example-projects/` and `example-projects-linux/`, each with `wind-tunnel/`, `quick-start/` and `axial-fan/` and a `manifest.json` that "lists the committed runs and the size caps" ([example-projects/README.md](../../example-projects/README.md)). "No result in these folders is a validation: the meshes are coarse on purpose" (same README). `scripts/make_example_projects.py` builds, `--check` checks and `--rerun-check` rebuilds into a temporary copy and compares the reproducible files byte for byte.
- **The aerofoil runs converge since ADR 0169.** The 8 stored NACA 0012 runs were regenerated under the new far-field default and "converge (409 / 230 iterations, exit 0)", so the aerofoil parts of ADR 0142 G1, ADR 0155 E2 and ADR 0148 N9 are ignored known misses with their limits intact ([[adr-thread-naca0012-sa-drag]]). E2 is `e2_rotated_mesh_matches_rotated_flow_known_miss`; the two forms "agree within 5.3e-9 at the final iteration" (ADR 0155 note).
- **The 3D example is first order.** `axial-fan` is FAN-01 passage B3 of ADR 0110 with `upwind`, "because the second-order run does not end (limit cycle)"; torque 1.4023 N m, 35 % below the measured 2.142 N m, kept as "for viewing the 3D geometry and flow pattern only" (ADR 0176). See [[fan-01]].
- **Size margin is gone:** after ADR 0157 "The Linux wind-tunnel is 176 667 bytes under its cap and the Windows one 173 707; the next aerofoil run in either tree does not fit." ADR 0176 question 2 (the 6 MB per-tree cap of `axial-fan`) is open for the maintainer.
- **The only real cross-platform check was inferred:** ADR 0150's CI `Full gate` on `ubuntu-latest` "was not run on this job's commits (no push): only the cylinder mesh hash was compared with a value CI itself printed, and that CI agrees for the other eight runs is inferred from their meshes".

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0142](../../docs/adr/0142-wave48h-example-projects.md) | 2026-10-28 | registration | | Two finished example projects that open straight to a picture | every limit met except two predicted known misses: E1 (2 aerofoil cases in the case editor) and G3 (Re 100 cylinder, 6 upward crossings, limit 7); found, not fixed: the end-time remainder step | 0145 (remainder step), 0169 (G1) |
| [0150](../../docs/adr/0150-job48p-linux-example-stores.md) | 2026-10-28 | registration | | Linux-built example stores in a second tree, same script in Docker | every limit met (P1 after amendment 1); Linux G3 the same ignored known miss; both trees about 63 MB, the job adds 11 343 206 bytes packed | 0157 |
| [0155](../../docs/adr/0155-job48u-rotate-the-mesh-instead-of-the-flow.md) | 2026-10-28 | registration | | `[mesh.transform]` (rotation about z) and two rotated NACA 0012 cases | every limit met; H1 0 of 41 sources differ; E2 9.5e-8 at iteration 2000 (limit 1e-6); E2 later a known miss (ADR 0169); nine questions open | 0157, 0163 (H1) |
| [0157](../../docs/adr/0157-job48w-merge-48u-rotated-cases-with-the-linux-tree.md) | 2026-10-29 | registration (merge) | | Merge 48U with 48P's per-platform trees; Linux runs of the rotated cases | every limit met except S1 (one pre-existing timing failure, ADR 0133 R1); S1r 250 / 0 / 7 | |
| [0163](../../docs/adr/0163-job48ac-case-hashes-h1-after-the-corrector-control-key.md) | 2026-10-29 | registration | | ADR 0155's H1 after the new key `solver.corrector_control`; `rcfd-mcp` `t2` its own temp parent | L1, L2, L3 met; L4, L6, L7 not run and L5 incomplete (low-memory reaper) | |
| [0176](../../docs/adr/0176-job49m-fan01-blade-passage-as-a-3d-example-project.md) | 2026-10-30 | registration | F-260 | FAN-01 blade passage as the 3D example `axial-fan` | P5 5 % part missed (1.4023 N m, +5.04 % against 1.335 N m), ignored known miss; P1 `finished`, `not_converged` at 1500, exit 5 | |

## Key numbers and limits

- **ADR 0142:** 10 committed runs; "the aerofoil runs end at their iteration cap and give a lift slope about 30 % below the slope of Ladson's data as the repository reads it"; D8's shedding window amended before re-processing (the runs were not repeated).
- **ADR 0150:** the failing Linux test was a mesh mismatch: "the cylinder's generator places its points with values whose last bits differ per platform, so the mesh Linux builds is not the mesh the store holds". `example-projects/` "stays byte for byte as it is".
- **ADR 0155:** "No other setting of the case is rotated, and settings that place something relative to the geometry get the warning MT101 when the rotation is not 0." Near iteration 20 `cl` differs by up to 6.5e-4 between the two forms; "the likely cause, the per-component inner solves, was not investigated". The schema documents both forms "at 4 and 2 degrees" in `example-projects/wind-tunnel` ([[doc-case-schema]]).
- **ADR 0157:** the Linux tree took the two new runs from `--docker --add-missing` (D3 (a)); "R1L shows the plain rebuild gives the same reproducible bytes".
- **ADR 0163:** H1 compares after "a documented normalisation of that one new default-valued key and keeps R0 as the only record"; the written text of every recorded case differs from 6867e40 "by exactly that key".
- **ADR 0176:** the per-tree cap of `axial-fan` is 6 MB (question 2).

## Open questions

- ADR 0155: none of its nine questions is decided in the addendum (among them `CHANGELOG.md`, question 6, cited by ADR 0159 question 8).
- ADR 0157: the regeneration command, S1's miss and the size margin.
- ADR 0163 Q1: two failures in the partial L5 run, outside the job's files.
- ADR 0176 question 2: the 6 MB per-tree cap.
- ADR 0169 question 3 (from [[adr-thread-naca0012-sa-drag]]): the case comment on `max_iterations` in the 4 aerofoil cases "is now false".

## Related

- [[example-projects]], [[studio]], [[naca0012]], [[cylinder-re100]], [[fan-01]], [[known-miss]], [[bit-identical]]
- Threads: [[adr-thread-wave48-egui-studio]], [[adr-thread-naca0012-sa-drag]], [[adr-thread-daily-gate-and-edit-lock]] (ADR 0137, the Linux expected values), [[adr-thread-fan01-rotating-solve]]
- Areas: [[roadmap-studio-desktop-app]], [[roadmap-quality-docs-and-validation]]
