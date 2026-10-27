# Security policy

## Reporting a vulnerability

Report a vulnerability privately through GitHub's private vulnerability reporting: on the repository page, open
**Security**, then **Report a vulnerability**. Do not open a public issue for it. Say what is affected, how to reproduce
it and what an attacker gains. Only the `main` branch is supported; there are no releases yet.

## What is in scope

rcfd is early-stage software. These parts run programs, write files or listen for requests, so they are the places to look:

* **`rcfd serve`.** It listens on `127.0.0.1:7878` by default and has **no authentication**: any local process can call
  it. Without `--root` it reads and writes files at any path the calling client names. `--addr` accepts any address,
  including a non-loopback one, which exposes the same unauthenticated service to the network. Run it only with `--root`
  and only on loopback.
* **The run service (`rcfd-runner`).** It starts the solver binary given by `RCFD_BIN`, or the `rcfd` binary next to the
  runner, and reads per-project settings from `<project>/.rcfd/runner-config.json`.
* **The MCP server (`rcfd-mcp`).** Some of its tools write files in the project (case files, exported results). Without
  `--root` file access is not confined; `--read-only` refuses the tools that change state.
* **The Claude Code bridge (`rcfd-claude`).** It starts the `claude` command-line tool with an allow-list of permissions.
  The allow-list is a floor, not the whole gate: user and project settings of that tool can add their own rules (see
  ADR 0126).
* Parsers of mesh and result files (`rcfd-io`), which read untrusted input.

The assistant crate (`rcfd-assistant`) makes outbound calls only to the language-model endpoints the user configures. rcfd
sends no telemetry.
