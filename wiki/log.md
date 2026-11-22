---
type: log
---

# Log

Append-only. Newest at the bottom. Entry header: `## [YYYY-MM-DD] <op> | <title>`.
Last five: `grep "^## \[" log.md | tail -5`

## [2026-11-22] init | Vault created
- Folders: `raw/`, `raw/assets/`, `sources/`, `entities/`, `concepts/`, `analyses/`, `templates/`, `tools/`.
- Schema: `CLAUDE.md` (ingest / query / lint), `AGENTS.md` pointer.
- Seed pages: [[overview]], [[karpathy-llm-wiki]], [[llm-wiki]].

## [2026-11-22] schema | Ingestion plan for existing repo docs
- Added [[ingest-plan]]: inventory, exclusions, granularity rules (ADR threads, roadmap areas), phases P0-P5, 4 open decisions.

## [2026-11-22] schema | Ingest-plan decisions applied
- Thread pages for ADRs; P0-P1 supervised, P2-P3 batched; wiki public, committed to `main` per phase; loop merges ingest into the wiki.
- `CLAUDE.md`: added Steady-state ingest section. [[ingest-plan]]: decisions + Ingest backlog section.

## [2026-11-22] ingest | P0 skeleton (8 repo docs at origin/main 2e94e5ef)
- Sources: [[doc-readme]], [[doc-product]], [[doc-index]], [[doc-roadmap-overview]], [[doc-architecture]], [[doc-glossary]], [[doc-contributing]], [[doc-process-agent-rules]].
- Entities (24): [[crate-graph]] + 19 crate pages, [[fan-01]], [[studio]], [[roadmap]], [[simulation-milestones]].
- Concepts (20): [[adr]], [[f-item]], [[roadmap-status]], [[wave]], [[gate]], [[registered-limit]], [[known-miss]], [[unverified]], [[clean-room]], [[provenance]], [[bit-identical]], [[registration]], [[case-file]], [[benchmark-descriptor]], [[template]], [[report]], [[mrf]], [[run]], [[field-store]], [[mcp]].
- [[overview]] written.
- Contradictions flagged: sliding mesh available (README) vs not available (roadmap overview); structural solver "does not exist" (PRODUCT.md) vs partly built.
- Found: local `main` was 282 behind; `working-framework.md` moved to `docs/process/`; `docs/adr/index.md` is a generated ADR catalog. [[ingest-plan]] corrected.
