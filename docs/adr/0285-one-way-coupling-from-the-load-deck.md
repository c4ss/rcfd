# ADR 0285: One-way coupling 1: loads of the F-237 deck applied to the solid surface

Status: **PLANNED 2026-11-12** (layout by the basic-tier plan, [ADR 0270](0270-structural-solver-basic-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are
the acceptance draft that the registration turns into checks.

Work package SB-15 of the structural solver, basic tier. Size M.

## Goal

Close the file route of fan breakdown item E4: build the structural shell surface that F-237 exports onto from a surface set of
the solid mesh (same nodes, normals toward the fluid), read the deck back (`PLOAD4`, `TEMP`), and apply its pressures as surface
loads and its temperatures as the thermal field of ADR 0281. The mapping from the fluid wall stays F-237's (plane and tube
parametrisations); a general geometric mapping is F-362.

## Dependencies

ADR 0272, ADR 0278, ADR 0281; existing roadmap items: F-237.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. A surface set of a solid mesh is turned into the shell
surface that the F-237 export writes onto, and a deck read back (PLOAD4, TEMP) is applied to the solid: the resultant force and
moment of the nodal loads equal deck_force_moment within 1e-12 relative; a deck element or node that matches no solid face or
node within a stated tolerance is refused with its id. Tube: the wall pressure of the fluid mesh of the F-237 tube check,
uniform, exported, written, parsed and applied to a thick solid tube (b/a = 2, plane strain) gives the radial displacement at
the bore within 1 percent and the hoop stress within 2 percent of Lame's solution; with a pressure varying along the axis and
around the circumference the solid's reactions equal the fluid's integrated force within 0.1 percent (the F-237 limit); TEMP of
a uniform temperature gives the free expansion alpha dT r within 1e-6.

## Reference to check against

Lame's cylinder (as ADR 0280) and the fluid-side integrals of F-237 (`fluid_force_moment`, `deck_force_moment`, ADR 0051 and ADR
0244).

## Files

`crates/rcfd-post/src/solid_loads.rs`, `crates/rcfd-post/tests/sb_deck_loads.rs`.

## What could make it miss

The sign convention of `PLOAD4` in the deck is recorded as unverified against any solver (ADR 0051 D8): this ADR fixes it only
within the repository. The heat-flux card `RCFDQ` has no use until a thermal solver of the solid exists (not in this tier).
Shell faces of a Tet10 surface (six nodes) do not exist in the deck's element set.
