# ADR 0597: F-673 registration: load height on the pre-stressed modes of a monosymmetric warping beam

Status: **REGISTERED 2026-11-15, written before any code of F-673 and before any run of its checks.** Not yet accepted by the
maintainer. Before
this text nothing of F-673 was written, compiled or run in the repository: the files listed under "What was read" were read, and
the arithmetic below was done with two plain numpy scripts in the scratch folder (`w49i-work/f673-scratch/arith.py`, log
`arith.log`, and `scan.py`, a scan of the reference in the load factor: the ADR 0553 section and the sine-series Rayleigh-Ritz
energy of ADR 0588 re-typed, with the kinetic energy of D2 derived here; its convergence in the number of terms; an independent
cubic-Hermite Euler-Bernoulli Vlasov model of the same energy with the same consistent mass; the coupled Timoshenko-Rayleigh
pinned frequency in closed form; the mass variants of the negative controls; no rcfd code; disclosed, its numbers quoted below).
Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-673 (deps F-670, F-627), structural solver, advanced tier, the follow-up proposed by
[ADR 0594](0594-f670-registration-load-height-on-the-pre-stressed-warping-modes-and-harmonic-path.md) (its "Limits": the
monosymmetric I couples the lateral translation of the centroid with the twist in the mass, not checked there). The section is
that of [ADR 0553](0553-f627-registration-monosymmetric-thin-walled-sections-for-the-warping-element.md), the distributed
load-height term and the buckling references those of
[ADR 0588](0588-f663-registration-distributed-loads-at-a-height-on-warping-beams.md), the pre-stressed modes those of ADR 0548,
the use of the entry on the modes (its force the applied load) that of ADR 0594 D1. Number 0597 given by the lead. Branch
`f673-work` from the local main tip c42de6ed (which carries F-635, F-643, F-661, F-663, F-667, F-668 and F-670), worktree
`w49i-work/f673`, scratch `w49i-work/f673-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test edited;
no new dependency, no unsafe code, the crate graph unchanged; clean-room. Every choice below that the maintainer would normally
be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation), except the open
questions at the end.

## What F-673 asks

The acceptance, binding, verbatim (`docs/roadmap.json`):

> Every limit not stated here is registered in the ADR before the run. The lowest flexural-torsional frequency of the
> fork-supported ADR 0553 monosymmetric I under a uniformly distributed load of 0.5 of its ADR 0588 buckling load at the shear
> centre, at the centroid and on flange 1 (prestressed_modes) equals a Rayleigh-Ritz reference with the consistent mass,
> including the inertial coupling of the shear-centre offset, derived in the ADR within 1e-2 at 16 elements; results without a
> load height bit-identical.

Clauses: **a0** every limit registered here; **a1** the lowest flexural-torsional frequency by `prestressed_modes` at `0.5` of
the ADR 0588 buckling load, at the shear centre, at the centroid and on flange 1, within `1e-2` of the Rayleigh-Ritz reference
of D3 at 16 elements; **a2** the reference has the consistent mass with the inertial coupling of the shear-centre offset, derived
here (D2); **a3** results without a load height bit-identical.

### Audit (done before this text): is anything to be built?

No, by reading. The mass of a warping element is `local_mass` in the centroid unknowns of the nodes, not transformed by the
offset (`mass_aug`; ADR 0553 D1: "the mass is exact in the centroid unknowns"); the stiffness and the moment part of `K_sigma`
are the shear-centre forms transformed to the centroid unknowns (`section_local_stiffness`, `section_local_geometric_stiffness`);
the line-load term of ADR 0588 is in `K_sigma` of `matrices_impl` with each element's own shear centre; `prestressed_modes` ->
`modes_impl` forms `K + K_sigma` and `mass_aug`. So in the shear-centre unknowns of the reference the library's mass carries the
coupling `rho A z_0 v_s phi` (D2), and **F-673 is test-only** (as the brief prefers): no library line changes (D6).

### Readings, registered

- **"The fork-supported ADR 0553 monosymmetric I"**: `monosymmetric_i(0.15, 0.012, 0.08, 0.008, 0.2, 0.006, 5/6)` (flange 1 on
  `+e_3`), `L = 3` along `x`, 16 Beam2 elements, orientation `y`, steel `E = 2e11`, `nu = 0.3`, `rho = 7850`, `u = 0` at node 0,
  `v = w = phi = 0` at both ends, warping free; node sets `x0`, `xl`, `mid` (node 8), element set `all` (the fork of ADR 0588 and
  ADR 0594 with this section).
- **"A uniformly distributed load of 0.5 of its ADR 0588 buckling load"**: downward, as the ADR 0588 cases at the centroid and on
  flange 1 (`MS-*-down`, flange 1 compressed; the only direction ADR 0588 has for those two), the load `q = -0.5 lambda_ref q_ref
  e_3`, `q_ref = 1000` N/m, `lambda_ref` the ADR 0588 Rayleigh-Ritz factor of the case (shear centre `135.747863965`, centroid
  `163.396623703`, flange 1 `126.453805543`): a physical load, the same for the model and the reference (ADR 0594 readings).
- **"At the shear centre, at the centroid, on flange 1"** (suffixes S, C, F): no entry; the entry `line_load_point("all", q,
  (0, 0))`; the entry at `(0, 0.0718681318681)` (flange 1 mid-plane, the `MS_TOP` of `sa_beam_warping_f663.rs`); each entry's
  force the applied `q` (ADR 0594 D1).
- **The pre-stress**: the warping static solve (`solve_static`) of a copy of the fork model with `Load::line_load("all", q)`,
  `WarpingPrestress::from_static(.., [0, 0])` (ADR 0588 A7 measured it within `1.4e-15` of the six-unknown pre-stress for the
  monosymmetric I at the centroid).
- **"prestressed_modes"**: `prestressed_modes(&fork, &pre, &w, &ModalOptions::new(1))`, the frequency `sqrt(eigenvalues[0])`.
  The lowest mode is the coupled lateral-torsional one (arithmetic 3: the second eigenvalue is `1.9` to `2.9` times the first, the
  vertical bending `2.2e5`).
- **"Results without a load height bit-identical"**: no library code line changes (D6); the existing bit records and the tests of
  the beam-warping path run unchanged (B0); an entry at the shear centre `(0, z_0)` gives the bits of no entry on the modes (B1).

## What was read (no run)

- `AGENT_RULES.md`; the brief; ADR 0594 (whole), ADR 0588 (whole), ADR 0553 (head to D5), ADR 0582 (by its use in 0588).
- `crates/rcfd-fem/src/beam_warping.rs`: the module doc, `WarpingSection::monosymmetric_i`, `Warping::line_load_point`,
  `offset_transform`, `shear_centre_end_forces`, `section_local_stiffness`, `augmented`, `stiffness_aug`, `mass_aug`,
  `local_mass`, `hermite_mass_block`.
- `crates/rcfd-fem/src/beam.rs`: `local_mass` (the two Timoshenko planes by `plane_mass` with `shear_y() G A`, `shear_z() G A`).
- Tests: `sa_beam_warping_f670.rs` (whole: the fork, the pre-stress, the checks helper), `sa_beam_warping_f663.rs` (the
  monosymmetric set-up, `MS_TOP`); the ADR 0588 and ADR 0594 scripts (`f663-scratch/arith.py`, `f670-scratch/arith.py`).

## Design

### D1. Use of the entry (no change)

As ADR 0594 D1: the operator of the modes is `K + K_sigma(pre) - omega^2 M`, `K_sigma(pre)` including `c M_H` with `c = q . (p -
s)` of the entry, `s = (0, z_0)` the element's shear centre; the entry's `q` is the applied load. For the centroid `c = -q_z z_0
= +52.58 lambda` (N), for flange 1 `c = q_z (0.07187 - z_0) = -19.29 lambda` (ADR 0588 D4).

### D2. The kinetic energy with the shear-centre offset (derived)

Section axes from the centroid, the shear centre at `(0, z_0)`, `z_0 = 5.2580303363e-2` (ADR 0553 D5). The section turns rigidly
in its plane by `phi` about the axis through `S` and the point `S` moves laterally by `v_s` (ADR 0553 D1): a point `(y, z)` moves
by `v_P = v_s - (z - z_0) phi`, `w_P = w_s + y phi`. Its in-plane kinetic energy per length is

`(rho/2) int (v_P,t^2 + w_P,t^2) dA = (rho/2) [A v_s,t^2 + 2 v_s,t phi_t int (z_0 - z) dA + phi_t^2 int ((z - z_0)^2 + y^2) dA
+ ...]`

and with `int z dA = int y dA = 0` (centroidal axes): `(rho/2) [A v_s,t^2 + 2 A z_0 v_s,t phi_t + (I_p + A z_0^2) phi_t^2] =
(rho/2) [A (v_s + z_0 phi)_t^2 + I_p phi_t^2]`, `I_p = I_y + I_z` about the centroid: **the translation of the centroid `v_c =
v_s + z_0 phi` and the rotation, uncoupled; in the shear-centre unknowns of the reference the coupling `rho A z_0 v_s,t phi_t`
and the polar term `rho (I_p + A z_0^2)` about `S`**. The axial motion `u_P = -y v_s' - omega_S phi'` (ADR 0553 D1; the vertical
plane does not couple, `y_0 = 0`) gives the rotary term `(rho/2) int (y v_s,t' + omega_S phi_t')^2 dA = (rho/2)(I_z v_s,t'^2 +
C_w phi_t'^2)` (`int y omega_S dA = 0`, the definition of `S`); the library has no warping inertia (ADR 0548 D4), so the
reference keeps `rho I_z v_s'^2` and drops `rho C_w phi'^2`. In the library the rotary term is `rho I_z theta_z^2` with the node
rotation `theta_z`; the Timoshenko shear strain of the x-y plane is that of `S` (ADR 0553 D1), so `theta_z -> v_s'` in the
Euler-Bernoulli limit: the same term. The reference kinetic energy:

`T_ref = (1/2) int (rho A (v_s + z_0 phi)_t^2 + rho I_z v_s,t'^2 + rho I_p phi_t^2) dx`.

This is the consistent discretisation of the library's mass (`local_mass` in centroid unknowns, ADR 0548 D4) read in
shear-centre unknowns. In the sine basis of D3 each block is diagonal: `M_vv[n] = (rho A + rho I_z k_n^2) L/2`, `M_vphi[n] =
rho A z_0 L/2`, `M_phiphi[n] = rho (I_p + A z_0^2) L/2`. (Had the rotary term been read on `v_c'`, the frequency would move by
`+2.2e-4`, arithmetic 3; recorded, not used.)

### D3. The reference (Rayleigh-Ritz, derived)

The energy of ADR 0588 D3 (shear-centre unknowns, Euler-Bernoulli, forks, the Wagner coefficient about the shear centre
`beta_x = -1.5259215331e-1`) at the load `lambda q_ref` with the kinetic energy of D2. Hamilton's principle on `v_s = sum a_n
sin(k_n x) e^{i omega t}`, `phi = sum b_n sin(k_n x) e^{i omega t}`, `k_n = n pi/L`, `n = 1 .. N` gives `(K + lambda G) x =
omega^2 M x`, `K`, `G` those of ADR 0588 D4 (400-point Gauss-Legendre; `M(x) = (q_z/2) x (L - x)`, `q_z = -q_ref`; `G` holds `int
M sin_m (-k_n^2 sin_n)`, `beta_x int M k_m k_n cos_m cos_n` and `(q . r) int sin_m sin_n`), `M` of D2. The reference is the lowest
`omega^2` at `N = 81` and `lambda = 0.5 lambda_ref` (converged: `N = 41` against `81` within `4.3e-9`). The mode ratio is that of
the model's components, `|phi(L/2)/v_c(L/2)|`, `v_c = v_s + z_0 phi` (the nodes are at the centroid).

| case | `c` per unit `lambda` (N) | `omega_ref^2` | `omega_ref` (rad/s) | `omega^2/omega_0^2` | `|phi/v_c|` at `L/2` (rad/m) |
|---|---|---|---|---|---|
| S (shear centre) | `0` | `2.782195293739e4` | **`166.799139498357`** | `1.1897` | `3.784772413` |
| C (centroid) | `+52.580303` | `2.603520849063e4` | **`161.354294924654`** | `1.1133` | `3.636579706` |
| F (flange 1) | `-19.287829` | `2.852178843833e4` | **`168.883949617271`** | `1.2196` | `3.746893414` |

`omega_0^2 = 2.338537032326e4` (unloaded, equal to the 2 x 2 coupled closed form of the first sine to `2e-16`). The lowest
eigenvalue at `lambda_ref` is `<= 3e-11 omega_0^2` for every case (the ADR 0588 buckling load is the zero of the reference's
frequency).

**Observation (recorded, not a check).** At half the buckling load the lowest frequency is *above* the unloaded one (scan.py):
the lowest branch rises from `omega_0^2` to about `1.31 omega_0^2` near `0.3 lambda_ref` and then falls to zero at `lambda_ref`;
its twist-to-lateral ratio `phi/v_s` goes from `+6.43` (unloaded) through zero near `0.3` to `-5.94` at `lambda_ref` (case S).
The unloaded coupled mode has the sense of rotation the moment stiffens; the buckling mode the opposite one, so the branch
changes character. The second eigenvalue stays apart (`5.9e4`, `7.6e4`, `5.4e4` at `0.5`), so the lowest mode at `0.5` is that
of the branch that goes to zero.

### D4. Supporting checks near the buckling load (not acceptance)

The measure of ADR 0594 D5: `z = |omega^2(lambda)|/omega_0^2`, `omega_0^2` the lowest eigenvalue of `modal_analysis` of the
unloaded model, `omega^2(lambda)` the lowest eigenvalue of `prestressed_modes` with `ModalOptions::new(1).shift(Shift::Value(0.5
omega_0^2))` at the load `lambda q_ref` (entry and pre-stress at that load): at `lambda_cr`, the lowest factor of
`buckling_factors_with(&fork, &pre_ref, &w_ref, &BucklingOptions::new(3))` at `q_ref` (Z), and at `lambda_ref` (Y).

### D5. The unloaded mass check (closed form, derived)

For the unloaded fork beam the first sine separates exactly also for the Timoshenko element: `v_s = a sin(kx)`, `theta_z = c
cos(kx)`, `phi = b sin(kx)`, `k = pi/L`, with the strain energy `E I_z theta_z'^2 + kappa G A (v_s' - theta_z)^2 + G J phi'^2 + E
C_w phi''^2` and the kinetic energy of D2 (with `theta_z` in the rotary term), a 3 x 3 frequency equation. Its lowest root:
**`omega_T = 152.755814868645` rad/s** (`omega_T^2 = 2.333433897618e4`, `-2.18e-3` from the Euler-Bernoulli value; the shear
parameter `E I_z k^2/(kappa G A) = 3.5e-3`).

### D6. What changes

`crates/rcfd-fem/tests/sa_beam_warping_f673.rs` (new); a short F-673 section in `docs/reference-data.md`. No library file.

## Arithmetic done before the code (disclosed; `arith.log`)

1. The ADR 0588 monosymmetric references re-derived through the re-typed functions: within `1.9e-12` of the registered values.
2. Unloaded: the coupled Euler-Bernoulli first-sine frequency `omega^2 = 2.338537e4` (second `7.19e4`; lateral alone `3.13e4`,
   torsion alone `5.38e4`: the offset couples them strongly); the Timoshenko closed form of D5; with the mass at the shear centre
   (`rho A v_s^2 + rho I_p phi^2`) or with the coupling term dropped, `omega_T` would be `+15.6%` higher; with the coupling of the
   opposite sign it is the same (the unloaded problem is symmetric in the sign of `z_0`); with the rotary inertia dropped,
   `+2.6e-4`. The vertical bending `2.25e5`.
3. The table of D3; convergence `N = 5 .. 81`; mass variants at `0.5 lambda_ref`: the coupling dropped `-21.5%`, `-19.6%`,
   `-22.2%` (S, C, F); the mass at the shear centre `-19.1%`, `-17.6%`, `-19.6%`; the coupling of the opposite sign `-34.7%`,
   `-32.3%`, `-35.6%`.
4. The Hermite Euler-Bernoulli FE of the same energy and the same coupled consistent mass, 16 elements, the library's per-element
   moment (ADR 0588 D3): `omega` `+1.5e-6` to `+1.8e-6` from the reference (8 elements `+2.3e-5` to `+2.8e-5`). **Estimate for
   the rcfd model at 16 elements: `omega` low by about `1.2e-3`** (the Timoshenko lateral plane, `-2.18e-3` in the unloaded
   `omega^2`, half in `omega`; and the model's buckling factor `3e-4` low, ADR 0588 A4, times the sensitivity `d ln omega^2/d ln
   lambda = -0.66`, `-0.76`, `-0.62` at `0.5`: about `-1e-4` in `omega`), discretisation `2e-6`.
5. At `lambda_ref (1 - 3e-4)` the reference has `omega^2/omega_0^2 = 8.1e-4` to `8.6e-4`: **estimate for Y about `8e-4`**; for Z
   (self-consistent) `1e-8`.
6. Negative controls at `0.5 lambda_ref`: C without the entry `-5.23e-2`, F without it `+8.1e-3`; C entry with the wrong sign
   `-1.72e-1`, F `+1.40e-2`; C entry doubled `+2.6e-2`, F `-1.2e-2`; the shear centre not subtracted (`r = p`): C `-5.23e-2`
   (`c = 0`), F `-4.84e-2`; the Wagner coefficient about the centroid in place of `beta_x`: `-5.9e-2`, `-6.1e-2`, `-5.8e-2`.
   Ratios `omega_C/omega_S = 0.967356879`, `omega_F/omega_S = 1.012498926`. Reference frequencies with the coupling dropped
   (`rho A z_0^2` kept): `130.8703296103`, `129.7436614651`, `131.3414946757`; with the mass at the shear centre:
   `134.9833059340`, `132.9134771139`, `135.8571297984`.
7. The mode ratio: Timoshenko against Euler-Bernoulli in the unloaded closed form `3.9e-3`; at `0.5` the ratio changes fast with
   the load (`d ln |phi/v_s|/d ln lambda` about `2`), so the model's `3e-4` lower buckling factor adds about `6e-4`. **Estimate
   for S `5e-3`.**

## Checks (limits fixed now)

Every aggregated error is asserted finite before it is compared (a non-finite value is a miss). The set-up of the readings at 16
elements; "the case" runs S, C and F.

| id | check | limit | estimate |
|---|---|---|---|
| B0 | `sa_beam_warping_f598_bits`, `_f626_bits`, `_f635_bits`, `_f661_bits`, `_f663_bits`, `_f667_bits`, `_f627`, `_f663`, `_f670` pass unchanged | pass | pass |
| B1 | **acceptance a3**: case S with an entry at `(0, z_0)` (`z_0 = shear_centre()[1]`) against no entry: the `prestressed_modes` eigenvalue and every component of mode 1 bit-equal | bit-equal | bit-equal |
| M0 | **acceptance a2 (the mass)**: `omega_0` of `modal_analysis` (unloaded) against `omega_T` of D5 | `<= 1e-4` relative | `5e-6` |
| A-S/C/F | **acceptance a1**: `|sqrt(eigenvalues[0])/omega_ref - 1|` at `0.5 lambda_ref` | `<= 1e-2` | `1.2e-3` |
| S-S/C/F | the mode: `|phi(L/2)/v(L/2)|` of mode 1 (components 3 and 1 of node 8) against D3 | `<= 3e-2` relative | `5e-3` |
| N3 | `omega_C/omega_S` against `0.967356879` and `omega_F/omega_S` against `1.012498926` | `<= 2e-3` relative each | `5e-4` |
| N1 | negative control: C without the entry against the C reference `> 2e-2` (`5.2e-2`); F without it against the F reference `> 4e-3` (`7e-3`) | as stated | as stated |
| N4 | negative control (a2, the inertial coupling): each case's model `omega` against the reference with the coupling dropped and against the reference with the mass at the shear centre (arithmetic 6) | `> 1e-1` each | `0.27`, `0.23` |
| Z-S/C/F | supporting: `z` at the model's `lambda_cr` (D4) | `<= 1e-2` | `1e-8` |
| Y-S/C/F | supporting: `z` at `lambda_ref` (D4) | `<= 1e-2` | `8e-4` |

Negative controls: N1 (the term acts on the modes at its size), N4 (the acceptance check sees the inertial coupling: without it
the frequency is about 20% away, twenty times the limit), N3 (with a tight limit: the flange-1 term is `8e-3` in `omega`, inside
the acceptance limit, so N3 and N1-F are the checks that see it), the mutations X1 to X7. Z is self-consistent and cannot see a
term missing on both the buckling and the modes path.

## Planned mutations (on a scratch copy, one at a time, own `CARGO_TARGET_DIR`)

| id | mutation | expected caught by |
|---|---|---|
| X1 | the mass placed at the shear centre: in `mass_aug` the warping element's `local_mass` taken as `offset_transform(local_mass, shear_centre)` (diagonal in the shear-centre unknowns) | M0, A-S/C/F, N4 |
| X2 | the coupling with the wrong sign: `offset_transform(local_mass, [0, 2 z_0])` (`v_c = v_s - z_0 phi`) | A-S/C/F, S (not M0: the unloaded problem does not see the sign) |
| X3 | `add_line_load_points` not called in `matrices_impl` | A-C, N3, Y-C/F (not Z: consistent; not N1, which runs without the entry) |
| X4 | the line-load term with the wrong sign | A-C, A-F, N3 |
| X5 | the centroidal Wagner coefficient in place of `beta_x` (the `- 2 z_0` dropped in `section_local_geometric_stiffness`) | A-S/C/F, Y (not Z: consistent) |
| X6 | the rotary inertia of the plane x-y dropped (`plane_mass` with `m_i = 0` for that plane) | M0 |
| X7 | the shear centre not subtracted in the line-load term (`r = p`) | A-C, N3, Y-C/F |

## Decision rule

**done**: every registered check (B0, B1, M0, A, S, N1, N3, N4, Z, Y) met in the one run and every mutation caught by at least
one check. **partial**: any registered check missed (a `*_known_miss` record), any mutation not caught, or any check met only
after a post-run amendment. **todo**: A missed for any case, or B0/B1 missed.

## Limits (not covered)

- The monosymmetric I of ADR 0553, downward, simply supported (forks), a UDL: other sections, supports, loads, the upward load
  (flange 1 in tension) are not checked.
- The pre-stressed harmonic and transient paths with this section and a load height (they take the same `matrices_impl` and
  `mass_aug`; ADR 0594 checked the harmonic resonance for the doubly symmetric I).
- No warping inertia (`rho C_w phi'^2`, as the library); the reference is Euler-Bernoulli against a shear-flexible element.
- The force restated on the entry, not tied to the pre-stress (ADR 0594 Q1).
- No case-file key (F-625).

