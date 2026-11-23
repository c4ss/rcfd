// What a run of the tests left behind, and its end (F-892, ADR 0789).
//
// A Studio that quits while the `rcfd serve` it started still runs leaves that service: it is detached, Playwright's
// forced end (`taskkill /pid <Studio> /T /F`) finds no Studio any more whose children it could walk, and nothing else
// knows the service's pid. The Studio's quit ends its own service (src/main/service/client.ts); this module is the
// harness's part, whatever a test does: launchStudio notes each launch in the run's registry file, and the global
// teardown (global-teardown.ts) ends, and prints, what is still there when the run is over:
//
//   1. the main process of each rcfd-studio whose executable is this checkout's packaged one (out/packaged.json);
//   2. then processes named rcfd with `serve` in the command line
//      a. whose command line names a folder of the registry (`--journal-dir <state folder>/journals`), or
//      b. whose pid is in the service.json of such a folder, or
//      c. whose executable is in this checkout's packaged engine folder.
//
// Rules 1 and 2c apply only when the registry has a launch (a run of the unit tests alone touches no packaged copy).
// A process of another name is never chosen (a pid can be reused), and neither is rcfd-runner or rcfd-mcp: the runner
// has a life of its own (G7).
import { execFileSync } from 'node:child_process';
import { appendFileSync, existsSync, readFileSync, readlinkSync } from 'node:fs';
import path from 'node:path';

/** The variable that names the run's registry file (set by playwright.config.ts, inherited by the workers). */
export const REGISTRY_VARIABLE = 'RCFD_TEST_LAUNCH_REGISTRY';

export interface ProcessRow {
  readonly pid: number;
  readonly ppid: number;
  /** The image name: `rcfd.exe`, `rcfd-studio`. */
  readonly name: string;
  /** The executable's path, '' where it cannot be read. */
  readonly exe: string;
  readonly cmd: string;
}

/** One line of the registry: a folder that holds a launched Studio's service record, and the Studio's pid once known. */
export interface Launch {
  /** RCFD_STUDIO_STATE_DIR, or the --user-data-dir (the record is then in its `service` folder). */
  readonly dir: string;
  /** The pid Playwright has for the launched Studio: on Windows the shell's that started it, the Studio's parent. */
  readonly studioPid?: number;
  /** The spec file that launched it. */
  readonly spec?: string;
}

export interface SweepContext {
  /** The packaged Studio's executable (out/packaged.json), null when there is none. */
  readonly studioExe: string | null;
  /** The packaged engine folder, null when there is none. */
  readonly packagedBin: string | null;
  readonly launches: readonly Launch[];
  /** The pid of each service.json found in a registered folder, with that folder. */
  readonly recordPids: ReadonlyMap<number, string>;
  readonly platform?: NodeJS.Platform;
}

export interface Leftover {
  readonly pid: number;
  readonly kind: 'studio' | 'service';
  readonly cmd: string;
  /** Which rule chose it: `packaged-studio`, `state-folder`, `service-record`, `packaged-engine`. */
  readonly rule: string;
  readonly spec: string | null;
}

const norm = (p: string, platform: NodeJS.Platform): string => {
  const s = platform === 'win32' ? p.replaceAll('/', '\\').toLowerCase() : p;
  return s.replace(/[\\/]+$/, '');
};
const baseName = (name: string, platform: NodeJS.Platform): string => (platform === 'win32' ? name.toLowerCase().replace(/\.exe$/, '') : name);
const within = (file: string, dir: string, platform: NodeJS.Platform): boolean => {
  const sep = platform === 'win32' ? '\\' : '/';
  return norm(file, platform).startsWith(norm(dir, platform) + sep);
};

/** The choice of what to end: pure, the Studios first. */
export function chooseLeftovers(procs: readonly ProcessRow[], ctx: SweepContext): Leftover[] {
  const platform = ctx.platform ?? process.platform;
  const out: Leftover[] = [];
  const specOfPid = new Map<number, string>();
  for (const l of ctx.launches) if (l.studioPid !== undefined && l.spec) specOfPid.set(l.studioPid, l.spec);
  if (ctx.studioExe) {
    const exe = norm(ctx.studioExe, platform);
    for (const p of procs) {
      // the main process only: its helpers (--type=renderer, gpu-process, ...) end with it
      if (baseName(p.name, platform) !== 'rcfd-studio' || norm(p.exe, platform) !== exe || /\s--type=/.test(p.cmd)) continue;
      out.push({ pid: p.pid, kind: 'studio', cmd: p.cmd, rule: 'packaged-studio', spec: specOfPid.get(p.pid) ?? specOfPid.get(p.ppid) ?? null });
    }
  }
  for (const p of procs) {
    if (baseName(p.name, platform) !== 'rcfd' || !/\sserve(\s|$)/.test(p.cmd)) continue;
    const cmd = norm(p.cmd, platform);
    const byFolder = ctx.launches.find((l) => l.dir !== '' && cmd.includes(norm(l.dir, platform)));
    const recordDir = ctx.recordPids.get(p.pid);
    let rule: string | null = null;
    let spec: string | null = null;
    if (byFolder) {
      rule = 'state-folder';
      spec = byFolder.spec ?? null;
    } else if (recordDir !== undefined) {
      rule = 'service-record';
      spec = ctx.launches.find((l) => norm(l.dir, platform) === norm(recordDir, platform))?.spec ?? null;
    } else if (ctx.packagedBin && p.exe !== '' && within(p.exe, ctx.packagedBin, platform)) rule = 'packaged-engine';
    if (rule) out.push({ pid: p.pid, kind: 'service', cmd: p.cmd, rule, spec });
  }
  return out;
}

