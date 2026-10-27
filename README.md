# rcfd

Clean-room, open source (AGPL-3.0) finite-volume CFD suite in Rust.
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
decisions and must not be used for safety-critical work. It comes with no
warranty of any kind (see the licences). The main gaps today:

* The steady rotating-frame solve of the FAN-01 reference fan case does not
  reach its expected operating point, and the diagnosis is open
  ([fan milestone](docs/milestone-fan.md)).
* There is no structural solver.
* Sliding-mesh transient solves are not available.
* The desktop app is being rebuilt; the new one does not have a window yet.

[The roadmap overview](docs/roadmap-overview.md) says what works today, what
does not and what comes next. See [docs/ROADMAP.md](docs/ROADMAP.md) for the
plan.

## Build and test

Prerequisites:

* Rust 1.90 or later to build the whole workspace. Each crate's
  `rust-version` states its own minimum (1.75 to 1.90): 1.75, or the
  highest minimum its dependencies declare if that is higher. Older
  toolchains are not tested.
* The render crate uses the GPU through wgpu. Without a GPU or a software
  adapter its tests print a skip message and pass. CUDA support is an
  optional feature, off by default ([docs/gpu.md](docs/gpu.md)).
* Python 3.9 or later is optional: the test-gate script and the data fetch
  script use it, and the Python tests skip without it.

Build and test:

    cargo build --workspace --release
    cargo test --workspace --release

The full test suite takes about an hour in release mode on a 16-thread
machine, and much longer in debug mode. The project's gate runs it with
`python scripts/gate_test.py` ([docs/test-process.md](docs/test-process.md)).

## Where to go next

* Documentation, tutorials and the example gallery start at
  [docs/index.md](docs/index.md); the tutorials are executed by the test suite.
* [CONTRIBUTING.md](CONTRIBUTING.md) is the clean-room policy you must agree
  to before you submit a change, and the workflow commands.
* [docs/architecture.md](docs/architecture.md) describes the crate layout.
* [The provenance record](docs/provenance.md) says where rcfd's knowledge
  comes from.
* [SECURITY.md](SECURITY.md) says how to report a vulnerability and which
  parts run programs or listen for requests; [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md)
  and [CHANGELOG.md](CHANGELOG.md) are next to it.

## Licence

rcfd is licensed under the [GNU AGPL v3](LICENSE). Third-party
data shipped in `validation-data/` and `cases/*/reference/` keep their own
terms and attributions; see `NOTICE` and `validation-data/manifest.json`.
The licences of the third-party Rust packages rcfd depends on are listed in
[THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md).

## Trademarks and independence

All product and company names mentioned in this repository are trademarks or
registered trademarks of their respective owners and are used only to describe
file formats or to compare capabilities. rcfd is an independent project and is
not affiliated with, endorsed by or sponsored by any of them.
