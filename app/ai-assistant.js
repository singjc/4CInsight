/** 4C Insight A2: on-demand native messaging. No HTTP listener, user tokens, or credential storage. */
export const AI_PROVIDERS=Object.freeze(['openai','claude','ollama']);
export const AI_JCODE_DOCS='https://jcode.sh/docs';
export const AI_CONNECTOR_DOCS='https://github.com/singjc/4CInsight/blob/main/docs/ai_assistant.md';
export function detectedOs(agent='') {
  const ua=String(agent).toLowerCase();
  if (/windows|win32|win64/.test(ua)) return 'windows';
  if (/macintosh|mac os|darwin/.test(ua)) return 'mac';
  if (/linux|x11|android/.test(ua)) return 'linux';
  return 'unknown';
}
export function osInstallInstructions(os) {
  if(os==='linux')return {label:'Linux',command:'Open the verified 4C AI Companion installer. It registers Firefox Native Messaging. Installation does not start a background service.',note:'Development build: see docs/ai_assistant.md. A normal Firefox release requires a signed extension.'};
  if(os==='mac')return {label:'macOS',command:'A signed 4C AI Companion installer is required before this connection works.',note:'macOS packaging and permission checks are not yet released.'};
  if(os==='windows')return {label:'Windows',command:'A signed 4C AI Companion installer is required before this connection works.',note:'Windows packaging and registry integration are not yet released.'};
  return {label:'Choose operating system',command:'',note:'Operating-system detection is approximate.'};
}
export function providerInstructions(provider) {
  if(provider==='claude')return {name:'Claude',command:'jcode login --provider claude',note:'Click Open JCode sign-in. JCode stores credentials outside 4C Insight.'};
  if(provider==='ollama')return {name:'Ollama',command:'jcode login --provider ollama',note:'Ollama must already be installed and running. Choose a local model in JCode.'};
  return {name:'OpenAI',command:'jcode login --provider openai',note:'Click Open JCode sign-in. API billing and subscription access can differ.'};
}
function esc(s){return String(s??'').replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;').replace(/'/g,'&#39;')}
export function safeAiContext(input = {}) {
  return {
    context_type: 'aggregate_4c_insight_summary_v1',
    integrative_result_available: Boolean(input.integrative),
    compare_groups_result_available: Boolean(input.compare),
    pathway_result_available: Boolean(input.pathways),
    pathway_sets_tested: Number.isSafeInteger(input.pathwayCount) && input.pathwayCount >= 0 ? Math.min(input.pathwayCount, 1000000) : null,
    pathway_evidence_type: ['exploratory','defined_comparison'].includes(input.pathwayEvidence) ? input.pathwayEvidence : 'unverified',
  };
}
export function validateAiQuestion(value) {
  const question = String(value ?? '').trim();
  if (!question || question.length > 4000) throw new Error('Enter a question of 1–4,000 characters.');
  return question;
}
export function aiRequestPayload({message, provider, model='', context=null, history=[]}) {
  if (!AI_PROVIDERS.includes(provider)) throw new Error('Choose OpenAI, Claude, or Ollama.');
  if (model && !/^[a-zA-Z0-9._:/+-]{1,100}$/.test(model)) throw new Error('Model ID contains unsupported characters.');
  return {message:validateAiQuestion(message), provider, model, context, history: history.slice(-6).map(entry=>({role:entry.role==='assistant'?'assistant':'user',content:String(entry.content||'').slice(0,2000)}))};
}

