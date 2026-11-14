# ADR 0530: F-614 registration: a non-equilibrium quadratic displacement test for the error-estimate samples of an opted Hex8

Status: **REGISTERED 2026-11-14, written before any code of F-614 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no test of F-614 was written or run. The arithmetic under "Arithmetic done before the code" was done
with plain numpy (dense, no rcfd code) in `w49i-work/f614-scratch` (`proto614.py`, log `proto614.log`; disclosed, its numbers
quoted below). Results go in the addendum at the end; nothing above the addendum is changed after a run.

Roadmap item F-614 (deps F-588), the follow-up proposed in the addendum of
[ADR 0525](0525-f588-registration-hex8-formulations-on-the-remaining-linear-paths.md) (open question 1; mutation MU12 uncaught,
check ES3b missed). Number 0530 given by the lead. Branch `f614-work` from local main (291a210c), worktree `w49i-work/f614`,
scratch `w49i-work/f614-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored `*_known_miss`
test with the limit intact and the measured number recorded); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe code; clean-room. The
brief overrides rule 9 for this item: the F-614 entries of `docs/ROADMAP.md` and `docs/roadmap.json` are updated on the branch in a
separate last commit (the svgs are not regenerated). Other agents edit F-604, F-597 and F-609 files; this item touches none of them.

## What F-614 asks

> Every limit not stated here is registered in the ADR before the run. A non-equilibrium quadratic displacement test for
> estimate::element_samples with an opted Hex8 (BBar and Incompatible), which catches the mutation MU12 of ADR 0525 (sampling with
> the compatible strain).

Test-only: no library change, no case-file key, defaults and `baseline_hashes` untouched. The test is a new file
`crates/rcfd-fem/tests/si_hex8x_estimate.rs` (existing tests are not edited).

## What was read (read only)

ADR 0525 (D2 patch recovery, Amendment 3, Addendum 1), `crates/rcfd-fem/src/estimate.rs` (`sampling_rule`, `element_samples`,
`fit_patch`, `recover_prepared`, `least_squares`), `crates/rcfd-fem/src/hex8x.rs` (`strain_at`, `mean_gradients`, `bbar_at`,
`mode_gradients`), `tests/si_hex8x_linear.rs` (helpers, `test_bbar`, ES1 to ES3). No source outside the repository.

## Findings that shape the design (from reading)

1. `estimate::element_samples` is private. The public observable of the samples is `estimate::recover`: the nodal stress at an
   interior node is the value at the patch origin of the least-squares fit (default basis `Linear` for Hex8, terms `1, x, y, z` in
   local coordinates `(x - origin) / scale`) over the samples of the elements around the node. The nodal value is therefore the
   constant coefficient `c_0`, independent of the scale.
2. The sampling rule of Hex8 is `quadrature::gauss_1`: ONE sample per element, at the centroid `xi = 0`, with the stress
   `D eps(xi = 0)`. The fit of an interior node uses its 8 elements: 8 points for 4 terms.
3. At `xi = 0` the strain of `Incompatible` is the compatible strain: the mode gradients of `hex8x::strain_at` are
   `det J_0 / det J(xi) J_0^{-T} dP/dxi` with `dP/dxi = -2 xi_k`, which is zero at `xi = 0` (and `det J_0 / det J(0) = 1`).
   So for an opted element of `Incompatible` the sample is the compatible sample up to the rounding of the extra zero term. A
   mutation of `element_samples` to the compatible strain is an EQUIVALENT mutant for `Incompatible` (no input can distinguish
   it); this, as much as the symmetry of the bending box, is the cause of the ES3b miss of ADR 0525, and a field of any shape
   cannot catch MU12 for `Incompatible` at this rule. The check for `Incompatible` is therefore a positive one (the library
   samples equal the independent compatible-strain computation to rounding), not a discriminating one; it is registered as such.
   `BBar` is discriminating: its centroid strain replaces the dilatation by the volume mean of the rule.
4. For a quadratic displacement `u` the compatible dilatation is linear in `x`; the B-bar centroid strain differs from the
   compatible one by `(mean - centre value) / 3` on the three normal components, which is nonzero on a distorted element
   because `det J` weights the mean (and because the trilinear interpolation of a quadratic field is not exactly quadratic).

## Design

Mesh: the F-374 patch box of ADR 0493 reading (a2), origin `[0.1, -0.2, 0.3]`, lengths `[1.2, 1, 0.8]`, division `[4, 3, 3]`
(`hex_box`), 12 interior nodes, every interior node moved deterministically by a formula in its coordinates (so that an
independent numpy prototype shares it): `p_k += 0.3 h_k sin(a_k)` with `h = (0.3, 1/3, 4/15)` the cell sizes and, with
`x, y, z` of the undisplaced node, `a_0 = 7.1 x + 3.3 y + 5.9 z + 0.5`, `a_1 = 4.3 x - 6.1 y + 2.7 z + 1.9`,
`a_2 = -3.7 x + 5.3 y + 6.9 z + 3.1`. A node on a face of the box is not moved. Material: steel `E = 2e11`, `nu = 0.3`, no
temperature.

Displacement (applied directly to the nodes, no solve; the vector index is `dof_map().equation(node, c)`), with `x, y, z` the
node coordinates:

```
u_x =  4e-3 x^2 - 1e-3 y z
u_y = -2e-3 y^2 + 1.5e-3 x z
u_z =  3e-3 z^2 + 2e-3 x y
```

Its dilatation is `8e-3 x - 4e-3 y + 6e-3 z` (nonuniform) and it is NOT an equilibrium solution: the Navier residual
`b = -(lambda + mu) grad(div u) - mu lap u` is, because here `lap u = grad div u = (8e-3, -4e-3, 6e-3)`, equal to
`-(lambda + 2 mu) (8e-3, -4e-3, 6e-3)`, constant and nonzero. Only the samples are compared; the stresses are not a solution of
anything.

Independent expectation in the test: for each element the test computes the centroid position, the compatible strain `B(0) u`
from the element's own gradients (`element_for(Hex8).point`), and the B-bar strain
`eps_ii += (tr(u^T mean) - tr(u^T grad(0))) / 3` with `mean` the det-J-weighted mean of the gradients over the 2 x 2 x 2 rule
(written in the test, like `test_bbar` of `si_hex8x_linear.rs`, not through `hex8x`), the stress `D eps`, and then the
least-squares fit `[1, dx, dy, dz] c = sigma` over the 8 elements of each interior node by the test's own normal equations
(Gauss elimination), `c_0` being the expected nodal stress. `smax` is the largest absolute component of the compatible centroid
stresses.

## Arithmetic done before the code (disclosed)

`proto614.py` (numpy, own Hex8 shape functions, standard connectivity order, the same node formula, the same field). Amplitude
of the node motion `0.2 h`: samples differ (B-bar against compatible, relative to the largest compatible stress) `9.0e-4`,
nodal stress difference (largest over the 12 interior nodes) `2.0e-4 smax`, smallest `1.8e-6 smax`; `0.3 h` (the registered
amplitude): `1.47e-3`, `3.67e-4 smax`, smallest `1.83e-5 smax`; `0.4 h`: `2.36e-3`, `9.0e-4 smax`, `1.2e-5 smax`. The amplitude
`0.3 h` was chosen as the middle of the range, safe against invalid elements; no other field or mesh was tried. The rounding of
the library and of the test is estimated at `1e-14 smax` (8 samples, a 4-term fit, condition below `1e2`).

## Checks (limits fixed now; every aggregated error is required finite before it is compared, a NaN misses)

| id | check | limit | estimate behind it |
| --- | --- | --- | --- |
| E0 | the field is not an equilibrium solution: `|b| / E` of the closed form above | `> 1e-3` | `1.45e-2` |
| E1 | BBar: `recover` nodal stress at every interior node against the independent B-bar expectation, `max abs(lib - exp) / smax` over nodes and the six components; every interior node recovered | `<= 1e-10` | rounding `1e-14` |
| E2 | BBar margin: the independent B-bar expectation against the independent compatible expectation, largest over the 12 nodes of `max abs(exp_bbar - exp_comp) / smax` | `>= 1e-5` | prototype `3.7e-4` |
| E2b | the same, the smallest over the 12 nodes | `>= 1e-6` | prototype `1.8e-5` |
| E3 | BBar: the library nodal stress against the independent COMPATIBLE expectation, `max abs(lib - exp_comp) / smax` (the check that catches MU12) | `>= 1e-5` | equals E2 when E1 is met: `3.7e-4` |
| E4 | Incompatible: `recover` nodal stress against the independent compatible expectation (the sample is the compatible one at `xi = 0`, finding 3), `max abs(lib - exp_comp) / smax` | `<= 1e-10` | rounding `1e-14` |
| E4b | Incompatible: the library nodal stress against the independent B-bar expectation (the formulations differ at the samples) | `>= 1e-5` | `3.7e-4` |
| E5 | Full control: `recover` nodal stress against the independent compatible expectation | `<= 1e-10` | rounding `1e-14` (the harness is not the cause of a miss) |
| E6 | `energy_error` for the three formulations returns finite estimate and solution norm; the estimates of BBar and of Incompatible differ from Full's by more than `1e-6` of the solution norm (an observation of the estimate path, which has its own strain call; no limit of ADR 0525) | finite; `> 1e-6` | observation |
| B1 | the branch changes no file under `crates/*/src`, `crates/rcfd-api/tests/baseline_hashes.rs` or a golden file | none | test-only item |
| B2 | `cargo test -p rcfd-fem --release` passes (ignored records excepted); `cargo test -p rcfd-api --release --test baseline_hashes` passes; clippy `-D warnings` on rcfd-fem; `cargo fmt -p rcfd-fem --check`; `cargo test -p rcfd-cli --release --test docs_site` | pass | test-only |

### Negative controls and planned mutations (scratch copy, own `CARGO_TARGET_DIR`, one at a time)

| id | mutation | expected to be caught by |
| --- | --- | --- |
| MU12 | `element_samples` uses `element.strain_at` (the compatible strain) for an opted Hex8 (both formulations) | E1, E3 (BBar); nothing for Incompatible alone (finding 3) |
| MU12i | the same only for an `Incompatible` block (BBar keeps its strain) | NOT caught by any check, by design: an equivalent mutant (the strain at `xi = 0` is the compatible one); recorded, not counted against the rule |
| MU13 | `hex8x::strain_at` of BBar without the mean-dilatation correction (ADR 0525 MU13) | E1, E3 |
| MU16 | `element_samples` takes the BBar strain with the plain (unweighted) mean of the gradients in place of the det-J-weighted mean | E1 only if the two means differ by more than `1e-10 smax` here; otherwise recorded as equivalent on this mesh |

The negative controls are E2, E2b and E4b (the formulations are distinguishable on this field) and E5 (the harness reproduces
the compatible strain).

## Decision rule

- **done:** E0 to E6 met; B1 and B2 met; MU12 (both formulations) and MU13 caught by a registered check (MU12i recorded as an
  equivalent mutant, as registered); no amendment after a run.
- **partial:** a check missed (recorded as a `*_known_miss`) or not run, MU12 or MU13 not caught, or a check met only after an
  amendment written after its run.
- **todo:** the test file does not build.

## Limits (not covered)

- L1. Only the samples of Hex8 at the centroid are observed, through the nodal value of a linear patch fit; a quadratic basis or
  the stress at other points of the elements is not exercised. `Incompatible` cannot be distinguished at this rule (finding 3);
  should a later item sample Hex8 at more points (for example `gauss_2`), a test of the same kind would then discriminate the
  enhanced strain there.
- L2. One mesh and one field; the limits are fixed from one prototype, with margins of about 30 for the BBar difference.
- L3. Constant elasticity and no temperature (the recovery's own refusals, ADR 0478).

## Open questions

None before the run.

## Addendum 1: results

Run of `crates/rcfd-fem/tests/si_hex8x_estimate.rs` (one test, `e0_to_e6_estimate_samples_of_an_opted_hex8_on_a_quadratic_field`),
2026-11-14. Limits are those registered above; no amendment was written. Every check met. `smax = 3.1559e9` Pa.

| check | limit | measured | met |
| --- | --- | --- | --- |
| E0 `|b| / E` | `> 1e-3` | 1.4499e-2 | yes |
| E2 BBar expectation against compatible expectation, largest over the 12 nodes | `>= 1e-5` | 3.6720e-4 | yes |
| E2b the same, smallest | `>= 1e-6` | 1.8330e-5 | yes |
| E1 BBar library nodal stress against the B-bar expectation | `<= 1e-10` | 4.5329e-16 | yes |
| E3 BBar library against the compatible expectation | `>= 1e-5` | 3.6720e-4 | yes |
| E4 Incompatible library against the compatible expectation | `<= 1e-10` | 4.5329e-16 | yes |
| E4b Incompatible library against the B-bar expectation | `>= 1e-5` | 3.6720e-4 | yes |
| E5 Full library against the compatible expectation | `<= 1e-10` | 4.5329e-16 | yes |
| E6 Full solution norm | `> 0`, finite | 4.3362e3 | yes |
| E6 BBar `|estimate - estimate_Full| / norm_Full` | `> 1e-6` | 9.0503e-4 | yes |
| E6 Incompatible, the same | `> 1e-6` | 4.6519e-5 | yes |
| B1 no file under `crates/*/src`, no hash table or golden file changed | none | none (the branch adds the ADR, one test file and the roadmap entries) | yes |
| B2 `cargo test -p rcfd-fem --release` (138 test binaries ok, none failed), `rcfd-api` `baseline_hashes` (3 passed, 1 ignored record), `docs_site` (2 passed), clippy `-D warnings`, `cargo fmt -p rcfd-fem --check` | pass | pass | yes |

The prototype numbers of the registration (`3.67e-4` largest, `1.83e-5` smallest) are reproduced by the Rust independent
expectation to the printed digits (E2, E2b), so the numpy prototype and the test agree on the field, the mesh and the B-bar
arithmetic.

### Mutations (scratch copy `f614-scratch/mut`, own target `target-f614-mut`, logs `mut-*.log`)

| id | result |
| --- | --- |
| MU12 (`element_samples` compatible strain for an opted Hex8) | CAUGHT: E1 missed at 3.6720e-4 (limit 1e-10) and E3 missed at 4.5329e-16 (limit `>= 1e-5`) |
| MU12i (compatible strain for an `Incompatible` block only) | not caught, as registered (equivalent mutant: the test passes unchanged; finding 3) |
| MU13 (`hex8x::strain_at` of BBar without the mean-dilatation correction) | CAUGHT: E1 (3.6720e-4), E3 (4.5e-16), E6 BBar (0 against `> 1e-6`) |
| MU16 (unweighted mean of the gradients in `hex8x::mean_gradients`; this also changes the BBar stiffness, which the test does not use) | CAUGHT: E1 missed at 7.9560e-4 |

### Deviations

- The first compile of the test failed on an ambiguous float type, and its first run stopped in the test's own set-up (the
  mesh copy did not carry the element set `all` that `SolidModel::add_material` needs; an `unwrap` panicked) after printing
  E0, E2 and E2b, which do not use the library. The set-up was corrected (the element sets are copied, as `rebuild` of
  `si_hex8x_linear.rs` does) and the test run again; no limit, field or mesh was changed, and no check that compares the
  library had run before the correction. Recorded for completeness; it is not a missed check.
- The `Incompatible` rows are positive checks (finding 3), not discriminating ones, as registered.

### Decision (by the rule above): done

E0 to E6 and B1, B2 met; MU12 and MU13 caught; MU12i is the registered equivalent mutant; no amendment after a run. The
finding that explains the ES3b miss of ADR 0525 is recorded: with the centroid rule of Hex8 the enhanced strain is zero at the
sample, so no field can separate `Incompatible` from the compatible strain at `element_samples`.

### Not run

The whole workspace suite (the lead runs the gate). No baseline or golden file was touched.

### Open questions (recommendations, not taken)

1. Should the patch recovery sample Hex8 at more than the centroid (for example `gauss_2`) so the estimate sees the enhanced
   strain? That would change the default recovered stresses of every Hex8 model, so it is listed and not taken.
   Recommendation: no, unless a model shows the centroid fit is too coarse.

## For the lead: roadmap entries

> F-614 DONE. ADR 0530, 2026-11-14: a non-equilibrium quadratic displacement field (dilatation 8e-3 x - 4e-3 y + 6e-3 z, Navier residual |b|/E 1.4e-2) on the distorted 4 x 3 x 3 patch box gives the recovered nodal stress of an opted Hex8 against an independent computation (own gradients, own B-bar mean, own normal equations): BBar 4.5e-16 and 3.7e-4 away from the compatible-strain expectation (limits 1e-10 and 1e-5), Incompatible equal to the compatible one to 4.5e-16 (its enhanced strain is zero at the centroid sample, which is why no field can separate it there), Full control 4.5e-16. Mutation MU12 of ADR 0525 (sampling with the compatible strain) is caught by E1 and E3; MU13 and an unweighted-mean mutation too; the Incompatible-only form of MU12 is an equivalent mutant. Test only, no library change; defaults and baseline hashes untouched.

> Proposed line for F-588's note, replacing its "Open:" sentence: Follow-up F-614 (ADR 0530, 2026-11-14) adds the discriminating test of estimate::element_samples for an opted Hex8: MU12 is now caught (BBar), and the ES3b miss is explained (the Incompatible enhanced strain is zero at the centroid sample). F-588 stays partial for the record of its own run (ES3b missed, RF3 amended once); the open item is closed.
