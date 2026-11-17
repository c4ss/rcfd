// Unit tests of the Claude Code panel (F-814; G11, G10): where claude comes from, the conservative allow-list and the
// argument check, the stream and transcript readers, session discovery, the port's checks, reference links, and the
// panel driven against the fake claude of tests/support/fake-claude.mjs (never a real claude). One test per rule:
// mirror read-only, resume, one writer, the allow-list in the argument list, never approve-everything, link detection.
import { expect, test } from '@playwright/test';
import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { appendFile, mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { cssCustomProperties } from '../../src/renderer/design/tokens.js';
import { isUuid, type ClaudeState } from '../../src/shared/claude.js';
import {
  AllowList,
  DELETE_TOOLS,
  isDeleteTool,
  checkArgs,
  checkArgsIn,
  ForbiddenArgument,
  parseAllowFile,
  allowFileHash,
  allowFileText,
  parseRule,
  PROJECT_READ_TOOLS,
  readAllowText,
  ruleProblem,
  suggest,
  withRule,
} from '../../src/main/claude/allow.js';
import { parseClaudeRequest } from '../../src/main/claude/bridge.js';
import { claudeConfig, type ClaudeConfig } from '../../src/main/claude/config.js';
import { classify, isAlive, parseAgents, projectDirName } from '../../src/main/claude/discover.js';
import { chatArgs } from '../../src/main/claude/driver.js';
import { ClaudePanel, ClaudeRefusal } from '../../src/main/claude/panel.js';
import { parseLine, uncheckedProblem } from '../../src/main/claude/stream.js';
import { parseEntry, readNew, tail } from '../../src/main/claude/transcript.js';
import { scan } from '../../src/main/claude/trust.js';
import { fileOpensIn, findRefs, paragraphs } from '../../src/renderer/claude/links.js';
import { APPROVE_EVERYTHING, claudeFixture, FAKE_CLAUDE, sampleTranscript, type ClaudeFixture } from '../support/claude.js';
import { until } from '../support/fake-service.js';
import { appRoot } from '../support/rcfd.js';

const S1 = '00000000-0000-4000-8000-000000000001';
const S2 = '00000000-0000-4000-8000-000000000002';
const RUN = 'r-20261028T152641-3900';

/** A pid that is certainly not running: a process that has already ended. */
function deadPid(): number {
  const r = spawnSync(process.execPath, ['-e', '0']);
  return r.pid!;
}

// ---- configuration

test.describe('claudeConfig', () => {
  const isFile = (p: string) => p.endsWith('claude.exe') || p.endsWith('.mjs');
  test('claude only from RCFD_STUDIO_CLAUDE, never from PATH; a Node script runs on the Studio runtime', () => {
    const none = claudeConfig({ PATH: 'C:\\Users\\x\\.local\\bin', USERPROFILE: 'C:\\Users\\x', HOME: '/home/x' }, '/ud', isFile, '/electron');
    expect(none.program.configured).toBe(false);
    expect(none.command).toBeNull();
    expect(none.program.configured ? '' : none.program.reason).toContain('RCFD_STUDIO_CLAUDE');
    const exe = claudeConfig({ RCFD_STUDIO_CLAUDE: 'C:\\tools\\claude.exe', CLAUDE_CONFIG_DIR: 'D:\\ch' }, '/ud', isFile, '/electron');
    expect(exe).toMatchObject({ program: { configured: true, path: 'C:\\tools\\claude.exe' }, command: 'C:\\tools\\claude.exe', prefix: [], home: 'D:\\ch', env: {} });
    const script = claudeConfig({ RCFD_STUDIO_CLAUDE: '/t/fake-claude.mjs', RCFD_STUDIO_CLAUDE_STATE: '/st' }, '/ud', isFile, '/electron');
    expect(script).toMatchObject({ command: '/electron', prefix: ['/t/fake-claude.mjs'], env: { ELECTRON_RUN_AS_NODE: '1' }, stateDir: '/st' });
    expect(claudeConfig({ RCFD_STUDIO_CLAUDE: '/nope/claude' }, '/ud', isFile, '/e').program.configured).toBe(false);
    expect(claudeConfig({}, '/ud', isFile, '/e').stateDir).toBe(path.join('/ud', 'claude'));
  });
});

// ---- the allow-list and the argument check

test.describe('the conservative allow-list (G11)', () => {
  test('the first list is exactly the read-class project tools of the app-owned server', () => {
    const l = AllowList.initial();
    expect(l.values).toEqual(PROJECT_READ_TOOLS.map((t) => `mcp__rcfd_project__${t}`));
    expect(l.values).toHaveLength(16);
    // nothing that edits, runs, cancels or deletes
    expect(l.values.filter((v) => /edit|create|start|cancel|delete|write|run_case|Bash|PowerShell/.test(v))).toEqual([]);
    expect(l.permissionArgs()).toEqual(['--permission-mode', 'dontAsk', '--permission-prompts', 'none', '--allowedTools', ...l.values]);
  });

  test('rules: bare shell and file tools, wildcard-only rules, control files and paths outside the project are refused', () => {
    for (const bad of ['Bash', 'Edit', 'Write', 'Read', 'WebFetch', 'Bash(*)', 'Bash(* foo)', 'Edit(*)', 'Edit(./.claude/settings.json)', 'Edit(CLAUDE.md)', 'Edit(.mcp.json)', 'Read(../secret)', 'Edit(/etc/passwd)', 'Edit(C:/x)', 'Bash(--dangerously-skip-permissions)', 'Edit(.rcfd/claude-allow.json)', 'Edit(*/x)', 'Edit(rcfd-project.json)', 'Edit(PROGRA~1/x)']) {
      expect(parseRule(bad), bad).toBeNull();
    }
    for (const good of ['Grep', 'mcp__rcfd_project__list_runs', 'Bash(git status)', 'Bash(cargo test *)', 'Edit(./notes/a.md)', 'Read(./runs/*/metrics.json)', 'Edit(./cases/**)']) {
      expect(parseRule(good), good).not.toBeNull();
    }
    expect(ruleProblem({ kind: 'tool', name: 'Bash' })).toContain('needs a pattern');
    expect(() => new AllowList().add({ kind: 'bash', pattern: '*' })).toThrow(/literal/);
  });

  test('ADR 0736: no allow-list holds a tool of the class delete, nor a whole server that would (the same list as the engine side)', async () => {
    // the engine's own list (crates/rcfd-mcp/tests/claude_allow/mod.rs) and the live tools of the class delete
    const rs = readFileSync(path.join(appRoot, '..', '..', 'crates', 'rcfd-mcp', 'tests', 'claude_allow', 'mod.rs'), 'utf8');
    const engine = /pub const DELETE_TOOLS: \[&str; \d+\] = \[([^\]]*)\]/.exec(rs)![1]!.match(/"([a-z_]+)"/g)!.map((q) => q.slice(1, -1));
    expect([...DELETE_TOOLS]).toEqual(engine);
    const inv = JSON.parse(readFileSync(path.join(appRoot, 'capabilities', 'inventory.json'), 'utf8')) as { capabilities: { id: string; access?: { project?: string } }[] };
    expect(inv.capabilities.filter((c) => c.access?.project === 'delete').map((c) => c.id.replace(/^mcp:/, ''))).toEqual([...DELETE_TOOLS]);
    // the first list holds none
    expect(AllowList.initial().values.filter((v) => isDeleteTool(v))).toEqual([]);
    expect(PROJECT_READ_TOOLS.filter((t) => (DELETE_TOOLS as readonly string[]).includes(t))).toEqual([]);
    for (const t of DELETE_TOOLS) {
      for (const name of [`mcp__rcfd_project__${t}`, `mcp__plugin_rcfd_project__${t}`, `mcp__other__${t}`]) {
        expect(ruleProblem({ kind: 'tool', name }), name).toContain('delete');
        expect(parseRule(name), name).toBeNull();
        expect(() => new AllowList().add({ kind: 'tool', name }), name).toThrow();
        // "Allow this from now on" offers nothing and writes nothing for it
        expect(suggest(name, {}, '/p'), name).toBeNull();
        expect('refused' in withRule(null, name), name).toBe(true);
      }
    }
    // a whole server is every tool of it, the delete class included
    for (const name of ['mcp__rcfd_project', 'mcp__plugin_rcfd_project']) expect(ruleProblem({ kind: 'tool', name }), name).toContain('delete');
    // a file holding one is refused as a whole
    const f = parseAllowFile(JSON.stringify({ version: 1, rules: ['mcp__rcfd_project__list_runs', 'mcp__rcfd_project__empty_trash'] }));
    expect(f.kind).not.toBe('list');
    // control: every other project tool of the inventory is still a valid rule
    const others = inv.capabilities.filter((c) => c.id.startsWith('mcp:') && c.access?.project !== undefined && c.access.project !== 'delete');
    expect(others.length).toBeGreaterThan(40);
    for (const c of others) expect(ruleProblem({ kind: 'tool', name: `mcp__rcfd_project__${c.id.slice(4)}` }), c.id).toBeNull();
  });

  test('never approve-everything: the argument check refuses every bypass, other modes and malformed rules', () => {
    const ok = chatArgs('--resume', S1, AllowList.initial(), '/cfg/mcp.json');
    expect(() => checkArgs(ok)).not.toThrow();
    const bad: string[][] = [
      [...ok, '--dangerously-skip-permissions'],
      [...ok, '--allow-dangerously-skip-permissions'],
      ok.map((a) => (a === 'dontAsk' ? 'bypassPermissions' : a)),
      ok.map((a) => (a === 'dontAsk' ? 'acceptEdits' : a)),
      ok.map((a) => (a === 'none' ? 'all' : a)),
      ok.filter((a, i) => !(a === '--permission-mode' || ok[i - 1] === '--permission-mode')),
      [...ok, '--permission-mode', 'dontAsk'],
      [...ok, '--permission-mode=dontAsk'],
      [...ok, '--allowedTools=Bash'],
      [...ok, '--allowedTools', 'Bash'],
      [...ok, '--allowedTools', 'Edit(./.claude/settings.json)'],
      [...ok, '--settings', '{}'],
      [...ok, '--add-dir', 'C:\\'],
      [...ok, '--permission-prompt-tool', 'mcp__x__approve'],
    ];
    for (const args of bad) expect(() => checkArgs(args), args.join(' ')).toThrow(ForbiddenArgument);
  });

  test('the MCP config must be one absolute file outside the project, with --strict-mcp-config', async () => {
    const fx = await claudeFixture('mcp');
    const project = path.join(fx.root, 'project');
    await mkdir(project, { recursive: true });
    const outside = path.join(fx.root, 'mcp.json');
    const inside = path.join(project, 'mcp.json');
    await writeFile(outside, '{"mcpServers":{}}');
    await writeFile(inside, '{"mcpServers":{}}');
    const l = AllowList.initial();
    await expect(checkArgsIn(chatArgs('--resume', S1, l, outside), project)).resolves.toBeUndefined();
    await expect(checkArgsIn(chatArgs('--resume', S1, l, inside), project)).rejects.toThrow(/inside the project/);
    await expect(checkArgsIn(chatArgs('--resume', S1, l, 'mcp.json'), project)).rejects.toThrow(/absolute/);
    await expect(checkArgsIn(chatArgs('--resume', S1, l, '{"mcpServers":{}}'), project)).rejects.toThrow(/inline JSON/);
    const noStrict = chatArgs('--resume', S1, l, outside).filter((a) => a !== '--strict-mcp-config');
    await expect(checkArgsIn(noStrict, project)).rejects.toThrow(/without --strict/);
  });

  test('the allow-list file: valid rules are used; one bad rule, an unknown key or another version refuses the whole file', () => {
    const good = parseAllowFile(JSON.stringify({ version: 1, rules: ['mcp__rcfd_project__list_runs', 'Bash(git status)', 'Edit(./notes/a.md)'] }));
    expect(good.kind).toBe('list');
    expect(good.kind === 'list' && good.list.values).toEqual(['mcp__rcfd_project__list_runs', 'Bash(git status)', 'Edit(./notes/a.md)']);
    for (const text of [
      JSON.stringify({ version: 1, rules: ['Grep', 'Bash'] }),
      JSON.stringify({ version: 1, rules: ['Grep'], mode: 'bypassPermissions' }),
      JSON.stringify({ version: 2, rules: [] }),
      JSON.stringify({ version: 1, rules: ['Edit(./.claude/settings.json)'] }),
      '{ not json',
    ]) {
      const r = parseAllowFile(text);
      expect(r.kind, text).toBe('refused');
    }
  });

  test('adding a rule (F-814): only a valid rule, never approve-everything, never twice, never to a refused file', () => {
    // no file: the Studio's first list is kept, then the rule
    const added = withRule(null, 'Bash(echo hello > made_by_bash.txt)');
    if ('refused' in added) throw new Error(added.refused);
    expect(added.rules).toEqual([...AllowList.initial().values, 'Bash(echo hello > made_by_bash.txt)']);
    expect(parseAllowFile(added.text)).toMatchObject({ kind: 'list' });
    expect(JSON.parse(added.text)).toEqual({ version: 1, rules: added.rules });
    // a file: its rules, then the rule
    const file = allowFileText(['Bash(git status)']);
    const more = withRule(file, 'Edit(./notes/plan.md)');
    expect('refused' in more ? more.refused : more.rules).toEqual(['Bash(git status)', 'Edit(./notes/plan.md)']);
    // the refusals of the file's own rules and of the spawn's argument check
    for (const bad of [
      'Bash',
      'Edit',
      'Write',
      'Bash(*)',
      'Bash(* --x)',
      'Edit(*)',
      'Edit(.rcfd/claude-allow.json)',
      'Edit(.claude/settings.json)',
      'Read(../secret)',
      'Bash(--dangerously-skip-permissions)',
      'bypassPermissions',
      'Bash(rm -rf /, Edit)',
      '--permission-mode',
      'Grep(x)',
    ]) {
      expect('refused' in withRule(null, bad), bad).toBe(true);
    }
    expect(withRule(file, 'Bash(git status)')).toEqual({ refused: 'Bash(git status) is already allowed by .rcfd/claude-allow.json' });
    const broken = withRule(JSON.stringify({ version: 1, rules: ['Bash'] }), 'Bash(git status)');
    expect('refused' in broken && broken.refused).toContain('correct the file before a rule is added');
    expect(allowFileHash(null)).toBeNull();
    expect(allowFileHash(file)).toMatch(/^[0-9a-f]{64}$/);
  });

  test('a denied call gets the narrowest exact rule, never a wildcard or a path outside the project', () => {
    const project = path.resolve('/proj/demo');
    expect(suggest('Bash', { command: 'echo hello > made_by_bash.txt' }, project)).toEqual({ kind: 'bash', pattern: 'echo hello > made_by_bash.txt' });
    expect(suggest('Bash', { command: 'rm *' }, project)).toBeNull();
    expect(suggest('Write', { file_path: path.join(project, 'notes', 'n.md') }, project)).toEqual({ kind: 'edit', pattern: 'notes/n.md' });
    expect(suggest('Write', { file_path: path.resolve('/elsewhere/n.md') }, project)).toBeNull();
    expect(suggest('Write', { file_path: path.join(project, 'CLAUDE.md') }, project)).toBeNull();
    expect(suggest('mcp__rcfd_project__start_run', {}, project)).toEqual({ kind: 'tool', name: 'mcp__rcfd_project__start_run' });
  });
});

