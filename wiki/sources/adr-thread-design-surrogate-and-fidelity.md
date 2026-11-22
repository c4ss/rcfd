---
type: source
tags: [adr-thread, design-loop, surrogate, gaussian-process, expected-improvement, multi-fidelity, fan]
sources: [doc-design-loop, doc-reference-data-f698-gaussian-process, doc-fan-project-breakdown]
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: the fan objective, the Gaussian-process surrogate and fidelity management (F-697, F-698, F-699 and their follow-ups)

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (registrations drafted on 2026-11-15, maintainer decisions of 2026-11-15, result addenda, follow-ups), 13 ADRs, 2026-11-15 to 2026-11-17.

Parts H1, H3 and H7 of `docs/fan-project-breakdown.md`: a fan objective over impedance points and tolerances (F-697, ADR 0620), a Gaussian-process surrogate with expected improvement (F-698, ADR 0621) with its stall pre-check (F-716) and its basin restart for multimodal functions (F-715, F-733, F-758, F-766, F-770), and a fidelity driver that screens with the reduced-order fan and corrects the screening from a higher tier (F-699, ADR 0622) with its uncertainty guard (F-717) and a Gaussian-process discrepancy rung R3 (F-718, F-734, F-735). All are library only and opt-in, and "every number ranks variants of the model, not real fans". See [[gaussian-process-surrogate]] and [[fidelity-management]].

## Current state

