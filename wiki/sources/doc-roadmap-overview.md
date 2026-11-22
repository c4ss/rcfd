---
type: source
tags: [roadmap, status]
sources: []
origin: docs/roadmap-overview.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: stable
---

# Roadmap overview

**Origin:** [docs/roadmap-overview.md](../../docs/roadmap-overview.md) at `origin/main` 2e94e5ef
**Kind:** status page (partly generated from `docs/roadmap.json`)

## Summary

Plain-language status: what rcfd is, counts by line of work, simulation milestones, what works, what does not, current priorities and what comes next. Hand-written parts last reviewed 2026-11-18.

## Key points

- `docs/roadmap.json` is the source of truth for every status; `docs/ROADMAP.md` and the counts are generated from it. See [[roadmap]].
- 786 items: 389 done, 297 partly done, 100 not started (~68% counting partial as half). Measures landed items, not solver accuracy.
- Largest line of work by far: Structural solver (154 done / 127 partial / 27 not started = 308 items).
- Three [[simulation-milestones]]: vortex shedding behind a cylinder (89%, headline not met), assistant-driven fan design (70%, not met), NACA 0012 lift (89%, not met).
- Status 'done' is decided by the rule each item registered before its work; the maintainer gives the go-ahead for a line of work and does not accept results one by one. See [[registration]].
- Doesn't work yet: [[fan-01]] stalls (diagnosis open); structural solver partly built (ADR 0270/0290/0310, plan approved 2026-11-12); sliding-mesh transient solves 'not available'; Studio being rebuilt, no installer.
- Priorities: (1) one workspace layer for Claude Code and the Studio (ADR 0173, F-270..F-275, set 2026-11-04) — see [[rcfd-workspace]]; computer fan; Studio; faster strict tests.
- Next: workspace layer; close FAN-01 diagnosis or change reference case; Studio shell/plugin then 3D; structural solver; noise/vibration from a converged fan; validation against a measured small fan.
- How to help: measure a real small fan (curve, sound, geometry), pick a not-started item, review a result.

## Contradictions with the wiki

- Sliding mesh: 'not available' here vs available from a case file in [[doc-readme]] (F-303, F-711). Flagged on [[overview]].

## Pages this source informs

- [[overview]]
- [[roadmap]]
- [[simulation-milestones]]
- [[fan-01]]
- [[roadmap-status]]
