---
type: source
tags: [adr-thread, design-loop, optimisation, nsga2, cma-es, nelder-mead, sobol, ledger]
sources: [doc-design-loop, doc-reference-data-f249-nsga2-nelder-mead, doc-reference-data-f297-nsga2-crossover, doc-reference-data-f260-cma-es, doc-fan-project-breakdown]
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-23
status: draft
---

# ADR thread: design-study optimisers, Sobol indices and the design ledger (F-249, F-297, F-260, F-268, F-700, F-705, F-706)

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (registrations with result addenda; maintainer decisions of 2026-11-15 and 2026-11-18), 8 ADRs, 2026-11-07 to 2026-11-16.

The study methods of the [[design-loop]] beyond the wave-32C set (ADR 0067): NSGA-II and two Nelder-Mead options with an extended warm start (F-249), a per-variable exchange crossover for NSGA-II (F-297), CMA-ES with restarts and the real-fan study (F-260), Sobol sensitivity indices (F-268, ADR 0240) and the remaining F-268 scope of ledger logging and uncertainty bands on the reduced-order fan (ADR 0623). Then the [[design-ledger]] itself: its optional record fields and the measured tier (F-700, ADR 0632), the store header after a torn first write (F-705, ADR 0644) and the private-ledger repository markers (F-706, ADR 0647). All library-level, opt-in, with defaults bit-identical.

## Current state

- **Optimisers in place:** `nsga2` (ADR 0209), opt-in `crossover = "exchange"` (ADR 0220), opt-in Nelder-Mead `initial_simplex = "start"` and `coefficients = "adaptive"` (ADR 0209), `cma_es` with restarts of increasing population (ADR 0222). Defaults unchanged.
- **F-249 partial** with two known misses (ADR 0209): MO1z, NSGA-II with the default SBX on the 30-parameter ZDT1 (C 0.35 to 0.54 against 1e-2; met only with the exchange crossover of F-297), and N3, the constrained quadratic from 20 seeded starts (19 of 20). F-310 (exchange crossover as the default for new studies, and the tool description) is todo.
- **F-260 partial** (ADR 0222): the CMA-ES clause is met (shifted Rastrigin d = 4 to $f \le 10^{-8}$ on 20 of 20 seeds within 40 000 evaluations, where every Nelder-Mead run stops at $f = 35.82$); "The FAN-01 study is not attempted (no converged FAN-01 solve, no blade-parameter keys)". F-260 is a headline item of the `fan` milestone (ADR 0757). F-315 (real-fan study with CMA-ES) is todo.
- **F-268 done** (ADRs 0121, 0240, 0623): Sobol indices (`design::sensitivity::run`), `Study::set_ledger_sink`, `FanRomModel` and `fan_rom_bands` (5, 50 and 95 percent quantiles for q, dp_s, power and tone1_pa under the ADR 0118 set-up T tolerances). The NC1 control miss on ST1 (|z| 3.87 against 4) "is accepted as a known miss" (maintainer decision B4, 2026-11-15).
- **F-700 done** (ADR 0632): optional top-level keys `parents`, `source`, `fidelity_level`, `uncertainty`, `objective`, `fitted`; `Fidelity::Measured` written only to a private ledger; `ledger::fidelity_pair` and the P8 queries.
- **F-705 done** (ADR 0644), **F-706 done** (ADR 0647).
- **Exposure:** all library only until F-702 (decision D1 of 2026-11-15). F-339 (MCP tools for tolerance and Sobol studies) "may be issued" under the class rule for additive golden edits (answer of 2026-11-18 in ADR 0240), together with F-702; neither was issued in week 47 because the line "is over its weekly budget" (ADR 0620, answers).
- **How the old questions were closed:** for ADRs 0209, 0220 and 0222 the maintainer decided on 2026-11-18 that the standing instruction applies: "the lead closes each on its written recommendation, except a question that would change a default result, a golden or a hash table, or that needs a source the repository lacks; those stay open and are named in the item's roadmap note."
- F-339 (MCP tools `design_uq` and `design_sobol`) was issued on 2026-11-23 by the brief `docs/adr/briefs/f339-mcp-uq-sobol-tools.md` (ADR 0784); its registration is not on main at 09647baa and the item is todo ([[doc-roadmap-json]]).

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0067](../../docs/adr/0067-wave32c-design-loop.md) | | registration + results (added by p3-records-a) | F-095, F-786 | Design-loop infrastructure: PIMPLE divergence guard, operating maps, DOE and optimisation, assistant tools | T3 known miss (best f 3.1255e-3 after 120 candidates against 1e-3); E1 met (49.0 % against 25 %); see [[adr-thread-early-workflow-and-partials]] | |
| [0121](../../docs/adr/0121-wave46d-design-ledger-and-tolerance-study-driver.md) | 2026-10-26 | registration + results (added by p3-records-a) | F-268 | Durable design ledger (H4) and tolerance / uncertainty study driver (F1) | all met, no known miss; L3b added by amendment 1 and met; F1 library-only, partial | 0240, 0623 |
| [0209](../../docs/adr/0209-f249-registration-nsga2-studies-nelder-mead-options-and-extended-warm-start.md) | 2026-11-07 | registration | F-249 | NSGA-II studies, two Nelder-Mead options, an extended warm start | partial; known misses MO1z (ZDT1 C 0.35 to 0.54) and N3 (19 of 20) | 0220 (MO1z) |
| [0220](../../docs/adr/0220-f297-registration-nsga2-per-variable-exchange-and-the-30-parameter-zdt1-check.md) | 2026-11-07 | registration | F-297 | Per-variable exchange crossover in NSGA-II; ZDT1 with 30 parameters | done; C 1.3e-3 to 1.6e-3 against 1e-2 with the opt-in; default still 0.543 | |
| [0222](../../docs/adr/0222-f260-registration-cma-es-and-the-real-fan-study.md) | 2026-11-08 | registration | F-260 | CMA-ES with restarts; the real-fan study | partial; CMA-ES clause met; FAN-01 study not attempted | |
| [0240](../../docs/adr/0240-f268-registration-sobol-sensitivity-indices.md) | 2026-11-12 | registration | F-268 | First-order and total Sobol indices for the tolerance driver | partial (Sobol part); NC1 ST1 |z| 3.87 against 4, accepted as known miss | 0623 |
| [0623](../../docs/adr/0623-f268-remaining-ledger-logging-and-reduced-order-bands.md) | 2026-11-15 | registration | F-268 | Automatic ledger logging from studies; bands on the reduced-order fan | done; 12 of 12 intervals contain the 100000-sample reference | |
| [0632](../../docs/adr/0632-f700-registration-ledger-records-for-objectives-surrogates-and-measured-data.md) | 2026-11-16 | registration | F-700 | Ledger records for objectives, surrogates, multi-fidelity and measured data | done | 0647 (markers) |
| [0644](../../docs/adr/0644-f705-registration-design-store-header-after-a-torn-first-write.md) | 2026-11-16 | registration (defect fix) | F-705 | The design store header after a torn first write | done; reopens at every byte of the first append (1354 points) | |
| [0647](../../docs/adr/0647-f706-registration-private-ledger-repository-markers-beyond-git-and-cargo.md) | 2026-11-16 | registration | F-706 | Private-ledger repository markers beyond git and Cargo | done; `.hg`, `.svn`, `rcfd-project.json` refused | |

