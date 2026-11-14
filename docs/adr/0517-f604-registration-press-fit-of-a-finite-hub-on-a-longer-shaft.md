# ADR 0517: F-604 registration: press fit of a finite hub on a longer shaft, the edge pressure peak and a bore-cell rule

Status: **REGISTERED 2026-11-14, written before any code of F-604 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text, diagnostic probes were run. They are disclosed below with every number used here. The probe was an
uncommitted test file, `f604-scratch/probe_f604.rs`, copied into `crates/rcfd-fem/tests/` only while it ran. It builds the
hub-on-shaft model through the existing public API (`contact::solve`, `SolidMesh`, `SolidModel`). There is also one disclosed
piece of arithmetic, `f604-scratch/corner_exponent.py` (numpy, no rcfd code), for the corner singularity. No library code was
written or changed. The limits below are fixed from the derivations and the probes. Results go in the addenda, and nothing above
the addenda is changed after a run.

Roadmap item F-604 (deps F-599). Number 0517 given by the lead. Branch `f604-work` (base 2f8c1d57), worktree `w49i-work/f604`,
scratch `w49i-work/f604-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits are fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with its limit intact); each registered run is made once; defaults stay bit-identical; `baseline_hashes.rs`,
the hash tables and the golden files are not edited; no new dependency and no unsafe code; existing tests and ignored records are
not edited; clean-room.

## What F-604 asks

The acceptance, verbatim, binding:

> Every limit not stated here is registered in the ADR before the run. A hub of length L on a shaft that extends beyond both of its faces, same material, radial interference d, frictionless: the contact pressure at mid-length against the plane-strain closed form within 2 percent for L >= 4 (b - a), the edge pressure peak located at the hub faces and converging under refinement of the bore cells, and a bore-cell rule for the 120 mm fan hub stated from the run. Fan relevance: hub-shaft press-fit stresses.

Clauses: (a1) the mid-length pressure against the plane-strain closed form within 2 percent for `L >= 4 (b - a)`; (a2) the edge
peak located at the hub faces; (a3) the edge peak converging under refinement of the bore cells; (a4) a bore-cell rule for the
120 mm fan hub, stated from the run. The brief adds the following. (b1) Derive the plane-strain closed form, and register `a`,
`b - a`, `L` and the shaft extension. (b2) Probe the edge singularity and register what converges, not a singular peak value.
(b3) Use quarter or eighth symmetry with Hex8, probe memory and time, and make a heavy run an ignored record that is run once,
alone. (b4) Mostly a test: no library edit is expected and no case-file key is added. (b5) Do not edit `nl_transient.rs`,
`nonlinear.rs` or `random.rs`. (b6) Defaults keep their bits and `baseline_hashes` is unchanged. (b7) Every aggregated error must
be finite.

## What was read

ADR 0507 (D1 to D4, D7 I, the node areas and the node pressure, amendment 2's one-percent face extension), ADR 0513 (D1: a node
pressure is a force over the contactor's chord area; D2: the `O(h)` bore error; D3: plane strain), `contact.rs` (`project`, the
search, `ContactSet::new`, `linear_solve`, `solve`; the linear driver uses `SparseLdlt` with minimum degree),
`tests/sa_contact_fit_f599.rs` (`Checks`, `worst`, `fit_model`), `rcfd-mesh/src/fan_design.rs` (`ComputerFan` default:
`r_hub = 0.0215` m, `MotorHub { z_start: 0.001, length: 0.024 }`; it has no shaft and no bore), and
`docs/fan-project-breakdown.md` (no shaft or bore dimension).

## D1. The closed forms (first principles)

A solid shaft of radius `a` and a hub (ring) of bore `a` and outer radius `b`, the same `E` and `nu`, with radial interference
`d` (the hub's bore is `a - d`), `d << a`. Lame (Timoshenko and Goodier, *Theory of Elasticity*, cited from memory; re-derived
here): in each body `u_r = A r + B/r`, with `B = 0` in the solid shaft, and `sigma_r + sigma_theta` constant in each body.

**Plane strain** (`eps_zz = 0` in both bodies). The constants are `E' = E/(1 - nu^2)` and `nu' = nu/(1 - nu)`. The shaft under
the external pressure `p`: `sigma_r = sigma_theta = -p`, so `u_s(a) = -(1 + nu)(1 - 2 nu) p a/E`. The ring under the internal
pressure `p`: `u_h(a) = (1 + nu) p a ((1 - 2 nu) a^2 + b^2)/(E (b^2 - a^2))`. Closing the interference,
`u_h(a) - u_s(a) = d`, gives `(1 + nu) p a ((1 - 2 nu)(b^2 - a^2) + (1 - 2 nu) a^2 + b^2)/(E (b^2 - a^2)) = 2 (1 - nu^2) p a
b^2/(E (b^2 - a^2)) = d`, so

    p_pe = E d (b^2 - a^2) / (2 a b^2 (1 - nu^2))     (ADR 0513 I1pe's formula).

**Free ends** (`sigma_zz = 0` in both bodies). The shaft: `u_s(a) = -(1 - nu) p a/E`. The ring: `u_h(a) = p a ((1 - nu) a^2 +
(1 + nu) b^2)/(E (b^2 - a^2))`. Closing the interference gives `2 p a b^2/(E (b^2 - a^2)) = d`, so

    p_0 = E d (b^2 - a^2) / (2 a b^2),    p_pe = p_0 / (1 - nu^2).

**Which one holds at mid-length of a long hub.** The interface is frictionless, so it carries no axial traction. Both hub faces
are free, and so are both shaft ends. The axial resultant of each body over any cross-section is therefore zero, separately for
each body. Far from the faces the state is independent of `z`, with `eps_zz` uniform in each body (generalised plane strain).
Then `sigma_zz = E eps_zz + nu (sigma_r + sigma_theta)` is uniform in each body, because `sigma_r + sigma_theta` is uniform in
Lame. A uniform `sigma_zz` with a zero resultant is zero. So the far field of a long frictionless press fit with free ends is
the `sigma_zz = 0` state, and its pressure is `p_0`, not `p_pe`. This is the open-ended cylinder of the textbooks. The two bodies
then change length differently: the shaft by `eps_zz = 2 nu p/E`, the hub by `-2 nu p a^2/(E (b^2 - a^2))`, and the interface
slides. The plane-strain closed form of clause (a1) is off by the factor `1/(1 - nu^2) = 1.0989` at `nu = 0.3`.
**Clause (a1) as written is therefore predicted to miss**, with `abs(p_mid/p_pe - 1) = 1 - (1 - nu^2) (1 + e_h) ~ 0.08 to 0.09`,
where `e_h` is the mid-length discretisation and end-effect error (P1 below: `0.0806`). It is registered literally (A1) and
expected to be a `*_known_miss` record. The derivation's reference is registered as A1f. The two controls CPE and CNU (D5)
separate the physics from the code: with `u_z = 0` everywhere the model is in plane strain and must give `p_pe`, and with
`nu = 0` both closed forms coincide.

## D2. The edge singularity (first principles; disclosed arithmetic)

Near the hub's bore corner at a face, the local problem is two-dimensional in the meridian plane and in plane strain (the hoop
strain is regular there). An elastic 90-degree wedge (the hub corner) is pressed, frictionless, on an elastic half-plane (the
shaft surface, which continues beyond the face), and both are the same material. Williams' method (Williams, J. Appl. Mech. 19
(1952) 526, cited from memory; the formulas below are re-derived and checked) uses the stress function
`Phi = rho^(lambda + 1) (A cos((lambda + 1) phi) + B sin((lambda + 1) phi) + C cos((lambda - 1) phi) + D sin((lambda - 1) phi))`
in each body, with the displacement of each term taken from Barber, *Elasticity*, Table 8.1 (from memory). The script first
checks those displacements against Hooke's law by finite differences (relative residual `3.0e-8`). The boundary conditions are:
the hub, `0 <= phi <= pi/2`, traction-free at `phi = pi/2`; the shaft, `-pi <= phi <= 0`, traction-free at `phi = -pi`;
shear-free on both sides of the interface `phi = 0`; `sigma_phiphi` and `u_phi` continuous there. The eight-by-eight determinant
has its first root in `(0, 1)` at **`lambda = 0.774013`**, independent of `nu`: the contact pressure behaves as
**`p ~ K rho^(-0.226)`**. Control: the same script with the interface bonded (the four continuity conditions) gives `0.5445` and
`0.9085`. These are the known roots of a single body with a 270-degree re-entrant corner, so the assembly of the determinant is
right.

Consequences, registered:

- **The peak value diverges.** On a mesh of cell `h` at the corner, the corner node's pressure (a force over a tributary area of
  size `h`) grows like `h^(-0.226)`: by `2^0.226 = 1.170` per halving, asymptotically. It is not a convergent quantity, and no
  limit is set on its value. Its growth rate is checked against the prediction (E2).
- **What converges.** The singularity is integrable, so the resultant pressure over a fixed band next to the face converges. The
  singular part's error is `O(h^0.774)` and the regular part's `O(h)`. These are E3's band means over `w = 0.25 (b - a)` and
  `w = 0.5 (b - a)`. The pressure at a fixed distance from the face converges too, but at the probe's meshes the pollution from
  the corner keeps its differences from falling (P1). It is checked only as a bounded change (E4).
- **The location.** The resultant pressure of the node row at the face is the largest of all rows, and the largest node pressure
  is in that row (E1).

## D3. The model (registered)

- **Dimensions.** `a = 1`, `b = 2` (so `b - a = a`, `b/a = 2`, as in ADRs 0507 and 0513), `d = 1e-3 a`, `E = 2e11`, `nu = 0.3`,
  frictionless, linear kinematics. The hub length is `L = 4 (b - a) = 4`, the smallest the clause names, and `L = 8 (b - a)` also
  runs as the longer case (clause: `L >= 4 (b - a)`).
- **Shaft extension** `e = 2 a` beyond each face. Why: the end disturbance of a free shaft end decays into the shaft as
  `exp(-k z/a)` with `Re k` near 2.7 for the first axisymmetric end mode of a solid cylinder (from memory, not derived). At
  `2 a` that is about `5e-3` of a disturbance that is itself small (the shaft beyond the face carries almost nothing). The check
  S1 runs `e = 3 a` against `e = 2 a`. P3: the band mean changes by `1e-5`.
- **Symmetry: an eighth.** `theta` in `[0, 90]` degrees with `u_x = 0` on `x = 0` and `u_y = 0` on `y = 0`, and `z >= 0` with
  `u_z = 0` on the hub's mid-plane `z = 0` (for both bodies). The model holds the two rigid motions left by frictionless contact
  (the hub's axial translation by `z = 0`, the rotation about `z` by the two planes).
- **Angular mesh.** `N_theta = 15` cells over 90 degrees (`Delta = 6` degrees, close to ADR 0513's 5.625). The hub's bore nodes
  are at `theta_j = j Delta`, radius `a - d`. The shaft is a polygon whose sides have their midpoints at radius `a` at the
  angles `theta_j`: its vertices are at radius `a/cos(Delta/2)` and angles `(j - 1/2) Delta`, plus its two half-sides ending on
  the symmetry planes at `(a, 0)` and `(0, a)`. Every bore node then has the initial gap `-d` exactly, which is ADR 0507 D7 I's
  construction cut at the planes. The shaft's cross-section is a square core `[0, 0.4 a]^2` of `8 x 8` cells. A band of
  `n_b = max(3, ceil(0.6 n))` uniform layers maps the core's boundary (16 segments) onto the polygon (16 segments: 14 sides and
  two half-sides).
- **Radial mesh.** The hub has `n` uniform cells through `b - a + d`. The shaft band has the `n_b` layers above, so its cell at
  the surface is about `0.6 a/n_b`, about `(b - a)/n`.
- **Axial mesh** (measured from each face, the same on both sides of it). There are three regions. Within `0.5 (b - a)` of the
  face the cells are uniform, of size `h = (b - a)/n` (the bore cells). From `0.5 (b - a)` to `(b - a)` there are
  `m = round(1/(h + 0.25))` cells whose sizes rise linearly from `h` to `0.25 (b - a)`, scaled to fill the region. Beyond that
  the cells are uniform at `0.25 (b - a)`, up to the mid-plane in the hub and up to the end in the shaft. The hub's rows
  `z = 0, 0.25, ...` up to `L/2 - (b - a)` are therefore the same on every mesh, and so are the rows within `0.5 (b - a)` of the
  face, on a mesh family that halves `h`.
- **Axial offset of the shaft's nodes** (from the probe, P2 and P3). The shaft's node planes are the hub's planes, each one
  except `z = 0` and the shaft's end moved by `0.05` of the next cell toward `+z`. Why: with matching planes every bore node sits
  on an edge shared by two shaft faces. The frictionless slide moves the node by less than the one-percent face extension of ADR
  0507 amendment 2, so the node is a candidate of both faces. In the far field, where the two faces are nearly coplanar, the
  choice between them flips by rounding, and with it the force split over their nodes. In the probe the Newton residual stalled
  at `3.7e-5` (P2, `L = 8`). With the offset each node lies inside one face only, `0.1` in `xi` from its edge, against the `0.01`
  extension. The slide (`3e-4 z` toward the mid-plane, relative to the shaft) moves it deeper. The price is a node-to-segment
  pattern change at `z = 0`, where the offset is zero: there the row pressure is `2.3e-2` below its neighbour's (P3). The checks
  therefore read **resultants over bands of rows**, never a single row (D4).
- **Contact.** The pair is the bore faces (contactor) against the shaft's outer faces (target), penalty `100 E/h`, one load step,
  penetration tolerance `1e-6 d`, Newton up to 30 iterations (ADR 0513's solve with `h` for `1/8`).
- **Mesh family.** `n = 4, 8, 16, 32`, so `h = (b - a)/n` halves each time. `n = 32` is the heavy record (D6).

## D4. What is read

For each row `k` of bore nodes (one `z`, all `theta_j`), `F_k` is the sum of the node forces and `A_k` the sum of the node areas
(ADR 0507 D4). Every quantity is a ratio of force to area, so the symmetry planes' half-area nodes enter with their weight. The
probe showed why this matters. The bore node on a symmetry plane sits on a shaft node, the end of a half-side, while all the
others sit at side midpoints. That pattern change moves force from the plane node (`0.82` of the row) to its neighbour (`1.11`),
and the interior nodes are uniform to `4e-4` (P1). The resultant of a row is unaffected to `3e-4`.

- `p_mid`: **the contact pressure at mid-length**, the resultant over `0 <= z <= 0.25 (b - a)`:
  `(F_0 + F_1/2)/(A_0 + A_1/2)`, with the rows at `z = 0` and `z = 0.25`.
- `B_w`: the resultant over the band `L/2 - w <= z <= L/2` at the face, the row at `L/2 - w` half-weighted, for
  `w = 0.25 (b - a)` and `0.5 (b - a)`.
- `P_f`: the face row's resultant `F_K/A_K` (the peak, D2), and `q_s`: the resultant of the row at the distance `s` from the
  face, `s = 0.25 (b - a)` and `0.5 (b - a)`.
- The row profile `F_k/A_k` over `k` and the largest node pressure with its row.

## D5. The probes (disclosed; every number used)

The probes used the model above with two differences. The axial mesh outside the face zones grew geometrically by 1.25 up to
`0.25 (b - a)` (scaled to fit), so the middle rows were not common to all meshes. And in P1 and P2 the shaft's planes matched the
hub's. Values are `F/A` over `p_0 = 7.5e7` Pa (`p_pe/p_0 = 1.098901`).

| probe | set-up | time | result |
|---|---|---|---|
| P1 | matching planes, `L = 4`, `n = 4, 8, 16` | 7, 26, 163 s (2964, 5546, 12907 nodes); peak working set 1075 MB over the three | z = 0 row: 1.01795, 1.01165, 1.00969 (`p/p_pe` 0.9263, 0.9206, 0.9188); `B_0.25`: 1.20736, 1.17149, 1.15939; `B_0.5`: 1.08937, 1.06926, 1.06373; `P_f`: 1.43677, 1.61463, 1.85269 (ratios 1.1238, 1.1474); `q_0.25`: 0.97794, 0.97871, 0.98294; `q_0.5`: 0.96483, 0.96662, 0.96947; the largest node pressure in the face row on every mesh; a dip of the profile to about 0.966 near `0.4 (b - a)` from the face; plane-node ratio 0.82, its neighbour 1.11, interior uniform to `4e-4` |
| P2 | matching planes, `L = 8`, `n = 8` | 12.7 min | "not converged in 30 iterations (relative residual 3.9e-5)"; again with 60 iterations, 3.7e-5 |
| P3 | planes offset by 0.05, `n = 4, 8` (`L = 4`); `n = 8`: `L = 8`; `e = 3`; `nu = 0`; `u_z = 0` everywhere | 4, 21, 61, 21, 17, 5 s; 3 augmentations each, at most 4 iterations per solve | `n = 4`: rows z = 0, 0.25: 0.99325, 1.02929; `B_0.25` 1.21213; `P_f` 1.44148. `n = 8`: rows 0.98749, 1.02225 (z = 0, 0.235); `B_0.25` 1.17545; `B_0.5` 1.07006; `P_f` 1.62109 (ratio 1.1246); `q_0.25` 0.97622. `L = 8`: rows from z = 0.49 to 2.9 between 1.0033 and 1.0072; `B_0.25` 1.17635. `e = 3`: `B_0.25` 1.17546. `nu = 0`: rows 0.98638, 1.02274. `u_z = 0`: rows 1.07193, 1.10942 (over `p_pe`: 0.9755, 1.0096) |

Reading: the mid-length is at `p_0` within about 1 percent and `0.08` below `p_pe` (D1 confirmed). The face-row resultant grows by
`1.124` and then `1.147` per halving, toward D2's `1.170`. The band means fall with differences `-0.0359`, then `-0.0121` (order
`1.57`). The fixed-distance values move by less than `5e-3` but not monotonically. The offset removes P2's stall without moving
the band means (`B_0.25`: `1.17545` against `1.17149` with matching planes at `n = 8`, within the mesh change).

## D6. Tests (one new file, `crates/rcfd-fem/tests/sa_contact_hub_f604.rs`)

The builder of D3 and the readings of D4, with ADR 0513's `Checks` and `worst` (every aggregate finite; b7). No library change.

- `f604_a1f_e1_hub_on_shaft_n4_n8` (fast, about 30 s): A1f, E1, E3f, NA1.
- `f604_controls_plane_strain_and_nu0` (fast, about 25 s): CPE, CNU.
- `f604_extension_and_long_hub` (fast, about 90 s): S1, L8.
- `f604_a1_plane_strain_closed_form_known_miss` (`#[ignore]`, the literal clause, predicted to miss by D1; one `n = 8` solve).
- `f604_edge_convergence_and_bore_cell_rule_heavy` (`#[ignore]`, heavy record: `n = 4, 8, 16, 32`, about 25 minutes and 3 to 5
  GB estimated from P1's scaling; run once, alone): A1f on every mesh, E1 to E4, R.

## Checks (limits fixed now)

Relative errors are `abs(x/x_ref - 1)`. A non-finite value is a miss.

| id | clause | quantity | limit | estimate |
|---|---|---|---|---|
| A1 | a1 (literal) | `abs(p_mid/p_pe - 1)`, `n = 8`, `L = 4` | `<= 2e-2` | **predicted miss**: D1, P1 `0.080` |
| A1f | a1 (D1's reference) | `abs(p_mid/p_0 - 1)` on every mesh of `L = 4` (`n = 4, 8, 16, 32`) | `<= 2e-2` | P3 `1.1e-2`, P1 `1.2e-2` and below |
| L8 | a1 (`L >= 4 (b - a)`) | `L = 8`, `n = 8`: `abs(p_mid/p_0 - 1)`; `abs(B_0.25(L 8)/B_0.25(L 4) - 1)` | `<= 2e-2`; `<= 5e-3` | P3: about `4e-3`; `7.6e-4` |
| NA1 | a1 (D1) | `abs(p_mid/p_pe - 1)`, `n = 8`, `L = 4` | `>= 5e-2` | D1: 0.08 to 0.09 |
| CPE | control | `u_z = 0` at every node, `n = 8`: `abs(p_mid/p_pe - 1)`; `abs(p_mid/p_0 - 1)` | `<= 2e-2`; `>= 5e-2` | P3: about `7.6e-3`; `9.1e-2` |
| CNU | control | `nu = 0`, `n = 8`: `abs(p_mid/p_0 - 1)` (here `p_0 = p_pe`) | `<= 2e-2` | P3: about `4.6e-3` |
| S1 | b1 | `e = 3 a` against `e = 2 a`, `n = 8`: `abs` relative change of `p_mid`, of `B_0.25` | `<= 1e-3` each | P3: `1e-5` |
| E1 | a2 | on every mesh and set-up run: the face row's resultant is the largest row resultant, and the largest node pressure lies in the face row | holds | P1, P3: holds everywhere |
| E2 | a3 (the peak) | `P_f(2n)/P_f(n)` for `n = 4, 8, 16`: each in `[1.08, 1.25]`; and `abs(log2(P_f(32)/P_f(16)) - 0.226)` | as stated; `<= 0.05` | D2: `1.170`, exponent 0.226; P1: 1.124, 1.147 (exponents 0.169, 0.198) |
| E3 | a3 (what converges) | for `w = 0.25, 0.5`: `D_1 = B(8) - B(4)`, `D_2 = B(16) - B(8)`, `D_3 = B(32) - B(16)`: `abs(D_3) < abs(D_2) < abs(D_1)`; both orders `log2(abs(D_i/D_{i+1})) >= 0.5`; `abs(D_3)/B(32)` | as stated; `<= 1e-2` | D2: order 0.77 to 1; P1: orders 1.57 (`w = 0.25`), 1.84 (`0.5`); `abs(D_2)/B = 1.0e-2`, `5e-3` |
| E3f | a3 (fast) | `n = 4, 8`: `abs(B(8)/B(4) - 1)` for `w = 0.25, 0.5` | `<= 5e-2` | P3: `3.0e-2`, about `2e-2` |
| E4 | a3 (fixed distance) | `abs(q_s(32)/q_s(16) - 1)` for `s = 0.25, 0.5` | `<= 1e-2` | P1: `4.3e-3`, `2.9e-3` at 16/8 |
| R | a4 | the bore-cell rule exists: some `n* <= 16` meets R's criteria (below) | holds | P1: `n* = 8` expected (`B_0.25` about 1.8 percent off) or 16 |

**The bore-cell rule** (R). With the heavy run's `n = 32` as the reference, `n*` is the smallest `n` in `{4, 8, 16}` that meets
all of the following. (i) The edge band resultants are within 2 percent of the reference: `abs(B_w(n)/B_w(32) - 1) <= 2e-2` for
`w = 0.25` and `0.5`. (ii) The mid-length pressure is within 1 percent of the reference: `abs(p_mid(n)/p_mid(32) - 1) <= 1e-2`.
(iii) It is within 2 percent of the closed form: `abs(p_mid(n)/p_0 - 1) <= 2e-2`. The reference's own error is estimated by
`abs(D_3)` and stated with the rule. The rule is stated as `n*` cells through `b - a` at the bore, in the hub and in the shaft's
surface layer, with cells of the same size `(b - a)/n*` axially within `0.5 (b - a)` of each face, and `Delta <= 6` degrees.
The rule is for the band resultants. **It is not a rule for the peak value**, which has none (D2). For the fan the rule is given
in these units. `ComputerFan` gives a hub cup of radius 21.5 mm and length 24 mm, and **no shaft radius and no bore**. Any
millimetre figure is therefore illustrative and nominal: an assumed shaft radius times `1/n*`, labelled as such. The bore and
shaft dimensions are an open question.

Negative controls: NA1 (the derivation's 8 to 9 percent, `>= 5e-2`); CPE's second limit (plane strain is not `p_0`).

Planned mutations (a `git archive` copy of the tip with its own `CARGO_TARGET_DIR`, running the three fast tests):

| id | mutation | expected to be caught by |
|---|---|---|
| MT1 | test: the hub's bore at `a - 2 d` (twice the interference) | A1f (`p_mid/p_0` near 2) |
| MT2 | test: `p_mid` read from the face rows instead of the mid-plane rows | A1f (`B_0.25` about 1.17) |
| MT3 | test: no axial offset of the shaft's planes (matching planes), in `f604_extension_and_long_hub` | the `L = 8` solve stalls (P2); recorded, not required |
| ML1 | `contact.rs`: the Uzawa update left out | the solves fail (augmentations exhausted) |
| ML2 | `contact.rs`: `project` returns the unnormalised normal | the solves fail or A1f |

## Decision rule

- **done**: A1 met (predicted not to be, by D1), and A1f, L8, NA1, CPE, CNU, S1, E1, E2, E3, E3f, E4 and R met; MT1, MT2, ML1 and
  ML2 caught; no library change; the existing `rcfd-fem` tests passing; `baseline_hashes` passing; fmt and clippy clean.
- **partial**: A1f, E1, E3 and R met (the mid-length against the derived reference, the location, a convergent quantity and the
  rule), and every other miss diagnosed and recorded as a `*_known_miss` test. **This is the predicted status**, because the
  literal clause (a1) names a closed form that D1 shows does not hold at mid-length of a frictionless hub with free faces.
- **todo**: otherwise.

## Limits (not covered)

- Friction. A stuck interface carries axial shear, so the far field is no longer `sigma_zz = 0` in each body. The edge
  singularity also changes.
- A rounded or chamfered bore edge, which removes the singularity. The rule's band resultants are what a sharp-edged design can
  be judged on.
- Different materials (a steel shaft in a plastic hub). D2's exponent and D1's closed forms change: the two-material Lame
  formula, and Dundurs' parameters for the corner.
- Large deformation, plasticity, and centrifugal loading of the hub (the fit's loss of pressure with speed).
- The fan's real bore and shaft dimensions, which the repository does not have.
- No case-file key (b4).
