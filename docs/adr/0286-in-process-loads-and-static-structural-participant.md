# ADR 0286: One-way coupling 2: fluid face forces in process, and a static solid as a co-simulation participant

Status: **PLANNED 2026-11-12** (layout by the basic-tier plan, [ADR 0270](0270-structural-solver-basic-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are
the acceptance draft that the registration turns into checks.

Work package SB-16 of the structural solver, basic tier. Size M.

## Goal

The in-process route: fluid face forces, as `cosim::Fluid::forces` returns them, mapped to the nodes of a solid surface set by a
given `cosim::InterfaceMap` and solved statically. On the same code a `StaticSolid` implements `cosim::Structure` (advance =
static solve; checkpoint and restore), which fixes the interface the intermediate tier's dynamic participant will use. It is
checked against a mock fluid only; coupling to the flow solver is intermediate.

## Dependencies

ADR 0275, ADR 0278; existing roadmap items: F-139.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. Fluid face forces on a wall are mapped to the nodes of a
solid surface set by a given InterfaceMap and solved statically: resultant force, moment and work equal the fluid side's within
1e-12; for a matching planar interface under uniform pressure the solid's displacement equals the uniaxial closed form within
1e-10, and for a 3:2 non-matching one within a limit registered from the estimate of the mapping error. A StaticSolid implements
cosim::Structure (advance is a static solve; restore returns the checkpoint bit for bit) and, with a test-only mock fluid whose
force falls linearly with the wall displacement (stiffness k_f), coupled_step converges to u = F0/(k_s + k_f) within 1e-8 with
the Aitken and with the least-squares update; a failing solve stops the step with the participant's name and both sides
restored. No flow solver is coupled.

## Reference to check against

Closed form: two springs in parallel, `u = F0/(k_s + k_f)` with `k_s = E A/L` of a bar (nu = 0). The protocol of
`docs/cosimulation-protocol.md` (ADR 0256) and the update of ADR 0261.

## Files

`crates/rcfd-post/src/solid_coupling.rs`, `crates/rcfd-post/tests/sb_solid_coupling.rs`.

## What could make it miss

A static structure against a real incompressible flow has no structural mass, so the added-mass ratio is unbounded: this
participant is for the mock and for quasi-static use, and the ADR must say so. Whether the participant belongs in `rcfd-post` or
in a later coupling crate is decision M5.
