# ADR 0582: F-643 registration: load height and non-uniform moment for monosymmetric warping beams (the load-height geometric term, the central point load at, above and below the shear centre)

Status: **REGISTERED 2026-11-15, written before any code of F-643 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-643 was written, compiled or run in the repository: the files listed under "What was
read" were read, and the arithmetic below was done by hand and evaluated with one plain numpy script in the scratch folder
(`w49i-work/f643-scratch/arith.py`, log `arith.log`: the section constants, a Rayleigh-Ritz sine-series solution of the
second-order energy of D2 for every load case below, its convergence in the number of terms, and an independent cubic-Hermite
Euler-Bernoulli Vlasov model of the same energy; no rcfd code; disclosed, its numbers quoted below). Results go in the addenda at
the end; nothing above the addenda is changed after a run.

Roadmap item F-643 (deps F-627), structural solver, advanced tier, the follow-up named by
[ADR 0553](0553-f627-registration-monosymmetric-thin-walled-sections-for-the-warping-element.md) (its question Q1, recommendation
(b): the load-height term together with a non-uniform moment check; its limit 1: the shear-force terms with an offset were not
checked). Number 0582 given by the lead. Branch `f643-work` from the local main tip 9559dc28 (which holds F-635, ADR 0574, and the
eigensolver remedies of F-642, ADR 0559, and F-650, ADR 0564), worktree `w49i-work/f643`, scratch `w49i-work/f643-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test edited;
no new dependency, no unsafe code, the crate graph unchanged; clean-room. Every choice below that the maintainer would normally
be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation), except the open
questions at the end.

## What F-643 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. The lateral-torsional buckling load of a simply supported
> doubly symmetric I under a central point load at the shear centre, above and below it, and of the ADR 0553 monosymmetric I
> under a central point load at the shear centre, each equals a closed form or a converged Rayleigh-Ritz reference derived in the
> ADR within 2 percent at 16 elements; uniform-moment results unchanged.

The brief adds: load height enters as a geometric stiffness term `P a` on the twist at the load point, to be derived; references
derived here (Rayleigh-Ritz converged in the number of terms), remembered coefficients marked as such; use the eigensolver remedy
of F-642; opt-in only.

Clauses: **a0** every limit registered here; **a1** doubly symmetric I, central point load at the shear centre, within `2e-2` of
the reference at 16 elements; **a2** the same with the load above the shear centre; **a3** below it; **a4** the ADR 0553
monosymmetric I, central point load at the shear centre, within `2e-2` (both directions of the load: the larger flange in
compression and in tension; "a central point load" does not name a direction, so both are checked); **a5** uniform-moment results
unchanged (bit for bit).

### Readings, registered

- **"Simply supported"**: the fork supports of ADR 0546 and ADR 0553 (`v = w = phi = 0` at both ends, `u = 0` at node 0, warping
  free), `L = 3`, 16 elements along `x` (the load at node 8, mid-span), steel `E = 2e11`, `nu = 0.3`, shear factor `5/6`.
- **"Doubly symmetric I"**: the section of ADR 0546 (`doubly_symmetric_i(0.1, 0.01, 0.18, 0.006)`), flange mid-planes at `z =
  +/- h_c/2 = +/- 0.095`. **"ADR 0553 monosymmetric I"**: `monosymmetric_i(0.15, 0.012, 0.08, 0.008, 0.2, 0.006)`, flange 1 (the
  larger) on `+e_3`, its mid-plane at `z = +0.0718681`, the shear centre at `z_0 = +0.0525803` from the centroid.
- **"Central point load"**: a force `P_ref = 1000` N along `-e_3` (downward; `+e_3` for the second direction of a4) at node 8,
  the reference load of the pre-stress; the buckling load is `lambda P_ref`.
- **"At the shear centre, above and below it"**: the point of application on the symmetry axis at the shear centre, on the top
  flange mid-plane (`z = +0.095`, above: on the side the force points away from, destabilising) and on the bottom flange
  mid-plane (`z = -0.095`, below).
- **"A closed form or a converged Rayleigh-Ritz reference"**: there is no exact closed form for a central point load (the
  moment-gradient coefficients of the codes are fits); the reference is the Rayleigh-Ritz solution of D2, converged in the number
  of terms (D4).
- **"Uniform-moment results unchanged"**: every path without the new load-point entry runs the arithmetic of ADR 0546, 0548,
  0553 and 0574 operation for operation; the bit records and test files of those ADRs pass unchanged, and the ADR 0559 default
  `-M_ref` factor is reproduced bit for bit (U1).
- **"Opt-in"**: a new `Warping::load_point` entry (D3); a `Warping` without it is unchanged. No case-file key (F-625).

## What was read (no run)

- `AGENT_RULES.md`; the brief; ADR 0553 (whole: D1 to D6, the amendments, the addendum), ADR 0559 (the design and the addendum),
  ADR 0564 (the head), ADR 0546 D1 and D2 and ADR 0508 D1 through their quotation in ADR 0553.
- `crates/rcfd-fem/src/beam_warping.rs` (the section, `Warping`, `local_geometric_stiffness`, `offset_transform`,
  `section_local_geometric_stiffness`, `matrices_impl`, `buckling_impl`, `WarpingPrestress`, `modes_impl`, `load_vector`).
- Tests: `sa_beam_warping_f627.rs` (the set-ups), `sa_beam_warping_f598.rs` (the set-ups), the ADR 0553 arithmetic script
  `f627-scratch/arith.py` (its section function, re-typed in `arith.py`).

## Design

### D1. The load-height term (first principles)

A dead force `F` (constant in magnitude and direction) is applied at a point `p` of the cross-section (section axes `y`, `z` from
the centroid). The section turns rigidly by `phi` about the longitudinal axis through the shear centre `s = (y_0, z_0)` (the
kinematics of ADR 0553 D1). Write `r = p - s` and `e_x x r = (-r_z, r_y)`. The exact displacement of the point under the twist
alone is `(cos phi - 1) r + sin phi (e_x x r)`, whose second-order part is `-(phi^2/2) r` (one rotation axis, so no ambiguity of
the rotation parametrisation). The second-order work of `F` is `-(phi^2/2) F . r`, and the second-order potential is

`Pi_F = (1/2) (F . r) phi^2`, `F . r = F_y (p_y - y_0) + F_z (p_z - z_0)`,

added at the twist of the loaded node. For a downward force `F_z = -P` at the height `a = p_z - z_0` above the shear centre,
`F . r = -P a`: the term `-(1/2) P a phi^2` lowers the stiffness (destabilising) for a load above the shear centre and raises it
below; this is the classical load-height term (Timoshenko and Gere, Theory of Elastic Stability, 1961, ch. 6; Trahair,
Flexural-Torsional Buckling of Structures, 1993, ch. 6; from memory, derived here). It is linear in the load, so it scales with
the buckling factor as the pre-stress does: it is a term of `K_sigma`. In the global unknowns of a node, `phi = e_1 . theta` (the
section axis `e_1` and the three rotation unknowns), so `K_sigma += (F . r) e_1 e_1^T` on the rotation equations of the node, `F`
in section axes `F_y = e_2 . F`, `F_z = e_3 . F`.

Other second-order terms of an eccentric load, written out and left out: the small rotation vector `theta = (phi, theta_y,
theta_z)` moves `r` by `(1/2) theta x (theta x r)` to second order, whose work is `(1/2) [(F . theta)(theta . r) - (F . r)
|theta|^2]`. For `F` and `r` in the section plane, the `theta_z^2` parts cancel for a vertical force on the symmetry axis, and
what remains besides the `phi^2` term is `-(1/2) F_z r_z theta_y^2` (and its mixed `theta_y theta_z` relatives), terms of the
in-plane bending rotation that do not couple to the lateral-torsional mode of a section symmetric about the `x z` plane. They are
not added (Limits). The first-order moment of an eccentric force about the centroid node, `p x F`, `M_x = p_y F_z - p_z F_y`,
belongs to the linear load; the entry refuses a load point with a nonzero `M_x` (D3), so the model's nodal loads at the centroid
stay the exact first-order load. For a vertical force on the symmetry axis (`p_y = F_y = 0`), `M_x = 0`.

**Without an entry the force acts at the shear centre.** The nodal force of the model does linear work only (`F . u_c`), and the
second-order energy of ADR 0546 and ADR 0553 D2 holds no load term: that is the energy of a load on the axis of the twist, the
shear centre. A load point at `p = s` therefore adds `F . r = 0` exactly (L0).

### D2. The second-order energy under a non-uniform moment (the reference energy)

In shear-centre unknowns (`v` the lateral displacement of the shear centre, `phi` the twist), Euler-Bernoulli, forks:

`Pi_2 = (1/2) int (E I_z v''^2 + G J phi'^2 + E C_w phi''^2) dx + lambda [ int M phi v'' dx + (1/2) int beta_x M phi'^2 dx + (1/2) Q phi(L/2)^2 ]`

with `M = M_y(x)` the major-axis moment of the reference load in the convention of ADR 0508 and ADR 0553 (`M_y = int z sigma dA`),
`beta_x = beta_y^S` the Wagner coefficient about the shear centre (ADR 0553 D2), `Q = F . r` of D1. Derivation:

- The normal-stress part is ADR 0553 D2: `-M v' phi' + (1/2) beta_x M phi'^2`, with `M` varying along `x`.
- The shear force `V = dM/dx` (through the shear centre) turns with the section by `phi`; its component `-phi V` along `y` does
  work on the slope `v'`: `-V phi v'` (the shear-force term of ADR 0508 D1 in shear-centre form). Together, `-int (M v' phi' + M'
  phi v') dx = -int (M phi)' v' dx = int M phi v'' dx - [M phi v']`, and the bracket vanishes at forks (`phi = 0`). This is the
  classical form `int M phi v''` of Vlasov's energy (Timoshenko and Gere ch. 6, Trahair ch. 6; from memory), whose Euler-Lagrange
  equation in `v` is the deformed-segment equilibrium `E I_z v'' = -M phi` (the major-axis moment has the component `M phi` about
  the turned minor axis).
