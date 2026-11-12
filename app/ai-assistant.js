/**
 * 4C Insight AI Assistant A1. Voluntary localhost JCode bridge only.
 * Credentials, source files and local workspace data never enter this module.
 * Chat and the bridge access token live in this page's JS memory only.
 */
export const AI_BRIDGE_URL = 'http://127.0.0.1:8765';
export const AI_PROVIDERS = Object.freeze(['openai', 'claude', 'ollama']);
export const AI_JCODE_DOCS = 'https://jcode.sh/docs';

export function detectedOs(agent = '') {
  const ua = String(agent).toLowerCase();
  if (/windows|win32|win64/.test(ua)) return 'windows';
  if (/macintosh|mac os|darwin/.test(ua)) return 'mac';
  if (/linux|x11|android/.test(ua)) return 'linux';
  return 'unknown';
}
export function osInstallInstructions(os) {
  if (os === 'windows') return {
    label: 'Windows 11 · PowerShell 5.1+',
    command: 'Invoke-WebRequest https://jcode.sh/install.ps1 -OutFile "$env:TEMP\\jcode-install.ps1"; notepad "$env:TEMP\\jcode-install.ps1"; & "$env:TEMP\\jcode-install.ps1"',
    note: 'Download and review the official PowerShell script before running it. Windows requires a local Python 3 interpreter to start the optional bridge.',
  };
  if (os === 'mac') return {
    label: 'macOS · Terminal',
    command: 'curl -fsSLo /tmp/jcode-install.sh https://jcode.sh/install && less /tmp/jcode-install.sh && bash /tmp/jcode-install.sh',
    note: 'Review the official installer before running it. Homebrew is also supported: brew tap 1jehuang/jcode && brew install jcode.',
  };
  if (os === 'linux') return {
    label: 'Linux · Terminal',
    command: 'curl -fsSLo /tmp/jcode-install.sh https://jcode.sh/install && less /tmp/jcode-install.sh && bash /tmp/jcode-install.sh',
    note: 'Review the official installer before running it. The architecture and runtime dependencies must match the downloaded JCode release.',
  };
  return {label:'Select your operating system', command:'',note:'Browser platform detection is approximate. Select Linux, macOS, or Windows manually.'};
}
export function providerInstructions(provider) {
  if (provider === 'claude') return {name:'Claude (Anthropic)', command:'jcode login --provider claude', note:'Sign in through JCode. Subscription and API access have different terms and billing.'};
  if (provider === 'ollama') return {name:'Ollama local models', command:'ollama list\njcode login --provider ollama\n# Choose a model you installed with: ollama pull <model-name>', note:'Install and start Ollama separately. The chosen model must be available locally. Model downloads use disk and memory.'};
  return {name:'OpenAI / ChatGPT / Codex', command:'jcode login --provider openai', note:'Use JCode’s own account login. ChatGPT subscription access and OpenAI API billing are not interchangeable.'};
}
function esc(s) {return String(s ?? '').replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;').replace(/'/g,'&#39;');}
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
let memory = {os:'', provider:'openai', model:'', token:'', connected:false, installed:false, version:'', status:'Not connected', showContext:false, history:[], busy:false};
function initialOs() { return detectedOs(typeof navigator !== 'undefined' ? `${navigator.userAgent || ''} ${navigator.platform || ''}` : ''); }
export function aiAssistantView() {
  if (!memory.os) memory.os = initialOs();
  const install = osInstallInstructions(memory.os);
  const selected = providerInstructions(memory.provider);
  return `<section class="page-heading"><div><span class="eyebrow">AI Assistant</span><h2>Ask questions about your analysis</h2><p>Use JCode with OpenAI, Claude, or an Ollama model. You control each message and any summary shared with the model.</p></div><div class="local-card"><strong>Optional local connection</strong><span>No background model request · No raw source files sent automatically</span></div></section>
  <section class="card ai-card"><div class="card-heading"><div><h3>1. Install or find JCode</h3><p>JCode is a separate program. A website cannot inspect or install software on your device by itself.</p></div><a href="${AI_JCODE_DOCS}" target="_blank" rel="noopener noreferrer">Official JCode docs</a></div>
    <div class="ai-controls"><label>Operating system<select id="ai-os"><option value="linux" ${memory.os==='linux'?'selected':''}>Linux</option><option value="mac" ${memory.os==='mac'?'selected':''}>macOS</option><option value="windows" ${memory.os==='windows'?'selected':''}>Windows</option><option value="unknown" ${memory.os==='unknown'?'selected':''}>Select OS</option></select></label><span class="ai-note">Detected OS is a suggestion only. Confirm your system and CPU architecture.</span></div>
    <p class="ai-note">${esc(install.note)}</p><div class="ai-command"><code id="ai-install-command">${esc(install.command || 'Select your OS.')}</code><button type="button" class="secondary-button" id="ai-copy-install">Copy instructions</button></div>
    <p class="ai-note">Run <code>jcode --version</code> locally to check the installation. The connection check below can detect JCode only when you start the optional bridge.</p></section>
  <section class="card ai-card"><div class="card-heading"><div><h3>2. Connect a model provider</h3><p>Configure credentials directly in JCode. Never paste API keys into this web page.</p></div></div>
    <div class="ai-controls"><label>Provider<select id="ai-provider"><option value="openai" ${memory.provider==='openai'?'selected':''}>OpenAI / ChatGPT</option><option value="claude" ${memory.provider==='claude'?'selected':''}>Claude</option><option value="ollama" ${memory.provider==='ollama'?'selected':''}>Ollama</option></select></label><label>Model ID (optional)<input id="ai-model" maxlength="100" autocomplete="off" value="${esc(memory.model)}" placeholder="Use JCode default model"/></label></div>
    <div class="ai-command"><code id="ai-login-command">${esc(selected.command)}</code><button type="button" class="secondary-button" id="ai-copy-login">Copy provider setup</button></div>
    <p class="ai-note" id="ai-provider-note">${esc(selected.note)}</p><p class="ai-note">After login, run <code>jcode auth-test --all-configured</code> in a terminal to check provider access. For Ollama, use <code>ollama list</code> or <code>ollama pull</code> in the terminal.</p></section>
  <section class="card ai-card"><div class="card-heading"><div><h3>3. Connect the optional local bridge</h3><p>Start the bridge on your own computer. It checks JCode and sends only approved chat prompts.</p></div></div>
    <div class="ai-command"><code>python3 tools/ai-assistant-bridge/bridge.py</code><button class="secondary-button" type="button" id="ai-copy-bridge">Copy command</button></div>
    <p class="ai-note">On Windows, use <code>py -3 tools/ai-assistant-bridge/bridge.py</code>. Start it from the 4C Insight source folder. The bridge displays a one-time access token in your terminal. This is a local helper, not a server upload or background installation.</p>
    <div class="ai-controls"><label>Session access token<input id="ai-token" type="password" autocomplete="off" spellcheck="false" placeholder="Paste the token from the bridge terminal" value="${esc(memory.token)}"/></label><button type="button" id="ai-check" class="secondary-button">Check JCode connection</button></div>
    <p id="ai-status" role="status" aria-live="polite" class="ai-status">${esc(memory.status)}${memory.version?` · ${esc(memory.version)}`:''}</p>
    <p class="ai-note">The token stays in this browser tab's memory; it is not saved in project state, History, or local storage. Access is restricted to this page's origin and to a loopback address. A browser may still block private-network requests.</p></section>
  <section class="card ai-card"><div class="card-heading"><div><h3>Assistant chat</h3><p>Chat requests JCode with its tool profile disabled. Existing JCode user-level hooks are outside this bridge's control.</p></div><button id="ai-clear-chat" class="secondary-button" type="button">Clear conversation</button></div>
    <div id="ai-chat" class="ai-chat" role="log" aria-live="polite">${memory.history.length?memory.history.map(entry=>`<div class="ai-message ${entry.role==='user'?'ai-user':'ai-assistant'}"><strong>${entry.role==='user'?'You':'Assistant'}</strong><p>${esc(entry.content)}</p></div>`).join(''):'<p class="ai-note">After you connect, ask a question about pathways, sample coverage, plots, or analysis methods.</p>'}</div>
    <label class="ai-context-choice"><input id="ai-include-context" type="checkbox" ${memory.showContext?'checked':''}/> Include the aggregate analysis summary shown below with my next message</label>
    <pre id="ai-context-preview" class="ai-context-preview">No summary selected</pre>
    <label for="ai-question">Your question</label><textarea id="ai-question" rows="3" maxlength="4000" placeholder="How should I interpret an exploratory pathway network?"></textarea>
    <div class="ai-actions"><button class="primary-button" id="ai-send" type="button" ${memory.busy?'disabled':''}>${memory.busy?'Waiting for model…':'Send to JCode'}</button><span class="ai-note">Cloud providers receive your message and any approved summary. Ollama can remain local.</span></div></section>`;
}

