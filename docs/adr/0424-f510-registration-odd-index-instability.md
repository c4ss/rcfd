# ADR 0424: F-510 registration: odd-index instability of a rotating part on the model path (a cube on springs with its axial motion tied and a negative axial support, the closed-form divergence `s = +/- omega_a` beside the gyroscopically stabilised in-plane pair; the same cube with a positive axial support stable; a cyclic sector whose single index 0 loses stiffness first with the stress stiffening off, against the full rotor)

Status: **REGISTERED 2026-11-13, written before any code of F-510 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-510 was written, compiled or run: the code listed under "What was read" was read and
the arithmetic below was done by hand (no script, no build, no run). The only numbers used below that came from a run are those
of the registered runs of ADR 0416 and ADR 0420 as their addenda print them (disclosed under "Arithmetic"). Results go in the
addendum at the end; nothing above the addendum is changed after a run.

Parents: [ADR 0416](0416-f505-registration-gyroscopic-stabilisation.md) (F-505: the state matrix, the flag `Stable` iff
`max|Re s| <= 1e-8 max|s|`, otherwise `Unstable { growth: max Re s }`, the odd-index theorem of its D3, the cube on springs CBI /
CBA; its amendment 1: the registered odd-index check on the cube rested on a wrong torsion estimate, the pre-stress cancels the
spin softening on a rigid rotation about the axis; its remaining question 1 (a): register the odd-index check on the model path
before its run) and [ADR 0420](0420-f511-registration-sector-whirl-stability.md) (F-511: `RotatingSector::whirl_state` per
harmonic index; its P1 was vacuous, no odd count at its test speed; its remaining question 1 (b): a sector check with a single
index forced odd, for example with `stress_stiffening(false)`). Roadmap item F-510 (dep F-505). Number 0424 given by the lead.
Branch `f510-odd-index-instability` (base `ac540bd5`), worktree `w49i-work/f510`, scratch `w49i-work/f510-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here before the code and the runs and not changed after a run (a miss
becomes an ignored `*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, the hash tables or a golden file; no new dependency, no unsafe code; clean-room. Other agents edit
`crates/rcfd-fem/src/loads.rs`, `beam.rs` (F-514) and `rcfd-post` (F-512); nothing here touches them.

## What F-510 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. A rotating part whose K_eff has an odd number of
> negative eigenvalues (a cube on springs with a negative axial support, registered with its closed form) is flagged unstable
> by RotatingPart::whirl_state with the closed-form growth rate within 1e-8 relative, and the same part with an even number is
> flagged stable where the closed form says so. A cyclic sector whose single index (k = 0 or 2k = N) has an odd number of
> negative eigenvalues of K_eff is flagged unstable by RotatingSector::whirl_state with the full rotor's growth rate within 1e-8
> relative.

Clauses: (c1) every limit not stated in the acceptance registered here before the run; (c2) a cube on springs with a negative
axial support whose `K_eff` has an odd number of negative eigenvalues, registered with its closed form, is flagged `Unstable` by
`RotatingPart::whirl_state` with the closed-form growth rate within 1e-8 relative; (c3) the same part with an even number is
flagged `Stable` where the closed form says so; (c4) a cyclic sector whose single index (`k = 0` or `2k = N`) has an odd count
is flagged `Unstable` by `RotatingSector::whirl_state` with the full rotor's growth rate within 1e-8 relative.

Readings (registered now).

- "Within 1e-8 relative" (c2, c4): `|growth / growth_ref - 1| <= 1e-8`.
- "The same part with an even number" (c3): the same mesh, in-plane springs, tie and speeds, with the axial support of the
  opposite sign (`+k_a` per node instead of `-k_a`): its `K_eff` has 0 negative eigenvalues below `omega_0` and 2 above (even),
  and the closed form (D3) says stable at every speed of the set-up, with the frequencies of D3; "flagged stable where the closed
  form says so" is read as: at every speed where the closed form says stable, flagged `Stable` and the closed-form frequencies
  found within 1e-8 relative. An even count where the closed form says unstable (the anisotropic cube in its band with the
  negative axial support, count 2) is also registered (check V2), as the control that the theorem runs one way only.
