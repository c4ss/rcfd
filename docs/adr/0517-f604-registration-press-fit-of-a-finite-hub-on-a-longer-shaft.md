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

## Amendment 1 (2026-11-14): the `n = 32` solve failed; a defect of `project`; the fix, its bit record and the corrected heavy run

Written after the registered runs of the three fast tests, of A1 and of the heavy record, before any library code and before
the corrected run. Nothing above is changed.

**What happened.** The heavy record `f604_edge_convergence_and_bore_cell_rule_heavy` was run once, alone. Its `n = 4, 8, 16`
solves converged. The `n = 32` solve (33829 nodes, 101k unknowns, peak working set 3.9 GB) stopped after 1769 s with "load step 1
of 1, augmentation 0: the active set cycled 4 times in one Newton solve (more than 3) at Newton iteration 9; nodes switching:
[28549, 29092]". These are two bore nodes on the symmetry planes, `theta = 0` at `z = 1.71875` and `theta = 90` degrees at
`z = 1.75`. The test stays the ignored record of that failure; its numbers are in addendum 1.

**Diagnostic probes (disclosed).** All are uncommitted files in `f604-scratch`, run from `crates/rcfd-fem/tests` and deleted
afterwards.

- P4 (`probe_f604b.rs`, `n = 16`): the local coordinates of every bore node on its face at the converged state. The plane nodes
  are at `xi = -1` and `+1` exactly, and every node is at `eta` from `0.885` to `0.934`: inside its face, `0.07` or more from
  every other edge. So the axial offset of D3 works as designed.
