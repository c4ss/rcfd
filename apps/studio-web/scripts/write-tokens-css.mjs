// Writes src/renderer/design/tokens.css from src/renderer/design/tokens.ts (the one source of the design tokens).
//
//   npm run tokens:css
//
// Node runs tokens.ts directly with type stripping (tokens.ts has no imports and only erasable TypeScript).
// tests/unit/tokens.spec.ts fails when the committed tokens.css is not exactly this output.
import { writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { tokensCssText } from '../src/renderer/design/tokens.ts';

const here = path.dirname(fileURLToPath(import.meta.url));
const out = path.resolve(here, '..', 'src', 'renderer', 'design', 'tokens.css');
writeFileSync(out, tokensCssText());
console.log(`wrote ${path.relative(process.cwd(), out)}`);
