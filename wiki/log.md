---
type: log
---

# Log

Append-only. Newest at the bottom. Entry header: `## [YYYY-MM-DD] <op> | <title>`. Last five: `grep "^## \[" log.md | tail -5`

## [2026-11-22] init | Vault created
- Folders: `raw/`, `raw/assets/`, `sources/`, `entities/`, `concepts/`, `analyses/`, `templates/`, `tools/`.
- Schema: `CLAUDE.md` (ingest / query / lint), `AGENTS.md` pointer.
- Seed pages: [[overview]], [[karpathy-llm-wiki]], [[llm-wiki]].

## [2026-11-22] schema | Ingestion plan for existing repo docs
- Added [[ingest-plan]]: inventory, exclusions, granularity rules (ADR threads, roadmap areas), phases P0-P5, 4 open decisions.

## [2026-11-22] schema | Ingest-plan decisions applied
- Thread pages for ADRs; P0-P1 supervised, P2-P3 batched; wiki public, committed to `main` per phase; loop merges ingest into the wiki.
- `CLAUDE.md`: added Steady-state ingest section. [[ingest-plan]]: decisions + Ingest backlog section.

## [2026-11-22] ingest | P0 skeleton (8 repo docs at origin/main 2e94e5ef)
- Sources: [[doc-readme]], [[doc-product]], [[doc-index]], [[doc-roadmap-overview]], [[doc-architecture]], [[doc-glossary]], [[doc-contributing]], [[doc-process-agent-rules]].
- Entities (24): [[crate-graph]] + 19 crate pages, [[fan-01]], [[studio]], [[roadmap]], [[simulation-milestones]].
- Concepts (20): [[adr]], [[f-item]], [[roadmap-status]], [[wave]], [[gate]], [[registered-limit]], [[known-miss]], [[unverified]], [[clean-room]], [[provenance]], [[bit-identical]], [[registration]], [[case-file]], [[benchmark-descriptor]], [[template]], [[report]], [[mrf]], [[run]], [[field-store]], [[mcp]].
- [[overview]] written.
- Contradictions flagged: sliding mesh available (README) vs not available (roadmap overview); structural solver "does not exist" (PRODUCT.md) vs partly built.
- Found: local `main` was 282 behind; `working-framework.md` moved to `docs/process/`; `docs/adr/index.md` is a generated ADR catalog. [[ingest-plan]] corrected.

## [2026-11-22] ingest | P1 job p1-numerics (snapshot 2e94e5ef)
- Ingested 12 numerics/physics docs (meshing, mesh quality, mesh import, dynamic mesh, structural solver, co-simulation, particles, acoustics, reactor+acoustics, GPU, component diagonal, BFS findings) as `doc-*` source pages at 2e94e5ef.
- New concept pages: mesh-quality, mesh-adaptation, dynamic-mesh, sliding-mesh, overset-mesh, fw-h, lagrangian-particles, component-diagonal, simple, piso.
- New entity pages: mesh-generators, fan-passage-mesh, aeroacoustics, structural-solver, cosimulation-protocol.
- Crate pages rcfd-mesh, rcfd-fem, rcfd-particles, rcfd-gpu, rcfd-reactor, rcfd-post, rcfd-fv, rcfd-io, rcfd-cli filled in from the docs (additive).
- Sliding-mesh contradiction: docs/dynamic-mesh.md sides with README (case tree since F-303 / F-711); callout on [[sliding-mesh]].
- New between-doc contradiction: rcfd-particles layer number (particles.md "layer 2" vs architecture.md layer 3).
- Recorded the open maintainer decision on the component-diagonal default (`averaged` unchanged; only `packed_bed` hash would move).

## [2026-11-22] ingest | P1 job p1-interfaces (snapshot 2e94e5ef)
- Ingested docs/case-schema.md as a hub plus 49 section source pages (keys, defaults, constraints summarised, not copied).
- Ingested 8 interface reference docs (run-control, studies, mcp-tools, service-protocol, interchange-formats, python, udf-abi, solid-case-file) and the 5 tutorials.
- New concepts: exit-codes, status-stream, checkpoint, stop-file, parametric-study, mcp-access-class, udf-sandbox, interchange-formats, setup-rules, located-error, divergence-guard, solid-case-file, executable-tutorials, project-journal.
- New entities: rcfd-service-protocol, rcfd-python.
- Added interface facts to case-file, run, report, template, field-store, mcp and the crate pages rcfd-cli, rcfd-mcp, rcfd-workspace, rcfd-runner, rcfd-api, rcfd-fem.
- Recorded 6 within-repo contradictions (see below).

