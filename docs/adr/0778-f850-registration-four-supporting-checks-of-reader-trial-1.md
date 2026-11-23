# ADR 0778: F-850 registration: four supporting checks that reader trial 1 found weaker than their record says

Status: **REGISTERED 2026-11-23, written before any code of F-850 and before any run of its checks.** Before this text
no test was run and nothing was built by this work. What was done: reading (below) and the arithmetic below (a Python
script in the scratch folder, on closed forms and on the reference geometry of two surface meshes). Results go in
addenda at the end; nothing above them is changed after a run.

It answers the brief `docs/adr/briefs/f850-four-supporting-checks.md` and follows `docs/process/agent-rules.md`,
version 5. Branch `f850-four-supporting-checks` from main d10ea0bd, scratch
`E:/projects/cfd/w49i-work/f850-four-supporting-checks-scratch`. The four records it adds a check to, none of which is
edited, and none of their test files:

- [ADR 0553](0553-f627-registration-monosymmetric-thin-walled-sections-for-the-warping-element.md) (F-627), A1' and A2',
  and its addendum 2, point 5;
- [ADR 0585](0585-f661-registration-pre-stress-on-the-warping-harmonic-path.md) (F-661), HB-, and its addendum 2;
- [ADR 0570](0570-f639-registration-curvature-free-fallback-of-the-total-lagrangian-contact-body.md) (F-639), HS20m-M
  and HS10m-M, and its addendum 2;
- [ADR 0453](0453-f534-registration-rainflow-residue-guard-gerber-message.md) (F-534), G2, and its addendum 2, point 1.

## What is asked

The acceptance of F-850 (`docs/roadmap.json`, read by a script): "Every limit not stated here is registered in the ADR
before the run. Test-only, in new test files (no existing test or record is edited, no library line changes): (1) a
running assertion of the 16-element factors of ADR 0553 A1' and A2' against the closed form within 2e-2; (2) a mutation
of the warping geometric stiffness that the check HB- of ADR 0585 must catch (an axial force placed in a moment slot, or
K_sigma indexed into the bending equations), committed as the item's mutation patch; (3) the moment-radius readings
HS20m-M and HS10m-M of ADR 0570 against a limit of 0.08, with a uniform pressure on the Hertz disc as a negative control
that the limit must refuse (0.118); (4) the Gerber message of ADR 0453 G2 asserted by equality."

The source of the four clauses is the check of the 16 agreed findings of
[reader trial 1](../process/trials/reader-trial-1/registration.md); none changes a result or a status.

## What was read

- `AGENTS.md`; `docs/process/agent-rules.md` (version 5) whole; the brief whole; the entry F-850 of `docs/roadmap.json`
  (by script); `docs/process/trials/reader-trial-1/registration.md` whole; [`docs/process/tolerances.md`](../process/tolerances.md)
  whole; section 7.4 of `docs/test-process.md`; the head and the table of checks of ADR 0771 (style).
- ADR 0553: the acceptance and readings, D3, the arithmetic, the table of checks, amendments 1 to 3, the results table
  of addendum 1 and addendum 2. **Not read whole** (D1, D2, D4 to D8, the mutations and the questions were not read).
- ADR 0585: the acceptance and readings, D1 to D3 and the head of D4, the arithmetic, the table of checks, the planned
  mutations, the results and mutation tables of addendum 1 and addendum 2. **Not read whole.**
- ADR 0570: the acceptance, D4, D5, the arithmetic, the table of checks, the results table and notes of addendum 1 and
  addendum 2. **Not read whole** (D1 to D3, the mutations).
- ADR 0453: "What was read", the design, the checks, the mutations and addendum 2. **Not read whole.**
- Tests, read and not changed: `crates/rcfd-fem/tests/sa_beam_warping_f627.rs` lines 1 to 220, 381 to 440 and 849 to
  928; `sa_beam_warping_f661.rs` lines 1 to 20, 56 to 165 and 322 to 412; `sa_contact_tl_fallback_f639.rs` lines 1 to 27
  and 236 to 413; `sa_fatigue_f534.rs` whole; `f601_common/mod.rs` lines 14 to 80 and 630 to 842; `f623_common/mod.rs`
  whole; the head of one committed mutation patch (`sa_hyper_structures_default_adr0746.mutation.patch`).
