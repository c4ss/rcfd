# ADR 0226: F-320 registration: wet-steam checks for MU3 and MU6

Status: **REGISTERED 2026-11-08**, written before any test code of F-320 and before any run of its checks. Results go in an
addendum; nothing above the addendum is changed after a run. Not yet accepted by the maintainer.

Parent: [ADR 0217](0217-f221-registration-non-equilibrium-condensing-steam-in-a-transonic-nozzle.md) (F-221), its addendum
(mutations MU3 and MU6 survived the fast checks, so F-221 stayed todo by the registered rule) and its question Q9, whose
recommendation (a) the lead took for roadmap item F-320: register a check that isolates the nucleated-mass term and a check of the
HLLC wave speeds against the frozen sound speed, then apply the ADR 0217 decision rule again.

Number: the lead gave 0226. Branch `f320-wetsteam-checks` from main c201a770 (the F-221 merge). Worktree
`E:/projects/cfd/w49i-work/f320`.

## What was run and read before this text (disclosed)

- **Read:** ADR 0217 in full (registration, amendment 1, addendum, gate line), `crates/rcfd-fv/src/wetsteam.rs` and
  `crates/rcfd-fv/tests/wetsteam.rs` at c201a770, the F-221 mutation script and its log
  (`E:/projects/cfd/w49i-work/f221-scratch/mutate.py`, `f221-mutations.log`). The library file at c201a770 is byte-identical to
  the copy the F-221 mutations were applied to (`f221-mut/wetsteam.rs.pristine`), so the six substitutions apply unchanged.
- **Arithmetic, no rcfd code** (scratch Python, `E:/projects/cfd/w49i-work/f320-scratch/margins.py`, not committed): with the M2
  formulas and the case-203 parameters of ADR 0217 (prototype values `c_pv = 1952.4402`, `c_pl = 4181.3731` J/(kg K),
  `R = 461.526` J/(kg K)), the mixture ratio `Gamma(y)` and the ratio of the vapour to the frozen sound speed
  `sqrt(gamma_v/Gamma(y))`: 1, 1.00646, 1.01556, 1.02932, 1.03439, 1.05259 at `y` = 0, 0.02, 0.05, 0.1, 0.12, 0.2; and the
  Einfeldt-Roe wave speeds of W2 for the state pairs of U1 and the step of U2 below, with the frozen and with the vapour sound
  speed (they set the Mach numbers of U1 and U2, see there). Also the R1 reference values below (products of the M1 table).
- **Estimates on paper** (no run) for L2: at the M1 state 1 (285 K, 12 kPa, dry) the nucleated-mass rate is
  `J (4/3) pi r*^3 rho_l = 1.2075e-2 /s`, so `y` is about `1.2e-8` at `1e-6 s`; droplets born at `r*` have `1 - r*/r = 0`, and
  a mean radius off `r*` by a fraction `eps` grows away from it at about `1.8e6 eps /s` (free-molecular growth, `Kn` near 400),
  so over `1e-6 s` (`1.8` e-folds) the growth term stays below `1e-5` of the nucleated mass. These estimates set L2's end time
  (`1e-6 s`), not its limits.

No library or test code of F-320 existed when this text was written, and no check below was run.

House rules (as ADR 0217):

- limits are fixed here, before the code and the runs, and are not changed after a run;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the measured number recorded; it is diagnosed,
  not tuned away;
- a defective set-up gets a corrected test by an amendment written before the corrected run; the old one stays as an ignored
  record;
- **no library change**: F-320 adds test code only, in a new file `crates/rcfd-fv/tests/wetsteam_f320.rs`; no existing test file
  is edited (`crates/rcfd-fv/tests/wetsteam.rs` stays as F-221 left it); no case-schema key; `baseline_hashes.rs`,
  `case_hashes_w48u.rs` and the goldens are not touched;
- each registered run is made once; nothing is re-run to pick a result;
- no new third-party crate, unsigned commits, no trailers, no push.