- **Fan objective done** (F-697, ADR 0620): `design::fan_objective` states impedance points as system curves with weights, terms with a statistic over tolerance specimens, limits per point or on an aggregate, and builds an ordinary `StudyDef`, so Nelder-Mead, CMA-ES, NSGA-II, the study store and the ledger sink run it unchanged. `FanRomTarget` evaluates the ADR 0119 reduced-order fan. Ten of ten mutations caught.
- **Surrogate partial** (F-698, ADR 0621): ordinary kriging with a Matern 5/2 correlation, maximum-likelihood hyperparameters, expected improvement after a Latin-hypercube start, failure and constraint handling, ledger-driven. "Done for the reduced-order tier only"; not covered: multi-fidelity joining, multi-objective EI, the study method and MCP tool (F-714, todo). The index shows ADR 0621's status word as DRAFT while the roadmap row is partial.
- **Basin restart** (F-715 partial, F-733 partial, F-758 done, F-766 done, F-770 done): the acquisition trigger helps on Hartmann 6 (9 of 10 against 6 of 10) and misses on Shekel 10 (17 of 20 against 18 of 20); the repetition trigger of ADR 0680 does 8 of 10 on Hartmann 6 and 19 of 20 on Shekel 10. On the reduced-order fan the restart "never fires, because the fitted length scales (2 to 10 in the unit cube) put the whole box inside the basin" (ADR 0673).
- **Stall pre-check** (F-716, ADR 0645): roadmap status **todo** by the registered rule, although "every acceptance clause met": the control NC1 "missed its registered ratio range on seed 4 (0.99351 against 0.9935, a range copied from rounded text), which the registered rule counts as todo". The agent recommended that the maintainer reclassify it as partial (ADR 0645 question 1). With the pre-check: 0 failed evaluations on every seed (34 to 39 without it). F-741 (the named point and the saving against a dearer model) is todo.
- **Fidelity driver partial** (F-699, ADR 0622, completed by ADR 0642): "Every check is met; done is the maintainer's call". Discrepancy ladder R0, R1, R2 chosen by leave-one-out with the one-standard-error rule, a correction "trusted only when it helps on held-out points and exceeds twice the stated uncertainty". Verified on synthetic sources only; no CFD or measured source, no calibration of the loss constants.
- **R3 partial** (F-718, ADR 0652; F-734, ADR 0656; F-735 done, ADR 0663): a zero-mean Gaussian process on the R2 residual; the predictive standard deviation's 1e-8 miss on two noise-free fits was "shown later to be errors of the double-precision reference" (ADR 0656: closed form within 1.3e-10 and 8.1e-10 of a double-double brute force). F-734 keeps one surviving mutation.

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0119](../../docs/adr/0119-wave46b-reduced-order-axial-fan-tier.md) | 2026-10-26 | registration + results (added by p3-records-a) | F-697 | Reduced-order axial-fan tier (strip theory, losses, fan curve, operating point, fan laws, steady-loading tones) | all met except Z1 (set-up defect, corrected Z1'); FAN-01 V1 reported, no limit: dp_s 45.7 Pa (-64 %) at the design point, 33 of 40 strips stalled | 0620 |
| [0620](../../docs/adr/0620-f697-registration-fan-objective-over-impedances-and-tolerances.md) | 2026-11-16 | registration (hub; maintainer decisions D1 to D6, B1 to B4) | F-697 | A fan objective over impedance points and tolerances (H1) | done (reduced-order tier only) | |
| [0621](../../docs/adr/0621-f698-surrogate-model-gaussian-process-and-expected-improvement.md) | 2026-11-15 | registration (status word DRAFT) | F-698 | GP surrogate over the ledger and EI proposal (H3) | partial; 11 of 12 mutations (imputation sign survived) | 0651, 0645 |
| [0622](../../docs/adr/0622-f699-fidelity-management-screen-confirm-correct.md) | 2026-11-16 | proposal then registration | F-699 | Screen with the ROM, confirm with a higher tier, correct (H7) | partial; every check met, mutation M5 survived | 0642 |
| [0642](../../docs/adr/0642-f717-registration-the-uncertainty-guard-and-p4-robustness.md) | 2026-11-16 | registration (test-only) | F-717 | Isolate the uncertainty guard; P4 without the file path | done; M5 now caught | |
| [0645](../../docs/adr/0645-f716-registration-stall-pre-check-for-the-fan-initial-design.md) | 2026-11-16 | registration | F-716 | A stall pre-check before each fan evaluation | todo by rule (NC1 transcription miss); acceptance met | F-741 (todo) |
| [0651](../../docs/adr/0651-f715-registration-robust-expected-improvement-on-multimodal-functions.md) | 2026-11-16 | registration | F-715 | A basin restart for EI; a check separating the imputation sign | proposed done, item partial; Hartmann 6 9 of 10 | 0673 |
| [0652](../../docs/adr/0652-f718-registration-gaussian-process-discrepancy-rung-r3.md) | 2026-11-16 | registration | F-718 | GP discrepancy rung R3 | partial; LOO sd 1.6e-8 and 6.3e-8 against 1e-8 | 0656, 0663 |
| [0656](../../docs/adr/0656-f734-registration-the-leave-one-out-standard-deviation-through-the-cholesky-factor.md) | 2026-11-16 | registration | F-734 | LOO sd of R3 through the Cholesky factor | partial; one registered mutation survived | |
| [0663](../../docs/adr/0663-f735-registration-a-zero-mean-absolute-noise-mode-of-the-gaussian-process.md) | 2026-11-16 | registration | F-735 | Zero-mean, absolute-noise GP mode used by R3 | done | |
| [0673](../../docs/adr/0673-f733-registration-basin-restart-on-shekel-10-and-on-the-fan.md) | 2026-11-17 | registration | F-733 | Basin restart on Shekel 10 and on the fan | partial; Shekel 10 17 of 20 against 18 of 20 (known-miss record) | 0680 |
| [0680](../../docs/adr/0680-f758-registration-a-repetition-trigger-for-the-basin-restart.md) | 2026-11-17 | registration | F-758 | A repetition trigger for the basin restart | done; Hartmann 6 8 of 10, Shekel 10 19 of 20 | 0686, 0689 |
| [0686](../../docs/adr/0686-f766-registration-a-fast-check-of-the-plain-pick-condition-of-the-repetition-trigger.md) | 2026-11-17 | registration (test-only) | F-766 | Fast check of the plain-pick condition | done (0.01 s test) | |
| [0689](../../docs/adr/0689-f770-registration-a-fast-check-of-the-window-condition-of-the-repetition-trigger.md) | 2026-11-17 | registration (test-only) | F-770 | Fast check of the window condition | done (0.01 s test) | |

## Key numbers and limits

- **ADR 0620:** per-point flows "within 1.4e-16 of the closed form"; mean static unbalance "within 1.7 SE of r_c m sigma sqrt(7 pi / 4) at 4096 samples"; Nelder-Mead and CMA-ES "reach the constrained optimum (speed scale 1.2, pitch 0.3, 3.6 W) within 4e-8 relative in power"; NSGA-II front "within 1.8 percent of the closed-form front". Remaining question 2: "`ComputerFan::default()` is not usable in the reduced-order model (stalled below about 80 % of free delivery, smoke 1)" (became F-713).
- **ADR 0621:** two-point closed form within 2.7e-15; coverage 0.938 on prior draws (0.865 with a wrong length scale); "the Branin minimum within 1 percent on 20 of 20 seeds in a median of 28 evaluations (random search 0 of 20), Hartmann 6 on 6 of 10 within 55 evaluations against CMA-ES 1 and DE 0 at 200, a failure disc avoided (3 percent failed against 86 percent without the failure handling)"; on the reduced-order fan "10 of 10 runs within 1 percent of a 9261-point grid optimum by evaluation 22". "High failure share on the fan (34 to 39 of 50)" led to F-716.
- **ADR 0642:** on source S8 (high = low + 0.075 + 0.01 z, stated uncertainty 0.05) the one-standard-error rule picks R1add "with a leave-one-out RMSE of 0.12 to 0.14 of the uncorrected one (limit 0.8), the discrepancy is 1.55 to 1.65 times the stated uncertainty, and the correction is untrusted only because it is not above twice that uncertainty".
- **ADR 0645:** "243 to 278 failed pre-checks per run ... all 2649 rejected points fail the objective at R3"; "a pre-check of the initial design alone changes nothing (34 to 39 failed)".
- **ADR 0651:** restart fires "when the best expected improvement falls below 1e-4 of the spread of the evaluated values", basin "an ellipsoid of radius 2 in the fitted length scales"; imputation at m + s keeps the failed share at 0.035 "where m - s gives 0.832 and no imputation 0.862".
- **ADR 0673:** the step's own process scores 11 of 20 on Shekel 10 "against 17 for the fresh process, so the fresh process is needed"; Shekel 10 minimum "confirmed numerically at -10.536443".
- **ADR 0680:** the repetition trigger fires "when the plain pick of the step and each of the last d + 1 evaluated observations lie in the incumbent's basin"; "388 picks, 0 violations"; "With the repetition trigger, propose depends on the order of the observations."
- **ADR 0663:** zero-mean mode $y \sim \mathcal{N}\left(0,\ s_f^2 (R + 10^{-8} I) + \operatorname{diag}(v)\right)$; "against brute force within 2e-13 of the process sd, gradient within 2.6e-7 of central differences".

## Open questions

- **ADR 0645 remaining question 1** (status under the NC1 transcription miss): "(a) Keep todo as the rule says. (b) The maintainer reclassifies the item to partial, because the control reproduced F-698 exactly and every acceptance clause is met. **Recommendation: (b)**. It needs the maintainer and is not taken here." No answer is recorded in ADR 0645; the roadmap row says todo. Question 3 (cost): "In R1 the run makes about three times the ROM operating-point solves of F-698 (443 to 478 against 150)".
- F-699 and F-701: "done is the maintainer's call" (roadmap gaps).
- ADR 0620 remaining questions 1 and 3 (aggregates repeated in point entries; other aggregation order and a chance constraint) carry recommendations (a).
- Any CFD evaluator behind the objective, any measured source, and the MCP tools (F-702, F-714) are not built.

## Before F-699: the fidelity-tier proposal (ADR 0136; added by job p3-records-b)

[ADR 0136](../../docs/adr/0136-fidelity-tiers-xhigh-to-xlow.md) (PROPOSED 2026-10-28, "scoping only; nothing implemented or run"; index item F-286) answers the maintainer's framing "build the foundation for an extra high precision simulation, and lower tiers that deliver a stated percentage accuracy for a multiplier speedup". It proposes five tiers, xhigh, high, medium, low and xlow: "one `fidelity` setting expanded into explicit knobs, error budgets against an xhigh reference, and target speedups". "Without `--fidelity`, the solver gets today's input." Its decision ids F-1 to F-13 "are not roadmap items"; only F-13 was decided by the maintainer (2026-10-28: "the tier is reachable through the MCP project tools (D9)"); F-1 to F-12 remain proposals. "Every accuracy and speedup number in this ADR is a TARGET or an ESTIMATE ... **Every number for the computer-fan class is an estimate, because no fan reference at any fidelity exists in the repository.**" Stage T5, the computer-fan class F, is blocked.

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0136](../../docs/adr/0136-fidelity-tiers-xhigh-to-xlow.md) | 2026-10-28 | proposal (scoping) | F-286 | Fidelity tiers xhigh to xlow: knobs, budgets, validation plan, staged plan T1 to T6 | proposed; D9 (F-13) decided; nothing built | ADR 0622 / F-699 for T6 (amendment 2026-11-15) |

