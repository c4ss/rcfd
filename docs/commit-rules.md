# Commit rules

Status: written 2026-10-27. These rules apply to commits made from now on; the 835 commits before them are not rewritten.

This page is for anyone who commits to rcfd. A commit message is read far more often than it is written: by the reviewer, by
the maintainer assembling a release from `git log --oneline vP..main` ([releasing](releasing.md), step 2), and by whoever runs
`git blame` on a line in two years and needs to know why it is the way it is. The diff already says what changed; the message
says why, and what a user will notice. Each rule below has the rule, the reason, and a good and a bad example.

1. **One logical change per commit.** If the subject needs "and" or a semicolon, it is two commits; as a guide, a commit that
   changes more than about 400 lines outside generated files, fixtures and data says in its body why it cannot be split.
   Why: a small commit can be reviewed, reverted and bisected on its own, and its message can be precise.
   Good: `rcfd-mesh: tet quality pass removes slivers below the dihedral limit`.
   Bad: `docs: add wave 46/47 items to the roadmap; reword F-042 wall-function note` (161 subjects in history have a `; `).

2. **The subject is `area: effect`, imperative or a plain statement, at most 72 characters, no full stop.** The area is the
   crate (`rcfd-fv`), `docs`, `scripts`, `ci`, or `ADR NNNN` for a decision record; the effect is what changes for the person
   using that area, not which files were touched. `Merge ...` and `release X.Y.Z` are the only subjects without an area.
   Why: `git log --oneline` is the index of the history and must be scannable at a glance (640 of the older subjects exceed 72).
   Good: `rcfd-cli: run exits 2 when the case file is missing`.
   Bad: `rcfd-cli: update main.rs and args.rs`.

3. **A change a user would notice edits `Unreleased` in CHANGELOG.md in the same commit**, one line under Added, Changed,
   Deprecated, Removed, Fixed or Security, written for the user. A commit that should change nothing a user can see says
   `No behaviour change.` in its body, as the history already does.
   Why: the changelog is then written when the change is understood best, and `git log --oneline vP..main -- CHANGELOG.md`
   lists exactly the user-visible commits of a release.
   Good: CHANGELOG line `* rcfd run exits with code 2, not 0, when the case file does not exist.`
   Bad: CHANGELOG line `* Refactored args.rs.`

4. **A commit that can break a user, or changes a default numerical result, starts its subject with `[breaking]` or
   `[results]`.** `[breaking]` is anything on the MINOR list of [releasing](releasing.md) (command-line option or exit code,
   case file, MCP tool or service method, UDF ABI, Python function, output format); `[results]` is a different number from an
   unchanged case with default settings. Either one means the next version raises MINOR while the version is below 1.0.
   Why: the release manager finds them with `git log --oneline vP..main | grep -E '^[0-9a-f]+ \[(breaking|results)\]'`.
   Good: `[results] rcfd-turb: SST default uses the 2003 production limiter`.
   Bad: the same change as `rcfd-turb: tidy SST production term`.

5. **The body says why, and is required** for any behaviour change, any `[breaking]` or `[results]` commit, any departure from
   an earlier ADR decision, and any change to validation data, licences or provenance. For `[results]` it names the case, the
   old and new value, and why the new one is right. Wrap at about 80 columns.
   Why: the reader in two years has the diff but not the conversation that led to it.
   Good: `Hysing case 1 rise velocity <old> -> <new> against the reference <value>; the old limiter clipped ...`.
   Bad: a `[results]` commit with no body, or a body that lists the files changed.

6. **Refer to decisions and work by their names:** `ADR 0128`, roadmap item `F-237`, a GitHub issue as `#42`; cite another
   commit by id only if it is already on `main`.
   Why: names survive in search and in a release; ids of unmerged branches are rewritten and then point nowhere.
   Good: `Implements F-237 as decided in ADR 0120 (D2).`
   Bad: `as discussed`, `see the other branch`, `per 3fa9c1e` for a commit that was later rebased.

7. **Refactoring, formatting, mechanical lint fixes and behaviour changes go in separate commits**, the first three saying
   `No behaviour change.` in the body.
   Why: a reviewer can skim a pure move and read a behaviour change closely only if the two are not mixed in one diff.
   Good: `rcfd-fv: cargo fmt` followed by `rcfd-fv: SIMPLEC keeps the boundary compensation`.
   Bad: one commit that renames a module, applies clippy fixes and changes a default.

8. **Validation data, licence texts, NOTICE and provenance records change in their own commit**, whose body names the source,
   the licence and the date fetched, or says UNVERIFIED.
   Why: these are the commits an auditor reads to establish what rcfd may redistribute and where every number came from.
   Good: `docs: state the Fluent .msh reader's format source as UNVERIFIED`.
   Bad: a new reference table added in the same commit as the solver change it validates, with no source in the body.

9. **Tests go in the same commit as the change they cover; acceptance limits are registered in their ADR in an earlier commit.**
   Why: every commit then builds and passes on its own, and the order of commits proves a limit was set before the run it governs.
   Good: `ADR 0129: per-test time limit (registered acceptance)`, then `scripts/gate_test.py: per-test time limit with tree kill`.
   Bad: a commit that widens a limit after the run, or a fix whose test arrives three commits later.

10. **Never in a commit, message or diff:** secrets or tokens; vendor code, binaries or documentation text (CONTRIBUTING,
    clean-room policy); local machine paths (write `<work dir>`, `<target dir>`); real session or request ids (use synthetic
    ones); tool or agent attribution lines such as `Co-Authored-By` or session links; subjects like `wip`, `fix`, `fix fix`,
    `fixup!` on `main`.
    Why: history is public and permanent once pushed; removing anything from it means rewriting every later commit id.
    Good: `docs, rcfd-claude: machine paths replaced by placeholders`.
    Bad: `wip` with `E:\work\gate47\log.txt` in the body.

11. **The only trailer is `Signed-off-by`**, required on every contributed commit as CONTRIBUTING says (`git commit -s`).
    Why: the sign-off certifies the Developer Certificate of Origin; nothing else in a message is machine metadata.

12. **A pull request is merged with its commits kept, not squashed**; tidy them before review (`git rebase -i`, `fixup!`
    commits folded in) so each one follows these rules.
    Why: ADRs cite individual commit ids, and the registered-before-run order of rule 9 exists only as separate commits.

A template with these reminders is in `.gitmessage`; use it with `git config commit.template .gitmessage`.
