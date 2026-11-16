# ADR 0618: F-689 registration: the interaction integral on closed curved fronts and at mirror-plane stations

Status: **REGISTERED 2026-11-15, written before any code of F-689 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-689 was written, compiled or run: the files under "What was read" were
read, and the arithmetic below was done by hand and with one plain Python script (numpy only; the rotated closed forms, the
perimeter of the quadratic edges of an ellipse, the hat bias; no rcfd code, no finite-element run; disclosed, script
`w49i-work/f689-scratch/py/pre.py`). Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-689 (dep F-682), the follow-up proposed by
[ADR 0610](0610-f682-registration-interaction-integral-on-curved-crack-fronts.md) (its remaining question 2). Number 0618 given
by the lead. Branch `f689-work` from the local main tip b61032f4 (which holds F-682, ADR 0610, and F-632, ADR 0605), worktree
`w49i-work/f689`, scratch `w49i-work/f689-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored `*_known_miss`
test with the limit intact and the measured number); each registered run made once; existing tests not edited; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no
unsafe code; clean-room. The roadmap files are not edited (the entries go in "For the lead"). Every choice below that the
maintainer would normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the
recommendation).

## What F-689 asks

The acceptance, binding:

> Every limit not stated here is registered in the ADR before the run. A closed (periodic) front without end stations and a mirror-plane option that zeroes the antisymmetric modes at stations on a mirror plane; a whole penny-shaped crack under tension inclined in a general direction with K_I, K_II and K_III at every station within 3 percent of the closed forms, and the results of ADR 0610 unchanged.

Clauses: **c0** every limit registered here; **c1** a closed (periodic) front without end stations; **c2** a mirror-plane option
that zeroes the antisymmetric modes at stations on a mirror plane; **c3** a whole penny-shaped crack under tension inclined in a
general direction, `K_I`, `K_II` and `K_III` at every station within 3 % of the closed forms; **c4** the results of ADR 0610
unchanged. From the brief: **c5** the closed forms of ADR 0610 reused, the general direction by rotating the shear direction in
the crack plane, verified; **c6** heavy runs `#[ignore]`d records run once, free RAM checked first; **c7** ADR 0610's results
bit-identical (its checks re-run); **c8** (house rule 3) opt-in, existing bits unchanged.

## What was read (no run)

`AGENT_RULES.md`; the brief; ADR 0610 in full (D1 to D7, the checks, addendum 1: in particular D1 "the crack faces and the end
faces", D4 the half `x >= 0` model and its supports, the end-station `K_III` record `-1.63 A_III` (PM) and `-0.83 A_III` (PC),
and remaining question 2); ADR 0605 D1 and D5 (the elliptical front, the end-face term of the front mean); `docs/reference-data.md`
sections F-632 and F-682; `crates/rcfd-fem/src/fracture.rs` at b61032f4 (all of it: `CrackFront::check`, `FrontGeometry::{hat,
hat_integral, edge_hat_integral, param, tangent}`, `check_boundary`, `j_integral`, `interaction_integral`, `opening_k`,
`interaction_integral_curved`, `penny_crack_half_cylinder`); `crates/rcfd-fem/tests/sa_fracture_curved.rs` (all);
`hex20.rs` (the node numbering `REF`, the faces `FACES`, `traction_loads`, `traction_field_loads`); `mesh.rs` (`HEX_EDGES`,
`face_incidence`); `solve.rs` (`StaticSolver`). `rcfd-api`, `rcfd-cli` and `rcfd-mcp` do not call `rcfd_fem::fracture` (ADR 0610,
unchanged at b61032f4), so no case result and no golden can change. The run logs of ADR 0610 (`f682-scratch/run1.log`,
`heavy_pm.log`: PC 36 351 unknowns solved in 8.8 s, PM 122 145 unknowns in 44.2 s) were read for the time estimates.

No paper is used beyond those ADR 0610 cites from memory (nothing here depends on them).

## Design

### D1. The closed front (c1)

`CrackFront::closed(self) -> CrackFront` marks a circular or elliptical front as closed: the nodes go once around the curve,
corner, mid-edge, corner, ..., mid-edge, the last node the mid-edge node between the last corner and the first (an **even**
number `>= 6` of nodes, `n/2 >= 3` stations, every corner a station, no end station). `CrackFront::check` for a closed front:

* refuses a straight front (`"crack front"`, "closed"), an odd number of nodes or fewer than 6 ("closed");
* computes the node parameters as for an open front (circle: `R atan2` from the first node; ellipse: the elliptic angle `nu`),
  then **unwraps** them with the period `P` (`2 pi R` for the circle, `2 pi` for the ellipse): `s_k = s_(k-1) + w(s_raw,k -
  s_raw,(k-1))`, `w(d) = d - P round(d/P)`; the order check of the open front then applies unchanged to the unwrapped values;
* refuses a span `s_last - s_0 >= P` (more than once around) and any station gap, the closing one `s_0 + P - s_(n-2)` included,
  `>= P/2` (the wrapped distance of the hat would be ambiguous).

