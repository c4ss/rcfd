# Glossary

The project's own terms, as the other pages use them, and the words the Studio's explanations take from here (`rcfd help
glossary` and the service's `help.glossary` return this table, ADR 0730). General CFD terms (SIMPLE, k-epsilon, y+) are not
listed; any CFD textbook defines them.

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
| Studio | The desktop application: the Electron app in `apps/studio-web/` (ADR 0800), in development and run from a checkout. It opens a project folder, edits its cases, starts and follows runs through the run service, shows a run's stored fields through time, compares runs and registers its project server with Claude Code, which runs in its own terminal (ADR 0801 Addendum 23). The older egui apps `rcfd-studio` and `rcfd-gui` were removed (F-449). | [The desktop tutorial](tutorials/desktop-assistant.md), [Studio plan](ui-restart-plan.md) |
| Template | A ready-made case the engine can write into a project (`new_case {name, template}`). The engine checks the case before it writes the file, never replaces a file, and keeps a line in the project journal. | [MCP tools](mcp-tools.md) |
| Lid-driven cavity | A square box of fluid whose top wall, the lid, slides sideways and drags the fluid round inside it. It is the first example of most flow solvers; the engine's template `lid_driven_cavity` is one, and so is the case of the command-line quick start. | [Command-line quick start](tutorials/quick-start-cli.md) |
| Run | One solve of a case by the project's runner, kept in `runs/<run id>/` with its settings, status, log and results. It goes on when the Studio is closed. | [Studio plan](ui-restart-plan.md) |
| Runner | `rcfd-runner`, the run service of a project folder: it owns every run of the project, queues the runs and starts their solvers. `rcfd-runner ensure` starts it. | [Run control](run-control.md) |
| Queued | The state of a run the runner has accepted but not started yet; it starts when a slot is free. | [Run control](run-control.md) |
| Running | The state of a run whose solver is computing; its step is the iteration or time step it has reached. | [Run control](run-control.md) |
| Iteration | One round of calculation of a steady solver. The solver repeats iterations until the residuals fall below the case's tolerances (`solver.tolerance_u`, `solver.tolerance_p`) or it reaches `solver.max_iterations`. | [Case schema](case-schema.md) |
| Converged | A steady run that stopped because its residuals met their targets; the engine's words are "residual targets met". It means the calculation stopped changing, not that the answer is right. | [Run control](run-control.md) |
| Field | A value in every cell of the mesh, such as `ux` (the x component of the velocity), `uy` or `p`, as a run stores it. | [Case schema](case-schema.md) |
| Field store | The fields a run stores as it goes, for playback (`output.store_interval`, in `runs/<run id>/fields`). The engine's start plan (`start_run` with `store: plan`) stores them about 100 times over the run, in single precision. | [MCP tools](mcp-tools.md) |
| Snapshot | One stored state of the fields in a run's field store, with its number, its step (the iteration it was stored at) and its time t. | [MCP tools](mcp-tools.md) |
| Field view | The Studio's picture of a stored field: the boundary surface of the mesh drawn as triangles, each coloured by the stored value of the cell that owns it, on the viridis colour map from the lowest to the highest value shown. | [ADR 0801](adr/0801-studio-web-dependencies-and-layout.md) |
| Report | A number a run computes as it goes, named in the case's `[reports]` table, such as the force on a patch (`type = "force"`). The engine gives no unit for report values. | [Case schema](case-schema.md) |
