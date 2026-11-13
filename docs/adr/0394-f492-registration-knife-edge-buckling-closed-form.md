# ADR 0394: F-492 registration: buckling factors of the knife-edge column against the solid's closed form

Status: **REGISTERED 2026-11-13, written before any code of F-492 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-492 was written, compiled or run: the files listed under "What was read" were read, and
the arithmetic below was done by hand and with a short scratch script of the closed forms (`w49i-work/f492-scratch/arith.py`,
disclosed under "Arithmetic"; it does not call rcfd). Results go in the addendum at the end; nothing above the addendum is changed
after a run.

Parent: [ADR 0390](0390-f395-registration-geometric-stiffness-prestressed-modes-and-buckling.md) (F-395; its check A9 missed,
addendum 1, "Diagnosis of the A9 miss" and remaining question 1). Roadmap item F-492 (deps F-395). Size S. Branch
`f492-knife-edge-closed-form` from main a12b7e1c, worktree `w49i-work/f492`, scratch `w49i-work/f492-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here before the code and the runs and not changed after a run (a miss
becomes an ignored `*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, of the hash tables or of any golden file; no new dependency, no unsafe code; clean-room. F-492 is test code
only: one new test file, `crates/rcfd-fem/tests/si_knife_edge_closed_form.rs`. No library file is changed, and the tests of ADR
0390 in `si_geometric.rs` (the A9 known miss among them) are not edited.

## What F-492 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. The knife-edge Hex20 beam of ADR 0390 (L/h = 20, nu = 0):
> the first two buckling factors within 1 percent of the lower root of (r^2 k^2/(kappa G A)) P^2 - (1 + a + r^2 k^2) P + P_E = 0
> (k = n pi/L, a = P_E/(kappa G A), r^2 = I/A), derived in the ADR; at L/h = 40 the second factor four times the first within 2
> percent.

Clauses: (a1) beam B of ADR 0390 (`L/h = 20`, `nu = 0`, Hex20): factor `n = 1` within 1 percent of the lower root of the quadratic
for `n = 1`; (a2) the same for `n = 2`; (a3) the same beam at `L/h = 40`: `|lambda_2/(4 lambda_1) - 1| <= 0.02`; (a4) the
quadratic derived in this ADR.

Reading of the clauses, registered:

- **Units.** The quadratic is in the axial force `P` (N); a buckling factor `lambda` is dimensionless (the multiple of the reference
  pre-stress `s_ref = -1e6 Pa`). "The factor within 1 percent of the root" is read as `|lambda |s_ref| A / P_n - 1| <= 0.01`, the
  same as comparing `lambda` with `P_n/(|s_ref| A)`.
- **`P_E` is the Euler load of the mode `n`**, `P_E,n = E I k^2 = n^2 pi^2 E I/L^2`, and `a = P_E,n/(kappa G A)` with the same
  `P_E,n` (so `a` grows as `n^2`, as ADR 0390's diagnosis has it, `a = 4.935e-3 n^2`). The derivation (D1) gives exactly this; the
  entry's text "P_E" without the index could be read as `pi^2 E I/L^2` for both modes, which is wrong for `n = 2` by about a factor
  4. Recorded as open question 1 (a re-wording of the note), not a change of a limit.
- "The knife-edge Hex20 beam of ADR 0390" is ADR 0390's set-up B unchanged: `hex_box([0, 0, 0], [L, h, b], [40, 2, 1])` upgraded
  to Hex20, the knife edges `ke0` (pin) and `ke1` (roller) on the neutral axis, `u_z = 0` everywhere, the pre-stress from the
  static solve with the balanced end tractions of `s_ref = -1e6 Pa`, `BucklingOptions::new(2)`.
- "At `L/h = 40`": the same section (`h = b = 0.1 m`) at `L = 4 m`, the mesh `[80, 2, 1]` (the element shape of B unchanged:
  `0.05 x 0.05 x 0.1` before the upgrade), the same supports, loads and options.

## What was read (no run)

- ADR 0390 in full: D1 (the geometric stiffness `K_sigma[(a, i), (b, j)] = delta_ij int N_a,k S_kl N_b,l dV`, the same block on all
  three components), D6 (`buckling::factors`), D7 (set-ups B and C), D8 (identification), the registered checks and the addendum,
  in particular the measured factors of B, `lambda_1 = 408.400`, `lambda_2 = 1600.710`, and the diagnosis of A9.
- `crates/rcfd-fem/tests/si_geometric.rs`: the helpers `Checks`, `box_mesh`, `beam`, `static_prestress`, `correlation`,
  `neutral_line`, and the buckling test of B (its known miss and its met-clauses companion).
- `docs/roadmap.json`, the entries F-395 and F-492.

**Disclosure.** The factors of B at `L/h = 20` were measured by ADR 0390's registered run (the set-up here is the same, so the run
here will reproduce them, bit for bit, unless the library changed in between). Clauses a1 and a2 are therefore known to be met
before this registration (arithmetic 3: 4.3e-5 and 1.7e-4 from the lower root). The `L/h = 40` beam has never been run.