## [2026-11-22] ingest | P1 job p1-evidence-fan (snapshot 2e94e5ef)
- Ingested docs/reference-data.md as a hub plus 147 section pages (one per `##` section), each with a summary and the section text verbatim (headings demoted, links rebased) so all values stay exact.
- Ingested 14 further evidence and fan docs: validation-data, verification-report, provenance, parity-matrix, test-process, releasing, CHANGELOG, milestone-fan, fan-project-breakdown, fan-measurement-protocol, design-loop, designs/exp-rot, designs/m-fan-b, milestones.json.
- New benchmark entities: lid-driven-cavity, differentially-heated-cavity, cylinder-re100, backward-facing-step, flat-plate, naca0012, dfg-cylinder-benchmark; plus validation-data-pack and computer-fan-target.
- New concepts: verification-vs-validation, gci-richardson, design-loop, design-ledger.
- Additive updates to fan-01, simulation-milestones, provenance, unverified, known-miss, gate, clean-room, benchmark-descriptor.
- Flagged 9 contradictions, all within the repo docs (none against existing wiki pages).

## [2026-11-22] ingest | P1 job p1-studio-process (snapshot 2e94e5ef)
- Ingested 18 Studio and process docs as source pages (P1): ui-restart-plan, studio-guardrails, studio-coverage, design-themes-proposal, DESIGN.md, 7 surface briefs, 3 plugin skills; lead-process, tolerances, framework-reconciliation, handover-2026-11-19, documentation-process-review, the 4 framework-2026-11-17 documents, briefs README, audit-report, 10 partials-audit groups, product-suites, SECURITY.md.
- New entities: studio-guardrails (G1-G22 table), lead, implementation-agent, maintainer, process-ledger, process-report, push-main, claude-plugin, studio-design-system.
- New concepts: tolerance-class, work-streams, maintainer-questions, brief, sources-read-addendum, view-and-report.
- Structural-solver question checked: neither ui-restart-plan nor studio-coverage settles it; the 2026-11-18 review (G12) calls "No structural solver" stale; callout on [[studio]].
- framework-2026-11-17 recorded as a proposal, with the reconciliation's adopted (AGENTS.md, roadmap validation in CI, F-863 measures, F-864 shadow pilot) and not adopted (task contract/closeout, transactional store, second publisher) parts.
- Every claim from docs/audit-report.md carries a `> [!caution]` unreviewed-model-output callout or an inline "unreviewed model output" tag on the pages that reuse it.
- Updated 13 existing pages additively (studio, registration, adr, gate, roadmap, roadmap-status, registered-limit, doc-process-agent-rules, rcfd-project, rcfd-runner, rcfd-tsstore, clean-room, mcp).

## [2026-11-22] ingest | P3 job p3-structural-a (snapshot 2e94e5ef)
- P3 structural (a): 133 ADRs (0354-0518) grouped into 11 thread pages `adr-thread-structural-*`.
- Created concepts cyclic-symmetry, campbell-diagram, mpc-transformation and mutation-testing.
- Wrote the Synthesis of roadmap-structural-solver. The solver library exists and is broad, and its checks are verification only. Many items are partial only by the mutation or amendment rule, with "The maintainer decides done".
- Settled the PRODUCT.md question: the solver library exists. No CLI, MCP or flow-case route reaches it (F-405 and F-404 todo, ADR 0438 Q3).
- New contradiction: the F-593 keys are approved in ADR 0505's answers (2026-11-18), while the roadmap gap and docs/structural-solver.md still say they wait for the maintainer.
- Updated structural-solver, rcfd-fem, solid-case-file and cosimulation-protocol additively.

## [2026-11-22] ingest | P3 job p3-structural-b (snapshot 2e94e5ef)
- Ingested the second 133 rows (132 unique ADRs, 0544 listed twice) of the Structural solver category; 5 new thread pages, 9 job-A thread pages extended with a "Second half" section.
- New concept pages: incompatible-modes-hex8, arc-length-continuation, hyperelastic-materials, beam-warping.
- Largest chain: 28 ADRs on the finite-strain incompatible Hex8 and rule L, ending with the warm-start local update recommended (ADR 0725) and its default switch open for the maintainer (ADR 0727, ADR 0729).
- Two default changes approved 2026-11-18 and made: Beam2 in the cyclic constraint (ADR 0744), hyperelastic structural elements (ADR 0746); lifting the remaining beam-sector opt-ins approved 2026-11-21, not made; stagnation default held (ADR 0619).
- Case-file keys approved 2026-11-18 under the additive-golden class rule but todo: F-616, F-621, F-625, F-842, J-based creep variant (ADR 0568).
- PRODUCT.md "no structural solver" confirmed stale from this half; the Studio has no stress/strain colouring (inventory and Studio sources checked).

