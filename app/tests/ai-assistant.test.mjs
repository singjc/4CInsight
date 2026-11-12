import test from 'node:test'
import assert from 'node:assert/strict'
import {readFile} from 'node:fs/promises'
import {
  AI_BRIDGE_URL,detectedOs,osInstallInstructions,providerInstructions,
  safeAiContext,validateAiQuestion,aiRequestPayload,aiAssistantView,requestBridge,
} from '../ai-assistant.js'

const src=(file)=>readFile(new URL(`../${file}`,import.meta.url),'utf8')

test('platform detection is advisory and has OS-specific reviewed installer instructions',()=>{
  assert.equal(detectedOs('Mozilla/5.0 (X11; Linux x86_64)'),'linux')
  assert.equal(detectedOs('Macintosh; Intel Mac OS X'),'mac')
  assert.equal(detectedOs('Windows NT 10.0; Win64'),'windows')
  assert.equal(detectedOs(''),'unknown')
  for(const os of ['linux','mac','windows']){
    const inst=osInstallInstructions(os)
    assert.match(inst.command,/jcode\.sh\/install/)
    assert.match(inst.note,/review/i)
    assert.doesNotMatch(inst.command,/curl[^\n]*\|\s*(bash|sh)|iex/i)
  }
})
test('provider setup uses JCode login, not browser keys or implicit billing',()=>{
  for(const provider of ['openai','claude','ollama']){
    assert.match(providerInstructions(provider).command,new RegExp(`jcode login --provider ${provider}`))
  }
  const html=aiAssistantView()
  assert.match(html,/AI Assistant/)
  assert.match(html,/Check JCode connection/)
  assert.match(html,/Official JCode docs/)
  assert.match(html,/Session access token/)
  assert.match(html,/never paste API keys/i)
  assert.doesNotMatch(html,/api[_-]?key[\s:=]+sk-/i)
})
test('approved context contains only fixed aggregate fields, with no patient or gene identifiers',()=>{
  const summary=safeAiContext({integrative:true,compare:true,pathways:true,pathwayCount:1713,pathwayEvidence:'exploratory',patient_id:'PATIENT_X',files:['private.raw']})
  assert.deepEqual(Object.keys(summary),['context_type','integrative_result_available','compare_groups_result_available','pathway_result_available','pathway_sets_tested','pathway_evidence_type'])
  assert.equal(summary.pathway_sets_tested,1713)
  assert.doesNotMatch(JSON.stringify(summary),/PATIENT_X|private.raw/)
})
test('question, provider and model fields have strict input limits',()=>{
  assert.equal(validateAiQuestion('  Hello  '),'Hello')
  assert.throws(()=>validateAiQuestion(' '),/Enter a question/)
  assert.throws(()=>validateAiQuestion('x'.repeat(4001)),/4,000/)
  assert.throws(()=>aiRequestPayload({message:'ok',provider:'curl'}),/Choose OpenAI/)
  assert.throws(()=>aiRequestPayload({message:'ok',provider:'ollama',model:'my-model; rm -rf'}),/Model ID/)
  const payload=aiRequestPayload({message:'Hi',provider:'openai',history:Array.from({length:9},(_,i)=>({role:'user',content:String(i)}))})
  assert.equal(payload.history.length,6)
})
test('bridge client uses fixed loopback endpoints, no cookies, token header and explicit POST',async()=>{
  let call
  const fake=async(url,opts)=>{call={url,opts};return {ok:true,json:async()=>({bridge:'ready'})}}
  const token='T'.repeat(35)
  assert.deepEqual(await requestBridge('/status',null,token,5000,fake),{bridge:'ready'})
  assert.equal(call.url,`${AI_BRIDGE_URL}/status`)
  assert.equal(call.opts.credentials,'omit')
  assert.equal(call.opts.headers['X-4C-Session'],token)
  assert.equal(call.opts.method,'GET')
  await requestBridge('/chat',{provider:'openai',message:'Hello'},token,5000,fake)
  assert.equal(call.opts.method,'POST')
  assert.equal(JSON.parse(call.opts.body).message,'Hello')
  await assert.rejects(()=>requestBridge('/arbitrary',null,token,5000,fake),/Unsupported bridge operation/)
})
test('browser module does not store keys, tokens or chat in persistent storage',async()=>{
  const code=await src('ai-assistant.js')
  assert.doesNotMatch(code,/\blocalStorage\b|\bindexedDB\b|\bsessionStorage\b|document\.cookie|apiKey\s*[:=]/i)
  assert.doesNotMatch(code,/fetch\s*\(\s*['"]https:\/\/(api\.openai|api\.anthropic)/i)
  assert.match(code,/textContent=memory\.showContext\?JSON\.stringify/)
})
test('AI tab mounts independently and keeps the scientific result state out of chat history',async()=>{
  const main=await src('main.js')
  const html=await src('index.html')
  assert.match(main,/\['ai', 'AI Assistant', 'A'\]/)
  assert.match(main,/ai: aiAssistantView/)
  assert.match(main,/mountAiAssistant/)
  assert.match(main,/'ai', 'history'/)
  assert.match(html,/ai-assistant\.css\?v=0\.22\.8/)
  assert.doesNotMatch(main,/aiToken:|aiHistory:|modelApiKey:/)
})
