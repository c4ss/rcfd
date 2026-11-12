# ADR 0304: Two-way coupling of an elastic solid with the incompressible flow solver: the piston on an elastic bar

Status: **PLANNED 2026-11-12** (layout by the intermediate-tier plan, [ADR
0290](0290-structural-solver-intermediate-tier-plan.md)); the registration is written by the implementation agent before any code.
Nothing here is a registered limit: the numbers below are the acceptance draft that the registration turns into checks.

Work package SI-14 of the structural solver, intermediate tier. Size M.

## Goal

The first run in which the flow solver and the structural solver exchange force and displacement to convergence within each time
step: `cosim::FlowWall` (the moving-mesh PIMPLE step with the wall pressure of ADR 0252) coupled by `coupled_step` to
`DynamicSolid`. The case has a closed form: a channel of incompressible fluid, open at the far end, closed at the near end by the
end face of an elastic bar, so that the fluid is an added mass `rho_f A L_f` on the bar (the spring-mounted piston of F-244 and
F-251 with the spring replaced by a solid). Adds the helper that turns interface displacements into mesh point positions for this
geometry.

## Dependencies

ADR 0303; roadmap items (SI ids are this tier's packages, ADR 0290 section 7): SI-13, F-251, F-351, F-361, F-362.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. Elastic bar (nu = 0) closing a channel of incompressible
fluid of length L_f, started from a displaced state, mass ratios rho_f L_f/(rho_s L_s) of 0.5 and 4: the coupled iteration
converges in every step to 1e-6 with Aitken's factor and with the least-squares update, iteration counts recorded; the frequency
of the first coupled mode equals the root of k L tan(k L) = rho_s L_s/(rho_f L_f) within 2 percent after the registered estimate
of the time-step and viscous effects; over ten periods the work of the fluid on the interface equals the change of the solid's
energy within 1e-4 of its amplitude; the loose coupling (one pass) at ratio 4 grows (registered control). With a 3:2 non-matching
interface mapped by F-362 the frequency changes by less than 2e-3. A failure of either participant leaves both at the start of the
step. The existing co-simulation tests (cosim_f139, cosim_f361, cosim_f362) pass unedited.

## Reference to check against

Closed form: the bar with an end mass (as ADR 0303) with m_a = rho_f A L_f, the added mass of a column of incompressible fluid
moved as a plug (momentum equation of the column; this is the model M6 of ADR 0211 and ADR 0250, whose measured coupling lag is
the basis of the time-step estimate). No published coupled benchmark is used: the flexible flag behind a cylinder (Turek and Hron,
2006) is public but needs large mesh motion and a nonlinear structure and its values are not in the repository (advanced tier).

## Files

`crates/rcfd-post/src/solid_fsi.rs`, `crates/rcfd-post/tests/si_fsi_piston.rs`; `docs/cosimulation-protocol.md` (the limits
section: a structural solver has been connected).

## What could make it miss

The riskiest package of the tier. The flow side is implicit Euler: its damping of the oscillation and the lag of the pairing may
exceed the 2 percent (the F-244 piston measured 0.13 percent in frequency and 4.7 percent in amplitude per turning point at
T/100). The run time of a moving-mesh PIMPLE case with several coupling iterations per step decides the mesh; the check is a heavy
record with a short fast variant. A wall that bends (non-uniform motion) is recorded, without a limit.
