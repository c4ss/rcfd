// The schema model and the learner-first disclosure of the settings surface (F-825; G2, G15; PRODUCT Principle 5), as
// pure functions: built from a test schema with the engine's shapes, from that schema with a key added (it appears with
// no code change), and from the real engine's settings.schema, where every case key of the F-824 inventory must get
// exactly one control. The real-engine test needs the rcfd binary and is skipped, with the reason, without it.
import { expect, test } from '@playwright/test';
import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { schemaLeaves, type Inventory } from '../support/capabilities.js';
import { addKey, testSchema } from '../support/fake-case-service.js';
import { appRoot, findRcfd, NO_RCFD } from '../support/rcfd.js';
import { caseModel, encode, entrySeed, formatValue, isSet, rangeText, recommendedText, unitOf, type Leaf } from '../../src/renderer/settings/schema.js';
import { COMMON_KEYS, instances, ownKeys, search, sections, setKeys } from '../../src/renderer/settings/disclosure.js';

const leafAt = (leaves: readonly Leaf[], pattern: string): Leaf => {
  const l = leaves.find((x) => x.pattern === pattern);
  if (!l) throw new Error(`no leaf ${pattern}`);
  return l;
};

test.describe('the model is the schema', () => {
  test('every leaf of the test schema becomes one control, with its type, unit, range, enumeration and default', () => {
    const schema = testSchema();
    const m = caseModel({ case: schema, monitors: {} });
    expect(m.leaves.map((l) => l.pattern).sort()).toEqual(schemaLeaves(schema).map((l) => l.path).sort());
    const alpha = leafAt(m.leaves, 'solver.alpha_u');
    expect([alpha.control, alpha.default, alpha.minimum, alpha.maximum]).toEqual(['number', 0.9, 0.05, 1]);
    expect(rangeText(alpha)).toBe('0.05 to 1');
    const dt = leafAt(m.leaves, 'solver.dt');
    expect(unitOf(dt)).toBe('s');
    expect(rangeText(dt)).toBe('at least 0');
    expect(leafAt(m.leaves, 'solver.algorithm')).toMatchObject({ control: 'enum', options: ['simple', 'simplec', 'piso'], default: 'simplec' });
    expect(leafAt(m.leaves, 'solver.adaptive')).toMatchObject({ control: 'boolean', default: false });
    expect(leafAt(m.leaves, 'solver.max_iterations')).toMatchObject({ control: 'integer', minimum: 1 });
    expect(leafAt(m.leaves, 'solver.direction')).toMatchObject({ control: 'list', item: { control: 'number' } });
    expect(leafAt(m.leaves, 'boundaries.*.velocity')).toMatchObject({ control: 'list', siDimension: 'm s^-1' });
    expect(leafAt(m.leaves, 'title').description).toBe('Case title');
  });

  test('declaration order is the engine’s (required), optional fields last; sections follow it', () => {
    const m = caseModel(testSchema());
    expect(sections(m).map((s) => s.title)).toEqual(['case', 'solver', 'boundaries']);
    const solver = m.root.children.find((c) => c.name === 'solver')!;
    expect(solver.kind === 'group' && solver.children.map((c) => c.name)).toEqual([
      'algorithm',
      'alpha_u',
      'max_iterations',
      'dt',
      'adaptive',
      'note',
      'corrections',
      'direction',
      'tolerance',
      'scheme',
      'limits',
      'rhie_chow',
    ]);
  });

  test('a key and a section added to the schema appear with no code change', () => {
    const before = caseModel(testSchema());
    let schema = addKey(testSchema(), 'solver', 'f825_added', {
      type: 'number',
      default: 0.25,
      minimum: 0,
      maximum: 2,
      'x-rcfd-si-dimension': 's',
      description: 'A key a later engine adds',
    });
    schema = addKey(schema, '', 'acoustics', {
      type: 'object',
      properties: { model: { type: 'string', enum: ['off', 'fwh'], default: 'off', description: 'Acoustic analogy' } },
      required: ['model'],
      additionalProperties: false,
      description: 'A section a later engine adds',
    });
    const after = caseModel(schema);
    expect(before.leaves.some((l) => l.pattern === 'solver.f825_added')).toBe(false);
    const added = leafAt(after.leaves, 'solver.f825_added');
    expect([added.control, added.default, rangeText(added), unitOf(added), added.description]).toEqual(['number', 0.25, '0 to 2', 's', 'A key a later engine adds']);
    expect(sections(after).map((s) => s.title)).toEqual(['case', 'solver', 'boundaries', 'acoustics']);
    expect(leafAt(after.leaves, 'acoustics.model')).toMatchObject({ control: 'enum', options: ['off', 'fwh'] });
    // a later kind of node still gets a control: JSON
    const odd = caseModel(addKey(testSchema(), '', 'future', { type: 'object', properties: {}, required: [], additionalProperties: false, description: 'x' }));
    expect(leafAt(odd.leaves, 'future').control).toBe('json');
  });
});

