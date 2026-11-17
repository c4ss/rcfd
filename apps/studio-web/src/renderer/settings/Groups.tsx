// The parts of a section (F-825): a group's own keys split by learner-first disclosure (disclosure.ts), the disclosures
// that keep the rest one step away, nested objects, and named collections with their entries.
import { useId, useState, type ReactNode } from 'react';
import { Button as AriaButton } from 'react-aria-components';
import { Button, Icon, StatePictogram, TextField } from '../ui/index.js';
import { useSettings } from './context.js';
import { keyCount, ownKeys, setCount } from './disclosure.js';
import { ConflictNote, KeyRow } from './KeyRow.js';
import { concrete, entryNames, valueAt, type Collection, type Group, type SettingNode } from './schema.js';

const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;

interface DisclosureProps {
  readonly id: string;
  readonly title: string;
  readonly notes?: ReactNode;
  readonly description?: string;
  /** The concrete path the disclosure opens onto (for goTo). */
  readonly path?: string;
  readonly children: ReactNode;
}

/** A ruled row that opens what sits one step away. Its panel is rendered only while open. */
export function Disclosure({ id, title, notes, description, path, children }: DisclosureProps) {
  const c = useSettings();
  const open = c.isOpen(id);
  const panel = useId();
  return (
    <>
      <AriaButton
        className="st-disclosure"
        aria-expanded={open}
        aria-controls={open ? panel : undefined}
        onPress={() => c.toggle(id)}
        data-disclosure={id}
        data-path={path}
      >
        <Icon name="caret-down" />
        <span className="st-disclosure-text">
          <span className="st-disclosure-line">
            <span className="st-disclosure-name" data-words="data">
              {title}
            </span>
            {notes}
          </span>
          {description && <span className="st-disclosure-desc sr-only">{description}</span>}
        </span>
      </AriaButton>
      {open && (
        <div id={panel} role="group" aria-label={title}>
          {children}
        </div>
      )}
    </>
  );
}

function Note({ children, set }: { children: ReactNode; set?: boolean }) {
  return <span className={set ? 'st-disclosure-note st-disclosure-note--set' : 'st-disclosure-note'}>{children}</span>;
}

/** A group's keys at a concrete path: the shown ones, "Show N more keys", then its nested objects and collections. */
export function GroupBody({ group, path }: { group: Group; path: string }) {
  const c = useSettings();
  const { shown, more, nested } = ownKeys(group, c.tree, path, c.ui === true);
  const where = path === '' ? 'the case' : path;
  const moreId = `more:${path}`;
  // with the engine's advanced flag (F-826, G15) the rest are its advanced keys, one step away behind Show all
  const moreTitle = c.ui
    ? c.isOpen(moreId)
      ? `${plural(more.length, 'advanced key', 'advanced keys')} of ${where}`
      : `Show all: ${plural(more.length, 'advanced key', 'advanced keys')} of ${where}`
    : c.isOpen(moreId)
      ? `${plural(more.length, 'more key', 'more keys')} of ${where}`
      : `Show ${plural(more.length, 'more key', 'more keys')} of ${where}`;
  return (
    <>
      {shown.map((l) => (
        <KeyRow key={l.pattern} leaf={l} path={concrete(l.pattern, path, group.pattern)} />
      ))}
      {more.length > 0 && (
        <Disclosure id={moreId} title={moreTitle}>
          {more.map((l) => (
            <KeyRow key={l.pattern} leaf={l} path={concrete(l.pattern, path, group.pattern)} />
          ))}
        </Disclosure>
      )}
      {nested.map((n) => (
        <NestedNode key={n.pattern} node={n} path={concrete(n.pattern, path, group.pattern)} />
      ))}
    </>
  );
}

