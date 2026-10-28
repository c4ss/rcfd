# ADR 0137: The first Linux full gate: per-platform expected values, the L1 control rule per run, reported defects

Status: **registered** 2026-10-28 before any changed test ran; the probe runs that measured the values below were made first and are
disclosed in "What was run before this ADR was written". House rules as in [ADR 0130](0130-wave48a-overview-and-run-record-contract.md):
no limit is changed; a missed limit becomes an ignored `*_known_miss` test with the limit intact; defaults of existing code stay
bit-identical; `crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded (its registered Windows values stay as they are).

## Context

The first `Full gate` workflow run on `ubuntu-latest` (CI run 40670041161, main a89ccb3) failed 16 tests (2273 passed). Every
expected value in the suite was recorded on the maintainer's Windows machine (x86_64-pc-windows-msvc). Mesh generators and several
initial fields call the platform's libm (`sin`, `cos`, `exp`, `powf`, `cbrt`), whose last bits differ between the Microsoft CRT and
glibc, so a bit-identity hash of anything built from them differs per platform. Precedent: `g0_planar_join_bit_identical_to_head_80c26e9`
(`crates/rcfd-mesh/src/non_conformal.rs`), one expected pair per platform, each proven on its platform against the code before the
change, other platforms print and skip.

All 16 failures were reproduced in Docker `rust:1.99-bookworm` (glibc 2.36, rustc 1.99.1, release, `--cpuset-cpus 0-3`) on main
a521f76 with the same values the CI log prints.

## Decision

### Rule for a Linux expected value (registered)

A test that asserts bit identity "to the code before change X" gets an x86_64 Linux expected value next to the Windows one only if,
in one Linux container (the image above):

1. the test's computation, dropped verbatim into a `git archive` of the reference commit the test names, gives that value; and
2. the same computation at HEAD gives the same value; and
3. where the CI log prints the value, it equals the CI value (this closes part of the gap between glibc 2.36 and Ubuntu 24.04's 2.39).

Otherwise the test stays as it is and is reported for a maintainer decision. The Windows values are not touched. Other platforms
(macOS, Linux on other architectures) print the computed values and skip, as g0 does. A test's limit (0 bits) is unchanged.

The ids in the test comments predate the history rewrites of 2026-10-27/28 and no longer resolve. The reference commits used are
the ids the remapped ADR texts cite for the same recording, checked by subject and date:

| Test | Id in the test | Reference commit (current history) | Source of the mapping |
|---|---|---|---|
| `g6_fan_meshes_unchanged`, `w45a_p5_defaults_are_bit_identical_to_main` | 511bed4 | e65b4ea | ADR 0115 ("P5 hashes recorded at e65b4ea"), ADR 0116 (G6 "at e65b4ea before any change") |
| `cavity_paths_match_registered_hashes` (coupled) | 8b68e46 | e79f7db | "F-114, the last commit before ADR 0005": e79f7db is the parent of 5938b0c, which adds ADR 0005 |
| `t4_annulus_interface_case_without_the_keys_is_bit_identical` | 2f1a3ce | fee6ecc | ADR 0112 T4 ("recorded at HEAD fee6ecc") |
| `a1_euler_and_non_periodic_bdf2_unchanged_bitwise` | (pre-fix code) | 7b2d092 | ADR 0043: fix 417ea40; 7b2d092 is its parent |
| `defaults_are_bit_identical_to_the_code_before_wave_33b` | (amendment 1) | c4a593d | "ADR 0069 amendment 1 (diagnosis results)", before the first solver change c3d130e |
| `b1_existing_constructions_are_bit_identical` | 0cef8f5 | 211d3e3 | ADR 0079 ("All on HEAD 211d3e3", hashes "recorded on 211d3e3") |
| `b1_construction_g_is_bit_identical` | c89495f | 6c3ddc9 | ADR 0083 ("All on HEAD 6c3ddc9") |
| `b1_construction_h_is_bit_identical` | 6f97dcb | feb015f | ADR 0087 ("All on HEAD feb015f") |
| `w0_default_output_bit_identical` | a2e0bfd | e325ab4 | ADR 0104 (smoke table "at HEAD e325ab4") |
| `x0_default_settings_reproduce_adr0108_x_40` | (ADR 0108 X run) | 132c0da | the ADR 0108 code commit ("VOF: opt-in height-function CSF fallback (ADR 0108)") |

### L1 negative control (ADR 0128 finding 12, applied per run)

ADR 0128 finding 12 registered a decision rule: if `10 w > 500 ms` (w = slowest of the 3 warm listings) the control "the listing done
10 times exceeds the budget" is asserted; if `10 w <= 500 ms` the budget cannot catch a 10x slowdown and the remaining guard is the
budget assertion itself. The rule was evaluated once, on Windows (w = 196 ms). On Linux the listing takes about 4-6 ms, so the
asserted control fails although the listing is 85-140 times inside the budget. Amendment: the test applies the registered rule on
every run. With `10 w > 500 ms` it asserts the control as before (Windows unchanged); otherwise it prints `10 w` and the factor
`500 ms / w` the budget does catch and does not assert the control. The 500 ms budget and its assertion are unchanged.

### Reported, not changed (production code or a decision is needed)

* **S4a / S4b (`crates/rcfd-runner/tests/runner_w47b.rs`), production defect on Unix.** `sys::lock_held` on Unix reads the runner pid
  from `runner.lock` and asks `ps -p <pid> -o comm=`. The runner is started by the client (`Client::ensure_runner`, `spawn_direct`,
  the `Child` dropped), so the client process is the runner's parent. A killed runner stays a zombie until its parent reaps it, and
  `ps` still prints its name for a zombie, so `runner_running()` reads true for as long as the client that started the runner
  lives (the test process here; any long-lived client process in use, not checked further) and `ensure_runner` never starts a new
  runner. Windows has no zombie state, which is why the tests pass there. Not changed here: the fix is in production code
  (`sys.rs`: treat state `Z` as dead, or keep and reap the child). The tests are left failing on Linux as the evidence.
* **C3 (`crates/rcfd-fv/tests/cavitation.rs`), production defect, 1 ulp.** `Cavitation::advance` documents a result in
  `[min(alpha_v, alpha_floor), 1]`. For condensation of a bubble already below the nucleus size the radius does not move, but the
  round trip `alpha_of_radius(radius(alpha_v))` (`cbrt`, then `powi(3)`) returns `alpha_v` minus 1 ulp with glibc
  (`1e-4` gives `0.00009999999999999998`), and `a.min(alpha_v)` keeps it. The documented bound is violated by 1 ulp; a clamp to
  `alpha_v` in that branch would fix it. Production code, not changed; the test (exact bound, no tolerance) is left as it is.
* **P1 (`crates/rcfd-mesh/tests/tetmesh.rs`), unregistered check, decision needed.** On Linux the curvature remesh of the libm-built
  cylinder gives a different surface and the default tet mesh (1100 cells, valid, Euler characteristic 2) has one face of 86.06 deg
  non-orthogonality, above `check_mesh`'s 85 deg error threshold; Windows gives 1061 cells under it. Same result at 7b2d092
  (2026-10-16) and e325ab4: not a regression. P1 is not a registered check (added after ADR 0030 C3), so there is no per-platform
  value to prove: the maintainer decides between the quality pass of ADR 0104 for P1, a known miss on Linux, or another set-up.
* **X0 (`crates/rcfd-fv/tests/vof_bubble_refinement_w40d.rs`)** is a 1e-9 tolerance against values from a Windows log, not a hash.
  It gets a Linux pair under the same rule (the ADR 0108 X code at 132c0da and the HEAD defaults give bit-identical results on
  Linux); listed here because the maintainer may prefer to treat a logged-value comparison differently from a hash.

## What was run before this ADR was written (disclosed)

Docker `rust:1.99-bookworm`, `--cpuset-cpus 0-3 --memory 16g --init`, release, one container at a time, on `git archive` copies:
the 16 failing tests at a521f76 (all reproduce, same values as CI), then each test's computation (copied from a521f76, printing every
value instead of stopping at the first assertion) at a521f76 and at its reference commit:

| Computation | HEAD a521f76 (Linux) | Reference commit (Linux) | CI value (left:) | Windows (registered) |
|---|---|---|---|---|
| G6 default, B3, E3 | 95aa2af7f4ed8b6c, b2563dff49086f53, ec27db08c0154f65 | e65b4ea: same | same three | 8471bf9a1aef1dd4, be64f588a252d0b4, 04fbabedbad41768 |
| P5 absent = `none`, `barth_jespersen` (30 iterations) | 971b2e8ae81ee451 (guard `velocity_runaway`), bb748c1fc0d93169 (no guard) | e65b4ea: same, same guards | printed: same | ec721116c0963182 (guard tripped as on Linux), 26e5a0c524957698 |
| Baseline cavity simple / simplec / coupled / piso / pimple | 07e7461c36893384, 58d5c4fd29432b05, **fa937c4ec7bcb078**, 2a042a42594c418b, 2a042a42594c418b | e79f7db: same five | coupled fa937c4ec7bcb078 | coupled 123b5330dae3f36a; the other four equal on both |
| T4 | de77a7409c95de77 | fee6ecc: same | same | 6eca7279950f43c3 |
| A1 Euler Taylor-Green, BDF2 cavity | b91593f04078212e, e8bd7eaeefd7048b | 7b2d092: same | b91593f04078212e | 8c2d50e6a8e9b9ee, e8bd7eaeefd7048b |
| W33B tg piso bdf2, tg pimple2 euler, tg frac bdf2 (cavity two equal on both) | 793cc167a5c8dce3, 7a9cd18ebf2fcbc2, 275aa31d35e717ff | c4a593d: same five | same | e9dca234e513bec8, 72d935a8d4993955, e46fbe511183e9fc |
| B1 S pursuit, S E, A' pursuit, A' E (refusal message identical) | bc973db0a03e9cf3, 482c0bf220810dff, 6f5b873240edf079, 0eb5abb172afc247 | 211d3e3, 6c3ddc9, feb015f: same | S pursuit bc973db0a03e9cf3 | eebcf5467f718bf0, de01897c36a13987, 3247673c05cd65bf, 35359c0db0e47e3a |
| B1 G S, FAN-01-C | aecdcc838d239069, cbed3848e92d676a | 6c3ddc9, feb015f: same | S aecdcc838d239069 | 59b26da9febc9b2e, 00b3e9673d470e52 |
| B1 H S, FAN-01-C | ce09460b1aa09597, a8bb6d7c50d384e1 | feb015f: same | S ce09460b1aa09597 | 6f6342d53409da61, 21104bbbdd9b9129 |
| W0 cube, sphere, torus, cyl_in_box, p1_in_code, s2_stl, sphere_h0.3, sphere_h0.25, cube4_h0.5 | (108, 2507394269494118308), (320, 15939201240145194707), (910, 9410422296272373650), (1134, 10856385456967318595), (1100, 8442970156282097805), (1022, 10466773633785522690), (823, 4586941130221034709), (1277, 9750081027263524416), (597, 11005660464813738162) | e325ab4: same nine | torus (910, 9410422296272373650) | torus (899, ...), cyl_in_box (1137, ...), p1_in_code (1061, ...), s2_stl (1115, ...) differ; the other five equal on both |
| X0 40 x 80: e_y, e_v (ADR 0108 X set-up at 132c0da; HEAD defaults) | 0.002053061866984063, 0.0015273879501069287 (y_c bits 0x3ff144986ff96e57, V bits 0x3fcebdf9e5131889) | 132c0da: bit-identical | same e_y | 0.002053058745789871, 0.0015273890756644926 (diff on Linux 3.1e-9 and 1.1e-9 > 1e-9) |
| P1 check report | 1100 cells, max non-orthogonality 86.06 deg | 7b2d092, e325ab4: same | (assert only) | passes |
| C3 | fails, 1e-4 to 0.00009999999999999998 | c4a593d: fails the same | same | passes |

Also checked: the coupled cavity hash is the same with `RAYON_NUM_THREADS` 1, 4 and 16 and with 4 or 16 CPUs (so it is a platform
difference, not thread-count nondeterminism). S4 diagnosis probe (S4a set-up, kill -9 of the runner, 2 s): `ps` shows the runner
in state `Z` with the test process as parent, `pid_alive` true, `runner_running` true.

Per-platform values are added for G6, P5, the coupled cavity hash, T4, A1, W33B, the three B1 tests, W0 and X0 (40 x 80 only; 80
and 160 were not run on Linux, so the ignored series test prints and skips them there). The Windows values are unchanged. The
X0 Linux pair passes the same 1e-9 limit against the Linux run of the ADR 0108 X code, as the Windows pair does against the Windows
log.

## Addendum: results with the change (2026-10-28)

Docker (same image and limits), `git archive` of the branch head a2dfecf (before the rebase onto main 81cf498, whose commits touch none of the changed files), the 16 tests of the CI failure list: 12 pass (G6, P5,
`cavity_paths_match_registered_hashes`, T4, L1, A1, W33B, the three B1, W0, X0 40 x 80); 4 fail as reported above (S4a, S4b,
C3, P1). L1 printed `10 w = 90.9 ms <= budget: the budget catches a listing 55x slower than measured`.

Windows (this machine, release, `git archive` of a2dfecf, own target dir): the 14 changed tests (the three B1, W0, A1, W33B, L1,
the three `baseline_hashes` tests, T4, G6, P5, X0 40 x 80) pass and print the registered Windows values (for example G6
8471bf9a1aef1dd4 / be64f588a252d0b4 / 04fbabedbad41768, T4 0x6eca7279950f43c3, X0 diff 0 to the 43A values). L1 asserted the
control as before: worst listing 80.6 ms, the listing 10 times 844 ms > 500 ms. No production code changed.

## Consequences

* The Linux full gate tests the same claims as the Windows gate for the tests above, against Linux values with the same provenance.
* The Windows values and the Windows behaviour of every changed test are unchanged.
* The next `Full gate` run on `ubuntu-latest` is the real check: the container is Debian bookworm (glibc 2.36), the CI image Ubuntu
  24.04 (glibc 2.39); condition 3 covers only the first value of each test that CI printed.
