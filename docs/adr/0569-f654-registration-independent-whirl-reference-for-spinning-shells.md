# ADR 0569: F-654 registration: an independent whirl reference for spinning shells (the free thin ring against its closed form)

Status: **REGISTERED 2026-11-15, written before any code of F-654 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-654 was written. The files under "What was read" were read, one
entry-path probe was run on the unchanged library (disclosed below: it printed only `Ok` or the error of each call, no
frequency), and the closed-form numbers below were computed by a short script of the closed form alone (no rcfd call;
`f654-scratch/reference.py`, disclosed). Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-654 (dep F-647), the follow-up proposed by [ADR 0561](0561-f647-registration-shells-on-the-rotating-cyclic-paths-and-the-full-rotor.md)
(its remaining question 1: its whirl checks compared the sector with the full ring built from the same sector, which is a
consistency check and not an independent reference; its run 1 missed because the shell `to_global` symmetrised the skew
Coriolis matrix to zero, so that run's whirl comparison was vacuous). Number 0569 given by the lead. Branch `f654-work` from
e33e2047 (which contains F-647), worktree `w49i-work/f654`, scratch `w49i-work/f654-scratch`. This is a test-only item unless a
defect is found.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored `*_known_miss`
test with the limit intact and the measured number); each registered run made once; defaults bit-identical; no edit of an
existing test, of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe
code; clean-room. `docs/ROADMAP.md` and `docs/roadmap.json` are not edited (the entries go in "For the lead"). Every choice
below that the maintainer would normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the
recommendation).

## What F-654 asks

The acceptance, binding:

> Every limit not stated here is registered in the ADR before the run. The forward and backward whirl frequencies of a spinning thin shell of revolution (a flat annular plate clamped at its inner radius, or a free thin ring) from RotatingSector::whirl_modes with Shell4 and Shell3 against a closed form derived or cited in the ADR, at three speeds up to half the first frequency: within the registered discretisation error at two meshes, the observed order within 0.5 of the expected one, and the split of each nodal-diameter pair within 1e-2 relative; models without shells keep their bits.

Clauses: **a0** every limit registered here; **a1** a spinning thin shell of revolution (here: the free thin ring, D1), its
forward and backward whirl frequencies from `RotatingSector::whirl_modes` with Shell4 and with Shell3; **a2** against a closed
form derived here (D3); **a3** three speeds up to half the first frequency; **a4** within the registered discretisation error
at two meshes; **a5** the observed order within 0.5 of the expected one; **a6** the split of each nodal-diameter pair within
`1e-2` relative; **a7** models without shells keep their bits.

## What was read

`AGENT_RULES.md`; the brief; the F-654 and F-647 entries of `docs/roadmap.json`; ADR 0561 (all of it, its amendment 2 and
addendum); ADR 0411 (D1 the Coriolis matrix `C = int rho N^T [a x] N`, `G = 2 Omega C`; D2 the dense solver; D3 the whirl
number and direction; D5 the sector); ADR 0403 (D1 `K_eff = K + Omega^2 (K_sigma,1 - M_perp)`); ADR 0473 (D2 the shell geometric
stiffness of the membrane resultants on the three translations, D3 the spin-softening mass of the director); ADR 0432 (Shell4:
MITC4, consistent mass with rotary inertia) and ADR 0454 (Shell3: constant-strain membrane, MITC3 with the stabilisation
`t^2 / (t^2 + alpha h^2)`, `alpha = 0.1`, `h` the longest edge). Sources at e33e2047: `rotating.rs` (`RotatingOptions`,
`RotatingPart::new`: with `stress_stiffening` off the pre-stress is `Prestress::uniform(model, 0)` and no static solve is made;
`with_unit_prestress`; `operator`), `rotating/whirl.rs` (`RotatingSector::whirl_modes`, `whirl_at`: `G = 2 Omega C` on the doubled
pair, the inertia test, the dense solver, the whirl number `kappa` and the wave sense `tau` of a pair, `MAX_WHIRL_UNKNOWNS =
1500`), `geometric.rs` (`Prestress`), `geometric_struct.rs` (`prestress_from_displacement`, `membrane_resultant`), `shell.rs`
(`local_geometric_stiffness`: `int grad N_a . N grad N_b` on the three translations; `local_spin_softening_mass`;
`coriolis_factors`, `local_coriolis`, `element_coriolis`), `shell3.rs` (the same three), `tests/sa_rotating_cyclic_shells.rs`
(the helpers; copied where used, not edited).

