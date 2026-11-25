# Brief: F-630, shells on the linear dynamic paths

Branch: f630-shells-linear-dynamics
Items: F-630
ADR: 0798
Rules version: 5
Model: Claude Opus 5.5
Issued: 2026-11-25
Size (lead): done; 0; 0; 0; 72767

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f630-shells-linear-dynamics`, which the lead made with the branch `f630-shells-linear-dynamics` at the commit that adds this brief; you start in it. Do not touch the main checkout or another worktree. Scratch folder: `E:/projects/cfd/w49i-work/f630-shells-linear-dynamics-scratch`.

## Goal

Your work is started with this line as its whole prompt:

    /goal The brief docs/adr/briefs/f630-shells-linear-dynamics.md (F-630, ADR 0798) is carried out on the branch f630-shells-linear-dynamics, checked out in this directory, and this conversation shows each of these with the command and its output: (1) AGENTS.md, docs/process/agent-rules.md (version 5) and the brief were read in full before any other work; (2) `git log --reverse --stat origin/main..HEAD` shows the registration of the ADR as the first commit, alone, and every amendment before the code or run it covers; (3) every registered check is listed with its limit, its measured result and met or missed as it came out, and a miss recorded as a miss meets this goal: no limit changed after a run, no test ignored, skipped, removed, loosened or run again to pass; (4) every registered mutation is listed with the test that caught it, or as not caught; (5) the output of the registered run is shown of `cargo fmt` (check) and clippy with `-D warnings` of rcfd-fem, and the tests the brief names, `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and `python scripts/process_report.py --check`, where only a generated file that is not current may fail; a run already shown is not made again; (6) `git status --short` is empty, nothing is pushed or merged, docs/roadmap.json, the ledger, goldens and existing tests are unchanged except as the brief's list of owned files says, and no process this work started is left running; (7) the last message is the report the brief asks for, with the head sha and the verdict done or partial by the registered rule. Stop before that, with the reason reported, where the brief or the rules say stop and report, or after 100 turns.

You run without a person to ask: an action you are denied is left undone and is in your report. End only processes you started. A command of more than two minutes runs in the background and is waited for. Name your model in the registration.

## Reading list

- docs/process/agent-rules.md 12991
- docs/adr/0545-f541-registration-shell3-on-the-remaining-paths.md 32706 (the table of paths, KM1 and RC1)
- docs/adr/0556-f628-registration-shells-in-the-cyclic-constraint.md 27070 (how an opt-in lifted a refusal and kept its tests)

Also in the repository, as needed: `crates/rcfd-fem/src/{model,transient,damped,harmonic,harmonic_direct,modal,mass,assemble,shell,shell3,mpc}.rs`; the tests that hold a refusal of shells on these paths (found by search, `sa_shell3_paths.rs` first) and `sa_cyclic_shells_refusals_adr0744.rs`; ADR 0744 and ADR 0746 (how refusals were lifted by default, on the maintainer's word).

## What the maintainer decided (2026-11-25)

On 2026-11-25 the maintainer answered "Yes" to resuming the roadmap by the priority lines; F-630 is of the structural line. Partial items of that line come first, and those read on 2026-11-25 wait on the maintainer or on numbers that were measured and missed. Decided by the lead (details): the paths take Shell4 and Shell3 by an opt-in, in the form of F-628 and F-629; the default refuses as now, with the same error, so that no existing refusal test changes; a path that already takes shells (the modal solve) gets its check and no option. Lifting the refusal by default is a question for the maintainer.

## Task

One registration, `docs/adr/0798-f630-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before any code or test**, per path (linear transient, damped modes, modal, mode superposition, direct harmonic; plain and on
   the transformation path where the path has one) and per element: what is refused now; a cantilever strip of each element
   against the closed-form Euler-Bernoulli solution (the free-vibration frequency and the decay or the energy over the run, and the harmonic
   tip response off resonance), with the modelling error of the strip (shear, width, the mesh) estimated before the run from the
   theory and the limit set from it, over at least three meshes; the direct harmonic against the superposition with all
   modes within 1e-8; the transient against the modal solution of the same discrete model (two routes); a damping check with a
   closed form; mixed models (a shell and a solid, a shell and a beam) taken or refused by name; layups, warped Shell4
   and the drilling mode, each taken or refused by name and not silently; refusals that stay (rotating, energy-momentum,
   cyclic without its option). Models without shells, and shells without the option, bit for bit against main on
   Windows and Linux x86_64.
2. **Change rcfd-fem**, the smallest change that does it. A closed form comes from a source that was read, or is derived in the registration.
3. **Tests** in new files `crates/rcfd-fem/tests/sa_shells_linear_dynamics_f630.rs` and `..._bits_f630.rs`. Mutations: the rotary
   mass, the drilling mass, the option ignored per path, the load vector of a shell left out of the harmonic, the Shell3 mass
   dispatch, the damping matrix without shells. Commit two as patches.
4. Run the new files, every test file of rcfd-fem whose name holds shell, modal, harmonic, transient or damped, `cargo test -p rcfd-fem
   --release --lib`, all unedited, and the checks rule 7 lists.

Decision rule to register: done if every registered row holds on both elements and all the paths of the acceptance, each mutation is caught and the named existing tests pass unedited; partial otherwise, with the path named.

Open questions for the maintainer, with options and a recommendation, not decided by you: whether the paths take shells by default, with options and a recommendation; any other that arises.

Files you own: `crates/rcfd-fem/src` as the registration names it, the two new test files with their patches and records. No other crate, no golden, no existing test.

## Common to this work

The acceptance of F-630 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
decision rule is registered before the run and is not changed after it; a result that misses is recorded as a miss with
its number; a run that does not decide is recorded as not deciding. No test is ignored, skipped, removed or quarantined
to make a run pass; a registered miss is held as the repository holds known misses, with its limit intact.

Clean-room: reference values only from published sources that were read and that a page of the repository records with
their citation; nothing from memory (rule 4); vendor documentation is never a source. New reference values go in as a
fragment under `docs/reference-data.d/`, not as an edit of a generated page.

Use your own target folder under the scratch folder for every cargo command (`CARGO_TARGET_DIR`). Three other agents run
on this machine at the same time and own other files; a test that fails on a file refused by the operating system under
that load is reported, not retried until it passes. Register no limit on wall-clock time. This work changes the structural library; a defect found outside its paths is reported, not repaired. Delete nothing
outside your worktree and your scratch folder. Commit large run records only as far as they are the evidence; prefer a
summary table with the full record in the scratch folder.

The lead merges with `scripts/push_main.py`. Do not push and do not merge. Commit messages are plain: no agent marker,
no trailers. Rule 9: you do not edit `docs/roadmap.json`, `docs/milestones.json` or a generated page; you write your
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0798-F-630.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; what the gate runs and what is a heavy record; mutations or negative controls; deviations; what was not
run; open questions with options and a recommendation; the result of `python scripts/process_report.py --check`; your
verdict by the registered rule.
