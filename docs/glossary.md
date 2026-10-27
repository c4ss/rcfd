# Glossary

The project's own terms, as the other pages use them. General CFD terms (SIMPLE, k-epsilon, y+) are not listed; any CFD textbook
defines them.

| Term | Meaning | Where it is used most |
| --- | --- | --- |
| ADR | Architecture decision record: a numbered page in `docs/adr/` that records one decision or one batch of work, its options, the limits set before the work and the measured results. Cited as "ADR 0089". | [ADR 0089, an example](adr/0089-wave38b-docs-site-tutorials-gallery.md) |
| F-xxx | A roadmap item id, for example F-028. Each item has dependencies, acceptance criteria and a status. | [Detailed roadmap](ROADMAP.md) |
| Done, PARTIAL, not started | Roadmap status marks. PARTIAL (also "partly done", `[~]` in the roadmap) means some of the item works and its note says what is missing. | [Roadmap overview](roadmap-overview.md) |
| Wave | A numbered batch of work, sometimes split into parts with a letter (wave 35C). Its plan and results are recorded in an ADR and in roadmap notes. | [Fan milestone](milestone-fan.md) |
| Gate | The full check run before work is merged: `cargo fmt --check`, `cargo clippy` with warnings as errors, and the whole test suite in release mode on a clean copy of the commit. It takes over an hour. | [Test process](test-process.md) |
| Registered limit | A pass/fail limit written down before the run it judges. If a result misses it, the miss is recorded, not tuned away. | [Roadmap overview](roadmap-overview.md) |
| Known miss | A registered limit that the current code does not meet. Its test is kept, marked as ignored, with the limit unchanged. | [Reference data](reference-data.md) |
| MISSED | A registered target that a measured result did not reach, as written in status notes. | [Fan milestone](milestone-fan.md) |
| UNVERIFIED | A value not checked against its source. "NOT read" means a source was not opened. | [Validation data](validation-data.md) |
| Clean-room | Every algorithm comes from published literature or first principles, with no vendor code, binaries or documentation text. | [Provenance record](provenance.md) |
| Provenance | The record of where a piece of knowledge came from, including every case in which vendor documentation was consulted. | [Provenance record](provenance.md) |
| Bit-identical | A change leaves the existing default results unchanged to the last bit, checked by stored result hashes. | [Detailed roadmap](ROADMAP.md) |
| Case file | A TOML or JSON file that describes one simulation: mesh, physics, boundaries, solver and reports. | [Case schema](case-schema.md) |
| Benchmark descriptor | A `case.json` file in `cases/` describing a validation case and its reference data. It is not a case file and `rcfd run` refuses it. | [Example gallery](tutorials/gallery.md) |
| MRF | Multiple reference frame (frozen rotor): a steady solve in which a rotating zone is computed in its own rotating frame. | [ADR 0008](adr/0008-mrf.md) |
| FAN-01 | The reference fan case: a 500 mm low-pressure axial fan from the FAU / EAA open data set (CAD and measured characteristic, CC BY 4.0). Its steady rotating-frame solve does not yet reach the measured operating point. | [Fan milestone](milestone-fan.md), [validation data](validation-data.md) |
| MCP | Model Context Protocol: the interface through which an AI assistant calls rcfd's tools. | [MCP tools](mcp-tools.md) |
| Studio | The new desktop application, being rebuilt on wgpu. It has no window yet. | [Studio plan](ui-restart-plan.md) |
