# ADR 0678: F-747 registration: rule L against F-bar with a J2 material on a refined reference (the punch block of ADR 0636 on 4^3, 8^3 and 16^3, an order-of-convergence reading, a J2 tolerance derived from it, the reason of the gap of ADR 0666 A1.2)

Status: **REGISTERED 2026-11-17, written before any code of F-747 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-747 was committed and none of its checks was run. One timing probe
was run before this text (disclosed under "Arithmetic", item 6): an uncommitted, since deleted test file that solved the punch
block with the J2 material to `u_z = -0.03` in three increments (not the registered load) and printed only wall times and the
residual counts per increment; no measure of this item was printed or seen. Results go in the addenda at the end; nothing
above the addenda is changed after a run.

Roadmap item F-747 (deps F-742), the follow-up proposed by
[ADR 0666](0666-f742-registration-the-warm-start-of-the-incompatible-mode-iteration-on-a-history-material-and-on-the-transient-and-rotating-paths.md)
(its remaining question 1, option (a)). The elements: the incompatible-modes Hex8 at finite strain of ADR 0608 with the rule
`IncompatibleStabilisation::LinearStress` of ADR 0630 D1 (rule L), solved with the variant S of ADR 0658 (backtracking and
decrease test of the mode update, cold start), as the diagnosis A1.2 of ADR 0666 solved it; the reference element the F-bar
Hex8 of ADR 0506 (`SolidFormulation::BBar` in the nonlinear driver); the J2 material of ADR 0666 reading c1; the punch block
of [ADR 0636](0636-f708-registration-the-linear-stress-coefficient-of-the-stabilised-incompatible-hex8-under-load.md) reading
c1. Number 0678 given by the lead. Branch `f747-work` from c2ec31e4 (F-742 / ADR 0666 and F-748 / ADR 0668 merged), worktree
`w49i-work/f747`, scratch `w49i-work/f747-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test
edited; no new dependency, no unsafe code, the crate graph unchanged; clean-room. **Test-only**: no library file is changed by
this item. Every choice below that the maintainer would normally be asked about is taken on the maintainer's standing
instruction of 2026-11-11 (take the recommendation), except the open questions at the end.

## What F-747 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. The punch block of ADR 0636 with the J2 material of ADR 0666 on 4^3, 8^3 and 16^3 (and the Hex20 on 8^3 if it runs), rule L and F-bar: the punch reaction and the corner bulge of each element against the finest reference with a registered order-of-convergence reading, a registered J2 tolerance derived from it, and the reason for the gap of ADR 0666 A1.2 identified or bounded; the default bits unchanged.

Clauses: **c0** every limit registered here; **c1** the punch block with the J2 material on `4^3`, `8^3`, `16^3`, rule L and
F-bar; **c2** the Hex20 on `8^3` if it runs; **c3** the punch reaction `R` and the corner bulge `w_c` of each element against
the finest reference; **c4** a registered order-of-convergence reading (with what happens outside the asymptotic range,
registered now); **c5** a registered J2 tolerance derived from it; **c6** the reason for the gap of ADR 0666 A1.2 identified or
bounded; **c7** the default bits unchanged.

### Readings, registered

- **c1: the set-up.** Exactly `run_block` of `si_eas_hg9.rs` (ADR 0666 reading c1): `hex_box` `[0, 1]^3` on `n^3`, `xmin`,
  `xmax` `u_x = 0`, `ymin`, `ymax` `u_y = 0`, `zmin` `u_z = 0`, the punch (the `zmax` nodes with `x, y <= 0.5 + 1e-9`) at
  `u_z = -0.1`, 10 increments, `NewtonOptions::new(10).threads(4).record_active_set(true)`; `R = -sum_punch r_z / mu`, `w_c =
  u_z(1, 1, 1)`, `mu = 1e6` (the unit). The J2 material `Material::j2(Material::isotropic_elastic(2.98e6, 0.49, 1000, 0),
  6.0e4, 3.0e5, 0)`. F-bar with the default options, rule L with S (as ADR 0666 A1.2). One change from that function, for cost
  (arithmetic 6): `.factor(DirectFactor::SupernodalNestedDissection)` on every mesh and every element, so that all three meshes
  are solved by the same factor; the set-up checks SU2, SU3 and FC show that this does not move the answers on `4^3` and
  `8^3` (the supernodal factor has the pivot tests of the envelope one, ADR 0542 D2). If S fails on a mesh, W (ADR 0660) is run
  once on that mesh and used (ADR 0666: W against S within `5.5e-13`), and this is reported.
- **c2: the Hex20.** `upgrade_to_hex20` of the regular `8^3` block, `SolidFormulation::Full` (the only formulation of the
  Hex20: 27 points), the same constraints and node sets by coordinates (mid-side nodes included), the J2 material, the same
  options. **Reported only**, not a reference: by the constraint count of Nagtegaal, Parks and Rice (1974, cited from memory;
  the count is first principles) a fully integrated element locks under isochoric flow when the constraints per element exceed
  the degrees of freedom per element in a large mesh: the Hex20 has per element about `1` corner and `3` edge nodes, `12`
  degrees of freedom, against `27` point constraints of plastic incompressibility; the Hex8 Full has `3` against `8`; the F-bar
  Hex8 `3` against `1`. So the Hex20 (27 points) and the Full Hex8 are expected to over-estimate `R` in the plastic regime and
  not to converge to the F-bar limit; F-bar is the reference element (it was designed for this: de Souza Neto, Peric, Dutko,
  Owen 1996, cited from memory).
- **c3, c4: the order-of-convergence reading.** For each measure `a` (`R` or `w_c`) and element `X` with the three regular
  values `a4, a8, a16` (`h = 1/4, 1/8, 1/16`, refinement ratio `2`): `d1 = a4 - a8`, `d2 = a8 - a16`.
  - **Asymptotic** when `d1 d2 > 0` and `p = log2(d1 / d2)` lies in `[0.5, 4.0]` (an order outside this band on three meshes of a
    trilinear element is read as pre-asymptotic or as cancellation). Then the Richardson limit `a_inf = a16 + d2 / (2^p - 1)`
    and its uncertainty `U = 1.25 |d2| / (2^p - 1)` (the factor of safety `1.25` of Roache's grid convergence index for three
    meshes, cited from memory; it is a convention, applied as stated).
  - **Not asymptotic** otherwise: no extrapolation; `a_inf = a16` and `U = |d2| / (2^0.5 - 1) = 2.414 |d2|` when `d1 d2 > 0`
    (the error bound of the slowest admitted order `0.5`), `U = max(|d1|, |d2|)` when `d1 d2 <= 0` (an oscillating sequence:
    the spread). This is what happens outside the asymptotic range: a bound in place of an estimate, reported as such.
  - **The error at mesh `n`** of element `X` against its own limit: `E_X(n) = (|a_X(n) - a_inf,X| + U_X) / |a_inf,X|`.
  - **The finest reference**: the F-bar limit `a_ref = a_inf,Fbar` with `U_ref = U_Fbar` (the element of ADR 0636 and ADR 0666
    as reference). Each element's value on each mesh is reported against it (`a_X(n) / a_ref - 1`) and against the F-bar
    `16^3` value.
- **c5: the J2 tolerance.** For rule L against F-bar on the same regular mesh `n` (the form of the ADR 0636 tolerances): `T(n) =
  1.25 (E_L(n) + E_Fbar(n))`, rounded up to two decimals, for `R` (`T_R`) and `w_c` (`T_w`) and `n = 4, 8, 16`: two elements each
  within its discretisation error of its limit, with the same factor `1.25`. Reported also: the tolerance of each element
  against the reference, `1.25 E_X(n)`. The derivation fixes the tolerance from the regular meshes; it is tested on a mesh not
  used in it (DB, DB4 of ADR 0649 D2) and for consistency at `16^3` (CJ).
- **c6: the reason for the gap, a registered classification.**
  - **G1, same limit**: `|a_inf,L - a_inf,Fbar| <= U_L + U_Fbar`, for `R`. If G1 holds, the gap of A1.2 is **identified** as
    discretisation error of two different sizes: the element with the larger `E(8)` carries the larger part.
  - If G1 does not hold, the gap is **bounded**: the limit gap `B = a_inf,L / a_inf,Fbar - 1` with the range `B +-
    (U_L / |a_inf,L| + U_Fbar / |a_inf,Fbar|)`, located by two shares (no limit; read with the thresholds below):
    - the **edge share** `gamma(n) = (R_e,L - R_e,Fbar) / (R_L - R_Fbar)`, `R_e` the part of `R` on the edge row of punch nodes
      (`|x - 0.5| < 1e-9` or `|y - 0.5| < 1e-9`): `gamma >= 0.5` at `8^3` and `16^3` reads "the gap sits at the punch edge";
    - the **stabilisation share** `sigma(n) = (R_Off - R_L) / (R_Fbar - R_L)`, `R_Off` the incompatible element with
      `IncompatibleStabilisation::Off` and S (if it runs; ADR 0636 N3: the elastic plain element failed on `8^3`): `|sigma| <=
      0.25` reads "not the stabilisation" (the plain element has the same gap), `sigma >= 0.75` "the stabilisation", between
      "shared".
  - The elastic contrast (EC1, a check) says whether the reading finds the two elements consistent on the material where ADR
    0636 saw the gap close; the Full Hex8 control (NC1, a check) whether it can see an element that does not converge to the
    reference.
- **c7: default bits.** No library file changes (the diff is checked to touch only `crates/rcfd-fem/tests/` and `docs/`).
  Records run anyway: `si_eas_hg_default_bits.rs`, `si_eas_hg3_bits.rs`, `si_eas_default_bits.rs` (B6); `baseline_hashes` is
  not required (no shared solver file touched) and not run.

## What was read

- `AGENT_RULES.md`; the brief; ADR 0666 (whole: readings, amendment 1, Addendum 1, the remaining questions, the roadmap
  section); ADR 0636 (readings c1, c3, c4, arithmetic 2, the results, N1, N3); ADR 0668 (style and the roadmap section).
- `crates/rcfd-fem/tests/si_eas_hg9.rs` (`run_block`, `block_mesh`, `distort`, `min_corner_ratio`, `plastic_points`,
  `j2_reaction_against_mesh_diagnosis`), `si_eas_hg8.rs` (helper list).
- `crates/rcfd-fem/src/nonlinear.rs` (`NewtonOptions`, `factor_solve`), `solve.rs` (`DirectFactor`), `hex20.rs`
  (`upgrade_to_hex20`, 27-point rule), `element.rs` (the Hex20 in the nonlinear driver).
- The logs of ADR 0636 (`f708-scratch/run1-default.log`) and ADR 0666 (`f742-scratch/run1.log`, `run2-diagnosis.log`), for the
  8-digit values of the set-up checks.

## Arithmetic done before the code (disclosed)

1. **The known values (A1.2 of ADR 0666, published before this text).** J2, `R / mu`: F-bar `0.09785876` (`4^3`),
   `0.07717089` (`8^3`); rule L (S) `0.08146704`, `0.06172012`. `w_c`: F-bar `0.05043897`, `0.03601575`; L `0.05092271`,
   `0.02976180`. DB4 (ADR 0666 run1): F-bar `0.07576858`, `0.03522022`; L (S) `0.06653272`, `0.03300841`. Elastic
   (neo-Hookean, ADR 0636 run1, rule L with the default options): F-bar `0.38597738`, `0.07220995` (`4^3`), `0.34403791`,
   `0.06372110` (`8^3`); L `0.43284260`, `0.07317520`, `0.36659596`, `0.06631664`. These are the set-up references SU1 to SU3.
2. **Why the reaction falls with `h`.** The punch is a node set: on the edge row (`x = 0.5`) the prescribed `-0.1` falls to the
   free surface over one element, so the loaded width is about `0.5 + c h` with `c` of order `1/2`: the reaction carries an
   error of order `h` from the definition of the load alone, the same sign for every element (from above). With `c = 1/2` the
   area factor `(1 + h)^2` is `1.563`, `1.266`, `1.129` on the three meshes: a fall of `19` per cent from `4^3` to `8^3`
   (A1.2: F-bar `21`, L `24` per cent), `11` per cent from `8^3` to `16^3`, `q = d1 / d2` near `2.2`, `p` near `1.1`. A rough
   expectation, not a model; it predicts that both elements are far from their limits on `4^3` (of order `50` per cent).
3. **What the reading predicts if both elements follow `p ~ 1.1`.** F-bar: `d1 = 0.0207`, `d2 ~ 0.0095`, `R16 ~ 0.068`,
   `R_inf ~ 0.060`, `U ~ 0.010`; L: `d1 = 0.0197`, `d2 ~ 0.0091`, `R16 ~ 0.053`, `R_inf ~ 0.045`, `U ~ 0.010`. Then G1 would
   hold, marginally (the limits differ by about `0.015` against `U_L + U_F ~ 0.020`), and `T_R` would be about `2.3` (`4^3`),
   `1.3` (`8^3`), `0.9` (`16^3`): a J2 tolerance derived from a reading this far from convergence
   is loose, and this is the honest expectation, not a defect of the derivation. The elastic sequence (F-bar `0.386`, `0.344`,
   L `0.433`, `0.367`) with `p ~ 1`: `R_inf,F ~ 0.30`, `R_inf,L ~ 0.30`: EC1 expected to hold.
4. **`w_c`.** J2: F-bar `0.0504` to `0.0360` (`-29` per cent), L `0.0509` to `0.0298` (`-42` per cent): the bulge changes more
   than the reaction; its reading may be outside the asymptotic range (then the bound of c4).
5. **The Full Hex8 (NC1).** Elastic, ADR 0636: `0.691`, `0.481` against F-bar `0.386`, `0.344` (`+79`, `+40` per cent). Under
   isochoric plastic flow the Full Hex8 locks (c2, the count `3` against `8`): its reaction is expected well above F-bar on
   every mesh and its limit above F-bar's beyond the uncertainties; but a slowly converging or oscillating Full sequence would
   widen its `U` (the bound of c4) and could make G1 hold for it; this is the risk of NC1.
6. **Cost (the timing probe, disclosed).** Three increments to `u_z = -0.03`, 4 threads, the machine shared: F-bar `8^3`
   envelope `0.8 s`, supernodal `0.7 s`; L (S) `8^3` supernodal `0.9 s`; F-bar `16^3` supernodal `16.1 s`, envelope `98.5 s`;
   L (S) `16^3` supernodal `17.4 s`; Hex20 `8^3` supernodal `11.1 s`; six or seven residuals per increment. The registered load
   has ten increments and more plastic flow: about `60` to `120 s` per `16^3` solve with the supernodal factor (with the
   envelope five or six times that, hence c1's factor), about `40 s` for the Hex20. The heavy record (six `16^3` solves, the
   Hex20 and the `4^3`, `8^3`, DB4 solves): about `8` to `15` minutes; it is `#[ignore]`d and run once. The fast test (the
   `4^3`, `8^3` and DB4 set-up rows and the synthetic reading rows): about `30 s`.

