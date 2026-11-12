# ADR 0299: Modes of a rotating part: stress stiffening by the centrifugal pre-stress, spin softening, Campbell diagram

Status: **PLANNED 2026-11-12** (layout by the intermediate-tier plan, [ADR
0290](0290-structural-solver-intermediate-tier-plan.md)); the registration is written by the implementation agent before any code.
Nothing here is a registered limit: the numbers below are the acceptance draft that the registration turns into checks.

Work package SI-9 of the structural solver, intermediate tier. Size M.

## Goal

The modal analysis of a part rotating at constant speed, in the rotating frame: `K_eff = K + Omega^2 (K_sigma,1 - M_perp)`, with
`K_sigma,1` the geometric stiffness of the centrifugal stress at unit speed (ADR 0298, one static solve) and `M_perp` the mass
matrix of the motion perpendicular to the axis (spin softening). Frequencies over a list of speeds with the modes tracked by
shape, the crossings with the engine-order lines `f = k Omega/(2 pi)`, and the table of a Campbell diagram as data (CSV and JSON).
The Coriolis coupling is neglected and the ADR says so (decision N5).

## Dependencies

ADR 0298; roadmap items (SI ids are this tier's packages, ADR 0290 section 7): SI-8, F-375, F-383.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. At zero speed the result has the bits of the modal analysis
of F-383. Radial bar clamped on the axis, nu = 0, axial modes: (omega_n,0^2 - omega_n^2)/Omega^2 = 1 within 1e-3 for n = 1, 2 at a
speed with centrifugal stress below 1e-4 E. Radial beam of square section clamped on the axis (slenderness 20): the first
out-of-plane (flap) and in-plane (lag) frequencies satisfy omega_flap^2 - omega_lag^2 = Omega^2 within 1e-4 Omega^2 on a symmetric
mesh; the stiffening (omega_flap^2 - omega_0^2)/Omega^2 equals at Omega = 0.2 omega_0 the Rayleigh quotient of the centrifugal
tension with the mode at rest (1.19, recomputed in the registration) within 2 percent and stays between 1 and that value up to
Omega = 5 omega_0. Campbell table: on a synthetic family omega^2 = omega_0^2 + S Omega^2 the crossings with orders 1 to 4 equal
omega_0/sqrt(k^2 - S) within 1e-8, and on the beam within 1 percent; a flap and a lag mode of a rectangular section that cross are
tracked without exchange (modal assurance above 0.9). A speed at which K_eff is not positive definite is reported as a loss of
stiffness with the speed, never as a non-finite frequency.

## Reference to check against

Closed forms and bounds, all derivable: the radial bar in the rotating frame, rho u_tt = E u_rr + rho Omega^2 u; the flap-lag
identity of a symmetric section; the first-order perturbation of an eigenvalue (the slope of omega^2 against Omega^2 at rest is
the Rayleigh quotient of the added stiffness); Southwell's lower bound omega^2 >= omega_0^2 + omega_string^2 with the flapping
frequency Omega of a rotating string clamped on the axis (Legendre's equation). Southwell and Gough, British ARC R&M 766 (1921),
from memory.

## Files

`crates/rcfd-fem/src/rotating.rs`, `crates/rcfd-fem/tests/si_rotating.rs`.

## What could make it miss

Without the Coriolis term the frequencies of modes that move in the plane of rotation are approximate; for a blade on a stiff hub
the error is small, for a disc it is not, and the package cannot measure it. The pre-stress is linear in Omega^2 (no large
untwist). The 1e-4 of the flap-lag identity assumes the two bending planes are discretised identically.