Sources: no new source. The checks use ADR 0217's model M (M1-M8), its solver W (W2 HLLC with the Einfeldt-Roe wave speeds of
Toro ch. 10, W4 the CFL step with the frozen sound speed), its M1 table (values of the F-221 prototype, an implementation written
before the library) and two textbook properties: the frozen sound speed is the isentropic derivative `(dp/drho)_s` at fixed
composition, and an upwind flux whose left wave speed is non-negative is the left state's physical flux. `docs/reference-data.md`
gets one line saying so under F-221.

## What F-320 asks

Roadmap: "New registered checks make mutations MU3 (nucleated-mass term dropped) and MU6 (sound speed in the wave-speed
estimates) fail; the F-221 decision rule is applied again and F-221 becomes partial." The lead's instruction for this branch: the
F-221 status that results from the rule is reported as a recommendation; F-221's roadmap entry is not changed here, only F-320's.

## What the code read shows (c201a770, read only)

1. **The nucleated-mass term (MU3)** is one line of `WetSteamModel::rates`: `dy += nuc.rate * 4/3 pi r*^3 rho_l`. `rates` is used
   by the solver's `sources` and, in the F-221 tests, by L1's reference ODE (`isochoric_reference` calls `m.rates`), so L1 compares
   the solver with an integration of the same `rates` and cannot see a defect inside it. M1 checks `nucleation()` (the rate `J`
   and `r*`) but never `rates()`. In every F-221 fast set-up droplets are present from the start and the growth term dominates
   (ADR 0217: in the L1 box about `7.7e-8` of `y = 1.34e-3` at `1e-5 s`). E1's equilibrium does not involve the term. The quasi-1-D
   reference R-Q1D codes the term itself, but it is a heavy check and the mutations ran only the fast ones.
2. **The sound speed (MU6)** has two sites, and the ADR 0217 mutation changes both: `WetSteamModel::sound_speed` (used only by
   `max_dt`, the W4 step) and the per-side `c_L`, `c_R` inside `hllc` (the W2 wave-speed estimates `S_L = min(u_nL - c_L, u_roe -
   c_roe)`, `S_R = max(u_nR + c_R, u_roe + c_roe)`; `c_roe` comes from `Gamma_roe` and is not mutated). A larger `c_L`, `c_R` widens
   the HLLC fan; the flux stays consistent and conservative and HLLC still resolves an isolated contact exactly, so the shock tube
   S1 stays within 4.0e-3. D1 runs at `y = 0`, where `Gamma(0) = gamma_v` and the mutant is the original. No fast check looks at
   `max_dt` or at a face where the wave-speed sign decides the upwind branch.
3. **What distinguishes the two sound speeds.** With `y > 0` the vapour sound speed exceeds the frozen one by the ratios above
   (2.93 percent at `y = 0.1`). In a flow whose frozen Mach number lies between 1 and that ratio the correct `S_L` is positive (the
   face takes the upwind flux, nothing travels upstream) and the mutant's is negative (the HLLC star flux, information leaks
   upstream). `rates`, `sound_speed`, `max_dt`, `hllc`, `physical_flux`, `to_cons`, `to_prim`, `nucleation`, `growth_rate`,
   `liquid_density` are public, so every check below needs test code only.

## Checks and limits (registered)

All fast (they run in the gate), in the new file `crates/rcfd-fv/tests/wetsteam_f320.rs`, case-203 model parameters (ADR 0217 M2,
`WetSteamModel::for_inlet(35.87e3, 368.3, 11.0)`), the default settings of W5 unless stated.

### For MU3: the nucleated-mass term

