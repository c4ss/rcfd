// The case schema as the settings surface reads it (F-825, G2, G15): a model built at run time from the engine's
// `settings.schema` reply, never from a list of keys kept in the Studio. Pure functions only (no DOM, no React), so the
// unit tests run them in Node against a test schema and against the real engine's schema.
//
// What the schema gives (crates/rcfd-core/src/settings.rs, Schema::to_json_schema): an object has `properties`,
// `required` (its non-optional fields in the engine's declaration order) and `additionalProperties: false`; an optional
// object carries `x-rcfd-optional`; a collection of named entries is an object with `x-rcfd-named-objects` and its entry
// schema in `additionalProperties`; a list has `items`; a leaf has `type`, `default`, `minimum`, `maximum`, `enum`,
// `description`, and for a number with a physical dimension `x-rcfd-si-dimension` (stored in SI), or a display unit
// `x-rcfd-unit`. Anything else a later schema adds becomes a JSON control, so every key still gets one.
//
// The Studio checks nothing here (G2): `encode` turns typed text into the JSON value to send (a number when the text is a
// number, otherwise the text, which the engine may read as a value with a unit or refuse), and the engine's reply says
// whether it was taken.
import type { JsonObject, JsonValue } from '../../shared/actions/params.js';

export type SchemaNode = { readonly [k: string]: unknown };

/** The scalar control kinds; `list` holds items of one of them, `json` takes any JSON text. */
export type ScalarControl = 'boolean' | 'integer' | 'number' | 'string' | 'enum' | 'json';
export type Control = ScalarControl | 'list';

interface Common {
  /** The last part of the path: the key's own name (`alpha_u`), `*` for a named entry. */
  readonly name: string;
  /** The schema path, dotted, with `*` for each named entry (the id of the inventory without `case:`). */
  readonly pattern: string;
  /** The engine's description of the key (G19), or ''. */
  readonly description: string;
}

/** The facts of a scalar schema node a control is made from. */
export interface ScalarSpec {
  readonly control: ScalarControl;
  /** The schema's default, when it states one. */
  readonly default?: JsonValue;
  readonly minimum?: number;
  readonly maximum?: number;
  /** The allowed values of an enumeration. */
  readonly options?: readonly string[];
  /** `x-rcfd-si-dimension`: the SI unit of a dimensioned number, as the engine writes it (`m^-1 kg s^-2`). */
  readonly siDimension?: string;
  /** `x-rcfd-unit`: a display unit the value is given in (`deg`); the engine never converts by it. */
  readonly unit?: string;
}

export interface Leaf extends Common, Omit<ScalarSpec, 'control'> {
  readonly kind: 'leaf';
  readonly control: Control;
  /** A list's item. */
  readonly item?: ScalarSpec;
  /** The schema node as the engine sent it. */
  readonly node: SchemaNode;
}

export interface Group extends Common {
  readonly kind: 'group';
  /** `x-rcfd-optional`: absent from the case until a key inside it is set. */
  readonly optional: boolean;
  /** Fields in the engine's declaration order (`required`), then the optional ones. */
  readonly children: readonly SettingNode[];
}

export interface Collection extends Common {
  readonly kind: 'collection';
  /** The schema of each entry. */
  readonly entry: SettingNode;
}

export type SettingNode = Leaf | Group | Collection;

export interface CaseModel {
  /** The case root (pattern ''). */
  readonly root: Group;
  /** Every leaf, depth first in declaration order. */
  readonly leaves: readonly Leaf[];
}

const isObject = (v: unknown): v is SchemaNode => typeof v === 'object' && v !== null && !Array.isArray(v);
const str = (v: unknown): string => (typeof v === 'string' ? v : '');
const num = (v: unknown): number | undefined => (typeof v === 'number' && Number.isFinite(v) ? v : undefined);

function join(at: string, name: string): string {
  return at === '' ? name : `${at}.${name}`;
}

function scalarSpec(node: SchemaNode): ScalarSpec {
  const type = node['type'];
  const options = Array.isArray(node['enum']) && node['enum'].every((x) => typeof x === 'string') ? (node['enum'] as string[]) : undefined;
  const control: ScalarControl = options
    ? 'enum'
    : type === 'boolean'
      ? 'boolean'
      : type === 'integer'
        ? 'integer'
        : type === 'number'
          ? 'number'
          : type === 'string'
            ? 'string'
            : 'json';
  const spec: { -readonly [K in keyof ScalarSpec]: ScalarSpec[K] } = { control };
  if ('default' in node) spec.default = node['default'] as JsonValue;
  const min = num(node['minimum']);
  const max = num(node['maximum']);
  if (min !== undefined) spec.minimum = min;
  if (max !== undefined) spec.maximum = max;
  if (options) spec.options = options;
  if (typeof node['x-rcfd-si-dimension'] === 'string') spec.siDimension = node['x-rcfd-si-dimension'];
  if (typeof node['x-rcfd-unit'] === 'string') spec.unit = node['x-rcfd-unit'];
  return spec;
}