## Open questions (none changes a default)

- None new. ADR 0594 Q1 (tie the entry's force to the pre-stress) stays as recommended there: (a) restated and documented now,
  (c) read from the static model with a case-file key (F-625).

## Amendment 1 (2026-11-15): a diagnostic of the M0 miss, registered before its run

Written after the one registered run (`f673-scratch/run1.log`) and before the run of the diagnostic below. **M0 missed**:
`omega_0` of `modal_analysis` is `5.22e-4` above `omega_T` (limit `1e-4`, estimate `5e-6`); every other check of the run was
met. M0 becomes the ignored record `m0_unloaded_coupled_mass_known_miss` with its limit intact; the B1 part of the same test
function, met, is kept as its own test `b1_shear_centre_entry` (the same code). No limit is changed.

**Diagnosis (by reading and one numpy emulation, `f673-scratch/diag_mass.py`, done after the run).** The estimate `5e-6`
assumed that the element's mass is the consistent discretisation of `T_ref` of D2 on the element's own fields. It is not, for a
section with an offset: `local_mass` interpolates the centroid translation from the nodal pair `(v_c, theta_z)` with the bending
shape functions of the plane (`plane_mass`), while the stiffness's fields are `v_s(x)` from `(v_s, theta_z)` and `phi(x)` from the
Hermite `(phi, chi)`. The nodal slope of the centroid line is `v_c' = v_s' + z_0 chi`, but the mass reads `theta_z` (about `v_s'`)
as that slope, so its `v_c(x)` is not `v_s(x) + z_0 phi(x)` inside the element: the coupling `rho A z_0 v_s phi` is integrated
with a twist field that has lost its nodal slopes. This is an `O(l^2)` error of the mass, not of D2, and it vanishes for a
section without an offset (which is why ADR 0594 M0 met `4e-6`). An Euler-Bernoulli Hermite emulation of exactly this
interpolation (nodal `(v_c, theta, phi, chi)`, stiffness in `v_s = v_c - z_0 phi`, mass on the `(v_c, theta)` Hermite) gives
`omega_0` against the closed form `+2.15e-3` (8 elements), **`+5.38e-4` (16)**, `+1.35e-4` (32), `+3.4e-5` (64), the ratio `0.25`
per halving; with `v_c(x) = v_s(x) + z_0 phi(x)` it gives `+7.4e-7` at 16. The measured `+5.22e-4` matches the emulation at 16
elements within `3%`.

**Diagnostic D-M0 (registered now, run once after this commit).** `e_n = omega_0(n)/omega_T - 1` of `modal_analysis` of the fork
beam with `n = 16, 32, 64` elements (same section, supports and mass; `omega_T` of D5):

| id | check | limit | estimate |
|---|---|---|---|
| DM-1 | `e_32/e_16` | in `[0.15, 0.35]` | `0.25` |
| DM-2 | `e_64/e_32` | in `[0.15, 0.35]` | `0.25` |
| DM-3 | `|e_64|` | `<= 1e-4` | `3.5e-5` |

DM-1 to DM-3 met confirm the diagnosis (an `O(l^2)` mass discretisation that converges to the coupled closed form of D5, so the
derived kinetic energy of D2 is the one the library converges to); they do not turn M0 into a met check. The decision rule is
unchanged: with M0 missed the proposed status is **partial**.

## Addendum 1 (2026-11-15): results

Written after the one registered run of `tests/sa_beam_warping_f673.rs` (release, `f673-scratch/run1.log`) and the one run of
the diagnostic of amendment 1 (`run2_dm.log`, which also ran `b1_shear_centre_entry`, the unchanged B1 code split out of the
missed test function). Commits: db1c2f82 (this registration), f87349f2 (amendment 1, before the diagnostic run), c41740c5 (the
test file: the registered checks, M0 as the ignored record `m0_unloaded_coupled_mass_known_miss` with its limit intact, the
diagnostic `dm_mass_convergence`). No library file changed. `cargo fmt -p rcfd-fem` was applied before the test commit (layout
only). No limit was changed, nothing re-run to pick a result. Logs in `w49i-work/f673-scratch`: `run1.log`, `run2_dm.log`,
`b0.log`, `fmt.log`, `clippy.log`, `docs_site.log`, `mutations.log`, `mut_*.log`; `arith.py`, `arith.log`, `scan.py` (before
any code); `diag_mass.py`, `diag_loaded.py` (after the run, diagnosis).

### Results against the registered limits

| id | measured | limit | verdict |
|---|---|---|---|
| B0 | `sa_beam_warping_f598_bits`, `_f626_bits`, `_f635_bits`, `_f661_bits`, `_f663_bits`, `_f667_bits` 1 each; `_f627` 7 passed 2 ignored, `_f663` 5, `_f670` 3 | pass | met |
| B1 (a3) | entry at `(0, z_0)` against none: eigenvalue `2.7696104757610396e4` both, all 119 components of mode 1 bit-equal | bit-equal | met |
| M0 | `omega_0` against `omega_T`: **`5.22e-4`** (`omega_0^2 = 23358.701`) | `1e-4` | **missed** (known miss, amendment 1) |
| A-S / C / F (a1) | `2.264e-3` / `2.263e-3` / `2.259e-3` (model low; `omega = 166.4215`, `160.9891`, `168.5024` rad/s) | `1e-2` | met |
| S-S / C / F | `1.98e-4` / `8.5e-4` / `3.8e-4` | `3e-2` | met |
| N3 | C/S `8.0e-7`; F/S `4.9e-6` | `2e-3` | met |
| N1 | C without the entry `5.46e-2`; F without it `5.9e-3` | `> 2e-2`; `> 4e-3` | met |
| N4 | against the reference without the coupling `0.272` / `0.241` / `0.283`; with the mass at the shear centre `0.233` / `0.211` / `0.240` | `> 1e-1` | met |
| Z-S / C / F | `lambda_cr` `135.706589742`, `163.340623671`, `126.417486944` (ADR 0588 measured `-3.04e-4`, `-3.43e-4`, `-2.87e-4`); `z` `5.9e-13` / `2.1e-13` / `7.2e-13` | `1e-2` | met |
| Y-S / C / F | `8.29e-4` / `8.56e-4` / `8.07e-4` (eigenvalue negative) | `1e-2` | met |
| DM-1 / DM-2 / DM-3 (amendment 1) | `e_16 = 5.219e-4`, `e_32 = 1.296e-4`, `e_64 = 3.24e-5`: ratios `0.2484`, `0.2500`; `|e_64| = 3.2e-5` | `[0.15, 0.35]`; `1e-4` | met |

**M0 diagnosed** (amendment 1): the element mass interpolates the centroid translation from the nodal `(v_c, theta_z)` with the
bending shape functions, so for a section with an offset the coupling `rho A z_0 v_s phi` sees a twist field without its nodal
slopes: an `O(l^2)` mass error, `+5.2e-4` at 16 elements, converging at the ratio `0.25` per halving to the coupled closed form
of D5. The emulation of `diag_mass.py` predicted `+5.38e-4` at 16. The derived kinetic energy D2 is the one the library
converges to; the limit `1e-4` was set from an estimate (`5e-6`) that assumed a consistent interpolation.

**A larger than estimated** (`2.26e-3` against `1.2e-3`, inside the limit): the same mass interpolation, evaluated at the loaded
state by the emulation of `diag_loaded.py`, lowers `omega` by `4.6e-4` at 16 elements (`1.2e-4` at 32; at half the buckling load
the mode has the opposite twist sense, so the sign is opposite to the unloaded one). With the Timoshenko lateral plane (about
`-1.1e-3`, from the unloaded closed form) and the model's lower buckling factor (`-1e-4`) this accounts for about `-1.7e-3`; the
rest (`-5e-4`) is plausibly the shear-flexible plane under load, not diagnosed further (no limit depends on it). The offsets
cancel in the ratios N3 to `5e-6`.

### Mutations (scratch copy `f673-scratch/mut` from `git archive` of c41740c5, own target directory, one at a time; the unmutated copy passes)

| id | mutation | caught by |
|---|---|---|
| X1 | the mass at the shear centre | A-S/C/F (`0.18` to `0.20`), S, N3, N4, DM-1 to DM-3 (M0 is ignored) |
| X2 | the coupling with the wrong sign | A-S/C/F (`0.32` to `0.36`), S, N3 (not DM, as registered for M0) |
| X3 | `add_line_load_points` not called | A-C, S-C/F, N3, then Y-C (the modes refuse beyond the buckling load) |
| X4 | the line-load term with the wrong sign | A-C, A-F (`1.18e-2`), S-C/F, N3, N4-C, then Y |
| X5 | the centroidal Wagner coefficient | A-S/C/F (`6e-2`), S, N3 (C/S `2.06e-3`), then Y |
| X6 | the rotary inertia of the plane x-y dropped | DM-1 to DM-3 (`|e_64| = 3.0e-4`; M0 is ignored) |
| X7 | the shear centre not subtracted in the line-load term | A-C, A-F, S-C/F, N3, B1, then Y |

Seven of seven caught. Deviation from the registration: X1 and X6 were to be caught by M0, which became an ignored record; they
are caught by the diagnostic of amendment 1 (X1 also by A, S, N4), registered after the run.

### Other runs

`cargo fmt -p rcfd-fem --check`: clean. `cargo clippy -p rcfd-fem --release --all-targets -- -D warnings`: clean. `cargo test -p
rcfd-cli --release --test docs_site` for the links of this ADR and of `docs/reference-data.md` (a new F-673 section): passed
(`docs_site.log`). Not run: `baseline_hashes` (no library file changed; rule 3 asks it only for a changed solver file), the whole
`rcfd-fem` suite and the workspace (the gate of the lead).

### Decision by the registered rule

**partial**: one registered check missed (M0, a `*_known_miss` record, diagnosed), every other check met in the one run, seven of
seven mutations caught. By clause: a1 met for the three load heights (`<= 2.26e-3` against `1e-2`); a2 the reference has the
derived coupled mass, and N4 shows the acceptance check sees it (without the coupling the frequency is `21` to `28%` away); a3
met (no library line changed; B0, B1). The miss is of a supporting mass check whose limit was tighter than the library's mass
discretisation for an offset section; whether the item is done in substance is the maintainer's call.

### Remaining questions

- **Q1. The consistent interpolation of the offset mass.** Options: (a) leave the mass as it is (centroid translation from
  `(v_c, theta_z)`, converges at `O(l^2)`, `5e-4` at 16 elements); (b) build the mass of an offset section in shear-centre
  unknowns, `T` of D2 with `v_c(x) = v_s(x) + z_0 phi(x)` on the element's own fields, transformed by `offset_transform` (the
  emulation: `7e-7` at 16 elements). (b) changes the modal, harmonic and transient results of every existing model with a
  shear-centre offset, so it is not taken; open for the maintainer, recommendation (b) as a follow-up with a bit record of the
  paths without an offset (unchanged, `z_0 = 0`).