- Library, read and not changed: `crates/rcfd-fem/src/beam_warping.rs` lines 250 to 262 (`wagner_about_shear_centre`),
  420 to 575 (`local_geometric_stiffness` and what it calls), 630 to 748, 899 to 1037 (`matrices_impl`) and 1635 to 1700
  (`WarpingPrestress::from_static`); `beam.rs` lines 525 to 535 (`end_resultants`: the resultants `(N, V_y, V_z, T, M_y,
  M_z)` at the two ends); `geometric.rs` lines 194 to 200 (`beam_terms`); `contact.rs` lines 20 to 34, 321 to 345
  (`ContactNode`) and 538 to 557 (`mortar_basis`); `fatigue.rs` lines 425 to 560 (`equivalent_amplitude`); `error.rs`
  (`InvalidValue` and its display); `loads.rs` lines 650 to 661 (`face_shape_functions`).
- `scripts/process_report.py` lines 440 to 520 (what it reads of a registration) and the header of
  `scripts/adr_test_index.py`.
- No paper was fetched and no published value is new here. Every reference value is a number of the four ADRs above
  (in the repository) or a closed form derived below. This registration adds no fragment under
  `docs/reference-data.d/`.

## Disclosure of prior knowledge and the arithmetic done beforehand

Known from the records: the 16-element factors of ADR 0553 (`178.68421280057328`, `53.97979218576993`; errors `1.29e-3`
and `3.9e-4`), HB- of ADR 0585 (`0`), the moment radii of ADR 0570 (`1.35e-2`, `3.40e-2`), and that G2's text is the
source's text (ADR 0453 addendum 2). The same numbers are expected again. Not known: the value of HB- under either
mutation below, the reading of a uniform pressure on the two meshes as the test will compute it, and the run times.

Arithmetic (`arith.py` in the scratch folder; no test and no library code was run):

1. **The uniform pressure, continuum.** ADR 0570 D5 reads `a_M = sqrt((5/2) M_2/M_0)`. For a uniform pressure on the
   disc of radius `a`, `M_0 = pi a^2 p` and `M_2 = pi a^4 p/2`, so `a_M = sqrt(5/4) a` and the error is `sqrt(5/4) - 1 =
   0.1180` (the `0.118` of the acceptance and of ADR 0570 addendum 2).
