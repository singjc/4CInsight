import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync, existsSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
import {dirname, join} from 'node:path';

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const main = readFileSync(join(root, 'main.js'), 'utf8');
const html = readFileSync(join(root, 'index.html'), 'utf8');

const expected = [
  ['project', 'Project', '1'],
  ['metadata', 'Sample Metadata', 'M'],
  ['qc', 'Explore & QC', '2'],
  ['compare', 'Compare Groups', '3'],
  ['results', 'Results', 'R'],
  ['longitudinal', 'Longitudinal', '4'],
  ['hits', 'Integrative Analysis', '5'],
  ['history', 'History', '6'],
  ['docs', 'Docs', '?'],
  ['settings', 'Settings', '⚙'],
];

test('workflow navigation preserves all ten scientific and project views', () => {
  const nav = main.match(/const tabs = \[([\s\S]*?)\n\]/)?.[1];
  assert.ok(nav, 'Expected the existing workflow navigation table');
  const actual = [...nav.matchAll(/\['([^']+)', '([^']+)', '([^']+)'\]/g)]
    .map(m => [m[1], m[2], m[3]]);
  assert.deepEqual(actual, expected);
});

test('retired assistant does not load, render, or restore its former tab', () => {
  assert.doesNotMatch(main, /aiAssistantView|mountAiAssistant|['"]ai['"]|\.\/ai-(?:assistant|runtime|evidence|markdown)/);
  assert.doesNotMatch(html, /ai-assistant\.css|ai-assistant\.js/);
  assert.match(main, /hits: significantHitsView/);
  assert.match(main, /history: historyView/);
  assert.match(main, /state\.integrativeView/);
  assert.match(main, /function scheduleLocalSessionSave\(/);
});

test('retired assistant bundles, tests, guide and native companion are absent', () => {
  for (const rel of [
    'ai-assistant.js', 'ai-assistant.css', 'ai-runtime.js',
    'ai-evidence.js', 'ai-markdown.js',
    'tests/ai-assistant.test.mjs', 'tests/ai-evidence.test.mjs',
    'tests/ai-markdown.test.mjs',
    '../docs/ai_assistant.md',
    '../tools/ai-companion/__pycache__/native_host.cpython-312.pyc',
    '../tools/ai-companion/__pycache__/test_native_host.cpython-312.pyc',
  ]) assert.equal(existsSync(join(root, rel)), false, `Retired file still exists: ${rel}`);
});
