// The project trash on the project home (ADR 0736, F-829; G12, G18, G22): Move to the trash for an ended run, a run's
// results and a case file, each confirmed in words that name what moves; Undo of a move as a step of the F-829 history
// (scope `project:<folder>`), which restores through the engine; and the trash itself, listed only when it holds
// anything, with Restore and Empty trash behind the decision panel.
//
// The engine moves and removes (trash_run, trash_results, trash_case, restore_trash, empty_trash, through the main
// process: ./door.ts); the Studio never moves or removes a file of the project itself (G12). At rest this adds no word to
// the home: the move controls are icon buttons, their words their accessible names and tooltips (G22).
import { useState } from 'react';
import type { ProjectView, TrashItem, TrashOutcome } from '../../shared/project.js';
import { history } from '../actions/index.js';
import { DecisionPanel } from '../history/DecisionPanel.js';
import { Button, IconButton } from '../ui/index.js';
import { trashRequest } from './door.js';
import { dateTime, kilobytes, plural } from './format.js';

/** The history scope of the project home's trash steps. */
export const trashScope = (folder: string): string => `project:${folder}`;

/** What one move takes: the engine operation, its arguments, and the words that name what moves. */
export interface TrashTarget {
  readonly what: 'run' | 'results' | 'case';
  readonly params: Readonly<Record<string, unknown>>;
  /** "run r-...", "the results of run r-...", "cases/a.toml". */
  readonly label: string;
  /** What moves and what stays, in words (the confirmation's body). */
  readonly moves: string;
}

/** The move of a run folder: the record, the log, the residuals, the fields and the output. */
export function runTarget(run: string): TrashTarget {
  return {
    what: 'run',
    params: { run },
    label: `run ${run}`,
    moves: `The whole folder runs/${run}/ moves: its record, log, residuals, stored fields and output. Its case file stays.`,
  };
}

/** The move of a run's results: fields/ and output/ (the parts the run holds); the record and the log stay. */
export function resultsTarget(run: string): TrashTarget {
  return {
    what: 'results',
    params: { run },
    label: `the results of run ${run}`,
    moves: `runs/${run}/fields/ and runs/${run}/output/ move, as far as the run holds them (its stored fields, monitors and reports). The run stays in the list with its record, log and residuals.`,
  };
}

/** The move of a case file; the runs made from it stay. */
export function caseTarget(path: string, unsavedSession?: string): TrashTarget {
  return {
    what: 'case',
    params: { path },
    label: path,
    moves:
      `The case file ${path} moves. The runs made from it stay, each with the settings it ran with.` +
      (unsavedSession ? ` The case editor's unsaved changes to it stay in session ${unsavedSession}.` : ''),
  };
}

/** The engine's refusal in words, with the way out (G19). */
export function trashWords(r: Extract<TrashOutcome, { ok: false }>): string {
  const data = (r.data ?? {}) as Record<string, unknown>;
  switch (r.code) {
    case 1002:
      return `${r.message}. Only an ended run moves to the trash: stop it, or wait until it ends.`;
    case 1003: {
      const runs = Array.isArray(data['runs']) ? (data['runs'] as unknown[]).map(String) : [];
      return `${r.message}.${runs.length ? ` Runs still going on from it: ${runs.join(', ')}.` : ''} Wait until they end, or cancel them.`;
    }
    case 1200:
      return typeof data['path'] === 'string'
        ? `${r.message}. ${data['path']} is taken by another file now: move or rename that file, then restore again.`
        : `${r.message}. It changed since it was read; the home shows it as it is now.`;
    case 1202:
      return `${r.message}. It is not there any more; the home shows what is there now.`;
    default:
      return r.message;
  }
}

const tokenOf = (r: TrashOutcome): string => (r.ok && typeof r.result['token'] === 'string' ? r.result['token'] : '');

/**
 * Moves a target to the trash through the engine and records the move as a step of the project's history: undo
 * restores it (restore_trash), redo moves it again (under a new token).
 */
export async function moveToTrash(folder: string, t: TrashTarget): Promise<TrashOutcome> {
  const r = await trashRequest(t.what, t.params);
  if (!r.ok) return r;
  let token = tokenOf(r);
  history().recordStep(trashScope(folder), {
    title: `move ${t.label} to the trash`,
    undo: async () => {
      const u = await trashRequest('restore', { token });
      return u.ok ? { ok: true } : { ok: false, message: trashWords(u) };
    },
    redo: async () => {
      const again = await trashRequest(t.what, t.params);
      if (!again.ok) return { ok: false, message: trashWords(again) };
      token = tokenOf(again);
      return { ok: true };
    },
  });
  return r;
}

