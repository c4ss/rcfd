// Claude Code for the open project (ADR 0801 Addendum 23): the Studio embeds no Claude Code. This module is the page's
// door to it (window.studioClaudeCode, over the project port): "Use with Claude Code" registers the engine's project
// server for this folder with Claude Code itself (or shows the command to run), "Open Claude Code here" opens a terminal
// in the folder running `claude`, and one quiet line in the header names the latest change an MCP client (Claude Code)
// made, as the engine's change feed reports it (the journal line's actor), with a link to what it changed. The view
// never moves on its own: only the link does, when pressed.
import { useEffect, useRef, useState } from 'react';
import type { ClaudeCodeOutcome, ClaudeCodeStatus, ProjectView } from '../../shared/project.js';
import { Button, IconButton } from '../ui/index.js';
import { IconToggle } from '../ui/IconToggle.js';
import { caseHash } from '../case/hash.js';
import { commandHandler } from '../commands/host.js';
import { clock } from './format.js';
import { activityOf, latestByClaude, registrationWords, whyOf, type ActivityTarget } from './activity.js';

export { registrationWords } from './activity.js';

const bridge = () => window.studioClaudeCode;

// ---- the door

const unreachable = (): ClaudeCodeStatus => ({
  claude: { found: false, reason: 'This window has no way to the Studio’s main process.' },
  folder: null,
  command: null,
  registration: 'unknown',
  registeredAs: null,
  detail: null,
  readAt: new Date().toISOString(),
});

export const claudeCodeActions = {
  status: (): Promise<ClaudeCodeStatus> => bridge()?.status() ?? Promise.resolve(unreachable()),
  register: (): Promise<ClaudeCodeStatus> => bridge()?.register() ?? Promise.resolve(unreachable()),
  copy: (): Promise<ClaudeCodeStatus> => bridge()?.copy() ?? Promise.resolve(unreachable()),
  open: (): Promise<ClaudeCodeOutcome> => bridge()?.open() ?? Promise.resolve({ ok: false, message: 'This window has no way to the Studio’s main process.' }),
};

function follow(t: ActivityTarget): void {
  switch (t.kind) {
    case 'run':
      commandHandler('go.runs')?.run({ run: t.run });
      return;
    case 'runs':
      commandHandler('go.runs')?.run({});
      return;
    case 'case':
      location.hash = caseHash.open(t.path);
      return;
    case 'path':
      commandHandler('home.show_path')?.run({ path: t.path });
      return;
  }
}

/** One quiet line: what Claude Code changed last in this project, linked. Nothing while it has changed nothing. */
export function ClaudeActivity({ view }: { view: ProjectView }) {
  const c = latestByClaude(view.changes);
  if (!c) return null;
  const a = activityOf(c);
  // F-273 (ADR 0837 D5): who (the actor), what (verb and thing) and why, the reason only as text (D3)
  const why = whyOf(c.reason);
  return (
    // G22: the line reports the change feed's latest entry, as the Changes list does: data, not chrome
    <span className="ph-activity" role="status" title={c.text} data-words="data" data-actor={c.actor?.client} data-pid={c.actor?.pid ?? undefined}>
      <span className="ph-activity-text">
        Claude Code {a.verb}{' '}
        {a.target && a.thing ? (
          <button type="button" className="ph-activity-link" data-words="data" onClick={() => follow(a.target!)}>
            {a.thing}
          </button>
        ) : a.target ? (
          <button type="button" className="ph-activity-link" onClick={() => follow(a.target!)}>
            Runs
          </button>
        ) : (
          <span data-words="data">{a.thing}</span>
        )}
        <span className="ph-activity-why" data-words="data" data-why={why.given ? 'given' : 'none'} data-reason-cut={why.cut ? 'true' : undefined}>
          ; why: {why.text}
        </span>
      </span>
      <time className="ph-activity-time" dateTime={c.seen} data-words="data">
        {clock(c.seen)}
      </time>
    </span>
  );
}

// ---- the header's two icons and the registration panel

