---
type: concept
tags: [runner, remote, studio, mcp, security]
sources: [adr-thread-runner-records-and-run-sites, adr-thread-studio-electron, doc-service-protocol, doc-studio-guardrails]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Run site

A run site is "where the runner launches a run's solver". There is always the site `local`, today's code path; a remote site is "a machine (a workstation, a cluster login node, a cloud VM) that has an rcfd engine installed and that the user can reach", named by the user in one file. "The project folder stays on the user's machine and stays the truth (G12)": a remote run's folder `runs/<id>/` is created locally and kept up to date from the site, so the Studio, Claude Code, the CLI and the MCP server read it as they read a local run (ADR 0739, F-848) ([[adr-thread-runner-records-and-run-sites]]).

## How rcfd uses it

- Asked for by the maintainer on 2026-11-17: "we should open the studio and if no external simulation service is specified it spins it up and supplies it, otherwise we can use an external one that is in the cloud to run high intensity or sharded or distributed simulations" ([[adr-thread-runner-records-and-run-sites]]).
- Phase 1 (built 2026-11-17): a user-level `sites.toml` (`RCFD_SITES_FILE`, else `%APPDATA%\rcfd\sites.toml` or `~/.config/rcfd/sites.toml`), `rcfd site list|check [--confirm]`, the agent `rcfd-runner site --stdio` over SSH (topology T1), `rcfd-runner start|rerun --site <name>`, live mirroring, forwarded pause / resume / cancel, outputs and fields downloaded at the end, records `site.json` and `site.jsonl` ([[adr-thread-runner-records-and-run-sites]]).
- A site takes no run until it is confirmed (`confirmed = "<fingerprints>"` written by `rcfd site check <name> --confirm`); an engine of another version needs the user's yes for that site and version; another protocol major is refused ([[adr-thread-runner-records-and-run-sites]]).
- MCP: `list_sites` and `start_run {site}`; an MCP client may name only a site with `assistant = true` (default false). The Studio therefore starts remote runs with `rcfd-runner start ... --site <name> --no-runner` and `ensure`, "a direct user action, not the assistant's" (ADR 0801 Addendum 24) ([[adr-thread-runner-records-and-run-sites]], [[adr-thread-studio-electron]]).
- Guardrail G10 gained the sentence that remote run sites are allowed only as named in the user's sites file and confirmed once (ADR 0739 Q3 (a)) ([[doc-studio-guardrails]]). The interactive service stays local; `rcfd serve` refuses a non-loopback `--addr` unless `--allow-unauthenticated-remote` (Q9) ([[doc-service-protocol]]).
- Not yet built: phase 2 (studies on a site, resumable downloads with progress, `rcfd site gc`), phase 3 (Slurm, then PBS), a pool of sites, a remote session service (T4), cloud batch (T3, its own ADR) ([[adr-thread-runner-records-and-run-sites]]).

## Related

- [[rcfd-runner]], [[run]], [[studio]], [[studio-guardrails]], [[rcfd-service-protocol]], [[roadmap-studio-desktop-app]]

## Open questions

- F-848 is `todo` on the roadmap although ADR 0739 says phase 1 of the engine and the Studio is built; the roadmap gap gives no reason ([[roadmap-studio-desktop-app]]).
- The cause of the one S1 failure in the full gate (2026-11-18) is "still inferred" ([[adr-thread-runner-records-and-run-sites]]).
