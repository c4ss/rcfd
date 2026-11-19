# ADR 0745: citations 2026 week 47, audit: five citations drawn by seed and re-read against their sources

Status: **REGISTERED 2026-11-19**, written before any source of the five citations was opened. Results go in an addendum;
nothing above the addendum is changed afterwards.

Brief: `docs/adr/briefs/citations-2026-w47.md`. Rules: `docs/process/agent-rules.md`, version 3. The task is the weekly
citation item of `docs/process/lead-process.md` ("Weekly", and "Citations from memory: the Sources read addendum"). Number:
the lead gave 0745. Branch `citations-2026-w47`, from main b52b882a. Agent: Claude Fable 5.2. No item of the roadmap; no code
is changed and no cargo command is run.

## What was read before this text

`docs/adr/briefs/citations-2026-w47.md`, `docs/process/agent-rules.md`, `docs/process/lead-process.md`,
`docs/process/citation-batches.md`; the function `from_memory_open` of `scripts/process_report.py`; and, after the draw, the
lines around each occurrence of the words in the five drawn ADRs, to name the citation. No source of any citation was
opened, searched for or fetched before this file was committed.

## The population

Every file `docs/adr/NNNN-*.md` whose text contains the words "from memory" (matched without regard to case, as
`scripts/process_report.py` section e matches them) and has no line that starts `## Sources read (20YY-MM-DD)`. On b52b882a
that is **305** of 736 ADR files, the "open" number of `python scripts/process_report.py --sections e` (311 cite from
memory, 6 have the addendum; the recorded cap in `docs/process/exceptions/from-memory-count.txt` is 305).

## The seed and the draw

Seed 20261119. The draw is `random.Random(20261119).sample(population, 5)` in Python 3, where `population` is the list of
the 305 file names in ascending order (`sorted`). The script is kept in the scratch folder as `draw.py`. It gives, in the
order drawn:

1. `0588-f663-registration-distributed-loads-at-a-height-on-warping-beams.md`
2. `0624-f701-hypothesis-record-and-registered-tests-for-the-long-horizon-loop.md`
3. `0344-f407-registration-matched-sponge-moving-along-the-layer-and-rotating.md`
4. `0028-wave18c-particles.md`
5. `0379-f345-registration-bdf2-for-the-library-moving-mesh-step-and-the-follow-conditions.md`

## The citation in each: the rule, and the five

Rule, fixed here: in each ADR the citation is the one at the first line, in file order, that contains the words. Where that
line cites nothing itself but states a convention for the ADR ("papers are cited from memory where named", "unverified means
from memory only", "all cited from memory" at the head of a list), the citation is the first one in file order that the
convention covers.

| # | ADR | line of the words | the citation, and what the ADR takes from it |
|---|---|---|---|
| 1 | 0588 | 173 | "the remembered textbook `1.13`": the moment factor `C_1` of a doubly symmetric simply supported beam under a uniformly distributed load at the shear centre, recalled as a check on the ADR's computed `1.1310`; no book is named, and the ADR says "not used" |
| 2 | 0624 | 499 (head of the reference list; convention) | the first entry of the list, line 502: Student (W. S. Gosset) (1908), "The probable error of a mean", Biometrika 6, 1-25; the ADR takes the one-sided Student t bounds `m -+ t SE` with `t = t_{1-a, n-1}` (line 157) and the volume and pages |
| 3 | 0344 | 25 (convention: "Papers are cited from memory where named") | the first paper named after it, line 125: "Hu 2001", given in the Sources at line 353 as F. Q. Hu, J. Comput. Phys. 173 (2001), "matched layer with a mean flow"; the ADR takes the statement that the paper discusses growing modes of matched layers with a mean flow, and says "not used" |
| 4 | 0028 | 29 (convention: "unverified" = from memory only) | the first thing so marked, lines 54 to 58: the validity limit of the Schiller-Naumann drag correlation, `Re <= 1000`, with `C_D = 0.44` above (the switch "marked UNVERIFIED"; the ADR's closing list names it first among its unverified references) |
| 5 | 0379 | 135 | D. J. Mavriplis and Z. Yang, "Construction of the discrete geometric conservation law for high-order time-accurate simulations on dynamic meshes", J. Comput. Phys. 213 (2006) 557-573; the ADR takes the swept-volume form of the BDF2 mesh flux at constant step, weights `3/2, -1/2`, and the volume, year and pages |

Another reading of row 4 was open: the first line of ADR 0028 after line 29 that itself holds the words is line 135
(SplitMix64, Steele, Lea and Flood 2014, the constants). The rule above takes the Schiller-Naumann limit because the ADR's
own convention puts it first. SplitMix64 was read in batch 1 for ADR 0273. Row 4 stays as registered.

## What will be done

For each of the five: look for the source in public literature, open it, find the place, and append
`## Sources read (2026-11-19)` at the end of that ADR with one row (citation, status, where, finding) in the form of
`docs/process/lead-process.md`, and "Differences for the lead" with the code that uses the value. Nothing above the appended
section is changed in any ADR. The other citations from memory of the five ADRs are not worked: the brief limits the item
to five citations, and each addendum says so.

- `read` only for a source opened in this task in which the place was found. A second-hand statement is `read` of that
  second source, named as such.
- Vendor documentation is not opened and is not a source. A publisher's page that refuses the fetch is `not obtainable` with
  the addresses tried; no retry under another client name, no sign-in, no download through a browser dialog.
- Row 1 names no source. Its status is expected to be `not a source`; if a public source that prints the factor is
  found, it is named in the finding as a second source.

## Checks

The item has no test and no limit of its own. What is checked at the end:

| check | limit | source of the limit | kind |
|---|---|---|---|
| `python scripts/process_report.py --check` | passes | `docs/process/lead-process.md`, "Merging and pushing" | unchanged default output |
| the open count of section e after the five addenda | does not rise; expected 300 | `docs/process/lead-process.md`, "Weekly"; `docs/process/exceptions/from-memory-count.txt` (305) | unchanged default output |
| `git diff` of the five ADRs against b52b882a | additions at the end only | agent rules, rule 18 | unchanged default output |

This record contains the words itself. Its addendum carries the five rows under the heading `## Sources read (2026-11-19)`,
so it does not count as open after the addendum; between this commit and the addendum the count on this branch is 306.

No negative control and no mutation: there is no code. Not covered: every citation from memory other than these five.

Size (agent): done; 0; 0; 0; 290000

## Decision rule

Done when each of the five ADRs has its appended section with a status from the three allowed, every `read` names the
place and the address, the count has not risen and `--check` passes. Partial if a row could not be written. A row that is
`not obtainable` does not make the item partial: the rule of `lead-process.md` counts citations looked at.