// ---- the readers

test.describe('stream and transcript readers', () => {
  const canned = readFileSync(path.join(appRoot, 'tests', 'support', 'claude', 'reply.stream.jsonl'), 'utf8')
    .split('\n')
    .map((l) => l.replaceAll('{{SESSION}}', S1).replaceAll('{{MODE}}', 'dontAsk').replaceAll('{{CWD}}', 'C:\\\\proj'));

  test('the canned stream: init, text deltas only (not the repeated full message), a tool call, a denial, the result', () => {
    const evs = canned.flatMap(parseLine);
    expect(evs.map((e) => e.kind)).toEqual(['init', 'text', 'text', 'text', 'tool', 'tool', 'denied', 'result']);
    expect(evs[0]).toMatchObject({ kind: 'init', session: S1, mode: 'dontAsk', version: '2.1.310' });
    expect(evs.filter((e) => e.kind === 'text').map((e) => (e.kind === 'text' ? e.text : '')).join('')).toContain(RUN);
    expect(parseLine('not json')).toEqual([{ kind: 'unrecognised', type: null }]);
    expect(parseLine('{"type":"system","subtype":"hook_response"}')).toEqual([]);
  });

  test('a turn is checked by its system/init: dontAsk passes, any other mode or an action before it does not', () => {
    const [init] = parseLine(canned[0]!);
    expect(uncheckedProblem(init!)).toBeNull();
    expect(uncheckedProblem({ kind: 'init', session: S1, mode: 'default', version: null, model: null })).toMatch(/did not apply --permission-mode dontAsk/);
    expect(uncheckedProblem({ kind: 'tool', id: 't', name: 'Bash', input: {} })).toMatch(/before a checked system\/init/);
    expect(uncheckedProblem({ kind: 'text', text: 'x' })).toMatch(/before/);
  });

  test('transcripts: role, text, tools and timestamp only; bookkeeping skipped; unknown shapes kept as unrecognised', async () => {
    expect(parseEntry('{"type":"user","message":{"content":"hi"},"timestamp":"t"}')).toEqual({ kind: 'message', role: 'user', text: 'hi', tools: [], at: 't' });
    expect(parseEntry('{"type":"queue-operation"}')).toBeNull();
    expect(parseEntry('{"type":"brand-new"}')).toEqual({ kind: 'unrecognised', type: 'brand-new' });
    expect(parseEntry('{"type":"user","message":{"content":[{"type":"tool_result","content":"4 runs"}]}}')).toMatchObject({ role: 'tool_result', text: '4 runs' });
    expect(parseEntry('{"type":"assistant","message":{"content":[{"type":"thinking","thinking":"x"},{"type":"tool_use","name":"Grep"}]}}')).toMatchObject({ role: 'assistant', text: '', tools: ['Grep'] });
    const fx = await claudeFixture('tr');
    const file = await fx.transcript('C:\\proj', S1, sampleTranscript('C:\\proj', S1));
    const t = await tail(file, 50);
    expect(t.entries.map((e) => (e.kind === 'message' ? e.role : e.kind))).toEqual(['user', 'assistant']);
    // a writer in the middle of a line: only complete lines are read
    await appendFile(file, '{"type":"user","message":{"content":"half');
    const a = await readNew(file, t.offset);
    expect(a.entries).toEqual([]);
    expect(a.offset).toBe(t.offset);
    await appendFile(file, ' done"}}\n');
    const b = await readNew(file, a.offset);
    expect(b.entries).toEqual([{ kind: 'message', role: 'user', text: 'half done', tools: [], at: null }]);
  });
});

