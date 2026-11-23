---
type: concept
tags: [foundation, pressure-velocity, time-stepping]
sources: [ref-mit-ocw-2-29-numerics]
builds_on: [pressure-correction-equation, implicit-time-integration]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Projection (fractional-step) methods

## In short

Advance the velocity in two moves each time step. First solve momentum without enforcing continuity (with no pressure, or with the old pressure), which gives an intermediate velocity that is not divergence free. Then solve a Poisson equation for a pressure (or pressure increment) and subtract its gradient: this "projects" the velocity onto the divergence-free fields. Unlike SIMPLE-type schemes there are no outer iterations within the step ([[ref-mit-ocw-2-29-numerics]] L18 s.19-22, L19 s.7-10).

## Explanation

**Idea.** "The divergence producing part of the velocity is 'projected out'" ([[ref-mit-ocw-2-29-numerics]] L18 s.19).

**Non-incremental (Chorin 1968).** The predictor ignores the pressure; the pressure at the new step comes from a Poisson equation built from the divergence of the predicted velocity, and the velocity is corrected with its gradient ([[ref-mit-ocw-2-29-numerics]] L18 s.20).

**Incremental (Goda 1979).** The predictor uses the old pressure; a Poisson equation for the pressure increment enforces continuity; this is "as the SIMPLE method, but without the iterations" ([[ref-mit-ocw-2-29-numerics]] L18 s.21).

**Rotational incremental (Timmermans et al. 1996).** Uses the old pressure in the predictor and corrects the pressure with an increment plus a term that depends on the velocity correction, so the scheme "accounts for u' in the pressure eqn."; it "can be made into a SIMPLE-like method, if iterations are added" ([[ref-mit-ocw-2-29-numerics]] L18 s.22).

**Fractional step (Kim and Moin 1985).** The step is split term by term: convection, diffusion, pressure. The OCW example (similar to Choi and Moin) advances the velocity with the old pressure and Crank-Nicolson averages of the convective, viscous and body-force terms, removes half the old pressure gradient, and adds half the gradient of the new pressure, which comes from a Poisson equation; the splitting error is second order in time, consistent with Crank-Nicolson ([[ref-mit-ocw-2-29-numerics]] L19 s.7-10). "Main difference with SIMPLE-type time-marching schemes: SIMPLE schemes solve the nonlinear pressure and momentum equations several times per time-step in outer iterations" ([[ref-mit-ocw-2-29-numerics]] L19 s.10).

## Builds on

- [[pressure-correction-equation]]: the Poisson equation that does the projection.
- [[implicit-time-integration]]: the time discretisation of the predictor.

## Leads to

- rcfd pages: [[piso]]

## How rcfd uses it

- On the cylinder O-grid with thin wall cells, `time_advance = "fractional_step"` is the recommended remedy for the PISO wall-pressure spokes ("the cheapest accepted", 29 s per simulated second), next to three correctors or a smaller dt (job 48T, ADR 0154) ([[doc-investigation-job48t-cylinder-spokes]]; [[piso]]).
- The step-consistent Rhie-Chow face flux still has located errors with fractional step ([[doc-case-schema-rhie-chow]]).

## Open questions

- Which projection variant (incremental or not, rotational or not) rcfd's `fractional_step` implements is not recorded in the wiki pages read for this page.
