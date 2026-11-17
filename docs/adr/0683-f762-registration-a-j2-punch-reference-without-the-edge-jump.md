# ADR 0683: F-762 registration: a J2 punch reference without the edge jump (the block of ADR 0636 under a smoothed pressure on 4^3, 8^3 and 16^3, rule L and F-bar, the three-mesh reading of ADR 0678, a J2 tolerance that separates the Full Hex8, and whether F-bar's edge excess disappears)

Status: **REGISTERED 2026-11-17, written before any code of F-762 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-762 was written, compiled or run, and no probe of any kind was run:
the files listed under "What was read" were read and the arithmetic below was done by hand (disclosed). Results go in the
addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-762 (deps F-747), the follow-up proposed by
[ADR 0678](0678-f747-registration-rule-l-against-f-bar-with-a-j2-material-on-a-refined-reference.md) (its remaining question
1, option (a), and question 3). The elements, the material and the solver options are those of ADR 0678 reading c1: the
incompatible-modes Hex8 at finite strain of ADR 0608 with the rule `IncompatibleStabilisation::LinearStress` of ADR 0630 D1
(rule L) and the variant S of ADR 0658 (backtracking and decrease test of the mode update, cold start); the F-bar Hex8 of ADR
0506 (`SolidFormulation::BBar`); the Full Hex8 (`SolidFormulation::Full`) as the negative control; the Hex20 (27 points) on
`8^3`; the J2 material of [ADR 0666](0666-f742-registration-the-warm-start-of-the-incompatible-mode-iteration-on-a-history-material-and-on-the-transient-and-rotating-paths.md)
reading c1; the block of [ADR 0636](0636-f708-registration-the-linear-stress-coefficient-of-the-stabilised-incompatible-hex8-under-load.md)
reading c1 with its constraints, the node-set punch replaced by a smoothed pressure. Number 0683 given by the lead. Branch
`f762-work` from 524249e1 (F-747 / ADR 0678 merged), worktree `w49i-work/f762`, scratch `w49i-work/f762-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test
edited; no new dependency, no unsafe code, the crate graph unchanged; clean-room. **Test-only**: no library file is changed by
this item. Every choice below that the maintainer would normally be asked about is taken on the maintainer's standing
instruction of 2026-11-11 (take the recommendation), except the open questions at the end. The rule L neo-Hookean `16^3`
negative pivot of ADR 0678 belongs to F-763 (ADR 0682) and is not touched here; this item runs no neo-Hookean solve.

## What F-762 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. The block of ADR 0636 with the J2 material of ADR 0666 under a load without the node-set edge jump (a prescribed pressure or a punch displacement smoothed over a registered width) on 4^3, 8^3 and 16^3, rule L and F-bar (and the Hex20 on 8^3): the three-mesh reading of ADR 0678 with an uncertainty below a registered few per cent, a J2 tolerance derived from it that separates the Full Hex8 from the reference, and whether F-bar's edge excess of ADR 0678 disappears; the default bits unchanged.

Clauses: **c0** every limit registered here; **c1** the block with the J2 material under a load without the edge jump, smoothed
over a registered width, on `4^3`, `8^3`, `16^3`, rule L and F-bar; **c2** the Hex20 on `8^3`; **c3** the three-mesh reading of
ADR 0678 (amendment 1) with an uncertainty below a registered few per cent; **c4** a J2 tolerance derived from it that
separates the Full Hex8 from the reference; **c5** whether F-bar's edge excess of ADR 0678 disappears; **c6** the default bits
unchanged.

### Readings, registered

- **c1: the load, a smoothed pressure (chosen over a smoothed displacement).** A smoothed prescribed displacement still ends in
  a junction of a Dirichlet and a free part of the top face, where the traction of a mixed boundary-value problem is in general
  singular (of order `r^-1/2` at the junction for an incompressible solid; Williams 1952, cited from memory, the order not used
  beyond this argument); a smooth pressure on a free face has no such junction. The other five faces carry roller conditions
  (`xmin`, `xmax` `u_x = 0`; `ymin`, `ymax` `u_y = 0`; `zmin` `u_z = 0`), which are planes of symmetry: the problem is a cell of
  a mirrored periodic problem whose data are smooth, so no geometric singularity is left (the plastic zone boundary, where the
  second derivatives jump, remains). Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).
  - **The profile**: the nominal traction on the reference top face `z = 1` is `t = -p0 g(x) g(y) e_z` (a dead load: fixed
    direction and magnitude per reference area), with `g(t) = 1` for `t <= 0.25`, `g(t) = 1 - S((t - 0.25) / 0.5)` for `0.25 < t
    < 0.75`, `g(t) = 0` for `t >= 0.75`, and `S(s) = 6 s^5 - 15 s^4 + 10 s^3` (the quintic smooth step: `S`, `S'`, `S''`
    continuous at both ends). The transition is centred on the punch edge `0.5` of ADR 0636, so the loaded "width" `int g =
    0.5` per direction equals the punch's (arithmetic 1).
  - **The registered smoothing width**: `w = 0.5`, **two element widths of the `4^3` mesh** (`h4 = 0.25`), four of `8^3`, eight
    of `16^3`; the breaks `0.25` and `0.75` lie on element boundaries of every mesh, so every element sees one polynomial piece
    of the profile.
  - **The magnitude**: `p0 = 1.5e5` (`2.5 sigma_y`, `0.15 mu`; arithmetic 3), applied in `10` equal increments of the load
    factor through the per-node forces of `solve` (`forces` times `lambda`).
  - **The consistent nodal forces** `F_a = int N_a p0 g g dA` over the reference top face (`F_a` the magnitude in `-z`), from the
    face shape functions of the library (`rcfd_fem::loads::face_shape_functions`, 4 and 8 nodes) and a `4 x 4` Gauss-Legendre
    rule per face, exact for a polynomial of degree 7 per variable (the integrand has degree `5 + 1` on Hex8 faces and `5 + 2`
    on Hex20 faces, arithmetic 2): the forces are exact to rounding, checked by LS1 to LS3.
  - **The rest of the set-up** exactly as `solve_block` of `si_eas_hg11.rs` (ADR 0678 c1): `hex_box` `[0, 1]^3` on `n^3`, the J2
    material `Material::j2(Material::isotropic_elastic(2.98e6, 0.49, 1000, 0), 6.0e4, 3.0e5, 0)`, `NewtonOptions::new(10)
    .threads(4).record_active_set(true).factor(DirectFactor::SupernodalNestedDissection)`, F-bar and Full with the default
    options, rule L with S (and W of ADR 0660 once if S fails, reported). The `zmax` face carries no displacement constraint.
