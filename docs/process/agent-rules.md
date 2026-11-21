# House rules for implementation agents

Version 4 (2026-11-21). This file is the governing copy of the rules for every agent that works on rcfd, in the campaign
stream and in the Studio stream alike. It replaces `w49i-work/AGENT_RULES.md` (2026-11-12) and `w49i-work/briefs/common.md`
(2026-11-08) from the moment the lead's briefs point here; ADRs that cite "house rule N" of the earlier file mean the rule
of the same number below. A change to a rule needs the maintainer's word, raises the version, and is dated in the list at
the end. The product limits of the Studio stay in [studio-guardrails.md](../studio-guardrails.md); where a guardrail and a
rule here speak of the same thing, both apply. The lead's side is in [lead-process.md](lead-process.md).

Follow the rules exactly. Your brief (a file under [docs/adr/briefs/](../adr/briefs/README.md)) names the item, the ADR
number, the branch, the worktree and your scratch folder.

1. Registration before code. First write `docs/adr/<number>-<item>-registration-<short-slug>.md` in the style of the newest
   registrations: the status line "REGISTERED <date>, written before any code of <item> and before any run of its checks";
   the brief it answers (path) and these rules by path and version (`docs/process/agent-rules.md`, version N); what you read and any arithmetic done beforehand; the design
   with its derivation from first principles or a cited published source; the table of checks (rule 14), every clause of the
   roadmap acceptance first; negative controls and planned mutations (rule 16); the size (rule 17); the decision rule (done,
   partial, todo); what is not covered. Commit that file alone as the first commit, before any library or test code.
2. Limits are not changed after a run. A missed limit becomes an ignored test named `*_known_miss` with the limit intact and
   the measured number recorded; it is diagnosed, not tuned away. A defective set-up gets a corrected test by an amendment
   written and committed before the corrected run; the old test stays as an ignored record. Each registered run is made once.
   Existing tests and ignored records are not edited. Parameters are fixed in the registration from a derivation, not searched
   for after seeing results. A Studio item records an amended acceptance and a miss in the same way.
3. Defaults stay bit-identical: anything that does not ask for the new capability produces exactly the bytes it produced
   before; new behaviour is opt-in. Never edit or re-record `crates/rcfd-api/tests/baseline_hashes.rs`, the case and mesh hash
   tables or any golden file (schema golden, CLI help, MCP tools list). If your design needs such an edit, build and check
   everything that does not, leave that step out, and report it as the step that needs the maintainer's approval. One class
   is approved in advance (the maintainer, 2026-11-18): where the edit only adds entries (a new key in the schema golden, a
   new tool in the tools list, new lines of CLI help, new rows of a case hash table) and removes or changes none, report it
   as "additive golden edit" with the diff; the lead re-records it and puts the diff in the ledger. `baseline_hashes.rs` is
   never in this class. If you change a shared solver file, run `cargo test -p rcfd-api --release --test baseline_hashes`.
4. Clean-room, from sources that were read. Every algorithm comes from published literature or first principles and is cited
   in the ADR. No vendor code, binaries or documentation; a fetched vendor page is not a source. Cite a source you have read,
   in the repository or fetched, with its place in it: an equation, table or page number, or a DOI. A citation from memory is
   allowed only where no source could be read; say so in those words and derive what you use. Reference values come from the
   repository ([reference-data.md](../reference-data.md), existing tests), a closed form, or a source read and located.
   A new reference value goes in a new file `docs/reference-data.d/<ADR number>-<item>.part`; the page is generated from
   those files and is never edited.
5. No new third-party crate, no unsafe code, and the crate graph stays acyclic. A new Studio dependency follows guardrail G5.
6. Open questions. Where you would ask the maintainer, take the option you would recommend and record in the ADR "taken on
   the maintainer's standing instruction of 2026-11-11 (take the recommendation)". Do not present it as a decision the
   maintainer made in person. If a question would change a default result, a golden or a hash table, changes the scope or
   priority of the item, or needs a paper, standard or data set that cannot be obtained, do not take it: list it as open
   (rule 8).
7. After the code, run your checks in the release profile and say which: your registered tests, the unit tests of the crates
   you touched (`--lib`), the named existing test files that exercise the code you changed, `baseline_hashes` if you touched a
   shared solver file, `docs_site` if you touched `docs/`, and `npm test` in `apps/studio-web` if you touched it. Then
   `cargo fmt -p <crate> --check` for each crate touched and `cargo clippy -p <crates> --all-targets -- -D warnings`. Do not
   run the whole workspace suite or a whole crate's integration tests: the lead's push script and the full run do that. Never
   kill a cargo process that is not yours, and do not stop your own runs to save time. The machine is shared: prefer small
   checks. A new test that takes more than about two minutes in release is `#[ignore]`d as a heavy record, run once by you,
   with a fast test on the same code path. Last, before your report, run `python scripts/process_report.py --check` in your
   worktree and say what it gave. Every failure it names is corrected before the report (a registration is corrected by an
   amendment, not by an edit), except one: a generated file that is only not current, such as `docs/adr/index.md`, is named
   in the report and left to the lead (rule 9).
