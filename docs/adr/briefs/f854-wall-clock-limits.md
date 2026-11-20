# Brief: F-854, two wall-clock limits that fail once on a loaded runner

Branch: f854-wall-clock-limits
Items: F-854
ADR: 0749
Rules version: 3
Model: Claude Fable 5.2
Issued: 2026-11-20
Size (lead): done; 0; 0; 2; 111288

The rules are `docs/process/agent-rules.md`, version 3, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f854-wall-clock-limits`, a new branch from main at the commit that adds this brief. Scratch folder: `E:/projects/cfd/w49i-work/f854-wall-clock-limits-scratch`.

## Reading list

- docs/process/agent-rules.md 11903
- crates/rcfd-fv/tests/vof_dambreak_amg.rs 5129
- crates/rcfd-post/tests/monitors_filtered_f456.rs 5568
- docs/adr/0052-wave28a-vof-wiring.md 30458 (A4, registered at line 72 onwards)
- docs/adr/0353-f456-registration-monitors-get-builds-only-what-is-asked.md 25614 (I2)
- docs/adr/0229-runner-liveness-identity-status-and-append-retries-and-the-shutdown-start-race.md 16513 (section "Gate (2026-11-12)": an earlier failure of A4 under load)
- .github/workflows/gate.yml 16103 (how the gate runs the tests and how many at once; read only, not to be changed)

Outside the repository, for reading: the artifacts of the gate in `E:/projects/cfd/w49i-work/gate-46cea4f8` (per shard
`test.log`, `times.tsv`, `attribution.md`); for comparison `gate-10ba1a34` and `gate-252c9279` in the same folder.

## What happened

The full gate of main at `46cea4f8` (run 44756958425, four shards, 2026-11-20) is red on two tests alone: 5933 passed,
2 failed, 1726 ignored. Both are limits on a wall-clock time, both passed on the gate's own re-run (the attribution calls
them flaky), and both passed three of three times on the lead's machine with nothing else running:

- `rcfd-fv` `vof_dambreak_amg::a4_dam_break_with_amg`, the assertion "A4 wall time" (`am.secs <= cg.secs`, ADR 0052 A4:
  "wall time at most that of plain CG"). Runner: AMG 35.1 s against CG 26.8 s, with 12.4 against 157.9 mean iterations per
  corrector; the binary took 62.1 s against 39.0 s and 44.6 s in the two gates before. Lead's machine: 7.5 to 8.0 s
  against 19.6 to 20.1 s. The same assertion failed once before under load (ADR 0229, gate of 2026-11-12).
- `rcfd-post` `monitors_filtered_f456::f456_i2_storing_an_increment_of_100_samples_at_1e5` (ADR 0353 I2: median of 200
  repeats at most 2 ms). Runner: median 2.0807 ms, max 3.3185 ms. Lead's machine: median 0.91 to 0.98 ms.

The two runs of A4 (CG, then AMG) are timed one after the other, so a load that arrives during the second is counted
against AMG alone. Whether that is the whole cause is not shown; it is the lead's reading and is yours to test.

## Task

One registration, `docs/adr/0749-f854-registration-<slug>.md`, the first commit of this work, alone.

- Show the cause for each test before changing anything: a registered number of repeated runs with nothing else running,
  and the same number under a registered, reproducible load on the machine (for example a fixed number of processes that
  keep every core busy), with the number of failures of each. If a test does not fail under any load you can register,
  say so and report the timings; do not invent a cause.
- Decide and register, for each test, a measurement that still checks the claim its ADR makes (AMG is not slower than
  plain CG; storing an increment costs at most 2 ms) and does not fail by the load of the machine alone. Candidates, not
  instructions: the best of a registered number of interleaved repeats; the time of the solves alone and not of the whole
  run; a time measure that does not count waiting. A measure that could no longer fail when the code gets slower is not
  acceptable: the registration names, for each test, a slowdown of the code that the new measurement must still catch,
  and a mutation shows it (with its committed patch and `# expect-fail:` line, as the rules ask).
- The numeric limits stay: "at most that of plain CG" and 2 ms. The change of how the time is measured is an amendment
  of the ADR that owns the limit, appended to ADR 0052 and to ADR 0353 (append-only, nothing above it changed), committed
  before the code. Every other assertion of both test files is untouched.
- Neither test is ignored, skipped, removed or retried by the gate to make it pass, and `gate.yml` is not changed. If the
  honest result for a test is that its claim does not hold under load whatever the measurement, that is a finding:
  report it and leave that test as it is.
- Run each changed test the registered number of times in release without load and under the registered load, and
  report every result.

Decision rule to register: done only if both tests pass every registered repeat with and without the load and each
mutation is caught; partial if one of the two is left as it is with its finding, or a registered run was not made;
stopped and reported if anything other than the measurement of these two times would have to change.

The lead merges with `scripts/push_main.py`. Do not push and do not merge. Commit messages are plain: no agent marker,
no trailers. Run in the background anything that takes longer than a few minutes.

## Report (under 400 words)

The registration's commit, the two amendments and the code commits; for each test the failures out of the repeats before
and after, with and without the load; the measurement chosen and why it still catches a slowdown; mutations; deviations,
each with the commit that precedes its corrected run; what was not run; your verdict by the registered rule.
