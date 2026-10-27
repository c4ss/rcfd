# The desktop app and the assistant sidebar

`rcfd-gui` is the desktop app: a settings editor driven by the case schema, a 2D field viewport, residual and report monitors, run
controls and an assistant sidebar that talks to a model of your choice (a local server such as Ollama or LM Studio, or a hosted API)
through the same tools as the [MCP server](mcp-design-loop.md).

This tutorial opens a cavity case in the app, runs it, uses the assistant's quick actions, connects a model and walks through
an approval. It is for people who prefer a window to the command line, or who want to try the assistant on a case. If you want
to script runs instead, start with the [quick start](quick-start-cli.md).

**What is and is not tested on this page.** Only the command-line steps (writing and checking the case file) are executed by the
tutorial test. Every GUI step below is marked `not-executed`: the test suite cannot click through a window. The app's logic is
covered by headless tests (frames rendered without a window, in `crates/rcfd-gui/tests/`, including a mock-model turn with an
approval in `smoke.rs`), but nobody has checked these instructions against the running app's look, and no real or local model has
been run through the sidebar in the test suite (roadmap F-145, F-150).

What you need: a checkout of this repository and a Rust toolchain with cargo (the [quick start](quick-start-cli.md) covers the
build of the command-line tool used in the first step). Connecting a model needs either a local model
server or a key for a hosted API; the steps before that, including the quick actions, need no model.

The steps: write and check a case, start the app, run the case and look at the result, try the quick actions, connect a model,
then ask it for a change and approve or deny its tool calls.

## 1. Write and check a case

Save this as `cavity.toml` in the directory you start the app from.

```toml file=cavity.toml
title = "cavity for the desktop app"

[mesh]
generator = "cavity_2d"
nx = 32
ny = 32

[physics]
nu = 0.01

[boundaries.lid]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]

[boundaries.walls]
velocity = "fixed"

[solver]
algorithm = "simplec"
convection = "upwind"
max_iterations = 1000
tolerance_u = 1e-6
tolerance_p = 1e-6
```

```console run
$ rcfd check cavity.toml
```

```text output
cavity.toml: ok, 1024 cells
```

## 2. Start the app

```console not-executed
cargo run --release -p rcfd-gui -- --root . cavity.toml
```

`rcfd-gui [--root DIR] [CASE.toml]`: `--root` confines the files the app and the assistant may touch, the optional case is opened at
start (without one the app starts from a template). The window has dockable panels (settings, viewport, monitors, log, assistant);
a panel can be hidden, floated or re-docked, and the layout is saved in the per-user config directory (restoring it after a restart
has not been tested end to end).

## 3. Run and look at the result (not executed)

```text not-executed
Settings panel   edit a value (for example physics.nu); out-of-range or wrong-type input is refused on the field
Run controls     Run / Pause / Resume / Stop / Step
Monitors panel   residuals and reports on a log scale, updated while the run goes on
Viewport         Raster or Cells view of a field (speed, ux, uy, p) with colour map, legend and hover value
```

## 4. The assistant sidebar without a model (not executed)

Four quick actions run the set-up tools directly, with no model and no network: **Assess mesh**, **Validate case**, **Suggest BCs**
and **Explain residuals**. They are enabled as soon as a case is open. These are the same tools an MCP client calls
(`assess_mesh`, `validate_case`, `suggest_boundary_conditions`, `explain_residuals`).

```text not-executed
Assistant panel > "No model needed:" > Assess mesh | Validate case | Suggest BCs | Explain residuals
```

## 5. Connect a model (not executed)

Pick a provider in the sidebar's provider selector and fill in the model name; the presets are:

| Preset | Protocol | Default base URL |
|---|---|---|
| `ollama` | OpenAI-compatible | `http://localhost:11434/v1` |
| `lm-studio` | OpenAI-compatible | `http://localhost:1234/v1` |
| `llama-cpp` | OpenAI-compatible | `http://localhost:8080/v1` |
| `vllm` | OpenAI-compatible | `http://localhost:8000/v1` |
| `openai` | OpenAI-compatible | `https://api.openai.com/v1` |
| `anthropic` | Anthropic Messages | the Anthropic API |

A hosted API needs a key: an environment variable named in the provider entry, or the key field of the sidebar. The code stores a key
from the sidebar in the OS keychain on Windows and macOS and falls back to `keys.json` in the per-user config directory; a round trip
against a real keychain has not been verified (the tests use an in-memory keychain, and the macOS code has not been built; roadmap
F-149). That the key stays out of transcripts, logs and error texts is shown by a planted-key scan with a mock provider (F-094 C5,
F-149), not with a real provider. Whether a given small local model calls the tools well has not been measured.

## 6. Ask, approve, deny (not executed)

The sidebar has two modes. **Read-only** refuses every tool that changes the case or writes files. **Approve writes** shows each such
call as a card with Approve and Deny; read-only tools (checking the mesh, reading settings and residuals) run without asking. A denied
call leaves the case unchanged and the model is told it was denied. Context chips show the open case, the run state, the selected
setting and the model.

```text not-executed
You:        The cavity should run at Re 400. Change the viscosity and run it.
Assistant:  set_setting physics.nu = 0.0025   [Approve] [Deny]
Assistant:  run                                 [Approve] [Deny]
```

That exchange is illustrative only: what a real model answers depends on the model.

## Where to go next

* [MCP design loop](mcp-design-loop.md): the tool calls the sidebar makes, executed.
* [MCP tools](../mcp-tools.md): the access classes and the approval model.
* [The case schema](../case-schema.md): every setting the settings panel edits.
