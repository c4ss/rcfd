// A tolerant reader of Claude Code session transcripts (`<claude home>/projects/<dir>/<session-id>.jsonl`), a port of
// crates/rcfd-claude/src/transcript.rs (ADR 0126). The line format is declared internal and version-dependent, so only
// four things are taken from it: role, text, tool names and timestamp, from lines whose top-level `type` is `user` or
// `assistant`. Known bookkeeping lines are skipped; anything else becomes an `unrecognised` entry. Nothing here throws
// on content.
//
// The mirror reads only complete lines: a writer in the middle of a line is never half-read.
import { open } from 'node:fs/promises';

export type Role = 'user' | 'assistant' | 'tool_result';

export type TranscriptEntry =
  | { readonly kind: 'message'; readonly role: Role; readonly text: string; readonly tools: readonly string[]; readonly at: string | null }
  | { readonly kind: 'unrecognised'; readonly type: string | null };

/** Bookkeeping line types observed on 2.1.310 that carry nothing for the panel. */
const SKIPPED = new Set([
  'queue-operation',
  'attachment',
  'atis-latch',
  'api-request-shape',
  'api-request-blob',
  'api-request',
  'last-prompt',
  'cost-state',
  'mode',
  'summary',
]);

const str = (v: unknown): string | null => (typeof v === 'string' ? v : null);
const obj = (v: unknown): Record<string, unknown> | null => (typeof v === 'object' && v !== null && !Array.isArray(v) ? (v as Record<string, unknown>) : null);

/** Parses one line. Null for blank and bookkeeping lines. */
export function parseEntry(line: string): TranscriptEntry | null {
  if (line.trim() === '') return null;
  let v: unknown;
  try {
    v = JSON.parse(line);
  } catch {
    return { kind: 'unrecognised', type: null };
  }
  const o = obj(v);
  const type = o ? str(o['type']) : null;
  if (!o || type === null) return { kind: 'unrecognised', type: null };
  if (SKIPPED.has(type)) return null;
  if (type !== 'user' && type !== 'assistant') return { kind: 'unrecognised', type };
  const content = obj(o['message'])?.['content'];
  const parts: string[] = [];
  const tools: string[] = [];
  let role: Role = type;
  if (typeof content === 'string') parts.push(content);
  else if (Array.isArray(content)) {
    for (const b of content) {
      const block = obj(b);
      switch (block ? str(block['type']) : null) {
        case 'text': {
          const t = str(block!['text']);
          if (t !== null) parts.push(t);
          break;
        }
        case 'tool_use': {
          const n = str(block!['name']);
          if (n !== null) tools.push(n);
          break;
        }
        case 'tool_result': {
          role = 'tool_result';
          const c = block!['content'];
          if (typeof c === 'string') parts.push(c);
          else if (Array.isArray(c)) for (const x of c) {
            const t = str(obj(x)?.['text']);
            if (t !== null) parts.push(t);
          }
          break;
        }
        default:
        // thinking, images and future block types are not shown
      }
    }
  } else return { kind: 'unrecognised', type };
  return { kind: 'message', role, text: parts.join('\n'), tools, at: str(o['timestamp']) };
}

/** The complete lines of `buf` (an unterminated last line is left), and the bytes they take. */
function completeLines(buf: Buffer, skipFirst: boolean): { lines: string[]; used: number } {
  const end = buf.lastIndexOf(0x0a) + 1;
  let body = buf.subarray(0, end);
  if (skipFirst) {
    const nl = body.indexOf(0x0a);
    body = nl < 0 ? body.subarray(0, 0) : body.subarray(nl + 1);
  }
  const lines = body
    .toString('utf8')
    .split('\n')
    .filter((l) => l.length > 0);
  return { lines, used: end };
}

const entries = (lines: string[]): TranscriptEntry[] => lines.map(parseEntry).filter((e): e is TranscriptEntry => e !== null);

/**
 * The last `max` entries of a transcript and the byte offset after the last complete line, so that `readNew`
 * continues exactly there. Reads from the end, doubling the window from 256 KiB, so a large transcript is not read whole.
 */
export async function tail(file: string, max: number): Promise<{ entries: TranscriptEntry[]; offset: number }> {
  const fh = await open(file, 'r');
  try {
    const len = (await fh.stat()).size;
    for (let window = 256 * 1024; ; window *= 2) {
      const start = Math.max(0, len - window);
      const buf = Buffer.alloc(len - start);
      await fh.read(buf, 0, buf.length, start);
      const { lines, used } = completeLines(buf, start > 0);
      const all = entries(lines);
      if (all.length >= max || start === 0) return { entries: all.slice(Math.max(0, all.length - max)), offset: start + used };
    }
  } finally {
    await fh.close();
  }
}

/** Entries appended since `offset` and the new offset; only complete lines are consumed. A shorter file starts over. */
export async function readNew(file: string, offset: number): Promise<{ entries: TranscriptEntry[]; offset: number; reset: boolean }> {
  const fh = await open(file, 'r');
  try {
    const len = (await fh.stat()).size;
    const reset = len < offset;
    const from = reset ? 0 : offset;
    const buf = Buffer.alloc(len - from);
    await fh.read(buf, 0, buf.length, from);
    const { lines, used } = completeLines(buf, false);
    return { entries: entries(lines), offset: from + used, reset };
  } finally {
    await fh.close();
  }
}

/**
 * The head of a transcript: the `cwd` written in its first lines (to confirm a session belongs to the folder, because
 * the directory naming maps different paths to the same name) and the start of its first user message. Reads at most
 * the first 1 MiB.
 */
export async function head(file: string): Promise<{ cwd: string | null; title: string | null }> {
  const fh = await open(file, 'r');
  try {
    const buf = Buffer.alloc(Math.min((await fh.stat()).size, 1 << 20));
    await fh.read(buf, 0, buf.length, 0);
    let cwd: string | null = null;
    let title: string | null = null;
    for (const line of completeLines(buf, false).lines) {
      let v: unknown;
      try {
        v = JSON.parse(line);
      } catch {
        continue;
      }
      const o = obj(v);
      if (!o) continue;
      cwd ??= str(o['cwd']);
      if (title === null && o['type'] === 'user') {
        const e = parseEntry(line);
        if (e?.kind === 'message' && e.role === 'user' && e.text.trim() !== '') title = e.text.trim().replace(/\s+/g, ' ').slice(0, 120);
      }
      if (cwd !== null && title !== null) break;
    }
    return { cwd, title };
  } finally {
    await fh.close();
  }
}
