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
