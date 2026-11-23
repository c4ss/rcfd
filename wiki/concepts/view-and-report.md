---
type: concept
tags: [studio, process, testing]
sources: [doc-ui-restart-plan, doc-studio-guardrails, doc-process-documentation-review, doc-impeccable-surfaces]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# View-and-report step

A human check of a Studio surface: the maintainer runs or looks at the result (often screenshots at 1440 by 900) and reports, because "Agents cannot see the window" ([[doc-ui-restart-plan]], §5.1).

## How rcfd uses it

- "Whether the app looks and feels right is for the user to judge, and every wave ends with a short view-and-report checklist" ([[doc-ui-restart-plan]]); waves 48 and later started only after the user had run the previous result and reported on it ([[doc-ui-restart-plan]]).
- Guardrail G13: each roadmap item has automated tests (unit, Playwright driving Electron) or a view-and-report step for the maintainer named in its acceptance ([[doc-studio-guardrails]]).
- Surface briefs built by agents without an interview say the maintainer "confirms or corrects at the view-and-report"; screenshots go to `apps/studio-web/test-results-visual/<item>/` ([[doc-impeccable-surfaces]]).
- As of 2026-11-18, 9 Studio items carried the step: 7 done (3 notes do not mention it, 4 count it met by screenshots in a git-ignored folder), 2 partial for the missing step; "whether its view-and-report step remains is for the unified rules to say" ([[doc-process-documentation-review]], G11, I3).

## Related

- [[studio]]
- [[studio-guardrails]]
- [[maintainer]]
