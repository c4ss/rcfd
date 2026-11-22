---
type: concept
tags: [foundation, shared-roots, mathematics, signal-processing]
sources: [ref-wikipedia-shared-roots, ref-nist-sematech-ehandbook]
builds_on: []
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Fourier analysis and spectra (DFT, FFT, PSD)

## In short

Fourier analysis writes a signal as a sum of sine waves; its spectrum says how much of the signal sits at each frequency. A sampled signal of `N` points has a discrete Fourier transform (DFT), computed fast by the FFT, with frequency bins spaced `f_s / N` apart; the power spectral density (PSD) is the signal's power per unit frequency ([[ref-wikipedia-shared-roots]] "Discrete Fourier transform", "Spectral density").

## Explanation

**Discrete Fourier transform.** For samples `x_0 ... x_(N-1)` ([[ref-wikipedia-shared-roots]] "Discrete Fourier transform", Eq. 1-2):

`X_k = sum_(n=0)^(N-1) x_n exp(-i 2 pi k n / N)`, `x_n = (1/N) sum_(k=0)^(N-1) X_k exp(+i 2 pi k n / N)`

`X_k` is the amplitude and phase of a sinusoid with `k` cycles per `N` samples. With sampling interval `dt` and rate `f_s = 1/dt`, bin `k` is at `f_k = k f_s / N` and the resolution is `df = f_s / N = 1/(N dt)`: a finer frequency resolution needs a longer record, not a faster sample rate (sec. 1.1).

**Real signals and energy.** For real input `X_k = conj(X_(N-k))`, so only half the bins are independent (sec. 3.17); Parseval: `sum |x_n|^2 = (1/N) sum |X_k|^2` (sec. 3.6) ([[ref-wikipedia-shared-roots]]).

**Pitfalls.** Sampling causes aliasing (frequencies above what the sample rate can represent fold onto lower ones), reduced by choosing the sample rate; truncating a record causes leakage (energy spreads into neighbouring bins), reduced by the record length and window functions; zero-padding gives a finer-looking spectrum but does not remove leakage ([[ref-wikipedia-shared-roots]] "Discrete Fourier transform", sec. 6.1). The FFT computes the DFT in `O(N log N)` operations, which is what makes it practical (intro, sec. 6.6); direct evaluation of Eq. 1 needs `N` terms for each of `N` outputs.

**Power spectral density.** `S_xx(f) = lim_(T->inf) (1/T) |x_T(f)|^2`; by the Wiener-Khinchin theorem it is the Fourier transform of the autocorrelation. Units are signal-squared per hertz (W/Hz, Pa^2/Hz, m^2/Hz); integrating it over frequency gives the mean power, the variance for a random signal. One periodogram is noisy; averaging many (e.g. Welch's method) converges to the PSD. Engineering usually uses one-sided spectra (positive frequencies only) ([[ref-wikipedia-shared-roots]] "Spectral density"). The NIST handbook's spectral plot is "a smoothed Fourier transform of the autocovariance function", used to find how many cyclic components a series has and which frequency dominates, from 0 to 0.5 cycles per observation ([[ref-nist-sematech-ehandbook]] sec. 1.3.3.27).

**What it adds.** First-year physics describes a single wave by its frequency; Fourier analysis decomposes any recorded signal (a pressure trace, a force history) into many, which is how tones, broadband noise and vibration modes are separated.

## Builds on

Floor only: sine waves, complex exponentials `exp(i theta)`, sums and integrals.

## Leads to

- `rotor-tone-noise` (blade-passing frequency and harmonics), `sound-levels-and-metrics` (bands, SPL), `harmonic-and-random-response` (PSD response of structures), [[cyclic-symmetry]] (harmonic index).
- rcfd: [[aeroacoustics]], [[fw-h]], [[campbell-diagram]].

## How rcfd uses it

- The FW-H tool outputs a signal CSV and spectra; its result is "only as good as the sampled surface pressure, which must resolve the blade-passing frequency" ([[fw-h]]).
- The stated acoustics validation path compares far-field spectra of a fan or airfoil with a held-out data set ([[aeroacoustics]]).
- The structural rotor-dynamics work produces a [[campbell-diagram]] (Campbell table) of frequencies; see that page for what rcfd computes.
