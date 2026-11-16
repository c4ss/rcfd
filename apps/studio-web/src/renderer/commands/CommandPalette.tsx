// The command palette (F-831; G20, G8, G16, G18, G19): every action of the registries and every Studio command, found by
// its name or its id, with its shortcut and its CLI and Claude Code equivalents. Loaded on first use by ./PaletteHost.
//
// Steps, all inside one modal dialog (React Aria: the focus stays inside while it is open and goes back where it was
// when it closes; Escape closes it from any step):
//   list       the search field and the commands; an unavailable one is listed with its reason, never hidden
//   pick       a value a command needs, chosen from what the engine or the main process lists (a session, a case, a
//              run, a folder): never made up
//   form       the other parameters, as the registry describes them (left empty: the engine's default)
//   confirm    the words of an action that loses something, with the safe choice focused (G18)
//   result     the engine's answer or its refusal in words, and the same request from the CLI and Claude Code
//   detail     why a command cannot run now, and what to do
//   shortcuts  the one shortcut table (src/shared/commands/shortcuts.ts), by where the keys work
import { useEffect, useMemo, useRef, useState, type FormEvent, type ReactNode } from 'react';
import {
  Autocomplete,
  Dialog,
  Header,
  Heading,
  Input,
  ListBox,
  ListBoxItem,
  ListBoxSection,
  Modal,
  ModalOverlay,
  Text,
  TextField as AriaTextField,
  type Key,
} from 'react-aria-components';
import type { ClaudeState } from '../../shared/claude.js';
import type { ProjectState } from '../../shared/project.js';
import type { ServiceState } from '../../shared/service.js';
import type { Confirmation } from '../../shared/actions/runs.js';
import {
  chordLabel,
  GROUP_ORDER,
  paletteModel,
  SCOPE_WORDS,
  SCOPES,
  searchPalette,
  SHORTCUTS,
  type PaletteEntry,
  type PaletteParam,
} from '../../shared/commands/index.js';
import { Button, StatePictogram, TextField } from '../ui/index.js';
import { watchServiceState } from '../actions/index.js';
import { watchProjectState } from '../project/door.js';
import { watchClaudeState } from '../claude/door.js';
import {
  choicesFor,
  confirmationFor,
  execute,
  followRun,
  isChoice,
  unavailableReason,
  valuesReason,
  type Choices,
  type Outcome,
  type PaletteEnv,
  type Values,
} from './run.js';
import type { PaletteRequest } from './PaletteHost.js';
import './palette.css';

type Done = Extract<Outcome, { kind: 'done' }>;

type Step =
  | { readonly kind: 'list' }
  | { readonly kind: 'shortcuts' }
  | { readonly kind: 'detail'; readonly entry: PaletteEntry; readonly reason: string }
  | { readonly kind: 'pick'; readonly entry: PaletteEntry; readonly param: PaletteParam; readonly values: Values; readonly choices: Choices | null }
  | { readonly kind: 'form'; readonly entry: PaletteEntry; readonly values: Values }
  | { readonly kind: 'confirm'; readonly entry: PaletteEntry; readonly values: Values; readonly confirmation: Confirmation }
  | { readonly kind: 'busy'; readonly entry: PaletteEntry }
  | { readonly kind: 'result'; readonly entry: PaletteEntry; readonly outcome: Done };

function useEnv(): PaletteEnv {
  const [project, setProject] = useState<ProjectState | null>(null);
  const [claude, setClaude] = useState<ClaudeState | null>(null);
  const [service, setService] = useState<ServiceState | null>(null);
  useEffect(() => watchProjectState(setProject), []);
  useEffect(() => watchClaudeState(setClaude), []);
  useEffect(() => watchServiceState(setService), []);
  return useMemo(() => ({ project, claude, service }), [project, claude, service]);
}

/** Runs `f` once the closing dialog has given the focus back, so a command that moves the focus has the last word. */
function afterFocusReturns(f: () => void): void {
  requestAnimationFrame(() => requestAnimationFrame(() => setTimeout(f, 0)));
}

