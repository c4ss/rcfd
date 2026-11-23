---
type: source
tags: [adr-thread, structural, fem, case-file, fan, demonstration]
sources: [doc-structural-solver, doc-solid-case-file]
origin: docs/adr/ (ADR list below)
origin_commit: 09647baa
created: 2026-11-22
updated: 2026-11-23
status: draft
---

# ADR thread: structural solver, the solid case file and the fan-blade demonstration

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 09647baa (first written at 2e94e5ef; ADRs 0780 and 0785 added at 09647baa, the earlier ADRs unchanged since).
**Kind:** ADR thread (registrations with result addenda and a maintainer-answers section), 6 ADRs: 4 of 2026-11-14 and 2 of 2026-11-23.

How a user reaches [[rcfd-fem]] without writing Rust: the [[solid-case-file]] run by a library function (F-386), its iterative solve to tolerance (F-526), the fan-blade demonstration on one blade of `ComputerFan::default()` (F-387) and the library blade mesher with its proposed case-file keys (F-593).

## Current state

- **Reduced parts and superelements from a case file** (F-616 done, ADR 0785, merged e3d7fd0b on 2026-11-23): "a solid case file attaches reduced part files to node sets of its model by `[superelements.<name>]` (file, set; parts in name order, each checked to lie exactly on its set) and reduces its model to a part file by `[reduction]` (interface, modes, interface_enrichment, file), run by `rcfd_api::solid_case::run_solid_case_any`". The solid bar of ADR 0536 from case files "equals the library in bits in the static solve and the modal analysis for both arrangements (248 and 221 unknowns), for two parts on one host (253) and with one block of enrichment (27 vectors, 275), and the part files equal the library's byte for byte"; 18 refusals located by key and line; "a case without the tables is unchanged (16 digests of six case files, baseline_hashes, the case hash tables)". "No golden holds the solid schema, whose own diff is 66 added lines and none removed." Not covered: the harmonic solve, beams and shells, the cutoff and residual vectors of the reduction, stresses of a combined run ([[doc-solid-case-file]], [[substructuring-and-craig-bampton]]).
- **Maintainer's answer of 2026-11-23 to ADR 0785** ("Yes to both F-616 questions"): (1) option (a), "the bar of ADR 0536 from a case file is its static solve and modal analysis; the harmonic solve of a combined model from a case file belongs to the item that gives the solid case file a harmonic analysis. F-616 stays **done**." (2) The follow-up, keys of `[reduction]` for the cutoff, the load's residual vector and the load patterns, is entered as **F-889** (todo, not issued).
- **F-842 registered and stopped before code** (ADR 0780, merged c557e935 on 2026-11-23, status todo unchanged): the acceptance (the inertia and the retry flag of every accepted increment in the solid case's output, with the opt-in for `NewtonOptions::indefinite_retry`) "presupposes a nonlinear static analysis in the solid case file (the file has static and modal only, reports no increments and has no key for the hyperelastic material or the incompatible-mode formulation of the 16^3 block) and a CLI subcommand for solid cases (F-405, todo)". The library already returns the inertia (`LoadStep::inertia`, `LoadStep::retried`). The lead: "The brief was issued against an acceptance that presupposes what does not exist; the agent was right to stop before code." The output design D1 to D4 (the opt-in key `indefinite_retry`, the report keys `inertia` and `retried`, the warning's text, the CLI summary) and the checks "stand for the next issue". See [[adr-thread-structural-incompatible-hex8-rule-l]] for ADR 0694's block and [[nonlinear-fem-newton]].

