# Drive rcfd from an assistant: the MCP design loop

`rcfd-mcp` is a Model Context Protocol server on stdio. An MCP client (an AI assistant such as Claude, or any program that speaks
JSON-RPC 2.0) lists its tools, sets up and runs cases, and iterates on a design with the `design_*` tools. This tutorial shows the
exact JSON-RPC messages an assistant sends, on two examples: the contraction duct of the design loop (a pressure-drop minimisation)
and the parametric axial fan passage. The tool reference is [MCP tools](../mcp-tools.md); the design machinery (studies, optimisers,
operating maps, the store) is explained in [the design loop](../design-loop.md); the plan and state of the fan work are in
[the fan milestone](../milestone-fan.md).

The same tools are used in-process by the assistant sidebar of the desktop app ([desktop tutorial](desktop-assistant.md)).

## Connect a client

Build the server with `cargo build --release -p rcfd-mcp` and register it with your MCP client. Most clients take a JSON entry of
this shape (the file name and location depend on the client):

```json not-executed
{
  "mcpServers": {
    "rcfd": {
      "command": "/path/to/rcfd/target/release/rcfd-mcp",
      "args": ["--root", "/path/to/your/work/dir"]
    }
  }
}
```

`--root` confines every file the tools read or write to that directory (paths with `..` or outside it are refused with error 1004).
Other options: `--read-only` (refuse every tool that changes state), `--max-cells`, `--max-steps`, `--max-run-seconds`,
`--max-design-evaluations` (`rcfd-mcp --help`). Tools that change state are marked in `tools/list` (`annotations.readOnlyHint` false,
`_meta."rcfd/needsApproval"` true) so that the client can ask you before each such call.

