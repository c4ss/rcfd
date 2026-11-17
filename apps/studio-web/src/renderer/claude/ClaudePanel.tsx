// The Claude Code panel (F-814; surface brief .impeccable/surfaces/web-src-renderer-claude-claudepanel-tsx.md): the
// Claude Code chat for the open project, as the right column of the workspace (DESIGN.md Layout, Claude Code panel).
//
// Everything shown comes in the state the main process pushes on the Claude port (window.studioClaude through ./door.ts,
// src/shared/claude.ts); the panel keeps only what is being typed. The rules of G11 are the main process's: this panel
// shows them (a read-only session says why, the one writer, the allow-list) and never works round them.
import { useEffect, useMemo, useRef, useState, type KeyboardEvent, type ReactNode } from 'react';
import { TextArea, TextField } from 'react-aria-components';
import type { ClaudeError, ClaudeState, ClaudeView, FeedItem, SessionRow, TurnView } from '../../shared/claude.js';
import { FLOOR_CAVEAT } from '../../shared/claude.js';
import type { ProjectView } from '../../shared/project.js';
import { Button, IconButton, Quiet, StatePictogram, Unavailable } from '../ui/index.js';
import { clock, dateTime } from '../project/format.js';
import { claudeActions, watchClaudeState } from './door.js';
import { findRefs, paragraphs, type Known, type Ref } from './links.js';
import './claude.css';

export interface ClaudePanelProps {
  /** The open project as the engine answered it: its runs and files are what a reply can link to. */
  readonly project: ProjectView;
  readonly onOpenRun: (id: string) => void;
  readonly onOpenFile: (path: string) => void;
  /** Hides the panel (the workspace keeps a button to show it again). */
  readonly onHide: () => void;
}

function useClaudeState(): ClaudeState | null {
  const [state, setState] = useState<ClaudeState | null>(null);
  useEffect(() => watchClaudeState(setState), []);
  return state;
}

const shortId = (id: string) => id.slice(0, 8);

const OPEN_WORD: Record<SessionRow['open'], string> = {
  open: 'open elsewhere, read-only',
  closed: 'closed',
  unknown: 'state unknown, read-only',
  studio: 'this Studio is writing',
};

export function ClaudePanel({ project, onOpenRun, onOpenFile, onHide }: ClaudePanelProps) {
  const state = useClaudeState();
  const view = state?.project ?? null;
  const known: Known = useMemo(
    () => ({
      runs: new Set(project.runs.items.map((r) => r.id)),
      files: new Set([...project.cases.items, ...project.meshes.items, ...project.notes.items].map((x) => x.path)),
      folder: project.folder,
    }),
    [project.runs.items, project.cases.items, project.meshes.items, project.notes.items, project.folder],
  );
  const name = project.info?.name ?? project.folder;
  return (
    <aside className="cc" aria-labelledby="cc-title">
      <div className="cc-head">
        <span className="cc-dot" aria-hidden="true" />
        <h2 id="cc-title" className="cc-title">
          Claude Code
        </h2>
        <span className="cc-for" title={project.folder}>
          · <span data-words="data">{name}</span>
        </span>
        <span className="cc-head-actions">
          {view && (
            <Button variant="ghost" onPress={() => void claudeActions.refresh()}>
              List again
            </Button>
          )}
          <IconButton variant="ghost" icon="close" label="Hide Claude Code" onPress={onHide} />
        </span>
      </div>
      {!state || !view ? (
        <p className="cc-empty" role="status">
          Reading the project’s Claude Code sessions…
        </p>
      ) : (
        <PanelBody state={state} view={view} known={known} onOpenRun={onOpenRun} onOpenFile={onOpenFile} />
      )}
    </aside>
  );
}

function PanelBody({
  state,
  view,
  known,
  onOpenRun,
  onOpenFile,
}: {
  state: ClaudeState;
  view: ClaudeView;
  known: Known;
  onOpenRun: (id: string) => void;
  onOpenFile: (path: string) => void;
}) {
  const [error, setError] = useState<ClaudeError | null>(null);
  const selectedRow = view.selected === null ? null : (view.sessions.find((s) => s.id === view.selected) ?? null);
  const busy = view.writer !== null;
  const select = (id: string | null) => {
    setError(null);
    void claudeActions.select(id).then((r) => setError(r.ok ? null : r.error));
  };
  return (
    <>
      {!state.program.configured && (
        <div className="cc-notice" role="note" data-words="exception">
          <p className="cc-notice-title">Claude Code is not connected</p>
          <p>{state.program.reason}</p>
        </div>
      )}
      <Sessions view={view} busy={busy} onSelect={select} />
      <Feed view={view} row={selectedRow} known={known} onOpenRun={onOpenRun} onOpenFile={onOpenFile} />
      <Trust view={view} onError={setError} />
      <Approvals view={view} />
      <TurnLine turn={view.turn} error={error} version={view.versionWarning} />
      <Composer state={state} view={view} row={selectedRow} onError={setError} />
    </>
  );
}

