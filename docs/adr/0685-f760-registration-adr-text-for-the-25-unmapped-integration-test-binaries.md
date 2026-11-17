# ADR 0685: F-760 registration: ADR text for the 25 integration-test binaries still unmapped in the test-to-ADR index

Status: **REGISTERED 2026-11-17, written before any change of another ADR and before any run of its checks.** Not yet accepted by the
maintainer. Roadmap item F-760 (deps F-737, [ADR 0679](0679-f737-registration-wave-tag-mapping-and-a-primary-adr-in-the-test-to-adr-index.md)
addendum 1, option 4 of its open questions). Number 0685 given by the lead. Branch `f760-work` from f4d77894, worktree
`w49i-work/f760`, scratch `w49i-work/f760-scratch`. No Rust file, golden or hash table is edited, and `scripts/adr_test_index.py` is not
edited (F-759, ADR 0684, changes its primary-ADR rule on another branch; the measurement here uses the script as on this base).

## What F-760 asks

> Every limit not stated here is registered in the ADR before the run. Each of the 25 binaries listed in ADR 0679 addendum 1 is named in
> the ADR that registered or first described it, found by reading the code and the git history, with no Rust file edited; the index shows
> at most 5 unmapped integration binaries with tests; a registered sample of the new mappings is read.

## What was read (before any edit)

* `python scripts/adr_test_index.py --report` on the base, saved as `f760-scratch/before.txt`: 1148 of 1175 integration binaries
  mapped (97.7 %); 27 unmapped test binaries, 25 of them with tests (109 tests): rcfd-api 8, rcfd-assistant 6, rcfd-cli 3, rcfd-core 1,
  rcfd-fv 2, rcfd-io 2, rcfd-post 2, rcfd-turb 1; the other two (`rcfd-fem` `si_eas_hg3_rd`, `si_eas_hg3_rl`) have no tests and are
  not part of F-760.
* For each of the 25 test files: `git log --diff-filter=A --follow` (the adding commit and the files of `docs/` it changed), the header
  comment of the file and its test names. All were added on 2026-10-13 or 2026-10-14, before most ADRs existed; the adding commits
  changed `docs/case-schema.md`, `docs/run-control.md`, `docs/udf-abi.md`, `docs/validation-data.md` or the roadmap, not an ADR
  (only the keychain commit touched an ADR, 0004).
* ADRs 0004, 0008, 0011, 0012, 0013, 0014, 0015, 0024, 0025, 0027, 0029, 0031, 0036, 0060, 0067, 0088 and 0123 by `grep` for the
  item numbers, the file names and the feature words, then by reading the passage found.
* How the index matches a file by citation: a `.rs` token in an ADR whose path ends in `rcfd-<crate>/tests/<file>.rs` (an ADR `E2` hit);
  a bare stem is not enough for stems shared by two crates (`workflow`, `interpolate`, `non_conformal`).

## Rule for choosing the ADR

The ADR that registered the binary; if the binary was committed without a registering ADR (most of them), the lowest-numbered ADR whose
text describes the feature the binary tests, by reading it. A binary whose feature no ADR describes is not guessed: it is named in this
ADR with the reason (and counts against the limit of 5). The note appended to the chosen ADR is a section at the end headed
`## Test files (added 2026-11-17, ADR 0685)`, naming the file as `crates/<crate>/tests/<file>.rs` and the passage of the ADR that
describes it; the text above it is not edited.

## The planned mapping (decided before editing, from the reading above)

| binary | adding commit (date) | ADR to name it | why that ADR |
|---|---|---|---|
| rcfd-assistant/full_loop | f71f6e2c (10-13) | 0004 | the assistant crate design: tools in process, the approval gate enforced below the UI |
| rcfd-assistant/keychain | e14221c9 (10-14) | 0004 | the key storage decision (env, keychain, file; in-memory backend in tests) |
| rcfd-api/sources | 2b85feb6 (10-13) | 0008 | F-031 rotating frame and body-force path (the porous part is reused in 0012) |
| rcfd-io/interchange | f6fa8721 (10-14) | 0011 | the file itself says its checks were registered in 0011 (F-119) |
| rcfd-post/gltf | 49735b69 (10-13) | 0011 | section 4, F-117 GLB structure and export |
| rcfd-api/non_conformal_case | e79f7db0 (10-13) | 0024 | `channel_2d_interface` through the settings tree |
| rcfd-fv/non_conformal | e79f7db0 (10-13) | 0024 | the non-conformal join of two channel blocks (section C) |
| rcfd-assistant/eval | 430b4bb7 (10-14) | 0025 | audit: F-094 eval harness C1-C8 |
| rcfd-api/scalars | e75d6470 (10-13) | 0027 | F-053 species and scalar transport in the incompressible solver |
| rcfd-fv/species | e75d6470 (10-13) | 0027 | same |
| rcfd-api/runctl | 643caf54 (10-13) | 0031 | D5, run-control additions (F-121) |
| rcfd-cli/interpolate | b22ae2b4 (10-13) | 0031 | F-115 restart on a new mesh, `rcfd-post::interpolate` |
| rcfd-post/interpolate | b22ae2b4 (10-13) | 0031 | same (first slice) |
| rcfd-api/properties | e75d6470 (10-13) | 0036 | property laws P1 to P6 use this file's Couette and conduction cases |
| rcfd-cli/data | f67e3c16 (10-13) | 0060 | section "Data pack" names `cargo test -p rcfd-cli --test data` |
| rcfd-io/datapack | f67e3c16 (10-13) | 0060 | same (the manifest it checks) |
| rcfd-assistant/workflow | f71f6e2c (10-13) | 0067 | the first ADR naming F-148 (tool layer F-090, F-147, F-148) |
| rcfd-api/udf | f05fd7dc (10-13) | 0088 | names F-122 ("ADR-less") and its tests `udf.rs`, ABI version 1 |
| rcfd-api/workflow | 643caf54 (10-13) | 0123 | the run service, F-120 and F-121 |
| rcfd-assistant/workflow_engine | 643caf54 (10-13) | 0123 | same |

