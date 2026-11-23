// Starting `rcfd serve` for the Studio, and finding it again after the Studio restarts (docs/studio-guardrails.md G7).
//
// The service is started detached, on loopback with an ephemeral port (`--addr 127.0.0.1:0`, G10), with its output
// going to a log file rather than a pipe: a pipe would break when the Studio exits, and the service must be able to
// outlive the Studio when it holds sessions. The address it prints ("rcfd service listening on <addr>") is read from
// that file. `service.json` in the state directory records the address and process id, so that the next start of the
// Studio attaches to the same service instead of starting a second one.
//
// Session journals (F-828 E-828c, ADR 0735 decision D1): the service is started with `--journal-dir <state dir>/journals`,
// so the engine keeps each session's journal (`<session>-<created>.journal.json`) that `rcfd run --journal` runs. The
// Studio writes no journal file itself (D-L1). With a sandbox root (`--root`) that does not hold the state directory the
// flag is left out, since the service would refuse a journal folder outside its sandbox; journal.get then has no file.
//
// No Electron import (unit-tested in Node).
import { spawn, type ChildProcess } from 'node:child_process';
import { closeSync, openSync } from 'node:fs';
import { mkdir, open, readFile, rm, stat, writeFile } from 'node:fs/promises';
import path from 'node:path';

/** The line `rcfd serve` prints once it listens (crates/rcfd-cli, cmd_serve). */
const LISTENING = /rcfd service listening on (\S+)/;

export interface StartedService {
  readonly child: ChildProcess;
  readonly pid: number;
  readonly host: string;
  readonly port: number;
  readonly log: string;
}

export interface ServiceRecord {
  readonly host: string;
  readonly port: number;
  readonly pid: number;
  readonly rcfd: string;
  readonly startedAt: string;
}

/** Reads the address `rcfd serve` printed: `127.0.0.1:50601` -> host and port. */
export function parseListening(text: string): { host: string; port: number } | null {
  const m = LISTENING.exec(text);
  if (!m) return null;
  const addr = m[1]!;
  const i = addr.lastIndexOf(':');
  if (i < 0) return null;
  const port = Number(addr.slice(i + 1));
  if (!Number.isInteger(port) || port <= 0) return null;
  let host = addr.slice(0, i);
  if (host.startsWith('[') && host.endsWith(']')) host = host.slice(1, -1);
  return { host, port };
}

/** The folder the started service keeps its session journals in: `<stateDir>/journals`, or null when `root` is given
 * and does not hold it (the service sandboxes the journal folder like every path). */
export function journalDirFor(stateDir: string, root: string | null): string | null {
  const dir = path.join(stateDir, 'journals');
  if (root === null) return dir;
  const rel = path.relative(path.resolve(root), path.resolve(dir));
  return rel !== '' && !rel.startsWith('..') && !path.isAbsolute(rel) ? dir : null;
}

/** True while a process of that id exists (signal 0 checks without touching it). */
export function processAlive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch (e) {
    // EPERM: it exists but belongs to someone else
    return (e as NodeJS.ErrnoException).code === 'EPERM';
  }
}

const recordFile = (stateDir: string) => path.join(stateDir, 'service.json');

export async function readRecord(stateDir: string): Promise<ServiceRecord | null> {
  try {
    const r = JSON.parse(await readFile(recordFile(stateDir), 'utf8')) as Partial<ServiceRecord>;
    if (typeof r.host !== 'string' || typeof r.port !== 'number' || typeof r.pid !== 'number' || typeof r.rcfd !== 'string') return null;
    return { host: r.host, port: r.port, pid: r.pid, rcfd: r.rcfd, startedAt: String(r.startedAt ?? '') };
  } catch {
    return null;
  }
}

export async function writeRecord(stateDir: string, r: ServiceRecord): Promise<void> {
  await mkdir(stateDir, { recursive: true });
  await writeFile(recordFile(stateDir), JSON.stringify(r, null, 1) + '\n');
}

export async function removeRecord(stateDir: string): Promise<void> {
  await rm(recordFile(stateDir), { force: true });
}

