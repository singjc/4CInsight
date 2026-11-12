/** 4C Insight A3: browser-first AI. No JCode, native host, extension, or localhost bridge. */
import { WEBLLM_IMPORT, LOCAL_MODELS, MODEL_DOCS, browserSupport, cloudChat, cleanQuestion, cleanModelId, ollamaModels, promptMessages, safeAiContext, selectionFromCatalog } from './ai-runtime.js?v=0.22.8';
const esc = text => String(text ?? '').replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;').replace(/'/g,'&#39;');
const state = {
  mode: 'local', model: LOCAL_MODELS[0].id, provider: 'openai', cloudModel: '', key: '', consent: false,
  includeContext: false, busy: false, loadedModel: '', engine: null, loading: false, progress: '', status: '',
  messages: [], question: '', gpu: null, gpuChecked: false, ollama: [],
};
let webllm = null;
let loadPromise = null;
const providerLabel = { openai:'OpenAI API', claude:'Claude API', ollama:'Ollama (already installed)' };
const defaults = { openai:'gpt-5.6-terra', claude:'claude-sonnet-4-6', ollama:'' };
function localReady() { return Boolean(state.engine && state.loadedModel === state.model); }
function status(message) { state.status = message; const el = typeof document === 'undefined' ? null : document.querySelector('#ai-status'); if (el) el.textContent = message; }
function chatHtml() {
  return state.messages.length ? state.messages.map(entry => `<div class="ai-message ${entry.role==='user'?'ai-user':'ai-assistant'}"><strong>${entry.role==='user'?'You':'Assistant'}</strong><p>${esc(entry.content)}</p></div>`).join('') : '<p class="ai-note">Ask a question about an analysis. Your original source files are never attached automatically.</p>';
}
function localPanel() {
  const support = state.gpuChecked ? state.gpu : null;
  return `<div class="ai-setup-panel"><h3>1. Prepare on-device AI</h3><p class="ai-note">The model runs in this browser. No account, extension, or separate application is needed. The first download requires your approval.</p>
  <div class="ai-model-grid"><label for="ai-local-model">Model<select id="ai-local-model">${LOCAL_MODELS.map(m => `<option value="${esc(m.id)}" ${state.model===m.id?'selected':''}>${esc(m.label)}</option>`).join('')}</select></label>
  <div class="ai-model-detail">${esc(LOCAL_MODELS.find(m=>m.id===state.model)?.note || '')} Estimated model GPU memory: ~${esc(LOCAL_MODELS.find(m=>m.id===state.model)?.memoryMB)} MB. This is not the download size or a guarantee of compatibility.</div></div>
  <div class="ai-actions"><button type="button" id="ai-check-gpu" class="secondary-button">Check browser compatibility</button>
  <button type="button" id="ai-load" class="primary-button" ${state.loading || state.busy ? 'disabled' : ''}>${state.loading?'Preparing…':localReady()?'Model ready — reload':'Download and start model'}</button>
  <button type="button" id="ai-unload" class="secondary-button" ${state.engine ? '' : 'disabled'}>Unload from memory</button></div>
  <p class="ai-note" id="ai-gpu-status">${support ? esc(support.message) : 'Compatibility has not been checked. WebGPU is needed for local inference.'}</p>
  <div id="ai-load-progress" class="ai-note" role="status" aria-live="polite">${esc(state.progress)}</div>
  <p class="ai-note">Model software and weights are downloaded from external model/CDN hosts. Inference stays on-device. The browser may reuse cached files; clearing site data can remove them.</p></div>`;
}
function cloudPanel() {
  const isOllama = state.provider === 'ollama';
  return `<div class="ai-setup-panel"><h3>1. Connect an optional online or existing local provider</h3>
  <p class="ai-warning"><strong>Advanced mode:</strong> A ChatGPT or Claude subscription does not automatically provide API access. Direct API keys in browser pages are risky. For sensitive work, use on-device AI or a future trusted sign-in service instead.</p>
  <div class="ai-model-grid"><label>Provider<select id="ai-provider">${Object.entries(providerLabel).map(([id,label])=>`<option value="${id}" ${state.provider===id?'selected':''}>${esc(label)}</option>`).join('')}</select></label>
  <label>Model ID<input id="ai-cloud-model" value="${esc(state.cloudModel)}" maxlength="101" placeholder="${esc(defaults[state.provider])}" autocomplete="off"></label></div>
  ${isOllama ? `<p class="ai-note">Ollama is optional and must already be installed and running. A website cannot install or start it. The local server must allow this site's origin.</p>
    <button type="button" id="ai-ollama-list" class="secondary-button">Find running Ollama models</button>
    <div class="ai-note">${state.ollama.length ? `Available: ${state.ollama.map(esc).join(', ')}` : 'No Ollama models detected in this tab.'}</div>` : `<p class="ai-note">You need API access and a dedicated key. <a href="${MODEL_DOCS[state.provider]}" rel="noopener noreferrer" target="_blank">Manage your ${esc(state.provider)} API keys</a>. API usage can incur charges.</p>
   <label>API key for this tab only<input type="password" id="ai-cloud-key" value="" autocomplete="off" spellcheck="false" placeholder="${state.key?'Key is held in this tab':'Paste a restricted short-lived key'}" /></label>
   <div class="ai-actions"><button type="button" id="ai-forget-key" class="secondary-button">Forget API key</button><span class="ai-note">This site never saves your key. Browser extensions or injected scripts might still access it.</span></div>
   <label class="ai-context-choice"><input type="checkbox" id="ai-key-consent" ${state.consent?'checked':''}> I understand the risk of putting an API key into a webpage and accept direct browser requests.</label>`}
  <p class="ai-note">${isOllama?'Messages stay on your device when using a local Ollama model.':'Your question and any approved context are sent directly to the selected provider.'}</p></div>`;
}
export function aiAssistantView() {
  return `<section class="page-heading"><div><span class="eyebrow">AI Assistant</span><h2>Ask questions about your analysis</h2><p>Use on-device AI without installing a companion application or browser extension. Optional online models are available for advanced users.</p></div><div class="local-card"><strong>Browser-first AI</strong><span>Local by default · Context shared only with approval</span></div></section>
  <section class="card ai-card"><div class="card-heading"><div><h3>Choose where the model runs</h3><p>On-device is the recommended option.</p></div></div>
  <div class="ai-mode-tabs" role="group" aria-label="AI model location"><button id="ai-mode-local" class="${state.mode==='local'?'ai-mode-active':''}" type="button" aria-pressed="${state.mode==='local'}">On-device · Private</button><button id="ai-mode-cloud" class="${state.mode==='cloud'?'ai-mode-active':''}" type="button" aria-pressed="${state.mode==='cloud'}">Online or Ollama · Advanced</button></div>
  ${state.mode==='local' ? localPanel() : cloudPanel()}</section>
  <section class="card ai-card"><div class="card-heading"><div><h3>2. Ask your question</h3><p>Chat is held only in this open page. The assistant cannot read raw data files or run scientific tools.</p></div><button id="ai-clear-chat" type="button" class="secondary-button">Clear chat</button></div>
  <div class="ai-example-prompts" aria-label="Example questions">${['Explain my pathway results','What does a PCoA cluster mean?','What should I check next?'].map(p=>`<button type="button" data-ai-prompt="${esc(p)}" class="secondary-button">${esc(p)}</button>`).join('')}</div>
  <div class="ai-chat" id="ai-chat" role="log" aria-live="polite">${chatHtml()}</div>
  <label class="ai-context-choice"><input type="checkbox" id="ai-include-context" ${state.includeContext?'checked':''}> Include the approved aggregate analysis summary with my next message</label>
  <pre class="ai-context-preview" id="ai-context-preview">No analysis summary will be sent.</pre>
  <label for="ai-question">Your question</label><textarea id="ai-question" maxlength="4000" rows="3" placeholder="What does my pathway network show?">${esc(state.question)}</textarea>
  <div class="ai-actions"><button id="ai-send" type="button" class="primary-button" ${state.busy || state.loading?'disabled':''}>${state.busy?'Working…':'Send question'}</button><span id="ai-status" class="ai-note" role="status" aria-live="polite">${esc(state.status || 'Choose a model to begin.')}</span></div></section>`;
}
function updateProgress(progress) {
  const raw = progress?.text || (Number.isFinite(progress?.progress) ? `${Math.round(progress.progress*100)}%` : 'Preparing model');
  state.progress = String(raw).slice(0, 240);
  const el = typeof document === 'undefined' ? null : document.querySelector('#ai-load-progress'); if(el) el.textContent = state.progress;
}
async function checkGPU() {
  const n = globalThis.navigator;
  if (!browserSupport(n).webgpu) return { ok:false, message:'WebGPU is unavailable in this browser. Try a recent Chrome/Edge or supported Firefox version, or use an optional online model.' };
  try {
    const adapter = await n.gpu.requestAdapter({powerPreference:'high-performance'});
    if(!adapter) return {ok:false,message:'No compatible WebGPU adapter was found. This device cannot run this WebLLM mode.'};
    return {ok:true,message:'WebGPU adapter detected. Model loading still depends on graphics memory and driver support.'};
  } catch { return {ok:false,message:'WebGPU access failed. Check browser GPU settings and supported hardware.'}; }
}
async function loadLocal() {
  if (state.loading) return;
  state.loading = true;state.progress = 'Loading WebLLM runtime…';
  try {
    const gpu = await checkGPU();state.gpuChecked=true;state.gpu=gpu;
    if (!gpu.ok) throw Error(gpu.message);
    if(!webllm) webllm = await import(WEBLLM_IMPORT);
    selectionFromCatalog(state.model, webllm.prebuiltAppConfig?.model_list || []);
    const selected = state.model;
    updateProgress({text:'Downloading/loading selected model…'});
    if(state.engine && state.loadedModel !== selected) { await state.engine.unload?.();state.engine=null;state.loadedModel=''; }
    if(!state.engine) state.engine = await webllm.CreateMLCEngine(selected,{initProgressCallback:updateProgress});
    else await state.engine.reload(selected);
    state.loadedModel = selected;state.progress = 'Model ready. You can send a question.';status('On-device model ready.');
  } catch(e) {status(`Model setup failed: ${String(e?.message||e).slice(0,210)}`);state.progress='Choose another model or try the advanced online option.';}
  finally {state.loading=false;}
}
async function submitQuestion(summary) {
  if (state.busy || state.loading)return;
  let question;
  try {question=cleanQuestion(state.question);}catch(e){status(e.message);return;}
  const previous = state.messages.slice();
  const context = state.includeContext ? summary : null;
  state.busy=true;status('Preparing answer…');
  try {
    let answer;
    if(state.mode==='local') {
      if (!localReady()) throw Error('Download and start an on-device model first.');
      const reply = await state.engine.chat.completions.create({messages:promptMessages(question,previous,context),stream:false,max_tokens:650});
      answer = reply?.choices?.[0]?.message?.content;
    } else {
      if(state.provider!=='ollama' && !state.consent) throw Error('Review and accept the advanced browser-key warning first.');
      const model=cleanModelId(state.cloudModel || defaults[state.provider]);
      answer=await cloudChat({provider:state.provider,model,key:state.key,question,history:previous,context});
    }
    if(typeof answer!=='string'||!answer.trim())throw Error('No text response returned.');
    state.messages=[...previous,{role:'user',content:question},{role:'assistant',content:answer.slice(0,16000)}].slice(-16);
    state.question='';state.status=state.mode==='local'?'Answered on your device.':'Response received. Review any model claims.';
  }catch(e){state.status=`Question failed: ${String(e?.message||e).slice(0,210)}`;}
  finally {state.busy=false;}
}
export function mountAiAssistant(root,{summary={}}={}) {
  const $=id=>root.querySelector(`#${id}`);
  const redraw=()=>{if (!root.querySelector('#ai-chat')) return;root.innerHTML=aiAssistantView();mountAiAssistant(root,{summary});};
  const syncPreview=()=>{const e=$('ai-context-preview');if(e)e.textContent=state.includeContext?JSON.stringify(safeAiContext(summary),null,2):'No analysis summary will be sent.';};
  $('ai-mode-local')?.addEventListener('click',()=>{state.mode='local';state.status='';redraw();});
  $('ai-mode-cloud')?.addEventListener('click',()=>{state.mode='cloud';state.status='';redraw();});
  $('ai-local-model')?.addEventListener('change',e=>{state.model=e.target.value;redraw();});
  $('ai-check-gpu')?.addEventListener('click',async()=>{state.gpu=await checkGPU();state.gpuChecked=true;redraw();});
  $('ai-load')?.addEventListener('click',async()=>{await loadLocal();redraw();});
  $('ai-unload')?.addEventListener('click',async()=>{if(state.engine){try{await state.engine.unload?.();}catch{}state.engine=null;state.loadedModel='';state.status='Model unloaded from active memory. The browser may retain cached model files.';redraw();}});
  $('ai-provider')?.addEventListener('change',e=>{state.provider=e.target.value;state.cloudModel='';state.key='';state.consent=false;state.ollama=[];redraw();});
  $('ai-cloud-model')?.addEventListener('input',e=>{state.cloudModel=e.target.value;});
  $('ai-cloud-key')?.addEventListener('input',e=>{state.key=e.target.value;});
  $('ai-key-consent')?.addEventListener('change',e=>{state.consent=e.target.checked;});
  $('ai-forget-key')?.addEventListener('click',()=>{state.key='';state.consent=false;redraw();});
  $('ai-ollama-list')?.addEventListener('click',async()=>{status('Checking local Ollama…');try{state.ollama=await ollamaModels();if(state.ollama.length&&!state.cloudModel)state.cloudModel=state.ollama[0];state.status=state.ollama.length?'Available Ollama models found.':'Ollama returned no installed models.';}catch{state.status='Ollama is not reachable. It must be installed, running, and allow this web origin.';}redraw();});
  $('ai-clear-chat')?.addEventListener('click',()=>{state.messages=[];state.question='';redraw();});
  root.querySelectorAll('[data-ai-prompt]').forEach(el=>el.addEventListener('click',()=>{state.question=el.dataset.aiPrompt||'';const input=$('ai-question');if(input){input.value=state.question;input.focus();}}));
  $('ai-include-context')?.addEventListener('change',e=>{state.includeContext=e.target.checked;syncPreview();});syncPreview();
  $('ai-question')?.addEventListener('input',e=>{state.question=e.target.value;});
  $('ai-send')?.addEventListener('click',async()=>{await submitQuestion(summary);redraw();});
}