- P5 (`probe_f604b.rs`, `n = 32`, `max_cycles` raised to 20 only to watch): a `Contribution` that adds nothing evaluates a
  second contact set at every Newton iterate (augmentation 0, so its multipliers are the solver's). After iterate 1, 49 bore
  nodes were inactive, and after iterate 3 none. At iterate 4 node 28549 had gap `-9.0e-8`, `xi = -1`, `eta = 0.8716`. At
  iterate 5 it had **no candidate face** (gap `None`) although nothing had moved it off its face. At iterate 6 it was back, with
  the penetration `4.7e-5` and a force 500 times its own: the active set cycles.
- P6 (`probe_f604c.rs`): the closest-point Newton of `project` (its arithmetic copied into the probe), applied at the converged
  states of `n = 4, 8, 16` to every bore node and its face. Every projection converged to `abs(dxi) <= 1e-14`. But the smallest
  step after the second iteration, which is the rounding floor of the iteration, was up to `3.2e-15`, `8.3e-15` and
  `9.9e-15`: it grows as the faces shrink.

**The cause: a defect of `contact.rs`.** `project` accepts the closest point only after a step `abs(dxi) <= 1e-14`, an
absolute tolerance in the face's parameters. The rounding floor of that step is about `ulp(abs(x))/(half the face's size)`. At
`abs(x) ~ 2` and a half-size of `0.0156` (`n = 32`) that is `4.4e-16/0.0156 = 2.8e-14`, above the tolerance. The steps then
wander at the rounding level for the 30 iterations, and the projection returns `None` ("not converged"). The node loses
contact for one iterate, and the solve cycles. The defect is not specific to this test. It hits any face smaller than about 40
ulps of its coordinates over `1e-14`: a face below about 2 percent of its distance from the origin, for example the 0.2 mm bore
cells of a 3 mm shaft at coordinates of 20 mm.

**The fix (D7, `contact.rs` `project` only).** When the 30 iterations end without a step `<= 1e-14`, the closest point is
accepted if the last step is `<= 1e-10`. The `|xi| <= 1 + 1e-2` test and everything after it are unchanged. Justification: the
gap is stationary in `xi` at the closest point, so an error `delta` in `xi` changes the gap by `O(delta^2 h)`, and `1e-10` is
far below anything the checks see. A step of `1e-10` after 30 Newton iterations on a bilinear face happens only at the rounding
floor, which is `1e-10` for a face of `1e-5` of its coordinates. **Bits:** a projection that converged before converges at the
same iteration, with the same bits. Only a projection that returned `None` for the iteration count can change, and then it
returns the point instead. No default path calls contact, and there is no case-file key.

**New checks (limits fixed now).**

| id | quantity | limit | estimate |
|---|---|---|---|
| B1 | the default-bits record: FNV-1a digests of the displacement, node forces, pressures, gaps and Newton residuals of three contact solves (ADR 0507 B1's two blocks pressed; ADR 0513's ADR 0507 fit; this ADR's hub at `n = 4`), recorded by the new file `tests/sa_contact_project_f604_default_bits.rs` on the unchanged library and written in before the fix | equal after the fix | P6: none of these projections is at the floor |
| PJ1 | a small face far from the origin: two unit-cell blocks of side `s = 1e-3` at `(10, 10, 10)`, the upper one offset by `0.3 s` in `x` and `y` and overlapping by `1e-3 s`; `ContactSet::evaluate` at `u = 0`: every contactor node has a candidate, and its gap against `-1e-3 s` | found; `<= 1e-9` relative | the floor `ulp(10)/(s/2) = 3.6e-12`, above `1e-14`: on the base the projection is predicted to fail (recorded, run once before the fix) |
| A1-H | the corrected heavy record `f604_edge_convergence_and_bore_cell_rule_heavy_a1`: the same model, checks and limits as the original (A1f, E1 to E4, R), run once after the fix, alone | as registered | n/a |

The fast tests are run again after the fix, as regression, not as new registered runs. Their numbers must be the same, because
no projection of `n <= 16` is at the floor (P6).

**Mutation ML3**: the fix reverted. Expected to be caught by PJ1. The decision rule adds B1 and PJ1 to the list for done and
counts A1-H in place of the original heavy record.

## Addendum 1: results (2026-11-14)

Commits after the registration (7150739e):

- the tests (671d6947);
- amendment 1 (8028545c);
- B1 and PJ1 recorded on the unchanged library (afbd6f3a, `f604-scratch/b1-base.log`);
- the fix in `contact.rs` `project` (45ce5ef4, lines 384, 404-405 and 410-414: a `last_step` and the acceptance at the rounding
  floor);
- the corrected heavy test A1-H, with the outcomes written into the two records' ignore reasons (bd923dee);
- a clippy form of PJ1's test (c56c9977).

Every registered run was made once, in release:

- the three fast tests (`run-fast.log`, 148 s);
- A1 (`run-a1.log`);
- the first heavy record (`run-heavy.log`, 1769 s), which failed at `n = 32`;
- B1 and PJ1, on the base and after the fix;
- A1-H, alone (`run-heavy-a1.log`, 1507 s). Its `n = 32` solve took 1336 s, with 2 augmentations of 5 and 2 Newton
  iterations. The first heavy run's peak working set at `n = 32` was 3.9 GB.

The fast tests were run again after the fix as regression (`regress.log`). Every reading is the same to the printed digits,
and B1 shows the bits are the same at `n = 4`. A1-H's readings at `n = 4, 8, 16` equal the first heavy run's.

All values are over `p_0 = 7.5e7` Pa unless stated.

| check | limit | measured | met |
|---|---|---|---|
| A1 `abs(p_mid/p_pe - 1)`, `n = 8` (the clause as written) | `<= 2e-2` | **`8.57e-2`** | **no** (predicted by D1; ignored record `f604_a1_plane_strain_closed_form_known_miss`) |
| A1f `abs(p_mid/p_0 - 1)`, `n = 4, 8, 16, 32` | `<= 2e-2` | `1.13e-2`, `4.76e-3`, `2.78e-3`, `2.11e-3` | yes |
| L8 `L = 8`: `abs(p_mid/p_0 - 1)`; `abs(B_0.25(L 8)/B_0.25(L 4) - 1)` | `<= 2e-2`; `<= 5e-3` | `3.40e-3`; `7.07e-4` | yes |
| NA1 `abs(p_mid/p_pe - 1)`, `n = 8` (negative control) | `>= 5e-2` | `8.57e-2` | as expected |
| CPE `u_z = 0` everywhere: against `p_pe`; against `p_0` | `<= 2e-2`; `>= 5e-2` | `7.57e-3`; `9.06e-2` | yes |
| CNU `nu = 0`: `abs(p_mid/p_0 - 1)` | `<= 2e-2` | `4.42e-3` | yes |
| S1 `e = 3 a` against `2 a`: `p_mid`, `B_0.25` | `<= 1e-3` each | `3.8e-7`, `6.5e-6` | yes |
| E1 the face row has the largest row resultant and holds the largest node pressure (every mesh and set-up) | holds | holds in all 10 solves | yes |
| E2 `P_f(2n)/P_f(n)` for `n = 4, 8, 16` | in `[1.08, 1.25]` | `1.1245`, `1.1480`, `1.1595` (`P_f` = 1.4415, 1.6210, 1.8609, 2.1576) | yes |
| E2 `abs(log2(P_f(32)/P_f(16)) - 0.226)` | `<= 0.05` | `1.25e-2` (local exponents 0.169, 0.199, 0.2135) | yes |
| E3 `B_0.25` (1.21213, 1.17540, 1.16128, 1.15666): decreasing differences; orders; `abs(D_3)/B(32)` | as registered; `>= 0.5`; `<= 1e-2` | holds; `1.38`, `1.61`; `3.99e-3` | yes |
| E3 `B_0.5` (1.09248, 1.07132, 1.06293, 1.06204): the same | as registered | holds; `1.33`, `3.25`; `8.33e-4` | yes |
| E3f `n = 4` to `8`: `B_0.25`, `B_0.5` | `<= 5e-2` | `3.03e-2`, `1.94e-2` | yes |
| E4 `abs(q_s(32)/q_s(16) - 1)`, `s = 0.25`, `0.5` | `<= 1e-2` | `1.74e-3`, `9.76e-4` | yes |
| R a rule `n* <= 16` exists | holds | **`n* = 8`** | yes |
| B1 the three digests after the fix | equal | equal (`0xf5619b600e920c5f`, `0x639fc374b3a9e917`, `0x5f0b23785337e56e`) | yes |
| PJ1 small face far from the origin: found, gap | found; `<= 1e-9` | on the base 2 of 4 nodes not found (`None`), as predicted; after the fix all 4 found, `7.48e-10` (the rounding of the placement `10 - 1e-6`) | yes |

Mutations: a `git archive` copy of c56c9977 in `f604-scratch/mut`, with `CARGO_TARGET_DIR` `target-f604-mut`, the script
`f604-scratch/mutate.sh`, and logs `MT1.log` to `ML3.log`.

| id | mutation | caught by |
|---|---|---|
| MT1 | bore at `a - 2 d` | A1f (`1.02`, `1.01`), CPE, CNU; the extension test's solve cycled |
| MT2 | `p_mid` read at the face | A1f (`0.21`, `0.18`), CPE, CNU, L8 |
| MT3 | matching planes (no axial offset) | the `L = 8` solve stalls at the relative residual `4.0e-5`: P2 again, with the fix in place, so P2's stall is the face switching of D3, not the rounding floor; recorded, as registered |
| ML1 | no Uzawa update | every fast test: the solves fail ("augmentation 1: not converged in 30 iterations", residual floor near `1e-12`) |
| ML2 | unnormalised normal | every fast test: "the gap tolerance 1e-9 ... not met after 20 augmentations" |
| ML3 | the fix reverted | PJ1 (2 of 4 nodes not found); B1 equal, as it must be |

Gates:

- `cargo test -p rcfd-fem --release` passes (exit 0; 839 passed, 0 failed, 233 ignored over 189 test binaries).
- `cargo test -p rcfd-api --release --test baseline_hashes` passes (3 passed, 1 ignored).
- `cargo clippy -p rcfd-fem --all-targets -- -D warnings` is clean.
- `cargo fmt -p rcfd-fem --check` is clean.
- `cargo test -p rcfd-cli --release --test docs_site` passes (run with this addendum in place).

### Findings

**Mid-length.** A frictionless hub with free faces on a longer shaft is at the `sigma_zz = 0` pressure `p_0` at mid-length,
not at the plane-strain `p_pe`. At `L = 4 (b - a)` the measured pressure is within `2.1e-3` of `p_0` (`n = 32`) and equals
`0.912 p_pe`. At `L = 8 (b - a)` it is within `3.4e-3` of `p_0`. Holding `u_z = 0` everywhere (CPE) brings `p_pe` back within
`7.6e-3`, and with `nu = 0` the two coincide (CNU). So the 8.6 percent of A1 is physics, not the code: **clause (a1) names the
wrong closed form.**

**The edge peak.** It is at the hub's faces on every mesh (E1). The pressure falls from mid-length to a minimum about
`0.5 (b - a)` from the face (the row there is at `0.956 p_0` at `n = 32`) and then rises to the face. **The peak value diverges.** The face row's
resultant grows by 1.124, 1.148 and 1.160 per halving of the bore cell. That approaches the `2^0.226 = 1.170` of the Williams
exponent `lambda = 0.774` (D2), for a 90-degree elastic corner pressed frictionless on a half-space of the same material. Any
reported peak is therefore a property of the mesh. **What converges** is the resultant over a band at the face:
`B_0.25 = 1.1567 p_0` and `B_0.5 = 1.0620 p_0` at `n = 32`, with orders 1.4 to 3.2 and last changes of `4.0e-3` and `8.3e-4`.
Extrapolating with the observed order 1.61 gives `B_0.25 = 1.1544 p_0` for the mean over the last quarter of `b - a`. The
fixed-distance row values settle to `1.7e-3` (E4).

**The bore-cell rule (R, from the run): 8 cells through `b - a` at the bore** (`h = (b - a)/8`). The cells are radial in the
hub and in the shaft's surface layer, the same size axially within `0.5 (b - a)` of each face, with `Delta <= 6` degrees in
`theta`. With this rule:

- The edge-band resultants are within `1.6e-2` (`B_0.25`) and `8.7e-3` (`B_0.5`) of the `n = 32` values, and within `1.8e-2`
  of the extrapolated `B_0.25`.
- The mid-length pressure is within `2.7e-3` of `n = 32` and within `4.8e-3` of `p_0`.

Four cells are not enough: `B_0.25` is `4.8e-2` off. Sixteen cells bring `B_0.25` within `4.0e-3` of `n = 32`, so use 16 when
the edge band must be within 1 percent.

The rule was run at `b/a = 2` (`b - a = a`) and is stated in `b - a`. For `b/a` far from 2 it is an extrapolation, and the
smaller of `a` and `b - a` is the safer unit.

**For the 120 mm fan the dimensions are nominal and partly missing.** `ComputerFan` gives the hub cup: radius 21.5 mm and
length 24 mm, so `L >= 4 (b - a)` holds for any boss thinner than 6 mm. It gives no shaft radius and no bore. As an
illustration only, assume a shaft of radius `a = 1.5 mm` and a boss of `b = 3 mm`; these values are not from the repository.
The rule then asks for bore cells of `0.19 mm` within `0.75 mm` of each hub face. The sharp-edged peak there is not a design
number. The band resultant is: `1.15 p_0` over the last `0.375 mm`.

**`contact.rs`.** The `n = 32` failure exposed a defect that is not specific to this test (amendment 1). The absolute `1e-14`
step tolerance in `project` lies below the rounding floor of faces smaller than about 2 percent of their coordinates. Such a
node drops out of contact for an iterate, and the active set cycles. The fix accepts the last step at the floor (`<= 1e-10`).
B1 shows the bits are unchanged where projections converged before, and PJ1 shows that the small face is found. Without the fix,
probe P5 ran its 30 Newton iterations and did not converge (`f604-scratch/probe7.log`: "not converged in 30 iterations
(relative residual 9.8e-1)", 5875 s).

**Deviations from the registration.** The only one is amendment 1 (the projection fix, B1, PJ1, A1-H), which was written and
committed before its code and its run. The probe file `f604-scratch/probe_f604.rs` is the first version of the probe. Its later
edits (row resultants, the axial offset, the residual prints) were made in the copy under `tests/` and are described in D5. The
logs `probe1.log` to `probe5.log` hold every number. **Not run:** nothing registered was skipped.

**Decision by the rule: partial.** Everything registered is met except A1, clause (a1) as written: `8.57e-2` against `2e-2`.
D1 predicted this before the run, and it is diagnosed: the plane-strain closed form does not describe a frictionless hub with
free faces, while `p_0` does, within `2.1e-3`. Done would need the acceptance changed to the `sigma_zz = 0` closed form, which
is the maintainer's decision (question 1).

### Remaining questions, with recommendations

1. **Clause (a1)'s closed form.** Options:
   - (a) Change the acceptance to "against the free-ended (`sigma_zz = 0`) closed form `E d (b^2 - a^2)/(2 a b^2)` within 2
     percent". This run meets that at `2.1e-3`, and F-604 would be done.
   - (b) Keep plane strain, which no correct code can meet for this set-up.
   - (c) Add an axially restrained variant.

   Recommendation: (a). CPE already shows that the plane-strain value is reproduced when the physics is plane strain.
2. **The fan's bore and shaft dimensions** are not in the repository. Options: (a) add a shaft radius and a boss to
   `ComputerFan` from a measured fan; (b) keep the rule dimensionless. Recommendation: (b) now, (a) when a fan is measured. This
   question is not taken here, because it needs data the repository does not have.
3. **The projection fix in `contact.rs`** is a library change the brief did not expect. Options: (a) keep it (B1, PJ1); (b) make
   the floor relative to the face size. Recommendation: (a). It is the smallest change that keeps every converged bit, and the
   fan's real bore cells (0.2 mm at 20 mm) need it.
4. **Speed.** The contact driver factorises with the simple-column `SparseLdlt`, which takes about 3 minutes per factorisation
   at 101k unknowns. Recommendation: a follow-up that lets `contact::linear_solve` use the supernodal factorisation of ADR
   0503/0509 (opt-in, with the default bits kept).

## For the lead: roadmap entries

> F-604 status partial. Note: Structural solver, advanced tier. Follow-up of F-599 (ADR 0513, 2026-11-14). ADR 0517, 2026-11-14. Model: a frictionless hub of length L on a shaft that runs 2a past each face, same material, b = 2a, d = 1e-3 a, an eighth of the fit in Hex8 with 6-degree sectors. The shaft's node planes are offset by 0.05 of a cell, so every bore node lies inside one shaft face. At mid-length the pressure matches the free-ended (sigma_zz = 0) closed form E d (b^2 - a^2)/(2 a b^2), within 2.1e-3 at L = 4 (b - a) and 3.4e-3 at L = 8 (b - a), and not the plane-strain one. The acceptance's plane-strain clause therefore misses at 8.6e-2 against 2e-2, as derived before the run: each body's axial resultant is zero, so sigma_zz = 0 in the far field. With u_z = 0 everywhere the plane-strain value comes back within 7.6e-3. The edge pressure peak is at the hub faces on every mesh and diverges as the Williams exponent 0.774 of a 90-degree corner on a half-space predicts: the face row grows by 1.124, 1.148 and 1.160 per halving, toward 1.170. The band resultants converge: the last quarter of b - a gives 1.157 p_0, with orders 1.4 to 1.6 and a last change of 4.0e-3. Bore-cell rule from the run: 8 cells through b - a at the bore, the same cell axially within 0.5 (b - a) of each face, and at most 6 degrees. That puts the edge bands within 1.6e-2 and the mid-length within 2.7e-3 of 32 cells; use 16 cells for 1 percent. The fan's shaft and bore dimensions are not in the repository; as a nominal illustration, a 1.5 mm shaft needs 0.19 mm bore cells. A 32-cell run exposed a projection tolerance in contact.rs that lies below the rounding floor of small faces. It is fixed: the closest point is accepted at a last step of at most 1e-10. Three digests show the bits unchanged where projections converged before, and a small face far from the origin is now found. All six mutations were caught or recorded as registered. No case-file key.

> Follow-up (proposed, not added): the press-fit acceptance against the free-ended closed form (deps F-604). Every limit not stated here is registered in the ADR before the run. Re-state F-604's clause (a1) against E d (b^2 - a^2)/(2 a b^2) (sigma_zz = 0), and keep plane strain for an axially restrained fit; the existing run of ADR 0517 is the evidence. Fan relevance: hub-shaft press-fit stresses.

> Follow-up (proposed, not added): the supernodal factorisation in the linear contact driver (deps F-427, F-594). Every limit not stated here is registered in the ADR before the run. contact::linear_solve with an opt-in supernodal L D L^T: the ADR 0517 hub fit at 32 cells through b - a in under a third of the present time, every reading within 1e-10 of the present driver, and without the option every result bit-identical. Fan relevance: hub-shaft press-fit stresses at fan scale.