- **R1 Rates decomposition at the M1 states.** `rates(rho, e, y, n)` at the three states of ADR 0217's M1 table, with
  `rho = p/((1 - y) R T)`, `e = internal_energy(T, y)` and, at states 2 and 3, `n = 3 y/(4 pi rho_l r^3)` from the table's
  `rho_l` and `r` (state 1: `n = 0`). Reference from the table's prototype values alone: `dy/dt = J (4/3) pi r*^3 rho_l + 3 y
  (dr/dt)/r` (the second term zero at state 1), `dn/dt = J`:

  | state (T K, p Pa, y, r m) | n (1/kg) | nucleated mass (1/s) | growth (1/s) | dy/dt (1/s) | dn/dt = J (1/(kg s)) |
  | --- | --- | --- | --- | --- | --- |
  | 285, 12000, 0, - | 0 | 1.2074962422e-2 | 0 | 1.2074962422e-2 | 2.0308632340e22 |
  | 290, 10000, 0.02, 1.5e-8 | 1.4164720880e18 | 9.3347934069e-8 | 2.3078177049e3 | 2.3078177050e3 | 7.6313126716e16 |
  | 300, 9000, 0.04, 2.0e-8 | 1.1978374189e18 | 1.9698755851e-31 | 1.7344013312e3 | 1.7344013312e3 | 3.4277088937e-8 |

  Limits: `dy/dt` within `5e-6` relative at each state (M1's `1e-6` on `J`, `r*`, `rho_l` and `dr/dt` propagated through the
  products: `1 + 3 + 1`), `dn/dt` within `1e-6` relative (M1's limit on `J`); the returned temperature and pressure equal `T`,
  `p` within `1e-12` relative. State 1 is the one where the nucleated mass is all of `dy/dt`; at states 2 and 3 it is `4e-11` and
  `1e-34` of it, so there R1 checks the growth part of `rates` (droplet radius from `y`, `n` and the growth law).
- **L2 Nucleation box (the set-up where the term matters).** The L1 box of ADR 0217 (2 x 2 cells of 1 mm, slip walls, at rest,
  point-implicit on), started **dry**: uniform `T = 285 K`, `p = 12 kPa`, `y = 0`, `n = 0` (M1 state 1); `dt = 1e-8 s` fixed;
  sampled at `t = 2.5e-7, 5e-7, 1e-6 s` (25, 50, 100 steps). Reference: classic RK4 with `dt = 1e-10 s` of the isochoric ODE at the
  box's `rho` and `e`, with the right-hand side written in the test from the model's public pieces, **not from `rates`**:
  `T = temperature(e, y)`, `p = (1 - y) rho R T`, `(J, r*) = nucleation(T, p, y, rho)`, `rho_l = liquid_density(T)`,
  `dn/dt = J`, `dy/dt = J (4/3) pi r*^3 rho_l + [y > 0 and n > 0] n 4 pi r^2 rho_l growth_rate(T, p, y, rho, r)`,
  `r = (3 y/(4 pi rho_l n))^(1/3)` (the M6 formula, as R-Q1D writes it); the reference also integrates the nucleated-mass part
  alone, `z' = J (4/3) pi r*^3 rho_l`. Limits at each sampled time: solver `y` within `1e-6` relative of the reference, `n` within
  `1e-6` relative; cell spread of `rho y`, `rho n`, `rho E` `<= 1e-12` (as L1); zero clips; and the relevance condition
  `z >= 0.99 y` of the reference (the liquid is nucleated mass to at least 99 percent: the term matters).

### For MU6: the sound speed, at its two sites

- **A1 Frozen sound speed of the EOS.** At five states at rest, `(T K, p Pa, y)` = (300, 20e3, 0), (285, 12e3, 0.02),
  (300, 20e3, 0.05), (290, 10e3, 0.1), (330, 30e3, 0.2), `n = 1e16 /kg` where `y > 0` (else 0): `sound_speed(w)` against the
  isentropic derivative at frozen composition of the library's own EOS, by a central difference along the isentrope's tangent
  (`de = (p/rho^2) drho`): `c^2 = [p(rho (1 + h), e + h p/rho) - p(rho (1 - h), e - h p/rho)]/(2 h rho)`, `h = 1e-5`, where
  `p(rho', e')` is `to_prim` of `(rho', 0, 0, 0, rho' e', rho' y, rho' n)` and `e = to_cons(w)[4]/rho`. Limit: relative `1e-7` on
  `c`. (The M2 pressure is bilinear in `rho` and `e`, so the difference is exact up to round-off; the vapour sound speed differs
  by 0, 0.646, 1.556, 2.93, 5.26 percent at these states.)