// ---- discovery

test.describe('session discovery', () => {
  test('open (listed with a live process), closed (not listed, or its process ended), unknown (the listing failed)', async () => {
    const fx = await claudeFixture('disc');
    const folder = path.join(fx.root, 'demo-project');
    const other = path.join(fx.root, 'demo_project'); // maps to the same transcript folder
    await mkdir(folder, { recursive: true });
    expect(projectDirName(folder)).toBe(projectDirName(other));
    await fx.transcript(folder, S1, sampleTranscript(folder, S1));
    await fx.transcript(folder, S2, sampleTranscript(folder, S2));
    await fx.transcript(other, '00000000-0000-4000-8000-000000000003', sampleTranscript(other, '00000000-0000-4000-8000-000000000003'));
    const agents = parseAgents(JSON.stringify([{ pid: process.pid, cwd: folder, sessionId: S2, name: 'terminal' }, { pid: deadPid(), cwd: folder, sessionId: S1 }]));
    const d = await classify([folder], fx.home, { ok: true, agents }, (pid) => pid === process.pid);
    expect(d.sessions.map((s) => [s.session, s.state.kind])).toEqual(
      expect.arrayContaining([
        [S1, 'closed'],
        [S2, 'open'],
      ]),
    );
    expect(d.sessions).toHaveLength(2); // the other folder's session is not listed
    expect(d.sessions.find((s) => s.session === S2)).toMatchObject({ name: 'terminal', title: 'Which runs converged?' });
    const u = await classify([folder], fx.home, { ok: false, reason: 'exit 2' }, () => true);
    expect(u.sessions.every((s) => s.state.kind === 'unknown')).toBe(true);
    expect(u.warnings.join(' ')).toContain('exit 2');
    expect(() => parseAgents('{}')).toThrow(/array/);
  });
});

