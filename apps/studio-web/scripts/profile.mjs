// F-817b (ADR 0800 §9, Q5, the maintainer's answer of 2026-11-16): the Chromium profile of the measurement scripts.
//
// With `--profile-root=<folder>` every launch of the Studio gets a fresh, empty profile of its own,
// `<folder>/launch-<n>`, passed as `--user-data-dir` (Electron then keeps userData, its caches and the GPU caches
// there), so a case does not share `%APPDATA%\rcfd Studio` with other Studio instances on the machine (the cache-lock
// errors of F-817's first P5 launch) and each launch starts with cold caches. The folder belongs to the caller, which
// removes it after the script has ended: scripts/bench.mjs makes one per case in the temporary directory and removes it.
// Without the flag a script launches with the user's own profile, as before. P10 (scripts/measure-cold-start.mjs) does
// not take the flag: it measures the user's own start.
import { mkdirSync } from 'node:fs';
import path from 'node:path';

const root = process.argv.find((a) => a.startsWith('--profile-root='))?.slice('--profile-root='.length) ?? '';
let launches = 0;

/** The Chromium arguments of one launch: a fresh profile folder under --profile-root, else none (the user's profile). */
export function profileArgs() {
  if (!root) return [];
  const dir = path.join(path.resolve(root), `launch-${++launches}`);
  mkdirSync(dir, { recursive: true });
  return [`--user-data-dir=${dir}`];
}

/** For the script's JSON: which profile its launches used. */
export function profileRecord() {
  return root ? { mode: 'scratch', root: path.resolve(root), launches } : { mode: 'own' };
}
