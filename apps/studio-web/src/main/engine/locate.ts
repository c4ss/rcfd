// Where the Studio finds its engine (rcfd, rcfd-mcp, rcfd-runner), with no setup (ADR 0801 Addendum 21, "Engine found
// without setup"). Pure, no Electron import: src/main/index.ts, scripts/package-dir.mjs (through dist/) and the tests
// (tests/support/rcfd.ts, tests/support/project.ts, tests/unit/engine-locate.spec.ts) share it.
//
// The order, first match wins (switch > variable > bundled or checkout):
//
//   1. --no-engine                    no engine: no project server, and no service started by the Studio
//   2. --engine-dir <folder>          the engine in that folder
//   3. RCFD_STUDIO_MCP, RCFD_STUDIO_RCFD   the engine the variables name (src/main/project/config.ts and
//                                     src/main/service/config.ts read them, as before Addendum 21)
//   4. the default folder             packaged (an installed Studio, out/packaged): <resources>/bin, which the
//                                     installers and scripts/package-dir.mjs fill; not packaged (`electron .` from the
//                                     checkout): $CARGO_TARGET_DIR/release when that is set, else <repo>/target/release
//                                     (the app folder apps/studio-web, then ../../target/release)
//
// An engine named by a switch or a variable is used whole: the Studio never mixes it with parts of the default one. One
// folder gives all three programs: RCFD_STUDIO_MCP alone gives the project server and the rcfd beside it (started with
// the first project), RCFD_STUDIO_RCFD alone the rcfd (started at launch) and the rcfd-mcp beside it.
//
// Attaching to a service someone else started is a separate choice (--service > RCFD_STUDIO_SERVICE): it replaces the
// service the Studio would start, whatever the engine is, and it still applies with --no-engine (attaching starts
// nothing). That attach is where engines on other machines (a cloud or a cluster) will plug in later; today it is
// loopback only (G10).
//
// Claude Code (ADR 0801 Addendum 23), which the Studio registers the project server with and opens in a terminal, is
// found the same way, first match wins (locateClaude below):
//
//   1. RCFD_STUDIO_CLAUDE             the claude program the variable names
//   2. --claude <path>                the claude program the switch names
//   3. its installer's folder         %USERPROFILE%\.local\bin\claude.exe on Windows, $HOME/.local/bin/claude
//                                     elsewhere (where the native installer of Claude Code puts it)
//
// Not found, the Studio shows the command to run and says Claude Code was not found where its installer puts it.
//
// Nothing is looked for on PATH (F-805 decision D3, ADR 0801 Addendum 19, Q-F820-2 answer (a)): the default folder is
// one known place inside the installed Studio or inside the checkout, never a search.
import path from 'node:path';

export interface EnginePart {
  /** The program's name without .exe. */
  readonly name: 'rcfd' | 'rcfd-mcp' | 'rcfd-runner';
  /** The cargo package that builds it. */
  readonly pkg: string;
}

export const ENGINE_PARTS: readonly EnginePart[] = [
  { name: 'rcfd-mcp', pkg: 'rcfd-mcp' },
  { name: 'rcfd', pkg: 'rcfd-cli' },
  { name: 'rcfd-runner', pkg: 'rcfd-runner' },
];

/** The file name of a program on this platform. */
export const exe = (name: string, platform: NodeJS.Platform = process.platform): string => (platform === 'win32' ? `${name}.exe` : name);

/** The one cargo command that builds these parts (all three when none are given). */
export function buildCommand(parts: readonly EnginePart[] = ENGINE_PARTS): string {
  return `cargo build --release ${(parts.length ? parts : ENGINE_PARTS).map((p) => `-p ${p.pkg}`).join(' ')}`;
}

/** The engine built in this checkout: $CARGO_TARGET_DIR/release, else <app folder>/../../target/release. */
export function checkoutRelease(env: Readonly<Record<string, string | undefined>>, appFolder: string): string {
  const target = env['CARGO_TARGET_DIR']?.trim();
  return target ? path.join(path.resolve(target), 'release') : path.resolve(appFolder, '..', '..', 'target', 'release');
}

export interface DefaultEngine {
  readonly dir: string;
  /** `bundled`: <resources>/bin of a packaged Studio; `checkout`: the target folder of the checkout it runs from. */
  readonly kind: 'bundled' | 'checkout';
}

/** The default folder (step 4 above). `appFolder`: app.getAppPath() (apps/studio-web for `electron .`). */
export function defaultEngine(
  env: Readonly<Record<string, string | undefined>>,
  packaged: boolean,
  resourcesPath: string,
  appFolder: string,
): DefaultEngine {
  return packaged ? { dir: path.join(resourcesPath, 'bin'), kind: 'bundled' } : { dir: checkoutRelease(env, appFolder), kind: 'checkout' };
}

/** The parts not in `dir`. */
export function missingParts(dir: string, isFile: (p: string) => boolean, platform: NodeJS.Platform = process.platform): EnginePart[] {
  return ENGINE_PARTS.filter((p) => !isFile(path.join(dir, exe(p.name, platform))));
}

