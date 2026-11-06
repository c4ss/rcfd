# ADR 0205: F-295 registration: one diffusivity per periodic seam pair

Status: **REGISTERED 2026-11-06**, written before any code of F-295 and before any run of its checks. Not yet accepted by the
maintainer. Before this text, one probe of the existing code was run on main 9631754 (scratch tests, not committed): the
periodic set-up of ADR 0199 C1 with the existing owner-cell diffusivity, and the seam pair differences of the momentum face
viscosity on the B2 fan passage. Its numbers are quoted in item 10, where they set a sweep count and the limits of the negative
controls. Results go in the addendum at the end.

Summary of the results (addendum): with `turbulence.periodic_seam.diffusivity = "pair"` both faces of every seam pair carry one
value in the SST momentum face viscosity and in the k and omega diffusivities, and the global k and omega budgets of the periodic
box of ADR 0199 C1 close to 1e-14 of the destruction (the owner rule misses them by 1.7e-7 to 6.5e-2); on the B2 fan passage the
pairs are equal to the bit where the owner rule differs by 13 to 27 percent. The defaults are bit-identical. Two set-ups were
corrected by amendments before their corrected runs (the S1 harness, and a graded seam for F1 so that the swapped weight can be
seen). Every check met its limit, every negative control fired, and every mutation made its check fail.

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

### Amendment 1 (2026-11-06): the static refusals of S1, written after its first run and before the corrected run

S1 ran once (release; code of D5 in 7c3b6fd, tests in a5f8cf3). Every accepted case met: CH on SIMPLEC with `pair`, CH on
`pimple` with `pair`, CH with `owner`, `k_epsilon` with `owner`, laminar CH with `owner`, NP with `pair`. The first refusal case,
laminar CH with `pair`, stopped the test in its own harness: the test read the static errors with `semantic_errors` after
`Case::parse_toml(..).unwrap()`, but `Case::parse_toml` runs the semantic checks itself and returns them as its error (here
`turbulence.periodic_seam.diffusivity`, line 25: `periodic_seam.diffusivity = "pair" is available with turbulence.model =
"k_omega_sst" only (ADR 0205), not with "laminar"`), as ADR 0199 item 1 records for its own refusal. The refusal is the one R1
registers, at its path and naming `k_omega_sst`; the `k_epsilon`, `spalart_allmaras` and `harmonic` cases were not reached. The
test as run stays as an ignored record (`s1_first_run_record`).

The corrected S1 reads the static diagnostics of a text as the error of `Case::parse_toml` or, when the text parses, as its
`semantic_errors`; both come before `Simulation::new`, so MU7 still separates a static refusal from the model's own refusal of the
option. The cases, the paths, the message condition and the R2 case are unchanged. No limit changes.

### Amendment 2 (2026-11-06): a graded seam for F1, written before any mutation was run

Found while preparing the mutations, before any of them ran: MU2 (the swapped weight) cannot be seen by F1 as registered. On
BOX the two faces of every pair are at the same distance from their owners (`w = 1/2` exactly, so the swapped weight gives the
same bits) and on SEC by symmetry up to round-off (P0: areas equal to 1e-15), so F1 would at best catch MU2 by a last-bit
accident. F1 stays as registered; MU2 is run against it once as registered and its result is recorded. The corrected set-up adds
**F1g**: F1 on **GBOX**, `box_2d_graded(16, 4, 1.0, 1.0, [Expansion(4), Uniform])` periodic in x (cell widths growing by 4 from
`xmin` to `xmax`, so `dn` at `xmax` is four times `dn` at `xmin` and `w = 0.8`), model walls `ymin`, `ymax`, the F1 field
`nu_t = 0.1 (1 + 0.5 sin(2 pi x))` and the limits of F1 (exact, exact, exact, `>= 1e-3`). MU2 must make F1g fail. Nothing else
changes.

### Addendum: results (2026-11-06)

Commits: registration d0dd005, V0b record d87c745 (on main 9631754, before any code), code 7c3b6fd (`rcfd-fv/src/geometry.rs`
`PeriodicPair` and `FvGeometry::periodic_pairs`, `rcfd-turb/src/eddy.rs` `SeamDiffusivity` and `pair_periodic_faces`,
`rcfd-turb/src/sst.rs` `set_periodic_diffusivity` and the paired `nu_face`, `gk_f`, `gw_f`, `rcfd-turb/src/model.rs`
`SstModel::set_option`, `rcfd-api/src/schema.rs` the optional `turbulence.periodic_seam`, `rcfd-api/src/turb.rs` R1, the option and
the laminar pair rule, `docs/case-schema.md`), tests as first run a5f8cf3, amendment 1 d825071 and the corrected S1 9585a25,
amendment 2 0e89ade and F1g 46469a9, and 22efe0b (a float literal of the tests written without excess digits for clippy, the
same bits). Every check was run once, in release; the exceptions are named where they occur. Tests:
`crates/rcfd-turb/tests/f295_seam_diffusivity.rs`, `crates/rcfd-api/tests/f295_seam_diffusivity.rs`,
`crates/rcfd-api/tests/f295_default_bits.rs`, with the case texts in `crates/rcfd-api/tests/f295_common/mod.rs`.

