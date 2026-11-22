---
type: source
tags: [roadmap, parity, clean-room]
sources: []
origin: docs/parity-matrix.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Parity matrix: commercial CFD capabilities versus the rcfd roadmap (2026-10-17)

**Origin:** [docs/parity-matrix.md](../../docs/parity-matrix.md) at origin/main 2e94e5ef
**Kind:** repo doc: first-pass capability inventory dated 2026-10-17, with a ranked gap list and candidate roadmap ids.

## Summary

Compares the capability lists of two commercial CFD products with the rcfd roadmap, one row per capability, and ranks the 25 largest gaps. At its date the roadmap had 158 items: 49 done (31 %), 92 partial (58 %) and 17 todo (11 %). The three top gaps were distributed-memory parallelism (MPI, F-100 todo), Eulerian-Eulerian and mixture multiphase with cavitation (F-127 todo), and mixing-plane and frozen-rotor interfaces (F-126 todo). Statuses are read mechanically from `docs/roadmap.json`; "partial does not mean half built".

## Key points

- Sources read: S1, S2 (public product pages), S3 to S10 (vendor online help, login required, contents and introduction pages), all through WebFetch, "which returns a model-condensed extraction of the page"; GK = general knowledge, unverified. Search snippets are not cited as read sources. See [[doc-provenance]] items 1 and 2.
- Coverage: 213 rows; 194 cite at least one read source, 16 are general knowledge only, 1 (assistant workflows) is rcfd-specific; two rows mix S and GK.
- Descriptions, priorities (P1/P2/P3) and gap judgements are the author's; "Priority labels are advisory and should be reviewed by the user before wave planning."
- Ranked gap 21, "Parametric studies and DOE/optimisation", was F-095 partial with no optimisation id at the time; ranked gap 19 "Adjoint and shape morphing" was F-137 todo.
- Candidate ids F-244 to F-267 are "proposals only; docs/roadmap.json is not edited". Among them F-254 "Operating-map automation for turbomachinery" and F-257 "DOE sampling and optimisation drivers".
- Includes a trademark and independence statement and a "Declined and not done" section.

## Contradictions with the wiki

> [!warning] Contradiction
> The proposed ids collide with real roadmap items allocated later: in [[doc-reference-data]] F-245 is droplet evaporation and TAB breakup ([[doc-reference-data-f245-tab-breakup]]), F-249 is NSGA-II and Nelder-Mead options ([[doc-reference-data-f249-nsga2-nelder-mead]]), F-253 is the particle wall film, F-254 the particle wall interaction, F-255 particle dispersion, F-260 CMA-ES ([[doc-reference-data-f260-cma-es]]); in [[doc-fan-project-breakdown]] F-250 is the sliding AMI. The parity matrix proposals with the same numbers (for example F-254 operating-map automation, F-260 intrinsic structural solver) are not those items. [[doc-milestone-fan]] still cites "F-254 candidate" and "F-257 candidate" in the parity-matrix sense. Within the docs.

## Pages this source informs

- [[roadmap]], [[clean-room]], [[provenance]], [[design-loop]], [[doc-milestone-fan]]