/** "Use with Claude Code" (opens the registration panel) and "Open Claude Code here", icons with no word at rest (G22). */
export function ClaudeCodeIcons({ open, onOpenChange }: { open: boolean; onOpenChange: (open: boolean) => void }) {
  const [said, setSaid] = useState<ClaudeCodeOutcome | null>(null);
  return (
    <>
      <IconToggle icon="claude-register" label="Use with Claude Code" isSelected={open} onChange={onOpenChange} className="ph-claude-toggle" />
      <IconButton variant="ghost" icon="claude-here" label="Open Claude Code here" onPress={() => void claudeCodeActions.open().then(setSaid)} />
      {said && !said.ok && (
        <span className="ph-claude-said" role="alert" data-words="exception">
          {said.message}
          <IconButton variant="ghost" icon="close" label="Dismiss" onPress={() => setSaid(null)} />
        </span>
      )}
      {said?.ok && <span className="sr-only" role="status">{said.message}</span>}
    </>
  );
}

/** The registration of this folder with Claude Code: its state, the command to run, Copy, and Register. */
export function ClaudeCodePanel({ folder, onClose }: { folder: string; onClose: () => void }) {
  const [s, setS] = useState<ClaudeCodeStatus | null>(null);
  const [busy, setBusy] = useState<'status' | 'register' | 'copy' | null>('status');
  const ref = useRef<HTMLElement>(null);
  useEffect(() => {
    let live = true;
    setBusy('status');
    void claudeCodeActions.status().then((r) => {
      if (!live) return;
      setS(r);
      setBusy(null);
    });
    return () => {
      live = false;
    };
  }, [folder]);
  useEffect(() => ref.current?.focus(), []);
  const act = (what: 'register' | 'copy') => {
    setBusy(what);
    void claudeCodeActions[what]().then((r) => {
      setS(r);
      setBusy(null);
      // Register is disabled once it is done: the focus it held comes back to the panel (Escape still closes it)
      requestAnimationFrame(() => {
        const a = document.activeElement;
        if (a === null || a === document.body) ref.current?.focus();
      });
    });
  };
  const state = s?.registration;
  const canRegister = s !== null && s.claude.found && s.command !== null && state !== 'registered' && state !== 'none';
  return (
    <section
      className="ph-claude"
      aria-labelledby="ph-claude-title"
      ref={ref}
      tabIndex={-1}
      onKeyDown={(e) => {
        if (e.key === 'Escape') {
          e.stopPropagation();
          onClose();
        }
      }}
    >
      <div className="ph-claude-head">
        <h2 id="ph-claude-title" className="ph-claude-title">
          Use with Claude Code
        </h2>
        <IconButton variant="ghost" icon="close" label="Close Use with Claude Code" onPress={onClose} />
      </div>
      <p className="ph-claude-state" role="status" data-state={state ?? 'reading'} data-words={state === 'registered' || state === undefined ? undefined : 'exception'}>
        {s === null ? 'Reading the registration…' : registrationWords(s)}
        {s?.registeredAs && (
          <>
            {' '}
            <span data-words="data">{s.registeredAs}</span>
          </>
        )}
      </p>
      {s && !s.claude.found && (
        <p className="ph-claude-missing" data-words="exception">
          {s.claude.reason}
        </p>
      )}
      {s?.command && (
        <div className="ph-claude-command">
          <code data-words="data">{s.command}</code>
          <Button onPress={() => act('copy')} isDisabled={busy !== null}>
            Copy
          </Button>
        </div>
      )}
      <p className="st-small">Run in the project folder. Claude Code asks before each tool.</p>
      <div className="ph-claude-actions">
        <Button variant="primary" onPress={() => act('register')} isDisabled={!canRegister || busy !== null}>
          {busy === 'register' ? 'Registering…' : state === 'other' ? 'Re-register' : 'Register'}
        </Button>
        {s?.detail && s.claude.found && (
          <span className="ph-claude-detail" role="status" data-words="data">
            {s.detail}
          </span>
        )}
      </div>
    </section>
  );
}
