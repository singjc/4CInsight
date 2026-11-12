import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {aiAssistantView} from '../ai-assistant.js';
import {OLLAMA_BASE,RECOMMENDED_MODELS,cleanQuestion,cleanModelId,safeAiContext,promptMessages,probeOllama,installedOllamaModels,pullRecommendedModel,ollamaChat,ollamaRequest} from '../ai-runtime.js';
const src=async file=>readFile(new URL(`../${file}`,import.meta.url),'utf8');
function reply(payload,ok=true,status=200){return {ok,status,json:async()=>payload};}
function streaming(packets,split=11){
  const data=new TextEncoder().encode(packets.map(item=>JSON.stringify(item)+'\n').join(''));
  return {ok:true,status:200,body:new ReadableStream({start(controller){for(let i=0;i<data.length;i+=split)controller.enqueue(data.slice(i,i+split));controller.close();}})};
}
test('AI is only local Ollama and retains application navigation',async()=>{
  const html=aiAssistantView();
  assert.match(html,/Local Ollama AI/);assert.match(html,/Check connection again/);
  assert.match(html,/Ollama installed but not connecting/);
  assert.doesNotMatch(html,/OpenAI API|Claude API|OpenRouter|WebGPU|JCode|Paste.*API key/);
  const main=await src('main.js'),index=await src('index.html');
  assert.match(main,/\['ai', 'AI Assistant', 'A'\]/);
  assert.match(main,/ai: aiAssistantView/);assert.match(main,/mountAiAssistant/);
  assert.match(index,/ai-assistant\.css\?v=0\.22\.8/);
});
test('all network access is restricted to the loopback Ollama API',async()=>{
  assert.equal(OLLAMA_BASE,'http://127.0.0.1:11434');
  await assert.rejects(()=>ollamaRequest('/api/delete',{method:'DELETE'},async()=>{}),/Unsupported/);
  let endpoint,options;
  await probeOllama(async(url,opts)=>{endpoint=url;options=opts;return reply({version:'0.13.0'});});
  assert.equal(endpoint,OLLAMA_BASE+'/api/version');
  assert.equal(options.credentials,'omit');assert.equal(options.referrerPolicy,'no-referrer');
  assert.equal(options.cache,'no-store');
  assert.equal(options.targetAddressSpace,'loopback');
  const code=(await src('ai-runtime.js'))+(await src('ai-assistant.js'));
  assert.doesNotMatch(code,/api\.openai\.com|api\.anthropic\.com|openrouter|web-llm|esm\.run|cloudChat|Authorization:\s*`Bearer/i);
});
test('probe detects running API, but cannot claim an executable is installed',async()=>{
  assert.equal(await probeOllama(async()=>reply({version:'1.2.3'})),'1.2.3');
  await assert.rejects(()=>probeOllama(async()=>{throw Error('failed');}),/stopped.*browser may block/);
  await assert.rejects(()=>probeOllama(async()=>reply({status:'up'})),/Ollama version/);
});
test('local model list excludes cloud, remote entries, invalid names and duplicates',async()=>{
  const models=[{name:'qwen2.5:1.5b',size:2000},{name:'qwen2.5:1.5b'}, {name:'gemma:cloud'}, {name:'org/model:cloud'}, {name:'remote:test',remote_host:'https://example.com'}, {name:'abc;bad'}, {name:'llama3.2:3b',size:3000}];
  const records=await installedOllamaModels(async(url)=>reply({models}));
  assert.deepEqual(records.map(m=>m.name),['qwen2.5:1.5b','llama3.2:3b']);
  assert.equal(records[0].size,2000);
  assert.throws(()=>cleanModelId('abc:cloud'),/Cloud models/);
  assert.throws(()=>cleanModelId('abc; rm -rf'),/Invalid model/);
});
test('only catalog-approved local model can be downloaded explicitly',async()=>{
  let options;
  const fetcher=async(url,opts)=>{assert.equal(url,OLLAMA_BASE+'/api/pull');options=opts;return streaming([{status:'downloading',total:200,completed:100},{status:'downloading',total:200,completed:200},{status:'success'}],17);};
  const events=[];
  assert.equal(await pullRecommendedModel(RECOMMENDED_MODELS[0].id,{fetcher,onProgress:p=>events.push(p)}),RECOMMENDED_MODELS[0].id);
  assert.equal(JSON.parse(options.body).stream,true);
  assert.equal(events[0].progress,50);
  assert.equal(events.at(-1).status,'success');
  await assert.rejects(()=>pullRecommendedModel('model:cloud',{fetcher}),/recommended/);
  await assert.rejects(()=>pullRecommendedModel('arbitrary:latest',{fetcher}),/recommended/);
});
test('failed or incomplete model download is never claimed successful',async()=>{
  await assert.rejects(()=>pullRecommendedModel(RECOMMENDED_MODELS[0].id,{fetcher:async()=>streaming([{status:'downloading'}])}),/without confirmation/);
  await assert.rejects(()=>pullRecommendedModel(RECOMMENDED_MODELS[0].id,{fetcher:async()=>streaming([{error:'model unavailable'}])}),/download failed/);
});
test('chat uses tool-free, local, bounded streaming with no remote fallback',async()=>{
  let options;
  const answer=await ollamaChat({model:'qwen2.5:1.5b',question:'Hello',fetcher:async(url,opts)=>{
    assert.equal(url,OLLAMA_BASE+'/api/chat');options=opts;
    return streaming([{message:{content:'Local '}},{message:{content:'answer'}},{done:true}],9);
  }});
  assert.equal(answer,'Local answer');
  const body=JSON.parse(options.body);
  assert.equal(body.stream,true);
  assert.equal(body.think,false);
  assert.equal(body.model,'qwen2.5:1.5b');
  assert.equal(body.tools,undefined);
  assert.equal(body.messages.at(-1).content,'Hello');
  assert.equal(options.credentials,'omit');
});
test('aggregates never transmit source filenames, patient IDs, or unknown user fields',()=>{
  const input={integrative:true,compare:true,pathways:true,pathwayCount:1713,pathwayEvidence:'exploratory',patientID:'PATIENT-PRIVATE',sources:['secret.raw'],key:'SECRET'};
  const ctx=safeAiContext(input);
  assert.deepEqual(Object.keys(ctx),['context_type','integrative_result_available','compare_groups_result_available','pathway_result_available','pathway_sets_tested','pathway_evidence_type']);
  const text=promptMessages('Explain?',[],input).at(-1).content;
  assert.match(text,/1713/);assert.doesNotMatch(text,/PATIENT-PRIVATE|secret\.raw|SECRET/);
  assert.equal(promptMessages('Hi').at(-1).content,'Hi');
  assert.throws(()=>cleanQuestion(''),/Enter a question/);
  assert.throws(()=>cleanQuestion('x'.repeat(4001)),/4,000/);
});
test('chat remains current-page-only, never accesses source files or stores credentials',async()=>{
  const code=(await src('ai-runtime.js'))+'\n'+(await src('ai-assistant.js'));
  assert.doesNotMatch(code,/localStorage|sessionStorage|indexedDB|document\.cookie|readAsArrayBuffer|readAsText|FormData\(|child_process|window\.open\(/i);
  assert.match(code,/safeAiContext\(summary\)/);
  assert.match(code,/root\.querySelector/);
  assert.match(code,/installedOllamaModels\(/);
});
