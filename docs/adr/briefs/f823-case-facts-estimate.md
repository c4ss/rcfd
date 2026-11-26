# Brief: F-823, case.facts refuses by the counts' estimate before it builds the mesh

Branch: f823-case-facts-estimate
Items: F-823
ADR: 0812
Rules version: 5
Model: Claude Opus 5.5
Issued: 2026-11-26
Size (lead): done; 1; 0; 0; 64887

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f823-case-facts-estimate`, on the branch `f823-case-facts-estimate`, which the lead made at the commit that adds this brief; you start in it. Do not touch the main checkout or another worktree. Scratch folder: `E:/projects/cfd/w49i-work/f823-case-facts-estimate-scratch`.

## Goal

The lead starts your work as a background subagent, with this line as its prompt:

    /goal The brief docs/adr/briefs/f823-case-facts-estimate.md (F-823, ADR 0812) is carried out on the branch f823-case-facts-estimate, checked out in E:/projects/cfd/rcfd/.claude/worktrees/f823-case-facts-estimate, and this conversation shows each of these with the command and its output: (1) AGENTS.md, docs/process/agent-rules.md (version 5) and the brief were read in full before any other work, and read again in full after any summary of the conversation; (2) `git log --reverse --stat origin/main..HEAD` shows the registration of ADR 0812 as the first commit of this work, alone, and every amendment before the code or run it covers; (3) every registered check is listed with its limit, its measured result and met or missed as it came out, and a miss recorded as a miss meets this goal: no limit changed after a run, no test ignored, skipped, removed, loosened or run again to pass; (4) every registered mutation is listed with the test that caught it, or as not caught; (5) the output of the registered run is shown of fmt and clippy of the crates touched, their `--lib` tests, the named test files and `baseline_hashes`, `npm run typecheck` and `npm test` in apps/studio-web, `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and `python scripts/process_report.py --check`, where only a generated file that is not current may fail; a run already shown is not made again; (6) `git status --short` is empty, nothing is pushed or merged, docs/roadmap.json, the ledger, goldens and existing tests are unchanged except as the brief's list of owned files says, and no process this work started is left running; (7) the last message is the report the brief asks for, with the head sha and the verdict done or partial by the registered rule. Stop before that, with the reason reported, where the brief or the rules say stop and report, or after 100 turns.

You run without a person to ask: an action you are denied is left undone and is in your report. End only processes you started. A command of more than two minutes runs in the background and is waited for. Name your model in the registration.

## Reading list

- AGENTS.md 1698
- docs/process/agent-rules.md 12991
- crates/rcfd-api/src/facts.rs 38680
- crates/rcfd-api/src/mesh_preview.rs 11518 (the estimate before the build and ESTIMATE_LIMIT)

Also, as needed: ADR 0809 with its addendum (on the branch f823-studio-ci-engine, `git show f823-studio-ci-engine:docs/adr/0809-f823-registration-error-walk-time-under-webgpu.md`), `crates/rcfd-api/src/service.rs` (one connection's requests answered in order),
`crates/rcfd-workspace/src/facts.rs`, the case editor's request of `case.facts` in `apps/studio-web`,
`apps/studio-web/tests/e2e/error-walk.spec.ts`, ADRs 0802 and 0804.

## What the lead decided (2026-11-26)

ADR 0809 found that `error-walk.spec.ts:177` waits on the engine, not on WebGPU: the case editor asks for `case.facts` after
each settings change, `case.facts` builds the whole mesh before its `max_cells` check (`facts.rs` 384-386; 2.25 million
cells at `ny` 1500), and the service answers one connection's requests in order, so the preview's refusal waits behind the
build. On its question 1 the lead takes the recommendation (a), on the maintainer's standing instruction of 2026-11-26 (go
with the recommended option always): a further registration of F-823 (rule 13) in which `case.facts` refuses by the counts'
estimate before it builds a generator mesh, as `mesh.preview` does. The reply changes by addition only where the case is
over the limit (G4: an engine interface changes only by addition; the refusal's code and key are registered); every result
of a case under the limit is unchanged bit for bit. On its question 2, (a): the lead reads the first GitHub Actions run of
the Linux job before any registration of the time limits under load.

## Task

One registration, `docs/adr/0812-f823-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before any code**: the estimate (the same one `mesh.preview` uses, from the `mesh.n*` counts), where it is
   applied in `case_facts`, the refusal it gives and its relation to the existing `max_cells` refusal (same code and key, or
   an added one, with its reason), the cases it covers (generator meshes) and those it does not (a mesh file: unchanged);
   checks: a two-route check that the estimate equals the built count on registered cases under the limit; the over-limit
   case is refused without a build (time and peak memory measured, with the limit's source); every reply under the limit
   unchanged (golden replies or `baseline_hashes` as they apply); `error-walk.spec.ts:177` within its unchanged 60 s on
   Windows and in the Ubuntu 24.04 container; mutations.
2. **Build** it in `crates/rcfd-api/src/facts.rs` (and the workspace mapping if a code is added). No Studio change unless
   registered.
3. Mutations on scratch copies, each recorded with what caught it; one committed as a patch (rule 16).
4. Run the checks rule 7 lists for the files touched and the registered e2e runs.

Decision rule to register: done if every row holds and each mutation is caught; partial otherwise.

Open questions for the maintainer, with options and a recommendation: any question of scope.

Files you own: `crates/rcfd-api/src/facts.rs`, `crates/rcfd-workspace/src/facts.rs` if a code is mapped, new tests, ADR
0812, `docs/roadmap/proposals/0812-F-823.json`.

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
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0812-F-823.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; mutations or negative controls; deviations; what was not run; open questions with options and a
recommendation; the result of `python scripts/process_report.py --check`; your verdict by the registered rule.
