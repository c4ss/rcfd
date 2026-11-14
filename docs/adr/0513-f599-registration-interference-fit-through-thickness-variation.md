# ADR 0513: F-599 registration: the through-thickness variation of the interference-fit contact pressure, and I3 re-registered

Status: **REGISTERED 2026-11-14, written before any code of F-599 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text one diagnostic probe was run (disclosed below with every number it printed): an uncommitted test
file `f599-scratch/probe_f599.rs`, copied into `crates/rcfd-fem/tests/` only while it ran and deleted afterwards, which builds
the ring-on-shaft fit of ADR 0507 D7 I with parameters (layers through the thickness, the `u_z` constraints, `nu`, `d`, the
ring's radial cells) through the existing public API. No library code was written or changed. The limits below are fixed from
the derivation in D2 and the probe; results go in the addenda; nothing above the addenda is changed after a run.

Roadmap item F-599 (deps F-427). Number 0513 given by the lead. Branch `f599-work` (base d41012db), worktree
`w49i-work/f599`, scratch `w49i-work/f599-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, of the hash tables or of any golden file; no new dependency, no unsafe code; existing tests and ignored
records not edited; clean-room.

## What F-599 asks

The acceptance, verbatim, binding:

> Every limit not stated here is registered in the ADR before the run. Find why the one-layer ring-on-shaft fit of ADR 0507 has node pressures of 0.9955 and 1.0045 of the mean at the bottom and top, then re-register I3. Fan relevance: hub-shaft press-fit stresses.

Clauses: (a1) find why (a diagnosis, with numbers); (a2) re-register I3 (a correct set-up or a correct reference, a limit
justified by the probe, the new check run once). The brief adds: (b1) if the cause is a defect of `contact.rs`, fix it with a
default-bits test first and keep the other F-427 results; if the I3 expectation was wrong, say so and keep I3's original as an
ignored record; (b2) no case-file key; (b3) do not edit the files of F-559, F-561 and F-570; (b4) defaults keep their bits,
`baseline_hashes` unchanged; (b5) every aggregated error finite.

## What was read

ADR 0507 (D7 I, arithmetic 4, the checks I1 to I4, addendum 1 with the I3 miss and its partial diagnosis), `contact.rs` (the
node areas of D4, `project`, `face_gap`, `evaluate_all`, `force_and_tangent`, the linear driver), `tests/sa_contact.rs`
(`fit_model`, `i1_to_i4_..._a3_known_miss`, the `Checks` helper and `worst`).

## D1. ADR 0507's I3 and what it assumed

I3 checked every shaft integration point: `sigma_xx, sigma_yy` against `-p_fe` and `sigma_xy` against 0, over `p_fe`, the mean
node pressure, limit `1e-6`, estimate "exact up to rounding". It measured `7.02e-3`. Two assumptions were in that estimate:

1. **The reference.** A node pressure is the node's force over its area on the *contactor* (the ring's bore, a chord of the
   circle `a - d`: `A_s = (a - d) 2 sin(Delta/2) t/2`). The shaft receives that force on its *polygon side* of length
   `2 a tan(Delta/2)`, half to each vertex (the ring node is at the side's mid-point). The uniform shaft stress of a force `F` per
   ring node is therefore `p_s = F/(a tan(Delta/2) t) = p (1 - d/a) cos(Delta/2)`, not `p`: with `d = 1e-3`, `Delta = 2 pi/64`,
   `p_s/p = 0.999 x 0.9987955 = 0.9977966`. ADR 0507's I3 with `-p_fe` is `2.2e-3` away from the exact discrete answer whatever
   else happens: the expectation was wrong in its reference.
2. **No through-thickness variation.** The plane-stress continuum solution (Lame in each body, `u_z = c z`) is an exact 3-D
   solution: in each body `sigma_r + sigma_theta` is constant (the shaft `-2 p`, the ring `2 p a^2/(b^2 - a^2)`), so `eps_zz =
   -nu (sigma_r + sigma_theta)/E` is constant, `u_z = eps_zz z` has no `r`-gradient, `gamma_rz = 0`, and the frictionless
   interface slides by the difference of the two thickness changes. The discrete ring does not reproduce this (D2).

## D2. The cause (first principles, arithmetic and probe)

**The ring's discrete thickness change is not constant near the bore.** Lame's ring displacement is `u(r) = C ((1 - nu) r + (1 +
nu) b^2/r)`, `C = p a^2/(E (b^2 - a^2))`; the strain sum `du/dr + u/r = 2 (1 - nu) C` is a small difference of large strains
(`eps_r = -1.375 p/E`, `eps_theta = 1.55 p/E`, sum `0.175 p/E` at the bore). A linear element through `[r0, r0 + h]` has `du/dr`
constant and `u/r` varying, so its strain sum varies across the element. Arithmetic (a few lines of Python, scratch, no rcfd
code), the element `[1, 1 + h]` with the exact nodal values: the strain sum at the bore over the exact is `1.413`, `1.218`,
`1.113` for `h = 0.125, 0.0625, 0.03125` (exact at the element centre, `0.633`, `0.794`, `0.891` at its outer node): an `O(h)`
error. Plane stress makes the thickness strain `eps_zz = -nu/(1 - nu) (eps_r + eps_theta)`, so the ring's top sinks by
`w(r)` with `w(1) = -1.06e-5` at `h = 0.125` against the exact `-7.50e-6` everywhere.

**With the bottom held, a varying `w(r)` couples into the bore's radial displacement.** In one Hex8 layer `u_z = w(r) z/t`, so
`gamma_rz = du_r/dz + w'(r) z/t`. `u_r` is linear in `z`, `u_r = u0(r) + u1(r) z`, and the shear energy is least for `u1 =
-w'/2`: the bore's top moves radially by `-w' t/2` relative to its bottom. With `w' > 0` near the bore (the bore sinks most) the
top of the bore lags the bottom; contact puts both on the shaft's face, so the top nodes are pushed harder: **the top pressure
is higher, the bottom lower, and the shaft is loaded linearly in `z`**. If instead the ring's `u_z` were symmetric about the
mid-plane, the shear `w' (z - t/2)/t` would have zero mean in `z` and no `u1` would be driven. Without Poisson (`nu = 0`) or
with the thickness held (plane strain), `w = 0` and the mechanism is absent. The error goes to zero with the radial mesh (`O(h)`
in `w'`), not with the mesh through the thickness, and it is linear in `d` (all of it scales with `p`). The relative slide of the
top nodes (`3.2e-5` m, `eta = 0.99936`, from the two thickness changes, ADR 0507 addendum 1) is geometric and also `O(d)`, but
its share of a face (`3e-4`) is too small to give `9e-3`.

So the variation is a **discretisation error of the coarse ring (8 linear cells through `b - a`) made visible by the
`u_z = 0` bottom constraint**, not a defect of `contact.rs`: the node-to-surface projection, the gap, the consistent node areas
and the node-pressure recovery are exact for the uniform state (probe P1, P2 below: `1e-12`). No library change is planned.

### The probe (disclosed; every number it printed)

`probe_f599.rs`: the D7 I model of ADR 0507 (penalty `100 E/(1/8)`, 1 step, tolerance `1e-6 d`, Newton 30 iterations), with
`nz` layers through `t`, the ring's radial cells `nr`, and `u_z` constraints: **Bottom** (ADR 0507: every `z = 0` node),
**Both** (`z = 0` and `z = t`, plane strain), **Minimal** (`u_z = 0` at three bottom nodes of each body: the shaft's centre,
`(0.5, 0)`, `(0, 0.5)`; the ring's outer nodes at 0, 90 and 180 degrees), **Mid** (the mid-plane nodes, `nz = 2`). Row ranges
are of `p/mean` over the bottom (`z = 0`) and top (`z = t`) contactor nodes; the split `s` = (top-row mean)/(bottom-row mean) -
1, from the midpoints of the printed ranges (approximate to `1e-6`).

| probe | set-up | bottom row | top row | split `s` | shaft `sigma_xx`/mean | other |
|---|---|---|---|---|---|---|
| P0 | ADR 0507 (nz 1, Bottom, nu 0.3, d 1e-3, nr 8) | [0.995474, 0.995718] | [1.004287, 1.004525] | 8.85e-3 | [-1.002512, -0.992976] | mean 7.525662e7; I3 with D1's reference 4.831e-3; node spread 4.526e-3; ring top `u_z` bore -9.1343e-6, outer -7.2518e-6; bore `u_x` bottom 7.373910e-4, top 7.369731e-4; `sigma_zz`/mean in shaft [-2.43e-3, 5.40e-3] |
| P1 | nu 0 | 1.000000 | 1.000000 | 0 | -0.997797 | I3 with D1's reference 1.246e-12; node spread 1.072e-11 |
| P2 | Both (plane strain) | 1.000000 | 1.000000 | 0 | -0.997797 | I3 with D1's reference 1.049e-12; node spread 6.336e-12; mean 8.271273e7; `sigma_zz`/mean -0.5987; penetration 2.670e-10 |
| P3 | nz 2, Bottom | [0.993748, 0.993885] | [1.009850, 1.010931] | 1.67e-2 | [-1.004657, -0.992056] | |
| P4 | Minimal (`u_z` free) | [0.999871, 0.999938] | [1.000062, 1.000129] | 1.9e-4 | [-0.998220, -0.997377] | ring top `u_z` bore -7.6399e-6, outer -7.2356e-6 |
| P5 | nz 2, Mid | [1.002649, 1.003183] | [1.002649, 1.003183] | 0 | [-0.999201, -0.993290] | |
| P6 | d 1e-4 | [0.995638, 0.995890] | [1.004116, 1.004361] | 8.51e-3 | [-1.003555, -0.993728] | |
| P7 | nz 4, Bottom | [0.993131, 0.993286] | [1.019081, 1.022797] | 2.79e-2 | [-1.006071, -0.991352] | |
| P8 | nr 16 | [0.998048, 0.998134] | [1.001869, 1.001952] | 3.83e-3 | [-0.999641, -0.995900] | ring top `u_z` bore -8.0924e-6 |
| P9 | nr 32 | [0.999315, 0.999334] | [1.000667, 1.000685] | 1.35e-3 | [-0.998227, -0.997354] | ring top `u_z` bore -7.6683e-6 |

A first attempt at `d = 1e-6` (tolerance `1e-12`) stopped with "not converged in 30 iterations (relative residual 2.56e-9)";
it is not used (the residual floor at that scale, ADR 0507 amendment 2's subject); `d = 1e-4` is the scaling probe.

Reading: P0 reproduces ADR 0507 (0.9955, 1.0045). P1 and P2 remove the variation entirely and make D1's reference exact to
`1e-12`. P8, P9 show the split falling with the radial cell (orders `log2(8.85/3.83) = 1.21`, `log2(3.83/1.35) = 1.50`, at
least the `O(h)` of the arithmetic). P6: `s` is `0.962` of P0's at a tenth of `d` (linear up to `4e-2`, the geometric slide's
share). P4 (u_z free) reduces it 46-fold: the variation follows the bottom constraint. P3, P7: more layers through `t` make it
larger (the constraint's boundary layer is resolved), not smaller. P5: with the mid-plane held the rows are equal, but the
symmetric barrel (edges higher than the middle) remains: the ring's `w(r)` error is still there.

## D3. I3 re-registered

The acceptance's I3 is the shaft's uniform stress. A check at the rounding level needs a set-up whose exact discrete solution is
uniform: **plane strain** (`u_z = 0` on `z = 0` and `z = t` of both bodies; a long hub on a long shaft, the interior of a
press fit), where the ring's thickness does not change, with D1's reference. The plane-stress set-up of ADR 0507 keeps a check
at the size of its discretisation error, with D1's reference.

Closed forms (plane strain: `E -> E/(1 - nu^2)`, `nu -> nu/(1 - nu)` in the plane-stress forms; the plane-stress pressure does
not contain `nu`): `p_pe = E d (b^2 - a^2)/(2 a b^2 (1 - nu^2)) = 7.5e7/0.91 = 8.241758e7 Pa`; the ring's hoop stress
`sigma_theta(r) = p a^2 (1 + b^2/r^2)/(b^2 - a^2)` (independent of the elastic constants); the shaft `sigma_r = sigma_theta =
-p`, `sigma_zz = nu (sigma_r + sigma_theta) = -2 nu p`. In the discrete model `p` in the shaft is `p_s = mean (1 - d/a)
cos(Delta/2)` (D1).

## D4. Tests (one new file, `crates/rcfd-fem/tests/sa_contact_fit_f599.rs`)

A parameterised copy of ADR 0507's `fit_model` (the probe's builder: `nz`, `nr`, the `u_z` set, `nu`, `d`); ADR 0507's
`Checks` and `worst` (every aggregate finite, b5). `sa_contact.rs` is not edited; ADR 0507's
`i1_to_i4_..._a3_known_miss` stays the ignored record of I3's original (b1). One solve per set-up, about a second each in
release.

## Checks (limits fixed now)

Relative errors are `|x/x_ref - 1|`; a non-finite value is a miss.

| id | clause | quantity | limit | estimate / probe |
|---|---|---|---|---|
| DG1 | a1 | ADR 0507's set-up: the bottom-row mean `p/mean` and the top-row mean | in `[0.9950, 0.9960]` and `[1.0040, 1.0050]` | P0: 0.995596, 1.004406 |
| DG2 | a1 | the same with `nu = 0`: every node pressure against the mean; the shaft against D1's reference (`sigma_xx, sigma_yy` against `-p_s`, the other four components against 0, over `p_s`) | `<= 1e-6`, `<= 1e-6` | P1: 1.1e-11, 1.2e-12 |
| DG3 | a1 | the split `s` at `nr = 8, 16, 32`: decreasing, `s_32 <= 2e-3`, both orders `log2(s_h/s_{h/2}) >= 1.0` | as stated | D2 `O(h)`; P0, P8, P9: 8.85e-3, 3.83e-3, 1.35e-3, orders 1.21, 1.50 |
| DG4 | a1 | linearity in `d`: `|s(d = 1e-4)/s(d = 1e-3) - 1|` | `<= 0.1` | P6: 3.8e-2 |
| DG5 | a1 | `u_z` free (Minimal): `|s|` | `<= 1e-3` | P4: 1.9e-4 |
| DG6 | a1 | two layers, bottom held: `s` | `>= 5e-3` | P3: 1.67e-2 |
| DG7 | a1 | two layers, mid-plane held: at every angle `|p_top/p_bottom - 1|` | `<= 1e-6` | P5: equal to 6 digits; mirror symmetry |
| I3pe | a2 | plane strain (one layer, `u_z = 0` on both faces, else D7 I of ADR 0507): every shaft integration point, `sigma_xx, sigma_yy` against `-p_s`, `sigma_zz` against `-2 nu p_s`, `sigma_xy, sigma_yz, sigma_xz` against 0, over `p_s` | `<= 1e-6` | rounding; P2: 1.05e-12 (in-plane components) |
| I3pe-n | a2 | plane strain: every node pressure against the mean | `<= 1e-6` | P2: 6.3e-12 |
| I1pe | a2 | plane strain: the mean against `p_pe`, and every node against `p_pe` | `<= 2e-2` each | P2: 3.58e-3 (mean); ADR 0507 arithmetic 4 |
| I2pe | a2 | plane strain: every ring element's centre hoop stress against Lame's at its radius with the mean (ADR 0507 I2's measure) | `<= 2e-2` | ADR 0507 I2 3.6e-3 (the same formula) |
| I4pe | a2 | plane strain: the penetration | `<= 1e-9` | P2: 2.67e-10 |
| I3ps | a2 | ADR 0507's plane-stress set-up, D1's reference: every shaft integration point as I3pe but `sigma_zz` against 0 | `<= 1e-2` | P0: 4.83e-3 (half the split plus the variation around) |
| I3ps-r | a2 | the same set-up: within the bottom row and within the top row, every node against its row mean | `<= 1e-3` | P0: 2.4e-4 |

Negative controls:

| id | what | expected |
|---|---|---|
| NI3 | I3pe's measure with ADR 0507's reference `-mean` in place of `-p_s` (and `sigma_zz` against `-2 nu mean`) | `>= 1e-3` (D1: `2.20e-3`) |
| NPS | I3pe's measure (limit `1e-6`) on the plane-stress set-up of I3ps | `> 1e-6` (P0: 4.83e-3) |

Planned mutations (a `git archive` copy of the tip, its own `CARGO_TARGET_DIR`, running the new test file):

| id | mutation | expected to be caught by |
|---|---|---|
| MT1 | test: I3pe's set-up without the `z = t` constraint | I3pe |
| ML1 | `contact.rs`: the Uzawa update left out (`update` keeps the multipliers) | the solve fails (augmentations exhausted: the pure-penalty penetration `p/eps = 4.7e-7 > 1e-9`) or I4pe |
| ML2 | `contact.rs`: `project` returns the unnormalised normal `a1 x a2` | I1pe or I3pe-n (or the solve fails) |
| ML3 | `contact.rs`: the curvature part's sign flipped in `force_and_tangent` | recorded, not required: the tangent only changes the path, and the converged state may meet every check (T1 of ADR 0507 is the tangent's check) |

## Decision rule

- **done**: DG1 to DG7, I3pe, I3pe-n, I1pe, I2pe, I4pe, I3ps and I3ps-r met; NI3 and NPS as expected; MT1, ML1 and ML2 caught;
  `contact.rs` unchanged (or, if a defect is found, fixed behind a default-bits test with ADR 0507's other results kept); the
  existing `rcfd-fem` tests passing; `baseline_hashes` passing; fmt and clippy clean.
- **partial**: DG2, DG3 and DG5 met (the diagnosis holds) and any other miss diagnosed and recorded as a `*_known_miss` test.
- **todo**: otherwise.

## Limits (not covered)

- No change of the contact formulation: the diagnosis finds no defect in `contact.rs`, so a remedy for plane-stress
  press fits is a mesh rule (radial cells at the bore) or a better element (incompatible modes or a quadratic ring), not
  contact code; the thickness `t` and the radial grading are not studied beyond D2.
- Friction (a stuck interface would turn the slide into shear stress) is outside F-427 and F-599.
- A real hub-shaft fit of the 120 mm fan is a finite-length hub on a longer shaft, neither plane stress nor plane strain; the
  end effects (the edge pressure peaks of a press fit) need a mesh refined at the hub's faces and are not checked here.
- No case-file key (b2).

## Addendum 1: results (2026-11-14)

The registration was committed alone (2fa3d8ee); the test file `crates/rcfd-fem/tests/sa_contact_fit_f599.rs` (b541a012) was
run once in release (`cargo test -p rcfd-fem --release --test sa_contact_fit_f599`, 7 tests, 13 s); adda693c is a type alias
in that file after the run, for clippy (no number changes). No library file was changed: `contact.rs` and every other source
file are as at the base d41012db.

| check | limit | measured | met |
|---|---|---|---|
| DG1 bottom-row mean, top-row mean (p/mean) | `[0.9950, 0.9960]`, `[1.0040, 1.0050]` | `0.995572`, `1.004428` | yes |
| DG2 `nu = 0`: node pressures, shaft against D1's reference | `<= 1e-6`, `<= 1e-6` | `1.07e-11`, `1.25e-12` | yes |
| DG3 splits at `nr = 8, 16, 32`; decreasing; `s_32`; orders | decreasing, `<= 2e-3`, `>= 1.0` | `8.895e-3`, `3.843e-3`, `1.356e-3`; orders `1.211`, `1.503` | yes |
| DG4 `abs(s(1e-4)/s(1e-3) - 1)` | `<= 0.1` | `3.80e-2` (`s(1e-4) = 8.558e-3`) | yes |
| DG5 `u_z` free: `abs(s)` | `<= 1e-3` | `1.93e-4` | yes |
| DG6 two layers, bottom held: `s` | `>= 5e-3` | `1.680e-2` | yes |
| DG7 two layers, mid-plane held: `abs(p_top/p_bottom - 1)` | `<= 1e-6` | `1.50e-11` | yes |
| **I3pe** plane strain: shaft against `-p_s`, `-2 nu p_s`, 0 (all six components) | `<= 1e-6` | **`1.05e-12`** | yes |
| I3pe-n plane strain: node pressures against the mean | `<= 1e-6` | `6.34e-12` | yes |
| I1pe mean and every node against `p_pe = 8.241758e7` | `<= 2e-2` each | `3.58e-3`, `3.58e-3` (mean `8.271273e7`) | yes |
| I2pe ring hoop against Lame | `<= 2e-2` | `3.73e-3` | yes |
| I4pe penetration | `<= 1e-9` | `2.67e-10` | yes |
| I3ps plane stress (ADR 0507's set-up): shaft against D1's reference | `<= 1e-2` | `5.41e-3` | yes |
| I3ps-r around the bottom row, the top row | `<= 1e-3` | `1.47e-4`, `1.40e-4` | yes |
| NI3 I3pe with ADR 0507's reference `-mean` | `>= 1e-3` | `2.20e-3` | as expected |
| NPS I3pe's measure on the plane-stress set-up | `> 1e-6` | `5.41e-3` | as expected |

I3ps measured `5.41e-3` against the probe's `4.83e-3` because the probe's measure had the in-plane components only and the
test's has all six (the plane-stress shaft carries `sigma_zz` up to `5.4e-3` of the mean, P0). The probe's other numbers are
reproduced to the printed digits.

Mutations (a `git archive` copy of b541a012 in `f599-scratch/mut` with its own `CARGO_TARGET_DIR`, the new test file run per
mutation by `f599-scratch/mutate.sh`, logs `MT1.log` to `ML3.log`):

| id | mutation | caught by |
|---|---|---|
| MT1 | I3pe's set-up without the `z = t` constraint | I3pe (`0.605`, from `-2 nu p_s` in `zz`), I3pe-n (`4.53e-3`), I1pe (`8.7e-2`) |
| ML1 | no Uzawa update | every test: the solve fails ("augmentation 1: not converged in 30 iterations", energy ratio `0.89`) |
| ML2 | unnormalised normal | every test: the solve fails (gap tolerance not met after 20 augmentations, gap `1.88e-6`) |
| ML3 | curvature sign flipped | DG5 (the active set cycled 4 times, a located error); the other six tests met (registered as not required) |

Gates: `cargo test -p rcfd-fem --release` passes (exit 0; 823 passed, 0 failed, 223 ignored over 184 test binaries, the new file
included); `cargo test -p rcfd-api --release --test baseline_hashes` passes (3 passed, 1 ignored); `cargo clippy -p rcfd-fem
--all-targets -- -D warnings` is clean (after adda693c); `cargo fmt -p rcfd-fem --check` is clean; `cargo test -p rcfd-cli
--release --test docs_site` passes (2 tests).

### The diagnosis

The 0.9955 and 1.0045 of ADR 0507 are a **discretisation error of the ring, made visible by the `u_z = 0` bottom constraint;
they do not come from a defect of the contact code**. The ring's 8 linear cells through `b - a` do not reproduce Lame's constant
strain sum `eps_r + eps_theta` near the bore (`1.41` of the exact at the bore node, `O(h)`). In plane stress the thickness change
`w(r) = -nu/(1 - nu) (eps_r + eps_theta) t` then varies (`-9.13e-6` at the bore against `-7.5e-6`). Its `r`-gradient drives a
transverse shear which, with the bottom held, is relieved by a `z`-linear radial displacement of the bore (top `7.3697e-4`,
bottom `7.3739e-4`), and contact pushes the lagging top harder. Evidence: without Poisson (DG2) or with the thickness held
(I3pe) the pressures are uniform to `1e-11` and the shaft to `1e-12`; the split falls with the radial cell at orders 1.2 and 1.5
(DG3), is linear in `d` (DG4: the geometric slide is not the cause), nearly vanishes with `u_z` free (DG5), and does not fall
with layers through the thickness (DG6). The node-to-surface asymmetry, the consistent node areas and the node-pressure
recovery are exact for the uniform state (DG2, I3pe).

**The I3 expectation was also wrong in its reference.** A node pressure is a force over the ring's chord area at `a - d`, which
the shaft carries over its polygon side, so the shaft's uniform stress is `mean (1 - d/a) cos(Delta/2) = 0.997797 mean`, not
`mean` (NI3: `2.20e-3`). No correct code could have met ADR 0507's I3 (`1e-6`, "exact up to rounding") in its set-up. Its
original stays the ignored record `i1_to_i4_interference_fit_ring_on_a_solid_shaft_a3_known_miss` in `sa_contact.rs` (not
edited). I3 is re-registered as I3pe (plane strain, D1's reference, `1e-6`, met at `1.05e-12`) with I3ps (ADR 0507's
plane-stress set-up, D1's reference, `1e-2`, met at `5.41e-3`).

Deviations: none from the registration. Not run: nothing registered was skipped.

**Decision by the rule: done.** Every check met, both negative controls as expected, MT1, ML1 and ML2 caught, ML3 recorded,
no library change, and the gates as listed.

Remaining questions, with recommendations:

1. **Plane-stress press fits in practice.** The one-layer ring needs fine radial cells at the bore for a pressure that is
   uniform through the thickness (split `1.4e-3` at 32 cells). Options: (a) a bore-cell rule in the fan documentation; (b)
   incompatible-mode or quadratic ring elements; (c) leave as is. Recommendation: (a), through the follow-up below; (b) belongs
   to the element roadmap.
2. **A finite hub on a longer shaft** (the fan's real case, with edge pressure peaks). Recommendation: the follow-up below.

## For the lead: roadmap entries

> F-599 status done. Note: Structural solver, advanced tier. Follow-up of F-427 (ADR 0507, 2026-11-14). ADR 0513, 2026-11-14. The 0.9955 and 1.0045 node pressures of the one-layer ring-on-shaft fit of ADR 0507 are a discretisation error of the ring made visible by the u_z = 0 bottom constraint; contact.rs has no defect. The 8 linear cells through b - a miss Lame's constant strain sum near the bore (1.41 of it at the bore node, O(h)), so the plane-stress thickness change varies (-9.13e-6 against -7.5e-6 at the bore); its gradient, with the bottom held, tilts the bore and loads its top harder. Without Poisson or in plane strain the pressures are uniform to 1e-11 and the shaft to 1e-12. The top/bottom split falls with the radial cell (8.9e-3, 3.8e-3, 1.4e-3 at 8, 16, 32 cells), is linear in d, nearly vanishes with u_z free (1.9e-4) and grows with layers through the thickness. The reference of ADR 0507's I3 was also wrong: the shaft carries mean (1 - d/a) cos(Delta/2) = 0.9978 of the mean node pressure. I3 re-registered: in plane strain the shaft is uniform within 1e-6 (1.05e-12), the pressure is within 3.6e-3 of E d (b^2 - a^2)/(2 a b^2 (1 - nu^2)) and the hoop stress within 3.7e-3; ADR 0507's plane-stress set-up with the corrected reference is within 1e-2 (5.4e-3). The original I3 stays an ignored record. Three of three required mutations caught. No library change, no case-file key.

> F-427 note: append "The I3 miss is explained in ADR 0513 (F-599): a ring discretisation error shown by the bottom constraint, and a reference that should have been mean (1 - d/a) cos(Delta/2); I3 re-registered there and met (plane strain, 1.05e-12)." and replace "(7.0e-3 against 1e-6, a through-thickness variation not yet explained)" with "(7.0e-3 against 1e-6, explained and re-registered by F-599)".

> Follow-up (proposed, not added): press fit of a finite hub on a longer shaft (deps F-599). Every limit not stated here is registered in the ADR before the run. A hub of length L on a shaft that extends beyond both of its faces, same material, radial interference d, frictionless: the contact pressure at mid-length against the plane-strain closed form within 2 percent for L >= 4 (b - a), the edge pressure peak located at the hub faces and converging under refinement of the bore cells, and a bore-cell rule for the 120 mm fan hub stated from the run. Fan relevance: hub-shaft press-fit stresses.