2. **The same on the two contact meshes.** The nodal forces of a pressure `p(r)` in the mortar weighting of ADR 0551 D1
   are `t_a = int Phi_a p dA`, with `Phi_i = N_i + (N_e + N_e')/4` at a corner and `Phi_e = N_e/2` at a mid-edge node.
   On the uniform part of the top surface (HS20: Quad8 faces of side `h = a/4`; HS10: Tri6 faces, two per square of side
   `h = a/3`, the diagonal taken from corner `(i, j)` to `(i + 1, j + 1)` in the script, which may differ from the
   mesh's), integrated by 48 x 48 midpoints per quadrilateral and the 48^2 centroids of a uniform subdivision per
   triangle, with `a = 0.1`, read by D5 over the nodes with `t_a > 0`: the uniform pressure on the disc gives the error
   `0.1302` on HS20 and `0.1388` on HS10; the Hertz pressure `sqrt(1 - r^2/a^2)` gives `1.340e-2` on HS20 and `2.303e-2`
   on HS10. The excess over `0.1180` and over `0` is the bias of the interpolant that ADR 0570 arithmetic 3 states (`a_M`
   raised by `1.3e-2` and `2.3e-2` for Hertz). The sum of the forces agrees with `int p dA` to `4e-4`.
3. **HB- under an axial force in a moment slot.** In `local_geometric_stiffness` the resultants at a Gauss point are
   `(vy, vz, t, my, mz) = (m(1), m(2), m(3), m(4), m(5))` and `m(0)` is the axial force. On the model of HB- (the fork
   beam, the pre-stress `N = -0.5 N_T = -6.407e5` from `WarpingPrestress::from_static`, so the end forces are given and
   `m(0) = N` in every element, every other resultant 0) the mutation `my = m(0)` adds the terms of a uniform moment
   `M_y = N` (ADR 0508 D1 as ADR 0553 D3 writes them for the sine mode: `-2 M a b k^2` between the lateral amplitude `a`
   and the twist amplitude `b`). The lateral equation of the first mode at the frequency of H1- is then
   `((P_y + N) k^2 - omega^2 rho A) a = M k^2 b`, with `P_y = 3.6625e5`, `k = pi/3`, `rho A = 24.178`, `omega^2 = 0.25 x
   29059 = 7265` (ADR 0585 arithmetic 1): `(-3.009e5 - 1.757e5) a = -7.026e5 b`, **`a/b = 1.47`** (metres of lateral
   displacement per radian of twist; the rotation `k a` is `1.54`). With `mz = m(0)` instead, the strong plane couples:
   `P_z = pi^2 E I_y/L^2 = 4.602e6`, `((P_z + N) k^2 - omega^2 rho A) a = 4.168e6 a`, **`a/b = 0.17`**. Both are
   Euler-Bernoulli estimates of one mode, good to a factor of about 3; both are ten orders above the limit `1e-10`.
4. **The Gerber text.** Rust prints `600e6` with `{:e}` as `6e8` and `400e6` as `4e8`, so the three texts of G2 are
   `mean stress 6e8 at or above the strength 6e8 (Goodman): no equivalent amplitude`, the same with `(Gerber)`, and
   `mean stress 4e8 at or above the strength 4e8 (Soderberg): no equivalent amplitude`.

## Design

No library line changes and no existing test or record is edited. Four new test files under `crates/rcfd-fem/tests/`,
one per record, each with the header `ADR 0778` and `F-850`, and one mutation patch beside the second:

| File | Holds | Runs in the gate |
| --- | --- | --- |
| `sa_beam_warping_ltb16_f850.rs` | check (1): A1'', A2'', A3'' | yes |
| `sa_beam_warping_hb_f850.rs` and `sa_beam_warping_hb_f850.mutation.patch` | check (2): HB-, mutation M1 | yes |
| `sa_contact_moment_radius_f850.rs` | check (3): HS20m-M', HS10m-M' (heavy records), NC and HZ (fast) | NC and HZ |
| `sa_fatigue_gerber_f850.rs` | check (4): G2= | yes |

Check (2) has its own file because the committed patch changes `local_geometric_stiffness`, which the buckling solves of
check (1) also use: in one binary the patch's command would fail for a reason that is not HB-.

### (1) The 16-element factors of ADR 0553 A1' and A2'

The set-up of `a_n_amended_run2_known_miss` (ADR 0553 amendment 2) copied into the new file: `WarpingSection::
monosymmetric_i(0.15, 0.012, 0.08, 0.008, 0.2, 0.006, 5/6)`, steel `E = 2e11`, `nu = 0.3`, `rho = 7850`, `L = 3` along
`x`, 16 Beam2 elements, fork supports, end moments `+-M_ref = +-1000` about `e_2`, the six-unknown pre-stress with the
beam resultants, `buckling_factors` with `BucklingOptions::new(3)` and `subspace_size = Some(40)`. Which of the two load
cases has flange 1 in compression is decided as there, from the static solution (the sign of `z_1 dtheta_y/dx`). The
lowest factor times `M_ref` of each case is compared with the closed form of ADR 0553 D3 twice: with the value the test
computes from the section constants (`closed` of the copied set-up), and with the digits ADR 0553 arithmetic 2 prints
(`1.78454564e5` with flange 1 in compression, `5.39587954e4` in tension). The second comparison does not share the
section constructor with the model.

### (2) HB- of ADR 0585 and the mutation it must catch

The set-up of H1- and HB- of `sa_beam_warping_f661.rs` copied into the new file: the fork beam of `doubly_symmetric_i(
0.1, 0.01, 0.18, 0.006, 5/6)`, 16 elements, the torque `T = 1000` at midspan, the pre-stress of the warping static solve
under the axial force `N = -0.5 N_T` (`WarpingPrestress::from_static`), `WarpingHarmonic` with that pre-stress, no
damping, the response at `0.5 omega_1(N)`. HB- is `max` over the components 0, 1, 2, 4, 5 of every node of the real and
imaginary parts, over `max` of the real twist; the largest twist is asserted finite and positive before the ratio is
formed. The check is the one ADR 0585 registered, with its limit.

**M1 (committed).** In `crates/rcfd-fem/src/beam_warping.rs`, `local_geometric_stiffness`, the line `let (vy, vz, t, my,
mz) = (m(1), m(2), m(3), m(4), m(5));` becomes `... = (m(1), m(2), m(3), m(0), m(5));`: the axial force placed in the
slot of the moment `M_y` (the first of the two mutations the acceptance names). Arithmetic 3 expects HB- near `1.5`.
The patch file is made with `git diff` and its first line is

    # expect-fail: cargo test --release -p rcfd-fem --test sa_beam_warping_hb_f850

**M2 (planned, scratch only).** The same line with `mz = m(0)`: the axial force in the slot of `M_z`. Expected near
`0.17`.

The rule for the patch, fixed now: M1 is committed if the command fails **at the HB- assertion** (the test prints the
ratio and "MISSED") and not by a refusal of a solve. If M1 fails only by a refusal and M2 fails at the assertion, M2 is
committed in its place and that is a deviation. If neither fails at the assertion, clause (2) is not held.

### (3) The moment radius of ADR 0570 against 0.08

The set-up of HS20m-M and HS10m-M of `sa_contact_tl_fallback_f639.rs` copied into the new file (through the shared
module `f601_common`, which is not edited): `hertz_block(4, 5, 4)` upgraded to Hex20 with `h = a/4`, and `hertz_block(3,
4, 4)` split and upgraded to Tet10 with `h = a/3`; the rigid sphere of radius 1; `hertz_prescribed`; `E = 1`, `nu =
0.3`; penalty `100/h`; `ContactOptions::new(1, 1e-8)` with `NewtonOptions::new(1).max_iterations(30)` and the mortar
contactor; the linear body. The reading is D5 of ADR 0570, `a_M = sqrt((5/2) M_2/M_0)` over the contactor nodes with
`t_a > 0` at their reference positions, against `a_H(F_fe)` of `hertz_theory`, `F_fe` four times the sum of the `z`
reactions as there. One function of the new file computes `a_M` from a list of `(t_a, X_a, Y_a)`; the solves, NC and HZ
all read through it.

**NC, the negative control of the acceptance.** On the top surface (the surface set "top") of each of the two meshes,
the nodal forces `t_a = int Phi_a p dA` of the uniform pressure `p = 1` on the disc `r <= a`, `a = A_H = 0.1`, with the
weighting functions of ADR 0551 D1 written in the test from `rcfd_fem::loads::face_shape_functions` (arithmetic 2), the
integral by 48 x 48 midpoints per Quad8 face and the 48^2 centroids of a uniform subdivision per Tri6 face. No solve.
The reading `abs(a_M/a - 1)` must be **above** 0.08: the limit refuses it. The limit `0.15` of ADR 0570 would have
passed the continuum value `0.118`; whether it passes the two discrete values is printed, not asserted.

**HZ, the fast test on the same code path (rule 7).** The same with the Hertz pressure `p = sqrt(1 - r^2/a^2)`: the
reading must be within 0.08. It shows in the gate that the limit passes the shape it is meant to pass, since the two
solves are heavy records.

**What is heavy.** ADR 0551 recorded 107 s and 142 s for these two solves on the shared machine, at and above the two
minutes of rule 7, and three other agents run here now. Both are registered as heavy records: `#[ignore]`d, each run
once by this work. No limit on time is registered.

### (4) The Gerber message of ADR 0453 G2 by equality

The three cases of G2 as `sa_fatigue_f534.rs` sets them up (amplitude `100e6`; Goodman and Gerber at `sigma_u = 600e6`,
Soderberg at `sigma_y = 400e6`, no compressive credit): each is refused with `FemError::InvalidValue`, key `"mean
stress"`, and its reason is compared by `assert_eq!` with the text formatted in the test from the pattern of ADR 0453
and with the literal text of arithmetic 4. Equality with either excludes "in modulus", so that clause of G2 needs no
assertion of its own.

## Table of checks

Every aggregated error is asserted finite before it is compared (a value that is not finite is a miss). Relative errors
are `abs(x/x_ref - 1)`.

| Id | Check | Limit | Source of the limit | Estimate | Margin | Kind |
| --- | --- | --- | --- | --- | --- | --- |
| A1'' | acceptance (1): the 16-element moment with flange 1 in compression against the closed form, computed in the test and as printed in ADR 0553 (`1.78454564e5`) | each `<= 2e-2` | the acceptance of F-850; ADR 0553 A1' | `1.29e-3` (ADR 0553 addendum 1) | 15.5 | code verification |
| A2'' | acceptance (1): the same with flange 1 in tension (`5.39587954e4`) | each `<= 2e-2` | the acceptance of F-850; ADR 0553 A2' | `3.9e-4` (ADR 0553 addendum 1) | 51 | code verification |
| A3'' | the two load cases put flange 1 one in compression and one in tension, and the larger moment is the compressed one (so that A1'' and A2'' compare the right roots) | holds | ADR 0553 A3, a guard of the two rows above without a number | holds (ADR 0553 addendum 1) | 1 | code verification |
| HB- | acceptance (2), the check: the bending components of the H1- response over the largest twist, unmutated | `<= 1e-10` | the acceptance of F-850, which names the check HB- of ADR 0585; this is that check's limit | `0` (ADR 0585 addendum 1) | undefined, the estimate is 0 | code verification |
| HS20m-M' | acceptance (3): ADR 0570 HS20m-M (heavy, run once): the solve succeeds; the moment radius against `a_H(F_fe)` | succeeds; `<= 0.08` | the acceptance of F-850 | `1.35e-2` (ADR 0570 addendum 1) | 5.9 | code verification |
| HS10m-M' | acceptance (3): ADR 0570 HS10m-M (heavy, run once): as HS20m-M' | succeeds; `<= 0.08` | the acceptance of F-850 | `3.40e-2` (ADR 0570 addendum 1) | 2.35 | code verification |
| NC | acceptance (3), the negative control: the reading of a uniform pressure on the Hertz disc, on the top surface of each of the two meshes | each `> 0.08` (refused) | the acceptance of F-850 (the limit the other way round, as the tolerance table states for a negative control) | `0.130` (HS20), `0.139` (HS10); continuum `0.118` | 1.6 and 1.7, the estimate over the limit | code verification |
| HZ | the reading of the Hertz pressure on the same two surfaces (the fast test of rule 7) | each `<= 0.08` | the acceptance of F-850 (the same limit on the same reading) | `1.34e-2` (HS20), `2.30e-2` (HS10) | 6.0 and 3.5 | code verification |
| G2= | acceptance (4): the three refusals of a positive mean, key `"mean stress"`, the reason equal to the text of ADR 0453 (formatted, and literal) | `==` | the acceptance of F-850; ADR 0453 G2 | equal (ADR 0453 addendum 2) | 1 | unchanged default output |

No row is of kind validation: the acceptance claims no agreement with a measured case.

## Negative control and mutations

The negative control is NC above. The mutations, each alone, on a `git archive` copy of the tree with the tests in the
scratch folder (`mut/`, target `target-mut`), by exact text replacement, the named command run once, the file restored:

| Id | Where | Change | Command (each `cargo test --release -p rcfd-fem --test ...`) | Must be caught by |
| --- | --- | --- | --- | --- |
| M1 | `beam_warping.rs`, `local_geometric_stiffness` | `my = m(0)`: the axial force in the slot of `M_y` | `sa_beam_warping_hb_f850` | HB-, at its assertion (expected near 1.5) |
| M2 | the same line | `mz = m(0)`: the axial force in the slot of `M_z` | `sa_beam_warping_hb_f850` | HB-, at its assertion (expected near 0.17) |
| M3 | `fatigue.rs`, the refusal of a positive mean | a full stop appended to the text | `sa_fatigue_gerber_f850`; and `sa_fatigue_f534`, expected to pass (containment) | G2= |
| M4 | `beam_warping.rs`, `wagner_about_shear_centre` | `self.wagner[0]` without `- 2.0 * z_0` (the wrong form `beta^C` of ADR 0553 N1, errors 0.33 and 0.50 there) | `sa_beam_warping_ltb16_f850` | A1'' or A2'' (an error above 2e-2, or a refusal of the solve, which is recorded as such) |

M1 is the committed one (rule 16), by the rule of the design. With M3 the existing test of G2 is expected to pass, which
is the weakness ADR 0453 addendum 2 names; if it fails, that is recorded and changes nothing here.

## Size

Size (agent): done; 0; 2; 0; 260000

(The outcome expected; further registrations; heavy runs on the shared machine, the two Hertz solves; questions for the
maintainer; bytes to read. The lead's size names no heavy run.)

## Decision rule

- **done**: A1'', A2'', A3'', HB-, HS20m-M', HS10m-M', HZ and G2= meet their limits, NC is refused on both meshes, and
  the committed mutation is caught by HB- at its assertion.
- **partial**: one of the four clauses cannot be held as registered; it is named with its number, held as the
  repository holds a known miss (an ignored test named `*_known_miss` with the limit intact), and the others stand.
- **todo**: none of the four is held.

Each registered run is made once. A set-up of this registration that proves defective is corrected by an amendment
committed before the corrected run. If a defect of the solver appears, the work stops and reports it.

## What the gate runs and what is a heavy record

The gate runs A1'', A2'', A3'', HB-, NC, HZ and G2= (four binaries, no solve longer than a few seconds expected) and
the committed patch M1. HS20m-M' and HS10m-M' are heavy records, ignored, run once here.

## What is not covered

- The cause of the excess of A1' and A2' over the closed form (ADR 0553 addendum 2, point 3) is not diagnosed.
- HB- stays a structural-zero check on its registered model; what changes is that one mutation of the class ADR 0585
  addendum 2 names is shown to be caught. The second class it names (`K_sigma` indexed into the bending equations) is
  not run.
- The moment radius stays an integral measure. A pressure of another shape whose `M_2/M_0` is within about 16 percent
  of Hertz's passes the limit 0.08; the peak check of ADR 0570 is not repeated here.
- The other points of the four addenda of 2026-11-19 (wording, counts, the pair M5b of ADR 0453) are records and need
  no check.
- No limit of the four ADRs changes, and their ignored records are not run again.