| Id | Measured | Limit | Result |
|---|---|---|---|
| P0 | BOX 4 pairs of 8 periodic faces, SEC 24 of 48; each face in one pair, each the other's partner; largest relative area difference 0 (BOX), 9.99e-16 (SEC); every `dn > 0` | as stated | met |
| R3 | `periodic_pairs` and `set_periodic_diffusivity(Pair)` return `the periodic faces are not paired: the partner of face 108 is face 113, whose partner is face 109`; the model stays `Owner`; `Owner` is accepted | as stated | met |
| F1 | BOX and SEC: 0 pairs with unequal bits, 0 off the D1 bits, 0 other faces changed; owner pair difference 7.83e-2 (BOX), 0.155 (SEC) of the largest `nu_face` | exact; `>= 1e-3` | met |
| F1g | GBOX (amendment 2), `w = 0.799999999999999` for all 4 pairs: 0, 0, 0; owner 8.83e-2 | exact; `>= 1e-3` | met |
| G1 | `pair`: BOX `q` 7.1e-16, `u_x` 6.5e-17, `u_y` 1.0e-16, SEC `q` 1.8e-16; `owner`: 8.67e-3, 8.79e-3, 4.88e-3, 3.23e-2 | `<= 1e-13`; `>= 1e-5` | met |
| C1p | largest `abs(B_k) / D_k` over the steps: Euler upwind 1.78e-14, Euler `linear_upwind` 1.38e-14, BDF2 upwind 1.67e-14, BDF2 `linear_upwind` 1.47e-14. Preconditions: F1 = 1 in every cell of every sweep; last-sweep change at most 8.5e-16 (k), 7.6e-16 (omega). Non-vacuity: smallest step change of k 2.79e-2 to 3.08e-2; largest seam `nu_t` difference 1.35e-2 to 1.42e-2 | `<= 1e-10`; `<= 1e-13`; `>= 1e-3` | met |
| C1pN | `owner`, smallest / largest `abs(B_k) / D_k`: Euler upwind 2.20e-6 / 2.876e-2, Euler `linear_upwind` 6.85e-7 / 3.16e-2, BDF2 upwind 5.34e-6 / 2.876e-2, BDF2 `linear_upwind` 1.39e-5 / 3.16e-2 | `>= 1e-8`; `>= 1e-3` | met (C1p can fail) |
| C2p | largest `abs(B_k) / D_k`, `abs(B_w) / D_w`: Euler upwind 9.7e-15, 8.2e-15; Euler `linear_upwind` 1.07e-14, 1.09e-14; BDF2 upwind 6.2e-15, 7.3e-15; BDF2 `linear_upwind` 6.2e-15, 7.6e-15. Preconditions met (F1 = 1; last-sweep change at most 1.0e-15, 7.6e-16). Non-vacuity: smallest step change k 2.89e-2, omega 0.263; seam `nu_t` difference up to 2.50e-2 to 2.60e-2 | `<= 1e-10`; `<= 1e-13`; `>= 1e-3` | met |
| C2pN | `owner`, smallest / largest of k and omega: Euler upwind 6.89e-6 / 5.91e-2, 4.82e-6 / 3.59e-2; Euler `linear_upwind` 4.96e-7 / 6.49e-2, 1.65e-7 / 3.90e-2; BDF2 upwind 2.40e-5 / 5.91e-2, 1.67e-5 / 3.59e-2; BDF2 `linear_upwind` 4.98e-5 / 6.49e-2, 3.58e-5 / 3.90e-2 | `>= 1e-8`; `>= 1e-3` | met (C2p can fail) |
| C1p30 | report only, `pair`, 30 sweeps: largest `abs(B_k) / D_k` 5.12e-9 (upwind, both schemes), 1.13e-8 (`linear_upwind`); `abs(B_w) / D_w` 1.46e-10 to 4.50e-10; last-sweep change of k 7.1e-10 to 1.2e-9 | none | reported |
| S1 | first run (a5f8cf3): the accepted cases met, then the harness stopped at the first refusal (amendment 1); corrected (9585a25): every accepted case without a diagnostic; `pair` with laminar, `k_epsilon` and `spalart_allmaras` located statically at the key with `periodic_seam.diffusivity = "pair" is available with turbulence.model = "k_omega_sst" only (ADR 0205), not with "<model>"`; `harmonic` located at the key | as stated | met (corrected set-up) |
| S2 | content hash without the key 18a63e11897b386c, with `owner` 18a63e11897b386c, with `pair` ebf6531e45333a22; the written text of CH holds no `periodic_seam` | as stated | met |
| T1 | B2S, 5 iterations, 638 seam pairs: `pair` 0 with unequal bits, 0 off the D1 bits; `owner` 0.271 | exact; `>= 1e-2` | met |
| T2 | B2, 3 `pimple` steps, 638 pairs: 0, 0; `owner` 0.131 | exact; `>= 1e-2` | met |
| T3 | LAM, 3 iterations, 40 seam faces: `pair` 0 off the bits of `nu`; `owner` largest 2.712e-2 = 10.7 `nu` | exact; `>= 1.001 nu` | met |
| V0a | `baseline_hashes.rs` and `case_hashes_w48u.rs` unedited; both pass before the gate (below); the gate: pending | pass | met before the gate |
| V0b | 5 of 5 runs bit-identical to the record of 9631754 after the code: CH `pimple` 0xdada28263c5403a0, S4 sector 0xabd83aa0fd750ae7, LAM 0xd8e366294f570e5e, CH `k_epsilon` 0x051bdeb285ab7c4a, B2S 0x85db6064d7777a63 | bit-identical | met |
| V0c | V0c1: CH `pimple` with `owner` written gives the V0b record 0xdada28263c5403a0; V0c2: NP with `pair` and without the key, 100 iterations each, both (100, 7, 0xf30d3fbec2e549d2) | the record; equal | met |
| V0d | `f286_default_bits.rs` unedited; passes before the gate (below); the gate: pending | pass | met before the gate |
| V0e | the gate of the commit that adds this addendum: pending | pass, apart from the known flakes | pending |