`FrontGeometry` gains `period: Option<f64>` (`None` for an open front). With a period:

* **the hat** of station `j` (stations `t_0 < ... < t_(m-1)`, `m = n/2`): neighbours `t_l = t_(j-1)` (`t_(m-1) - P` for `j = 0`)
  and `t_r = t_(j+1)` (`t_0 + P` for `j = m - 1`); `d = w(s - t_j)`; `phi = (d - (t_l - t_j))/(t_j - t_l)` for `t_l - t_j < d <=
  0`, `(t_r - t_j - d)/(t_r - t_j)` for `0 < d < t_r - t_j`, else 0. Every hat is a full hat; `sum_j phi_j = 1` everywhere;
* **the weight** `int phi_j ds`: circle `(t_r - t_l)/2`; ellipse the edge integral of ADR 0605 D1 over all `m` quadratic edges,
  the closing edge (nodes `n - 2`, `n - 1`, `0`) included.

For an open front nothing changes: the new branches are taken only when `period` is `Some` (c8). `j_integral` and
`interaction_integral_curved` take closed fronts through these two functions; `interaction_integral` (ADR 0605) refuses them as
before (not straight); `opening_k` does not use the hats. A closed front may be half-symmetric (a whole penny in the half `z >= 0`
model): `K_I` is doubled as before. `check_boundary` is not changed (a whole model has no end face; the end-face exemption is
not reached by the whole cylinder below; limits).

### D2. The mirror-plane option (c2), derived

`CrackFront::mirror_plane(self, point, normal) -> CrackFront` declares a plane of mirror symmetry of the model (geometry and
loads) that cuts the front; several may be declared. **The stations on it**: those whose corner node is within `FRONT_TOLERANCE
L` of the plane (`L` the length scale of the front check: the chord, the radius or `a`).

**Derivation.** Let `S` be the reflection in the plane (unit normal `m`), the model symmetric: `u(Sx) = S u(x)`. A station on the
plane with the front crossing it: the front is mapped onto itself, so at the station the tangent `T` is `+/- m` (`S T = -T`) and
the advance `e` and the crack normal `n` lie in the plane (`S e = e`, `S n = n`); `b3 = e x n = +/- m`. The tearing
displacement `u . b3 = +/- u . m` is odd under `S`, so it and its jump vanish on the plane: `K_III = 0` at the station; the
opening and sliding displacements (`u . n`, `u . e`) are even. The station value of a hat that the plane halves is defined as the
value of the whole model's (mirror-doubled) symmetric hat: for modes I and II the integrand of the doubled domain is even under
`S` (ADR 0610 D1: the end-face term vanishes for them), so the half model's `M / int phi ds` equals the doubled one (both
numerator and weight double); for mode III the integrand is odd and the doubled integral is zero (exactly, on a mirror-symmetric
mesh, up to rounding). So the option sets `K_III = 0` at the stations on a mirror plane and leaves `K_I`, `K_II` and every other
station unchanged. (A plane through a station that maps the front onto itself has its normal either along the crack normal, the
half-symmetric model of ADR 0547, which already zeroes `K_II` and `K_III`, or along the tangent, this case.)

**The library.** In `interaction_integral_curved`, after the front check: for each declared plane (unit normal; refused if it has
no direction) the stations on it; refused (`"crack front"`, "mirror plane") a plane that holds no station, a plane at one of
whose stations `|T . m| < 1 - PARALLEL_TOLERANCE` (not normal to the front), and any mirror plane on a closed front (a closed
front is a whole model). At the stations found `k_iii = 0.0`. `j_integral` accepts and ignores the planes (J has no end-face term
on a mirror plane: there `q . m = 0`, `sigma_mb = 0` for `b` in the plane and `u_m,b = 0`, so `P_mk q_k = sigma_im u_i,k q_k - W
q_m = sigma_mm u_m,k q_k = 0`). `interaction_integral` (ADR 0605) refuses a front with mirror planes ("mirror plane":
use `interaction_integral_curved`): a new refusal, reached only by a front that asks for the new option (c8).

### D3. The whole cracked cylinder (c3)