// ---- the port

test.describe('the Claude port checks every message', () => {
  test('exact fields, a session that is null or a UUID, a bounded text, a hex token', () => {
    expect(parseClaudeRequest({ op: 'send', id: 1, session: null, text: 'hi' })).toMatchObject({ ok: true });
    expect(parseClaudeRequest({ op: 'send', id: 1, session: S1, text: 'hi' })).toMatchObject({ ok: true });
    expect(parseClaudeRequest({ op: 'select', id: 2, session: S1 })).toMatchObject({ ok: true });
    for (const bad of [
      { op: 'send', id: 1, session: '--dangerously-skip-permissions', text: 'hi' },
      { op: 'send', id: 1, session: S1, text: 'hi', argv: ['--x'] },
      { op: 'send', id: 1, session: S1, text: '' },
      { op: 'send', id: 1, session: S1, text: 'x'.repeat(20_001) },
      { op: 'send', id: 1, session: S1, text: 'hi', program: 'cmd.exe' },
      { op: 'select', id: 2, session: 'abc' },
      { op: 'confirm', id: 3, token: '../x' },
      { op: 'spawn', id: 4 },
      { op: 'state', id: -1 },
      'state',
      // F-814: allow takes exactly a rule and a base (null or a SHA-256), undoAllow exactly force
      { op: 'allow', id: 5, rule: 'Bash(git status)' },
      { op: 'allow', id: 5, rule: '', base: null },
      { op: 'allow', id: 5, rule: 'x'.repeat(2001), base: null },
      { op: 'allow', id: 5, rule: 'Bash(git status)', base: 'abc' },
      { op: 'allow', id: 5, rule: 'Bash(git status)', base: null, file: '.rcfd/claude-allow.json' },
      { op: 'allow', id: 5, rule: ['Bash'], base: null },
      { op: 'undoAllow', id: 6 },
      { op: 'undoAllow', id: 6, force: 'yes' },
    ]) {
      expect(parseClaudeRequest(bad).ok, JSON.stringify(bad)).toBe(false);
    }
    expect(isUuid(S1)).toBe(true);
    expect(parseClaudeRequest({ op: 'allow', id: 5, rule: 'Bash(git status)', base: null })).toMatchObject({ ok: true });
    expect(parseClaudeRequest({ op: 'allow', id: 5, rule: 'Bash(git status)', base: 'a'.repeat(64) })).toMatchObject({ ok: true });
    expect(parseClaudeRequest({ op: 'undoAllow', id: 6, force: false })).toMatchObject({ ok: true });
  });
});

// ---- links