/**
 * What the starter of a process is told while it starts (F-892, ADR 0789). The Studio's quit must never leave a service
 * that is being started: between "the quit has begun" and "the child is known" there is no await.
 */
export interface StartHooks {
  /** Asked just before the spawn, in the same turn: true (the Studio is quitting) spawns nothing, and the start rejects. */
  readonly cancelled?: () => boolean;
  /** Called with the child in the turn of the spawn, before its address is waited for. */
  readonly onSpawned?: (child: ChildProcess) => void;
}

/** The message of a start that `cancelled` stopped before the spawn. */
export const START_CANCELLED = 'the start was cancelled before the service was spawned';

/**
 * Starts `<rcfd> serve --addr 127.0.0.1:0 --ui [--root <root>] [--journal-dir <dir>]` (the dir of `journalDirFor`)
 * and resolves once it listens. Rejects if it exits first or
 * prints no address within `timeoutMs`.
 */
export function startService(rcfd: string, root: string | null, stateDir: string, timeoutMs = 20_000, hooks: StartHooks = {}): Promise<StartedService> {
  // ADR 0730 P4: --ui switches on the engine's UI metadata and help (settings.schema {ui: true}, help.*)
  const journals = journalDirFor(stateDir, root);
  const args = ['serve', '--addr', '127.0.0.1:0', '--ui', ...(root ? ['--root', root] : []), ...(journals ? ['--journal-dir', journals] : [])];
  return startProcess(rcfd, args, stateDir, timeoutMs, hooks);
}

/**
 * Spawns `<file> <args>` detached, its output appended to `<stateDir>/service.log`, and resolves once it has printed
 * the line of a listening service. `startService` is this with the arguments of `rcfd serve`; the unit tests give it
 * a stand-in command.
 */
export async function startProcess(file: string, args: readonly string[], stateDir: string, timeoutMs = 20_000, hooks: StartHooks = {}): Promise<StartedService> {
  await mkdir(stateDir, { recursive: true });
  const log = path.join(stateDir, 'service.log');
  // Appended to, so an earlier service that still runs keeps its own lines; this start reads from here on.
  const from = await stat(log).then((s) => s.size, () => 0);
  const fd = openSync(log, 'a');
  let child: ChildProcess;
  try {
    // no await from this question to onSpawned: a quit sees either no child, or a child it can end
    if (hooks.cancelled?.()) throw new Error(START_CANCELLED);
    child = spawn(file, [...args], { detached: true, windowsHide: true, stdio: ['ignore', fd, fd] });
  } finally {
    closeSync(fd);
  }
  child.unref();
  const exited = new Promise<string>((resolve) => {
    child.once('error', (e) => resolve(`cannot start ${file}: ${e.message}`));
    child.once('exit', (code, signal) => resolve(`${file} serve ended (${signal ?? `exit code ${code}`}) before it listened`));
  });
  let ended: string | null = null;
  void exited.then((r) => (ended = r));
  hooks.onSpawned?.(child);
  const deadline = Date.now() + timeoutMs;
  for (;;) {
    const text = await readFrom(log, from);
    const addr = parseListening(text);
    if (addr && child.pid !== undefined) return { child, pid: child.pid, host: addr.host, port: addr.port, log };
    if (ended !== null) throw new Error(`${ended}${tail(text)}`);
    if (Date.now() > deadline) {
      child.kill();
      throw new Error(`${file} serve printed no address within ${timeoutMs} ms${tail(text)}`);
    }
    await new Promise((r) => setTimeout(r, 20));
  }
}

async function readFrom(file: string, from: number): Promise<string> {
  const h = await open(file, 'r');
  try {
    const { size } = await h.stat();
    if (size <= from) return '';
    const buf = Buffer.alloc(Math.min(size - from, 1 << 20));
    const { bytesRead } = await h.read(buf, 0, buf.length, from);
    return buf.subarray(0, bytesRead).toString('utf8');
  } finally {
    await h.close();
  }
}

const tail = (text: string) => (text.trim() ? `; its output: ${text.trim().slice(-500)}` : '');
