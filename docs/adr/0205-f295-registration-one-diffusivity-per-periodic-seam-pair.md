# ADR 0205: F-295 registration: one diffusivity per periodic seam pair

Status: **REGISTERED 2026-11-06**, written before any code of F-295 and before any run of its checks. Not yet accepted by the
maintainer. Before this text, one probe of the existing code was run on main 9631754 (scratch tests, not committed): the
periodic set-up of ADR 0199 C1 with the existing owner-cell diffusivity, and the seam pair differences of the momentum face
viscosity on the B2 fan passage. Its numbers are quoted in item 10, where they set a sweep count and the limits of the negative
controls. Results go in the addendum at the end.

Parents: [ADR 0199](0199-f286-registration-transient-k-omega-sst-with-pimple.md) amendment 2 (the finding) and question 5
(option (b), recommendation taken by the lead), [ADR 0183](0183-exp-rot-registration.md) (check H, a rotational seam), ADR 0155
(optional objects of the case schema), ADR 0156 D6 (`hash_omit_default`), ADR 0036 (laminar regions). Roadmap item F-295
(depends on F-286).

Number: the lead gave 0205 to F-295 (0204 is on the branch `f291-gradient-options`).

House rules ([ADR 0130](0130-wave48a-overview-and-run-record-contract.md)):

- limits are fixed here, before the code and the runs, and are not changed after a run;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the number recorded, and the cause is
  diagnosed, not tuned away;
- a defective set-up gets a new corrected test by an amendment written before the corrected run, and the old one stays as an
  ignored record;
- the new code path is opt-in and the defaults stay bit-identical: `crates/rcfd-api/tests/baseline_hashes.rs` and
  `case_hashes_w48u.rs` are not edited or re-recorded, and no golden is re-recorded;
- each registered run is made once, and nothing is re-run to pick a result;
- no new third-party crate, no commit trailers.

Clean-room: the rule below is the flux balance of a pair of finite-volume faces, derived here from the operators of
`crates/rcfd-fv` and the eddy-viscosity plumbing of `crates/rcfd-turb`. No vendor code, text or install was used.

## What F-295 asks

The roadmap entry: on a periodic seam each face of a pair takes one common diffusivity, so that k, omega and momentum diffusion
is conservative across a seam where the eddy viscosity differs; opt-in, defaults bit-identical (baseline hashes unchanged); a
global budget on a periodic case with a differing eddy viscosity is checked within a limit fixed by the ADR. FAN-01 and check H of
ADR 0183 both have rotational seams, so this item comes before check H is read.

## What the code read shows (main 9631754)

1. **Periodic faces.** `Mesh::set_periodic` makes a patch `Cyclic` with its partner's name; `set_periodic_rotation` does so for
   both patches with the rotation. `FvGeometry::periodic_partner` holds, per periodic patch, the global partner face of each of
   its faces (geometric match by the translation or the rotation, `periodic_map`), and `dn` the normal distance from the owner
   centre to each boundary face. Each patch is matched to its partner separately, so nothing states that the two maps are
   inverse to each other.
2. **The periodic condition** (`rcfd_fv::bc::Periodic`). On face `f` of owner `P` with partner face `g` of owner `Q`, with
   `s = dn_f + dn_g`: face value `w q_P + (1 - w) q_Q`, `w = dn_g / s`, and normal gradient `(q_Q - q_P) / s`. The partner value
   `q_Q` is taken from the field when the coefficients are evaluated, so within one sweep it is the previous iterate (lagged).
3. **The diffusion operator** (`rcfd_fv::diffusion::laplacian`). A boundary face adds `Gamma_f |S_f| (c q_P + d)` to its owner,
   so the diffusive flux out of `P` through `f` is `Gamma_f |S_f| (q_Q - q_P) / s` and the one out of `Q` through `g` is
   `Gamma_g |S_g| (q_P - q_Q) / s`. With `|S_f| = |S_g|` (matched faces) the pair sums to
   `(Gamma_f - Gamma_g) |S| (q_Q - q_P) / s`: the pair is conservative only if `Gamma_f = Gamma_g`. A periodic face gets no
   non-orthogonal correction (its `a` is not 0).