/** The parameters the form asks for: all but the required choices (asked one by one before it). */
const formParams = (e: PaletteEntry): PaletteParam[] => e.params.filter((p) => !(isChoice(p) && !p.optional));

const PARAM_WORD: Readonly<Record<string, string>> = {
  session: 'a session',
  case: 'a case file',
  run: 'a run',
  recent: 'a folder',
  'claude-session': 'a Claude Code session',
};

export default function CommandPalette({ request }: { request: PaletteRequest }) {
  const env = useEnv();
  const envRef = useRef(env);
  envRef.current = env;
  const model = useMemo(() => paletteModel(), []);
  const byId = useMemo(() => new Map(model.map((e) => [e.id, e])), [model]);
  const [open, setOpen] = useState(false);
  const openRef = useRef(false);
  openRef.current = open;
  const [step, setStep] = useState<Step>({ kind: 'list' });
  const [query, setQuery] = useState('');

  // the window's state arrives a moment after the first mount: a shortcut waits for it (or 400 ms) before it is read
  const [waited, setWaited] = useState(false);
  useEffect(() => {
    const t = setTimeout(() => setWaited(true), 400);
    return () => clearTimeout(t);
  }, []);
  const ready = env.project !== null || waited;

  const show = (s: Step) => {
    setStep(s);
    setOpen(true);
  };
  const toList = () => {
    setQuery('');
    show({ kind: 'list' });
  };
  const closeThen = (after?: () => void | Promise<unknown>) => {
    setOpen(false);
    if (after) afterFocusReturns(() => void after());
  };

  const run = async (e: PaletteEntry, values: Values) => {
    if (openRef.current) setStep({ kind: 'busy', entry: e });
    const o = await execute(e, values, envRef.current);
    if (o.kind === 'close') closeThen(o.after);
    else show({ kind: 'result', entry: e, outcome: o });
  };

  /** The next thing a command needs: a choice, the form, a confirmation, then the run. */
  const next = async (e: PaletteEntry, values: Values, formDone = false) => {
    const p = e.params.find((x) => isChoice(x) && !x.optional && values[x.name] === undefined);
    if (p) {
      setQuery('');
      show({ kind: 'pick', entry: e, param: p, values, choices: null });
      const choices = await choicesFor(e, p, values, envRef.current);
      setStep((s) => (s.kind === 'pick' && s.entry === e && s.param === p ? { ...s, choices } : s));
      return;
    }
    if (!formDone && formParams(e).length > 0) return show({ kind: 'form', entry: e, values });
    const why = valuesReason(e, values, envRef.current);
    if (why) return show({ kind: 'detail', entry: e, reason: why });
    const confirmation = confirmationFor(e, values, envRef.current);
    if (confirmation) return show({ kind: 'confirm', entry: e, values, confirmation });
    await run(e, values);
  };

  const start = (e: PaletteEntry) => {
    if (e.id === 'palette.open') return toList();
    if (e.id === 'help.shortcuts') return show({ kind: 'shortcuts' });
    const reason = unavailableReason(e, envRef.current);
    if (reason) return show({ kind: 'detail', entry: e, reason });
    void next(e, {});
  };

  const handled = useRef(0);
  useEffect(() => {
    if (!ready || handled.current === request.n) return;
    handled.current = request.n;
    const e = byId.get(request.id);
    if (e) start(e);
  }, [request, ready]);

  let body: ReactNode;
  switch (step.kind) {
    case 'list':
      body = <CommandList model={model} env={env} query={query} setQuery={setQuery} onRun={(id) => start(byId.get(id)!)} />;
      break;
    case 'shortcuts':
      body = <ShortcutList onBack={toList} />;
      break;
    case 'detail':
      body = (
        <>
          <StepHead title={step.entry.title} sub={step.entry.id} />
          <div className="cp-body">
            <p className="cp-reason">
              <span className="cp-unavailable-word">Unavailable.</span> {step.reason}
            </p>
            <Facts entry={step.entry} />
          </div>
          <div className="cp-actions">
            <Button variant="primary" autoFocus onPress={toList}>
              Back to commands
            </Button>
          </div>
        </>
      );
      break;
    case 'pick':
      body = (
        <ChoiceList
          step={step}
          query={query}
          setQuery={setQuery}
          onChoose={(v) => void next(step.entry, { ...step.values, [step.param.name]: v })}
          onBack={toList}
        />
      );
      break;
    case 'form':
      body = <ParamForm entry={step.entry} values={step.values} onSubmit={(v) => void next(step.entry, v, true)} onBack={toList} />;
      break;
    case 'confirm':
      body = (
        <>
          <StepHead title={step.confirmation.title} sub={step.entry.id} />
          <div className="cp-body">
            <p className="cp-confirm-body">{step.confirmation.body}</p>
          </div>
          <div className="cp-actions">
            <Button variant="primary" autoFocus onPress={() => closeThen()}>
              {step.confirmation.keep}
            </Button>
            <Button onPress={() => void run(step.entry, step.values)}>{step.confirmation.confirm}</Button>
          </div>
        </>
      );
      break;
    case 'busy':
      body = (
        <>
          <StepHead title={step.entry.title} sub={step.entry.id} />
          <div className="cp-body">
            <p role="status">Asking the engine…</p>
          </div>
        </>
      );
      break;
    case 'result': {
      const o = step.outcome;
      body = (
        <>
          <StepHead title={o.title} sub={step.entry.id} failed={!o.ok} />
          <div className="cp-body" role={o.ok ? 'status' : 'alert'}>
            {o.lines.map((l, i) => (
              <p key={i}>{l}</p>
            ))}
            {o.reply !== null && (
              <pre className="cp-reply" tabIndex={0} aria-label="The engine’s reply">
                {o.reply}
              </pre>
            )}
            {o.equivalents.length > 0 && (
              <ul className="cp-equivalents" aria-label="The same request elsewhere">
                {o.equivalents.map((l) => (
                  <li key={l}>{l}</li>
                ))}
              </ul>
            )}
          </div>
          <div className="cp-actions">
            <Button variant="primary" autoFocus onPress={() => closeThen()}>
              Done
            </Button>
            {o.run && (
              <Button onPress={() => closeThen(() => followRun(o.run!))}>
                Show run {o.run} in Runs
              </Button>
            )}
            <Button onPress={toList}>Back to commands</Button>
          </div>
        </>
      );
      break;
    }
  }

  return (
    <ModalOverlay
      isOpen={open}
      onOpenChange={(o) => {
        if (!o) setOpen(false);
      }}
      isDismissable
      className="cp-overlay"
    >
      <Modal className="cp-modal">
        <Dialog className="cp-dialog" aria-label="Command palette">
          {body}
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}

// ---- parts

function StepHead({ title, sub, failed }: { title: string; sub?: string; failed?: boolean }) {
  return (
    <div className="cp-head">
      <Heading slot="title" className="cp-title">
        {failed && <StatePictogram state="failed" />}
        {title}
      </Heading>
      {sub && <span className="cp-sub">{sub}</span>}
    </div>
  );
}

/** A chord as key caps: "Ctrl+Shift+P" is Ctrl, Shift, P. */
export function Keys({ keys }: { keys: string }) {
  const parts = keys === '+' ? ['+'] : keys.endsWith('++') ? [...keys.slice(0, -2).split('+'), '+'] : keys.split('+');
  return (
    <span className="cp-keys" aria-label={keys}>
      {parts.map((k, i) => (
        <kbd key={i} className="cp-kbd" aria-hidden="true">
          {chordLabel(k)}
        </kbd>
      ))}
    </span>
  );
}

/** What the palette knows of a command: its words, its id, its shortcuts and its equivalents. */
function Facts({ entry }: { entry: PaletteEntry }) {
  return (
    <dl className="cp-facts">
      <dt>What it does</dt>
      <dd>{entry.description}</dd>
      <dt>Id</dt>
      <dd>{entry.id}</dd>
      {entry.shortcuts.length > 0 && (
        <>
          <dt>Shortcut</dt>
          <dd>
            {entry.shortcuts.map((s) => (
              <span key={`${s.scope}:${s.keys}`} className="cp-fact-key">
                <Keys keys={s.keys} /> {s.scope === 'global' ? '' : SCOPE_WORDS[s.scope].toLowerCase()}
              </span>
            ))}
          </dd>
        </>
      )}
      <dt>Engine</dt>
      <dd>{entry.engine ?? 'none: a view of the Studio'}</dd>
      <dt>Command line</dt>
      <dd>{entry.cli ?? 'none'}</dd>
      <dt>Claude Code</dt>
      <dd>{entry.mcp ?? 'none'}</dd>
      {entry.note && (
        <>
          <dt>Note</dt>
          <dd>{entry.note}</dd>
        </>
      )}
    </dl>
  );
}

/** Tells the palette which option has the (virtual) focus, for the facts under the list. */
function FocusReport({ focused, id, report }: { focused: boolean; id: string; report: (id: string) => void }) {
  useEffect(() => {
    if (focused) report(id);
  }, [focused, id, report]);
  return null;
}

function CommandList({
  model,
  env,
  query,
  setQuery,
  onRun,
}: {
  model: readonly PaletteEntry[];
  env: PaletteEnv;
  query: string;
  setQuery: (q: string) => void;
  onRun: (id: string) => void;
}) {
  const found = useMemo(() => searchPalette(model, query), [model, query]);
  const [focused, setFocused] = useState<string | null>(null);
  const focusedEntry = found.find((e) => e.id === focused) ?? found[0] ?? null;
  const reasons = new Map(found.map((e) => [e.id, unavailableReason(e, env)]));
  const row = (e: PaletteEntry) => {
    const reason = reasons.get(e.id) ?? null;
    return (
      <ListBoxItem key={e.id} id={e.id} textValue={e.title} className={reason ? 'cp-item cp-item--unavailable' : 'cp-item'}>
        {({ isFocused }) => (
          <>
            <FocusReport focused={isFocused} id={e.id} report={setFocused} />
            <span className="cp-item-line">
              <Text slot="label" className="cp-item-title">
                {e.title}
              </Text>
              <span className="cp-item-id">{e.id}</span>
              <span className="cp-item-side">
                {query !== '' && <span className="cp-item-group">{e.group}</span>}
                {e.shortcut && <Keys keys={e.shortcut} />}
              </span>
            </span>
            <Text slot="description" className="cp-item-reason">
              {reason ? (
                <>
                  <span className="cp-unavailable-word">Unavailable:</span> {reason}
                </>
              ) : (
                <span className="cp-visually-hidden">
                  {e.id}
                  {e.shortcut ? `, ${e.shortcut}` : ''}
                </span>
              )}
            </Text>
          </>
        )}
      </ListBoxItem>
    );
  };
  return (
    <>
      <StepHead title="Commands" sub={`${model.length} commands; ↑ ↓ to move, Enter to run, Esc to close`} />
      <Autocomplete inputValue={query} onInputChange={setQuery}>
        <AriaTextField aria-label="Find a command by its name or its id" className="cp-search" autoFocus>
          <Input className="ui-input cp-input" placeholder="Type a command or an id (Run the case, run.start)" />
        </AriaTextField>
        <ListBox
          aria-label="Commands"
          className="cp-list"
          onAction={(k: Key) => onRun(String(k))}
          renderEmptyState={() => <p className="cp-empty">No command matches “{query}”. Try a word of its name or of its id.</p>}
        >
          {query === ''
            ? GROUP_ORDER.map((g) => {
                const items = found.filter((e) => e.group === g);
                return items.length === 0 ? null : (
                  <ListBoxSection key={g} id={g} className="cp-section">
                    <Header className="cp-group">{g}</Header>
                    {items.map(row)}
                  </ListBoxSection>
                );
              })
            : found.map(row)}
        </ListBox>
      </Autocomplete>
      <div className="cp-detail" aria-live="polite">
        {focusedEntry && (
          <>
            <p className="cp-detail-text">{focusedEntry.description}</p>
            <p className="cp-detail-eq">
              <span>Command line: {focusedEntry.cli ?? 'none'}</span>
              <span>Claude Code: {focusedEntry.mcp ?? 'none'}</span>
            </p>
          </>
        )}
      </div>
      <div className="cp-foot">
        <span>
          <Keys keys="Ctrl+/" /> every shortcut
        </span>
        <span>Unavailable commands stay listed with the reason.</span>
      </div>
    </>
  );
}

function ChoiceList({
  step,
  query,
  setQuery,
  onChoose,
  onBack,
}: {
  step: Extract<Step, { kind: 'pick' }>;
  query: string;
  setQuery: (q: string) => void;
  onChoose: (value: string) => void;
  onBack: () => void;
}) {
  const word = PARAM_WORD[step.param.kind] ?? step.param.name;
  const q = query.trim().toLowerCase();
  const shown = (step.choices?.choices ?? []).filter((c) => q === '' || `${c.label} ${c.detail ?? ''} ${c.value}`.toLowerCase().includes(q));
  return (
    <>
      <StepHead title={step.entry.title} sub={`Choose ${word} (${step.param.name})`} />
      {step.choices === null ? (
        <div className="cp-body">
          <p role="status">Reading the list…</p>
        </div>
      ) : (
        <Autocomplete inputValue={query} onInputChange={setQuery}>
          <AriaTextField aria-label={`Find ${word}`} className="cp-search" autoFocus>
            <Input className="ui-input cp-input" placeholder={`Type to narrow the list of ${step.choices.choices.length}`} />
          </AriaTextField>
          <ListBox
            aria-label={`Choose ${word}`}
            className="cp-list"
            onAction={(k: Key) => onChoose(String(k))}
            renderEmptyState={() => <p className="cp-empty">{step.choices!.choices.length === 0 ? step.choices!.empty : `Nothing matches “${query}”.`}</p>}
          >
            {shown.map((c) => (
              <ListBoxItem key={c.value} id={c.value} textValue={c.label} className={c.reason ? 'cp-item cp-item--unavailable' : 'cp-item'}>
                <span className="cp-item-line">
                  <Text slot="label" className="cp-item-title">
                    {c.label}
                  </Text>
                  {c.detail && <span className="cp-item-id">{c.detail}</span>}
                  {c.mark && <span className="cp-item-side cp-item-mark">{c.mark}</span>}
                </span>
                {c.reason && (
                  <Text slot="description" className="cp-item-reason">
                    <span className="cp-unavailable-word">Unavailable:</span> {c.reason}
                  </Text>
                )}
              </ListBoxItem>
            ))}
          </ListBox>
        </Autocomplete>
      )}
      <p className="cp-param-help">{step.param.description}</p>
      <div className="cp-actions">
        <Button onPress={onBack}>Back to commands</Button>
      </div>
    </>
  );
}

/** A typed value of a parameter from its text; empty and optional is left out (the engine's default applies). */
function parseParam(p: PaletteParam, text: string): { ok: true; value: unknown } | { ok: false; error: string } {
  const t = text.trim();
  if (t === '') return p.optional ? { ok: true, value: undefined } : { ok: false, error: `${p.name} is needed: ${p.description}` };
  switch (p.kind) {
    case 'integer':
    case 'number': {
      const n = Number(t);
      if (!Number.isFinite(n) || (p.kind === 'integer' && !Number.isSafeInteger(n))) return { ok: false, error: `${p.name} must be ${p.kind === 'integer' ? 'a whole number' : 'a number'}.` };
      if (p.minimum !== undefined && n < p.minimum) return { ok: false, error: `${p.name} must be at least ${p.minimum}.` };
      if (p.maximum !== undefined && n > p.maximum) return { ok: false, error: `${p.name} must be at most ${p.maximum}.` };
      return { ok: true, value: n };
    }
    case 'boolean':
      if (t === 'true' || t === 'false') return { ok: true, value: t === 'true' };
      return { ok: false, error: `${p.name} must be true or false.` };
    case 'string-array':
      return { ok: true, value: t.split(/\r?\n/).map((s) => s.trim()).filter(Boolean) };
    case 'object':
    case 'json':
      try {
        const v: unknown = JSON.parse(t);
        if (p.kind === 'object' && (typeof v !== 'object' || v === null || Array.isArray(v))) return { ok: false, error: `${p.name} must be a JSON object ({…}).` };
        return { ok: true, value: v };
      } catch (err) {
        return { ok: false, error: `${p.name} is not JSON: ${String(err).replace(/^SyntaxError: /, '')}` };
      }
    default:
      if (p.enum && !p.enum.includes(t)) return { ok: false, error: `${p.name} must be one of ${p.enum.join(', ')}.` };
      return { ok: true, value: t };
  }
}

const KIND_HINT: Readonly<Record<string, string>> = {
  'string-array': ' One per line.',
  object: ' A JSON object.',
  json: ' Any JSON value (a number, "text", true, [..], {..}).',
};

function ParamForm({ entry, values, onSubmit, onBack }: { entry: PaletteEntry; values: Values; onSubmit: (v: Values) => void; onBack: () => void }) {
  const params = formParams(entry);
  const [draft, setDraft] = useState<Record<string, string>>({});
  const [errors, setErrors] = useState<Record<string, string>>({});
  const chosen = Object.entries(values);
  const submit = (ev?: FormEvent) => {
    ev?.preventDefault();
    const out: Record<string, unknown> = { ...values };
    const errs: Record<string, string> = {};
    for (const p of params) {
      const r = parseParam(p, draft[p.name] ?? '');
      if (!r.ok) errs[p.name] = r.error;
      else if (r.value !== undefined) out[p.name] = r.value;
    }
    setErrors(errs);
    if (Object.keys(errs).length === 0) onSubmit(out);
  };
  return (
    <>
      <StepHead title={entry.title} sub={entry.id} />
      <form className="cp-form" onSubmit={submit} aria-label={`Parameters of ${entry.title}`}>
        {chosen.length > 0 && (
          <p className="cp-chosen">
            {chosen.map(([k, v]) => (
              <span key={k}>
                {k}: <strong>{String(v)}</strong>
              </span>
            ))}
          </p>
        )}
        {params.map((p, i) => (
          <TextField
            key={p.name}
            label={`${p.name}${p.optional ? ' (optional)' : ''}`}
            description={`${p.description}${KIND_HINT[p.kind] ?? ''}${p.optional ? ' Leave it empty for the engine’s default.' : ''}`}
            value={draft[p.name] ?? ''}
            onChange={(t) => setDraft((d) => ({ ...d, [p.name]: t }))}
            isInvalid={errors[p.name] !== undefined}
            errorMessage={errors[p.name]}
            autoFocus={i === 0}
            className="cp-field"
          />
        ))}
        <div className="cp-actions">
          <Button type="submit" variant="primary">
            {entry.title}
          </Button>
          <Button onPress={onBack}>Back to commands</Button>
        </div>
      </form>
    </>
  );
}

function ShortcutList({ onBack }: { onBack: () => void }) {
  return (
    <>
      <StepHead title="Keyboard shortcuts" sub="Ctrl is Cmd on macOS. Kept keys belong to the system or another item." />
      <div className="cp-shortcuts" tabIndex={0} role="region" aria-label="Keyboard shortcuts by where they work">
        {SCOPES.map((scope) => {
          const rows = SHORTCUTS.filter((s) => s.scope === scope);
          if (rows.length === 0) return null;
          return (
            <section key={scope} className="cp-scope" aria-labelledby={`cp-scope-${scope}`}>
              <h3 id={`cp-scope-${scope}`} className="cp-scope-title">
                {SCOPE_WORDS[scope]}
              </h3>
              <table className="cp-table">
                <tbody>
                  {rows.map((s) => (
                    <tr key={`${s.scope}:${s.keys}`} className={s.reserved ? 'cp-row--kept' : undefined}>
                      <th scope="row">
                        <Keys keys={s.keys} />
                      </th>
                      <td>{s.does}</td>
                      <td className="cp-owner">{s.reserved ? `kept: ${s.owner}` : s.owner}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </section>
          );
        })}
      </div>
      <div className="cp-actions">
        <Button variant="primary" autoFocus onPress={onBack}>
          Back to commands
        </Button>
      </div>
    </>
  );
}
