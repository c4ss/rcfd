# Brief: F-339, a reader of the work and of its additive golden edit, before the merge

Branch: f339-reader-golden-edit
Items: F-339
ADR: 0784
Rules version: 5
Model: Claude Fable 5.2 (a reader: no code, no commit)
Issued: 2026-11-24
Size (lead): done; 0; 0; 0; 125876

The rules are `docs/process/agent-rules.md`, version 5, in the repository; read them in full. This is a reader's brief
(`docs/process/lead-process.md`: "A reader, an agent that reads an item's work afresh, is issued for an item that changes
a default result or a golden"). You read and report; you write no code, no ADR and no commit, and you change no file in
any worktree. Scratch folder for anything you run: `E:/projects/cfd/w49i-work/f339-reader-golden-edit-scratch`.

## Reading list

- docs/process/agent-rules.md 12991
- docs/process/trials/reader-trial-1/registration.md 10684 (the three kinds of defect)
- docs/adr/0784-f339-registration-mcp-tools-for-tolerance-and-sobol-studies-of-the-reduced-order-fan.md 36035 (on the branch f339-mcp-uq-sobol-tools, read with git show)
- crates/rcfd-mcp/tests/golden/tools_list_default.json 27188
- crates/rcfd-mcp/tests/golden/tools_list_project_f270.json 38978

## What you read

- The branch `f339-mcp-uq-sobol-tools` (tip 2ed42d55) against its base fcc84a84: `git diff fcc84a84 2ed42d55` and
  `git log fcc84a84..2ed42d55`, read from any checkout (`git show`), not by checking it out in a shared worktree.
- Its record: `docs/adr/0784-f339-registration-mcp-tools-for-tolerance-and-sobol-studies-of-the-reduced-order-fan.md` on that
  branch, the registration, amendment 1 and addendum 1, and the item F-339 in `docs/roadmap.json` (read with a script).
- The additive golden edit it reports for the lead: `E:/projects/cfd/w49i-work/f339-mcp-uq-sobol-tools-scratch/g3_additive_edit.patch`
  (read only), and the two goldens it changes, `crates/rcfd-mcp/tests/golden/tools_list_default.json` and
  `tools_list_project_f270.json`.
- As needed: `crates/rcfd-api/src/design/` (the drivers `uq::run` and `sensitivity::run`), `crates/rcfd-mcp/src/`, and
  `apps/studio-web/capabilities/` (the inventory and coverage rows the addendum gives to the lead).

## What you look for

The three kinds of defect that pass every machine check (reader trial 1, `docs/process/trials/reader-trial-1/`):

1. **A stated result that does not follow** from what the ADR records (a number beside its limit that does not meet it,
   a "met" or "yes" the record does not support, a status the decision rule does not give).
2. **A test that cannot fail**: an assertion that holds for any computed value, a comparison of a value with itself or
   with a copy of the same computation, a check the committed mutation does not reach.
3. **A limit so loose that it tests nothing**.

And, for the golden: whether the patch only adds (every earlier entry byte-identical and in order, the two new entries
equal to what the server prints, nothing changed or removed), and whether the sixteen count literals it raises are all
and only the ones that hold the list's length. Check this with a script of your own on the files, not by reading the
patch alone. Running the item's tests in your scratch folder (a copy, `CARGO_TARGET_DIR` there) is allowed.

## What you hand back

A report: for each finding, the file and line or the ADR's section, the kind (1, 2, 3, golden, other), what is wrong and
why, and how sure you are; then "no finding" for each kind you looked for and did not find. Whether the patch is
additive, with your script's output. No status: the lead decides it. Keep the report under 1500 words.
