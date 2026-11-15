# ADR 0570: F-639 registration: the curvature-free fallback of the total Lagrangian contact body

Status: **REGISTERED 2026-11-15, written before any code of F-639 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-639 was written or run in the repository; no probe and no diagnostic run was made.
The arithmetic below was done by hand (no rcfd code, no script). Results go in the addenda at the end; nothing above the
addenda is changed after a run.

Roadmap item F-639 (dep F-623), the follow-up proposed by
[ADR 0551](0551-f623-registration-mortar-contactor-for-quadratic-faces.md) (its remaining question 2: the known misses HS20t and
HS10t, "singular tangent" at the first Newton solve of the total Lagrangian body). Number 0570 given by the lead. Branch
`f639-work` from d1730051 (main), worktree `w49i-work/f639`, scratch `w49i-work/f639-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test or
ignored record edited; no new dependency, no unsafe code, the crate graph unchanged; clean-room; `docs/ROADMAP.md` and
`docs/roadmap.json` not edited (the entries are proposed at the end of the addendum). Every choice the maintainer would normally
be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation) and marked so.

## What F-639 asks

The acceptance, verbatim, binding:

> Every limit not stated here is registered in the ADR before the run. When the exact tangent of the total Lagrangian contact body fails the pivot test, that iteration takes the contact matrix without its curvature part (ADR 0507 amendment 2, as the linear body); ADR 0551 HS20t and HS10t (the mortar contactor, total Lagrangian body) meet radius 0.15, peak 0.20, approach 0.10 with the gap tolerance 1e-8; the existing total Lagrangian contact records and the default bits unchanged.

Clauses: **c1** every other limit registered here before the run; **c2** on the total Lagrangian body an iteration whose exact
tangent fails the pivot test takes the contact matrix without its curvature part, `eps G G^T`; **c3** HS20t and HS10t of ADR
0551 meet radius 0.15, peak 0.20, approach 0.10 with the gap tolerance `1e-8`; **c4** the existing total Lagrangian contact
records unchanged; **c5** the default bits unchanged.

## What was read (no run)

ADR 0551 (whole: D1 to D7, HS20t and HS10t, addendum 1 with the diagnosis of the radius misses of HS20m and HS10m and of the
total Lagrangian failures, the remaining questions), ADR 0507 (D2, D3, amendment 2: the fallback of the linear body, and its
statement "the total Lagrangian path is unchanged (its factorisation is in `nonlinear.rs`, not edited)"; amendment 3), ADR 0542
(D1 to D3: the supernodal factor passed to the total Lagrangian body through `NewtonOptions::factor`; its check P, the total
Lagrangian hub refused at the first tangent), ADR 0566 (D2: `FactorReuse`, one cached analysis per pattern, used by
`contact::solve_with` over the steps and augmentations). Code: `crates/rcfd-fem/src/contact.rs` (the driver `solve_with`,
`linear_solve` with its fallback, `force_and_tangent(.., curvature)`, `convex_tangent`, `begin`, `ContactOptions`,
`NewtonSolve::convex_iterations`), `crates/rcfd-fem/src/nonlinear.rs` (the trait `Contribution`, `contribute_with`,
`add_triplets`, `Problem::attempt_with`, `factor_solve` and its two "singular tangent" texts, `solve_from_reusing`). Tests:
`sa_contact_mortar_hertz_f623.rs` (the HS set-up and its four known-miss records), `f601_common/mod.rs` (`hertz_block`,
`hertz_prescribed`, `hertz_read`, `hertz_theory`, `R_H = 1`, `A_H = 0.1`, `NU_H = 0.3`), `f623_common/mod.rs` (the tangent
models), `sa_newton_supernodal_f618.rs` (`f618_p_refused_pivot_tl_hub`), `f624_common/mod.rs` (`fit_model`, `options`).

Findings that shape the design:

1. The total Lagrangian contact body is `nonlinear::solve_from_reusing` with the contact set as its first `Contribution`; the
   reduced tangent (element tangent plus every contribution's triplets) is factored in `nonlinear::factor_solve`, and a failed
   pivot test ends the attempt with "singular tangent: pivot ..". The driver has no access to a contribution's matrix without
   its curvature part, so the fallback needs a new, defaulted method of the trait.
2. `solve_from` makes one attempt, without halving: a pivot failure of the total Lagrangian contact body ends the whole contact
   solve today. A fallback therefore changes only runs that fail today, but those are default paths too.
3. **Audit of existing tests.** `sa_newton_supernodal_f618.rs::f618_p_refused_pivot_tl_hub` (ADR 0542 P, not ignored) asserts
   that the total Lagrangian hub with ADR 0517's interference is refused at the first tangent with "load step 1 of 1,
   augmentation 0: solver newton: singular tangent: pivot". The two ignored records `hs20t_.._known_miss`, `hs10t_.._known_miss`
   of ADR 0551 record the same failure on the Hertz problem. `sa_newton_supernodal_f618.rs` H3 asserts a "singular tangent"
   without halvings on a model without contact (unaffected: no contribution there offers a fallback). A fallback that were
   always on would turn the refusal of P into something else: **the fallback must be opt-in** (D3). No other test asserts the
   behaviour of a pivot failure on the total Lagrangian body; the `convex_iterations` field is read by the bit records
   `f601_common::result_words`, `sa_contact_f615_default_bits.rs`, `sa_newton_f618_default_bits.rs`, `f624_common::flat`, where
   it is 0 on every total Lagrangian path and stays 0 without the option.
4. The linear body's fallback (ADR 0507 amendment 2) rebuilds the reduced tangent with `force_and_tangent(.., false)` at the
   same state and factors it; the right-hand side is unchanged; the count is `NewtonSolve::convex_iterations`.
5. **The radius reading.** ADR 0551's radius misses on HS20m and HS10m (2.07e-1, 4.89) came from fitting `p^2 = c_0 + c_1 r^2`
   to the nodal values `t_a/A_a`, which are coefficients of the traction in the weighting basis `Phi` and oscillate between
   corner and mid-edge nodes (ADR 0551 diagnosis 1; F-638 is the item that recovers the traction itself). Reading HS20t and
   HS10t the same way would inherit that miss whatever the fallback does. The radius is therefore read here from the moments of
   the nodal forces (D5), a reading derived below from the Hertz distribution and from what the mortar formulation determines,
   fixed now before any run. The peak and the approach are read as ADR 0551 read them (they met there at 2.5e-2 and below).

## Design

### D1. The trait (`crates/rcfd-fem/src/nonlinear.rs`)

`Contribution` gains two methods with defaults:

    /// Whether the contribution has a tangent without its curvature part for an iteration whose exact tangent fails the
    /// pivot test (ADR 0570 D1). The default is false.
    fn has_fallback_tangent(&self) -> bool { false }
    /// The triplets of that tangent at `(u, load_factor)` (no force; nothing recorded). The default adds nothing.
    fn add_fallback_tangent(&self, u: &[f64], load_factor: f64, tangent: &mut Vec<(usize, usize, f64)>) -> Result<()> { .. }

Every existing implementation keeps the defaults (no implementation is edited).

### D2. The driver (`Problem::attempt_with`, c2)

`fallback = control.is_none() && !options.unsymmetric && contributions.iter().any(|c| c.has_fallback_tangent())`, decided once
per attempt. With `fallback` false nothing changes: the same calls in the same order (`contribute_with`, `reduce`,
`factor_solve`), so every path without such a contribution keeps its bits (c5). With `fallback` true, at every iteration:

1. the element tangent `K_e` is kept (a clone) before the contributions are added; each contribution's triplets are collected
   in its own list, the forces summed as `contribute_with` sums them, and the concatenation of the lists (the same triplets in
   the same order as `contribute_with`) added to `K_e` by `add_triplets`: the exact tangent is bit for bit the one without the
   option;
2. the reduced exact tangent is factored by `factor_solve` as now; if that succeeds the iteration is Newton's, unchanged;
3. if it fails with a text beginning "singular tangent" (the pivot refusal of the factor or the pivot post-test, both texts of
   `factor_solve`; the analysis errors "tangent factorisation: .." are not a pivot test and end the attempt as now), the tangent
   is rebuilt as `K_e` plus, per contribution in order, `add_fallback_tangent` at the same `(u, lt)` for a contribution with
   `has_fallback_tangent()`, or its own exact triplets of step 1 otherwise; reduced with the same residual and the same
   increment, factored by `factor_solve` (the same factor, the same `FactorReuse`); its correction and its right-hand side
   (for the energy) are the iteration's. If this factorisation fails too, the attempt ends with "without the curvature part: "
   followed by the factor's text.

The iterate is unchanged by the fallback (the residual is the exact one; only the matrix of that iteration changes), so the
converged state solves the same equations; an iteration near the solution, where the exact tangent factors, is Newton's.

### D3. The contact set (`crates/rcfd-fem/src/contact.rs`, c2, c4, c5)

`ContactOptions` (`#[non_exhaustive]`) gains `pub curvature_fallback: bool`, **default `false`**, and the builder
`curvature_fallback(bool)`. `ContactSet` keeps the flag and a counter (a `Cell<usize>`, reset by `begin`):

