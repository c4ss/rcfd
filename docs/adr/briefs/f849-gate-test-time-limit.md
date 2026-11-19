# Brief: F-849, the two immersed-solid tests at the per-test limit of the full gate

Branch: f849-gate-test-time-limit
Items: F-849
ADR: 0740
Rules version: 3
Model: Claude Fable 5.2
Issued: 2026-11-19
Size (lead): done; 0; 2; 0; 280627

The rules are `docs/process/agent-rules.md`, version 3, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f849-gate-test-time-limit` (the lead creates it from main). Scratch folder: `E:/projects/cfd/w49i-work/f849-gate-test-time-limit-scratch`.

## Reading list

- docs/process/agent-rules.md 11903
- docs/adr/0218-f219-registration-immersed-solid-with-modified-forcing.md 37025
- docs/adr/0249-f346-registration-immersed-force-in-unsteady-flow.md 60657
- docs/adr/0653-f723-registration-daily-full-gate-with-failures-attributed-to-adrs.md 42010
- scripts/gate_test.py 20545
- .github/workflows/gate.yml 15426
- crates/rcfd-fv/tests/immersed_solid_f219.rs 40670
- crates/rcfd-fv/tests/immersed_unsteady_force_f346.rs 49413
- docs/process/tolerances.md 2978

## Task

Item F-849 of `docs/roadmap.json` (read its acceptance and note first; they carry the measured times and their runs).
Register in `docs/adr/0740-f849-registration-<slug>.md` and choose route (a), route (b) or both, with the reason:

- Route (a), a shorter set-up, changes what a registered test computes. Rule 2 holds: the existing tests are not edited. A
  shorter test is a new test registered here and by an amendment appended to ADR 0218 and ADR 0249, written and committed
  before its run, with every physical limit of those ADRs unchanged and the measured values before and after recorded. The
  long tests stay; say in the registration what they become (they may not be ignored, skipped or removed to make the gate
  pass, so if they stay in the gate their time limit is route (b)).
- Route (b), a higher limit for these two tests only, is a change of `scripts/gate_test.py` or of the workflow's
  `--timeout-min`. The job limit of 340 min has to hold the longer attribution re-run (ADR 0653 Q3). Register the limit
  from the measured times: the worst seen is a time-out at 3600 s, so derive the limit from the spread between runners,
  not from one run.

The acceptance asks for three consecutive full gates on windows-latest. You cannot run them: register the check so that the
lead can read it from the `times.tsv` of three gates, and propose partial until they exist. Measure locally what you can
(each of the two tests once in release on this machine counts as a heavy run; the size allows two). `scripts/test_gate_test.py`
and its neighbours are the tests of the gate scripts. Do not start a workflow run.