test.describe('text in, value out: the Studio refuses nothing (G2)', () => {
  const m = caseModel(testSchema());
  test('numbers are sent as numbers, anything else as the text for the engine to read or refuse', () => {
    expect(encode(leafAt(m.leaves, 'solver.alpha_u'), '0.5')).toBe(0.5);
    expect(encode(leafAt(m.leaves, 'solver.alpha_u'), ' 2 ')).toBe(2);
    expect(encode(leafAt(m.leaves, 'solver.alpha_u'), '1e-3')).toBe(0.001);
    expect(encode(leafAt(m.leaves, 'solver.dt'), '10 ms')).toBe('10 ms');
    expect(encode(leafAt(m.leaves, 'solver.max_iterations'), '12')).toBe(12);
    expect(encode(leafAt(m.leaves, 'solver.max_iterations'), '1.5')).toBe('1.5');
    expect(encode(leafAt(m.leaves, 'solver.max_iterations'), 'abc')).toBe('abc');
    expect(encode(leafAt(m.leaves, 'title'), ' cavity ')).toBe(' cavity ');
    expect(encode(leafAt(m.leaves, 'solver.direction'), '1, 0, 0')).toEqual([1, 0, 0]);
    expect(encode(leafAt(m.leaves, 'solver.direction'), '[1, 2]')).toEqual([1, 2]);
    expect(encode(leafAt(m.leaves, 'solver.direction'), '')).toEqual([]);
    expect(encode(leafAt(m.leaves, 'solver.direction'), '1, 2 km/h')).toEqual([1, '2 km/h']);
  });
  test('values as text', () => {
    expect([formatValue(0.9), formatValue(1e-7), formatValue(-1e30), formatValue([1, 0, 0]), formatValue(''), formatValue(true)]).toEqual([
      '0.9',
      '1e-7',
      '-1e+30',
      '1, 0, 0',
      '',
      'true',
    ]);
  });
});

test.describe('learner-first disclosure (PRODUCT Principle 5, G15)', () => {
  const m = caseModel(testSchema());
  const solver = m.root.children.find((c) => c.name === 'solver')!;
  if (solver.kind !== 'group') throw new Error('solver is a group');
  const tree = { title: '', solver: { algorithm: 'simplec', alpha_u: 0.9, tolerance: 1e-4, scheme: 'upwind' }, boundaries: { lid: { kind: 'wall', velocity: [1, 0, 0] } } };

  test('a section shows its first keys in declaration order; the rest are one step away; a set key is always shown', () => {
    const fresh = ownKeys(solver, {}, 'solver');
    expect(fresh.shown.length).toBe(COMMON_KEYS);
    expect(fresh.shown.map((l) => l.name)).toEqual(['algorithm', 'alpha_u', 'max_iterations', 'dt', 'adaptive', 'note', 'corrections', 'direction']);
    expect(fresh.more.map((l) => l.name)).toEqual(['tolerance', 'scheme']);
    expect(fresh.nested.map((n) => n.name)).toEqual(['limits', 'rhie_chow']);
    const changed = ownKeys(solver, tree, 'solver');
    expect(changed.shown.map((l) => l.name)).toContain('tolerance');
    expect(changed.more.map((l) => l.name)).toEqual(['scheme']);
  });

  test('"Set in this case" lists what differs from the default, and lists with values, inside entries too', () => {
    expect(setKeys(m, tree).map((k) => k.path)).toEqual(['solver.tolerance', 'boundaries.lid.velocity']);
    expect(isSet(leafAt(m.leaves, 'solver.alpha_u'), undefined)).toBe(false);
  });

  test('search reaches every key, also inside entries and in collections without one', () => {
    for (const l of m.leaves) {
      const hits = search(m, tree, l.pattern.split('.').filter((p) => p !== '*').join(' '));
      expect(hits.some((h) => h.leaf === l), l.pattern).toBe(true);
    }
    expect(search(m, tree, 'velocity patch').map((h) => h.path)).toEqual(['boundaries.lid.velocity']);
    expect(search(m, { solver: {} }, 'velocity patch')).toEqual([expect.objectContaining({ path: null, collection: 'boundaries' })]);
    expect(search(m, tree, 'under-relaxation').map((h) => h.path)).toEqual(['solver.alpha_u']);
  });

  test('nothing is hidden for good: each key of each entry has a place', () => {
    const all = instances(m.root, tree, '').map((i) => i.path);
    expect(all).toContain('boundaries.lid.kind');
    expect(all).toContain('solver.rhie_chow.enabled');
    expect(all.length).toBe(m.leaves.length);
  });

  test('a new entry starts from its first key with a default', () => {
    const b = m.root.children.find((c) => c.name === 'boundaries')!;
    expect(b.kind === 'collection' && entrySeed(b.entry, 'boundaries.inlet')).toEqual({ path: 'boundaries.inlet.kind', value: 'wall' });
  });
});

