// Writes the capability inventory of F-824 (capabilities/inventory.json) from the engine itself, and its report
// docs/studio-coverage.md from that inventory (the map of G15 was retired by ADR 0808, F-981).
//
//   npm run capabilities                   # ask the rcfd binaries, write both files
//   npm run capabilities -- --report-only  # only the report, from the committed inventory (no rcfd needed)
//
// rcfd is found as the tests find it (tests/support/rcfd.ts: RCFD_BIN, $CARGO_TARGET_DIR/release, target/release above
// this folder); rcfd-runner and rcfd-mcp must sit next to it (`cargo build --release -p rcfd-cli -p rcfd-runner -p rcfd-mcp`).
// Node runs the two TypeScript files directly with type stripping (they import only Node built-ins).
// tests/unit/capabilities.spec.ts fails when either file is not what this script writes.
import { readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { registerHooks } from 'node:module';

// Type stripping does not map an import of `./x.js` to `./x.ts` (the TypeScript sources import with .js, as tsc wants);
// this hook does, so tests/support/rcfd.ts can import src/main/engine/locate.js.
registerHooks({
  resolve(specifier, context, next) {
    try {
      return next(specifier, context);
    } catch (e) {
      if (e?.code === 'ERR_MODULE_NOT_FOUND' && specifier.startsWith('.') && specifier.endsWith('.js')) {
        return next(specifier.slice(0, -3) + '.ts', context);
      }
      throw e;
    }
  },
});

const { findRcfd, NO_RCFD } = await import('../tests/support/rcfd.ts');
const { engineBinaries, inventoryReport, inventoryText, readEngine } = await import('../tests/support/capabilities.ts');

const here = path.dirname(fileURLToPath(import.meta.url));
const app = path.resolve(here, '..');
const repo = path.resolve(app, '..', '..');
const inventoryPath = path.join(app, 'capabilities', 'inventory.json');
const reportPath = path.join(repo, 'docs', 'studio-coverage.md');
const readJson = (p) => JSON.parse(readFileSync(p, 'utf8'));

let inventory;
if (process.argv.includes('--report-only')) {
  inventory = readJson(inventoryPath);
} else {
  const rcfd = findRcfd();
  if (rcfd === null) {
    console.error(NO_RCFD);
    process.exit(1);
  }
  const bins = engineBinaries(rcfd);
  if ('missing' in bins) {
    console.error(`not found next to ${rcfd}: ${bins.missing.join(', ')} (cargo build --release -p rcfd-runner -p rcfd-mcp)`);
    process.exit(1);
  }
  inventory = readEngine(bins);
  writeFileSync(inventoryPath, inventoryText(inventory));
  console.log(`wrote ${path.relative(process.cwd(), inventoryPath)} from ${rcfd}: ${JSON.stringify(inventory.counts)}`);
}

writeFileSync(reportPath, inventoryReport(inventory));
console.log(`wrote ${path.relative(process.cwd(), reportPath)}`);
