---
type: concept
tags: [foundation, verification-validation, measurement]
sources: [ref-nist-tn-1297, ref-nist-sematech-ehandbook, ref-oberkampf-trucano-2002-sand2002-0529, ref-nparc-vv-tutorial]
builds_on: [probability-and-statistics-basics]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Measurement uncertainty

## In short

A measured value comes with an uncertainty: an estimate of how far the true value may lie from it. The standard way is to give each source of uncertainty as a standard deviation (standard uncertainty), combine them by root-sum-of-squares into a combined standard uncertainty, and multiply by a coverage factor, usually `k = 2` for about 95 %, to state an expanded uncertainty `Y = y +- U` ([[ref-nist-tn-1297]] sec. 2, 5, 6).

## Explanation

**Standard uncertainty.** Each component is an estimated standard deviation `u_i` (sec. 2.4). It is evaluated by *Type A*, statistics of repeated observations (e.g. the standard deviation of a mean of `n` readings, App. A-4, A-5), or *Type B*, any other information through an assumed distribution (e.g. a rectangular distribution of half-width `a` gives `a / sqrt(3)`, App. A-6, A-7) ([[ref-nist-tn-1297]]). Instead of "random" and "systematic uncertainty", TN 1297 speaks of components arising from random or from systematic effects, since a component can change category (sec. 2.3); each recognised systematic effect is corrected for, and the correction's own uncertainty is a component (sec. 5.2).

**Combining (law of propagation of uncertainty).** For a result `y = f(x_1, ..., x_N)` (App. A, Eq. A-3):

`u_c^2(y) = sum_i (df/dx_i)^2 u^2(x_i) + 2 sum_(i<j) (df/dx_i)(df/dx_j) u(x_i, x_j)`

The partial derivatives are sensitivity coefficients and `u(x_i, x_j)` the covariances; for independent inputs only the first sum remains ([[ref-nist-tn-1297]]; [[probability-and-statistics-basics]]).

**Expanded uncertainty.** `U = k u_c`, with the measurand "confidently believed" to lie in `y - U <= Y <= y + U` (sec. 6.1). `k = 2` gives about 95 % for a normal distribution, `k = 3` more than 99 % (sec. 6.2); NIST reports `k = 2` by convention (sec. 6.5). `y +- u_c` alone is about 68 % (sec. 5.4) ([[ref-nist-tn-1297]]).

**Scatter of a mean.** A mean of `N` readings is known to `t s / sqrt(N)`, not to `s`: more replications narrow the interval though the scatter of single readings stays the same ([[ref-nist-sematech-ehandbook]] sec. 1.3.5.2).

**In CFD validation.** Experimental data carry random and bias (systematic) errors that must be quantified and documented, and comparison plots should show error bars on the data ([[ref-nparc-vv-tutorial]] `valassess.html`). Oberkampf and Trucano split the measurement error `E1 = u_nature - u_exp` into a bias shift and a random part and consider the bias "the more important error"; examples are transducer drift, amplifier gain, free-stream Mach number and intrusive probes ([[ref-oberkampf-trucano-2002-sand2002-0529]] sec. 4.1.2).

**What it adds.** [[probability-and-statistics-basics]] gives the standard deviation of one data set; measurement uncertainty turns all sources of doubt, statistical or not, into one number with a stated coverage that a comparison can use.

## Builds on

- [[probability-and-statistics-basics]]: standard deviation, normal distribution, covariance, confidence of a mean.

## Leads to

- [[validation-comparison]], [[verification-vs-validation]].

## How rcfd uses it

- **Uncertainty budget for a user-measured fan:** each measurement's standard uncertainty should be "at most one third of the comparison tolerance the tools will be held to" ([[verification-vs-validation]], from [[doc-fan-measurement-protocol]]).
- **Band from scatter between data sets (ADR 0760):** for NACA 0012 the band term `s = 0.018` is the scatter of Ladson's three grit zones ([[verification-vs-validation]]).
- **A band needs the stated uncertainty (ADR 0762):** the round-robin scatter is "another quantity than the one the acceptance names", so F-870 waits for F-877 to read the measurement's uncertainty ([[verification-vs-validation]], [[adr-thread-fan01-design-point]]).
