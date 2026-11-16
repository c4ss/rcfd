// One case key as a row (F-825): its name with its unit, the engine's description, a control made from its schema node,
// its default and range, whether the case set it, and the engine's message when a change was refused. The control kind
// comes from the schema (schema.ts): a switch for a boolean, a select for an enumeration, a text field for the rest.
import { useId, useState, type KeyboardEvent } from 'react';
import { Button, Select, StatePictogram, Switch, TextField } from '../ui/index.js';
import { useSettings } from './context.js';
import { encode, formatValue, isSet, rangeText, unitOf, valueAt, type Leaf } from './schema.js';
import '../history/history.css';

export interface KeyRowProps {
  readonly leaf: Leaf;
  /** The concrete path (`boundaries.lid.velocity`). */
  readonly path: string;
  /** The visible name; default the key's own name (a search result shows its whole path). */
  readonly label?: string;
}

const TEXT_HINT: Partial<Record<Leaf['control'], string>> = {
  list: 'Values separated by commas, or a JSON list.',
  json: 'A JSON value.',
};

export function KeyRow({ leaf, path, label }: KeyRowProps) {
  const c = useSettings();
  const value = valueAt(c.tree, path);
  const shown = value ?? leaf.default;
  const set = isSet(leaf, value);
  const error = c.errors.get(path) ?? null;
  const pending = c.pending.has(path);
  const unit = unitOf(leaf);
  const range = rangeText(leaf.control === 'list' && leaf.item ? leaf.item : leaf);
  const descId = useId();
  const errId = useId();
  const accessibleName = unit ? `${path}, ${unit}` : path;
  const name = label ?? (leaf.name === '*' ? (path.split('.').pop() ?? path) : leaf.name);
  const dimension = leaf.siDimension ?? leaf.item?.siDimension;
  const describedBy = [descId, error ? errId : null].filter(Boolean).join(' ');

  let control;
  if (leaf.control === 'boolean') {
    control = (
      <Switch
        isSelected={shown === true}
        isReadOnly={!c.editable}
        onChange={(on) => void c.change(path, on)}
        aria-describedby={describedBy}
      >
        <span className="sr-only">{accessibleName}</span>
        <span aria-hidden="true">{shown === true ? 'true' : shown === false ? 'false' : formatValue(shown)}</span>
      </Switch>
    );
  } else if (leaf.control === 'enum' && leaf.options) {
    const options = leaf.options.map((o) => ({ id: o, label: o }));
    if (typeof shown === 'string' && !leaf.options.includes(shown)) options.push({ id: shown, label: shown });
    control = (
      <Select
        aria-label={accessibleName}
        options={options}
        selected={typeof shown === 'string' ? shown : null}
        onChange={(id) => {
          if (id !== shown) void c.change(path, id);
        }}
        isDisabled={!c.editable}
        placeholder="Choose a value"
      />
    );
  } else {
    control = <TextControl leaf={leaf} path={path} shown={formatValue(shown)} name={accessibleName} describedBy={describedBy} invalid={error !== null} />;
  }

  return (
    <div className="st-key" data-path={path} data-pattern={leaf.pattern} data-control={leaf.control} data-set={set ? 'true' : 'false'} aria-busy={pending}>
      <div className="st-key-head">
        <span className="st-key-name">
          {name}
          {unit ? `, ${unit}` : ''}
        </span>
        {label === undefined && <span className="st-key-path">{path}</span>}
        <p className="st-key-desc" id={descId}>
          {leaf.description || 'The schema gives no description of this key.'}
          {dimension && <span className="st-key-si">Stored in SI ({dimension}); a number with a unit is converted by the engine.</span>}
        </p>
      </div>
      <div className="st-key-control">
        {control}
        {TEXT_HINT[leaf.control] && <span className="st-key-hint">{TEXT_HINT[leaf.control]}</span>}
        {error && (
          <span className="st-key-error" id={errId}>
            <StatePictogram state="failed" size={12} />
            <span>Change failed. {error}</span>
          </span>
        )}
      </div>
      <div className="st-key-meta">
        <span>{leaf.default !== undefined ? `Default ${formatValue(leaf.default) || '(empty)'}` : 'No default in the schema'}</span>
        {range && <span>Range {range}</span>}
        {leaf.control === 'enum' && leaf.options && <span>{leaf.options.length} values</span>}
        {pending && <span>Sending to the engine</span>}
        {set && (
          <span className="st-set">
            Set
            {leaf.default !== undefined && (
              <Button isDisabled={!c.editable || pending} onPress={() => void c.change(path, leaf.default!)} aria-label={`Reset ${path} to its default`}>
                Reset
              </Button>
            )}
          </span>
        )}
      </div>
    </div>
  );
}

