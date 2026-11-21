# ADR 0755: F-864 registration: a roadmap proposal as a file, validated, applied and replayed on six merged ADRs

Status: **REGISTERED 2026-11-21, written before any code of F-864 and before any run of its checks.**
Before this text nothing was written by this work. What was done: reading (listed below), three read-only commands on
the repository (a count of the keys of `docs/roadmap.json`, a comparison of its bytes with `roadmap_shards.encode`, the
subjects of the last 25 commits that changed it), and one run of the existing `python scripts/test_roadmap_shards.py`
(9 tests, all pass). Results go in addenda at the end; nothing above them is changed after a run.

It answers the brief `docs/adr/briefs/f864-item-owned-roadmap-shadow.md` and follows `docs/process/agent-rules.md`,
version 4. Branch `f864-item-owned-roadmap-shadow` from main 47961cd5 (`47961cd58bfe64d4368cf19a12907557f20038dd`),
scratch `w49i-work/f864-item-owned-roadmap-shadow-scratch`. Python 3.9.13, git 2.45.2. No cargo build and no cargo test
is run by this work (a timing measurement of another job runs on the machine); see "What is not covered".

## What is asked

The acceptance of F-864 (`docs/roadmap.json`), by clause:

- (0) "docs/roadmap.json stays the one source and no page generated from it changes."
- (1) "an agent's proposed roadmap entry is a file of a registered form on its branch (status, the note entry, the gap,
  follow-up items), which a script validates and applies to roadmap.json with the result the lead would have typed, shown
  on the entries of at least six merged ADRs of November 2026 replayed from their addenda".
- (2) "two such proposals for different items made from one base apply in either order with the same result, and two for
  the same item, a duplicate id, a missing dependency, a cycle, a stale base and a malformed file each give a named
  failure that leaves roadmap.json as it was, each shown by a unit test".
- (3) "the differences between what the script applied and what the lead wrote for the replayed merges are listed, each
  explained."
- (4) "The registration proposes, as an open question with options, the change of rule 9 of the agent rules that a
  cutover would need, and the cutover itself; neither is made here."

The brief adds: the script never writes `docs/roadmap.json` by default and writes nothing when a proposal fails; the six
ADRs are chosen by a registered rule before any result is seen; the count of differences is a result, not a limit; each
unit test fails before the code exists; one committed mutation for the validation and one for the application;
`scripts/push_main.py`, `scripts/process_report.py`, `docs/process/lead-process.md` and the behaviour of
`roadmap_shards.py split` and `join` with its tests are not changed.

## What was read

- The brief; `docs/process/agent-rules.md` (version 4), whole; `scripts/roadmap_shards.py` and
  `scripts/test_roadmap_shards.py`, whole; `docs/process/framework-reconciliation.md`, whole;
  `docs/process/framework-2026-11-17/loop-migration-plan.md`, whole (a proposal, not a rule);
  `scripts/render_roadmap.py`, whole; the entries F-863 and F-864 of `docs/roadmap.json`.
- `scripts/process_report.py` lines 436 to 520 (the lint of a registration); the list of lines of `scripts/push_main.py`
  that name a mutation patch, and `scripts/test_push_main.items.mutation.patch` for the form of a patch;
  `.github/workflows/ci.yml` lines 105 to 115 (CI runs every `scripts/test_*.py`).
- For the form of a registration: ADR 0753 lines 1 to 187 (its registration, not its addendum) and the table of checks
  of ADR 0752 (lines 170 to 200).
- For the form of a proposal: the section "For the lead: roadmap entries" of ADRs 0741, 0742, 0744 and 0746. None of
  them is among the six of the replay.

Not read before this text: the addendum of any of the six ADRs of the replay (of ADR 0752 and 0753 the list of headings
was seen, with their line numbers), and no diff of `docs/roadmap.json`. Of what the lead wrote, the subjects of 25
commits were seen and the two entries F-863 and F-864.

Facts used:

- `docs/roadmap.json` at 47961cd5 is an array of 770 items; its bytes are exactly
  `json.dumps(items, ensure_ascii=False, indent=2) + "\n"`, which is `roadmap_shards.encode`. 472 items have the keys id,
  title, phase, deps, status, acceptance, note; 293 have `gap` as well; 5 have no `note`. New items are at the end of
  the array (the last six are F-859 to F-864).
