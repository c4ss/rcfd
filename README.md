# rcfd

Clean-room, open source (Apache-2.0 OR MIT) finite-volume CFD suite in Rust.
It has a command-line solver, a solver service, an MCP server that lets an AI
assistant drive the solver, and a desktop app. Clean-room means every
algorithm comes from published literature or first principles, with no vendor
code, binaries or documentation text. Vendor documentation was consulted in a
few recorded cases: mostly contents pages, to list capabilities, and some
theory pages, as a check of model constants that are now marked UNVERIFIED.
The [provenance record](docs/provenance.md) lists each case.
[CONTRIBUTING.md](CONTRIBUTING.md) sets out the policy.

## Status and main limits

rcfd is early-stage software. It has not been validated for engineering
decisions, and nothing in it is certified for safety-critical use. The main
gaps today:

* The steady rotating-frame solve of the reference fan case does not reach
  its expected operating point, and the diagnosis is open.
* There is no structural solver.
* Sliding-mesh transient solves are not available.
* The desktop app is being rebuilt; the new one does not have a window yet.

[The roadmap overview](docs/roadmap-overview.md) says what works today, what
does not and what comes next. See [docs/ROADMAP.md](docs/ROADMAP.md) for the
plan.

## Build and test

    cargo build --workspace
    cargo test --workspace

## Where to go next

* Documentation, tutorials and the example gallery start at
  [docs/index.md](docs/index.md); the tutorials are executed by the test suite.
* [CONTRIBUTING.md](CONTRIBUTING.md) is the clean-room policy you must agree
  to before you submit a change, and the workflow commands.
* [docs/architecture.md](docs/architecture.md) describes the crate layout.

## Licence

rcfd is licensed under either of

* Apache License, Version 2.0 (`LICENSE-APACHE`)
* MIT licence (`LICENSE-MIT`)

at your option. Unless you state otherwise, any contribution you submit for
inclusion is dual-licensed as above, without additional terms. Third-party
data shipped in `validation-data/` and `cases/*/reference/` keep their own
terms and attributions; see `NOTICE` and `validation-data/manifest.json`.

## Trademarks and independence

All product and company names mentioned in this repository are trademarks or
registered trademarks of their respective owners and are used only to describe
file formats or to compare capabilities. rcfd is an independent project and is
not affiliated with, endorsed by or sponsored by any of them.
