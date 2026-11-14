# ADR 0494: F-583 registration: the harmonic stress of a viscoelastic material in the direct harmonic solve, as the split stress of the elastic base

Status: **REGISTERED 2026-11-14, written before any code of F-583 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-583 was written into the library or the tests. Two probes were run beforehand, both
disclosed below: a throw-away test file (a copy of `sa_harmonic_visco.rs` plus a probe test, deleted, not committed; no new
library code) that measured the strain of the existing solver against the discrete chain and the strain of the viscoelastic
route against the table route (arithmetic 4), and a numpy script (`w49i-work/f583-scratch/probe_nc.py`) for the negative
controls (arithmetic 5). The bit record B1 of the default stress paths is a test file written and run on the unchanged base
aa0e4d5b after this commit and before any library code (D5). Results go in the addenda at the end; nothing above the addenda
is changed after a run.

Roadmap item F-583 (deps F-577), follow-up 2 of [ADR 0491](0491-f577-registration-viscoelastic-material-in-the-direct-harmonic-solve.md)
(see also [ADR 0485](0485-f410-registration-linear-viscoelasticity-prony-series.md)). Number 0494 given by the lead. Branch
`f583-work` from aa0e4d5b, worktree `w49i-work/f583`, scratch `w49i-work/f583-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number); each registered run made once; defaults bit-identical; no
edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe code, the
crate graph unchanged; clean-room. The brief of this item asks for the roadmap entry in `docs/ROADMAP.md` and
`docs/roadmap.json` to be updated by this branch in a separate last commit (the svgs are not regenerated); the proposed note is
also given under "For the lead: roadmap entries".

## What F-583 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. DirectHarmonic::stress of a response with a viscoelastic material returns (G*/G_0) dev sigma_0 + (K*/K_0) vol sigma_0 at each integration point, sigma_0 the elastic stress of the base. On the rods A, B and C of ADR 0491 at 40 elements the axial (or shear) stress at the integration points equals the modulus times the strain of the discrete chain within 1e-9 relative; for equal weights it equals the stress of the LossFactor::Table route within 1e-12. Models without the material keep their bits.

Clauses: (a0) every limit registered; (a1) the stress is the split stress at each integration point; (a2) rods A, B, C at 40
elements: the axial (rods A, B) or shear (rod C) stress equals the modulus times the strain of the discrete chain, `1e-9`
relative; (a3) equal weights (rod A): equal to the stress of the `LossFactor::Table` route, `1e-12`; (a4) models without the
material keep their bits.

Readings, registered:

- **(a1) "elastic stress of the base".** `sigma_0 = D_0 eps` of the base material of each viscoelastic material, at the
  integration points of `HarmonicResponse::stress` (the real and the imaginary part of the displacement, each
  `sigma(x_part) - sigma(0)`), computed on the model with each viscoelastic material replaced by its elastic base. Then, per
  element and per part (real and imaginary), `dev sigma_0 = sigma_0 - (tr sigma_0 / 3) I` (Voigt: the normal components
  minus the mean, the shear components unchanged), `vol sigma_0 = (tr sigma_0 / 3) I`, and the stress is the complex
  `(G*/G_0) dev sigma_0 + (K*/K_0) vol sigma_0`. For a material of equal weights (one part, ADR 0491 D1) the split is not
  needed and the stress is `sigma_0` times `E*/E_0`, exactly as the table route multiplies by `(s, s eta)`.
- **(a2) "the modulus times the strain of the discrete chain".** The rod's modulus `Mod*(omega)` of ADR 0491 (`K* + 4 G*/3`
  for rods A and B, `G*` for rod C) times the strain of the 1-D chain of 40 linear elements with that modulus
  (`(u_{e+1} - u_e)/h` of element `e`), at every integration point of the element (eight per Hex8; the strain is constant over
  an element in this arrangement). The measure is pointwise: the maximum over the points and the ten frequencies of
  `|sigma_solver - sigma_ref| / |sigma_ref|`, for the component the acceptance names (`xx` for A, B; `xy` for C). The other
  components are checked as well (S2): with the lateral motion held the axial rod has `sigma_yy = sigma_zz = (K* - 2 G*/3)
  eps_xx` (zero for rod A, `nu = 0`) and the shear rod has no other stress; measured as `max |sigma_solver - sigma_ref| / max
  |sigma_ref,primary|` over the points of each frequency.
- **(a3) "the stress of the `LossFactor::Table` route".** Rod A with the table of `modulus_table(A, f_k)` (ADR 0491 E1) on the
  elastic base, `DirectHarmonic::stress` of its response at each `omega_k`, against the stress of the viscoelastic route,
  both at `omega_k`: all six components at all points, normalised by the largest stress amplitude of the table route over the
  points and components of that frequency, and pointwise for `xx`.

## What was read (no run)

`AGENT_RULES.md`; the brief; ADR 0491 whole (D1, D2, the checks, the addendum, remaining question 2, follow-up 2) and ADR
0485 (D1, D2, arithmetic 2); `harmonic_direct.rs` (`stress`, `coefficients`, `viscoelastic_parts`, `Damping`), `harmonic.rs`
(`HarmonicResponse::stress`, `HarmonicStress`), `recover.rs` (`IntegrationPointField::compute_with`: the material is
evaluated per point, which the viscoelastic material refuses without state), `mat_visco.rs` (`harmonic_base`, `split`),
`model.rs` (`SolidModel`, `Material::base`), the tests `sa_harmonic_visco.rs` (rods, chain, the checks, RF (vi)) and
`sa_harmonic_visco_default_bits.rs` (the digest scheme).

## Arithmetic done before the code (disclosed)

1. **The split stress** (first principles; the correspondence principle as in ADR 0491 arithmetic 1). The steady stress
   amplitude of the Prony material is `sigma* = 2 G* dev eps* + K* tr eps* 1`; the elastic base stress is `sigma_0 = 2 G_0 dev
   eps* + K_0 tr eps* 1`, so `dev sigma_0 = 2 G_0 dev eps*`, `vol sigma_0 = K_0 tr eps* 1`, and `sigma* = (G*/G_0) dev sigma_0
   + (K*/K_0) vol sigma_0` exactly (isotropic base). It is the contraction with the same two pieces whose stiffnesses make the
   solver's matrix (ADR 0491 D1), so the stress is the one the solver's equations assumed.
2. **The rod.** ADR 0491 arithmetic 2: with the lateral motion held, `eps` has only `xx` (axial) or `gamma_xy` (shear), uniform
   over the section and constant along an element (linear element), so the stress at all eight points of an element is the
   same `D* eps`: `sigma_xx = (K* + 4 G*/3) eps`, `sigma_yy = sigma_zz = (K* - 2 G*/3) eps`; shear `sigma_xy = G* gamma`.
3. **The chain stress.** The chain solution of ADR 0491 (arithmetic 6) is the exact finite-element solution of the rod, so
   `Mod* (u_{e+1} - u_e)/h` per element is the exact discrete stress; the solver differs by rounding times the amplification
   of a difference quotient.
4. **Probe 1 (disclosed, run before registering).** On the unchanged solver, the strain `(u_{e+1} - u_e)/h` of each element
   from the response vector against that of the chain, pointwise relative, max over the ten frequencies, 40 elements: rod A
   `3.207e-12`, rod B `3.222e-12`, rod C `2.090e-12` (the stress adds one complex multiplication, about `1e-16`). The
   acceptance's `1e-9` has a margin of 300. **Probe 2:** the strain of the viscoelastic route against the strain of the table
   route (rod A, ten frequencies): pointwise `1.561e-13`, normalised by the largest strain `1.482e-13` (the response vectors
   differ by `1.4e-13`, ADR 0491 E1; the difference quotient does not amplify it, the strain being a smooth field). The stress
   adds the coefficient rounding of the two routes (`E*/E` against `(s, s eta)`, about `1e-16` times the response factor
   already inside E1). Estimate `3e-13`; the acceptance's `1e-12` is kept, with a margin of 3; a miss is recorded and
   diagnosed (not tuned).
5. **Negative controls (numpy, disclosed).** Stress of rods B and C by the wrong routes, over the ten frequencies, minimum of
   the relative difference to `Mod* eps`: (a) the one-part route `E*/E_0` on the base modulus: B (axial) `3.436e-2`, C (shear)
   `9.229e-3`; (b) no scaling at all (`sigma_0`): B `6.166e-2`, C `1.079e-1`.
6. **Zero frequency.** At `omega = 0` the coefficients are real long-term ratios and the static rod has `sigma_xx = F/A`
   exactly (equilibrium), whatever the modulus. Rod B at `omega = 0`: `|sigma_xx A/F - 1|` at the points. ADR 0491 Z1 gave the
   tip within `1.74e-14`; the strain difference quotient (a displacement difference over a step `1/40` of the tip) multiplies
   by about 40: estimate `1e-12`, limit `1e-10`.
7. **The shift.** As ADR 0491 arithmetic 9: `1.7e-15` on the response, the strain times about 40: limit `1e-12`.

## Design

### D1. `DirectHarmonic::stress` of a viscoelastic part (`harmonic_direct.rs`)

The refusal of ADR 0491 D2 is lifted. If any part of the solver is `Damping::Viscoelastic`:

1. `model` is checked against the solver: the parts of the solver must be those of the model's materials in order (a
   viscoelastic material of equal weights gives one part, of unequal weights two, any other material one); a model with
   another number of materials is refused (key `"model"`).
2. `sigma_0` is `response.stress(&base_model)`, `base_model` the model with every `Material::Viscoelastic` replaced by its
   elastic base (`Material::base`), by a new `pub(crate)` method `SolidModel::with_viscoelastic_bases` (`model.rs`; the only
   edit of that file, a method that nothing else calls).
3. Per material, the coefficient pair: for a non-viscoelastic material, and for a viscoelastic one of equal weights, `(c, c)`
   with `c` the coefficient of its single part (the stress is `sigma_0 c`, as before); for a viscoelastic one of unequal
   weights `(G*/G_0, K*/K_0)` from its two consecutive parts (the same `coefficients(omega)` as the matrices).
4. Per element and point, for `(cs, ck)` with unequal weights only, per real and imaginary part `s`: `m = (s_0 + s_1 + s_2)/3`,
   `vol = (m, m, m, 0, 0, 0)`, `dev = s - vol`, and the complex stress is `cs (dev_re + i dev_im) + ck (vol_re + i vol_im)`;
   for equal pairs, the product `(re + i im) c` of the existing code.

The code of a model without a viscoelastic material is the code of ADR 0448 amendment 2, operation for operation (the new code
sits in a branch only a viscoelastic part enters; the old path keeps its statements).

### D2. The tests

`crates/rcfd-fem/tests/common/mod.rs` (new, registered here): the rods, the models, the chain with all nodal values, `Checks`,
`worst`, `least`, `base`, `standard`, `star`, `wlf`, factored from `sa_harmonic_visco.rs` for the use of the new tests. The old
test file is not rewritten onto it (rule 2) and keeps its own copies (known duplication, recorded as a remaining question).
`sa_harmonic_visco_stress.rs` (all checks but N) and `sa_harmonic_visco_stress_default_bits.rs` (N1).

### D3. The one superseded old check

`sa_harmonic_visco.rs` RF (vi) asserts that the stress of a viscoelastic response is refused. That is the behaviour F-583
replaces. Its assertion is replaced by a comment naming this ADR and the check S1A of the new test (the one existing test
edit, registered here before it is made); the rest of the file is untouched.

### D4. Documentation

The module doc of `harmonic_direct.rs` and the doc of `stress`; `docs/reference-data.md` gains the F-583 row next to the F-577
rows if the file has them (derived, no source read).

### D5. The bit record of the default stress (B1)

`sa_harmonic_visco_stress_default_bits.rs`, written after this commit and run on the unchanged base aa0e4d5b before any
library code; its digests (FNV-1a 64 over the bits, as ADR 0485 D8) are written into the file then. Content, models without
the viscoelastic material: `DirectHarmonic::stress` of the 40-element bar with `Constant(0.02)` at three frequencies; of the
same bar with a `Table`; of the two-material bar (`Constant` on one set, `Table` on the other) at two frequencies; of the
cantilever with Rayleigh damping and dashpots at a frequency B0 does not use; the refusal message of the stress of a response
of a different size. The B0 file of ADR 0491 (11 digests) must also still pass.

## Checks (limits fixed now)

All errors required finite (a NaN or an infinity is a miss).

| Id | Clause | What | Limit | Estimate behind the limit |
|---|---|---|---|---|
| S1A | a1, a2 | rod A, 40 Hex8, ten `omega_k`: `sigma_xx` at the 320 points vs `M* eps_chain`, pointwise relative, max | `<= 1e-9` | probe `3.2e-12` |
| S1B | a1, a2 | rod B (unequal weights), same | `<= 1e-9` | probe `3.2e-12` |
| S1C | a1, a2 | rod C (shear), `sigma_xy` vs `G* gamma_chain` | `<= 1e-9` | probe `2.1e-12` |
| S2 | a1 | the other components of rods A, B, C (`yy`, `zz` = `(K* - 2 G*/3) eps` for A, B; all others zero), normalised by the largest primary stress of the frequency | `<= 1e-9` | `~1e-11` (same rounding) |
| S3 | a1 (structure) | 8 points per element, the point `x` inside its element's interval, the primary stress equal at the eight points of an element within `1e-12` of its amplitude | as stated | arithmetic 2 |
| SE1 | a3 | rod A, viscoelastic route vs `LossFactor::Table` route, all components and points, normalised by the largest amplitude of the table stress of that frequency, max over ten | `<= 1e-12` | `3e-13` (probe 2) |
| SE2 | a3 | the same, `xx` pointwise relative | `<= 1e-12` | `3e-13` |
| SF1 | a1 (two materials) | the rod of ADR 0491 HD with `Constant(0.03)` on the elastic left half (`nu = 0.3`) and the material of B on the right: `sigma_xx` vs the chain's `Mod_e eps_e`, `Mod_e = M_0 (1 + 0.03 i)` left, `M*` right, pointwise | `<= 1e-9` | `~1e-11` |
| SF2 | a1 (order) | the same with the viscoelastic material on the left and the elastic one on the right | `<= 1e-9` | `~1e-11` |
| Z1 | a1 | rod B at `omega = 0`: `max |sigma_xx A/F - 1|` over the points | `<= 1e-10` | arithmetic 6, `1e-12` |
| T1 | a1 | rod B with a WLF shift at 383 K (`temperature`) vs the unshifted with `tau a_T`, ten `omega_k`: all components normalised by the largest amplitude, max | `<= 1e-12` | arithmetic 7 |
| C0 | a1 | a viscoelastic material without terms vs its base (`nu = 0` rod, `omega` at `0.5` and `1.5` of the base's first resonance, no damping): stress, normalised | `<= 1e-12` | `0` expected (coefficient 1) |
| P1 | a1 | rod B stress at `omega_c`, threads 1 and 4 | bits equal | the stress is deterministic from equal responses |
| NC1 | negative control | rods B and C: the solver's stress vs `wrong_modulus eps_chain` (the one-part route `E*/E_0` on the base modulus), `min` over the ten frequencies of the primary stress's relative difference (max over points per frequency) | `>= 5e-3` | probe `3.4e-2` (B), `9.2e-3` (C) |
| NC2 | negative control | rods B and C: the solver's stress vs the unscaled elastic stress `M_0 eps_chain`, same measure | `>= 4e-2` | probe `6.2e-2` (B), `1.08e-1` (C) |
| RF1 | refusal | the stress of a response of a 4-element rod with the 40-element model: refused (`FieldLength`, key `"displacement"`) | as stated | `response.stress` |
| RF2 | refusal | `stress` of the rod B solver with a model of one more material: refused, key `"model"` | as stated | D1 step 1 |
| N1 | a4 | B1: every digest of the base aa0e4d5b (D5), and B0 of ADR 0491 (11 digests) | equal | D1 |
| N2 | a4 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | no shared output path changed |
| N3 | a4 | `cargo test -p rcfd-fem --release` (all of it) | pass | D1, D3 |

## Planned mutations (on a scratch copy, one at a time, own target directory)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | `cs` and `ck` swapped in the split | S1B, S1C, S2, SF1, SF2 |
| MU2 | the volumetric term dropped (`cs dev` only) | S1B, S2 (rod C has no volumetric stress, S1C does not see it) |
| MU3 | the coefficient applied to the real part only (`(re c.re, im c.re)`) | S1A, S1B, S1C, SE1 |
| MU4 | the part cursor replaced by the material index | SF1, SF2 (the second material takes a wrong part) |
| MU5 | the equal-weight coefficient `E*/E` used for the shear and bulk of unequal weights | S1B, S1C |
| MU6 | `sigma_0` from the model with the viscoelastic material not replaced | every S check (the stress call fails) |
| MU7 | an equal-weight material taken through the split (`c dev + c vol`) | informational: SE1 is expected to move toward `1e-12` (ADR 0491 MU4 saw `1.1e-12` on the response); recorded either way, not part of the decision rule |

## Decision rule

- **done**: every check of the table met (S1A to S1C, S2, S3, SE1, SE2, SF1, SF2, Z1, T1, C0, P1, NC1, NC2, RF1, RF2, N1 to
  N3), every planned mutation MU1 to MU6 caught by at least one check, fmt and clippy clean, B0, B1 and the baseline hashes
  unchanged.
- **partial**: B0, B1 and the baseline hashes unchanged, (a1) and (a2) met (S1A to S1C, S2, SF1, SF2), and a miss of
  SE1/SE2 (a3) diagnosed and recorded as a `*_known_miss` test with its number.
- **todo**: otherwise (in particular any change of the default bits).

## Limits (not covered)

- The isotropic base only (as ADR 0491); solid elements on the elimination path only.
- The stress is the harmonic amplitude at the integration points; no extrapolation to nodes, no fatigue measure (the blade
  fatigue chain that consumes it is not part of this item).
- No stress of the Rayleigh or dashpot terms (as ADR 0448); the Rayleigh `beta K` term takes the instantaneous stiffness (ADR 0491).
- The damped modes still refuse the material (ADR 0491 follow-up 1).
- A model that gives the same material to several element sets is taken per element through its material index (as the matrices).

## Open questions taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. **The old RF (vi) check** (D3): lift it by replacing the one assertion. Taken (the alternative, leaving a failing test, is
   not workable).
2. **Common test module** (D2): the new tests share helpers through `tests/common/mod.rs`; the old file is not rewritten onto it
   (rule 2). Taken.
3. **The pointwise limit for rod A against the table route** is the acceptance's `1e-12`; no looser limit is registered.

## Addendum 1: results (2026-11-14)

Registered run: commit 38323b46 (`cargo test -p rcfd-fem --release --test sa_harmonic_visco --test sa_harmonic_visco_stress --
--nocapture --test-threads=1`), each check run once; no set-up was corrected and no limit was changed. B1 was printed on the
unchanged base aa0e4d5b (`f583-scratch/b1-base.log`) and written into the test at commit b540a91b, before any library code.
`cargo fmt -p rcfd-fem` before commit 38323b46 changed only the layout of the B1 file and of the new test (digests unchanged).

| Id | Limit | Measured | Met |
|---|---|---|---|
| S1A rod A `sigma_xx` against `M* eps_chain`, pointwise, 320 points, ten frequencies | `<= 1e-9` | `3.41e-12` | yes |
| S1B rod B (unequal weights) | `<= 1e-9` | `3.22e-12` | yes |
| S1C rod C (shear, `sigma_xy`) | `<= 1e-9` | `2.09e-12` | yes |
| S2 other components, rods A, B, C | `<= 1e-9` | `6.1e-15`, `1.44e-12`, `2.2e-14` | yes |
| S3 points of an element (8 per element, inside, equal), rods A, B, C | `<= 1e-12` | `2.2e-13`, `2.5e-13`, `1.5e-13` | yes |
| SE1 rod A against the `LossFactor::Table` route, all components, normalised | `<= 1e-12` | `1.50e-13` (0 at six of the ten frequencies) | yes |
| SE2 the same, `xx` pointwise | `<= 1e-12` | `1.57e-13` | yes |
| SF1 two materials, elastic half (loss 0.03) left | `<= 1e-9` | `4.03e-12` | yes |
| SF2 viscoelastic left, elastic half right | `<= 1e-9` | `6.00e-12` | yes |
| Z1 rod B at `omega = 0`, `max |sigma_xx A/F - 1|` | `<= 1e-10` | `3.42e-14` | yes |
| T1 WLF shift at 383 K against `tau a_T` | `<= 1e-12` | `1.09e-14` | yes |
| C0 no terms against the base | `<= 1e-12` | `0` | yes |
| P1 threads 1 and 4 | bits equal | equal | yes |
| NC1 one-part route, min over ten (B, C) | `>= 5e-3` | `3.436e-2`, `9.229e-3` | yes |
| NC2 unscaled elastic stress, min over ten (B, C) | `>= 4e-2` | `6.166e-2`, `1.079e-1` | yes |
| RF1 response of another size | refused, `FieldLength` `"displacement"` | as stated | yes |
| RF2 another material list | refused, key `"model"` | as stated | yes |
| N1 B1 (5 digests of aa0e4d5b) and B0 (11 digests of ADR 0491) | equal | equal | yes |
| N2 `baseline_hashes` | pass | pass (3 passed, 1 ignored) | yes |
| N3 `cargo test -p rcfd-fem --release` (all result lines, doc tests included) | pass | 683 passed, 0 failed, 170 ignored (146 result lines) | yes |

The probes predicted `3.2e-12` (A, B), `2.1e-12` (C) for S1 and `1.5e-13` for the strain difference to the table route; the
measured numbers agree. SE1 is not zero at four of the ten frequencies for the reason of ADR 0491 E1 (the two routes round
`E*/E` and `(s, s eta)` differently).

**Mutations** (`f583-scratch/mutate.py` on a git-archive copy of 38323b46 with its own target directory, log
`f583-scratch/mutations.log`; MU0, the copy without a mutation, passes 4 of 4):

| Id | Caught by |
|---|---|
| MU1 `cs` and `ck` swapped | S1B (`1.9e-2`), S1C (`8.1e-2`), S2 (`6.3e-2`), SF1, SF2, Z1 (`2.6e-2`), and NC2 for rod C (`3.4e-2` against its limit `4e-2`) |
| MU2 volumetric term dropped | S1B (`0.63`), S2, SF1, SF2, Z1; S1C does not (no volumetric stress), as registered |
| MU3 coefficient on the real part only | S1A (`5.0e-2`), S1B, S1C, S2, SE1 (`5.0e-2`), SE2, SF1, SF2 |
| MU4 part index by the material index | SF2 (refused by the model check: the second material meets the bulk part); SF1 passes (the two indexings coincide when the elastic material comes first) |
| MU5 `cs` on the volumetric part | S1B (`4.8e-2`), S2, SF1, SF2, Z1; S1C does not (no volumetric stress in shear) |
| MU6 model with the viscoelastic material not replaced | every S, SE, SF and NC test (the recovery refuses the material) |
| MU7 equal-weight material through the split | not caught: all four tests pass (informational, outside the decision rule) |

MU1 to MU6 are each caught by at least one check. Two statements of the registered table were wrong about which checks catch
a mutation, not about whether it is caught: MU4 (SF1 does not, SF2 does) and MU5 (S1C does not: the shear rod has no
volumetric stress, so the error shows in S1B, SF1, SF2 and Z1). MU7 shows the split of an equal-weight material to be
indistinguishable from the product at `1e-12` in these runs.

**Deviations.** (1) The roadmap entry of F-583 does not exist on the base aa0e4d5b (it was added to main afterwards, at
58c660b5), so there is no entry to update on this branch and none was added; the lead applies the note below. (2) The one
registered edit of an existing test (D3): the RF (vi) assertion of `sa_harmonic_visco.rs` replaced by a comment; the file's
other tests pass unchanged. (3) The mutation table named S1C for MU5 and SF1 for MU4 in error (above). Not run: the whole
workspace suite (the lead's gate).

**Shared solver files changed.** `harmonic_direct.rs`: module doc; `DirectHarmonic::stress` (the viscoelastic early return
replaces the refusal of ADR 0491 D2; the path of the other models is unchanged); new private `stress_viscoelastic`. `model.rs`:
new `pub(crate)` `SolidModel::with_viscoelastic_bases` (called by nothing else). Tests: new `tests/common/mod.rs`,
`sa_harmonic_visco_stress.rs`, `sa_harmonic_visco_stress_default_bits.rs`; `sa_harmonic_visco.rs` (one comment in place of
RF (vi)). `docs/reference-data.md`: the F-583 rows.

Other runs: `cargo clippy -p rcfd-fem --all-targets --release -- -D warnings`: clean; `cargo fmt -p rcfd-fem --check`: clean;
`cargo test -p rcfd-api --release --test baseline_hashes`: pass; `cargo test -p rcfd-cli --release --test docs_site`: 2 passed.

**Decision by the registered rule: done.** Every check of the table is met, MU1 to MU6 are each caught, B0, B1 and the
baseline hashes are unchanged, fmt and clippy are clean.

### Remaining questions

1. **Duplicated test helpers.** `sa_harmonic_visco.rs` keeps its own copies of the rods, the chain and `Checks`. Options: (a)
   leave them (rule 2); (b) rewrite the old file onto `tests/common`. Recommendation: (a) now, (b) when the maintainer allows an
   edit of an existing test file.
2. **Shells and beams** (structural elements without points in `IntegrationPointField`) and orthotropic bases are not taken;
   the isotropic solid base only. Recommendation: no item until a fatigue case needs it.
3. **Fatigue use.** The stress amplitude per component is given; combining it into an equivalent stress for a blade fatigue
   run is a separate item. Recommendation: open it when the fan plan calls for it.

## For the lead: roadmap entries

Proposed status of F-583: **done**. Proposed note:

> ADR 0494, 2026-11-14: DirectHarmonic::stress (harmonic_direct.rs) of a response with a viscoelastic material returns (G*/G_0) dev sigma_0 + (K*/K_0) vol sigma_0 at each integration point, sigma_0 the elastic stress of the base (computed on the model with each viscoelastic material replaced by its base, SolidModel::with_viscoelastic_bases), and E*/E_0 times sigma_0 for equal weights; the refusal of ADR 0491 D2 is lifted. Met: on the rods A, B and C of ADR 0491 at 40 elements and ten frequencies, the axial (or shear) stress at all integration points equals the modulus times the strain of the discrete chain within 3.4e-12 (A), 3.2e-12 (B) and 2.1e-12 (C) pointwise against the limit 1e-9, the other components within 1.4e-12; for equal weights the stress equals that of the LossFactor::Table route within 1.5e-13 against 1e-12; two materials in either order within 6e-12; zero frequency, shift and no-term checks within 3.4e-14, 1.1e-14 and 0; threads bit-identical; mutations MU1 to MU6 caught; default bits (B1, 5 digests, and B0 of ADR 0491) and baseline hashes unchanged. The refusal assertion RF (vi) of sa_harmonic_visco.rs is replaced by a comment (ADR 0494 D3).
