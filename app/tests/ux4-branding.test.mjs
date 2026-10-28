import test from 'node:test'
import assert from 'node:assert/strict'
import { readFile, access } from 'node:fs/promises'

const app = new URL('../', import.meta.url)
const html = await readFile(new URL('index.html', app), 'utf8')
const css = await readFile(new URL('styles.css', app), 'utf8')

test('served app uses the existing 4C Insight SVG logo as its only visible branding', async () => {
  const logo = await readFile(new URL('assets/4c-insight-logo.svg', app), 'utf8')
  assert.match(logo, /^<svg xmlns="http:\/\/www\.w3\.org\/2000\/svg"/)
  assert.match(html, /<h1 class="brand-heading"><img class="brand-logo" src="\.\/assets\/4c-insight-logo\.svg" alt="4C Insight"/)
  assert.doesNotMatch(html, /class="brand-mark"|<h1>4C Insight<\/h1>|Local-first multi-omics cohort analysis<\/p>/)
  assert.match(css, /\.brand-logo\s*\{/)
  assert.match(css, /\.brand-logo\s*\{[^}]*object-fit:\s*contain/s)
})

test('logo uses a Pages-safe relative path and mobile header rules', () => {
  assert.match(html, /src="\.\/assets\/4c-insight-logo\.svg"/)
  assert.match(html, /styles\.css\?v=0\.21\.0-ux4-branding/)
  assert.match(css, /@media \(max-width: 720px\)\s*\{/)
  assert.match(css, /\.brand-logo\s*\{ height: 64px; \}/)
  assert.match(html, /id="system-chip"/)
})

test('root-level legacy logo is removed after the move', async () => {
  await assert.rejects(access(new URL('../../4C_Insight_logo_practical_trace_balanced_tight_margin.svg', import.meta.url)), /ENOENT/)
})
