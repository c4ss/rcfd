---
type: concept
tags: [validation, verification, process]
sources: [doc-reference-data, doc-verification-report, doc-milestone-fan, doc-reference-data-f869-cylinder-mean-drag, doc-changelog, doc-fan-measurement-protocol, doc-design-loop, doc-test-process, adr-thread-simulation-milestones-validation, adr-thread-fan01-design-point, adr-thread-design-surrogate-and-fidelity, ref-oberkampf-trucano-2002-sand2002-0529, ref-nparc-vv-tutorial, ref-mfix-vv-manual, ref-nist-tn-1297]
builds_on: [code-verification-and-manufactured-solutions, solution-verification, validation-comparison]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Verification vs validation

Verification asks whether the numbers are computed (or quoted) correctly; validation asks whether they agree with the physical world. rcfd's docs keep three separate things apart: checking a reference value against its source, checking a solution against its own discretisation error, and comparing with a measurement.

## How rcfd uses it

- **Reference-value verification.** In [[doc-reference-data]], "Verified" means the value in the repo agrees with the source, and "secondary" means the source reproduces the primary table rather than being it. Wave 6a counted about 34 verified, 3 mismatched and about 8 unverifiable rows ([[doc-reference-data]]). The wave 13 re-check is in [[doc-verification-report]]: 57 rows, 44 confirmed, 4 differ, 7 unverified, 2 not re-checked, and after the 2026-10-27 relabel 40 confirmed and 11 unverified ([[doc-verification-report]]). Labels such as UNVERIFIED, NOT read, from memory and data not obtained are used throughout ([[unverified]]).
- **Solution verification.** The FAN-01 mesh study of ADR 0086 is called "first solution-verification study". Its error bands are "NOT shown credible" because the empty-passage check failed ([[doc-milestone-fan]]). See [[gci-richardson]].
- **Validation against computations versus measurements.** The cylinder mean-drag band of F-869 is built from eleven values: "Nine of the eleven values are quotations and two are primary; all are computations, none is a measurement" ([[doc-reference-data-f869-cylinder-mean-drag]]).
- **What is not validated.** The 0.1.0 changelog says rcfd "has not been validated for engineering decisions" and "No fan prediction in this release is validated against measurement". Noise and structural results are not validated either ([[doc-changelog]]). The fan objective ranks "variants of the model, not real fans" ([[doc-design-loop]]).
- **Measurement uncertainty budget.** For a user-measured fan, each measurement's standard uncertainty should be "at most one third of the comparison tolerance the tools will be held to (so that a disagreement is the model's, not the rig's)" ([[doc-fan-measurement-protocol]]).
- **Code verification of the tests themselves.** Two checks look at whether tests can fail at all: a scan for tests that cannot fail, and a mutation spot-check of registered limits that "detected 10 of 12" ([[doc-test-process]]).

- **Band from the measurement's scatter (ADR 0760):** for NACA 0012 the band term s = 0.018 is the scatter of Ladson's three grit zones, because "a computed value cannot be asked to agree with the measurement more closely than the measurement's three sets agree with each other" ([[adr-thread-simulation-milestones-validation]]).
- **A band needs the measurement's stated uncertainty (ADR 0762):** the round-robin scatter is "another quantity than the one the acceptance names", so F-870 cannot be done until F-877 reads the uncertainty ([[adr-thread-fan01-design-point]]).
- **Model-only claims:** every design-loop result carries the caveat "every number ranks variants of the model, not real fans" ([[adr-thread-design-surrogate-and-fidelity]]).

## Builds on

The general terms, from the open V&V literature read in P6:
- [[code-verification-and-manufactured-solutions]]: AIAA verification, "solving the equations right"; order-of-accuracy tests against exact and manufactured solutions ([[ref-oberkampf-trucano-2002-sand2002-0529]] sec. 2.1, 3.4.3; [[ref-mfix-vv-manual]] sec. 1.1, 2.1).
- [[solution-verification]]: the numerical error of one calculation (round-off, iteration, discretisation) and grid convergence studies ([[ref-nparc-vv-tutorial]]).
- [[validation-comparison]]: AIAA validation, "the degree to which a model is an accurate representation of the real world from the perspective of the intended uses of the model"; "Individual computational outcomes of a model are validated; codes are not validated" ([[ref-oberkampf-trucano-2002-sand2002-0529]] sec. 2.1, 4.1.1).
- [[measurement-uncertainty]]: the standard, combined and expanded uncertainty a measured value carries ([[ref-nist-tn-1297]]).

None of the V&V sources read in P6 discusses rcfd's first kind of checking, reference-value verification (does the number in the repo match its source); see [[provenance]] for how rcfd records it.

## Related

[[registered-limit]], [[known-miss]], [[registration]], [[validation-data-pack]], [[gci-richardson]], [[provenance]]

## Open questions

- No comparison tolerance is set yet for a computer fan ([[doc-fan-measurement-protocol]]).
