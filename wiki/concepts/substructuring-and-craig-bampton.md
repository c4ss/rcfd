---
type: concept
tags: [foundation, structural, dynamics, reduction]
sources: [ref-felippa-ifem, ref-linder-2025-arxiv-2501-12833, ref-nasa-sp-8050-structural-vibration-prediction, adr-thread-structural-dynamics-harmonic, doc-reference-data-f430-substructuring]
builds_on: [modal-analysis, multi-point-constraints]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Substructuring, static condensation and Craig-Bampton reduction

## In short

A large model can be split into parts (substructures, superelements) that are each reduced to a few unknowns on their interfaces and then joined. For statics, eliminating the interior unknowns is exact (static condensation). For dynamics, the Craig-Bampton method keeps the interface unknowns plus a handful of the part's vibration modes with its interface held fixed ([[ref-felippa-ifem]] ch. 10; [[ref-linder-2025-arxiv-2501-12833]] sec. 2.2).

## Explanation

**Static condensation.** Partition the superelement's unknowns into internal ones $\mathbf u_i$ (not connected to other parts) and boundary ones $\mathbf u_b$. The second block row gives $\mathbf u_i = \mathbf K_{ii}^{-1}(\mathbf f_i - \mathbf K_{ib}\mathbf u_b)$ (Felippa, eq. 10.3), and substituting into the first: $$\tilde{\mathbf K}_{bb} = \mathbf K_{bb} - \mathbf K_{bi}\mathbf K_{ii}^{-1}\mathbf K_{ib}, \qquad \tilde{\mathbf f}_b = \mathbf f_b - \mathbf K_{bi}\mathbf K_{ii}^{-1}\mathbf f_i$$ (10.5), a Schur complement. $\mathbf K_{ii}$ must be non-singular: the part must have no zero-energy modes other than rigid motion, and fixing its boundary must suppress them (otherwise it is "floating", Remark 10.1). After the global solve the interior is recovered from (10.3) ([[ref-felippa-ifem]]).

**Why dynamics needs more.** Condensation reproduces statics exactly but carries no interior inertia. Substructure methods suit large structures of dissimilar parts and can give more accurate high-frequency modes than one model of the same size ([[ref-nasa-sp-8050-structural-vibration-prediction]], sec. 2.1.2.1).

**Craig-Bampton.** Approximate $$\begin{bmatrix}\mathbf q_b\\ \mathbf q_i\end{bmatrix} \approx \begin{bmatrix}\mathbf I & \mathbf 0\\ \boldsymbol\Psi & \boldsymbol\Phi\end{bmatrix}\begin{bmatrix}\mathbf q_b\\ \boldsymbol\eta\end{bmatrix}$$ (Linder et al., eq. 8), with

- constraint modes $\boldsymbol\Psi = -\mathbf K_{ii}^{-1}\mathbf K_{ib}$: column $j$ is the static interior shape for a unit displacement of boundary unknown $j$ with the others fixed (the condensation shapes above);
- fixed-interface normal modes $\boldsymbol\Phi$: the lowest $n_m$ solutions of $(\mathbf K_{ii} - \omega_m^2\mathbf M_{ii})\boldsymbol\phi_m = 0$, mass-normalised.

Projecting gives a reduced model whose stiffness is block diagonal, $\tilde{\mathbf K}_{bb}$ and $\mathrm{diag}\,\omega_m^2$ (eqs. 9-11). It reproduces the static compliance for loads at the boundary exactly, and the kept modes must cover the frequency range of the response; "a very small number of normal modes is sufficient" for most vibration problems (in the paper's example, 25 modes against 3000 constraint modes). Fixed-interface methods are expected to converge better than free-interface ones when the interfaces mostly stick ([[ref-linder-2025-arxiv-2501-12833]]).

**Joining parts.** The retained boundary unknowns are physical, so reduced parts are assembled like elements, or tied by constraints when interfaces do not match ([[multi-point-constraints]]).

## Builds on

- [[modal-analysis]]: the fixed-interface eigenproblem and mass normalisation.
- [[multi-point-constraints]]: congruent transformations $\mathbf T^T\mathbf K\mathbf T$ (Craig-Bampton is one) and interface coupling.

## Leads to

- [[structural-solver]], [[adr-thread-structural-dynamics-harmonic]]

## How rcfd uses it

- Static condensation and fixed-interface component mode synthesis (F-430, ADR 0469, partial: H1 missed as predicted); reference data on [[doc-reference-data-f430-substructuring]] ([[adr-thread-structural-dynamics-harmonic]]).
- Residual vectors of interior loads with modes chosen by cutoff (F-552, ADR 0474, partial) and of caller load patterns (F-559, ADR 0511, partial; two clauses hold only with the interface enrichment of F-603) ([[adr-thread-structural-dynamics-harmonic]]).
