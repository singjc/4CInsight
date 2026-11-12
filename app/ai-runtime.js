/** 4C Insight B4. Only a local Ollama endpoint is permitted. No online inference providers. */
export const OLLAMA_BASE = 'http://127.0.0.1:11434';
export const OLLAMA_INSTALL = 'https://ollama.com/download';
export const RECOMMENDED_MODELS = Object.freeze([
  {id:'qwen2.5:1.5b', label:'Lightweight · Qwen2.5 1.5B', note:'Smaller download and memory footprint.'},
  {id:'llama3.2:3b', label:'Balanced · Llama 3.2 3B', note:'Better language capacity on suitable hardware.'},
  {id:'qwen2.5:0.5b', label:'Very small · Qwen2.5 0.5B', note:'Useful on lower-memory computers; may give weaker answers.'},
]);
export const SYSTEM_NOTE = 'You are a scientific explanation assistant for 4C Insight. Distinguish exploratory association from independent validation. Do not invent patient measurements, p-values, diagnoses or clinical conclusions. When evidence is absent say so. You have no tools and must not imply you analyzed original data. Any context provided is an approved aggregate summary only.';
export function safeAiContext(input = {}) {
  return {
    context_type:'aggregate_4c_insight_summary_v1',
    integrative_result_available:Boolean(input.integrative),
    compare_groups_result_available:Boolean(input.compare),
    pathway_result_available:Boolean(input.pathways),
    pathway_sets_tested:Number.isSafeInteger(input.pathwayCount)&&input.pathwayCount>=0 ? Math.min(input.pathwayCount,1000000) : null,
    pathway_evidence_type:['exploratory','defined_comparison'].includes(input.pathwayEvidence)?input.pathwayEvidence:'unverified',
  };
}
export function cleanQuestion(raw) {
  const value=String(raw??'').trim();
  if(!value || value.length>4000) throw Error('Enter a question of 1–4,000 characters.');
  return value;
}
export function cleanModelId(raw) {
  const value=String(raw??'').trim();
  if(!/^[A-Za-z0-9][A-Za-z0-9._:/+\-]{0,100}$/.test(value)) throw Error('Invalid model name.');
  if(/(?:^|[:/])cloud(?:$|[:/])/i.test(value) || /(?:^|\/)ollama\.com\//i.test(value)) throw Error('Cloud models are blocked. Choose an installed local model.');
  return value;
}
export function isLocalModel(entry) {
  if(!entry || typeof entry.name!=='string') return false;
  try {cleanModelId(entry.name);}catch{return false;}
  // Reject explicitly remote model descriptors if Ollama reports them.
  if(entry.remote_host || entry.remote_model || entry.cloud === true) return false;
  return true;
}
export function localModelRecords(payload) {
  if(!Array.isArray(payload?.models)) return [];
  const found=new Set();
  return payload.models.filter(isLocalModel).filter(entry=>{if(found.has(entry.name))return false;found.add(entry.name);return true;}).slice(0,100)
    .map(entry=>({name:entry.name,size:Number.isFinite(entry.size)&&entry.size>=0?entry.size:null}));
}
export function promptMessages(question, history = [], context = null) {
  const limited=Array.isArray(history)?history.slice(-8).filter(item=>['user','assistant'].includes(item?.role)&&typeof item.content==='string').map(item=>({role:item.role,content:item.content.slice(0,4000)})):[];
  const note=context?`\n\nUser-approved aggregate summary (not raw data):\n${JSON.stringify(safeAiContext(context))}`:'';
  return [{role:'system',content:SYSTEM_NOTE},...limited,{role:'user',content:cleanQuestion(question)+note}];
}
const fetchOptions={mode:'cors',cache:'no-store',credentials:'omit',referrerPolicy:'no-referrer',targetAddressSpace:'loopback'};
export async function ollamaRequest(path,opts={},fetcher=globalThis.fetch) {
  if(!['/api/version','/api/tags','/api/chat','/api/pull'].includes(path)) throw Error('Unsupported local Ollama operation.');
  let response;
  try {response=await fetcher(OLLAMA_BASE+path,{...fetchOptions,...opts});}
  catch {throw Error('Cannot reach local Ollama. It may be stopped, or the browser may block this site.');}
  if(!response?.ok){
    let error='';
    try {const json=await response.json();error=typeof json?.error==='string'?json.error.slice(0,170):'';}catch{}
    throw Error(`Ollama returned HTTP ${response?.status??'unknown'}${error?`: ${error}`:''}`);
  }
  return response;
}
export async function probeOllama(fetcher=globalThis.fetch) {
  const response=await ollamaRequest('/api/version',{method:'GET'},fetcher);
  const json=await response.json();
  if(typeof json?.version!=='string' || json.version.length>70) throw Error('The local service did not return an Ollama version.');
  return json.version;
}
export async function installedOllamaModels(fetcher=globalThis.fetch) {
  const response=await ollamaRequest('/api/tags',{method:'GET'},fetcher);
  return localModelRecords(await response.json());
}
async function readStream(response,onPacket,limit=10000000) {
  if(!response.body?.getReader){
    const value=await response.json();onPacket(value);return;
  }
  const reader=response.body.getReader();const decoder=new TextDecoder();let buffer='';let received=0;
  try {
    for(;;){
      const {value,done}=await reader.read();if(done)break;
      received+=value.byteLength;
      if(received>limit) throw Error('Ollama response exceeded the allowed size.');
      buffer+=decoder.decode(value,{stream:true});
      const lines=buffer.split('\n');buffer=lines.pop();
      for(const line of lines){if(line.trim())onPacket(JSON.parse(line));}
      if(buffer.length>1048576)throw Error('Ollama sent an oversized response record.');
    }
    buffer+=decoder.decode();if(buffer.trim())onPacket(JSON.parse(buffer));
  }finally{reader.releaseLock();}
}
export async function pullRecommendedModel(model,{onProgress=()=>{},signal,fetcher=globalThis.fetch}={}) {
  if(!RECOMMENDED_MODELS.some(x=>x.id===model)) throw Error('Choose a recommended local model.');
  const response=await ollamaRequest('/api/pull',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({model,stream:true}),signal},fetcher);
  let success=false;
  await readStream(response,packet=>{
    if(typeof packet?.error==='string')throw Error(`Model download failed: ${packet.error.slice(0,170)}`);
    if(typeof packet?.status==='string'){
      const progress=Number.isFinite(packet.completed)&&Number.isFinite(packet.total)&&packet.total>0 ? Math.min(100,Math.round(packet.completed/packet.total*100)) : null;
      onProgress({status:packet.status.slice(0,100),progress});
      if(packet.status==='success')success=true;
    }
  });
  if(!success)throw Error('The model download ended without confirmation. Check installed models.');
  return model;
}
export async function ollamaChat({model,question,history=[],context=null,signal,onText=()=>{},fetcher=globalThis.fetch}={}) {
  const chosen=cleanModelId(model);
  const response=await ollamaRequest('/api/chat',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({model:chosen,messages:promptMessages(question,history,context),stream:true,think:false,keep_alive:'5m',options:{num_predict:650}}),signal},fetcher);
  let answer='';
  await readStream(response,packet=>{
    if(typeof packet?.error==='string')throw Error(`Local model failed: ${packet.error.slice(0,160)}`);
    const piece=packet?.message?.content;
    if(typeof piece==='string'){
      answer=(answer+piece).slice(0,16000);
      onText(answer);
    }
  });
  if(!answer.trim())throw Error('The local model did not return any text.');
  return answer;
}