## Design

### D1. The quadratic (first principles)

A column along `x` with a rectangular section, in-plane bending in `y`, with the Timoshenko kinematics `u_x = -y psi(x)`, `u_y =
w(x)` (`y` from the neutral axis, `psi` the rotation of the section, `w` the deflection). The linear strain energy per unit length
is

    (E I psi_x^2 + Q (w_x - psi)^2)/2,   Q = kappa G A.

The solid's initial-stress energy (ADR 0390 D1) for a uniform axial stress `S_xx = s` and no other component is `s int (u_x,x^2 +
u_y,x^2 + u_z,x^2) dV/2` (every displacement component is differentiated along `x`, the `delta_ij` of the block). With the
kinematics above, `int_A u_x,x^2 dA = I psi_x^2` and `int_A u_y,x^2 dA = A w_x^2`; with the compressive force `P = -s A > 0`, per
unit length

    -P (w_x^2 + r^2 psi_x^2)/2,   r^2 = I/A.

The first term is the classical one (Engesser's column has only it); the second is the work of the axial stress on the axial
displacement of the rotating sections. A beam theory derived from the beam's own geometric stiffness keeps only the first; the
solid element has both, which is the cause of ADR 0390's A9 miss.

The knife edges on the neutral axis at `x = 0, L` give `w = 0` and no end moment (`psi_x = 0`, natural); the modes are `w = W sin
kx`, `psi = Psi cos kx`, `k = n pi/L`. Averaged over the length (the common factor `L/4` dropped), the total energy is the
quadratic form of `(W, Psi)` with the matrix

    [ Q k^2 - P k^2        -Q k                       ]
    [ -Q k                 E I k^2 + Q - P r^2 k^2    ].

Neutral stability is a zero determinant: `(Q - P) k^2 (E I k^2 + Q - P r^2 k^2) - Q^2 k^2 = 0`. Divided by `Q k^2` and expanded,

    (r^2 k^2/Q) P^2 - (1 + a + r^2 k^2) P + P_E,n = 0,   P_E,n = E I k^2,   a = P_E,n/Q,

the roadmap's equation with `P_E` read as `P_E,n` (reading above). The lower root (the buckling load of mode `n`) is computed in
the form free of cancellation, `P_n = 2 P_E,n/(c_1 + sqrt(c_1^2 - 4 c_2 P_E,n))`, `c_1 = 1 + a + r^2 k^2`, `c_2 = r^2 k^2/Q`. To
first order `P_n = P_E,n/(1 + a + r^2 k^2)` (relative `O(a r^2 k^2)` from the quadratic term). The upper root is of order `Q/(r^2
k^2) ~ 1e11 N` (a shear mode outside beam theory), far above every factor wanted. With `r^2 k^2 = 0` the quadratic falls to
Engesser's `P_E,n/(1 + a)` (ADR 0390 arithmetic 1).

What the closed form leaves out, of order `a^2`, `a r^2 k^2` and `(r^2 k^2)^2` (`<= 4e-4` for `n = 2` at `L/h = 20`): the
warping of the section (the closed form has plane sections and a shear coefficient; the solid has the parabolic shear
distribution, which `kappa = 5/6` represents to first order only at `nu = 0`), the Engesser-Haringx ambiguity (`O(a^2)`, ADR 0390),
the end regions at the knife edges (the end face free except on the line), and the discretisation (ADR 0390 arithmetic 7, below
1e-3).

### D2. The set-ups

As ADR 0390 D7, with the length a parameter: `E = 2e11 Pa`, `nu = 0`, `rho = 7850 kg/m^3`, `h = b = 0.1 m`, `A = 0.01 m^2`, `I =
8.3333e-6 m^4`, `G = E/2`, `kappa = 5/6`, `r^2 = h^2/12`.

- **B20** (`L = 2 m`, mesh `[40, 2, 1]`): ADR 0390's beam B, copied (not imported: integration tests of one crate do not share
  modules, and `si_geometric.rs` is not edited).
- **B40** (`L = 4 m`, mesh `[80, 2, 1]`): the same with the length doubled. Its `n = 2` has the `k` of B20's `n = 1`, so the same
  closed-form root 4.083826e6 N (arithmetic 2), a consistency the checks use (C4).
- Both: pre-stress from the static solve at `s_ref = -1e6 Pa` (`Prestress::from_static`), `buckling::factors` with
  `BucklingOptions::new(2)`. Mode identification as ADR 0390 D8 (correlation of `u_y` on the neutral line `y = h/2, z = 0` with
  `sin(n pi x/L)`).

### D3. The test file

`crates/rcfd-fem/tests/si_knife_edge_closed_form.rs`, two tests: `a1_a2_c1_c2_n1_knife_edge_beam_l_over_h_20` and
`a3_c1_c2_c3_c4_knife_edge_beam_l_over_h_40`. The closed form is a function `lower_root(length, n) -> f64` (force, N). Each test
builds its beam, solves the static pre-stress and the buckling problem once, and checks with the `Checks` collector of
`si_geometric.rs` (copied). Estimated time in release: B20 about as ADR 0390's buckling test (seconds); B40 twice the unknowns.

## Arithmetic before this registration (disclosed)

By hand and with `arith.py` (closed forms only; `s_ref A = 1e4 N`):

1. `kappa G A = 8.333333e8 N`, `r^2 = 8.3333e-4 m^2`. `L/h = 20`: `n = 1`: `P_E = 4.112335e6 N`, `a = 4.934802e-3`, `r^2 k^2 =
   2.056168e-3`, lower root `P_1 = 4.083826e6 N` (`lambda = 408.3826`), upper root 4.08e11 N; `n = 2`: `P_E,2 = 1.644934e7 N`, `a
   = 1.973921e-2`, `r^2 k^2 = 8.224670e-3`, `P_2 = 1.600433e7 N` (`lambda = 1600.4326`). Engesser alone: 4.092141e6, 1.613093e7
   (the lower roots are 2.03e-3 and 7.85e-3 below). Closed-form ratio `P_2/(4 P_1) - 1 = -2.026e-2` (Engesser alone -1.452e-2).
   The determinant at the computed root vanishes to 1.5e-16 relative (check of the algebra of D1).
2. `L/h = 40`: `n = 1`: `P_E = 1.028084e6 N`, `a = 1.233701e-3`, `r^2 k^2 = 5.140419e-4`, `P_1 = 1.026291e6 N` (`lambda =
   102.6291`); `n = 2`: `P_2 = 4.083826e6 N` (`lambda = 408.3826`, the value of B20's `n = 1`). Ratio `P_2/(4 P_1) - 1 =
   -5.198e-3` (Engesser alone -3.683e-3): 0.52 percent, inside the 2 percent of a3 with 1.48 percent to spare.
3. ADR 0390's measured factors of B20 (`408.400`, `1600.710`) against the lower roots: `+4.3e-5` and `+1.73e-4`; against Engesser
   alone `-1.99e-3` and `-7.68e-3`. Disclosed above; these are what the B20 run is expected to reproduce.
4. Estimate for B40 against the lower roots. The model error of D1 scales with `a` and `r^2 k^2` (as `(h k)^2`): B40's `n = 1` has
   a quarter of B20's `n = 1` terms, B40's `n = 2` the same terms as B20's `n = 1`; the end regions and the knife edges are the
   same in absolute size, relatively half as long. Discretisation: B40's `n = 2` has 40 elements per half wave, as B20's `n = 2`
   (ADR 0390 arithmetic 7: below 1e-3). Estimate `|lambda |s_ref| A/P_n - 1| <= 2e-4` for both modes of B40; the ratio `lambda_2/(4
   lambda_1)` then within `4e-4` of the closed-form ratio, so `|lambda_2/(4 lambda_1) - 1| = 5.2e-3 +- 4e-4`.
5. Run time: B40 has 1533 nodes (B20 773), 4.6e3 unknowns before the constraints; ADR 0390's beam test (three buckling solves)
   ran in seconds in release. Estimate well under 30 s for each test.

## Registered checks

All in `crates/rcfd-fem/tests/si_knife_edge_closed_form.rs`, release profile, run once: `CARGO_BUILD_JOBS=4 cargo test -p rcfd-fem
--release --test si_knife_edge_closed_form -- --nocapture --test-threads=1`. Relative differences are `|x - y|/|y|`; `P_n` is the
lower root of D1 for the length of the set-up.

### A. The acceptance

| Id | Clause | Measure | Limit | Estimate |
| --- | --- | --- | --- | --- |
| A1 | a1, a4 | B20, two factors: `|lambda_1 |s_ref| A / P_1 - 1|`, `P_1 = 4.083826e6 N` | <= 0.01 | 4.3e-5 (arith. 3, known) |
| A2 | a2, a4 | the same run: `|lambda_2 |s_ref| A / P_2 - 1|`, `P_2 = 1.600433e7 N` | <= 0.01 | 1.7e-4 (arith. 3, known) |
| A3 | a3 | B40, two factors: `|lambda_2/(4 lambda_1) - 1|` | <= 0.02 | 5.2e-3 (arith. 2, 4) |

### C. Companions

| Id | Measure | Limit | Estimate |
| --- | --- | --- | --- |
| C1 | B20 and B40, each factor `n = 1, 2`: `|lambda_n |s_ref| A / P_n - 1|` (the sharper form of A1, A2; for B40 the closed form of each factor) | each <= 1e-3 | B20 4.3e-5, 1.7e-4; B40 <= 2e-4 |
| C2 | identification (ADR 0390 D8): the two buckling modes of B20 and of B40 against `sin(n pi x/L)` | each >= 0.99 | > 0.999 |
| C3 | B40: `|lambda_2/(4 lambda_1) - P_2/(4 P_1)|` (the measured ratio against the closed-form ratio 0.994802) | <= 1e-3 | <= 4e-4 (arith. 4) |
| C4 | B40's `lambda_2` against B20's `lambda_1` (the same closed form, arith. 2): `|lambda_2,B40/lambda_1,B20 - 1|` | <= 1e-3 | <= 2e-4 |
| C5 | each run returned two factors, Sturm record counted = expected = 2 | as stated | D6 of ADR 0390 |

C4 needs both beams; it is placed in the B40 test, which then also solves B20 (one more buckling solve; the B20 test is not
called from it).

### N. Negative controls

| Id | Control | Expected |
| --- | --- | --- |
| N1 | C1's measure for B20 against Engesser alone (the quadratic with `r^2 k^2 = 0`, `P_E,n/(1 + a)`) | `> 1e-3` for each of `n = 1, 2` (about 2.0e-3 and 7.7e-3: the sharper check sees the rotating-section term) |
| N2 | the clause of ADR 0390 A9 on B20: `|lambda_2/(4 lambda_1) - 1|` | `> 0.02` (2.0135e-2, ADR 0390: the ratio check at `L/h = 20` is not met by the solid, which is why a3 is asked at `L/h = 40`) |
| N3 | A2's measure with the quadratic read without the mode index (`k = pi/L` in every term, so `P_E = pi^2 E I/L^2` and its `a`, for `n = 2`) | `> 0.01` (2.92: the reading of `P_E` matters) |

### S. Existing tests

| Id | Measure | Limit |
| --- | --- | --- |
| S1 | `cargo test -p rcfd-fem --release --test si_geometric` (the ADR 0390 tests, unchanged; its known miss stays ignored) | 0 failed |
| S2 | `git diff a12b7e1c --stat -- crates`: the new test file only | as stated |

The whole `rcfd-fem` suite is not run (no library line changes; the lead runs the gate).

### Mutations (scratch copy `w49i-work/f492-scratch/copy`, not committed)

| Id | Mutation | Expected to be caught by |
| --- | --- | --- |
| Mu1 | the reference without the rotating-section term (`r^2 k^2 = 0` in `lower_root`: Engesser) | C1 (B20 `n = 1, 2`; B40 `n = 2`) |
| Mu2 | the upper root taken for the lower | A1, A2, C1, C3 |
| Mu3 | `lower_root` evaluated at `k = pi/L` for every `n` (the mode index dropped, as N3) | A2, C1, C3 |
| Mu4 | B40 built with `L = 2 m` and `[80, 2, 1]` (the length not changed) | A3 (ratio near 2 percent), C1, C3 |
| Mu5 | library, `geometric.rs`: the block of `K_sigma` put on components 1 and 2 only (the `u_x` block dropped, removing the rotating-section term) | C1 (B20, about 2e-3 and 8e-3) |

## Decision rule

- **done**: A1 to A3 met, every C and S check met, the negative controls as expected.
- **partial**: any A check missed with a diagnosed cause, kept as a `*_known_miss` record; or a C or S check missed.
- **todo**: the checks could not be run.

## Limits of this registration

- One section, `nu = 0`, knife edges on the neutral axis, Hex20 only; no convergence study.
- The closed form is first order in the beam corrections; the solid's section warping and the end regions are not modelled
  (`O(a^2)`, below every limit here).
- `L/h = 20` reproduces a run already made (disclosed); the new evidence is `L/h = 40` and the sharper companion C1.
- No library change; the ADR 0390 record `a5b_a8_a9_c2_c3_n1_buckling_of_the_beam_known_miss` stays as it is (its limit is the
  roadmap's old clause and is still missed; N2 records that).