- `render_roadmap.schema_problems`: the keys of an item; a `gap` only on a partial item, one line of 1 to 500
  characters; phases 0 to 9.
- `roadmap_shards.validate`: ids, statuses, title and acceptance, deps that exist, no duplicate, no cycle.
- Rule 9: the status; the note entry as one line (ADR number, date, outcome, one sentence); for a partial item its
  `gap`, which replaces the previous one; follow-up items with title, deps and acceptance.
- In the four addenda read, a note entry starts "ADR <number>, <date>, <outcome>: "; a follow-up is one paragraph
  "Title: ... Deps: ... Acceptance: ..."; ADR 0742 labels its paragraphs ("Status of F-262:", "Note entry for F-262:",
  "Gap of F-262:"); none states a phase for a follow-up.

## Design

Python 3.9 standard library only. One new script, `scripts/roadmap_proposals.py`, that imports `roadmap_shards` for
`validate`, `encode` and `ITEM_ID`; `roadmap_shards.py` and its test file are not edited. Tests in
`scripts/test_roadmap_proposals.py`.

### D1. The form of a proposal (form 1)

One file per item that an addendum proposes entries for: a JSON object in UTF-8.

| Key | Required | Value |
|---|---|---|
| `form` | yes | the integer 1 |
| `adr` | yes | the four digits of the ADR, as a string |
| `item` | yes | the id of an item that exists in the roadmap (`F-NNN`) |
| `base` | yes | the 40 hexadecimal characters of the commit of main the proposal was written against |
| `status` | yes | `done`, `partial` or `todo`: the status proposed, also when it is the status the item has |
| `note` | yes | the new note entry: one line that starts `ADR <adr>, <YYYY-MM-DD>, ` (rule 9) |
| `gap` | no | only with status `partial`: one line of 1 to 500 characters; it replaces the previous gap |
| `follow_ups` | no | a list of objects, each a proposed new item without an id |

A follow-up has `title` and `acceptance` (one line each, not empty) and `deps` (a list); it may have `phase` (0 to 9;
without it the phase of `item` is taken, since rule 9 asks an agent for none) and `key` (a name of lower-case letters,
digits and hyphens, unique in the file). A dep is the id of an item of the roadmap or `new:<key>` for another follow-up
of the same file. An id, a note and a status are not given for a follow-up: the lead gives the id, the status is `todo`.
No other key is allowed anywhere. Not chosen: a Markdown block in the addendum read by a parser (it is the prose the lead
retypes today, and a parser of prose fails silently); one file for several items (two branches would then meet in one
file, which is what the item-owned form is to avoid).

**Where it lies.** `docs/roadmap/proposals/<adr>-<item>.json`, for example `docs/roadmap/proposals/0755-F-864.json`; the
name of the file must be its `adr` and `item`. It is beside the folder `docs/roadmap/items/` that the plan of 2026-11-17
names for the records, it is not a file any generator reads, and two branches never write the same file unless they
write for the same ADR and item. No such file is committed under `docs/` by this work, because rule 9 as it stands does
not allow it (open question 1). The proposal files of the replay lie in `scripts/roadmap_replay_f864/`, with the same
names.

### D2. Validation

Every failure is a `ProposalError` with a name, printed by the command as `<name>: <message>` with exit status 1.

| Name | When |
|---|---|
| `malformed` | the file is not JSON or not an object; a key is missing, unknown or of the wrong type; `form` is not 1; `adr`, `item` or `base` do not have their shape; the name of the file is not `<adr>-<item>.json`; the status is not one of three; the note is empty, has a line break or does not start `ADR <adr>, <date>, `; a gap with a status other than partial, or not one line of 1 to 500 characters; a follow-up without title, acceptance or deps, with a wrong phase or key |
| `unknown-item` | `item` is not in the roadmap |
| `same-item` | two proposals of the set name one item |
| `stale-base` | `base` is not a commit of the repository, has no `docs/roadmap.json`, or is not an ancestor of (or equal to) the commit the roadmap is from |
| `stale-item` | the entry of `item` in the roadmap differs from its entry at `base` (the message names the fields: note, status, gap, ...) |
| `duplicate-id` | an id given to a follow-up exists already (the start number is too low), or two follow-ups of a file have one `key` |
| `missing-dependency` | a dep of a follow-up is neither an item of the roadmap nor a `new:<key>` of the same file |
| `cycle` | the follow-ups depend on each other in a circle (found by `roadmap_shards.validate` on the result) |
| `invalid-roadmap` | the roadmap given, or the result, fails `roadmap_shards.validate` for another reason |
| `refused-output` | the output path is the roadmap that was read, and `--in-place` was not given |

