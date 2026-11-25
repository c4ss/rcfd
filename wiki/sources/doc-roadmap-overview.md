---
type: source
tags: [roadmap, status]
sources: []
origin: docs/roadmap-overview.md
origin_commit: 045708b9
created: 2026-11-22
updated: 2026-11-25
status: stable
---

# Roadmap overview

**Origin:** [docs/roadmap-overview.md](../../docs/roadmap-overview.md) at `origin/main` 709e302e (first read at 2e94e5ef; re-read at 09647baa and 930153a4; only the generated blocks changed)
**Kind:** status page (partly generated from `docs/roadmap.json`)

## Summary

Plain-language status: what rcfd is, counts by line of work, simulation milestones, what works, what does not, current priorities and what comes next. Hand-written parts last reviewed 2026-11-18.

## Key points

- `docs/roadmap.json` is the source of truth for every status; `docs/ROADMAP.md` and the counts are generated from it. See [[roadmap]].
- ~~786 items: 389 done, 297 partly done, 100 not started~~ (2e94e5ef); ~~791 items: 391 done, 297 partly done, 103 not started at 5b96a1f8~~ ~~792 items: 391 done, 297 partly done, 104 not started at 8ee32b1c (F-886 entered) (~68% counting partial as half, unchanged)~~; 794 items: 403 done, 299 partly done, 92 not started at 09647baa, "about 70%" (the lead's main, without F-886; the merged tree d0fb2b2e reads 795, 403, 299, 93 and 69%, as main does from 8732f2ae, with Quality, docs and validation at 16 not started; ~~at 930153a4, F-888 done: 795, 404, 299, 92 and 70%, the Studio desktop app line 41 / 12 / 6~~). At 709e302e: 798 items, 409 done, 300 partly done, 89 not started, "about 70%". Measures landed items, not solver accuracy.
- Quality, docs and validation line: ~~18 / 20 / 22~~ (2e94e5ef), ~~20 done / 20 partial / 25 not started at 5b96a1f8; the other lines of work are unchanged~~; 29 / 22 / 15 at 09647baa. Also changed at 09647baa: Claude and the MCP server 10 / 11 / 1 (was 9 / 11 / 2), Studio desktop app 40 / 12 / 7 (was 40 / 12 / 6), Structural solver 156 / 127 / 26. At 709e302e: Studio desktop app 44 / 13 / 5, Quality, docs and validation 30 / 22 / 15, Compressible flow and acoustics 10 / 8 / 8 (was 9 / 8 / 9); the other lines are unchanged ([[doc-roadmap-json]]).
- Largest line of work by far: Structural solver (~~154 done / 127 partial / 27 not started = 308 items~~; 156 / 127 / 26 = 309 items at 09647baa).
- ~~Three~~ Ten [[simulation-milestones]] at 09647baa (see the section below); the first three: ~~vortex shedding behind a cylinder (89%, headline not met), assistant-driven fan design (70%, not met), NACA 0012 lift (89%, not met)~~ (2e94e5ef). At 5b96a1f8: vortex shedding 20 items, 16 done / 3 partial / 1 not started, 88%, headline **met** (F-029 met, F-869 met); assistant-driven fan design 92 items, 49 / 31 / 12, 70%, not met (unchanged); NACA 0012 lift 23 items, 18 / 4 / 1, 87%, not met (F-868 met, F-043 still partly met). The per-cent figures fell because the new items F-881 and F-884 joined these milestones ([[doc-roadmap-json]]).
- Status 'done' is decided by the rule each item registered before its work; the maintainer gives the go-ahead for a line of work and does not accept results one by one. See [[registration]].
- Doesn't work yet: [[fan-01]] stalls (diagnosis open); structural solver partly built (ADR 0270/0290/0310, plan approved 2026-11-12); sliding-mesh transient solves 'not available'; Studio being rebuilt, no installer.
- Priorities: (1) one workspace layer for Claude Code and the Studio (ADR 0173, F-270..F-275, set 2026-11-04) — see [[rcfd-workspace]]; computer fan; Studio; faster strict tests.
- Next: workspace layer; close FAN-01 diagnosis or change reference case; Studio shell/plugin then 3D; structural solver; noise/vibration from a converged fan; validation against a measured small fan.
- How to help: measure a real small fan (curve, sound, geometry), pick a not-started item, review a result.

## Changes from 2e94e5ef to 5b96a1f8

Only the two generated blocks (`process-report:overview` and the milestones block) changed; the hand-written parts are still "last reviewed on 2026-11-18". The F-869 headline line now reads "met" (was partly met, with the observed order 1.387 outside the registered [1.5, 2.5]); the F-868 headline line now reads "met" (was partly met, missing 17 degrees). The fan headlines F-870, F-872, F-874 are unchanged.

## Changes from 5b96a1f8 to 09647baa

Again only the generated blocks changed. The milestone table, with the whole-roadmap row as at 709e302e (795, 404, 299, 92 at 930153a4; the ten milestone rows are unchanged since 09647baa, none of the seven items changed at 709e302e is a member):

| Milestone | Items | Done | Partly done | Not started | Per cent | Headline result |
|---|---|---|---|---|---|---|
| Whole roadmap | 798 | 409 | 300 | 89 | 70% | not a milestone |
| Vortex shedding behind a cylinder | 20 | 17 | 3 | 0 | 92% | met |
| Assistant-driven fan design | 92 | 49 | 31 | 12 | 70% | not met |
| Aerofoil lift (NACA 0012) | 24 | 20 | 4 | 0 | 92% | not met |
| Lid-driven cavity | 12 | 11 | 1 | 0 | 96% | met |
| Turbulent backward-facing step | 16 | 12 | 4 | 0 | 88% | not met |
| Turbulent flat plate skin friction | 18 | 13 | 5 | 0 | 86% | not met |
| Differentially heated cavity | 13 | 12 | 1 | 0 | 96% | met |
| Shock tube and wedge | 8 | 7 | 1 | 0 | 94% | met |
| Packed bed pressure drop | 14 | 14 | 0 | 0 | 100% | met |
| Structure and vibration of a fan rotor | 39 | 21 | 14 | 4 | 72% | not met |

- The six milestones from the lid-driven cavity to the packed bed come from ADR 0775 (F-871), the fan-rotor structure from ADR 0779 (F-875) ([[adr-thread-simulation-milestones-validation]], [[doc-milestones-json]]).
- Vortex shedding rose from 88% to 92% (F-881 done), aerofoil lift from 87% to 92% with one more member (F-884 and F-887 done); the aerofoil headline is still "not met" because F-043 is partly met.
- Headline lines of the new milestones: the backward-facing step is "not met" through F-042 (Standard 5.350 and Realizable 6.413 miss the window 6.16 to 6.36 on BFS-C) and F-043 (wall-resolved SST 6.554, BSL 5.956); the flat plate is "not met" through F-041 (Spalart-Allmaras $C_f$ +5.29% against the power law at x = 1.0, limit 5%) and F-043 (-7.36% against White at x = 0.97); the fan-rotor structure is "not met" with F-387 met and F-406 and F-436 not started.

## Contradictions with the wiki

- Sliding mesh: 'not available' here vs available from a case file in [[doc-readme]] (F-303, F-711). Flagged on [[overview]].

## Changes at 975beadb (F-339)

F-339 todo to partial: 798 items, 409 done, 301 partial, 88 not started, about 70 %; Design studies and optimisation 20 / 17 / 7; the milestone Assistant-driven fan design 92 items, 49 / 32 / 11, 71 %, not met.

## Changes at 2514444c (F-893)

F-893 entered (2026-11-24, the maintainer's answer) and done: 799 items, 410 done, 301 partial, 88 not started, about 70 % ([[adr-thread-studio-electron]]).

## Changes at d2cc5641 (F-894, F-339)

F-894 entered and done, F-339 partial to done: 800 items, 412 done, 300 partial, 88 not started, about 70 % ([[adr-thread-mcp-tools]]).

## Changes at 21e04d2e (F-895)

F-895 entered and done: 801 items, 413 done, 300 partial, 88 not started, about 70 % ([[adr-thread-mcp-tools]]).

## Changes at 82985b47 (the lead's readings)

F-270, F-272, F-275, F-287, F-288 and F-465 partial to done: 801 items, 419 done, 294 partial, 88 not started ([[adr-thread-workspace-layer]], [[adr-thread-structural-fsi-coupling]]).

## Changes at 3b8f12c4 (F-886)

F-886 done: 801 items, 420 done, 294 partial, 87 not started ([[adr-thread-vof-multiphase-particles]]).

## Changes at 045708b9 (F-889)

F-889 done: 801 items, 421 done, 294 partial, 86 not started ([[adr-thread-structural-case-file-and-fan-blade]]).

## Pages this source informs

- [[overview]]
- [[roadmap]]
- [[simulation-milestones]]
- [[fan-01]]
- [[roadmap-status]]
