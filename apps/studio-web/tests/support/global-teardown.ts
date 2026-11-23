// The global teardown of the Playwright run (playwright.config.ts): ends, and prints, a packaged Studio or an
// `rcfd serve` the run left behind (F-892, ADR 0789; the rules are in leftovers.ts). It prints and does not fail the run.
import { existsSync, readFileSync, rmSync } from 'node:fs';
import path from 'node:path';
import { REGISTRY_VARIABLE, sweepLeftovers } from './leftovers.js';
import { appRoot } from './rcfd.js';

export default function globalTeardown(): void {
  const registryFile = process.env[REGISTRY_VARIABLE];
  let studioExe: string | null = null;
  let packagedBin: string | null = null;
  const packaged = path.join(appRoot, 'out', 'packaged.json');
  if (existsSync(packaged)) {
    try {
      const { executable } = JSON.parse(readFileSync(packaged, 'utf8')) as { executable: string };
      studioExe = executable;
      const exeDir = path.dirname(executable);
      packagedBin = process.platform === 'darwin' ? path.join(exeDir, '..', 'Resources', 'bin') : path.join(exeDir, 'resources', 'bin');
    } catch {
      // no packaged copy to speak of
    }
  }
  try {
    sweepLeftovers({ registryFile, studioExe, packagedBin });
  } catch (e) {
    console.log(`leftover sweep failed: ${e instanceof Error ? e.message : String(e)}`);
  }
  if (registryFile) rmSync(registryFile, { force: true });
}
