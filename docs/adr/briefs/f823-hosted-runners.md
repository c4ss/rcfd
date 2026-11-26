# Brief: F-823, the Studio's job on GitHub's hosted runners: the registered skips and the Windows timing and GPU tests

Branch: f823-hosted-runners
Items: F-823
ADR: 0818
Rules version: 5
Model: Claude Opus 5.5
Issued: 2026-11-26
Size (lead): done; 0; 0; 0; 59465

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f823-hosted-runners`, on the branch `f823-hosted-runners`, which the lead made at the commit that adds this brief; you start in it. Do not touch the main checkout or another worktree. Scratch folder: `E:/projects/cfd/w49i-work/f823-hosted-runners-scratch`.

## Goal

The lead starts your work as a background subagent, with this line as its prompt:

    /goal The brief docs/adr/briefs/f823-hosted-runners.md (F-823, ADR 0818) is carried out on the branch f823-hosted-runners, checked out in E:/projects/cfd/rcfd/.claude/worktrees/f823-hosted-runners, and this conversation shows each of these with the command and its output: (1) AGENTS.md, docs/process/agent-rules.md (version 5) and the brief were read in full before any other work, and read again in full after any summary of the conversation; (2) `git log --reverse --stat origin/main..HEAD` shows the registration of ADR 0818 as the first commit of this work, alone, and every amendment before the code or run it covers; (3) every registered check is listed with its limit, its measured result and met or missed as it came out, and a miss recorded as a miss meets this goal: no limit changed after a run, no test ignored, skipped, removed, loosened or run again to pass; (4) every registered mutation is listed with the test that caught it, or as not caught; (5) the output of the registered run is shown of `npm run typecheck` and `npm test` in apps/studio-web, `actionlint`, `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and `python scripts/process_report.py --check`, where only a generated file that is not current may fail; a run already shown is not made again; (6) `git status --short` is empty, nothing is pushed or merged, docs/roadmap.json, the ledger, goldens and existing tests are unchanged except as the brief's list of owned files says, and no process this work started is left running; (7) the last message is the report the brief asks for, with the head sha and the verdict done or partial by the registered rule. Stop before that, with the reason reported, where the brief or the rules say stop and report, or after 100 turns.

You run without a person to ask: an action you are denied is left undone and is in your report. End only processes you started. A command of more than two minutes runs in the background and is waited for. Name your model in the registration.

## Reading list

- AGENTS.md 1698
- docs/process/agent-rules.md 12991
- .github/workflows/ci.yml 13387
- docs/adr/0812-f823-registration-case-facts-refuses-by-the-counts-estimate.md 31389

Also in the repository, as needed: ADRs 0802, 0804 and 0809 (the studio-web job and its skip step), ADR 0807 amendment 1 (the
skipped record of `canvas-f979.spec.ts`), the failing specs (`provenance-f828.spec.ts`, `workspace-first-open.spec.ts`,
`run-control.spec.ts`, `workspace.spec.ts`, `responsiveness.spec.ts`, `a11y.spec.ts`) with the sources of their limits, and the
logs of the GitHub Actions runs 45886730170, 45891872417 and 45894509968 (`gh run view <id> --log-failed`, and their
artifacts with `gh run download`).

## What the lead decided (2026-11-26)

On ADR 0809's question 2 the lead read the first GitHub Actions runs of the studio-web job (runs 45886730170 at 3b6cc7cc,
45891872417 at a010ee59, 45894509968 at 911476f3), as ADR 0812 question 2 also asks for run-control.spec.ts:268. On
ubuntu-24.04 every test passed (890 in the report), and the step "no test skipped but the registered ones" failed: the four
tests of `canvas-f979.spec.ts` that ADR 0807 amendment 1 keeps as a skipped record are not in `STUDIO_ALLOWED_SKIPS`. On
windows-latest these failed: provenance-f828.spec.ts:288 in all three runs; workspace-first-open.spec.ts:59 in all three
("no WebGPU adapter: the GPU process cannot be held"); run-control.spec.ts:268 (P2) and workspace.spec.ts:389 in two;
responsiveness.spec.ts:260 and a11y.spec.ts:427 in one each. Locally, on the maintainer's machine, these pass. It is a
further registration under F-823 (rule 13). No test is ignored, skipped or loosened to pass; a limit changes only from a
source (rule 15) by an amendment; a known miss becomes a record only by an amendment (rule 2).

## Task

One registration, `docs/adr/0818-f823-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before any change**: from the logs and the artifacts of the three runs (error contexts, traces), each
   failure's cause as a hypothesis with its evidence (a hosted runner without a GPU adapter; the runner's CPU against a limit
   measured on another machine; a race the runner's timing exposes; a defect), and for each the remedy it points to: a
   deterministic wait or a fix where it is a race or a defect; the test run where its premise holds (for example a WebGPU
   test where an adapter exists, with the condition stated and checked, never a skip to pass) by amendment; or a known miss
   recorded. The four skipped tests of ADR 0807 enter `STUDIO_ALLOWED_SKIPS` by name, with the ADR that keeps them. Any local
   reproduction (for example with the CPU restricted or the GPU disabled) is registered with its number of runs.
2. **Change** only what the registration names. No limit changes without its source; no unregistered skip.
3. Mutations on scratch copies, each recorded with what caught it; a committed patch if a Studio file changed (rule 16).
4. Run the checks as registered; record the CPU load with each timed run. The lead runs the job on GitHub from the branch
   before the merge (workflow_dispatch); its result decides with yours.

Decision rule to register: done if every row holds locally and in the lead's run on GitHub and each mutation is caught;
partial otherwise.

Open questions for the maintainer, with options and a recommendation: any question of scope.

Files you own: the studio-web job of `.github/workflows/ci.yml`, the failing specs and the app files the registration names,
ADR 0818, `docs/roadmap/proposals/0818-F-823.json`. Not the canvas files: F-977 works on them.

## Common to this work

The acceptance of F-823 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
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
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0818-F-823.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; mutations or negative controls; deviations; what was not run; open questions with options and a
recommendation; the result of `python scripts/process_report.py --check`; your verdict by the registered rule.
