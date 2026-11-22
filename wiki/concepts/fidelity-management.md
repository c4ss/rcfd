---
type: concept
tags: [design-loop, multi-fidelity, reduced-order, discrepancy, fan]
sources: [adr-thread-design-surrogate-and-fidelity, adr-thread-design-optimisers-and-ledger, ref-frazier-2018-bayesian-optimization, ref-rasmussen-williams-2006-gpml]
builds_on: [multi-fidelity-modelling]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Fidelity management (screen, confirm, correct)

Multi-fidelity design screens many candidates with a cheap model, confirms a few with a dearer source, and uses the measured disagreement to correct the cheap screening. In rcfd it is `design::fidelity::run`, part H7 of the fan plan (F-699, ADR 0622), library only and opt-in ([[adr-thread-design-surrogate-and-fidelity]]).

## How rcfd uses it

- **Driver:** a low `ledger::Model` screens; per round exploit picks (by the corrected objective) and explore picks (greedy max-min distance) go to a `HighSource` (any model, or `MeasuredTable` from the protocol CSV with a variant column, tier measured, private); one `fidelity_pair` record per promotion with the prediction made before the high result ([[adr-thread-design-surrogate-and-fidelity]]).
- **Discrepancy ladder:** R0, R1 (additive or multiplicative), R2 ($\rho\, y_L + b_0 + \mathbf{c}\cdot\mathbf{x}$) chosen by leave-one-out error with the one-standard-error rule; a correction is "trusted only when it helps on held-out points and exceeds twice the stated uncertainty" ([[adr-thread-design-surrogate-and-fidelity]]).
- **Uncertainty guard isolated** (F-717, ADR 0642) on a synthetic offset-noise source; the surviving mutation of ADR 0622 is caught ([[adr-thread-design-surrogate-and-fidelity]]).
- **Rung R3** (F-718, F-734, F-735): a zero-mean [[gaussian-process-surrogate]] on the R2 residual; its leave-one-out standard deviation through the Cholesky factor agrees with a double-double brute force within 1.3e-10 and 8.1e-10 to 9.2e-10 ([[adr-thread-design-surrogate-and-fidelity]]).
- **Records and privacy:** the pair record is "the only one" (decision D4, 2026-11-15); measured values and pairs with a measured source go only to a private ledger (D3) ([[adr-thread-design-optimisers-and-ledger]]).
- Verified on synthetic sources only ([[adr-thread-design-surrogate-and-fidelity]]).

## Builds on

- [[multi-fidelity-modelling]]: screening with cheap sources, discrepancy models between fidelities, the cost budget ([[ref-frazier-2018-bayesian-optimization]] sec. 5).
- [[cross-validation]]: the leave-one-out choice of rung ([[ref-rasmussen-williams-2006-gpml]] sec. 5.3, 5.4.2).
- [[linear-regression-least-squares]]: the R1 and R2 corrections are low-order fits.
- [[gaussian-process-surrogate]]: rung R3.

## Related

[[design-loop]], [[design-ledger]], [[gaussian-process-surrogate]], [[computer-fan-target]], [[adr-thread-design-surrogate-and-fidelity]]

## Open questions

- No CFD or measured source, no calibration of the loss constants, no F-697 `ObjectiveModel` as the low model; done is "the maintainer's call" (F-699 gap) ([[adr-thread-design-surrogate-and-fidelity]]).
