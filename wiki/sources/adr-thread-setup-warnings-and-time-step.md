---
type: source
tags: [adr-thread, numerics, setup-rules, warnings, sv301, sv302, sv202, time-step, courant, end-time]
sources: [adr-thread-piso-stability-consistent-correctors, adr-thread-naca0012-sa-drag, adr-thread-wave48-egui-studio]
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: the time step, the end of a run and the set-up warnings (SV202, SV301, SV302, `short_last_step`)

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (registrations with result addenda), 5 ADRs, 2026-10-28 to 2026-11-12. Only ADR 0235 carries items (F-249, F-269, F-328, F-329).

What a run does before and at its last step, and what it says to the user about a set-up that is likely to go wrong. A transient fixed-step run took one more, tiny step at its end and that spoiled state was its result (ADR 0145). A refined cylinder kept its time step and gave garbage without a warning (ADR 0153). The pre-run coupling warning SV301 of ADR 0156 ([[adr-thread-piso-stability-consistent-correctors]]) was wired into the run service and the MCP tools (ADR 0160) and given a settled-step form and a single warning path, beside a new static warning SV302 for the trailing-edge mechanism of ADR 0171 (ADR 0172, [[adr-thread-naca0012-sa-drag]]). ADR 0235 gave SV202 its fuller text. See [[setup-rules]].

## Current state

- **A fixed-step run ends on `solver.end_time`.** A step that would leave a remainder below 1e-6 of itself is stretched so that it ends on the end time, on "the four fixed-step paths of Context 1"; a separate last step shorter than 5 % of the step proposed for it queues a `short_last_step` warning on the paths that solve a pressure (ADR 0145). A run the end test stops within 1e-12 of `solver.end_time` reports `solver.end_time` exactly, and "every field, flux, monitor value, statistic, particle and continued run is bit-identical to the unchanged tree, so no opt-in was needed" (ADR 0160).
- **The Studio sets the time step at run start** (incompressible transient runs, Studio side only): "Simulate for N s", an automatic step under the solver's Courant controller, a Courant estimate in the captions and a warning above Courant 2; "The settings are run overrides, recorded with the run and never written into the case or the editor" (ADR 0153). The egui Studio that held this is deleted ([[adr-thread-wave48-egui-studio]]).
- **SV301** fires at a run's start from `run.headless` and MCP `run_headless` (ADR 0160) and, since ADR 0172, at the settled step: "the watch emits exactly one SV301 (rho 1.97, step 10) with 3 evaluations in 60 steps and fields bit-identical to a manual loop; silent after a pre-run SV301". Its preflight takes the first step's viscosity: "the power-law channel's estimate goes from 0.0112 to 4.30 and warns" (ADR 0160). Not wired: "The chain driver, `run.start` / `run.step` and `settings.validate`" (ADR 0160; ADR 0172's LP1 to LP3 then gave headless, chain and CLI the same text).
- **SV302** is "a risk notice with a registered false positive": it fires on M1 at 0 degrees, which ran clean in ADR 0171; "All 12 shipped examples exit 0 and none carries SV302" (ADR 0172). On FAN-01 it is a false negative and "by the maintainer's decision its level stays at 3 000" (ADR 0174, [[adr-thread-fan01-rotating-solve]]). The guard hint now names `barth_jespersen` "and no longer says 'lower the relaxation factors'".
- **SV202** (F-328 done, ADR 0235 A): for the compressible solver it states "that every patch is a slip wall and an outlet is extrapolated"; for every other solver that the case is refused unless every mesh patch that is not empty or cyclic has a boundaries entry, "and that an entry without keys is a fixed no-slip wall".
- **Also in ADR 0235:** `rcfd map --warm-start extended` (F-329 done; defaults byte-identical) and the vacuous pass of `schemes_wave13` T2 closed by `t2b` (F-269 note).

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0145](../../docs/adr/0145-job48k-end-time-remainder-step.md) | 2026-10-28 | registration | | A fixed-step transient run ends on a whole step | every limit met; the control reproduced the defect (2001 steps, last `cd` -4.052611e5, against 2000 and 1.327161) | 0160 (time stamp) |
| [0153](../../docs/adr/0153-job48s-time-step-set-when-a-run-starts.md) | 2026-10-28 | registration | F-837 | The time step set when a run starts (Studio) | (a) met (256 x 256 cylinder for 40 s: 10,742 steps in 1.31 h, Courant at most 0.4501, St 0.1663); known misses (f) and D2 (F ended 1.7e-14 s short of 3 s) | |
| [0160](../../docs/adr/0160-job48z-end-time-stamp-corrector-count-record-sv301-wiring.md) | 2026-10-29 | registration | | Final time stamp, corrector count in `residuals.jsonl`, SV301 in the run service and MCP | every limit met except N5 as worded (two tests failing identically on main 444255c) | 0172 |
| [0172](../../docs/adr/0172-job49j-set-up-warnings-sv301-at-the-settled-step-one-warning-path-linear-upwind-wall-risk-sv302-guard-hint-and-correctors-in-the-plots.md) | 2026-10-30 | registration | | SV301 at the settled step, one warning path, SV302, guard hint, correctors in the plots and `read_residuals` | one miss: LW4 on two of three meshes (+38 % on M1, +50 % on M1w15, limit 30 %); everything else met | |
| [0235](../../docs/adr/0235-small-follow-ups-sv202-text-map-warm-start-flag-t2-vacuous-pass.md) | 2026-11-12 | registration (bundle) | F-249, F-269, F-328, F-329 | SV202's fuller text; `rcfd map` flag for the extended warm start; the vacuous T2 | every check met, every mutation caught; F-328, F-329 proposed done, F-269's T2 item resolved | |

