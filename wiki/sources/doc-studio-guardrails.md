---
type: source
tags: [studio, guardrails, process]
sources: []
origin: docs/studio-guardrails.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: stable
---

# Electron Studio guardrails

**Origin:** [docs/studio-guardrails.md](../../docs/studio-guardrails.md) at `origin/main` 2e94e5ef
**Kind:** rules page (approved by the maintainer)

## Summary

The fixed limits of the Electron Studio (F-448 and its successors). Status: "APPROVED by the maintainer 2026-11-16 (G1 to G14 as drafted; G15 to G21 added the same day at the maintainer's request; G22 added the same day)". Written by the lead from the house rules, ADR 0350 and `docs/ui-restart-plan.md`. Every Studio roadmap item names the guardrails it touches, and its acceptance shows that it keeps them. Breaking a guardrail needs the maintainer's word and an ADR, like a change to a house rule. Product truth lives in `PRODUCT.md`, the visual system in `DESIGN.md`; this file only states what must not happen. The rules are listed one by one on [[studio-guardrails]].

## Key points

- 22 guardrails: G1 clean room; G2 Rust is the engine; G3 a picture can be wrong and look right; G4 engine interfaces change only by addition; G5 dependencies; G6 honest about limits; G7 runs outlive the window (amended 2026-11-17: runner closes with the Studio); G8 accessibility (no WCAG contrast requirement); G9 performance limits P2, P4, P5, P7, P8, P10 from ADR 0350; G10 Electron security; G11 Claude Code; G12 the project folder is the contract; G13 testable without eyes; G14 rcfd-studio is retired; G15 everything the engine can do, the Studio can do; G16 no UI-only capability; G17 reproducible, with provenance one click away; G18 non-destructive by default; G19 explain in place; G20 every action has a command; G21 never block; G22 implicit by default, explicit on exception.
- G7 amendment (2026-11-17): the Studio owns only a runner whose `.rcfd/runner.json` record says `started_by: "studio"` with its own `studio_pid` (ADR 0801 Q-RL-2); on close, an owned idle runner is shut down quietly; with active runs the close asks (Stop runs and close / Keep them running and close / Cancel, default Cancel); setting "Keep the runner going after the Studio closes" (off by default) and `--keep-runner` keep the old behaviour. Details in ADR 0801 addendum "Runner closes with the Studio".
- G10 run sites sentence added 2026-11-17 by the maintainer's answer to ADR 0739 Q3, option (a).
- G15 inventory comes from the engine itself (case schema, `service.info`, MCP tool list, CLI help); a coverage test fails when a capability has no UI path and no recorded, dated exemption ([[doc-studio-coverage]]).
- G22 sets a per-surface budget of visible chrome words at rest, counted from the DOM by a test; raising a budget needs the maintainer's word.
- Open for the maintainer: the UI framework and component approach; platforms beyond Windows 11 (Linux answered 2026-11-17: Docker first, then CI weekly, ADR 0801 Addendum 19, recorded as "WebGL 2 on software rendering"; macOS not asked); installers (answered 2026-11-17, built in CI; an installed Studio uses the engine it ships with, `<resources>/bin`, never PATH; signing waits for the maintainer's certificate).

## Contradictions with the wiki

- G6 uses stress and strain colouring as its example of a capability that does not exist ("needs a structural solver"). Read narrowly (the Studio has no such colouring) this is not contradicted; read as "no structural solver exists" it conflicts with [[rcfd-fem]]. See [[studio]].
- G8 says there is no WCAG contrast requirement (removed 2026-11-16); the prototype surface brief in [[doc-impeccable-surfaces]] still cites "G8 WCAG 2.2 AA". Within the repo docs.

## Pages this source informs

- [[studio-guardrails]]
- [[studio]]
- [[rcfd-runner]]
- [[clean-room]]
- [[view-and-report]]
- [[studio-design-system]]
- [[work-streams]]
