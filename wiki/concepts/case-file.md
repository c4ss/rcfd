---
type: concept
tags: [case]
sources: [doc-glossary, doc-case-schema, doc-tutorial-quick-start-cli, doc-tutorial-gallery, doc-run-control, doc-python]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Case file

A TOML or JSON file that describes one simulation: mesh, physics, boundaries, solver and reports ([[doc-glossary]]).

## How rcfd uses it

- Every setting is documented in `docs/case-schema.md` (to ingest in P1).
- `rcfd check <case>` builds the mesh and validates the set-up without solving ([[doc-readme]]).
- The case file is a settings tree; `rcfd schema` prints its JSON Schema, the service returns it from `settings.schema`, and the Studio's case editor and the assistant tools read the same tree ([[doc-case-schema]]). Every section is summarised on the hub [[doc-case-schema]] and its section pages.
- Every setting has a dotted path (`solver.dt`, `boundaries.lid.velocity_value`); `rcfd run --set path=json` overrides one before anything else; malformed argument exit 2, unknown path or invalid value exit 3 ([[doc-run-control]]). The Python package sets values the same way ([[doc-python]]).
- Errors are located: key and TOML line ([[located-error]]); data-driven [[setup-rules]] run before solving.
- New keys and optional tables are inert at their defaults and many are left out of the settings hash, so older cases keep their text, hash and results ([[doc-case-schema]], [[bit-identical]]).
- `*.toml` files in `cases/` are case files; `case.json` files are [[benchmark-descriptor]]s, which `rcfd run` refuses ([[doc-tutorial-gallery]]). Other input formats: the [[solid-case-file]], reactor cases and FW-H surface files.
- `rcfd validate` checks keys and values without building anything ([[doc-tutorial-quick-start-cli]]).

## Related

- [[benchmark-descriptor]]
- [[template]]
- [[report]]
- [[run]]
- [[located-error]]
- [[setup-rules]]
- [[parametric-study]]
- [[solid-case-file]]
