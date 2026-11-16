// A tolerant parser of `claude -p --output-format stream-json --verbose --include-partial-messages` lines, a port of
// crates/rcfd-claude/src/stream.rs (ADR 0126). No schema is published; the shapes were observed on Claude Code 2.1.310:
// `system/init` (session_id, permissionMode, claude_code_version, model), `stream_event` wrapping a
// `content_block_delta` / `text_delta` (text), `assistant` (message.content[] with `tool_use` blocks: id, name, input),
// `system/permission_denied` (tool_name, tool_use_id, message) and `result` (subtype, is_error, result, session_id,
// total_cost_usd). Text comes from the deltas only, because the full `assistant` event repeats it.

export type StreamEvent =
  | { readonly kind: 'init'; readonly session: string; readonly mode: string | null; readonly version: string | null; readonly model: string | null }
  | { readonly kind: 'text'; readonly text: string }
  | { readonly kind: 'tool'; readonly id: string; readonly name: string; readonly input: unknown }
  | { readonly kind: 'denied'; readonly tool: string; readonly toolUseId: string; readonly message: string }
  | {
      readonly kind: 'result';
      readonly session: string;
      readonly isError: boolean;
      readonly subtype: string;
      readonly text: string | null;
      readonly costUsd: number | null;
    }
  | { readonly kind: 'unrecognised'; readonly type: string | null };

const str = (v: unknown): string | null => (typeof v === 'string' ? v : null);
const obj = (v: unknown): Record<string, unknown> | null => (typeof v === 'object' && v !== null && !Array.isArray(v) ? (v as Record<string, unknown>) : null);

/** Parses one stdout line. Empty for blank lines and for known shapes the panel does not need. */
export function parseLine(line: string): StreamEvent[] {
  if (line.trim() === '') return [];
  let v: unknown;
  try {
    v = JSON.parse(line);
  } catch {
    return [{ kind: 'unrecognised', type: null }];
  }
  const o = obj(v);
  const type = o ? str(o['type']) : null;
  if (!o || type === null) return [{ kind: 'unrecognised', type: null }];
  const sub = str(o['subtype']);
  const kind = sub === null ? type : `${type}/${sub}`;
  const unrec = (t: string | null = kind): StreamEvent[] => [{ kind: 'unrecognised', type: t }];
  if (type === 'assistant') {
    const content = obj(o['message'])?.['content'];
    if (!Array.isArray(content)) return unrec('assistant');
    return content
      .map(obj)
      .filter((b): b is Record<string, unknown> => b !== null && b['type'] === 'tool_use')
      .map((b): StreamEvent => {
        const id = str(b['id']);
        const name = str(b['name']);
        return id !== null && name !== null ? { kind: 'tool', id, name, input: b['input'] ?? null } : { kind: 'unrecognised', type: 'assistant/tool_use' };
      });
  }
  if (type === 'system' && sub === 'init') {
    const session = str(o['session_id']);
    if (session === null) return unrec();
    return [{ kind: 'init', session, mode: str(o['permissionMode']), version: str(o['claude_code_version']), model: str(o['model']) }];
  }
  if (type === 'system' && sub === 'permission_denied') {
    const tool = str(o['tool_name']);
    const toolUseId = str(o['tool_use_id']);
    if (tool === null || toolUseId === null) return unrec();
    return [{ kind: 'denied', tool, toolUseId, message: str(o['message']) ?? '' }];
  }
  if ((type === 'system' && (sub === 'status' || sub === 'thinking_tokens')) || type === 'rate_limit_event' || type === 'user') return [];
  if (type === 'stream_event') {
    const ev = obj(o['event']);
    const et = ev ? str(ev['type']) : null;
    if (et === null) return unrec();
    if (et !== 'content_block_delta') return [];
    const d = obj(ev!['delta']);
    if (!d) return unrec('stream_event/content_block_delta');
    if (d['type'] !== 'text_delta') return [];
    const t = str(d['text']);
    return t === null ? unrec('stream_event/text_delta') : [{ kind: 'text', text: t }];
  }
  if (type === 'result') {
    const session = str(o['session_id']);
    if (session === null) return unrec();
    const cost = o['total_cost_usd'];
    return [
      {
        kind: 'result',
        session,
        isError: typeof o['is_error'] === 'boolean' ? (o['is_error'] as boolean) : true,
        subtype: sub ?? '',
        text: str(o['result']),
        costUsd: typeof cost === 'number' && Number.isFinite(cost) ? cost : null,
      },
    ];
  }
  if (type === 'system') return []; // other system lines (hook output and the like) carry no action
  return unrec();
}

/**
 * Why `ev` aborts a turn whose permission mode has not been checked yet; null when it may pass (ADR 0128 finding 5).
 * Only a `system/init` with `permissionMode` `dontAsk` checks the turn; anything with an action before it aborts.
 */
export function uncheckedProblem(ev: StreamEvent): string | null {
  const before = (what: string) => `${what} arrived before a checked system/init (session_id and permissionMode dontAsk)`;
  switch (ev.kind) {
    case 'init':
      return ev.mode === 'dontAsk' ? null : `the CLI did not apply --permission-mode dontAsk (system/init reports ${JSON.stringify(ev.mode)})`;
    case 'unrecognised':
      if (ev.type === 'system/init') return "the CLI's system/init line has no session_id, so its permission mode cannot be checked";
      return before(`an unrecognised line (${JSON.stringify(ev.type)})`);
    case 'tool':
      return before(`a tool call (${ev.name})`);
    case 'text':
      return before('assistant text');
    case 'denied':
      return before(`a permission denial (${ev.tool})`);
    case 'result':
      return before('a result');
  }
}