`penny_crack_whole_cylinder(&PennyCrack) -> Result<CrackedWholeCylinder>`: the Hex20 mesh of `penny_crack_half_cylinder` (the
same parameters, called, not copied) reflected in `x = 0`. Nodes with `x == 0.0` exactly (the plane nodes of the half: the
angle `pi/2` columns, the core column `i = 0` and their mid-edge nodes, upper and lower crack faces alike) are shared, every
other node gets its mirror `(-x, y, z)`. A reflected element keeps its orientation with the node permutation
`[0, 3, 2, 1, 4, 7, 6, 5, 11, 10, 9, 8, 15, 14, 13, 12, 16, 19, 18, 17]` (corners `0 3 2 1` and `4 7 6 5`; the mid-edge
nodes of `hex20::REF` follow their edges: `(0,3) = 11`, `(3,2) = 10`, `(2,1) = 9`, `(1,0) = 8`, top alike, verticals `16, 19, 18,
17`). Sets: element `all`; nodes `all`, `top_centre` `(0, 0, H)`, `bottom_centre` `(0, 0, -H)`, `top_rim` `(0, R, H)`; surfaces
`top`, `bottom`, `side` by a scan of the boundary faces (all 8 nodes at `z >= H (1 - 1e-12)`, `z <= -H (1 - 1e-12)`, `rho >= R
(1 - 1e-12)`). Front: the half's `f_0 .. f_(4 n_theta)` (from `theta = -pi/2` to `pi/2`), then the mirrors of `f_(4 n_theta - 1)
.. f_1`: `8 n_theta` nodes, `4 n_theta` stations, `crack_front()` circular about the origin, normal `z`, closed.

The nodes of `x >= 0` are bit-for-bit those of the half (the half is built by the same call), and the mirrored ones exact
negations, so the whole mesh is exactly mirror-symmetric in `x` and (as the half) in `z`.

**Supports of the whole (free) body:** `u = 0` at `top_centre`; `u_x = u_y = 0` at `bottom_centre`; `u_x = 0` at `top_rim`
(three translations, the rotations about `x` and `y`, the rotation about `z`). The loads are self-equilibrated. These supports
are all satisfied by the symmetric solution of ADR 0610's half model (`u_x = 0` on `x = 0`; its `top_centre` `u_y = u_z = 0`,
`bottom_centre` `u_y = 0`), so under ADR 0610's load the whole model has the half's solution up to rounding (used by MW3).

### D4. The closed forms in a general direction (c3, c5)

Remote uniaxial tension `sigma` along `d = (sin beta cos psi, sin beta sin psi, cos beta)`. On the crack plane: `sigma_nn = sigma
cos^2 beta`, the shear `tau = sigma sin beta cos beta` along `t = (cos psi, sin psi, 0)`. The problem is isotropic in the plane,
so ADR 0610 D2's solution for a shear along `y` rotated by `psi - pi/2` about `z` is the solution: the slip `b0 sqrt(1 - rho^2/a^2)
t`, `b0` of ADR 0610 D2, and near the front its components along `e` (mode II) and `b3 = e x n` (mode III):

`K_I = A_I = 2 sigma_nn sqrt(a/pi)`, `K_II = A_II (e . t) = A_II cos(theta - psi)`, `K_III = A_III (b3 . t) = A_III sin(theta - psi)`,

`A_II = 4 tau sqrt(a/pi)/(2 - nu)`, `A_III = 4 (1 - nu) tau sqrt(a/pi)/(2 - nu)` (ADR 0610 D2), `e = (cos theta, sin theta, 0)`,
`b3 = (sin theta, -cos theta, 0)`. **Verification** (arithmetic 2): (i) `psi = pi/2` gives ADR 0610's `A_II sin theta`, `-A_III
cos theta` exactly; (ii) the projection of the rotated slip jump onto `e` and `b3`, divided by the jumps of the unit Williams
fields, equals the formulas to `4.2e-16` at 37 angles; (iii) the front mean of `G = (K_I^2 + K_II^2)/E' + K_III^2/(2 mu)` is
independent of `psi` (to `4e-16`), as the energy of a rotated load must be. The energy check of ADR 0610 (vi) then carries over.
The reference of a station is the (periodic) hat average of the closed form (ADR 0610 D2), 16 Gauss points on 64 panels on each
side, the neighbours wrapped by `2 pi`.

`beta = 45 deg` (ADR 0610's), `psi = 35 deg = 7 pi/36` (general: not a multiple of the station spacing `pi/8` or `pi/16` and not a
mirror direction of the mesh). `A_I = 0.564190`, `A_II = 0.663752`, `A_III = 0.464627` (times `sigma sqrt(a)`).

### D5. The whole ellipse (c1 on an elliptical closed front)

The whole cylinder of ADR 0605's coarse ellipse spec (`PennyCrack::new(a, 12 a, 10 a, a/8, 2, 4, 0.4 a, 1, 0.65 a, 1.5, 1.6)`)
mapped in the test by the map of `elliptical_crack_block` (ADR 0605 D3; `rho >= r_t` confocal, the core affine), `b = a/2`,
mode I tension `sigma` along `z` (`sigma z` on `top`, `-sigma z` on `bottom`), the supports of D3. Front
`CrackFront::elliptical(.., a, b).closed()`, 16 stations at equal steps of the elliptic angle. Reference: the periodic linear hat
average of Irwin's form with `ds = sqrt(a^2 sin^2 phi + b^2 cos^2 phi) dphi` (ADR 0610 D3).

### D6. ADR 0610 unchanged (c4, c7)

