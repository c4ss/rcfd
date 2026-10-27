# Roadmap overview

This page is the short, plain-language version of the plan. The full list of 175 items, with acceptance criteria, is in [the detailed roadmap](ROADMAP.md). That file is generated from `roadmap.json`, which is the source of truth for every status below.

It is for anyone who wants to know what rcfd can do today and what comes next. To learn how to use rcfd, start at [the documentation home](index.md) instead.

## What rcfd is

rcfd is a finite-volume CFD (computational fluid dynamics) solver written in Rust. It is built clean-room: every algorithm comes from published literature or first principles, with no vendor code, binaries or documentation text. Vendor documentation was consulted in a few recorded cases: mostly contents pages, to list capabilities, and some theory pages, as a check of model constants that are now marked UNVERIFIED. The [provenance record](provenance.md) lists each case. The long-term goal is an open-source toolset that covers what commercial simulation packages do, starting with CFD, so that small teams can build cheap products on top of it.

It is early-stage software. It has not been validated for engineering decisions, and nothing here is certified for safety-critical use.

## Where it stands

Of the 175 roadmap items, 95 are done, 56 are partly done and 24 have not started. Counting each partly done item as half gives about 70%. That figure measures how many planned items have landed, not how accurate the solver is. A partly done item usually works for some cases and has a stated gap.

| Area | Done | Partly done | Not started |
| --- | --- | --- | --- |
| Workspace and architecture | 3 | 3 | 0 |
| Mesh data structures and IO | 10 | 2 | 0 |
| Finite-volume core and linear solvers | 20 | 3 | 2 |
| Turbulence | 3 | 8 | 0 |
| Heat transfer and species | 13 | 10 | 0 |
| Compressible flow | 3 | 2 | 1 |
| Post-processing and VTK output | 11 | 4 | 0 |
| Scripting API, CLI and settings tree | 8 | 1 | 0 |
| AI assistant and desktop app integration | 16 | 5 | 1 |
| Advanced physics and scale (stretch) | 8 | 18 | 20 |

## What works today

- Incompressible and compressible flow on unstructured meshes, with a range of turbulence models, heat transfer, species transport and particles.
- Mesh import and export in common formats, VTK output, a command-line tool, a scripting API and an MCP server that lets an AI assistant drive the solver.
- A test suite of about 2,200 tests, run as a gate before each merge. Numeric limits are written down before a run, and a miss is recorded as a known miss, never tuned away.

## What does not work yet

- FAN-01, the reference fan case, stalls. The steady rotating-frame solve of the reference fan does not reach the expected operating point. Several suspected causes have each been ruled out as the single cause, and the diagnosis is open. See [the fan milestone](milestone-fan.md).
- There is no structural solver. Stress and strain pictures, and blade deformation, need one, and it has not been started.
- Sliding-mesh transient solves, needed for rotor-stator interaction and broadband noise, are not available.
- The desktop app is being rebuilt. The old interface is frozen. The new one is described below and does not have a window yet.

## What we are working on

### Computer fan

The first target is a 120 mm axial computer fan. The aim is a parametric fan that an AI assistant can modify, with predictions of airflow, noise and vibration, and of how they change with manufacturing error and dust build-up. The parts that do not need a converged fan solve are in place: geometry and tolerance tools, a fast reduced-order performance model, rotor-dynamics checks and a design ledger that keeps every variant. The plan and its limits are in [the fan project breakdown](fan-project-breakdown.md). Nothing predicts a real fan better than the measured data it is checked against, and we have none yet.

### Studio, the new desktop application

Studio will render with wgpu and treat a project as a folder holding many runs. You will be able to play a run back through time, run several in the background, and chat with Claude Code in a sidebar. The libraries underneath exist and are tested without a window: the project format, a background run service, a time-series store, a wgpu render core and the Claude Code bridge. The application shell and the Claude Code plugin come next. See [the Studio plan](ui-restart-plan.md).

### Faster tests that stay strict

A full test run takes over an hour. We are adding per-test time limits and a better runner, and checking for tests that can never fail. See [the test process](test-process.md).

## What comes next

1. Close the FAN-01 diagnosis, or choose a different reference case for the rotating machinery path.
2. Studio shell and plugin, then 3D rendering of fields and animation.
3. A structural solver, so stress and strain can be shown. This is waiting on a decision.
4. Noise and vibration predictions from a converged fan solution.
5. Validation of the fan models against measured data from a real small fan.

## How you can help

- Measure a small fan. The most useful contribution is data from one real computer fan: its fan curve at several impedances, its sound level and its geometry. [The measurement protocol](fan-measurement-protocol.md) says how.
- Pick up an item that has not started. The detailed roadmap lists them with acceptance criteria. Read the contribution guide in the repository root first, because the clean-room rule is strict.
- Review a result. Each batch of work ends with measured values set against limits written down beforehand. A second look at a limit or a known miss helps.

## How to read the status marks

| Mark | Meaning |
| --- | --- |
| Done | Every acceptance criterion in the item is met and tested. |
| Partly done | Some of it works. The item's note says what is missing. |
| Not started | Nothing usable exists. |

## Where to go next

- [The documentation home](index.md) maps every page by what you want to do.
- [The quick-start tutorial](tutorials/quick-start-cli.md) runs a first case from the command line.
- [The fan milestone](milestone-fan.md) gives the detail behind the reference fan status above.