## Key numbers and limits

- **ADR 0145:** "Each `time += d` rounds, so n steps of dt can fall short of the end time by more than 1e-12"; in ADR 0142's smoke the extra step "was 3.5e-12 s long, with C_D -4e5 and C_L 9e5".
- **ADR 0153:** "The Courant target 0.5 is not a claim of stability on every mesh: on the 256 x 256 O-grid (grading 100) the PISO solver with its default two correctors is unstable well below Courant 1 (P4), found here and not fixed (question 2)." The registered decision rule "read literally, then names neither step"; the job kept A, the adaptive Courant controller (question 7).
- **ADR 0160:** `residuals.jsonl` carries `correctors` and `corrector_cap` "on the lines of `adaptive` runs only"; "With a turbulence model the preflight's viscosity misses the first sweep, by 14 to 21 % on the two set-ups measured".
- **ADR 0172:** K1 rho 4.358667 bit-identical at the fixed step; SV302 level 3000, measured `W_max` M1 29043 (ADR 0171: 21000), M1w05 6944, M1w15 2406 (ADR 0171: 1600), "close under the level 3000". Expected firing: "fires on M1 at any angle (RA false positive, RB true positive) and M1w05 (RD); silent on M1w15 (RF) and whenever a limiter is set or the solver is not the steady one".
- **ADR 0235 B2:** `--warm-start extended` largest relative difference to `--cold` 5.67e-9 (`dp`, point 1), "iterations warm 61 and 60 against cold 89 and 88" (`REL_WARM` 1e-4). **C1:** `t2b` over(BDF2) 2.4667e-2, over(Bdf2Bounded) 1.1564e-9 (limits >= 0.02; <= 0.25 over(BDF2) and <= 0.02).

## Open questions

- ADR 0153 question 2 (PISO with two correctors unstable below Courant 1 on the graded O-grid; see the adaptive correctors on [[adr-thread-piso-stability-consistent-correctors]]) and question 7 (which step is automatic).
- ADR 0172 LW4: the cause (ADR 0171 aggregated the metric differently) "is not verified".
- ADR 0235 Q1: the extended warm start as the map default (recommendation (b) once the maintainer lifts the bit-identity rule for maps; (a) until then). Q2: `warm_start` in the design tools, a registered golden edit (recommendation (b), with ADR 0209 Q6).

## Related

- Predecessor (added by p3-records-a): the setup-validation rule engine (21 rules held as data) began in ADR 0031 (wave 19C, F-240) and F-240 was set done by ADR 0092; see [[adr-thread-early-workflow-and-partials]].
- [[setup-rules]], [[piso]], [[divergence-guard]], [[exit-codes]], [[naca0012]], [[cylinder-re100]], [[bit-identical]]
- Threads: [[adr-thread-piso-stability-consistent-correctors]], [[adr-thread-naca0012-sa-drag]], [[adr-thread-wave48-egui-studio]], [[adr-thread-design-optimisers-and-ledger]] (F-249)
- Areas: [[roadmap-numerics-and-solvers]], [[roadmap-studio-desktop-app]]
- [[doc-investigation-job49h-te-divergence]] (results folder of ADR 0171)