// ---- sessions

function Sessions({ view, busy, onSelect }: { view: ClaudeView; busy: boolean; onSelect: (id: string | null) => void }) {
  const reason = busy ? 'A message is being answered; wait for the reply or stop it, then choose another session.' : null;
  return (
    <section className="cc-sessions" aria-labelledby="cc-sessions-title">
      <div className="cc-section-head">
        <h3 id="cc-sessions-title" className="cc-section-title">
          Sessions <span data-words="data">{view.sessions.length}</span>
        </h3>
        {/* G22: when the list was read is said to readers */}
        <Quiet>{view.listedAt ? ` listed ${clock(view.listedAt)}` : ' not listed yet'}</Quiet>
      </div>
      <ul className="cc-session-list" aria-label="Claude Code sessions of this project">
        <li>
          <SessionButton current={view.selected === null} disabledReason={reason} onPress={() => onSelect(null)} label="New chat">
            <span className="cc-session-name">New chat</span>
            <Quiet> starts a session in this folder</Quiet>
          </SessionButton>
        </li>
        {view.sessions.map((s) => (
          <li key={s.id}>
            <SessionButton
              current={view.selected === s.id}
              disabledReason={reason}
              onPress={() => onSelect(s.id)}
              label={`${s.name ?? s.title ?? `Session ${shortId(s.id)}`}, ${OPEN_WORD[s.open]}${s.pid !== null ? `, process ${s.pid}` : ''}`}
            >
              <span className="cc-session-name" data-words="data">
                {s.name ?? s.title ?? `Session ${shortId(s.id)}`}
              </span>
              <span className={`cc-session-state cc-session-state--${s.open}`} data-open={s.open}>
                {OPEN_WORD[s.open]}
                {s.pid !== null ? <span data-words="data">{` · process ${s.pid}`}</span> : ''}
                {s.modified ? <span data-words="data">{` · ${dateTime(s.modified)}`}</span> : ''}
              </span>
            </SessionButton>
          </li>
        ))}
      </ul>
      {view.sessions.length === 0 && <Quiet>No Claude Code session has been started in this folder yet.</Quiet>}
      {view.warnings.length > 0 && (
        <ul className="cc-warnings" aria-label="Problems listing the sessions" data-words="exception">
          {view.warnings.map((w) => (
            <li key={w}>{w}</li>
          ))}
        </ul>
      )}
    </section>
  );
}

function SessionButton({
  current,
  disabledReason,
  onPress,
  label,
  children,
}: {
  current: boolean;
  disabledReason: string | null;
  onPress: () => void;
  label: string;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      className={`cc-session${current ? ' cc-session--current' : ''}`}
      aria-current={current ? 'true' : undefined}
      aria-label={label}
      aria-disabled={disabledReason !== null && !current ? 'true' : undefined}
      title={disabledReason ?? undefined}
      onClick={() => {
        if (disabledReason === null || current) onPress();
      }}
    >
      {children}
    </button>
  );
}

// ---- the transcript