Derivation of the two stale checks. A proposal states a status and a gap for the item as its author saw it at `base`. If
the item's entry changed since (the lead recorded a gate result, a maintainer's answer, another merge), the status
proposed may no longer follow, and nobody has compared the two: that is the lost update of two writers of one record,
and the check is the comparison of the record with the version read (optimistic concurrency by a version check; derived
here, no source is cited for it). The ancestor check makes the comparison meaningful: a base that is not in the history
of the roadmap's commit was never a state of this roadmap.

### D3. Application

`apply(items, proposals, start, base_items)` returns a new list and changes nothing it was given:

1. every proposal is validated (D2), all of them before anything is built;
2. the proposals are put in the order of (`adr`, `item`), whatever order they were given in;
3. for each, on a copy of the item's entry: `status` is set; the note entry is appended to `note` after one space, or
   becomes the note where there is none; with status `partial` a `gap` given replaces the gap and no `gap` given leaves
   it; with another status the key `gap` is removed (the schema allows a gap on a partial item only);
4. the follow-ups, in the order of step 2 and then of the file, get the ids `F-<start>`, `F-<start+1>`, ... and are
   appended at the end of the array with the keys id, title, phase, deps, status `todo`, acceptance, and no note;
   `new:<key>` deps become ids;
5. the result is checked by `roadmap_shards.validate` and by the gap rule; only then is anything written.

