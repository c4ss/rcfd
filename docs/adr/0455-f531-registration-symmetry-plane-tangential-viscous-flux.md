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