interface TextControlProps {
  readonly leaf: Leaf;
  readonly path: string;
  /** The engine's value as text. */
  readonly shown: string;
  readonly name: string;
  readonly describedBy: string;
  readonly invalid: boolean;
}

/**
 * A text field that sends on Enter or when focus leaves, and goes back to the engine's value on Escape. Before it sends,
 * it reads the engine's value again: when another writer changed it after the typing began, nothing is sent and the
 * field asks which to keep (F-829, G18).
 */
function TextControl({ leaf, path, shown, name, describedBy, invalid }: TextControlProps) {
  const c = useSettings();
  const [draft, setDraft] = useState<string | null>(null);
  const [refused, setRefused] = useState<string | null>(null);
  /** The engine's value as text when the typing began. */
  const [basis, setBasis] = useState<string | null>(null);
  /** The engine's value now, when another writer changed it after the typing began. */
  const [overtaken, setOvertaken] = useState<string | null>(null);
  const text = draft ?? shown;

  const send = (sent: string) => {
    void c.change(path, encode(leaf, sent)).then((ok) => {
      if (ok) {
        setDraft((d) => (d === sent ? null : d));
        setRefused(null);
        setBasis(null);
      } else {
        setRefused(sent);
      }
    });
  };

  const commit = (force: boolean) => {
    if (draft === null) return;
    if (draft === shown && !invalid) {
      setDraft(null);
      setBasis(null);
      return;
    }
    if (!force && (draft === refused || overtaken !== null)) return;
    const sent = draft;
    if (basis === null || !c.current) return send(sent);
    const began = basis;
    void c.current(path).then((now) => {
      const nowText = now.ok ? formatValue(now.value ?? leaf.default) : null;
      if (nowText !== null && nowText !== began) setOvertaken(nowText);
      else send(sent);
    });
  };

  const keepTheirs = () => {
    setDraft(null);
    setRefused(null);
    setBasis(null);
    setOvertaken(null);
    c.clearError(path);
  };

  const onKeyDown = (e: KeyboardEvent) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      commit(true);
    } else if (e.key === 'Escape' && (draft !== null || invalid)) {
      e.preventDefault();
      keepTheirs();
    }
  };

  return (
    <>
      <TextField
        aria-label={name}
        aria-describedby={describedBy}
        value={text}
        onChange={(t) => {
          if (draft === null) setBasis(shown);
          setDraft(t);
        }}
        onKeyDown={onKeyDown}
        onBlur={() => commit(false)}
        isReadOnly={!c.editable}
        isInvalid={invalid}
        spellCheck="false"
        autoComplete="off"
      />
      {overtaken !== null && draft !== null && (
        <div className="hi-overtaken" role="alert">
          <p>
            Not sent: {path} changed outside the Studio while you typed. It is now {overtaken || 'empty'} (it was {basis || 'empty'} when
            you began).
          </p>
          <div className="hi-choices">
            <Button
              onPress={() => {
                setBasis(overtaken);
                setOvertaken(null);
                send(draft);
              }}
            >
              Set mine: {draft || 'empty'}
            </Button>
            <Button onPress={keepTheirs}>Keep {overtaken || 'empty'}</Button>
          </div>
        </div>
      )}
    </>
  );
}
