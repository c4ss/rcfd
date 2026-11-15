# ADR 0568: F-637 registration: the pressure drift of the F-bar Hex8 under creep at finite strain

Status: **REGISTERED 2026-11-15, written before any code of F-637 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-637 was written, compiled or run in the repository. The arithmetic below was done by
hand and with plain numpy prototypes in the scratch folder `w49i-work/f637-scratch` (no rcfd code was run; disclosed, their
numbers quoted below): `probe1d.py` (one vectorised one-dimensional solver of the cylinder with the options: one midpoint or two
Gauss points per element, the variational F-bar on or off, the law of the library or the `J`-based variant of D2, and the
mutations MU3, MU4 as flags; the element force from the analytic derivative of the strain, the tangent by central differences),
`probe_battery.py` (log `probe_battery.log`), `probe_pairs.py` (logs `probe_pairs.log`, `probe_mu3.log`). The first MU3 line of
`probe_pairs.log` is a wrong form of the mutation (`E = F_bar - I`), superseded by `probe_mu3.log` (the library's form, below).
The rcfd numbers quoted are those recorded in ADR 0549 and ADR 0563. Results go in the addendum at the end; nothing above the
addendum is changed after a run.

Roadmap item F-637 (deps F-580, F-587), the follow-up 2 of [ADR 0549](0549-f580-registration-creep-on-the-b-bar-hex8-against-a-finite-strain-reference.md)
(its open question 2). Number 0568 given by the lead. Branch `f637-work` from e33e2047, worktree `w49i-work/f637`, scratch
`w49i-work/f637-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test edited; no new crate, no unsafe
code; clean-room. Choices the maintainer would normally be asked about are taken on the maintainer's standing instruction of
2026-11-11 (take the recommendation), as marked.

## What F-637 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. The growth of 4.4e-3 of p of the point-wise stress difference against the finite-strain reference at r = 1.05 (ADR 0549 F4, equal in sigma_r and sigma_theta) diagnosed: its scaling with p/E, its presence in a one-dimensional F-bar proxy, and whether a J-based volumetric part of the law removes it, each with a registered limit; the default element and every existing result bit-identical.

Clauses: (c0) every limit registered here; (c1) the scaling of the growth with `p/E`; (c2) its presence in a one-dimensional
F-bar proxy; (c3) whether a `J`-based volumetric part of the law removes it; (c4) the default element and every existing result
bit-identical. The note of ADR 0563 adds: the pair of runs at `p` and `p/100` cancels the discretisation error linear in the
load and isolates the finite-strain part.

## What was read (no run)

`AGENT_RULES.md`; the brief; ADR 0549 in full (arithmetic 2 to 5, F4, the mutation table and its diagnosis, open question 2);
ADR 0563 in full (the pair of runs, the proxy); ADR 0506 D1 (the variational F-bar); `crates/rcfd-fem/tests/si_fbar.rs` and
`si_fbar_bore.rs` (the helpers); `crates/rcfd-fem/src/nonlinear.rs` (`fbar_kinematics`: `E_bar = E + (alpha^2 - 1)(E + I/2)`,
`alpha^2 = (J_bar/J)^(2/3)`); `crates/rcfd-fem/src/mat_creep.rs` (`return_map`: `sigma_tr = D (E - eps_c)`, the volumetric
part `kappa tr(E - eps_c)`); the scratch probes of ADR 0549 (`probe_fbar1d_loc.py`, `probe_ref1d.py`) and ADR 0563
(`mutate.py`); `docs/roadmap.json` (F-637, F-636, F-580, F-587).

## Definitions, registered

- `d_k(x)`: at a point `x` (library: every stiffness-rule point, polar `S`; proxy: every Gauss point) the vector
  `(sigma_r, sigma_theta)/p - reference/p` at step `k` (steps 40 and 70 are `5 t*` and `20 t*`), the reference that of the
  same law at the same `p` (`N = 400`, linear interpolation, `ref_at`).
- **`F4(p) = max_x |d_70(x) - d_40(x)|_inf`**, ADR 0549's F4 (all points, both components), with the radius of its worst point.
- **The finite-strain part** (ADR 0563's pair): with `q = p/100`, `Gfs(x_p) = max_x |(d_70 - d_40)(x_p) - (d_70 - d_40)(q)|`
  over the same points (the run at `q` with `A = 1/(E q^3)` is the same small-strain problem in `sigma/q`, so the part of the
  growth that is independent of the load cancels, ADR 0563 arithmetic 3).
- **The mismatch** `M`: in the proxy, at every Gauss point the quantity `Q = tr E_bar - ln J_bar` and its deviation from the
  element's weighted mean, `kappa (Q - Q_mean)/p`; `M` is the largest growth of it between steps 40 and 70. It is a function of
  the kinematics only; arithmetic 1 shows why it should be the pressure error of the library's law.

## Arithmetic (before any code; prototypes disclosed)

1. **The mechanism (first principles).** The law's volumetric stress is `kappa tr(E - eps_c)` with `eps_c` traceless, so the
   pressure of a point is `kappa tr E`. With the stretches `lambda_i = exp(e_i)`, `tr E = sum (lambda_i^2 - 1)/2 = ln J +
   sum e_i^2 + O(e^3)`, and with `e_i = ln J/3 + e_i'` (deviatoric logarithmic strains) `tr E = ln J + sum e_i'^2 + O(e^3)`
   for small `ln J`. The continuum at a point fixes `tr E = p_h/kappa` (`p_h` the pressure, `~p`): its `ln J` follows the local
   deviatoric strain, `ln J = p_h/kappa - sum e'^2`. The F-bar gives every point of an element the same `J_bar` (`det F_bar =
   J_bar`), so at its points `kappa tr E_bar = kappa ln J_bar + kappa sum e'^2(x)`: the pressure carries `kappa` times the
   variation of `sum e'^2` inside the element, which the continuum absorbs in `J` and the element cannot (one volume per element).
   This is `kappa (Q - Q_mean)`. Size: `kappa/p = 1/(3(1 - 2 nu) p/E) = 8333`. In the stationary creep state of the plane-strain
   tube `e'_theta = -e'_r = e`, `e ~ e_b (a/r)^2` grows with time, `sum e'^2 = 2 e^2`, its radial derivative `-8 e^2/r`; with
   the Gauss points `h/sqrt 3` apart and `e ~ 7.6e-3` at `r = 1.05` at `20 t*` (equivalent creep strain `9.7e-3` at the bore),
   `h = 8.6e-3` there, the difference between the two points of an element is `kappa 8 e^2 h/(sqrt 3 r) / p ~ 1.8e-2`, about
   `9e-3` at each point from the mean, of which the part grown between `5 t*` and `20 t*` is a fraction: the order of the
   measured `4.4e-3`. Its consequences, each a prediction checked below:
   - **scaling**: `kappa e de h / p` with `e ~ p/E` (the creep strain at `n t*` is a fixed multiple of `p/E`): `F4/p` is
     proportional to `p/E`, the growth is a finite-strain effect, and the pair cancels nothing of it but the factor `1 - q/p`;
   - **mesh**: proportional to `h` (halved on 96 elements);
   - **location**: the largest `e de h ~ r^-5 h(r)`; the graded mesh has `h(r) ~ dR/ds = 2 (R - r0)`, `r0 = a - (b - a)/(e^2 -
     1) = 0.8435`, so `r^-5 (r - r0)` is largest at `r = 5 r0/4 = 1.0543` (ADR 0549's `1.0545`);
   - **equal in `sigma_r` and `sigma_theta`**: a pressure;
   - **the `J`-based law removes it**: with a volumetric stress that depends on `J` alone, every point of an F-bar element has
     the same volumetric stress, as the continuum (D2).
2. **Prototype numbers** (`probe_battery.log`, `probe_pairs.log`; references `N = 400` of the same law; `p = 2e5`):

   | Quantity | Prototype | Library (recorded) |
   |---|---|---|
   | `F4(p)`, F-bar 48, library law | `4.3939e-3` at `r = 1.0549` (`+4.394e-3`, `+4.367e-3`) | `4.396e-3` at `r = 1.0545` (ADR 0549) |
   | `F4(p/10)`, `F4(p/100)` (over their own `p`) | `4.4175e-4`, `4.4146e-5` (ratios `0.1005`, `0.01005`) | not run |
   | `Gfs(p)`, `Gfs(p/10)` | `4.3498e-3`, `3.9760e-4`, ratio `0.09141` (linear in `p/E`: `(1/10 - 1/100)/(1 - 1/100) = 0.090909`) | not run |
   | `M(p)` | `4.3362e-3` at `r = 1.0549` (`F4/M = 1.013`) | |
   | one midpoint, no F-bar, 48 elements (control) | `F4 = 1.09e-6` | |
   | F-bar 96 elements | `2.1950e-3` at `r = 1.0539` (ratio `0.4996`) | |
   | F-bar 48, `J`-based law, against the `J`-based reference | `F4 = 5.86e-5` at `r = 1.127`; `Gfs = 5.81e-5`; `M = 4.334e-3` (the mismatch is still there, the law does not see it) | |
   | `J` law at `p/100`: `F4` | `2.85e-5` (at `r = 1.99`, the load-independent remnant of the transient) | |
   | references 400 against 800 elements (library law, `J` law) | `9.5e-7`, `9.5e-7` | ADR 0549 R1 `2.2e-6` at the FE points |
   | `J` reference against the library-law reference at `p/100`, `20 t*` | `1.5e-6` (the same small-strain problem) | |
   | `J` reference against the library-law reference at `p`, `20 t*` | `2.6e-3` (a different finite-strain problem) | |
   | MU4 (creep rate halved), F-bar 48 | `F4 = 4.30e-3` at `r = 1.0014` | `4.31e-3` at `r = 1.001` (ADR 0549) |
   | MU3 (linear strain in the BBar arm), F-bar 48 | `F4 = 8.40e-3` at `r = 1.0014` | `4.31e-3` at `r = 1.005` (ADR 0549) |

   The proxy is the numpy form of ADR 0549's `probe_fbar1d_loc.py` (it repeats its `4.3939e-3` at `1.0549` to five digits) with
   the derivative of the strain analytic (a central difference of it at `1e-7 p/E` carried a rounding of `1e-5` relative into a
   force multiplied by the stress, and the Newton loop stalled; found while writing the prototype). Under MU3 the proxy and the
   library agree on the location (the bore) and not on the size; the predictions below for MU3 use the library's recorded
   numbers.
3. **The limits.** Each limit is set from the prototype's estimate with a margin for what the three-dimensional mesh adds (the
   24 chords of the bore, the angular variation of the points, ADR 0563 arithmetic 3) and below the value of the alternative it
   is to exclude:
   - scaling (c1): `|rho/0.090909 - 1| <= 5e-2`, `rho = Gfs(p/10)/Gfs(p)` of the library (estimate `5.5e-3`; a quadratic
     scaling would give `rho = 0.0091`, a load-independent growth `0/0`, i.e. a `Gfs` near zero); and the growth is all
     finite-strain: `|Gfs(p)/((1 - q/p) F4(p)) - 1| <= 5e-2` (estimate `4e-5`; a load-independent part `L` would enter as `L/F4`).
     `F4(p/100) <= 2e-4` (estimate `4.4e-5`, the proxy's load-independent floor `2.85e-5`; 4.5 times for the three-dimensional
     floor, 22 times below `F4(p)`). The worst point of `Gfs(p)` in `[1.04, 1.07]` (arithmetic 1: `1.0543`).
   - presence (c2): proxy against library `|F4_proxy/F4_lib - 1| <= 5e-2` (estimate `5e-4`) and the same for `Gfs` (estimate
     about the same); the radii of their worst points within `5e-3` (estimate `4e-4`; one radial point row of the library at
     `r = 1.05` is `5e-3` wide, the bore rows `1.001` and `1.005` under MU3, MU4 are `5e-2` away); the mechanism `|F4_proxy/M - 1|
     <= 5e-2` (estimate `1.3e-2`); the control without F-bar `F4 <= 1e-4` (estimate `1.1e-6`, 44 times below the drift); the
     mesh `F4(96)/F4(48)` in `[0.45, 0.55]` (estimate `0.4996`); the proxy's own scaling `F4(p/10)/F4(p)` in `[0.095, 0.105]`
     (estimate `0.1005`).
   - the `J`-based law (c3): `F4_J(p) <= 3e-4` and `Gfs_J(p) <= 3e-4` (estimates `5.9e-5`, `5.8e-5`; 5 times above, 15 times
     below the drift of the library law); the control that the mismatch is still in the kinematics, `M_J >= 3e-3` (estimate
     `4.33e-3`), so that the removal is the law's, not a changed kinematics; the `J` reference and the library-law reference at
     `p/100` within `1e-5` over steps 0, 40, 70 (estimate `1.5e-6`: the variant is the same law at small strain).
4. **Tolerances.** The references and the proxies stop Newton at `max |du| <= 1e-10 max |u|` (taken on the maintainer's
   standing instruction of 2026-11-11, the recommendation): ADR 0549 amendment 1 measured the rounding floor of the reference at
   `2e-14` at `p` and at most `2e-11` at `p/1000`, so at `p/100` at most about `2e-12`, 50 times below; its effect on a stress
   at most `2e-7` of `p` (ADR 0549 amendment 1 scaled), 200 times below `F4(p/100)`. At most 40 iterations, else a panic (a
   set-up defect, an amendment before any corrected run).
5. **Cost.** Three library runs (`creep_run(BBAR, p | p/10 | p/100)`, about 15 s each in release, ADR 0563), six references and
   eleven proxies (48 to 800 elements, under a second each in release). One test, about one minute.

## Design

Test only, a new file `crates/rcfd-fem/tests/si_fbar_drift.rs`; no library change; `si_fbar.rs` and `si_fbar_bore.rs` not
edited.

- **D1, copied helpers.** From `si_fbar_bore.rs` verbatim (which copied them from `si_fbar.rs`): `Checks`, `worst`, `max_abs`,
  `rebuild`, `with_formulation`, `element_coords`, `element_u`, `newton`, `cylinder`, `PolarPoint`, `polar_points`,
  `creep_material`, `creep_times`, `creep_run`, `ref_return`, `RefElement`, `Reference`, `reference_with`, `ref_at`,
  `ref_differences` (the ones not used are left out). A test binary cannot import another's functions.
- **D2, the `J`-based law** (written in the test, opt-in by construction: nothing in the library changes). The volumetric part
  of the hyperelastic energy `W_vol = (kappa/2) (ln J)^2`, `J = sqrt(det C)`, `C = I + 2 E`, gives `S_vol = 2 dW_vol/dC = kappa
  ln J C^-1` (with `dJ/dC = J C^-1/2`; the standard form, Simo and Hughes, Computational Inelasticity, Springer 1998, cited from
  memory and derived here). The deviatoric part is the library's: the radial return of `s_tr = 2 G (dev E - eps_c)` by the same
  local equation (`q_tr - q - 3 G A dt q^n = 0`), `S = kappa ln J C^-1 + s`. At small strain `ln J = tr E + O(E^2)` and `C^-1 =
  I + O(E)`, so the law is the library's to first order. Under the F-bar `det C_bar = J_bar^2` at every point of the element.
- **D3, the proxy** `fn proxy(p, nel, points, fbar, law) -> Proxy`: the one-dimensional plane-strain total Lagrangian tube of
  ADR 0549 arithmetic 2 (dead bore pressure, the same 71 step ends, the graded radial mesh), `nel` linear elements, one midpoint
  (weight 2) or two Gauss points (`xi = -+1/sqrt 3`), the weight `R(xi) h/2` per point, the stretches `(1 + (u1 - u0)/h,
  1 + u(xi)/R(xi), 1)`; the F-bar of ADR 0549 arithmetic 4 (`J_bar` the weighted mean of `J`, `lambda_bar = (J_bar/J)^(1/3)
  lambda`); `E_bar = (lambda_bar^2 - 1)/2`; the force `sum w (dE_bar/du) . S` with the derivative analytic (`d ln lambda_bar =
  d ln lambda + (d ln J_bar - d ln J)/3`); the law `ref_return` (the library's law, copied) or D2; the element tangent by central
  differences of the force at `1e-7 p/E`, the Thomas solve, the stop of arithmetic 4. It returns the stress over `p` at the points
  of every step as a `Reference` (the copied type, its radii the points', so `ref_at` interpolates a one-point proxy as a
  reference) and `kappa (Q - Q_mean)/p` per point. With one midpoint, no F-bar and the library's law it is the copied
  reference; R0 checks that.
- **D4, measures** `fn growth(d40, d70, radii) -> (F4, r)` and `fn pair(...) -> (Gfs, r)` over a set of points.
- One test `f637_pressure_drift_of_the_f_bar_hex8_under_creep`, run once: the three library runs, the references
  (`reference_with(x, 400, 1e-10, false)` at `p`, `p/10`, `p/100` and `N = 800` at `p`; the `J` references `proxy(x, 400, 1,
  false, J)` at `p`, `p/100` and `N = 800` at `p`), the proxies; all checks printed and aggregated by `Checks`.

## Checks

`p = 2e5`, `q = p/100`. Every measure is required finite before it is compared.

| Id | Clause | Measure | Limit | Estimate |
|---|---|---|---|---|
| L0 | c4 | the three BBar runs complete the 71 step ends (`creep_run` asserts); `F4_lib(p)` repeats ADR 0549's bits: `|F4_lib(p)/4.396e-3 - 1|` | holds; `<= 2e-4` (the printed digits) | the same bits |
| K1 | c1 | `rho = Gfs_lib(p/10)/Gfs_lib(p)`: `|rho/0.090909 - 1|` | `<= 5e-2` | `5.5e-3` |
| K2 | c1 | `|Gfs_lib(p)/(0.99 F4_lib(p)) - 1|` | `<= 5e-2` | `4e-5` |
| K3 | c1 | `F4_lib(p/100)` (over `p/100`) | `<= 2e-4` | `4.4e-5` |
| K4 | c1 | the radius of the worst point of `Gfs_lib(p)` | in `[1.04, 1.07]` | `1.0545` (arithmetic 1: `1.0543`) |
| X1 | c2 | `|F4_proxy(p)/F4_lib(p) - 1|` (proxy: F-bar, 48 elements, two points, the library law) | `<= 5e-2` | `5e-4` |
| X2 | c2 | `|r(F4_proxy) - r(F4_lib)|` | `<= 5e-3` | `4e-4` |
| X3 | c2 | `|Gfs_proxy(p)/Gfs_lib(p) - 1|` | `<= 5e-2` | `<= 5e-3` |
| X4 | c2 | `|F4_proxy(p)/M(p) - 1|` and the radii of their worst points within `5e-3` | `<= 5e-2` | `1.3e-2`, the same point |
| X5 | c2 (control) | one midpoint, no F-bar, 48 elements, the library law: `F4` | `<= 1e-4` | `1.1e-6` |
| X6 | c2 | `F4_proxy` on 96 elements over that on 48 | in `[0.45, 0.55]` | `0.4996` |
| X7 | c2 | `F4_proxy(p/10)/F4_proxy(p)` | in `[0.095, 0.105]` | `0.1005` |
| J1 | c3 | F-bar proxy, 48 elements, the `J` law, against the `J` reference: `F4_J(p)` | `<= 3e-4` | `5.9e-5` |
| J2 | c3 | the same, `Gfs_J(p)` (the proxy and the reference at `q` with the `J` law) | `<= 3e-4` | `5.8e-5` |
| J3 | c3 (control) | `M_J(p)`: the F-bar mismatch is still in the kinematics | `>= 3e-3` | `4.33e-3` |
| J4 | c3 | the `J` reference against the library-law reference at `p/100`, steps 0, 40, 70, at the reference midpoints | `<= 1e-5` | `1.5e-6` |
| R0 | | `proxy(p, 400, 1, false, library law)` against `reference_with(p, 400, 1e-10, false)`, steps 0, 40, 70 | `<= 1e-6` | rounding |
| R1 | | the references 400 against 800 at the library's points (library law) and at the proxy's points (`J` law), steps 40, 70 | `<= 1e-4` | `2.2e-6`, `9.5e-7` |
| U1 | c4 | `git diff e33e2047 -- crates/rcfd-fem/tests/si_fbar.rs crates/rcfd-fem/tests/si_fbar_bore.rs` | empty | not edited |
| S0 | c4 | `git diff --stat e33e2047 -- crates/*/src` | empty | no library change |
| S1 | c4 | `cargo test -p rcfd-fem --release` (all, with the bit records `si_fbar_default_bits`, `sa_creep_default_bits`, `si_hex8x_bits`, `si_hex8x_linear_bits` and the tests of ADR 0549 and 0563), `cargo test -p rcfd-api --release --test baseline_hashes`, `cargo test -p rcfd-cli --release --test docs_site`, `cargo fmt -p rcfd-fem --check`, `cargo clippy -p rcfd-fem --release --all-targets -- -D warnings` | 0 failed, clean | |

Reported without a limit: the signed components of `F4` at its worst point (library and proxy; equal in `sigma_r` and
`sigma_theta` if it is a pressure); `F4_lib(p/10)`; `Gfs` radii; `M` at `p/10`; the `J` law's `F4` at `p/100`; the `J` reference
against the library-law reference at `p`; the largest equivalent creep strains.

## Planned mutations (scratch copy `f637-scratch/mut` of the code commit, its own target dir; each run once against the new test)

| Id | Mutation | Expected catcher |
|---|---|---|
| MU1 | `nonlinear.rs`, `fbar_kinematics`: `a2m1` multiplied by 0 (ADR 0549's MU1: the fully integrated total Lagrangian element) | X1 (the locking growth, ADR 0549: `0.26` of the maximum), L0 |
| MU3 | `nonlinear.rs`, `fbar_kinematics`: `green_lagrange(&h)` replaced by the linear strain (ADR 0549's MU3) | X2 (`F4_lib` worst at `r = 1.005`, ADR 0549), L0 |
| MU4 | `mat_creep.rs`, `time_factor` for `m = 0`: `dt/2` (ADR 0549's MU4) | X2 (worst at `r = 1.001`, ADR 0549), L0 |
| MT1 | the test's proxy: the F-bar factor set to 1 (two points, full integration) | X1, X4 (locking, `~0.26`), X6 |
| MT2 | the test's `J` law: `kappa ln J C^-1` replaced by `kappa tr E I` (the library's volumetric part) | J1, J2 (`4.4e-3`) |

L0 catches every library mutation by itself (it compares with the recorded bits); the catcher named first is the one that
shows the diagnosis sees the mutation. None is declared equivalent.

## Decision rule

- **done**: L0, K1 to K4, X1 to X7, J1 to J4, R0, R1, U1, S0, S1 met in the single registered run, and every planned mutation
  caught by a registered check (or a failed solve).
- **partial**: any registered limit missed (an ignored `*_known_miss` test with the limit intact and the number), a mutation not
  caught, or any check met only after an amendment written after the run.
- **todo**: X1 missed together with K1.

## Limits (what is not covered)

The `J`-based law is tested in the one-dimensional proxy only, against a reference with the same law; the library's Hex8 with
a `J`-based law is not run, because the library has no such material and an opt-in variant of `Material::Creep` would change
the material schema (a golden) and the case-file surface, which needs the maintainer (open question, proposed as a follow-up).
Whether the small-strain law on `E` and `S` or a `J`-based one is the right finite-strain creep model is not decided here (ADR
0549 Limits); the diagnosis is that the F-bar's one volume per element cannot follow the law's `tr E` constraint at second
order. Only the registered mesh, `m = 0`, `n = 4`, `b/a = 2`, `nu = 0.3`. No case-file key.

## Addendum 1: results (2026-11-15)

Code: commit 4d84f85d (test only: the new file `crates/rcfd-fem/tests/si_fbar_drift.rs`, the copied helpers, `Law`,
`j_return`, `Proxy`, `proxy`, `proxy_growth`, `library_growth`, `largest`, `pair`, `mismatch_growth`, `ref_gap` and the test);
after the run, commit 1af12daf turned the registered test into the ignored record
`f637_pressure_drift_of_the_f_bar_hex8_under_creep_known_miss` (J4 a check, the limit intact) and added
`f637_pressure_drift_of_the_f_bar_hex8_under_creep_met_checks` (the same body and limits, J4 printed, not asserted), so the
suite runs the met checks. No library file and no existing test changed. Logs in `w49i-work/f637-scratch`: `run1-f637.log` (the
registered run, single, 60 s in release), `probe_j4.log` (the numpy diagnosis of J4 after the run), `mutations.log`,
`mut-MU1.log`, `mut-MU3.log`, `mut-MU4.log`, `mut-MT1.log`, `mut-MT2.log`, `s1-rcfd-fem.log`, `s2-baseline.log`,
`s2-docs.log`, `s3-fmt.log`, `s3-clippy.log`.

| Id | Measured | Limit | Result |
|---|---|---|---|
| L0 | three BBar runs, 71 step ends each, 0 halvings; `F4_lib(p) = 4.3960e-3` at `r = 1.0545`, `8.7e-6` from ADR 0549's printed `4.396e-3` | holds; `<= 2e-4` | met |
| K1 | `rho = 0.091399`: `5.39e-3` from the linear `0.090909` | `<= 5e-2` | met (estimate `5.5e-3`) |
| K2 | `Gfs_lib(p) = 4.3518e-3`: `4.29e-5` | `<= 5e-2` | met (estimate `4e-5`) |
| K3 | `F4_lib(p/100) = 4.4146e-5` | `<= 2e-4` | met (estimate `4.4e-5`) |
| K4 | `r = 1.0545` | `[1.04, 1.07]` | met (arithmetic 1: `1.0543`) |
| X1 | proxy `4.3939e-3`: `4.63e-4` | `<= 5e-2` | met |
| X2 | `1.0549` against `1.0545`: `3.8e-4` | `<= 5e-3` | met |
| X3 | proxy `Gfs = 4.3498e-3`: `4.67e-4` | `<= 5e-2` | met |
| X4 | `M = 4.3362e-3`: `1.33e-2`; the same point | `<= 5e-2`; `<= 5e-3` | met |
| X5 | `1.094e-6` | `<= 1e-4` | met |
| X6 | `2.1950e-3 / 4.3939e-3 = 0.49955` | `[0.45, 0.55]` | met |
| X7 | `4.4175e-4 / 4.3939e-3 = 0.10054` | `[0.095, 0.105]` | met |
| J1 | `F4_J(p) = 5.859e-5` at `r = 1.127` | `<= 3e-4` | met |
| J2 | `Gfs_J(p) = 5.807e-5` | `<= 3e-4` | met |
| J3 | `M_J(p) = 4.334e-3` | `>= 3e-3` | met |
| J4 | **`1.2975e-4`** | `<= 1e-5` | **missed** (the estimate `1.5e-6` was wrong, below) |
| R0 | `1.95e-11` | `<= 1e-6` | met |
| R1 | `9.59e-7` (library law, library points); `9.43e-7` (`J` law, proxy points) | `<= 1e-4` | met |
| U1 | `git diff e33e2047 -- crates/rcfd-fem/tests/si_fbar.rs crates/rcfd-fem/tests/si_fbar_bore.rs` empty | empty | met |
| S0 | `git diff --stat e33e2047 -- crates/*/src` empty | empty | met |
| S1 | `cargo test -p rcfd-fem --release`: 270 test binaries, 1139 passed, 0 failed, 304 ignored (the bit records and the tests of ADR 0549 and 0563 among the passed); `baseline_hashes` 3 passed, 1 ignored (as on the base); `docs_site` 2 passed; `cargo fmt -p rcfd-fem --check` clean; `cargo clippy -p rcfd-fem --release --all-targets -- -D warnings` clean | 0 failed, clean | met |

Reported: the library's `F4` components at the worst point `(+4.3960e-3, +4.3714e-3)` at `p`, `(+4.4190e-4, +4.3947e-4)` at
`p/10`, `(+4.4146e-5, +4.3878e-5)` at `p/100`, all at `r = 1.0545` (a pressure: one sign, the two components equal to `6e-3`);
`F4_lib(p/10)/F4_lib(p) = 0.1005`, `F4_lib(p/100)/F4_lib(p) = 0.01004`; `Gfs_lib(p/10) = 3.9775e-4` at `1.0545`; the proxy
repeats the library to four digits at every pressure; `F4_J(p/100) = 2.85e-5` at `r = 1.99` (the load-independent remnant of
the transient at the outer surface); the `J` reference against the library-law reference at `p`, `20 t*`: `1.27e-2`; the largest
equivalent creep strains: library `9.667e-3`, `9.653e-4`, `9.652e-5`, reference `9.715e-3`, `9.701e-4`, `9.700e-5`.

### The diagnosis (from the registered numbers)

The growth of ADR 0549's F4 (`4.40e-3` of `p` at `r = 1.0545`) is a pressure error of the F-bar element, finite-strain in
origin and `O(h)` in size. (c1) It is proportional to `p/E`: `F4/p` falls by `0.1005` and `0.01004` at `p/10` and `p/100`; the
finite-strain part isolated by the pair scales as `rho = 0.0914` against the linear `0.0909`, and it is the whole growth
(`Gfs/(0.99 F4) - 1 = 4e-5`). (c2) The one-dimensional F-bar proxy has it to four digits at the same point; without F-bar (one
midpoint per element) it is gone (`1.1e-6`); on twice the elements it halves (`0.4996`); and it equals to `1.3e-2` the kinematic
mismatch `kappa (Q - Q_mean)/p` of arithmetic 1, at the point predicted by `r = 5 r0/4`: the law's pressure `kappa tr E_bar`
contains `kappa sum e'^2`, which varies inside the element and which the continuum absorbs in `J`, while the F-bar gives the
element one `J`. (c3) With the `J`-based volumetric part `kappa ln J C^-1` the proxy's growth against the reference of the same
law falls 75-fold to `5.9e-5` (`Gfs_J = 5.8e-5`) although the mismatch is still in the kinematics (`M_J = 4.33e-3`): the
`J`-based law removes it, in the proxy.

### J4, the miss

J4 was to show that the `J` variant is the library's law at small strain: the two references at `p/100` within `1e-5` over steps
0, 40, 70 and the three components. Measured `1.2975e-4`. Diagnosis (`probe_j4.py`, numpy, after the run; not a re-run of the
test): the gap is in `S_zz` at the bore at `20 t*` (`1.30e-4`; `S_r` `1.5e-6`, `S_theta` `2.3e-5`); at `p` it is `1.27e-2`, a
ratio `0.0102`, linear in `p/E`. The estimate was wrong because the prototype printed `S_r` only (`1.5e-6`), and the
registration read the agreement as second order: `kappa ln J C^-1` and `kappa tr E I` differ at first order in the strain
relative to the stress (`C^-1 = I - 2E + ...` times a volumetric stress of order `p`), so the gap is of order `p` times the creep
strain, `1e-4` of `p` at `p/100`. The two laws agree in the limit `p/E -> 0`, not to `1e-5` at `p/100`. The miss does not touch
J1 to J3, which compare the proxy with a reference of the same law. Recorded in the ignored
`f637_pressure_drift_of_the_f_bar_hex8_under_creep_known_miss`, the limit intact.

### Mutations (scratch copy `f637-scratch/mut` of 1af12daf, target `f637-scratch/mut-target`, each run once against the met-checks test, the file restored after)

| Id | Result | Caught by |
|---|---|---|
| MU1 (`a2m1` times 0) | `F4_lib(p) = 0.261` at `r = 1.255`; L0, K2, K3, K4, X1 (`0.98`), X2, X3 missed | **caught** (X1, as predicted, and L0) |
| MU3 (linear strain in `fbar_kinematics`) | `F4_lib(p) = 4.31e-3` at `r = 1.0049` (components `-2.1e-3`, `-4.3e-3`: no longer a pressure); L0, K4, X2 (`5.0e-2`) missed | **caught** (X2, as predicted, and L0) |
| MU4 (time factor `dt/2`) | `F4_lib(p) = 4.31e-3` at `r = 1.0010`; L0, K3 (`1.9e-3`), K4, X2 (`5.4e-2`) missed | **caught** (X2, as predicted, and L0) |
| MT1 (the proxy's F-bar off) | proxy `F4 = 0.261`; X1, X2, X3, X4, J1, J2, X7 missed | **caught** (X1, X4, as predicted; X6 was met: the locking error halves too) |
| MT2 (the `J` law's volumetric part `kappa tr E I`) | `F4_J = 4.39e-3`; J1, J2 missed | **caught** (J1, J2, as predicted) |

### Deviations

- **J4 missed** in the single registered run (above); not tuned. After the run the registered test became the ignored record, and
  a test of the met checks (the same body and limits, J4 printed) was added so that the suite runs them (commit 1af12daf); no
  limit and no measure changed.
- MT1 was predicted to be caught also by X6; X6 was met under MT1 (the locking error of the fully integrated proxy is `O(h)`
  too). X1 and X4 caught it, as predicted.
- Nothing registered was skipped.

### Decision (by the rule)

Every registered check but J4 is met in the single registered run, and every planned mutation is caught; J4 is missed (`1.30e-4`
against `1e-5`). **Proposed status: partial.** The three diagnostic clauses of the acceptance (scaling, proxy, `J`-based law)
are met by their registered checks; the miss is in the control that the variant equals the library's law at small strain, whose
estimate was wrong (the agreement is first order, not second order).

### Remaining questions

1. **J4.** Options: (a) leave it as the known miss (the diagnosis does not depend on it); (b) re-register the control as a
   scaling, `gap(p/100)/gap(p) <= 2e-2` (measured `0.0102`), in a follow-up. Recommendation: (a); the item is diagnostic and the
   scaling of the gap is reported here.
2. **The `J`-based law in the library.** The library's Hex8 was not run with a `J`-based law: `Material::Creep` has no such
   option, and adding one changes the material schema golden and the case-file surface. Options: (a) an opt-in volumetric variant
   of the creep material (`kappa ln J C^-1`, consistent tangent `kappa C^-1 (x) C^-1 - 2 kappa ln J dC^-1/dC`), defaults
   bit-identical, the schema golden re-recorded with the maintainer's approval; (b) leave the drift as a documented property of
   the small-strain law on `E`. Recommendation: (a), as a follow-up that needs the maintainer for the schema.
3. **The bore-zone bound of ADR 0563** (its open question 1): the growth `G` there is this drift; a deviatoric measure
   `sigma_theta - sigma_r` would not carry it (the two components grow by `4.396e-3` and `4.371e-3`). Recommendation: take it in
   the follow-up of question 2, where the drift is removed in the element itself.