4. **The diffusivities** (`rcfd_turb::eddy::effective_viscosity`). Internal faces `nu + w nu_t,o + (1 - w) nu_t,n` (the linear
   weight of the face); boundary faces, periodic ones included, `nu + nu_t` of their own owner; wall faces then take the wall-law
   value. In k-omega SST it gives the momentum face viscosity (`KOmegaSst::nu_face`) and, inside every sweep, the k and omega
   diffusivities from the cell values `sigma_k(F1) nu_t*` and `sigma_w(F1) nu_t*` (`gk_f`, `gw_f`), on the steady and the
   transient path alike. k-epsilon, Spalart-Allmaras, the Reynolds stress model and the subgrid models call the same function.
5. **Who reads the momentum face viscosity.** The momentum predictors of `simple`/`simplec`, the coupled solver and `pimple`; the
   transposed stress source of `viscous_stress = "full"` (on translationally periodic faces with `nu_face[f]`); the
   diffusivity `nu_face / Sc` of the scalars with a Schmidt number (`scalars.rs`); the local pseudo-time step; the `*_effective`
   force components (wall faces only).
6. **Laminar regions** (ADR 0036, `regions::mask_faces`). `TurbState::nu_face` sets the molecular viscosity on every face that
   touches a laminar cell; a boundary face is judged by its owner only, so a seam face whose partner cell is laminar keeps
   `nu + nu_t` of its own (turbulent) owner while its partner face has `nu`.
7. **The lag is a separate property.** Convection on a seam also uses the lagged partner value (upwind inflow takes it; the
   central part of the other schemes is deferred). Converged sweeps remove the lag; one sweep does not. F-295 does not change it
   (it is what made the homogeneous set-up H of ADR 0199 defective on the periodic box).
