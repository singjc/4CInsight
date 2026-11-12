/** 4C Insight B4: strictly local Ollama, no cloud/browser model engine or companion. */
import {OLLAMA_INSTALL,RECOMMENDED_MODELS,cleanQuestion,probeOllama,installedOllamaModels,pullRecommendedModel,ollamaChat} from './ai-runtime.js?v=0.22.8';
import {AI_EVIDENCE_SCOPES,availableAiEvidence,createAiEvidence,previewAiEvidence} from './ai-evidence.js?v=0.22.8';
import {renderAiMarkdown} from './ai-markdown.js?v=0.22.8';
const esc=v=>String(v??'').replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;').replace(/'/g,'&#39;');
const state={connected:false,checking:false,version:'',models:[],selected:'',busy:false,pulling:false,status:'',error:'',progress:'',messages:[],question:'',includeContext:false,evidenceScope:'pathways',started:false,controller:null};
let currentRoot=null,currentReports={};
const fmtBytes=v=>Number.isFinite(v)&&v>0?` · ${(v/1024**3).toFixed(2)} GB on disk`:'';
function stateStatus(){if(state.checking)return 'Checking Ollama on this computer…';if(state.connected)return `Local Ollama connected${state.version?' · version '+state.version:''}`;return state.error||'Local Ollama not connected.';}
function chatMarkup(){return state.messages.length?state.messages.map(m=>`<div class="ai-message ${m.role==='user'?'ai-user':'ai-assistant'}"><strong>${m.role==='user'?'You':'Local assistant'}</strong>${m.role==='user'?`<p>${esc(m.content)}</p>`:`<div class="ai-markdown">${renderAiMarkdown(m.content)}</div>`}</div>`).join(''):'<p class="ai-note">Ask a question about your research. No source files are attached automatically.</p>';}
function setupMarkup(){
  const installed=state.models;
  const tips=!state.connected?`<details class="ai-help"><summary>Ollama installed but not connecting?</summary><p>First, open the Ollama application. On Linux, the normal installer usually starts a service automatically. If it is stopped, use the commands below or ask your system administrator.</p><p>A hosted website may need permission to contact your computer. Ollama must allow only the exact 4C Insight website origin, and your browser must permit access to the local device.</p><pre>systemctl is-active ollama\nsudo systemctl start ollama\nsudo systemctl enable ollama</pre><p>For Linux system services, a system administrator can set <code>OLLAMA_ORIGINS=https://singjc.github.io</code> and <code>OLLAMA_NO_CLOUD=1</code> in an Ollama systemd override, then restart the service. Do not use <code>OLLAMA_HOST=0.0.0.0</code>. This page cannot change these settings itself.</p><p>For Windows and macOS, start Ollama from the normal application menu. Browser origin access may still need configuration.</p><p><a href="https://docs.ollama.com/faq" target="_blank" rel="noopener noreferrer">Official Ollama configuration guide</a></p></details>`:'';
  return `<section class="card ai-card"><div class="card-heading"><div><h3>Local model connection</h3><p>Ollama runs the model on this computer. No separate AI companion, browser extension, or cloud account is required.</p></div></div>
  <div class="ai-connection ${state.connected?'ai-online':'ai-offline'}" role="status" aria-live="polite"><span class="ai-dot"></span><div><strong>${esc(stateStatus())}</strong>${state.connected?'<small>Only locally installed models are available.</small>':'<small>The webpage can check a running Ollama service, but cannot check installed programs or start them.</small>'}</div></div>
  <div class="ai-actions"><button id="ai-reconnect" type="button" class="secondary-button" ${state.checking||state.busy?'disabled':''}>${state.checking?'Checking…':'Check connection again'}</button>${!state.connected?`<a class="secondary-button ai-as-link" href="${OLLAMA_INSTALL}" target="_blank" rel="noopener noreferrer">Get Ollama</a>`:''}</div>
  ${tips}
  ${state.connected?`<div class="ai-model-panel"><label for="ai-model">Model on this computer</label>${installed.length?`<select id="ai-model">${installed.map(m=>`<option value="${esc(m.name)}" ${m.name===state.selected?'selected':''}>${esc(m.name)}${esc(fmtBytes(m.size))}</option>`).join('')}</select>`:'<p class="ai-note">Ollama is running, but no local models were found. Choose a recommended model below.</p>'}
  <div class="ai-actions"><button id="ai-refresh-models" type="button" class="secondary-button" ${state.pulling?'disabled':''}>Refresh models</button></div>
  <details class="ai-help" ${!installed.length?'open':''}><summary>Download another local model</summary><p>Select a model and approve the download. Model weights come from Ollama's registry. The assistant does not send your cohort data to the registry.</p><label for="ai-recommended">Recommended local model</label><select id="ai-recommended">${RECOMMENDED_MODELS.map(m=>`<option value="${esc(m.id)}">${esc(m.label)}</option>`).join('')}</select><button id="ai-pull" type="button" class="secondary-button" ${state.pulling?'disabled':''}>${state.pulling?'Downloading…':'Download model'}</button><p class="ai-note" id="ai-pull-progress" role="status" aria-live="polite">${esc(state.progress)}</p></details></div>`:''}
  <p class="ai-note">Privacy: 4C Insight sends chat requests only to <code>127.0.0.1:11434</code>. For strict local-only use, disable Ollama's cloud features and use locally downloaded models. Model downloads require internet access.</p></section>`;
}
export function aiAssistantView(){
 const available=availableAiEvidence(currentReports);
 const selected=state.evidenceScope==='all'?'all':available.includes(state.evidenceScope)?state.evidenceScope:available[0]||'pathways';
 const contextActive=Boolean(state.includeContext&&available.length);
 const preview=contextActive?previewAiEvidence(currentReports,selected):'No scientific evidence will be attached to this question.';
 return `<section class="page-heading"><div><span class="eyebrow">AI Assistant</span><h2>Ask questions about your analysis</h2><p>Use an Ollama model on your computer to explain the actual results saved in 4C Insight. No external AI account is needed.</p></div><div class="local-card"><strong>Local Ollama AI</strong><span>Local models · Read-only scientific context by approval</span></div></section>
${setupMarkup()}
<section class="card ai-card"><div class="card-heading"><div><h3>Ask your question</h3><p>Chat is held only in this open page. The assistant can explain approved results but cannot access original files, run analyses or change scientific results.</p></div><button id="ai-clear" type="button" class="secondary-button">Clear chat</button></div>
<div class="ai-example-prompts">${['Explain my pathway results','What does a PCoA cluster mean?','What should I check next?'].map(p=>`<button type="button" data-ai-prompt="${esc(p)}" class="secondary-button">${esc(p)}</button>`).join('')}</div>
<div class="ai-chat" id="ai-chat" role="log" aria-live="polite">${chatMarkup()}</div>
<div class="ai-evidence-settings"><div class="ai-evidence-title"><strong>Use your calculated results</strong><span>${available.length?'Available: '+available.map(id=>AI_EVIDENCE_SCOPES.find(s=>s.id===id)?.label||id).join(', '):'No saved scientific results are available in this session'}</span></div>
<label class="ai-context-choice"><input type="checkbox" id="ai-include-context" ${contextActive?'checked':''} ${!available.length?'disabled':''}/> Include the selected, read-only scientific evidence with my next question</label>
<label for="ai-evidence-scope">Evidence to use</label><select id="ai-evidence-scope" ${!available.length?'disabled':''}>${AI_EVIDENCE_SCOPES.filter(opt=>opt.id==='all'?available.length>1:available.includes(opt.id)).map(opt=>`<option value="${esc(opt.id)}" ${opt.id===selected?'selected':''}>${esc(opt.label)}</option>`).join('')}</select>
<p class="ai-note">Only bounded statistics, pathway names, and selected gene identifiers are included. Patient and observation IDs, original source filenames, and raw data are excluded. Your choice is shown below before you send.</p>
<details class="ai-evidence-preview" ${contextActive?'open':''}><summary>Review the exact evidence included with this question</summary><pre id="ai-context-preview" class="ai-context-preview">${esc(preview)}</pre></details>
</div>
<label for="ai-question">Your question</label><textarea id="ai-question" maxlength="4000" rows="3" placeholder="Ask about a pathway, gene, q-value, cluster or result…">${esc(state.question)}</textarea>
<div class="ai-actions"><button id="ai-send" type="button" class="primary-button" ${state.busy||state.pulling||!state.connected||!state.selected?'disabled':''}>${state.busy?'Answering…':'Send question'}</button><button id="ai-stop" type="button" class="secondary-button" ${!state.busy&&!state.pulling?'disabled':''}>Stop</button><span id="ai-status" class="ai-note" role="status" aria-live="polite">${esc(state.status || (!state.connected?'Connect Ollama to begin.':'Ready for a local question.'))}</span></div></section>`;}