/** A nested object or a collection inside a group: one step away. */
export function NestedNode({ node, path }: { node: Group | Collection; path: string }) {
  const c = useSettings();
  const n = setCount(node, c.tree, path);
  const keys = keyCount(node);
  const absent = node.kind === 'group' && node.optional && valueAt(c.tree, path) === undefined;
  const notes = (
    <>
      {node.kind === 'collection' ? (
        <Note>
          named entries, {plural(entryNames(c.tree, path).length, 'entry', 'entries')}, {plural(keys, 'key', 'keys')} each
        </Note>
      ) : (
        <Note>{plural(keys, 'key', 'keys')}</Note>
      )}
      {n > 0 && <Note set>{n} set</Note>}
      {node.kind === 'group' && node.optional && <Note>{absent ? 'optional, not in the case: defaults apply' : 'optional'}</Note>}
    </>
  );
  return (
    <Disclosure id={`group:${path}`} title={node.label ?? node.name} notes={notes} description={node.help ?? node.description} path={path}>
      <div className="st-nested">
        {node.description && <p className="st-head-inline sr-only">{node.description}</p>}
        {node.kind === 'group' ? <GroupBody group={node} path={path} /> : <CollectionBody collection={node} path={path} />}
      </div>
    </Disclosure>
  );
}

/** A collection's entries, each one step away, and the bar that adds one by name. */
export function CollectionBody({ collection, path }: { collection: Collection; path: string }) {
  const c = useSettings();
  const entries = entryNames(c.tree, path);
  return (
    <>
      {entries.length === 0 && (
        <p className="st-head-inline">
          No entries yet. Each entry of {path} has {plural(keyCount(collection.entry), 'key', 'keys')}; add one by its name.
        </p>
      )}
      {entries.map((e) => (
        <Entry key={e} node={collection.entry} path={`${path}.${e}`} name={e} />
      ))}
      <AddEntry path={path} />
    </>
  );
}

function Entry({ node, path, name }: { node: SettingNode; path: string; name: string }) {
  const c = useSettings();
  const n = setCount(node, c.tree, path);
  return (
    <Disclosure id={`entry:${path}`} title={name} notes={n > 0 ? <Note set>{n} set</Note> : <Note>entry</Note>} path={path}>
      <div className="st-nested">
        <RemoveEntry path={path} keys={keyCount(node)} />
        {node.kind === 'group' ? (
          <GroupBody group={node} path={path} />
        ) : node.kind === 'collection' ? (
          <CollectionBody collection={node} path={path} />
        ) : (
          <KeyRow leaf={node} path={path} />
        )}
      </div>
    </Disclosure>
  );
}

/** Removing an entry is confirmed in words that name what is lost (G18). */
function RemoveEntry({ path, keys }: { path: string; keys: number }) {
  const c = useSettings();
  const [asking, setAsking] = useState(false);
  const error = c.errors.get(path);
  if (!asking) {
    return (
      <div className="st-entry-bar">
        <Button isDisabled={!c.editable} onPress={() => setAsking(true)} aria-label={`Remove entry ${path}`}>
          Remove entry
        </Button>
        {error && (
          <span className="st-key-error">
            <StatePictogram state="failed" size={12} />
            <span>Remove failed. {error}</span>
          </span>
        )}
        <ConflictNote path={path} />
      </div>
    );
  }
  return (
    <div className="st-confirm" role="group" aria-label={`Remove ${path}?`}>
      <span>
        Remove {path} and its {plural(keys, 'key', 'keys')} from the session?
      </span>
      <Button
        variant="primary"
        autoFocus
        onPress={() => {
          setAsking(false);
          void c.removeEntry(path);
        }}
      >
        Remove
      </Button>
      <Button onPress={() => setAsking(false)}>Keep</Button>
    </div>
  );
}

function AddEntry({ path }: { path: string }) {
  const c = useSettings();
  const prefilled = c.prefill.get(path);
  const [name, setName] = useState(prefilled ?? '');
  const [seen, setSeen] = useState(prefilled);
  if (prefilled !== seen) {
    setSeen(prefilled);
    if (prefilled !== undefined) setName(prefilled);
  }
  const error = c.errors.get(`add:${path}`);
  const add = () => {
    void c.addEntry(path, name).then((ok) => {
      if (ok) setName('');
    });
  };
  return (
    <div className="st-entry-bar" data-path={`add:${path}`}>
      <TextField
        aria-label={`Name of a new entry of ${path}`}
        placeholder="Name of the new entry"
        value={name}
        onChange={setName}
        isReadOnly={!c.editable}
        onKeyDown={(e) => {
          if (e.key === 'Enter') {
            e.preventDefault();
            add();
          }
        }}
      />
      <Button icon="plus" isDisabled={!c.editable} onPress={add}>
        Add entry
      </Button>
      {error && (
        <span className="st-key-error">
          <StatePictogram state="failed" size={12} />
          <span>Adding failed. {error}</span>
        </span>
      )}
    </div>
  );
}
