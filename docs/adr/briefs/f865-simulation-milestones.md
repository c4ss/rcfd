# Brief: F-865, per cent complete for the whole roadmap and for each simulation milestone

Branch: f865-simulation-milestones
Items: F-865
ADR: 0756
Rules version: 4
Model: Claude Fable 5.2
Issued: 2026-11-21
Size (lead): done; 0; 0; 8; 64012

The rules are `docs/process/agent-rules.md`, version 4, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f865-simulation-milestones`, a new branch from main at the commit that adds this brief. Scratch folder: `E:/projects/cfd/w49i-work/f865-simulation-milestones-scratch`.

## Reading list

- docs/process/agent-rules.md 12458
- scripts/render_roadmap.py 10862
- scripts/roadmap_progress.py 11360
- docs/roadmap-overview.md 8503
- docs/milestone-fan.md 20829

Also in the repository, as needed: `docs/roadmap.json` (1.9 MB: read the id, title, status, deps, acceptance and gap of
every item with a script; read a note only where the acceptance does not settle whether an item belongs), the folder
`cases/`, `docs/validation-data.md`, `scripts/test_render_roadmap.py`.

## What the maintainer asked for (2026-11-21)

"Can we make the roadmap split into total % complete, then also % complete for simulation milestones 1. vortex shedding
2. the fan case 3. aerofoil lift and the other major cases that are specifically modelled for added new features acting
retrospectively and into the future for our future roadmap items."

Today the overview gives one figure for all items (done counts 1, partly done one half) and a table by line of work. No
figure says how far the project is towards a simulation a user can name.

## The lead's decisions, for you to register and build on

- **A milestone** is a named simulation with a stated result: for example "vortex shedding behind a cylinder: the
  Strouhal number and the force coefficients against the published reference". It has an id, a title, the case or cases
  it runs, the published reference it is judged against, and one or more **headline items**: the roadmap items whose
  acceptance is that comparison.
- **Membership.** An item belongs to a milestone if the milestone's simulation cannot be set up, run or trusted without
  it: the physics and numerics it uses, the mesh and boundary conditions it needs, its post-processing, its validation
  data and its headline items. An item that only uses the milestone's case as a convenient test of something else does
  not belong. An item may belong to several milestones or to none (most process, Studio and tooling items belong to
  none). Each membership has a one-line reason.
- **Where it is written.** A new optional key of an item in `docs/roadmap.json`, `milestones`, a list of milestone ids;
  the definitions of the milestones in one small file (`docs/milestones.json`). A new item states its milestones when
  the lead enters it. Nothing else holds the mapping.
- **The figures.** For the whole roadmap and for each milestone: the number of items done, partly done and not started,
  and the per cent with a partly done item counted as one half, as the overview does today. Beside each milestone's per
  cent, the state of its headline items in words (met, partly met with the gap, not met, not started). A milestone whose
  items are 90 per cent done and whose headline comparison misses must read as not met; the per cent alone never says
  "met".
- The three milestones named by the maintainer are fixed: vortex shedding, the fan case, aerofoil lift. The others are
  proposed by you and approved by the maintainer.

## Task

One registration, `docs/adr/0756-f865-registration-<slug>.md`, the first commit of this work, alone.

1. **The mechanism.** `render_roadmap.py` accepts the key `milestones` and shows an item's milestones on the generated
   item list. A script (a new `scripts/roadmap_milestones.py`) validates `docs/milestones.json` and the keys against each
   other (an unknown milestone id, a headline item that is not a member, a milestone without a headline item, a milestone
   without a member each fail) and writes the figures as a generated block of `docs/roadmap-overview.md` between markers
   of its own, and one figure file if you register one. It has `--check`, and a unit test in `scripts/` fails when the
   block is not current, so that CI catches it. With no `milestones` key anywhere and no definitions file, every
   generated file is what it is today, byte for byte.
2. **The three milestones.** Write their definitions and the membership of every item of the roadmap for them, with the
   reasons. You do not edit `docs/roadmap.json` (rule 9): deliver the mapping as one file in your ADR's folder or beside
   the ADR (`<item id>, <milestone id>, <reason>`), and a command of your script that applies it to a roadmap file, which
   the lead runs on main. State for each milestone the count of members and the figures that result on the roadmap of
   your base commit.
3. **The other major cases.** Propose, under "Open questions for the maintainer" with a recommendation, the list of
   further milestones: from the cases in `cases/`, the validation registry (F-006, F-125) and the larger lines of the
   roadmap (for example the backward-facing step, the flat plate, the heated cavity, the shock tube and the wedge, the
   packed bed, conjugate heat transfer, a rotor with cyclic symmetry for the structural solver, an overset case). For
   each: title, reference, headline items, and a rough count of members by a keyword search, marked as rough. Do not map
   them in full before they are approved.
4. **A check of the mapping.** Register before the mapping how it is checked: for a sample of items drawn by a registered
   seed (at least 40, half of them members and half not), a second reading against the membership rule, with the number
   of disagreements reported. A disagreement is a result, not a failure, and each is listed.
5. Unit tests for each validation failure and for the figures, failing before the code; mutations as rule 16 asks, at
   least one committed patch for the validation and one for the figures.

Do not change: `scripts/process_report.py`, `scripts/push_main.py`, `docs/process/lead-process.md`,
`scripts/roadmap_shards.py` (other agents work on them); the existing generated block of the overview; the status of any
item; `docs/milestone-fan.md`, except that you may link it from the fan milestone's definition.

Decision rule to register: done only if the mechanism has its tests and leaves today's output unchanged without the new
key, the three milestones are defined with every item mapped and reasoned, the sample check is made and reported, and the
further milestones are proposed; partial if a milestone has no headline item that the roadmap holds (say which item
would have to be entered) or the sample check was not made; stopped and reported if the membership rule cannot be
applied to a class of items (say which, with examples).

The lead merges with `scripts/push_main.py`. Do not push and do not merge. Commit messages are plain: no agent marker,
no trailers. Run in the background anything that takes longer than a few minutes. A timing measurement of another job is
running on this machine: run no cargo build and no cargo test other than `cargo test -p rcfd-cli --release --test docs_site`.

## Report (under 400 words)

The registration's commit, any amendment and the code commits; the three milestones with their headline items, member
counts and figures; the whole-roadmap figure; the sample check's disagreements; the proposed further milestones; the
command the lead runs to apply the mapping; mutations; deviations; what was not run; open questions; the result of
`python scripts/process_report.py --check`; your verdict by the registered rule.