function repaint(){const root=currentRoot;if(root?.isConnected){root.innerHTML=aiAssistantView();bind(root,currentReports);}}
function setStatus(text){state.status=text;const element=currentRoot?.querySelector('#ai-status');if(element)element.textContent=text;}
async function connect(){if(state.checking)return;state.checking=true;state.error='';repaint();try{
    const version=await probeOllama();const models=await installedOllamaModels();state.connected=true;state.version=version;state.models=models;
    if(!models.some(m=>m.name===state.selected))state.selected=models[0]?.name||'';
    state.status=models.length?`${models.length} local model(s) available.`:'Ollama is connected. Download a local model to begin.';
  }catch(e){state.connected=false;state.models=[];state.selected='';state.error=String(e?.message||e).slice(0,220);state.status='Connection unavailable. See setup help above.';}
  finally{state.checking=false;repaint();}}
async function pull(){const sel=currentRoot?.querySelector('#ai-recommended');const model=sel?.value;if(!model||state.pulling)return;
 state.pulling=true;state.progress='Starting download…';state.controller=new AbortController();repaint();
 try{await pullRecommendedModel(model,{signal:state.controller.signal,onProgress:packet=>{state.progress=packet.status+(packet.progress===null?'':` · ${packet.progress}%`);const p=currentRoot?.querySelector('#ai-pull-progress');if(p)p.textContent=state.progress;}});
 state.progress='Model download complete.';const models=await installedOllamaModels();state.models=models;state.selected=models.some(m=>m.name===model)?model:models[0]?.name||'';setStatus('New local model is ready.');
 }catch(e){setStatus(e.name==='AbortError'?'Model download stopped.':`Download failed: ${String(e?.message||e).slice(0,180)}`);}
 finally{state.pulling=false;state.controller=null;repaint();}}
