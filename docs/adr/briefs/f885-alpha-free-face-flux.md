# Brief: F-885, an opt-in face flux that does not depend on alpha_u, then check H

Branch: f885-alpha-free-face-flux
Items: F-885
ADR: 0811
Rules version: 5
Model: Claude Opus 5.5
Issued: 2026-11-26
Size (lead): done; 1; 0; 0; 84583

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f885-alpha-free-face-flux`, on the branch `f885-alpha-free-face-flux`, which the lead made at the commit that adds this brief; you start in it. Do not touch the main checkout or another worktree. Scratch folder: `E:/projects/cfd/w49i-work/f885-alpha-free-face-flux-scratch`.

## Goal

The lead starts your work as a background subagent, with this line as its prompt:

    /goal The brief docs/adr/briefs/f885-alpha-free-face-flux.md (F-885, ADR 0811) is carried out on the branch f885-alpha-free-face-flux, checked out in E:/projects/cfd/rcfd/.claude/worktrees/f885-alpha-free-face-flux, and this conversation shows each of these with the command and its output: (1) AGENTS.md, docs/process/agent-rules.md (version 5) and the brief were read in full before any other work, and read again in full after any summary of the conversation; (2) `git log --reverse --stat origin/main..HEAD` shows the registration of ADR 0811 as the first commit of this work, alone, and every amendment before the code or run it covers; (3) every registered check is listed with its limit, its measured result and met or missed as it came out, and a miss recorded as a miss meets this goal: no limit changed after a run, no test ignored, skipped, removed, loosened or run again to pass; (4) every registered mutation is listed with the test that caught it, or as not caught; (5) the output of the registered run is shown of fmt and clippy of the crates touched, their `--lib` tests, the named test files and `baseline_hashes`, `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and `python scripts/process_report.py --check`, where only a generated file that is not current may fail; a run already shown is not made again; (6) `git status --short` is empty, nothing is pushed or merged, docs/roadmap.json, the ledger, goldens and existing tests are unchanged except as the brief's list of owned files says, and no process this work started is left running; (7) the last message is the report the brief asks for, with the head sha and the verdict done or partial by the registered rule. Stop before that, with the reason reported, where the brief or the rules say stop and report, or after 100 turns.

You run without a person to ask: an action you are denied is left undone and is in your report. End only processes you started. A command of more than two minutes runs in the background and is waited for. Name your model in the registration.

## Reading list

- AGENTS.md 1698
- docs/process/agent-rules.md 12991
- docs/adr/0810-f885-registration-the-fixed-points-of-the-relaxed-and-unrelaxed-outer-iterations.md 35580
- crates/rcfd-api/tests/f885_outer_fixed_points.rs 34314

Also in the repository, as needed, by section: ADR 0767 (the record and check H's outer rule as written), ADR 0183 (check H),
ADR 0766 (F-879), the pimple step in `crates/rcfd-fv/src/pimple.rs` (the lines ADR 0810 names), the momentum
interpolation in the literature (a public source for an interpolation whose result at convergence does not depend on the
relaxation factor; rule 15).

## What the lead decided (2026-11-26)

ADR 0810 names the cause (H1): `r_au = V/a_p` is built from the relaxed momentum diagonal and enters the face flux through
`gamma_p` and the pressure, transient and body-force parts of the Rhie-Chow interpolation, so continuity's fixed point
depends on alpha_u and the unrelaxed final outer iteration moves to another state. On its question 1 the lead takes the
recommendation (a), on the maintainer's standing instruction of 2026-11-26 (go with the recommended option always): an
opt-in face flux that does not depend on alpha_u, registered under F-885 (rule 13), then check H again. On its question 2,
the same way: the fan's relaxed-loop stall (about 5e-3 to 9e-3 m/s, C = 0.0366 above 1e-2) is recorded in this
registration before any long run, with what it means for ADR 0183's rule if it persists.

## Task

One registration, `docs/adr/0811-f885-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before any code**: the interpolation, from a public source with its equation, and why its converged face flux
   does not depend on alpha_u; the opt-in switch (default off, every default bit-identical, `baseline_hashes` unchanged);
   the checks: on the cavity and 20 steps of H(dt), the controls K0/F0 of ADR 0810 with the switch on (J ABSENT by ADR
   0810's limits) and a two-route check that at a converged outer loop the face flux is the same for two alpha_u; the
   fan's relaxed-loop stall recorded first, with a rule fixed in advance for whether check H can be read; then check H as
   registered in ADR 0183 and ADR 0766 with the switch on, its time and the machine stated before it starts (rule 15).
2. **Build** the switch in the pimple step; no default changes.
3. Mutations on scratch copies, each recorded with what caught it; one committed as a patch (rule 16).
4. **Run** as registered; a long run is waited for in the background.

Decision rule to register: done for F-885 if the switch's rows hold and check H decides a class by ADR 0183's rule with
the switch on; partial otherwise, with the reason.

Open questions for the maintainer, with options and a recommendation: whether the switch becomes the default (a default
change, never in this work), and any question of scope.

Files you own: the opt-in switch in `crates/rcfd-fv/src/pimple.rs` and its settings, new tests and investigation files,
ADR 0811, `docs/roadmap/proposals/0811-F-885.json`. No Studio files.

## Common to this work

The acceptance of F-885 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
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
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0811-F-885.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; mutations or negative controls; deviations; what was not run; open questions with options and a
recommendation; the result of `python scripts/process_report.py --check`; your verdict by the registered rule.
