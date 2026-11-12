# ADR 0324: Aerodynamic damping and added mass of a mode by prescribed motion of the wall (energy method)

Status: **PLANNED 2026-11-12** (layout by the advanced-tier plan, [ADR 0310](0310-structural-solver-advanced-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are the
acceptance draft that the registration turns into checks.

Work package SA-14 of the structural solver, advanced tier. Size M.

## Goal

A flutter margin is the sign and size of the work the fluid does on a vibrating mode. Add the machinery: move the fluid wall in a
structural mode shape at a given amplitude and frequency (through the interface weights of F-362 and the mesh-motion helper of ADR
0304), run the flow solver over registered cycles, and from the interface force history extract the in-phase part (added mass or
stiffness) and the quadrature part (work per cycle, aerodynamic damping ratio of the mode). Checked where the fluid force has a
closed form; the fan passage is not part of the acceptance (ADR 0310, not planned).

## Dependencies

ADR 0311; roadmap items (SA ids are this tier's packages, ADR 0310 section 7): SA-1, F-362, F-383, F-400, F-401.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. Force harmonics: for a synthetic force history with known
in-phase and quadrature parts the extraction returns them within 1e-10, and the work per cycle equals pi q0 times the quadrature
part within 1e-10. Piston on a column of incompressible fluid with an open end (the case of F-401): added mass rho_f A L_f within
2 percent and a damping ratio below the registered viscous and time-step estimate. Piston driving a plane channel of gap h and
length L_f: force per piston velocity equal to A times the oscillating-channel impedance i w rho L_f/(1 - tanh(kappa)/kappa),
kappa = (h/2) sqrt(i w/nu), real and imaginary parts within 3 percent at Womersley numbers 1 and 5 after the registered
extrapolation in the time step, and the low-frequency limit 12 mu L_f/h^2 within 3 percent. The damping ratio of the bar on the
open column from this method and from the decay of the coupled run of F-401 are both recorded. A history that has not become
periodic within the registered tolerance is refused with the cycle-to-cycle change.

## Reference to check against

Closed forms: plug flow of an incompressible column; laminar oscillating flow between plates (the plane-channel form of
Womersley's solution; one ordinary differential equation, derived in the registration). The energy method is the work integral of
the interface force over a cycle (Carta, J. Eng. Power 89 (1967) 419, from memory; first principles).

## Files

`crates/rcfd-post/src/solid_aerodamp.rs`, `crates/rcfd-post/tests/sa_aerodamp.rs`.

## What could make it miss

The flow side is implicit Euler: its phase error turns about w dt/2 of the inertial force into false damping, which at 200 steps
per period is of the size of the physical damping at the higher Womersley number; the registration must fix the step sizes and the
extrapolation from that estimate before any run. Long runs (heavy record with a short fast variant). A wall moving in its own
plane is not covered by the protocol's tests.
