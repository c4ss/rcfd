# ADR 0795: F-828 registration: the Studio's numbers that still have no provenance

Status: **REGISTERED 2026-11-25, written before any code of F-828's remaining numbers and before any run of its checks.**
Before this text no test or library code was written and no file of the repository was changed. What was done: reading
(below); a build of the unchanged engine (`rcfd`, `rcfd-mcp`, `rcfd-runner`, release) and of the unchanged Studio; one probe
of the unchanged Studio from the scratch folder (the lists and the Tab counts of "Findings"), which is no registered check;
and, after the text was written, the link check `docs_site` the brief asks for. Results go in addenda at the end; nothing
above them is changed afterwards.

It answers the brief `docs/adr/briefs/f828-provenance-remaining.md` ([the file](briefs/f828-provenance-remaining.md)) and
follows [docs/process/agent-rules.md](../process/agent-rules.md), version 5. Branch `f828-provenance-remaining` from main
25311db5, scratch `w49i-work/f828-provenance-remaining-scratch`. Model: Claude Opus 5.5.

## What is asked

The acceptance of F-828 in `docs/roadmap.json`: "Any number on screen opens its provenance (run id, snapshot, file, engine
version, unit); every run shows the CLI command that reproduces it; a test that a run started from the Studio and the same
run started by that command give bit-identical results on a quick-start case (G17); units come from the engine, never
converted in the Studio." The gap: the Runs residual columns, the compared run's residuals, the legend ticks, the domain
size, the scale bar, the runner's budgets and the home counts. The lead's details are the brief's section "What the
maintainer decided": the existing trigger, registry and popover; a passive span named by its row, so no Tab stop per number;
`not recorded` where the record holds nothing; a counted or rounded number says what and from which answer; a unit from the
engine or said not recorded. A number not in that list is reported, not built.

## What was read

`AGENTS.md`, the rules and the brief in full; `docs/studio-guardrails.md` (G2, G3, G6, G8, G9, G17, G19, G20, G22 and the
word budget); [ADR 0734](0734-f828-proposal-run-records-launch-argv-build-id-time-unit-rerun-and-mesh-hash.md);
[ADR 0801](0801-studio-web-dependencies-and-layout.md), addendum 8 and the sections of F-828 and D-L1 found by search;
[tolerances.md](../process/tolerances.md). Of `apps/studio-web`: `src/shared/{provenance,project,run}.ts`,
`src/renderer/{provenance,runs,workspace,field,project,settings,ui}`, `src/renderer/first/FirstPath.tsx` (the picture),
`src/main/{provenance,project,run}`, the specs `provenance`, `field`, `compare`, `f837`, `trash`, `first-path`, `a11y`,
`word-budget`, `idle-frames` and `project` (unit and e2e, in part) and `tests/support`. Of the engine, only to name a
number's source: `rcfd-workspace` (`queries.rs`, `meshes.rs`, `runner_config.rs`, `derive.rs`), `rcfd-project/project.rs`
(`validate`, the counts) and the tool list of `rcfd-mcp/project.rs`. No published value is used.

## Findings of the reading and of the probe

1. **What the existing specs fix.** A row of the Runs table has one `[data-provenance-trigger]`, opened first by Ctrl+I on
   the row (`provenance.spec.ts`); the counts of `<Provenance` per source file (unit `provenance.spec.ts`); the texts of
   `.fv-legend-ticks > span`; the two operations of the provenance port. So the new numbers use a second form of the
   trigger, a passive span that carries `data-provenance-name` and `data-provenance-member` and no
   `data-provenance-trigger`, made by one component of `src/renderer/provenance`.
2. **The keyboard.** A trigger of its own takes the focus; the others are opened by Ctrl+I while an element that names them
   (`data-provenance`) or contains them has it. Ctrl+I opens one number; a legend names five. So Ctrl+I, pressed again while
   the popover it opened is open, must step to the next number the same element names.
3. **The field view's readout key is one for every view.** With two views (Compare), Ctrl+I in B's view opens A's readout,
   by the order of registration: a number with the provenance of another run. The key becomes the view's own.
