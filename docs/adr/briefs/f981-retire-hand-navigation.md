# Brief: F-981, Retire G15's coverage test and the Studio's hand-only surfaces

Branch: f981-retire-hand-navigation
Items: F-981
ADR: 0808
Rules version: 5
Model: Claude Opus 5.5
Issued: 2026-11-26
Size (lead): done; 0; 0; 0; 129193

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f981-retire-hand-navigation`, on the branch `f981-retire-hand-navigation`, which the lead made at the commit that adds this brief; you start in it. Do not touch the main checkout or another worktree. Scratch folder: `E:/projects/cfd/w49i-work/f981-retire-hand-navigation-scratch`.

## Goal

The lead starts your work as a background subagent, with this line as its prompt:

    /goal The brief docs/adr/briefs/f981-retire-hand-navigation.md (F-981, ADR 0808) is carried out on the branch f981-retire-hand-navigation, checked out in E:/projects/cfd/rcfd/.claude/worktrees/f981-retire-hand-navigation, and this conversation shows each of these with the command and its output: (1) AGENTS.md, docs/process/agent-rules.md (version 5) and the brief were read in full before any other work, and read again in full after any summary of the conversation; (2) `git log --reverse --stat origin/main..HEAD` shows the registration of ADR 0808 as the first commit of this work, alone, and every amendment before the code or run it covers; (3) every registered check is listed with its limit, its measured result and met or missed as it came out, and a miss recorded as a miss meets this goal: no limit changed after a run, no test ignored, skipped, removed, loosened or run again to pass; (4) every registered mutation is listed with the test that caught it, or as not caught; (5) the output of the registered run is shown of `npm run typecheck` and `npm test` in apps/studio-web, `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and `python scripts/process_report.py --check`, where only a generated file that is not current may fail; a run already shown is not made again; (6) `git status --short` is empty, nothing is pushed or merged, docs/roadmap.json, the ledger, goldens and existing tests are unchanged except as the brief's list of owned files says, and no process this work started is left running; (7) the last message is the report the brief asks for, with the head sha and the verdict done or partial by the registered rule. Stop before that, with the reason reported, where the brief or the rules say stop and report, or after 100 turns.

You run without a person to ask: an action you are denied is left undone and is in your report. End only processes you started. A command of more than two minutes runs in the background and is waited for. Name your model in the registration.

## Reading list

- AGENTS.md 1698
- docs/process/agent-rules.md 12991
- docs/studio-guardrails.md 15516 (G15 and G19 retired, G16, G20, G23)
- docs/adr/0803-the-studio-as-claudes-canvas-record.md 5545
- apps/studio-web/tests/unit/capabilities.spec.ts 11742 (the coverage test of G15)
- apps/studio-web/capabilities/coverage.json 81701

Also in the repository, as needed: `apps/studio-web/scripts/write-capabilities.mjs` and `apps/studio-web/tests/support/
capabilities.ts` (the inventory and the coverage report), `docs/studio-coverage.md`, the renderer's folders under
`apps/studio-web/src/renderer/` (`commands`, `help`, `first`, `settings`, `case`, `actions`, `workspace`), and the e2e specs
that drive them.

## What the lead decided (2026-11-26)

ADR 0803 records the maintainer's answer: the Studio's views stay and hand navigation goes. G15 (every capability reachable
by hand) and its coverage test are retired, with the menus, editors and navigation built only to reach a capability by
hand. The views the canvas uses (plots, field and mesh views, run lists as views, the theme), the inventory of
capabilities drawn from the engine (F-824: it still says what the engine can do), the short ways to look (pan, zoom, scrub,
pick, provenance) and every named action of G16 and G20 stay. Which surfaces are hand-only is this registration's to list,
each with its file and why; a surface the canvas or the F-976 e2e test uses is not hand-only.

## Task

One registration, `docs/adr/0808-f981-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before any change**: the list of hand-only surfaces (file, what it does, why it is hand-only), the list of
   what stays and why, every test that is retired or removed with its file and the reason (the maintainer's answer of
   2026-11-26, ADR 0803), and the rows: npm test passes; the canvas's e2e test (`canvas-f976.spec.ts`) and the views' tests
   pass; the inventory test against the engine still passes; the coverage report no longer requires a UI path (G15) and the
   inventory still lists every capability; the Studio opens a project and shows a view by `show_view`.
2. **Change** the Studio only: remove or retire the surfaces and tests as registered; the coverage map's G15 check is retired,
   the inventory kept. No change under `crates/`. A retired test is kept as a skipped record only where the registration says
   why it is kept; a removed test is named in the ADR.
3. Mutations on scratch copies: a retired surface's route left reachable; the inventory check dropped; the canvas test made
   to depend on a removed surface; each recorded with what caught it. Commit one as a patch (rule 16).
4. Run `npm run typecheck`, `npm test` in `apps/studio-web` and `docs_site`.

Decision rule to register: done if every row holds and each mutation is caught; partial otherwise.

Open questions for the maintainer, with options and a recommendation: a surface whose status is a question of scope.

Files you own: the hand-only surfaces and their tests as registered, `apps/studio-web/tests/unit/capabilities.spec.ts`,
`apps/studio-web/capabilities/coverage.json`, `apps/studio-web/tests/support/capabilities.ts`,
`apps/studio-web/scripts/write-capabilities.mjs`, `docs/studio-coverage.md`, ADR 0808, `docs/roadmap/proposals/0808-F-981.json`.
Not the canvas files (`apps/studio-web/src/renderer/canvas/`, the canvas parts of `session.ts` and `shared/project.ts`): F-979
works on them at the same time.

## Common to this work

The acceptance of F-981 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
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
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0808-F-981.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; mutations or negative controls; deviations; what was not run; open questions with options and a
recommendation; the result of `python scripts/process_report.py --check`; your verdict by the registered rule.
