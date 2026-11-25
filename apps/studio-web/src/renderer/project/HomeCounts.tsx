// F-828 (ADR 0795): the counts of the project home with their provenance: what was counted, by whom (the engine's answer,
// or the Studio over the names in a folder or over the engine's list) and the engine's own count where project_info gives
// one. A count is no tab stop: the Show segment of its kind opens it by Ctrl+I (data-provenance, `countKeys`), then the
// other counts of the kind; a click opens those that are not in a button.
import type { ReactNode } from 'react';
import type { ProjectView } from '../../shared/project.js';
import { Traced, homeCount } from '../provenance/index.js';

export type CountKind = 'cases' | 'meshes' | 'runs' | 'notes' | 'designs';

/** Where a kind's total comes from. */
const TOTAL: Readonly<Record<CountKind, { readonly file: string; readonly readBy: string; readonly counted: string }>> = {
  cases: { file: 'cases/', readBy: 'the Studio, from the names of the files below cases/', counted: 'by the Studio: every file below cases/, except the temporary file of an atomic write' },
  notes: { file: 'notes/', readBy: 'the Studio, from the names of the files below notes/', counted: 'by the Studio: every file below notes/, except the temporary file of an atomic write' },
  designs: { file: 'designs/', readBy: 'the Studio, from the names in designs/', counted: 'by the Studio: every file and folder directly in designs/, except the temporary file of an atomic write' },
  meshes: { file: 'meshes/', readBy: 'list_meshes (rcfd-mcp, read-only), total', counted: 'by the engine: the meshes of meshes/, an OpenFOAM folder as one' },
  runs: { file: 'runs/', readBy: 'list_runs (rcfd-mcp, read-only), total', counted: 'by the engine: the runs of runs/ it could read' },
};

interface Own {
  readonly view: ProjectView;
  readonly pkey: string;
  readonly quantity: string;
  readonly value: number;
  readonly file: string;
  readonly readBy: string;
  readonly counted: string;
  readonly engineCount?: { readonly field: string; readonly value: number } | undefined;
  readonly children?: ReactNode;
}

/** A count with its own facts. */
export function Count({ view, pkey, children, ...facts }: Own) {
  return (
    <Traced pkey={pkey} subject={() => ({ kind: 'project', source: homeCount({ ...facts, folder: view.folder, engine: view.engine, readAt: view.readAt }) })}>
      {children ?? facts.value}
    </Traced>
  );
}

const engineCountOf = (view: ProjectView, kind: CountKind): Own['engineCount'] => {
  const value = view.info?.counts[kind];
  return value === undefined ? undefined : { field: `counts.${kind}`, value };
};

/** The total of a kind (its segment, its group's head). */
export function KindCount({ view, kind, pkey, value }: { view: ProjectView; kind: CountKind; pkey: string; value: number }) {
  return <Count view={view} pkey={pkey} quantity={kind} value={value} {...TOTAL[kind]} engineCount={engineCountOf(view, kind)} />;
}

/** The sum of the four kinds the home lists. */
export function AllCount({ view, value }: { view: ProjectView; value: number }) {
  return (
    <Count
      view={view}
      pkey="home:seg:all"
      quantity="entries"
      value={value}
      file="cases/, meshes/, runs/, notes/"
      readBy="the totals of the four kinds (their own provenance says where each was read)"
      counted={`by the Studio: the sum of cases ${view.cases.total}, meshes ${view.meshes.total}, runs ${view.runs.total} and notes ${view.notes.total}`}
    />
  );
}

/** The engine's own count, shown where it is not the number the home lists. */
export function EngineCount({ view, kind, value }: { view: ProjectView; kind: CountKind; value: number }) {
  return <Count view={view} pkey={`home:${kind}:engine`} quantity={`${kind} counted by the engine`} value={value} file={TOTAL[kind].file} readBy={`project_info (rcfd-mcp, read-only), counts.${kind}`} counted="by the engine" />;
}

/** The entries a filter leaves. */
export function ShownCount({ view, kind, value, filter }: { view: ProjectView; kind: CountKind; value: number; filter: string }) {
  return (
    <Count
      view={view}
      pkey={`home:${kind}:shown`}
      quantity={`${kind} shown`}
      value={value}
      file={TOTAL[kind].file}
      readBy={TOTAL[kind].readBy.replace(/, total$/, '')}
      counted={`by the Studio: the listed entries whose text holds “${filter}”`}
    />
  );
}

/** The two numbers of "The first N of M are listed". */
export function FirstOf({ view, kind, listed, total }: { view: ProjectView; kind: CountKind; listed: number; total: number }) {
  return (
    <>
      The first{' '}
      <Count
        view={view}
        pkey={`home:${kind}:listed`}
        quantity={`${kind} listed`}
        value={listed}
        file={TOTAL[kind].file}
        readBy={TOTAL[kind].readBy.replace(/, total$/, '')}
        counted="by the Studio: the entries of the list it read (it reads a limited number)"
      />{' '}
      of <KindCount view={view} kind={kind} pkey={`home:${kind}:total`} value={total} /> are listed.
    </>
  );
}

/** A remainder the Studio subtracts: the engine's total less the entries listed. */
export function MoreCount({ view, pkey, quantity, total, listed, file, readBy }: { view: ProjectView; pkey: string; quantity: string; total: number; listed: number; file: string; readBy: string }) {
  return <Count view={view} pkey={pkey} quantity={quantity} value={total - listed} file={file} readBy={readBy} counted={`by the Studio: ${total} in all less the ${listed} listed`} />;
}

export const ISSUES_READ = 'project_info (rcfd-mcp, read-only), issues_total';
export const TRASH_READ = 'list_trash (rcfd-mcp, read-only), total';

/** The key of the runs of a case, and of the files of a mesh (a path may hold a space; a key may not). */
export const runsKey = (path: string): string => `home:case-runs:${encodeURIComponent(path)}`;
export const filesKey = (path: string): string => `home:mesh-files:${encodeURIComponent(path)}`;

const rest = (kind: CountKind): readonly string[] => [`home:${kind}`, `home:${kind}:shown`, `home:${kind}:engine`, `home:${kind}:listed`, `home:${kind}:total`];

/** The keys the segment of a kind names: its own count, then the others of the kind. */
export function countKeys(view: ProjectView, kind: Exclude<CountKind, 'designs'>): string {
  return [`home:seg:${kind}`, ...rest(kind), ...(kind === 'meshes' ? view.meshes.items.filter((m) => m.files > 1 || m.kind === 'openfoam').map((m) => filesKey(m.path)) : [])].join(' ');
}

/** The keys the All segment names: its own count, then the groups that have no segment. */
export const ALL_KEYS = ['home:seg:all', 'home:issues', 'home:issues:more', ...rest('designs'), 'home:trash', 'home:trash:older'].join(' ');