**Entry-path probe (disclosed).** A temporary test file (`tests/zz_probe_f654.rs`, deleted after the run, never committed) built
the D2 sector at the coarse mesh about the z axis (Shell4 and Shell3, `nu = 0`, no support) and printed only `Ok` or the error of
each call on the unchanged library: `RotatingSector::new` with `stress_stiffening(false)` and `coriolis(true)`: `Ok`;
`whirl_modes(2, 30)`: `Ok` with modes; `geometric_struct::prestress_from_displacement` of a zero field: `Ok`;
`RotatingSector::with_unit_prestress` with both effects: `Ok`, `whirl_modes(3, 30)`: `Ok` with modes; `RotatingSector::new` with
the default options (stress stiffening on): `SingularStiffness { node: 0, component: 2, .. }` for both kinds (the static
centrifugal solve of a free sector at index 0 is singular: the axial translation and the rotation about the axis). That is why
set F below gives the pre-stress through `with_unit_prestress` (D4). No frequency was printed.

## Design

### D1. The choice: the free thin ring, as a cylindrical band of shells with `nu = 0`

The two shells the acceptance names test different blocks of the shell Coriolis matrix of ADR 0561 D2. For a flat disc spinning
about its normal, the transverse motion `w` is along the axis, `a x w = 0`, and only the rotary block `m2 W^T [a x] W` acts:
the thin-plate result (Lamb and Southwell, cited from memory) is that the transverse modes do not split in the rotating frame at
all, so the split of the acceptance would be a relative error of a quantity that is zero to order `(t/R)^2`. The in-plane
flexural modes of a ring split at first order in `Omega` (Bryan's effect, Bryan 1890, cited from memory; derived below from
first principles), entirely through the translation block `m0 [a x]`, the main term of the shell Coriolis matrix. The ring is
taken.

The ring is a **cylindrical band**: mid-surface radius `R`, axial width `L`, radial thickness `t`, so the shell normal is radial
and the in-plane bending of the ring is the circumferential bending of the shell. With **`nu = 0`** an axially uniform motion of
the band is an exact solution of shell theory with free edges (the axial moment `M_x = D (kappa_x + nu kappa_theta)` and the
axial force vanish for `kappa_x = 0`, `epsilon_x = 0`), so the band is exactly a ring of section `A = L t`, `I = L t^3 / 12` with
the thin-shell constitutive law; with `nu != 0` the free edges would bend anticlastically and the band would be neither a ring
nor a plate strip. The facet of a band contains the axis direction, so `a_3 = 0` in facet axes: the rotary block vanishes (ADR
0561 arithmetic 1), and `m1 = 0` for a homogeneous shell. This check therefore tests the translation block, the spin-softening
mass of the translations and (set F) the membrane geometric stiffness; the rotary block is left to the follow-up (Limits).

### D2. The geometry and meshes (fixed now)

Axis `n = (1, 2, 2) / 3` through `o = (0.3, -0.2, 0.5)` (tilted, as ADR 0556 D3, so that no global axis is special), in-plane
basis `e_a = (2, -1, 0) / sqrt 5`, `e_b = n x e_a`. `R = 0.1` m, `t = 5e-4` m (`t / R = 0.005`), `L = 5e-3` m, steel `E = 2e11`
Pa, `nu = 0`, `rho = 7850` kg/m^3, `BlockProperty::shell(t)`. `N = 32` sectors. Node `(i, a)` of a sector at `o + R (cos phi_i
e_a + sin phi_i e_b) + (a L / n_a) n`, `phi_i = 2 pi i / (N n_c)`, `i = 0 .. n_c`, `a = 0 .. n_a`; quads `(i, a), (i + 1, a), (i +
1, a + 1), (i, a + 1)` (Shell4), or each quad cut into two Shell3 along the diagonal from its first node (as ADR 0561). Node sets
`low` (`i = 0`) and `high` (`i = n_c`), `Constraint::cyclic_with("low", "high", n, o, 32, CyclicOptions::new().shells(true))`,
the material on the element set `all`. **No support**: the ring is free.