- **The measures** (in place of the punch reaction, which is now the load itself): the **load-weighted settlement** `S = sum_a
  F_a (-u_z,a) / sum_a F_a` (the work-conjugate displacement of the load: the work of the dead load is `p0 S / 4`), and the
  **corner bulge** `w_c = u_z(1, 1, 1)` (as ADR 0678). Reported without a limit: the centre settlement `-u_z(0, 0, 1)` and the
  band part `S_b = sum_{a in band} F_a (-u_z,a) / sum_a F_a`, the band the loaded nodes with `max(x, y) > 0.25 + 1e-9` (where
  the load is not at its full value).
- **c2: the Hex20.** `upgrade_to_hex20` of the regular `8^3` block, `Full` (27 points), the same constraints by the node sets
  of `hex_box` kept by the upgrade, the forces from the 8-node faces of the upgraded `zmax` set. Reported only, as in ADR 0678
  c2 (a single mesh; no reading).
- **c3: the reading.** Exactly `reading` of `si_eas_hg11.rs` (ADR 0678 c4 after amendment 1): `d1 = a4 - a8`, `d2 = a8 - a16`;
  asymptotic when `d1 d2 > 0` and `p = log2(d1 / d2)` in `[0.5, 4]`, then `a_inf = a16 - d2 / (2^p - 1)` and `U = 1.25 |d2| /
  (2^p - 1)`; otherwise `a_inf = a16` and `U = |d2| / (2^0.5 - 1)` (`d1 d2 > 0`) or `U = max(|d1|, |d2|)`. It is tested again on
  the synthetic sequences of ADR 0678 NC2 before any solve. **The registered few per cent**: `U / |a_inf| <= 0.03` for `S` and
  `<= 0.05` for `w_c` (the smaller, far-field measure), for F-bar and for rule L. **The reference**: the F-bar limit `a_ref =
  a_inf,Fbar` with `U_ref = U_Fbar` (as ADR 0678).
