---
type: concept
tags: [policy, provenance]
sources: [doc-glossary, doc-readme, doc-contributing, doc-process-agent-rules, doc-studio-guardrails, doc-product-suites, doc-audit-report, doc-provenance, doc-parity-matrix, doc-validation-data, doc-designs-m-fan-b]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Clean-room rule

Every algorithm comes from published literature or first principles, with no vendor code, binaries or documentation text ([[doc-glossary]]).

## How rcfd uses it

- No copying, translating, decompiling or deriving from proprietary CFD software; knowing a feature exists is fine, how it works inside is not a source ([[doc-contributing]]).
- Cite a source actually read, with a locator (equation, table, page, DOI); a fetched vendor page is not a source ([[doc-process-agent-rules]], rule 4).
- The Studio may not copy vendor branding, wording, screenshots or assets ([[doc-product]]).
- Guardrail G1 for the Studio: no layout, wording, icon, screenshot, workflow diagram or asset from commercial CFD software, its manuals or help files; nothing decompiled; "Knowing that a feature exists is fine; copying how it looks or what it is called is not"; fonts, icons and images original or under a compatible licence, listed in `THIRD_PARTY_LICENSES.md` ([[doc-studio-guardrails]], [[studio-guardrails]]).
- The product-suites backlog (2026-10-17) names vendor capabilities from public product pages and the headings of one vendor online-help contents page (P21), "not the theory text", recorded in `docs/provenance.md`; it copied no vendor text or code ([[doc-product-suites]]). See the tension noted there.
- The 2026-10-14 audit found no vendor code, text or data in the repo, but vendor-specific identifiers in the roadmap notes of 23 items, later reworded generically ([[doc-audit-report]]; unreviewed model output).
- The rule as stated in `docs/provenance.md`: "Algorithms, model constants, file formats and benchmark data are to come from published literature (papers, textbooks, public reports, public format specifications) and from first principles." "No decompilation, disassembly or other inspection of vendor binaries was ever done". An audit of 2026-10-27 found no decompiler output. The same page lists all six recorded exceptions ([[doc-provenance]]).
- On the roadmap notes, the provenance record says "about 20 items" named commercial settings and modules, and that those names "came from file listings of a commercial product installed on the author's machine" (audit of 2026-10-14). They were reworded between 2026-10-13 and 2026-10-22 ([[doc-provenance]]). The audit summary above gives 23 items.
- The parity matrix (2026-10-17) used vendor product pages (S1, S2) and login-only online-help contents pages (S3 to S10) through a model-condensed fetch. It paraphrased capability names only ([[doc-parity-matrix]]).
- Data: "copies of datasets distributed with commercial software are never used"; GPL data is fetched, never vendored ([[doc-validation-data]]).
- New readers follow the same rule: the M-FAN-B OBJ and glTF readers are written "in the crate from the public specifications" with no third-party crate ([[doc-designs-m-fan-b]]).

## Related

- [[doc-provenance]], [[doc-parity-matrix]]
- [[provenance]]
- [[unverified]]
