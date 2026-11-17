// Run sites in the main process (F-848, ADR 0739 phase 1; src/shared/sites.ts): the engine's `rcfd site list --json`
// and `rcfd site check <name> [--confirm]`, the remote start (`rcfd-runner start ... --site <name> --no-runner`), the
// Where choice the Studio remembers per project in its user data, and the link state of each remote run read from the
// files the local runner writes into its folder.
//
// While the sites file is absent no engine program runs and no file is written: the Studio at rest with no sites file
// looks and acts as before F-848 (P9, G22). The one read it adds is each listed run's run.json `site`, once per run and
// session, so that a remote run keeps its badge when its site, or the whole file, is gone (P7); a local run adds nothing
// to the view. The `rcfd` and `rcfd-runner` are the ones beside the engine's project server, never from PATH (F-805
// D3). No Electron import: the unit tests drive it with fakes.
import { spawn } from 'node:child_process';
import { existsSync } from 'node:fs';
import { mkdir, open, readFile, rename, writeFile } from 'node:fs/promises';
import path from 'node:path';
import {
  LOCAL,
  isSiteName,
  linkSettled,
  NO_SITES,
  parseSiteCheck,
  parseSiteList,
  runSiteOf,
  type RunSite,
  type SiteCheckResult,
  type SitesState,
} from '../../shared/sites.js';
import { TERMINAL_STATES } from './runs.js';

type Env = Readonly<Record<string, string | undefined>>;

/**
 * The sites file the engine reads (crates/rcfd-runner/src/site/sites.rs sites_path): RCFD_SITES_FILE, else
 * %APPDATA%\rcfd\sites.toml on Windows, $XDG_CONFIG_HOME/rcfd/sites.toml or ~/.config/rcfd/sites.toml elsewhere; null
 * without a home.
 */
export function sitesFilePath(env: Env = process.env, platform: NodeJS.Platform = process.platform): string | null {
  const named = env['RCFD_SITES_FILE'];
  if (named) return named;
  const p = platform === 'win32' ? path.win32 : path.posix;
  let base: string | undefined;
  if (platform === 'win32') base = env['APPDATA'] || undefined;
  else base = env['XDG_CONFIG_HOME'] || (env['HOME'] ? p.join(env['HOME'], '.config') : undefined);
  return base ? p.join(base, 'rcfd', 'sites.toml') : null;
}

/** One engine command's answer: its exit code (null when it did not end or start), standard output and error. */
export interface CliAnswer {
  readonly code: number | null;
  readonly out: string;
  readonly err: string;
  /** Why it could not be started or did not end. */
  readonly failure: string | null;
}

/**
 * Runs `exe args` with the null device as its standard input and resolves with its answer (never rejects). A program
 * that leaves a child holding its streams (Windows handle inheritance, ADR 0146) is answered 2 s after its exit.
 */
export function engineCommand(exe: string, args: readonly string[], cwd: string | undefined, timeoutMs: number): Promise<CliAnswer> {
  return new Promise((resolve) => {
    let out = '';
    let err = '';
    let done = false;
    let exitCode: number | null = null;
    let afterExit: NodeJS.Timeout | null = null;
    const finish = (failure: string | null) => {
      if (done) return;
      done = true;
      clearTimeout(timer);
      if (afterExit) clearTimeout(afterExit);
      child.stdout?.destroy();
      child.stderr?.destroy();
      resolve({ code: exitCode, out, err, failure });
    };
    const child = spawn(exe, [...args], { ...(cwd ? { cwd } : {}), stdio: ['ignore', 'pipe', 'pipe'], windowsHide: true });
    child.stdout!.on('data', (d: Buffer) => {
      out = (out + d.toString('utf8')).slice(-1_048_576);
    });
    child.stderr!.on('data', (d: Buffer) => {
      err = (err + d.toString('utf8')).slice(-8192);
    });
    child.once('error', (e) => finish(`${exe} could not be started (${e.message})`));
    child.once('exit', (code) => {
      exitCode = code;
      afterExit = setTimeout(() => finish(null), 2000);
    });
    child.once('close', (code) => {
      exitCode = code;
      finish(null);
    });
    const timer = setTimeout(() => {
      child.kill();
      finish(`${path.basename(exe)} ${args.slice(0, 2).join(' ')} did not end within ${timeoutMs / 1000} s`);
    }, timeoutMs);
  });
}