Two meshes, a uniform refinement by 2 in both directions (so that the longest edge of the Shell3 stabilisation halves too):
**coarse** `n_c = 4`, `n_a = 1` (128 facets around); **fine** `n_c = 8`, `n_a = 2` (256 around). The element angle `Delta = 2 pi
/ (N n_c)`. Facets are planar rectangles (no warp). Harmonic indices `k = 2` and `k = 3` (nodal diameters 2 and 3; with `N = 32`
their aliases `N - k` and above are far higher). Reduced unknowns of a pair (arithmetic 3): at most 288, well below 1500.

### D3. The closed form (first principles)

Rotating frame turning with `Omega` about `n`; polar angle `theta` about `n` in the sense of the rotation; `w` the radial
(outward) and `v` the tangential (sense of increasing `theta`) displacement of the centre line, uniform across the width. The
position `(R + w) e_r + v e_theta`; with `n x e_r = e_theta`, `n x e_theta = -e_r` the absolute velocity is `(w_t - Omega v) e_r
+ (v_t + Omega (R + w)) e_theta`, and the kinetic energy per unit length `T = (m/2) [(w_t - Omega v)^2 + (v_t + Omega (R +
w))^2]`, `m = rho A`. Its part quadratic in `(w, v)`: `(m/2)(w_t^2 + v_t^2) + m Omega (v_t w - w_t v) + (m Omega^2 / 2)(w^2 +
v^2)` (the linear part is the centrifugal load, carried by the pre-stress). The middle term is `Omega u_t . (n x u)` (ADR 0411 D1,
the Coriolis matrix), the last `Omega^2 / 2 |P u|^2` (the spin softening, `P = I - n n^T`). The strain energy per unit length
`(1/2)(EA eps^2 + EI kappa^2)` of the thin curved beam, `eps = (v' + w) / R`, `kappa = (v' - w'') / R^2` (`' = d/d theta`; the
rotation of the section is `(v - w') / R`, its derivative over `R` is the change of curvature). With a hoop tension `N_0`
(set F) the second-order strain of the centre line `(eps^2 + psi^2) / 2`, `psi = (w' - v) / R` the rotation of the tangent,
adds `(N_0 / 2)(eps^2 + psi^2)` (the shells' geometric stiffness of ADR 0473 D2, `u_{k,s} N_ss u_{k,s}` over the three
translations, is this form: `u_s,s` is `eps`, `u_n,s` is `psi`). The steady hoop tension of a free thin ring is `N_0 = m Omega^2
R^2` (radial equilibrium of an element, `N_0 / R = m Omega^2 R`).

Lagrange: `m (w_tt - 2 Omega v_t - c Omega^2 w) + dV/dw = 0`, `m (v_tt + 2 Omega w_t - c Omega^2 v) + dV/dv = 0` (`c = 1` with
spin softening). The travelling wave `w = W cos(n theta - omega t)`, `v = V sin(n theta - omega t)` (`omega > 0`: travelling in
the sense of the rotation) gives the real symmetric system

`[K - m (omega^2 + c Omega^2) I + 2 m Omega omega J] (W, V)^T = 0`, `J = [[0, 1], [1, 0]]`,

`K = (EA / R^2) [[1, n], [n, n^2]] + (EI n^2 / R^4) [[n^2, n], [n, 1]] + s m Omega^2 [[1 + n^2, 2 n], [2 n, 1 + n^2]]`