## Key numbers and limits

- **ADR 0209:** T3 with the opt-in options "reaches best f 3.70e-4 after 120 recorded candidates (limit 1e-3; the default stays at 3.1e-3, bit for bit)"; NSGA-II on FON within 4.4e-3 of the analytic front (limit 1e-2); extended warm start reproduces cold runs "to 6.9e-9, 3.1e-11 and 2.6e-11 (limit 1e-4)" for temperature, k-epsilon and k-omega SST.
- **ADR 0220:** "the papers cross each variable with probability 0.5 and state no exchange between the children"; FON with the exchange "C at most 5.9e-3".
- **ADR 0222:** "Without restarts 3 of 20 seeds reach the target; in 10 dimensions 14 of 20 (recorded)". Defaults: `restarts` 0, `sigma0` 0.3.
- **ADR 0240:** Ishigami (N 16384) and a linear function within 4 SE of closed forms (largest z 2.41); delta-method coverage 0.920 to 0.963 at 1.96 SE (400 runs, limit 0.90); "the interaction ST3 - S3 = 0.239 seen where SRC gives SRC_2^2 = 5e-6".
- **ADR 0623:** ledger sink "0 duplicate and 0 missing at all 55 kill points of a Nelder-Mead study"; `FanRomModel` "bit for bit equal to FanRomEvaluator::metrics at 64 points"; "the sd within 2.7 percent of first-order propagation"; the one default change ("uq::mean_sd returns the value of a bit-constant sample") "listed for the maintainer".
- **ADR 0644:** "on the unchanged library exactly the 60 header points failed"; fix: "the header is written when the opened file is empty".
- **Maintainer decisions of 2026-11-15** (shared by ADRs 0620 to 0624): D1 exposure library only until F-702; D2 measured tier library only; D3 strict privacy split ("A public ledger gets rounded aggregates only (3 significant digits, at least 5 pairs)"); D4 one fidelity pair record; D5 optional ledger keys; D6 order "F-268 and F-700, then F-697, then F-698 and F-699 in parallel, then F-701"; B1 measured CSV with a variant-id column; B2 hypothesis level 0.05 with $6 / (\pi^2 i^2)$; B3 estimated limits fixed from a disclosed smoke run "and not moved after a run"; B4 the ADR 0240 NC1 miss accepted.

## Open questions

- ADR 0240 Q2 (a service method and MCP tool for Sobol indices) became F-339 and is approved under the additive-golden class rule; it waits with F-702.
- ADR 0209 Q5 and ADR 0220 (the NSGA-II sources): ADR 0209 recorded the constants "from memory"; ADR 0220 read the papers. ADR 0222 Q1 recommended splitting the FAN-01 study into its own item (F-315 now exists).
- Questions that would change a default, a golden or a hash, or need a source the repository lacks, "stay open and are named in the item's roadmap note" (2026-11-18 decision). The F-249 roadmap gap names ADR 0209 Q1 and Q7.

## Related

- [[design-loop]], [[design-ledger]], [[parametric-study]], [[computer-fan-target]], [[fan-01]], [[bit-identical]], [[known-miss]]
- [[adr-thread-design-surrogate-and-fidelity]], [[adr-thread-design-hypothesis-record]], [[adr-thread-rom-reference-fan-and-c-grid]]
- [[roadmap-design-studies-and-optimisation]]
- Earlier history (added by p3-records-a): ADR 0121 made the ledger "a JSON-lines file of evaluated variants, with units, evaluator, solver version, verdict and fidelity tier on each entry; it survives sessions and lets a result be re-run", with four service methods and four MCP tools; the driver samples by Monte Carlo or Latin hypercube and "stops on a standard-error rule"; "No default changes: no existing path calls the ledger or the driver". The numbers "verify the driver and the ledger on analytic functions and say nothing about the spread of a real fan's flow, power, noise or vibration". ADR 0067 is the first design-loop wave ([[adr-thread-early-workflow-and-partials]]).
