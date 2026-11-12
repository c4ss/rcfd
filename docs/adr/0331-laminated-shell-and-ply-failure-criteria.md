# ADR 0331: Laminated shell: layup stiffness, ply stresses and failure indices

Status: **PLANNED 2026-11-12** (layout by the advanced-tier plan, [ADR 0310](0310-structural-solver-advanced-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are the
acceptance draft that the registration turns into checks.

Work package SA-21 of the structural solver, advanced tier. Size M.

## Goal

A composite or skin-core blade as a layered shell: plies with a material (ADR 0312), a thickness and an angle; the membrane,
coupling and bending stiffness and the transverse shear stiffness of the layup through the shell's section hook; thermal
resultants; stresses and strains at the top and bottom of each ply in its material axes; failure indices and reserve factors by
the maximum-stress, maximum-strain, Tsai-Hill and Tsai-Wu criteria.

## Dependencies

ADR 0312; roadmap items (SA ids are this tier's packages, ADR 0310 section 7): SA-2, F-392.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. One isotropic ply gives the bits of the Shell4 of F-392.
Layup stiffness: a symmetric layup has no coupling (|B| <= 1e-12 |A| t); [0/90]s has A11 = A22 = t (Q11 + Q22)/2; [0/+60/-60]s and
[0/45/-45/90]s have A11 = A22, A66 = (A11 - A12)/2 and A16 = A26 = 0, all within 1e-12 relative. Simply supported square plate of
a cross-ply symmetric layup under a sinusoidal load: centre deflection q0 a^4/(pi^4 (D11 + 2 (D12 + 2 D66) + D22)) within 1
percent at thickness/side 0.01 with 16 x 16 elements. Unsymmetric [0/90] strip heated uniformly: curvature equal to the closed
form of the two-ply laminate (solved by hand in the registration) within 0.5 percent with zero resultants. Off-axis coupon at 15,
30, 45 and 60 degrees: ply stresses sigma c^2, sigma s^2 and -sigma s c within 1e-10, Tsai-Hill failure stress (c^4/X^2 + (1/S^2 -
1/X^2) s^2 c^2 + s^4/Y^2)^(-1/2) within 1e-10, and Tsai-Wu equal to the tensile and compressive strengths on the axes within
1e-12. A layup with a non-positive thickness or an unknown material is refused with the ply.

## Reference to check against

Classical lamination theory and its closed forms (Jones, Mechanics of Composite Materials; Reddy, Mechanics of Laminated Composite
Plates and Shells, for the Navier solution; from memory, all derivable); the criteria are algebraic definitions (Tsai and Wu, J.
Compos. Mater. 5 (1971) 58).

## Files

`crates/rcfd-solid/src/laminate.rs`, `crates/rcfd-solid/tests/sa_laminate.rs`.

## What could make it miss

The transverse shear stiffness of a layup needs a correction factor that depends on the layup (registered rule; thin plates in the
checks). No interlaminar stress, no delamination, no progressive failure. Strengths and the fibre pattern of a moulded blade are
inputs that the repository does not have. Shell3 takes the layup only if ADR 0322 is merged at the start.
