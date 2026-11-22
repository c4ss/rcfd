---
type: source
tags: [reference-data, wave-section]
sources: []
origin: docs/reference-data.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Reference data: Wave 17C GUI, assistant and renderer (F-143, F-144, F-149, F-150, F-073, ADR 0025)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Wave 17C GUI, assistant and renderer (F-143, F-144, F-149, F-150, F-073, ADR 0025)" (lines 705-741) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Wave 17C GUI, assistant and renderer references (ADR 0025), with wave 17A known misses.

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


Wave 17C introduces no physical reference value. The checks use these conventions, all from memory and unverified as citations:

- Rec. 709 luma weights (0.2126, 0.7152, 0.0722) for the colour-map luminance test.
- The "1-2-5" family (with 2.5) of tick steps.
- Min/max bucket decimation for plot data.

The OpenAI-compatible wire format (`POST /v1/chat/completions`, `GET /v1/models`, SSE `data:` events with `[DONE]`, `Authorization: Bearer`) and the default local ports (Ollama 11434, LM Studio 1234, llama.cpp 8080, vLLM 8000) are recalled from memory and unverified against current documentation. They were checked only against the loopback mock server written for the tests. That server shares the author's understanding of the format, so agreement is not independent evidence about any real server. The assistant crate and these tests were removed with the local-model assistant (F-838); the paragraph stays as the record of what they were checked against.

The lid-driven cavity logic check (top-row mean ux, return flow) is a sanity bound, not a benchmark comparison.

Wave 17A (ADR 0023): convergence floor diagnosis and pseudo-transient control. Known misses added (limits as registered, never widened; ignored tests):

| Item | Limit | Measured | Test |
| --- | --- | --- | --- |
| F-106 `ser` controller, cavity Re 1000 / step Re_h 400 | residual 1e-6 | not converged (cavity 0.18 after 6000 its, scale at cap) | `rcfd-fv/tests/pseudo_control.rs::*_known_miss` |
| F-106 `ser2` controller (symmetric), same cases | residual 1e-6 | cavity 1.6e-2, step 7.7e-5 | `pseudo_control.rs::*_ser2_known_miss` |
| F-106 controller on C2/D1/D2 (local cfl 1, physical 0.2) | converge, registered accuracy | residual 0.21 or NaN | `rcfd-api/tests/convergence_wave17a.rs::f?_*_known_miss`, `f?b_*_known_miss` |
| F-209 error-controlled stepping, linear decay (relative and absolute scale) | steps <= 0.8 x uniform | 81 vs 76; 46 vs 36 | `rcfd-fv/src/ddt.rs::e1*_known_miss` |

Findings:

- The F-200 C2/D1/D2/E2 residual targets are met with `solver.residual_norm = scaled` (new passing tests F1c, F2c, F3c, F4); the reference normalisation stalled by construction.
- The F-061 nozzle 1e-8 residual floor (2.619e-5) stays undiagnosed.
- Wave-13 D5b n_r = 128 is slow convergence.

No external reference data were used in wave 17A. The Taylor-Green and flat-plate acceptance items of F-209 were not run.


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
