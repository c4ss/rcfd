# ADR 0151: Job 48Q, a faster CPU contour render at 1M cells, with byte-identical images

Status: **registered** 2026-10-28, in two parts.
* Part 1 (this section and the next two): the control and profile set-up and the byte-identity checks. Registered before any
  run of this job.
* Part 2: the timing limits, registered after the control runs and before any after run. The maintainer's brief orders
  "profile, register the limit, then optimise".
* Results go in the addendum. Not yet accepted by the maintainer.

House rules of [ADR 0130](0130-wave48a-overview-and-run-record-contract.md):
* limits are fixed before the registered runs;
* a missed limit becomes an ignored `*_known_miss` test with the limit intact;
* a defective set-up gets an amendment before the corrected run;
* defaults and outputs stay bit-identical. Here that means the rendered pixels and the PNG bytes of every existing render
  test;
* `crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded.

No unsafe code, no new crate, and the crate graph stays acyclic.

## Problem

Full gate run 40745275594 (`windows-latest`, commit 7eb8b79, `scripts/gate_test.py` with `jobs=3`) failed two release timing
limits. Both pass on the maintainer's machine and passed in earlier gates.

| Test | CI | Limit | Earlier local figures |
|---|---|---|---|
| `rcfd-post` `render_pixels::r2_one_million_cells_render_in_under_two_seconds` | render 2.316 s; PNG encode 0.097 s (188138 bytes) | render < 2 s | render 0.903 s, encode 0.053 s (`docs/audit/partials-group-3.md`) |
| `rcfd-cli` `render_cli_w39a::r4_cli_renders_a_one_million_cell_contour_image_in_under_two_seconds` | `rcfd call render.png` 2.086 s. `run.initialize` 22.3 s, not limited | < 2 s | 1.253 s smoke, 1.555 s registered ([ADR 0092](0092-wave39a-partials-closeout-rules-fluent-render.md)) |

## The maintainer's decision (2026-10-28)

Investigate a real speedup of the 1M-cell CPU render path, both the `rcfd-post` render and the CLI call, so that it meets
2 s with margin on a slow, shared runner with 2-4 cores.
1. First profile on this machine (release), with a controlled measurement, where the time goes: rasterisation, colour
   mapping, allocation, threading, PNG encode and base64.
2. Register the limit, for example render <= 1.2 s on this machine as the margin proxy, with the existing 2 s limits
   unchanged.
3. Register the byte-identity check: pixel-exact on the existing r1 and r3 tests, and a hash of the 1M-cell image.
4. Then optimise. Candidates:
   * tiling or scanline rasterisation;
   * no per-cell allocation;
   * deterministic threads (`std::thread::scope`, no new crate);
   * a cached colour-map lookup table.

   A cheaper PNG compression level is excluded, because its bytes would change.
5. Report timing under limited cores too.
6. If no safe speedup meets the margin, say so with the measurements. No limit is loosened.

## Context (read at main 6867e40)

* **Render path.** `render_scalar` in `crates/rcfd-post/src/render.rs` runs on one thread, in these steps:
  1. `view`: the bounding box over all points (about 2.0M for the 1M-cell cavity), the view axis and the slice.
  2. `frame`, then `Image::new`: a white RGBA image, filled pixel by pixel.
  3. `cell_polygon` for every cell:
     * it pushes the projections of the points of all the cell's faces (24 for a hex) into a new `Vec`;
     * it takes the cell's extent along the view axis for the slice test;
     * `convex_hull` then sorts the 24 points (stable sort), runs `dedup_by` within 1e-14 px, and builds a monotone chain with
       two boxed iterators.

     The hulls are kept as 1M separate `Vec`s.
  4. The min and max of the finite values of the drawn cells, in cell order.
  5. For every drawn cell in cell order: `colormap`, then `fill_polygon`, which makes a new `Vec` of crossings per scanline.
     A later cell overwrites an earlier one on a shared pixel.
  6. The colour bar and its two labels.

  A 1M-cell render thus makes several heap allocations per cell and frees 1M hulls at the end.
* **CLI path (r4).** `rcfd call` is a new process. It connects to `rcfd serve`, whose `render.png` handler
  (`crates/rcfd-api/src/service.rs`) does the following:
  * computes the field, then calls `render_scalar`, `Image::to_png` and `base64`, and builds the JSON reply;
  * `serve_stream` writes the reply with `writeln!` straight to the `TcpStream`, unbuffered.

  The client reads the line, parses it and prints it.
* **PNG.** png 0.18 at its default settings. Its bytes depend on the compression level and the filters, so the encoding
  stays as it is.
* **Colour map.** `colormap` is a continuous function of `t`: 9 stops, piecewise linear, rounded. A lookup table indexed by
  a quantised `t` would change pixels, so that candidate is out unless it is exact.
* **Signed zeros in the folds.** `f64::min` and `f64::max` may return either zero on a tie, and `label(vmin)` prints
  `-0.000` for -0. Any fold whose result reaches the image therefore keeps its order:
  * the bounding box;
  * the value range;
  * a polygon's y extent.
* **Gate.** `scripts/gate_test.py` runs one process per test, 3 at once (`RUST_TEST_THREADS=3`,
  [ADR 0127](0127-test-gate-efficiency.md)). The runner's core count is not in the run's log (UNVERIFIED). GitHub documents
  2 vCPUs for a private repository's standard Windows runner (UNVERIFIED here).
* **Thread count on Windows.** In std 1.99, `available_parallelism` on Windows is
  `GetSystemInfo().dwNumberOfProcessors`; it ignores the process affinity mask (read from the toolchain's
  `library/std/src/sys/thread/windows.rs`). A process pinned to 2 logical CPUs here still sees 16.
* **This host.**
  * AMD Ryzen 7 9800X3D: 8 cores, 16 logical CPUs. The SMT siblings are logical CPUs (2k, 2k+1), from
    `GetLogicalProcessorInformation`.
  * 93.6 GB of RAM; rustc 1.99.0.
  * Other agents share the host: the CPU was 28-40 % busy when this was written, with about 22 GB of RAM free.

## Registered set-up, part 1: control and profile runs

**Builds.**
* Release builds, in the worktree's own target folder.
* The control binaries were built at 6867e40, before any change, and copied before any rebuild:
  * `rcfd.exe` (SHA-256 starting `20B2DFE4`);
  * the `render_pixels` test binary (`4A026728`);
  * the `render_cli_w39a` test binary (`46B0AC40`).
* The control `render_cli_w39a` reaches `rcfd.exe` through an absolute path fixed at compile time. Its runs therefore happen
  before any rebuild of `rcfd.exe`.
* The new CLI harness takes each `rcfd.exe` path as a parameter.

**Conditions.** Each run records its start time, the CPU load over the 2 s before it, and the free RAM.

| Id | Affinity mask | Other load | Models |
|---|---|---|---|
| U | none | none from this job | this machine, as for the earlier local figures |
| S2 | 0xC000 (logical 14 and 15: one core, two SMT threads) | none from this job | a 2-vCPU VM whose vCPUs are SMT siblings |
| P2 | 0x5000 (logical 12 and 14: two cores) | none from this job | a 2-vCPU VM on separate cores |
| S2L | 0xC000 | two busy loops (`python -c "while True: pass"`), each pinned to 0xC000 | the gate's 3 concurrent test processes on a 2-vCPU runner |

* The mask is set at process creation, with `cmd /c start "" /affinity <mask> /b /wait <exe> ...`. Children inherit it.
  Probe: a Python process started this way, and its child, both showed 0xC000.
* The busy loops are stopped in a `finally`.

**Control runs, on the base code (6867e40).**
* **C-r2.** The control `render_pixels` binary, `r2_one_million_cells_render_in_under_two_seconds --exact --test-threads 1
  --nocapture`. 5 rounds; each round runs U, S2, P2 and S2L, in that order. Recorded: the render and encode times.
* **C-r4.** The control `render_cli_w39a` binary, r4 with `--exact --test-threads 1 --nocapture`. 3 rounds over U, S2, P2
  and S2L. Each r4 run takes about 20 s and up to a few GB, mostly the session's initialisation. Recorded: the
  `render.png` call time.
* **C-profile (Q0).** `crates/rcfd-post/tests/render_speed_w48q.rs`, `q0_profile_control_phases`, ignored and run by hand.
  * It uses the base renderer's verbatim copy (below) with phase marks: timer calls between the phases, with no change to the
    arithmetic.
  * Workload: r2's mesh, field and options. 5 repetitions in one process, under U and under S2L.
  * Phases timed:
    * `view`;
    * `frame` plus the image;
    * the cell polygons (projection, hull, the 1M `Vec`s);
    * the value range;
    * colour plus fill;
    * the colour bar;
    * freeing the hulls;
    * the PNG encode.
  * Probes on the same data, outside the control's code path:
    * the polygon phase with reused buffers (the allocation share);
    * the polygon phase with the cell's point indices deduplicated before the sort (the sort share);
    * the fill with a reused crossing buffer;
    * `colormap` alone.

  Each probe's hulls are checked equal to the control's.
* **C-cli (Q5).** `crates/rcfd-cli/tests/render_cli_speed_w48q.rs`, `q5_cli_render_timing`, ignored and run by hand.
  * One `rcfd serve` per `rcfd.exe` listed in `RCFD_W48Q_EXES` (default: the build's own), each with r4's session, created
    and initialised.
  * Then 7 rounds, alternating between the binaries:
    * `rcfd call service.info`: the process and connection floor;
    * `rcfd call render.png`, with r4's parameters.

    The first `render.png` call after the initialisation is reported apart, as r4 times it.
  * The stdout of `render.png` must be byte-identical across the binaries.
* **C-cli in process (Q6).** In the same file, `q6_cli_handler_breakdown`, ignored. r4's session in an in-process
  `Service`; `call("render.png")` is timed, and so are `base64` and the serialisation of the reply's JSON line. 5
  repetitions, condition U.

Medians are reported, together with the minimum and the maximum.

## Registered byte-identity checks (exact limits)

The oracle is the base code. `crates/rcfd-post/tests/render_control_w48q/mod.rs` is a verbatim copy of the base
`render_scalar` and the private functions it calls. Only these changes are made:
* `Image::new` and `Image::put` become free functions;
* phase marks are added;
* the hulls are dropped explicitly before the return.

| Id | Check | Limit |
|---|---|---|
| Q1 | `render_speed_w48q::q1_matches_the_base_renderer_byte_for_byte`: the product `render_scalar` against the copy, over the matrix below. It runs in debug and release, and in the gate | Equal in every case: RGBA bytes, width, height, the bits of `vmin` and `vmax`, `cells_drawn` and the PNG bytes |
| Q2 | `q2_r1_and_r3_images_are_as_before`: r1's and r3's images, each from its own set-up. It runs in the gate | The SHA-256 of the RGBA bytes and of the PNG bytes equal the values recorded from the base code |
| Q3 | `q3_one_million_cell_image_is_as_before`: the 1M-cell image of r2. Ignored, release only, run by hand: the mesh takes 2.7 GB, too much to add to the gate next to r2 | The RGBA and PNG SHA-256, `vmin`, `vmax` and `cells_drawn` equal the base values, and the image equals the copy's, rendered in the same process |

**Matrix of Q1.** Cell values are pseudo-random unless stated, so that every cell has its own colour and an order error shows.

| # | Mesh | Options and values | What it exercises |
|---|---|---|---|
| 1 | `cavity_2d(8)` | default options (colour bar) | the colour bar and labels |
| 2 | `cavity_2d_rect(32, 32)`, width 64, no colour bar | | cell edges at 8 + 1.5 i px. Every other edge passes through pixel centres, which are drawn by both cells, so cell order decides |
| 3 | `cavity_2d_rect(3, 5, 1, 2)`, width 2048 | | coarse cells spanning hundreds of rows each |
| 4 | `warp` of `cavity_2d_rect(17, 13)` by a smooth non-affine map | width 777, height 500 | skewed quads; crossings on sloped edges; explicit height |
| 5 | `cylinder_ogrid(24, 8, 0.5, 2.0)` | width 513 | curved cells around a hole: the background shows |
| 6 | `backward_step` | width 300 | a non-rectangular domain |
| 7 | `box_mesh` 4 x 5 x 6, graded | axis 0, 1 and 2; a slice inside a layer, a slice on a layer boundary (two layers pass the slice test, so order decides), and a slice outside (nothing drawn, range (0, 1)) | 3D slices |
| 8 | the 2D cavity of #2 | values with NaN, +inf and -inf | non-finite values and the range fold |
| 9 | the same | `range` given: increasing; equal ends; reversed | the range options |
| 10 | the same | all values equal | `span = 1` |
| 11 | the same | width 10 (clamped to 64), height 5 (clamped to 32) | the clamps |
| 12 | `box_mesh` 3 x 3 x 3 with z length 1e-20, viewed along x | the extreme values on these cells | degenerate hulls (fewer than 3 points after the dedup): not filled, yet counted in `cells_drawn` and in the range |
| 13 | `annulus_3d`, default axis and slice | | a curved 3D mesh |
| 14 | `cavity_2d_rect(256, 256)`, r2's field, width 1024 (release; also in debug) | | a medium mesh at r2's width |

**Order of work.**
1. This registration.
2. The tests: the copy, Q0-Q6, and the recorded base hashes of Q2 and Q3. They use only APIs that exist at 6867e40, and
   they are run on the base code before `render.rs` is changed. Q1 then proves that the copy matches the product on the
   matrix.
3. The control runs.
4. Part 2.
5. The change.
6. The after runs.

The constants of Q2 and Q3 are taken from the base product, not from the copy.

## What I already knew when registering

* The code reading in Context, the gate log of run 40745275594, and the earlier local figures quoted in Problem.
* No measurement of this job has been made. The only probe run is the Python affinity probe in the set-up, which is not a
  check.
