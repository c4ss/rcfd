---
type: concept
tags: [interface, run]
sources: [doc-run-control, doc-case-schema-other-solver-physics, doc-case-schema-restart-chain-setup-rules, doc-case-schema-particles, doc-case-schema-adaptive-time-stepping, doc-case-schema-mesh-generators, doc-mcp-tools, doc-tutorial-quick-start-cli]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Checkpoint and restart file

A checkpoint (restart file, default `<out>/restart.rcfd`) is "a checksummed `rcfd-io` container holding velocity, pressure, face fluxes, the iteration counter and (with the energy equation) temperature, tied to the mesh by its content hash"; since ADR 0031 it also holds the mesh and, for transient runs, time, step counter and BDF2 history ([[doc-run-control]]).

## How rcfd uses it

- Written when the run is interrupted, hits the time limit or the step cap, at the end, and every `--checkpoint-every n` steps; `output.retain_checkpoints = K` keeps the newest K periodic files ([[doc-run-control]]).
- Available only for steady SIMPLE/SIMPLEC laminar runs (with or without energy) and transient laminar constant-density PISO/PIMPLE runs without energy, scalars, statistics, frozen flow, solids, rheology or user functions; other runs give a `warning` event and `--continue-from` fails with exit 1 ([[doc-run-control]]).
- Not checkpointable either: fractional-step runs, transient runs with particles, `[mesh.adapt]` runs ([[doc-case-schema-other-solver-physics]], [[doc-case-schema-particles]], [[doc-case-schema-mesh-generators]]). Adaptive time-stepping restart files carry the controller state ([[doc-case-schema-adaptive-time-stepping]]).
- Two ways back in: `--continue-from` restores the exact state on the same mesh (a transient restart continues bit for bit); `--restart-from` / `restart.from` transfers the solution onto another mesh (`method` `gradient` default, `nearest`, ...), with a non-zero `max_flux_imbalance` ([[doc-run-control]], [[doc-case-schema-restart-chain-setup-rules]]).
- `chain` stages start from the previous stage's restart file; `run.headless` does not honour `restart.from` ([[doc-case-schema-restart-chain-setup-rules]]).
- Project runs: `start_run` with `continue_from` starts from that run's `output/restart.rcfd` (ADR 0200) ([[doc-mcp-tools]]).

## Related

- [[stop-file]], [[exit-codes]], [[status-stream]], [[run]], [[bit-identical]]
