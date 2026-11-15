# ADR 0588: F-663 registration: distributed loads at a height on warping beams (the consistent load-height term on the Hermite twist, the uniformly distributed load on the top flange, at the shear centre and on the bottom flange)

Status: **REGISTERED 2026-11-15, written before any code of F-663 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-663 was written, compiled or run in the repository: the files listed under "What was
read" were read, and the arithmetic below was done by hand and evaluated with one plain numpy script in the scratch folder
(`w49i-work/f663-scratch/arith.py`, log `arith.log`: the section and helper functions re-typed from the ADR 0582 script, a
Rayleigh-Ritz sine-series solution of the energy of D3 for every load case below, its convergence in the number of terms, the
cubic-Hermite element block of D1 against its closed form, and an independent cubic-Hermite Euler-Bernoulli Vlasov model of the
same energy in three variants; no rcfd code; disclosed, its numbers quoted below). The bit record B1 of the paths without the
new entry is a test file written and run on the base after this commit and before any library code (D5). Results go in the
addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-663 (deps F-643), structural solver, advanced tier, the follow-up named by
[ADR 0582](0582-f643-registration-load-height-and-non-uniform-moment-for-monosymmetric-warping-beams.md) (its question Q2,
recommendation (b): a consistent distributed load-height term on the Hermite twist). The element, sections and buckling path are
those of [ADR 0546](0546-f598-registration-warping-degree-of-freedom-for-beam2-and-i-section-lateral-torsional-buckling.md), ADR 0548 and
[ADR 0553](0553-f627-registration-monosymmetric-thin-walled-sections-for-the-warping-element.md); the point-load term is that of
ADR 0582 D1; the pre-stress on the harmonic path is that of [ADR 0585](0585-f661-registration-pre-stress-on-the-warping-harmonic-path.md).
Number 0588 given by the lead. Branch `f663-work` from 83b9d18c (the F-661 branch, which carries F-635, F-643 and F-661),
worktree `w49i-work/f663`, scratch `w49i-work/f663-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test edited;
no new dependency, no unsafe code, the crate graph unchanged; clean-room. Every choice below that the maintainer would normally
be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation), except the open
questions at the end.

## What F-663 asks

The acceptance, binding, verbatim (`docs/roadmap.json` of the main checkout; the item is not yet in the branch's copy):

> Every limit not stated here is registered in the ADR before the run. The lateral-torsional buckling load of a simply supported
> doubly symmetric I under a uniformly distributed load on the top flange, at the shear centre and on the bottom flange equals a
> converged Rayleigh-Ritz reference derived in the ADR within 2 percent at 16 elements; results without a distributed load height
> unchanged.

The brief adds: the distributed term is the consistent `(1/2) int q (r . e) phi^2 dx` on the Hermite twist interpolation, to be
derived; references derived here; opt-in only; existing results bit-identical, the F-643 records included.

Clauses: **a0** every limit registered here; **a1** doubly symmetric I, uniformly distributed load on the top flange, within
`2e-2` of the reference at 16 elements; **a2** the same at the shear centre; **a3** on the bottom flange; **a4** results without
a distributed load height unchanged (bit for bit, the F-643 point-load results included).

### Readings, registered

- **"Simply supported"**: the fork supports of ADR 0546, 0553 and 0582 (`v = w = phi = 0` at both ends, `u = 0` at node 0,
  warping free), `L = 3`, 16 elements along `x`, steel `E = 2e11`, `nu = 0.3`, shear factor `5/6`.
- **"Doubly symmetric I"**: `doubly_symmetric_i(0.1, 0.01, 0.18, 0.006)` of ADR 0546 and 0582, flange mid-planes at `z = +/-
  0.095`.
- **"Uniformly distributed load"**: `Load::line_load("all", [0, 0, -q_ref])`, `q_ref = 1000` N/m downward on every element (the
  consistent nodal loads of ADR 0422), the reference load of the pre-stress; the buckling load is `lambda q_ref`.
- **"On the top flange, at the shear centre, on the bottom flange"**: the line of application on the symmetry axis at `z =
  +0.095` (top flange mid-plane, above the shear centre, destabilising), at the shear centre (`z = 0`, no entry: the default; and
  an entry at `(0, 0)`, L0) and at `z = -0.095`.
- **"A converged Rayleigh-Ritz reference derived in the ADR"**: the sine-series solution of D3 (D4), converged in the number of
  terms.
- **"Results without a distributed load height unchanged"**: every path without the new entry runs the arithmetic of ADR 0546,
  0548, 0553, 0574, 0582 and 0585 operation for operation; their test files and bit records pass unchanged, and the new bit record
  B1 (D5), recorded on the base, holds the F-643 point-load factors and matrices and the factors of the UDL without an entry.
- **"Opt-in"**: a new `Warping::line_load_point` entry (D2); a `Warping` without it is unchanged. No case-file key (F-625).

## What was read (no run)

- `AGENT_RULES.md`; the brief; ADR 0582 (whole), ADR 0585 (head, D5 bit-record pattern), ADR 0553 through ADR 0582.
- `crates/rcfd-fem/src/beam_warping.rs` (the module doc, `Warping`, `hermite`, `twist_block`, `add_twist`, `rows14`,
  `local_geometric_stiffness`, `to_global`, `offset_transform`, `section_local_geometric_stiffness`, `matrices_impl`,
  `add_load_points`, `hermite_mass_block`, `solve_static`, `WarpingPrestress::from_static`, `WarpingHarmonic::with_prestress`).
- `crates/rcfd-fem/src/beam.rs` (`line_load_vector`, `element_line_load`, `local_end_forces`, `end_resultants`, the section
  forces of ADR 0422 D4), `geometric.rs` (`with_beam_resultants`), `geometric_struct.rs` (`beam_end_forces`), `loads.rs`
  (`nodal_load_vector`, the line load), `model.rs` (`Load::LineLoad`).
- Tests: `sa_beam_warping_f643.rs` (the set-ups), `sa_beam_warping_f661_bits.rs` (the digest); the ADR 0582 script
  `f643-scratch/arith.py`.

## Design

### D1. The distributed load-height term (first principles)

A dead force per unit length `q` (constant in magnitude and direction) acts along the line of section points `p = (p_y, p_z)`
(section axes, from the centroid) over a warping element. The section at `x` turns rigidly by `phi(x)` about the axis through the
shear centre `s = (y_0, z_0)`. ADR 0582 D1 derived, for a force `F` at `p`, the second-order potential `(1/2) (F . r) phi^2`, `r
= p - s` (the second-order displacement of the point is `-(phi^2/2) r`, its work `-(phi^2/2) F . r`). The force on `dx` is `q
dx`; the potentials add, so the second-order potential of the distributed load is

`Pi_q = (1/2) int (q . r) phi(x)^2 dx`, `q . r = q_y (p_y - y_0) + q_z (p_z - z_0)`, `q_y = e_2 . q`, `q_z = e_3 . q`,

the brief's `(1/2) int q (r . e) phi^2 dx` with `e` the direction of `q`. On a warping element the twist is the cubic Hermite
`phi(x) = sum_i H_i(x) t_i` of `t = (phi_0, chi_0, phi_1, chi_1)` (`chi = phi'`, the functions of `hermite`), so the consistent
form is `Pi_q = (1/2) t^T (c M_H) t`, `c = q . r` constant over the element, `M_H[i][j] = int_0^l H_i H_j dx`. That integral is
the cubic-Hermite block that `hermite_mass_block` already evaluates (four-point Gauss, exact for the degree 6). In closed form
(direct integration of the four cubics; the script checks the library's quadrature order against it, arithmetic 1):

`M_H = (l/420) [[156, 22 l, 54, -13 l], [22 l, 4 l^2, 13 l, -3 l^2], [54, 13 l, 156, -22 l], [-13 l, -3 l^2, -22 l, 4 l^2]]`,

whose `phi`-`phi` entries sum to `l` (a rigid twist `phi = 1` gives `int c phi^2 = c l`: the total of the lumped point terms of
ADR 0582 for the force `q l`). The term is linear in `q`, so it scales with the buckling factor: a term of `K_sigma`. In the
14-unknown local order it is `c M_H` on the twist unknowns `TWIST = [3, 6, 10, 13]` (`add_twist`), turned to global axes by
`to_global` (the `phi` rows go to the rotation equations through `e_1`, the `chi` rows to the warping equations).

**Shear-centre unknowns.** `phi` is the same in the centroid and in the shear-centre unknowns (`O` of ADR 0553 D1 leaves `phi`
alone and the term has no `v`, `w` entries; `O^T A O = A` for an `A` on the twist only), so the term needs no offset transform;
`s` enters only through `c`.

**First-order torque and the other second-order terms** are as ADR 0582 D1: the model's line load acts at the centroid, so an
entry with a first-order torque about the centroid, `p_y q_z - p_z q_y != 0`, is refused; the in-plane second-order terms
(`theta_y^2`) are not added (Limits). For a vertical load on the symmetry axis both vanish.

**Lumped against consistent.** Lumping `q l` into point terms at the nodes (the route ADR 0582 left to the user) puts `c l` on
`phi_i^2` and nothing on `chi`; the consistent block also couples `phi` with `chi` and the neighbours. Both converge to `Pi_q` at
`O(l^2)`; the difference is a check (A8) and a negative control.

### D2. API (in `rcfd_fem::beam_warping`)

`Warping::line_load_point(self, element_set: &str, force: Vec3, point: [f64; 2]) -> Warping`: the reference load of the
pre-stress holds the force per unit length `force` (global components, N/m) on every element of the set, applied along `point =
(p_y, p_z)` (m, section axes, from the centroid). Its only effect is `c M_H` of D1 per element in `K_sigma` of [`matrices`] (so in
`buckling_factors`, `buckling_factors_with`, `prestressed_modes` and the pre-stressed harmonic path of ADR 0585), `s` the shear
centre of that element's section, `e_1`, `e_2`, `e_3` its frame. Accessor `line_load_points()`. The force is restated, as for
`load_point` (ADR 0582 Q1, recommendation (a)); it must be the line load of the reference load (documented, not checked).
Refused in `matrices` (and through it in every caller), in this order per entry:

- a non-finite force or point component (key `"load_point"`);
- an unknown element set (`UnknownSet`, kind element);
- an element of the set that is not a warping element (key `"warping"`);
- a nonzero first-order torque about the centroid, `|p_y q_z - p_z q_y| > 1e-9 |q| |p|` (`TORQUE_TOLERANCE`, key
  `"load_point"`).

Elements of different sections in one set are accepted: the shear centre is per element (no ambiguity, unlike a node). The terms
are pushed as triplets after those of `add_load_points` at the end of `matrices_impl`; with no entry the new function returns
before pushing anything, so every matrix and result without an entry is bit-identical.

### D3. The pre-stress under a distributed load, and the reference energy

**What the element sees.** `with_beam_resultants` and `WarpingPrestress::from_static` take the end forces `K u` of each element,
without subtracting the element's consistent load (unlike the section forces of ADR 0422 D4). For a uniform `q_z` the consistent
load is `q_z l/2` at each node and `(l^2/12) e_1 x q` (`-q_z l^2/12` about `e_2`) at node 0, the opposite at node 1
(`line_load_vector`), and `K u` equals the true end forces plus it. With `end_resultants` (`r_0 = -f_0`, `r_1 = f_1`) both end
moments `M_y` are shifted by `+q_z l^2/12` and both end shears become the mid-element value. Under `M_y = int z sigma dA` (the
end resultant on the positive face) the simply supported moment is `M(x) = (q_z/2) x (L - x)`, `M'' = -q_z`, and the mean of the
chord minus the parabola over an element is `-q_z l^2/12`: the element's linear moment is the chord shifted by the mean bump,
which is the `L^2` projection of the parabola onto linear functions (the bump is symmetric about the element centre), and its
shear is the element mean. This is the library's behaviour, not changed here; the derivation is checked by E1 and its effect on
the factor is estimated by the "lib" variant of the Hermite model (arithmetic 4).

**The reference energy** (ADR 0582 D2 with the distributed term in place of the point term), shear-centre unknowns,
Euler-Bernoulli, forks:

`Pi_2 = (1/2) int (E I_z v''^2 + G J phi'^2 + E C_w phi''^2) dx + lambda [ int M phi v'' dx + (1/2) int beta_x M phi'^2 dx + (1/2) int (q . r) phi^2 dx ]`,

`M(x) = (q_z/2) x (L - x)`, `beta_x` the Wagner coefficient about the shear centre (zero for the doubly symmetric I).

### D4. The references (Rayleigh-Ritz, derived; arithmetic 3)

`v = sum a_n sin(n pi x/L)`, `phi = sum b_n sin(n pi x/L)`, `n = 1 .. N` (every `n`; each meets the fork conditions). The
stiffness is diagonal; the geometric matrix holds `int M sin_m (-k_n^2 sin_n) dx`, `beta_x int M k_m k_n cos_m cos_n dx` and
`(q . r) int sin_m sin_n dx`, the integrals by Gauss-Legendre with 400 points over `[0, L]` (the moment is smooth; 800 points
change every result by `<= 2.2e-14`). The lowest positive `lambda` of `(K + lambda G) x = 0`. Converged in `N`: `N = 5, 11, 21,
41, 81`; the change from 41 to 81 is `<= 2.4e-8` for every case; the cubic-Hermite E-B model of the same energy with the exact
moment agrees with `N = 81` within `1.3e-5` at 16 and `8.2e-7` at 32 elements (`O(l^4)`): the reference is the `N = 81` value,
uncertain by about `1e-8`. `q_ref = 1000` N/m, `q_z = -q_ref` (downward) unless stated.

| case | `c = q . r` (N) | `lambda_ref` (`N = 81`) | N 41 vs 81 | Hermite E-B 16 el., exact / lib / lib lumped |
|---|---|---|---|---|
| DS-top: top flange (`p_z = +0.095`) | `-95` | **`45.1856414209`** | `3.5e-9` | `4.4e-6` / `3.1e-6` / `2.0e-6` |
| DS-SC: at the shear centre | `0` | **`59.0640165887`** | `4.7e-9` | `5.9e-6` / `4.0e-6` / `4.0e-6` |
| DS-bot: bottom flange (`p_z = -0.095`) | `+95` | **`77.1475881351`** | `5.7e-9` | `7.4e-6` / `5.0e-6` / `6.1e-6` |
| MS-SC-down: monosymmetric (ADR 0553), at the shear centre, downward (flange 1 compressed) | `0` | **`135.747863965`** | `2.1e-8` | `1.2e-5` / `9.6e-6` / `9.6e-6` |
| MS-SC-up: the same, upward (`q_z = +q_ref`, flange 1 in tension) | `0` | **`69.3139026678`** | `6.0e-9` | `9.0e-6` / `7.7e-6` / `7.7e-6` |
| MS-C-down: at the centroid (`p = (0, 0)`, `r_z = -z_0`), downward | `+52.580303` | **`163.396623703`** | `2.4e-8` | `1.3e-5` / `1.0e-5` / `1.1e-5` |
| MS-top-down: on flange 1 (`p_z = 0.0718681318681`), downward | `-19.287828` | **`126.453805543`** | `2.0e-8` | `1.2e-5` / `9.2e-6` / `8.9e-6` |

Records (not limits): the doubly symmetric moment factor `C_1 = (lambda q L^2/8)/M_cr = 6.644702e4/5.875035e4 = 1.1310`, near
the remembered textbook `1.13` (from memory, not used); top/SC `0.7650`, bottom/SC `1.3062`; monosymmetric down/up `1.9585`; the
monosymmetric I at the shear centre downward with `beta_x = 0`: `98.619`.

### D5. The bit record B1 (recorded on the base)

A test file `tests/sa_beam_warping_f663_bits.rs` that uses only the API of the base 83b9d18c, written and run once after this
commit and before any library line, its printed digests (FNV-1a 64 over the count and the `to_bits` of each value, the digest of
ADR 0585) becoming its constants. It holds, each at 16 elements with the default `BucklingOptions::new(3)`: (1) the factors of
the ADR 0582 DS-top point load (`load_point` at `(0, 0.095)`); (2) the same at 8 elements; (3) the factors of the doubly
symmetric UDL without an entry; (4) of the monosymmetric UDL downward and (5) upward without an entry; (6) `K` and `K_sigma`
values of `matrices` of the DS-top point-load model; (7) `K_sigma` of `matrices` of the UDL model; (8) the factors of
`buckling_factors_with` of the warping static solve under the UDL. After the change it runs unchanged and compares.

## Arithmetic done before the code (disclosed; `arith.log`)

1. `hermite_mass_block`'s order: the Hermite block by six-point Gauss against the closed form of D1 at `l = 3/16`: `2.0e-16`
   relative; the `phi`-`phi` entries sum to `l` exactly. Entries at `l = 3/16`: `M_H[0][0] = 0.0696428571`, `M_H[0][2] =
   0.0241071429`, `M_H[0][1] = 0.00184151786`, `M_H[0][3] = -0.00108816964`, `M_H[1][1] = 6.27790179e-5`, `M_H[1][3] =
   -4.70842634e-5`.
2. The ADR 0582 DS-top point-load reference through the re-typed functions: `76.65319068242` (registered `76.653190682`).
3. The table of D4.
4. Discretisation and shear flexibility. The Hermite E-B model at 16 elements: `<= 1.3e-5` with the exact moment, `<= 1.0e-5`
   with the library's per-element moment of D3 ("lib"), and lumped against consistent height terms `1.1e-6` (DS-top), `1.1e-6`
   (DS-bot), `7.6e-7`, `3.1e-7` (MS); 8 elements `<= 1.9e-4`. The rcfd element has a shear-flexible lateral plane, `E I_z
   k^2/(kappa G A) = 1.86e-3` (doubly symmetric), `3.5e-3` (monosymmetric); ADR 0582 measured `-3.5e-4` to `-1.7e-3` for the
   central load. **Estimate `3e-3` for every LTB case at 16 and 8 elements.**

## Checks (limits fixed now)

Every aggregated error is asserted finite before it is compared (a non-finite value is a miss). Set-up of every LTB solve: the
fork model of the readings (`fork` of `sa_beam_warping_f643.rs`, re-typed); the reference load `Load::line_load("all", q)` on a
copy of the model; the six-unknown static solve and `Prestress::from_static(..).with_beam_resultants(.., [0, 0])`;
`buckling_factors(&unloaded, &pre, &warping, &BucklingOptions::new(3))`; the warping `Warping::new().block(0, ws)` plus, where
the case has a height, `.line_load_point("all", q, p)`. The factor is `factors[0]`.

| id | check | limit | estimate |
|---|---|---|---|
| B0 | `sa_beam_warping_f598.rs`, `_f598_bits`, `_f626`, `_f626_bits`, `_f627`, `_f635`, `_f635_bits`, `_f643`, `_f661`, `_f661_bits`, `sa_buckling_sign_asymmetric_f642.rs`, `sa_buckling_restart_f650.rs`, `sa_geometric_struct_f558_bits.rs` pass unchanged after the change; `cargo test -p rcfd-api --release --test baseline_hashes` passes | pass | pass |
| B1 | **acceptance a4**: the bit record of D5 after the change | bit-equal | bit-equal |
| A1 | **acceptance a1**: DS-top, `|lambda/45.1856414209 - 1|` | `<= 2e-2` | `3e-3` |
| A2 | **acceptance a2**: DS-SC (no entry), `|lambda/59.0640165887 - 1|` | `<= 2e-2` | `3e-3` |
| A3 | **acceptance a3**: DS-bot, `|lambda/77.1475881351 - 1|` | `<= 2e-2` | `3e-3` |
| A4 | the offset path under a UDL: MS-SC-down and MS-SC-up (no entry), MS-C-down (point `(0, 0)`), MS-top-down (point `(0, 0.0718681318681)`), each against D4 | `<= 2e-2` each | `3e-3` |
| A5 | A1 to A3 at 8 elements against the same references | `<= 2e-2` each | `3e-3` |
| A6 | A1 with the beam along global `y` (orientation `z`, the permuted supports of `sa_beam_warping_f643.rs`, the load along `-e_3` of that frame) against axis `x` | `<= 1e-6` relative | `1e-12` |
| A7 | A1 and MS-C-down with the pre-stress of the warping static solve (`solve_static` with the line load, `WarpingPrestress::from_static(.., [0, 0])`, `buckling_factors_with`) against the six-unknown pre-stress | `<= 1e-9` relative | `1e-14` |
| A8 | lumped against consistent: DS-top and DS-bot with the height as ADR 0582 point entries (`load_point` of `q l` at nodes 1 to 15 and `q l/2` at nodes 0 and 16, at the same point) against A1 and A3: `d = |lambda_lumped/lambda - 1|` | `1e-8 < d <= 1e-4` each | `1.1e-6` |
| E1 | D3 on the pre-stress of A1 (`Prestress::beam_terms(e)`, public): for every element, with `r_0 = -f[0..6]`, `r_1 = f[6..12]` of its end forces `f`, `r_0[2] = r_1[2] = V(x_m)`, `r_0[4] = M(x_0) + q_z l^2/12`, `r_1[4] = M(x_1) + q_z l^2/12` | `<= 1e-9` of `max |M|` | `1e-12` |
| L0 | an entry at the shear centre adds nothing: DS-SC with `.line_load_point("all", q, (0, 0))` and MS-SC-down with `(0, z_0)`, against no entry: factors bit-equal | bit-equal | bit-equal |
| L1 | element of the term: `matrices` of the DS-top model with and without the entry (the same pre-stress): `stiffness` bit-equal; `geometric` with the same pattern and equal (`==`) outside the twist equations (rotation-x and warping) of the nodes; on them the difference equals `c` times the 16 assembled blocks `M_H` of D1 with `c = -95` | equal; `<= 1e-12` of `max |c M_H|` | `1e-16` |
| L2 | the per-element shear centre: a beam of two warping blocks (elements 0 to 7 the monosymmetric section, 8 to 15 the doubly symmetric one), one entry on all at `(0, 0.05)`, the downward `q`: the rigid-twist energy `u^T (K_sigma(entry) - K_sigma(none)) u`, `u` the unit rotation about `e_1` at every node (warping zero), equals `8 l c_MS + 8 l c_DS`, `c_MS = -q_ref (0.05 - z_0)`, `c_DS = -q_ref 0.05` | `<= 1e-12` relative | `1e-15` |
| N1 | negative controls on measured factors: DS-top/DS-SC `< 0.85` and DS-bot/DS-SC `> 1.2` (reference `0.765`, `1.306`); MS-SC-down/MS-SC-up `> 1.7` (`1.958`); MS-SC-down against the `beta_x = 0` reference `98.619` more than `0.2` away (`0.376`) | as stated | as stated |
| R | refusals, by `buckling_factors`: an unknown element set (`UnknownSet`); an element without a warping unknown (two disjoint fork beams, block 0 warping, block 1 a plain Beam2 block, the entry on the elements of block 1: key `"warping"`); a non-finite force and a non-finite point (`"load_point"`, two cases); a lateral load `e_2` at `(0, 0.095)` (first-order torque, `"load_point"`); accepted: the lateral load at `(0, 0)`, the vertical load at `(0, 0.095)`, and the two-block set of L2 | exact | |

In E1 `x_0`, `x_1` are the element ends, `x_m` its centre, `M(x) = (q_z/2) x (L - x)` and `V = dM/dx = (q_z/2)(L - 2x)`: `r_1`
is the resultant on the positive face (`r_1[4] = int z sigma dA`, `r_1[2] = int tau_xz dA`), and the equilibrium of a slice
gives `dM_y/dx = V_z` in that reading. A sign convention of `V` opposite to this would show as `r[2] = -V(x_m)` and is a defect
of this check's reading, not of the library; it is reported, not re-run.

Negative controls: N1 (the term acts with the right sign and size), L0 (no term at the shear centre), A4 (the shear centre is
subtracted: with `r = p` MS-SC-down would move; and the Wagner term under a parabolic moment), A6 (the frame), A8 (the
consistent block differs from lumping, within its estimate), L2 (per-element `s`), R.

## Planned mutations (on a scratch copy, one at a time, own `CARGO_TARGET_DIR`)

| id | mutation | expected caught by |
|---|---|---|
| Y1 | the term with the wrong sign (`-c`) | A1, A3, A5, L1, N1 |
| Y2 | the shear centre not subtracted (`r = p`) | A4, L0, L2 |
| Y3 | the term left out | A1, A3, A5, L1, N1, A8 |
| Y4 | the term doubled | A1, A3, A5, L1, A8 |
| Y5 | the lumped form (`c l/2` on each `phi`, nothing else) in place of `M_H` | L1, A8 |
| Y6 | the `chi` rows and columns of `M_H` dropped (`phi`-`phi` part only) | L1, A8 |
| Y7 | the force taken in global components (not `e_2 . q`, `e_3 . q`) | A6 |
| Y8 | the term not turned to global axes (local `phi` on global rotation x) | A6 |
| Y9 | the first-order torque refusal removed | R |
| Y10 | the shear centre of the first warping block used for every element | L2 |

## Decision rule

**done**: every registered check (B0, B1, A1 to A8, E1, L0, L1, L2, N1, R) met in the one run and every mutation caught.
**partial**: any registered check missed (a `*_known_miss` record), any mutation not caught (none is declared equivalent), or any
check met only after a post-run amendment. **todo**: A1 to A3 or B1 missed.

## Limits (not covered)

- A uniform distributed load per element set, a vertical load on the symmetry axis (or through the centroid); a load with a
  first-order torque is refused (the model would need it as a distributed torque). Varying distributed loads (a trapezoid) are
  not offered; the model's `Load::LineLoad` is uniform per element.
- The in-plane second-order terms of an eccentric load (`theta_y^2`) are not added (ADR 0582 D1).
- Only the simply supported span under a UDL is checked; cantilevers, continuous beams and restrained warping are not.
- The element's moment under a UDL is linear per element (the `L^2` projection, D3); not changed here.
- The reference is Euler-Bernoulli; the rcfd element is shear-flexible (arithmetic 4).
- The term is in `K_sigma` of `matrices`, so also in the pre-stressed modes and the pre-stressed harmonic path of ADR 0585;
  neither is checked with a line-load entry.
- No case-file key (F-625).

## Open questions (none changes a default)

- **Q1. Restating the force** (ADR 0582 Q1): (a) restated on the entry (this item); (b) read from the model's `Load::LineLoad`
  on the static model and carried in the `WarpingPrestress`. Recommendation (a) now, (b) with a case-file key (F-625), taken on
  the maintainer's standing instruction of 2026-11-11 (take the recommendation).
- **Q2. The element moment under a distributed load**: (a) linear per element from `K u` (now, the `L^2` projection, converges
  at `O(l^2)` in the moment); (b) the parabolic moment from the element's line load added in `local_geometric_stiffness`.
  Option (b) would change results of existing models with line loads and warping, so it is not taken; listed as open.

## Addendum 1 (2026-11-15): results

Written after the one registered run. Commits: 23cb24db (this registration), fcb68a35 (the bit record B1, run once on the base
83b9d18c plus the registration, no library line changed; its digests are its constants), f384dc5e (`beam_warping.rs`: the field
and the builder `Warping::line_load_point`, the accessor `line_load_points`, the function `add_line_load_points` and its one call
at the end of `matrices_impl`, a module-doc paragraph; `tests/sa_beam_warping_f663.rs`). Logs in `w49i-work/f663-scratch`:
`run1.log` (the registered run, release), `b1_base.log` (the base run of B1), `b0.log`, `baseline.log`, `docs_site.log`,
`fmt.log`, `clippy.log`, `mutations.log` and `mut_Y*.log`; `arith.py`, `arith.log` (before any code).

### Results against the registered limits

| id | measured | limit | verdict |
|---|---|---|---|
| B0 | `sa_beam_warping_f598` 8 passed 1 ignored, `f598_bits` 1, `f626` 7, `f626_bits` 1, `f627` 7 passed 2 ignored, `f635` 4 passed 1 ignored, `f635_bits` 1, `f643` 4, `f661` 4, `f661_bits` 1, `sa_buckling_sign_asymmetric_f642` 2, `sa_buckling_restart_f650` 2, `sa_geometric_struct_f558_bits` 1; `baseline_hashes` 3 passed 1 ignored | pass | met |
| B1 (a4) | nine digests (the F-643 DS-top point-load factors at 16 and 8 elements, its `K` and `K_sigma`; the UDL factors without an entry, doubly symmetric and monosymmetric both ways, its `K_sigma`, and `from_static`): each equal to the base | bit-equal | met |
| A1 (a1) | DS top flange: `45.184863919` against `45.1856414209`, `-1.72e-5` | `2e-2` | met |
| A2 (a2) | DS shear centre: `59.063014527` against `59.0640165887`, `-1.70e-5` | `2e-2` | met |
| A3 (a3) | DS bottom flange: `77.147015523` against `77.1475881351`, `-7.4e-6` | `2e-2` | met |
| A4 | MS at the shear centre downward `-3.04e-4`, upward `-1.91e-5`; at the centroid downward `-3.43e-4`; on flange 1 downward `-2.87e-4` | `2e-2` each | met |
| A5 | 8 elements: `+8.4e-5`, `+1.18e-4`, `+1.66e-4` | `2e-2` each | met |
| A6 | axis y, DS top flange: `5.7e-15` | `1e-6` | met |
| A7 | `from_static`: DS top flange `8.9e-16`, MS at the centroid `1.4e-15` | `1e-9` | met |
| A8 | lumped against consistent: DS top `1.088e-6`, DS bottom `1.079e-6` | `1e-8 < d <= 1e-4` | met |
| E1 | end shears and moments of the pre-stress against D3: `3.9e-14` of `max |M|` (element 0: `V = -1406.25`, `M = -2.9296875` and `-266.6015625`, as derived) | `1e-9` | met |
| L0 | DS entry at `(0, 0)`, MS entry at `(0, z_0)`: factors bit-equal to no entry | bit-equal | met |
| L1 | `K` bit-equal; `K_sigma` pattern equal, equal off the twist equations; the 196 twist entries against `c M_H`: `6.7e-17` of the largest | equal; `1e-12` | met |
| L2 | rigid-twist energy `-71.12954495500` against `-71.12954495500017`: `2.8e-15` | `1e-12` | met |
| N1 | DS top/SC `0.7650`; bottom/SC `1.3062`; MS down/up `1.9579`; MS against `beta_x = 0` `0.376` | `< 0.85`; `> 1.2`; `> 1.7`; `> 0.2` | met |
| R | nine cases, each with its key (`UnknownSet`, `load_point` twice for non-finite values, `load_point` for the lateral load at the flange, `warping` for the plain beam); the four accepted cases accepted | exact | met |

The doubly symmetric values lie within `1.7e-5` of the Euler-Bernoulli reference, well inside the estimate `3e-3`; the
shear-flexible lateral plane lowers them much less than under the central point load of ADR 0582 (`3.5e-4` to `7.2e-4`),
plausibly because the UDL mode is closer to the first sine and the moment is smooth (not diagnosed further; no limit depends on
it). The monosymmetric values lie `1.9e-5` to `3.4e-4` low. At 8 elements the values are high by `0.8e-4` to `1.7e-4`, the
discretisation of the per-element linear moment (arithmetic 4 predicts `6e-5` to `1.4e-4` for the E-B model). E1 confirms D3 to
round-off: the pre-stress of a uniform line load is the `L^2` projection of the parabolic moment.

### Mutations (scratch copy `f663-scratch/mut`, own target directory, one at a time; the unmutated copy passes)

| id | mutation | caught by |
|---|---|---|
| Y1 | term with the wrong sign | A1, A3, A4 (centroid, flange 1), A5, A8, L1, L2, N1 |
| Y2 | `r = p` | A4 (centroid, flange 1), L0 (MS), L2 |
| Y3 | term left out | A1, A3, A4, A5, A8, L1, L2, N1 |
| Y4 | term doubled | A1, A3, A4, A5, A8, L1, L2 |
| Y5 | lumped form in place of `M_H` | A8 (lumped differs, both), L1 |
| Y6 | `chi` rows and columns of `M_H` dropped | A8 (lumped within, both: `d` above `1e-4`), L1 |
| Y7 | force in global components | A6 |
| Y8 | term not turned to global axes | A6 |
| Y9 | first-order torque refusal removed | R |
| Y10 | the first block's shear centre for every element | L2 |

Ten of ten caught. Y5 and Y6 are caught only by A8 and L1: a wrong element block that keeps the total `c l` moves the factor
far inside the acceptance limit `2e-2`, which is why A8 and L1 were registered.

### Other runs and deviations

`cargo fmt -p rcfd-fem --check`: clean. `cargo clippy -p rcfd-fem --release --all-targets -- -D warnings`: clean. `cargo test -p
rcfd-cli --release --test docs_site`: the first run failed on one broken link in this registration (the file name of ADR 0546
was mistyped in the head); the link target was corrected after the run (an editorial change above the addenda; no check, limit
or design text changed) and the second run passed (`docs_site.log`). Not run: the whole `rcfd-fem` suite and the workspace (the
gate of the lead); the new code runs only with a `line_load_point` entry, and the tests that reach `matrices_impl` without one
are those of B0 and B1. `docs/reference-data.md` has a new F-663 section.

### Decision by the registered rule

**done**: every registered check (B0, B1, A1 to A8, E1, L0, L1, L2, N1, R) met in the one run, no amendment, ten of ten
mutations caught. The acceptance clauses: a1 `1.72e-5`, a2 `1.70e-5`, a3 `7.4e-6`, each against a Rayleigh-Ritz reference
derived here and converged to `2.4e-8`, at 16 elements against `2e-2`; a4 bit-identical (B1, B0, `baseline_hashes`). Done is the
maintainer's call.

### Remaining questions

- Q1 (the force restated on the entry, or read from the model's line load): options (a) restated, (b) read from the model and
  carried in the `WarpingPrestress`. Recommendation (a) now, (b) with a case-file key (F-625); taken on the maintainer's standing
  instruction of 2026-11-11 (take the recommendation).
- Q2 (the element moment under a line load): options (a) the linear `L^2` projection from `K u` (now; measured `<= 1.7e-4` at 8
  elements), (b) the parabolic moment of the element's line load in `local_geometric_stiffness`. (b) would change the results
  of existing models with line loads and warping elements, so it stays open for the maintainer; recommendation (a), no change.
- The load-height terms reach the pre-stressed modes and the pre-stressed harmonic path through `K_sigma` but are not checked
  there; a follow-up is proposed below.

## For the lead: roadmap entries

> Proposed status: done.

> F-663 note: Structural solver, advanced tier. Follow-up of F-643 (ADR 0582, 2026-11-15). DONE. ADR 0588, 2026-11-15: distributed load height on the warping buckling path (`rcfd_fem::beam_warping`): `Warping::line_load_point(element_set, q, point)` adds the consistent second-order term (1/2) int (q . (p - s)) phi^2 dx of a uniform dead line load along a section point p on the Hermite twist of each element (c M_H, M_H = int H_i H_j dx = (l/420)[156, 22l, 54, -13l; ...]), s each element's own shear centre, to K_sigma of `matrices` (so buckling_factors, buckling_factors_with, prestressed_modes, the pre-stressed harmonic path); without an entry a line load acts at the shear centre, as before; a load with a first-order torque about the centroid, an unknown set and an element without a warping unknown are refused. The pre-stress of a uniform line load (end forces K u) is shown to be the L2 projection of the parabolic moment (end moments shifted by q_z l^2/12, mid-element shear; 3.9e-14). Uniformly distributed load on a simply supported span, L = 3, 16 elements, default BucklingOptions, against Rayleigh-Ritz references derived in the ADR (sine series, converged to 2.4e-8): doubly symmetric I on the top flange 1.7e-5, at the shear centre 1.7e-5, on the bottom flange 7.4e-6 (C_1 = 1.131); ADR 0553 monosymmetric I at the shear centre (both directions), at the centroid and on flange 1 within 3.4e-4; 8 elements within 1.7e-4; axis y 6e-15; from_static 1.4e-15; the lumped point entries of ADR 0582 within 1.1e-6 of the consistent term. Results without a distributed load height bit-identical (nine digests recorded on the base, the F-643 point-load factors and matrices among them; the ADR 0546 to 0585 bit records; baseline_hashes). Ten of ten mutations caught. Not covered: non-uniform distributed loads, loads with a first-order torque, the in-plane second-order terms of an eccentric load, other supports, the parabolic moment within an element (the L2 projection is used, ADR 0588 Q2), pre-stressed modes and the harmonic path with a load height, no case-file key.

> Follow-up proposed: Load height on the pre-stressed warping modes and harmonic path. Deps F-663, F-661. Acceptance: Every limit not stated here is registered in the ADR before the run. The lowest flexural-torsional frequency of the fork-supported doubly symmetric I of ADR 0588 under a uniformly distributed load of 0.5 of its buckling load on the top flange, at the shear centre and on the bottom flange (prestressed_modes, and the resonance of the pre-stressed WarpingHarmonic) equals a Rayleigh-Ritz reference of the same energy with the consistent mass derived in the ADR within 1e-2 at 16 elements and falls to zero at the ADR 0588 buckling load within 1e-2; results without a load height bit-identical.
