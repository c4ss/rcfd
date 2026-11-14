# ADR 0455: F-531 registration: the viscous flux of the Symmetry condition for flow tangential to the plane

Status: **REGISTERED 2026-11-14, written before any code of F-531 and before any run of its checks.** Not yet accepted by the
maintainer. Results go in the addendum at the end; nothing above the addendum is changed after a run.

Parents: [ADR 0446](0446-f421-registration-aerodynamic-damping-energy-method.md) (F-421: amendment 3, the `Symmetry` planes of
the one-cell-deep channel add `+13 %` (QF) and `+7 %` (W5) to the real part of the impedance; diagnosis 1, the excess is per
length of the developed flow), [ADR 0434](0434-f401-registration-piston-on-an-elastic-bar.md) (F-401: the miss VS, a
tip motion proportional to `nu` on a plug with four `Symmetry` side walls), [ADR 0451](0451-f530-registration-piso-stability-at-large-diffusion-numbers.md)
(F-530: the fixed-mesh channel; the default PISO step needs `nu dt/dy^2` below about 2.5), [ADR 0010](0010-component-diagonal.md)
(the averaged and the corrected component diagonal of the segregated kernels), ADR 0069 F3 and ADR 0074 W1 (the corrected
split in the transient kernel, `PimpleSettings::component_diagonal`, case key `solver.component_diagonal = "corrected"`), and
the analysis page [component-diagonal-default.md](../component-diagonal-default.md) (what a corrected default would move; the
maintainer's open decision). Roadmap item F-531 (deps F-421). Number 0455 given by the lead. Branch `f531-symmetry-flux` (from
main `2aea1247`), worktree `w49i-work/f531`, scratch `w49i-work/f531-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here before the code and the runs; each registered run made once; a
missed limit becomes an ignored `*_known_miss` test with the limit intact; defaults bit-identical; no edit of
`baseline_hashes.rs`, the hash tables or a golden file; existing tests are not edited (F-401's VS is repeated in a copy); no new
dependency, no unsafe code; clean-room (every formula below is derived here from the momentum equation and the discrete
operators as they stand in the repository; nothing is taken from literature).

## What F-531 asks

The acceptance of the roadmap entry, binding: "Every limit not stated here is registered in the ADR before the run. Stokes'
second problem (a wall oscillating in its plane, the fluid bounded by a Symmetry plane at a registered distance) and the
oscillating plane channel of ADR 0446 between the two Symmetry planes of a one-cell reduction give the closed-form wall shear
and impedance within registered limits; the extra dissipation measured by ADR 0446 (+7 to +13 percent of the real part) is
explained and removed or documented; F-401's VS check is repeated with the result."

Clauses: (c0) every other limit registered here; (c1) Stokes' second problem with a `Symmetry` plane at a registered distance:
closed-form wall shear and impedance within registered limits; (c2) the oscillating channel of ADR 0446 between two `Symmetry`
planes of a one-cell reduction: the closed-form impedance within registered limits; (c3) the extra dissipation of ADR 0446
explained, and removed or documented; (c4) F-401's VS check repeated with the result.

## What I read, and the arithmetic done before this registration (disclosed)

Read: ADR 0446 (D3, D6, amendments 2 and 3, addendum), ADR 0434 (D3, VS and its diagnosis), ADR 0451, ADR 0010, the page
`docs/component-diagonal-default.md`; the code `crates/rcfd-fv/src/bc.rs` (`Symmetry`, `FaceCoeffs`), `diffusion.rs`
(`laplacian`), `pressure.rs` (`pressure_bc`), `pimple.rs` (the momentum predictor and the correctors of `PimpleSolver::step`,
lines 1690 to 2150 at `2aea1247`, and the same split in the lag estimate, lines 2280 to 2330), `sixdof.rs`
(`patch_face_forces`), `rcfd-post/src/solid_aerodamp.rs` (`periodic_response`, its time convention `t_n = n dt` and
`Z = (c, w m_a) = -F/(i w Q)` for `q = Q e^{i w t}`), and the tests `sa_aerodamp.rs`, `piso_diffusion_f530.rs`,
`si_fsi_piston.rs`, `rcfd-fv/tests/ale_flows.rs`.

Arithmetic (Python, `f531-scratch/predict.py` and `stokes_predict.py`, no solver run): the models of D2 and D3 below, evaluated
at the parameters of this registration and of ADR 0446. The ADR 0446 numbers used for comparison (QF `3.3926`, `3.0318`,
`3.3961`; W5's per-length part `1.0993 + 5.6755 i`) were already published there, so their agreement with D1 below is a
**postdiction**, stated as such; the predictions for the new runs (Stokes, the corrected channel, VS) are made here before any
of them.

## D1. The diagnosis (from the code, before any run)

**The Symmetry condition's own viscous flux is right.** For a vector field `Symmetry::coeffs` gives, for component `k` on a face
of unit normal `n`, the face value `u_f,k = (1 - n_k^2) u_P,k - n_k sum_{j != k} n_j u_P,j` (`FaceCoeffs::from_value`, gradient
coefficient `c = (a - 1)/dn`). On a plane normal to an axis (`n = e_z` here) the tangential components get `a = 1, b = 0, c = 0,
d = 0`: exactly zero gradient, so the Laplacian puts **no** viscous flux of the tangential momentum through the plane, as the
condition intends. The normal component gets `a = 0, c = -1/dn`: a Dirichlet zero, an implicit diagonal term `nu |S_f|/dn` in
the normal component's matrix only. The pressure condition (`pressure_bc`) is the zero normal gradient on such a face, as for
the default zero-gradient velocity; the face flux is `u_b . S = 0`.

**The cause is the averaged component diagonal of the PISO correctors.** `PimpleSolver::step` adds the diagonals of the
component matrices (`a_p[c] += m.a.diag[c]`, line 1798) and divides by the number of active components (`n_active`), then forms
`rAU = V/a_P` and, by default (`component_diagonal = false`), `HbyA_k = H_k/a_P` (line 1906 to 1911), `H_k = b_k - sum_nb a_nb
u_nb`, and the corrected velocity `u_k = HbyA_k - rAU (grad p)_k`. The momentum predictor solved `a_k u_k = H_k - V (grad p)_k`
with the component's own diagonal `a_k`. Where the correctors reach their fixed point the velocity satisfies

`a_P u_k = H_k(u) - V (grad p)_k`, i.e. `a_k u_k = H_k(u) - V (grad p)_k - (a_P - a_k) u_k`:

the momentum equation of the tangential component with an extra linear sink `lambda_c u_k`, `lambda_c = (a_P - a_k)/V`, in every
cell next to the plane. With the option `component_diagonal = true` (ADR 0069 F3; case key `solver.component_diagonal =
"corrected"`, ADR 0074 W1) `HbyA_k = (H_k + (a_P - a_k) u_k)/a_P` and the fixed point is the component's own equation: the
sink is removed exactly. This is the defect ADR 0010 found in the steady kernels ("acts like a partial no-slip wall"), here in
the transient kernel; it is not a defect of the `Symmetry` coefficients.

For an axis-aligned plane the normal component's extra diagonal is `nu |S_f|/(d/2)` (`dn = d/2`, `d` the cell thickness normal
to the plane), per volume `2 nu/d^2`; the tangential component's diagonal does not change, so

`lambda_c = (1/n_active) sum over the Symmetry faces of the cell of 2 nu/d_f^2` (planes normal to the tangential direction itself excluded).

* **The one-cell-deep channel of ADR 0446** (`zmin`, `zmax` `Symmetry`, depth `h`, three active components): every cell has two
  such faces, `lambda = (1/3)(2 + 2) nu/h^2 = 4 nu/(3 h^2)`, uniform. The channel then obeys `(s + lambda) u = -p_x/rho + nu u_yy`:
  in the formula and the discrete model of ADR 0446 `s` is replaced by `s + lambda` inside the profile factor,
  `Z_lambda = A rho L_f (i w + lambda)/(1 - tanh(kappa')/kappa')`, `kappa' = (h/2) sqrt((i w + lambda)/nu)`, and discretely
  `Z_dt,lambda = A rho L_f G(s + lambda) s/(i w)` (ADR 0451 D2's `G`). Evaluated (postdiction): QF (`Wo = 1`, 8 cells, `L_f = 20
  h`, `N = 100`): `Z_dt = 3.0041 + 1.0842 i`, `Z_dt,lambda = 3.3956 + 1.0670 i`, `+13.0 %` real; ADR 0446 measured `3.3926 +
  1.079 i` with `Symmetry`, `3.3961` with 10 outer iterations, `3.0318` with zero gradient. W5 (`Wo = 5`, 40 cells, `N = 400`):
  per length `100 h`, `1.0283 + 5.6802 i` and `1.1002 + 5.6749 i`, `+7.0 %` real; ADR 0446 measured the per-length part `1.0993
  + 5.6755 i` (`+6.9 %`). The "length doubling" diagnosis of ADR 0446 (the excess is per length) follows: `lambda` acts in the
  whole developed flow.
* **Stokes' second problem with a Symmetry top** (2-D, `frontAndBack` empty, two active components): only the top cell has the
  face, `lambda_top = (1/2) 2 nu/dy^2 = nu/dy^2`: a tangential friction `nu u_top/dy` per area of the plane, half that of a
  no-slip wall at the plane (`2 nu/dy`). It **grows** under refinement, so the default's error does not vanish with the mesh
  (D3 gives the numbers).
* **F-401's open column** (4 x 4 cells across, `d = 0.25 m`, four `Symmetry` side walls, three active components): edge cells
  `lambda = (2/3) nu/d^2`, corner cells `(4/3) nu/d^2`; the mean over the section (8 edge, 4 corner, 4 interior cells) is
  `(2/3) nu/d^2 = 10.7 nu`. On the plug it acts as a viscous damping force `rho_f A L_f lambda_mean v` proportional to `nu`:
  the kind of effect VS found. Order of magnitude (not a registered limit): a modal damping ratio `lambda_mean (m_a/(m_s +
  m_a))/(2 w)` = `10.7e-2 x 0.921/(2 x 605.8)` = `8.1e-5` at `nu = 1e-2` (ratio 4, `w` the root of ADR 0434 D1, the shares of
  ADR 0446 B1), giving a tip deviation of order `0.56 zeta U` = `4.5e-5 U` within the first quarter period, against the measured
  `8.28e-5 U`: the same order; the end wall's own splitting (ADR 0252) is not estimated.

So the expected answer to (c3) is: **explained** (the averaged diagonal, D1), **removed** by the existing opt-in
`component_diagonal`, and **documented** for the default, which stays averaged (a corrected default moves the registered
`packed_bed` hash, `component-diagonal-default.md`; the maintainer's open decision, not taken here). No library line is
planned. If a run contradicts D1 (S0a or Q3 below), the cause is elsewhere and is located before anything is changed.

## D2. Stokes' second problem in a finite layer (closed form)

A wall at `y = 0` moves in its plane with `u_w = Re(V e^{i w t})`; the fluid fills `0 < y < H` and `y = H` is a symmetry plane
(no shear, no normal flow). Parallel flow `u(y, t)`, no pressure gradient (the flow is uniform in `x`; the pressure is
constant): `u_t = nu u_yy`, `u(0) = u_w`, `u_y(H) = 0`. With `u = Re(U(y) e^{i w t})`: `i w U = nu U''`, `U = V cosh(k (H -
y))/cosh(k H)`, `k = sqrt(i w/nu) = (1 + i)/delta`, `delta = sqrt(2 nu/w)`. The shear stress of the fluid on the wall (the
`x` load per area, `rho nu u_y(0)`) is `tau_w = -rho nu V k tanh(k H)`. The impedance (force resisting the motion per
velocity, the convention of `periodic_response`) per area is

`Z/A = rho nu k tanh(k H) = rho sqrt(i w nu) tanh(k H)`;

the wall shear's amplitude is `|Z|/A |V|` and it leads the wall velocity by `arg(Z/A) + pi` (opposes it). Limits: `H -> infinity`
gives the semi-infinite Stokes layer `rho sqrt(w nu) e^{i pi/4}`; `H -> 0` gives `rho i w H` (the layer moves as a rigid slab).

Parameters: `H = 1 m`, `w = 2 pi rad/s`, `nu = w H^2/2 = pi m^2/s` (so `delta = H`, `k H = 1 + i`: the symmetry plane is in the
layer, `|tanh(k H)|` differs from 1 by 0.3), `rho = 1000 kg/m^3`. `Z/A = 2551.5096 + 4258.9815 i` (Pa s/m).

Set-up: `box_2d(1, n_y, H/n_y, H, false, false)` (one column of square cells, `frontAndBack` empty, two active components);
`ymin` `FixedValue` `(v_n, 0, 0)`, `v_n = (q_n - q_{n-1})/dt`, `q_n = q0 sin(w n dt)`, `q0 = 1e-3 m`; `ymax` `Symmetry`;
`xmin`, `xmax` `ZeroGradient` (one cell wide: the inflow equals the outflow); `p = 0` (`FixedValue`) at `xmax`, zero gradient
elsewhere; `PimpleSettings::piso(nu)`, central convection, momentum and pressure solvers `tol_abs 1e-16`, `tol_rel 1e-13`,
`max_iter 5000` (as ADR 0446), `component_diagonal` as stated per run; fluid at rest at `t = 0`. The load `F_n` is the `x` sum of
`patch_face_forces` on `ymin` after step `n` (`rho nu A (u_0 - v_n)/(dy/2)` on this mesh), and `Z = periodic_response(F, N, q0,
w, 1e-6).impedance()` of the last of `C = 4` cycles (the slowest transient, `nu (pi/2H)^2 = 7.75 /s`, decays by `4.3e-4` per
period: the change between cycles 3 and 4 is of order `1e-10`). `A` is the area of the `ymin` faces read from the mesh.

Levels: `n_y = 20`, `N = 1000, 2000` and `n_y = 40`, `N = 4000, 8000` (diffusion numbers `nu dt/dy^2 = 1.26, 0.63` at both;
below the 2.5 of ADR 0451, although the pressure here is trivially uniform). Extrapolated `Z_ext = 2 Z_2N - Z_N` (implicit Euler
is first order in `dt`).

## D3. The discrete models of the column (exact for the step as it stands)

Per volume, with `kap = nu/dy^2`, cell 0 at the wall and cell `n_y - 1` at the plane: the `x` matrix has the diagonal
`a_x,i = 1/dt + kap (l_i + r_i)`, `l_0 = 2` (half cell to the wall), `l_i = 1` otherwise, `r_{n-1} = 0` (`Symmetry`: zero
gradient for `u_x`), `r_i = 1` otherwise, off-diagonals `-kap`; the `y` matrix the same except `r_{n-1} = 2` (the Dirichlet zero
of the normal component); right side `b = u_old/dt + 2 kap v e_0`. The step (`n_outer = 1`, `n_correctors = 2`; the pressure
stays zero because every `y` flux and `HbyA_y` is zero): predictor `u* = A_x^{-1} b`, then twice `u <- (b - O u + c (a_P -
a_x) u)/a_P`, `a_P = (a_x + a_y)/2`, `c = 1` with `component_diagonal`, `c = 0` by default. With `c = 1` the corrector returns
`u*` exactly (`b - O u* = a_x u*`): the step is implicit Euler ("model IE"). With `c = 0` it is the averaged step ("model AV").
Both are linear, `u_n = T u_{n-1} + g v_n`; the periodic state is `U = (I - T e^{-i w dt})^{-1} g`, `F/v = rho nu (U_0 - 1)/(dy/2)`,
and `Z/A = -(F/v) s/(i w)`, `s = (1 - e^{-i w dt})/dt` (`v = s Q`). The test builds `T` column by column with the same
arithmetic and solves the complex system by Gaussian elimination.

Predictions (`stokes_predict.py`): model IE `n_y = 20`: `Z_1000 = 2573.0286 + 4238.7189 i`, `Z_2000 = 2563.5479 + 4247.6763 i`,
`Z_ext` real `+1.00e-3`, imaginary `-5.51e-4` from D2; `n_y = 40`: `Z_ext` `+2.49e-4`, `-1.39e-4`. Model AV `n_y = 20`: `Z_1000 =
3236.4999 + 2191.7341 i`, `Z_2000 = 3304.0010 + 2130.3701 i`, `Z_ext` `+0.321`, `-0.514`; `n_y = 40`, `N = 4000`: `3313.3554 +
2122.7376 i` (`+0.299`, `-0.502` at that level). With the correctors converged (200 instead of 2) model AV gives `+0.320`
(`n_y = 20`) and `+0.327` (`n_y = 40`): the half wall of D1, growing slightly under refinement.

## D4. The channel between two Symmetry planes (c2, c3)

ADR 0446's plane channel (D6 there) with `Symmetry` on `zmin`, `zmax`, moved by `ChannelMotion` through `FlowWall`, the
impedance by `drive_mode` and `periodic_response`; the code of `sa_aerodamp.rs` is copied (that file is not edited). The only
change of the corrected runs is `PimpleSettings::component_diagonal = true`.

* **QS (fast)**: QF's set-up (`n_x = 30`, `n_y = 8`, `L_f = 20 h`, `Expansion(5.5)`, `Wo = 1`, `C = 3`, `N = 50, 100`), three runs
  per level: `Symmetry` + corrected (QS), `Symmetry` + default (QD, ADR 0446's QF), zero gradient + default (QZ). Predictions:
  QS equals QZ up to the effect of the changed `rAU = V/a_P` on the corrector splitting; `rAU` changes by `lambda/(a_P/V)` below
  `0.7 %` in the cells (`a_P/V >= 1/dt + 2 kap = 301 /s` at `N = 100`, `lambda = 2.09 /s`), and the splitting itself moves Z by
  about `1e-3` (ADR 0446: `3.3926` against `3.3961` with 10 outer iterations), so `|QS/QZ - 1|` of order `1e-5`. Against the
  formula: QF measured (ADR 0446) the extrapolated real and imaginary parts `+0.111`, `+0.046` with the `+13 %` / `-1.6 %` of D1
  inside; removing it gives about `-1.7e-2` real and `+6.3e-2` imaginary (the short channel's ends, as QF in ADR 0446).
* **QE (explanation, fast)**: `Delta Z = Z(QD) - Z(QZ)` at `N = 100` against `Delta Z_model = Z_dt,lambda - Z_dt = 0.3915 -
  0.0171 i` (D1, `n_y = 8`); postdiction from ADR 0446's numbers `0.3608` real (`-7.8 %`). The same with `n_outer = 10` (QE10;
  the fixed point of D1 is reached by the outer iterations): estimate within `3 %` (ADR 0446's `3.3961` with `Symmetry`; the
  zero-gradient run with 10 outer iterations was not made there).
* **WS (heavy record, `#[ignore]`, run once)**: ADR 0446's registered W5 set-up (`n_y = 40`, `n_x = 120`, `L_f = 100 h`,
  `Expansion(21)`, `Wo = 5`, `C = 6`, `N = 100, 200, 400`, `Z_ext = 2 Z_400 - Z_200`) with `Symmetry` and the corrected diagonal.
  Prediction: the extrapolated errors of W5c (zero-gradient planes, ADR 0446 addendum: `8.29e-3` real, `1.58e-3` imaginary)
  within about `1e-4`. About 200 s.

## D5. F-401's VS repeated (c4)

`si_fsi_piston.rs` is copied (only what QF1 needs: the bar, the column, `coupled_step` with Aitken, `run_case`) into a new file;
its settings function takes the flag. VS is ADR 0434's check: QF1's Aitken run (ratio 4, matching wall, 25 steps) at `nu =
1e-2` against `nu = 1e-4`, `max_n |q_n(1e-2) - q_n(1e-4)|/U <= 1e-6`. **V0** (default, the copy's faithfulness): ADR 0434's
`8.28e-5 U` reproduced. **V1**: the same with `component_diagonal = true`, VS's limit `1e-6`. Estimate: below `1e-6` if the
averaged diagonal is VS's only cause (D1's order estimate `4.5e-5 U` against the measured `8.28e-5 U` leaves room for another
part, e.g. ADR 0252's splitting at the moving end wall, which is not estimated; this check can miss). **V2** (record): the
corrected run at `nu = 1e-3` against `1e-4`.

## Registered checks

Test files `crates/rcfd-post/tests/symmetry_flux_f531.rs` (S, Q, W) and `crates/rcfd-post/tests/symmetry_vs_f531.rs` (V). Every
value is printed; a test fails at the end if any of its checks missed. Relative errors of a complex quantity are given per part
(`Re Z/Re Z_ref - 1`, `Im Z/Im Z_ref - 1`) unless "modulus" is said (`|Z - Z_ref|/|Z_ref|`). Release profile.

| Id | Clause | Quantity | Limit | Estimate |
|---|---|---|---|---|
| A0 | c0 | this file committed before any code | holds | holds |
| S1a | c1 | Stokes, corrected, `n_y = 20`, `N = 1000, 2000`: modulus error of `Z_N` against model IE | `<= 1e-6` each | `1e-10` |
| S1b | c1 | the same, `Z_ext` against D2, per part | `<= 3e-3` each | `+1.00e-3`, `-5.51e-4` |
| S1c | c1 | corrected, `n_y = 40`, `N = 4000, 8000`: `Z_ext` against D2, per part; and each level against model IE (modulus) | `<= 1e-3` each; `<= 1e-6` | `+2.49e-4`, `-1.39e-4`; `1e-10` |
| S1d | c1 | wall shear of S1c's `Z_ext`: amplitude `|Z|/(A rho sqrt(w nu))` against `|tanh(k H)|`, relative; phase `arg Z` against `arg(Z/A)` of D2 | `<= 1e-3`; `<= 1e-3 rad` | `2e-4`; `2e-4 rad` |
| S0a | c3 | Stokes, default, `n_y = 20`, `N = 1000, 2000`: modulus error of `Z_N` against model AV | `<= 1e-6` each | `1e-10` |
| S0b | c3 (NC1) | default, `n_y = 20`, `Z_ext` against D2: real error, minus imaginary error | `>= 0.1` each | `0.321`, `0.514` |
| S0c | c3 | default, `n_y = 40`, `N = 4000`: modulus error against model AV; real error against D2 (does not fall with the mesh) | `<= 1e-6`; `>= 0.25` | `1e-10`; `0.299` |
| Q1 | c2 | QS against QZ at `N = 50` and `100`, per part | `<= 1e-3` each | `1e-5` |
| Q2 | c2 | QS `Z_ext = 2 Z_100 - Z_50` against ADR 0446's formula, per part | `<= 0.2` each (QF's limit) | `-1.7e-2`, `+6.3e-2` |
| Q3 | c3 | QE: `Re Delta Z/Re Delta Z_model - 1` at `N = 100` | `<= 0.15` in magnitude | `-0.078` (postdiction) |
| Q4 | c3 | QE10 (`n_outer = 10`): the same | `<= 5e-2` in magnitude | `<= 3e-2` |
| Q5 | c2 | every QS, QD, QZ, WS level periodic | change `<= 1e-6` | `<= 1e-12` |
| W1 | c2 | WS (heavy record): `Z_ext` against the formula, per part | `<= 3e-2` each (W5's limit) | `8.3e-3`, `1.6e-3` |
| W2 | c2 | WS: its per-part errors minus W5c's (`8.29e-3`, `1.58e-3`, ADR 0446) | `<= 2e-3` each in magnitude | `1e-4` |
| V0 | c4 | VS default in the copy: `dev/8.28e-5 - 1` | `<= 1e-3` in magnitude (ADR 0434 printed three digits; the copy runs the same code) | `0` to the printed digits |
| V1 | c4 | VS with the corrected diagonal: `max_n abs(q_n(1e-2) - q_n(1e-4))/U` | `<= 1e-6` (VS's limit) | below `1e-6` if D1 is the whole cause (uncertain) |
| V2 | c4 | corrected, `nu = 1e-3` against `1e-4` | record | — |

### Negative controls

* NC1 = S0b: the default must miss the closed form by at least 0.1 (the test sees the defect).
* NC2: QZ with the corrected diagonal (zero-gradient planes: every component diagonal is the same) against QZ default at `N =
  100`: modulus difference `<= 1e-12` (the option changes nothing where the diagonals agree; estimate: round-off of `(a + a +
  a)/3`).

### Planned mutations (on a scratch copy, each run against the fast tests)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | `pimple.rs`, the corrected split of `step`: drop the `(a_P - a_k) u_k` term (`HbyA_k = H_k/a_P` with the flag) | S1a, Q1 |
| MU2 | `pimple.rs`, the corrected split: `diag_k[(k + 1) % 3]` in place of `diag_k[k]` | S1a |
| MU3 | `bc.rs`, `Symmetry`: the tangential coefficient `a = 1 - n_k^2` replaced by `0.99 (1 - n_k^2)` (a slip with friction) | S1a, Q1 |
| MU4 | the test's closed form: `tanh(k H)` replaced by `coth(k H)` (a no-slip top) | S1b, S1c |
| MU5 | the test's model AV: one corrector instead of two | S0a |

## Decision rule

* **done**: every check of the table met on its registered run (W1, W2 included), both negative controls hold, every mutation
  caught, nothing met only after an amendment.
* **partial**: a registered check missed (a `*_known_miss` record with the limit intact), a mutation not caught, or a check met
  only after an amendment made after its run; in particular V1 missing (VS has another cause besides D1) makes the item partial
  with the cause located as far as the runs allow.
* **todo**: S0a or Q3 contradicts D1 (the diagnosis is wrong) and no other cause is located.

## Limits (what this does not cover)

* Only planes normal to a coordinate axis; on an inclined plane the tangential components also get a lagged cross coupling
  (`b = -n_k sum n_j u_j`), not examined.
* Only the transient constant-density PISO kernel. The steady kernels, the variable-density kernel and VOF have their own
  switches (ADR 0010, ADR 0103/0114); `SlipWall` and `Directed` share the mechanism (component-specific diagonals) and are not
  run here.
* The corrected diagonal with the consistent correctors (ADR 0214 F0) is refused on moving meshes (ADR 0258 V2); not exercised.
* No change of the default: whether `corrected` becomes the default is the maintainer's open decision
  (`component-diagonal-default.md`, options 1 to 3); it would move the registered `packed_bed` hash and is not taken here.

## Amendment 1 (2026-11-14, written after the registered runs of S1, S0, Q and Q4 on commit `8c64b9be`, before any corrected run)

**What the registered Q4 run showed** (`cargo test -p rcfd-post --release --test symmetry_flux_f531 q4_ -- --nocapture`, log
`f531-scratch/run4_q4.log`): `QD10 = 3.03084 + 1.08003 i`, `QZ10 = 3.03293 + 1.08075 i`, `Delta Z = -0.00209 - 0.00073 i` against
the model's `0.39152 - 0.01714 i`: `Re Delta Z/Re Delta Z_model - 1 = -1.005` (**missed**, limit `5e-2`). The other registered
runs so far (S1, S0, Q1 to Q3, Q5, NC1, NC2) are reported in the addendum.

**A defect of the set-up.** The test built the ten-outer-iteration settings with `PimpleSettings::pimple(nu, 10)`, which also
sets `alpha_u = 0.8`, `alpha_p = 0.5` and `n_correctors = 1`. D1's fixed point is derived without under-relaxation and with the
correctors of `piso`, and Q4's estimate rests on ADR 0446's diagnosis, which set `n_outer = 10` on the `piso` settings
(`f421-scratch/diag_qf_test.rs`: `fl.settings.n_outer = outer`; two correctors, no relaxation). The registered run therefore
measured another configuration than the one registered. (The relaxed configuration shows no excess at all; this is recorded, not
explained here: a 0-D version of the fixed point with relaxation predicts a sink `(a_P - a_k)/alpha_u`, larger, not zero, so the
relaxed PIMPLE loop does something D1 does not describe. It is listed as a remaining question in the addendum.)

**Correction (Q4c, the limit `5e-2` unchanged).** `PimpleSettings::piso(nu)` with `n_outer = 10` (everything else as Q4 and QF:
two correctors, `alpha_u = alpha_p = 1`), QD10c and QZ10c at `N = 100`, `Re Delta Z/Re Delta Z_model - 1`. Estimate (ADR 0446's
QD with ten outer iterations, `3.3961`; the zero-gradient run with ten outer iterations not known): within `3e-2`. The registered
test `q4_default_excess_with_ten_outer_iterations` stays as an ignored record (attribute only); `q4c_default_excess_with_ten_piso_outer_iterations`
is the corrected run. Q4 counts as met only after this amendment, if met.

## Addendum 1: results (2026-11-14)

Runs, each once, release profile, `CARGO_BUILD_JOBS=4`, logs in `w49i-work/f531-scratch`: on commit `8c64b9be` S1
(`run1_s1.log`, 22.5 s), S0 (`run2_s0.log`, 9.4 s), Q (`run3_q.log`, 2.5 s), Q4 (`run4_q4.log`, 5.1 s); after amendment 1, on
`9db8d675`, Q4c (`run5_q4c.log`, 7.8 s); on `b85059cf` V (`run6_vs.log`, 1.0 s) and the heavy record WS (`run7_ws.log`, 197 s);
the unregistered diagnosis of Q4c (`diag1_length.log`, code `zz_f531_diag.rs`, model `channel_step_model.py`); the mutations
(`MU*.log`, `mutations_summary.log`).

### Checks

| Id | Limit | Measured | Met |
|---|---|---|---|
| A0 | registration first | `50be33e0` precedes every code commit | yes |
| S1a | `<= 1e-6` each | `1.34e-11`, `1.29e-11` against model IE | yes |
| S1b | `<= 3e-3` each | real `+1.002e-3`, imaginary `-5.51e-4` (`Z_ext = 2554.067 + 4256.634 i` against `2551.510 + 4258.981 i`) | yes |
| S1c | `<= 1e-3` each; `<= 1e-6` | `+2.485e-4`, `-1.386e-4`; `1.24e-11` at both levels | yes |
| S1d | `<= 1e-3`; `<= 1e-3 rad` | amplitude `1.11743` against `abs(tanh(k H)) = 1.11747` (`3.6e-5`); phase `1.71e-4 rad` | yes |
| S0a | `<= 1e-6` each | `7.7e-15`, `1.8e-14` against model AV | yes |
| S0b (NC1) | `>= 0.1` each | real `+0.3214`, imaginary `-0.5142` | holds |
| S0c | `<= 1e-6`; `>= 0.25` | `1.2e-14`; real `+0.2986` at `n_y = 40` (`+0.3214` extrapolated at 20): the default's error does not fall with the mesh | yes |
| Q1 | `<= 1e-3` each | `N = 50`: `3.1e-5`, `4.0e-4`; `N = 100`: `1.1e-5`, `1.1e-4` | yes |
| Q2 | `<= 0.2` each | real `-1.64e-2`, imaginary `-3.94e-2` | yes |
| Q3 | `<= 0.15` | `Delta Z = 0.36077 - 0.01364 i` against `0.39152 - 0.01714 i`: `-7.86e-2` | yes |
| Q4 as registered | `<= 5e-2` | `-1.005` (the relaxed `pimple` constructor; amendment 1) | **no** (record `q4_default_excess_with_ten_outer_iterations`) |
| Q4c (amendment 1) | `<= 5e-2` | `Delta Z = 0.36061 - 0.01492 i`: `-7.90e-2` | **no** (`q4c_default_excess_with_ten_piso_outer_iterations_known_miss`) |
| Q5 | `<= 1e-6` | every channel level `<= 3.0e-14` (QS, QD, QZ, NC2, Q4, Q4c), WS `<= 1.8e-11`; the Stokes runs `<= 3.0e-8` (corrected), `<= 1.3e-14` (default) | yes |
| NC2 | `<= 1e-12` | `1.3e-14` | holds |
| W1 | `<= 3e-2` each | real `+8.291e-3`, imaginary `-1.582e-3` (`Z_ext = 0.94449 + 5.69156 i`) | yes |
| W2 | `<= 2e-3` each | `1.3e-6`, `2.4e-6` | yes |
| V0 | `<= 1e-3` | `8.28164e-5 U`: `2.0e-4` | yes |
| V1 | `<= 1e-6` | **`1.0e-10 U`** (default `8.28e-5 U`) | yes |
| V2 | record | `1.6e-11 U` (`nu = 1e-3` against `1e-4`); corrected against default at `nu = 1e-4`: `8.4e-7 U` | recorded |

### What the numbers say

* **(c3) Explained.** The `Symmetry` coefficients give the tangential momentum zero flux through the plane; the extra dissipation
  is the averaged diagonal of the PISO correctors (D1). The default Stokes step is the two-corrector averaged model to `2e-14`
  (S0a, S0c), and the corrected step is implicit Euler to `1e-11` (S1a, S1c): the mechanism is located to round-off. In a
  transient run the effect can be large: next to a `Symmetry` plane one Stokes depth from the wall the impedance is `+32 %` (real)
  and `-51 %` (imaginary) off, and the error does not fall with the mesh (`+0.30` at 40 cells).
* **(c3) Removed (opt-in) and documented.** `PimpleSettings::component_diagonal = true` (case key `solver.component_diagonal =
  "corrected"`, existing since ADR 0069 F3 / ADR 0074 W1) removes it: QS equals the zero-gradient planes to `1e-5` (Q1), WS
  equals W5c to `2.4e-6` (W2) and meets W5's registered limits (W1), which the registered W5 run of ADR 0446 missed with `8.55e-2`.
  No library line was changed; the default stays averaged (the maintainer's open decision, `component-diagonal-default.md`,
  updated with these numbers).
* **(c1, c2) The closed forms.** Stokes' layer within `2.5e-4` (impedance) and `3.6e-5`, `1.7e-4 rad` (wall shear amplitude and
  phase) at 40 cells after the extrapolation in the step; the channel between two `Symmetry` planes within `1.6e-2`, `3.9e-2` (QF's
  short channel) and `8.3e-3`, `1.6e-3` (W5's set-up).
* **(c4) VS.** With the corrected diagonal F-401's VS deviation is `1.0e-10 U` against the limit `1e-6 U` (default `8.28e-5 U`):
  the averaged diagonal next to the four `Symmetry` side walls is the whole of VS's miss; ADR 0252's end-wall splitting is not
  visible at this size. D1's order estimate `4.5e-5 U` was within a factor 2.
* **Q4 and Q4c: D1's fixed point is not the step.** D1 claimed that the outer iterations reach the infinite-corrector fixed point
  `a_P u_k = H_k - V (grad p)_k`. They do not: with `alpha_u = 1` every outer iteration restarts from the predictor (solved with
  the component's own diagonal), so the converged outer loop is the **two-corrector** map with a converged pressure. Unregistered
  diagnosis (`diag1_length.log`): the shortfall of `Delta Z` is per length, not at the ends (per `20 h`: `0.36199` at 8 cells,
  `-7.5 %`; `0.33539` at 16 cells and `N = 200`, `-15.9 %`; the ends of the Symmetry and zero-gradient runs are equal, `0.0303`
  and `0.0315`), and an exact linear model of the two-corrector averaged step of the developed channel (flux fixed in each
  corrector; `channel_step_model.py`) gives `0.36351 - 0.01387 i` (8 cells) and `0.33584 - 0.00561 i` (16 cells): the measured
  per-length parts to `0.4 %` and `0.13 %`. So Q3's `-7.9 %` and Q4c's `-7.9 %` are the two-corrector map against D1's idealised
  fixed point, which over-estimates the sink by `7 %` at `nu dt/dy^2 = 1` and `16 %` at `2`. The ADR 0446 percentages (`+13 %`,
  `+7 %`) are reproduced by the step itself (QD; ADR 0446 W5). Q4c stays a miss (limit intact); the cause is a wrong estimate in
  D1, not a second mechanism in the solver.
* **Relaxed PIMPLE (the registered Q4).** With `PimpleSettings::pimple(nu, 10)` (`alpha_u = 0.8`, `alpha_p = 0.5`, one
  corrector) the excess is `Delta Z = -0.0021 - 0.0007 i`: essentially none. Not explained (a 0-D version of the relaxed fixed
  point predicts a larger sink, amendment 1); a remaining question.

### Mutations (scratch copy `f531-scratch/copy`, `mutate.py`; each run against S1, S0 and Q, then reverted)

| Id | Mutation | Caught by |
|---|---|---|
| MU1 | the corrected split without `(a_P - a_k) u_k` | S1a (`0.43`), S1b to S1d, Q1 (`0.11`): yes |
| MU2 | `diag_k[(k + 1) % 3]` | S1: a panic (index out of bounds: the inactive `z` component of the 2-D column has no diagonal); not caught numerically by Q1, since in the channel `a_y = a_x` and the swap changes nothing for `u_x`: yes, by the panic only |
| MU3 | `Symmetry` tangential `a` times `0.99` | S1a (`0.12`), S1b, S1c, S0a (`2.6e-3`), Q1 (`3.8e-3`): yes |
| MU4 | `coth` in the closed form | S1b (`0.25`), S1c, S1d (phase `0.49 rad`); also S0b, S0c: yes |
| MU5 | model AV with one corrector | S0a (`3.6e-2`), S0c: yes |

### Deviations

* Q4 was run with the relaxed `pimple` constructor (amendment 1); Q4c is the corrected run (missed).
* Arithmetic slips of the registration text, not affecting any limit: D2 says `abs(tanh(k H))` "differs from 1 by 0.3"; it is
  `1.117` (by 0.12). The Stokes periodicity estimate "of order `1e-10`" was `3e-8` with the corrected diagonal (`1e-14` with the
  default; the limit `1e-6` was met). Q2's imaginary estimate `+6.3e-2` had the wrong sign (measured `-3.9e-2`; ADR 0446 reported
  QF's imaginary error as a magnitude).
* The mutations were built with the worktree's target directory. The copy's mutated `rcfd-fv` replaced the shared artifact, and
  the first run of the files afterwards (`run8_files.log`) linked the reverted MU3 and failed with MU3's numbers. Touching the two
  unchanged worktree sources forced a rebuild, and the files then pass as they stand (`run8b_files.log`: 3 passed, 3 ignored; 1
  passed). Every registered run was made before the mutations, on clean builds.
* W2 compares magnitudes (`abs(e_WS) - e_W5c`): ADR 0446's addendum gives W5c's errors as magnitudes; the WS signs are `+`, `-`.
* The Q4c diagnosis used unregistered runs (the 16-cell level first at `N = 100` diverged, `nu dt/dy^2 = 4.0`, above ADR 0451's
  limit, and was repeated at `N = 200`).

### Not run

* The rcfd-fv test suite and `baseline_hashes`: no line of `rcfd-fv`, `rcfd-api` or any other library was changed (two new test
  files in `rcfd-post`, an ADR, a topic page).
* An inclined `Symmetry` plane, `SlipWall`, `Directed`, the steady and variable-density kernels (Limits).

### Decision by the registered rule

**Partial.** Every clause of the acceptance is met on its registered run (c1: S1a to S1d; c2: Q1, Q2, W1, W2; c3: S0a to S0c,
Q3, the removal by the opt-in option and the documentation; c4: V0, V1), but the registered check Q4 missed on a defective set-up
and its corrected run Q4c missed (a known miss with the limit intact, diagnosed as a wrong estimate of D1).

### Remaining questions

1. **The default of the transient kernel.** F-531 adds transient evidence to the open decision of `component-diagonal-default.md`
   (options 1 keep `averaged`, 2 make `corrected` the default and re-register `packed_bed`, 3 keep it and warn). Not taken here: it
   changes default results and a registered hash. Recommendation: option 2 for the transient `piso`/`pimple` kernel at least (an
   O(1) error next to `Symmetry` planes that grows under refinement, and F-401's VS and ADR 0446's W5 miss are instances of it);
   failing that, option 3.
2. **The relaxed PIMPLE loop shows no excess** (the registered Q4). Options: (a) a follow-up item that models the relaxed
   two-level loop exactly (as `channel_step_model.py` does for PISO) and explains it; (b) nothing, since `corrected` removes the
   question. Recommendation: (a), small; it says whether `alpha_u < 1` hides the defect in other PIMPLE results.
3. **D1's fixed-point estimate** is superseded by the two-corrector model; nothing to decide (recorded so that later estimates
   use the step's own map).

### For the lead: roadmap entries

Proposed status of F-531: **partial**. Note:

> Numerics. PARTIAL (ADR 0455, 2026-11-12; registered before the code, amendment 1 after a registered run). The Symmetry condition's own coefficients give the tangential momentum exactly zero flux through an axis-aligned plane; the extra dissipation found by ADR 0446 is the averaged component diagonal of the PISO correctors (HbyA_k = H_k/a_P; the plane loads only the normal component's diagonal), located to round-off: Stokes' second problem with a Symmetry plane one Stokes depth from the wall matches the exact two-corrector averaged step to 2e-14 and is +32 percent (real) and -51 percent (imaginary) off the closed form rho sqrt(i w nu) tanh(k H), not falling with the mesh. Removed by the existing opt-in PimpleSettings::component_diagonal (solver.component_diagonal = "corrected"): the Stokes step is implicit Euler to 1e-11 and the extrapolated impedance and wall shear are within 2.5e-4 of the closed form at 40 cells; ADR 0446's channel between two Symmetry planes equals the zero-gradient planes to 1e-5 (QF) and 2.4e-6 (W5 set-up, within 8.3e-3 and 1.6e-3 of the formula, where the registered W5 run of ADR 0446 missed with 8.55e-2); F-401's VS drops from 8.28e-5 U to 1.0e-10 U (limit 1e-6 U). Default unchanged (documented in component-diagonal-default.md). Partial: Q4 (the default's excess with ten outer iterations against the infinite-corrector sink 4 nu/(3 h^2)) missed on a defective set-up (the relaxed pimple constructor, amendment 1) and its corrected run by 7.9e-2 (limit 5e-2), diagnosed: the converged step is the two-corrector map, which an exact model reproduces to 0.4 percent. Five mutations caught (one only by a panic). No library line changed. Open: ADR 0455 Q1, Q2.

F-401 (note addition):

> ADR 0455 (F-531, 2026-11-12): the VS miss is the averaged component diagonal of the PISO correctors next to the four Symmetry side walls; with PimpleSettings::component_diagonal = true VS is 1.0e-10 U (limit 1e-6 U).

F-421 (note addition):

> ADR 0455 (F-531, 2026-11-12): the Symmetry-plane dissipation of amendment 3 is the averaged component diagonal of the PISO correctors; with the opt-in corrected diagonal the registered W5 set-up (Symmetry planes) gives 8.3e-3 and 1.6e-3 against the formula, equal to W5c to 2.4e-6.

Follow-up item F-NEW, "The averaged component diagonal in the relaxed PIMPLE loop", deps F-531, acceptance:

> Every limit not stated here is registered in the ADR before the run. The channel of ADR 0455 QF with Symmetry planes under PimpleSettings::pimple (alpha_u 0.8, alpha_p 0.5, one corrector, 10 outer iterations), whose excess over the zero-gradient planes was -0.0021 against the averaged two-corrector PISO step's 0.36, is explained by an exact linear model of the relaxed step that reproduces the measured impedance of both runs within registered limits; the dependence on alpha_u is stated.