function Feed({
  view,
  row,
  known,
  onOpenRun,
  onOpenFile,
}: {
  view: ClaudeView;
  row: SessionRow | null;
  known: Known;
  onOpenRun: (id: string) => void;
  onOpenFile: (path: string) => void;
}) {
  const end = useRef<HTMLLIElement | null>(null);
  const last = view.feed[view.feed.length - 1];
  useEffect(() => {
    end.current?.scrollIntoView({ block: 'end' });
  }, [view.feed.length, last?.text.length]);
  const title =
    view.selected === null ? 'New chat' : `Transcript of ${row?.name ?? row?.title ?? `session ${shortId(view.selected)}`}`;
  return (
    <div className="cc-feed-wrap">
      {row && row.readOnly && row.open !== 'studio' && (
        <p className="cc-mirror" role="note" data-words="exception">
          <span className="cc-mirror-word">Mirror</span> {row.readOnly}
        </p>
      )}
      {/* F-816 (axe listitem): the log role is on a box of its own, so the entries stay items of a list */}
      <div className="cc-feed-log" role="log" aria-label={title}>
        <ol className="cc-feed">
          {/* G22: an empty transcript is the normal start: the composer's placeholder carries it, the rest is said to readers */}
          {view.feed.length === 0 && (
            <li className="cc-feed-empty sr-only">
              {view.selected === null
                ? 'Ask about this project: its cases, runs and results. Claude Code works in the project folder with the project tools below.'
                : 'Nothing in this transcript yet.'}
            </li>
          )}
          {view.feed.map((i) => (
            <FeedEntry key={i.key} item={i} known={known} onOpenRun={onOpenRun} onOpenFile={onOpenFile} />
          ))}
          <li ref={end} className="cc-feed-end" aria-hidden="true" />
        </ol>
      </div>
      {view.feedError && (
        <p className="cc-line cc-line--fail" data-words="exception">
          {view.feedError}
        </p>
      )}
    </div>
  );
}

function FeedEntry({ item, known, onOpenRun, onOpenFile }: { item: FeedItem; known: Known; onOpenRun: (id: string) => void; onOpenFile: (path: string) => void }) {
  switch (item.kind) {
    case 'user':
      return (
        <li className="cc-msg cc-msg--you" data-kind="user" data-words="data">
          <div className="cc-who">You</div>
          <Text text={item.text} />
        </li>
      );
    case 'assistant': {
      const refs = findRefs(item.text, known);
      return (
        <li className="cc-msg cc-msg--claude" data-kind="assistant" data-provisional={item.provisional ? 'true' : undefined} data-words="data">
          <div className="cc-who">Claude Code</div>
          <Text text={item.text} />
          {item.tools.length > 0 && <p className="cc-tools">Used {item.tools.join(', ')}</p>}
          {refs.length > 0 && <Refs refs={refs} onOpenRun={onOpenRun} onOpenFile={onOpenFile} />}
        </li>
      );
    }
    case 'tool':
      return (
        <li className="cc-event" data-kind="tool" data-words="data">
          <span className="cc-event-word">Tool</span> {item.tools.join(', ')}
          {item.text && item.text !== '{}' && item.text !== 'null' ? <span className="cc-event-detail"> {item.text}</span> : null}
        </li>
      );
    case 'tool_result':
      return (
        <li className="cc-event" data-kind="tool_result" data-words="data">
          <span className="cc-event-word">Result</span> <span className="cc-event-detail">{item.text}</span>
        </li>
      );
    case 'denied':
      return (
        <li className="cc-event cc-event--denied" data-kind="denied" data-words="exception">
          <span className="cc-event-word">Not allowed</span> {item.text}
          <span className="cc-event-detail">
            {item.suggestion
              ? ` The allow-list rule ${item.suggestion} would allow it. The Studio adds no rules; to allow it, add the rule to the project’s .rcfd/claude-allow.json and confirm the change here.`
              : ' No narrow rule would allow exactly this call, so the Studio suggests none.'}
          </span>
        </li>
      );
    case 'note':
      return (
        <li className="cc-event" data-kind="note" data-words="data">
          {item.text}
        </li>
      );
    case 'unrecognised':
      return (
        <li className="cc-event" data-kind="unrecognised" data-words="data">
          {item.text}
        </li>
      );
  }
}

function Text({ text }: { text: string }) {
  return (
    <>
      {paragraphs(text).map((p, k) => (
        <p key={k} className="cc-p">
          {p.map((run, j) => (run.bold ? <b key={j}>{run.text}</b> : <span key={j}>{run.text}</span>))}
        </p>
      ))}
    </>
  );
}

function Refs({ refs, onOpenRun, onOpenFile }: { refs: readonly Ref[]; onOpenRun: (id: string) => void; onOpenFile: (path: string) => void }) {
  return (
    <div className="cc-refs" role="group" aria-label="References in this reply">
      {refs.map((r) =>
        r.kind === 'run' ? (
          <Button key={`run:${r.id}`} variant="chip" onPress={() => onOpenRun(r.id)} aria-label={`Open run ${r.id} in Runs`}>
            {r.id}
          </Button>
        ) : (
          <Button key={`file:${r.path}`} variant="chip" onPress={() => onOpenFile(r.path)} aria-label={`Show ${r.path} on the project home`}>
            {r.path}
          </Button>
        ),
      )}
    </div>
  );
}

