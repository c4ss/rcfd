# Brief: F-471, the corrected order of the coarsest dense LU as the default

Branch: f471-coarse-lu-default
Items: F-471
ADR: 0743
Rules version: 3
Model: Claude Fable 5.2
Issued: 2026-11-19
Size (lead): done; 0; 1; 0; 123699

The rules are `docs/process/agent-rules.md`, version 3, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f471-coarse-lu-default` (the lead creates it from main). Scratch folder: `E:/projects/cfd/w49i-work/f471-coarse-lu-default-scratch`.

## Reading list

- docs/process/agent-rules.md 11903
- docs/adr/0369-f474-registration-amg-definiteness-and-coarse-lu-order.md 39615
- docs/adr/0363-f466-registration-bounded-forcing-stop-on-t.md 42728
- crates/rcfd-linalg/src/amg.rs 29453

## Task

A further registration of F-471: `docs/adr/0743-f471-registration-<slug>.md`. The maintainer approved the default change on
2026-11-18: `AmgSettings::coarse_lu_interchanges_first` becomes the default (the roadmap note and gap of F-471 say what is
known: no registered hash moved on a scratch copy; the bounded T run of ADR 0268 and ADR 0363 changes; ADR 0363's records H1,
H2 and H3a and ADR 0369 L1 assert the old behaviour).

- The registration lists the changed results case by case and the records it re-records, before the code: which tests assert
  the old default, what each becomes (an ignored record of the old behaviour under the opt-out, and a new test of the new
  default), and the prediction for each.
- A way to ask for the old order stays, and a test shows it reproduces the old bits.
- Run `cargo test -p rcfd-api --release --test baseline_hashes` and the case hash tests. A changed registered hash is not
  re-recorded: stop there and report it with the case and both values (rule 3; the approval covers the default, not a hash).
- Run the AMG-using test files of `rcfd-linalg`, `rcfd-fv` and `rcfd-api` by name (list them in the registration with the
  reason each is on the list); do not run a whole crate's integration tests.