/** The icon button that moves one target, its confirmation and the engine's answer. */
export function TrashButton({ folder, target, isDisabled }: { folder: string; target: TrashTarget; isDisabled?: boolean }) {
  const [asking, setAsking] = useState(false);
  const [failed, setFailed] = useState<string | null>(null);
  const [said, setSaid] = useState('');
  const go = async () => {
    setAsking(false);
    const r = await moveToTrash(folder, target);
    if (r.ok) setSaid(`Moved ${target.label} to the trash. Undo puts it back.`);
    else setFailed(trashWords(r));
  };
  return (
    <>
      <IconButton variant="ghost" icon="trash" label={`Move ${target.label} to the trash`} isDisabled={isDisabled ?? false} onPress={() => setAsking(true)} />
      {asking && (
        <DecisionPanel
          title={`Move ${target.label} to the trash?`}
          onCancel={() => setAsking(false)}
          choices={[
            { label: 'Keep it', onPress: () => setAsking(false), safe: true },
            { label: 'Move to the trash', onPress: () => void go() },
          ]}
        >
          <p>{target.moves}</p>
          <p>It goes to .rcfd/trash/ in the project folder. Undo, or Restore in the trash, puts it back; nothing is removed until the trash is emptied.</p>
        </DecisionPanel>
      )}
      {failed !== null && (
        <DecisionPanel
          title={`Failed to move ${target.label} to the trash`}
          failed
          onCancel={() => setFailed(null)}
          choices={[{ label: 'Dismiss', onPress: () => setFailed(null), safe: true }]}
        >
          <p>The engine answered: {failed}</p>
          <p>Nothing was moved.</p>
        </DecisionPanel>
      )}
      <span className="sr-only" aria-live="polite">
        {said}
      </span>
    </>
  );
}

const KIND_WORD: Readonly<Record<string, string>> = { run: 'Run', results: 'Results', case: 'Case file' };

/** The trash of the project: shown only when it holds anything (or could not be read). */
export function TrashGroup({ view }: { view: ProjectView }) {
  const [emptying, setEmptying] = useState(false);
  const [failed, setFailed] = useState<string | null>(null);
  const [said, setSaid] = useState('');
  const trash = view.trash;
  if (!trash || (trash.total === 0 && trash.error === null)) {
    return said ? (
      <span className="sr-only" aria-live="polite">
        {said}
      </span>
    ) : null;
  }
  const items = trash.items;
  const restore = async (i: TrashItem) => {
    const r = await trashRequest('restore', { token: i.token });
    if (r.ok) setSaid(`Restored ${i.path}.`);
    else setFailed(`Failed to restore ${i.path}: ${trashWords(r)}`);
  };
  const empty = async () => {
    setEmptying(false);
    const r = await trashRequest('empty', { tokens: items.map((i) => i.token) });
    if (r.ok) setSaid(`Removed ${plural(items.length, 'item')} from the trash for good.`);
    else setFailed(`Failed to empty the trash: ${trashWords(r)}`);
  };
  return (
    <section className="ph-group" aria-labelledby="ph-g-trash" data-group="trash" data-words="exception">
      <h2 id="ph-g-trash" className="ph-group-head">
        <span className="ph-group-title">Trash</span>
        <span className="ph-group-count">{trash.total}</span>
        <span className="ph-group-what">
          {' '}
          .rcfd/trash/, {kilobytes(trash.bytes)} kB
        </span>
      </h2>
      {trash.error !== null && (
        <div className="ph-group-error" role="alert">
          <span>The trash could not be read: {trash.error}</span>
        </div>
      )}
      {items.length > 0 && (
        <table className="ph-table" data-words="data">
          <thead>
            <tr>
              <th scope="col">Item</th>
              <th scope="col">Kind</th>
              <th scope="col">Moved</th>
              <th scope="col" className="ph-num">
                Size, kB
              </th>
              <th scope="col">
                <span className="sr-only">Restore</span>
              </th>
            </tr>
          </thead>
          <tbody>
            {items.map((i) => (
              <tr key={i.token} data-token={i.token}>
                <th scope="row" className="ph-name-cell">
                  {i.path}
                  {i.parts.length > 0 && <span className="ph-run-id">{i.parts.join(', ')}</span>}
                </th>
                <td>{KIND_WORD[i.kind] ?? i.kind}</td>
                <td>
                  {dateTime(i.trashed)}
                  {i.reason ? `, ${i.reason}` : ''}
                </td>
                <td className="ph-num">{kilobytes(i.bytes)}</td>
                <td data-words="chrome">
                  <Button variant="chip" onPress={() => void restore(i)} aria-label={`Restore ${i.path}`}>
                    Restore
                  </Button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      {trash.total > items.length && <p className="ph-more">and {trash.total - items.length} older items</p>}
      {items.length > 0 && <Button onPress={() => setEmptying(true)}>Empty trash</Button>}
      {emptying && (
        <DecisionPanel
          title={`Remove ${plural(items.length, 'item')} in the trash for good?`}
          onCancel={() => setEmptying(false)}
          choices={[
            { label: 'Keep the trash', onPress: () => setEmptying(false), safe: true },
            { label: 'Remove for good', onPress: () => void empty() },
          ]}
        >
          <p>
            The engine removes {items.map((i) => i.path).join(', ')} ({kilobytes(items.reduce((n, i) => n + i.bytes, 0))} kB) from the disk. This cannot be undone: no
            Undo and no Restore brings them back.
          </p>
          {trash.total > items.length && <p>The {trash.total - items.length} older items not listed here stay in the trash.</p>}
        </DecisionPanel>
      )}
      {failed !== null && (
        <DecisionPanel title="Failed: the trash was not changed" failed onCancel={() => setFailed(null)} choices={[{ label: 'Dismiss', onPress: () => setFailed(null), safe: true }]}>
          <p>{failed}</p>
        </DecisionPanel>
      )}
      <span className="sr-only" aria-live="polite">
        {said}
      </span>
    </section>
  );
}
