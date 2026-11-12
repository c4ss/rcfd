# ADR 0303: Dynamic solid as a co-simulation participant, on interface weights built from the two surfaces

Status: **PLANNED 2026-11-12** (layout by the intermediate-tier plan, [ADR
0290](0290-structural-solver-intermediate-tier-plan.md)); the registration is written by the implementation agent before any code.
Nothing here is a registered limit: the numbers below are the acceptance draft that the registration turns into checks.

Work package SI-13 of the structural solver, intermediate tier. Size M.

## Goal

`DynamicSolid` implements `cosim::Structure` with the time integration of ADR 0301: `advance` integrates over the coupling step
with the nodal forces held, in one or more sub-steps; checkpoint and restore are bit for bit. The interface weights are no longer
given: they are built by the geometric mapping of F-362 from the nodes and faces of a solid surface set and the fluid wall.
Checked against mock fluids only (an added mass, a spring); the flow solver is ADR 0304.

## Dependencies

ADR 0301; roadmap items (SI ids are this tier's packages, ADR 0290 section 7): SI-11, F-139, F-361, F-362, F-385.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. A bar (nu = 0) whose end face is the interface, coupled by
coupled_step to a mock fluid that returns the force -m_a times the wall acceleration: the first coupled frequency equals the root
of k L tan(k L) = rho A L/m_a within 1 percent at 100 steps per period, for mass ratios m_a/(rho A L) of 0.5 and 4; at ratio 4 the
plain iteration (factor 1) diverges as the protocol predicts (registered control) while Aitken's factor and the least-squares
update converge to 1e-8 within registered iteration caps. With a mock spring k_f and a slow load the response tends to F_0/(k_s +
k_f) of F-385 within 1e-6. Mapping built by F-362 for a matching and for a 3:2 non-matching interface: resultant force, moment and
work conserved within 1e-12, and the coupled frequency changes by less than 1e-3 between the two. Restore returns the checkpoint
bit for bit; a failing structural step stops the coupled step with the participant's name and both sides restored. No flow solver
is coupled.

## Reference to check against

Closed form: the longitudinal vibration of a bar with a mass on its end, k L tan(k L) = rho A L/m_a (derivable from the wave
equation with the boundary condition E A u_x = -m_a u_tt). The stability bound of the plain iteration is the one of
`docs/cosimulation-protocol.md` (ADR 0256), the updates those of ADR 0261, the weights those of ADR 0263.

## Files

`crates/rcfd-post/src/solid_dynamic.rs`, `crates/rcfd-post/tests/si_solid_dynamic.rs`.

## What could make it miss

F-362 is registered and not yet run: if its builder misses, this package falls back to given weights and the mapping clause is not
run (partial). The mock's acceleration is a difference of displacements over the coupling step, which adds a lag the registration
must estimate before fixing the 1 percent. Shell interfaces (mid-surface, both sides wetted) are not covered.
