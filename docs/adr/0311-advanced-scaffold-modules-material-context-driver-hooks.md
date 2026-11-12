# ADR 0311: Scaffold of the advanced tier: module layout, material variants with a point context and a state layout, extension points of the Newton driver

Status: **PLANNED 2026-11-12** (layout by the advanced-tier plan, [ADR 0310](0310-structural-solver-advanced-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are the
acceptance draft that the registration turns into checks.

Work package SA-1 of the structural solver, advanced tier. Size M.

## Goal

Prepare the shared files once, so that every later package of the tier fills a file of its own. Declare the new modules of
`rcfd-solid`, `rcfd-linalg` and `rcfd-post` (ADR 0310 section 2.2) as stubs. Add the material variants (orthotropic, tabulated,
viscoelastic, creep, J2, hyperelastic) as data that is refused until its package lands; give the constitutive call a point context
(temperature, time increment, material axes at the point) and each material a declared state size per integration point, with
commit and roll-back unchanged from ADR 0300. Add the element kind `Shell3`, the shell property variant with a layup, and one
section-stiffness hook in the shell so that a laminate replaces the thickness integral without editing the element. Give the
Newton driver two extension points: a list of extra contributions (residual and tangent: contact, follower loads) and a
step-control hook (arc-length). Add a complex number type to `rcfd-linalg`. No analysis numerics.

## Dependencies

none in this list; roadmap items (SA ids are this tier's packages, ADR 0310 section 7): F-372, F-388, F-397.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. The modules, material variants, point context, state layout,
element kind, layup property, section-stiffness hook and the two driver extension points of ADR 0310 section 2 exist; every test
of the basic and intermediate tiers (the sb_ and si_ files) compiles and passes unedited, and a registered set of models (patch
test, cantilever, bar modes, the elastica of F-397, the free vibration of F-398, a Shell4 plate) gives displacements, stresses,
frequencies and histories with the same bits as before the change; with an empty contribution list and no step hook the Newton
driver takes the same iterations with the same residuals bit for bit; the default point context (no temperature, no time
increment, global axes) reproduces the isotropic stress and tangent bit for bit; each new material variant, the element kind
Shell3 and the layup property are refused by every solve with an error naming the variant and the set, never ignored; the complex
type satisfies (a b) conj(b) = a |b|^2 and a/a = 1 within 1e-15 on a registered sample and prints and parses losslessly; the
baseline hashes are unchanged.

## Reference to check against

No physics: the reference is the two earlier tiers (bit identity) and the refusals. The models for a declare-once scaffold are ADR
0271 and ADR 0291.

## Files

`crates/rcfd-solid/src/{lib.rs, model.rs, material.rs, mesh.rs, element.rs, shell.rs, nonlinear.rs}` (additions only), one-line
stubs of the modules of ADR 0310 section 2.2; `crates/rcfd-linalg/src/lib.rs` (four `pub mod` lines), `complex.rs`, stubs
`cskyline.rs`, `eigen_general.rs`, `sparse_chol.rs`; `crates/rcfd-post/src/lib.rs` (three `pub mod` lines), stubs
`solid_acoustic.rs`, `solid_aerodamp.rs`, `solid_rotor.rs`; `crates/rcfd-solid/tests/sa_scaffold.rs`.

## What could make it miss

The earlier tiers are plans. If the constitutive call of ADR 0273 has a fixed argument list and the tests call it directly, the
context goes into a second entry point with a forwarding default (ADR 0310 section 2.4, point 1). If the Newton driver of ADR 0300
is one closed function, the extension points mean restructuring it, and the bit identity of its iterations is then the hard
clause. It edits more shared files than ADR 0291 did, hence M.