4. **Tab stops** of the unchanged Studio (1440 by 900, the quick-start project, walked by Tab from the page's start until
   the first element comes again): Home 32, Runs 32, Runs with the budgets open 41, Workspace 40, Workspace comparing 39.
5. **Numbers with no provenance on Home, Runs, Workspace and Compare** beyond the list below are the ones of "Open" at the
   end of this text: they are not in the brief's list and are not built.

## The numbers (registered)

"Named by" is the element whose focus Ctrl+I reads; every number opens by the pointer too: **click** on the number, or,
where a click selects a row or a segment, **hover and Ctrl+I**. No entry is a Tab stop. Every popover shows Value, Unit,
Run, Snapshot, File, Read by and Engine; a project number shows Run and Snapshot as `none` with the reason, and the project
folder. "More" is what it shows besides.

| Id | Number | Source (method, file, field) | Unit's source | Named by; pointer | More |
|---|---|---|---|---|---|
| R | Runs table: the latest residual of u and of p of a run | `run_status` `last_residual.residuals.<eq>`, the last line of `runs/<id>/residuals.jsonl` | `res_<eq>` in `runs/<id>/output/monitors.json`, else `none` with the reason | the cell (arrow keys), and the row after its own number; hover | Rounded: to 3 significant figures |
| C | Workspace, comparing: each residual of the compared run at the line | `read_residuals` of the compared run B, `runs/<B>/residuals.jsonl` | B's `output/monitors.json` | B's band name; click | Step of the sample |
| L1 | Legend ticks, range of the shown snapshot | `runs/<id>/fields/store.rts`, the shown field and snapshot | the field's unit in `store.rts` | the field view after its readout; click | Tick k of 5; Range: least and greatest of the shown values; Rounded: 5 significant figures |
| L2 | Legend ticks, range of the whole run | as L1, all snapshots | as L1 | as L1 | Range: over n snapshots |
| L3 | Legend ticks, fixed range | typed in the window | as L1 | as L1 | Range: typed, not recorded by the engine |
| L4 | Legend ticks, one range of two runs | `store.rts` of A and of B | as L1 | either field view; click | Range: the union of both views' ranges; Compared run and its snapshot |
| L5 | Legend ticks of the difference B - A | as L4 | as L1 | the difference view; click | Range: of the differences the Studio subtracted (f64, shown as f32) |
| D | The domain's size (2 or 3 extents, one text) | the points of the stored mesh in `store.rts`: max - min per axis | `length_unit` of `data.open` or of the derived picture, else the engine's documented unit (ADR 0738), said `not recorded in store.rts` | the field view; click | Counted: the extents in full; Rounded: 4 significant figures |
| S | The scale bar's length (a face-on 2D view) | chosen by the Studio from the camera (1, 2 or 5 times a power of ten, at most a quarter of the view) over `store.rts` | as D | the field view; click | Chosen: the rule; the file holds no such number: `not recorded` |
| B1 | Runner budgets: a key's value | `runner_config` `values.<key>`, `.rcfd/runner-config.json` | `not recorded` (the key's name is not read as a unit) | the key's field; click on its name | Hash of the file; a typed value not written says so; an empty key is no number and says `not recorded`: the runner's default |
| B2 | Runner budgets: "in use N" | `runner_config` in use, `.rcfd/runner.json` `config.<key>` | as B1 | the key's field, after B1; click | the runner's start and `config_source` |
| H1 | Home: the All segment | the sum of H2's four totals | none: a count | its segment; hover | Counted: the four summands |
| H2 | Home: the segments Cases, Meshes, Runs, Notes and the heads of these groups | Cases, Notes: the Studio counts the files below `cases/`, `notes/`; Meshes: `list_meshes` `total`; Runs: `list_runs` (its runs, all pages) | none: a count | the segment of the kind; segment: hover, head: click | Counted: what; Engine's count: `project_info` `counts.<kind>` |
| H3 | Home: the heads of designs/, Issues and Trash | designs: entries of `designs/` counted by the Studio; `project_info` `issues_total`; `list_trash` `total` | none: a count | the All segment; click | as H2 |
| H4 | Home: "N shown" with a filter | counted by the Studio over the listed entries | none | the segment of the kind; click | Counted: entries whose name holds the filter's text |
| H5 | Home: "the engine counts N" | `project_info` `counts.<kind>` | none | the segment of the kind (designs: All); click | Counted by the engine |
| H6 | Home: "The first N of M are listed", "and N more" (issues), "and N older items" (trash) | listed entries; totals as H2, H3; a difference says the Studio subtracted | none | the segment of the kind, or All; click | Counted |
| H7 | Home: runs per case | the Studio counts the runs of `list_runs` whose `case` is the file | none | the case's row; click | Counted |
| H8 | Home: the files of a mesh of several files | `list_meshes` `files` | none | the Meshes segment; click | Counted by the engine |