8. **The case tree.** `TurbState::new` (`rcfd-api/src/turb.rs`) builds the model and applies string options with
   `TurbulenceModel::set_option`; `SstModel` has none today (the trait's default refuses any name). A live `settings.set` of a
   `turbulence.*` path rebuilds the model through the same function.
9. **Written text and hashes.** `Case::to_toml` and `to_json_string` write the settings data, which lists every key of a
   non-optional object at its default; H1 of `case_hashes_w48u.rs` compares those texts for every case file and docs block, and
   normalises only the keys of its list `DEFAULT_KEYS` (a new non-optional key would need an edit of that test, which is not
   allowed here). A key inside an optional object (ADR 0155) stays out of the data of every case that does not set it, so the
   texts and the settings hashes of all existing cases stay as they are; `hash_omit_default` (ADR 0156 D6) also keeps the hash of
   a case that writes the default.
10. **The probe** (main 9631754, existing owner-cell diffusivity, release, not committed).
    - The first C1 set-up of ADR 0199 (the periodic box of item C1p below, `omega0 = 10`), every scheme and convection: the
      change of k per sweep is 7.1e-10 at sweep 30, 1.2e-12 at 40, 2.3e-15 at 50 and at round-off (at most 1.1e-15 for k, 9e-15
      for omega) from sweep 55 on, about a factor 0.5 per sweep. omega is uniform at the fixed point of the sweeps, so its
      diffusion is zero and its budget measures the seam lag alone: 1.2e-10 to 4.5e-10 of its destruction per step with 30
      sweeps, at most 1.8e-14 with 60, 90 or 120. **With 30 sweeps the lag alone misses the 1e-10 limit of C1, whatever the
      diffusivity**; 60 sweeps put it at round-off. The k budget with 60 to 120 sweeps (equal to 4 digits) is between 6.9e-7 and
      3.2e-2 of the destruction per step, 2.876e-2 at step 1 of Euler upwind (ADR 0199's record). F1 = 1 in every cell of every
      sweep. The largest seam difference `|nu_t,P - nu_t,Q|` of a step is 5.9e-5 to 1.4e-2 (the largest `nu_t` 0.095 to 0.134;
      the profile is convected across the seam, so the difference passes near zero at some steps).
    - The same with `omega0 = 10 - 5 sin(2 pi x)` (C2p below): F1 = 1 in every cell of every sweep; budgets per step with 60
      sweeps: k 5.0e-7 to 6.5e-2, omega 1.65e-7 to 3.9e-2.
    - B2 (the S6 text of ADR 0199, 13 627 cells, built in 0.83 s; seam patches `seam_lo` and `seam_hi`, 638 faces each,
      rotational): the largest `|nu_f - nu_g| / max nu_f` of the solver's momentum face viscosity over the seam pairs is
      5.7e-2, 9.4e-2, 0.131, 0.153, 0.164 after `pimple` steps 1 to 5 (0.9 s per step) and 0.649, 0.461, 0.325, 0.332, 0.271
      after SIMPLEC iterations 1 to 5 (0.09 s per iteration).

## Decisions (fixed now)

**D1. The rule.** For a periodic pair `(f, g)` with `f < g` (global face indices), `P` the owner of `f`, `Q` the owner of `g`,
and `w = dn_g / (dn_f + dn_g)`, both faces take

```text
Gamma_f = Gamma_g = nu + w a_P + (1 - w) a_Q
```

computed once per pair and stored on both faces, where `a` is the cell part of the diffusivity (`nu_t` for the momentum face
viscosity, `sigma_k(F1) nu_t*` for k, `sigma_w(F1) nu_t*` for omega). This is the internal-face formula of item 4 applied across
the seam, with the weight of the periodic condition's face value (item 2); on a translational seam of a uniform mesh it is the
value the face would have as an internal face. With equal values on both faces the pair's diffusive fluxes cancel (item 3).
Rejected: the mean of the two owner values (it ignores the distances on graded seams) and the harmonic mean (no other face of
these models uses it).

**D2. Where it acts.** k-omega SST only (question 1): the momentum face viscosity (`KOmegaSst::nu_face`) and the k and omega
diffusivities of every sweep, steady and transient. In the case tree the laminar-region rule extends across the seam: a pair whose
face `f` or `g` touches a laminar cell (its owner) takes `nu` on both faces, the internal-face rule of item 6. The scalars with a
Schmidt number and the transposed stress source read the momentum face viscosity, so they follow it (item 5). Nothing else
changes: the lag (item 7), the convection, the other models, the rheology and property face viscosities (question 1), and the
records of ADR 0199 (they keep the default).

**D3. The key.** `turbulence.periodic_seam.diffusivity`, an enumeration with the values `"owner"` (the default: each face of a
pair takes its own owner's value, the behaviour of every earlier run) and `"pair"` (D1). `turbulence.periodic_seam` is an optional
object (ADR 0155), so a case that does not set it keeps its data, written text and hashes (item 9); the object and the field are
`hash_omit_default`, so a case that writes `diffusivity = "owner"` has the hash of the same case without it. Case text:

```text
[turbulence.periodic_seam]
diffusivity = "pair"
```

**D4. Scope and refusals.**

- R1: `diffusivity = "pair"` with `turbulence.model` other than `k_omega_sst` (laminar, `k_epsilon`, `spalart_allmaras`, the
  algebraic and subgrid models, `rsm`, `plugin`) is a static located error at `turbulence.periodic_seam.diffusivity`
  (`semantic_errors`), naming `k_omega_sst`.
- R2: a value other than `owner` and `pair` is the enumeration's located error at the same path.
- R3: if the periodic maps of the mesh are not a pairing (a face whose partner's patch has no map, or whose partner's partner is
  not the face), `pair` is refused by `rcfd_fv` with an error, which the case tree locates at the same path from
  `Simulation::new`. `owner` never reads the pairs.
- `pair` on a mesh without periodic patches is accepted and changes nothing (no pair). `owner` is accepted with every model.
- Every refusal of SST that exists stays (`piso`, dynamic meshes, energy, error control, ...).

**D5. Code (the plan, with the files).**

- `crates/rcfd-fv/src/geometry.rs`: `PeriodicPair { f, g, w }` and `FvGeometry::periodic_pairs() -> Result<Vec<PeriodicPair>>`
  (the pairs with `f < g`, each periodic face in exactly one pair, R3).
- `crates/rcfd-turb/src/eddy.rs`: `SeamDiffusivity { Owner, Pair }` (default `Owner`) and
  `pair_periodic_faces(pairs, owner, nu, cell, face)`, which writes D1 on both faces of every pair.
- `crates/rcfd-turb/src/sst.rs`: `KOmegaSst::set_periodic_diffusivity(mode) -> Result<()>` (computes and keeps the pairs for
  `Pair`, drops them for `Owner`), `periodic_diffusivity()`, and the call of `pair_periodic_faces` after `effective_viscosity` in
  `nu_face` and for `gk_f`, `gw_f` in `update`, only when the pairs are kept.
- `crates/rcfd-turb/src/model.rs`: `SstModel::set_option("periodic_diffusivity", "owner" | "pair")`; any other name keeps the
  trait's refusal.
- `crates/rcfd-api/src/schema.rs`: the optional object `turbulence.periodic_seam` with the field `diffusivity` (D3).
- `crates/rcfd-api/src/turb.rs`: R1 in `check` (before the laminar return); in `TurbState::new`, for `pair`, the call of
  `set_option` (an `Err` located at the key, R3) and the pairs kept for the laminar rule of D2 in `TurbState::nu_face`.
- `docs/case-schema.md`: prose in the `k_omega_sst` section (no TOML block is added to or changed in the docs, so H1's docs
  blocks stay as recorded).

**D6. Why the defaults stay bit-identical.** A case without the key (or with `owner`) builds the SST model with no pairs, so
`nu_face` and the sweep compute `effective_viscosity` exactly as before and never call the new function; `TurbState::nu_face` runs
`mask_faces` alone, as before; every other model and the laminar path never read the key. The schema change is an optional
object, so the data, the written text and the settings hash of every existing case are unchanged (item 9). V0 checks this.

## Checks and limits (fixed now)

Every check is run once, in release. All but C1p30 are gate tests.

### Library checks (`crates/rcfd-turb/tests/f295_seam_diffusivity.rs`)

Set-ups:

- **BOX**: `box_2d(16, 4, 1.0, 1.0)` periodic in x (the box of ADR 0199 C1's first set-up), model walls `ymin`, `ymax`.
- **SEC**: `annulus_3d_sector(8, 6, 4, r_in 0.5, r_out 1.0, height 0.5, angle 40 degrees, uniform)`, a rotational seam, model walls
  `inner`, `outer`.
- The model: `KOmegaSst::new(geom, nu = 0.1, walls, k0 = 1, omega0 = 10)`; `pair` set with `set_periodic_diffusivity`.
- For F1 and G1, with `s = x` (BOX) or `s = theta / angle` (SEC): `nu_t = 0.1 (1 + 0.5 sin(2 pi s))` written into the model's
  `nut`; the scalar `q = 1 + 0.5 sin(2 pi s) + 0.2 y` (BOX) or `+ 0.2 z` (SEC) with the default conditions (periodic on the seam,
  zero gradient elsewhere); on BOX also the velocity `u = (sin(2 pi x) + 0.1 y, 0.5 sin(2 pi x + 1), 0)`, components x and y.

| Id | Check | Limit |
|---|---|---|
| P0 | precondition of every check, both meshes: `periodic_pairs` lists every face of every periodic patch exactly once; for each pair the partner of `g` is `f`; `abs(|S_f| - |S_g|) <= 1e-12 |S_f|`; `dn > 0`; BOX has 4 pairs, SEC 24 | as stated (a miss is a defective set-up) |
| R3 | BOX with the partner map of one `xmin` face changed to another `xmax` face (not a pairing) | `periodic_pairs` and `set_periodic_diffusivity(Pair)` return `Err`; `Owner` returns `Ok` |
| F1 | `nu_face` of the same state in both modes, both meshes. `pair`: for every pair `nu_f` and `nu_g` have the same bits, equal to the bits of D1 (computed in the test from `nut`, `dn` and the owners); every face that is not a seam face has the bits of the `owner` result. Non-vacuity: in `owner` mode the largest pair difference `>= 1e-3` of the largest `nu_face` | exact, exact, exact; `>= 1e-3` |
| G1 | global diffusion budget, both meshes, `G = abs(sum_c (A x - b)_c) / sum_f abs(F_f)`, with `(A, b) = laplacian(geom, nu_face, bc(x), None)`, `bc(x)` the boundary coefficients evaluated from the same field `x` (no lag), `F_f` the diffusive flux of each non-empty face; `x = q` (both meshes) and `x = u_x`, `u_y` (BOX) | `pair`: `G <= 1e-13` for each; negative control, `owner`: `G >= 1e-5` for each |

Transient budgets (C1 of ADR 0199 on the periodic box): BOX, `ymin` / `ymax` the walls with k, omega and velocity zero gradient;
`u = (1, 0, 0)` uniform (so `S = 0`) and `phi` its face flux (`face_flux`); `k = 1 + 0.5 sin(2 pi x)`, `nu = 0.1`; `euler` and
`bdf2`, `dt = 0.05`, 10 steps, k and omega convection `upwind` and `linear_upwind`, linear solver `tol_rel = 1e-14`,
`tol_abs = 1e-300`, `max_iter = 1000`; **M = 60 sweeps per step** (item 10: 30 leaves the lag at 1e-10 to 5e-10). With `c0, c1,
c2` the weights of the scheme (ADR 0199 D1), `omega*` the omega at the start of the last sweep and `'` the values after it:
`B_k = sum_c V_c ((c0 k' - c1 k^n + c2 k^(n-1)) / dt + beta* omega*_c k'_c)` against `D_k = sum_c V_c beta* omega*_c k'_c`, and
`B_w = sum_c V_c ((c0 w' - c1 w^n + c2 w^(n-1)) / dt + beta1 omega*_c w'_c)` against `D_w = sum_c V_c beta1 omega*_c w'_c`. With
F1 = 1 and `S = 0` these are the only terms that do not cancel across faces (P = 0, `(1 - F1) CD = 0`, zero-gradient walls with
zero flux), so each is zero up to round-off if every seam pair is conservative.

| Id | Set-up | Limit |
|---|---|---|
| C1p | `omega0 = 10`, `pair` | every step `abs(B_k) <= 1e-10 D_k`. Preconditions (a miss is a defective set-up): F1 = 1 in every cell of every sweep; the change of the last sweep `<= 1e-13 max k` and `<= 1e-13 max omega`. Non-vacuity: every step `max abs(k' - k^n) >= 1e-3`; the largest seam `abs(nu_t,P - nu_t,Q)` over the steps `>= 1e-3` |
| C1pN | negative control: C1p with `owner` | every step `abs(B_k) >= 1e-8 D_k`, and the largest over the steps `>= 1e-3 D_k` |
| C2p | `omega0 = 10 - 5 sin(2 pi x)` (gradients of k and omega opposed, so `CD <= 0` and F1 = 1, item 10), `pair` | every step `abs(B_k) <= 1e-10 D_k` and `abs(B_w) <= 1e-10 D_w`; the preconditions of C1p; non-vacuity as C1p and every step `max abs(w' - w^n) >= 1e-3` |
| C2pN | negative control: C2p with `owner` | every step `abs(B_k) >= 1e-8 D_k` and `abs(B_w) >= 1e-8 D_w`; the largest of each over the steps `>= 1e-3` |
| C1p30 | report only (ignored test): C1p with M = 30 | none; the lag of item 7 in `B_k` and `B_w` |

The negative-control limits are below the probe's smallest owner values (6.9e-7, 5.0e-7 and 1.65e-7) by more than a factor 16.

### Case-tree checks (`crates/rcfd-api/tests/f295_seam_diffusivity.rs`)

Texts: **CH**, the channel of ADR 0199 (`box_2d` 4 x 20, periodic in x, SST, drive `(1, 0, 0)`, `k_initial = 1`,
`omega_initial = 40`, upwind); **NP**, CH with `periodic_x = false`, no drive, `xmin` a fixed inlet `(15, 0, 0)` with
`turbulence_spec = "values"`, `k_value = 1`, `omega_value = 40`, and `xmax` a fixed-pressure outlet; **B2**, the S6 text of ADR
0199 (`pimple`), and B2S, the same with `algorithm = "simplec"` in place of its `pimple`, `n_outer` and `time_scheme` lines;
**LAM**, CH on SIMPLEC with `[regions.lam]` a box zone `box_min = [-1, -1, -1]`, `box_max = [0.25, 3, 1]` (the cells of the first
column, the owners of the `xmin` faces) and `turbulence = "laminar"`.

| Id | Check | Limit |
|---|---|---|
| S1 | scope. Accepted with no diagnostic (`parse_toml`, `semantic_errors`, `Simulation::new`): CH on SIMPLEC with `pair`; CH on `pimple` (`euler`) with `pair`; CH on SIMPLEC with `owner`; CH with `k_epsilon` and `owner`; laminar CH with `owner`; NP with `pair`. Located at `turbulence.periodic_seam.diffusivity` by `semantic_errors` (static, R1), the message naming `k_omega_sst`: `pair` with laminar CH, with `k_epsilon`, with `spalart_allmaras`. Located at the same path at parse (R2): `diffusivity = "harmonic"` | as stated |
| S2 | text and hash: CH and CH with `diffusivity = "owner"` have the same settings content hash; CH's `to_toml` text does not contain `periodic_seam`; CH with `pair` has another hash | as stated |
| T1 | B2S, 5 iterations, the solver's momentum face viscosity (`momentum_face_viscosity`). `pair`: for every seam pair the same bits on both faces, equal to the bits of D1 from the field `nut`; `owner`: the largest `abs(nu_f - nu_g) / max nu_f` over the pairs after iteration 5 `>= 1e-2` | exact; `>= 1e-2` |
| T2 | B2 (`pimple`), 3 steps, as T1 | exact; `>= 1e-2` |
| T3 | LAM, 3 iterations, the seam faces of the momentum face viscosity. `pair`: every seam face has the bits of `physics.nu`; `owner`: the largest seam face viscosity `>= 1.001 nu` | exact; `>= 1.001 nu` |

The probe gives the owner values of T1 and T2 (0.271 after iteration 5, 0.131 after step 3), so 1e-2 leaves a factor above 10.

### Default bits (V0)

| Id | Check | Limit |
|---|---|---|
| V0a | `baseline_hashes.rs` and `case_hashes_w48u.rs`, unedited | pass |
| V0b | `crates/rcfd-api/tests/f295_default_bits.rs`: FNV-1a (as `f286_default_bits.rs`) of the bits of every cell field the run has among `ux, uy, uz, p, k, omega, nuTilda, epsilon, nut` and of the face flux, recorded on main 9631754 with this text and the test only, committed before any code: CH on `pimple` (`bdf2`, `n_outer = 3`, `dt = 0.1`), 20 steps; the S4 sector of ADR 0199 (SST, frame, `pimple`), 5 steps; LAM, 100 iterations; CH with `k_epsilon` on SIMPLEC, 100 iterations; B2S, 5 iterations | bit-identical after the code |
| V0c | after the code: the CH `pimple` run of V0b with `diffusivity = "owner"` written; NP with `pair` against NP without the key | V0c1: the V0b record; V0c2: the same hash |
| V0d | `f286_default_bits.rs` (ADR 0199 V0b), unedited | pass |
| V0e | the gate (fmt per package, clippy `--workspace --all-targets -D warnings`, every test in release on a git-archive copy, `scripts/gate_test.py`, 3 jobs) on the final commit | pass, apart from the known flakes |

### Mutations (scratch, not committed)

Each is applied alone to the final tree, the named test is run once, and it must fail. Their numbers go in the addendum.

| Id | Mutation | Must fail |
|---|---|---|
| MU1 | `pair_periodic_faces` writes the value on `f` only (`g` keeps its owner value) | F1 |
| MU2 | the pair value uses the swapped weight `dn_f / (dn_f + dn_g)` | F1 (the D1 bits; the pair stays equal, G1 can pass) |
| MU3 | the k and omega diffusivities are left unpaired (the momentum face viscosity is paired) | C1p |
| MU4 | the omega diffusivity is left unpaired (k paired) | C2p |
| MU5 | the laminar pair rule of D2 removed from `TurbState::nu_face` | T3 |
| MU6 | `TurbState::new` does not pass the key to the model | T1 |
| MU7 | the static refusal R1 removed | S1 |

### Classes

V0 is a precondition of landing: a miss of V0a or V0b means the change is not merged and the cause is recorded. A miss of any
other limit is reported with its number and kept as an ignored `*_known_miss` test with the limit intact. A negative control
(C1pN, C2pN, the owner halves of F1, G1, T1 to T3) or a mutation that does not make its check fail means the check is not shown
to be able to fail: it is reported as a defective set-up, corrected by an amendment before any re-run, and never counted as a pass.
A miss of P0 or of a precondition of C1p / C2p is a defective set-up of the checks that use it.

## Predictions (mine; moderate confidence)

| Id | Prediction |
|---|---|
| PR1 | F1 exact; G1 with `pair` near 1e-16, with `owner` 1e-3 to 1e-2 |
| PR2 | C1p and C2p at round-off, below 1e-13 (the probe's omega budget with 60 sweeps, 1.8e-14 at most, is the floor) |
| PR3 | C1p30 reports 1e-10 to 5e-10, the lag of the probe |
| PR4 | T1 to T3 exact; V0 bit-identical |

## Budgets

Every gate test above takes under 30 s in release (the probe: the 20 runs of the periodic box with up to 120 sweeps in 7.6 s;
B2 builds in 0.83 s, 0.09 s per SIMPLEC iteration, 0.9 s per `pimple` step).

## Not in scope (and why)

- The other turbulence models and the rheology face viscosity, which use the same owner rule on seams (question 1).
- The lag of the periodic partner value (item 7), in diffusion and convection, and the H set-up records of ADR 0199.
- Check H itself and the FAN-01 runs (question 2), and the default (question 3).

## Open questions for the maintainer (each with a recommendation)

1. **The other models.** k-epsilon, Spalart-Allmaras, the Reynolds stress model, the subgrid and mixing-length models (all
   through `effective_viscosity`) and the rheology face viscosity (`rheology::nu_faces`, boundary faces from the owner) have the
   same property on a seam. Options: (a) SST only, as registered; (b) a follow-up item that applies D1 to each of them under the
   same key, each with F1/G1-type checks and a transient budget where the model has a time derivative. Recommendation: (b), at
   low priority: FAN-01 and check H run SST.
2. **Check H and FAN-01.** The H(dt) and H(dt/2) texts of ADR 0183 D5 were registered without the key. Options: (a) run H as
   registered (owner); (b) an amendment of ADR 0183 by the EXP-ROT job, before H runs, that sets `diffusivity = "pair"` in both
   texts; (c) run H both ways and report the difference. Recommendation: (b): the roadmap note puts this item before H is read,
   and the probe shows seam differences of 6 to 16 percent of the face viscosity in the first B2 steps.
3. **The default.** Options: (a) `owner` stays the default (opt-in, as built); (b) `pair` becomes the default, which changes every
   SST run with a periodic seam and needs the maintainer to lift the bit-identity rule for it. Recommendation: (a) now, and (b) only
   after a registered comparison on FAN-01 shows what it changes.

## Addenda

Results of the checks follow in an addendum, in the order of the tables.