- "The full rotor's growth rate" (c4): the growth rate of `RotatingPart::whirl_state` of the full rotor built from the sector
  (`cyclic::full_rotor`), with the same options, at the same speed.

## What was read (branch base ac540bd5, read only)

- `AGENT_RULES.md`; the brief; ADR 0416 (all, with amendment 1 and the addendum); ADR 0420 (all, with the addendum).
- `crates/rcfd-fem/src/rotating/whirl/state.rs` (all), `crates/rcfd-fem/src/rotating/whirl/state/sector.rs` (all),
  `crates/rcfd-fem/src/rotating.rs` (lines 1 to 330 and 456 to 478: the module comment, `RotatingOptions`, `Path`,
  `RotatingPart` with its fields, `unit_centrifugal_model`, `new`, `with_unit_prestress`, `operator`, `check_model`).
- `crates/rcfd-fem/src/model.rs` (`Constraint` with `Linear` and `LinearTerm`, `ElasticSupport::nodal_spring`, `add_support`
  with its `positive_finite` refusal, `check_supported`), `crates/rcfd-fem/src/mpc.rs` (`uses_transformation`, `add_supports`),
  `crates/rcfd-fem/src/error.rs` (`UnknownSet`, `ComponentOutOfRange`).
- Tests: `si_rotating_whirl_state.rs` (lines 1 to 370 and 579 to 620: `Checks`, `closed`, `cube`, `uniform`,
  `translation_share`, CBI), `si_rotating_whirl_sector_state.rs` (lines 1 to 470: DS, `full_of`, `ds_options`, `omega_ref`,
  `matching`, `union_of`, `flag_of`, the D/B1/L1/U1/F1/P1 test).

## Design

Notation of ADR 0411, 0416 and 0420: speed `Omega >= 0` about `a = e_z`; `K = K_eff(Omega) = K_0 + Omega^2 D`, `D = K_sigma,1 -
M_perp` (both effects; `D = -M_perp` with the stress stiffening off); `M` symmetric positive definite; `G = 2 Omega C` skew; the
pencil `Q(s) = s^2 M + s G + K`; the state matrix and the flag of ADR 0416 D1 to D3.

### D1. The odd-index theorem (ADR 0416 D3, restated as used)

`det(s^2 I + s G^ + K^) = prod_j (s - s_j)` (`2 n` roots); at `s = 0`: `det K^ = prod_j s_j`. If every root is imaginary they
come in conjugate pairs and `prod s_j = prod |s_j|^2 >= 0`. So `det K < 0` (an odd number of negative eigenvalues) forces a root
off the imaginary axis, and by the symmetry of the spectrum one with `Re s > 0`: unstable for any skew `G`. The theorem gives the
flag, not the growth rate: the growth rate needs the closed form of the set-up.

### D2. The negative axial support: why the library needs an opt-in, and which one

A negative axial support cannot be built from the model as it stands: `SolidModel::add_support` refuses a spring stiffness that
is not positive (`positive_finite`), and nothing in the rotating frame softens the axial motion (`M_perp` projects out the axial
component; `K_sigma,1` vanishes on rigid translations). Two options:

