// Run ids and project files in Claude Code's replies (F-814): each one the open project holds becomes a reference chip
// that opens the run in the Runs view, a case file in the case editor (F-812), or any other file on the project home. Only what the engine listed for this project
// links (its runs, cases, meshes and notes); a run id or path the project does not hold stays plain text, so a link
// never leads nowhere. Pure, so the unit tests run it in Node.

export type Ref = { readonly kind: 'run'; readonly id: string } | { readonly kind: 'file'; readonly path: string };

export interface Known {
  /** Run ids of the project (`list_runs`). */
  readonly runs: ReadonlySet<string>;
  /** Project-relative, `/`-separated paths of its cases, meshes and notes. */
  readonly files: ReadonlySet<string>;
  /** Those of `files` the engine reads as cases (files of cases/ ending .toml or .json, ADR 0148): they open in the editor. */
  readonly cases?: ReadonlySet<string>;
  /** The project folder, to recognise absolute paths inside it. */
  readonly folder: string;
}

/** The run id form of the project folder contract (`r-<UTC yyyymmddThhmmss>-<4 hex>`). */
const RUN_ID = /\br-\d{8}T\d{6}-[0-9a-f]{4}\b/g;
/** A project-relative path under one of the listed folders. */
const REL = /(?:^|[\s(`'"*[<])((?:\.\/)?(?:cases|meshes|notes)\/[^\s`'"*()[\]<>,;]+)/g;

const trimEnd = (p: string) => p.replace(/[.:!?]+$/, '');

/** The references of `text` that the project holds, each once, in the order they first appear. */
export function findRefs(text: string, known: Known): Ref[] {
  const found: { at: number; ref: Ref; key: string }[] = [];
  for (const m of text.matchAll(RUN_ID)) {
    if (known.runs.has(m[0])) found.push({ at: m.index, ref: { kind: 'run', id: m[0] }, key: `run:${m[0]}` });
  }
  const files = (t: string, offset: number) => {
    for (const m of t.matchAll(REL)) {
      const rel = trimEnd(m[1]!.replace(/^\.\//, ''));
      if (known.files.has(rel)) found.push({ at: offset + m.index, ref: { kind: 'file', path: rel }, key: `file:${rel}` });
    }
  };
  const slashed = text.replace(/\\/g, '/');
  files(slashed, 0);
  // absolute paths inside the project folder: the folder (any separators, ASCII case ignored) followed by a path
  const folder = known.folder.replace(/\\/g, '/').replace(/\/+$/, '');
  if (folder) {
    const low = slashed.toLowerCase();
    const needle = `${folder.toLowerCase()}/`;
    for (let i = low.indexOf(needle); i >= 0; i = low.indexOf(needle, i + 1)) {
      const rest = slashed.slice(i + needle.length).match(/^[^\s`'"*()[\]<>,;]+/)?.[0] ?? '';
      const rel = trimEnd(rest);
      if (known.files.has(rel)) found.push({ at: i, ref: { kind: 'file', path: rel }, key: `file:${rel}` });
      const run = rel.match(/^runs\/(r-\d{8}T\d{6}-[0-9a-f]{4})(?:\/|$)/)?.[1];
      if (run && known.runs.has(run)) found.push({ at: i, ref: { kind: 'run', id: run }, key: `run:${run}` });
    }
  }
  const seen = new Set<string>();
  return found
    .sort((a, b) => a.at - b.at)
    .filter((f) => (seen.has(f.key) ? false : (seen.add(f.key), true)))
    .map((f) => f.ref);
}

/** Where a file reference opens: a case file in the case editor, anything else on the project home (filtered to it). */
export function fileOpensIn(path: string, known: Known): 'case' | 'home' {
  return known.cases?.has(path) ? 'case' : 'home';
}

/** A reply's text as paragraphs of plain and bold runs (`**bold**`, the one mark the panel draws; DESIGN.md). */
export function paragraphs(text: string): { bold: boolean; text: string }[][] {
  return text
    .split(/\n{2,}/)
    .map((p) => p.trim())
    .filter((p) => p !== '')
    .map((p) =>
      p
        .split(/(\*\*[^*\n]+\*\*)/)
        .filter((s) => s !== '')
        .map((s) => (s.startsWith('**') && s.endsWith('**') && s.length > 4 ? { bold: true, text: s.slice(2, -2) } : { bold: false, text: s })),
    );
}