test.describe('run ids and files in a reply become links', () => {
  const folder = 'C:\\Users\\x\\quick-start';
  const known = {
    runs: new Set([RUN, 'r-20261028T152641-5cc1']),
    files: new Set(['cases/cavity-re100.toml', 'cases/inlet.csv', 'notes/README.md', 'meshes/README.md']),
    cases: new Set(['cases/cavity-re100.toml']),
    folder,
  };
  test('only what the project holds links; each once, in order; relative, ./ and absolute paths', () => {
    const text = `Run **${RUN}** converged in 412 iterations. Its case is cases/cavity-re100.toml; see ./notes/README.md. Run r-29991231T000000-dead is not here, nor is cases/missing.toml. Again ${RUN}, and C:\\Users\\x\\quick-start\\meshes\\README.md and C:/users/x/quick-start/runs/r-20261028T152641-5cc1/metrics.json.`;
    expect(findRefs(text, known)).toEqual([
      { kind: 'run', id: RUN },
      { kind: 'file', path: 'cases/cavity-re100.toml' },
      { kind: 'file', path: 'notes/README.md' },
      { kind: 'file', path: 'meshes/README.md' },
      { kind: 'run', id: 'r-20261028T152641-5cc1' },
    ]);
    expect(findRefs('nothing to see: r-2026-11-16, cases/', known)).toEqual([]);
    expect(findRefs('cases/cavity-re100.toml.', known)).toEqual([{ kind: 'file', path: 'cases/cavity-re100.toml' }]);
  });
  test('a case file opens in the case editor; a data file, a mesh and a note on the project home', () => {
    expect(fileOpensIn('cases/cavity-re100.toml', known)).toBe('case');
    expect(fileOpensIn('cases/inlet.csv', known)).toBe('home');
    expect(fileOpensIn('meshes/README.md', known)).toBe('home');
    expect(fileOpensIn('notes/README.md', known)).toBe('home');
    expect(fileOpensIn('cases/cavity-re100.toml', { ...known, cases: undefined })).toBe('home');
  });
  test('the reply text: paragraphs, and bold only where the reply bolds', () => {
    expect(paragraphs('Run **412** done.\n\nNext **one**')).toEqual([
      [
        { bold: false, text: 'Run ' },
        { bold: true, text: '412' },
        { bold: false, text: ' done.' },
      ],
      [
        { bold: false, text: 'Next ' },
        { bold: true, text: 'one' },
      ],
    ]);
  });
});

// ---- the panel against the fake claude

interface Rig {
  fx: ClaudeFixture;
  folder: string;
  panel: ClaudePanel;
  states: ClaudeState[];
  config: ClaudeConfig;
}

async function rig(tag: string, env: Record<string, string> = {}, files: Record<string, string> = {}, discoverMs = 100): Promise<Rig> {
  const fx = await claudeFixture(tag);
  const folder = path.join(fx.root, 'project');
  for (const d of ['cases', 'notes', 'runs']) await mkdir(path.join(folder, d), { recursive: true });
  await writeFile(path.join(folder, 'rcfd-project.json'), '{"schema_version":1,"name":"Demo"}\n');
  for (const [rel, text] of Object.entries(files)) {
    await mkdir(path.dirname(path.join(folder, rel)), { recursive: true });
    await writeFile(path.join(folder, rel), text);
  }
  const config: ClaudeConfig = {
    program: { configured: true, path: FAKE_CLAUDE, from: 'RCFD_STUDIO_CLAUDE' },
    command: process.execPath,
    prefix: [FAKE_CLAUDE],
    env: fx.env(env),
    home: fx.home,
    stateDir: fx.state,
  };
  const panel = new ClaudePanel({ config, rcfdMcp: path.join(fx.root, 'rcfd-mcp.exe'), discoverMs, mirrorMs: 50 });
  const states: ClaudeState[] = [];
  panel.onState((s) => states.push(s));
  return { fx, folder, panel, states, config };
}

const view = (r: Rig) => r.panel.state.project!;
const settle = (r: Rig) => until(() => (r.panel.state.project?.writer === null && r.panel.state.project?.turn?.status !== 'running' ? true : null), 15_000, 'the turn to end');