- **The solid case file exists** (F-386, ADR 0438): `rcfd_api::solid_case`, a TOML file with its own schema (`[mesh]` box or Gmsh, `[node_sets]`, `[materials]`, `[constraints]`, `[loads]` traction / pressure / nodal_force / gravity, `[analysis]` static or modal, `[solver]` direct or cg with block_jacobi, ic0, ic_fill, amg, amg_quadratic, amg_quadratic_smoothed, `[output]`); `run_solid_case_file` writes the `.vtu` and a JSON report `rcfd-solid-report/1`. **Partial**: A14 (iterative route to 1e-10 on the Tet10 file cantilever) missed for all six preconditioners, a residual floor. F-526 (ADR 0443) met the 1e-10 on the F-381 Hex8 file and printed both floors (ADR partial by an amendment after a run; item done).
- **No CLI subcommand and no MCP tool:** ADR 0438 leaves them as "the maintainer's step: each changes a golden" (ADR 0438 Q3, recommendation: a separate item once the maintainer approves the golden change). That item is F-405, todo ([[roadmap-structural-solver]]).
- **Fan-blade demonstration** (F-387, ADR 0500, done): static response to centrifugal and pressure load and the modes at rest, with a user page; numbers on [[structural-solver]] ([[doc-structural-solver]]).
- **Library blade mesher** (F-593, ADR 0505, partial): `rcfd_fem::builders` `fan_blade_tet4`, `fan_blade_tet10`, `computer_fan_blade_tet10` with the node sets `all`, `hub`, `tip` and surface sets `hub`, `tip`, `pressure`, `suction`; at the F-387 divisions the meshes and results equal the test's construction bit for bit. The case-file half (`[mesh] generator = "fan_blade"`, `[loads.<name>] kind = "centrifugal"`) is not built.
- **Maintainer answer of 2026-11-18** (written into ADR 0505 on 2026-11-19): the case-file keys are "Approved under the class rule for additive golden edits (agent rules, rule 3, from version 2)"; "The item may be issued. The same keys are proposed in ADR 0500 (open question 1)".

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0438](../../docs/adr/0438-f386-registration-solid-case-file.md) | 2026-11-14 | registration (hub) | F-386 | A solid case file of its own and the library entry that runs it | partial; A14 missed (residual floor 5.7e-10 to 6.2e-10 against 1e-10) | 0443 |
| [0443](../../docs/adr/0443-f526-registration-iterative-solid-case-and-box-bound.md) | 2026-11-14 | registration | F-526 | Iterative solve of a solid case file to its tolerance, and the box bound | ADR partial (amendment after a run); item done | |
| [0500](../../docs/adr/0500-f387-registration-fan-blade-static-and-modal-demonstration.md) | 2026-11-14 | registration (demonstration) | F-387 | Fan blade: static response to centrifugal and pressure load, modes at rest; user page | done (S0 after amendment 1) | 0505 |
| [0505](../../docs/adr/0505-f593-registration-library-fan-blade-mesher-in-rcfd-fem-builders.md) | 2026-11-14 | registration + maintainer answers | F-593 | Library fan-blade mesher; case-file keys proposed | partial (library half done); keys approved 2026-11-18 | |
| [0780](../../docs/adr/0780-f842-registration-inertia-and-retry-in-the-solid-case-output.md) | 2026-11-23 | registration, stopped before code | F-842 | The inertia and the retry of the nonlinear driver in the solid case's output | todo (a third outcome of its decision rule, accepted by the lead); no check run; Q1 open | |
| [0785](../../docs/adr/0785-f616-registration-case-file-keys-for-superelements-and-interface-enrichment.md) | 2026-11-23 | registration + amendments 1 and 2 (both before any library line) | F-616 | Case-file keys for superelements (ADR 0469, ADR 0536) and interface enrichment (ADR 0532) | done; B1 to B4 and R3 equal the library in bits; 8 of 8 mutations caught; Q1 answered (a), F-889 entered | |

## Key numbers and limits

- F-386 gap: "On the Tet10 file cantilever every CG preconditioner stops at a true residual of 5.7e-10 to 6.2e-10 of |b| against the registered 1e-10 (a rounding floor)" ([[roadmap-structural-solver]]).
- F-526 (ADR 0443): "six preconditioners reach 1e-10 on the F-381 Hex8 file with agreement within 1.2e-12".
- F-387 (from the user page): first frequency 188.8 -> 186.1 Hz coarse to fine, reactions balance to about 1e-11; E = 2.5 GPa, nu = 0.38, 1300 kg/m^3, 2000 rpm and 50 Pa ([[doc-structural-solver]]).
- Proposed `fan_blade` keys (ADR 0505): `divisions` default `[12, 2, 8]`, `n_c >= 2`; `centrifugal` with `axis`, `origin` (default `[0, 0, 0]`), `speed` in rad/s, "one unit only, no `rpm` alias".