- **c4: the J2 tolerance.** As ADR 0678 c5: `E_X(n) = (|a_X(n) - a_inf,X| + U_X) / |a_inf,X|` and `T(n) = 1.25 (E_L(n) +
  E_Fbar(n))` rounded up to two decimals, for `S` (`T_S`) and `w_c` (`T_w`), `n = 4, 8, 16`. It is checked for consistency on
  the regular meshes (CJ), on DB4 (ADR 0649 D2, seed `0xF719_0008`, amplitude `0.25`; a mesh not in the derivation, DB), and
  **it must separate the Full Hex8**: `|S_Full(n) / S_ref - 1| > T_S(n)` (against the reference, NC1a) and `|S_Full(n) /
  S_Fbar(n) - 1| > T_S(n)` (against F-bar on the same mesh, NC1b) on every `n`, and the Full limit not the F-bar limit (G1 of
  ADR 0678 c6 does not hold, NC1c).
- **c5: whether F-bar's edge excess disappears, a registered classification.** In ADR 0678 F-bar's punch reaction was `1.64`
  and `1.38` times its limit on `8^3` and `16^3` against rule L's `1.32` and `1.125`, and the edge row carried more than the
  whole gap. With `d_X(n) = |a_X(n) / a_inf,X - 1|` (the distance of element `X` from its own limit, without `U`), for `S`:
  - **X1 "disappears"** when `d_Fbar(n) <= max(d_L(n), 0.02)` for `n = 8` and `n = 16` (F-bar is no farther from its limit than
    rule L, or within two per cent), **and** G1 holds for `S` (`|S_inf,L - S_inf,Fbar| <= U_L + U_Fbar`: the two elements share
    a limit, so the comparison of distances is meaningful);
  - **"persists"** otherwise, and then the band share `gamma_b(n) = (S_b,L - S_b,Fbar) / (S_L - S_Fbar)` (`n = 4, 8, 16`) says
    whether the remaining gap sits where the load varies (`gamma_b >= 0.5` on `8^3` and `16^3`) or elsewhere.
  - The classification is reported, not a pass or a miss; `gamma_b`, `d_X(n)` and the gap `S_L / S_Fbar - 1` are reported in
    either case, with the same reading made for `w_c` (reported only).
- **c6: default bits.** No library file changes (the diff is checked to touch only `crates/rcfd-fem/tests/` and `docs/`).
  Records run anyway: `si_eas_hg_default_bits.rs`, `si_eas_hg3_bits.rs`, `si_eas_default_bits.rs` (B6); `baseline_hashes` is
  not required (no shared solver file touched) and not run.

## What was read

- `AGENT_RULES.md`; the brief; ADR 0678 (whole: readings, amendment 1, Addendum 1, the remaining questions, the roadmap
  section); ADR 0636 (header and reading c1); ADR 0666 (by reference through ADR 0678).
- `crates/rcfd-fem/tests/si_eas_hg11.rs` (whole); `crates/rcfd-fem/src/loads.rs` (`face_shape_functions`, `face_rule`,
  `add_face_load`, `nodal_load_vector`), `model.rs` (`Load::pressure`, `add_load`), `mesh.rs` (`FaceRef`, `face_nodes`),
  `hex20.rs` (`upgrade_to_hex20` keeps surface sets, `face_rule` 3 x 3), `nonlinear.rs` (`solve`: per-node forces times the load
  factor).

## Arithmetic done before the code (disclosed)

1. **The integrals of the profile.** `int_0^1 S = 1/2` and `int_0^1 s S ds = 6/7 - 15/6 + 10/5 = 5/14`. So `int_0^1 g = 0.25 +
   0.5 x 0.5 = 1/2`; the first moment `M1 = int_0^1 x g dx = 1/32 + 0.5 (0.25 x 0.5 + 0.5 x (1/2 - 5/14)) = 1/32 + 1/16 + 1/28 =
   29/224`. The total force `sum F_a = p0 / 4` and the first moments `sum F_a x_a = sum F_a y_a = p0 x 29/224 x 1/2 = 29 p0 /
   448` (the isoparametric functions reproduce `x` and `y`, so `sum N_a x_a = x`). These are LS1 and LS2. A step at `0.5` would
   give `M1 = 1/8 = 28/224` and the same total: LS2, not LS1, tells the profile.