The learner's picture (first path) is the same field view: its ticks and scale bar are L1 and S there.

## Design (registered)

- `src/shared/provenance.ts`: a project number (`ProjectNumberSource`) and its rows (`projectFacts`); nothing else changed.
- `src/renderer/provenance`: the passive component (`Traced`), a third popover for a project number (no command: it says
  none is recorded), and in the registry: an element names one or more keys (separated by spaces); the numbers of a focused
  element are, in order, its own, the old-form triggers inside it (unchanged), those it names, the new-form ones inside it;
  Ctrl+I opens the first, and again, while one of them is open, the next, coming round to the first. Ctrl+I over an open
  popover that has no next number does nothing (today it replaced the popover by the selected run's). The focus goes back
  as now.
- The surfaces named in the table take the component; the field view's keys are the view's own (finding 3).
- No port operation, request, dependency, colour, word at rest or Tab stop is added. Everything shown is what the surface
  already holds; a run's record is read when a popover opens, as now.

## Set-up of the checks

Windows 11, the release engine of 25311db5's sources in the scratch target folder, the Studio built and packaged from the
final code commit, one Playwright worker, headless as the suite sets it, `apps/studio-web` the working folder. The engine's
answer outside the Studio: a second `rcfd-mcp --read-only` on an untouched copy of the same project or on the project
itself, started by the test (`list_runs`, `run_status`, `read_residuals`, `runner_config`, `project_info`, `list_meshes`,
`list_trash`), the engine's `derive_picture` (`slice` through the middle: `range` and `bounds`), the records' bytes and, for
values of the store, the independent decoder `tests/support/rts-decode.ts`.

## Checks

No limit on time is set. A row is met if every assertion of its test holds. New files:
`apps/studio-web/tests/unit/provenance-f828.spec.ts` (U) and `tests/e2e/provenance-f828.spec.ts` (the others). Limits in
words are equality of text or of f64 unless said.

| Id | Clause | Check | Limit | Source of the limit | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|---|
| KR | R | u and p of two runs by pointer, by the cell and by the row (Ctrl+I, Ctrl+I): run, file, unit, exact value | the engine's `run_status` value bit for bit (f64) and its unit text | acceptance; tolerances, bit-identity row | equal | none | code verification |
| KC | C | each compared residual at a common stop: B's run, file, value, unit; pointer and band name | `read_residuals` of B at that step, bit for bit | same | equal | none | code verification |
| KL | L1 to L5 | the five ticks in each range mode, pointer and keyboard; the fact Value of the ends | ends equal to the engine's `range` of the slice (L1), of the decoded snapshots (L2, L4, L5), of the typed numbers (L3) as f32; unit the store's | same | equal | none | code verification |
| KD | D | the extents and unit, pointer and keyboard | the engine's `bounds` of the slice, f32 equality of each extent; unit `length_unit` of the reply | same | equal | none | code verification |
| KS | S | length and unit, pointer and keyboard; `not recorded` said | the text shown; the rule's fact; unit as KD | acceptance | equal | none | code verification |
| KB | B1, B2 | a written key and an in-use key of a project with a runner: value, hash, file, unit `not recorded`; typed value and empty key say so | `runner_config`'s answer and the file | acceptance | equal | none | code verification |
| KH | H1 to H8 | every home number on a project made to show them all (a filter; an OpenFOAM mesh folder; over 5000 notes; over 50 issues; over 50 trash items; a design) | the second engine's answers and the folder's own listing | acceptance | equal | none | code verification |
| KF | L1, S (learner) | a tick and the scale bar in the first path's picture, after the window's own first run | as KL, KS by the decoder | acceptance | equal | none | code verification |
| KE | all | the fact Engine of each kind: the run record's `solver_version`, or the project server's name and version | equal | acceptance | equal | none | code verification |
| SC | scan | every visible text with a digit and every number field with a value on Home, Runs (budgets open, a run selected), Workspace, Compare (side, difference) is in a trigger of either form, or matches an exempt selector of the table below, each with its reason; each group R, C, L, D, S, B1, H1, H2, H7 found at least once by the scan (so none is left out) | 0 others | brief, task 1 | 0 | none | code verification |
| KT | Tab | Tab stops of the five views, walked as in finding 4 | 32, 32, 41, 40, 39 | finding 4; brief | equal | none | unchanged default output |
| KK | keyboard | Escape gives the focus back to the element that had it, after one and after three Ctrl+I; Ctrl+I over an open popover of a lone number leaves it | as written | G8 | as written | none | code verification |
| KQ | at rest | every new trigger has the text of its place: no underline, border, fill or padding; the word-budget spec passes | none differs | G22 | none | none | unchanged default output |
| U | facts | `projectFacts`, the new sources and the registry's order, without a window; the static counts of the old trigger unchanged; no `run`/`copy` port operation added; no unit factor in the new code | as written | acceptance, G2 | as written | none | code verification |
| X | existing | unedited: `provenance`, `a11y`, `word-budget`, `runs`, `compare`, `field`, `field-derived`, `f837`, `f891`, `project` (home), `trash`, `first-path`, `idle-frames` e2e specs; `npm run test:unit`; `npm run typecheck` | all pass; no request or redraw at rest (`idle-frames`) | brief, task 4 | pass | none | unchanged default output |
| P | process | `docs_site`, `roadmap_proposals.py check`, `process_report.py --check` | pass; only a generated file that is not current may fail | rule 7 | pass | none | unchanged default output |

### Exempt of the scan, with the reason

| What | Reason |
|---|---|
| names, ids, paths, file names, hashes, versions in words | not numbers: names |
| clock times, dates, the header's and lane's times | a time of day, not a quantity |
| the change lane, event and log text, notices, the engine's messages, "why" lines | quoted text of the engine or of an exception |
| labels and hints of a control ("a whole number from 1"), commands shown | words of a rule, not a value |
| text only for readers (`sr-only`) | not on screen |
| the numbers of "Open" below (each by its selector) | outside the brief's list: reported |
| a number field the user edits whose key has a trigger (budgets) | its name opens it (B1) |

### Mutations (rule 16)

Each alone on the final code commit, the Studio built and packaged, the new e2e file run once, the file restored. A mutation
the compiler refuses is replaced by an amendment.

| Id | Group | Change | Must fail |
|---|---|---|---|
| M1 | R | the residual's source names another run (the list's first) | KR |
| M2 | C | the compared residual's source names the selected run | KC |
| M3 | L | the legend's unit is that of another field of the store | KL |
| M4 | D | the field view does not name the domain (the keyboard path removed) | KD |
| M5 | S | the scale bar is drawn without its trigger | SC |
| M6 | B | the in-use number's file and reader are those of the written value | KB |
| M7 | H | a home segment names nothing (the keyboard path removed) | KH |
| M8 | H | the runs of a case carry the count of all runs | KH |
| M9 | all | a second Ctrl+I does not step | KR |
| M10 | H | a group head's count is drawn without its trigger | SC |

