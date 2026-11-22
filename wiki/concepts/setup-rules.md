---
type: concept
tags: [interface, case, validation]
sources: [doc-case-schema-restart-chain-setup-rules, doc-run-control, doc-tutorial-gallery, doc-case-schema-mesh-generators, doc-case-schema-periodic-gradient]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Setup rules (SV rules)

Data-driven checks of a resolved case and its mesh quality, evaluated by `rcfd check`, `rcfd validate`, `rcfd run` and `--check-only` before anything is solved (F-240, ADR 0031) ([[doc-case-schema-restart-chain-setup-rules]]).

## How rcfd uses it

- Rule file `crates/rcfd-api/rules/setup_rules.json`, format `rcfd-setup-rules` version 1; 21 rules: SV001-SV013 errors, SV101-SV105 warnings, SV201-SV203 infos ([[doc-case-schema-restart-chain-setup-rules]]).
- Each diagnostic has `id`, `severity`, `category`, a located `path`, `message` and sometimes a suggested `fix` ("never applied") ([[doc-case-schema-restart-chain-setup-rules]]).
- Errors stop the run with exit 3; warnings go to stderr and `warning` status events with the rule id ([[doc-run-control]]).
- Computed rules needing a built simulation: SV301 (pre-run pressure-lag estimate, warn at 1.3 or more per step, fix `solver.corrector_control = "adaptive"`, ADR 0156, ADR 0172) and SV302 (`linear_upwind` without limiter on wall cells with aspect ratio x tan(non-orthogonality) of 3000 or more, ADR 0172) ([[doc-case-schema-restart-chain-setup-rules]]).
- Example: `axial_fan_cgrid.toml` triggers SV101 (maximum non-orthogonality over 70 degrees) ([[doc-tutorial-gallery]]).
- A related warning id outside the SV series: `MT101` for settings left in world coordinates under `[mesh.transform]` ([[doc-case-schema-mesh-generators]]).

## Related

- [[located-error]], [[exit-codes]], [[status-stream]], [[case-file]]