(`s = 1` with stress stiffening; the last matrix is `(W + nV)^2 + (nW + V)^2` of `eps^2 + psi^2`). Its determinant is a quartic in
`omega`; the two roots of the flexural branch are one positive `omega_f` (forward) and one negative `-omega_b` (backward), the
two others belong to the extensional branch (far higher, `EA R^2 / EI = 12 R^2 / t^2 = 4.8e5`). The reference frequencies are
`omega_f` and `omega_b`, the split `omega_b - omega_f`.

Inextensional limit (`EA -> infinity`, `V = -W / n`): `omega^2 + (4 n / (n^2 + 1)) Omega omega + c Omega^2 - omega_0^2 - s
Omega^2 (n^2 - 1)^2 / (n^2 + 1) = 0`, `omega_0^2 = EI n^2 (n^2 - 1)^2 / (m R^4 (n^2 + 1))`, so `omega = -2 n Omega / (n^2 + 1)
+/- sqrt(...)` and the rotating-frame split is `4 n Omega / (n^2 + 1)` exactly (Bryan's factor `2 / (n^2 + 1)` of the pattern's
precession, cited from memory, follows). With both effects the root is `sqrt(omega_0^2 + Omega^2 n^2 (n^2 - 1)^2 / (n^2 + 1)^2)`
(arithmetic 1). A check of the extensible form: with `EI = 0` and set F, `omega = -n Omega` is an exact root (the backward wave
stands still in the inertial frame: the lariat, a wave speed `sqrt(N_0 / m) = Omega R` equal to the material speed; arithmetic
2).

The whirl number of a root (ADR 0411 D3, `kappa = Im(phi^H C phi) / phi^H M phi`): with `u = Re(phi e^{i omega t})` the wave is
`phi_w = W e^{-i n theta}`, `phi_v = i V e^{-i n theta}`, and `kappa = -2 W V / (W^2 + V^2)` (arithmetic 4); inextensional:
`kappa = +/- 2 n / (n^2 + 1)` for the forward (`V = -W/n`) and the backward root, so the split is `2 kappa Omega` at first
order.

The reference is computed in the test: for each root, bisection of the determinant on `[0.9, 1.1]` times the inextensional root
(200 halvings; the extensible roots differ from the inextensional ones by `~ 1e-5`, arithmetic 5), then `(W, V)` the null vector
and `kappa`. The determinant is evaluated without the cancellation of the membrane terms: `K - mS I + 2 m Omega omega J = alpha u
u^T + B`, `alpha = EA / R^2`, `u = (1, n)`, and `det = det B + alpha (b_22 - 2 n b_12 + n^2 b_11)` (the naive product cancels
`alpha^2`-sized terms, `alpha / |B| ~ 1e4`).

### D4. The two effect sets

- **Set S** (spin softening and Coriolis, no stress stiffening): `RotatingOptions::new(2).coriolis(true).stress_stiffening(false)`,
  `RotatingSector::new` (no static solve, the probe). Reference `s = 0`, `c = 1`.
- **Set F** (both effects and Coriolis): `RotatingOptions::new(2).coriolis(true)`, `RotatingSector::with_unit_prestress(model,
  P, options)`, `P = geometric_struct::prestress_from_displacement(model, u_0)`, `u_0` the uniform radial expansion at 1 rad/s:
  at every node the translation `U_0 e_r` (`e_r` the unit of the part of `x - o` perpendicular to `n`), zero rotations, `U_0 =
  rho R^3 / E`. Each facet's chord stretches by exactly `U_0 / R` (the nodes move radially; the chord scales by `(R + U_0)/R`), its
  normal displacement is uniform and its rotations zero, so its membrane resultant is `N_ss = E t U_0 / R = rho t R^2`, `N_xx =
  N_sx = 0` (`nu = 0`): the hoop tension of D3 at 1 rad/s. Reference `s = 1`, `c = 1`. The library's own static solve cannot give
  it (singular, the probe).

