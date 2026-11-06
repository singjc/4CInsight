import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';

const corrected = readFileSync(new URL('../../src/corrected.rs', import.meta.url), 'utf8');
const bridge = readFileSync(new URL('../../wasm/src/lib.rs', import.meta.url), 'utf8');
const rep = corrected.match(/fn\s+parse_repertoire_bytes_with_audit\s*<[^>]*>\s*\([\s\S]*?(?=fn\s+parse_repertoire_workbook_with_audit)/)?.[0];

test('M2/R4b only repertoire XLSX reader borrows bytes without cloning', () => {
  assert.ok(rep, 'missing repertoire parser');
  assert.match(rep, /Xlsx\s*::\s*new\s*\(\s*Cursor\s*::\s*new\s*\(\s*bytes\s*\)\s*\)/);
  assert.doesNotMatch(rep, /bytes\s*\.\s*to_vec\s*\(/);
});

test('M2/R4b panic hook is installed at start of multiomics session', () => {
  assert.match(bridge, /fn install_m2_repertoire_panic_hook\(\)/);
  assert.match(bridge, /4C Insight WASM panic:/);
  assert.match(bridge, /js_namespace\s*=\s*console,\s*js_name\s*=\s*error/);
  const begin = bridge.match(/pub\s+fn\s+multiomics_begin_json\s*\([^)]*\)\s*->\s*Result\s*<\s*String\s*,\s*JsValue\s*>\s*\{([^}]*)/s)?.[1];
  assert.ok(begin, 'missing multiomics begin function');
  assert.match(begin, /install_m2_repertoire_panic_hook\(\);/);
});