/** The rcfd and rcfd-studio processes of the machine. An empty list where they cannot be listed. */
export function listProcesses(): ProcessRow[] {
  try {
    if (process.platform === 'win32') {
      const ps =
        "Get-CimInstance Win32_Process | Where-Object { $_.Name -eq 'rcfd.exe' -or $_.Name -eq 'rcfd-studio.exe' } | " +
        "ForEach-Object { [pscustomobject]@{ pid = $_.ProcessId; ppid = $_.ParentProcessId; name = $_.Name; exe = [string]$_.ExecutablePath; cmd = [string]$_.CommandLine } } | ConvertTo-Json -Compress";
      const text = execFileSync('powershell', ['-NoProfile', '-NonInteractive', '-Command', ps], { encoding: 'utf8', windowsHide: true }).trim();
      if (!text) return [];
      const rows = JSON.parse(text) as ProcessRow | ProcessRow[];
      return (Array.isArray(rows) ? rows : [rows]).map((r) => ({ pid: Number(r.pid), ppid: Number(r.ppid), name: String(r.name), exe: String(r.exe ?? ''), cmd: String(r.cmd ?? '') }));
    }
    const text = execFileSync('ps', ['-eo', 'pid=,ppid=,comm=,args='], { encoding: 'utf8' });
    const rows: ProcessRow[] = [];
    for (const line of text.split('\n')) {
      const m = /^\s*(\d+)\s+(\d+)\s+(\S+)\s+(.*)$/.exec(line);
      if (!m) continue;
      const name = path.basename(m[3]!);
      if (name !== 'rcfd' && name !== 'rcfd-studio') continue;
      const pid = Number(m[1]);
      let exe = '';
      try {
        exe = readlinkSync(`/proc/${pid}/exe`);
      } catch {
        exe = m[4]!.split(' ')[0] ?? '';
      }
      rows.push({ pid, ppid: Number(m[2]), name, exe, cmd: m[4]! });
    }
    return rows;
  } catch {
    return [];
  }
}

/** Adds one line to the run's registry. Does nothing outside a run that has one. */
export function noteLaunch(launch: Launch, file: string | undefined = process.env[REGISTRY_VARIABLE]): void {
  if (!file) return;
  try {
    appendFileSync(file, JSON.stringify(launch) + '\n');
  } catch {
    // the registry is a help for the teardown, never a reason for a test to fail
  }
}

export function readRegistry(file: string | undefined): Launch[] {
  if (!file || !existsSync(file)) return [];
  const out: Launch[] = [];
  for (const line of readFileSync(file, 'utf8').split('\n')) {
    if (!line.trim()) continue;
    try {
      const l = JSON.parse(line) as Partial<Launch>;
      if (typeof l.dir !== 'string') continue;
      out.push({ dir: l.dir, ...(typeof l.studioPid === 'number' ? { studioPid: l.studioPid } : {}), ...(typeof l.spec === 'string' ? { spec: l.spec } : {}) });
    } catch {
      // a line cut short by a killed worker
    }
  }
  return out;
}

/** The pid in the service.json of each registered folder (the folder itself, or its `service` folder under a user data folder). */
export function recordPids(launches: readonly Launch[]): Map<number, string> {
  const out = new Map<number, string>();
  for (const l of launches) {
    for (const f of [path.join(l.dir, 'service.json'), path.join(l.dir, 'service', 'service.json')]) {
      try {
        const pid = (JSON.parse(readFileSync(f, 'utf8')) as { pid?: unknown }).pid;
        if (typeof pid === 'number') out.set(pid, l.dir);
      } catch {
        // no record there
      }
    }
  }
  return out;
}

export interface SweepOptions {
  readonly registryFile: string | undefined;
  readonly studioExe: string | null;
  readonly packagedBin: string | null;
  readonly log?: (line: string) => void;
}

/** Ends what the run left (the Studios, then the services) and returns it; each one is printed. */
export function sweepLeftovers(o: SweepOptions): Leftover[] {
  const launches = readRegistry(o.registryFile);
  const ended: Leftover[] = [];
  // the packaged copy is this run's only if the run launched a Studio: a run of the unit tests alone ends nothing of it
  const ran = launches.length > 0;
  // two passes: a service is listed again once the Studios are gone, so one a Studio started meanwhile is seen too
  for (const kind of ['studio', 'service'] as const) {
    const ctx: SweepContext = { studioExe: ran ? o.studioExe : null, packagedBin: ran ? o.packagedBin : null, launches, recordPids: recordPids(launches) };
    for (const l of chooseLeftovers(listProcesses(), ctx)) {
      if (l.kind !== kind) continue;
      try {
        process.kill(l.pid);
      } catch {
        continue; // gone meanwhile
      }
      ended.push(l);
      (o.log ?? console.log)(
        `leftover ${l.kind === 'studio' ? 'Studio' : 'rcfd serve'} ended after the run: pid ${l.pid} (${l.rule}${l.spec ? `, launched by ${l.spec}` : ''}): ${l.cmd}`,
      );
    }
  }
  return ended;
}
