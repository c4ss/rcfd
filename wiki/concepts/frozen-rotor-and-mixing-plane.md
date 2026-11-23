---
type: concept
tags: [foundation, rotating, turbomachinery, mixing-plane]
sources: [ref-liu-hasegawa-2026-vpm-mrf-sliding, ref-chima-1998-nasa-tm-206613]
builds_on: [relative-and-absolute-velocity-formulations]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Frozen rotor and mixing plane (steady rotor-stator interfaces)

## In short

A steady simulation of a rotor next to a stator has to decide how the two talk to each other. The frozen rotor keeps the rotor at one fixed position and passes the flow across the interface as it is, so the answer depends on that position. The mixing plane averages the flow around the circumference at the interface, so each row sees only the other's average.

## Explanation

- **The underlying problem.** Rotor and stator move relative to each other, so their interaction is unsteady. Methods for multistage machines are (1) isolated blade rows analysed one after another, (2) averaging-plane methods, (3) the average-passage method and (4) full unsteady simulation, which is expensive and often changes blade counts to small integer ratios ([[ref-chima-1998-nasa-tm-206613]], introduction).
- **Frozen rotor (MRF).** The domain is split into rotating and stationary regions; the rotor is fixed inside the rotating region and only the inertia terms of the rotating frame enter its momentum equation, so no mesh moves. It neglects the unsteady interaction between the rotor and stationary boundaries and "often fails to capture detailed transient characteristics" ([[ref-liu-hasegawa-2026-vpm-mrf-sliding]], sec. 1). Comparing with a sliding mesh, MRF overpredicted the pressure drop of a rotating cuboid at low rotation speeds (its inertia sources "assume a fully developed rotating flow") and underpredicted it at higher speeds because "the transient interaction between rotating and stationary parts is not considered" (same paper, sec. 3.4).
- **Mixing plane.** At a common interface the flow next to the boundary is integrated around the circumference at each spanwise station and the average is passed to the neighbouring row; "since averaging-plane methods often use mixed-out averages, they are commonly referred to as mixing-plane methods" (Denton, Dawes) ([[ref-chima-1998-nasa-tm-206613]], introduction and interface boundary condition).
- **What to average.** Any two thermodynamic and three kinematic properties define an average state, and information is lost, so one chooses what to conserve. The mixed-out average conserves mass, momentum and energy and represents the fully mixed flow far downstream (it can introduce mixing losses early); the kinetic-energy average conserves mass and total enthalpy and represents the local state ([[ref-chima-1998-nasa-tm-206613]], averaging techniques). With the mixed-out average the mass-flow error was under 0.1 percent, with the kinetic-energy average under 1 percent with jumps at each plane (results).
- **Boundary reflections.** Forcing the flow to be axisymmetric at the plane is robust but reflects outgoing waves; characteristic (non-reflecting) conditions in perturbations about the neighbour's average avoid that ([[ref-chima-1998-nasa-tm-206613]], introduction). The interface "does not guarantee conservation between blade rows" exactly (interface boundary condition).

## Builds on

- [[relative-and-absolute-velocity-formulations]]: the rotating zone is solved in its own frame; quantities cross the interface in a consistent (absolute) frame.
- [[periodic-and-cyclic-boundaries]]: the mixed-out average uses pitchwise periodicity, which makes the integral of the circumferential fluxes vanish ([[ref-chima-1998-nasa-tm-206613]]).

## Leads to

- [[mrf]], [[f-126]], [[doc-case-schema-mixing-plane]], [[sliding-mesh]] (the transient alternative)

## How rcfd uses it

- **Mixing plane in rcfd:** a stage interface between a frozen-rotor zone and a stationary zone in the steady loop (F-126, ADR 0055, ADR 0059); circumferential averaging in radial bands conserving mass, angular momentum and normal momentum flux, passed downstream as a fixed absolute velocity, band pressure passed back, under-relaxed (`relax` 0.5); not delivered: transient and coupled solvers, compressible mixed-out state, rothalpy, sector periodicity, turbulence or scalar transfer through the plane ([[doc-case-schema-mixing-plane]]).
- **Hub item:** F-126 (frozen-rotor and stage interfaces with sector periodicity) is partial; its acceptance "is not attempted" ([[f-126]]).
- **Frozen rotor on FAN-01:** the steady rotating-frame solve does not reach its expected operating point ([[mrf]], [[fan-01]]).