Speeds: `omega_1` the reference `n = 2` frequency at rest (`Omega = 0`, extensible), `Omega_j = (j / 6) omega_1`, `j = 1, 2, 3`
(up to half the first frequency, a3). `K_eff` stays positive definite on `k = 2, 3` up to `Omega_3`: inextensional `omega_0^2 -
Omega^2 > 0` for set S, `omega_0^2 + Omega^2 ((n^2 - 1)^2 / (n^2 + 1) - 1) > 0` for set F; a loss of stiffness is a set-up
failure.

### D5. The measures

At each kind, mesh, set, speed `Omega_j` and `k in {2, 3}`: `whirl_modes(k, Omega_j)` (the dense solver, the default), its modes
ascending; `omega_lo`, `omega_hi` the first two (the pair), `omega_3` the third. Against `omega_f`, `omega_b`:

- `e_lo = omega_lo / omega_f - 1`, `e_hi = omega_hi / omega_b - 1`;
- split `d = (omega_hi - omega_lo) / (omega_b - omega_f) - 1`;
- the observed order of each frequency `p = log2(|e_coarse| / |e_fine|)` (the refinement ratio is exactly 2);
- the whirl numbers `kappa_lo / kappa_f - 1`, `kappa_hi / kappa_b - 1` (`kappa` of the library);
- the direction and wave sense of the library: `omega_lo` `Whirl::Forward` with `tau >= 0.99`, `omega_hi` `Whirl::Backward`
  with `tau <= -0.99` (`tau = +1` a wave travelling in the sense of the rotation, as `omega_f` of D3 is).

### D6. The discretisation error expected (the estimate behind WF)

On an axially uniform motion MITC4 reduces to a ring of straight two-node elements: linear `w`, `v`, `theta`, the transverse
shear tied at the mid-point (arithmetic 6). For a wave of number `k = n / R` and element length `h = R Delta` in the thin limit
the tie gives `theta = (2/h) i tan(kh/2) W`, the bending energy is `(2 sin(kh/2)/(kh))^2 (2 tan(kh/2)/(kh))^2 = 1 + (kh)^2/12`
times the exact one, the consistent mass of linear interpolation `(4 + 2 cos kh)/6 = 1 - (kh)^2/6` times, so `omega_h / omega = 1
+ (kh)^2 / 8 + O(h^4)`, `kh = n Delta`: expected order **2**, magnitude `(n Delta)^2 / 8` (coarse: `1.2e-3` for `n = 2`, `2.7e-3`
for `n = 3`; fine `3.0e-4`, `6.8e-4`). The chord geometry adds `O(Delta^2)` too. Shell3 has the same order (linear fields) with a
constant not derived here (the diagonal cut and the stabilisation `alpha h^2 / t^2`, which softens the shear by a term
`~ 0.2 alpha (n/R)^2 h^2`, `O(h^2)`, opposite in sign), hence its doubled limit constant. The model error (shell against thin
ring: transverse shear and rotary inertia, `~ (t/R)^2 n^2 / 12 x 1.7 = 1.4e-5` for `n = 2`, `3.2e-5` for `n = 3`, arithmetic 7)
is `<= 5 %` of the fine-mesh estimate, so it shifts the observed order by `<= 0.07`.

## Checks (limits fixed now)

