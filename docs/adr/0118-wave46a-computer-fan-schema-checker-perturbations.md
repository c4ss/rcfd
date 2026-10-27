# ADR 0118: Wave 46A, computer-fan parameter schema, pre-meshing constraint checker and manufacturing perturbations

Status: **ACCEPTED (registered 2026-10-26)**. The limits and decision rules below were fixed before any registered test ran. The
house rules of ADRs 0110-0117 apply:
* a missed limit becomes an ignored `*_known_miss` test with the limit intact;
* a defective set-up gets a new corrected test, registered here as an amendment before it runs;
* defaults stay bit-identical, and `crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded.

Written at main 9fbedff (worktree `<scratch dir>`, branch `w46a`, target `<target dir>`). It serves
parts A1, A2 and A5 of `docs/fan-project-breakdown.md` for an **axial** computer fan. Clean-room: first principles, our own ADRs
and standard statistics. Parents: ADR 0065 (blade geometry), ADR 0067 (seeded generator), ADRs 0068/0083 (C-grid).

No CFD is run. Every threshold of the checker is a **process input**, not physics (see "Checker thresholds"). The tests show that
the checker applies the thresholds and locates the violation. They do not show that the thresholds are right for any real mould
or fan.

## What exists (re-checked against the source)

* `rcfd_mesh::fan::AxialFan` (ADR 0065) has:
  * blade count, hub and tip radius, tip clearance, the domain length, the root leading-edge position and the section family
    (NACA 4-digit or double circular arc);
  * any number of radial stations. Each station has chord, stagger, camber, camber position, thickness, axial sweep and
    tangential lean;
  * located validation (`FanError`).
* Nothing serialises. No frame, motor hub, density, roughness or tolerance exists. There is no checker beyond the parameter
  validation, and no RNG in the mesh crate.
* The seeded generator `Rng` (xoshiro256**, ADR 0067) lives in `rcfd_api::design::rng`. `rcfd-mesh` cannot depend on `rcfd-api`.

## Decisions

1. **One schema, `rcfd_mesh::fan_design::ComputerFan`** (serde, `deny_unknown_fields`). It contains:
   * the rotor: `n_blades`, `r_hub`, `r_tip`, `tip_gap`, `z_le_root`, `family`;
   * spanwise control points `stations[]`: `span` fraction, `chord`, `stagger_deg`, `camber`, `camber_pos`, `thickness`, `sweep`
     and `skew_deg`. Skew is the circumferential position of the leading edge, positive toward the rotation; it maps to
     `lean = r skew`. The variation between control points is linear;
   * `hub` (motor hub): `z_start`, `length`;
   * `frame`: square `side`, `depth`, `inlet_bell_radius`, `n_struts`, `strut_width` and `strut_depth`, with the struts at the
     outlet face;
   * `density`, the nominal blade `roughness_ks`, and an optional `deviation`, which holds the manufacturing errors of one
     specimen (below).
2. **`PARAMS`** gives every scalar a path, an SI unit, inclusive bounds, a default, and a flag `meshed` saying whether the existing
   sector generators realise it. It serialises, so the schema itself can be exported.
   * The bounds are the 40 to 140 mm computer-fan envelope of the breakdown section 1, plus the ADR 0065 validation ranges.
   * The default is an illustrative 120 mm 7-blade fan. It is not a measured fan.
3. **`ComputerFan::axial_fan(k, inlet, outlet)`** gives blade `k` as the `AxialFan` the generators take. Its deviations are
   applied: stagger plus the common and per-blade errors, chord times `1 + chord_rel`, and absolute thickness times
   `1 + thickness_rel`. The tip clearance is `tip_gap + tip_gap_delta`.
4. **`ComputerFan::check(&Limits)`** returns `Vec<Violation>`. Each violation has:
   * a `Code` (snake_case in JSON);
   * a parameter `path`: for station-wise checks, the nearest control point;
   * the `blade` for per-blade specimens, and the `radius`;
   * the `value`, the `limit` and a message.

   Bounds and structure are checked first. If they fail, the geometric checks are not run. The geometric checks are made at
   `n_radii` = 21 uniform radii plus the control-point radii, on every distinct blade, against its +theta neighbour.
5. **`Dist`** is one of `fixed`, `uniform` or `truncated_normal`. Truncated means renormalised, not clamped. Each has an analytic
   CDF, mean and variance. Sampling uses one uniform per draw: the truncated normal inverts its CDF by bisection on a series /
   continued-fraction `normal_cdf`.
   * **`Tolerances`** holds one `Dist` per operator.
   * **`ComputerFan::ensemble(tol, n, seed)`** draws `n` specimens in a fixed order:
     * per specimen: common stagger, tip-gap error, eccentricity, eccentricity direction (uniform on [0, 360) deg);
     * then per blade: stagger, chord, thickness, mass, roughness.
6. **`ComputerFan::static_unbalance()`** is `U = sum_k m_k c_k` in the rotor plane [kg m].
   * `m_k = density * V_k * (1 + mass_rel_k)`. The mass error is spread uniformly over the blade.
   * `V_k` and `c_k` come from the closed triangulation of blade `k` at `MASS_RESOLUTION` = (96, 48), through a new
     `BladeSurface::centroid` (the first moment of the origin tetrahedra), rotated to `2 pi k / N`.
   * The hub is taken as axisymmetric and balanced.
   * The eccentricity of the axis in the bore changes the tip gap, not the mass distribution about the axis, so it does not
     enter `U`.
7. **`Rng` moves to `rcfd_core::rng`**, byte for byte, and `rcfd_api::design::rng` re-exports it. This adds no new dependency and
   no second generator.

## The checker (A2): codes and first-principles statements

| Code | Test |
|---|---|
| `out_of_bounds` | A scalar outside its `PARAMS` bounds, or not finite. |
| `invalid` | Structure: the first control point is at span 0 and the last at span 1; `deviation.blades` is empty or has `n_blades` entries; the `AxialFan::validate` rules of every realised blade apply, with the paths renamed to the schema. |
| `min_thickness` | Absolute maximum thickness `chord * thickness < min_wall`. |
| `tip_gap` | The smallest gap `tip_gap + tip_gap_delta - eccentricity` is below `min_tip_gap`, or the largest gap over `r_tip` is above `max_tip_gap_rel`. |
| `tip_rub` | The smallest gap is not positive. Only reachable through eccentricity: a non-positive concentric gap is already `invalid` (the ADR 0065 rule `tip_clearance > 0`). |
| `hub_tip_ratio` | `r_hub / r_tip` outside the range. |
| `solidity` | `c N / (2 pi r)` outside the range. |
| `collision` | The section polygons (64 cosine-spaced points per side, unrolled plane `(z, s = r theta)`, a cylinder isometry) of neighbouring blades intersect, or one contains the other. |
| `passage_gap` | Their distance is below `min_passage_gap`. |
| `axial_overlap` | Mould: the s-ranges of neighbouring sections overlap. |
| `undercut` | Mould: an axial line crosses one section more than twice. |
| `draft` | Mould: `asin |n_z|` of the 3-D surface normal is below `min_draft_deg` somewhere inside `[edge_band, 1 - edge_band]` of the chord, on either side, at a checked radius. The root radius `r_hub` is included. |
| `frame_wall` | `side / 2 - (r_tip + gap) < min_wall`, the thinnest frame wall at mid-side. |
| `axial_fit` | The blade leaves `[0, depth - strut_depth - min_axial_gap]`, or the hub ends past `depth`. |
| `root_off_hub` | The root section leaves `[hub.z_start, hub.z_start + hub.length]`. |

Mouldability from first principles assumes an **axial two-plate mould**: two halves pulled along the rotation axis, with no side
actions and no rotating cores.
* A face can be released only by the half it faces. Every axial line must therefore meet the moulded part in one interval:
  * if a line crosses one section more than twice, the steel between the crossings is trapped (`undercut`);
  * if a line crosses two neighbouring blades, the steel between them can be pulled neither way (`axial_overlap`).
* A face parallel to the pull scrapes on release, so it needs a draft `asin |n . e_z| > 0`.
* The parting line runs along the leading and trailing edges, where the draft must pass through zero. The edge band (5 % of
  chord) is that strip. The `undercut` test covers the whole section, edges included.
* `Limits::axial_mould = false` switches the three mould tests off, for printed parts or moulds with rotating cores.

## Checker thresholds (process inputs, UNVERIFIED as design values)

| Threshold | Value | Status |
|---|---|---|
| `min_wall` | 0.8 mm | UNVERIFIED moulding rule of thumb for thin-wall thermoplastics |
| `min_tip_gap` | 0.3 mm | assembly/run-out allowance, a stated assumption |
| `max_tip_gap_rel` | 0.05 of `r_tip` | a stated assumption |
| `hub_tip` | [0.2, 0.7] | UNVERIFIED typical range |
| `solidity` | [0.3, 2.0] | UNVERIFIED typical range |
| `min_passage_gap` | 0.8 mm | equal to `min_wall` (mould steel between blades) |
| `min_axial_gap` | 1 mm | rotor-to-strut, a stated assumption |
| `min_draft_deg` | 1 deg | UNVERIFIED moulding rule of thumb |
| `edge_band` | 0.05 | a stated assumption |
| `n_radii` | 21 | a stated assumption |

All of them are fields of `Limits`, which the caller sets.

## Honest gap list: what the existing sector generators realise

The sector generators are `fan_mesh` (h_slit, ADR 0065) and `fan_cgrid` (ADR 0068/0079/0083). Each meshes **one periodic blade
passage**, with a **hub cylinder `r_hub`** and a **shroud cylinder `r_tip + clearance`** over the whole domain length.

**Realised** (`meshed = true`):
* `n_blades`, `r_hub`, `r_tip`, `tip_gap` (as `tip_clearance`), `z_le_root` and `family`;
* every control-point field: span (to radius), chord, stagger, camber, camber position, thickness, sweep, and skew as lean;
* `deviation.stagger_common_deg` and `deviation.tip_gap_delta`, because they are the same on every blade.

**Not realised** (`meshed = false`):
* **Per-blade deviations** (stagger, chord, thickness): a single passage with cyclic seams can only carry identical blades. They
  need a full-annulus mesh (B3).
* **Eccentricity**: the gap varies with azimuth, so the domain is not periodic.
* **Mass, density and `mass_rel`**: they have no role in the flow mesh. They are used only by `static_unbalance`.
* **Roughness**: the solver has a case-wide `turbulence.roughness_height` (all walls), not a per-blade or blade-only roughness.
  The mesh carries none.
* **Motor hub length and start**: the mesh hub is a cylinder over the whole domain, so there is no hub nose and no open annulus
  ahead of or behind the hub.
* **Frame**: no square outline, no inlet bell, no frame depth (the shroud cylinder runs through the whole domain), and no struts.
  This is B4.
* **Chordwise thickness form**: fixed by the family. Only the spanwise maximum thickness is a parameter.
* **Trailing-edge thickness**: both families close the trailing edge to zero thickness. `min_thickness` therefore tests the
  maximum thickness only. A moulded edge thickness is neither tested nor realisable.
* **Tip shape**: no winglets, end plates or rotating ring. The schema does not offer one.
* **Root fillet**: not in the schema, and not realisable.
* **No caller**: the case tree (`mesh.fan`, `fan_case.rs`) does not read `ComputerFan` and does not call the checker. The checker is
  a library function, run before meshing by whoever builds the variant. Wiring it into `mesh.fan` would add schema keys.
  Proposed as a roadmap item, not done here, so that defaults stay bit-identical.

## Registered checks

All checks are in `crates/rcfd-mesh/tests/fan_design_w46a.rs` unless named otherwise. The fan `F0 = ComputerFan::default()`, and
`L0 = Limits::default()`.

### S, schema (A1)
* **S1, round-trip.** `F0`, and a specimen with a full `deviation` (the E2 ensemble member 0), go through `serde_json` and back:
  * the result is equal (`PartialEq`) and every scalar is bitwise equal;
  * re-serialising gives the same string.
  * Negative control NS-S: an unknown key (`"tip_gapp"`) is rejected, and so is a control point without `chord`.
* **S2, table.**
  * Every scalar path of `F0` and of the full specimen has a `PARAMS` spec, and every spec is used.
  * Every `PARAMS` default equals the corresponding field of `F0` bitwise.
  * `F0` is inside every bound.
  * `serde_json::to_string(PARAMS)` contains every path and its unit.
* **S3, realisation.**
  * A specimen whose deviation is all zeros realises every blade bitwise equal to the nominal `AxialFan`.
  * For blade 3 of a specimen with stagger common 0.7 deg, blade stagger -0.4 deg, `chord_rel` 0.03 and `thickness_rel` -0.05:
    * chord equals the nominal times 1.03 to relative 1e-15;
    * absolute thickness equals the nominal times 0.95 to relative 1e-14;
    * stagger equals the nominal plus 0.3 deg to 1e-15 rad;
    * `tip_clearance` equals `tip_gap + tip_gap_delta` exactly.
* **S4, meshing the realised default.** `F0.axial_fan(0, 0.03, 0.04)` must build with `build_sector_mesh` (h_slit) and with
  `build_cgrid_mesh_with(camber_seam_h)`, at the smoke spec below. The mesh quality is reported, with no limit. `pursuit` is
  reported only: it rejected the default at smoke.

### C, checker (A2)
* **C1.** `F0.check(&L0)` is empty.
* **C2, constructed violations.** Each variant is `F0` with the change shown. It must yield the code at the path, with the blade
  and radius where given, and no `out_of_bounds` (negative control NC1: a bounds-only checker accepts every one of them).

| Variant | Change | Expected code, path |
|---|---|---|
| thin tip | `stations[2].thickness` 0.015 | `min_thickness`, `stations[2].thickness` |
| small gap | `tip_gap` 0.2 mm | `tip_gap`, `tip_gap` |
| large gap | `tip_gap` 3.5 mm, `frame.side` 0.13 | `tip_gap`, `tip_gap` |
| eccentric rub | eccentricity 1.2 mm | `tip_rub`, `deviation.eccentricity` |
| hub ratio | `r_hub` 0.042 | `hub_tip_ratio`, `r_hub` |
| low tip solidity | tip chord 0.012, thickness 0.1 | `solidity`, `stations[2].chord` |
| axial overlap | hub chord 0.026, stagger 50 | `axial_overlap`, `stations[0].chord`, radius `r_hub` |
| collision | 11 blades, hub chord 0.022, stagger 60, thickness 0.35, `axial_mould` false | `collision`, `stations[0].chord` |
| passage gap | as collision with hub thickness 0.30 | `passage_gap`, `stations[0].chord` |
| draft | hub stagger 12, `frame.depth` 0.03, `hub.length` 0.028 | `draft`, `stations[0].stagger_deg`, radius `r_hub` (the root) |
| undercut | hub stagger 0, hub camber 0.12 | `undercut`, `stations[0].stagger_deg`, radius `r_hub` |
| thin blade 3 | per-blade deviations, blade 3 `thickness_rel` -0.45 | `min_thickness`, blade 3 only |
| frame wall | `frame.side` 0.119 | `frame_wall`, `frame.side` |
| axial fit | `z_le_root` 0.006 | `axial_fit` |
| root off hub | `hub.z_start` 0.004, `hub.length` 0.02 | `root_off_hub`, `z_le_root` |

* **C3, NC2.** With `axial_mould = false`, the axial-overlap, draft and undercut variants no longer report their mould code.
* **C4, structure.** Each of these is reported at the named path:
  * `n_blades` 30: `out_of_bounds` at `n_blades`, value 30, limit 15;
  * `deviation.blades` of length 3: `invalid` at `deviation.blades`;
  * `stations[1].span` 0: `invalid` at `stations[1].span`;
  * `tip_gap_delta` -1.1 mm: `invalid` at `tip_gap`, with no geometric code.
* **C5.** A `Violation` serialises with `"code":"min_thickness"` and round-trips.

### E, ensembles (A5)
Set-up T. Seed 46, n = 4000 specimens of `F0`:
* `stagger_common_deg`: truncated normal (0, 0.5, -1, 1);
* `stagger_blade_deg`: truncated normal (0, 0.5, -0.5, 0.5);
* `chord_rel`: truncated normal (0, 0.01, -0.015, 0.02);
* `thickness_rel`: uniform (-0.05, 0.05);
* `tip_gap_delta`: truncated normal (0, 1e-4, -2e-4, 2e-4);
* `eccentricity`: truncated normal (0, 1e-4, 0, 3e-4);
* `mass_rel`: truncated normal (0.005, 0.01, -0.01, 0.04);
* `roughness_ks`: uniform (0, 2e-5).

* **E1, determinism.**
  * The same seed gives a bitwise identical serialised ensemble, and seed 47 differs.
  * All-`fixed`-0 tolerances realise every blade of every specimen bitwise equal to the nominal blade, and `U` equals the nominal
    `U` bitwise.
* **E2, statistics.** There are 9 variables: the 8 above and the eccentricity direction, uniform on [0, 360). Each has n = 4000
  per-specimen draws or 28 000 per-blade draws. Limits:
  * mean: `|x_bar - mu| <= 4.5 sigma / sqrt(n)`. This is the central limit theorem, with a two-sided false-alarm rate of 7e-6.
  * variance: `|s^2 - sigma^2| <= 4.5 sqrt((mu_4 - sigma^4) / n)`, the asymptotic standard error of the sample variance.
    `mu_4` comes from composite Simpson quadrature of the analytic density (20 000 intervals), because a one-sided tail
    truncation is far from normal kurtosis.
  * Kolmogorov-Smirnov: `D_n <= sqrt(ln(2 / alpha) / (2 n))` with alpha = 1e-3. This is the Dvoretzky-Kiefer-Wolfowitz
    inequality with Massart's constant, exact for finite n.
  * **E2b, far tail.** The same three limits for 28 000 direct draws (seed 4601) of a truncated normal (0, 0.01, 0.02, 0.06), the
    tail beyond 2 sigma.
* **E3, analytic moments.** `Dist::mean` and `Dist::variance` match the Simpson quadrature of the density to a relative 1e-9, for
  every truncated normal of T and E2b.
* **Negative controls** (seed 4602, n = 28 000). Each must exceed its limit by at least 2x:
  * NS1: a normal (0, 0.5) clamped to [-0.5, 0.5], tested against the truncated CDF with KS. Smoke gave D = 0.160 against a limit
    of 0.01165, 13.8x.
  * NS2: a uniform with the mean and variance of `chord_rel`, tested against its truncated CDF with KS. Smoke gave 0.0497, 4.3x.
  * NS3: the parent mean 0.005 taken as the expected mean of `mass_rel`. The analytic shift is 1.38e-3 against a limit of
    4.5 sigma_t / sqrt(n) = 2.36e-4, 5.8x.

### I, imbalance (A5)
Here `m0` and `c0` are the mass and centroid of blade 0 of `F0` from `blade_mass`, `r_c = |c0_xy|`, and `R_k` is the rotation by
`2 pi k / N`.
* **I1.** The nominal `|U| <= 1e-12 N m0 r_c`. Reason: symmetry, with round-off only.
* **I2.** Only blade 3 has `mass_rel` = 0.01. Then `|U - 0.01 m0 R_3 c0| <= 1e-10 * 0.01 m0 r_c`.
* **I3.** Twenty specimens (seed 46) with only `mass_rel` drawn from T: `|U - m0 sum_k eps_k R_k c0| <= 1e-10 m0 r_c` each.
* **I4, independent mass properties.** Mass and centroid from the triangulation at (96, 48) are compared with an independent
  quadrature. The quadrature uses Green's theorem on a 4000-point section polygon for `int int (1, r cos(s/r), r sin(s/r), z) dz ds`,
  with 16-point Gauss-Legendre in `r` between control points, because `dV = dr dz ds` with `s = r theta`.
  * Limits: relative mass error `<= 1e-3`; `|c - c_ref| <= 2e-4 r_c`; and error ratio (96,48)/(192,96) `>= 3` for both, as
    O(h^2) gives 4.
  * Applied to blade 0 of `F0` and to blade 3 of the S3 specimen.
  * Reason for 1e-3: identical blades scale `U` by a common factor, so a 1e-3 mass error is a 1e-3 relative error in `U`. That
    is far below the percent-level spread of the tolerances.
* **Negative controls:**
  * NI1: centroids not rotated give `|U| = N m0 r_c` against the I1 limit;
  * NI2: `mass_rel` ignored gives an I2 error of `0.01 m0 r_c`, 1e10 times the limit.

### D, defaults
* **D1.** These pass unchanged in the gate:
  * `crates/rcfd-api/tests/revolved_sector_w45b.rs::g6_fan_meshes_unchanged`, the hashes of the default `axial_fan_sector`
    mesh and the ADR 0086 C-grid meshes;
  * every existing `fan*.rs`, `cgrid*.rs` and `fan_sector.rs` test;
  * `baseline_hashes.rs`.
* **D2.** `git diff main -- crates/rcfd-api/tests/baseline_hashes.rs` is empty. The only changes to `fan.rs` are a serde derive
  on `SectionFamily` and the new `BladeSurface::centroid`. `Rng` is moved byte for byte, its unit test moves with it, and the
  `design` tests pass unchanged.

### Status decision rule
* A2 is delivered if C1-C5 pass.
* A5 is delivered if E1-E3 and I1-I4 pass.
* A1 is delivered if S1-S4 pass.
* A negative control that does not exceed its limit by 2x declares its check non-discriminating. That is a miss of the check.

## Smoke results before registration (rule d; disclosed)

The code (`fan_design.rs`, the `Rng` move, `centroid`) and two ignored smoke tests were written before this ADR. The smokes
(`smoke_w46a`, `smoke_w46a_mass_and_stats`, debug build) ran once each. A first run used a mesh spec with `n_gap` 4, which
`graded_cells` rejects for 2 + 2 boundary-layer cells; it was corrected to 7. No registered test has run.
* **Default `F0`:** no violation. Margins, measured by re-running with tightened limits:
  * frame wall 1.0 mm (limit 0.8);
  * minimum thickness 1.21 mm at the root (0.8);
  * blade `z_max` 19.8 mm (22.0);
  * minimum draft 24.2 deg, at the root, suction side, x = 0.95 (1.0);
  * passage gap above 10 mm.
* **Meshing the realised default** (spec: `n_up` 8, `n_chord` 16/24, `n_down` 8, `n_side` 10, `n_mid` 2, `n_nose` 6, `n_core` 2,
  `n_span` 10, `n_gap` 7, `n_bl` 2, growth 1.2, first cell 5e-5 m):
  * h_slit: 11 648 cells, max non-orthogonality 86.1 deg;
  * c_grid `camber_seam_h`: 12 841 cells, max non-orthogonality 87.9 deg;
  * c_grid `pursuit`: **rejected**, "the wake line leaves the passage" at `n_down`, k = 0, r = 0.0215. The pursuit construction
    does not handle the 40 deg root stagger with this pitch. That is a B1 finding, not fixed here.
* **Variants:**
  * every C2 variant gives its expected code;
  * some variants also give others, which C2 allows:
    * axial fit with low stagger;
    * root off hub with 11 blades;
    * thin blade 3 gives one violation per control point;
  * the draft variant was smoked twice. With `hub.length` 0.029, the hub end 0.001 + 0.029 exceeded the depth 0.03 by
    round-off and gave `axial_fit`. With 0.028 (registered) it gives `draft` only: 0.116 deg at the root, suction side, x = 0.86;
  * a root-stagger scan from 8 to 22 deg showed draft violations, 0.09-0.16 deg, without undercut at 8-14 deg.
* **Mass:**
  * blade 0 has `m0` = 1.169 g and centroid (40.33, -6.71, 8.35) mm;
  * the nominal `|U|` was 7.6e-21 kg m;
  * triangulation against quadrature:

    | Resolution | Mass error | Centroid error / `r_c` |
    |---|---|---|
    | (48, 24) | 1.60e-3 | 2.5e-4 |
    | (96, 48) | 3.99e-4 | 6.3e-5 |
    | (192, 96) | 9.99e-5 | 1.6e-5 |

  * The quadrature itself changed by 3e-10 relative from 2000 to 4000 polygon points. `MASS_RESOLUTION` was raised from (48, 24)
    to (96, 48) after this.
* **Statistics controls:**
  * NS1 D = 0.160, NS2 D = 0.0497, against a limit of 0.01165;
  * the correct sampler gave D = 3.4e-3 on the NS1 distribution (seed 1);
  * the far-tail distribution gave D = 4.2e-3, in 0.21 s for 28 000 draws in debug.
* **Timing (debug):**
  * an ensemble of 2000 x 7: 53 ms;
  * a per-blade `check`: about 0.2 s per specimen;
  * `static_unbalance` at (48, 24): 7 ms.

## Out of scope

* A3 (centrifugal), A4 (export), and the case-tree wiring of the schema.
* Any CFD.
* E1 (balance grade): `U` is given in kg m; `G = U omega / M` is part E.
* The effect of roughness on the flow.

## Files

* `crates/rcfd-mesh/src/fan_design.rs` (new), `crates/rcfd-mesh/src/lib.rs`, `crates/rcfd-mesh/src/fan.rs` (derive, centroid).
* `crates/rcfd-core/src/rng.rs` (moved), `crates/rcfd-core/src/lib.rs`, `crates/rcfd-api/src/design/rng.rs` (re-export).
* `crates/rcfd-mesh/tests/fan_design_w46a.rs` (new).

## References

* Kolmogorov-Smirnov statistic and the Dvoretzky-Kiefer-Wolfowitz inequality, with Massart's tight constant (P. Massart, Ann.
  Probab. 18 (1990) 1269-1283): `P(sup |F_n - F| > eps) <= 2 exp(-2 n eps^2)`.
* Truncated normal mean and variance: the standard closed forms `mu + sigma (phi(a) - phi(b)) / Z`, and
  `sigma^2 [1 + (a phi(a) - b phi(b)) / Z - ((phi(a) - phi(b)) / Z)^2]`. E3 checks them against quadrature.
* The asymptotic variance of the sample variance, `(mu_4 - sigma^4) / n`: a standard result.
* `erfc` continued fraction (Laplace) and the `erf` series: the same forms as `crates/rcfd-reactor/src/mixfrac.rs`. The unit
  test checks them against Phi(1) = 0.841344746068543, Phi(-3) = 1.3498980316301e-3 and Phi(-5) = 2.8665157187919e-7.
* Moulding thresholds: UNVERIFIED rules of thumb, as stated in the table above.

## Addendum: measured values (one registered run each, debug build, `target-w46a`, on the working tree committed as 8bace16, rebased as 64b4fd3)

Every registered check passed. No known-miss test was created.

**A1 schema**
* **S1:** the default and the seed-46 specimen round-trip through JSON equal and bitwise equal, and re-serialising gives the same
  string. NS-S: the unknown key `tip_gapp` and the missing `chord` are both rejected.
* **S2:** every scalar has a spec and every spec is used. All 32 `PARAMS` paths serialise with their units, and every default is
  bitwise equal to the default fan's field.
* **S3:** a zero deviation realises every blade bitwise equal to the nominal blade. For blade 3:
  * chord: nominal times 1.03 (limit 1e-15 relative);
  * absolute thickness: nominal times 0.95 (limit 1e-14);
  * stagger: nominal + 0.3 deg (limit 1e-15 rad);
  * clearance: exact.
* **S4:** h_slit gives 11 648 cells (max non-orthogonality 86.07 deg). c_grid `camber_seam_h` gives 12 841 cells (87.91 deg).
  c_grid `pursuit` rejects the default with "the wake line leaves the passage" at `n_down`, r = 0.0215 (reported, as at smoke).

**A2 checker**
* **C1:** the default fan gives no violation.
* **C2:** every variant gave its code at its path, with the registered radius where one was registered, and no `out_of_bounds`.
  NC1, the bounds-only checker, accepts all 15. Value against limit:

  | Variant | Value | Limit |
  |---|---|---|
  | thin tip | 0.783 mm | 0.8 |
  | small gap | 0.2 mm | 0.3 |
  | large gap | 0.0603 r_tip | 0.05 |
  | eccentric rub | -0.2 mm | 0 |
  | hub ratio | 0.724 | 0.7 |
  | tip solidity | 0.272 | 0.3 |
  | axial overlap | 0.624 mm | 0, at `r_hub` |
  | collision | distance 0 | |
  | passage gap | 0.517 mm | 0.8 |
  | draft | 0.116 deg | 1, at `r_hub` |
  | undercut | 4 crossings | 2, at `r_hub` |
  | thin blade 3 | 0.666 mm | 0.8; three violations, all blade 3 |
  | frame wall | 0.5 mm | 0.8 |
  | axial fit | 22.85 mm | 22.0 |
  | root off hub | root LE 1.96 mm | hub start 4.0 mm |

* **C3:** with `axial_mould = false`, the overlap, draft and undercut variants lose their mould code.
* **C4** and **C5:** pass.

**A5 ensembles: E1** passes. Same seed gives the same JSON, seed 47 differs, zero tolerances are bitwise nominal, and `U` is
bitwise nominal.

**E2:** measured over limit for the mean, variance and KS statistics. The largest ratio is 0.699, the KS of the far-tail
distribution.

| Variable | n | Mean | Variance | KS |
|---|---|---|---|---|
| stagger_common_deg | 4000 | 0.239 | 0.117 | 0.391 |
| tip_gap_delta | 4000 | 0.168 | 0.006 | 0.327 |
| eccentricity | 4000 | 0.009 | 0.151 | 0.288 |
| eccentricity direction | 4000 | 0.094 | 0.068 | 0.348 |
| stagger_blade_deg | 28 000 | 0.170 | 0.115 | 0.355 |
| chord_rel | 28 000 | 0.134 | 0.012 | 0.505 |
| thickness_rel | 28 000 | 0.012 | 0.083 | 0.351 |
| mass_rel | 28 000 | 0.153 | 0.009 | 0.416 |
| roughness_ks | 28 000 | 0.083 | 0.312 | 0.453 |
| E2b far tail | 28 000 | 0.509 | 0.194 | 0.699 |

**E3:** the quadrature mass is 1 to 1e-12. The worst relative errors of the analytic moments are 1.7e-14 for the mean and
1.1e-13 for the variance (far tail), against a limit of 1e-9.
* Deviation in reading the limit: for the zero-mean distributions, "relative" was taken relative to `max(|mean|, sigma)`,
  because a relative error of a zero mean is undefined. This was decided while writing the test, before the run.

**Negative controls:** NS1 13.77x, NS2 4.27x and NS3 5.75x their limits (required 2x).

**Imbalance**
* **I1:** the nominal `|U|` is 2.44e-20 kg m, 7.3e-5 of the limit. NI1 is 1.0e12 times the limit.
* **I2:** `|U|` is 4.781441e-7 kg m (0.478 g mm), equal to `0.01 m0 r_c` to 3.8e-4 of the limit. NI2 is 1.0e10 times the
  limit.
* **I3:** the worst of 20 specimens is 6.8e-6 of the limit.
* **I4:**

  | Blade | Mass | Mass error | Centroid error / `r_c` | Error ratios |
  |---|---|---|---|---|
  | F0 blade 0 | 1.169093 g | 3.99e-4 (limit 1e-3) | 6.32e-5 (2e-4) | 4.00 and 4.00 (limit 3) |
  | S3 blade 3 | 1.143958 g | 4.52e-4 | 6.90e-5 | 4.00 and 4.00 |

**Other findings**
* A unit test, not a registered check: the first version of the `normal_cdf` continuity assertion ignored the slope `2 h phi(x)`
  and failed. Its assertion was corrected; `normal_cdf` itself was not changed.
* The reference values Phi(1), Phi(-3) and Phi(-5) agree with an independent evaluation (Python `math.erfc`) to below 1e-16.

**What these results do not show**
* They do not show that the thresholds, the default fan or the tolerance magnitudes describe any real fan or mould.
* They do not show that `U` matches a real rotor, whose hub, magnet and bearing are not modelled.
* They do not show that the realised meshes are fit for CFD: S4 builds them but checks no quality limit.

## Final gate

* **Gate run.** On a fresh `git archive` of e548e88 (the branch rebased onto main 53e1fac), in `<scratch dir>`:
  * `cargo fmt --check`: clean;
  * `cargo clippy --workspace --all-targets -D warnings`: clean;
  * `cargo test --workspace --release --no-fail-fast`: **2042 passed / 0 failed / 651 ignored**.
* **Reconciliation with wave 45.** The wave-45 gate was 2028 / 0 / 649. This wave adds 13 registered tests and the
  `normal_cdf` unit test, which gives 2042, and the 2 ignored smokes, which give 651.
* **D1 and D2.** In that run, `g6_fan_meshes_unchanged`, `baseline_hashes.rs` and `docs_site_builds_with_no_broken_links`
  passed. `git diff main -- crates/rcfd-api/tests/baseline_hashes.rs` is empty.
* **After the gate.** Main moved to 7fcdf9c (wave 46C `rcfd_post::rotordyn` and docs; none of the files touched here). The
  branch was rebased onto it: the commits are a306711 (registration), 64b4fd3 (implementation) and cc88f3e (addendum).
  The re-runs on the rebased branch: fmt clean, workspace clippy clean, and this ADR's tests 13 passed / 2 ignored, plus the `normal_cdf` and `rng` unit tests. There was no second full test gate.