## Design (test-only)

### D1. The test file `crates/rcfd-fem/tests/si_eas_hg11.rs`

Helpers copied from `si_eas_hg9.rs` (existing tests are not edited), with `run_block` extended by the factor of c1, the edge
row reaction `R_e`, and the formulation and stabilisation of the element (F-bar, L, Off, Full, Hex20), each with the J2 or the
neo-Hookean material of ADR 0636.

- `reading(a4, a8, a16) -> Reading { asymptotic, p, a_inf, u }` (c4), and `error_at`, `tolerance` (c5).
- `j2_refined_reference_setup` (fast, default): SU1, SU2, SU3, FC, JY on `4^3`, `8^3`, DB4; NC2 (the reading on synthetic
  sequences); M0, M1 for DB4.
- `j2_refined_reference_heavy_record` (`#[ignore]`, heavy record, run once): every solve of c1, c2, c6 on the three meshes and
  DB4; RC0, JY on `16^3`, RD, TJ, CJ, DB, EC1, NC1 and the reports (G1, `B`, `gamma`, `sigma`, the Hex20, every value against
  the reference, the iterations and the plastic point counts).

## Checks

Every measure is required finite before it is compared (a non-finite value or a failed solve misses).

| Id | Clause | Measure | Limit | Estimate |
|---|---|---|---|---|
| SU1 | c1 | neo-Hookean, F-bar and rule L (default options), `4^3` and `8^3`: `R`, `w_c` against arithmetic 1, relative | `<= 1e-6` each | the same solve, another factor: `~1e-10` |
| SU2 | c1 | J2, F-bar and L (S), `4^3` and `8^3`: `R`, `w_c` against arithmetic 1, relative | `<= 1e-6` each | as SU1 |
| SU3 | c1 | J2, DB4 (ADR 0649 D2, seed `0xF719_0008`, amplitude `0.25`): F-bar and L (S) `R`, `w_c` against arithmetic 1 | `<= 1e-6` each | as SU1 |
| M0, M1 | DB | DB4: smallest corner `det J` over centre `det J`; moved nodes | `> 0`; `343` | `0.0762`, `343` |
| FC | c1 | J2 `4^3`, F-bar and L: supernodal against envelope, `R` and `w_c`, relative | `<= 1e-9` each | rounding of two factors over ten increments |
| JY | c1 | plastic points (`p[12] > 0` of the J2 state) at the end of every J2 solve of F-bar and L, every mesh and DB4 | `> 0` each | ADR 0666: `491`, `493` of 512 |
| NC2 | c4 | the reading on synthetic sequences: (i) `a_n = 1 + 0.3 h^1.5`: asymptotic, `p` within `1e-10` of `1.5`, `a_inf` within `1e-10` of `1`, `U` within `1e-10` of `1.25 |d2| / (2^1.5 - 1)`; (ii) `(1, 0.9, 0.95)`: not asymptotic, `a_inf = 0.95`, `U = 0.1`; (iii) `(1, 0.5, 0.49)`: not asymptotic (`p = 5.6`), `U = 2.414... x 0.01` within `1e-12` | as stated | closed form |
| RC0 | c1 | the `16^3` J2 solves of F-bar and L succeed (`R`, `w_c` finite) | yes | the probe |
| RD | c3, c4 | the readings of `R` and `w_c` for F-bar and L (J2): `a_inf`, `U` finite and `U / |a_inf| <= 1` | each | arithmetic 3: `U / a_inf ~ 0.2` |
| TJ | c5 | `T_R(n)`, `T_w(n)`, `n = 4, 8, 16`: finite | each | arithmetic 3 |
| CJ | c5 | J2 regular meshes: `|R_L / R_Fbar - 1| <= T_R(n)`, `|w_L / w_Fbar - 1| <= T_w(n)`, `n = 4, 8, 16` | each | met on `4^3`, `8^3` nearly by construction (disclosed); at `16^3` a consistency test |
| DB | c5 | DB4 J2: `|R_L / R_Fbar - 1| <= T_R(8)`, `|w_L / w_Fbar - 1| <= T_w(8)` (a mesh not in the derivation) | each | `0.122`, `0.063` (ADR 0666) against `~1.3` |
| EC1 | c6 | neo-Hookean F-bar and L (default options) on `4^3`, `8^3`, `16^3`: G1 for `R` (`|R_inf,L - R_inf,Fbar| <= U_L + U_Fbar`) | holds | arithmetic 3 |
| NC1 | c6 | the Full Hex8 (J2) on `4^3`, `8^3`, `16^3` against F-bar: G1 for `R` does **not** hold, and `R_Full(n) > R_Fbar(n)` on every mesh | both | arithmetic 5 (the risk stated) |
| B6 | c7 | `si_eas_hg_default_bits`, `si_eas_hg3_bits`, `si_eas_default_bits`; the diff touches only `crates/rcfd-fem/tests/` and `docs/` | pass; yes | |
| S6 | | `cargo fmt -p rcfd-fem --check`, `cargo clippy -p rcfd-fem --all-targets -- -D warnings` | clean | |
| S8 | | `cargo test -p rcfd-cli --release --test docs_site` (docs/ touched) | pass | |
| S7 | | `cargo test -p rcfd-fem --release --lib`; `--test si_eas_hg`, `si_eas_hg2`, `si_eas_hg3`, `si_eas_hg5`, `si_eas_hg6`, `si_eas_hg7`, `si_eas_hg8`, `si_eas_hg9` (the files ADR 0666 ran, and its own) | pass | |