`tests/sa_fracture_curved.rs`, `sa_fracture_modes.rs`, `sa_fracture.rs` are not edited and are run (fast tests and the heavy
records; EC1 its known miss, expected with the same number). In addition **BI**: a scratch dump test (not committed;
`f689-scratch/bi/dump.rs`) prints with `{:?}` (the shortest round-trip form of every `f64`) every `ModeFactors` and `FrontValue`
of ADR 0610's set-ups (PC: Corrected, DomainOnly and `j_integral` ring 2; PQ and EC: the same; MS: `interaction_integral` and
both curved terms, rings 2 to 4; PM: rings 2 to 4 Corrected, ring 3 DomainOnly, `j_integral` ring 3; EH: rings 2 to 4, ring 3
DomainOnly and `j_integral`), run once in a `git archive` copy of b61032f4 and once in the branch tip; the two outputs must be
byte-identical.

### D7. Opt-in (c8)

New items in `fracture.rs`: `CrackFront::{closed, mirror_plane, is_closed, mirror_planes}`, `CrackedWholeCylinder`,
`penny_crack_whole_cylinder`. Changed existing lines, each only adding a branch taken by the new options or a field initialised
to its default: the `CrackFront` struct and its three constructors (`closed: false`, no mirror planes), `CrackFront::check` (the
node-count test and the unwrapping for closed fronts), `FrontGeometry` (`period`), `hat` and `hat_integral` (an early branch on
`period`), `interaction_integral` (the mirror refusal), `interaction_integral_curved` (the mirror stations). No other library
file changes; no case-file key. Tests in a new file `crates/rcfd-fem/tests/sa_fracture_closed.rs`.

### D8. Limits (not covered)

* As ADR 0610 D7: plane cracks, no non-planar front, no crack-face traction, no body force, no thermal strain, one isotropic
  material; fronts given as a circle or an ellipse (no spline front).
* A closed front must be circular or elliptical. `check_boundary` keeps its end-face exemption for closed fronts (a whole model
  has no end face; a boundary face of a ring normal to the local tangent would be wrongly accepted; not reachable by the meshes
  here).
* The mirror option zeroes `K_III` only (the one antisymmetric mode of a mirror plane normal to the front); it trusts the user
  that the model is symmetric (geometry and loads); it is not offered on closed fronts.
* Mixed mode on the whole ellipse not checked (no verified closed form, as ADR 0610 D7); mode I only.

## Arithmetic done beforehand (disclosed)

`f689-scratch/py/pre.py` (numpy):

1. **The perimeter of the quadratic edges** of the ellipse `b = a/2` through `(a cos t, b sin t)` at `t = 2 pi k/16` and the
   mid-angles, 8-point Gauss-Legendre on each edge (the rule of ADR 0605 D1): `0.048441055` against `4 a E(k) = 0.048442241`,
   relative `-2.45e-5` (32 edges: `-1.5e-6`).
2. **The rotated closed forms** (D4): (i) to (iii) as stated there; `A_I/(sigma sqrt a) = 0.5641896`, `A_II = 0.6637525`, `A_III =
   0.4646267`.
3. **Hat bias** (ADR 0610 arithmetic 4): station spacing `pi/8` (coarse whole, 16 stations) `1.28e-2`, `pi/16` (medium, 32
   stations) `3.2e-3`: the references are hat averages.