- **A2 Time-step bound.** `box_2d(3, 2, 3e-3, 1e-3)` (cells `1 mm x 0.5 mm`, depth 1 mm), condensation off, each A1 state uniform
  with `u = (120, -40, 0) m/s`: `max_dt()` against `cfl dx dy/((|u_x| + c) dy + (|u_y| + c) dx)` (W4's `cfl 2V/sum_f (|u.n| +
  c)|S_f|` over the four non-empty faces) with `c` from A1's difference. Limit: relative `1e-7`.
- **U1 Upwinding of frozen-supersonic face pairs.** `hllc(model, l, r, n)` against `physical_flux` of the upwind state, for three
  pairs whose frozen Mach number `M_f = u_n/c_f` (`c_f^2 = Gamma(y) p/rho`) lies between 1 and the vapour-to-frozen ratio:
  (a) `n = (1, 0, 0)`, `l` = (300 K, 20 kPa, `y` 0.1, `M_f` 1.015 along `n`), `r` = (301 K, 20.1 kPa, `y` 0.1, `M_f` 1.015):
  upwind state `l`; (b) the same states moving along `-n` (`M_f` -1.015): upwind state `r`; (c) `n = (0.6, 0.8, 0)`, `l` =
  (290 K, 10 kPa, `y` 0.1, `M_f` 1.015 along `n` plus 20 m/s along `t = (-0.8, 0.6, 0)`), `r` = (292 K, 10.2 kPa, `y` 0.12, the
  same): upwind state `l`. `n = 1e16 /kg` in all states. Limit: the flux equals the upwind physical flux **bit for bit** (W2 returns
  `F_L` when `S_L >= 0` and `F_R` when `S_R <= 0`). Sizing arithmetic (disclosed above): with the frozen sound speed `S_L/c_l` =
  +0.0150, +0.0147 for (a), (c) and `S_R/c_l = -0.0150` for (b); with the vapour sound speed -0.0143, -0.0143 and +0.0144.
- **U2 Supersonic Riemann problem: nothing travels upstream.** The S1 tube (`box_2d(200, 1, 1 m, 5 mm)`), condensation off,
  `y = 0.1`, `n = 0`; left of `x = 0.5 m` the base state `T = 300 K`, `p = 20 kPa`, `u = 1.015 c_f` along `+x`; right of it the
  same density, velocity and `y` with `p = 20.04 kPa` (0.2 percent higher). `xmin` `Fixed(base)`, `xmax` `Extrapolate`, `ymin`,
  `ymax` `Slip`; 100 steps of `max_dt`. Limit: every cell with centre `x < 0.5 m` keeps the base state, `max(|rho/rho_b - 1|,
  |p/p_b - 1|, |u_x/u_b - 1|, |u_y|/u_b, |y - 0.1|) <= 1e-12` (round-off). Sizing: `S_L/c` at the step +0.0145 with the frozen
  and -0.0143 with the vapour sound speed; the slow wave `u - c` moves 0.2 cells in the 100 steps.

### Negative controls

- **NC3** (U1 can see a non-upwind flux): the three U1 pairs with `|M_f|` 0.985 in place of 1.015 (subsonic): the HLLC flux
  differs from the upwind physical flux, `max_k |F_k - F_up,k| / max_k |F_up,k| > 1e-9` for each pair.
- **NC4** (U2 can see upstream travel): the U2 set-up with `u = 0.985 c_f` (subsonic): the U2 measure over the cells `x < 0.5 m`
  exceeds `1e-10`.

## Mutations (registered)

Each applied to `crates/rcfd-fv/src/wetsteam.rs` of a `git archive` copy of the F-320 test commit (scratch, not committed); for
each mutation the fast checks of both test targets run once in release: `cargo test --release -p rcfd-fv --test wetsteam --test
wetsteam_f320` (ten F-221 fast checks, the eight above). A mutation is **caught** if at least one fast check fails on it (a failed
assertion or a panic); a mutant that does not compile is a defective mutation (an amendment, not a catch).

- **MU1-MU6** exactly as ADR 0217 (the same text substitutions as `f221-scratch/mutate.py`): MU1 `phi = 0`; MU2 the factor
  `(1 - r*/r)` dropped; MU3 the nucleated-mass term `J (4/3) pi r*^3 rho_l` dropped from `rates`; MU4 `q(y) = 0`; MU5 the
  condensation source also added to the mixture density equation; MU6 `gamma_v` in place of `Gamma(y)` in `sound_speed` and in
  `hllc`'s `c_L`, `c_R`.
- **MU6a** only the `hllc` site of MU6; **MU6b** only the `sound_speed` site. (They show that each site has its own check.)

Predictions (mine, not limits): MU3 caught by R1 (state 1: 0 against 1.2075e-2 /s) and L2 (`y` stays 0); MU6 by A1, A2, U1, U2;
MU6a by U1 and U2 only; MU6b by A1 and A2 only; MU1 by R1 (`J` is 93 times larger at state 1) besides the F-221 catches; MU2 by R1
at states 2 and 3; MU4 and MU5 by the F-221 checks only (L2's reference uses the same `temperature` and its `y` is `1e-8`); L2 does
not catch MU1 or MU2, because its reference calls the same `nucleation` and `growth_rate`.

## Decision rules (registered)

**F-320.** Done (`[x]`, maintainer acceptance still required) if R1, L2, A1, A2, U1, U2, NC3, NC4 are met on the unmutated code,
MU3 and MU6 are each caught by at least one of these eight checks, MU6a and MU6b are each caught, MU1, MU2, MU4, MU5 are still
caught, and the gate passes (B1: every existing test passes unchanged). Partial (`[~]`) if MU3 and MU6 are caught by the new checks
but another condition misses (named). Todo if MU3 or MU6 survives, or if a new check misses on the unmutated code in a way that
leaves MU3 or MU6 uncaught. F-320's roadmap entry is set by this rule on this branch; F-221's is not touched (the lead's).

**F-221, the ADR 0217 rule applied again (a recommendation).** The correctness set becomes K' = ADR 0217's K (P1, P2, M1, L1b in
place of L1 by amendment 1, E1, C1, C1b, S1, D1, B1) plus R1, L2, A1, A2, U1, U2, NC3, NC4, and "every mutation caught" is
evaluated for MU1-MU6 against the fast checks of K'. B1 for F-221 is settled by its gate line (ADR 0217) and, for the added test
file, by this branch's gate. The flow set F and V1 keep their ADR 0217 results: F-320 changes no library code, so the heavy runs'
numbers stand (S1n missed on the coarse run; S2, N1, Q1, G1g, NC1, NC2 met; V1 (a) and (b) met).

ADR 0217's rule lists three cases: K, F, V1 all met: partial; K met, F met or missed in part, V1 missed: partial; any miss in K:
todo. The case **K met, F missed in part, V1 met** is not written out. My reading, fixed here before the results: it is partial,
a fortiori from the second case (the same F, a better V1), and the same as the common process rule ("partial if any registered
check is a known miss") and ADR 0217's Q11 ("With Q9 (a) done and met: partial"). The reading is the maintainer's to confirm (Q1
below); the status it gives is reported as a recommendation.

## Not in F-320

- No library change, no heavy rerun (the library is unchanged; the F-221 heavy results stand), no case keys (F-316), no real-gas
  vapour (F-317), no viscous terms (F-318), no moments (F-319), the coarse-run unsteadiness (ADR 0217 Q10) not studied.

## Open questions (with recommendations)

- **Q1. The unwritten case of ADR 0217's rule** (K met, F missed in part, V1 met). Options: (a) partial (the reading above);
  (b) todo until S1n is met on the coarse grid too. Recommendation (a): the coarse-grid steadiness is a flow-set check the rule
  allows to miss in part, and the second case already gives partial with V1 missed.
- **Q2. The new checks in F-221's set.** They are registered here as members of K' for F-221's rule. Recommendation: keep them
  there for any later F-221 work (F-316 to F-319 rerun them).

## Addendum 1: results (2026-11-08; the registered text above is unchanged)

Code: `crates/rcfd-fv/tests/wetsteam_f320.rs` (new; the eight checks), committed as 253d25c7 on top of the registration
8b1be78e before any run. No library file, existing test file, case key, hash table or golden was touched. The checks ran once
(release, `--test-threads=1 --nocapture`, log outside the repository `E:/projects/cfd/w49i-work/f320-checks.log`); the mutations
ran once on a `git archive` copy of 253d25c7 (`E:/projects/cfd/w49i-work/f320-mut`, scripts in `f320-scratch`, log
`f320-mutations.log`).

### Checks on the unmutated code

| Check | Limit | Measured | Verdict |
| --- | --- | --- | --- |
| R1 `dy/dt` vs the M1 decomposition | 5e-6 relative | 1.97e-8, 6.84e-10, 6.71e-10 at states 1, 2, 3 (state 1: 1.2074962185e-2 against 1.2074962422e-2 /s) | met |
| R1 `dn/dt` vs `J` | 1e-6 relative | 2.06e-8, 3.16e-8, 8.10e-8 | met |
| R1 returned `T`, `p` | 1e-12 relative | 0 and at most 2.2e-16 | met |
| L2 `y` vs the ODE without `rates` | 1e-6 relative | 7.9e-13, 9.2e-13, 1.2e-12 at 2.5e-7, 5e-7, 1e-6 s (`y` = 3.0187e-9, 6.0375e-9, 1.2075e-8) | met |
| L2 `n` | 1e-6 relative | 4.4e-14, 4.4e-14, 4.0e-14 (`n` = 5.08e15, 1.02e16, 2.03e16 /kg) | met |
| L2 cell spread / clips | 1e-12 / 0 | 0 / 0 | met |
| L2 relevance `z >= 0.99 y` | 0.99 | `z/y` = 1.0000000312, 1.0000001431, 1.0000007756 (the growth term is slightly negative: `r*` rises with the latent heat released) | met |
| A1 `sound_speed` vs the EOS derivative | 1e-7 relative | 2.5e-13, 4.3e-14, 6.9e-14, 6.4e-13, 2.2e-13 at `y` = 0, 0.02, 0.05, 0.1, 0.2 (vapour value +0, +0.646, +1.556, +2.932, +5.259 percent) | met |
| A2 `max_dt` vs the bound with the EOS `c` | 1e-7 relative | 2.1e-13, 3.7e-14, 6.0e-14, 5.5e-13, 1.8e-13 | met |
| U1 HLLC = upwind physical flux | bit for bit | bit for bit at (a), (b), (c) | met |
| U2 upstream departure, frozen Mach 1.015 | 1e-12 | 2.2e-16 (downstream 2.0e-3; `t` = 1.689e-4 s, 100 steps) | met |
| NC3 subsonic pairs differ from the upwind flux | > 1e-9 | 1.83e-5, 1.82e-5, 2.22e-4 | met |
| NC4 upstream departure, frozen Mach 0.985 | > 1e-10 | 2.02e-4 | met |

### Mutations (the registered command, once each)

`cargo test` stops at the first failing test binary, so where an F-221 check already failed the F-320 target did not run (MU1,
MU2, MU4, MU5); the registered criterion (at least one fast check fails) does not depend on it.

| Mutation | Fast checks that failed | Verdict |
| --- | --- | --- |
| MU1 `phi = 0` | M1, E1, L1b (F-320 target not reached) | caught |
| MU2 `(1 - r*/r)` dropped | M1, E1 (F-320 target not reached) | caught |
| MU3 nucleated-mass term dropped | F-221 checks: none. **R1** (state 1: `dy/dt` 0 against 1.2075e-2 /s), **L2** (`y` 0 against 3.019e-9 at 2.5e-7 s) | **caught** |
| MU4 `q(y) = 0` | C1, C1b, E1, L1b (F-320 target not reached) | caught |
| MU5 source also into the mixture density | C1, C1b, L1b (F-320 target not reached) | caught |
| MU6 `gamma_v` at both sites | F-221 checks: none. **A1** (`y` 0.02: 410.862 against 408.225 m/s), **A2** (2.7922e-7 against 2.8077e-7 s), **U1** ((a): mass flux 63.93239 against the upwind 63.93337), **U2** (upstream departure 8.94e-5) | **caught** |
| MU6a the `hllc` site only | U1, U2 (upstream 9.01e-5); A1, A2 pass | caught |
| MU6b the `sound_speed` site only | A1, A2; U1, U2 pass | caught |

Every prediction that was evaluated held. The ten F-221 fast checks gave the same verdicts for MU1-MU6 as in ADR 0217.

### Status by the decision rules

**F-320:** R1, L2, A1, A2, U1, U2, NC3, NC4 met; MU3 caught by R1 and L2, MU6 by A1, A2, U1 and U2; MU6a and MU6b each caught
by their own checks; MU1, MU2, MU4, MU5 still caught. With the gate passing (B1, below), F-320 is **done** (`[x]`) by its rule;
maintainer acceptance still required.

**F-221 (recommendation; its roadmap entry is not changed on this branch):** K' met (ADR 0217's K as its addendum and gate
line record, plus the eight checks above); every mutation MU1-MU6 caught by the fast checks of K'; F missed only in part (S1n on
the coarse 230 x 10 run, 8.18e-6 at its cap; S2, N1, Q1, G1g, NC1, NC2 met); V1 (a) and (b) met (worst -4.932 percent, onset
+1.90 percent). By the registered reading of the unwritten case (Q1), **F-221 partial** (`[~]`), with the remaining part named as
ADR 0217 Q11 does: case keys (F-316), real-gas vapour (F-317), viscous terms (F-318); polydispersion (F-319) and the coarse-grid
unsteadiness (ADR 0217 Q10) also open; done stays the maintainer's call.

### Supplementary run (not registered, informational; after the registered results)

To evaluate the predictions for the mutations whose F-320 target the registered command did not reach, the F-320 target alone
(`cargo test --release -p rcfd-fv --test wetsteam_f320`) ran once for MU1, MU2, MU4, MU5 on the same copy (log
`f320-supplementary.log`). It changes no verdict above.

| Mutation | F-320 checks that failed | Prediction |
| --- | --- | --- |
| MU1 | R1 (state 1: 1.124 against 1.2075e-2 /s, the factor `1 + phi`); L2 passes (its reference calls the same `nucleation`) | held |
| MU2 | R1 (state 2: 2414.66 against 2307.82 /s), **L2** (`y` 6.051429e-9 against 6.051446e-9 at 2.5e-7 s, 2.8e-6 relative) | R1 held; L2 predicted to pass, it failed: without `(1 - r*/r)` the droplets born at `r*` grow at once (`y` doubles) and the stiff growth separates the solver's step from the fine RK4 |
| MU4 | none | held |
| MU5 | **L2** (`y` 1.2074878e-8 against 1.2074858e-8 at 1e-6 s, 1.7e-6 relative) | predicted to pass, it failed: the source added to the density lowers `e` and `T` by about 1e-8 relative, and `J` responds at about 1/K |

### The gate (2026-11-08)

Gated commit `e8e7359511149bbe8ba0f284e1cfa85b3bdb00bf` (addendum 1 and the F-320 roadmap entry; the branch head differs from it
only by this ADR's text: the supplementary section and this section). A `git archive` copy in `E:/projects/cfd/w49i-work/gate-f320/src`,
`run.sh` from `gate-f301/run.sh`, started by the 5 s poll wrapper `outer2.sh` (one acquire process). The wrapper waited from
17:56 to 21:12 behind the f301 gate, then `run.sh` waited for 19 GB of free memory until 22:02. Results: fmt per package 0,
`clippy --workspace --all-targets --locked -D warnings` 0, tests (release, `scripts/gate_test.py`, three jobs) **3302 passed,
0 failed, 913 ignored** (3302 test processes in 625 binaries, wall 6031 s, lowest free memory 12 GB), the eight F-320 checks
among them. No known flake failed. This settles B1 for F-320: every existing test passes with the new test file, and no library
file changed.
