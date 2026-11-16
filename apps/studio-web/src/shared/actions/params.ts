// The parameter schema of a Studio action (F-827): a small, typed description of each parameter that gives the action's
// TypeScript parameter type, a JSON Schema for the command palette (F-831) and in-place help (G19), and the shape check
// the dispatcher runs before anything is sent. It checks shapes only (a string, an integer at least 1); what a value
// means for a case is the engine's to decide (G2), which is why there is no case knowledge here.
//
// Shared by the renderer, the main process and the tests: no Node or DOM types.

/** A JSON value, as it crosses the service port (src/main/service/bridge.ts checks the same thing again). */
export type JsonValue = null | boolean | number | string | readonly JsonValue[] | { readonly [key: string]: JsonValue };
export type JsonObject = { readonly [key: string]: JsonValue };

interface Common {
  /** One sentence for the palette and for help on the spot (G19). */
  readonly description: string;
  /** True when the parameter may be left out (the engine's default applies). */
  readonly optional?: boolean;
}

/**
 * The kinds of parameter. `session` is a session id of the service: a replay maps the ids of the sessions a script made
 * to the ids its own run makes (dispatcher.ts, replay).
 */
export type ParamSpec =
  | (Common & { readonly type: 'session' })
  | (Common & { readonly type: 'string'; readonly enum?: readonly string[] })
  | (Common & { readonly type: 'integer'; readonly minimum?: number; readonly maximum?: number })
  | (Common & { readonly type: 'number'; readonly minimum?: number; readonly maximum?: number })
  | (Common & { readonly type: 'boolean' })
  | (Common & { readonly type: 'string-array' })
  | (Common & { readonly type: 'object' })
  | (Common & { readonly type: 'json' });

export type ParamsSpec = { readonly [name: string]: ParamSpec };

type ValueOf<S extends ParamSpec> = S['type'] extends 'session' | 'string'
  ? string
  : S['type'] extends 'integer' | 'number'
    ? number
    : S['type'] extends 'boolean'
      ? boolean
      : S['type'] extends 'string-array'
        ? readonly string[]
        : S['type'] extends 'object'
          ? JsonObject
          : JsonValue;

type RequiredKeys<P extends ParamsSpec> = { [K in keyof P]: P[K] extends { readonly optional: true } ? never : K }[keyof P];
type OptionalKeys<P extends ParamsSpec> = { [K in keyof P]: P[K] extends { readonly optional: true } ? K : never }[keyof P];

/** The TypeScript type of the parameters a spec describes. */
export type ParamsOf<P extends ParamsSpec> = { [K in RequiredKeys<P>]: ValueOf<P[K]> } & { [K in OptionalKeys<P>]?: ValueOf<P[K]> };

const isPlainObject = (v: unknown): v is Record<string, unknown> => {
  if (typeof v !== 'object' || v === null || Array.isArray(v)) return false;
  const proto = Object.getPrototypeOf(v);
  return proto === Object.prototype || proto === null;
};

/** Null when `v` is made of JSON values only. */
export function jsonValueProblem(v: unknown, where: string, depth = 0): string | null {
  if (depth > 64) return `${where} is nested deeper than 64`;
  if (v === null || typeof v === 'boolean' || typeof v === 'string') return null;
  if (typeof v === 'number') return Number.isFinite(v) ? null : `${where} is not a finite number`;
  if (Array.isArray(v)) {
    for (let i = 0; i < v.length; i++) {
      if (!(i in v)) return `${where}[${i}] is a hole`;
      const p = jsonValueProblem(v[i], `${where}[${i}]`, depth + 1);
      if (p) return p;
    }
    return null;
  }
  if (isPlainObject(v)) {
    for (const k of Object.keys(v)) {
      const p = jsonValueProblem(v[k], `${where}.${k}`, depth + 1);
      if (p) return p;
    }
    return null;
  }
  return `${where} is not a JSON value`;
}

function valueProblem(name: string, spec: ParamSpec, v: unknown): string | null {
  switch (spec.type) {
    case 'session':
      return typeof v === 'string' && v.length > 0 ? null : `${name} must be a session id (a non-empty string)`;
    case 'string':
      if (typeof v !== 'string') return `${name} must be a string`;
      if (spec.enum && !spec.enum.includes(v)) return `${name} must be one of ${spec.enum.join(', ')}`;
      return null;
    case 'integer':
    case 'number': {
      if (typeof v !== 'number' || !Number.isFinite(v)) return `${name} must be a number`;
      if (spec.type === 'integer' && !Number.isSafeInteger(v)) return `${name} must be an integer`;
      if (spec.minimum !== undefined && v < spec.minimum) return `${name} must be at least ${spec.minimum}`;
      if (spec.maximum !== undefined && v > spec.maximum) return `${name} must be at most ${spec.maximum}`;
      return null;
    }
    case 'boolean':
      return typeof v === 'boolean' ? null : `${name} must be true or false`;
    case 'string-array':
      return Array.isArray(v) && v.every((x) => typeof x === 'string') ? null : `${name} must be a list of strings`;
    case 'object':
      return isPlainObject(v) ? jsonValueProblem(v, name) : `${name} must be an object`;
    case 'json':
      return jsonValueProblem(v, name);
  }
}

/** Null when `params` has the shape `spec` describes: no unknown name, every required one, each of its kind. */
export function paramsProblem(spec: ParamsSpec, params: unknown): string | null {
  if (!isPlainObject(params)) return 'the parameters must be an object';
  for (const k of Object.keys(params)) {
    if (!Object.hasOwn(spec, k)) {
      const known = Object.keys(spec);
      return `unknown parameter '${k}'${known.length ? ` (expected ${known.join(', ')})` : ' (this action takes none)'}`;
    }
  }
  for (const [k, s] of Object.entries(spec)) {
    const v = params[k];
    if (v === undefined) {
      if (!s.optional) return `missing parameter '${k}': ${s.description}`;
      continue;
    }
    const p = valueProblem(k, s, v);
    if (p) return p;
  }
  return null;
}

/** The names of the parameters that hold session ids. */
export function sessionParams(spec: ParamsSpec): string[] {
  return Object.entries(spec)
    .filter(([, s]) => s.type === 'session')
    .map(([k]) => k);
}

/** A JSON Schema (draft 2020-12 subset) of a parameter spec, for the palette, help and the capability inventory. */
export function toJsonSchema(spec: ParamsSpec): JsonObject {
  const properties: Record<string, JsonValue> = {};
  for (const [k, s] of Object.entries(spec)) {
    const base: Record<string, JsonValue> = { description: s.description };
    switch (s.type) {
      case 'session':
        properties[k] = { ...base, type: 'string', minLength: 1, 'x-rcfd-kind': 'session' };
        break;
      case 'string':
        properties[k] = { ...base, type: 'string', ...(s.enum ? { enum: [...s.enum] } : {}) };
        break;
      case 'integer':
      case 'number':
        properties[k] = {
          ...base,
          type: s.type,
          ...(s.minimum !== undefined ? { minimum: s.minimum } : {}),
          ...(s.maximum !== undefined ? { maximum: s.maximum } : {}),
        };
        break;
      case 'boolean':
        properties[k] = { ...base, type: 'boolean' };
        break;
      case 'string-array':
        properties[k] = { ...base, type: 'array', items: { type: 'string' } };
        break;
      case 'object':
        properties[k] = { ...base, type: 'object' };
        break;
      case 'json':
        properties[k] = base;
        break;
    }
  }
  const required = Object.entries(spec)
    .filter(([, s]) => !s.optional)
    .map(([k]) => k);
  return { type: 'object', properties, required, additionalProperties: false };
}