- (a) **`RotatingPart::with_nodal_springs(self, set, component, stiffness) -> Result<RotatingPart>`**: adds `stiffness` (any
  finite value, negative too) to the diagonal entry of `K` (the part's stored stiffness, the one `operator` starts from) at the
  equation of `component` of every node of the node set `set`, after the pre-stress was computed. The springs therefore do not
  take part in the static solve of the pre-stress (the function's documentation says so); every analysis of the part
  (`operator`, `modes`, `campbell`, `whirl_modes`, `whirl_state`, ...) then sees them, on either path (`Path::Elimination`: a
  fixed equation is dropped by the reduction afterwards; `Path::Transformation`: `K` carries the model's supports already and
  is reduced by `T^T K T`). Refusals: an unknown node set (`FemError::UnknownSet`), a node without the component
  (`FemError::ComponentOutOfRange`), a non-finite stiffness (`"stiffness"`).
- (b) A model support that accepts a negative stiffness: it would reach the static solves (an indefinite `K` there), the case
  files and their schema golden.

**(a) is taken** on the maintainer's standing instruction of 2026-11-11 (take the recommendation): it is confined to the rotating
module, no existing function changes (nothing calls the new one; `SolidModel::add_support` keeps its refusal), and a negative
stiffness never reaches a static solve. The code: a new file `crates/rcfd-fem/src/rotating/whirl/state/springs.rs`, a child
module of `rotating::whirl::state` (so that it reads the private fields of `RotatingPart`), and one line `mod springs;` in
`state.rs`; an inherent method, so no `pub use` is needed. This is the only library change of F-510.

### D3. The tied cube CT: closed forms (first principles)

**Set-up.** The cube of ADR 0416 (one Hex8, `h = 0.1 m`, centred on the origin, steel, `m = rho h^3 = 7.85 kg`, nodal springs
`k_x = m omega_x^2 / 8`, `k_y = m omega_y^2 / 8` in `x`, `y` at all 8 nodes), but instead of `u_z = 0` at every node: **the
axial motion tied**, seven linear constraints `u_z(node i) - u_z(node 0) = 0`, `i = 1 .. 7` (so the cube's axial motion is one
unknown, the axial translation, and the rocking about `x` and `y` is removed), and **an axial support** `k_a = m omega_a^2 / 8`
per node (`ElasticSupport::nodal_spring("all", 2, k_a)`, positive, in the model). The part `CT+` is `RotatingPart::new` of this
model; the part `CT-` is the same followed by `.with_nodal_springs("all", 2, -2 k_a)`: net axial support `-k_a` per node, the
negative axial support. `omega_0 = 1000 rad/s`, `omega_a = 0.6 omega_0 = 600 rad/s`; isotropic `omega_x = omega_y = omega_0`
(CTI) or anisotropic `omega_x = omega_0`, `omega_y = 1.2 omega_0` (CTA). Options `RotatingOptions::new(4).coriolis(true)`, both
effects on.

**The pre-stress.** The unit centrifugal load has no axial component. In the static solve the tied axial unknown `q_z` carries
the sum of the axial rows: the elastic part sums to zero for any displacement (an element's nodal forces are in equilibrium),
the load is zero, the springs give `8 k_a q_z`; so `q_z = 0` (for any `k_a != 0`), and the in-plane solution is that of the cube
of ADR 0416 with `u_z = 0` at every node (the same equations). So `K_sigma,1` is that of ADR 0416's cube, and it is the
pre-stress of CT- too: the support's sign does not enter (and the API spring of CT- is outside the static solve by D2 anyway).

**Decoupling.** Let `1_z` be the uniform axial translation (the one axial unknown of the tie). `K_0 1_z = 0` on the elastic part
(a rigid translation), `K_sigma,1 1_z = 0` (the geometric stiffness acts on gradients), `M_perp 1_z = 0` (the projection removes
the axial component), the consistent mass is `m_ab delta_ij` per component so `1_z^T M v = 0` for every in-plane `v`, and `C 1_z
= 0`, `1_z^T C = 0` (`a x e_z = 0`, `(a x w)_z = 0`); the springs are diagonal. So the reduced problem (17 unknowns) is the
direct sum of ADR 0416's cube problem (16 in-plane unknowns, CBI or CBA, exactly, with the same pre-stress) and the axial
`1 x 1` problem `m s^2 + 8 k_net = 0`, `m = 1_z^T M 1_z` (the consistent mass sums to the total mass):

- CT- (`k_net = -k_a`): `s = +/- omega_a`, **a real pair: the axial divergence with growth rate `omega_a` exactly**; one
  negative eigenvalue of `K_eff`.
- CT+ (`k_net = +k_a`): `s = +/- i omega_a`, a frequency `omega_a`, no negative eigenvalue.

**The in-plane part** (from ADR 0416 D5 and its registered run): CBI below `omega_0` positive definite, stable, the translation
pair at `omega_0 -/+ Omega`; above `omega_0` two negative eigenvalues of `K_eff` (the translation pair), gyroscopically
stabilised, the pair at `Omega -/+ omega_0`, both backward; the torsion stays near `sqrt(3) omega_0` at every speed (amendment 1
of ADR 0416: the pre-stress cancels the spin softening on a rigid rotation about the axis; measured 1731.8 to 1731.9 rad/s at
`1.2 .. 2.0 omega_0`), so it stays positive; the elastic modes of the cube (of order `1e5` rad/s) are not affected at these
speeds. Measured in ADR 0416's run: CBI counts `0, 2, 2, 2` at `0.5, 1.2, 1.5, 2.0 omega_0`, flagged `Stable` at each. CBA (D5
anisotropic of ADR 0416): in the band `omega_x < Omega < omega_y` one negative eigenvalue and the divergence `sigma = sqrt(lambda_+)`
(at `1.1 omega_0`: `sigma = 99.5893` 1/s, ADR 0416 arithmetic 2), count 1.

Hence the counts and closed forms of the set-up (CTI at `Omega / omega_0 = 0, 0.5, 1.2, 1.5, 2.0`):

| Part | Speeds | Count of `K_eff` | Parity | Closed form |
|---|---|---|---|---|
| CTI- | `0, 0.5` | 1 | odd | unstable, growth `omega_a`; the pair at `omega_0 -/+ Omega` (at 0 double, `omega_0`) |
| CTI- | `1.2, 1.5, 2.0` | 3 | odd | unstable, growth `omega_a`; the pair at `Omega -/+ omega_0` |
| CTI+ | `0.5` | 0 | even | stable: `omega_a`, `omega_0 -/+ Omega` |
| CTI+ | `1.2, 1.5, 2.0` | 2 | even | stable: `omega_a`, `Omega -/+ omega_0` |
| CTA+ | `1.1` | 1 | odd | unstable, growth `sigma(1.1) = 99.5893` |
| CTA- | `1.1` | 2 | even | unstable (the closed form says so): the two positive real roots `omega_a = 600` and `sigma(1.1)`, growth `omega_a` |

No frequency of the set-up coincides with `omega_a = 600`: the pair at `500, 1500` (`0.5`), `200, 2200` (`1.2`), `500, 2500`
(`1.5`), `1000, 3000` (`2.0`), `1000, 1000` (`0`); the torsion near 1732.

### D4. The sector: a single index forced odd (c4)

**Set-up.** The disc sector DS of ADR 0411 / 0420 (`N = 6`, the annular Hex20 sector, hub clamped, cyclic about `e_z`), with
`RotatingOptions::new(4).modal(ds_modal()).coriolis(true).stress_stiffening(false)` on the sector and on its full rotor alike
(the full rotor by `full_rotor`, hub clamped, `RotatingPart::new(full, e_z, 0, same options)`). With the stress stiffening off,
`D_k = -M_perp,k <= 0` on every index: `K_eff,k(t) = K_0,k - t M_perp,k` decreases in the Loewner order as `t = Omega^2` grows,
so every eigenvalue of `(K_eff,k, M_k)` is non-increasing in `t` and **the count of negative eigenvalues of each index is
non-decreasing in `Omega`** (bisection on "count `>= j`" is valid), and a loss is guaranteed for every index with in-plane
motion (ADR 0420 D3 (ii)). Below `omega_k,1` (the lowest rest frequency of index `k`) no index has lost (`K_0 - t M_perp >= K_0
- t M`).

**Which index loses first (expectation, from ADR 0420 arithmetic 2, not a check):** index 0's torsion (a tangential motion,
`phi^T M_perp phi ~ phi^T M phi`, at about 2420 rad/s `= 1.46 omega_ref` at low speed), with the pre-stress off fully softened,
loses at about `1.2 .. 1.5 omega_ref`; index 1's in-plane pair is near 5000 rad/s; the bending modes (`omega_ref = 1655.32`
rad/s for index 1 and up) carry little in-plane motion and lose later. So index 0 (single) is expected to lose first, with a count
of 1 (a simple lowest eigenvalue of a real problem), odd: flagged `Unstable` by D1. With every other index positive definite
there, the others are stable (`K > 0`, `M > 0`, `G` skew: stable), so the full rotor's growth rate is index 0's.

