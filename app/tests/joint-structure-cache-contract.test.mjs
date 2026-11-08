import test from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'

const read = path => readFile(new URL(`../${path}`, import.meta.url), 'utf8')

test('cache versions are coherent after joint-analysis release', async () => {
  const [main, html, worker, documentation, examples, visuals] = await Promise.all([
    read('main.js'), read('index.html'), read('analysis-worker.js'),
    read('docs-content.js'), read('docs-toy-examples.js'), read('docs-toy-visuals.js'),
  ])
  const version = main.match(/const APP_VERSION = '([^']+)'/)?.[1]
  assert.ok(version && /^\d+\.\d+\.\d+(?:-[a-z0-9-]+)?$/.test(version), 'A valid cache revision is required')
  for (const asset of ['styles.css', 'main.js', 'feedback.js']) {
    assert.ok(html.includes(`./${asset}?v=${version}`), `${asset} must share the application revision`)
  }
  for (const [source,asset] of [
    [main,'integrative-evidence.js'], [main,'multiomics-alignment.js'],
    [main,'integrative-distance-input.js'], [main,'joint-structure.js'],
    [worker,'joint-structure.js'], [documentation,'docs-new-modalities.js'],
    [examples,'docs-toy-visuals.js'], [visuals,'docs-new-modalities.js'],
  ]) {
    assert.ok(source.includes(`${asset}?v=${version}`), `${asset} must share the application revision`)
  }
  for (const [name,source] of [['main.js',main], ['analysis-worker.js',worker], ['docs-content.js',documentation], ['docs-toy-examples.js',examples], ['docs-toy-visuals.js',visuals]]) {
    const versions = [...source.matchAll(/\.js\?v=(\d+\.\d+\.\d+(?:-[a-z0-9-]+)?)/g)].map(match => match[1])
    for (const queryVersion of versions) assert.equal(queryVersion,version,`${name} has a stale versioned script import`)
  }
  assert.doesNotMatch(main, /0\.21\.0-i2b-source-distances/)
  assert.doesNotMatch(worker, /0\.21\.0-i2b-source-distances/)
})
