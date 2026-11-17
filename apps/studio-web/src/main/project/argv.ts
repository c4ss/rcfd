// F-837: `rcfd-studio <PROJECT_DIR>`, the removed egui Studio's start with a project: the packaged app opens the folder
// named on its command line, as Open project folder would (the engine's project server decides whether it is a project;
// a folder that is not one shows the same failure as the dialog's).
//
// The arguments after the program (packaged) or after the app's path (`electron .`), without Chromium's and Node's
// switches (`--inspect=0`, `--remote-debugging-port=0`, which Playwright adds): the first one left is the folder,
// resolved against the working folder the Studio was started in. No Electron import (the unit tests call it).
import path from 'node:path';

export function projectArgument(argv: readonly string[], defaultApp: boolean, cwd: string): string | null {
  const args = argv.slice(defaultApp ? 2 : 1).filter((a) => a !== '' && !a.startsWith('-'));
  const first = args[0];
  return first === undefined ? null : path.resolve(cwd, first);
}