- `has_fallback_tangent()` is the flag;
- `add_fallback_tangent(u, ..)` evaluates the set at `u` (as `convex_tangent`, nothing recorded), pushes for every active entry
  and every pair `(r, s)` of its nodes' equations the value `eps G_r G_s` whenever the exact value `eps G_r G_s - t H_rs` or
  that value is non-zero (so the index set is that of the exact triplets of `add`, and the pattern of the reduced tangent,
  hence the cached analysis of ADR 0566, is the same), and adds 1 to the counter;
- in `solve_with` the total Lagrangian path reports `NewtonSolve::convex_iterations` as the counter after the solve (0 without
  the option, as now); the doc of the field names both bodies.

The linear body is unchanged: it falls back always (ADR 0507 amendment 2) and does not read the option. The option is taken
opt-in on the maintainer's standing instruction of 2026-11-11 (take the recommendation), because finding 3: an always-on
fallback would change the default outcome of ADR 0542 P. Making it the default is left open (it changes a default result).

### D4. The models of the checks

- **HS20f, HS10f** (c3; heavy records, ignored, each run once): ADR 0551 HS20t and HS10t exactly (`hertz_block(4, 5, 4)`
  upgraded to Hex20, `h = a/4`; `hertz_block(3, 4, 4)` split and upgraded to Tet10, `h = a/3`; the rigid sphere `centre (0, 0,
  R)`, radius `R = 1`; `hertz_prescribed`; `E = 1`, `nu = 0.3`; penalty `100/h`; `ContactOptions::new(1, 1e-8)` (1 step, gap
  tolerance `1e-8`, 20 augmentations, 3 cycle events), `NewtonOptions::new(1).max_iterations(30)`, the mortar contactor,
  `Kinematics::TotalLagrangian`, `DirectFactor::SupernodalNestedDissection`), plus `.curvature_fallback(true)`. Nothing else
  changes. Readings: `F_fe = 4 |sum of the z reactions over the prescribed equations|` and `hertz_theory(F_fe)` as ADR 0551;
  the **radius** by D5; the **peak** `sqrt(c_0)` of `hertz_read` (ADR 0551's fit over the nodes with `p >= 0.5 max p`, the
  nodal `t_a/A_a`) against `p_H`; the **approach** of `hertz_theory`.
