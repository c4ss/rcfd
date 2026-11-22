---
type: concept
tags: [foundation, verification-validation]
sources: [ref-oberkampf-trucano-2002-sand2002-0529, ref-mfix-vv-manual, ref-nparc-vv-tutorial, ref-celik-2008-jfe-discretisation-uncertainty]
builds_on: [truncation-error-and-order-of-accuracy]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Code verification and the method of manufactured solutions

## In short

Code verification checks that a program solves the equations it claims to solve: one runs it on a problem whose exact answer is known, refines the mesh, and checks that the error falls at the rate (order) the method promises. Since exact solutions are rare, the method of manufactured solutions (MMS) picks a smooth function, works out what source term would make it an exact solution, and adds that source to the code's equations ([[ref-mfix-vv-manual]] ch. 2; [[ref-oberkampf-trucano-2002-sand2002-0529]] sec. 3.4.3).

## Explanation

**What is checked.** Verification is "the process of determining that a model implementation accurately represents the developer's conceptual description of the model and the solution to the model" (AIAA Guide, quoted in [[ref-oberkampf-trucano-2002-sand2002-0529]] sec. 2.1, p. 14), "solving the equations right" ([[ref-nparc-vv-tutorial]] `verassess.html`). After Roache, it has two parts: verification of the *code* (finding bugs and wrong implementations, without experimental data) and verification of a *calculation* (an error band on one result) ([[ref-nparc-vv-tutorial]]; [[ref-oberkampf-trucano-2002-sand2002-0529]] sec. 3.2.3). This page is the first part; the second is [[solution-verification]].

**Order-of-accuracy test.** The discretisation error (discrete solution minus exact PDE solution) of a consistent scheme behaves like `O(h^p)` on fine enough grids ([[ref-oberkampf-trucano-2002-sand2002-0529]] sec. 3.3.1, Eq. 3). The test compares the *observed* order, the rate at which the error falls under systematic refinement, with the *formal* order of the scheme; it is "considered a rigorous method for performing code verification" ([[ref-mfix-vv-manual]] ch. 2 intro). With error norms on meshes refined by ratio `r` ([[ref-mfix-vv-manual]] sec. 2.1, Eq. 2.15):

`p = ln(||DE_coarse|| / ||DE_fine||) / ln(r)`

using the L1, L2 or Linf norm of the error (Linf is the most demanding, L1 the least, Eq. 2.12-2.14).

**Where exact solutions come from.** Analytical solutions (closed forms for simplified physics), benchmark ODE solutions (Blasius, Taylor-Maccoll, stagnation flow) and benchmark PDE solutions (lid-driven cavity, flat plate, cylinder, backward-facing step); no published solution should count as a benchmark until reproduced independently, and code-to-code comparison "is a verification activity--NOT a validation activity" ([[ref-oberkampf-trucano-2002-sand2002-0529]] sec. 3.4.3, pp. 49-51). The MFiX manual ranks exact or manufactured solutions above literature benchmarks, and both above regression or code-to-code comparison ([[ref-mfix-vv-manual]] sec. 1.2).

**MMS procedure** ([[ref-mfix-vv-manual]] sec. 2.1; [[ref-oberkampf-trucano-2002-sand2002-0529]] p. 51):
1. Write the equations as `L[u] = 0`.
2. Choose a manufactured solution `U` (it need not be physical): smooth, with continuous derivatives to the equation's order, all terms of similar size so none dominates, and within the code's physical limits.
3. Apply `L` to `U`; the left-over terms form a source; solve `L[u] = L[U]`, of which `U` is the exact solution.
4. Take boundary and initial conditions from `U` (Dirichlet, Neumann or mixed).
5. Refine systematically, compute error norms, compute the observed order, compare with the formal order.

**What MMS finds.** Salari and Knupp applied it to the compressible and incompressible Navier-Stokes equations with 21 deliberate coding mistakes; it "correctly diagnosed every error in the set that prevented the governing equations from being solved correctly", but cannot detect mistakes that only affect efficiency ([[ref-oberkampf-trucano-2002-sand2002-0529]] p. 51). It only verifies "those code components invoked when setting up a MMS"; one MMS cannot verify a complex code fully ([[ref-mfix-vv-manual]] sec. 2.1).

**What it adds.** [[truncation-error-and-order-of-accuracy]] says what order a scheme should have; code verification measures whether the program actually reaches it, which separates coding errors from modelling errors before any comparison with experiment ([[ref-oberkampf-trucano-2002-sand2002-0529]] sec. 3.2.1).

## Builds on

- [[truncation-error-and-order-of-accuracy]]: the formal order the test compares against, and the error model `C h^p`.

## Leads to

- [[solution-verification]] (error estimates for a single calculation), [[verification-vs-validation]].
- The JFE policy requires previous code verification to be cited before discretisation error is estimated ([[ref-celik-2008-jfe-discretisation-uncertainty]]).

## How rcfd uses it

- **Structural solver:** "The checks are verification, not validation": closed forms, manufactured solutions, observed orders, patch tests, bit-identities and mutation catches. F-378 (observed order of displacement and stress by a manufactured solution, partial: Tet4 traction case L2 order 1.7925 on the pair 8 to 16 against 1.8), F-458 (the Tet4 order on 16 -> 32 as a registered check, done, ADR 0365) and F-459 (manufactured solutions on distorted meshes, done, ADR 0355) ([[roadmap-structural-solver]]).
- **Overset mesh:** the Taylor-Green interpolated-cell order was 1.62 against a registered 1.7 (missed), background order 1.98 ([[overset-mesh]]).
- **Gradient options:** on the manufactured problem the options keep second order but "do not reduce the error" ([[gradient-reconstruction]]).
- **Tests of the tests:** a mutation spot-check of registered limits "detected 10 of 12" ([[verification-vs-validation]], [[mutation-testing]]).