export function mountAiAssistant(root, {summary={}}={}) {
  const el = id=>root.querySelector(`#${id}`);
  const context = safeAiContext(summary);
  const rerender = () => { root.innerHTML = aiAssistantView(); mountAiAssistant(root,{summary}); };
  const status = msg=>{memory.status=msg;const tag=el('ai-status');if(tag)tag.textContent=`${msg}${memory.version?' · '+memory.version:''}`;};
  el('ai-os')?.addEventListener('change',e=>{memory.os=e.currentTarget.value;rerender();});
  el('ai-provider')?.addEventListener('change',e=>{memory.provider=e.currentTarget.value;rerender();});
  el('ai-model')?.addEventListener('input',e=>{memory.model=e.currentTarget.value;});
  el('ai-token')?.addEventListener('input',e=>{memory.token=e.currentTarget.value; memory.connected=false;});
  const copy=(id,get)=>el(id)?.addEventListener('click',()=>{if(typeof navigator!=='undefined'&&navigator.clipboard)void navigator.clipboard.writeText(get()).catch(()=>status('Clipboard unavailable. Select and copy the command manually.'));});
  copy('ai-copy-install',()=>osInstallInstructions(memory.os).command);
  copy('ai-copy-login',()=>providerInstructions(memory.provider).command);
  copy('ai-copy-bridge',()=>memory.os==='windows'?'py -3 tools/ai-assistant-bridge/bridge.py':'python3 tools/ai-assistant-bridge/bridge.py');
  const preview=el('ai-context-preview');
  const refreshPreview=()=>{if(preview)preview.textContent=memory.showContext?JSON.stringify(context,null,2):'No analysis summary will be sent.';};
  el('ai-include-context')?.addEventListener('change',e=>{memory.showContext=e.currentTarget.checked;refreshPreview();});
  refreshPreview();
  el('ai-clear-chat')?.addEventListener('click',()=>{memory.history=[];rerender();});
  el('ai-check')?.addEventListener('click',async()=>{
    if(!memory.token || memory.token.length < 20) {status('Enter the bridge token from your terminal first.');return;}
    status('Checking local bridge…');
    try {
      const data=await requestBridge('/status', null, memory.token, 8000);
      memory.installed=Boolean(data.installed);memory.version=String(data.version||'').slice(0,100);memory.connected=memory.installed;
      status(data.installed?'JCode detected. Choose a provider and send a question.':'Bridge running; JCode was not found on its PATH. Install JCode and restart the bridge.');
    } catch(e){memory.connected=false;status(`Bridge unavailable: ${e.message}`);}
  });
  el('ai-send')?.addEventListener('click',async()=>{
    if(memory.busy)return;
    if(!memory.connected){status('Connect the bridge and confirm JCode installation first.');return;}
    let payload;
    try {payload=aiRequestPayload({message:el('ai-question').value,provider:memory.provider,model:memory.model,context:memory.showContext?context:null,history:memory.history});}
    catch(error){status(error.message);return;}
    memory.busy=true;status('JCode is responding…');
    const prior=memory.history.slice();
    try {
      const reply=await requestBridge('/chat',payload,memory.token,125000);
      if(typeof reply.answer!=='string'||!reply.answer.trim())throw new Error('No response from JCode');
      memory.history=[...prior,{role:'user',content:payload.message},{role:'assistant',content:reply.answer.slice(0,12000)}].slice(-16);
      status('Response received. No 4C files or analysis settings were changed.');
    } catch(error) {memory.history=prior;status(`Chat failed: ${error.message}`);}
    finally{memory.busy=false;rerender();}
  });
}

export async function requestBridge(path,payload,token,timeoutMs=8000,fetcher=globalThis.fetch){
  if(!['/status','/chat'].includes(path))throw new Error('Unsupported bridge operation');
  if(typeof token!=='string'||token.length<20)throw new Error('Missing session access token');
  const controller=new AbortController();
  const timer=setTimeout(()=>controller.abort(),timeoutMs);
  try {
    const response=await fetcher(`${AI_BRIDGE_URL}${path}`,{
      method:payload==null?'GET':'POST',mode:'cors',cache:'no-store',credentials:'omit',redirect:'error',
      headers:{'X-4C-Session':token,...(payload==null?{}:{'Content-Type':'application/json'})},
      ...(payload==null?{}:{body:JSON.stringify(payload)}),signal:controller.signal,
    });
    if(!response.ok)throw new Error(`Local bridge HTTP ${response.status}`);
    return await response.json();
  }catch(error){if(error.name==='AbortError')throw new Error('Local bridge timed out');throw error;}
  finally{clearTimeout(timer);}
}
