import test from 'node:test'
import assert from 'node:assert/strict'
import {readFileSync} from 'node:fs'
import {join} from 'node:path'
import {fileURLToPath} from 'node:url'
import {composeFeedback, githubIssueDraftURL} from '../feedback.js'
const app = join(fileURLToPath(new URL('../', import.meta.url)))
const read = name => readFileSync(join(app,name),'utf8')

test('header has one GitHub link, one feedback action and existing global state actions', () => {
  const html=read('index.html')
  const head=html.split('<header class="topbar">')[1]?.split('</header>')[0]||''
  for (const id of ['feedback-actions','feedback-report','global-state-actions','load-state','save-state']) {
    assert.equal(head.split(`id="${id}"`).length-1,1,id)
  }
  assert.match(head,/github\.com\/singjc\/4CInsight/)
  assert.match(html,/<dialog id="feedback-dialog"/)
  assert.match(html,/<dialog id="feedback-error-prompt"/)
})
test('external issue draft has explicit title and body, and never claims posting', () => {
  const draft=composeFeedback({kind:'bug',title:'Plot did not load',description:'Zoom issue',version:'test-version'})
  const url = new URL(githubIssueDraftURL(draft))
  assert.equal(url.origin,'https://github.com')
  assert.equal(url.pathname,'/singjc/4CInsight/issues/new')
  assert.match(url.searchParams.get('body'),/Zoom issue/)
  assert.doesNotMatch(url.searchParams.get('body'),/sample_file_private\.xlsx/)
  const js=read('feedback.js')
  assert.doesNotMatch(js,/\bfetch\s*\(|XMLHttpRequest|sendBeacon|api\.github\.com|GITHUB_TOKEN|authorization\s*:/i)
})
test('error content is opt-in and editable before any external navigation', () => {
  const draft=composeFeedback({title:'Problem',diagnostic:'PRIVATE_SAMPLE_NAME',includeDiagnostic:false})
  assert.doesNotMatch(draft.body,/PRIVATE_SAMPLE_NAME/)
  const allowed=composeFeedback({title:'Problem',diagnostic:'Reviewed example',includeDiagnostic:true})
  assert.match(allowed.body,/Reviewed example/)
  const html=read('index.html')
  assert.match(html,/id="feedback-include-diagnostic" type="checkbox"/)
  assert.match(html,/id="feedback-diagnostic"/)
  assert.match(html,/Nothing is submitted until you confirm it on GitHub/)
})
test('feature draft omits bug-only steps; no empty title',()=>{
  assert.throws(()=>composeFeedback({title:' '}),/Add a short title/)
  const issue=composeFeedback({kind:'feature',title:'My request'})
  assert.match(issue.title,/Feature/)
  assert.doesNotMatch(issue.body,/Steps to reproduce/)
})
test('reporting requires sign-in and offers a local download alternative',()=>{
  const html=read('index.html')
  assert.match(html,/https:\/\/github\.com\/signup/)
  assert.match(html,/id="feedback-download"/)
  assert.match(html,/role="status" aria-live="polite"/)
  const js=read('feedback.js')
  assert.match(js,/MutationObserver/)
  assert.match(js,/unhandledrejection/)
  assert.match(js,/\.error-banner\[role=/)
  assert.match(js,/\.analysis-warnings > div/)
})
