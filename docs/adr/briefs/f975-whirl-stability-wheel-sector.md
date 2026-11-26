# Brief: F-975, whirl_stability of the cyclic beam wheel sector converges

Branch: f975-whirl-stability-wheel-sector
Items: F-975
ADR: 0806
Rules version: 5
Model: Claude Opus 5.5
Issued: 2026-11-26
Size (lead): done; 1; 0; 0; 88625

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f975-whirl-stability-wheel-sector`, on the branch `f975-whirl-stability-wheel-sector`, which the lead made at the commit that adds this brief; you start in it. Do not touch the main checkout or another worktree. Scratch folder: `E:/projects/cfd/w49i-work/f975-whirl-stability-wheel-sector-scratch`.

## Goal

The lead starts your work as a background subagent, with this line as its prompt:

    /goal The brief docs/adr/briefs/f975-whirl-stability-wheel-sector.md (F-975, ADR 0806) is carried out on the branch f975-whirl-stability-wheel-sector, checked out in E:/projects/cfd/rcfd/.claude/worktrees/f975-whirl-stability-wheel-sector, and this conversation shows each of these with the command and its output: (1) AGENTS.md, docs/process/agent-rules.md (version 5) and the brief were read in full before any other work, and read again in full after any summary of the conversation; (2) `git log --reverse --stat origin/main..HEAD` shows the registration of ADR 0806 as the first commit of this work, alone, and every amendment before the code or run it covers; (3) every registered check is listed with its limit, its measured result and met or missed as it came out, and a miss recorded as a miss meets this goal: no limit changed after a run, no test ignored, skipped, removed, loosened or run again to pass; (4) every registered mutation is listed with the test that caught it, or as not caught; (5) the output of the registered run is shown of fmt and clippy of rcfd-fem, its `--lib` tests, the named test files and `baseline_hashes`, `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and `python scripts/process_report.py --check`, where only a generated file that is not current may fail; a run already shown is not made again; (6) `git status --short` is empty, nothing is pushed or merged, docs/roadmap.json, the ledger, goldens and existing tests are unchanged except as the brief's list of owned files says, and no process this work started is left running; (7) the last message is the report the brief asks for, with the head sha and the verdict done or partial by the registered rule. Stop before that, with the reason reported, where the brief or the rules say stop and report, or after 100 turns.

You run without a person to ask: an action you are denied is left undone and is in your report. End only processes you started. A command of more than two minutes runs in the background and is waited for. Name your model in the registration.

## Reading list

- AGENTS.md 1698
- docs/process/agent-rules.md 12991
- docs/adr/0799-f672-registration-whirl-and-full-rotor-of-a-beam-sector-by-default.md 40425 (its addenda: the E-W miss, P7)
- crates/rcfd-fem/tests/sa_beam_opt_ins_default_f672.rs 33511 (the known miss `e_w_..._known_miss` and its sector W)

Also in the repository, as needed: ADR 0595 (the wheel sector W), `crates/rcfd-fem/src/rotating/whirl.rs` and the
gyroscopic state eigen solve (`gyroscopic_state_eigen`, its QR iteration and its limit of 30 steps), `docs/process/tolerances.md`,
the tests of `whirl_state` and `whirl_stability`.

## What the lead decided (2026-11-26)

F-672 (ADR 0799) is partial on one row: `whirl_stability` of the wheel sector W at harmonic indices 0 to 3 and 0, 30 and 60
rad/s is a solver error ("QR iteration did not converge on rows 116..=119 in 30 steps"), on main as well. F-975 is the
follow-up the agent proposed, entered as written. It is on line 4 of the priority statement (close partial items).

## Task

One registration, `docs/adr/0806-f975-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before any code**: the diagnosis, read from the code and shown by a registered control: why the QR iteration
   does not converge on these rows (the matrix's structure: repeated or nearly repeated eigenvalues, a zero block, scaling),
   and the remedy from a published algorithm with its place in the source (rule 4) or from first principles; the rows of
   F-975's acceptance, with the agreement limit from the tolerance table; the defaults bit-identical where the remedy is not
   taken (a model whose iteration converged now keeps its bits, shown by `baseline_hashes` and the named tests); negative
   controls and mutations (rule 16).
2. **Change** the eigen solve as registered, opt-in only if the remedy would change a result that converges today.
3. Run the known miss of ADR 0799 as a further registered check of this work (a new test that calls the same set-up, not an
   edit of the ignored record), and the checks rule 7 lists.

Decision rule to register: done if every row holds and each mutation is caught; partial otherwise.

Open questions for the maintainer, with options and a recommendation, not decided by you: a remedy that changes a default
result.

Files you own: the eigen solve's file under `crates/rcfd-fem/src/` (lines named in the registration), new test files,
ADR 0806, `docs/roadmap/proposals/0806-F-975.json`. Not the ignored record of ADR 0799.

## Common to this work

The acceptance of F-975 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
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
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0806-F-975.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; mutations or negative controls; deviations; what was not run; open questions with options and a
recommendation; the result of `python scripts/process_report.py --check`; your verdict by the registered rule.
