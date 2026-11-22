---
type: source
tags: [tutorial, cli]
sources: []
origin: docs/tutorials/quick-start-cli.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Tutorial: quick start, the rcfd command line

**Origin:** [docs/tutorials/quick-start-cli.md](../../docs/tutorials/quick-start-cli.md) at `origin/main` 2e94e5ef
**Kind:** tutorial (executed by tests)

## Summary

Build `rcfd`, write a 16 x 16 lid-driven cavity case, validate it, check the mesh and set-up, run it, post-process. "Every command on this page is executed by `cargo test -p rcfd-cli --test tutorials`."

## Key points

- Build: `cargo build --release -p rcfd-cli`; binary `target/release/rcfd`.
- `rcfd --help` lists the exit codes: 0 success, 2 usage error, 3 invalid case, 4 mesh check failed, 5 not converged, 6 diverged, 7 interrupted, 8 wall-clock limit ([[exit-codes]]; 1 is not in this list).
- `rcfd validate` checks keys and values without building; a misspelt key gives `line 8: reports.lid_drag.patches: unknown setting 'patches'` and exit 3 ([[located-error]]).
- `rcfd check` builds the mesh, checks quality and the setup rules without solving (`my_cavity.toml: ok, 256 cells`).
- `rcfd run ... --out results` writes `solution.vtu`, `monitors.json`/`.csv`, `report.html` and the checkpoint `restart.rcfd`; `--set physics.nu=0.02` overrides one setting.
- `rcfd post`, `rcfd sample`, `rcfd schema --out schema.json`.
- Fence tags: `file=`, `run` (with `exit=<n>`), `output`, `run-slow`, `not-executed`; "An untagged block fails the test" (ADR 0089).

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[executable-tutorials]], [[rcfd-cli]], [[case-file]], [[exit-codes]], [[checkpoint]]
