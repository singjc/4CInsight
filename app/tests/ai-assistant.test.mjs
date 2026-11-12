import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {
  detectedOs,osInstallInstructions,providerInstructions,safeAiContext,validateAiQuestion,
  aiRequestPayload,aiAssistantView,askCompanion,receiveCompanionMessage,validAiEnvelope,
} from '../ai-assistant.js';
const src=file=>readFile(new URL(`../${file}`,import.meta.url),'utf8');

test('OS detection is advisory and setup never pretends browsers can install native tools',()=>{
  assert.equal(detectedOs('X11; Linux x86_64'),'linux');
  assert.equal(detectedOs('Macintosh'),'mac');
  assert.equal(detectedOs('Windows NT 10.0'),'windows');
  for(const os of ['linux','mac','windows']) {
    const x=osInstallInstructions(os);
    assert.ok(x.note.length>35);
    assert.doesNotMatch(x.command,/curl[^\n]*\|\s*bash|iex/);
  }
});
test('provider setup is JCode-based without keys in the page',()=>{
  for(const p of ['openai','claude','ollama'])assert.match(providerInstructions(p).command,new RegExp(`jcode login --provider ${p}`));
  const html=aiAssistantView();
  assert.match(html,/Check connection/);
  assert.match(html,/Open JCode sign-in/);
  assert.match(html,/No server, access token, or manual bridge/);
  assert.doesNotMatch(html,/Session access token|Paste the token|python3 tools\/ai-assistant-bridge/);
});
test('explicit context contains only fixed aggregate fields',()=>{
  const s=safeAiContext({integrative:true,compare:true,pathways:true,pathwayCount:1713,pathwayEvidence:'exploratory',patient_id:'SECRET',files:['private.tsv']});
  assert.deepEqual(Object.keys(s),['context_type','integrative_result_available','compare_groups_result_available','pathway_result_available','pathway_sets_tested','pathway_evidence_type']);
  assert.doesNotMatch(JSON.stringify(s),/SECRET|private\.tsv/);
});
test('limits on provider, model, prompt and history stay in force',()=>{
  assert.equal(validateAiQuestion(' x '),'x');
  assert.throws(()=>validateAiQuestion(''),/Enter a question/);
  assert.throws(()=>validateAiQuestion('x'.repeat(4001)),/4,000/);
  assert.throws(()=>aiRequestPayload({message:'hi',provider:'unknown'}),/Choose OpenAI/);
  assert.throws(()=>aiRequestPayload({message:'hi',provider:'openai',model:'bad;rm'}),/Model ID/);
  assert.equal(aiRequestPayload({message:'hi',provider:'ollama',history:Array.from({length:9},()=>({role:'user',content:'x'}))}).history.length,6);
});
test('native-message client sends only allowlisted operations and resolves correlated replies',async()=>{
  const sent=[];const origin='https://singjc.github.io';const host={window:{location:{origin},postMessage:(msg,to)=>sent.push({msg,to})},setTimeout,clearTimeout};
  await assert.rejects(()=>askCompanion('arbitrary',{},300,host),/Unsupported companion action/);
  const promise=askCompanion('status',{},2000,host);
  assert.equal(sent.length,1);assert.equal(sent[0].msg.op,'status');assert.equal(sent[0].to,origin);
  assert.equal(receiveCompanionMessage({source:host.window,origin,data:{channel:'wrong',direction:'response',id:sent[0].msg.id,ok:true}},host),false);
  assert.equal(receiveCompanionMessage({source:host.window,origin,data:{channel:'4c-insight-ai-v2',direction:'response',id:sent[0].msg.id,ok:true,result:{installed:true}}},host),true);
  assert.deepEqual(await promise,{installed:true});
});
test('native client rejects messages from foreign origin or window',()=>{
  assert.equal(validAiEnvelope({channel:'4c-insight-ai-v2',direction:'response',id:'a2-1-x',ok:true}),true);
  assert.equal(validAiEnvelope({channel:'bad',direction:'response',id:'a2-1-x',ok:true}),false);
});
test('AI tab restores independently but no credentials, chat or tokens persist',async()=>{
  const code=await src('ai-assistant.js');
  const main=await src('main.js');
  const html=await src('index.html');
  assert.match(main,/\['ai', 'AI Assistant', 'A'\]/);
  assert.match(main,/mountAiAssistant/);
  assert.match(html,/ai-assistant\.css\?v=0\.22\.8/);
  assert.doesNotMatch(code,/\blocalStorage\b|\bindexedDB\b|\bsessionStorage\b|document\.cookie|X-4C-Session|AI_BRIDGE_URL/);
  assert.doesNotMatch(code,/\bfetch\s*\(/);
  assert.doesNotMatch(main,/aiToken:|aiHistory:|modelApiKey:/);
});
