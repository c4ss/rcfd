# ADR 0319: Flexible rotor on bearings in the stationary frame: whirl, critical speeds, unbalance response, joined to the rigid-rotor model

Status: **PLANNED 2026-11-12** (layout by the advanced-tier plan, [ADR 0310](0310-structural-solver-advanced-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are the
acceptance draft that the registration turns into checks.

Work package SA-9 of the structural solver, advanced tier. Size M.

## Goal

Fan breakdown item E2 has a rigid rotor on two bearings (`rcfd-post::rotordyn`, ADR 0120). Add the flexible one: a shaft of beam
elements with rotary inertia and the gyroscopic matrix of a spinning beam, rigid discs (mass, polar and transverse inertia; from a
solid model's mass matrix for the hub with blades), bearings with stiffness and damping matrices including cross-coupling, and
from them the whirl frequencies over speed with forward and backward labels (Campbell table in the stationary frame), critical
speeds, stability, and the response to unbalance. `rcfd-post::solid_rotor` converts to and from the rigid model's data.

## Dependencies

ADR 0317; roadmap items (SA ids are this tier's packages, ADR 0310 section 7): SA-7, F-391, F-393.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. At zero speed the frequencies equal those of the beam model
of F-391 within 1e-10. Disc on a light shaft between two bearings: critical speed sqrt(k/m) and unbalance amplitude e r^2/sqrt((1
- r^2)^2 + (2 zeta r)^2) within 1e-3 at r = 0.5, 1 and 2. In the stiff-shaft limit the critical speeds and the response equal
rotordyn::critical_speeds and rotordyn::response for the same rotor within 1e-6. Uniform shaft on pins with rotary inertia, 20
elements: forward and backward whirl of modes 1 and 2 equal the roots of E I k^4 - rho A w^2 - rho I k^2 w^2 +/- 2 rho I k^2 Omega
w = 0, k = n pi/L, within 0.5 percent at three speeds. Overhung disc on a massless cantilever shaft: the four whirl speeds equal
the roots of the two-unknown closed form (derived in the registration) within 0.5 percent. Anisotropic bearings split the critical
speed into sqrt(kx/m) and sqrt(ky/m) within 1e-3. A cross-coupled stiffness k_c on a damped disc is stable below c w_n and
unstable above, the threshold within 1 percent. Forward and backward are labelled by the sense of the orbit; an unstable speed is
reported, never dropped.

## Reference to check against

Closed forms: the Jeffcott rotor with and without cross-coupling; the spinning beam with rotary inertia (from its kinetic energy);
the overhung disc (Stodola; two-unknown flexibility model). The repository's rigid model (ADR 0120) is the reference of the
stiff-shaft limit. Element matrices: Nelson and McVaugh, J. Eng. Ind. 98 (1976) 593 (from memory; re-derived).

## Files

`crates/rcfd-solid/src/rotor.rs`, `crates/rcfd-post/src/solid_rotor.rs`, `crates/rcfd-solid/tests/sa_rotor.rs`,
`crates/rcfd-post/tests/sa_solid_rotor.rs`.

## What could make it miss

Only the head of `rotordyn.rs` was read (rigid rotor, isotropic bearings, complex coordinates); if its types do not map onto
general bearings the join covers the isotropic case only. Bearing stiffness and damping of a sleeve or ball bearing of a small fan
are inputs: none is in the repository. Blade flexibility is not coupled to the shaft in this package (discs are rigid).