Why two proposals for different items commute: step 3 reads and writes the entry of the proposal's own item only, so two
proposals for different items write disjoint entries; step 4 is the one shared effect (the ids and the end of the
array), and step 2 fixes its order independently of the order of arrival. So a set gives one result in any order of its
arguments. Applied one after the other in two calls, two proposals without follow-ups give the same bytes in both
orders (when the roadmap's commit is not moved between the calls; the stale checks then compare each item with the base
and the other item's change is not seen by them). With follow-ups the ids of two calls follow the two start numbers the
lead gives; that is the lead's choice and not the script's.

The command:

    python scripts/roadmap_proposals.py check FILE... [--roadmap PATH] [--at REV] [--repo DIR] [--start N]
    python scripts/roadmap_proposals.py apply FILE... [--roadmap PATH] [--at REV] [--repo DIR] [--start N] [--output PATH] [--in-place]

`--roadmap` is the roadmap read (default `docs/roadmap.json` of the repository), `--at` the commit it is from (default
`HEAD`), `--start` the number of the first new id (required by `apply` when a follow-up exists; `check` takes the highest
number plus one without it). `apply` writes the result with `roadmap_shards.encode` to standard output, or to `--output`
through a temporary file beside it that is renamed. An `--output` that is the roadmap read is refused without
`--in-place`. So the script writes `docs/roadmap.json` only when it is named as output and `--in-place` is given, and on
any failure it writes nothing. The roadmap at a base is read with `git show <base>:docs/roadmap.json`.

## The replay, registered

**The rule of choice.** The population is the ADRs numbered 0739 to 0753 (the range the brief names) whose file at
47961cd5 has a heading that ends "For the lead: roadmap entries". There are 13: 0740, 0741, 0742, 0743, 0744, 0746, 0747,
0748, 0749, 0750, 0751, 0752, 0753. The six of the replay are the six highest numbers: **0748, 0749, 0750, 0751, 0752,
0753**. Nothing replaces one that cannot be replayed; it is then reported as not replayed. The rule was fixed from the
list of files alone.

**Writing a proposal from the addendum alone.** For each item the section proposes entries for, one file. `status`, `note`
and `gap` are the quoted paragraphs, letter for letter, without the "> " and without a label before the entry ("Note
entry for F-262: "); a status stated in a sentence outside a quote is taken from the sentence. A follow-up paragraph is
cut at its labels "Title:", "Deps:" and "Acceptance:": the title without its closing full stop, the deps as the ids
named, the acceptance letter for letter. No phase and no key is written unless the addendum states one. Where a section
does not have this shape, what was done is said per case in the addendum. The roadmap, its history and the lead's
commits are not looked at while the six files are written; the files are committed before the comparison is run.

**The commits.** On the first-parent line of main up to 47961cd5, for an ADR with number N and for an item X of one of
its proposal files:

- A is the first commit at which the ADR's file has the heading "For the lead: roadmap entries";
- L, the lead's edit, is the first commit at or after A in which the entry of X in `docs/roadmap.json` differs from that
  of its first parent; the roadmap before is that of L's first parent, written L^;
- `base` of the proposal is the merge base of the second parent of A and L^ when A is a merge, and the first parent of A
  when it is not.

The roadmap of L^ is taken with `git show` into the scratch folder and the proposals are applied to that copy with
`--at L^`, and `--start` the lowest number of an id that L adds (the highest number plus one when L adds none). If the
application fails with `stale-item` or `stale-base`, that is recorded as a difference, and the application is made once
more with `base` set to L^ in a scratch copy of the proposal, so that the content can be compared. A driver,
`scripts/roadmap_replay_f864.py`, finds A, L and the base by these rules, applies, and prints the differences; it reads
the history and writes into the scratch folder only.

**The comparison.** The result R is compared with the roadmap of L:

- for X: `status`; `gap`; the note added (the text of L's note after the note of L^, where L's note starts with it; the
  whole note otherwise), each as an exact string;
- each follow-up with the item L adds at the same place in the order of ids: title, deps, acceptance, phase, and the note
  the lead gave it. A follow-up with no item added by L is looked for by its subject among the items entered after L up
  to 47961cd5; the match is by reading and is said;
- every other entry that L changes or adds and no proposal of the ADR names.

One difference is counted for each field of an item that is not equal, and one for each entry of the last kind. Each is
given one of these kinds: (a) the lead's own addition (a gate, a decision, an answer, an item not proposed, a note on a
new item); (b) a changed wording of what the agent proposed; (c) an entry the lead did not take; (d) an error of the
addendum; (e) an error of the lead; (f) a rule of the form or of the script that the lead does not follow (the
separator, the place, the phase, a stale base). A difference that fits none, or whose cause cannot be read from the ADR
and the history, is "unexplained".

## Table of checks

Rows C1 to C7, V, W and U are unit tests of `scripts/test_roadmap_proposals.py`. "Fails before" is shown once, by a run
of the committed test file before `scripts/roadmap_proposals.py` exists (the import fails and every test with it); the
log is kept in the scratch folder. The limits are pass or fail or a count, so the margin is n/a.

| Id | Check | Limit | Source of the limit | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|
| A0 | clause (0): `git diff --stat 47961cd5` on the last commit names files under `scripts/` and this ADR only; `scripts/roadmap_shards.py`, `scripts/test_roadmap_shards.py`, `scripts/push_main.py`, `scripts/process_report.py` and `docs/process/lead-process.md` are not in it | no other file | acceptance of F-864, clause (0); the brief ("Do not change") | holds | n/a | unchanged default output |
| A1 | clause (1): the six replays: each addendum is written as proposal files of form 1 without loss, the files validate, and the application to the roadmap of L^ gives a result (after the registered second application where the first is stale) | 6 of 6 | acceptance of F-864, clause (1); the decision rule of the brief | 6 of 6 | n/a | code verification |
| A3 | clause (3): the differences between R and the roadmap of L, counted by kind (a) to (f) | every difference listed; 0 unexplained; the count itself has no limit | acceptance of F-864, clause (3); the brief ("The count of differences is a result, not a limit") | 10 to 30 differences, most of kind (a) and (f); 0 unexplained | n/a | code verification |
| C1 | clause (2): two proposals for different items from one base, both with a follow-up: the set gives the same bytes in both orders of the arguments; without follow-ups, applied one after the other in two calls, the same bytes in both orders | fails before, passes after | acceptance of F-864, clause (2) | passes | n/a | code verification |
| C2 | clause (2): two proposals for the same item in one set fail as `same-item`; the second of them applied to the result of the first fails as `stale-item` | fails before, passes after | acceptance of F-864, clause (2) | passes | n/a | code verification |
| C3 | clause (2): a start number that gives a follow-up an id the roadmap has fails as `duplicate-id`; two follow-ups with one key fail as `duplicate-id` | fails before, passes after | acceptance of F-864, clause (2) | passes | n/a | code verification |
| C4 | clause (2): a follow-up whose dep is no item fails as `missing-dependency`; a `new:` dep without its key the same | fails before, passes after | acceptance of F-864, clause (2) | passes | n/a | code verification |
| C5 | clause (2): two follow-ups that depend on each other fail as `cycle` | fails before, passes after | acceptance of F-864, clause (2) | passes | n/a | code verification |
| C6 | clause (2): in a temporary git repository, a base on a side branch that is not an ancestor of the roadmap's commit fails as `stale-base`; a base that is an ancestor, with the item's note changed by a later commit, fails as `stale-item` and names the note; a base that is no commit fails as `stale-base` | fails before, passes after | acceptance of F-864, clause (2); the brief ("a base that is not an ancestor of the roadmap's commit, or an item whose note changed since the base") | passes | n/a | code verification |
| C7 | clause (2): each malformed file of the list of D2 (not JSON, not an object, a missing key, an unknown key, form 2, a short base, a wrong file name, a status of another word, a note of two lines, a note without its head, a gap on a done item, a gap of 501 characters, a follow-up without acceptance, a phase of 12) fails as `malformed` | fails before, passes after | acceptance of F-864, clause (2) | passes | n/a | code verification |
| W | clause (2), "leaves roadmap.json as it was": in every failing case of C2 to C7, run through the command with `--output`, the bytes of the roadmap file are those before the call, the output file does not exist, and the exit status is 1 with the name at the head of the message; an `--output` that is the roadmap fails as `refused-output` without `--in-place` and writes it with `--in-place`; without `--output` the result goes to standard output and no file changes | fails before, passes after | acceptance of F-864, clause (2); the brief ("when a proposal fails validation nothing is written. It never writes `docs/roadmap.json` by default") | passes | n/a | code verification |
| V | D3 by its steps: the note appended after one space and set where there is none; the status set; the gap replaced, left (partial without a gap) and removed (done); follow-ups at the end with ids from the start number, the phase of the item or their own, `new:` deps as ids, status todo and no note; the items given are not changed | fails before, passes after | the brief ("the note entry appended to the item's note, the status and gap set, follow-ups appended with ids from a start number the lead gives") | passes | n/a | code verification |
| U | on the live `docs/roadmap.json`, with the base taken as the roadmap itself: a proposal for its first item gives an array equal to the live one in every other entry and in every other key of that entry, and `encode` of the unchanged items is the file's bytes | fails before, passes after | acceptance of F-864, clause (1) ("the result the lead would have typed": nothing else moves) | passes | n/a | unchanged default output |
| E | the 9 existing tests of `scripts/test_roadmap_shards.py` pass, the file and `roadmap_shards.py` unedited; `python -m unittest discover -s scripts -p "test_*.py"` passes as CI runs it | all pass | the brief ("the existing behaviour of `roadmap_shards.py split` and `join` and its tests"); `.github/workflows/ci.yml` line 113 | all pass | n/a | unchanged default output |
| M | the mutations below | each caught by the named test | rule 16; the brief (one committed patch for the validation and one for the application) | each caught | n/a | code verification |
| P | `python scripts/process_report.py --check` on the last commit | 0 failures other than a generated file that is not current | rule 7 of `docs/process/agent-rules.md`, version 4 | `docs/adr/index.md` not current, nothing else | n/a | code verification |

Clause (4) is not a check: it is the two open questions below, repeated in the addendum under the fixed heading.

## Negative controls and mutations

Each is applied to the finished script with `git apply`, the test file is run, the failing tests are noted, and it is
taken back with `git apply -R`.

- **M1, validation (committed,** `scripts/test_roadmap_proposals.validate.mutation.patch`, first line
  `# expect-fail: python scripts/test_roadmap_proposals.py`**):** the comparison of the item's entry with its entry at
  the base is removed. Expected: the `stale-item` tests of C6 and C2 fail.
- **M2, application (committed,** `scripts/test_roadmap_proposals.apply.mutation.patch`, the same first line**):** the
  proposals are taken in the order given and not in the order of (`adr`, `item`). Expected: C1 fails (the ids of the
  follow-ups differ between the two orders).
- **M3, validation (scratch):** the check for two proposals of one item is removed. Expected: C2 fails.
- **M4, application (scratch):** the gap is not removed when the status is not partial. Expected: V fails.
- **M5, writing (scratch):** the check of the output path is removed. Expected: W fails (`refused-output`).

The negative controls of the design are the failing cases C2 to C7 themselves, and the run of the tests before the
script exists.

## Order of the work

1. This file, committed alone.
2. The test file, committed; its run without the script (all fail), log kept.
3. `scripts/roadmap_proposals.py`, committed; C1 to C7, W, V, U, E.
4. M1 to M5; the two patches committed.
5. The six addenda read; the proposal files written and committed, before the driver is run on them.
6. The driver, committed; A1, A3; the list of differences.
7. A0; the addendum; P.

## Open questions for the maintainer (clause 4; proposed here, decided by nobody in this work)

**Q1. The change of rule 9 that would let an agent commit a proposal file.** Options: (a) no change: the addendum keeps
its quoted paragraphs and the lead retypes them; (b) rule 9 allows one more path, `docs/roadmap/proposals/<adr>-<item>.json`
of form 1, written by the agent beside the quoted paragraphs for a time (both, compared by the lead with the script's
`check`), `docs/roadmap.json` still written by the lead alone, with the script; (c) as (b) and the quoted paragraphs are
dropped at once. Recommendation before the results: (b), if the replay shows that the form holds the six addenda; the
addendum gives the recommendation again with the counts.

**Q2. The cutover: `docs/roadmap/items/` as the source and `docs/roadmap.json` generated.** Options: (a) not now; the
proposals of Q1 (b) are run for a registered number of merges first, and the cutover is decided on their differences;
(b) cut over now, `roadmap.json` generated by `roadmap_shards.py join` and committed, so that its readers
(`render_roadmap.py`, `roadmap_progress.py`, `process_report.py`, `adr_test_index.py`, the Studio capability tooling)
change nothing; (c) cut over and move each reader to the item files. Recommendation before the results: (a). The
addendum states the cost for each reader by reading its code.

## Size

Size (agent): done; 0; 0; 2; 150000

(No heavy run. The two questions are Q1 and Q2. The bytes are the reading list, the six addenda and the diffs of
`docs/roadmap.json` of their merges.)

## Decision rule

The brief's:

- **done**: the six replays are made (A1) and every difference is listed and explained (A3: 0 unexplained); C1 to C7
  each have their test and pass, with W, V, U and E; M1 to M5 are each caught; A0 holds (no file under `docs/` other
  than this ADR changes); P gives no failure other than a generated file that is not current.
- **partial**: a replay could not be made from the addendum alone, or a difference is unexplained, or a registered check
  was not run or not met.
- **stopped and reported**: form 1 cannot hold what an addendum of the six proposes (for example an entry for which the
  form has no key). The work then stops at that addendum and reports it; the form is not widened after the fact without
  an amendment committed first.

A count of differences, however large, does not by itself make the work partial.

## What is not covered

- No change of rule 9, no file under `docs/roadmap/`, no cutover, no change of any reader of `roadmap.json`.
- `cargo test -p rcfd-cli --release --test docs_site` (rule 7, for a change under `docs/`) is not run: no cargo run is
  allowed to this work while the other job's timing is measured. The only file under `docs/` is this ADR; the lead's push
  script runs the test at the merge.
- The other seven ADRs of the population are not replayed.
- Whether an agent writes a correct file unaided is not tested: the six files are written by this work from addenda that
  were written as prose.
- The Markdown views, the SVGs and the ADR index are not generated from item files here.
- Proposals that change an existing item's title, deps, acceptance or phase: form 1 has no key for them; they stay with
  the lead.