Not named in an existing ADR, to be named in this ADR with the reason (5 binaries, 20 tests):

| binary | tests | adding commit | reason found |
|---|---|---|---|
| rcfd-api/mesh_params | 4 | 0704ccaf | a wave-11 bug fix documented in `docs/case-schema.md`; no ADR describes the generator parameters |
| rcfd-turb/channel_wall_function | 2 | f71f6e2c | F-040 wall distance and wall functions; ADR 0014 and 0015 describe the turbulence models, not this test |
| rcfd-cli/physics_cases | 7 | f71f6e2c | the shipped turbulence and compressible examples through the binary; ADR 0005 only lists the cases in a hash list |
| rcfd-assistant/physics_workflow | 5 | f71f6e2c | settings-aware assistant helpers; no ADR describes them |
| rcfd-core/table_expr | 2 | 40473ad5 | F-105 tables called from expressions; ADR 0036 lists table-file laws as not delivered, so it does not describe this test |

## Checks (limits fixed now)

| id | limit | estimate behind it |
|---|---|---|
| A1 | each of the 20 binaries of the first table is named, by path, in a note appended to its ADR; for each of the ADRs the diff against the base is additions at the end of the file only | 20 appended notes in 12 ADR files |
| A2 | the 5 binaries of the second table are each named in this ADR with a reason | 5 rows |
| A3 | unmapped integration binaries with tests in the index after the edit: **at most 5** (baseline 25), and those are the 5 of A2 | the 20 notes cite the path form the index matches (E2); the 5 are left |
| A4 | no test loses an ADR; tests that gain an ADR at most 120 (the 89 tests of the 20 binaries, plus allowance for a stem shared by two files) | 109 tests in the 25 binaries, 20 of them in the 5 left |
| A5 | the total of mapped tests rises by at least 85 of 7743 (all tests) and not more than 120 | as A4 |
| A6 | `git diff --stat f4d77894` shows no file outside `docs/adr/` | rule 3 and the acceptance |
| A7 | sample: 8 of the 20 new mappings drawn with Python `random.Random(7600).sample(<the 20 binaries in the order of the first table>, 8)`, each read with its note (the note, the ADR text it cites and the file header); **at least 6 of 8 agree** (the ADR describes the feature the binary tests) | the mapping was chosen by reading; the weak ones (0060 for `datapack`, 0067 for `workflow`, 0036 for `properties`) are expected to be the disagreements, 2 of 20 to 4 of 20 |
| C1 | `python -m unittest discover -s scripts` from the clean worktree: all pass | 85 run, 4 skipped at ADR 0679 |
| C2 | `cargo test -q --release -p rcfd-cli --test docs_site` passes (files under `docs/` changed) | passed at ADR 0679 |

Decision rule: done if A1 to A7, C1 and C2 are met; partial if A3 or A7 is missed or a clause of the acceptance is not met; the limits
are not changed after the run.

## Negative controls and mutations

There is no code. Control N1: the index run before and after is the same script and the same inputs apart from the appended notes.
Control N2: the report of the unmapped list after the edit must contain exactly the 5 binaries of the second table and nothing else
from the first.

## Limits (what is not covered)

* The mapping is a reading: for ADR-less items the ADR is the first one that describes the feature, not a registration of the test.
  It gives the daily gate (ADR 0653) a place to attribute a failure, not a statement that the ADR registered these limits.
* `primary_adr` of the mapped tests is decided by the script as on this base (E2 evidence); ADR 0684 (F-759) may change it.
* The 5 binaries of the second table stay unmapped; they need an ADR written for them or the maintainer's decision.
* The two binaries without tests (`si_eas_hg3_rd`, `si_eas_hg3_rl`), the library and bin targets without tests are not touched.
