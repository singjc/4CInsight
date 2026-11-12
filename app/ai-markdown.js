/** Safe, dependency-free subset of Markdown for local LLM output.
 * All model text is untrusted. No raw HTML, images, script, style, or unsafe URLs.
 * Supports paragraphs, headings, emphasis, code, links, block quotes and lists.
 */
const escapeHtml=s=>String(s??'').replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;').replace(/'/g,'&#39;');
const safeLink=raw=>{
  try { const url=new URL(raw);return url.protocol==='https:'||url.protocol==='http:'?url.href:null; }
  catch { return null; }
};
function inline(raw){
  const parts=[];
  const stash=html=>{const k=parts.length;parts.push(html);return `\uE000${k}\uE001`;};
  // Remove private placeholders from untrusted text; tokens are generated only here.
  let text=String(raw??'').replace(/[\uE000\uE001]/g,'');
  text=text.replace(/`([^`\n]{1,600})`/g,(_,code)=>stash(`<code>${escapeHtml(code)}</code>`));
  text=text.replace(/\[([^\]\n]{1,200})\]\(([^\s()<>]{1,500})\)/g,(full,label,href)=>{
    const uri=safeLink(href);
    return uri?stash(`<a href="${escapeHtml(uri)}" target="_blank" rel="noopener noreferrer nofollow">${escapeHtml(label)}</a>`):full;
  });
  text=escapeHtml(text)
    .replace(/\*\*([^*\n]{1,800})\*\*/g,'<strong>$1</strong>')
    .replace(/__([^_\n]{1,800})__/g,'<strong>$1</strong>')
    .replace(/(^|[^*])\*([^*\n]{1,800})\*(?!\*)/g,'$1<em>$2</em>')
    .replace(/(^|[^_])_([^_\n]{1,800})_(?!_)/g,'$1<em>$2</em>')
    .replace(/~~([^~\n]{1,800})~~/g,'<del>$1</del>');
  return text.replace(/\uE000(\d+)\uE001/g,(_,n)=>parts[Number(n)]??'');
}
export function renderAiMarkdown(raw){
  const lines=String(raw??'').slice(0,16000).replace(/\r\n?/g,'\n').split('\n').slice(0,500);
  const blocks=[];let paragraph=[],listType=null,items=[],inFence=false,code=[];
  const flushParagraph=()=>{if(paragraph.length){blocks.push(`<p>${paragraph.map(inline).join(' ')}</p>`);paragraph=[];}};
  const flushList=()=>{if(listType){blocks.push(`<${listType}>${items.map(item=>`<li>${inline(item)}</li>`).join('')}</${listType}>`);listType=null;items=[];}};
  for(let line of lines){
    const f=line.match(/^\s*```\s*([A-Za-z0-9_-]*)\s*$/);
    if(f){flushParagraph();flushList();if(inFence){blocks.push(`<pre><code>${escapeHtml(code.join('\n'))}</code></pre>`);code=[];inFence=false;}else{inFence=true;code=[];}continue;}
    if(inFence){code.push(line);continue;}
    if(!line.trim()){flushParagraph();flushList();continue;}
    const heading=line.match(/^\s{0,3}(#{1,4})\s+(.+)$/);
    const quote=line.match(/^\s{0,3}>\s?(.+)$/);
    const ordered=line.match(/^\s{0,3}\d{1,4}[.)]\s+(.+)$/);
    const bullet=line.match(/^\s{0,3}[-*+]\s+(.+)$/);
    if(heading){flushParagraph();flushList();const level=Math.min(5,heading[1].length+2);blocks.push(`<h${level}>${inline(heading[2])}</h${level}>`);continue;}
    if(/^\s{0,3}(---+|\*\*\*+)\s*$/.test(line)){flushParagraph();flushList();blocks.push('<hr>');continue;}
    if(quote){flushParagraph();flushList();blocks.push(`<blockquote>${inline(quote[1])}</blockquote>`);continue;}
    if(ordered||bullet){flushParagraph();const type=ordered?'ol':'ul';if(listType!==type){flushList();listType=type;}items.push((ordered||bullet)[1]);continue;}
    flushList();paragraph.push(line.trim());
  }
  flushParagraph();flushList();
  if(inFence)blocks.push(`<pre><code>${escapeHtml(code.join('\n'))}</code></pre>`);
  return blocks.join('\n');
}