async function send(){if(state.busy||!state.connected||!state.selected)return;
 let question;try{question=cleanQuestion(state.question);}catch(e){setStatus(e.message);return;}
 const history=state.messages.slice();const chosen=state.selected;
 if(!state.models.some(m=>m.name===chosen)){setStatus('Choose a locally installed model.');return;}
 state.busy=true;state.controller=new AbortController();state.messages=[...history,{role:'user',content:question},{role:'assistant',content:''}];state.question='';repaint();setStatus('Local model is preparing an answer…');
 try{
 const evidence=state.includeContext&&availableAiEvidence(currentReports).length?createAiEvidence(currentReports,state.evidenceScope):null;
 if(evidence && JSON.stringify(evidence).length>12500)throw Error('Evidence is too large. Choose a narrower section.');
 const answer=await ollamaChat({model:chosen,question,history,context:evidence,signal:state.controller.signal,onText:text=>{
 state.messages[state.messages.length-1].content=text;const chat=currentRoot?.querySelector('#ai-chat');if(chat){const last=chat.querySelector('.ai-message:last-child .ai-markdown');if(last){last.innerHTML=renderAiMarkdown(text);chat.scrollTop=chat.scrollHeight;}}
 }});
 state.messages[state.messages.length-1].content=answer;state.status='Answered using a locally installed Ollama model.';
 }catch(e){state.messages=history;state.question=question;state.status=e.name==='AbortError'?'Answer stopped.':`Local chat failed: ${String(e?.message||e).slice(0,180)}`;}
 finally{state.busy=false;state.controller=null;state.messages=state.messages.slice(-16);repaint();}}
function bind(root,reports){
 const $=id=>root.querySelector('#'+id);
 const syncPreview=()=>{
   const out=$('ai-context-preview');
   if(!out)return;
   const scopes=availableAiEvidence(reports);
   out.textContent=state.includeContext&&scopes.length?previewAiEvidence(reports,state.evidenceScope):'No scientific evidence will be attached to this question.';
 };syncPreview();
 $('ai-reconnect')?.addEventListener('click',()=>{void connect();});
 $('ai-refresh-models')?.addEventListener('click',()=>{void connect();});
 $('ai-model')?.addEventListener('change',e=>{state.selected=e.target.value;repaint();});
 $('ai-pull')?.addEventListener('click',()=>{void pull();});
 $('ai-clear')?.addEventListener('click',()=>{state.messages=[];state.question='';repaint();});
 root.querySelectorAll('[data-ai-prompt]').forEach(button=>button.addEventListener('click',()=>{
   state.question=button.dataset.aiPrompt||'';
   const available=availableAiEvidence(reports);
   const scope=/pathway/i.test(state.question)?'pathways':/PCoA|cluster/i.test(state.question)?'joint':available.length>1?'all':available[0];
   if(available.includes(scope)||(scope==='all'&&available.length)){state.evidenceScope=scope;state.includeContext=true;repaint();}
   const input=$('ai-question');if(input){input.value=state.question;input.focus();}
 }));
 $('ai-include-context')?.addEventListener('change',e=>{state.includeContext=e.target.checked;repaint();});
 $('ai-evidence-scope')?.addEventListener('change',e=>{state.evidenceScope=e.target.value;repaint();});
 $('ai-question')?.addEventListener('input',e=>{state.question=e.target.value;});
 $('ai-send')?.addEventListener('click',()=>{void send();});
 $('ai-stop')?.addEventListener('click',()=>{state.controller?.abort();});
}
export function mountAiAssistant(root,{evidence={},summary={}}={}){
 currentRoot=root;currentReports=evidence;
 const available=availableAiEvidence(evidence);
 if(available.length && state.evidenceScope!=='all' && !available.includes(state.evidenceScope))state.evidenceScope=available[0];
 if(root?.isConnected)root.innerHTML=aiAssistantView();
 bind(root,evidence);
 if(!state.started){state.started=true;void connect();}
 else if(state.connected){void connect();}
}
