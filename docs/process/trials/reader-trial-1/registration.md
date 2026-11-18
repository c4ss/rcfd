# Reader trial 1: does a reader of another model find what a reader of the author's kind does not?

Registered 2026-11-18 by Claude, before any reader was run. It is item I9 of
[the process review](../../documentation-process-review.md). The maintainer judges the result; no rule changes here.

## Question

Three kinds of defect can pass every machine check: a stated result that does not follow from what the ADR records,
a test that cannot fail, and a limit so loose that it tests nothing. The trial measures how often two kinds of
reader find such a defect when one is planted, and what else they report.

## Sample

Drawn by `python docs/process/trials/reader-trial-1/select.py` at commit 10ba1a34, seed 20261118, from the done
items of the category "Structural solver" that have exactly one ADR named for the item (at most 60 kB) and test files
named for the item or its ADR (at most 60 kB together). 37 items are eligible; 8 are drawn:

| item | planted | ADR |
|---|---|---|
| F-638 | none | 0571 |
| F-661 | result | 0585 |
| F-595 | limit | 0531 |
| F-571 | test | 0604 |
| F-534 | test | 0453 |
| F-639 | result | 0570 |
| F-627 | limit | 0553 |
| F-615 | none | 0533 |

## Planted defects

A planter (one agent, not a reader) copies each item's ADR and test files to a folder outside the repository and
changes the six planted items, one defect each:

- **result**: one number in the results part of the ADR is changed so that a stated conclusion no longer follows
  from the numbers beside it (for example a measured value moved outside its stated limit while the text still says
  it is met).
- **test**: one assertion of a test is changed so that it holds for any computed value (for example a value compared
  with itself, or a bound replaced by one that cannot be exceeded), with the surrounding text unchanged.
- **limit**: one registered limit is widened by a factor of 100 in the ADR and in the test together, so that both
  agree and the limit is far from the recorded value.

The planter writes a key (file, line, what was changed) that only the scorer reads. A plant must not be detectable
by a comparison inside one file that a machine check would make (the copies still parse; the test file still reads as
compilable Rust). The two items marked "none" are copied unchanged.

## Readers

Each of the 8 items goes to two readers, each a fresh agent that sees one item only:

- **same kind**: the model this session runs on (Claude Fable 5.2). Which Claude model wrote each item is not
  recorded, so "same kind" means the newest Claude model, not the author.
- **other model**: Claude Sonnet 5.5.

Both are Claude models. No model of another maker is installed here, so the trial compares two Claude models, which is
a weaker contrast than I9 asked for.

A reader is given the folder of its item (the ADR copy and the test copies), told to read nothing else, and asked for
three lists: results that do not follow, checks that cannot fail, and limits that test nothing; each finding with
file, line and one sentence of reason. It is not told that defects were planted or how many there are. The
instruction to read nothing else is not enforced.

## Scoring

- **Detection** (decided by the key, no judgement): a planted defect counts as found when a finding names the changed
  line (within 3 lines) or the changed quantity, in the list of its kind or another. 6 plants per reader.
- **Findings on unchanged text**: counted per reader and listed. They are not judged true or false here; a sample of
  them is described so the maintainer can judge whether they are worth a reader's time.
- **Cost**: tokens and wall time per reader, as the agent tool reports them.

## What the trial cannot show

Six plants per reader: a difference of one or two detections is within chance, so only a large difference (four or
more of six) is read as a difference. Planted defects are cruder than real ones. One category. A reader that finds
all plants may still miss real defects of other kinds.

## Recorded after the run

The results go below this line as an addendum; nothing above it is edited.

## Results (2026-11-18)

Run the same day by Claude: one planter agent, then 16 reader agents, each a fresh agent with one item. The readers'
answers, the key and the list of agreed findings are in `results/`; `score.py` recomputes the counts from them.

**Detection of the six plants.**

| item | planted | same kind (Fable 5.2) | other model (Sonnet 5.5) |
|---|---|---|---|
| F-661 | result | found | found |
| F-639 | result | found | found |
| F-571 | test | found | found |
| F-534 | test | found | found |
| F-595 | limit | found | found |
| F-627 | limit | missed | found |
| total | | 5 of 6 | 6 of 6 |

By the registered reading, a difference of one is within chance: the trial shows no difference between the two
readers. Both found every changed result and every changed test. The one miss was a widened limit that agreed in the
ADR and the test.

**Findings on unchanged text.** Same kind 54, other model 64, over the eight items; none judged true or false here.
The two unplanted items drew findings like the planted ones (F-638: 4 and 12; F-615: 6 and 6), so a reader does not
return an empty list on a clean item. 19 findings were made by both readers at the same place (within 3 lines).
Three of those follow from a plant (F-661 lines 300 and 327, F-571 line 351). The other 16 are in
`results/agreed-on-unchanged-text.txt`. Examples, each stated by both readers independently:

