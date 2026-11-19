# ADR 0585: F-661 registration: the pre-stress on the warping harmonic path

Status: **REGISTERED 2026-11-15, written before any code of F-661 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-661 was written, compiled or run in the repository: the files listed under "What was
read" were read, and the arithmetic below was done by hand and evaluated with one plain numpy script in the scratch folder
(`w49i-work/f661-scratch/arith.py`, log `arith.log`: the cubic-Hermite Vlasov torsion model of ADR 0574 `arith.py` with Wagner's
axial term, its harmonic response, the resonance search of ADR 0574 D5, the continuous modal series, and the pinned-beam bending
frequencies under the axial force; no rcfd code; disclosed, its numbers quoted below). The bit record B0 of the default warping
harmonic path is a test file written and run on the base after this commit and before any library code (D5). Results go in the
addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-661 (deps F-635), structural solver, advanced tier, the follow-up named by
[ADR 0574](0574-f635-registration-warping-unknown-on-the-harmonic-and-transient-paths.md) (its question Q2). The element and
matrices are those of [ADR 0548](0548-f626-registration-warping-unknown-on-the-static-solve-modes-and-pre-stressed-modes.md)
(the 14-unknown stiffness, mass and geometric stiffness, the pre-stress `WarpingPrestress` of its D2) with the sections of
[ADR 0553](0553-f627-registration-monosymmetric-thin-walled-sections-for-the-warping-element.md) and the load points of
[ADR 0582](0582-f643-registration-load-height-and-non-uniform-moment-for-monosymmetric-warping-beams.md); Wagner's axial term is
that of ADR 0473 D1 and [ADR 0508](0508-f558-registration-beam-moment-shear-wagner-and-shell-rotation-geometric-terms.md) D1.
Number 0585 given by the lead. Branch `f661-work` from 535de064 (the F-643 branch, which carries F-635), worktree
`w49i-work/f661`, scratch `w49i-work/f661-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test edited;
no new dependency, no unsafe code, the crate graph unchanged; clean-room. Every choice below that the maintainer would normally
be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation), except the open
questions at the end.

The lesson of ADR 0574 amendment 2 is applied before the run: the resonance search ends at the first solve the resonance pivot
`1e-8` refuses, so every check that compares a searched resonance with an eigenvalue to better than `1e-8` runs the search with
`resonance_pivot(0.0)`, and every estimate below says whether a refusal can end its search (D4).

## What F-661 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. The direct harmonic solve with the warping unknown takes
> the pre-stress of the warping static solve (K + K_sigma); the first torsional resonance of a fork-supported I beam under an
> axial force N = +/-0.5 of the torsional buckling load equals sqrt((G J k^2 + N I_p/A k^2 + E C_w k^4)/(rho I_p)) within 1e-3
> at 16 elements; every result without the option bit-identical.

Clauses: **a0** every limit registered here; **a1** `WarpingHarmonic` takes the pre-stress of the warping static solve
(`WarpingPrestress::from_static` of ADR 0548 D2) and solves on `K + K_sigma`; **a2** the first torsional resonance of the
fork-supported I beam under `N = +0.5 |N_T|` and under `N = -0.5 |N_T|` within `1e-3` of C4 (below) at 16 elements; **a3** every
result without the option bit-identical.

### The closed form, derived (C4), and the sign of `N`

First principles (Vlasov torsion with Wagner's term, the derivation of ADR 0548 arithmetic 5 carried out here). A doubly
symmetric section (shear centre at the centroid) twisting by `phi(x, t)` under a uniform axial force `N` (the stress `sigma = N/A`
on every fibre). A fibre at the distance `r` from the axis moves by `r phi` sideways and its length grows, to second order, by
`(1/2) r^2 phi'^2` per unit length: the axial stress does the work `(1/2) int int sigma r^2 phi'^2 dA dx = (1/2) (N I_p/A) int
phi'^2 dx`, `I_p = I_y + I_z` the polar moment about the centroid. The energy is `U = (1/2) int ((G J + N I_p/A) phi'^2 + E C_w
phi''^2) dx`, the kinetic energy `(1/2) int rho I_p phi_t^2 dx` (no warping inertia, as ADR 0548 D4). Hamilton's principle gives

`E C_w phi'''' - (G J + N I_p/A) phi'' + rho I_p phi_tt = 0`,

and the fork supports (`phi = 0`, the warping free so `phi'' = 0`) at `x = 0, L` are satisfied by `phi = sin(k_m x) e^{i omega t}`,
`k_m = m pi/L`:

**C4**: `omega_m^2 = (G J k_m^2 + N I_p/A k_m^2 + E C_w k_m^4)/(rho I_p)`.

With `N` **tension positive** the axial term stiffens the twist. The torsional buckling load is the `N` at which `omega_1 = 0`:
`N_T = -(G J + E C_w k_1^2) A/I_p` (compression), `|N_T| = 1.281370e6` N for the section below. **The reading of the acceptance's
`N`: tension positive, the same as the code's.** In the code the axial force of a warping element is `A sigma_xx` of its stress
point (`matrices_impl`: `n_ax = section.area() * normal_stress(stress[0], axis)`, tension positive), and its Wagner block is `+n_ax
(I_y + I_z)/A int phi'^2` on the Hermite twist (ADR 0546 D2): ADR 0548 P1 measured `omega_1^2` up by `1.5` under a tip force
`+0.5 |N_T|` along `+x` at the free end `xl` (a tension). So `N = +0.5 |N_T|` (tension) gives `omega_1^2 = 1.5 C3` and `N = -0.5
|N_T|` (compression) `0.5 C3`, `C3 = (G J k_1^2 + E C_w k_1^4)/(rho I_p)` of ADR 0574.

### Readings, registered

- **"Fork-supported I beam"**: the full fork beam of ADR 0574 (ADR 0548 F4): the I section `doubly_symmetric_i(0.1, 0.01, 0.18,
  0.006, 5/6)`, steel `E = 2e11`, `nu = 0.3`, `rho = 7850`, `L = 3` along `x`, 16 Beam2 elements, `u = 0` at node 0, `v = w =
  phi = 0` at both ends, the warping free at both ends, the harmonic torque `T = 1000` about `x` at midspan (node 8; ADR 0574 Q1,
  reading (a)). The torsion-only model of ADR 0548 F1 (the same beam with components 0, 1, 2, 4, 5 held at every node) is used
  for supporting checks.
- **"Under an axial force N"**: the pre-stress is that of the warping static solve (`beam_warping::solve_static`) of the fork
  beam with the nodal force `(N, 0, 0)` at `xl` and no other load (statically determinate: the axial force is `N` in every
  element), made by `WarpingPrestress::from_static(model, warping, result, [0, 0])`. This is the set-up of ADR 0548 P1 and P2.
  The torque of the harmonic load is not in the pre-stress (the pre-stress model and the harmonic model share the mesh, the
  warping blocks and the numbering).
- **"Takes the pre-stress ... (K + K_sigma)"**: `K_sigma` is the geometric stiffness of `matrices` of ADR 0546 D5 with that
  pre-stress (the stiffness of the six-unknown part with the warping elements masked, plus the 14-unknown geometric stiffness of
  each warping element, plus the load-point terms of ADR 0582 when the `Warping` has load points). It enters `A(omega)` real and
  undamped, and the Rayleigh stiffness is `K + K_sigma`: the convention of the plain direct harmonic solve with a pre-stress (ADR
  0543 D5, `harmonic_direct.rs` `value`). The loss factor multiplies the elastic `K` only (a loss factor is a property of the
  material, the geometric stiffness is not; as ADR 0543 D5). Taken on the maintainer's standing instruction of 2026-11-11 (take
  the recommendation), the same as the existing path.
- **Compression beyond the weak-axis Euler load.** `0.5 |N_T| = 6.407e5` N is `1.749` times the weak-axis Euler load `pi^2 E I_z/L^2
  = 3.663e5` N (ADR 0548 arithmetic 5 says the same). Under `N = -0.5 |N_T|` the full fork beam's `K + K_sigma` is indefinite (the
  first weak bending mode has `omega^2 < 0`): the static state is not stable in bending. The linear harmonic problem is still
  defined, and for the doubly symmetric section the twist does not couple with bending (the axial pre-stress has no moments), so
  the torsional resonance is C4 all the same. The acceptance names the full fork beam and both signs; **the full beam in
  compression is checked as the acceptance asks (H2-)**, and the same check on the torsion-only model (H2T-), which is stable, is
  registered beside it. ADR 0548 tested compression only on the torsion-only model, because the eigensolver needs a positive
  definite pencil. The harmonic solve does not need one. The `LDL^T` factor without pivoting takes an indefinite matrix unless a
  pivot falls below its threshold (D4).
- **"The option"**: a new field `prestress: Option<WarpingPrestress>` of `WarpingHarmonicOptions` with the builder
  `prestress(p)`. Without it (`None`, the default) `WarpingHarmonic::new` and `response` run the code of ADR 0574 unchanged.
  Opt-in; no case-file key (F-625).

### Audit of refusals this change could lift (done before this text)

None. `WarpingHarmonic` has no pre-stress today and nothing refuses one (there is no field to pass it). The plain
`DirectHarmonic` refuses beams (ADR 0574 amendment 1) and is not touched. Searched the warping tests (`sa_beam_warping_f598.rs`,
`_f626.rs`, `_f627.rs`, `_f635.rs`, `_f643.rs` and the bit records) for `prestress` with `WarpingHarmonic`: no hit.

## What was read (no run)

- `AGENT_RULES.md`; the brief; ADR 0574 (whole, including amendment 2: the resonance-pivot lesson); ADR 0548 D2, D5, arithmetic
  5, P1 to P3 and the addendum; ADR 0553 D2, D4; ADR 0582 (the load points in `matrices`); ADR 0508 D1 (Wagner); the "For the lead"
  sections of ADR 0572 and 0573.
- `crates/rcfd-fem/src/beam_warping.rs`: `matrices_impl`, `WarpingPrestress` (`from_static`, `torsion_free`, `with_torsion`,
  `check_for`), `modes_impl` (how the pre-stressed modes form `K + K_sigma`: `geometric::add_on_pattern` of `matrices`),
  `stiffness_aug`, `mass_aug`, `augmented`, `reduced_dynamic`, `WarpingHarmonicOptions`, `WarpingHarmonic` (whole).
- `harmonic_direct.rs`: the module text of ADR 0543, `DirectHarmonicOptions::prestress`, the pre-stress assembly and `value`.
- `geometric.rs`: `add_on_pattern`. Tests: `sa_beam_warping_f635.rs` (the fork beam, the series, the search D5, the checks),
  `sa_beam_warping_f626.rs` (`torsion_only`, `axial_prestress`, P1 to P3), `sa_beam_warping_f635_bits.rs` (the digest).

## Design

### D1. The pre-stress option

`WarpingHarmonicOptions::prestress: Option<WarpingPrestress>` (default `None`), builder `prestress(p: WarpingPrestress)`.
`WarpingHarmonic::new` with `Some(p)`:

1. the checks and the reduced `K`, `M` of ADR 0574 D2 unchanged (`reduced_dynamic`);
2. `p.check_for(model, warping)` (RF10 of ADR 0548 D3: a `from_static` pre-stress of other blocks, sections or numbering is
   refused, key `"warping"`; whether RF8 applies), then `matrices_impl(model, &p.prestress, warping, threads, check_torque)` (RF8:
   a torque on a warping element in a `torsion_free` pre-stress refused, key `"warping"`; the `Prestress::check_fits` refusals);
   its `geometric` matrix reduced by the same prescribed and restrained equations (`constraint::reduce`, zero values; the same
   free set);
3. the union pattern of the reduced `K`, `M` and `K_sigma` with three value arrays `kv`, `mv`, `gv`.

`K` stays `stiffness_aug` (the elastic stiffness of ADR 0548, the same bits as without the option). `response` with `gv`:

`A(omega) = (1 + i eta) K + K_sigma - omega^2 M + i omega (alpha M + beta (K + K_sigma))`,

entry by entry `re = k + g - omega^2 m`, `im = eta k + omega (alpha m + beta (k + g))`. Without the option `gv` is absent and the
entry is computed by the expression of ADR 0574 unchanged (`re = k - omega^2 m`, `im = eta k + omega (alpha m + beta k)`), on the
pattern of `K` and `M` only. The pivot rule, the refusal texts and the response are those of ADR 0574 D2.

### D2. What the option does not change

`reduced_dynamic`, `WarpingTransient`, `matrices`, `modes_impl`, `solve_static`, `WarpingPrestress` and every other function are
not changed. No other module is touched.

### D3. Refusals

Those of ADR 0574 D2 and, with the option, RF8 and RF10 of ADR 0548 D3 and the `check_fits` refusals of `Prestress` (each with
its key).

### D4. The resonance search and the refusals on the way (registered)

The search is D5 of ADR 0574 unchanged (secant on `g(s) = 1/Re X_d(s)`, the undamped midspan twist; steps stop at `|s_{j+1} -
s_j| <= 1e-6 |s_j|`, at most 20; a solve refused as a resonance, key `"frequency"`, ends the search with that iterate as the
result), started at `0.98` and `1.02` of C4's `omega_1^2`. Refusals that can end it, accounted for in each estimate:

- **At the torsional resonance**, on the default options: the pivot rule refuses an iterate within about `1e-8` relative of the
  model's eigenvalue (ADR 0574 amendment 2). Such an iterate is at most about `1e-8` from the eigenvalue in `s` (`5e-9` in
  `omega`), below the discretisation error `2.4e-7` and `7.2e-7` (arithmetic 2) by a factor of about 50; the acceptance limit
  `1e-3` is not near it. The eigenvalue checks H3+ and H3- run the search with `resonance_pivot(0.0)` (ADR 0574 H3'), so a
  refusal cannot end them; their estimate is the numpy search to `1e-11`.
- **At a bending pole**: the twist does not couple with bending, but the factorisation sees every equation. A bending
  eigenvalue of `K + K_sigma` within about `1e-8` of an iterate `s` would be refused. Arithmetic 4: the nearest bending poles are
  `0.52` (weak, tension) and `2.73` (strong) of the torsional `s` in tension, and `-0.43` (weak, negative) and `5.15` (weak, `m =
  2`) in compression, all far from the iterates in `[0.98, 1.02]`.
- **An accidental small pivot of the indefinite matrix** (compression on the full beam): the `LDL^T` factor in the reverse
  Cuthill-McKee order without pivoting can, in principle, meet a near-zero pivot of a leading block even away from an
  eigenvalue. Not computed (the numpy model does not reproduce the library's order and the bending part); the estimate is "no
  refusal", and the risk is disclosed. A refusal at the first evaluation `s_0` (which the search D5 does not take as a result:
  `g(s_0)` must exist) would be a miss of H2-; the stable torsion-only H2T- then still shows the closed form.

## Arithmetic done before the code (disclosed)

`arith.log`. Section: `A = 3.08e-3`, `I_y = 2.098267e-5`, `I_z = 1.669907e-6`, `I_p = 2.265257e-5`, `G J = 6125.128`, `E C_w =
3008.333` (as ADR 0548 arithmetic 1).

1. **C4.** `|N_T| = 1.281370e6`. `N = +0.5 |N_T| = 6.406852e5`: `omega_1^2 = 87177.174` (ratio `1.5` to C3 `58118.116`),
   `omega_1 = 295.25781` rad/s. `N = -0.5 |N_T|`: `omega_1^2 = 29059.058` (ratio `0.5`), `omega_1 = 170.46718` rad/s.
2. **16 Hermite elements** (the twist model of ADR 0574 `arith.py`, the Wagner block `N I_p/A int H' H'`, i.e. `G J -> G J + N
   I_p/A`; supports `phi = 0` at both ends): tension, the eigenvalue `+4.8e-7` in `s`, `omega_1` `+2.41e-7` from C4; compression
   `+1.44e-6` in `s`, `omega_1` `+7.22e-7` (as ADR 0548 P1, `4.8e-7` and `1.4e-6`). The search D5 (no pivot test): 3 secant
   steps in each case, `-3.8e-12` and `-1.1e-11` from the eigenvalue in `s`. 8 elements: `omega_1` `+3.88e-6` and `+1.15e-5`.
3. **Responses** against the continuous modal series with `N` (4000 terms, the modes `sin(k_m x)`, `omega_m^2` of C4, a loss
   factor on the elastic part `K_m = G J k_m^2 + E C_w k_m^4` only, the Rayleigh stiffness term on `K_m + N I_p/A k_m^2`):
   tension / compression: undamped at `0.5 omega_1(N)` `3.5e-6` / `1.4e-8`; loss factor `eta = 0.02` at `omega_1(N)` `3.6e-5` /
   `3.6e-5`; Rayleigh `zeta = 0.02` (`alpha = zeta omega_1(N)`, `beta = zeta/omega_1(N)`) at `omega_1(N)` `1.2e-5` / `3.6e-5`;
   both together at `0.5 omega_1(N)` `3.5e-6` / `1.8e-8`. The wrong conventions: the series with the loss factor on `K + K_sigma`
   differs from the model's response by `0.50` / `0.50`; the model with `beta K` only (not `beta (K + K_sigma)`) differs from the
   series by `0.20` / `0.33`. The model without the pre-stress at `0.5 omega_1(N)` differs from the series with `N` by `0.78` /
   `0.56`.
4. **Bending poles** of the full fork beam under `N` (pinned Euler beam in both planes, `omega^2 = (E I k_m^4 + N k_m^2)/(rho
   A)`, Timoshenko ignored; for D4 only): tension `45671` (weak `m = 1`, `0.524` of the torsional `s`), `237789` (strong, `2.73`),
   `382025` (weak `m = 2`); compression `-12447` (weak `m = 1`, negative: beyond its Euler load), `149552` (weak `m = 2`, `5.15`),
   `179671` (strong, `6.18`).
5. **Mutations** (arithmetic of the twist model): `-K_sigma` moves the tension resonance to `s` `-0.67` and the compression one
   to `+2.0` of C4; `2 K_sigma` to `+0.33` and `-1.0` (zero: buckled).

## Checks (limits fixed now)

Every aggregated error is asserted finite before it is compared (a non-finite value is a miss). The fork beam of the readings,
16 elements, the midspan torque, the pre-stress of `N = s_N 0.5 |N_T|` (`s_N = +1` tension, `-1` compression) unless stated;
"twist" is component 3 of node 8; "C4" is `omega_1^2` of arithmetic 1 for that `N`. A label with `+` or `-` is run for that
sign; `+-` for both.

| id | check | limit | estimate |
|---|---|---|---|
| B0 | the bit record of D5 (the default `WarpingHarmonic`), base and after; the existing bit records (`sa_beam_warping_f598_bits`, `_f626_bits`, `_f635_bits`) unchanged | equal (bit) | exact |
| P0 | the option with a zero pre-stress (`Prestress::uniform` zero, `torsion_free`): the responses of B0's four cases bit-equal to those without the option | equal (bit) | exact (`k + 0 = k`) |
| H2+- | **acceptance a1, a2**: the search D4 on `WarpingHarmonic` with the option (default resonance pivot), full fork beam: `|omega_r/sqrt(C4) - 1|` | `<= 1e-3` | `2.4e-7` (+), `7.2e-7` (-) |
| H2T+- | H2 on the torsion-only model (the pre-stress from the same fork static solve) | `<= 1e-3` | `2.4e-7`, `7.2e-7` |
| H3+ | the search with `resonance_pivot(0.0)` on the full beam in tension against the torsional eigenvalue (the mode with the largest twist at node 8) of ADR 0548 `prestressed_modes` (4 modes) with the same pre-stress | `<= 1e-8` relative | `4e-12` |
| H3- | the same on the torsion-only model in compression (`prestressed_modes`, 1 mode; the full beam's pencil is indefinite) | `<= 1e-8` relative | `1.1e-11` |
| H8+- | H2 at 8 elements (the pre-stress from the 8-element fork) | `<= 1e-4` | `3.9e-6`, `1.2e-5` |
| H1+- | undamped twist at `0.5 omega_1(N)` against the series with `N` | `<= 3e-5` relative | `3.5e-6`, `1.4e-8` |
| H5+- | loss factor `0.02` at `omega_1(N)`: `|X/X_c - 1|` against the series (the loss factor on the elastic part) | `<= 2e-4` | `3.6e-5`, `3.6e-5` |
| H6+- | Rayleigh `zeta = 0.02` at `omega_1(N)`: as H5 (the Rayleigh stiffness `K + K_sigma`) | `<= 2e-4` | `1.2e-5`, `3.6e-5` |
| H6b+- | both of H5, H6 at `0.5 omega_1(N)` | `<= 3e-5` | `3.5e-6`, `1.8e-8` |
| H7+- | negative control: `WarpingHarmonic` without the option at `0.5 omega_1(N)` against the series with `N` | `> 0.3` | `0.78`, `0.56` |
| HS | the sign: the H2 resonance in tension over that in compression, against `sqrt(3)` | `<= 1e-5` relative | `5e-7` |
| HB- | the bending components (0, 1, 2, 4, 5 of every node) of the H1- response on the full beam: `max|.|` over `max|twist|` | `<= 1e-10` | `0` |
| R | refusals with the option, key each: a `torsion_free` pre-stress of a static solve with a torque (the fork beam with the midspan torque) (RF8, `"warping"`); a `from_static` pre-stress of another section (`saint_venant()` blocks) (RF10, `"warping"`); a `from_static` pre-stress of the 8-element model on the 16-element model (`"warping"`, RF10 numbering) | exact | |

Negative controls: H7 (the option acts), HS (the sign of `N`), P0 (a zero pre-stress is the default path), the wrong
conventions of arithmetic 3 (mutations Z3, Z4), HB- (no spurious coupling in the indefinite case).

## Planned mutations (on a scratch copy, one at a time, own `CARGO_TARGET_DIR`)

| id | mutation | expected caught by |
|---|---|---|
| Z1 | the option ignored (`K_sigma` not added) | H2+-, H2T+-, H3+-, H1+-, HS |
| Z2 | `- K_sigma` | H2+-, H2T+-, H1+-, HS, or a refusal |
| Z3 | the loss factor on `K + K_sigma` | H5+- |
| Z4 | the Rayleigh stiffness `beta K` only | H6+- |
| Z5 | `2 K_sigma` | H2+-, H1+- (compression: resonance at 0, a refusal) |
| Z6 | the check `check_for` skipped (RF10 not applied; `check_torque` true) | R (RF10 cases) |
| Z7 | `K_sigma` not reduced by the restrained and prescribed equations (the full geometric matrix indexed as the free set) | H2+-, H1+-, or a refusal |

## Decision rule

**done**: B0, P0, H2+-, H2T+-, H3+-, H8+-, H1+-, H5+-, H6+-, H6b+-, H7+-, HS, HB- and R met, and every mutation caught.
**partial**: any registered check missed (a `*_known_miss` record), any mutation not caught, or any check met only after a
post-run amendment. **todo**: H2+ or H2- missed with H2T of the same sign missed, or B0 missed.

## Limits (not covered)

- No pre-stress on `WarpingTransient` (the linear transient); the harmonic path only.
- No warping inertia `rho C_w` (C4 neglects it too); no mode-superposition harmonic.
- The pre-stress with torque (`from_static` of a load with torsion) and with moments (lateral-torsional coupling) is taken (the
  same `matrices` as `prestressed_modes`) but not checked against a closed form here; H3+ checks the path against the
  pre-stressed modes for the axial case only.
- The load points of ADR 0582 enter `K_sigma` when the `Warping` has them (as in `matrices`); not checked on this path.
- Monosymmetric sections (ADR 0553) with an axial pre-stress couple twist and bending; not checked here.
- No case-file key (F-625).

## Open questions (to be taken on the maintainer's standing instruction of 2026-11-11 when answered; none changes a default)

- **Q1. Damping of `K_sigma`.** Options: (a) the loss factor on the elastic `K` only, the Rayleigh stiffness `K + K_sigma` (this
  ADR, as ADR 0543 D5); (b) both on `K + K_sigma`; (c) both on `K` only. Recommendation (a), the existing convention.
- **Q2. A pre-stress on `WarpingTransient`.** Recommendation: a separate item when a case needs it.

## D5. The bit record B0

`tests/sa_beam_warping_f661_bits.rs`, written and run on the base (535de064 plus this ADR, no library line changed): digests
(FNV-1a 64 of ADR 0291) of `WarpingHarmonic` responses (re and im, every equation) on the 16-element fork beam under the midspan
torque, default options: undamped at `0.5 sqrt(C3)`, with the loss factor `0.02` at `sqrt(C3)`, with Rayleigh `zeta = 0.02` at
`sqrt(C3)`, and at `omega = 0`; and the undamped response at `0.5 sqrt(C3)` with the warping held at both ends. It runs unchanged
after the change.

## Addendum 1 (2026-11-15): results

Written after the one registered run of `sa_beam_warping_f661.rs` (log `f661-scratch/run1.log`, code commit eebb2933), the B0
runs (`b0_base2.log` on the base 054a7b74 with no library line changed, `after.log`) and the mutation runs (`mutations.log`,
`mut_Z*.log`). The numbers are those the tests printed (release). No limit was changed, no amendment was written, and nothing
was re-run to pick a result.

### Results against the registered limits

| id | measured | limit | verdict |
|---|---|---|---|
| B0 | 5 digests of the default `WarpingHarmonic` bit-identical to the base record; the ADR 0546, 0548 and 0574 bit records unchanged | equal | met |
| P0 | the four responses with a zero pre-stress bit-equal to those without the option | equal | met |
| H2+ (acceptance) | `2.32e-7` (search ended at step 2 by a resonance refusal, as D4 accounts) | `1e-3` | met |
| H2- (acceptance, full beam beyond its weak-axis Euler load) | `7.20e-7` (likewise; no refusal of the indefinite matrix away from the resonance) | `1e-3` | met |
| H2T+ / H2T- | `2.32e-7` / `7.20e-7` | `1e-3` | met |
| H3+ | `7.1e-14` (the torsional mode is mode 2 of the pre-stressed modes, `87177.2159`) | `1e-8` | met |
| H3- | `1.9e-12` | `1e-8` | met |
| H8+ / H8- | `3.87e-6` / `1.15e-5` | `1e-4` | met |
| H1+ / H1- | `3.55e-6` / `1.39e-8` | `3e-5` | met |
| H5+ / H5- | `3.62e-5` / `3.61e-5` | `2e-4` | met |
| H6+ / H6- | `1.23e-5` / `3.61e-5` | `2e-4` | met |
| H6b+ / H6b- | `3.55e-6` / `1.80e-8` | `3e-5` | met |
| H7+ / H7- | `0.776` / `0.565` | `> 0.3` | met |
| HS | `4.89e-7` | `1e-5` | met |
| HB- | `0` | `1e-10` | met |
| R (3 refusals) | each with key `"warping"` | exact | met |

Every measured value is within a factor of 1.1 of its arithmetic estimate. The resonance searches on the default pivot ended at
step 2 by the refusal D4 predicted (an iterate within about `1e-8` of the eigenvalue). That refusal is far below the `1e-3`
limit. The searches with `resonance_pivot(0.0)` (H3) ended by the step rule.

### Mutations (scratch copy `f661-scratch/mut`, own target dir, one at a time)

| mutation | result | caught by |
|---|---|---|
| Z1 `K_sigma` not added | caught | H1, H5, H6, H6b, H2, H2T, H3, H8 (both signs), HS |
| Z2 `- K_sigma` | caught | as Z1 |
| Z3 loss factor on `K + K_sigma` | caught | H5+-, H6b+- |
| Z4 Rayleigh `beta K` only | caught | H6+-, H6b+- |
| Z5 `2 K_sigma` | caught | as Z1 |
| Z6 `check_for` skipped | caught | R (both RF10 cases) |
| Z7 `K_sigma` not reduced | caught | a refusal (key `"pattern"`, a column out of range) in H1 to H6b and P0; H2, H2T, H3, H8, HS |

Seven of seven caught. Z3 was also caught by H6b, which the registration did not predict.

### Other runs

`cargo test -p rcfd-fem --release` for the warping test binaries (`sa_beam_warping_f598`, `_f598_bits`, `_f626`, `_f626_bits`,
`_f627`, `_f635`, `_f635_bits`, `_f643`, `_f661`, `_f661_bits`): all passed, the ignored records stay ignored. `cargo test -p
rcfd-api --release --test baseline_hashes`: 3 passed, 1 ignored (unchanged). `cargo clippy -p rcfd-fem --release --all-targets
-- -D warnings`: clean. `cargo fmt -p rcfd-fem --check`: clean. The whole rcfd-fem suite was not run (the lead runs the gate).

### Decision

**Done** by the registered rule: every registered check met in the one run, every mutation caught, no amendment. By clause:
a1 is met (H7, P0 and mutations Z1 to Z5 show that `K + K_sigma` of the `from_static` pre-stress is the matrix solved); a2 is
met for both signs on the full fork beam and on the torsion-only model; a3 is met (B0, P0).

### Remaining questions

- Q1 (the damping of `K_sigma`): (a) the loss factor on the elastic `K` only and the Rayleigh stiffness `K + K_sigma`, as taken
  (the convention of ADR 0543 D5); (b) both on `K + K_sigma`; (c) both on `K`. Recommendation: (a).
- Q2 (a pre-stress on `WarpingTransient`): recommendation, a separate item when a case needs it (proposed below).
- Q3 (compression beyond the weak-axis Euler load): the harmonic solve takes an indefinite `K + K_sigma` without comment. Options:
  (a) leave as is (the linear problem is defined, and the solve refuses only a near-singular pivot); (b) count the negative
  pivots and report them on the solver (as a Sturm count) so a caller sees that the pre-stressed state is unstable. Recommendation:
  (b) as a small follow-up, because a harmonic response about an unstable static state can be misread.

## For the lead: roadmap entries

> Proposed status: done.

> Structural solver, advanced tier. Follow-up of F-635 (ADR 0574, 2026-11-15). DONE. ADR 0585, 2026-11-15: the direct harmonic solve with the warping unknown takes a pre-stress (`WarpingHarmonicOptions::prestress`, a `WarpingPrestress`, for example `from_static` of the warping static solve): A = (1 + i eta) K + K_sigma - omega^2 M + i omega (alpha M + beta (K + K_sigma)), with K_sigma the geometric stiffness of ADR 0546 `matrices`, real and undamped and in the Rayleigh stiffness (the convention of ADR 0543). RF8 and RF10 of ADR 0548 apply. Fork-supported I beam, torque at midspan, axial force N = +/-0.5 of the torsional buckling load (tension positive, as in the code): first torsional resonance 2.3e-7 (tension) and 7.2e-7 (compression, the full beam beyond its weak-axis Euler load, K + K_sigma indefinite) from sqrt((G J k^2 + N I_p/A k^2 + E C_w k^4)/(rho I_p)) at 16 elements (3.9e-6 and 1.2e-5 at 8), the same on the torsion-only model. The searched resonance matches the pre-stressed modes to 7e-14 and 2e-12. Responses are within 3.6e-5 of the continuous series with a loss factor, with Rayleigh damping, and undamped. Defaults bit-identical (5 digests, and a zero pre-stress gives the bits of no option). Seven of seven mutations caught. No pre-stress on the warping transient, no warping inertia, no case-file key.

> Follow-up proposed: Pre-stress on the warping transient path. Deps F-661. Acceptance: Every limit not stated here is registered in the ADR before the run. WarpingTransient takes the pre-stress of the warping static solve (K + K_sigma); the midspan twist of the fork-supported I beam under a step torque and an axial force N = +/-0.5 of the torsional buckling load follows the continuous modal series with omega_m^2 = (G J k_m^2 + N I_p/A k_m^2 + E C_w k_m^4)/(rho I_p) within 1.5e-2 of the static twist over two periods at 16 elements; every result without the option bit-identical.

> Follow-up proposed: Negative-pivot count of the pre-stressed warping harmonic solve. Deps F-661. Acceptance: Every limit not stated here is registered in the ADR before the run. WarpingHarmonic with a pre-stress reports the number of negative eigenvalues of K + K_sigma on the free equations (a Sturm count of the real factor at omega = 0); the fork-supported I beam under N = -0.5 of the torsional buckling load reports exactly the number of weak-axis bending modes beyond their Euler load (one), and 0 under N = +0.5; every result without the option bit-identical.

## Addendum 2 (2026-11-19, the lead): what the check HB- shows

Source: reader trial 1 (`docs/process/trials/reader-trial-1/registration.md`, "Check of the 16 agreed findings", and `results/check-of-agreed-findings.json` there). Two readers flagged the place and a checker read the repository for it. Nothing above this addendum is changed, no test was run again, and no result, limit or status of the item changes. Line numbers are those of this file before this addendum.

- **HB- is a structural-zero check with no demonstrated failure.** On the registered model the section is doubly symmetric,
  the pre-stress is a pure axial force and the load a pure torque, so the bending block has a zero right-hand side and no
  coupling entry: the registered estimate is exactly 0 and the measured value is 0. None of the seven planned mutations names
  HB- as its catcher, and omitting, negating or doubling `K_sigma` leaves it at 0. It can fail: the geometric stiffness has
  twist-bending coupling terms driven by end moments and shear forces of the pre-stress, so an axial force put into a moment
  slot, or `K_sigma` indexed into the bending equations, would make it non-zero. That class of error was not shown to be
  caught.
- **Lines 226 to 227 list HB- under "Negative controls".** It is not one: it is a check that is expected to pass.
- The acceptance does not need HB-. A mutation that HB- must catch is part of roadmap item F-850.
