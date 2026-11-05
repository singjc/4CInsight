import test from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { methodDocs } from '../docs-content.js'
import { methodToyExamples, exampleSearchText, renderToyExamples } from '../docs-toy-examples.js'

const main = await readFile(new URL('../main.js', import.meta.url), 'utf8')
const css = await readFile(new URL('../styles.css', import.meta.url), 'utf8')

function approx(actual, expected, tolerance = 1e-3) {
  assert.ok(Math.abs(actual - expected) <= tolerance, `${actual} not within ${tolerance} of ${expected}`)
}

test('every documentation section has a worked example with matching table dimensions', () => {
  const ids = new Set(methodDocs.map((entry) => entry.id))
  assert.equal(ids.size, methodDocs.length)
  assert.deepEqual(new Set(Object.keys(methodToyExamples)), ids)
  for (const section of methodDocs) {
    assert.ok(section.references.length, `${section.id} must retain its references`)
    const examples = methodToyExamples[section.id]
    assert.ok(examples.length > 0, `${section.id} has no example`)
    for (const example of examples) {
      assert.ok(example.title && example.question && example.meaning && example.limit)
      assert.ok(example.columns.length >= 2)
      assert.ok(example.rows.length >= 2)
      assert.ok(example.steps.length >= 2)
      assert.ok(example.rows.every((row) => row.length === example.columns.length))
    }
  }
})

test('toy examples have search text and accessible native disclosure controls', () => {
  for (const section of methodDocs) {
    const html = renderToyExamples(section.id)
    assert.match(html, /<details class="docs-toy-example"/)
    assert.match(html, /<summary>/)
    assert.match(html, /scope="col"/)
    assert.match(html, /Illustrative values only/)
    assert.ok(exampleSearchText(section.id).includes(methodToyExamples[section.id][0].title))
  }
  assert.match(main, /exampleSearchText\(section\.id\)/)
  assert.match(main, /renderToyExamples\(section\.id\)/)
  assert.match(main, /id="docs-search"/)
  assert.match(css, /\.docs-toy-example > summary:focus-visible/)
})

test('HTML in future example content is escaped', () => {
  const first = methodToyExamples['local-first'][0]
  const original = first.question
  try {
    first.question = '<script>alert("x")</script>'
    const html = renderToyExamples('local-first')
    assert.doesNotMatch(html, /<script>/)
    assert.match(html, /&lt;script&gt;/)
  } finally {
    first.question = original
  }
})

test('median-ratio and effect-size arithmetic in explanations are consistent', () => {
  const geometric = Math.sqrt(10 * 20)
  const factorA = 10 / geometric
  const factorB = 20 / geometric
  approx(factorA, Math.SQRT1_2)
  approx(factorB, Math.SQRT2)
  approx(10 / factorA, 20 / factorB)
  assert.equal(Math.log2(20 / 10), 1)
})

test('BH, volcano, CLR, AUC and PCA toy calculations are consistent', () => {
  const p = [0.001, 0.020, 0.040, 0.200]
  const q = p.map((value, index) => value * p.length / (index + 1))
  for (let i = q.length - 2; i >= 0; i -= 1) q[i] = Math.min(q[i], q[i + 1])
  approx(q[0], 0.004, 1e-9)
  approx(q[1], 0.04, 1e-9)
  approx(q[2], 0.053333333, 1e-6)
  assert.equal(q.filter((value) => value <= 0.05).length, 2)
  approx(-Math.log10(0.05), 1.301, 1e-3)
  const clrA = 0.5 * Math.log((9 + 0.5) / (1 + 0.5))
  const clrB = 0.5 * Math.log((1 + 0.5) / (9 + 0.5))
  approx(clrA, 0.923, 1e-3)
  approx(clrB, -0.923, 1e-3)
  const within = [0.2, 1.3]
  const between = [0.8, 1.0, 1.2, 1.4]
  const wins = within.flatMap((a) => between.map((b) => a < b ? 1 : a === b ? 0.5 : 0))
    .reduce((sum, value) => sum + value, 0)
  assert.equal(wins / (within.length * between.length), 0.625)
  approx(Math.hypot(2 - 3, 2 - 3), Math.SQRT2, 1e-9)
  approx(Math.hypot(2 - 9, 2 - 9), Math.sqrt(98), 1e-9)
})

test('documentation remains static, browser-only, and does not request new permissions', async () => {
  const code = await readFile(new URL('../docs-toy-examples.js', import.meta.url), 'utf8')
  assert.doesNotMatch(code, /\bfetch\s*\(|\bXMLHttpRequest\b|navigator\.permissions/)
  assert.doesNotMatch(code, /\binnerHTML\b|\beval\s*\(/)
  assert.match(main, /docs-toy-examples\.js\?v=0\.21\.0-[a-z0-9-]+/)
})