| Id | Clause | Check | Limit | Estimate behind it |
|---|---|---|---|---|
| RF1 | a2 | the reference code, `EI = 0`, set F, `n = 2, 3`, the three speeds: `|omega_b / (n Omega) - 1|` | `<= 1e-12` | D3: `-n Omega` is an exact root (arithmetic 2); bisection to rounding |
| RF2 | a2 | the reference code with `EA` times `1e8`: `omega_f`, `omega_b` against the inextensional closed form, both sets, all speeds, `n = 2, 3` | `<= 1e-8` relative | the extensible correction scales with `EI / (EA R^2)`: `2e-6 x 1e-8` |
| WF4 | a1, a2, a4 | Shell4, both meshes, both sets, the three speeds, `k = 2, 3`: `|e_lo|`, `|e_hi|` | `<= 0.25 (k Delta)^2` (coarse `2.4e-3` / `5.4e-3`, fine `6.0e-4` / `1.4e-3` for `k = 2` / `3`) | D6: `(k Delta)^2 / 8`, twice it |
| WF3 | a1, a2, a4 | Shell3, as WF4 | `<= 0.5 (k Delta)^2` | D6: same order, constant not derived, four times the Shell4 estimate |
| OD4, OD3 | a5 | the observed order `p` of each of the 24 frequencies (two sets, three speeds, two `k`, two members) per kind | `|p - 2| <= 0.5` | D6: order 2, model error shifts `p` by `<= 0.07` |
| SP4, SP3 | a6 | the split of each pair, both meshes, both sets, the three speeds, `k = 2, 3`: `|d|` | `<= 1e-2` | acceptance limit; the split is the difference of two frequencies of the same mode shape family, its error is at most of the order of `e` (`<= 2.7e-3` coarse) |
| KA4, KA3 | a1 | the whirl numbers of the pair, the same 24 cases: `|kappa / kappa_ref - 1|` | `<= 1e-2` | the mode-shape ratio `V/W` and the mass of the axial and rotation components (`O((t/R)^2)`); the split is `2 kappa Omega` at first order |
| DI4, DI3 | a1 | the same cases: `omega_lo` forward with `tau >= 0.99`, `omega_hi` backward with `tau <= -0.99`; and `omega_3 >= 1.5 omega_b` (the pair is the lowest two) | as stated | D3 signs; the next modes at `k = 2, 3` are the out-of-plane ring modes, `>= 3 omega` by the ring formula of a narrow band (cited from memory, an estimate only) |
| NC1 | a6 | set S without Coriolis (`coriolis(false)`), coarse, `Omega_3`, `k = 2, 3`, each kind: `omega_hi / omega_lo - 1` | `<= 1e-10` | without `G` the pair is the double eigenvalue of a cyclic pair; the split of SP is the Coriolis term's |
| B1 | a7 | every existing rcfd-fem test passes unchanged (release) | pass | no library line changes (test-only item) |
| B2 | a7 | `cargo test -p rcfd-api --release --test baseline_hashes`; `cargo test -p rcfd-cli --release --test docs_site` | pass | as B1 |

Models without shells keep their bits by construction (no library change); B1 and B2 confirm it. If a defect is found and the
library changes, the change is opt-in or confined to the shell routines and B1/B2 are run on it.

## Negative controls

NC1 above (the split vanishes without the Coriolis matrix). RF1 and RF2 are the controls of the reference code.

## Mutations (on a scratch copy, own `CARGO_TARGET_DIR`)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | the translation block `m0 [a x]` of the shell Coriolis matrix zero (`coriolis_factors` of `shell.rs`, used by both kinds) | SP, KA, DI |
| MU2 | the translation block with the opposite sign | DI (the wave sense), KA |
| MU3 | the sector whirl with `G = Omega C` in place of `2 Omega C` (`whirl_at`) | SP, WF, KA |
| MU4 | the spin-softening mass of Shell4 zero (`local_spin_softening_mass` of `shell.rs`) | WF4 |
| MU5 | the shell geometric stiffness of Shell4 without the local normal component (`c = 2` dropped in `local_geometric_stiffness`) | WF4 (set F) |
| MU6 | the Shell3 Coriolis matrix halved (`local_coriolis` of `shell3.rs`) | SP3, KA3 |

No mutation is declared equivalent.

## Decision rule

**done**: every check RF1 to B2 met, every mutation caught. **partial**: any registered check missed (recorded as a
`*_known_miss`), or any mutation uncaught, or a check met only after a post-run amendment. **todo**: WF and SP not run.

## Arithmetic (disclosed)

