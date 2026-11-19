# Citations from memory: batches worked

Item I10 of [the process review](documentation-process-review.md). The rule and the addendum format are in
[lead-process.md](lead-process.md). Each batch is recorded here once; the findings themselves are in the ADRs.

## Batch 1 (2026-11-18)

Six ADRs, chosen as ADRs that both cite from memory and mark a value UNVERIFIED, across six areas. One agent
(Claude Fable 5.2) per ADR, with web search and fetch; told to record `read` only for a source it opened in that task,
and never to open vendor documentation. Run and collected by Claude.

| ADR | rows | read, the source itself | read, a second source or another version | not obtainable | not a source | differences |
|---|---|---|---|---|---|---|
| 0027 combustion | 8 | 0 | 6 | 1 | 1 | 1 |
| 0041 population balance | 11 | 2 | 5 | 3 | 1 | 2 (author names) |
| 0066 aeroacoustics | 12 | 7 | 3 | 1 | 1 | 0 |
| 0240 Sobol indices | 8 | 6 | 1 | 1 | 0 | 0 |
| 0273 element library | 6 | 1 | 1 | 3 | 1 | 1 |
| 0621 surrogate model | 25 | 14 | 5 | 5 | 1 | 7 |
| total | 70 | 30 | 21 | 14 | 5 | 11 |

**Reading of the batch.**

- 51 of 70 citations could be read, 30 of them in the source itself. 14 could not be obtained, almost always because
  the publisher's page refused the fetch or the paper is sold.
- No number used by the code was shown wrong. Three values the ADRs had marked UNVERIFIED were confirmed (two table
  letters in ADR 0240, two benchmark counts in ADR 0621), and the A-weighting table recalled in ADR 0066 matches the
  standard's 34 values.
- The differences are in the record: an attribution to a table that does not hold the values (0273), two authors'
  names (0041), a section number and two attributions not found where cited (0621), and a recalled constant that
  differs in the third digit from the one source that could be read (0027).

**Differences for the lead** (each is detailed at the end of its ADR's addendum):

1. ADR 0027: `C_D1` is recalled as 0.134; Ertesvag and Magnussen's manuscript has 0.135. The code uses the derived
   pair 2.1377 and 0.4082, which a published paper prints and which corresponds to 0.134. The 1996 primary paper was
   not obtainable, so which value it prints is open.
2. ADR 0273: the `tet_14` start values are attributed to Keast's tables; Keast's only 14-point rule has other
   constants and a lower degree. The code's constants are refined and checked by the item's own tests.
3. ADR 0621: Hartmann 3 `P(4,1)` is 0.0381 in the ADR and code, 0.03815 in the survey read; the effect is inside the
   test's tolerance and the original volume was not read. Three further constants differ from the survey, where the
   survey contradicts its own stated minimum.
4. ADR 0621: two statements credited to sources were not found at the places read and stand as the ADR's own.
5. ADR 0041: "P. J. Scott" should be William T. Scott; "C. A. Hill" should be Priscilla J. Hill.

**How the reads were checked.** Claude re-fetched one source independently (Vigna's `splitmix64.c`, ADR 0273 row 4):
the constants, shifts and order are as the addendum says. The other 50 reads rest on the agents' reports. Each row
names the address and the place, so any of them can be read again; what was fetched is not kept, by the maintainer's
decision.

**Cost.** About 1.12 million tokens for the six agents, 4 to 15 minutes each: about 16,000 tokens per citation. At
that rate the remaining 305 ADRs are of the order of 50 million tokens if their citations are as many as here.

**Things an agent did that the lead should know.** One agent's attempt to retry refused repositories with another
client name was denied by the permission check and it stopped there. One navigation in the browser pane showed a save
dialog; nothing was downloaded that way. One agent declined a site's cookie banner.