**The rule (fixed now; its values are computed by the run and printed, not chosen):**

1. `omega_ref` as ADR 0420 (the lowest rest frequency over `k = 0 .. 3` by `whirl_modes(k, 0)`).
2. Counts `c_k(Omega)` = the count of `RotatingSector::whirl_state(k, Omega).state.loss` (0 for `None`; a zero pivot, `None`
   inside the loss, is treated as "not lost" for the bisection and is not expected at the bisection's points).
3. For each `k = 0 .. 3`: `Omega_hi,k` the first of `2, 4, 8, 16 omega_ref` with `c_k >= 1`; 60 halvings of `[0.8 omega_ref,
   Omega_hi,k]` on `c_k >= 1` give `Omega_L,k` (the upper end); `infinity` if none by `16 omega_ref` (recorded).
4. `k_o` = the single index (`0` or `3`) with the smaller `Omega_L,k` (it must be finite; otherwise the set-up is defective).
   Its second crossing `Omega_2`: the first of `2, 4, 8, 16 omega_ref` at or above `Omega_L,k_o` with `c_k_o >= 2`, then 60
   halvings of `[Omega_L,k_o, that speed]` on `c_k_o >= 2`; `infinity` if none by `16 omega_ref`.
5. `Omega_next = min(Omega_2, min_{k != k_o, Omega_L,k > Omega_L,k_o} Omega_L,k, 16 omega_ref)`, and **the test speed
   `Omega_T = sqrt((Omega_L,k_o^2 + Omega_next^2) / 2)`** (the middle in `Omega^2`, so that by the monotone counts `c_k_o = 1` and
   every index that had not lost at `Omega_L,k_o` has still not lost, and the losing eigenvalue is well away from zero). If a
   pair index has lost before `Omega_L,k_o` (against the expectation), it is excluded from `Omega_next` and stays as it is at
   `Omega_T` (it may be stable or not); this is printed.

## Arithmetic (done before the code, by hand; disclosed)

1. Masses and springs: `m = 7850 x 0.001 = 7.85 kg`; `k_n = m omega_0^2 / 8 = 981 250 N/m`; `k_a = m omega_a^2 / 8 = 7.85 x
   360 000 / 8 = 353 250 N/m`; CT- adds `-706 500 N/m` per node; `k_y = m (1.2 omega_0)^2 / 8 = 1 413 000 N/m` for CTA.
2. `sigma(1.1 omega_0)` of CTA from ADR 0416 D5 (`S = 1 + 1.44 + 2.42 = 4.86`, `P = (1 - 1.21)(1.44 - 1.21) = -0.0483`, `D =
   S^2 - 4P = 23.8128`, `lambda_- = -(S + sqrt D)/2 = -4.86996`, `lambda_+ = P / lambda_- = 0.0099180` in `omega_0^2`, `sigma =
   0.099589 omega_0`), as ADR 0416 arithmetic 2 (99.5893 1/s). The test computes it by the same formula (`closed` of ADR 0416's
   test, copied).
3. Rounding of the cube (ADR 0416 arithmetic 5): `||A|| ~ 3e5`, eigenvalue errors `~7e-10` absolute: `~1e-12` relative to
   `omega_a = 600`, `~7e-12` relative to `sigma = 99.6`, `~4e-12` relative to the smallest pair frequency 200. Estimates `<= 1e-11`
   against the limits `1e-8`. The axial eigenvalue is simple (D3: no coincidence), and decoupled, so its condition is of order 1.
4. Sector (ADR 0420 arithmetic 3): full `|s|` up to `~3e5`, eigenvalue errors `~1e-9` absolute; the growth at `Omega_T` is of the
   order of `|s|` of the losing motion, `~1e2 .. 1e3` 1/s (by D4 step 5 the losing eigenvalue of `K_eff` is about half its
   distance to the next loss), so `~1e-11 .. 1e-12` relative; with the stress off there is no pre-stress, so the sector and the
   full rotor differ only by the two QR runs. Sizes as ADR 0420: index unknowns `42, 84, 84, 42`, full 252; the bisections about
   `4 x 60 + 60` state solves of order `<= 168`, about a few seconds in release.

## Checks (limits fixed now)

| # | Clause | Check | Limit | Estimate behind it |
|---|---|---|---|---|
| O1 | c2 | CTI- at `0, 0.5, 1.2, 1.5, 2.0 omega_0`: `loss.negative` = `Some(1), Some(1), Some(3), Some(3), Some(3)`; flagged `Unstable`; `|growth / omega_a - 1|` | exact; each; `<= 1e-8` | D3; `~1e-12` (arithmetic 3) |
| O2 | c2 | CTI- at those speeds: every eigenvalue other than the two nearest `+/- omega_a` has `|Re s| <= 1e-8 max|s|` (only the axial divergence grows, the in-plane part stays stabilised); and the eigenvalue nearest `-omega_a` within `1e-8 omega_a` of it | each; `<= 1e-8` | D3 |
| O3 | c2 | CTI- at those speeds: the closed-form pair present: for each of `omega_0 -/+ Omega` (below `omega_0`) resp. `Omega -/+ omega_0` (above), `min_j |s_j - i omega_cf| / omega_cf` (at 0 both `omega_0`, two distinct eigenvalues each within the limit) | `<= 1e-8` | `~4e-12` |
| V1 | c3 | CTI+ at `0.5, 1.2, 1.5, 2.0 omega_0`: `loss` = `None, Some(2), Some(2), Some(2)` (even); flagged `Stable`; the eigenvalue `i omega_a` and the pair `omega_0 -/+ Omega` resp. `Omega -/+ omega_0` present as in O3 | exact; each; `<= 1e-8` | D3 |
| V2 | c3 (control) | CTA- at `1.1 omega_0`: `loss.negative == Some(2)` (even); flagged `Unstable` with `|growth / omega_a - 1|`; the second largest real part against `sigma(1.1)`: `|Re s_(2) / sigma - 1|` | exact; flag; `<= 1e-8`; `<= 1e-8` | D3; arithmetic 2, 3 |
| V3 | c2 | CTA+ at `1.1 omega_0`: `loss.negative == Some(1)` (odd); flagged `Unstable` with `|growth / sigma(1.1) - 1|` | exact; flag; `<= 1e-8` | arithmetic 3 |
| Q1 | c2 | the stored residual of every returned mode of O1 to V3 | `<= 1e-9` | `~1e-11` (ADR 0416 Q1 on the cubes: up to 8.8e-11) |
| S0 | c4 | the derived speeds printed (D4: `omega_ref`, `Omega_L,k` per index, `k_o`, `Omega_2`, `Omega_next`, `Omega_T`); `Omega_L,k_o > 0.8 omega_ref` | printed; holds | D4 |
| S1 | c4 | at `Omega_T`: `c_k_o == 1` (odd); index `k_o` flagged `Unstable` | each | D1, D4 step 5 |
| S2 | c4 | the full rotor at `Omega_T`: flagged `Unstable`; `|growth_k_o / growth_full - 1|` | flag; `<= 1e-8` | arithmetic 4 |
| S3 | c4 | Sylvester: the full rotor's count equals the sum of the indices' counts (each pair count even), and it is odd | exact | ADR 0420 D2 |
| S4 | c4 | the union of the four indices' eigenvalues against the full rotor's (count; ADR 0420's matching rule): `max |s_sector - s_full| / |s_full|`; per index the sector's flag equals the D3 flag of the full eigenvalues matched to it | count; `<= 1e-8`; each | ADR 0420 U1, F1 (6.7e-14 there) |
| R1 | D2 | refusals of `with_nodal_springs` on CT: an unknown set (`UnknownSet`), component 3 (`ComponentOutOfRange`), stiffness `NaN` and `inf` (`"stiffness"`); and `with_nodal_springs("all", 2, 0.0)` leaves `operator(1.5 omega_0)` bit-identical | each | D2 |
| R2 | rule 3 | existing tests pass unchanged (release): `si_rotating_whirl_state`, `si_rotating_whirl_sector_state`, `si_rotating_whirl`, the unit tests of `rotating`; `cargo check -p rcfd-post --tests` | pass | additive edit |

Negative controls (expected to fail the property, each with a limit):

| # | Control | Expected |
|---|---|---|
| N1 | CTI- at `1.5 omega_0` with `coriolis(false)`: the in-plane pair diverges too (`sqrt(Omega^2 - omega_0^2) = 1118.034 > omega_a`) | flagged `Unstable`, `|growth / (sqrt(1.25) omega_0) - 1| <= 1e-8` (growth no longer `omega_a`) |
| N2 | V2 itself: an even count with the closed form unstable is flagged `Unstable` (the theorem runs one way only) | as V2 |

Planned mutations (on a `git archive` copy in `f510-scratch/mut`, each run once against the new test file and the unit tests of
`rotating::whirl::state`; what catches each is recorded):

1. `with_nodal_springs` adds `|stiffness|` (the sign lost): O1 (counts `0, 0, 2, 2, 2`, flagged `Stable`), V2, S-checks not.
2. `with_nodal_springs` adds to component `(component + 1) % 3`: O1 (axial support stays `+k_a`), O3.
3. `with_nodal_springs` adds to the first node of the set only: O1 growth (net axial `8 k_a - 2 k_a > 0`: stable).
4. `with_nodal_springs` adds half the stiffness: O1 growth (net `0`: a zero eigenvalue, then neither the count nor the growth).
5. In `state_solve` the growth taken as the minimum real part of the unstable spectrum (`fold(INFINITY, f64::min)` over
   `Re s`): O1 (growth `-omega_a`), V2, S2.
6. In `RotatingSector::whirl_state_at`, `G = 0` for a single index (`multiplicity == 1`): S2 (index 0's growth then that of the
   undamped divergence), S4.
7. In `RotatingSector::whirl_state_at`, `K_eff` at speed 0 for a single index: S1 (count 0, stable).
8. `STABILITY_TOLERANCE = 1.0` (every speed `Stable`): O1, V2, V3, S1, S2.

## Decision rule

- **done**: O1 to R2 met, N1 and N2 as expected, and all eight mutations caught.
- **partial**: O1, V1 and S2 met (clauses c2 to c4 in their core reading) with another check missed and its cause diagnosed (a
  `*_known_miss` record), or a mutation not caught, or a check met only after an amendment made after its run.
- **todo**: O1 or S2 missed, or a state solve refused.

## Limits (what is not covered)

The negative support exists only on the rotating part (D2); a model, a case file or a static solve cannot carry one. The cube's
axial motion is tied, so the cube has no rocking (its rocking with a negative axial support would need a closed form of the
pre-stressed tilt, not derived here). One sector geometry with the stress stiffening off (with both effects on, ADR 0420 found
the first loss at index 1, a pair, at 13.46 `omega_ref`, and no single index lost by `16 omega_ref`, so no odd single index is
reachable there without another disc). The odd count of a pair index is impossible (ADR 0420 D1: always even). Damping, tracking
over speed and the flag at a defective double eigenvalue, as ADR 0416.