1. Inextensional root with `s = c = 1`: under the square root `4 n^2 / (n^2 + 1)^2 + ((n^2 - 1)^2 - (n^2 + 1)) / (n^2 + 1)`;
   with `a = n^2`: `((a - 1)^2 - a - 1)(a + 1) + 4a = (a^2 - 3a)(a + 1) + 4a = a (a - 1)^2`, so `Omega^2 n^2 (n^2 - 1)^2 / (n^2 +
   1)^2`.
2. Lariat: `EI = 0`, `s = c = 1`, `omega = -n Omega`: `m (omega^2 + Omega^2) = m Omega^2 (n^2 + 1)`, so `K_11 - mS = EA / R^2`,
   `K_22 - mS = EA n^2 / R^2`, `K_12 + 2 m Omega omega = EA n / R^2 + 2 n m Omega^2 - 2 n m Omega^2`; determinant `EA^2 n^2 / R^4 -
   EA^2 n^2 / R^4 = 0`.
3. Unknowns: fine mesh `n_c = 8`, `n_a = 2`: `8 x 3` independent nodes, 144 unknowns, doubled 288 for a pair.
4. `kappa`: `n x (phi_w e_r + phi_v e_theta) = phi_w e_theta - phi_v e_r`; `phi^H C phi = m int (conj(phi_v) phi_w -
   conj(phi_w) phi_v) = m (2 pi R) (-2 i W V)`, `phi^H M phi = m (2 pi R)(W^2 + V^2)`.
5. The closed-form script (`f654-scratch/reference.py`, the D3 determinant by bisection, no rcfd): `omega_1 = 195.490042` rad/s
   (inextensional `195.490564`), speeds `32.5817`, `65.1633`, `97.7450` rad/s. Reference `(omega_f, omega_b)`, rad/s:

   | Set | `Omega_j` | `n = 2` | `n = 3` |
   |---|---|---|---|
   | S | 1 | 168.4447, 220.5756 | 532.7648, 571.8637 |
   | S | 2 | 139.4094, 243.6713 | 511.3670, 589.5649 |
   | S | 3 | 108.2893, 264.6821 | 488.7232, 606.0200 |
   | F | 1 | 173.2960, 225.4270 | 538.8813, 577.9803 |
   | F | 2 | 158.4180, 262.6801 | 535.5212, 613.7193 |
   | F | 3 | 149.7816, 306.1748 | 541.9851, 659.2824 |

   The splits differ from `4 n Omega / (n^2 + 1)` by `<= 3e-5` relative (the extension).
6. D6: `(2 sin s / 2s)^2 ~ 1 - (kh)^2/12`, `(tan s / s)^2 ~ 1 + (kh)^2/6` (`s = kh/2`), product `1 + (kh)^2/12`; mass `1 -
   (kh)^2/6`; ratio of `omega^2` `1 + (kh)^2/4`. `(k Delta)^2`: coarse `9.64e-3` (`k = 2`), `2.17e-2` (`k = 3`); fine `2.41e-3`,
   `5.42e-3`.
7. Model error: `(t/R)^2 / 12 = 2.08e-6`; shear `E / (kappa_s G) = 2.4` (`nu = 0`, `kappa_s = 5/6`) and rotary inertia `1`, half
   of the sum on `omega`: `2.08e-6 x n^2 x 1.7`.

## Limits (what is not covered)

- The rotary block `m2 W^T [a x] W` of the shell Coriolis matrix and the layup coupling `m1`: zero on a band (D1). They keep the
  uniform-field checks of ADR 0561 CO1. Follow-up: the flat annular disc clamped at its inner radius (transverse modes, a
  semi-analytical reference of the plate with the Lamb-Southwell membrane stress).
- The library's own centrifugal pre-stress: on a free ring the static solve is singular (the probe), so set F gives the exact
  hoop pre-stress through `with_unit_prestress`; the static path was checked by ADR 0561 against the full ring.
- `nu != 0` (anticlastic edges), the stationary frame, the sparse whirl solvers, the split modes and the whirl state with shells,
  the Shell4 ANDES membrane, warped facets.

## For the lead: roadmap entries

To be completed in the addendum.
