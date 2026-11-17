// Writes the capability inventory of F-824 (capabilities/inventory.json) from the engine itself, and the coverage report
// docs/studio-coverage.md from that inventory and the hand-kept map capabilities/coverage.json.
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
import { findRcfd, NO_RCFD } from '../tests/support/rcfd.ts';
import { checkCoverage, coverageReport, engineBinaries, inventoryText, readEngine } from '../tests/support/capabilities.ts';

const here = path.dirname(fileURLToPath(import.meta.url));
const app = path.resolve(here, '..');
const repo = path.resolve(app, '..', '..');
const inventoryPath = path.join(app, 'capabilities', 'inventory.json');
const coveragePath = path.join(app, 'capabilities', 'coverage.json');
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

const coverage = readJson(coveragePath);
const roadmap = readJson(path.join(repo, 'docs', 'roadmap.json'));
writeFileSync(reportPath, coverageReport(inventory, coverage, roadmap));
console.log(`wrote ${path.relative(process.cwd(), reportPath)}`);
const res = checkCoverage(inventory, coverage, roadmap);
if (res.unmapped.length > 0 || res.problems.length > 0) {
  for (const id of res.unmapped) console.error(`not mapped: ${id}`);
  for (const p of res.problems) console.error(`problem: ${p}`);
  process.exit(2);
}