8. Write "Addendum 1: results" in the ADR: every check with the measured number and whether the limit was met, deviations,
   what was not run, the actual size (rule 17), and the decision by the registered rule. A miss with a diagnosed cause is an
   acceptable outcome; in that case, or if a clause of the acceptance was not met or not run, propose partial, not done.
   Questions that rule 6 leaves open go under the fixed heading "Open questions for the maintainer", each with options and a
   recommendation. The lead writes the dated answer under "Answers of the maintainer (<date>)" in the same ADR, on main.
9. The roadmap is written on main only, by the lead, in both streams. Do not edit `docs/roadmap.json`, `docs/ROADMAP.md`,
   `docs/roadmap-overview.md`, `scripts/roadmap_progress.py`, the roadmap SVGs, `docs/adr/index.md` or `docs/process/ledger.tsv`.
   A branch that needs a new item asks the lead and gets it by merging main. End the addendum with "For the lead: roadmap
   entries": the status you propose, the new note entry for your item as one line (ADR number, date, outcome, one sentence),
   for a partial item its `gap` (at most 500 characters that say what is missing now; it replaces the previous gap), and any
   follow-up items with title, deps and acceptance. Each is one quoted paragraph on a single line starting with "> ". A
   follow-up's acceptance names a number or a named reference, or hands its limits to the registration. Describe the work as
   "ADR <number>, <date>" without the words "branch" or "not merged". You may update the topic page under `docs/` of your
   subject, except its generated sections.
10. Scratch files (mutation scripts, logs, copies) go in your scratch folder named in the brief and nowhere else.
11. Commits: plain messages in the repository's style, unsigned, with no Co-Authored-By or other trailers. Commit on your
    branch only. Do not push, do not merge into main, do not touch other branches or worktrees or the main checkout.
12. Test headers. Every test root file under `crates/*/tests` names its registering record in its leading comment as
    `ADR NNNN`, and its item as `F-NNN` where it has one. A Studio spec file names its item or its ADR in the comment at the
    head of the file. A failing test is linked to its record through this line.
13. Follow-ups of a miss. A follow-up that adds no acceptance clause of its own, because it diagnoses or remedies a miss of an
    item, is registered under that item: its ADR is named `NNNN-fMMM-registration-...` with the parent's item number, rules 1
    and 2 apply, its commits cite its ADR number in the subject, and the item stays partial until a registered check of its
    acceptance passes. A follow-up with its own acceptance is a new item. You propose either; the lead decides at issue.
14. The table of checks has, for each row: the check, the limit, the source of the limit, the estimate, the margin (limit
    divided by your estimate) and the kind, one of: unchanged default output, code verification, solution verification,
    validation. An item whose acceptance claims agreement with a measured case is not done without a passing row of kind
    validation.
15. Each limit cites its source: a row of the [tolerance table](tolerances.md), a published reference with its place in it, or
    the acceptance of the parent item. A limit whose only source is your own estimate is listed by the process report, and the
    lead does not issue the item's code until it has a source.
16. One committed mutation. Run the planned mutations on a scratch copy and record what caught each, as before. In addition
    commit one of them as a patch file beside the test it must fail (`<test file stem>.mutation.patch`, made with `git diff`; its first line is `# expect-fail: <the command that must fail>`).
    The lead's push script applies it, runs that test, expects a failure and removes the patch.
17. Size. The registration states five values on one line that starts `Size (agent):`, fixed before any code: the expected outcome of this registration (done or
    partial), further registrations expected, heavy runs on the shared machine, questions for the maintainer, and bytes to
    read. The addendum states the actual five. Your size is scored for calibration; nothing is charged against it.
18. Records. A new ADR file name contains one kind word: registration, amendment, diagnosis, proposal, plan, record, review,
    audit, job or wave. Every ADR is append-only from its creating commit: results, answers and corrections are added as
    dated addenda, and nothing above them is changed. When an ADR has reached the length limit the maintainer sets, a further
    decision in its subject is a new ADR.

## Report

Your final message, under 450 words: branch and tip commit; the commits; files changed (exact line ranges for any shared
solver file); the results table (check, limit, measured, met); mutations and what caught them; fmt and clippy result; the
decision by the rule with the reason; open questions with your recommendation; anything you could not do and why. State
plainly anything that failed or was skipped.

## Terms

- Maintainer acceptance: the maintainer's go-ahead for a line of work to be scoped or prioritised, written in the priority
  statement of `docs/ROADMAP.md`. It is not a review of each result. "Done" is the lead's decision by the registered rule.
- Line of work: a category of `scripts/roadmap_progress.py`, or an id range the priority statement names.
- Verification: a check against the equations or the code's own earlier output. Validation: a check against a measured case.

## Changes

- Version 4, 2026-11-21: rule 7 asks for a run of `scripts/process_report.py --check` before the report (the maintainer's
  word of that day, from finding 17 of F-851, ADR 0752).
- Version 3, 2026-11-18: rule 4 names the fragment files of the reference-data page, which is now generated.
- Version 2, 2026-11-18: rule 3 gains the class of additive golden edits, approved in advance by the maintainer and
  re-recorded by the lead.
- Version 1, 2026-11-18: rules 1 to 11 carried over from `w49i-work/AGENT_RULES.md`; rule 4 requires sources that were read,
  with a locator; rule 8 gains the two fixed headings; rule 9 covers both streams, the one-line note entry and the `gap`;
  rules 12 to 18 added. From the documentation process review and the maintainer's decisions of 2026-11-18.
