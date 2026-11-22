---
type: concept
tags: [foundation, acoustics, noise-metrics, decibel]
sources: [ref-hubbard-1991-nasa-rp-1258-vol2, ref-hubbard-1991-nasa-rp-1258-vol1]
builds_on: [fourier-analysis-and-spectra]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Sound levels and noise metrics

## In short

Sound pressures span a factor of about a million between the threshold of hearing and the threshold of pain, so they are reported on a logarithmic scale, the decibel. Sound pressure level (SPL) compares an rms pressure with $20\,\mu\text{Pa}$; sound power level compares a radiated power with $10^{-12}$ W. Spectra are summed into bands, and A-weighting tilts them to follow the ear's sensitivity ([[ref-hubbard-1991-nasa-rp-1258-vol2]] ch. 11, pp. 3-9; Glossary pp. 415-417).

## Explanation

**Sound pressure level.** ([[ref-hubbard-1991-nasa-rp-1258-vol2]] Glossary)

$$\text{SPL} = 20 \log_{10}\frac{p}{p_{ref}}\ \text{dB}, \qquad p_{ref} = 2\times 10^{-5}\ \text{Pa}$$ (Hubbard vol. 2, glossary)

where $p$ is the (rms) sound pressure "corresponding to a particular frequency bandwidth". A factor 10 in pressure is 20 dB; a factor 2 is about 6 dB.

**Sound power level.** For a source radiating acoustic power $W$ (Glossary):

$$\text{PWL} = 10 \log_{10}\frac{W}{W_{ref}}\ \text{dB}, \qquad W_{ref} = 10^{-12}\ \text{W}$$ (Hubbard vol. 2, glossary)

The factor is 10 rather than 20 because power goes as pressure squared.

**Overall level and bands.** The overall sound pressure level (OASPL) gives equal weight to all frequencies; it "is not standardized but is generally considered to extend from 20 to 20000 Hz", the range of human hearing (Glossary; ch. 11, p. 3). Spectra are commonly grouped into octave or 1/3-octave bands, each with its own SPL (ch. 11, pp. 4, 8). Band levels add on an energy basis, $L = 10\log_{10}\sum_i 10^{L_i/10}$, the form of the A-weighted sum below; two equal incoherent levels make 3 dB more.

**A-weighting.** The A-weighting filter, originally meant to approximate loudness for SPL between 24 and 55 dB, "has been found to correlate very well with noisiness and loudness of many sounds with broadband spectra regardless of level" ([[ref-hubbard-1991-nasa-rp-1258-vol2]] ch. 11, p. 8, fig. 5). It cuts low frequencies strongly; the ear is most sensitive near 3 kHz (p. 4). Applied to band levels, the weighted levels are summed on an energy basis (p. 9):

$$L_A = 10\log_{10}\left[\sum_{i=1}^{n} 10^{L_A(i)/10}\right]$$ (Powell and Fields, eq. 1)

with $L_A(i)$ the A-weighted SPL of band $i$. Other ratings (loudness in sones and phons, perceived noise level PNL, EPNL) build further on band levels (pp. 9-10).

**Distance.** For a point source in free space the level falls 6 dB per doubling of distance at all frequencies; atmospheric absorption adds a frequency-dependent loss (ch. 12, pp. 55-56).

**Tones and spectra.** A rotor's spectrum is plotted as SPL against frequency or harmonic of blade-passage frequency ([[ref-hubbard-1991-nasa-rp-1258-vol1]] ch. 1, fig. 7; see [[rotor-tone-noise]]). The pressure spectrum itself comes from [[fourier-analysis-and-spectra]].

**What it adds.** [[fourier-analysis-and-spectra]] splits a pressure signal into frequencies; this page turns those amplitudes into the decibel numbers that are compared with hearing and with measurements.

## Builds on

- [[fourier-analysis-and-spectra]]: spectra, band powers and Parseval's relation between time and frequency domains.

## Leads to

- [[aeroacoustics]], [[rotor-tone-noise]]

## How rcfd uses it

- rcfd's noise metrics are SPL, OASPL, A weighting, Parseval spectra, 1/3-octave bands and sphere sound power, written as `rcfd-spectrum` v1 JSON ([[doc-acoustics]]).
- The A-weighting uses the IEC 61672-1 analytic formula with constants from secondary sources, the standard itself not read; 1/3-octave labels not verified against IEC 61260-1 ([[aeroacoustics]]).
- The ERP figure for a fan frame or housing assumes radiation efficiency one ([[aeroacoustics]]).
