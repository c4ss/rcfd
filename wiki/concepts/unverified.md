---
type: concept
tags: [evidence, provenance]
sources: [doc-glossary, doc-readme, doc-reference-data, doc-verification-report, doc-provenance, doc-validation-data]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# UNVERIFIED and 'NOT read'

UNVERIFIED marks a value not checked against its source; 'NOT read' means a source was not opened ([[doc-glossary]]).

## How rcfd uses it

- Model constants checked against vendor theory pages are marked UNVERIFIED ([[doc-readme]]).
- These constants are marked UNVERIFIED until a published source confirms them ([[doc-provenance]]):
  - Realizable $A_0 = 4.04$ (the published report prints 4.0)
  - $\kappa = 0.4187$, $E = 9.793$
  - the two-layer constants $A_\mu = 70$, $A_\varepsilon = 2 c_l$, $Re_y^* = 200$
  - the Zeldovich NO rate constants
  - Proudman $\alpha = 0.1$
- The survey label set used in [[doc-validation-data]] is: READ (opened and read), seen (only a citation, record page or snippet), NOT read (not opened), UNVERIFIED ("a value not checked against a primary text") ([[doc-validation-data]]).
- [[doc-reference-data]] adds "from memory", "derived" (in an ADR) and "data not obtained". Most structural (FEM) sections say the named literature was "NOT read" ([[doc-reference-data]]).
- Counts after the 2026-10-27 relabel: 40 confirmed and 11 unverified or partly verified of the 57 rows of the wave 13 re-check ([[doc-verification-report]]).

## Related

- [[provenance]]
- [[clean-room]]
- [[verification-vs-validation]], [[doc-reference-data-not-verifiable-online]]