- F-638, `sa_contact_mortar_recovery_hertz_f638.rs:61`: a check asserts the literal `true`; the penetration it names
  is printed and not compared with its tolerance.
- F-615, `sa_contact_supernodal_f615.rs:639`: the negative control N2, the only check registered to catch mutations
  M5 and M6, is `#[ignore]`.
- F-534, `sa_fatigue_f534.rs:89`: G3 requires only `is_ok()`; the returned amplitude is not checked.
- F-627, ADR 0553 line 415: "every measured value is at or below its estimate, except A1'" contradicts the table
  (C1 is above its estimate, A1' is below).
- F-615, ADR 0533 line 245: the explanation by nine factorisations does not fit the recorded times.

**Cost.** Tokens as the agent tool reports them: same kind 769,654 for 8 items, other model 743,947. Mean wall time
per item 100 s and 57 s. The planter used 125,121 tokens.

**Deviations from the registration.**

- F-595: the limit was widened in the ADR only (two places). The item's test file is a bit record with no numeric
  limit, and the test that carries that limit is not named for the item, so it was not in the folder.
- Two line hits of the scorer are a neighbouring line, not the plant (F-595 line 162, F-534 line 26); each of those
  readers also named the planted line, so no count changes.
- The same-kind reader of F-661 named the planted line in its one-line reply; nothing else was read from the replies.

**What this does and does not show.**

- A fresh reader of either model, given only the ADR and the tests, found 11 of 12 crude planted defects in one to
  two minutes per item. The question of I9, whether another model finds what the author's kind does not, gets the
  answer "no difference seen", with two Claude models and six plants.
- The larger observation is outside what the trial was built to measure: on text nobody changed, two independent
  readers agreed on 16 places in eight done items. Whether those are real defects is not established. If most are,
  the useful rule would be a fresh reader per item before it is marked done, whatever the model, not a second model.
- Not shown: the rate of real defects in done items, the rate of false findings, and anything outside the structural
  category.

The maintainer judges whether a rule follows. The 16 agreed findings are listed for the lead; none was acted on.

## Check of the 16 agreed findings (2026-11-18)

Asked for by the maintainer after the results. Four checker agents (Claude Fable 5.2) read the repository itself for
each place: the ADRs, the tests, the source and the other tests, which the readers had not seen. Their verdicts and
evidence are in `results/check-of-agreed-findings.json`. Claude then compared three of the verdicts with the source
(F-638 line 61, F-615 line 639, F-627 line 415) and they held. No test was run and nothing was fixed.

| verdict | count | places |
|---|---|---|
| not a defect | 5 | F-638 test lines 61 and 106; F-595 ADR line 159; F-615 test line 639; F-534 test line 89 |
| real but minor | 10 | F-661 test line 397; F-639 ADR line 225; F-595 ADR line 293; F-615 ADR line 245; F-534 ADR lines 112 and 125; F-627 ADR lines 411, 416, 453, 464 |
| real | 1 | F-627 ADR line 415 |

- **Not a defect (5).** Each is explained by something outside the reader's folder or stated earlier in the ADR. The
  literal `true` in F-638 reports a solve whose tolerance the driver enforces. The ignored N2 of F-615 was ignored by
  a later item that lifted the refusal on purpose, and its guard runs in that item's test. G3 of F-534 is covered by
  another test file. Three of these were the examples named in the results above.
- **Real but minor (10).** Seven are statements in the record that are loose, over-general or miscounted. Two are
  supporting checks weaker than their text says (F-661 HB-, F-639 moment radius). One is a status question the ADR
  leaves open (F-627), which the status rule of 2026-11-18 settles as done. None changes a verdict.
- **Real (1).** F-627, ADR 0553 line 415: "every measured value is at or below its estimate, except A1'" is false.
  A1' is below its estimate; C1, D1 and E2'(b) are above theirs. All are inside their limits, so the item's result
  stands and the sentence is wrong.

**What this adds.** No finding showed a wrong result or a check that passes wrongly: every done item of the sample
keeps its status. Of 16 places that two readers agreed on, 11 were true as stated, and all 11 concern the quality of
the record or the strength of a supporting check. Five were false alarms caused by the reader not seeing the rest of
the repository. A fresh reader per item would therefore mostly improve the records, at about 95,000 tokens per reader
and item plus about 29,000 per finding checked, and a third of what two readers agree on would still need checking
against the repository. The 38 and 48 findings made by one reader only were not checked.

**Left for the lead.** Each real entry has a proposed fix in the JSON. ADRs are append-only, so a fix is an addendum
to the ADR or a registered follow-up; the two weak supporting checks and the roadmap note of F-627, which still opens
with "PARTIAL", are the ones that touch more than wording.