**The S1 runs.** The first S1 run stopped in its harness (amendment 1). Before the corrected test existed, a failed edit of the
test file left the first-run harness in place and that test was run a second time, with the same output; nothing was chosen from
it. The corrected S1 then ran once.

**Mutations** (scratch, each applied alone to 46469a9 and reverted; the named test run once; the tree was clean after each):

| Id | Test | Measured | Result |
|---|---|---|---|
| MU1 | F1 | BOX: 4 pairs with unequal bits | fails |
| MU2 | F1 (as registered) | BOX: 0 pairs off the D1 bits (`w = 1/2`, as amendment 2 said); SEC: 19 of 24 pairs off the D1 bits in the last bit (the round-off of its distances), so F1 failed | fails (by round-off only on SEC) |
| MU2 | F1g | `w` became 0.2; 4 of 4 pairs off the D1 bits | fails |
| MU3 | C1p | Euler upwind `abs(B_k) / D_k` 2.876e-2 (the owner value) | fails |
| MU4 | C2p | Euler upwind `abs(B_k) / D_k` 1.02e-14, `abs(B_w) / D_w` 3.61e-2 | fails |
| MU5 | T3 | `pair`: 20 of 40 seam faces off the bits of `nu` | fails |
| MU6 | T1 | 638 of 638 pairs with unequal bits and off the D1 bits | fails |
| MU7 | S1 | laminar CH with `pair`: no static diagnostic | fails |

**Predictions.** PR1 met for F1 and for G1 with `pair` (6.5e-17 to 7.1e-16), falsified in size for the owner G1 of SEC (3.2e-2,
above the predicted 1e-2). PR2 met (at most 1.8e-14). PR3 met for `B_w` (1.5e-10 to 4.5e-10) and falsified for `B_k`
(5.1e-9 to 1.1e-8): with 30 sweeps the lag of k itself (last-sweep change 7e-10 to 1.2e-9) enters its budget through the seam
faces. PR4 met.

Before the gate, on 22efe0b in release: `cargo fmt --check` of `rcfd-fv`, `rcfd-turb`, `rcfd-api` and `cargo clippy -p rcfd-fv
-p rcfd-turb -p rcfd-api --all-targets -D warnings` clean; the non-ignored tests of `baseline_hashes`, `case_hashes_w48u`, `exp_rot_pc`, `f286_default_bits`, `f286_transient_sst`,
`physics_schema`, `regions`, `registers`, `sst_schema`, `turb_models` and `wall_options`: 60 passed, 0 failed.

**Questions added by the results:** none. Questions 1 to 3 stand.
