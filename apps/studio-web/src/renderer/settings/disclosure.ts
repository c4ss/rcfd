// Learner-first disclosure of the case keys (F-825; PRODUCT.md Principle 5; G15: disclosure decides where a key sits,
// never whether it exists). Pure functions over the model of schema.ts and the session's settings tree.
//
// With the engine's UI metadata (F-826, ADR 0730: `settings.schema {ui: true}`, CaseModel.ui) rule 2 reads its
// `x-rcfd-advanced` flag: a group's common keys are shown, its advanced keys sit one step away behind "Show all". Without
// it (a service started without --ui) the tiers come from the schema's structure alone, with no Studio-side list of keys:
//
//   1. The sections are the schema's top-level fields in the engine's declaration order (`required`), optional ones
//      last; the case's own top-level keys (title) form the first section. A first index entry lists every key the case
//      set (isSet): what a case changed is never more than one step away.
//   2. In a section or group, its own keys come first, in declaration order: the first COMMON_KEYS are shown, the rest
//      are one step away ("Show N more keys"). A key that is set is always shown, wherever it sits.
//   3. Nested objects, optional objects and named collections are one step away, each a disclosure that says how many
//      keys it holds and how many are set.
//   4. Search reaches every key in one step; "Show every key" opens every step at once. Nothing is hidden for good.
import type { JsonValue } from '../../shared/actions/params.js';
import { concrete, entryNames, isSet, valueAt, type CaseModel, type Collection, type Group, type Leaf, type SettingNode } from './schema.js';

/** How many of a section's own keys are shown before "Show N more keys" (rule 2) when the schema has no advanced flag. */
export const COMMON_KEYS = 8;

/** One entry of the section index. */
export interface Section {
  /** The section's path ('' for the case's own top-level keys). */
  readonly id: string;
  readonly title: string;
  readonly node: Group | Collection;
}

/** The case's own top-level keys, as a group of their own (pattern ''). */
export function rootSection(model: CaseModel): Group {
  return { ...model.root, name: 'case', children: model.root.children.filter((c) => c.kind === 'leaf') };
}

/** The section index (rule 1), in the engine's order. */
export function sections(model: CaseModel): Section[] {
  const out: Section[] = [];
  const own = rootSection(model);
  if (own.children.length > 0) out.push({ id: '', title: 'case', node: own });
  for (const c of model.root.children) {
    if (c.kind === 'group' || c.kind === 'collection') out.push({ id: c.pattern, title: c.name, node: c });
  }
  return out;
}

export interface OwnKeys {
  /** Shown at first: the first COMMON_KEYS own keys, and every own key that is set. */
  readonly shown: readonly Leaf[];
  /** One step away (rule 2). */
  readonly more: readonly Leaf[];
  /** Nested objects and collections, each one step away (rule 3). */
  readonly nested: readonly (Group | Collection)[];
}

/**
 * The keys of a group at a concrete path, split by rule 2: with the engine's advanced flag (`ui`), the common keys and
 * every key that is set are shown and the advanced ones are one step away; without it, the first COMMON_KEYS.
 */
export function ownKeys(group: Group, tree: unknown, path: string, ui = false): OwnKeys {
  const leaves = group.children.filter((c): c is Leaf => c.kind === 'leaf');
  const shown: Leaf[] = [];
  const more: Leaf[] = [];
  leaves.forEach((l, i) => {
    const at = concrete(l.pattern, path, group.pattern);
    const common = ui ? !l.advanced : i < COMMON_KEYS;
    (common || isSet(l, valueAt(tree, at)) ? shown : more).push(l);
  });
  return { shown, more, nested: group.children.filter((c): c is Group | Collection => c.kind !== 'leaf') };
}

/** How many keys a schema node holds (a collection counts the keys of one entry). */
export function keyCount(node: SettingNode): number {
  if (node.kind === 'leaf') return 1;
  if (node.kind === 'group') return node.children.reduce((n, c) => n + keyCount(c), 0);
  return keyCount(node.entry);
}

/** Every key at or under a node at a concrete path, with its concrete path and value (named entries expanded). */
export function instances(node: SettingNode, tree: unknown, path: string): { leaf: Leaf; path: string; value: JsonValue | undefined }[] {
  if (node.kind === 'leaf') return [{ leaf: node, path, value: valueAt(tree, path) }];
  if (node.kind === 'group') return node.children.flatMap((c) => instances(c, tree, concrete(c.pattern, path, node.pattern)));
  return entryNames(tree, path).flatMap((e) => instances(node.entry, tree, `${path}.${e}`));
}

/** How many keys at or under a node are set. */
export function setCount(node: SettingNode, tree: unknown, path: string): number {
  return instances(node, tree, path).filter((i) => isSet(i.leaf, i.value)).length;
}

/** Every key the case set (rule 1's first entry), in schema order. */
export function setKeys(model: CaseModel, tree: unknown): { leaf: Leaf; path: string; value: JsonValue | undefined }[] {
  return instances(model.root, tree, '').filter((i) => isSet(i.leaf, i.value));
}

/** Every named collection of the case that has entries, with their names. */
export function filledCollections(model: CaseModel, tree: unknown): { collection: Collection; path: string; entries: string[] }[] {
  const out: { collection: Collection; path: string; entries: string[] }[] = [];
  const walk = (n: SettingNode, path: string) => {
    if (n.kind === 'group') n.children.forEach((c) => walk(c, concrete(c.pattern, path, n.pattern)));
    else if (n.kind === 'collection') {
      const entries = entryNames(tree, path);
      if (entries.length) out.push({ collection: n, path, entries });
      for (const e of entries) walk(n.entry, `${path}.${e}`);
    }
  };
  walk(model.root, '');
  return out;
}

export interface SearchHit {
  readonly leaf: Leaf;
  /** The concrete path, or null when the key sits in a collection that has no entry yet. */
  readonly path: string | null;
  /** The collection to add an entry to first, when path is null. */
  readonly collection?: string;
}

/** Keys whose path, name or description holds every word of the query (rule 4). */
export function search(model: CaseModel, tree: unknown, query: string): SearchHit[] {
  const words = query.toLowerCase().split(/\s+/).filter((w) => w.length > 0);
  if (words.length === 0) return [];
  const hits: SearchHit[] = [];
  const matches = (l: Leaf) => {
    const hay = `${l.pattern} ${l.label ?? ''} ${l.help ?? ''} ${l.description}`.toLowerCase();
    return words.every((w) => hay.includes(w));
  };
  const walk = (n: SettingNode, path: string | null, collection?: string) => {
    if (n.kind === 'leaf') {
      if (matches(n)) hits.push(path === null ? { leaf: n, path, collection: collection! } : { leaf: n, path });
    } else if (n.kind === 'group') {
      for (const c of n.children) walk(c, path === null ? null : concrete(c.pattern, path, n.pattern), collection);
    } else {
      const entries = path === null ? [] : entryNames(tree, path);
      if (entries.length === 0) walk(n.entry, null, collection ?? path ?? n.pattern);
      for (const e of entries) walk(n.entry, `${path}.${e}`, collection);
    }
  };
  walk(model.root, '');
  return hits;
}

/** The index entry a concrete path belongs to. */
export function sectionOf(path: string, list: readonly Section[]): string {
  const head = path.split('.')[0] ?? '';
  return list.some((s) => s.id === head) ? head : '';
}
