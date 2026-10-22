# rcfd

Clean-room, open source (AGPL-3.0) finite-volume CFD suite in Rust.
See `docs/ROADMAP.md` for the plan, `CONTRIBUTING.md` for the clean-room
policy and `docs/architecture.md` for the crate layout.

    cargo build --workspace
    cargo test --workspace

Documentation, tutorials and the example gallery start at `docs/index.md`; the
tutorials are executed by the test suite.

## Licence

rcfd is licensed under the [GNU AGPL v3](LICENSE). Third-party
data shipped in `validation-data/` and `cases/*/reference/` keep their own
terms and attributions; see `NOTICE` and `validation-data/manifest.json`.

## Trademarks and independence

All product and company names mentioned in this repository are trademarks or
registered trademarks of their respective owners and are used only to describe
file formats or to compare capabilities. rcfd is an independent project and is
not affiliated with, endorsed by or sponsored by any of them.
