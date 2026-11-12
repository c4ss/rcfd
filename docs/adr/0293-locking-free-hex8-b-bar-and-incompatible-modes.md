# ADR 0293: Hex8 without locking: mean-dilatation (B-bar) and incompatible-mode formulations, opt-in per element block

Status: **PLANNED 2026-11-12** (layout by the intermediate-tier plan, [ADR
0290](0290-structural-solver-intermediate-tier-plan.md)); the registration is written by the implementation agent before any code.
Nothing here is a registered limit: the numbers below are the acceptance draft that the registration turns into checks.

Work package SI-3 of the structural solver, intermediate tier. Size M.

## Goal

The fully integrated Hex8 of the basic tier locks in bending (parasitic shear) and near incompressibility. Add two formulations
selected per element block, the default staying full integration bit for bit: the mean-dilatation B-bar (volumetric locking) and
nine incompatible modes condensed in the element, modified so that the patch test holds on distorted elements (shear locking in
bending; this is the enhanced-strain element in its simplest form).

## Dependencies

ADR 0291; roadmap items (SI ids are this tier's packages, ADR 0290 section 7): SI-1, F-372, F-374, F-376, F-379.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. Two opt-in formulations of Hex8, default unchanged bit for
bit. Both pass the patch test of F-374 on distorted meshes within 1e-10 and a single element of either has exactly six zero
eigenvalues (the seventh above 1e-6 of the largest) for aspect ratios 1, 10 and 100. Cantilever of F-379 (L/h = 20, tip shear
force) with ONE element through the thickness and 20 along: incompatible modes within 1 percent of the closed form at nu = 0 and
within 3 percent at nu = 0.3, while full integration on the same mesh is the registered locking control (below 80 percent); at L/h
= 100 with elements of aspect ratio 5 incompatible modes stay within 2 percent. Thick cylinder b/a = 2 in plane strain with nu =
0.4999: B-bar gives the bore displacement within 1 percent of Lame on the mesh on which full integration is below 90 percent
(control). Recovered stress on the patch test equals D eps within 1e-10; the condensed internal modes are recovered for the
stress.

## Reference to check against

Closed forms: the Timoshenko cantilever and Lame's cylinder of ADR 0280; the locking factor of a fully integrated bilinear element
in pure bending, derived in the registration (for aspect ratio 1 and nu = 0 the bending energy is over-estimated by one half,
hence the control). Methods, from memory: Hughes, Int. J. Numer. Meth. Engng 15 (1980) 1413 (B-bar); Wilson, Taylor, Doherty and
Ghaboussi (1973) and Taylor, Beresford and Wilson, Int. J. Numer. Meth. Engng 10 (1976) 1211 (incompatible modes and the
patch-test correction); Simo and Rifai, Int. J. Numer. Meth. Engng 29 (1990) 1595 (the enhanced-strain view).

## Files

`crates/rcfd-solid/src/hex8x.rs`, `crates/rcfd-solid/tests/si_hex8x.rs`.

## What could make it miss

The 3 percent at nu = 0.3 is a draft (the transverse strain in bending is only partly relieved by the modes). The control numbers
are estimates. The two formulations are not combined in this package. Mass uses the standard Hex8 mass. The incompatible element
is not offered to the nonlinear package (ADR 0300).