Reported without a limit (c3, c6): every value against the reference and against the F-bar `16^3` value; the per-element
tolerances `1.25 E_X(n)`; G1 for J2 `R` and `w_c` and the classification of c6 (`B` and its range, `gamma(n)` on the three
meshes, `sigma(n)` where Off runs); the Hex20 on `8^3` (`R`, `w_c` against F-bar `8^3` and the reference); the Off and Full
values; the readings of the Off and Full sequences and of the neo-Hookean `w_c`; the plastic point counts; increments,
halvings and residuals of every solve; the wall time of every `16^3` solve.

### Negative controls

| Id | Control | Expected |
|---|---|---|
| NC1 | the Full Hex8, an element that locks under isochoric flow | G1 does not hold: the reading tells a non-converging element from the reference |
| NC2 | the reading on sequences with a known answer, an oscillating and a too-fast one | the classification and the numbers of c4 |
| EC1 | (positive control) the elastic material, where ADR 0636 saw the gap close | G1 holds |

### Planned mutations (on a scratch copy of the test commit, each run once against `j2_refined_reference_setup`)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | the ratio inverted in `reading` (`q = d2 / d1`) | NC2 (i) |
| MU2 | the asymptotic test dropped (always extrapolate) | NC2 (ii), (iii) |
| MU3 | the factor of safety dropped (`U = |d2| / (2^p - 1)`) | NC2 (i) |
| MU4 | the punch set without its edge row (`x, y < 0.5 - 1e-9`) | SU1, SU2 |
| MU5 | the yield stress times `1000` (the J2 material elastic) | JY, SU2 |
| MU6 | the DB4 seed changed (`0xF719_0009`) | SU3 |