- **HS20m-M, HS10m-M** (supporting the reading; heavy, ignored, each run once): ADR 0551 HS20m and HS10m exactly (the linear
  body, no option needed: the linear body falls back already), read by D5 for the radius.
- **FD** (c2, the driver): a single Hex8 unit cube `hex_box([0.0; 3], [1.0; 3], [1, 1, 1])`, `E = 1`, `nu = 0.3`, the face `zmin`
  fixed in all components (model constraints, value 0), the load `0.01` on `u_z` of the node `(1, 1, 1)` (equation `q`), solved
  by `nonlinear::solve_from` from zero (`NewtonOptions::new(1)`, envelope factor) with one contribution of three kinds: **honest**
  (force `k u_q` at `q`, tangent `(q, q, k)`, `k = 1`, no fallback), **spoiled** (the same force, the exact tangent reported as
  `(q, q, k - 100)`, which makes the reduced tangent indefinite at every iteration since the cube's own diagonal there is below
  1 (`E` times a unit cell), `has_fallback_tangent` true, fallback `(q, q, k)`, a counter of the calls), and **spoiled without
  fallback** (`has_fallback_tangent` false). The spoiled run's fallback rebuilds `K_e + (q, q, k)`, the honest run's tangent is
  `K_e + (q, q, k)` by the same `add_triplets`: the two runs perform the same floating-point operations.