/** The fields of an object node in the engine's declaration order: `required` first, then the rest as listed. */
export function fieldOrder(node: SchemaNode): string[] {
  const props = isObject(node['properties']) ? Object.keys(node['properties']) : [];
  const required = Array.isArray(node['required']) ? node['required'].filter((k): k is string => typeof k === 'string' && props.includes(k)) : [];
  return [...required, ...props.filter((k) => !required.includes(k))];
}

function build(node: SchemaNode, name: string, at: string): SettingNode {
  const description = str(node['description']);
  const props = node['properties'];
  const ap = node['additionalProperties'];
  // An object without fields is a key of its own (the inventory counts it as one): a JSON control.
  if (isObject(props) && Object.keys(props).length > 0) {
    const children = fieldOrder(node).map((k) => {
      const child = props[k];
      return build(isObject(child) ? child : {}, k, join(at, k));
    });
    return { kind: 'group', name, pattern: at, description, optional: node['x-rcfd-optional'] === true, children };
  }
  if (isObject(ap)) {
    return { kind: 'collection', name, pattern: at, description, entry: build(ap, '*', join(at, '*')) };
  }
  if (isObject(props)) return { kind: 'leaf', name, pattern: at, description, control: 'json', node };
  if (node['type'] === 'array') {
    const items = node['items'];
    // A list of objects has no item control of its own: the whole list is one JSON control.
    if (isObject(items) && !isObject(items['properties']) && !isObject(items['additionalProperties'])) {
      return { kind: 'leaf', name, pattern: at, description, control: 'list', item: scalarSpec(items), node, ...listUnit(items) };
    }
    return { kind: 'leaf', name, pattern: at, description, control: 'json', node };
  }
  return { kind: 'leaf', name, pattern: at, description, ...scalarSpec(node), node };
}

function listUnit(items: SchemaNode): Pick<ScalarSpec, 'siDimension' | 'unit'> {
  const s = scalarSpec(items);
  return { ...(s.siDimension ? { siDimension: s.siDimension } : {}), ...(s.unit ? { unit: s.unit } : {}) };
}

/** The model of a `settings.schema` reply (`{case, monitors}`), or of a case schema itself. */
export function caseModel(reply: unknown): CaseModel {
  const schema = isObject(reply) && isObject(reply['case']) ? reply['case'] : reply;
  if (!isObject(schema) || !isObject(schema['properties'])) throw new Error('settings.schema: the reply has no case schema with properties');
  const root = build(schema, '', '');
  if (root.kind !== 'group') throw new Error('settings.schema: the case schema has no keys');
  const leaves: Leaf[] = [];
  const walk = (n: SettingNode) => {
    if (n.kind === 'leaf') leaves.push(n);
    else if (n.kind === 'group') n.children.forEach(walk);
    else walk(n.entry);
  };
  walk(root);
  return { root, leaves };
}

// ---- values

/** The value at a dotted path of the settings tree (settings.get without a path), or undefined when it is absent. */
export function valueAt(tree: unknown, path: string): JsonValue | undefined {
  if (path === '') return tree as JsonValue;
  let v: unknown = tree;
  for (const part of path.split('.')) {
    if (!isObject(v) || !(part in v)) return undefined;
    v = v[part];
  }
  return v as JsonValue;
}

/** The names of the entries of a collection at a concrete path. */
export function entryNames(tree: unknown, path: string): string[] {
  const v = valueAt(tree, path);
  return isObject(v) ? Object.keys(v) : [];
}

export function sameValue(a: unknown, b: unknown): boolean {
  if (a === b) return true;
  if (typeof a === 'number' && typeof b === 'number') return Number.isNaN(a) && Number.isNaN(b);
  if (Array.isArray(a) && Array.isArray(b)) return a.length === b.length && a.every((x, i) => sameValue(x, b[i]));
  if (isObject(a) && isObject(b)) {
    const ka = Object.keys(a);
    return ka.length === Object.keys(b).length && ka.every((k) => k in b && sameValue(a[k], b[k]));
  }
  return false;
}