- **ADR 0785, the checks:** part files equal the library part's text byte for byte (314 745 bytes with one block of enrichment and 27 residual vectors; 143 980 bytes without the key and with 0). Host runs: 248 coordinates and 243 host displacements (arrangement I), 221 and 243 (II), 253 and 243 with two parts attached in the order `a_root`, `b_tip`, 0 differ; modal: 8 eigenvalues and 1984 (I) or 1768 (II) vector entries, 0 differ; with the enriched part 275 coordinates, 8 eigenvalues, 2200 entries, 0 differ. B5 (against the full model) 6.353e-13 over 81 nodes, equal to ST1 of ADR 0536. The JSON schema digest of the flow case is unchanged (`0xc611735ab88857f7`, 7712 bytes). Held by `crates/rcfd-api/tests/f616_superelement_case.rs`.
- **ADR 0785, amendments:** (1) `[reduction]` is an optional object of the schema and stays out of the settings tree of a case that does not give it; the collection `superelements` "cannot be optional (the option is for objects) and is in the tree as an empty table". (2) `cantilever_tet10_amg.toml` is refused by `run_solid_case_file` on main (CG with `amg_quadratic` does not converge to 1e-10, the existing known miss of ADR 0438's A14), so for a refused file the digest check records its problems as text.
- **ADR 0780, findings of the reading:** the analysis type of the solid case file is `static` or `modal` (`crates/rcfd-api/src/solid_case.rs`), "and no crate of the API, the CLI or the MCP server names `NewtonOptions`" (the lead's check on main). The registered warning text: "increment <k> (load factor <lambda>): the accepted state has inertia nu = <nu> > 0, the equilibrium is not shown to be stable (ADR 0719 D2)"; the sign convention is Sylvester's law of inertia as used in ADR 0719 D2 (the negative pivots of the $L D L^T$ of the reduced tangent count its negative eigenvalues).

## Open questions

- **ADR 0780 Q1 (open at 09647baa): where are the nonlinear static analysis of the solid case file and the CLI subcommand built?** Options: (a) F-842 waits and a new item adds the nonlinear static analysis to the solid case file; (b) F-842 is widened to build both (it "overlaps F-621 and F-405" and adds lines to the CLI help golden); (c) as (a), and the CLI summary is taken out of F-842's acceptance and given to F-405. Recommendation (c), which the lead shares.
- F-889 (keys of `[reduction]` for the cutoff and the residual vector of ADR 0474 and the load patterns of ADR 0511) is entered and not issued at 09647baa.
- ADR 0438 Q3 / F-405: a CLI subcommand and MCP tool for the solid file (golden edits), not yet issued at this snapshot.
- ADR 0505 Q2: mid-edge nodes on the curved blade surface, recommendation (a) keep straight-edge midpoints for now.

> [!warning] Contradiction — are the F-593 case-file keys still waiting for the maintainer?
> The F-593 gap in `docs/roadmap.json` ([[roadmap-structural-solver]]) says the case-file half "needs the user to approve the case-file keys proposed in ADR 0505", and [[doc-structural-solver]] says the keys "wait for the maintainer". ADR 0505's answers section (maintainer, 2026-11-18, written in 2026-11-19) approves them and says "The item may be issued". Within the repo docs; the ADR is newer.

## Related

- [[solid-case-file]], [[structural-solver]], [[rcfd-fem]], [[rcfd-api]], [[roadmap-structural-solver]], [[substructuring-and-craig-bampton]], [[nonlinear-fem-newton]]
- [[adr-thread-structural-dynamics-harmonic]] (ADRs 0469, 0532 and 0536, the library side of the reduction), [[adr-thread-structural-incompatible-hex8-rule-l]] (ADR 0694)
- [[fan-01]], [[adr-thread-structural-linear-solvers]]