/** The engine the Studio uses, before the variables are read (they are, by the two config modules, for `variables`). */
export type EngineChoice =
  | { readonly from: 'no-engine' }
  | { readonly from: 'variables' }
  | {
      readonly from: '--engine-dir' | 'bundled' | 'checkout';
      readonly dir: string;
      /** The parts not found there; empty when the engine is complete. */
      readonly missing: readonly EnginePart[];
    };

export function chooseEngine(
  env: Readonly<Record<string, string | undefined>>,
  switches: { readonly engineDir: string | null; readonly noEngine: boolean },
  fallback: DefaultEngine,
  isFile: (p: string) => boolean,
  platform: NodeJS.Platform = process.platform,
): EngineChoice {
  if (switches.noEngine) return { from: 'no-engine' };
  if (switches.engineDir) return { from: '--engine-dir', dir: switches.engineDir, missing: missingParts(switches.engineDir, isFile, platform) };
  if (env['RCFD_STUDIO_MCP']?.trim() || env['RCFD_STUDIO_RCFD']?.trim()) return { from: 'variables' };
  return { from: fallback.kind, dir: fallback.dir, missing: missingParts(fallback.dir, isFile, platform) };
}

/** The program `name` of a complete chosen folder, else null. */
export function partPath(choice: EngineChoice, name: EnginePart['name'], platform: NodeJS.Platform = process.platform): string | null {
  return 'dir' in choice && choice.missing.length === 0 ? path.join(choice.dir, exe(name, platform)) : null;
}

const list = (names: readonly string[]): string =>
  names.length <= 1 ? (names[0] ?? '') : `${names.slice(0, -1).join(', ')} and ${names[names.length - 1]}`;

/**
 * Why a chosen folder gives no engine, in words: where the Studio looked, which files are not there, and what to do.
 * Null when the engine is complete or not chosen from a folder.
 */
export function missingReason(choice: EngineChoice, platform: NodeJS.Platform = process.platform): string | null {
  if (choice.from === 'no-engine') return 'The Studio was started without an engine (--no-engine).';
  if (!('dir' in choice) || choice.missing.length === 0) return null;
  const files = list(choice.missing.map((p) => exe(p.name, platform)));
  const what = choice.missing.length === 1 ? 'is' : 'are';
  if (choice.from === 'checkout') {
    return `The Studio looked for the engine built in this checkout, in ${choice.dir}: ${files} ${what} not there. Build ${choice.missing.length === 1 ? 'it' : 'them'} with: ${buildCommand(choice.missing)}`;
  }
  if (choice.from === 'bundled') {
    return `The engine that comes with the Studio is incomplete: ${files} ${what} not in ${choice.dir}. Install the Studio again, or start it with --engine-dir <folder>.`;
  }
  return `--engine-dir names ${choice.dir}, but ${files} ${what} not there.`;
}

/** Where the Studio found `claude` (ADR 0801 Addendum 23), or why not, in words. */
export type ClaudeLocation =
  | { readonly found: true; readonly path: string; readonly from: 'RCFD_STUDIO_CLAUDE' | '--claude' | 'installer' }
  | { readonly found: false; readonly reason: string };

/** The folder Claude Code's native installer puts `claude` in: <home>/.local/bin. */
export function claudeInstallerPath(env: Readonly<Record<string, string | undefined>>, platform: NodeJS.Platform = process.platform): string | null {
  const home = platform === 'win32' ? env['USERPROFILE']?.trim() : env['HOME']?.trim();
  if (!home) return null;
  const p = platform === 'win32' ? path.win32 : path.posix;
  return p.join(home, '.local', 'bin', exe('claude', platform));
}

/** `claude`: RCFD_STUDIO_CLAUDE, then --claude, then the installer's folder; never PATH. */
export function locateClaude(
  env: Readonly<Record<string, string | undefined>>,
  switchPath: string | null,
  isFile: (p: string) => boolean,
  platform: NodeJS.Platform = process.platform,
): ClaudeLocation {
  const variable = env['RCFD_STUDIO_CLAUDE']?.trim();
  if (variable) {
    return isFile(variable)
      ? { found: true, path: variable, from: 'RCFD_STUDIO_CLAUDE' }
      : { found: false, reason: `RCFD_STUDIO_CLAUDE names ${variable}, which is not a file.` };
  }
  if (switchPath) {
    return isFile(switchPath) ? { found: true, path: switchPath, from: '--claude' } : { found: false, reason: `--claude names ${switchPath}, which is not a file.` };
  }
  const installed = claudeInstallerPath(env, platform);
  if (installed && isFile(installed)) return { found: true, path: installed, from: 'installer' };
  return {
    found: false,
    reason: `Claude Code was not found where its installer puts it (${installed ?? (platform === 'win32' ? '%USERPROFILE%\\.local\\bin\\claude.exe' : '~/.local/bin/claude')}). Run the command below in a terminal, or start the Studio with --claude <path>.`,
  };
}
