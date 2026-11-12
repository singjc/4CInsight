import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {aiAssistantView} from '../ai-assistant.js';
import {LOCAL_MODELS, WEBLLM_IMPORT, browserSupport, safeAiContext, cleanQuestion, cleanModelId, selectionFromCatalog, promptMessages, cloudChat, ollamaModels} from '../ai-runtime.js';
const src = async file => readFile(new URL(`../${file}`,import.meta.url),'utf8');

test('browser-first UI defaults to on-device with no JCode, extension, port or token',async()=>{
  const html=aiAssistantView();
  assert.match(html,/On-device · Private/);
  assert.match(html,/Download and start model/);
  assert.match(html,/Choose where the model runs/);
  assert.match(html,/approved aggregate analysis summary/);
  assert.doesNotMatch(html,/JCode|Firefox bridge|native companion|Access token/i);
  const main=await src('main.js'),index=await src('index.html');
  assert.match(main,/\['ai', 'AI Assistant', 'A'\]/);
  assert.match(main,/ai: aiAssistantView/);
  assert.match(main,/mountAiAssistant/);
  assert.match(main,/'ai', 'history'/);
  assert.match(index,/ai-assistant\.css\?v=0\.22\.8/);
});
test('local model list is bounded and must match actual runtime catalog',()=>{
  assert.match(WEBLLM_IMPORT, /web-llm@0\.2\.88/);
  assert.equal(LOCAL_MODELS.length,4);
  const first=LOCAL_MODELS[0];
  const chosen=selectionFromCatalog(first.id,[{model_id:first.id,vram_required_MB:1200}]);
  assert.equal(chosen.vramRequiredMB,1200);
  assert.throws(()=>selectionFromCatalog(first.id,[]),/not present/);
  assert.throws(()=>selectionFromCatalog('unknown',[]),/listed/);
  assert.ok(LOCAL_MODELS.every(m=>Number.isFinite(m.memoryMB)&&m.memoryMB>0));
});
test('browser support does not guess GPU memory or promise WebGPU',()=>{
  assert.equal(browserSupport({}).webgpu,false);
  assert.equal(browserSupport({gpu:{requestAdapter:()=>{}}}).webgpu,true);
});
test('aggregate context never includes patient IDs, raw file names, or user-injected fields',()=>{
  const input={integrative:true,compare:false,pathways:true,pathwayCount:1713,pathwayEvidence:'exploratory',patient_id:'PATIENT_X',files:['private.raw'],secrets:'sk-123'};
  const context=safeAiContext(input),txt=JSON.stringify(context);
  assert.equal(context.pathway_sets_tested,1713);
  assert.deepEqual(Object.keys(context),['context_type','integrative_result_available','compare_groups_result_available','pathway_result_available','pathway_sets_tested','pathway_evidence_type']);
  assert.doesNotMatch(txt,/PATIENT_X|private\.raw|sk-123/);
  const message=promptMessages('Interpret?',[],input).at(-1).content;
  assert.match(message,/1713/);
  assert.doesNotMatch(message,/PATIENT_X|private\.raw|sk-123/);
});
test('question, history and model ID inputs are bounded',()=>{
  assert.equal(cleanQuestion(' Test '),'Test');
  assert.throws(()=>cleanQuestion(' '),/Enter a question/);
  assert.throws(()=>cleanQuestion('x'.repeat(4001)),/4,000/);
  assert.throws(()=>cleanModelId('abc;rm -rf'),/valid model ID/);
  const history=Array.from({length:11},(_,i)=>({role:'user',content:String(i)}));
  assert.equal(promptMessages('hello',history).length,10);
  assert.equal(promptMessages('hello').at(-1).content,'hello');
});
test('OpenAI direct API call is explicit, ephemeral and uses exact endpoint',async()=>{
  let request;
  const mock=async(url,opts)=>{request={url,opts};return {ok:true,json:async()=>({choices:[{message:{content:'Example reply'}}]})}};
  const answer=await cloudChat({provider:'openai',model:'gpt-5.6-terra',key:'test-only',question:'Hi'},mock);
  assert.equal(answer,'Example reply');
  assert.equal(request.url,'https://api.openai.com/v1/chat/completions');
  assert.equal(request.opts.credentials,'omit');
  assert.equal(request.opts.cache,'no-store');
  assert.equal(request.opts.headers.Authorization,'Bearer test-only');
  assert.equal(JSON.parse(request.opts.body).messages.at(-1).content,'Hi');
  await assert.rejects(()=>cloudChat({provider:'openai',model:'test',question:'Hi'},mock),/API key/);
});
test('Claude browser request uses documented opt-in header and blocks missing key',async()=>{
  let request;
  const mock=async(url,opts)=>{request={url,opts};return {ok:true,json:async()=>({content:[{type:'text',text:'Claude says hello'}]})}};
  assert.equal(await cloudChat({provider:'claude',model:'claude-sonnet-4-5',key:'test-key',question:'Hi'},mock),'Claude says hello');
  assert.equal(request.url,'https://api.anthropic.com/v1/messages');
  assert.equal(request.opts.headers['anthropic-dangerous-direct-browser-access'],'true');
  assert.equal(request.opts.credentials,'omit');
  assert.ok(JSON.parse(request.opts.body).max_tokens>0);
  await assert.rejects(()=>cloudChat({provider:'claude',model:'test',question:'Hi'},mock),/API key/);
});
test('Ollama is an optional fixed local origin; no user-defined network proxy',async()=>{
  let endpoint;
  const mock=async(url,opts)=>{endpoint=url;return {ok:true,json:async()=>url.endsWith('/tags')?{models:[{name:'small:latest'}]}:{message:{content:'local'}}}};
  assert.deepEqual(await ollamaModels(mock),['small:latest']);
  assert.equal(endpoint,'http://127.0.0.1:11434/api/tags');
  assert.equal(await cloudChat({provider:'ollama',model:'small:latest',question:'Hi'},mock),'local');
  assert.equal(endpoint,'http://127.0.0.1:11434/api/chat');
});
test('provider errors do not include raw key strings',async()=>{
  const key='sk-THIS-IS-SECRET';
  const mock=async()=>({ok:false,status:401,json:async()=>({error:{code:'invalid_key',message:key}})});
  await assert.rejects(()=>cloudChat({provider:'openai',model:'test',question:'Hi',key},mock),e=>!e.message.includes(key));
});
test('no JCode or persistent chat/credential storage, automatic raw upload or scientific refit',async()=>{
  const ui=await src('ai-assistant.js'),rt=await src('ai-runtime.js');
  for(const code of [ui,rt]) {
    assert.doesNotMatch(code,/localStorage|sessionStorage|indexedDB|document\.cookie|ai-assistant-bridge|ai-companion|jcode login|child_process|exec\(/i);
    assert.doesNotMatch(code,/readAsArrayBuffer|readAsText|FormData\(/i);
  }
  assert.match(ui,/textContent=state\.includeContext\?JSON\.stringify/);
  assert.match(ui,/await import\(WEBLLM_IMPORT\)/);
  assert.match(ui,/engine\.chat\.completions\.create/);
});