- For uniform `M` it is the energy of ADR 0553 D3; the script reproduces both uniform-moment closed forms (ADR 0546 and ADR 0553
  D3) to `4e-16` (arithmetic 2), which checks the signs of the Wagner and coupling terms against ADR 0553.
- The sign of the coupling term does not matter (`v -> -v`); the sign of `beta_x M` does, and is that of ADR 0553 (D2 there).

The library already has every term of D2 except `Q`: the moment, shear-force and Wagner terms of ADR 0508 D1 and ADR 0546 D2 on
the end forces of each element (linear moment, constant shear per element), in shear-centre form for an offset section (ADR 0553
D2, D4). ADR 0553 registered those shear-force terms with an offset as unchecked (its limit 1); the a4 checks are their check. The
FE uses the integrated-by-parts form with the Timoshenko rotation fields; the reference uses `int M phi v''` with `v` of the shear
centre: independent routes to the same energy.

### D3. API (in `rcfd_fem::beam_warping`)

`Warping::load_point(self, node_set: &str, force: Vec3, point: [f64; 2]) -> Warping`: the reference load of the pre-stress holds
the force `force` (global components, N) at each node of the set, applied at `point = (p_y, p_z)` (m, section axes, from the
centroid). Its only effect is the term of D1, `(F . (p - s)) e_1 e_1^T` on the rotation equations of the node in `K_sigma` of
[`matrices`] (so in `buckling_factors`, `buckling_factors_with` and `prestressed_modes`); `s` the shear centre of the warping
elements at the node, `e_1`, `e_2`, `e_3` their frame. The static solve, the load vector, the modes without pre-stress, the
harmonic and transient paths do not read it (the linear load is the model's). Accessor `load_points()`. Refused in `matrices`
(and through it in every caller):

- an unknown set (`UnknownSet`); a node without a warping unknown (key `"warping"`);
- a non-finite force or point component (key `"load_point"`);
- a nonzero first-order torque about the centroid, `|p_y F_z - p_z F_y| > 1e-9 |F| |p|` (key `"load_point"`; D1);
- warping elements at the node with different shear centres (key `"load_point"`; the twist axis would be ambiguous).

With no entry nothing is added (no triplet), so every matrix and result is bit-identical. The caller states the force again
(rather than the library reading the model's loads) because the buckling path takes the pre-stress and the unloaded model, as
the ADR 0553 tests do; documented: the force must be the one in the reference load of the pre-stress.

### D4. The references (Rayleigh-Ritz, derived; arithmetic 3)

`v = sum a_n sin(n pi x/L)`, `phi = sum b_n sin(n pi x/L)`, `n = 1 .. N` (every `n`, odd and even; each satisfies the fork
conditions `v = v'' = phi = phi'' = 0`). The stiffness is diagonal (`E I_z k_n^4 L/2`, `(G J k_n^2 + E C_w k_n^4) L/2`); the
geometric matrix holds `int M sin_m (-k_n^2 sin_n) dx` (coupling), `beta_x int M k_m k_n cos_m cos_n dx` and `Q sin_m(L/2)
sin_n(L/2)`; the integrals by Gauss-Legendre with 400 points on each half (the moment has its kink at `L/2`; 800 points change the
result by `< 3e-14`). The lowest positive `lambda` of `(K + lambda G) x = 0` (Cholesky of `K`, symmetric eigenproblem). Converged
in `N`: `N = 5, 11, 21, 41, 81`; the change from 41 to 81 is `<= 3.2e-6` for every case, and the cubic-Hermite E-B model of the
same energy at 32 elements agrees with `N = 81` within `4e-7`: the reference is the `N = 81` value, uncertain by about `2e-6`.

| case | `Q = F . r` (N m) | `lambda_ref` (`N = 81`), `P_ref = 1000` | N 41 vs 81 | Hermite E-B 16 el. |
|---|---|---|---|---|
| DS-SC: doubly symmetric, at the shear centre | `0` | **`106.64131188`** | `1.2e-6` | `3.4e-6` |
| DS-top: above, top flange (`a = +0.095`) | `-95` | **`76.653190682`** | `1.7e-6` | `1.8e-6` |
| DS-bot: below, bottom flange (`a = -0.095`) | `+95` | **`147.52392793`** | `3.1e-6` | `7.8e-6` |
| MS-SC-down: monosymmetric, at the shear centre, downward (flange 1 compressed) | `0` | **`229.64455760`** | `1.5e-6` | `5.0e-6` |
| MS-SC-up: the same, upward (flange 1 in tension) | `0` | **`133.91738647`** | `9.0e-7` | `7.9e-6` |
| MS-C-down: monosymmetric, at the centroid (`r_z = -z_0`), downward | `+52.580303` | `285.61557187` | `2.9e-6` | `8.2e-6` |
| MS-top-down: on flange 1 (`r_z = 0.0718681 - z_0`), downward | `-19.287828` | `210.84970872` | `1.5e-6` | `4.7e-6` |
| MS-C-up: at the centroid, upward | `-52.580303` | `106.14875777` | `1.2e-6` | `4.5e-6` |

Records (not used as limits): the doubly symmetric central-load coefficient `M_max/M_cr = (lambda P L/4)/M_cr = 7.998098e4/5.875035e4
= 1.361`, near the remembered textbook `1.35` to `1.36` (from memory, not used); load height top/SC `0.7188`, bottom/SC `1.3834`;
monosymmetric down/up `1.7148`; the monosymmetric I at the shear centre with `beta_x = 0`: `177.97`.

## Arithmetic done before the code (disclosed; `arith.log`)

1. Sections (the script re-types ADR 0553's): doubly symmetric `A = 3.08e-3`, `I_z = 1.6699066667e-6`, `J = 7.9626666667e-8`,
   `C_w = 1.5041666667e-8`; monosymmetric as ADR 0553 D5 (`z_0 = 5.2580303363e-2`, `beta_x = -1.5259215331e-1`).
2. Uniform moment through the Ritz code (`N = 9`): `5.8750350408e4` (doubly symmetric, both signs) and `5.3958795381e4`,
   `1.7845456376e5` (monosymmetric), each within `4.5e-16` of the closed forms of ADR 0546 and ADR 0553 D3.
3. The table of D4.
4. Discretisation and shear flexibility. The Hermite E-B model: 8 elements `<= 1.3e-4`, 16 elements `<= 8e-6`. The rcfd element
   has a shear-flexible lateral plane: `E I_z k^2/(kappa G A) = 1.9e-3` (doubly symmetric) and `3.5e-3` (monosymmetric) at `L =
   3`, which lowers `P_y` by that fraction and the factor by up to about half of it; ADR 0546 measured `+5.9e-4` and ADR 0553
   `1.3e-3`, `3.9e-4` under uniform moment. **Estimate `3e-3` for every LTB case at 16 and at 8 elements.**

## Checks (limits fixed now)

Every aggregated error is asserted finite before it is compared (a non-finite value is a miss). Set-up of every LTB solve: the
fork model of the readings; the reference load `P_ref` at node 8 as a model nodal force; the six-unknown static solve and
`Prestress::from_static(..).with_beam_resultants(.., [0, 0])` (as the ADR 0553 tests); `buckling_factors(&unloaded, &pre,
&warping, &BucklingOptions::new(3))`, the default options (the ADR 0559 and ADR 0564 remedies are in the base); the warping
`Warping::new().block(0, ws)` plus, where the case has a load point, `.load_point("mid", F, p)`. The factor is `factors[0]`.

| id | check | limit | estimate |
|---|---|---|---|
| B0 | `sa_beam_warping_f598.rs`, `sa_beam_warping_f598_bits.rs`, `sa_beam_warping_f626.rs`, `sa_beam_warping_f626_bits.rs`, `sa_beam_warping_f627.rs`, `sa_beam_warping_f635.rs`, `sa_buckling_sign_asymmetric_f642.rs`, `sa_buckling_restart_f650.rs`, `sa_geometric_struct_f558_bits.rs` pass unchanged after the change; `cargo test -p rcfd-api --release --test baseline_hashes` passes | pass | pass |
| U1 | **acceptance a5**: the ADR 0553 monosymmetric I under uniform moment (16 elements, `L = 3`, the ADR 0553 set-up) through the changed code: `-M_ref` with the default options bit-equal to `178.68421280057305` (ADR 0559 A1); `-M_ref` and `+M_ref` with `subspace_size = Some(40)` bit-equal to `178.68421280057328` and `53.97979218576993` (ADR 0553 runs 2 and 3) | bit-equal | bit-equal |
| A1 | **acceptance a1**: DS-SC, `|lambda/106.64131188 - 1|` | `<= 2e-2` | `3e-3` |
| A2 | **acceptance a2**: DS-top, load point `(0, +0.095)`, `|lambda/76.653190682 - 1|` | `<= 2e-2` | `3e-3` |
| A3 | **acceptance a3**: DS-bot, load point `(0, -0.095)`, `|lambda/147.52392793 - 1|` | `<= 2e-2` | `3e-3` |
| A4 | **acceptance a4**: MS-SC-down, load point `(0, z_0)`, `|lambda/229.64455760 - 1|` | `<= 2e-2` | `3e-3` |
| A5 | **acceptance a4**: MS-SC-up (force `+e_3`), load point `(0, z_0)`, `|lambda/133.91738647 - 1|` | `<= 2e-2` | `3e-3` |
| A6 | the offset load-height path: MS-C-down (point `(0, 0)`), MS-top-down (point `(0, 0.0718681318681)`), MS-C-up, each against D4 | `<= 2e-2` each | `3e-3` |
| A7 | A1 to A5 at 8 elements (load at node 4), against the same references | `<= 2e-2` each | `3e-3` |
| A8 | A2 and A4 with the beam along global `y` (orientation `z`, the permuted supports of ADR 0553 A6, the force along `-e_3` of that frame) against axis `x` | `<= 1e-6` relative | `1e-12` |
| A9 | A2 and A4 with the pre-stress of the warping static solve (`solve_static`, `WarpingPrestress::from_static(.., [0, 0])`, `buckling_factors_with`) against the six-unknown pre-stress | `<= 1e-9` relative | `1e-12` |
| L0 | a load point at the shear centre adds nothing: DS-SC with `.load_point("mid", F, (0, 0))` and MS-SC-down with `(0, z_0)`, against the same without an entry: factors bit-equal | bit-equal | bit-equal |
| L1 | element of the term: `matrices` of the DS-top model with and without the entry (the same pre-stress): `stiffness` bit-equal; `geometric` bit-equal except at the rotation-x equation of node 8, where the difference is `-P_ref * 0.095 = -95` | bit-equal; `<= 1e-12` relative | exact; `1e-16` |
| N1 | negative controls on the measured factors: DS-top/DS-SC `< 0.8` and DS-bot/DS-SC `> 1.25` (reference `0.719`, `1.383`); MS-SC-down/MS-SC-up `> 1.5` (`1.715`); MS-SC-down against the `beta_x = 0` reference `177.97` `> 0.15` away (`0.29`) | as stated | as stated |
| R | refusals, by `buckling_factors`: an unknown set (`UnknownSet`); a node without a warping unknown (a two-block model, the load-point set on a node of the non-warping block: key `"warping"`); a non-finite force and a non-finite point (`"load_point"`, two cases); a lateral force `e_2` at `(0, 0.095)` (first-order torque, `"load_point"`); a node shared by two warping blocks with different shear centres (`"load_point"`); and accepted: the lateral force at `(0, 0)` and the vertical force at `(0, 0.095)` | exact | |

Negative controls: N1 (the term acts, with the right sign and size; the Wagner term acts under a moment gradient), L0 (no term at
the shear centre), A6 (the shear centre is subtracted: with `r = p` MS-SC-down would move), A8 (the frame), R.

The models of R: for the node without a warping unknown, two disjoint fork beams of 16 elements (block 0 along `x` at `y = 0`, warping; block 1 along `x` at `y = 1`, a plain Beam2 block), the load-point set on the mid node of block 1 (key `"warping"`); for the shear-centre case, one beam of two warping blocks (elements 0 to 7 the monosymmetric section, 8 to 15 the doubly symmetric one), the load point at node 8 (key `"load_point"`).

## Planned mutations (on a scratch copy, one at a time, own `CARGO_TARGET_DIR`)

| id | mutation | expected caught by |
|---|---|---|
| Z1 | the term with the wrong sign (`-(F . r)`) | A2, A3, A6, L1, N1 |
| Z2 | the shear centre not subtracted (`r = p`) | A4, A5, A6, L0 |
| Z3 | `r = p + s` | A4, A5, A6, L0 |
| Z4 | the term left out | A2, A3, A6, L1, N1 |
| Z5 | the term on the global rotation-x equation regardless of `e_1` | A8 |
| Z6 | the force taken in global components (not `e_2 . F`, `e_3 . F`) | A8 |
| Z7 | the first-order torque refusal removed | R |
| Z8 | the term doubled | A2, A3, L1 |
| Z9 | the shear-force terms of `local_geometric_stiffness` dropped (the moment-gradient part of D2) | A1 to A5 |
| Z10 | the moment and shear part of an offset section not transformed (`G` in place of `O^T G O` in `section_local_geometric_stiffness`) | A4, A5 |

## Decision rule

**done**: every registered check (B0, U1, A1 to A9, L0, L1, N1, R) met in the one run and every mutation caught. **partial**: any
registered check missed (a `*_known_miss` record), any mutation not caught (none is declared equivalent), or any check met only
after a post-run amendment. **todo**: A1 to A5 or U1 missed.

## Limits (not covered)

- Load height for a vertical force on the symmetry axis and for a force through the centroid; an eccentric force with a
  first-order torque is refused (the model would need that torque as a nodal moment; D1).
- The second-order terms of an eccentric load in the in-plane rotation `theta_y` (D1) are not added.
- Distributed loads with load height (a load point is a node set; a distributed load would be lumped by the user into nodal
  forces at a height, which converges but is not checked).
- Only the central point load on a simply supported span is checked; other moment diagrams, continuous beams, cantilevers and
  restrained warping under a moment gradient are not.
- The reference is Euler-Bernoulli; the rcfd element is shear-flexible (arithmetic 4).
- The term is in `K_sigma` of `matrices` (buckling and pre-stressed modes); the pre-stressed modes with a load point are not
  checked.
- No case-file key (F-625); sections without symmetry (ADR 0553 Q2) as before.

## Open questions (none changes a default)

- **Q1. Reading the force from the model's loads** instead of restating it on the entry. Options: (a) restated (this item: the
  buckling path takes an unloaded model and a pre-stress); (b) read from the model given to the static solve, stored in the
  `WarpingPrestress`. Recommendation (a) now; (b) if a case-file key is added (F-625), where the load and its height would be one
  entry.
- **Q2. Distributed loads at a height.** Options: (a) by lumped nodal entries (now); (b) a distributed load-height term
  `(1/2) int q (r . e) phi^2 dx` with the element's Hermite twist (consistent). Recommendation (b) as a separate item when a case
  needs it.

## Addendum 1 (2026-11-15): results

Written after the one registered run. Code and tests: commit 4f5c711d (`beam_warping.rs`: the field and the builder
`Warping::load_point`, the accessor `load_points`, the function `add_load_points` and its one call at the end of
`matrices_impl`, a module-doc paragraph; `tests/sa_beam_warping_f643.rs`). Logs in `w49i-work/f643-scratch`: `run1.log` (the
registered run, release), `b0.log`, `baseline.log`, `docs_site.log`, `fmt.log`, `clippy.log`, `mutations.log` and `mut_Z*.log`;
the reference arithmetic `arith.py`, `arith.log` (before any code).

### Results against the registered limits

| id | measured | limit | verdict |
|---|---|---|---|
| B0 | `sa_beam_warping_f598` 8 passed 1 ignored, `f598_bits` 1, `f626` 7, `f626_bits` 1, `f627` 7 passed 2 ignored, `f635` 4 passed 1 ignored, `sa_buckling_sign_asymmetric_f642` 2, `sa_buckling_restart_f650` 2, `sa_geometric_struct_f558_bits` 1; `baseline_hashes` 3 passed 1 ignored | pass | met |
| U1 (a5) | `-M_ref` default `178.68421280057305`; `-M_ref` subspace 40 `178.68421280057328`; `+M_ref` subspace 40 `53.97979218576993`: each bit-equal | bit-equal | met |
| A1 (a1) | DS at the shear centre: `106.58468663` against `106.64131188`, `-5.3e-4` | `2e-2` | met |
| A2 (a2) | DS above: `76.626135450` against `76.653190682`, `-3.5e-4` | `2e-2` | met |
| A3 (a3) | DS below: `147.41742208` against `147.52392793`, `-7.2e-4` | `2e-2` | met |
| A4 (a4) | MS at the shear centre, downward (flange 1 compressed): `229.31935516` against `229.64455760`, `-1.42e-3` | `2e-2` | met |
| A5 (a4) | MS at the shear centre, upward: `133.82441944` against `133.91738647`, `-6.9e-4` | `2e-2` | met |
| A6 | MS at the centroid down `-1.67e-3`, on flange 1 down `-1.31e-3`, at the centroid up `-5.0e-4` | `2e-2` | met |
| A7 | 8 elements: `-4.2e-4`, `-2.8e-4`, `-5.3e-4`, `-1.18e-3`, `-4.6e-4` | `2e-2` | met |
| A8 | axis y: DS above `2.0e-15`, MS downward `8.4e-14` | `1e-6` | met |
| A9 | `from_static`: DS above `4.9e-15`, MS downward `9.5e-15` | `1e-9` | met |
| L0 | DS entry at `(0, 0)` and MS entry at `(0, z_0)`: factors bit-equal to no entry | bit-equal | met |
| L1 | `K` bit-equal; `K_sigma` pattern equal, bit-equal except at the twist of node 8; there the difference is `-95` exactly (`0` relative) | bit-equal; `1e-12` | met |
| N1 | DS top/SC `0.7189`; bottom/SC `1.3831`; MS down/up `1.7136`; MS against `beta_x = 0` `0.289` | `< 0.8`; `> 1.25`; `> 1.5`; `> 0.15` | met |
| R | eight cases, each with its key (`UnknownSet`, `load_point` twice for non-finite values, `load_point` for the lateral force at the flange, `warping`, `load_point` for two shear centres); the two accepted cases accepted | exact | met |

Every LTB value lies below its Euler-Bernoulli reference by `3e-4` to `1.7e-3`, inside the estimate `3e-3`, as the
shear-flexible lateral plane predicts (arithmetic 4); the Hermite Euler-Bernoulli model of the same energy is within `8e-6` at 16
elements, so the discretisation contributes little. The default `BucklingOptions` returned every case, including the
sign-asymmetric monosymmetric spectra (the ADR 0559 and ADR 0564 remedies); no set-up amendment was needed.

### Mutations (scratch copy `f643-scratch/mut`, own target directory, one at a time; the unmutated copy passes)

| id | mutation | caught by |
|---|---|---|
| Z1 | term with the wrong sign | A2, A3, A6, A7, L1, N1 |
| Z2 | `r = p` | A4, A5, A6, A7, L0, N1 |
| Z3 | `r = p + s` | A4, A5, A6, A7, L0, N1 |
| Z4 | term left out | A2, A3, A6, A7, L1, N1, R (the refusals go with it) |
| Z5 | term on global rotation x regardless of `e_1` | A8 (DS above) |
| Z6 | force in global components | A8 (DS above) |
| Z7 | first-order torque refusal removed | R |
| Z8 | term doubled | A2, A3, A6, A7, L1 |
| Z9 | shear-force terms of `local_geometric_stiffness` dropped | A1 to A7 (16 values) |
| Z10 | offset moment and shear part not transformed | A4, A5, A6, U1 |

Ten of ten caught. Z5 and Z6 are caught by the DS case of A8 only; the MS case of A8 has `F . r = 0` (load at the shear centre),
as expected.

### Other runs

`cargo test -p rcfd-cli --release --test docs_site`: 2 passed (after the `docs/reference-data.md` section). `cargo fmt -p
rcfd-fem --check`: clean. `cargo clippy -p rcfd-fem --release --all-targets -- -D warnings`: clean. Not run: the whole
`rcfd-fem` suite and the workspace (the gate of the lead); the new code runs only with a load-point entry, and the tests that
reach `matrices_impl` without one are those of B0.

### Decision by the registered rule

**done**: every registered check (B0, U1, A1 to A9, L0, L1, N1, R) met in the one run, no amendment, ten of ten mutations
caught. The acceptance clauses: a1 `5.3e-4`, a2 `3.5e-4`, a3 `7.2e-4`, a4 `1.42e-3` and `6.9e-4`, each against a Rayleigh-Ritz
reference derived here and converged to `3.2e-6`, at 16 elements against `2e-2`; a5 bit-identical. Done is the maintainer's
call.

### Remaining questions

- Q1 (force restated on the entry or read from the model), recommendation (a) now, (b) with a case-file key (F-625).
- Q2 (distributed loads at a height), recommendation (b) as a separate item when a case needs it.

## For the lead: roadmap entries

> Proposed status: done.

> F-643 note: Structural solver, advanced tier. DONE. ADR 0582, 2026-11-15: load height on the warping buckling path (`rcfd_fem::beam_warping`): `Warping::load_point(set, force, point)` adds the second-order term (1/2) (F . (p - s)) phi^2 of a dead force at a section point p, the twist about the shear centre s, to K_sigma of `matrices` (so buckling_factors, buckling_factors_with, prestressed_modes); without an entry a nodal force acts at the shear centre, as before; a force with a first-order torque about the centroid and different shear centres at the node are refused. The non-uniform-moment terms of ADR 0508 and ADR 0553 (shear-force terms in shear-centre form) are checked by this item. Central point load on a simply supported span, L = 3, 16 elements, default BucklingOptions, against Rayleigh-Ritz references derived in the ADR (sine series of int M phi v'' + (1/2) int beta_x M phi'^2 + the load-height term, converged to 3.2e-6): doubly symmetric I at the shear centre 5.3e-4, top flange 3.5e-4, bottom flange 7.2e-4; ADR 0553 monosymmetric I at the shear centre 1.4e-3 (larger flange compressed) and 6.9e-4 (in tension); at the centroid and on flange 1 within 1.7e-3; 8 elements within 1.2e-3; axis y 8e-14; from_static 1e-14. Uniform-moment results bit-identical (the ADR 0553 and 0559 factors, the ADR 0546, 0548, 0553 bit records, baseline_hashes). Ten of ten mutations caught. Not covered: distributed loads at a height, eccentric loads with a first-order torque, the in-plane second-order terms of an eccentric load, other moment diagrams and supports, pre-stressed modes with a load point, no case-file key.

> Follow-up proposed: distributed loads at a height on warping beams (a consistent load-height term (1/2) int q (r . e) phi^2 dx on the Hermite twist). Deps F-643. Acceptance: Every limit not stated here is registered in the ADR before the run. The lateral-torsional buckling load of a simply supported doubly symmetric I under a uniformly distributed load on the top flange, at the shear centre and on the bottom flange equals a converged Rayleigh-Ritz reference derived in the ADR within 2 percent at 16 elements; results without a distributed load height unchanged.
