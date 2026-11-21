# Brief: F-867, the fan milestone is the whole fan design goal

Branch: f867-fan-milestone-design-goal
Items: F-867
ADR: 0757
Rules version: 4
Model: Claude Fable 5.2
Issued: 2026-11-21
Size (lead): done; 0; 0; 4; 102202

The rules are `docs/process/agent-rules.md`, version 4, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f867-fan-milestone-design-goal`, a new branch from main at the commit that adds this brief. Scratch folder: `E:/projects/cfd/w49i-work/f867-fan-milestone-design-goal-scratch`.

## Reading list

- docs/process/agent-rules.md 12458
- docs/milestone-fan.md 20829
- docs/fan-project-breakdown.md 12694
- docs/milestones.json 2800
- docs/adr/0756-f865-mapping.tsv 6159
- scripts/roadmap_milestones.py 15550
- docs/adr/0756-f865-registration-per-cent-complete-for-the-roadmap-and-for-each-simulation-milestone.md 31712

Also in the repository, as needed: `docs/roadmap.json` (1.9 MB: read it with a script, never whole), `docs/acoustics.md`,
`docs/design-loop.md`, `docs/structural-solver.md`, `scripts/test_roadmap_milestones.py`.

## What the maintainer decided (2026-11-21)

ADR 0756 defined the fan milestone as the validation of FAN-01 at its design point: 22 members, 89 per cent, headline
F-126 not met. Its members are mostly foundation items done long ago, and 19 items that name a fan in their title belong
to no milestone. The lead asked the maintainer whether the fan milestone is the validation alone or the whole design goal
of `docs/milestone-fan.md`. The answer: "Whole design goal".

The goal, from that page: Claude, through the assistant tool layer, iterates on fan shape and size, sets up and runs the
test cases, reads the results, and converges on designs that keep noise low while flow is high across a range of
operating conditions.

## Task

One registration, `docs/adr/0757-f867-registration-<slug>.md`, the first commit of this work, alone. You change no
script unless a defect of `roadmap_milestones.py` stops you (then say so and register the change); you do not edit
`docs/roadmap.json`, `docs/milestones.json` or any generated page (rule 9). You deliver files beside the ADR and the
command the lead runs, as ADR 0756 did.

1. **The definition.** Redefine the milestone `fan` as the design goal. Its result is the goal above in one sentence.
   Its headline items are the items whose acceptance is a result the goal needs and that can be met or missed: at the
   least the validation of the fan solve (today F-126; ADR 0756 question 3 proposes an item of its own for the FAN-01
   design point, which the maintainer has not answered: say what you would use if it existed), the noise prediction, the
   operating map, the optimiser and the design loop driven by the assistant. Take them from the capability chain of
   `docs/milestone-fan.md` and from the fan project breakdown, and name for each link of the chain the roadmap item or
   items that carry it, or say that no item carries it.
2. **Membership.** Register the rule before mapping: the classes M1 to M4 of ADR 0756 stay, read against the design goal
   instead of the validation run; add what the goal needs beyond a solve (geometry, meshing of the passage, the noise
   model, the operating map, the reduced-order model and the fidelity ladder, the optimiser, the design ledger and loop,
   the MCP and Studio tools the loop is driven through, the structural checks of the blade and rotor where the breakdown
   names them as part of the goal). Say which of these you leave out and why. Every membership has a one-line reason;
   every item that names a fan in its title is either a member or listed with the reason it is not.
3. **The mapping.** Read every item of the roadmap against the rule. Deliver the fan memberships as a file of the form of
   `docs/adr/0756-f865-mapping.tsv` and the new definition as a file, with the one command that replaces the fan
   milestone's definition and memberships on main and leaves the other two milestones byte for byte as they are. Show on
   a scratch copy that it does, and give the figures that result: members, done, partly done, not started, per cent, and
   the state of each headline item.
4. **The check.** A sample drawn by a registered seed before the second reading: at least 40 pairs (item, fan), half
   members and half not, read a second time by a reader that has not seen the mapping; the disagreements listed, each
   decided with its reason.
5. **What the figure will and will not say.** One paragraph in the addendum, for the maintainer: how many of the members
   are foundation items done before the fan work began, how many are open, and whether the per cent now moves with the
   open fan work.

Decision rule to register: done only if the definition names an item or "no item" for every link of the capability
chain, every item naming a fan in its title is placed, the sample check is made and reported, and the command leaves the
other milestones unchanged; partial if a link of the chain could not be read from the pages or the sample check was not
made; stopped and reported if the goal cannot be given headline items that can be met or missed.

Open questions for the maintainer, with options and a recommendation, not decided by you: items that would have to be
entered for links of the chain that no item carries.

The lead merges with `scripts/push_main.py`. Do not push and do not merge. Commit messages are plain: no agent marker,
no trailers. Run no cargo command other than `cargo test -p rcfd-cli --release --test docs_site`.

## Report (under 400 words)

The registration's commit and the others; the headline items and the chain with its items; members and figures before and
after; the fan-titled items left out, with reasons; the sample check's disagreements; the command the lead runs; the
paragraph of task 5 in two sentences; deviations; what was not run; open questions; the result of
`python scripts/process_report.py --check`; your verdict by the registered rule.
