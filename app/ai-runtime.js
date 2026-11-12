/** A3: browser LLM orchestration. No native processes, persistent credentials, or raw source access. */
export const WEBLLM_IMPORT = 'https://esm.run/@mlc-ai/web-llm@0.2.88';
export const LOCAL_MODELS = Object.freeze([
  { id: 'Qwen2.5-0.5B-Instruct-q4f16_1-MLC', label: 'Lightweight · Qwen2.5 0.5B', note: 'Small model. Answers may be less accurate.', memoryMB: 945 },
  { id: 'Llama-3.2-1B-Instruct-q4f16_1-MLC', label: 'Balanced · Llama 3.2 1B', note: 'Good starting option on compatible GPUs.', memoryMB: 880 },
  { id: 'Qwen2.5-1.5B-Instruct-q4f16_1-MLC', label: 'Larger · Qwen2.5 1.5B', note: 'Requires more graphics memory.', memoryMB: 1630 },
  { id: 'Qwen2.5-3B-Instruct-q4f16_1-MLC', label: 'Higher capacity · Qwen2.5 3B', note: 'Requires a capable GPU.', memoryMB: 2505 },
]);
export const CLOUD_MODES = Object.freeze(['openai', 'claude', 'ollama']);
export const MODEL_DOCS = Object.freeze({
  openai: 'https://platform.openai.com/api-keys',
  claude: 'https://platform.claude.com/settings/keys',
  ollama: 'https://ollama.com/download',
});
export const SYSTEM_NOTE = 'You are a scientific explanation assistant for 4C Insight. Distinguish exploratory association from independent validation. Do not invent cohort measurements or clinical conclusions. The information below, if provided, is a user-approved limited aggregate summary, not the full data. When evidence is absent say so.';
export function safeAiContext(input = {}) {
  return {
    context_type: 'aggregate_4c_insight_summary_v1',
    integrative_result_available: Boolean(input.integrative),
    compare_groups_result_available: Boolean(input.compare),
    pathway_result_available: Boolean(input.pathways),
    pathway_sets_tested: Number.isSafeInteger(input.pathwayCount) && input.pathwayCount >= 0 ? Math.min(input.pathwayCount, 1000000) : null,
    pathway_evidence_type: ['exploratory', 'defined_comparison'].includes(input.pathwayEvidence) ? input.pathwayEvidence : 'unverified',
  };
}
export function cleanQuestion(raw) {
  const text = String(raw ?? '').trim();
  if (!text || text.length > 4000) throw Error('Enter a question of 1–4,000 characters.');
  return text;
}
export function cleanModelId(raw) {
  const text = String(raw ?? '').trim();
  if (!/^[A-Za-z0-9][A-Za-z0-9._:/+\-]{0,100}$/.test(text)) throw Error('Enter a valid model ID.');
  return text;
}
export function selectionFromCatalog(modelId, catalog = []) {
  const chosen = LOCAL_MODELS.find(m => m.id === modelId);
  if (!chosen) throw Error('Choose one of the listed local models.');
  const record = catalog.find(m => m.model_id === modelId);
  if (!record) throw Error('This model is not present in the current WebLLM catalog. Choose another model.');
  return { ...chosen, vramRequiredMB: Number.isFinite(record.vram_required_MB) ? record.vram_required_MB : chosen.memoryMB };
}
export function promptMessages(question, history = [], context = null) {
  const cleanHistory = Array.isArray(history) ? history.slice(-8).filter(x => (x.role === 'user' || x.role === 'assistant') && typeof x.content === 'string')
    .map(x => ({ role: x.role, content: x.content.slice(0, 4000) })) : [];
  const text = cleanQuestion(question);
  // Always mark an approved aggregate separately from the user question.
  const suffix = context ? `\n\nApproved aggregate context (not raw cohort data):\n${JSON.stringify(safeAiContext(context))}` : '';
  return [{ role: 'system', content: SYSTEM_NOTE }, ...cleanHistory, { role: 'user', content: text + suffix }];
}
function requestError(status, message = '') {
  const msg = String(message).slice(0, 180).replace(/sk-[A-Za-z0-9_-]+/g, '[redacted]');
  return Error(`Provider request failed (HTTP ${status}). ${msg}`.trim());
}
async function requestJSON(url, options, fetcher) {
  let response;
  try { response = await fetcher(url, { ...options, cache: 'no-store', credentials: 'omit', referrerPolicy: 'no-referrer' }); }
  catch { throw Error('Cannot reach the provider. Check network access and browser origin permissions.'); }
  let json;
  try { json = await response.json(); }
  catch { throw requestError(response.status, 'The provider returned an invalid response.'); }
  if (!response.ok) throw requestError(response.status, json?.error?.type || json?.error?.code || json?.error || 'Check your API access and model.');
  return json;
}
/** Advanced-only direct API calls. The caller must obtain explicit risk consent; never persist API keys. */
export async function cloudChat({ provider, question, model, key = '', history = [], context = null, signal }, fetcher = globalThis.fetch) {
  if (!CLOUD_MODES.includes(provider)) throw Error('Select a supported provider.');
  const selected = cleanModelId(model);
  const messages = promptMessages(question, history, context);
  const bodyCommon = { model: selected, messages, stream: false };
  let url, headers, body;
  if (provider === 'openai') {
    if (!String(key).trim()) throw Error('Enter an OpenAI API key for this tab.');
    url = 'https://api.openai.com/v1/chat/completions';
    headers = { 'Authorization': `Bearer ${key}`, 'Content-Type': 'application/json' };
    body = {...bodyCommon, max_completion_tokens: 1024};
  } else if (provider === 'claude') {
    if (!String(key).trim()) throw Error('Enter a Claude API key for this tab.');
    url = 'https://api.anthropic.com/v1/messages';
    headers = { 'x-api-key': key, 'anthropic-version': '2023-06-01', 'anthropic-dangerous-direct-browser-access': 'true', 'Content-Type': 'application/json' };
    body = { model: selected, system: messages[0].content, messages: messages.slice(1), max_tokens: 1024 };
  } else {
    url = 'http://127.0.0.1:11434/api/chat';
    headers = { 'Content-Type': 'application/json' };
    body = {...bodyCommon, options:{num_predict:640}};
  }
  const result = await requestJSON(url, { method: 'POST', headers, body: JSON.stringify(body), signal }, fetcher);
  const answer = provider === 'claude' ? result?.content?.filter(x => x.type === 'text').map(x => x.text).join('\n') : result?.message?.content || result?.choices?.[0]?.message?.content;
  if (typeof answer !== 'string' || !answer.trim()) throw Error('The provider returned no text response.');
  return answer.slice(0, 16000);
}
export async function ollamaModels(fetcher = globalThis.fetch) {
  const data = await requestJSON('http://127.0.0.1:11434/api/tags', { method: 'GET' }, fetcher);
  return Array.isArray(data.models) ? data.models.filter(x => typeof x?.name === 'string').map(x => x.name).slice(0, 100) : [];
}
export function browserSupport(navigatorLike = globalThis.navigator) {
  return { secure: typeof globalThis.isSecureContext === 'boolean' ? globalThis.isSecureContext : true,
    webgpu: Boolean(navigatorLike?.gpu && typeof navigatorLike.gpu.requestAdapter === 'function'),
    storage: Boolean(navigatorLike?.storage && typeof navigatorLike.storage.estimate === 'function') };
}