The committed one is M1, `apps/studio-web/tests/e2e/provenance-f828.spec.mutation.patch`, whose first line is
`# expect-fail: cd apps/studio-web && npm run build && npm run package:dir && npx playwright test tests/e2e/provenance-f828.spec.ts --project=electron`.

Size (agent): done; 0; 0; 1; 300000

## Decision rule

**done** if every number of the registered list opens its provenance by pointer and keyboard with the registered facts, the
scan finds none without, each mutation is caught and the named existing specs pass unedited; **partial** otherwise (the
brief's rule). A number whose source cannot be named is recorded as missing and makes the verdict partial: none is known
now. Where a mutation names a row, it is caught if that row's test fails.

## What is not covered

- The service route of a field view (a store opened through `data.open`): its length unit is in row U, not in a window.
- Other platforms; a screen reader's reading of a popover.
- The numbers of "Open".

## Open (numbers outside the brief's list, for the maintainer's decision; none is built)

The compare metrics table (`compare_runs`); the band ends and the scrubber's flag; the tick labels of the plots; Home's runs
table Step and Time and the sizes in kB (meshes, trash); the counts of the Runs filter and of "n of m shown"; the numbers of
the Runs rows' words beside the result; the field view's controls (cut, iso-level, rake, the two ends of a fixed range) and
its note of cells outside the range; counts inside a sentence (log, events, confirmations).

## Addendum 1: results (2026-11-25)

Commits: registration 98af4079; the code and the two specs c98b5f0e; the committed mutation cfa2f9da. The registered run was
made once, at cfa2f9da, whose only change after the final code commit is the patch file: typecheck, build, package,
`npm run test:unit`, then the fourteen e2e files in one command with one worker (115 passed, 5.6 min). Amendments: none.

### Checks

| Id | Limit | Measured | |
|---|---|---|---|
| KR | `run_status` bit for bit, the unit's text | equal for u and p of two runs, by pointer, cell and row | met |
| KC | `read_residuals` of B bit for bit | equal, B's run, file and unit, pointer and band name | met |
| KL | ends equal as f32; unit the store's | equal in the five range modes (snapshot, run, fixed, shared, difference), pointer and keyboard | met |
| KD | `bounds` as f32; `length_unit` | equal, pointer and keyboard | met |
| KS | the text, the rule, the unit as KD; `not recorded` | as written | met |
| KB | `runner_config` and the file | equal; unit `not recorded`; typed and empty keys say so | met |
| KH | the second engine's answers and the folder | equal for H1 to H8 | met |
| KF | as KL and KS by the decoder | equal | met |
| KE | the record's `solver_version`, or the server's name and version | equal where each kind is read | met |
| SC | 0 numbers without a trigger or an exemption | 0 in each of the seven states scanned | met |
| KT | 32, 32, 41, 40, 39 | 32, 32, 41, 40, 39 | met |
| KK | as written | as written | met |
| KQ | none differs; the word budget | 0 differ; `word-budget` 10 of 10 | met |
| U | as written | 7 of 7 | met |
| X, typecheck | passes | passes | met |
| X, e2e | the thirteen existing files pass unedited | 108 of 108: provenance 10, a11y 12, word-budget 10, runs 3, compare 7, field 17, field-derived 10, f837 13, f891 4, project 11, trash 3, first-path 6, idle-frames 2 | met |
| X, `npm run test:unit` | all pass | 562 passed, **1 failed**, 22 did not run (a serial file) | **missed** |
| P | pass; only a generated file that is not current may fail | made with this addendum; the output is in the agent's report | in the report |

### The miss

`tests/unit/data-routes.spec.ts:94` ("P12, tiny raw"): expected the length units `[null, "m"]`, received `[null, null]`. The
test takes `f447_bench` from `RCFD_F447_BENCH`, from `$CARGO_TARGET_DIR/release/examples`, or from a `target` folder of a
directory above. My target folder held `rcfd`, `rcfd-mcp` and `rcfd-runner` and not the example, so the test walked up and
took the main checkout's, built 2026-11-16 22:03 UTC, whose bytes hold no text `length_unit`; the engine names it since
8bba78f2 (2026-11-17). So the registered set-up (the engine of these sources) did not hold for that file. No file under
`src/main`, which the test exercises, and no crate is changed by this work. The cause is a diagnosis and not a proof: no run
with a bench of these sources was made, since the session's goal allows no test to be run again to pass and no run to be made
twice; rule 2's amendment for a defective set-up was for that reason not used. The test is untouched. What decides it:
`cargo build --release -p rcfd-api --example f447_bench`, then `npm run test:unit` once.

### Mutations (each on cfa2f9da, built, packaged, the new e2e file run once, the file restored)

| Id | Caught by | First failing assertion |
|---|---|---|
| M1 | KR | Run: the first run instead of the row's |
| M2 | KC | Run: the selected run instead of B |
| M3 | KL (three tests: one view, compare, learner) | Unit: `m/s` instead of the store's `unknown` |
| M4 | KD (two tests) | Ctrl+I does not reach the domain |
| M5 | SC (and the tests of KS and KF) | the scan lists the scale bar |
| M6 | KB | File: `.rcfd/runner-config.json` instead of `.rcfd/runner.json` |
| M7 | KH | Ctrl+I on Cases does not open its head's count |
| M8 | KH (and SC) | the runs of a case |
| M9 | KR (and five other tests) | the second Ctrl+I stays on the first number |
| M10 | SC (and KH) | the scan lists the head's count |

M7 was made on the segment Cases. Each change compiled, so none was replaced.

### Deviations

1. Development runs of the new e2e file before the registered run: five (dev1: 1 failed, 6 not run; dev2, on an app not
   rebuilt: 6 failed; dev3: 5 failed; dev4: 1 failed; dev5, KB alone: passed), and one of the new unit file (7 passed). The
   failures were faults of the test (arguments, locators) and one of the code (the scale bar took no pointer event). The
   scan's exempt selectors gained the trash's dates and its kB text there, both of registered categories.
2. H2, Runs: the total is `list_runs` `total`, not a count of all pages. H4: the fact says "whose text holds", since the
   filter is compared with more than the name. Scan group C is asserted by Compare having more triggers than Workspace, and
   by KC, not by a name of its own.
3. A build with `CARGO_TARGET_DIR` written in Git Bash's form made
   `E:\e\projects\cfd\w49i-work\f828-provenance-remaining-scratch\target`, outside the scratch folder. It is left for the lead
   to remove, since the brief forbids deleting there.

Not run: the other e2e files, the visual project, the engine's tests, Linux. The new specs are 7 unit tests (under a second)
and 7 e2e tests (about 1.3 min). No heavy record.

Size (agent), actual: partial; 0; 0; 1; 400000

### Decision

**partial** by the registered rule, for one part of one row: `npm run test:unit` did not pass whole. Every number of the list
opens its provenance by pointer and keyboard with the registered facts, the scan finds none without, the ten mutations are
caught and the existing e2e specs pass unedited.

## Open questions for the maintainer

1. Scope (left open): should the numbers of "Open" above carry provenance too, so that "any number on screen" holds without
   exception? (a) No more: the list of the brief is the scope. (b) Those that are values of a run: the compare metrics table,
   the band ends, the scrubber's flag, Home's Step and Time. (c) All of "Open", the tick labels of the plots and the sizes in
   kB included. Recommended: (b), by a registration of its own: the trigger and the keyboard path exist, and the others are
   labels of an axis, sizes of files or counts in a sentence.

## For the lead: roadmap entries

Status proposed: partial. No follow-up is proposed. To weigh: `findBench` of `tests/support/f447.ts` takes a bench of any age
from a `target` folder above, so in a worktree the tests of `data-routes.spec.ts` judge another checkout's build.

Gap proposed, then the note:

> Row X of ADR 0795 missed: npm run test:unit 562 passed, 1 failed, 22 did not run. data-routes.spec.ts:94 took an f447_bench of 2026-11-16 from the main checkout, which lacks length_unit (in the engine since 2026-11-17). Wanted: one npm run test:unit with a current bench. The other numbers on screen are an open question of scope (ADR 0795).

> ADR 0795, 2026-11-25, partial: the numbers of F-828's gap have their provenance: the Runs residual columns, the compared run's residuals, the legend's ticks in five range modes, the domain's size, the scale bar, the runner's budgets and the home counts open it by pointer and keyboard with value, unit, run, snapshot, file, reader and engine, equal to run_status, read_residuals, derive_picture, runner_config and the counting tools asked outside the Studio; a passive trigger and a Ctrl+I that steps, Tab stops unchanged (32, 32, 41, 40, 39); a scan of Home, Runs, Workspace and Compare finds no other number without it beyond the registered exemptions; 10 of 10 mutations caught, one committed; 115 e2e tests pass, the 13 existing files unedited; npm run test:unit missed by one test, which found a bench of another checkout.

The same entry as `docs/roadmap/proposals/0795-F-828.json`.