// ---- the folder's control files

function Trust({ view, onError }: { view: ClaudeView; onError: (e: ClaudeError | null) => void }) {
  const t = view.trust;
  if (t.state === 'confirmed') return null;
  if (t.state === 'error') {
    return (
      <div className="cc-notice cc-notice--failed" role="alert" data-words="exception">
        <p className="cc-notice-title">
          <StatePictogram state="failed" />
          Failed to check the folder
        </p>
        <p>{t.message}</p>
      </div>
    );
  }
  return (
    <section className="cc-notice" aria-labelledby="cc-trust-title" data-words="exception">
      <p id="cc-trust-title" className="cc-notice-title">
        Confirm the Claude Code files of this folder
      </p>
      <p>
        These files change what Claude Code runs or reads here (hooks, instructions, MCP servers, the allow-list). No chat starts until you have
        looked at them and confirmed them; a change asks again. Changed since last confirmed: {t.changed.join(', ')}.
      </p>
      <ul className="cc-files" aria-label="Claude Code control files">
        {t.files.map((f) => (
          <li key={f.path}>
            <span className="cc-file">{f.path}</span>
            <span className="cc-file-meta">
              {f.kind === 'link' ? 'link, not followed' : `${f.bytes} bytes`} · {f.hash.slice(0, 19)}
            </span>
          </li>
        ))}
      </ul>
      <Button
        onPress={() => {
          onError(null);
          void claudeActions.confirm(t.token).then((r) => onError(r.ok ? null : r.error));
        }}
      >
        Confirm these {t.files.length} files
      </Button>
    </section>
  );
}

// ---- tool approvals

function Approvals({ view }: { view: ClaudeView }) {
  const [open, setOpen] = useState(false);
  const a = view.allow;
  return (
    <section className="cc-approvals" aria-labelledby="cc-approvals-title">
      <div className="cc-section-head">
        <h3 id="cc-approvals-title" className="cc-section-title">
          Tool approvals
        </h3>
        {/* G22: the rule (everything else is denied) is said to readers and in the tooltip; a refused list speaks */}
        <Button
          variant="ghost"
          className="cc-disclose"
          aria-expanded={open}
          onPress={() => setOpen(!open)}
          aria-label={a.error ? 'Allow-list refused' : `${a.rules.length} allowed, everything else denied`}
        >
          {a.error ? (
            <span data-words="exception">Allow-list refused</span>
          ) : (
            <span data-words="data" title={`${a.rules.length} allowed, everything else denied`}>
              {a.rules.length}
            </span>
          )}
        </Button>
      </div>
      {a.error && (
        <p className="cc-line cc-line--fail" role="alert" data-words="exception">
          <StatePictogram state="failed" /> Failed: {a.error}
        </p>
      )}
      {open && (
        <div className="cc-allow">
          <p className="cc-line">
            {a.source === 'file' ? `From ${a.file} in this folder.` : `The Studio’s first list: read-only project tools (${a.file} is not in this folder).`} Claude Code runs
            with permission mode dontAsk: a tool call no rule allows is denied, never asked and never approved.{' '}
            {view.mcp.server ? `Project tools come from ${view.mcp.server}.` : (view.mcp.note ?? '')}
          </p>
          <ul className="cc-rules" aria-label="Allowed tools" data-words="data">
            {a.rules.map((r) => (
              <li key={r}>{r}</li>
            ))}
          </ul>
          <p className="cc-line">{FLOOR_CAVEAT}</p>
        </div>
      )}
    </section>
  );
}

// ---- the last turn

const TURN_WORD: Record<TurnView['status'], string> = {
  starting: 'Checking the session…',
  running: 'Claude Code is answering',
  finished: 'Answered',
  failed: 'Failed',
  aborted: 'Failed: stopped by the Studio',
  cancelled: 'Stopped',
  refused: 'Not sent',
};