/** The engine's own message: the last lines of standard error, without its `error: ` prefix. */
export const engineWords = (a: CliAnswer, what: string): string =>
  a.failure ??
  (a.err
    .trim()
    .split(/\r?\n/)
    .slice(-6)
    .join(' ')
    .replace(/^error:\s*/, '') ||
    `${what} ended with exit code ${a.code}`);

// ---- the Where choice, per project, in the Studio's user data (Q8: never in the project)

const CHOICES_FORMAT = 'rcfd-studio-site-choices';

/** The key of a project folder: Windows and macOS file systems ignore letter case by default. */
const folderKey = (folder: string, platform: string = process.platform): string => {
  const n = path.resolve(folder).replace(/[\\/]+$/, '');
  return platform === 'win32' || platform === 'darwin' ? n.toLowerCase() : n;
};

/** The last Where of each project: site names only, by the project's folder. */
export class SiteChoices {
  constructor(private readonly file: string) {}

  private async load(): Promise<Record<string, string>> {
    try {
      const v = JSON.parse(await readFile(this.file, 'utf8')) as { format?: unknown; projects?: unknown };
      if (v?.format !== CHOICES_FORMAT || typeof v.projects !== 'object' || v.projects === null) return {};
      const out: Record<string, string> = {};
      for (const [k, s] of Object.entries(v.projects as Record<string, unknown>)) if (isSiteName(s)) out[k] = s;
      return out;
    } catch {
      return {};
    }
  }

  /** The project's last Where; LOCAL when none was kept. */
  async get(folder: string): Promise<string> {
    return (await this.load())[folderKey(folder)] ?? LOCAL;
  }

  /**
   * Keeps `site` as the project's Where. LOCAL removes the project's entry, and a LOCAL choice of a project that has
   * none writes nothing (so a Studio that never used a site never writes this file).
   */
  async set(folder: string, site: string): Promise<void> {
    const all = await this.load();
    const key = folderKey(folder);
    if (site === LOCAL) {
      if (!(key in all)) return;
      delete all[key];
    } else {
      if (!isSiteName(site) || all[key] === site) return;
      all[key] = site;
    }
    await mkdir(path.dirname(this.file), { recursive: true });
    const tmp = `${this.file}.${process.pid}.tmp`;
    await writeFile(tmp, JSON.stringify({ format: CHOICES_FORMAT, version: 1, projects: all }, null, 2) + '\n');
    await rename(tmp, this.file);
  }
}

// ---- the engine's site commands

export interface SitesDeps {
  /** The engine's `rcfd` (beside the project server). */
  readonly rcfd: string;
  /** The engine's `rcfd-runner` (beside the project server). */
  readonly runner: string;
  readonly choices: SiteChoices | null;
  /** Opens a file in the system's program for it; resolves with an error message, empty when it opened. */
  openPath?(file: string): Promise<string>;
  env?: Env;
  /** How long a check may take, ms (the first ssh connection to a site can be slow). */
  checkTimeoutMs?: number;
}

export class Sites {
  constructor(private readonly deps: SitesDeps) {}

  /** The sites file's path, and whether it is there (nothing else is read). */
  file(): { readonly file: string | null; readonly exists: boolean } {
    const file = sitesFilePath(this.deps.env ?? process.env);
    return { file, exists: file !== null && existsSync(file) };
  }

