# ADR 0512: F-561 registration: thick sphere with J2 on a 4 x 4-per-face cubed-sphere octant, residual hoop stress after unloading

Status: **REGISTERED 2026-11-14, written before any code of F-561 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-561 was written into the repository. A scratch probe (`w49i-work/f561-scratch/
f561_probe.rs`, a copy of the sphere builder and reader of ADR 0475 with the cubed-sphere density as a parameter, no new
library code) was run on the base to size the mesh and the elastic Lame scatter; its numbers are disclosed under "Arithmetic
done beforehand" and are not a registered run. Results go in the addenda at the end; nothing above the addenda is changed after
a run.

Roadmap item F-561 (deps F-422), the follow-up 2 of ADR 0475. Number 0512 given by the lead. Branch `f561-work` from main
6c4ba2d9, worktree `w49i-work/f561`, scratch `w49i-work/f561-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no
unsafe code; no case-file key; clean-room. This is a test item: no library file is edited.

## What F-561 asks

The acceptance of the roadmap entry, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. The partly plastic sphere of ADR 0475 on a 4 x 4-per-face cubed-sphere octant with 8 radial Hex20 layers: the residual hoop stress at the bore after unloading from p(1.25 a) within 2 percent of sigma_y - (12/7) p, the elastic Lame hoop stress of the same mesh at every point of the inner layer within 1 percent, and S1, S2 of ADR 0475 still within 1 percent.

Three clauses: (a1) residual hoop stress at the bore, 2 percent; (a2) elastic Lame hoop stress at every point of the inner
layer, 1 percent; (a3) S1 and S2 of ADR 0475, 1 percent.

## What was read

- ADR 0475 (F-422): D5 (the sphere: `a = 1 m`, `b = 2 m`, `E = 2e11`, `nu = 0.3`, `sigma_y = 2e8`, perfectly plastic; octant with
  normal displacement fixed on the three symmetry planes; cubed sphere from the faces of the unit cube split `n x n`, mapped
  by `p -> p |p|_inf / |p|_2`, 8 radial layers, Hex8 cells upgraded to Hex20 by `upgrade_to_hex20_mapped` with that map; bore
  pressure `Load::pressure`; S4 a driver contribution `-g(lambda) f1`, `g = 1 - |2 lambda - 1|`, 16 steps, from `p1 = p(1.25 a)`),
  its arithmetic 3 and 4, S1, S2, C1, S4 and its diagnosis: S4 missed on the 2 x 2 octant (`-0.612380 sigma_y`, `2.627e-2`
  against 2e-2); the elastic Lame field of that mesh already scattered by `-1.4 .. +1.0` percent in the hoop stress; remedy
  "a finer octant" = this item.
- `crates/rcfd-fem/tests/sa_j2.rs`: the sphere builder (`sphere`, `SPHERE_N = 2`), the reader `sphere_points`, `Ramp`, the S4
  fit and the S1, S2, C1 rows. The builder is test code there; `rcfd_fem::builders` has no cubed-sphere generator (searched:
  `cubed` occurs only in that test). Taken: the generator is added in the new test file (`sa_j2_f561.rs`) with the density `n`
  as a parameter, not to `builders` (a library addition is not asked for by a test item and would need its own default-bits
  record).

## Arithmetic done beforehand (disclosed)

1. **The closed form, by hand (checked, not trusted).** Spherical symmetry gives `sigma_theta = sigma_phi`; Tresca and von
   Mises coincide, `sigma_eq = |sigma_theta - sigma_r|`. Loaded, perfectly plastic zone `a < r < c`: `sigma_theta - sigma_r =
   sigma_y` (hoop tension, so the flow gives `sigma_theta > sigma_r`); equilibrium `d sigma_r/dr = 2 sigma_y / r`; elastic
   shell `c < r < b`, Lame; the pressure `p(c) = 2 sigma_y ln(c/a) + (2 sigma_y/3)(1 - c^3/b^3)` (ADR 0475 arithmetic 3).
   For `a = 1`, `b = 2`, `c = 1.25`: `2 ln 1.25 = 0.4462871`, `(2/3)(1 - 1.953125/8) = 0.5039063`, so `p1 = p(1.25 a) =
   0.9501934 sigma_y`. At the bore the loaded state is `sigma_r = -p1`, `sigma_theta = sigma_r + sigma_y = sigma_y - p1`.
   Elastic unloading by `-p1` changes the stress by the Lame field of `-p1` (thick sphere, internal pressure `p`, outer
   surface free): `sigma_r = -p (b^3/r^3 - 1)/(b^3/a^3 - 1)`, `sigma_theta = p (b^3/(2 r^3) + 1)/(b^3/a^3 - 1)`; with
   `b^3/a^3 = 8`: `sigma_theta,el(r) = (p/7)(1 + 4 a^3/r^3)`, `sigma_theta,el(a) = (5/7) p`, `sigma_theta - sigma_r =
   (12/7) p a^3/r^3`. Residual `sigma_theta(a) = (sigma_y - p1) - (5/7) p1 = sigma_y - (12/7) p1`. **The item's formula is exact
   for `b/a = 2`** (in general the coefficient is `3 b^3/(2 (b^3 - a^3))`, which is `12/7` only at `b/a = 2`) **and for this
   `p1`**, in the small-strain theory, provided the unloading is elastic: the residual `sigma_theta - sigma_r` at the bore is
   `sigma_y - (12/7) p1 = -0.628903 sigma_y`, magnitude below `sigma_y`, so no reversed yielding (it would need `p1 > (7/6)
   sigma_y`). Numerically `(12/7) * 0.9501934 = 1.628903`; closed value `-0.628903 sigma_y` (the value of ADR 0475). The formula
   needs no correction; no alternative formula is tested. The closed residual field is `sigma_theta,res(r) = sigma_y - p1 + 2
   sigma_y ln r - (p1/7)(1 + 4/r^3)` for `a <= r <= c` (the plastic zone of the loading, which contains the whole inner layer).
2. **Mesh.** `n = 4`: `3 n^2 = 48` Hex20 elements per radial layer, 8 layers, 384 elements; inner layer = layer 0 (radii
   `[1, 1.125]`), the elements `0 .. 47`. Probe on the base: 384 elements, 2009 nodes; octant volume `3.664828` against
   `7 pi / 6 = 3.665191` (relative `9.9e-5`; the 2 x 2 octant `1.5e-3`).
3. **Elastic Lame scatter of this mesh (probe).** J2 with `sigma_y = 1e30` (never yields), bore pressure `p1`, one step, hoop
   stress of the 1296 integration points of the inner layer against `(p1/7)(1 + 4/r^3)` at the point's reference radius,
   relative to the local value: **largest `1.162e-2`**, mean `6.96e-3`, signed: `+0.1 .. +1.2` percent in the first third of
   the layer, `-1.0 .. -0.6` percent in the middle third, `+0.4 .. +0.7` percent in the last third (the 2 x 2 octant:
   largest `2.73e-2`, mean `9.6e-3`). The systematic part (the sign pattern over the three Gauss radii of a layer, about
   `0.7` percent, almost unchanged from `n = 2` to `n = 4`) is the radial resolution of 8 Hex20 layers; the spread within a
   third is angular. The refinement the item asks for (angular only, the 8 layers kept by the item) therefore cannot be
   expected to bring the pointwise scatter far below the limit of (a2): the probe says (a2) will be close to or above 1
   percent. The limit stays the item's; a miss is recorded, not tuned.
4. **Fit alone.** ADR 0475 addendum: the quadratic fit of the exact residual field over the Gauss radii of the inner layer
   `[1, 1.125]` gives the bore value within `7.4e-4` relative. The residual is a difference of two numbers of the size of
   `sigma_y`, so a pointwise error of `0.7` percent in each hoop field is about `4.5e-3 sigma_y`, `0.7` percent of the
   residual `0.629 sigma_y`, per field; the fit extrapolates to the layer's face. Estimate for (a1): `0.5 .. 2.5` percent
   against the limit 2 percent (the 2 x 2 octant gave 2.6). Not known beforehand.

## Design

The new test file `crates/rcfd-fem/tests/sa_j2_f561.rs`; no library file is touched, so no default-bits record is added (the
rcfd-api `baseline_hashes` test and the whole rcfd-fem suite are still run).

- **Octant** `sphere(n, mat, p)`: the builder of ADR 0475 D5 with the density `n` (a cubed sphere: faces `x = 1`, `y = 1`, `z =
  1` of the unit cube split `n x n`, radius `r_k = 1 + k/8`, point `r q`, Hex8 upgraded with the map `p |p|_inf / |p|_2`),
  surface set "bore" = face 0 of the elements of layer 0, node sets on the three symmetry planes with normal displacement 0.
- **Material** `Material::j2(isotropic(E = 2e11, nu = 0.3), sigma_y = 2e8, 0, 0)`; the never-yielding control `sigma_y = 1e30`.
- **Stress recovery (registered reading).** At every integration point of the Hex20 stiffness rule (27 per element) of the
  inner layer: the second Piola-Kirchhoff stress `element_second_piola_kirchhoff(converged displacement, committed state)`,
  the hoop stress `sigma_theta = (tr S - n.S.n)/2` with `n` the unit vector of the point's reference position, the radius
  `r` the norm of the reference position. No nodal averaging, no smoothing. The difference of this stress measure to the
  Cauchy stress is of the order of the strain (`<= 3e-3`), disclosed; the probe of arithmetic 3 contains it.
- **Residual at the bore (a1)**: the loaded path of ADR 0475 S4 (`Ramp`, 16 steps, 8 up and 8 down, from `p1 = p(1.25 a)`,
  the dead load of `Load::pressure` as a nodal vector, driver `solve_with`); the hoop stress of all 1296 points of the
  inner layer least-squares fitted by `c0 + c1 t + c2 t^2`, `t = r - 1.125`, evaluated at `r = 1` (the reading of ADR 0475
  S4, unchanged); compared with `sigma_y - (12/7) p1`.
- **Elastic Lame (a2)**: the elastic material under `Load::pressure(p1)`, `solve` with one step; every one of the 1296 points
  against `(p1/7)(1 + 4/r^3)`, error relative to the local Lame value; the maximum over the points, computed so that a NaN
  is not hidden (the test asserts every aggregated number finite before comparing).
- **S1, S2 (a3)**: as ADR 0475 on this mesh: the loading to `p(1.25 a)` (8 steps) and `p(1.5 a)` (10 steps) with
  `Load::pressure`; `c_FE` the mean of `r (sigma_eq/sigma_y)^(1/3)` over the points that have not yielded (`alpha = 0`), the
  error `|p(c_FE) - p|/p`. C1 as a companion: no point with `r < c - 0.05 a` elastic, none with `r > c + 0.05 a` yielded.

## Checks (limits fixed now)

| Id | Clause | Check | Limit | Estimate behind it |
|---|---|---|---|---|
| M1 | mesh | `n = 4`: 384 elements, 2009 nodes; octant volume (sum of the weights) vs `7 pi/6` | counts exact; volume `<= 5e-4` relative | probe `9.9e-5` (5 times margin) |
| R1 | a1 | residual hoop stress at the bore (fit, above) vs `sigma_y - (12/7) p1 = -0.628903 sigma_y`, `abs(FE - closed) / abs(closed)` | `<= 2e-2` | item; arithmetic 4 (0.5 .. 2.5 percent) |
| E1 | a2 | elastic hoop stress of every point of the inner layer vs Lame, the largest `abs(hoop - Lame) / Lame` over the 1296 points | `<= 1e-2` | item; the probe measured `1.162e-2` (arithmetic 3): expected to miss; the number is recorded |
| S1 | a3 | `c = 1.25 a`: `abs(p(c_FE) - p) / p` | `<= 1e-2` | item; ADR 0475 `7.8e-4` on the 2 x 2 octant |
| S2 | a3 | `c = 1.5 a`: same | `<= 1e-2` | item; ADR 0475 `4.0e-4` |
| C1 | a3 comp. | points wrong at S1 and S2 (`r < c - 0.05 a` elastic, `r > c + 0.05 a` yielded) | 0 | ADR 0475 0 of 2592; the radial layers are unchanged |
| F1 | all | every aggregated error finite (R1, E1, S1, S2 and the diagnostics below), the S4 solve and the loading solves return `Ok` | all | an `f64::max` over NaN would hide a failure |

Recorded without a limit (printed in the run, quoted in the addendum, not checks): the loaded fit at the bore against
`sigma_y - p1`; the elastic fit at the bore against `5/7 p1`; the largest pointwise error of the unloaded hoop against the
closed residual field over the inner layer; the mean error of E1; the errors of R1 and E1 on the 2 x 2 octant (N1); the number
of load steps and the largest alpha of the inner layer.

### Negative controls

| Id | Control | Expected |
|---|---|---|
| N1 | the same code on `n = 2` (the octant of ADR 0475): the residual at the bore reproduces ADR 0475 S4, `-0.612380 sigma_y` (6 digits quoted; limit `2e-5 sigma_y`); the E1 largest error on `n = 2` is above `1e-2` (probe `2.73e-2`); the R1 error on `n = 2` is above `2e-2` (ADR `2.627e-2`) | all three: the reader reproduces the old one and the limits discriminate a coarse mesh |
| N2 | the residual is not the loaded hoop: `abs(FE residual - (sigma_y - p1)) / sigma_y` (the loaded closed value at the bore, no unloading) | `>= 0.5` (the closed gap is `0.679`) |
| N3 | the unloading leaves nothing without plasticity: the elastic material through the same `Ramp` path: the largest absolute hoop stress of the inner layer at the end | `<= 1e-6 sigma_y` (Newton tolerance `1e-10`) |
| N4 | the unloading is elastic (no reversed yielding): the equivalent plastic strain of every inner-layer point after the unloading equals that after the loading (an up-only run `g = lambda`, 8 steps, the same increments as the up leg of the Ramp), the largest difference | `<= 1e-12` |

### Planned mutations (on a scratch copy, one at a time, each run once after the registered run)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MT1 | the closed form `10/7` for `12/7` | R1 (`p1/7` is 0.136 sigma_y: 22 percent) |
| MT2 | the reader takes `sigma_rr` for the hoop stress | R1, E1, N2 or N3 |
| MT3 | the fit evaluated at `r = 1.0625` (mid-layer) | R1 |
| MT4 | the unloading leg removed (`g = lambda`, load only) | R1, N2 |
| MT5 | the library: `alpha` not updated in the return map (MU4 of ADR 0475) | S1, S2, C1 |
| MT6 | the library: the driver keeps the committed states (MU5 of ADR 0475) | S1, S2, R1 |
| MT7 | the library: the yield stress used in the return map `1.05 sigma_y` | S1, S2, R1 |

MT1 to MT4 are test-file mutations; MT5 to MT7 edit `mat_j2.rs` and `nonlinear.rs` in a scratch copy with its own target
directory; the worktree's library is never edited. A mutation not caught is reported.

## Decision rule

- **done**: R1, E1, S1, S2, C1, F1 and M1 met, N1 to N4 as expected, every planned mutation caught by at least one check,
  fmt and clippy clean, the rcfd-fem suite, the baseline hashes and the docs-site test passing.
- **partial**: S1, S2, C1, F1, M1 and N1 to N4 met and a miss of R1 or E1 diagnosed and recorded as a `*_known_miss` test
  with the limit intact.
- **todo**: otherwise (in particular a changed default bit).

## Limits (not covered)

- One mesh density of the item (`n = 4`, 8 layers) and the `n = 2` control; no convergence study in `n` or in the number of
  radial layers (the probe says the radial layers set a floor of about 0.7 percent of the hoop scatter at 8 layers).
- Small-strain closed forms against the total-Lagrangian driver (Green-Lagrange, second Piola-Kirchhoff): the difference is of
  the order of the strain.
- Only the unloading from `p(1.25 a)`, which is elastic; a reversed-yield unloading (from `p(1.5 a)`) has no closed form here.
- Perfect plasticity only (`H_iso = H_kin = 0`); no case-file key.

## Open questions

None taken beforehand. The two readings that matter are fixed above (pointwise relative error against the local Lame value;
the S4 fit of ADR 0475 unchanged), on the item's own words and on ADR 0475; the alternatives appear in the addendum only as
diagnostics without a limit.

## Addendum 1: results (2026-11-14)

Registered run: `f561_registered_run` (now an ignored record, test file commit 6744a782), made once, log
`f561-scratch/registered_run.log`; the library is unchanged (no file under `crates/rcfd-fem/src` edited), so no default-bits
record was needed.

| Id | Check | Limit | Measured | Met |
|---|---|---|---|---|
| M1 | 384 elements, 2009 nodes; volume vs `7 pi/6` | exact; `5e-4` | exact; `9.92e-5` | yes |
| R1 | residual hoop stress at the bore vs `sigma_y - (12/7) p1 = -0.628903 sigma_y` | `2e-2` | `-0.611919 sigma_y`, `2.7005e-2` | **no** (known miss) |
| E1 | elastic hoop vs Lame, largest relative error over the 1296 points of the inner layer | `1e-2` | `1.1623e-2` (mean `6.96e-3`) | **no** (known miss) |
| S1 | `c = 1.25 a` | `1e-2` | `7.97e-5` | yes |
| S2 | `c = 1.5 a` | `1e-2` | `1.02e-3` | yes |
| C1 | points wrong at S1, S2 | 0 | 0 of 10368 each | yes |
| F1 | every aggregate finite, the solves `Ok` | all | all (`max_finite` panics on a NaN) | yes |
| N1 | `n = 2`: bore residual vs ADR 0475 S4; R1 error above 2e-2; E1 error above 1e-2 | `2e-5`; above; above | `-0.612380` (diff `4.1e-7`); `2.627e-2`; `2.730e-2` | yes |
| N2 | gap of the residual to `sigma_y - p1` | `>= 0.5` | `0.662` | yes |
| N3 | elastic material through the Ramp, largest hoop after the unloading | `1e-6 sigma_y` | `2.5e-176` | yes |
| N4 | alpha after unloading vs after loading, inner layer | `1e-12` | `0` (bit equal) | yes |

Met rows continue in `f561_met_rows` (104 s in release); the two misses in `r1_residual_hoop_stress_known_miss` and
`e1_elastic_lame_known_miss` (ignored, limits intact). Recorded diagnostics (no limit): the loaded fit at the bore `0.077015
sigma_y` against `sigma_y - p1 = 0.049807`; the elastic fit at the bore `0.724774 p1` against `5/7 = 0.714286`; the largest
pointwise error of the unloaded hoop against the closed residual field `1.159e-2 sigma_y`; largest alpha of the inner layer
`1.221e-3` (strain scale of the arithmetic 3); 16 increments.

**Diagnosis.** Refining the angular resolution from 2 x 2 to 4 x 4 per face did not move the misses (R1 `2.627e-2` to
`2.700e-2`, E1's mean `9.6e-3` to `7.0e-3`, its largest `2.73e-2` to `1.16e-2`): at 8 radial layers the error is radial. The
fit at the bore is `+1.5` percent high in the elastic field (`0.7248` against `0.7143`) and `+0.027 sigma_y` high in the loaded
plastic field; the residual is their difference, `0.017 sigma_y` off. Diagnostic run `diag_radial_layers` (ignored, no limit,
not a registered check): the same readings with 16 radial layers: n = 4: residual at the bore `-0.626058 sigma_y` (error
`4.52e-3`), pointwise residual error `5.86e-3 sigma_y`, elastic largest `9.30e-3`, mean `2.44e-3`; n = 2: residual error
`4.20e-3`, elastic largest `2.89e-2` (angular). So with 16 layers and 4 x 4 both clauses would be met by this reading (R1
with a 4.4 times margin, E1 with 7 percent); with 8 layers neither is, by 1.35 and 1.16 times. The formula `sigma_y - (12/7)
p` is exact (arithmetic 1), so the miss is the discretisation of the item's 8 radial Hex20 layers (three Gauss radii per layer
oscillating around the `1/r^3` field), not the closed form and not the material.

**Mutations** (`f561-scratch/mutate.py`, on a git-archive copy of commit 6744a782 with its own target directory, log
`mutations.log`, the registered run each time; MT0 reproduces the two misses and nothing else):

| Id | Caught by (beyond the two misses) |
|---|---|
| MT1 `10/7` | R1 (`7.12e-1`; the registration's "22 percent" divided by the wrong closed value) |
| MT2 hoop = `sigma_rr` | R1 (`1.008`), N2 (`4.5e-2`), E1 (`2.37`), N1 (`6.2e-1`) |
| MT3 fit at `r = 1.0625` | R1 (`3.20e-1`), N2 (`0.478`), N1 (`0.185`) |
| MT4 loading only | R1 (`1.12`), N2, N3 (`0.66`), N1 |
| MT5 alpha not updated | S1 (`4.07e-2`), S2 (`9.08e-2`), C1 (2160 and 4752 wrong); not R1 (its path stays elastic after the first yield step) |
| MT6 committed states kept | R1 (`1.0`), N2, S1, S2, C1, N1 |
| MT7 yield `1.05 sigma_y` | R1 (`1.05e-1`), S1 (`2.05e-2`), S2 (`3.09e-2`), N1 |

Every planned mutation was caught.

**Other runs.** `cargo fmt -p rcfd-fem --check` clean; `cargo clippy -p rcfd-fem --all-targets -- -D warnings` clean (one unused
field of the test reader removed after the run, commit 980b7e98; no behaviour changed). Full `cargo test -p rcfd-fem --release`,
the rcfd-api `baseline_hashes` test and the docs-site test: see the lead's report.

**Decision by the registered rule: partial.** S1, S2, C1, F1, M1 and N1 to N4 are met, R1 and E1 are diagnosed misses recorded as
`*_known_miss` tests with the limits intact; all mutations caught; the library and the baselines unchanged.

**Open questions.**
1. **The 8 radial layers of the item.** Options: (a) accept partial; (b) a follow-up on 4 x 4 per face with 16 radial layers
   (measured here as a diagnostic: R1 `4.5e-3`, E1 `9.3e-3`), registered as its own item with the same limits.
   Recommendation: (b), taking (a) for F-561 itself.
2. **Reading of E1.** The pointwise error relative to the local Lame value is the strict reading. Relative to the bore value
   `5 p1/7` the largest error would be about `1.0e-2` (estimated from the signed bins, not measured as a check): not offered as
   a pass. Recommendation: keep the strict reading.

## For the lead: roadmap entries

Status proposed for F-561: partial.

> ADR 0512, 2026-11-14: PARTIAL by the registered rule. The octant of ADR 0475 on 4 x 4 cells per face with 8 radial Hex20 layers (a test-side cubed-sphere generator, crates/rcfd-fem/tests/sa_j2_f561.rs; no library change, the closed form sigma_y - (12/7) p1 is exact for b/a = 2 and was derived by hand in the registration): S1 8.0e-5 and S2 1.0e-3 (limit 1e-2), C1 0 wrong, controls N1 to N4 and 7 mutations met. Residual hoop stress at the bore missed (-0.611919 sigma_y against -0.628903, 2.70e-2 > 2e-2; the 2 x 2 octant 2.63e-2) and the elastic Lame hoop stress at every point of the inner layer missed (largest 1.162e-2 > 1e-2): ignored *_known_miss tests, limits intact. Diagnosed: refining the angles does not change them, the error is the radial resolution of 8 Hex20 layers; diagnostic with 16 layers: residual 4.5e-3, elastic largest 9.3e-3.

Follow-up (proposed, not added): Title: "Thick sphere with J2: residual hoop stress after unloading with 16 radial Hex20 layers". Deps: F-561. Acceptance:

> Every limit not stated here is registered in the ADR before the run. The partly plastic sphere of ADR 0475 on a 4 x 4-per-face cubed-sphere octant with 16 radial Hex20 layers: the residual hoop stress at the bore after unloading from p(1.25 a) within 2 percent of sigma_y - (12/7) p, the elastic Lame hoop stress at every integration point of the inner layer within 1 percent of the local value, S1 and S2 within 1 percent, with the 8-layer misses of ADR 0512 kept as a negative control.