function TurnLine({ turn, error, version }: { turn: TurnView | null; error: ClaudeError | null; version: string | null }) {
  const failed = turn && (turn.status === 'failed' || turn.status === 'aborted');
  return (
    <div className="cc-turn" role="status">
      {error && (
        <p className="cc-line cc-line--note" data-words="exception">
          Not done: {error.message}
        </p>
      )}
      {turn && !(error && turn.status === 'refused') && (
        <p className={`cc-line${failed ? ' cc-line--fail' : ''}`} data-turn={turn.status} data-words={failed || turn.status === 'refused' ? 'exception' : 'chrome'}>
          {failed && <StatePictogram state="failed" />}
          {TURN_WORD[turn.status]}
          {turn.status === 'running' && turn.pid !== null ? ` (process ${turn.pid})` : ''}
          {turn.costUsd !== null && turn.status === 'finished' ? ` · cost ${turn.costUsd.toFixed(4)} USD` : ''}
          {turn.message && turn.status !== 'finished' ? `: ${turn.message}` : ''}
        </p>
      )}
      {turn && turn.status === 'finished' && (turn.model !== null || turn.sessionCostUsd !== null) && (
        // F-837: the model the CLI said it used, and what this session's turns from this Studio cost
        <p className="cc-line cc-line--meta" data-turn-meta="">
          {turn.model !== null && (
            <span data-words="data" data-model={turn.model} title="The model Claude Code reported for this turn">
              {turn.model}
            </span>
          )}
          {turn.sessionCostUsd !== null && (
            <span data-session-cost={turn.sessionCostUsd} title="The costs Claude Code reported for this session’s turns sent from this Studio, summed">
              {turn.model !== null ? ' · ' : ''}
              {/* G22: the sum sign, not a word; its words are for readers and in the tooltip */}
              <span aria-hidden="true">∑ </span>
              <span className="sr-only">cost of this session: </span>
              <span data-words="data">{turn.sessionCostUsd.toFixed(4)} USD</span>
            </span>
          )}
        </p>
      )}
      {version && (
        <p className="cc-line cc-line--note" data-words="exception">
          {version}
        </p>
      )}
    </div>
  );
}

// ---- typing

function Composer({ state, view, row, onError }: { state: ClaudeState; view: ClaudeView; row: SessionRow | null; onError: (e: ClaudeError | null) => void }) {
  const [draft, setDraft] = useState('');
  const running = view.writer !== null;
  const why = !state.program.configured
    ? 'Claude Code is not connected (see above).'
    : row?.readOnly && row.open !== 'studio'
      ? row.readOnly
      : view.trust.state !== 'confirmed'
        ? 'Confirm the folder’s Claude Code files first.'
        : view.allow.error
          ? 'The allow-list file is refused (see Tool approvals).'
          : null;
  const send = () => {
    const text = draft;
    if (text.trim() === '' || running || why) return;
    onError(null);
    void claudeActions.send(view.selected, text).then((r) => {
      if (r.ok) setDraft('');
      else onError(r.error);
    });
  };
  const stop = () => void claudeActions.cancel().then((r) => onError(r.ok ? null : r.error));
  const onKey = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey && !e.nativeEvent.isComposing) {
      e.preventDefault();
      send();
    } else if (e.key === 'Escape' && running) {
      e.preventDefault();
      stop();
    }
  };
  if (why && !running) {
    return (
      <div className="cc-compose cc-compose--off" data-words="exception">
        <p className="cc-line">
          <span className="cc-mirror-word">Read-only</span> {why}
        </p>
      </div>
    );
  }
  const target = view.selected === null ? 'a new session' : (row?.name ?? row?.title ?? `session ${shortId(view.selected)}`);
  return (
    <form
      className="cc-compose"
      onSubmit={(e) => {
        e.preventDefault();
        send();
      }}
    >
      <TextField className="cc-field" aria-label={`Message to Claude Code, ${target}`} value={draft} onChange={setDraft}>
        <TextArea className="ui-input cc-input" placeholder="Ask about this project" rows={3} onKeyDown={onKey} aria-describedby="cc-keys" />
      </TextField>
      <div className="cc-compose-actions">
        {/* G22: the keys are said to readers (the field's description) and listed in the palette's shortcuts */}
        <span className="cc-hint sr-only" id="cc-keys">
          Enter sends, Shift+Enter starts a new line{running ? ', Escape stops' : ''}
        </span>
        {running ? (
          <Button onPress={stop}>Stop</Button>
        ) : draft.trim() === '' ? (
          <Unavailable label="Send" reason="Type a message first." />
        ) : (
          <Button type="submit" variant="primary" icon="send">
            Send
          </Button>
        )}
      </div>
    </form>
  );
}