- **Tier matrix (D2, Table B1, class S):** medium is "the case as written"; xhigh h/2 and dt/2, high h/sqrt 2 and dt/sqrt 2, low sqrt 2 h and sqrt 2 dt; "Schemes stay as written at every tier ... Medium, high and xhigh therefore differ in h and dt only, and form a refinement family". xlow "is not a run": an evaluator (strip theory, lumped network, correlation) through the design loop or the MCP tool `evaluate_xlow`, "offered only where such a model exists".
- **Amendment 2026-11-15 (maintainer decision):** "The cross-tier ledger rule and multi-fidelity correction proposed here (T6, the `cross_tier_check` record) are superseded by the fidelity pair and discrepancy record of ADR 0622 (roadmap F-699, with the ledger fields in F-700). This ADR keeps the tiers themselves".
- Proposed roadmap items (no ids assigned in the ADR): the fidelity setting and the MCP tier tools (T1), validation of class S (T2), mixed-precision solves (T3, T4), the cross-tier rule (T6). Open: D1 to D8 and the open decisions of the ADR's last section.

## Related

- [[gaussian-process-surrogate]], [[fidelity-management]], [[design-loop]], [[design-ledger]], [[computer-fan-target]], [[known-miss]]
- [[adr-thread-design-optimisers-and-ledger]], [[adr-thread-design-hypothesis-record]], [[adr-thread-rom-reference-fan-and-c-grid]]
- [[roadmap-design-studies-and-optimisation]]
- Earlier history (added by p3-records-a): the ROM tier lives in `crates/rcfd-post/src/fan_rom.rs` (ADR 0119). "The tier is not validated against any measured fan, and a design ranking from it is a hypothesis for CFD or test." Against FAN-01 "over all 20 points the rms dp_s difference is 104 Pa and the rms power difference is 24 %". "An efficiency objective on this tier drives a design to the stall edge, so a design loop needs a stall-margin constraint (not provided here)." The steady-loading tone "(1.25e-9 Pa at 1 m for set-up C) is a lower bound of the tonal level, not a prediction of it."
