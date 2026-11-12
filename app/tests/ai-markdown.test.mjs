import test from 'node:test';
import assert from 'node:assert/strict';
import {renderAiMarkdown} from '../ai-markdown.js';
test('Markdown renders headings, emphasis, ordered and unordered lists without raw syntax',()=>{
 const out=renderAiMarkdown('## Pathways\n\n**Evidence** has *limits*.\n\n- PCoA is exploratory\n- q = 0.018\n\n1. Verify the cohort\n2. Validate the pathway');
 assert.match(out,/<h4>Pathways<\/h4>/);
 assert.match(out,/<strong>Evidence<\/strong>/);
 assert.match(out,/<em>limits<\/em>/);
 assert.match(out,/<ul><li>PCoA is exploratory<\/li><li>q = 0\.018<\/li><\/ul>/);
 assert.match(out,/<ol><li>Verify the cohort<\/li>/);
});
test('model-generated HTML, Javascript links and event handlers cannot execute',()=>{
 const out=renderAiMarkdown('<img src=x onerror=alert(1)> **safe** [click](javascript:alert) <script>alert(1)</script>');
 assert.doesNotMatch(out,/<img|<script|<a href="javascript/i);
 assert.match(out,/&lt;img/);
 assert.match(out,/<strong>safe<\/strong>/);
});
test('safe absolute links use protective rel attributes; code stays escaped',()=>{
 const out=renderAiMarkdown('[Reactome](https://reactome.org?a=1&b=2) `x <script>`\n\n```js\nconst x = "<script>"\n```');
 assert.match(out,/href="https:\/\/reactome\.org\/\?a=1&amp;b=2"/);
 assert.match(out,/rel="noopener noreferrer nofollow"/);
 assert.match(out,/<code>x &lt;script&gt;<\/code>/);
 assert.match(out,/<pre><code>const x = &quot;&lt;script&gt;&quot;<\/code><\/pre>/);
});
test('rendering bounds untrusted model content',()=>{
 const out=renderAiMarkdown('z'.repeat(60000));
 assert.ok(out.length<=16030);
});