test.describe('the panel (main process) against the fake claude', () => {
  test.describe.configure({ timeout: 60_000 });

  test('resume a closed session: claude -p --resume in the project folder, the message on stdin, the allow-list in argv', async () => {
    // the fake pauses 60 ms between its stream lines, twice the panel's 30 ms coalescing window, so a state carrying the
    // streamed "denied" line is pushed before the transcript replaces it (ADR 0801 Addendum 19, Q-F821-3); with lines
    // that arrive in one pipe read both happen inside one window
    const r = await rig('resume', { FAKE_CLAUDE_DELAY_MS: '60' });
    await r.fx.transcript(r.folder, S1, sampleTranscript(r.folder, S1));
    await r.panel.setProject(r.folder);
    await until(() => view(r).sessions.find((s) => s.id === S1 && s.open === 'closed'), 10_000, 'S1 listed closed');
    r.panel.select(S1);
    await until(() => view(r).feed.length === 2, 5000, 'the transcript tail');
    const sent = await r.panel.send(S1, 'Which run converged?');
    expect(sent.session).toBe(S1);
    await settle(r);
    const chats = await r.fx.chats();
    expect(chats).toHaveLength(1);
    const { argv, cwd, stdin } = chats[0]!;
    // the program is spawned with the project folder as its working folder; the message is on stdin, not in argv
    expect(path.resolve(cwd).toLowerCase()).toBe(path.resolve(r.folder).toLowerCase());
    expect(JSON.parse(stdin.trim())).toEqual({ type: 'user', message: { role: 'user', content: 'Which run converged?' } });
    expect(argv).not.toContain('Which run converged?');
    expect(argv.slice(argv.indexOf('--resume'))).toEqual(['--resume', S1]);
    expect(argv).not.toContain('--session-id');
    // the allow-list, recorded: dontAsk, prompts off, exactly the 16 read-class project tools
    const i = argv.indexOf('--allowedTools');
    expect(argv.slice(argv.indexOf('--permission-mode'), i)).toEqual(['--permission-mode', 'dontAsk', '--permission-prompts', 'none']);
    expect(argv.slice(i + 1, argv.indexOf('--resume'))).toEqual(AllowList.initial().values);
    expect(argv.filter((a) => APPROVE_EVERYTHING.some((re) => re.test(a)))).toEqual([]);
    // the app-owned MCP config: outside the project, one server, the engine's project server on this folder
    const mcp = argv[argv.indexOf('--mcp-config') + 1]!;
    expect(argv).toContain('--strict-mcp-config');
    expect(mcp.startsWith(r.fx.state)).toBe(true);
    const cfg = JSON.parse(await readFile(mcp, 'utf8')) as { mcpServers: Record<string, { args: string[] }> };
    expect(Object.keys(cfg.mcpServers)).toEqual(['rcfd_project']);
    expect(cfg.mcpServers['rcfd_project']!.args).toEqual(['--project', r.folder, '--root', r.folder]);
    // the reply streamed into the feed, then the transcript's lines replaced it
    const v = view(r);
    expect(v.turn).toMatchObject({ status: 'finished', mode: 'dontAsk', exitCode: 0 });
    await until(() => view(r).feed.filter((f) => f.kind === 'user').length === 2 && !view(r).feed.some((f) => f.provisional), 5000, 'the transcript lines of the turn');
    const reply = view(r).feed.filter((f) => f.kind === 'assistant').at(-1);
    expect(reply).toMatchObject({ kind: 'assistant', provisional: false });
    expect(reply!.text).toContain(RUN);
    // F-814: the turn's denied call stays after the transcript replaced the stream (the transcript has no rule for it)
    expect(view(r).feed.at(-1)).toMatchObject({ kind: 'denied', provisional: false, suggestion: 'Bash(echo hello > made_by_bash.txt)' });
    // the denied Bash call showed with its narrowest rule while the turn streamed
    expect(r.states.some((s) => s.project?.feed.some((f) => f.kind === 'denied' && f.suggestion === 'Bash(echo hello > made_by_bash.txt)'))).toBe(true);
    await r.panel.setProject(null);
  });

  test('mirror read-only: a session another process has open is shown and never written to', async () => {
    const r = await rig('mirror');
    const file = await r.fx.transcript(r.folder, S2, sampleTranscript(r.folder, S2));
    await r.fx.setAgents([{ pid: process.pid, cwd: r.folder, sessionId: S2, name: 'terminal' }]);
    await r.panel.setProject(r.folder);
    const row = await until(() => view(r).sessions.find((s) => s.id === S2 && s.open === 'open'), 10_000, 'S2 listed open');
    expect(row.readOnly).toContain(`open in process ${process.pid}`);
    r.panel.select(S2);
    await until(() => view(r).feed.length === 2, 5000, 'the mirror');
    // the other process writes: the mirror follows
    await appendFile(file, JSON.stringify({ type: 'user', cwd: r.folder, message: { role: 'user', content: 'typed in the terminal' } }) + '\n');
    await until(() => view(r).feed.some((f) => f.text === 'typed in the terminal'), 5000, 'the new line');
    // and the Studio does not write to it
    const e = await r.panel.send(S2, 'hello').catch((x: unknown) => x);
    expect(e).toBeInstanceOf(ClaudeRefusal);
    expect((e as ClaudeRefusal).code).toBe('read_only');
    expect(await r.fx.chats()).toEqual([]);
    expect(view(r).writer).toBeNull();
    await r.panel.setProject(null);
  });

  test('opened by another process after the last listing: the check right before the spawn refuses it', async () => {
    // a listing period longer than the test, so only the check before the spawn can see the change
    const r = await rig('late', {}, {}, 600_000);
    await r.fx.transcript(r.folder, S1, sampleTranscript(r.folder, S1));
    await r.panel.setProject(r.folder);
    expect(view(r).sessions.find((s) => s.id === S1)?.open).toBe('closed');
    // another process opens it, in another folder (the session is the unit of the one-writer rule)
    await r.fx.setAgents([{ pid: process.pid, cwd: path.join(r.fx.root, 'elsewhere'), sessionId: S1 }]);
    const late = (await r.panel.send(S1, 'hello').catch((x: unknown) => x)) as ClaudeRefusal;
    expect(late.code).toBe('read_only');
    expect(late.message).toContain(`open in process ${process.pid}`);
    expect(await r.fx.chats()).toEqual([]);
    expect(view(r).writer).toBeNull();
    expect(view(r).turn).toMatchObject({ status: 'refused' });
    await r.panel.setProject(null);
  });

  test('a session whose state cannot be established is not written to', async () => {
    const r = await rig('unknown', { FAKE_CLAUDE_AGENTS_FAIL: '1' });
    await r.fx.transcript(r.folder, S1, sampleTranscript(r.folder, S1));
    await r.panel.setProject(r.folder);
    const row = await until(() => view(r).sessions.find((s) => s.id === S1), 10_000, 'S1 listed');
    expect(row.open).toBe('unknown');
    expect(row.readOnly).toContain('cannot be established');
    const e = (await r.panel.send(S1, 'hello').catch((x: unknown) => x)) as ClaudeRefusal;
    expect(e.code).toBe('read_only');
    expect(await r.fx.chats()).toEqual([]);
    await r.panel.setProject(null);
  });

  test('one writer per session: the lock is held for the whole turn, and a second message is refused', async () => {
    const r = await rig('writer', { FAKE_CLAUDE_HOLD_MS: '1500' });
    await r.fx.transcript(r.folder, S1, sampleTranscript(r.folder, S1));
    await r.fx.transcript(r.folder, S2, sampleTranscript(r.folder, S2));
    await r.panel.setProject(r.folder);
    await until(() => view(r).sessions.filter((s) => s.open === 'closed').length === 2, 10_000, 'both closed');
    const first = r.panel.send(S1, 'one');
    // the lock is taken at once, before any check that waits
    expect(view(r).writer).toMatchObject({ session: S1 });
    const second = (await r.panel.send(S1, 'two').catch((x: unknown) => x)) as ClaudeRefusal;
    expect(second.code).toBe('busy');
    expect(second.message).toContain('one writer per session');
    await first;
    const w = await until(() => (view(r).writer?.pid ? view(r).writer : null), 5000, 'the writer pid');
    expect(view(r).sessions.find((s) => s.id === S1)).toMatchObject({ open: 'studio', pid: w.pid });
    expect(((await r.panel.send(S1, 'three').catch((x: unknown) => x)) as ClaudeRefusal).code).toBe('busy');
    expect(((await r.panel.send(S2, 'other').catch((x: unknown) => x)) as ClaudeRefusal).code).toBe('busy');
    expect(() => r.panel.select(S2)).toThrow(/being answered/);
    // a running `claude -p` is listed by `claude agents --json`, so another Studio (or terminal) sees the session open
    const listedPid = () => {
      const listed = spawnSync(process.execPath, [FAKE_CLAUDE, 'agents', '--json'], { env: { ...process.env, ...r.fx.env() }, encoding: 'utf8' });
      return parseAgents(listed.stdout).find((a) => a.session === S1)?.pid ?? null;
    };
    expect(await until(listedPid, 5000, 'the running claude -p listed')).toBe(w.pid);
    await settle(r);
    expect(view(r).writer).toBeNull();
    expect((await r.fx.chats()).map((c) => c.stdin.includes('"one"'))).toEqual([true]);
    // the lock is free again: the next message goes
    await r.panel.send(S1, 'four');
    await settle(r);
    expect(await r.fx.chats()).toHaveLength(2);
    await r.panel.setProject(null);
  });

  test('a new chat: --session-id with an id chosen by the Studio, and its transcript then mirrored', async () => {
    const r = await rig('new');
    await r.panel.setProject(r.folder);
    const { session } = await r.panel.send(null, 'Hello');
    expect(isUuid(session)).toBe(true);
    expect(view(r).selected).toBe(session);
    await settle(r);
    const [c] = await r.fx.chats();
    expect(c!.argv.slice(-2)).toEqual(['--session-id', session]);
    await until(() => view(r).sessions.find((s) => s.id === session && s.open === 'closed'), 10_000, 'the new session listed');
    // the transcript's two lines, and the turn's denied call kept after them (F-814)
    await until(() => view(r).feed.length === 3 && view(r).feed.every((f) => !f.provisional), 5000, 'the transcript of the new session');
    expect(view(r).feed.map((f) => f.kind)).toEqual(['user', 'assistant', 'denied']);
  });

  test('a turn whose CLI did not apply dontAsk is stopped at once', async () => {
    const r = await rig('abort', { FAKE_CLAUDE_MODE: 'default', FAKE_CLAUDE_DELAY_MS: '50' });
    await r.panel.setProject(r.folder);
    await r.panel.send(null, 'Hello');
    await settle(r);
    expect(view(r).turn).toMatchObject({ status: 'aborted', mode: 'default' });
    expect(view(r).turn!.message).toContain('did not apply --permission-mode dontAsk');
    expect(view(r).feed.some((f) => f.kind === 'assistant')).toBe(false);
  });

  test('stop: the process is killed and the turn says so', async () => {
    const r = await rig('stop', { FAKE_CLAUDE_HOLD_MS: '20000' });
    await r.panel.setProject(r.folder);
    await r.panel.send(null, 'Hello');
    await until(() => view(r).feed.some((f) => f.kind === 'assistant'), 5000, 'text');
    const pid = view(r).writer!.pid!;
    r.panel.cancel();
    await settle(r);
    expect(view(r).turn).toMatchObject({ status: 'cancelled' });
    await until(() => !isAlive(pid), 5000, 'the claude process to be gone');
  });

  test('the folder’s Claude Code control files must be confirmed; a change asks again; the record is outside the project', async () => {
    const r = await rig('trust', {}, { 'CLAUDE.md': '# instructions\n', '.claude/settings.json': '{"hooks":{}}' });
    await r.panel.setProject(r.folder);
    const t = view(r).trust;
    expect(t.state).toBe('pending');
    if (t.state !== 'pending') throw new Error('pending expected');
    expect(t.files.map((f) => f.path)).toEqual(['.claude/settings.json', 'CLAUDE.md']);
    expect(((await r.panel.send(null, 'hi').catch((x: unknown) => x)) as ClaudeRefusal).code).toBe('not_confirmed');
    expect(view(r).writer).toBeNull();
    // a stale token is refused
    await writeFile(path.join(r.folder, 'CLAUDE.md'), '# changed\n');
    expect(((await r.panel.confirm(t.token).catch((x: unknown) => x)) as ClaudeRefusal).code).toBe('stale');
    const t2 = view(r).trust;
    if (t2.state !== 'pending') throw new Error('pending expected');
    await r.panel.confirm(t2.token);
    expect(view(r).trust).toEqual({ state: 'confirmed', files: 2 });
    expect((await scan(r.folder)).map((f) => f.path)).toEqual(['.claude/settings.json', 'CLAUDE.md']); // nothing was written into the folder
    await r.panel.send(null, 'hi');
    await settle(r);
    expect(await r.fx.chats()).toHaveLength(1);
    await writeFile(path.join(r.folder, '.claude', 'settings.json'), '{"hooks":{"x":1}}');
    expect(((await r.panel.send(null, 'again').catch((x: unknown) => x)) as ClaudeRefusal).code).toBe('not_confirmed');
    const t3 = view(r).trust;
    expect(t3.state === 'pending' && t3.changed).toEqual(['.claude/settings.json']);
  });

  test('a refused allow-list file starts no chat; a valid one is passed rule by rule', async () => {
    const bad = await rig('allow-bad', {}, { '.rcfd/claude-allow.json': JSON.stringify({ version: 1, rules: ['Grep', 'Bash'] }) });
    await bad.panel.setProject(bad.folder);
    const t = view(bad).trust;
    if (t.state === 'pending') await bad.panel.confirm(t.token);
    expect(view(bad).allow.error).toContain('needs a pattern');
    expect(((await bad.panel.send(null, 'hi').catch((x: unknown) => x)) as ClaudeRefusal).code).toBe('refused');
    expect(await bad.fx.chats()).toEqual([]);
    const good = await rig('allow-good', {}, { '.rcfd/claude-allow.json': JSON.stringify({ version: 1, rules: ['mcp__rcfd_project__list_runs', 'Bash(git status)'] }) });
    await good.panel.setProject(good.folder);
    const g = view(good).trust;
    if (g.state === 'pending') await good.panel.confirm(g.token);
    await good.panel.send(null, 'hi');
    await settle(good);
    const [c] = await good.fx.chats();
    const i = c!.argv.indexOf('--allowedTools');
    expect(c!.argv.slice(i + 1, c!.argv.indexOf('--session-id'))).toEqual(['mcp__rcfd_project__list_runs', 'Bash(git status)']);
  });

  test('Allow this from now on (F-814): the suggested rule is written to the file, conflict-safe, then used; undo takes it out', async () => {
    const RULE = 'Bash(echo hello > made_by_bash.txt)';
    const r = await rig('allow-add');
    await r.panel.setProject(r.folder);
    expect(view(r).trust).toEqual({ state: 'confirmed', files: 0 });
    expect(view(r).allow).toMatchObject({ source: 'initial', hash: null, undo: null });
    // nothing suggested yet: no rule can be added
    expect(((await r.panel.allowRule(RULE, null).catch((x: unknown) => x)) as ClaudeRefusal).code).toBe('bad_request');
    await r.panel.send(null, 'hi');
    await settle(r);
    await until(() => (view(r).feed.some((f) => f.kind === 'denied' && f.suggestion === RULE && !f.provisional) ? true : null), 5000, 'the denial kept');
    // a rule other than the suggestion is refused, even a valid one
    expect(((await r.panel.allowRule('Bash(git status)', null).catch((x: unknown) => x)) as ClaudeRefusal).code).toBe('bad_request');
    // a base that is not the file's: stale, nothing written
    expect(((await r.panel.allowRule(RULE, 'a'.repeat(64)).catch((x: unknown) => x)) as ClaudeRefusal).code).toBe('stale');
    expect(await readAllowText(r.folder)).toBeNull();
    await r.panel.allowRule(RULE, null);
    const text = await readAllowText(r.folder);
    expect(JSON.parse(text!)).toEqual({ version: 1, rules: [...AllowList.initial().values, RULE] });
    expect(view(r).allow).toMatchObject({ source: 'file', hash: allowFileHash(text), undo: RULE });
    // the Studio's own write keeps the folder confirmed; the record is outside the project
    expect(view(r).trust).toEqual({ state: 'confirmed', files: 1 });
    // the next chat passes the rule
    await r.panel.send(null, 'again');
    await settle(r);
    const chats = await r.fx.chats();
    expect(chats[1]!.argv).toContain(RULE);
    expect(chats[1]!.argv.filter((a) => APPROVE_EVERYTHING.some((re) => re.test(a)))).toEqual([]);
    // another writer changes the file: undo is refused (stale) and the file is kept; force puts back the first list
    const theirs = allowFileText([...AllowList.initial().values, RULE, 'Bash(git status)']);
    await writeFile(path.join(r.folder, '.rcfd', 'claude-allow.json'), theirs);
    expect(((await r.panel.undoAllow(false).catch((x: unknown) => x)) as ClaudeRefusal).code).toBe('stale');
    expect(await readAllowText(r.folder)).toBe(theirs);
    await r.panel.undoAllow(true);
    // the other writer's change was a change of a control file the Studio did not make: it still asks before a chat
    expect(view(r).trust.state).toBe('pending');
    expect(await readAllowText(r.folder)).toBe(allowFileText(AllowList.initial().values));
    expect(view(r).allow).toMatchObject({ source: 'file', rules: AllowList.initial().values, undo: null });
    expect(((await r.panel.undoAllow(false).catch((x: unknown) => x)) as ClaudeRefusal).code).toBe('bad_request');
    await r.panel.setProject(null);
  });

  test('Allow this from now on: refused while the folder’s control files are not confirmed, or the file is refused', async () => {
    const RULE = 'Bash(echo hello > made_by_bash.txt)';
    const r = await rig('allow-trust');
    await r.panel.setProject(r.folder);
    await r.panel.send(null, 'hi');
    await settle(r);
    await until(() => (view(r).feed.some((f) => f.kind === 'denied' && f.suggestion === RULE) ? true : null), 5000, 'the denial');
    await writeFile(path.join(r.folder, 'CLAUDE.md'), '# new instructions\n');
    expect(((await r.panel.allowRule(RULE, null).catch((x: unknown) => x)) as ClaudeRefusal).code).toBe('not_confirmed');
    expect(await readAllowText(r.folder)).toBeNull();
    const t = view(r).trust;
    if (t.state === 'pending') await r.panel.confirm(t.token);
    // a refused file is not added to
    await mkdir(path.join(r.folder, '.rcfd'), { recursive: true });
    const bad = JSON.stringify({ version: 1, rules: ['Bash'] });
    await writeFile(path.join(r.folder, '.rcfd', 'claude-allow.json'), bad);
    await r.panel.refresh();
    const t2 = view(r).trust;
    if (t2.state === 'pending') await r.panel.confirm(t2.token);
    expect(((await r.panel.allowRule(RULE, allowFileHash(bad)).catch((x: unknown) => x)) as ClaudeRefusal).code).toBe('refused');
    expect(await readAllowText(r.folder)).toBe(bad);
    await r.panel.setProject(null);
  });

  test('not configured: sessions are still listed from their transcripts, read-only, and nothing is started', async () => {
    const r = await rig('off');
    const panel = new ClaudePanel({
      config: { ...r.config, program: { configured: false, reason: 'not told' }, command: null, prefix: [] },
      rcfdMcp: null,
      discoverMs: 100,
      mirrorMs: 50,
    });
    await r.fx.transcript(r.folder, S1, sampleTranscript(r.folder, S1));
    await panel.setProject(r.folder);
    const row = await until(() => panel.state.project!.sessions.find((s) => s.id === S1), 5000, 'S1');
    expect(row.open).toBe('unknown');
    expect(((await panel.send(null, 'hi').catch((x: unknown) => x)) as ClaudeRefusal).code).toBe('not_configured');
    expect(await r.fx.starts()).toEqual([]);
    await panel.setProject(null);
  });
});

// ---- the stylesheet

test('claude.css uses only defined tokens and no colour literals or rounded corners', () => {
  const css = readFileSync(path.join(appRoot, 'src', 'renderer', 'claude', 'claude.css'), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
  const defined = new Set(cssCustomProperties().map(([k]) => k));
  const used = [...css.matchAll(/var\((--[a-z0-9-]+)/g)].map((m) => m[1]!);
  expect(used.length).toBeGreaterThan(0);
  expect(used.filter((v) => !defined.has(v))).toEqual([]);
  expect(css.match(/#[0-9a-f]{3,8}\b|rgba?\(|hsla?\(|oklch\(/gi) ?? []).toEqual([]);
  const radii = [...css.matchAll(/border-radius:\s*([^;]+)/g)].map((m) => m[1]!.trim());
  expect(radii.filter((v) => v !== 'var(--radius-none)' && v !== '0')).toEqual([]);
  expect(css).toContain('background: var(--role-claude-head)'); // the head, DESIGN.md's claude-head role (meadow)
  expect(css).not.toContain('var(--color-meadow)');
});
