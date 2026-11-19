# Brief: F-672, the Beam2 refusal of the cyclic constraint lifted by default

Branch: f672-beam2-cyclic-default
Items: F-672
ADR: 0744
Rules version: 3
Model: Claude Fable 5.2
Issued: 2026-11-19
Size (lead): done; 0; 0; 0; 167690

The rules are `docs/process/agent-rules.md`, version 3, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f672-beam2-cyclic-default` (the lead creates it from main). Scratch folder: `E:/projects/cfd/w49i-work/f672-beam2-cyclic-default-scratch`.

## Reading list

- docs/process/agent-rules.md 11903
- docs/adr/0595-f672-registration-beam2-elements-in-the-cyclic-constraint.md 28935
- docs/adr/0556-f628-registration-shells-in-the-cyclic-constraint.md 27070
- docs/adr/0593-f666-registration-hyperelastic-structural-elements-on-the-cyclic-rotating-cms-and-mpc-paths.md 31004
- crates/rcfd-fem/src/cyclic.rs 68778

## Task

A further registration of F-672: `docs/adr/0744-f672-registration-<slug>.md`. The maintainer approved the default change on
2026-11-18: the refusal of Beam2 elements in the cyclic constraint is lifted by default, which retires ADR 0556 NC3 and
ADR 0593 A-B as assertions of the refusal.

- The registration lists the changed results case by case and the records it re-records, before the code: each test that
  asserts the refusal, what it becomes (an ignored record, with a copy that asserts acceptance and the ring result of
  ADR 0595 within its 1e-10), and what still refuses Beam2 afterwards (ADR 0595's "not covered": full_rotor with beams, the
  rotating paths on a beam sector, beam ends on interpolated or axis boundaries, line loads in the sector).
- Every model without Beam2 in a cyclic constraint keeps its bits; show it with the existing bit records and
  `baseline_hashes`.
- F-666's default lift (hyperelastic structural elements by default, RF1 retired) is not part of this brief; it follows this
  one because it touches the same refusal tests. Say in the addendum what F-666 will find changed.