test.describe('the engine’s UI metadata (F-826, ADR 0730)', () => {
  /** The test schema as `settings.schema {ui: true}` sends it: x-rcfd-ui on the root, metadata on some nodes. */
  const withMeta = () => {
    const schema = testSchema() as Record<string, any>;
    schema['x-rcfd-ui'] = 'rcfd-ui/1';
    const solver = schema['properties']['solver'];
    solver['x-rcfd-label'] = 'Solver';
    const alpha = solver['properties']['alpha_u'];
    Object.assign(alpha, {
      'x-rcfd-label': 'Velocity relaxation',
      'x-rcfd-help': 'How much of each new velocity a steady run takes.',
      'x-rcfd-recommended': { minimum: 0.5, maximum: 0.8, source: 'A. Author, A Book, 1999' },
      'x-rcfd-see': 'case-schema.md#solver',
    });
    for (const k of ['dt', 'adaptive', 'note']) solver['properties'][k]['x-rcfd-advanced'] = true;
    solver['properties']['max_iterations']['x-rcfd-unit'] = 'iterations';
    return schema;
  };

  test('labels, help, see, recommended ranges with their source and units are read as the engine sends them', () => {
    const m = caseModel({ case: withMeta(), monitors: {} });
    expect(m.ui).toBe(true);
    const alpha = leafAt(m.leaves, 'solver.alpha_u');
    expect([alpha.label, alpha.help, alpha.see, alpha.advanced]).toEqual(['Velocity relaxation', 'How much of each new velocity a steady run takes.', 'case-schema.md#solver', false]);
    expect(recommendedText(alpha)).toEqual({ range: '0.5 to 0.8', source: 'A. Author, A Book, 1999' });
    expect(unitOf(leafAt(m.leaves, 'solver.max_iterations'))).toBe('iterations');
    // a range without its source is not a recommendation (ADR 0730 Q5: no source, no range)
    const bare = withMeta();
    bare['properties']['solver']['properties']['alpha_u']['x-rcfd-recommended'] = { minimum: 0.5, maximum: 0.8 };
    expect(recommendedText(leafAt(caseModel(bare).leaves, 'solver.alpha_u'))).toBeNull();
    // without the metadata the model is as before
    const plain = caseModel({ case: testSchema(), monitors: {} });
    expect(plain.ui).toBe(false);
    expect(leafAt(plain.leaves, 'solver.alpha_u')).toMatchObject({ advanced: false });
    expect(leafAt(plain.leaves, 'solver.alpha_u').label).toBeUndefined();
  });

  test('the advanced flag decides what sits behind Show all; a set key is always shown; nothing is hidden for good', () => {
    const m = caseModel({ case: withMeta(), monitors: {} });
    const solver = m.root.children.find((c) => c.name === 'solver')!;
    if (solver.kind !== 'group') throw new Error('solver is a group');
    const fresh = ownKeys(solver, {}, 'solver', m.ui);
    expect(fresh.more.map((l) => l.name)).toEqual(['dt', 'adaptive', 'note']);
    expect(fresh.shown.map((l) => l.name)).not.toContain('dt');
    expect(fresh.shown.length + fresh.more.length).toBe(solver.children.filter((c) => c.kind === 'leaf').length);
    const set = ownKeys(solver, { solver: { dt: 0.5 } }, 'solver', m.ui);
    expect(set.shown.map((l) => l.name)).toContain('dt');
    // a key added to the schema with the flag is placed with no Studio change
    const added = withMeta();
    added['properties']['solver']['properties']['brand_new'] = { type: 'number', default: 1, 'x-rcfd-advanced': true, 'x-rcfd-label': 'Brand new' };
    const m2 = caseModel(added);
    const s2 = m2.root.children.find((c) => c.name === 'solver')!;
    if (s2.kind !== 'group') throw new Error('solver is a group');
    expect(ownKeys(s2, {}, 'solver', m2.ui).more.map((l) => l.name)).toContain('brand_new');
    // search reads the engine's label and help too
    expect(search(m, {}, 'velocity relaxation').map((h) => h.path)).toEqual(['solver.alpha_u']);
  });
});

