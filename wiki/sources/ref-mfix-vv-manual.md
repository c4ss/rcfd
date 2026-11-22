---
type: source
tags: [reference, foundation, verification-validation, mms, cite-only]
sources: []
origin: https://mfix.netl.doe.gov/doc/vvuq-manual/main/html/index.html
licence: "© Copyright 2020, NETL Multiphase Flow Science Team" (page footer); no open licence stated, cite only
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# MFiX Documentation Volume 3: Verification and Validation Manual (NETL)

**Origin:** NETL Multiphase Flow Science Team, *MFiX Documentation Volume 3: Verification and Validation Manual*, Third Edition, National Energy Technology Laboratory (US DOE). Pages read: index, `introduction.html` (sec. 1.1-1.2), `mms/index.html` (ch. 2 intro), `mms/overview.html` (sec. 2.1), `references2.html`.
**Kind:** code V&V manual (HTML, Sphinx).
**Licence:** copyright notice only; cite only.
**Read:** 2026-11-22, by job p6-roots.

## Summary

A compact statement of the MMS procedure for code verification by order-of-accuracy testing, with the definitions it rests on and a ranking of verification test types. The procedure follows Roy (2005), "Review of code and solution verification procedures for computational simulation", J. Comput. Phys. 205(1):131-156, and Oberkampf and Roy (2010), *Verification and Validation in Scientific Computing* (references [26], [19]); neither was read here.

## Key points by section

### Sec. 1.1 "Verification and Validation"
- AIAA (1998) definitions; Roache (1997): verification is "solving the equations right", validation "solving the right equations".
- Code verification assesses the correctness of software and numerical algorithms; solution verification the accuracy of a particular numerical solution; validation asks whether the mathematical model predicts the physics. Roache (2009): "Verification is a mathematics issue; not a physics issue", so verification comes first.

### Sec. 1.2 "Verification Test Selection Criteria"
- Each test exercises sub-models, is cheap enough to run regularly, and overlaps little with others.
- Ranking: most preferable, comparison of numerical error against an exact or manufactured solution; less preferable, benchmark results from the literature; least preferable, regression against earlier versions or code-to-code comparison.

### Ch. 2 introduction
- Order-of-accuracy testing is "considered a rigorous method for performing code verification": the observed order (rate at which discretisation error falls under systematic refinement) is compared with the formal order. Discretisation error = numerical solution of the discrete equations minus exact PDE solution; since "the exact solution to a PDE is unknown for most practical problems", MMS makes a chosen analytic function exact by adding source terms.

### Sec. 2.1 "MMS Procedure Overview"
- Steps: (1) write the PDE as `L[u] = 0` (Eq. 2.1; Burgers example Eq. 2.2-2.3); (2) choose a manufactured solution `U`, not necessarily physical (example `U = A + sin(x + Ct)`); (3) apply `L` to `U` to get the source term, solve `L[u] - L[U] = 0` (Eq. 2.4-2.8); (4) initial and boundary conditions from `U` (Eq. 2.9-2.11; any type: Dirichlet, Neumann, Robin, mixed); (5) refine the mesh systematically; (6) compute discretisation-error norms L1, L2, Linf (Eq. 2.12-2.14; L1 least and Linf most demanding); (7) observed order `p = ln(||DE_(l+1)|| / ||DE_l||) / ln(r)` (Eq. 2.15, Fig. 2.1 with `r = 2`); ideally `p` becomes constant.
- Choosing `U`: analytic, with continuous derivatives at least to the PDE's order; no derivative discontinuities; all terms of similar magnitude so none dominates; stay within the code's physical limits (MFiX stops below 250 K).
- Limits: "this verification is limited to those code components invoked when setting up a MMS"; "most codes are far too complex to complete a full verification in a single MMS".

## Contradictions with the wiki

None found.

## Pages this source informs

[[code-verification-and-manufactured-solutions]], [[verification-vs-validation]]
