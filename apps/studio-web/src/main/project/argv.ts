// The Studio's own command line: the project folder (F-837) and the engine switches (ADR 0801 Addendum 21, "Engine
// found without setup"). No Electron import (the unit tests call it).
//
// F-837: `rcfd-studio <PROJECT_DIR>`, the removed egui Studio's start with a project: the packaged app opens the folder
// named on its command line, as Open project folder would (the engine's project server decides whether it is a project;
// a folder that is not one shows the same failure as the dialog's).
//
// The engine switches, so a Windows shortcut or a .desktop file can carry them (src/main/engine/locate.ts has the order
// they are applied in):
//
//   --engine-dir <folder>    use the engine in <folder> (rcfd, rcfd-mcp and rcfd-runner inside it)
//   --no-engine              start without an engine: no project server, no service started by the Studio
//   --service <host:port>    attach to a service someone else started (loopback only), as RCFD_STUDIO_SERVICE does
//
// Each also takes the `--name=value` form. The arguments after the program (packaged) or after the app's path
// (`electron .`), without Chromium's and Node's switches (`--inspect=0`, `--remote-debugging-port=0`, which Playwright
// adds) and without the engine switches and their values: the first one left is the folder, resolved against the
// working folder the Studio was started in.
import path from 'node:path';

/** The engine switches that take a value (the next argument, or after `=`). */
const VALUED = ['--engine-dir', '--service'] as const;

export interface EngineSwitches {
  /** `--engine-dir`: the folder of the engine to use, resolved against the working folder; null when not given. */
  readonly engineDir: string | null;
  /** `--no-engine`. */
  readonly noEngine: boolean;
  /** `--service`: the `host:port` to attach to, as written; null when not given. */
  readonly service: string | null;
  /** Switches given without their value, in words (the last of a repeated switch wins; that is not a problem). */
  readonly problems: readonly string[];
}

const ownArgs = (argv: readonly string[], defaultApp: boolean): readonly string[] => argv.slice(defaultApp ? 2 : 1);

export function engineSwitches(argv: readonly string[], defaultApp: boolean, cwd: string): EngineSwitches {
  const args = ownArgs(argv, defaultApp);
  let engineDir: string | null = null;
  let service: string | null = null;
  let noEngine = false;
  const problems: string[] = [];
  for (let i = 0; i < args.length; i++) {
    const a = args[i]!;
    if (a === '--no-engine') {
      noEngine = true;
      continue;
    }
    for (const name of VALUED) {
      let value: string | undefined;
      if (a === name) {
        const next = args[i + 1];
        if (next !== undefined && next !== '' && !next.startsWith('-')) {
          value = next;
          i++;
        } else {
          problems.push(`${name} needs a value`);
          continue;
        }
      } else if (a.startsWith(`${name}=`)) {
        value = a.slice(name.length + 1);
        if (!value.trim()) {
          problems.push(`${name} needs a value`);
          continue;
        }
      } else continue;
      if (name === '--engine-dir') engineDir = path.resolve(cwd, value.trim());
      else service = value.trim();
    }
  }
  return { engineDir, noEngine, service, problems };
}

export function projectArgument(argv: readonly string[], defaultApp: boolean, cwd: string): string | null {
  const args = ownArgs(argv, defaultApp);
  const left: string[] = [];
  for (let i = 0; i < args.length; i++) {
    const a = args[i]!;
    if ((VALUED as readonly string[]).includes(a)) {
      // the switch's value is not the folder
      const next = args[i + 1];
      if (next !== undefined && next !== '' && !next.startsWith('-')) i++;
      continue;
    }
    if (a !== '' && !a.startsWith('-')) left.push(a);
  }
  const first = left[0];
  return first === undefined ? null : path.resolve(cwd, first);
}
