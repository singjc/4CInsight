import test from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'

const html = await readFile(new URL('../index.html', import.meta.url), 'utf8')
const css = await readFile(new URL('../styles.css', import.meta.url), 'utf8')
const readme = await readFile(new URL('../../README.md', import.meta.url), 'utf8')

test('the existing logo opens the Stanford 4C website in a separate tab', () => {
  assert.match(html, /<h1 class="brand-heading"><a class="brand-link" href="https:\/\/4c\.stanford\.edu\/"[^>]*><img class="brand-logo" src="\.\/assets\/4c-insight-logo\.svg" alt="4C Insight"/)
  assert.match(html, /class="brand-link"[^>]*target="_blank"[^>]*rel="noopener noreferrer"/)
  assert.match(html, /class="brand-link"[^>]*aria-label="Visit Stanford 4C website"/)
  assert.match(css, /\.brand-link:focus-visible\s*\{[^}]*outline:/s)
  assert.equal((html.match(/class="brand-logo"/g) || []).length, 1)
})

test('README is short, states the purpose, and links to the app and methods', () => {
  assert.ok(readme.length < 1500, 'README must remain concise')
  assert.match(readme, /process and analyze multi-omics cohort data/i)
  assert.match(readme, /gene and transcript expression/i)
  assert.match(readme, /Integrate seven data types/i)
  assert.match(readme, /pathway enrichment/i)
  assert.match(readme, /Original cohort files are not uploaded/i)
  assert.match(readme, /https:\/\/singjc\.github\.io\/4CInsight\//)
  assert.match(readme, /docs\/integrative_pathways_guide\.md/)
  assert.match(readme, /https:\/\/4c\.stanford\.edu\//)
  assert.match(readme, /does not imply Stanford endorsement/i)
})