const channel='4c-insight-ai-v2';
let nextId=0;
const pending=new Map();
export function validAiEnvelope(msg){return Boolean(msg && msg.channel===channel && msg.direction==='response' && typeof msg.id==='string' && msg.id.length<=80 && (typeof msg.ok==='boolean'));}
export function askCompanion(op,payload={},timeout=120000,host=globalThis){
  if(!['status','chat','login'].includes(op))return Promise.reject(new Error('Unsupported companion action'));
  if(typeof host.window?.postMessage!=='function')return Promise.reject(new Error('Browser messaging is unavailable'));
  const id=`a2-${++nextId}-${Math.random().toString(36).slice(2,10)}`;
  return new Promise((resolve,reject)=>{
    const timer=host.setTimeout(()=>{pending.delete(id);reject(new Error('AI Companion did not respond. Check the extension setup.'));},timeout);
    pending.set(id,{resolve,reject,timer});
    host.window.postMessage({channel,direction:'request',id,op,payload},host.window.location.origin);
  });
}
export function receiveCompanionMessage(event,host=globalThis){
  if(event?.source!==host.window || event.origin!==host.window.location.origin || !validAiEnvelope(event.data))return false;
  const task=pending.get(event.data.id);if(!task)return false;
  pending.delete(event.data.id);host.clearTimeout(task.timer);
  if(event.data.ok)task.resolve(event.data.result||{});
  else task.reject(new Error(String(event.data.error||'AI Companion unavailable').slice(0,200)));
  return true;
}
let listenerInstalled=false;
function listen(){if(listenerInstalled||typeof window==='undefined')return;window.addEventListener('message',receiveCompanionMessage);listenerInstalled=true;}
let memory={os:'',provider:'openai',model:'',connected:false,installed:false,version:'',status:'Checking on-demand connection…',checkedOnce:false,showContext:false,history:[],busy:false};
function initialOs(){return detectedOs(typeof navigator!=='undefined'?`${navigator.userAgent||''} ${navigator.platform||''}`:'');}
export function aiAssistantView(){
  if(!memory.os)memory.os=initialOs();
  const setup=osInstallInstructions(memory.os);
  return `<section class="page-heading"><div><span class="eyebrow">AI Assistant</span><h2>Ask questions about your analysis</h2><p>JCode connects to your chosen model. No server, access token, or manual bridge launch is needed after the one-time companion setup.</p></div><div class="local-card"><strong>On-demand local connection</strong><span>Chat only · User-approved context · No background service</span></div></section>
  <section class="card ai-card"><div class="card-heading"><div><h3>1. Connect JCode</h3><p>The AI Companion starts when you request it. It does not run as a separate server.</p></div><a href="${AI_CONNECTOR_DOCS}" target="_blank" rel="noopener noreferrer">Set up AI Companion</a></div>
    <div class="ai-controls"><label>Operating system<select id="ai-os"><option value="linux" ${memory.os==='linux'?'selected':''}>Linux</option><option value="mac" ${memory.os==='mac'?'selected':''}>macOS</option><option value="windows" ${memory.os==='windows'?'selected':''}>Windows</option><option value="unknown" ${memory.os==='unknown'?'selected':''}>Choose OS</option></select></label>
    <button id="ai-check" type="button" class="primary-button">Check connection</button><span id="ai-status" class="ai-status" role="status" aria-live="polite">${esc(memory.status)}${memory.version?' · '+esc(memory.version):''}</span></div>
    <p class="ai-note">${esc(setup.command)} ${esc(setup.note)} <a href="${AI_JCODE_DOCS}" target="_blank" rel="noopener noreferrer">Get JCode</a>. The browser must approve the extension and native component. No program is installed without your consent.</p>
  </section>
  <section class="card ai-card"><div class="card-heading"><div><h3>2. Choose your model</h3><p>Sign in through JCode. Credentials are not sent to 4C Insight.</p></div></div>
    <div class="ai-controls"><label>Provider<select id="ai-provider"><option value="openai" ${memory.provider==='openai'?'selected':''}>OpenAI / ChatGPT</option><option value="claude" ${memory.provider==='claude'?'selected':''}>Claude</option><option value="ollama" ${memory.provider==='ollama'?'selected':''}>Ollama</option></select></label><label>Model ID (optional)<input id="ai-model" maxlength="100" autocomplete="off" value="${esc(memory.model)}" placeholder="JCode default model" /></label><button id="ai-login" type="button" class="secondary-button">Open JCode sign-in</button></div>
    <p id="ai-provider-note" class="ai-note">${esc(providerInstructions(memory.provider).note)}</p>
  </section>
  <section class="card ai-card"><div class="card-heading"><div><h3>3. Chat</h3><p>Only your question and any approved aggregate summary are sent to the model. JCode is called with tools disabled.</p></div><button id="ai-clear-chat" type="button" class="secondary-button">Clear chat</button></div>
    <div id="ai-chat" class="ai-chat" role="log" aria-live="polite">${memory.history.length?memory.history.map(m=>`<div class="ai-message ${m.role==='user'?'ai-user':'ai-assistant'}"><strong>${m.role==='user'?'You':'Assistant'}</strong><p>${esc(m.content)}</p></div>`).join(''):'<p class="ai-note">Ask a question after the connection check succeeds.</p>'}</div>
    <label class="ai-context-choice"><input id="ai-include-context" type="checkbox" ${memory.showContext?'checked':''}/> Include the aggregate analysis summary shown below with my next message</label>
    <pre id="ai-context-preview" class="ai-context-preview">No analysis summary will be sent.</pre>
    <label for="ai-question">Your question</label><textarea id="ai-question" rows="3" maxlength="4000" placeholder="How should I interpret this pathway network?"></textarea>
    <div class="ai-actions"><button id="ai-send" type="button" class="primary-button" ${memory.busy?'disabled':''}>${memory.busy?'Waiting…':'Send to JCode'}</button><span class="ai-note">Existing JCode user-level hooks may still run. Tool-free chat is not an OS sandbox.</span></div>
  </section>`;
}
export function mountAiAssistant(root,{summary={}}={}){
  listen();const el=id=>root.querySelector(`#${id}`),context=safeAiContext(summary);
  const rerender=()=>{root.innerHTML=aiAssistantView();mountAiAssistant(root,{summary});};
  const status=m=>{memory.status=m;const node=el('ai-status');if(node)node.textContent=`${m}${memory.version?' · '+memory.version:''}`;};
  el('ai-os')?.addEventListener('change',e=>{memory.os=e.currentTarget.value;rerender();});
  el('ai-provider')?.addEventListener('change',e=>{memory.provider=e.currentTarget.value;rerender();});
  el('ai-model')?.addEventListener('input',e=>{memory.model=e.currentTarget.value;});
  const preview=el('ai-context-preview');const draw=()=>{if(preview)preview.textContent=memory.showContext?JSON.stringify(context,null,2):'No analysis summary will be sent.';};
  el('ai-include-context')?.addEventListener('change',e=>{memory.showContext=e.currentTarget.checked;draw();});draw();
  el('ai-clear-chat')?.addEventListener('click',()=>{memory.history=[];rerender();});
  const check=async()=>{
    status('Checking on-demand connection…');try{const result=await askCompanion('status',{},8000);memory.installed=Boolean(result.installed);memory.connected=memory.installed;memory.version=String(result.version||'').slice(0,100);
      status(memory.installed?'JCode is ready. Choose a provider.':'AI Companion is available, but JCode is not installed. Open Get JCode.');}
    catch(_){memory.connected=false;status('AI Companion not detected. Complete the one-time setup and select Check connection.');}
  };
  el('ai-check')?.addEventListener('click',()=>void check());
  if(!memory.checkedOnce){memory.checkedOnce=true;void check();}
  el('ai-login')?.addEventListener('click',async()=>{
    if(!memory.connected){status('Connect JCode first.');return;}
    if(!window.confirm(`Open JCode sign-in for ${memory.provider}?`))return;
    status('Opening JCode sign-in…');try{const r=await askCompanion('login',{provider:memory.provider},15000);status(r.message||'JCode sign-in opened.');}
    catch(e){status(e.message);}
  });
  el('ai-send')?.addEventListener('click',async()=>{
    if(memory.busy)return;if(!memory.connected){status('Check the AI Companion connection first.');return;}
    let payload;try{payload=aiRequestPayload({message:el('ai-question').value,provider:memory.provider,model:memory.model,context:memory.showContext?context:null,history:memory.history});}
    catch(e){status(e.message);return;}
    memory.busy=true;status('JCode is responding…');const prior=[...memory.history];
    try{const r=await askCompanion('chat',payload,125000);if(typeof r.answer!=='string'||!r.answer.trim())throw Error('JCode returned no response');
      memory.history=[...prior,{role:'user',content:payload.message},{role:'assistant',content:r.answer.slice(0,12000)}].slice(-16);status('Response received. No project files were changed.');}
    catch(e){memory.history=prior;status(`Chat failed: ${e.message}`);}finally{memory.busy=false;rerender();}
  });
}
