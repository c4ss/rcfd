---
type: source
tags: [reference, foundation, shared-roots, statistics, nist, cite-only]
sources: []
origin: https://www.itl.nist.gov/div898/handbook/
licence: none stated on the pages read; NIST's site policy (https://www.nist.gov/copyrights-disclaimers) says information on NIST sites is public and may be copied or distributed "With the exception of material marked as copyrighted", with credit requested
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# NIST/SEMATECH e-Handbook of Statistical Methods (sections read)

**Origin:** *NIST/SEMATECH e-Handbook of Statistical Methods*, https://www.itl.nist.gov/div898/handbook/ (frameset title; no authors or date shown on the frameset). Sections read: 1.3.5.1 (eda351.htm), 1.3.5.2 (eda352.htm), 1.3.5.6 (eda356.htm), 1.3.6.6.1 (eda3661.htm), 1.3.3.27 (eda33r.htm), 4.4.3.1 (pmd/section4/pmd431.htm).
**Kind:** US Government statistics handbook (web).
**Licence:** the pages carry no statement; NIST's general site policy allows copying except for material marked as copyrighted. Not verified as public domain on the pages themselves, so cite only (a copy into `raw/` would need the maintainer to accept the site policy).
**Read:** 2026-11-22, by job p6-roots.

## Summary

Sample statistics (mean, median, variance, standard deviation, robust scale measures), confidence limits for a mean, the normal distribution and the central limit theorem, the spectral plot, and least-squares fitting of a straight line.

## Key points by section

### 1.3.5.1 "Measures of Location"
- Mean `Ybar = sum Y_i / N`; median the middle sorted value (average of the two middle values for even `N`); mode the most frequent value.
- The mean is optimal for normal data but pulled by skewness and long tails; the median is rank based and robust.

### 1.3.5.6 "Measures of Scale"
- Variance `s^2 = sum (Y_i - Ybar)^2 / (N - 1)`; standard deviation `s` its square root; range; average absolute deviation; median absolute deviation (MAD); interquartile range.
- For 10 000 samples: normal SD 0.997, MAD 0.681; Cauchy SD 998.389, MAD 1.16. The SD is best for normal data and not robust for long-tailed data.

### 1.3.5.2 "Confidence Limits for the Mean"
- `Ybar +- t_(1-alpha/2, N-1) s / sqrt(N)`; the interval narrows as `N` grows.
- The 95 % describes the method: about 95 % of intervals from repeated samples contain the true mean; "a 95 % confidence interval does not mean that there is a 95 % probability that the interval contains the true mean".
- Example ZARR13.DAT: `N = 195`, mean 9.261460, `s = 0.022789`, 95 % interval (9.258242, 9.264679).

### 1.3.6.6.1 "Normal Distribution"
- `f(x) = exp(-(x - mu)^2 / (2 sigma^2)) / (sigma sqrt(2 pi))`; standard normal `mu = 0`, `sigma = 1`; estimated by the sample mean and standard deviation.
- Central limit theorem: as `N` grows the sample mean becomes approximately normal whatever the variable's distribution, centred on `mu` with standard deviation approaching `sigma / sqrt(N)`.
- Many classical tests and regression models assume normal errors; the assumption should be tested.

### 1.3.3.27 "Spectral Plot"
- "a smoothed Fourier transform of the autocovariance function": smoothed variance (power) against frequency in cycles per observation, 0 to 0.5 for equally spaced data (0.5 is a two-point cycle). Answers how many cyclic components there are and which frequency dominates; trends should be removed first.

### 4.4.3.1 "Least Squares"
- Criterion `Q = sum [y_i - f(x_i; beta)]^2`, minimised over the parameter estimates.
- For linear models the minimisation is done analytically (partial derivatives set to zero); nonlinear models need iterative algorithms.
- Straight line `y = beta0 + beta1 x + eps`: `beta1 = sum (x_i - xbar)(y_i - ybar) / sum (x_i - xbar)^2`, `beta0 = ybar - beta1 xbar`; residual standard deviation `sigma = sqrt(Q / (n - p))` with `p` parameters. The page does not use the word "residual" or "normal equations".

## Contradictions with the wiki

None.

## Pages this source informs

[[probability-and-statistics-basics]], [[linear-regression-least-squares]], [[fourier-analysis-and-spectra]], [[measurement-uncertainty]]