- **FC** (c2, the contact set): ADR 0551's tangent models M20s (`hex20_contactor_model` against the rigid sphere `centre (0.5,
  0.45, 0)`, radius 0.97) and M20q (`hex20_on_hex20_model`, deformable curved Hex20 target), penalty 10, the multipliers `0.1 +
  0.01 k`, the mortar contactor, at `t1_state(n)`, each with `.curvature_fallback(true)`: the triplets of
  `add_fallback_tangent`, summed per `(r, s)`, against those of `convex_tangent`, summed per `(r, s)`; the sequence of
  `(r, s)` of `add_fallback_tangent` against that of `Contribution::add` at the same state; the counter after one call; the flag
  `has_fallback_tangent` with and without the option.
- **NF-HS** (c4, c5; fast): the HS20t set-up of ADR 0551 without the option (`curvature_fallback` left at its default).
- **P-f** (c2 on a contact model of ADR 0542; fast): ADR 0542 P (`fit_model(Setup { n: 4, ..BASE })`, `options(&s,
  Route::Contact)` of `f624_common`, the total Lagrangian hub) with `.curvature_fallback(true)`.

### D5. The radius from the moments of the nodal forces (registered reading)

Hertz's pressure `p(r) = p_0 sqrt(1 - r^2/a^2)` has the moments `M_0 = int p dA = (2/3) pi p_0 a^2` and `M_2 = int p r^2 dA = 2
pi p_0 a^4 int_0^1 s^3 sqrt(1 - s^2) ds = 2 pi p_0 a^4 (2/15)` (with `w = 1 - s^2`: `(1/2) int_0^1 (1 - w) sqrt(w) dw = (1/2)(2/3
- 2/5) = 2/15`), so `M_2/M_0 = (2/5) a^2` and

    a_M = sqrt( (5/2) M_2 / M_0 ),    M_0 = sum_a t_a,    M_2 = sum_a t_a (X_a^2 + Y_a^2),

the sums over the contactor nodes with `t_a > 0` (the nodal forces of `ContactResult::nodes`, `X_a`, `Y_a` the reference
coordinates of the node; both sums over the quarter body, whose ratio is that of the whole). Why this reading and not the fit
of nodal values: the mortar constraint and its multipliers determine the forces `t_a = int Phi_a lambda_h dA` (the traction
tested by the weighting functions), not pointwise values; `sum_a t_a f(X_a) = int lambda_h (sum_a Phi_a f(X_a)) dA` is the
traction tested by a smooth function, which converges where pointwise values oscillate. The interpolant `sum_a Phi_a f(X_a)`
reproduces constants and, on straight edges, linear functions (`sum_a Phi_a X_a = sum_i N_i X_i + sum_e N_e (X_i + X_j)/4 +
sum_e N_e X_e/2 = X` with `X_e = (X_i + X_j)/2`); for `f = r^2` it exceeds `r^2` by `sum_e N_e |X_i - X_j|^2/8` (from `(r_i^2 +
r_j^2)/4 + r_e^2/2 - r_e^2 = |X_i - X_j|^2/8` on a straight edge). This bias is part of the estimate (arithmetic 3), not
corrected. The nodal fit of ADR 0551 (`hertz_read`'s `a_fe`) is printed as well, without a limit.

## Arithmetic done before the code (disclosed)

1. **The scale of the first iterations** (why the curvature part matters, and the uncertainty). The first Newton iteration
   starts from `u = 0` with the far boundary displaced (`delta = a^2/R = 0.01` plus Boussinesq); no contact force (the gap is 0
   at the origin only). Its correction lifts the surface by about `delta`, so the second iterate penetrates the sphere by up to
   `0.01` over `r < sqrt(2 delta) = 0.14`. The penalty `eps_a = (100/h) A_a`, `A_a` about `h^2/6` (ADR 0551 PW; `h = 0.025`: `1e-4`),
   so `eps_a` about `0.4` and the nodal forces up to `t = 4e-3`, about 850 times the equilibrium total over the quarter (`F/4`
   about `3.7e-4` against `sum t` about `eps int pen dA = 4000 (pi/2)(delta rho^2/2 - rho^4/8) = 0.31`, `rho^2 = 0.02`). The
   curvature part `- t d2 gbar` against the sphere is `- t Sum N_b N_c (I - n n^T)/|x - c|`, of order `t/R = 4e-3` per node,
   tangential (no penalty stiffness there), against the elastic tangential stiffness of a node of order `E h = 2.5e-2`. ADR
   0551 measured the pivots `-0.148` (HS20t) and `-0.0336` (HS10t), larger than this estimate of the curvature part at the second
   iterate; the iteration at which they occurred was not recorded, and the tangent's indefiniteness could come from a later
   iterate with larger forces (the curvature part) or from the element's geometric stiffness under a large local strain (which
   the fallback does not change). **This cannot be settled before the run without a probe, and none was made.** If the
   element's part is the cause, the fallback factorisation fails too and HS20f or HS10f misses with "without the curvature
   part"; that would be recorded as a known miss with its diagnosis.
2. **Convergence with the fallback**: the matrix `eps G G^T` is the Gauss-Newton matrix of the penalty energy; the missing part is
   of relative size `t/(R E h)`, at equilibrium (`t` about `p_0 A_a = 0.07 x 1e-4 = 7e-6`) `3e-4`, so once the forces are near
   their equilibrium the exact tangent is expected to factor again and the iteration to become Newton's (ADR 0551 HS20m: Newton
   iterations `[9, 3, 2, ..]` on the linear body). Estimate: the 30 iterations per solve suffice, 10 to 20 augmentations as in
   ADR 0551 (10 and 14 on the linear body), so the gap tolerance `1e-8` is reached; uncertain.
3. **The moment reading** (D5). Bias of the interpolant on the fine contact faces (straight edges, uniform `h`): Quad8
   `int sum_e N_e dA = 4A/3` (the corners `-A/12` each), all four edges `h`: `M_2` raised by `(h^2/8)(4/3) M_0 = (h^2/6) M_0`;
   Tri6 `int sum_e N_e dA = A` (`A/3` each), edges `h, h, h sqrt 2`: `(1/3)(4 h^2)/8 = h^2/6` per unit `M_0`, the same. Against
   `M_2 = 0.4 a^2 M_0`: HS20 (`h = a/4`) `(1/16)/2.4 = 2.6e-2`, `a_M` raised by `1.3e-2`; HS10 (`h = a/3`) `(1/9)/2.4 = 4.6e-2`, `a_M`
   raised by `2.3e-2`. The discretisation of the pressure's edge (a square-root fall over one element): the moments are
   integrals, so the error is of the order of the redistribution over the outer element ring, estimated a few percent of `M_2`.
   Hertz's own approximations in the set-up (ADR 0540 arithmetic 3: the tilt of the sphere's normal, the finite block) are
   within 1e-2. Estimate: `a_M` within 2 to 6 percent of `a_H` on the linear body (HS20m-M, HS10m-M).
4. **Total Lagrangian against small-strain Hertz** (ADR 0551 arithmetic 5): strain at the centre about `p_0/E* = 6.4e-2`; the
   force at a given approach changes by a few percent, the radius by its cube root, the reference positions of the contact
   nodes move radially by at most `(1 - 2 nu)(1 + nu) a p_0/(3 E) = 1.2e-3`, `1.2e-2` of `a`. Estimate: radius and peak within 3
   to 8 percent, the approach within 5 to 10 percent (uncertain). The limits are the acceptance's.
5. **Cost**: ADR 0551: about 11 000 (HS20) and 14 700 (HS10) unknowns; HS20m 107 s and HS10m 142 s on the linear body; the
   supernodal factor of the total Lagrangian body a few seconds per Newton iteration, 50 to 200 iterations in all: several
   minutes to about half an hour each. Heavy records, each run once.
6. **FD**: the spoiled tangent at `q` is `K_e,qq + 1 - 100 < 0` (`K_e,qq` of a unit Hex8 cell with `E = 1` is below 1), so the
   envelope factor fails its pivot test at every iteration; the fallback matrix is the honest one, built by the same calls on
   the same data. Expected: the same bits as the honest run, a fallback at every correction.
7. **FC**: the fallback triplets carry `eps G G^T` on the index set of the exact triplets, `convex_tangent` the same values
   where they are non-zero; summed per entry (adding `+0.0` leaves a non-zero sum's bits unchanged), equal bit for bit.

## Checks (limits fixed now)

Every aggregated error is the maximum of finite values; a non-finite value is a miss of its check. Relative errors are
`|x/x_ref - 1|`. "Bit for bit" compares `f64::to_bits`.

| id | clause | quantity | limit | estimate |
|---|---|---|---|---|
| HS20f | c3 | D4 HS20f (heavy, run once): the solve succeeds with the gap tolerance `1e-8`; radius `a_M` (D5) against `a_H(F_fe)`; peak `sqrt(c_0)` against `p_H`; approach | succeeds; `0.15`, `0.20`, `0.10` | arithmetic 1 to 4 |
| HS10f | c3 | the same on Tet10 | succeeds; `0.15`, `0.20`, `0.10` | arithmetic 1 to 4 |
| FB | c2 | in HS20f and HS10f: the sum of `convex_iterations` over the solves | `>= 1` each | ADR 0551 (the exact tangent failed at augmentation 0) |
| HS20m-M | (D5) | ADR 0551 HS20m (linear body, heavy, run once): succeeds; radius `a_M` against `a_H(F_fe)` | succeeds; `0.15` | arithmetic 3: 2 to 6 percent |
| HS10m-M | (D5) | ADR 0551 HS10m: as HS20m-M | succeeds; `0.15` | arithmetic 3 |
| FD | c2 | D4 FD: the spoiled run succeeds; its displacement, residuals and energies against the honest run; the spoiled contribution's fallback calls against the number of corrections (`energies.len()`); the run without fallback refused with `FemError::Solver { solver: "newton" }` and a reason beginning "singular tangent: pivot" | succeeds; bit for bit; equal, `>= 1`; refused | arithmetic 6 |
| FC | c2 | D4 FC on M20s and M20q: summed triplets of the fallback against `convex_tangent`; the `(r, s)` sequence against `add`'s; the counter after one call; `has_fallback_tangent` true with the option, false without | bit for bit; equal; 1; true, false | arithmetic 7 |
| P-f | c2 | ADR 0542 P with the option: either the solve succeeds with `convex_iterations >= 1` in some solve, or it fails with a reason containing "without the curvature part" | one of the two | D2 (the fallback is attempted where the exact tangent fails) |
| NF-HS | c4, c5 | D4 NF-HS (the default): refused with `FemError::Solver { solver: "contact" }`, the reason containing "load step 1 of 1, augmentation 0", "singular tangent: pivot" and "at node 1129 component 1" (ADR 0551's record) | refused, the texts | D3 (default off) |
| B | c4, c5 | the existing tests and records, unchanged and passing: every test of `rcfd-fem` in release (among them `f618_p_refused_pivot_tl_hub`, `sa_contact_default_bits.rs`, `sa_contact_project_f604_default_bits.rs`, `sa_contact_f615_default_bits.rs`, `sa_contact_f601_default_bits.rs`, `sa_contact_f623_default_bits.rs`, `sa_newton_f618_default_bits.rs`, `sa_newton_reuse_f624.rs`, `sa_follower_default_bits.rs`, `sa_j2_default_bits.rs`); `cargo test -p rcfd-api --release --test baseline_hashes`; `cargo test -p rcfd-cli --release --test docs_site` | pass | D2, D3 |

Recorded without a limit: for HS20f, HS10f, HS20m-M, HS10m-M the augmentations, the Newton iterations and the fallback
iterations per solve, the run time, `F_fe`, the active nodes, `a_M`, the nodal-fit radius `a_fe` of ADR 0551 (its error
against `a_H` printed, no limit: the subject of F-638), the force `4 sum t |n_z|`; for P-f its outcome and numbers.

Negative controls: NF-HS (the default path keeps ADR 0551's refusal) and FD's run without fallback (refused). The two ignored
records of ADR 0551 (`hs20t_.._known_miss`, `hs10t_.._known_miss`) are not edited and not re-run (their runs stand).

### Planned mutations (on a `git archive` copy of the final tree in `f639-scratch/mut`, target `f639-scratch/target-mut`, each alone by exact text replacement, the fast tests of the new file and `sa_newton_supernodal_f618.rs::f618_p_refused_pivot_tl_hub` run once, the file restored)

| id | mutation | expected to be caught by |
|---|---|---|
| MF1 | `ContactSet::has_fallback_tangent` returns `false` | FC (flag), P-f |
| MF2 | `add_fallback_tangent` keeps the curvature part (`- t H`) | FC (values) |
| MF3 | the driver's fallback takes every contribution's exact triplets (no call of `add_fallback_tangent`) | FD (refused) |
| MF4 | the default of `curvature_fallback` set to `true` | NF-HS, `f618_p_refused_pivot_tl_hub` |
| MF5 | the counter not incremented | FC (counter), P-f if it succeeds |
| MF6 | the fallback matrix built on the exact tangent (the clone after the contributions instead of `K_e`) | FD |
| MF7 | the total Lagrangian path reports `convex_iterations` 0 | P-f if it succeeds; otherwise declared not caught by a fast check (FB, heavy, covers it) |

MF7 is declared now: if P-f ends in the failure branch, no fast check sees the reported count, and the mutation is recorded as
uncaught by the fast tests (the heavy FB is not re-run under mutation).

## Decision rule

- **done**: every check met on its registered run (the heavy records included), the negative controls as expected, every planned
  mutation caught by a check named for it (MF7 as declared above counts as caught only if P-f catches it), B unchanged, fmt and
  clippy clean.
- **partial**: B and NF-HS met (defaults and records unchanged) and FD and FC met, and any other miss (or a check not run)
  diagnosed and recorded as a `*_known_miss` test, or a mutation not caught, or a check met only after an amendment.
- **todo**: otherwise, in particular any change of the default bits.

## D6. Files

`crates/rcfd-fem/src/nonlinear.rs` (D1, D2: the trait's two methods, `attempt_with`), `crates/rcfd-fem/src/contact.rs` (D3);
the new test file `crates/rcfd-fem/tests/sa_contact_tl_fallback_f639.rs` (FD, FC, P-f, NF-HS fast; HS20f, HS10f, HS20m-M,
HS10m-M ignored as heavy records), using `f601_common`, `f623_common` and `f624_common` as they are (not edited); docs: the
contact module's doc comment.

## Limits (not covered)

- The fallback acts in the static attempt without a load-factor control and with the symmetric factors (envelope and
  supernodal); the arc-length path and the unsymmetric factor (ADR 0560) are not given it.
- It is opt-in (`ContactOptions::curvature_fallback`); a `ContactSet` used directly as a contribution of `nonlinear::solve_from`
  or `solve_with` carries the flag of the options it was made with.
- It changes the matrix of an iteration, not its residual: an element tangent that is itself indefinite still fails.
- No load-step change of the Hertz set-up (ADR 0551 remaining question 2 (b)); no case-file key, no CLI or API surface.
- The radius of HS20f and HS10f is read from the moments of the nodal forces (D5), not from a recovered traction (F-638).

## Open questions

Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation):

1. **Opt-in or always on for the total Lagrangian body**: (a) always on, as the linear body (changes the outcome of ADR 0542 P,
   a default path); (b) opt-in. Taken (b).
2. **Where the fallback matrix comes from**: (a) a defaulted trait method (D1); (b) a second contact solve on failure (not per
   iteration, changes the iterates). Taken (a).
3. **The radius reading**: (a) ADR 0551's nodal fit (inherits its miss, finding 5); (b) the recovered traction at the load-rule
   points (F-638's subject, not yet built); (c) the moments of the nodal forces (D5). Taken (c).

Open (not taken): making the fallback the default of the total Lagrangian contact body (changes a default result, ADR 0542 P).