  /** The sites as the engine lists them (no process when the file is absent), and the project's remembered Where. */
  async list(project: string | null): Promise<SitesState> {
    const { file, exists } = this.file();
    if (!exists) return { ...NO_SITES, file };
    const choice = project && this.deps.choices ? await this.deps.choices.get(project) : LOCAL;
    if (!existsSync(this.deps.rcfd)) return { file, exists, sites: [], error: `there is no ${path.basename(this.deps.rcfd)} beside the engine's project server`, choice };
    const a = await engineCommand(this.deps.rcfd, ['site', 'list', '--json'], undefined, 15_000);
    if (a.code !== 0) return { file, exists, sites: [], error: engineWords(a, 'rcfd site list'), choice };
    try {
      return { file, exists, sites: parseSiteList(JSON.parse(a.out)), error: null, choice };
    } catch {
      return { file, exists, sites: [], error: 'rcfd site list printed no JSON', choice };
    }
  }

  /**
   * `rcfd site check <name>`, with `--confirm` the user's yes (the engine writes it into the sites file). Starts the
   * site's agent once; sends no project data. Exit 1 with a report means the site may not take a run now.
   */
  async check(name: string, confirm: boolean): Promise<SiteCheckResult> {
    if (!isSiteName(name) || name === LOCAL) return { ok: false, message: `“${name}” is not a site name (letters, digits, _ and -).` };
    if (!this.file().exists) return { ok: false, message: 'There is no sites file, so there is no site to check.' };
    const a = await engineCommand(this.deps.rcfd, ['site', 'check', name, ...(confirm ? ['--confirm'] : [])], undefined, this.deps.checkTimeoutMs ?? 120_000);
    if (a.code === 0 || a.code === 1) {
      try {
        const v = JSON.parse(a.out) as unknown;
        if (typeof v === 'object' && v !== null) return { ok: true, check: parseSiteCheck(v) };
      } catch {
        // no report: the engine's message
      }
    }
    return { ok: false, message: engineWords(a, `rcfd site check ${name}`) };
  }

  /** Opens the sites file in the system's editor; the message says why it did not open (empty when it did). */
  async open(): Promise<string> {
    const { file, exists } = this.file();
    if (!file || !exists) return 'There is no sites file to open.';
    if (!this.deps.openPath) return 'This Studio cannot open files in another program.';
    return this.deps.openPath(file);
  }

  /**
   * `rcfd-runner start <case> [options] --set ... --site <name> --no-runner --project <folder>`: the run is queued
   * here with `site` in its run.json (the engine admits the site first: confirmed, and its engine this machine's or one
   * the user said yes to). The runner is started afterwards by the caller (rcfd-runner ensure), so no runner inherits
   * this command's streams. Resolves with the new run id or the engine's refusal.
   */
  async start(folder: string, argv: readonly string[]): Promise<{ readonly ok: true; readonly run: string } | { readonly ok: false; readonly message: string }> {
    if (!existsSync(this.deps.runner)) return { ok: false, message: `there is no ${path.basename(this.deps.runner)} beside the engine's project server` };
    const a = await engineCommand(this.deps.runner, [...argv, '--no-runner', '--project', folder], folder, this.deps.checkTimeoutMs ?? 120_000);
    const id = a.out.trim().split(/\r?\n/).pop()?.trim() ?? '';
    if (a.code === 0 && /^r-[A-Za-z0-9T-]+$/.test(id)) return { ok: true, run: id };
    return { ok: false, message: engineWords(a, 'rcfd-runner start --site') };
  }

  async remember(folder: string, site: string): Promise<void> {
    try {
      await this.deps.choices?.set(folder, site);
    } catch {
      // a convenience: a user data folder that cannot be written does not stop a run
    }
  }
}

// ---- the link state of remote runs, from their folders

/** Bytes of a run's events.jsonl read from its end to find its last state change. */
const EVENTS_TAIL = 16 * 1024;
/** Bytes of site.jsonl read from its end. */
const SITE_LOG_TAIL = 64 * 1024;

