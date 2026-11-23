---
type: concept
tags: [foundation, structural, fem]
sources: [ref-felippa-ifem, ref-bathe-mit-2-094-fea-notes, adr-thread-structural-verification-elements, adr-thread-structural-materials-nonlinear, adr-thread-structural-hyperelastic-and-follower, adr-thread-structural-incompatible-hex8-rule-l, adr-thread-structural-beams-shells]
builds_on: [isoparametric-elements-and-quadrature, linear-elasticity]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Element locking (shear and volumetric) and its cures

## In short

Low-order displacement elements can be far too stiff for some deformations: a straight-sided quadrilateral cannot bend without also shearing, and an element of nearly incompressible material cannot deform without changing volume. The spurious ("parasitic") energy "locks" the element, giving deflections that can be orders of magnitude too small. Cures change how strains are interpolated or integrated ([[ref-felippa-ifem]] secs. 17.6-17.7; [[ref-bathe-mit-2-094-fea-notes]] lectures 9, 19-20).

## Explanation

**Shear locking in plane elements.** Under pure bending the Quad4 sides stay straight, so a shear strain appears inside each element that the exact solution does not have (Felippa, Fig. 17.13). The bending energy ratio $r = U^e_{FEM}/U^e_{exact}$ measures it: $r \gg 1$ means locking; for aspect ratio $a/b = 10$ and $\nu = 0$, $r \approx 50$ and the element gives "only about 2% of the correct solution"; refining along the beam alone does not help, since $r \to 1/(1-\nu^2)$ with one layer through the height (secs. 17.6.1-17.6.2) ([[ref-felippa-ifem]]).

**Shear locking in beams and plates.** The 2-node displacement-based Timoshenko element cannot make its transverse shear strain zero along the whole element, so thin beams need about 200 elements for a 10 percent error; plate elements based on Reissner-Mindlin theory lock the same way ([[ref-bathe-mit-2-094-fea-notes]], lectures 20-21; see [[beam-theory]], [[plate-and-shell-theory]]).

**Volumetric locking.** As $\nu \to 0.5$ the volumetric strain $\epsilon_V \to 0$ while the pressure $p = -\kappa\epsilon_V$ stays finite, so a pure displacement element must get a nearly zero divergence right; with too many volume constraints the element locks and shows large, wrong pressures as the bulk modulus increases ([[ref-bathe-mit-2-094-fea-notes]], lecture 9).

**Cures.**

- *Modified integration* (Felippa, sec. 17.7): reduced integration uses a rank-deficient rule; weighted integration blends rules, $\mathbf K^e = (1-\gamma)\mathbf K^e_{1\times1} + \gamma\mathbf K^e_{2\times2}$ (17.38); selective integration splits the elasticity matrix into parts integrated by different rules (17.41); selective reduced integration (SRI) when one part's rule is rank-deficient alone. The same tools treat the "pressure locking" of 3D elements with near-incompressible material ([[ref-felippa-ifem]]).
- *Mixed u/p formulation* (Bathe, lecture 9): interpolate pressure separately, element-wise, and condense it statically, $(\mathbf K_{uu} - \mathbf K_{up}\mathbf K_{pp}^{-1}\mathbf K_{pu})\hat{\mathbf u} = \mathbf R$ (9.25); the 9/3 element is the recommended one. A pressure space is acceptable only if the inf-sup condition (9.27) holds uniformly as $h \to 0$; otherwise spurious pressures or locking appear ([[ref-bathe-mit-2-094-fea-notes]]).
- *Assumed (mixed-interpolated) strains*: replace the displacement-based transverse shear by an assumed field, constant in the 2-node beam (20.26) and tied to edge values in the MITC plate and shell elements (21.22-21.24) ([[ref-bathe-mit-2-094-fea-notes]]).
- *Historical list* for the plane case: hybrid stress (1964), Hellinger-Reissner mixed models (1965), modified Gauss integration (1971), incompatible displacement modes (1973), assumed strains (1978); on rectangles they all give the same pure-bending-exact Quad4 (Felippa, sec. 17.6.3) ([[ref-felippa-ifem]]).

**The cost of under-integration.** A rank-deficient element has zero-energy modes besides rigid motion (Felippa, sec. 17.5.1); these are the hourglass modes the rcfd incompatible-mode work fights in compression ([[incompatible-modes-hex8]]). The B-bar and F-bar methods and hourglass stabilisation were not described in the sources read.

## Builds on

- [[isoparametric-elements-and-quadrature]]: interpolation, Gauss rules and element rank.
- [[linear-elasticity]]: the shear/volumetric split and the incompressible limit $\nu \to 0.5$.

## Leads to

- [[incompatible-modes-hex8]], [[hyperelastic-materials]] (mean-dilatation Hex8), [[plate-and-shell-theory]], [[beam-theory]]

## How rcfd uses it

- Opt-in B-bar and incompatible-mode Hex8 per element block (F-390, ADR 0493, done) ([[adr-thread-structural-verification-elements]]).
- B-bar / F-bar Hex8 in the nonlinear driver (F-587, ADR 0506) ([[adr-thread-structural-materials-nonlinear]]); on the inflated sphere at $\kappa/\mu = 10^4$ the Full Hex8 is `15.6` off and the mean-dilatation Hex8 `4.1e-3` (ADR 0557) ([[hyperelastic-materials]]).
- Finite-strain incompatible modes (Wilson modes with Taylor's correction, Simo-Armero enhanced gradient) and their spurious zero eigenvalue in compression at stretch `0.60797` (ADR 0614) ([[incompatible-modes-hex8]]; [[adr-thread-structural-incompatible-hex8-rule-l]]).
- Shell4 uses MITC4 transverse shear, Shell3 MITC3 with stabilisation ([[adr-thread-structural-beams-shells]]).