Each `jsonl` block below is one session: the test of this tutorial starts `rcfd-mcp --root .` in a fresh working directory (holding a
copy of the repository's `cases/`), sends the lines and requires every response to be free of errors. What one session leaves on
disk (case files, study directories) is used by the next; the in-memory simulation sessions are not.

## Handshake and the tool list

```jsonl run
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"tutorial","version":"1"}}}
{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"list_cases","arguments":{}}}
```

```text output
"serverInfo":{"name":"rcfd"
"name":"design_create_study"
"name":"design_propose"
"rcfd/needsApproval":true
```

## The contraction duct

A 2D channel of length 5 and height 1 whose upper wall ramps down at `ramp_angle` degrees from `ramp_start` (mesh generator
`wedge_2d`, 40 x 12 cells), laminar, inlet velocity 1. The reports give the inlet and outlet pressures, the mean outlet velocity and
the pressure drop `dp`; reports are the metrics the design tools optimise.

```toml file=duct.toml
title = "contraction duct"

[mesh]
generator = "wedge_2d"
nx = 40
ny = 12
lx = 5.0
ly = 1.0
ramp_start = 0.5
ramp_angle = 6.0

[physics]
nu = 0.05
rho = 1.0

[boundaries.xmin]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]

[boundaries.xmax]
velocity = "zero_gradient"
pressure = "fixed"
pressure_value = 0.0

[boundaries.ymin]
velocity = "fixed"

[boundaries.ymax]
velocity = "fixed"

[solver]
algorithm = "simplec"
convection = "upwind"
max_iterations = 4000
tolerance_u = 1e-6
tolerance_p = 1e-6

[reports.p_in]
type = "area_average"
patch = "xmin"
field = "p"

[reports.p_out]
type = "area_average"
patch = "xmax"
field = "p"

[reports.ux_out]
type = "area_average"
patch = "xmax"
field = "ux"

[reports.dp]
type = "expression"
expression = "p_in - p_out"
```

### Set up, check and run once

```jsonl run
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"create_case","arguments":{"path":"duct.toml"}}}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"check_mesh","arguments":{}}}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"run","arguments":{"wait_seconds":120}}}
{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"status","arguments":{}}}
{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"get_field_stats","arguments":{"name":"speed"}}}
```

```text output
"session":"s1"
"name":"frontAndBack"
"state":"finished"
"converged":true
"name":"dp"
```

### A design study

Two parameters, the ramp angle in [0, 12] degrees and the ramp start in [0.5, 3]; minimise `dp` subject to "mean outlet velocity at
least 1.3" (the constraint keeps the optimiser from flattening the ramp), Nelder-Mead from (6, 0.5). The study lives in the
directory `contraction` (`study.json` and the evaluation store `evaluations.jsonl`).

```jsonl run
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"design_create_study","arguments":{"dir":"contraction","base":"duct.toml","name":"contraction","parameters":[{"name":"angle","lo":0,"hi":12,"init":6,"path":"mesh.ramp_angle"},{"name":"start","lo":0.5,"hi":3.0,"init":0.5,"path":"mesh.ramp_start"}],"objectives":[{"metric":"dp","sense":"min"}],"constraints":[{"metric":"ux_out","cmp":">=","limit":1.3,"scale":1.3}],"method":{"type":"nelder_mead","budget":80,"restarts":1,"seed":1}}}}
```

The assistant now loops: ask the method for candidates, evaluate them, repeat. `design_propose` has no side effects;
`design_evaluate` runs the case for one candidate and stores the result (a repeated call returns the stored record without running).

```jsonl run
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"design_propose","arguments":{"dir":"contraction"}}}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"design_evaluate","arguments":{"dir":"contraction","params":{"angle":6,"start":0.5}}}}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"design_evaluate","arguments":{"dir":"contraction","params":{"angle":6,"start":0.5}}}}
```

```text output
"candidates":[{"angle":6.0,"start":0.5}
"status":"converged"
"cached":true
```

Instead of evaluating candidate by candidate the assistant can let rcfd run the method for a bounded number of evaluations; the
next call continues from the store. Then it reads the best feasible record and compares candidates:

```jsonl run
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"design_run_study","arguments":{"dir":"contraction","max_evaluations":4}}}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"design_list_study","arguments":{"dir":"contraction"}}}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"design_compare_variants","arguments":{"dir":"contraction","indices":[0,1,2]}}}
```

```text output
"evaluations_run":4
"feasible":true
```

Running the method to its budget (80 evaluations plus one restart; in ADR 0067 E1 this reduced `dp` from 9.94 to 5.07 at an angle
of 4.46 degrees and a start of 1.77 with 77 evaluations) takes longer and runs only in the slow test:

```jsonl run-slow
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"design_run_study","arguments":{"dir":"contraction","max_evaluations":200}}}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"design_list_study","arguments":{"dir":"contraction"}}}
```

```text output
"state":"complete"
```

### Write the chosen variant and map its operating range

`design_create_variant` writes a case file for a parameter point (validated through the settings tree); an operating map then runs it
over inlet velocities, each point restarting from the previous one:

```jsonl run
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"design_create_variant","arguments":{"path":"variants/angle4.toml","dir":"contraction","params":{"angle":4,"start":1.5}}}}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"design_run_operating_map","arguments":{"out_dir":"map_angle4","base":"variants/angle4.toml","axes":[{"path":"boundaries.xmin.velocity_value","values":[[0.8,0,0],[1.0,0,0],[1.2,0,0]]}]}}}
```

```text output
"converged":3
```

`map_angle4/map.csv` holds one row per inlet velocity with `dp`, `ux_out` and the iteration counts.

## The axial fan passage

The parametric fan generator builds a periodic single-passage mesh from blade count, hub and tip radius, chord, stagger, camber,
thickness, sweep, lean and tip gap (`[mesh.fan]`, see [meshing](../meshing.md)). Through MCP the assistant creates the case from the
example file and checks the mesh:

```jsonl run
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"create_case","arguments":{"path":"cases/axial_fan_sector/axial_fan_sector.toml"}}}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"get_setting","arguments":{"path":"mesh.fan.n_blades"}}}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"check_mesh","arguments":{}}}
```

```text output
"name":"seam_lo"
"name":"blade"
```

A fan design study has the same shape as the duct study, with parameters mapped to `mesh.fan.*` paths (for example `mesh.fan.tip_stagger`)
and conditions for several operating points. **It is not part of this tutorial on purpose**: the steady rotating-frame solution on
these meshes has not been shown to converge to mesh-independent torque, and the comparison with the measured FAN-01 fan missed
(details and numbers in [the fan milestone](../milestone-fan.md)). A design iteration on the fan today would optimise numerical error.
The fan example files are smoke cases of the geometry and the plumbing, as the [gallery](gallery.md) says.

## What an assistant adds

The messages above are what an assistant sends; the assistant's part is choosing them: which parameters, bounds, objectives and
constraints, when to stop, and how to read the metrics. The rcfd side is deterministic: the same study definition and seed give the
same history, and every evaluation is stored and resumable.