4. **Sizes** (by hand from ADR 0610's logs): the whole cylinder doubles the half less the plane `x = 0`: coarse about 5 120 Hex20
   and 70 000 unknowns (PC 36 351), medium about 18 300 Hex20 and 240 000 unknowns (PM 122 145). Time: the coarse whole about 20 s
   to 40 s with the supernodal factor (PC 8.8 s), the medium 2 to 4 minutes and an estimated 3 to 6 GB of factor (a heavy record,
   run alone after checking that at least 16 GB of RAM are free).

## Checks

Common: `E = 2e11`, `nu = 0.3`, `a = 0.01`, `sigma = 1e8`, `beta = 45 deg`, `psi = 35 deg`. Errors of `K_I` relative; of `K_II`
and `K_III` as `|K_j - K_ref,j| / A_mode` (the closed forms have zeros along the front) and, in WC4 and WM4, also relative where
`|K_ref,j| >= A_mode/2`. Every aggregated value asserted finite (F1). Coarse whole: `PennyCrack::new(a, 10 a, 10 a, a/8, 2, 4,
0.25 a, 1, 0.5 a, 1.5, 1.6)` (ADR 0610 PC's spec), 16 stations, supernodal. Medium whole: ADR 0610 PM's spec (`a/16`, 3, 8, ...,
1.4, 1.5), 32 stations.

| check | clause | set-up | limit | estimate |
|---|---|---|---|---|
| WC1 | c1, c3 | coarse whole, general direction, ring 2: `K_I` at all 16 stations against `A_I`, the largest relative error | `<= 0.03` | 0.002 |
| WC2 | c1, c3 | the same: `K_II` at all stations, `|K - K_ref,j|/A_II` | `<= 0.03` | 0.003 |
| WC3 | c1, c3 | the same: `K_III` at all stations, `|K - K_ref,j|/A_III` | `<= 0.03` | 0.006 |
| WC4 | c3 | the same: `K_II`, `K_III` relative to `K_ref,j` where `|K_ref,j| >= A/2` | `<= 0.03` | 0.012 |
| WC5 | c1 | the same: `j_integral` ring 2 (closed) at every station against the hat average of `G` | `<= 0.10` | 0.04 |
| WCR | record | `DomainOnly` errors; counts and solve time | none | |
| WM1 | c3 | medium whole (heavy, `#[ignore]`), ring 3: `K_I`, all 32 stations, relative | `<= 0.03` | 0.001 |
| WM2 | c3 | the same: `K_II`, `/A_II` | `<= 0.03` | 0.001 |
| WM3 | c3 | the same: `K_III`, `/A_III`, all stations | `<= 0.03` | 0.003 |
| WM4 | c3 | the same: relative where `|K_ref,j| >= A/2` | `<= 0.03` | 0.004 |
| WM5 | c1 | per station and mode `(max - min)/A_mode` over rings 2, 3, 4 | `<= 0.01` | 0.002 |
| WM6 | c1, c5 | `j_integral` ring 3 against the hat average of `G`, every station | `<= 0.04` | 0.02 |
| WM7 | c5 | the front mean of J against that of `G` | `<= 0.02` | 0.015 |
| WMR | record | `DomainOnly` ring 3 errors; counts, time | none | |
| MW1 | c2 | ADR 0610's coarse half (PC) with `mirror_plane((0,0,0), x)`: `K_III` at the two end stations | `== 0` | 0 |
| MW2 | c2, c8 | the same: every other value (all `K_I`, `K_II`, interior `K_III`, `s`, weights) against the run without the option | bit-identical | 0 |
| MW3 | c2 | the coarse whole under ADR 0610's load (`psi = 90 deg`), ring 2, against the half with the option, its 9 stations (the same indices), `max |Delta K|/A_mode` over stations and modes | `<= 1e-6` | 1e-9 |
| MW4 | c2 | the same whole: `|K_III|/A_III` at the stations at `theta = +/- pi/2` | `<= 1e-6` | 1e-10 |
| MWR | record | the half without the option at the end stations (`K_III/A_III`, ADR 0610's end-face term) | none | |
| CL1 | c1 | coarse whole (no solve; `j_integral` of a zero field): `|sum w_j/(2 pi a) - 1|` and `max_j |w_j/(a pi/8) - 1|` | `<= 1e-12` | 1e-15 |
| CL2 | c1 | whole coarse ellipse (no solve): `|sum w_j/(4 a E(k)) - 1|` | `<= 1e-4` | 2.5e-5 |
| WE1 | c1 | whole coarse ellipse, mode I, ring 2: `K_I` against the periodic linear hat average of Irwin, every station | `<= 0.08` | 0.012 |
| WE2 | c1 | the same: `max(|K_II|, |K_III|)/max K_ref` (zero by the `z` symmetry of the mesh) | `<= 1e-6` | 1e-9 |
| R1 | D1 | a straight front `.closed()` refused (`"crack front"`, "closed") | refused | |
| R2 | D1 | the whole front with its last node dropped (odd), `.closed()`, refused ("closed") | refused | |
| R3 | D1 | the whole front with two corners swapped, refused ("order") | refused | |
| R4 | D2 | the half (PC) with a mirror plane `z = a` (no station) refused ("mirror plane") | refused | |
| R5 | D2 | the half with a mirror plane through the station `(0, -a, 0)` of normal `(1, 1, 0)/sqrt 2` refused ("mirror plane") | refused | |
| R6 | D2 | the closed whole front with a mirror plane refused ("mirror plane") | refused | |
| R7 | D2 | `interaction_integral` (ADR 0605) of the straight plate front with a mirror plane refused ("mirror plane") | refused | |
| BI | c4, c7 | D6: the dumps of ADR 0610's set-ups from b61032f4 and from the branch tip | byte-identical | identical |
| F1 | brief | every aggregated value finite | finite | finite |

The refusals R1 to R7 use zero displacement fields (no solve). The ADR 0610 tests (`sa_fracture_curved.rs`, fast and heavy, EC1
expected to miss with `2.2893e-2` as recorded), ADR 0605's (`sa_fracture_modes.rs`, fast and heavy) and ADR 0547's fast tests
(`sa_fracture.rs`) are run unchanged; `cargo test -p rcfd-api --release --test baseline_hashes` is run.

## Negative controls and mutations

Controls: WCR, WMR (the domain term alone on a closed front); MWR (the end-face term the option removes); MW2 (the option touches
nothing else). Mutations, each run once on a scratch copy (`f689-scratch/mut`, its own target dir), the fast tests of
`sa_fracture_closed.rs` (WM not run for mutations):

| mutation | change | expected to be caught by |
|---|---|---|
| Y1 | the periodic hat without wrapping `d` | WC1 to WC3 (the stations at the seam) |
| Y2 | the closed circle's weight as an open front's (half hats at the first and last stations) | CL1, WC1 |
| Y3 | the closed ellipse's weight without the closing edge | CL2, WE1 |
| Y4 | the mirror option zeroes `K_II` instead of `K_III` | MW1, MW2, MW3 |
| Y5 | the mirror option applied at every station | MW2 |
| Y6 | the tangency test of the mirror plane removed | R5 |
| Y7 | the reflected Hex20 without the node permutation | WC (the solve or the results) |
| Y8 | the period of a closed circle `2 pi` instead of `2 pi R` | CL1 or R (refused), WC |

## Decision rule

* **done**: WC1 to WC5, WM1 to WM7, MW1 to MW4, CL1, CL2, WE1, WE2, R1 to R7, BI, F1 met; every mutation caught by at least one
  check; the ADR 0610, 0605 and 0547 tests pass unchanged (EC1 its recorded known miss); fmt and clippy clean; `baseline_hashes`
  passes.
* **partial**: the code built and the fast checks run, but a registered check missed (a `*_known_miss` record), or WM or BI not
  run, or a mutation not caught.
* **todo**: the closed front not built.

## Limits (not covered)

As D8.

## Addendum 1: results (2026-11-15)

Code (commit 29a0b341): `crates/rcfd-fem/src/fracture.rs`, the only library file changed (336 lines added, 5 changed, after
`cargo fmt`): as D1 to D3 and D7. Existing lines changed, each adding a branch taken only by the new options or a field at its
default: the module documentation (lines 20-22), the `CrackFront` struct and its three constructors (104-107, 118-119, 130-131,
148-149), the new `closed`, `mirror_plane`, `is_closed`, `mirror_planes` (160-184), the node-count test of `CrackFront::check` (210-225, the open message unchanged), `mut s` (249), the unwrapping for
closed fronts and the span and gap checks (397-410, 425-449), `period` in `FrontGeometry` (458, 514-515; the
`#[allow(dead_code)]` of `length` removed, now used), the early branches of `hat` and `hat_integral` (605-607, 636-638), the new
private `closed_neighbours`, `closed_hat`, `closed_hat_integral`, `mirror_stations` (675-789), the mirror refusal of
`interaction_integral` (1270-1275), the documentation and the mirror stations of `interaction_integral_curved` (2751-2754, 2798,
2958-2959), and appended `CrackedWholeCylinder`, `HEX20_MIRROR`, `penny_crack_whole_cylinder` (3275-3379). Tests: the new file
`crates/rcfd-fem/tests/sa_fracture_closed.rs`. No existing test edited; `docs/reference-data.md` gets an F-689 section.

The fast tests were run once (`f689-scratch/run1.log`, 82.7 s for the four, one thread). BI was run once in each tree
(`f689-scratch/bi/base.out`, `tip.out`). No development probe was run before the registered run.

| check | limit | measured | met |
|---|---|---|---|
| WC1 whole penny, `psi = 35 deg`, ring 2, `K_I`, 16 stations | `<= 0.03` | 6.99e-4 | yes |
| WC2 `K_II`, all stations, `/A_II` | `<= 0.03` | 7.34e-4 | yes |
| WC3 `K_III`, all stations, `/A_III` | `<= 0.03` | 4.86e-3 | yes |
| WC4 `K_II`, `K_III` relative where `|K_ref| >= A/2` | `<= 0.03` | 5.00e-3 | yes |
| WC5 J (closed `j_integral`) against `G` of the closed forms | `<= 0.10` | 3.95e-2 (J low everywhere) | yes |
| WCR domain term alone (record) | none | `K_I` 3.27e-2, `K_II` 0.139, `K_III` 7.49e-2; 66 174 unknowns, 5 120 Hex20, solve 22.4 s | (record) |
| WM1 to WM7, WMR medium whole penny (heavy record) | as registered | **not run** (free RAM 4 GB to 6 GB, below the registered 16 GB, throughout) | **not run** |
| MW1 `K_III` at the two mirror-plane stations of the half | `== 0` | 0, 0 | yes |
| MW2 every other value against the run without the option | bit-identical | identical | yes |
| MW3 whole (`psi = 90 deg`) against the half with the option, `max |dK|/A` | `<= 1e-6` | 2.22e-13 | yes |
| MW4 whole, `|K_III|/A_III` at `theta = +/- pi/2` | `<= 1e-6` | 6.0e-15 | yes |
| MWR the half without the option, end-station `K_III/A_III` (record) | none | -0.958, -0.958 (closed form 0) | (record) |
| CL1 closed circle weights | `<= 1e-12` | 8.9e-16 (16 stations) | yes |
| CL2 closed ellipse weights against `4 a E(k)` | `<= 1e-4` | 2.45e-5 | yes |
| WE1 whole ellipse, mode I, ring 2, `K_I` against Irwin | `<= 0.08` | 1.10e-2 | yes |
| WE2 whole ellipse, `max(|K_II|, |K_III|)/max K_ref` | `<= 1e-6` | 9.4e-14 | yes |
| R1 to R7 the refusals | refused | each refused with its key and text | yes |
| BI dumps of ADR 0610's set-ups, b61032f4 against 29a0b341 | byte-identical | identical (40 lines, PC, EC, PQ, MS, PM, EH) | yes |
| F1 every aggregated value finite | finite | finite | yes |

**What the numbers say.** On the whole cylinder with a closed front and a shear direction that is neither a mesh mirror nor a
station direction, every station is an interior station: `K_III` is now checked at all 16 stations, including the four where the
half and quarter meshes are joined, and the errors equal those of ADR 0610's half model on the same mesh resolution (PC: 7.0e-4,
7.4e-4, 4.9e-3). Under ADR 0610's load the whole model reproduces the half model with the mirror option to `2.2e-13 A`: the
option gives at the mirror-plane stations the value of the whole model's symmetric hat, as D2 derives, where the half model
without the option has the end-face term `-0.958 A_III` (closed form 0). (ADR 0610's addendum lists `-0.83 A_III` for the end
stations of PC in its PCR row, the domain term alone; `-0.958` is the corrected value at ring 2 on the same mesh, printed here
for the first time.)

ADR 0610 unchanged: BI byte-identical; `cargo test -p rcfd-fem --release --test sa_fracture_curved --test sa_fracture_modes
--test sa_fracture -- --include-ignored` (`f689-scratch/existing.log`, `existing-modes.log`): `sa_fracture` 8 passed,
`sa_fracture_curved` 6 passed and EC1 its known miss with the recorded `2.2893e-2` (every other printed number as in ADR 0610's
addendum 1: PM1 5.82e-4, PM3 1.51e-3, PM5 1.27e-3, PM6 1.80e-2, PC3 4.93e-3, EH1 2.45e-3, MS 0), `sa_fracture_modes` 7 passed
(heavy record included). `cargo test -p rcfd-api --release --test baseline_hashes`: 3 passed, 1 ignored. `cargo clippy -p
rcfd-fem --all-targets --release -- -D warnings` clean; `cargo fmt -p rcfd-fem --check` clean.

Mutations (`f689-scratch/mutate.py` on the copy `f689-scratch/mut` of 29a0b341, target `target-mut`, the fast tests of
`sa_fracture_closed.rs`; log `f689-scratch/mutations.log`), each run once:

| mutation | caught by | as registered? |
|---|---|---|
| Y1 periodic hat without the wrap | WC1 to WC5 (1.0), CL2 (2.7e-2), WE1, MW3, MW4 | yes (more) |
| Y2 closed circle weight as open | CL1 (0.50), WC1 to WC5, MW3 | yes (more) |
| Y3 closed ellipse without the closing edge | CL2 (7.95e-2), WE1 (1.08) | yes |
| Y4 mirror zeroes `K_II` | MW1, MW2, MW3 (0.99) | yes |
| Y5 mirror at every station | MW2, MW3 (0.98) | yes |
| Y6 tangency test removed | R5 (accepted) | yes |
| Y7 reflected Hex20 without the permutation | WC, MW, WE, CL (the mesh refuses the inverted elements: `InvertedElement`) | yes |
| Y8 period `2 pi` on a closed circle | CL1, WC, MW (the front refused: "not in order") | yes |

All 8 mutations caught.

Deviations: none from the registered set-ups. **Not run: WM** (the heavy record, `wm1_to_wm7_whole_penny_heavy`, about 240 000
unknowns): the registration requires at least 16 GB of free RAM before it; the machine had 3.2 GB to 7.9 GB free during the
work (another tenant's WSL virtual machine held 28 GB to 34 GB), and a monitor checking every 30 s from 16:35 to 16:44 saw at most
6 GB. The test is in place, `#[ignore]`d, with its limits; it has not been run once, so no number of it exists to be kept or
re-run.

**Decision by the registered rule: partial.** WM was not run (the rule: "WM or BI not run" gives partial). Every check that was
run is met, every mutation is caught, BI is byte-identical, the ADR 0610, 0605 and 0547 tests pass unchanged (EC1 its recorded
known miss), fmt, clippy and `baseline_hashes` clean. Clause by clause: c0 every limit registered before the run; c1 the closed
front (WC, CL1, CL2, WE, R1 to R3) met; c2 the mirror option (MW1 to MW4, R4 to R7) met; c3 the whole penny inclined in a general
direction, `K_I`, `K_II` and `K_III` at every station within 3 % (WC1 to WC4: largest `4.9e-3`) met on the coarse mesh, the
medium record not run; c4 and c7 ADR 0610 unchanged (BI, its tests) met; c5 D4 verified; c6 WM `#[ignore]`d, RAM checked (and
found short); c8 D7 and MW2.

Remaining questions:

1. Status and WM. The acceptance's own clause (3 % at every station of a whole penny under a general direction) is met on the
   coarse whole cylinder; only the registered medium record is missing, for lack of free memory. Options: (a) partial as the rule
   gives, the lead runs `cargo test -p rcfd-fem --release --test sa_fracture_closed -- --ignored wm1_to_wm7 --nocapture` once
   on a machine with 16 GB free and records the numbers in an addendum 2; (b) done, treating WM as a record beyond the acceptance.
   Recommendation: (a), then done if WM meets its limits (expected: the half-model PM of ADR 0610 measured `1.5e-3` on the same
   resolution).
2. A closed front keeps the end-face exemption of `check_boundary` (D8): a ring boundary face normal to the local tangent would
   be accepted. Options: (a) leave it (no whole model has such a face); (b) disable the exemption for closed fronts (one
   condition, opt-in). Recommendation: (b) in a later item with a test that builds such a face; not done here because no
   registered check covers it.
3. A spline front (a front given by nodes only) and mixed-mode elliptical cracks remain uncovered (ADR 0610 D7); the citations of
   ADR 0610 remain from memory.
4. A case-file key for fracture parameters: still open (it would change the schema golden; not taken).

## Addendum 2: the WM record, run by the lead (2026-11-16)

The registered heavy record `wm1_to_wm7` was run once by the lead on the branch tip d3bfd588 (`cargo test -p rcfd-fem --release
--test sa_fracture_closed -- --ignored wm1_to_wm7 --nocapture`, 122 s) with 65 GB of free memory, above the registered 16 GB. The
limits are those of the registration; nothing was changed before the run. Log: `w49i-work/f689-scratch/wm-lead.log`.

| Check | Limit | Measured | Met |
|---|---|---|---|
| WM1 K_I against 2 sigma_nn sqrt(a/pi), every station | <= 3e-2 | 5.79e-4 | yes |
| WM2 K_II against A_II cos(theta - psi), / A_II | <= 3e-2 | 1.59e-4 | yes |
| WM3 K_III against A_III sin(theta - psi), / A_III | <= 3e-2 | 1.51e-3 | yes |
| WM4 K_II, K_III relative where abs(K_ref) >= A/2 | <= 3e-2 | 1.52e-3 | yes |
| WM5 spread over rings 2 to 4 / A_mode | <= 1e-2 | 1.27e-3 | yes |
| WM6 J against the hat average of G | <= 4e-2 | 1.80e-2 | yes |
| WM7 front mean of J against that of G | <= 2e-2 | 1.36e-2 | yes |

Recorded, no limit: the domain term alone (WMR, ring 3) errs by 3.06e-2 in K_I, 1.35e-1 in K_II and 6.95e-2 in K_III, so the
correction terms carry the medium mesh as they carried the coarse one. F1 (finite) met.

Decision: every registered check is now met; by question 1 of this ADR, F-689 is **done**.

## For the lead: roadmap entries

Proposed status of F-689: partial by the registered rule (question 1: done once the WM record is run and met).

> Proposed status: done (after addendum 2).

> Structural solver, advanced tier. Follow-up of F-682 (ADR 0610). ADR 0618, 2026-11-12: rcfd_fem::fracture gains closed (periodic) circular and elliptical crack fronts without end stations (CrackFront::closed: the node parameters unwrapped with the period, every station a full periodic hat, the weights of the closing edge included; j_integral and interaction_integral_curved take them), the mirror-plane option CrackFront::mirror_plane (K_III = 0 at stations on a mirror plane normal to the front, derived from the odd tearing displacement under the reflection; refused for a plane without a station, a plane not normal to the front, a closed front, and in interaction_integral), and penny_crack_whole_cylinder (the half cylinder of ADR 0610 reflected in x = 0). Defaults bit-identical: the results of ADR 0610's set-ups byte-identical to b61032f4 in a full {:?} dump, its tests and ADR 0605's unchanged. Met: a whole penny (66 174 unknowns, 16 stations) under tension inclined 45 deg towards psi = 35 deg, ring 2, K_I within 7.0e-4, K_II within 7.3e-4 A_II and K_III within 4.9e-3 A_III of the hat-averaged closed forms 2 sigma_nn sqrt(a/pi), A_II cos(theta - psi), A_III sin(theta - psi) at every station (limit 3 percent; ADR 0610's forms with the shear direction rotated, verified); the half model with the mirror option equal to the whole model within 2.2e-13 A (the end-face term it removes was -0.96 A_III); whole elliptical crack b/a = 0.5 mode I within 1.1e-2 of Irwin; 8 of 8 mutations caught. Not run: the medium whole-penny record (about 240 000 unknowns; free RAM below the registered 16 GB). Not covered: spline fronts, mixed-mode elliptical cracks, the end-face exemption of the ring boundary test on closed fronts. No case-file key.

No follow-up item is proposed: the remaining run is the WM record of this item (question 1), and question 2 is a one-condition change that can join a later fracture item.
