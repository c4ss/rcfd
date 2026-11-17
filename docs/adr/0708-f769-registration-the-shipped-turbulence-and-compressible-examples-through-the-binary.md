# ADR 0708: F-769 registration: the shipped turbulence and compressible examples through the binary and the checks of their test file

Status: **REGISTERED 2026-11-17, written before any run of the checks of [ADR 0705](0705-f769-registration-adrs-for-the-five-feature-groups-that-no-adr-describes.md)
(F-769, group 3 of 5).** Not yet accepted by the maintainer. Results are in the addendum of ADR 0705.

**These tests predate this ADR.** `crates/rcfd-cli/tests/physics_cases.rs` was committed on 2026-10-13 in the first import of rcfd
(`f71f6e2c`); no ADR registered its limits. ADR 0005 only lists the case files in a hash list. This ADR records what the file checks and
which limits it carries, read from the code, from the case files it runs and from the roadmap notes of F-042, F-043 and F-060. It is not a
decision taken before the tests. No Rust file is edited and no test is run for it (ADR 0705, Limits).

## The feature

The `rcfd` binary (`CARGO_BIN_EXE_rcfd`) runs and validates the shipped example cases of the turbulence and compressible physics under
`cases/`: `channel/kepsilon_channel.toml` (F-042), `channel/kepsilon_inlet.toml`, `channel/sst_channel.toml` (F-043), `sod/sod.toml` and
`wedge/wedge.toml` (F-060). The file runs the binary as a process (`rcfd run <case> --out <dir> --quiet`, `rcfd validate <case>`), so it
checks the exit code, the files written and the diagnostics on stderr, not library values. The exit codes are those of
`docs/run-control.md`: 0 converged or finished, 3 invalid case or settings, 5 steady run stopped at its iteration cap.

## Limits carried by the file

| id | test | kind | limit | source |
|---|---|---|---|---|
| C1 | `every_physics_example_validates` | exact | `rcfd validate` of the five cases `channel/kepsilon_channel.toml`, `channel/kepsilon_inlet.toml`, `channel/sst_channel.toml`, `sod/sod.toml`, `wedge/wedge.toml` exits with code **0** | the assert |
| C2 | `sod_runs_and_writes_compressible_fields` | exact | `rcfd run sod/sod.toml` exits **0**; `solution.vtu` contains the arrays `rho`, `mach`, `T`; `monitors.csv` is written | the asserts |
| C3 | `wedge_runs_to_the_end_time` | exact | `rcfd run wedge/wedge.toml` exits **0** (a transient run that reached its end time exits 0) and `solution.vtu` is written. The test does not read the end time back | the asserts |
| C4 | `kepsilon_channel_converges_and_writes_turbulence_fields` | exact | `rcfd run channel/kepsilon_channel.toml` exits **0** (converged); `solution.vtu` contains `k`, `epsilon`, `nut`, `y_plus`; the header of `monitors.csv` contains `yplus_lower` | the asserts |
| C5 | `sst_channel_converges_to_unit_wall_y_plus_and_writes_omega` | numeric | `rcfd run channel/sst_channel.toml` exits **0**; `solution.vtu` contains `k`, `omega`, `nut`, `y_plus`; in the last row of `monitors.csv` the column `yplus_lower` is within **0.01** of 1 (absolute), and the column `u_mean` is within **10 %** of the DNS bulk velocity 17.4 (Moser, Kim and Mansour 1999, quoted in the case file). The comment of the test records the measured bulk velocity 18.17, which is +4.4 % from 17.4 and "see the F-043 roadmap note": the 10 % limit is a window around that recorded value, not a validation to a tighter band | the asserts; the comment in the file; the F-043 note |
| C6 | `kepsilon_inlet_example_runs` | exact | `rcfd run channel/kepsilon_inlet.toml` exits with **0 or 5** (converged, or stopped at the iteration cap; any other code is a failure) and `solution.vtu` is written | the asserts |
| C7 | `unsupported_combinations_are_reported_with_line_numbers` | exact and structural | three edited copies of the shipped cases each make `rcfd validate` exit **3**: (a) `sod.toml` with `rk_order = 3` replaced by `rk_order = 1` (a limited reconstruction needs a higher order): stderr contains `compressible.rk_order` and `line N`, N being the line of `rk_order` in the file; (b) `kepsilon_inlet.toml` with `algorithm = "simplec"` replaced by `"piso"` (k-epsilon with a transient algorithm): stderr contains `turbulence.model`; (c) `kepsilon_inlet.toml` with `p_ref_cell = 99999` inserted under `[solver]` (an error found only when the simulation is built): stderr contains `solver.p_ref_cell` and `line ` | the asserts |

## What the file does not check

* Values of the solution: only C5 reads two numbers, both from the monitors file. The sod tube is not compared with the exact Riemann
  solution here, the wedge shock angle is not read, the k-epsilon channel is not compared with DNS (those belong to F-042, F-043 and F-060
  and their own tests).
* Residual histories, iteration counts, the contents of the other fields, the VTU structure beyond the array names.
* The other shipped cases of `cases/` (cavity, backward step, flat plate, naca0012, packed bed, fans), and every error other than the three
  of C7. The message text beyond the quoted keys and the line number of (a) is not checked.
* Run time: the limits above are exit codes and files; there is no time limit in the file.
