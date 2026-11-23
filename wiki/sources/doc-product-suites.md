---
type: source
tags: [product, backlog, clean-room]
sources: []
origin: docs/product-suites.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: stable
---

# Product suites backlog: vendor capabilities outside CFD (2026-10-17)

**Origin:** [docs/product-suites.md](../../docs/product-suites.md) at `origin/main` 2e94e5ef
**Kind:** backlog log ("a log, not a plan")

## Summary

Logs one commercial vendor catalogue's capabilities outside CFD, one entry per suite (S-01 to S-32, categories A to J), for whoever triages work beyond CFD. "Status: first-pass backlog, 2026-10-17. This is a log, not a plan. Nothing here is scheduled, nothing is added to `docs/roadmap.json`." The CFD-class inventory lives in `docs/parity-matrix.md` (F-244 to F-267). The common prerequisites are FE element infrastructure with the mesh-container extension, the material schema and the co-simulation interface.

## Key points

- **Clean-room statement:** capability names are "short paraphrases of public product pages and of one documentation contents page from the vendor's online help (P21, which normally needs a login)"; `docs/provenance.md` says what was consulted. "No vendor text or code was copied; no vendor binary or install was inspected." Candidate algorithms are public textbook or published-paper methods ([[clean-room]], [[provenance]]).
- **Sources:** 20 vendor product pages P1-P20 fetched with WebFetch ("a model-condensed extraction of the page, not raw HTML"); P21, the vendor online help's theory reference, "chapter and section headings only, not the theory text"; three WebSearch snippet sets Q1-Q3; seven fetches failed (404), so those areas rest on general knowledge, marked GK.
- **Categories:** A structures (S-01 to S-11), B electronics and electromagnetics (S-12 to S-16), C optics and photonics (S-17, S-18), D acoustics and NVH (S-19), E particle and granular (S-20), F materials (S-21, S-22), G systems, embedded software, digital twin, safety (S-23 to S-26), H geometry, meshing, pre/post (S-27 to S-29), I platform (S-30 to S-32), J other.
- Each entry: purpose, capabilities, sources read, CFD overlap and dependencies, size and priority class (P2 natural next step, P3 substantial standalone, P4 long-term), open route, first slice.
- **S-01 structural mechanics** is P2, "the one suite with an already-registered CFD hook, F-260"; first slice: small-strain linear-elastic tet/hex elements, patch tests and a clamped-plate deflection benchmark. On 2026-10-17 the page treats the structural solver as intent (F-260); [[rcfd-fem]] was built later.
- **Honest coverage statement:** P1 counts as names only; the algorithm and standard names under "Open route" are general knowledge, not read in this pass; priority and size classes "are judgement, not measured effort"; "Nothing from any vendor install or binary was touched."

## Contradictions with the wiki

- Tension, not a contradiction: [[doc-contributing]] and `AGENTS.md` say vendor documentation is never a source. This page reads one vendor documentation contents page (P21, headings only) to name capabilities, not to derive algorithms, which guardrail G1's line "Knowing that a feature exists is fine; copying how it looks or what it is called is not" ([[studio-guardrails]]) covers for the Studio. Recorded for the maintainer's eye; the repo docs do not call it a breach.

## Pages this source informs

- [[clean-room]]
- [[provenance]]
- [[rcfd-fem]]