- ADR 0594 Q1 (the entry's force restated): unchanged, recommendation (a) now, (c) with a case-file key (F-625); taken on the
  maintainer's standing instruction of 2026-11-11 (take the recommendation).

## For the lead: roadmap entries

> Proposed status: partial.

> F-673 note: Structural solver, advanced tier. Follow-up of F-670 and F-627 (ADR 0594, ADR 0553, 2026-11-15). PARTIAL. ADR 0597, 2026-11-15: test-only (no library change): the distributed load height of `Warping::line_load_point` on `prestressed_modes` of the fork-supported ADR 0553 monosymmetric I (L = 3, 16 elements, downward UDL of 0.5 of the ADR 0588 buckling load at the shear centre, at the centroid and on flange 1). The kinetic energy of the offset section is derived (in shear-centre unknowns rho A (v_s + z_0 phi)^2 + rho I_z v_s'^2 + rho I_p phi^2, the library's centroid mass) and the Rayleigh-Ritz reference built on it (sine series, converged to 4.3e-9); at half the buckling load the lowest frequency is above the unloaded one (the branch changes its twist sense on the way to zero). Measured: 2.26e-3 below the references for the three heights (limit 1e-2); without the inertial coupling the reference would be 21 to 28 percent away; ratios between heights 5e-6; mode shape 8.5e-4; at the model's buckling load omega^2/omega_0^2 <= 7.2e-13, at the ADR 0588 one 8.6e-4; an entry at the shear centre gives the bits of none. Missed: the unloaded frequency against the coupled Timoshenko closed form, 5.2e-4 against the limit 1e-4 (a known-miss record), diagnosed as an O(l^2) error of the element mass for an offset section (the centroid translation is interpolated from (v_c, theta_z), so the coupling sees a twist without its nodal slopes): 1.3e-4 at 32 and 3.2e-5 at 64 elements, ratio 0.25 per halving. Seven of seven mutations caught. Not covered: the harmonic and transient paths with this section and a load height, the upward load, other supports and loads, warping inertia, a consistent offset mass (ADR 0597 Q1, open: it changes existing results), no case-file key. Follow-up proposed: Consistent mass of warping sections with a shear-centre offset.

> Follow-up proposed: Consistent mass of warping sections with a shear-centre offset. Deps F-673. Acceptance: Every limit not stated here is registered in the ADR before the run. The mass of a warping element with a shear-centre offset is the consistent discretisation of the kinetic energy of ADR 0597 D2 on the element's own fields (v_c = v_s + z_0 phi inside the element); the lowest unloaded frequency of the fork-supported ADR 0553 monosymmetric I equals the coupled Timoshenko-Rayleigh closed form of ADR 0597 D5 within 1e-4 at 16 elements, and the ADR 0597 pre-stressed frequencies stay within 1e-2 of their references; results of sections without an offset bit-identical (the change of results with an offset approved by the maintainer).