async function tail(file: string, bytes: number): Promise<string | null> {
  let h;
  try {
    h = await open(file, 'r');
  } catch {
    return null;
  }
  try {
    const size = (await h.stat()).size;
    const from = Math.max(0, size - bytes);
    const buf = Buffer.alloc(size - from);
    await h.read(buf, 0, buf.length, from);
    const text = buf.toString('utf8');
    // a cut first line is not a line
    return from > 0 ? text.slice(text.indexOf('\n') + 1) : text;
  } finally {
    await h.close();
  }
}

const lines = (text: string | null): Record<string, unknown>[] => {
  const out: Record<string, unknown>[] = [];
  for (const l of (text ?? '').split(/\r?\n/)) {
    if (!l.trim()) continue;
    try {
      const v = JSON.parse(l) as unknown;
      if (typeof v === 'object' && v !== null && !Array.isArray(v)) out.push(v as Record<string, unknown>);
    } catch {
      // a line being written
    }
  }
  return out;
};

const readJson = async (file: string): Promise<Record<string, unknown> | null> => {
  try {
    const v = JSON.parse(await readFile(file, 'utf8')) as unknown;
    return typeof v === 'object' && v !== null && !Array.isArray(v) ? (v as Record<string, unknown>) : null;
  } catch {
    return null;
  }
};

/** run.json's `site` of a run (null for a local run, or a record that does not read). */
export async function recordedSite(folder: string, run: string): Promise<string | null> {
  const j = await readJson(path.join(folder, 'runs', run, 'run.json'));
  return j && isSiteName(j['site']) ? (j['site'] as string) : null;
}

/** The link state of one remote run, from site.json, site.jsonl, the tail of events.jsonl and status.json. */
export async function readRunSite(folder: string, run: string, site: string): Promise<RunSite> {
  const dir = path.join(folder, 'runs', run);
  const [siteJson, siteLog, events, status] = await Promise.all([
    readJson(path.join(dir, 'site.json')),
    tail(path.join(dir, 'site.jsonl'), SITE_LOG_TAIL),
    tail(path.join(dir, 'events.jsonl'), EVENTS_TAIL),
    readJson(path.join(dir, 'status.json')),
  ]);
  const endedOnSite = lines(events).some((e) => (e['kind'] ?? e['event']) === 'state' && TERMINAL_STATES.has(String(e['to'] ?? e['state'] ?? '')));
  const endedHere = TERMINAL_STATES.has(String(status?.['state'] ?? ''));
  return runSiteOf({ site, siteJson, events: lines(siteLog), endedOnSite, endedHere });
}

/**
 * What a project session reads of its runs' sites, after each read of the run list: run.json's `site` once per run, and
 * the link state of each remote run until its link is done. Nothing while `enabled` says no and no remote run was seen.
 */
export class RunSites {
  /** run.json's `site` by run id (null: a local run). */
  private recorded = new Map<string, string | null>();
  private last: Readonly<Record<string, RunSite>> = {};

  constructor(
    private readonly folder: string,
    private readonly enabled: () => boolean,
    private readonly read: { recordedSite: typeof recordedSite; readRunSite: typeof readRunSite } = { recordedSite, readRunSite },
  ) {}

  /** The link state of the remote runs among `ids`; null when nothing changed since the last call. */
  async update(ids: readonly string[]): Promise<Readonly<Record<string, RunSite>> | null> {
    if (!this.enabled() && Object.keys(this.last).length === 0) return null;
    const next: Record<string, RunSite> = {};
    for (const id of ids) {
      if (!this.recorded.has(id)) this.recorded.set(id, await this.read.recordedSite(this.folder, id));
      const site = this.recorded.get(id);
      if (!site) continue;
      const before = this.last[id];
      next[id] = before && linkSettled(before) ? before : await this.read.readRunSite(this.folder, id, site);
    }
    if (JSON.stringify(next) === JSON.stringify(this.last)) return null;
    this.last = next;
    return next;
  }
}
