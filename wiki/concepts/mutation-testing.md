---
type: concept
tags: [process, evidence, testing]
sources: [doc-process-agent-rules, adr-thread-structural-materials-nonlinear, adr-thread-structural-linear-solvers, adr-thread-structural-beams-shells]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Planned mutations (mutation testing in registrations)

Every [[registration]] lists negative controls and planned mutations: deliberate defects of the new code that a registered check
must catch. The mutations are run on a scratch copy and what caught each is recorded; one of them is committed as a patch file beside
the test it must fail (`<test file stem>.mutation.patch`, rule 16) ([[doc-process-agent-rules]]).

## How rcfd uses it

- In the structural ADRs the decision rule makes an item **partial** when a planned mutation is not caught by its registered check,
  even when every limit is met. Examples: F-470 (M5), F-529 (M11), F-594 (M5 caught by L4 (b) but not E4 (a)), F-537 (Mu7), F-582
  (MU7), F-513 (MU7) ([[adr-thread-structural-linear-solvers]], [[adr-thread-structural-materials-nonlinear]],
  [[adr-thread-structural-beams-shells]]).
- A mutation that changes no behaviour on the set-up is called "equivalent"; the ADRs record the diagnosis but do not change the rule
  after the run: "that Mu7 is equivalent (no behaviour to catch) is the diagnosis, not a reason to change the rule after the run"
  (ADR 0457) ([[adr-thread-structural-materials-nonlinear]]).
- Whether such items are done is left to the maintainer ("The maintainer decides done") ([[adr-thread-structural-materials-nonlinear]]).

## Related

- [[registration]], [[known-miss]], [[registered-limit]], [[roadmap-status]]