## [2026-11-22] ingest | P3 job p3-moving-meshes (snapshot 2e94e5ef)
- Ingested the 77 ADRs of "Moving meshes, rotors and coupling" into 6 thread pages plus the FAN-01 rotating-solve stub thread.
- Settled the sliding-mesh question: ADR 0637 c1 met (case file runs from `rcfd run`), ADR 0646 done; README and docs/dynamic-mesh.md are right, docs/roadmap-overview.md's "not available" is stale (reviewed 2026-11-18, after the ADRs of 2026-11-16).
- New concept pages immersed-solid and strong-fsi-coupling.
- Recorded the pattern of maintainer answers of 2026-11-18: five default changes of this area held "until the fan path needs it" (F-721/F-724, F-364, F-365, F-775).
- Two new within-repo contradictions flagged (strong coupling in the tree; default wall pressure).
- Area page synthesis written.

## [2026-11-22] ingest | P3 job p3-quality-design (snapshot 2e94e5ef)
- ingest | P3 quality and design ADR threads (p3-quality-design): 9 thread pages over 74 ADR files (75 list rows; ADR 0751 names two items).
- Validation headlines: F-868 partial (0 to 16 deg met, 17 deg known miss, unsteady arm issued as ADR 0764); F-869 partial (order 1.387 outside [1.5, 2.5], ADR 0763 Q1 open); F-870 partial and stopped on the band (uncertainty untraced, F-877 needs a person).
- Gate thread: daily CI gate with attribution, four shards from 2026-11-21; edit-lock F-855 still partial until a hosted gate shows F2.
- Test-to-ADR index: 19 ADRs; coverage near complete, reader agreement the recurring miss; primary-ADR definition decided 2026-11-18.
- Design line: all library only until F-702; GP surrogate, fidelity driver, hypothesis record and C-grid constructions on the reduced-order fan only.
- New entity/concept pages: test-to-adr-index, edit-lock, gaussian-process-surrogate, fidelity-management, hypothesis-record, roadmap-proposal.
- Contradictions flagged: 2 (naca0012, test-to-adr-index).

## [2026-11-22] ingest | P3 job p3-physics (snapshot 2e94e5ef)
- P3 physics: 64 ADRs (numerics 24, turbulence 7, heat 2, compressible/acoustics 13, multiphase/particles 8, meshing 9, post 1) compiled into 13 new `adr-thread-*` pages; ADR 0234 added to `adr-thread-immersed-solid`.
- Seven roadmap area pages got their Synthesis and ADR-thread lists.
- New concept pages: `amg`, `k-omega-sst`, `k-epsilon`, `spalart-allmaras`.
- Fourteen existing concept/entity pages extended additively with citations to the thread pages.
- Two contradictions flagged (F-474 gap vs ADR 0743; F-221 roadmap status vs ADR 0226).

## [2026-11-22] ingest | P3 job p3-studio-mcp (snapshot 2e94e5ef)
- P3 job p3-studio-mcp: 24 ADRs of "Studio desktop app" and "Claude and the MCP server" grouped into 5 thread pages (workspace layer, Studio engine entry points, runner records and run sites, Studio Electron, MCP tools), with hubs 0173, 0350, 0800, 0801 read for framing.
- New concept [[run-site]] (ADR 0739, F-848).
- Synthesis written for the three Use-family area pages, including Command line, Python and API (no ADRs; from the generated table and P1 pages).
- 15 existing entity/concept pages extended additively with citations to the thread pages.
- Contradictions flagged: G11 vs ADR 0801 Addendum 23 (embedded Claude Code panel removed); docs/mcp-tools.md "adds 25 tools" vs its own table and ADR 0738; ADR 0801 engine follow-ups table rows still "proposed" while ADRs 0734/0736/0738 are accepted and built.
- Thread pages adr-thread-studio-electron, adr-thread-workspace-layer and adr-thread-mcp-tools are ready for job p3-records-b to add its no-item ADRs (0130-0350 range, 0135, 0308).

## [2026-11-22] ingest | P3 job p3-records-a (snapshot 2e94e5ef)
- P3 records-a: ADRs 0001 to 0129 (founding decisions and waves 13 to 47) grouped by subject into 10 new thread pages; every ADR of the list is on a thread.
- Filled the FAN-01 rotating-solve stub with 24 wave ADRs (MRF ADR 0008 to the wave-45 limiter, cyclic-seam and revolution-sector ADRs) and created the hub entity [[f-126]].
- Created [[adr-thread-founding-decisions]] with a table of the crates created by ADRs 0001 to 0129, and linked the crate pages to their creating ADRs.
- Extended 7 existing threads with their early ADRs (layering 0033/0090/0098, daily gate 0127/0129, design 0067/0118/0119/0121, structural rotordynamics 0120) and added predecessor links to 9 more.
- Created concept [[vof]]; added early-history bullets to [[k-omega-sst]], [[mrf]], [[fan-01]].
- Wrote "Early waves" Synthesis bullets on 12 roadmap area pages; replaced the placeholders of `roadmap-parallel-and-gpu`.

