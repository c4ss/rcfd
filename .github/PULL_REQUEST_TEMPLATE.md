**What this changes and why**

**Roadmap item / ADR**

**Checklist**

- [ ] `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings` are clean.
- [ ] The full test gate passes (`scripts/gate_test.py`, see `docs/test-process.md`); totals:
- [ ] New acceptance limits are registered in an ADR before the runs they govern, and no existing limit was widened.
- [ ] Provenance declared: every algorithm, constant, data file or table has its published source (or "first principles")
      named here or in the doc comments, data licences are stated, and unchecked values are marked UNVERIFIED. No vendor code,
      binaries or documentation text was used (`CONTRIBUTING.md`).
- [ ] Every commit carries a `Signed-off-by:` line (DCO, `git commit -s`).
