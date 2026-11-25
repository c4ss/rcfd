# Brief: F-894, a normal distribution open on one side, which panics in the tolerance and Sobol drivers and ends the MCP server

Branch: f894-normal-one-open-side
Items: F-894
ADR: 0792
Rules version: 5
Model: Claude Opus 5.5
Issued: 2026-11-25
Size (lead): done; 0; 0; 0; 136865

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f894-normal-one-open-side`, a new branch from main at the commit that adds this brief. Scratch folder: `E:/projects/cfd/w49i-work/f894-normal-one-open-side-scratch`.

## Reading list

- docs/process/agent-rules.md 12991
- docs/adr/0791-f339-registration-arguments-of-design-uq-and-design-sobol.md 31727 (findings 1 and 2, K1, addendum 1 (M10, M11, M14, M15 are caught through the panic), the lead's reading)
- crates/rcfd-api/src/design/uq.rs 33111 (norm_cdf, norm_inv, Dist::validate and Dist::inv_cdf)
- crates/rcfd-reactor/src/mixfrac.rs 21232 (erf, erfc and erfc_inv)
- crates/rcfd-mcp/tests/design_uq_sobol_args_f339.rs 37804 (k1_one_open_side_known_miss, and B1, whose mutations are read again)

Also in the repository, as needed: every caller of `erfc`, `erf` and `erfc_inv` (`grep -rn erfc crates --include=*.rs`: the flamelet and the fan design call them), the line of `docs/mcp-tools.md` that documents the open side, `docs/design-loop.md`, the existing tests of the distributions in `crates/rcfd-api/src/design/` and of `erfc` in `crates/rcfd-reactor`, and the records of ADR 0791, read only: `E:/projects/cfd/w49i-work/f339-uq-sobol-test-gaps-scratch` (mutate.py).

## What the maintainer decided (2026-11-25)

On 2026-11-25 the maintainer answered "Yes, issue it" to entering and issuing the fix of K1 of ADR 0791. `erfc(+inf)` is not a number, so `norm_cdf` is not a number at an infinite bound, `Dist::validate` lets the kept mass pass (`NaN < 1e-12` is false) and `inv_cdf` hands `erfc_inv` a value that is not a number, which asserts (`mixfrac.rs:161`); the call ends `rcfd-mcp`. The lead reproduced the panic by K1. The sentence this work must be able to write: every one-sided normal samples within its finite bound and agrees with the closed form of the truncated inverse CDF within the registered limit, and every other result is bit-identical to main.

## Task

One registration, `docs/adr/0792-f894-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before any code or test.** (a) The closed form and how the reference is computed independently of the
   code under test (from first principles or a published source that was read), at samples `u` that include both ends of
   the unit interval, for a bound at the mean, near it and far out in each tail up to the smallest kept mass `validate`
   accepts; the error limit of each, derived, not found by running. Say where the double-precision form
   `Phi(a) + u (Phi(b) - Phi(a))` loses accuracy with one open side (a bound far out in the upper tail) and register what
   is done there: a limit that holds, or a form that keeps the accuracy and leaves every two-sided and two-open result
   bit-identical. (b) `erfc(+inf) == 0` and `erfc(-inf) == 2` exactly, and `erfc`, `erf` and `erfc_inv` bit-identical for
   finite arguments over a registered set. (c) The refusals: a kept mass that is not a number or below 1e-12 is refused by
   name and nothing is written. (d) `uq::run`, `sensitivity::run`, `design_uq` and `design_sobol` with `lo` alone and
   with `hi` alone, each form of the open side (absent, `null`, the string), bit-identical among the forms. (e) One call
   with one open side through the built `rcfd-mcp` on stdio that is answered, followed by a `ping` that is answered. (f)
   Bit-identity: before any change, record from main the results the fix must not move (two finite bounds, two open
   bounds, uniform, beta; both drivers), and compare after it.
2. **Fix it** in `erfc` and, where the registration says so, in `Dist`. `erfc` is called by other code: list every caller
   and state what it can pass.
3. **K1 and the mutations.** `k1_one_open_side_known_miss` is not edited (rule 2): it passes when run with `--ignored`,
   and a gated test of your own file holds the same limits. M10, M11, M14 and M15 of ADR 0791 were caught through the
   panic, which the fix removes: run those four again after it, record which test catches each, and add a gated test for
   any that is no longer caught. Commit one mutation patch of the fix that a registered test must catch.
4. Run your new file, `design_uq_sobol_args_f339`, `design_uq_sobol_f339`, the tests of `rcfd-reactor`, `rcfd-api --lib`,
   the tests of every crate that calls `erfc`, and the checks rule 7 lists.

Decision rule to register: done if every registered row holds, K1 passes with its limit unchanged, M10, M11, M14 and M15 are each caught after the fix, the committed mutation is caught, and the results of (f) are bit-identical; partial otherwise. Whether F-339 is then done is the lead's decision at the merge.

Open questions for the maintainer, with options and a recommendation, not decided by you: none expected; any that arises, with options and a recommendation.

Files you own: `erfc` and its tests in `crates/rcfd-reactor/src/mixfrac.rs`, `Dist` in `crates/rcfd-api/src/design/uq.rs`, new test files under `crates/rcfd-api/tests` and `crates/rcfd-mcp/tests`, and the sentence of `docs/mcp-tools.md` or `docs/design-loop.md` only if the registration finds it wrong. No golden, no existing test, no tool-list text. If the fix moves any golden or any default result, stop and report.

## Common to this work

The acceptance of F-894 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
decision rule is registered before the run and is not changed after it; a result that misses is recorded as a miss with
its number; a run that does not decide is recorded as not deciding. No test is ignored, skipped, removed or quarantined
to make a run pass; a registered miss is held as the repository holds known misses, with its limit intact.

Clean-room: reference values only from published sources that were read and that a page of the repository records with
their citation; nothing from memory (rule 4); vendor documentation is never a source. New reference values go in as a
fragment under `docs/reference-data.d/`, not as an edit of a generated page.

Use your own target folder under the scratch folder for every cargo command (`CARGO_TARGET_DIR`). No other agent of
the lead runs when this is issued; the machine may carry other load; a test that fails on a file refused by the operating system under
that load is reported, not retried until it passes. Register no limit on wall-clock time. Change no solver source but the functions named under the files you own; if another defect of the library appears, report it. Delete nothing
outside your worktree and your scratch folder. Commit large run records only as far as they are the evidence; prefer a
summary table with the full record in the scratch folder.

The lead merges with `scripts/push_main.py`. Do not push and do not merge. Commit messages are plain: no agent marker,
no trailers. Rule 9: you do not edit `docs/roadmap.json`, `docs/milestones.json` or a generated page; you write your
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0792-F-894.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; what the gate runs and what is a heavy record; mutations or negative controls; deviations; what was not
run; open questions with options and a recommendation; the result of `python scripts/process_report.py --check`; your
verdict by the registered rule.
