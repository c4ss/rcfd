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