/**
 * Whether a key holds something other than its default: a value that differs from the schema's stated default, or a
 * list with items (the schema states no default for a list). An absent key (inside an optional object not in the
 * case) is not set.
 */
export function isSet(leaf: Leaf, value: JsonValue | undefined): boolean {
  if (value === undefined) return false;
  if (leaf.control === 'list') return Array.isArray(value) && value.length > 0;
  if (leaf.default === undefined) return false;
  return !sameValue(value, leaf.default);
}

/** A concrete path for a pattern inside the entries named so far: `boundaries.*.velocity` in `boundaries.lid`. */
export function concrete(pattern: string, base: string, basePattern: string): string {
  if (basePattern === '') return pattern;
  if (pattern === basePattern) return base;
  return base + pattern.slice(basePattern.length);
}

// ---- text in, text out

const NUMBER = /^[-+]?(\d+\.?\d*|\.\d+)([eE][-+]?\d+)?$/;
const INTEGER = /^[-+]?\d+$/;

/** A value as the text of its field. */
export function formatValue(v: JsonValue | undefined): string {
  if (v === undefined || v === null) return '';
  if (typeof v === 'string') return v;
  if (typeof v === 'number' || typeof v === 'boolean') return String(v);
  if (Array.isArray(v)) return v.every((x) => x === null || typeof x !== 'object') ? v.map((x) => formatValue(x)).join(', ') : JSON.stringify(v);
  return JSON.stringify(v);
}

function encodeScalar(spec: { readonly control: Control }, text: string): JsonValue {
  const t = text.trim();
  switch (spec.control) {
    case 'integer':
      return INTEGER.test(t) && Number.isSafeInteger(Number(t)) ? Number(t) : text;
    case 'number':
      return NUMBER.test(t) ? Number(t) : text;
    case 'boolean':
      return t === 'true' ? true : t === 'false' ? false : text;
    case 'json':
      try {
        return JSON.parse(text) as JsonValue;
      } catch {
        return text;
      }
    default:
      return text;
  }
}

/**
 * The JSON value to send for the text typed into a key's field. Nothing is refused here: text that is not a number in a
 * number field is sent as text, and the engine reads it (a value with a unit) or answers why not.
 * A list takes values separated by commas, or a JSON list.
 */
export function encode(leaf: Pick<Leaf, 'control' | 'item'>, text: string): JsonValue {
  if (leaf.control !== 'list') return encodeScalar(leaf, text);
  const t = text.trim();
  if (t === '') return [];
  if (t.startsWith('[')) {
    try {
      const v = JSON.parse(t) as JsonValue;
      if (Array.isArray(v)) return v;
    } catch {
      // not JSON: read as values separated by commas
    }
  }
  const item = leaf.item ?? { control: 'string' as const };
  return t.split(',').map((x) => encodeScalar(item, x.trim()));
}

/** The range of a number as words: `0.05 to 1`, `at least 0`, `at most 3`, or null. */
export function rangeText(spec: Pick<ScalarSpec, 'minimum' | 'maximum'>): string | null {
  const { minimum: lo, maximum: hi } = spec;
  if (lo !== undefined && hi !== undefined) return `${lo} to ${hi}`;
  if (lo !== undefined) return `at least ${lo}`;
  if (hi !== undefined) return `at most ${hi}`;
  return null;
}

/** The unit a key's label carries (DESIGN.md: the unit sits in the label), or null. */
export function unitOf(leaf: Pick<ScalarSpec, 'siDimension' | 'unit'>): string | null {
  return leaf.unit ?? leaf.siDimension ?? null;
}

/**
 * The first key of an entry schema that has a default, and its concrete path under `entryPath`: setting it to its
 * default makes a new named entry (settings.set makes named entries on demand), with every other key at its default.
 */
export function entrySeed(entry: SettingNode, entryPath: string): { path: string; value: JsonValue } | null {
  const find = (n: SettingNode, at: string): { path: string; value: JsonValue } | null => {
    if (n.kind === 'leaf') return n.default !== undefined ? { path: at, value: n.default } : n.control === 'list' ? { path: at, value: [] } : null;
    if (n.kind === 'group') {
      for (const c of n.children) {
        if (c.kind === 'group' && c.optional) continue;
        const r = find(c, join(at, c.name));
        if (r) return r;
      }
    }
    return null;
  };
  return find(entry, entryPath);
}

/** A JSON value as an object, for the parameters of an action. */
export function asJsonObject(v: unknown): JsonObject {
  return isObject(v) ? (v as JsonObject) : {};
}
