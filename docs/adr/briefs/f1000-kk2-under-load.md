# Brief: F-1000, KK2 of provenance-escape-f988.spec.ts under load in a whole npm test

Branch: f1000-kk2-under-load
Items: F-1000
ADR: 0843
Rules version: 5
Model: Claude Opus 5.5
Issued: 2026-11-26
Size (lead): done; 1; 0; 0; 49317

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f1000-kk2-under-load`, on the branch `f1000-kk2-under-load`, which the lead made at the commit that adds this brief; you start in it. Do not touch the main checkout or another worktree. Scratch folder: `E:/projects/cfd/w49i-work/f1000-kk2-under-load-scratch`.

## Goal

The lead starts your work as a background subagent, with this line as its prompt:

    /goal The brief docs/adr/briefs/f1000-kk2-under-load.md (F-1000, ADR 0843) is carried out on the branch f1000-kk2-under-load, checked out in E:/projects/cfd/rcfd/.claude/worktrees/f1000-kk2-under-load, and this conversation shows each of these with the command and its output: (1) AGENTS.md, docs/process/agent-rules.md (version 5) and the brief were read in full before any other work, and read again in full after any summary of the conversation; (2) `git log --reverse --stat origin/main..HEAD` shows the registration of ADR 0843 as the first commit of this work, alone, and every amendment before the code or run it covers; (3) every registered check is listed with its limit, its measured result and met or missed as it came out, and a miss recorded as a miss meets this goal: no limit changed after a run, no test ignored, skipped, removed, loosened or run again to pass; (4) every registered mutation is listed with the test that caught it, or as not caught; (5) the output of the registered run is shown of fmt and clippy of the crates touched, their `--lib` tests and the named test files, and the Studio's `npm test`, `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and `python scripts/process_report.py --check`, where only a generated file that is not current may fail; a run already shown is not made again; (6) `git status --short` is empty, nothing is pushed or merged, docs/roadmap.json, the ledger, goldens and existing tests are unchanged except as the brief's list of owned files says, and no process this work started is left running; (7) the last message is the report the brief asks for, with the head sha and the verdict done or partial by the registered rule. Stop before that, with the reason reported, where the brief or the rules say stop and report, or after 100 turns.

You run without a person to ask: an action you are denied is left undone and is in your report. End only processes you started. A command of more than two minutes runs in the background and is waited for. Name your model in the registration.

## Reading list

- AGENTS.md 1698
- docs/process/agent-rules.md 12991
- docs/adr/0813-f988-registration-a-ctrl-i-pressed-before-escape-reopens-the-popover.md 29850
- apps/studio-web/tests/e2e/provenance-escape-f988.spec.ts 4778

Also in the repository, as needed: the provenance registry and popover of the Studio's renderer (ADR 0813's V2, one record per
pressed key), `provenance-f828.spec.ts`, ADR 0808 Addendum 8, the push log lines of F-273's merge in the ledger (2026-11-27),
the Studio's Playwright configuration (workers, timeouts).

## What the lead decided (2026-11-28)

In the push of F-273's merge (2026-11-27) the first whole `npm test` failed at KK2 of `provenance-escape-f988.spec.ts`
("two keys in flight: a popover open after Escape", the machine at about 100 % CPU) and at run-control.spec.ts:268's P2 (a
timing spec of F-991); the second run passed. F-988 (ADR 0813) had KK2 pass 5 of 5 and the spec 10 of 10 alone. The lead
entered F-1000 and issues it, on the maintainer's standing instruction of 2026-11-26 (go with the recommended option always).

## Task

One registration, `docs/adr/0843-f1000-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before any run**: the hypotheses for KK2's failure under load (for example a third key or a late request that
   V2's one record per key does not cover, a timing of the test's own waits, the order of Escape and the request's answer),
   each with what it predicts and an arm that decides it (a trace of the keys and requests in flight at the failure, KK2
   under a registered CPU load alone and in a whole `npm test`); the remedy each points to; the runs: KK2 alone at least 10
   under the load, and whole `npm test` runs under the load (at least 5), every one passing; negative controls and mutations
   (rule 16), one committed as a patch whose expect-fail command builds and packages the app first; the numbers fixed in
   advance (rule 15). The case is not loosened: no wait lengthened past a limit an earlier ADR fixed, no assertion removed.
2. **Run** as registered. A defect found in the renderer is fixed only if the registration names the fix and its check.
3. Every test that starts a runner ends it (ADR 0828's helpers).

Decision rule to register: done for F-1000 if the cause is found, the remedy holds in every registered run and each mutation
is caught; partial otherwise.

Open questions for the maintainer, with options and a recommendation: any question of scope.

Files you own: `provenance-escape-f988.spec.ts`, the provenance registry and popover files the registration names, new tests,
ADR 0843, `docs/roadmap/proposals/0843-F-1000.json`. No solver files. You are the one agent running the Studio's `npm test`
now.

## Common to this work

The acceptance of F-1000 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
decision rule is registered before the run and is not changed after it; a result that misses is recorded as a miss with
its number; a run that does not decide is recorded as not deciding. No test is ignored, skipped, removed or quarantined
to make a run pass; a registered miss is held as the repository holds known misses (rule 2), with its limit intact.

Clean-room: reference values only from published sources that were read and that a page of the repository records with
their citation; nothing from memory (rule 4); vendor documentation is never a source.

Use your own target folder under the scratch folder for every cargo command (`CARGO_TARGET_DIR`). Other agents run on
this machine at the same time and own other files; a test that fails on a file refused by the operating system under
that load is reported, not retried until it passes. Delete nothing outside your worktree and your scratch folder.

The lead merges with `scripts/push_main.py`. Do not push and do not merge. Commit messages are plain: no agent marker,
no trailers. Rule 9: you do not edit `docs/roadmap.json`, `docs/milestones.json` or a generated page; you write your
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0843-F-1000.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; mutations or negative controls; deviations; what was not run; open questions with options and a
recommendation; the result of `python scripts/process_report.py --check`; your verdict by the registered rule.
