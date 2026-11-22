---
type: source
tags: [interface, case, run]
sources: [doc-case-schema]
origin: docs/case-schema.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Case schema: restart, chain, setup rules (wave 19C)

**Origin:** [docs/case-schema.md](../../docs/case-schema.md), section "Wave 19C: `restart`, `chain`, setup rules (F-115, F-241, F-240, ADR 0031)", at `origin/main` 2e94e5ef. Hub: [[doc-case-schema]]
**Kind:** reference section

## Summary

Starting a run from a restart file, chaining stages, and the data-driven setup rules evaluated before anything is solved.

## Key points

- `restart.from` (`""` = fresh start), `source_case`, `method` (`gradient` default, `nearest`, `inverse_distance`, `least_squares`, `conservative`, `conservative_overlap`). Same mesh: exact restore; other mesh: velocity and pressure transferred, flux rebuilt, `max_flux_imbalance` reported ("not zero").
- `rcfd run` and the chain honour `restart.from`; "the service method `run.headless` does not".
- `chain.sequence` + `chain.stages.<name>.set` (`"dotted.path=<JSON value>"`), `restart` `auto`/`previous`/`none`, `allow_unconverged` (exit 5 accepted). Results go to `<out>/<stage>/`; `--no-chain` runs the base case.
- Setup rules (F-240): `crates/rcfd-api/rules/setup_rules.json`, format `rcfd-setup-rules` version 1; 21 rules (SV001-SV013 errors, SV101-SV105 warnings, SV201-SV203 infos). Errors stop the run with exit 3; warnings go to stderr and `warning` events.
- Computed rules: **SV301** (ADR 0156; warning at an estimated lag amplification of 1.3 or more per step; in ADR 0154's runs every clean set-up was at most 1.166, every spoky or diverged one at least 2.418) and **SV302** (ADR 0172; `linear_upwind` without a limiter on wall cells with `aspect ratio x tan(non-orthogonality)` of 3000 or more).

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[setup-rules]], [[checkpoint]], [[exit-codes]]