## [2026-11-22] ingest | P3 job p3-records-b (snapshot 2e94e5ef)
- Ingested 130 ADRs without an item in the file name (0130 to 0801) as 6 new thread pages and sections on 7 existing threads; every ADR of the list is now linked from an `adr-thread-*` page.
- Wave 48 (egui Studio) is recorded as history: 18 ADRs, every follow-up "not yet accepted by the maintainer", the crates deleted 2026-11-16.
- The NACA 0012 SA drag chain (48Y to 49H) explains today's defaults: effective wall shear in force and moment reports and the `freestream` far-field role for SA, SST and k-epsilon.
- The wave-48 adaptive PISO correctors and SV301 (ADRs 0154 to 0162) are filed as the prequel of the F-299 consistent-corrector thread.
- FAN-01 wave 49 (ADRs 0174 to 0181): the bladed stall is driven by the convection deferred correction at the blade-root leading edge and the hub trailing-edge pocket; no candidate fix converged it.
- The structural plan thread maps all 66 planned packages to their items, registrations and threads; status 13/5/0 (basic), 7/8/4 (intermediate), 9/17/3 (advanced) done/partial/todo.

## [2026-11-22] ingest | P4 job p4-leftovers (snapshot 2e94e5ef)
- Ingested the 7 `docs/investigations/*/README.md` (f868, job48t, job48y, job49a, job49d, job49e, job49h) as `doc-investigation-*` source pages, with each folder's run-output files listed but not ingested.
- The NACA 0012 chain is now traceable on [[naca0012]]: 48Y (SA drag 1.4-4.6x) -> 49A (two causes) -> 49D (both fixes, slope 0.986 x 2 pi) -> F-868 (bit-for-bit reproduction), with 49E (buffer-layer wall mode) and 49H (trailing-edge divergence) beside it.
- Job 48T recorded on [[cylinder-re100]] and [[piso]]: wall pressure spokes from PISO's lost coupling at large wall diffusion numbers.
- Ingested `example-projects/` (README, manifest, all notes, meshes READMEs) as [[doc-example-projects]] plus three entity pages; `example-projects-linux/` described there, not ingested separately.
- Linked the example projects from [[studio]], [[rcfd-project]], [[run]], [[fan-01]] and the benchmark entities.
- Two contradictions within the repo docs flagged on [[doc-example-projects]], one stale crate path flagged on [[doc-investigation-job48t-cylinder-spokes]].

## [2026-11-22] ingest | P2 roadmap areas (snapshot 2e94e5ef)
- [[doc-roadmap-json]] and 15 `entities/roadmap-*` pages generated by `tools/gen_roadmap_pages.py` (786 items; counts match the overview of 2026-11-18). Synthesis sections written by the P3 jobs.

## [2026-11-22] lint | P5 consolidation
- `tools/unwrap.py` on the whole vault (hard wraps removed; rule added to `CLAUDE.md` and the agent brief). `tools/build_index.py` rebuilt [[index]].
- Coverage: all 760 ADR files under `docs/adr/` are linked from an `adr-thread-*` page; 0 dead ADR links; 0 broken wikilinks.
- Contradiction callout added on [[rcfd-mcp]] (tool count 25/42 vs the 36-row table).
- [[overview]] rewritten; [[ingest-plan]] backlog lists the 11 commits on `origin/main` after the snapshot.

## [2026-11-22] ingest | P6 steps 1-2: foundations map and open-source survey
- P6 step 1 (map): drafted `analyses/foundations-map.md` from the "How rcfd uses it" sections of 32 method concept pages and 5 method-like entity pages; 186 rows in shared roots plus 11 branches, 153 new foundation nodes proposed.
- P6 step 2 (sources): `analyses/foundations-sources.md` lists 56 references; 7 US Government public-use (NTRS: Menter TM-103975, Farassat TM-2007-214853, NACA 1135, Hubbard RP-1258 vol. 1, van Leer ICASE 92-43, Meakin CR-200828), TMR via the CC0 TMBWG repository, 23 copyable now after verification.
- Nothing downloaded into `raw/`.
- The TMR's nasa.gov hosts did not resolve or failed TLS on 2026-11-22; the TMBWG GitHub Pages mirror (already cited by the repo) was used.
- Gaps: no verified copyable primary source for Rhie-Chow/SIMPLE/PISO, unstructured limiters, smoothed-aggregation AMG, MRF/mixing plane, sliding interfaces, immersed boundaries, GCL, VOF compression/CSF, FEM (only CC BY-NC-SA course material), GCI per Celik 2008, NSGA-II/kriging.
