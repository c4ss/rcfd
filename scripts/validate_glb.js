// Runs the Khronos glTF-Validator (npm package gltf-validator, run with node) on GLB files (ADR 0011, F-117).
// Usage: node scripts/validate_glb.js <file.glb> [...]
// Install once: npm install gltf-validator   (in a scratch directory; NODE_PATH must reach it)
const fs = require('fs');
const validator = require('gltf-validator');
(async () => {
  let bad = 0;
  for (const f of process.argv.slice(2)) {
    const bytes = new Uint8Array(fs.readFileSync(f));
    const r = await validator.validateBytes(bytes, { uri: f });
    const i = r.issues;
    console.log(`${i.numErrors === 0 ? 'PASS' : 'FAIL'} ${f}: validator ${r.validatorVersion} errors=${i.numErrors} warnings=${i.numWarnings} infos=${i.numInfos} hints=${i.numHints}`);
    for (const m of i.messages) console.log(`   [${['error', 'warning', 'info', 'hint'][m.severity]}] ${m.code} ${m.pointer || ''} ${m.message}`);
    if (i.numErrors) bad++;
  }
  process.exit(bad ? 1 : 0);
})();