2. **The quadrature.** On every mesh an element face lies in one piece of `g(x) g(y)`, a polynomial of degree at most 5 per
   variable; times `N_a` (degree 1 per variable on Hex8 faces, at most 2 on the serendipity Hex20 face) and the constant face
   Jacobian of the regular block: degree 6 or 7 per variable, integrated exactly by 4-point Gauss-Legendre (exact to degree 7).
   A 2-point rule (degree 3) is not exact: mutation MU2.
3. **The magnitude.** ADR 0678 measured the punch reaction limits `R_inf / mu = 0.047` (F-bar), `0.045` (L) at `u = -0.1` over
   the area `0.25`: a mean pressure of about `0.19 mu = 1.9e5` for a settlement of `0.1`. `p0 = 1.5e5`, about `80` per cent of
   it, keeps the settlement below the punch's (estimated `S` of order `0.03` to `0.08`: the elastic settlement of a uniform
   pressure on an incompressible half-space over a square of side `1` (the mirrored load) is about `1.12 p b (1 - nu^2) / E =
   0.042`, cited from memory as the order only, the finite depth lowers it, plasticity raises it) while the load is above first
   yield (a uniform strip load on a half-space first yields at `p = pi k = 1.81 sigma_y` by Tresca, cited from memory; `p0 = 2.5
   sigma_y`): JY is expected to hold on every mesh. The isotropic hardening `H = 3e5` (`0.1 E`) keeps the response free of a
   limit load under force control.
4. **What the reading expects.** With smooth data, a trilinear element's displacement-type measures converge at order up to
   `2`; the plastic zone boundary lowers it. With `p` between `1.5` and `2` and a change `d1` of `5` to `15` per cent of the
   value, `U = 1.25 d2 / (2^p - 1)` is `0.3` to `2.5` per cent of the limit: hence `3` per cent for `S`. The corner bulge is the
   far-field response of the volume pushed aside, smaller and slower: `5` per cent. Both are an expectation with risk, not a
   certainty: a sequence outside the band gives the bound `2.414 |d2|`, which could miss.
5. **The Full Hex8 (NC1).** By the constraint count of ADR 0678 c2 (8 point constraints of plastic incompressibility against 3
   degrees of freedom per element), the Full Hex8 locks under isochoric plastic flow: stiffer, `S_Full < S_Fbar`; the ADR 0636
   elastic punch had it `+79` and `+40` per cent stiffer on `4^3`, `8^3`. The separation needs `|S_Full / S_ref - 1|` above
   `T_S(n)`, which by arithmetic 4 is of order `0.03` to `0.15`. Risk: ADR 0678 saw the Full Hex8 below F-bar on `8^3` and
   inverted on `16^3` under the punch; the smooth load removes the edge where it inverted, but a failed `16^3` Full solve misses
   NC1 (a non-finite measure).
6. **Cost.** ADR 0678 measured `16^3` supernodal solves of `61` to `130 s` and the Hex20 `8^3` `25 s` with the machine shared.
   The heavy record (three `16^3` solves: F-bar, L, Full; the Hex20; the `4^3`, `8^3` and DB4 solves): about `5` to `8` minutes,
   `#[ignore]`d, run once. The fast test (no `16^3` solve): about `20 s`.

## Design (test-only)

### D1. The test file `crates/rcfd-fem/tests/si_eas_hg_smooth.rs`

