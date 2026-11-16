---
version: 1
slug: "web-src-renderer-claude-claudepanel-tsx"
primary_target: "apps/studio-web/src/renderer/claude/ClaudePanel.tsx"
related_targets: ["apps/studio-web/src/renderer/claude","apps/studio-web/src/main/claude","apps/studio-web/src/shared/claude.ts","apps/studio-web/src/renderer/project/ProjectHome.tsx"]
---

# Claude Code panel (Electron Studio, F-814)

Scope: the right column of an open project's workspace in `apps/studio-web/`: the project's Claude Code sessions, the
transcript of the selected one (mirrored live, read-only when another process has it open), and a message field that
resumes a closed session or starts a new one with `claude -p`. Built inside the established DESIGN.md world (Aicher
system); one new token role (`chat-own` = sky-wash); the head uses the meadow colour token directly, since a
`claude-head` role would change the meadow users that `tests/unit/tokens.spec.ts` locks (proposed to the maintainer); no
new hue jobs. Inferred from DESIGN.md
Layout and Components and the prototype brief (no interview was possible: the item was built by a subagent); the
maintainer may correct it.

Mode: Operate.

Audience and job: a learner or engineer who works on the project with Claude Code, in a terminal or in the Studio. They
want to see what a session is doing without leaving the Studio, to go on with a closed session where it stopped, and to
follow a run id or a file Claude names straight to the run or the file. Frequency: whenever Claude Code works on the
project; left open beside the other views.

Proof and content: the session list and the transcript are Claude Code's own files and its `claude agents --json`
listing; the reply text is Claude's, bold only where it bolds; reference chips are made only for run ids and files the
project holds (as read through the engine's project server). The panel never invents a state: a session whose state
cannot be established is "state unknown", read-only.

Constraints: `docs/studio-guardrails.md` G10 (the Studio does not chat with Claude itself: it drives the user's own
Claude Code CLI), G11 (one writer per session, mirror read-only, never an approve-everything flag, the conservative
allow-list, the folder's Claude Code control files confirmed first), G8 (keyboard: Tab through the list, Enter selects,
Enter sends, Shift+Enter new line, Escape stops; words beside every state; no contrast requirement), G12 (the Studio
writes nothing into the project folder), G13.

States and ranges: claude not configured (sessions still listed, read-only); no session yet; sessions open elsewhere,
closed, unknown, or answered by this Studio; listing failed; transcript unreadable or unrecognised lines; control files
to confirm, or changed since confirmed; an allow-list file refused; a turn answering, answered, stopped, refused, failed,
or aborted because the CLI did not apply dontAsk; denied tool calls with the rule that would allow them; 0 to many
sessions (the list scrolls at 168px); the last 50 transcript entries.

Unresolved: pictograms for the session states (words only for now); writing the allow-list file from a denied call
needs an engine entry point; a file chip opens the project home filtered to the file until the case editor (F-812).

## Direction contract

THESIS: A quiet, ruled column beside the work: who is talking in which session, what they said, and where it points in
the project. It refuses the chat-bubble app with avatars and the "approve all" toolbar.

OWN-WORLD: DESIGN.md unchanged otherwise: a 32px meadow head with an 8px square ink dot and the bold title, ink rules
between modules, silver hairlines between rows, your messages in sky-wash on the right, Claude's between hairlines with
no box, chips (the chip Button variant) beneath a reply, orange only for focus. Radius none.

FIRST VIEWPORT: at 1440x900 the panel is 360px on the right of the project views, full height under the header band:
head, Sessions (list with "New chat"), the transcript filling the column, Tool approvals (a disclosure), the last
turn's line, the message field with Send or Stop.

FINISH: screenshots at 1440x900 in `test-results-visual/f814/`; finish review by the maintainer.
