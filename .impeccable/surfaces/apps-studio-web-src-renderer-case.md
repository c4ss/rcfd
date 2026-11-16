---
version: 1
slug: "apps-studio-web-src-renderer-case"
primary_target: "apps/studio-web/src/renderer/case/CaseEditorPage.tsx"
related_targets: ["apps/studio-web/src/renderer/case", "apps/studio-web/src/renderer/settings", "apps/studio-web/src/main/case"]
---

# Case editor (Electron Studio, F-812)

Scope: opening a case file of the open project, editing it with the F-825 settings surface, checking it and saving it
back, all through the engine; plus the case-level actions new case (template or shipped example), duplicate, check the
saved file and close the session. Routes: `#/case/open/<cases/...>` (opening), `#/case/<session>` (editing),
`#/case/new` (a new case), `#/case` (the sessions open on case files). Inside the established DESIGN.md world (the
Wayfinding Program): no new token, hue job, face or shape.

Mode: Operate.

Assumptions (no interview was possible: the surface was shaped by an agent from the roadmap item; the maintainer
confirms or corrects at the view-and-report):

- A1. The visitor arrives from the project home with one case in mind: a learner changing one or two values of a
  quick-start case, or an engineer adjusting a case before a run. Both expect the file to be the truth and the Studio to
  change only what they changed.
- A2. Other writers (Claude Code, the CLI, a text editor) work on the same folder at the same time. A file changed under
  the editor is normal, not a failure, and is never resolved for the visitor.
- A3. The F-825 surface (learner-first disclosure, search, every key) is the editing body unchanged; F-812 adds a case
  file panel at the head of its right column and a header band that names the file.

## Job and audience

Open a case file, change settings with the engine's controls and verdicts, and put the change back into the same file,
in place, with the comments and layout the file had, knowing exactly which keys were written and that nobody else's edit
was overwritten.

## Outcome and proof

- Primary action: Save (Ctrl+S), which writes only the settings changed since the last save, through the engine's
  `edit_case`; the panel then says which keys were written and in which form (`minimal`: lines edited in place).
- Secondary: check the case (settings.validate, F-825), check the mesh (mesh.check), check the saved file (the
  check-only launch, with the exit code the CLI gives), duplicate, new case, close the session (asks first when changes
  are unsaved and names them).
- Proof: the panel shows the file's path, the hash the editor read, when it was opened and saved, and the Claude Code
  equivalent of each action (G16, G17). The round trip is proven by a test that reads the saved file with `rcfd`.

## Selected direction

Visual authority: DESIGN.md (header band, project tab, Buttons, Inputs, the Hairline Density Rule, state as pictogram
plus word). The editor is the F-825 page with two changes:

1. The header band names the case file as a white project tab (the file is the thing being edited), with "Project
   home" and "New case" as ghost buttons and the file's state word on the right.
2. The right column starts with a "Case file" panel ruled like the check panel's sections: the path in title type, the
   state (pictogram plus word), Save as the one primary button, then the other case actions as default buttons. Notices
   (changed outside, conflict, refused) open inside this panel, ruled, never as a modal or a toast; focus moves to the
   notice heading when one appears.

Conflict (G18) is a ruled notice with a three-column hairline table (setting, yours, in the file now) and three named
choices: write my changes into the file as it is now; load the file as it is now (drop mine, named with their count);
keep editing. None is preselected. Red is not used: a conflict is not a failure.

## Scope and boundaries

- In: TOML and JSON case files under `cases/` of the open project; every key of the schema (through F-825); saves that
  set keys, including keys in new entries; the engine's refusals shown as they come.
- Out: undo (F-829), the command palette (F-831), help text beyond the schema (F-826), removing an entry from a file
  (the engine cannot yet: shown as a refusal with the way out), editing files outside `cases/`.
- Anti-goals: no TOML or JSON parsing in the Studio, no copy of the schema or the validation, no "save anyway", no merge
  the visitor did not choose, no silent reload over unsaved changes.

## States and ranges

- Opening: "Opening cases/x.toml"; failed open: the engine's diagnostics (key, problem) with the fix (edit the file in
  a text editor or ask Claude Code, then open again); a sandboxed service that cannot reach the project.
- Open: no changes; N changes not saved (the keys listed, up to 12 then "and N more"); saving; saved (keys written,
  form, mesh check when a mesh key was written); refused by the engine (its message, each diagnostic leading to its
  key); cannot be saved in place (a removed entry); changed outside with nothing unsaved (load the file as it is now);
  conflict (above); file gone; project not open; run in progress (the surface is read-only, F-825).
- New case: the engine's templates (1 today) and shipped examples (about 70 today); a taken name or a bad name is the
  engine's message.

## Interaction and layout

- 1440 by 900: as F-825 (index 264px, keys flexible, right column 360px); the case panel scrolls with the right column.
- Keyboard: Tab order header band, then the F-825 order with the case panel first in the right column. Ctrl+S saves from
  anywhere on the page. Every choice is a named button; Escape in the duplicate and new-case name fields clears them.
- Live: the editor reads the file's state again when the engine's change feed names its path, and when the session's
  settings change (another writer of the session included).

## Constraints and open decisions

- G2: the case text is never read or written by the Studio; values, messages and lists of templates and examples are
  the engine's.
- G8: keyboard and visible focus everywhere; state by word and shape as well as colour.
- G16: the session actions go through the F-827 dispatcher; the case-file actions go through the case port, each with
  its Claude Code equivalent named (they combine project-server tools and service methods, so the registry, one service
  method per action, cannot hold them yet).
- Open for the maintainer: the right place for the case panel (top of the right column, as here, or a band above the
  three columns); whether Save should also live in the header band.
