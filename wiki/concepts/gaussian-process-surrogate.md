---
type: concept
tags: [design-loop, surrogate, gaussian-process, kriging, expected-improvement, bayesian-optimisation]
sources: [adr-thread-design-surrogate-and-fidelity, doc-reference-data-f698-gaussian-process, doc-design-loop]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Gaussian-process surrogate and expected improvement

A Gaussian-process (kriging) surrogate fits a stochastic model to the evaluated design points, predicting a mean and an uncertainty at any untried point; expected improvement (EI) proposes the next point by weighing the predicted gain against that uncertainty. In rcfd it is `design::surrogate` and `design::bayes`, built by F-698 (ADR 0621) for part H3 of the fan plan, library only and opt-in ([[adr-thread-design-surrogate-and-fidelity]]).

## How rcfd uses it

- **Model:** ordinary kriging with a Matern 5/2 correlation (squared exponential opt-in), length scales and an optional nugget by maximum likelihood from ten starts with the analytic gradient, a 1e-8 jitter, leave-one-out from one factorisation ([[adr-thread-design-surrogate-and-fidelity]]).
- **Loop:** Latin-hypercube start, EI, failed points imputed at m + s and multiplied by a failure model, constraints by feasibility probability, pending points believed at their mean; the ledger line order does not matter ([[adr-thread-design-surrogate-and-fidelity]]).
- **Results:** Branin within 1 percent on 20 of 20 seeds in a median of 28 evaluations; Hartmann 6 on 6 of 10 within 55; the reduced-order fan 10 of 10 within 1 percent of a 9261-point grid optimum by evaluation 22 (ADR 0621) ([[adr-thread-design-surrogate-and-fidelity]]).
- **Basin restart** (F-715, F-733, F-758): a restart away from the incumbent's basin (radius 2 in fitted length scales), first on a small acquisition (Hartmann 6 9 of 10, Shekel 10 17 of 20, a known miss) and then by a repetition trigger (8 of 10 and 19 of 20, ADR 0680). On the reduced-order fan the restart never fires ([[adr-thread-design-surrogate-and-fidelity]]).
- **Stall pre-check** (F-716, ADR 0645): a cheaper model checks each pick first; 0 failed evaluations against 34 to 39 of 50 without it ([[adr-thread-design-surrogate-and-fidelity]]).
- **Zero-mean, absolute-noise mode** (F-735, ADR 0663) used by the discrepancy rung R3 of [[fidelity-management]] ([[adr-thread-design-surrogate-and-fidelity]]).
- Test-function minima were "from memory" ([[doc-reference-data-f698-gaussian-process]]); Shekel 10 was "confirmed numerically at -10.536443" (ADR 0673) ([[adr-thread-design-surrogate-and-fidelity]]).

## Related

[[design-loop]], [[design-ledger]], [[fidelity-management]], [[hypothesis-record]], [[computer-fan-target]], [[adr-thread-design-surrogate-and-fidelity]]

## Open questions

- Multi-fidelity joining, multi-objective EI, the study method and MCP tool (F-714, F-702) are not built ([[adr-thread-design-surrogate-and-fidelity]]).
- ADR 0645 remaining question 1 (reclassify F-716 from todo to partial) has no recorded answer ([[adr-thread-design-surrogate-and-fidelity]]).