The heavy record is not run under the mutations (its rows depend on the same `reading`, checked by NC2).

## Decision rule

- **done**: SU1, SU2, SU3, M0, M1, FC, JY, NC2, RC0, RD, TJ, CJ, DB, EC1, NC1, B6, S6, S7, S8 met in their single registered run;
  every mutation caught; c6 classified (identified or bounded); no amendment.
- **partial**: B6 met, but another check missed, or a mutation not caught, or a check met only after an amendment.
- **todo**: B6 fails (a default bit changed), or the `16^3` J2 block cannot be solved for either element.

## Limits (not covered)

- One J2 material, one load level, one punch geometry with a node-set punch (the load definition itself converges at order
  `h`, arithmetic 2); no contact punch; no statistics.
- The reference is an extrapolation from three meshes; a `32^3` mesh is not run (about `8` times the `16^3` cost per solve on
  the supernodal factor's scaling, and the memory of its factor), so the asymptotic range is read, not proved.
- The Hex20 only on `8^3`, reported only (27 points; no reduced rule in the repository).
- No distorted mesh but DB4; no other rule of ADR 0630 but L and Off; W only as a fallback (ADR 0666 showed W equal to S).
- The default stays cold (ADR 0660 open question 2).

## Open questions (taken or open)

1. **The design of the reading** (the band `[0.5, 4]`, the factor `1.25`, the bounds outside the band, the tolerance as the sum
   of the two element errors, F-bar as the reference element, the supernodal factor on all meshes): taken on the maintainer's
   standing instruction of 2026-11-11 (take the recommendation).
2. **Whether the J2 tolerance replaces the ADR 0636 tolerances in ADR 0666 JB1, JB2**: not taken (it would re-judge another
   item's registered checks); open for the maintainer, with the numbers of this item.

## Amendment 1 (2026-11-17, written after the registered run of the fast test and before any further run)

The registered run of `j2_refined_reference_setup` (test commit 78d48d81, log `run1-fast.log` in the scratch folder, 21 s) met
every row but one: SU1, SU2, SU3 (largest `1.2e-7` against `1e-6`), M0 (`0.0762`), M1 (`343`), FC (largest `4.7e-12` against
`1e-9`), JY (every count positive), NC2 (ii), (iii) and the `p` and `U` rows of NC2 (i). It missed **NC2 (i) `|a_inf - 1|`:
`9.375e-3` against `1e-10`**.

**Diagnosis.** The registration's formula of the Richardson limit in reading c4, `a_inf = a16 + d2 / (2^p - 1)` with `d2 = a8 -
a16`, has the wrong sign: for `a_n = a + c h^p` one has `d2 = c h16^p (2^p - 1)` and so `a16 + d2 / (2^p - 1) = a + 2 c h16^p`,
while `a16 - d2 / (2^p - 1) = a` (the standard form `f_1 + (f_1 - f_2) / (r^p - 1)`, `f_1` the finest). The miss is exactly
`2 x 0.3 x (1/16)^1.5 = 9.375e-3`. The negative control NC2 caught a defect of the registration itself, which is what it is
for. Arithmetic 3 had used the correct sign (limits below the `16^3` values). Nothing else of the reading depends on the sign:
`p`, the asymptotic band, `U` and the bounds outside the band are unchanged.

**Correction (registered now).** Reading c4 reads `a_inf = a16 - d2 / (2^p - 1)`; every other definition, limit and check stays
as registered. In `si_eas_hg11.rs`: the reading as registered is kept as `reading_registered` and the registered run as the
ignored record `j2_refined_reference_setup_known_miss` (code and limits unchanged, the measured value in its ignore text);
`j2_refined_reference_setup_corrected` runs the same rows with the corrected `reading`, once. The heavy record
`j2_refined_reference_heavy_record` has not been run; it uses the corrected `reading` and is run once after this amendment. The
mutations are run against `j2_refined_reference_setup_corrected`. The registered rule gives **partial** whatever follows (a check
met only after an amendment).

## Addendum 1: results (2026-11-17)

The registered fast run once on 78d48d81 (`run1-fast.log`, 21 s; amendment 1); the corrected fast run once on cf00d2e1
(`run2-fast.log`, 15 s); the heavy record once on cf00d2e1 (`run3-heavy.log`, 409 s, the machine shared and the mutations
building beside it: F-bar `16^3` 130 s, L 67 s, Full 61 s, neo-Hookean F-bar 30 s, L 55 s, Off 17 s, Hex20 `8^3` 25 s); the
mutations once each on a scratch copy of cf00d2e1 (`mutations.log`, `mut-MU*.log`); S6, S7, B6, S8 once (`fmt.log`,
`clippy.log`, `s7-lib.log`, `s7-tests.log`, `s8-docs.log`). All in `w49i-work/f747-scratch`. No limit, set-up or measure was
changed after a run; the heavy record is kept as the ignored `j2_refined_reference_heavy_record_known_miss` (code and limits
unchanged, the measured values in its ignore text). It is the only run of the `16^3` meshes; the fast test
`j2_refined_reference_setup_corrected` (15 s) covers the same solve path, the reading and the set-up on `4^3`, `8^3` and DB4.

### The values (J2, `R / mu`, `w_c`)

| Mesh | F-bar | rule L (S) | Full Hex8 | Off (S) | Hex20 |
|---|---|---|---|---|---|
| `4^3` | `0.09785876`, `0.05043897` | `0.08146704`, `0.05092271` | `0.14215013`, `0.01953616` | fails (load 0.43) | |
| `8^3` | `0.07717089`, `0.03601575` | `0.06172012`, `0.02976180` | `0.07611689`, `0.02568226` | fails (0.23) | `0.05144384`, `0.02110758` |
| `16^3` | `0.06488529`, `0.02807182` | `0.05278510`, `0.02079916` | fails (inverted, 0.988) | fails (0.15) | |
| DB4 | `0.07576858`, `0.03522022` | `0.06653272`, `0.03300841` | | | |

The readings (c4): F-bar `R` asymptotic, `p = 0.752`, `R_inf = 0.04692`, `U = 0.0225` (`0.48` of the limit); `w_c` `p = 0.861`,
`0.01833`, `U = 0.0122` (`0.66`). Rule L `R` asymptotic, `p = 1.144`, `R_inf = 0.04540`, `U = 0.0092` (`0.20`); `w_c` `p =
1.239`, `0.01421`, `U = 0.0082` (`0.58`). Against the reference (the F-bar limit): F-bar `R` `+109`, `+64`, `+38` per cent on
`4^3`, `8^3`, `16^3`; rule L `+74`, `+32`, `+12.5` per cent; the Hex20 on `8^3` `+9.6` per cent (`w_c` `+15`); the Full Hex8
`+203`, `+62` per cent. Against the F-bar `16^3` value: rule L `+26`, `-4.9`, `-18.6` per cent. Neo-Hookean: F-bar `R`
asymptotic, `p = 1.217`, `R_inf = 0.3124`, `U = 0.017` (`0.054`); rule L failed on `16^3`, so it has no reading.

### Results

| Id | Limit | Measured | Met |
|---|---|---|---|
| SU1 (NH, `4^3`, `8^3`) | `<= 1e-6` | largest `6.4e-8` | yes |
| SU2 (J2, `4^3`, `8^3`) | `<= 1e-6` | largest `1.2e-7` | yes |
| SU3 (DB4) | `<= 1e-6` | largest `1.2e-7` | yes |
| M0, M1 | `> 0`; `343` | `0.0762`; `343` | yes |
| FC | `<= 1e-9` | largest `4.7e-12` | yes |
| JY | `> 0` | every J2 F-bar and L solve: `4^3` `493`, `491`; `8^3` `3738`, `3794`; `16^3` `29151`, `27395`; DB4 `3765`, `3799` | yes |
| NC2 (registered run) | as registered | **`|a_inf - 1| = 9.375e-3`** in (i); the other rows met | **no** (the registered formula's sign, amendment 1) |
| NC2 (corrected) | as registered | `p` off by `3.8e-15`, `a_inf` exact, `U` off by `2.3e-17`; (ii), (iii) met | yes (after the amendment) |
| RC0 | the `16^3` J2 F-bar and L solves succeed | both, no halving | yes |
| RD | `U / |a_inf| <= 1` | `0.479`, `0.664` (F-bar `R`, `w_c`); `0.203`, `0.579` (L) | yes |
| TJ | finite | `T_R = 3.21`, `2.11`, `1.54`; `T_w = 6.98`, `4.13`, `2.80` (`4^3`, `8^3`, `16^3`) | yes |
| CJ | `<= T(n)` | `R`: `0.168`, `0.200`, `0.186`; `w_c`: `0.010`, `0.174`, `0.259` | yes |
| DB | `<= T(8)` | `0.122` (`R`), `0.063` (`w_c`) | yes |
| EC1 | G1 holds (NH) | **not measurable**: the neo-Hookean rule L `16^3` solve fails (singular tangent, pivot `-4.65e6` at load factor `0.966` after five halvings) | **no** (known miss) |
| NC1 | G1 does not hold for Full; `R_Full > R_Fbar` on every mesh | **the Full `16^3` solve fails** (inverted element at load factor `0.988`); **`R_Full = 0.07612 < R_Fbar = 0.07717` on `8^3`** | **no** (known miss) |
| B6 | pass; the diff only tests and docs | the three bit records pass; `git diff --stat c2ec31e4` touches `crates/rcfd-fem/tests/si_eas_hg11.rs` and this ADR only | yes |
| S6 | clean | fmt and clippy (`--all-targets -D warnings`) clean | yes |
| S7 | pass | `--lib` 15; `si_eas_hg` 8, `si_eas_hg2` 3, `si_eas_hg3` 4, `si_eas_hg5` 3, `si_eas_hg6` 4, `si_eas_hg7` 4, `si_eas_hg8` 5, `si_eas_hg9` 4, `si_eas_hg11` 1 passed | yes |
| S8 | pass | `docs_site` 2 passed | yes |

### c6: the reason for the gap of ADR 0666 A1.2

- **G1 holds for `R` and for `w_c`**: `|R_inf,L - R_inf,Fbar| = 0.0015` against `U_L + U_Fbar = 0.032`; `|w_inf,L - w_inf,Fbar|
  = 0.0041` against `0.020`. By the registered classification the gap is **identified as discretisation error of two
  different sizes**, and the element with the larger `E(8)` is **F-bar** (`E_R(8)`: F-bar `1.12`, L `0.56`). Both elements
  converge from above; F-bar more slowly (`p = 0.75` against `1.14`). The gap of A1.2 (`-0.168`, `-0.200`, and `-0.186` on
  `16^3`) is F-bar's excess, not rule L's deficit: rule L is the nearer of the two to the common limit on every mesh.
- **Where it sits**: the edge share `gamma = 1.12`, `1.35`, `1.61` (`4^3`, `8^3`, `16^3`): the edge row of punch nodes carries
  more than the whole gap (F-bar's edge-row reaction is about twice rule L's on `16^3`, `0.0401` against `0.0205`; the interior
  rows give some back). So the gap sits in the representation of the edge jump of the node-set punch (arithmetic 2).
- **The stabilisation share** `sigma` is not available: the plain element (Off) fails on every mesh with J2 (the mode residual
  does not decrease, ADR 0658 D1, at load factors `0.43`, `0.23`, `0.15`), as it failed elastically in ADR 0636 N3.
- **Supporting, reported**: the Hex20 on `8^3` is `+9.6` per cent from the reference, nearer than rule L (`+32`) and F-bar
  (`+64`) on the same mesh, against the expectation of c2 (locking by the constraint count); and the Full Hex8 is below F-bar
  on `8^3`. Both say that F-bar's excess on this punch is not a volumetric locking that F-bar removes. The cause inside F-bar
  (the candidate: the constant dilatation of the edge element under the jump of the prescribed displacement) is not
  diagnosed here.
- **Bound**: the limit gap `B = R_inf,L / R_inf,Fbar - 1 = -0.032 +- 0.68`. The identification rests on G1 with F-bar's wide `U`
  (48 per cent of its limit); rule L's own reading is tighter (20 per cent) and lands within 3.3 per cent of F-bar's. The data
  are in the registered asymptotic band, but three meshes cannot show that they are in the asymptotic range.

### The J2 tolerance (c5)

`T_R(n) = 3.21`, `2.11`, `1.54` and `T_w(n) = 6.98`, `4.13`, `2.80` on `4^3`, `8^3`, `16^3`: registered, derived, met by every
regular mesh and by DB4, and **uninformative**: with errors of `0.4` to `1.6` of the limit on these meshes, a tolerance for two
elements on the same mesh built from their errors is larger than any gap a defect would plausibly cause. The per-element
tolerances against the reference are the useful reading: rule L `1.25 E_R = 1.25`, `0.70`, `0.46`; F-bar `1.96`, `1.40`,
`1.08` (`4^3`, `8^3`, `16^3`). The ADR 0636 tolerances (`0.15`, `0.10`) do not transfer to J2 on this punch, in either form.

### Deviations

- Amendment 1: the registered Richardson formula had the wrong sign, caught by NC2 (i); corrected before the heavy record.
- EC1 and NC1 missed by failed solves (`16^3` neo-Hookean rule L: a negative pivot of the tangent near full load, the loss of
  positive definiteness ADR 0636 saw for the plain element on `8^3`; `16^3` Full Hex8 with J2: an inverted element), and NC1
  also by `R_Full < R_Fbar` on `8^3` (arithmetic 5 expected locking above F-bar). They are known misses; no corrected test is
  registered, since neither is a defect of the set-up.
- Arithmetic 3 predicted the rule L values well (`16^3` `0.053`, limit `0.045`) and F-bar's poorly (`p ~ 1.1` assumed, `0.75`
  measured). c2 expected the Hex20 to lock; it lies nearest to the reference.
- MU4 was caught by a panic of the test (the empty edge-row set), not by the SU1, SU2 rows it was registered against.

### Mutations (scratch copy of cf00d2e1, `j2_refined_reference_setup_corrected`, each run once)

| Id | Caught | By |
|---|---|---|
| MU1 (ratio inverted) | **yes** | NC2 (i): asymptotic, `p` (`3.0` off), `a_inf`, `U` |
| MU2 (asymptotic test dropped) | **yes** | NC2 (ii) (`a_inf` NaN), (iii) |
| MU3 (no factor of safety) | **yes** | NC2 (i) `U` (`1.2e-3` off) |
| MU4 (punch without its edge row) | **yes, not as registered** | the test panics (`EmptySet "edge"`) before SU1 |
| MU5 (`sigma_y` times 1000) | **yes** | SU2 (`R` up to `2.1` off), JY (`0` plastic points) |
| MU6 (DB4 seed) | **yes** | SU3 (`1.7e-2` to `2.4e-2` off) |

Six of six caught; MU4 by a panic rather than by the registered rows.

### Not run

Nothing registered was skipped; the stabilisation share `sigma` could not be formed (Off fails). Not run by design: a `32^3`
mesh, a contact punch, other J2 materials, W except as the fallback (not needed: S converged on every mesh), `baseline_hashes`
(no shared solver file touched). The whole rcfd-fem suite was not run (rule 7).

### Decision

**partial**, by the registered rule: B6 met (no library file changed, the bit records pass), the `16^3` J2 block solved for
both elements, the order-of-convergence reading made and the reason for the A1.2 gap identified (G1: a common limit within the
uncertainties, F-bar carrying the larger error, located at the punch edge row), the J2 tolerance registered and derived; but
NC2 was met only after amendment 1, EC1 and NC1 missed (failed solves on `16^3`, and the Full Hex8 below F-bar on `8^3`), and
one mutation was caught by a panic rather than by its registered rows.

### Remaining questions

1. **The J2 tolerance is uninformative on this punch.** Options: (a) a load without the node-set edge jump (a prescribed
   pressure, or a punch displacement smoothed over a registered width) with the same three-mesh reading, where `U` should be a
   few per cent; (b) a `32^3` run of this punch; (c) accept the per-element tolerances of this ADR. Recommendation: (a),
   proposed below.
2. **Rule L loses positive definiteness on the refined neo-Hookean punch** (`16^3`, load factor `0.966`): new, not seen in ADR
   0636 (`4^3`, `8^3`). Options: (a) a follow-up diagnosing it (the ADR 0628 sweep on the `16^3` edge elements at the failing
   state, S and W, finer increments); (b) record it as a limit of rule L. Recommendation: (a), proposed below.
3. **F-bar's excess at the punch edge.** Options: (a) a diagnosis of the F-bar edge element; (b) take the rule L or Hex20
   values as the J2 reference for later items. Recommendation: (a) inside the follow-up of question 1 (a smooth load removes
   the jump and tells whether the excess is the edge's).
4. **Whether these readings re-judge ADR 0666 JB1, JB2** (open question 2 of the registration): open for the maintainer, not
   taken; the numbers say the JB misses are F-bar's error rather than rule L's.
5. **Default.** W stays opt-in (ADR 0660 open question 2): open for the maintainer.

## For the lead: roadmap entries

> Proposed status: partial.

> Structural solver, intermediate tier. Follow-up of F-742 (ADR 0666). PARTIAL. ADR 0678, 2026-11-17 (tests only, no library file changed): rule L (with S) against the F-bar Hex8 with the J2 material of ADR 0666 on the punch block of ADR 0636 on 4^3, 8^3 and 16^3 (the 16^3 solves 67 s and 130 s, a heavy record run once), with a registered three-mesh Richardson reading (order band 0.5 to 4, factor of safety 1.25, a bound outside the band). Punch reaction: F-bar order 0.75, limit 0.0469 +- 0.0225; rule L order 1.14, limit 0.0454 +- 0.0092; the limits agree (the corner bulge's too), so the 17 to 20 per cent gap of ADR 0666 A1.2 is discretisation error carried mostly by F-bar (64 per cent above the limit on 8^3 against 32 for rule L, 38 against 12.5 on 16^3) and located at the edge row of the node-set punch; the Hex20 on 8^3 is 9.6 per cent above the limit. The J2 tolerance derived from the reading (rule L against F-bar on the same mesh: 3.21, 2.11, 1.54 for the reaction) is met on every mesh and on DB4 but uninformative; the per-element tolerances against the limit (rule L 1.25, 0.70, 0.46) are the useful reading. Missed: the registered Richardson formula had a sign error, caught by the synthetic control and corrected by amendment; the elastic positive control could not be read (rule L with the neo-Hookean material loses positive definiteness on 16^3 at load factor 0.966); the Full Hex8 negative control failed (inverted on 16^3, below F-bar on 8^3). The plain incompatible element fails with J2 on every mesh. Six of six mutations of the test's reading and set-up caught (one by a panic). No default bit changed (the bit records pass). Not covered: a 32^3 mesh, a smooth or contact punch, other J2 materials, the cause inside F-bar of its edge excess.

> F-742 (note, ADR 0678, 2026-11-17): the reaction gap of rule L against F-bar with J2 (JB1, JB2) is F-bar's larger discretisation error at the punch edge; both elements reach a common limit within the uncertainty of a three-mesh reading (F-747).

> Follow-up proposed: A J2 punch reference without the edge jump. Deps F-747. Acceptance: Every limit not stated here is registered in the ADR before the run. The block of ADR 0636 with the J2 material of ADR 0666 under a load without the node-set edge jump (a prescribed pressure or a punch displacement smoothed over a registered width) on 4^3, 8^3 and 16^3, rule L and F-bar (and the Hex20 on 8^3): the three-mesh reading of ADR 0678 with an uncertainty below a registered few per cent, a J2 tolerance derived from it that separates the Full Hex8 from the reference, and whether F-bar's edge excess of ADR 0678 disappears; the default bits unchanged.

> Follow-up proposed: Rule L loses positive definiteness on the refined neo-Hookean punch block. Deps F-747. Acceptance: Every limit not stated here is registered in the ADR before the run. The failing solve of ADR 0678 (rule L, the neo-Hookean material of ADR 0636, the punch block on 16^3, the default options) reproduced first; the lowest eigenvalue of the ADR 0628 sweep and of the free tangent at the last converged state located by element; the same solve with S and W of ADR 0658 and ADR 0660 and with finer increments; the cause identified or bounded against a registered criterion; the default bits unchanged.