A new file (the name chosen so as not to collide with a parallel item's numbered file); helpers copied from `si_eas_hg11.rs`
(existing tests are not edited): `Checks`, `Rng`, `distort`, `min_corner_ratio`, the J2 material, the kinds and options,
`reading`, `error_at`, `tolerance`, `same_limit`, `plastic_points`. New: `profile` (`g`), `pressure_forces(mesh, p0, profile)`
(the consistent forces of D1 by `face_shape_functions` and the 4 x 4 rule), `settlement` (the measure `S` from the forces and
a displacement field), and `solve_smooth` (the set-up of c1).

- `j2_smooth_reference_setup` (fast, default): NC2; LS1, LS2 (`4^3`, `8^3`, `16^3`, Hex20 `8^3`), LS3, SM; the J2 solves of
  F-bar and L (S) on `4^3`, `8^3` and DB4 with SG and JY; FC on `4^3`; M0, M1 for DB4.
- `j2_smooth_reference_heavy_record` (`#[ignore]`, heavy record, run once): every solve of c1, c2, c4 on the three meshes and
  DB4; RC0, SG, JY on `16^3`, RU, TJ, CJ, DB, NC1a, NC1b, NC1c and the reports (the readings, every value against the reference
  and against the F-bar `16^3` value, the per-element tolerances, G1, X1, `d_X`, `gamma_b`, the Hex20, the centre settlement,
  the plastic point counts, the increments, halvings and residuals, the wall time of every solve).

## Checks

Every measure is required finite before it is compared (a non-finite value or a failed solve misses).

| Id | Clause | Measure | Limit | Estimate |
|---|---|---|---|---|
| NC2 | c3 | the reading on the synthetic sequences of ADR 0678 NC2: (i) `1 + 0.3 h^1.5`: asymptotic, `|p - 1.5|`, `|a_inf - 1|`, `|U - 1.25 |d2| / (2^1.5 - 1)|`; (ii) `(1, 0.9, 0.95)`: not asymptotic, `|a_inf - 0.95|`, `|U - 0.1|`; (iii) `(1, 0.5, 0.49)`: not asymptotic, `|U - 0.01 / (2^0.5 - 1)|` | (i) `<= 1e-10` each; (ii), (iii) `<= 1e-12` | ADR 0678 corrected run: `3.8e-15`, `0`, `2.3e-17` |
| LS1 | c1 | `sum F_a` against `p0 / 4`, relative; Hex8 `4^3`, `8^3`, `16^3`, Hex20 `8^3` | `<= 1e-12` each | arithmetic 1, 2: rounding |
| LS2 | c1 | `sum F_a x_a` and `sum F_a y_a` against `29 p0 / 448`, relative; the same meshes | `<= 1e-12` each | arithmetic 1, 2 |
| LS3 | c1 | with `g = 1`: the forces of D1 against the library's consistent loads of `Load::pressure("zmax", p0)` (`nodal_load_vector`), largest difference over the largest force; Hex8 `4^3`, Hex20 `8^3` | `<= 1e-12` each | two exact rules on a flat face |
| SM | c1 | the measure `S` of the synthetic field `u_z = -(1 + x)` on `4^3`: against `1 + 29/112` | `<= 1e-12` | arithmetic 1 |
| FC | c1 | J2 `4^3`, F-bar and L: supernodal against envelope, `S` and `w_c`, relative | `<= 1e-9` each | ADR 0678 FC: `4.7e-12` |
| SG | c1 | every J2 solve of F-bar and L (`4^3`, `8^3`, `16^3`, DB4): `S > 0` and `w_c > 0` | each | the load presses, the volume bulges |
| JY | c1 | plastic points (`p[12] > 0`) at the end of every J2 solve of F-bar and L, every mesh and DB4 | `> 0` each | arithmetic 3 |
| M0, M1 | c4 | DB4: smallest corner `det J` over centre `det J`; moved nodes | `> 0`; `343` | `0.0762`, `343` (ADR 0678) |
| RC0 | c1 | the `16^3` J2 solves of F-bar and L succeed | yes | ADR 0678 RC0 |
| RU | c3 | `U / |a_inf|` of F-bar and of rule L: for `S`; for `w_c` | `<= 0.03`; `<= 0.05` | arithmetic 4 |
| TJ | c4 | `T_S(n)`, `T_w(n)`, `n = 4, 8, 16` finite | each | |
| CJ | c4 | `|S_L / S_Fbar - 1| <= T_S(n)`, `|w_L / w_Fbar - 1| <= T_w(n)`, `n = 4, 8, 16` | each | met nearly by construction on the regular meshes (disclosed) |
| DB | c4 | DB4: `|S_L / S_Fbar - 1| <= T_S(8)`, `|w_L / w_Fbar - 1| <= T_w(8)` | each | a mesh not in the derivation |
| NC1a | c4 | `|S_Full(n) / S_ref - 1| > T_S(n)`, `n = 4, 8, 16` | each | arithmetic 5 (the risk stated) |
| NC1b | c4 | `|S_Full(n) / S_Fbar(n) - 1| > T_S(n)`, `n = 4, 8, 16` | each | arithmetic 5 |
| NC1c | c4 | G1 for `S` does not hold for the Full Hex8 against F-bar (`|S_inf,Full - S_inf,Fbar| > U_Full + U_Fbar`) | holds | arithmetic 5 |
| B6 | c6 | `si_eas_hg_default_bits`, `si_eas_hg3_bits`, `si_eas_default_bits`; the diff touches only `crates/rcfd-fem/tests/` and `docs/` | pass; yes | |
| S6 | | `cargo fmt -p rcfd-fem --check`, `cargo clippy -p rcfd-fem --all-targets -- -D warnings` | clean | |
| S7 | | `cargo test -p rcfd-fem --release --lib`; `--test si_eas_hg9`, `si_eas_hg10`, `si_eas_hg11`, `si_eas_hg_smooth` | pass | |
| S8 | | `cargo test -p rcfd-cli --release --test docs_site` (docs/ touched) | pass | |

Reported without a limit (c2, c3, c5): the classification X1 with `d_X(n)`, G1 for `S` and `w_c`, `gamma_b(n)`; the Hex20 on
`8^3` (`S`, `w_c` against F-bar `8^3` and the reference); every value against the reference and against the F-bar `16^3`
value; the per-element tolerances `1.25 E_X(n)`; the Full readings; the centre settlement; the plastic point counts; the
increments, halvings and residuals of every solve; the wall time of every `16^3` solve.

### Negative controls

| Id | Control | Expected |
|---|---|---|
| NC1 (a, b, c) | the Full Hex8, an element that locks under isochoric flow | outside the derived tolerance on every mesh; a different limit |
| NC2 | the reading on sequences with a known answer | the numbers of c3 |
| LS2 | a profile with the right total but the wrong shape (MU1) | caught |

### Planned mutations (on a scratch copy of the test commit, each run once against `j2_smooth_reference_setup`)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | the quintic step replaced by a linear ramp (`S(s) = s`) | LS2 (`int s^2 = 1/3` against `5/14`) |
| MU2 | the 4 x 4 face rule replaced by 2 x 2 Gauss | LS1, LS2 |
| MU3 | the load direction flipped (`+z`) | SG |
| MU4 | the Richardson limit with the sign of ADR 0678 as registered (`a16 + d2 / (2^p - 1)`) | NC2 (i) |
| MU5 | the yield stress times `1000` | JY |
| MU6 | `S` as the plain mean of `-u_z` over the loaded nodes | SM |

The heavy record is not run under the mutations (its rows depend on the same `reading`, `pressure_forces` and `settlement`,
checked by NC2, LS1 to LS3 and SM).

## Decision rule

- **done**: NC2, LS1, LS2, LS3, SM, FC, SG, JY, M0, M1, RC0, RU, TJ, CJ, DB, NC1a, NC1b, NC1c, B6, S6, S7, S8 met in their single
  registered run; every mutation caught by its registered row; X1 classified; no amendment.
- **partial**: B6 met, but another check missed, or a mutation not caught (or caught other than by its registered row), or a
  check met only after an amendment.
- **todo**: B6 fails (a default bit changed), or the `16^3` J2 block cannot be solved for either element.

## Limits (not covered)

- One J2 material, one load level, one profile and one width; a dead (not follower) pressure; the smoothed-displacement variant
  of the acceptance is not run (it keeps a Dirichlet and free junction, c1); no contact punch.
- The reference is an extrapolation from three meshes; no `32^3` mesh; the asymptotic range is read, not proved.
- The Hex20 only on `8^3`, reported only. The plain incompatible element (Off) is not run (it failed with J2 on every mesh in
  ADR 0678). No neo-Hookean solve (the elastic positive control of ADR 0678 is F-763's subject).
- The result says nothing directly about the node-set punch: the ADR 0678 values are not re-run.

## Open questions (taken or open)

1. **The design** (the pressure over the smoothed displacement, the quintic profile, the width of two `4^3` elements, `p0`, the
   measure `S`, the few per cent `0.03` and `0.05`, the X1 criterion, F-bar as the reference element): taken on the maintainer's
   standing instruction of 2026-11-11 (take the recommendation).
2. **Whether a J2 tolerance from this load replaces the ADR 0636 tolerances in ADR 0666 JB1, JB2** (the open question 2 of ADR
   0678): not taken (it would re-judge another item's registered checks); open for the maintainer, with the numbers of this item.
