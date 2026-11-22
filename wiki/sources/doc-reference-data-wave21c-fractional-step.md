---
type: source
tags: [reference-data, wave-section]
sources: []
origin: docs/reference-data.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Reference data: Wave 21C run-loop wiring and fractional-step references (F-056, F-054, F-210, ADR 0037)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Wave 21C run-loop wiring and fractional-step references (F-056, F-054, F-210, ADR 0037)" (lines 877-895) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Wave 21C run-loop wiring and fractional-step references (ADR 0037).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


Web searches were run for the entries marked "verified" (publisher or society records, abstracts). No paper was read in full, so the numerical content of every entry below is either derived in the ADR or checked by the registered tests, not taken from the papers.

| Quantity | Source | Status |
| --- | --- | --- |
| Projection method, one Poisson solve per step, non-incremental pressure | A. J. Chorin (1968), "Numerical solution of the Navier-Stokes equations", Math. Comp. 22, 745-762 | verified citation (AMS record); paper not read |
| Fractional step with boundary conditions for the intermediate velocity that remove the splitting error at the wall (`u* = u_b + dt grad(q)_tangential` for the lagged pressure term) | J. Kim and P. Moin (1985), "Application of a fractional-step method to incompressible Navier-Stokes equations", J. Comput. Phys. 59, 308-323 | abstract and the NASA TM-85898 precursor found; volume and page numbers not confirmed against a publisher record (UNVERIFIED); the lagged-increment form implemented here is the author's reading, not copied |
| Incremental pressure-correction schemes, order of the splitting error of the non-incremental scheme | J.-L. Guermond, P. Minev and J. Shen (2006), "An overview of projection methods for incompressible flows", Comput. Methods Appl. Mech. Engrg. 195, 6011-6045; J.-L. Guermond and L. Quartapelle (1997) | verified citation and abstract; theorems not re-derived (the measured F8 result is consistent with a first-order non-incremental steady state, see the ADR addendum) |
| Collocated-grid fractional step (pressure-smoothing face flux) | Zang, Street and Koseff (1994), J. Comput. Phys. 114, 18-33 | UNVERIFIED, from memory, not used quantitatively |
| Rotational pressure-correction form (named gap, wave 25A) | Timmermans, Minev and van de Vosse (1996), Int. J. Numer. Methods Fluids 22, 673-688 | UNVERIFIED, from memory, not read; NOT implemented and not used |
| Wave 25A (ADR 0043) BDF2 extrapolation of the periodic partner value; line search on the convex enthalpy potential; exact 1-D discrete Brinkman solution; compensated column sums | derived in ADR 0043 from first principles | derived; no external data; the Voller-Swaminathan linearised-source update (1991) was considered from memory (UNVERIFIED) and not implemented |
| Taylor-Green vortex decay `exp(-2 nu t)` | wave 7 entry above | verified earlier |
| Lid-driven cavity centreline data, Re = 100 | Ghia, Ghia and Shin (1982); two secondary copies | verified earlier (the CSV files in `cases/cavity/reference`) |
| Particle tracking and chemistry closed forms used by the wiring checks | ADR 0027, ADR 0028 entries above | unchanged; the wiring checks compare the run path with the library, so they introduce no new reference values |



## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
