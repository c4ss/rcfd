---
type: concept
tags: [foundation, shared-roots, statistics]
sources: [ref-nist-sematech-ehandbook, ref-nist-tn-1297]
builds_on: []
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Probability and statistics basics

## In short

A set of repeated measurements or samples is summarised by its mean (typical value) and standard deviation (spread). The normal distribution describes many such sets, and the central limit theorem explains why: averages tend to be normal. A confidence interval says how well the mean is known from a finite sample; covariance says how two quantities vary together ([[ref-nist-sematech-ehandbook]] sec. 1.3.5, 1.3.6.6.1; [[ref-nist-tn-1297]] App. A).

## Explanation

**Location.** Mean $\bar{Y} = \sum Y_i / N$; median the middle value of the sorted data; mode the most frequent value. The mean is best for normal data but pulled by skew and long tails; the median is robust ([[ref-nist-sematech-ehandbook]] sec. 1.3.5.1).

**Spread.** Sample variance $s^2 = \sum (Y_i - \bar{Y})^2 / (N - 1)$, standard deviation $s = \sqrt{s^2}$; alternatives are the range, the median absolute deviation (MAD) and the interquartile range. The standard deviation is best for normal data but not robust: for 10 000 Cauchy samples the SD was 998.389 while the MAD was 1.16 ([[ref-nist-sematech-ehandbook]] sec. 1.3.5.6).

**Normal distribution.** $f(x) = \dfrac{e^{-(x - \mu)^2 / (2 \sigma^2)}}{\sigma \sqrt{2 \pi}}$ with mean $\mu$ and standard deviation $\sigma$; $\mu = 0$, $\sigma = 1$ is the standard normal ([[ref-nist-sematech-ehandbook]] sec. 1.3.6.6.1).

**Central limit theorem.** As $N$ grows, the sample mean becomes approximately normal whatever the original distribution, centred on $\mu$, with standard deviation approaching $\sigma / \sqrt{N}$ ([[ref-nist-sematech-ehandbook]] sec. 1.3.6.6.1). So averaging $N$ independent values shrinks the scatter of the average by $\sqrt{N}$.

**Confidence interval for a mean.** $\bar{Y} \pm t_{1-\alpha/2,\, N-1}\, s / \sqrt{N}$, with $t$ the Student-t percentile for $N - 1$ degrees of freedom. The confidence level describes the method: about 95 % of intervals built this way contain the true mean, which is not the same as a 95 % probability that this interval does. Example: $N = 195$, mean 9.261460, $s = 0.022789$, 95 % interval (9.258242, 9.264679) ([[ref-nist-sematech-ehandbook]] sec. 1.3.5.2).

**Covariance.** When a result depends on several inputs that vary together, their covariance $u(x_i, x_j)$ enters the combined variance alongside the individual variances ([[ref-nist-tn-1297]] App. A, Eq. A-3); for independent inputs it is zero.

**What it adds.** Calculus treats a quantity as one exact number; statistics treats it as a spread of possible values, which is what is needed to compare a computation with a measurement that scatters.

## Builds on

Floor only: sums, square roots, the exponential function, integrals of a function.

## Leads to

- [[measurement-uncertainty]] (standard uncertainty is a standard deviation), [[validation-comparison]].
- [[linear-regression-least-squares]], `design-of-experiments`, `multivariate-normal-and-kernels`, `maximum-likelihood-estimation`, `cross-validation`, `evolutionary-algorithms`, `turbulent-dispersion-random-walk`.

## How rcfd uses it

- The Gaussian-process surrogate predicts "a mean and an uncertainty at any untried point" ([[gaussian-process-surrogate]]); its leave-one-out standard deviation is checked against a double-double brute force (rung R3) ([[fidelity-management]]).
- The NACA 0012 validation band of ADR 0760 uses the scatter of Ladson's three grit zones ($s = 0.018$) ([[verification-vs-validation]]).
- Particle dispersion is a discrete random walk driven by $k$ and $\varepsilon$ ([[lagrangian-particles]]).