test.describe('the real engine', () => {
  const rcfd = findRcfd();
  test('every case key of settings.schema gets exactly one control, and each matches the F-824 inventory', () => {
    test.skip(rcfd === null, NO_RCFD);
    const r = spawnSync(rcfd!, ['serve', '--stdio'], { input: '{"jsonrpc":"2.0","id":1,"method":"settings.schema"}\n', encoding: 'utf8', timeout: 60_000, maxBuffer: 64 << 20 });
    const reply = JSON.parse(r.stdout.split('\n').find((l) => l.trim() !== '')!) as { result: { case: Record<string, unknown> } };
    const m = caseModel(reply.result);
    const patterns = m.leaves.map((l) => l.pattern);
    expect(new Set(patterns).size).toBe(patterns.length);
    expect([...patterns].sort()).toEqual(schemaLeaves(reply.result.case).map((l) => l.path).sort());
    const inv = JSON.parse(readFileSync(path.join(appRoot, 'capabilities', 'inventory.json'), 'utf8')) as Inventory;
    const keys = inv.capabilities.filter((c) => c.kind === 'case_key').map((c) => c.id.slice('case:'.length));
    expect([...patterns].sort()).toEqual([...keys].sort());
    // every key's control kind comes from its node
    for (const l of m.leaves) {
      const t = l.node['type'];
      const expected = Array.isArray(l.node['enum']) ? 'enum' : t === 'array' ? 'list' : t;
      expect(l.control, l.pattern).toBe(expected);
    }
    // learner-first: every section has its head shown and the rest one step away
    for (const s of sections(m)) {
      if (s.node.kind !== 'group') continue;
      const k = ownKeys(s.node, {}, s.id);
      expect(k.shown.length, s.id).toBe(Math.min(COMMON_KEYS, k.shown.length + k.more.length));
    }
  });

  test('with rcfd serve --ui the same keys come with the engine’s labels and advanced flags (F-826, ADR 0730)', () => {
    test.skip(rcfd === null, NO_RCFD);
    const req = '{"jsonrpc":"2.0","id":1,"method":"settings.schema","params":{"ui":true}}\n';
    const r = spawnSync(rcfd!, ['serve', '--stdio', '--ui'], { input: req, encoding: 'utf8', timeout: 60_000, maxBuffer: 64 << 20 });
    const reply = JSON.parse(r.stdout.split('\n').find((l) => l.trim() !== '')!) as { result: { case: Record<string, unknown> } };
    const m = caseModel(reply.result);
    expect(m.ui).toBe(true);
    expect(m.leaves.map((l) => l.pattern).sort()).toEqual(schemaLeaves(reply.result.case).map((l) => l.path).sort());
    expect(m.leaves.filter((l) => l.advanced).length).toBeGreaterThan(50);
    expect(leafAt(m.leaves, 'solver.alpha_u').label).toBeTruthy();
    expect(leafAt(m.leaves, 'solver.alpha_u').advanced).toBe(false);
    // G15: every key of every section has its place, shown or one step away
    for (const s of sections(m)) {
      if (s.node.kind !== 'group') continue;
      const k = ownKeys(s.node, {}, s.id, true);
      expect(k.shown.length + k.more.length, s.id).toBe(s.node.children.filter((c) => c.kind === 'leaf').length);
    }
  });
});
