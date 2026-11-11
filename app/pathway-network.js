/** P5: browser-local, descriptive pathway and gene membership networks.
 * Graphs are derived ONLY from existing pathway results, scored input and exact GMT memberships.
 * An edge reports gene overlap or membership; it is not a physical interaction or a new test.
 * The statistical engine, original gene rankings and pathway q-values are never modified.
 */
const str=v=>String(v??'')
const esc=v=>str(v).replaceAll('&','&amp;').replaceAll('<','&lt;').replaceAll('>','&gt;').replaceAll('"','&quot;').replaceAll("'",'&#39;')
const cleanGene=v=>str(v).trim().toUpperCase()
const valid=v=>typeof v==='number'&&Number.isFinite(v)
const clamp=(v,lo,hi)=>Math.max(lo,Math.min(hi,v))
const key=r=>`${str(r?.pathway)}\u0000${str(r?.description)}`
const hash=s=>{let h=2166136261;for(const c of str(s)){h^=c.charCodeAt(0);h=Math.imul(h,16777619)}return h>>>0}
const num=(value,fallback,min,max)=>{const n=Number(value);return Number.isFinite(n)?clamp(n,min,max):fallback}
const integer=(value,fallback,min,max)=>Math.floor(num(value,fallback,min,max))
const fix=n=>valid(n)?String(Math.round(n*10000)/10000):''
const tsv=v=>{const x=str(v).replace(/[\t\n\r]/g,' ');return /^\s*[=+@]/.test(x)||/^\s*-(?!\d)/.test(x)?"'"+x:x}

export const PATHWAY_NETWORK_SCHEMA='4c-insight-pathway-network-v1'
export const DEFAULT_PATHWAY_NETWORK_CONFIG=Object.freeze({view:'overlap',maxPathways:40,qCutoff:0.05,genes:'leading',metric:'jaccard',minCommon:2,minSimilarity:0.10,maxGenes:120,hideSingleGenes:true,seed:20261111})
export function defaultPathwayNetworkConfig(){return {...DEFAULT_PATHWAY_NETWORK_CONFIG}}
export function normalizePathwayNetworkConfig(raw={}){
  const c={...DEFAULT_PATHWAY_NETWORK_CONFIG,...raw}
  return {
    view:['overlap','bipartite'].includes(c.view)?c.view:'overlap',
    maxPathways:integer(c.maxPathways,40,2,250),
    qCutoff:num(c.qCutoff,0.05,0,1),
    genes:['leading','matched'].includes(c.genes)?c.genes:'leading',
    metric:['jaccard','overlap'].includes(c.metric)?c.metric:'jaccard',
    minCommon:integer(c.minCommon,2,1,500),
    minSimilarity:num(c.minSimilarity,0.10,0,1),
    maxGenes:integer(c.maxGenes,120,1,500),
    hideSingleGenes:!!c.hideSingleGenes,
    seed:integer(c.seed,20261111,1,4294967295),
  }
}
function availablePathways(report,sets,c,input){
  const testedGenes=new Set((input?.rows||[]).map(r=>cleanGene(r.gene)).filter(Boolean))
  if(!testedGenes.size)throw new Error('The original scored gene background is unavailable. Restore matching gene results before creating a network.')
  const byKey=new Map()
  for(const entry of sets.pathways||[]){
    const k=`${str(entry.name)}\u0000${str(entry.description)}`
    const a=byKey.get(k)||[];a.push(entry);byKey.set(k,a)
  }
  const rows=(report.results||[]).filter(r=>valid(r.q_value)&&r.q_value<=c.qCutoff)
    .sort((a,b)=>a.q_value-b.q_value||str(a.pathway).localeCompare(str(b.pathway))||str(a.description).localeCompare(str(b.description)))
    .slice(0,c.maxPathways)
  const used=[];const rejected=[]
  for(const [i,r] of rows.entries()){
    const original=byKey.get(key(r))||[]
    if(original.length!==1){rejected.push({pathway:str(r.pathway),reason:original.length?'Ambiguous GMT membership':'No exact GMT membership'});continue}
    const genes=[...new Set((original[0].genes||[]).map(cleanGene).filter(Boolean))]
    const overlap=new Set((r.leadingGenes||[]).map(cleanGene))
    const matched=genes.filter(g=>testedGenes.has(g))
    if(valid(r.matchedSetSize)&&matched.length!==r.matchedSetSize){rejected.push({pathway:str(r.pathway),reason:'Matched gene count differs from saved enrichment result'});continue}
    const selected=c.genes==='leading'?matched.filter(g=>overlap.has(g)):matched
    used.push({id:`p${i}`,type:'pathway',name:str(r.pathway),description:str(r.description),
      pathwayKey:key(r),geneSet:genes,geneSetSize:genes.length,genes:selected,
      p_value:r.p_value,q_value:r.q_value,normalizedScore:r.normalizedScore,foldEnrichment:r.foldEnrichment,
      enrichmentScore:r.enrichmentScore,leadingCount:overlap.size,reportIndex:(report.results||[]).indexOf(r)})
  }
  return {nodes:used,rejected,eligible:rows.length}
}
function membershipPathwayNetwork(report,sets,config,input){
  const c=normalizePathwayNetworkConfig(config)
  if(!Array.isArray(report?.results)||!Array.isArray(sets?.pathways))throw new Error('Saved pathway results and exact GMT gene sets are required')
  const {nodes,rejected,eligible}=availablePathways(report,sets,c,input)
  const edges=[]
  for(let i=0;i<nodes.length;i++)for(let j=i+1;j<nodes.length;j++){
    const a=nodes[i],b=nodes[j],A=new Set(a.genes),B=new Set(b.genes)
    const common=a.genes.filter(g=>B.has(g)).sort()
    if(common.length<c.minCommon||!A.size||!B.size)continue
    const score=c.metric==='jaccard'?common.length/(A.size+B.size-common.length):common.length/Math.min(A.size,B.size)
    if(score+1e-12<c.minSimilarity)continue
    edges.push({source:a.id,target:b.id,type:'pathway_overlap',commonGenes:common,
      overlapGenes:common.length,similarity:score,metric:c.metric})
  }
  return {schema:PATHWAY_NETWORK_SCHEMA,view:'overlap',config:c,nodes,edges,
    excludedExactSets:rejected,eligiblePathways:eligible,shownPathways:nodes.length,
    descriptor:'Edges mean shared gene identifiers in the selected pathway sets, not protein or gene interactions.'}
}
function membershipGeneNetwork(report,sets,config,input){
  const c=normalizePathwayNetworkConfig(config)
  const base=membershipPathwayNetwork(report,sets,c,input)
  const rankMap=new Map((input?.rows||[]).map((r,i)=>[cleanGene(r.gene),{rank:i+1,score:r.score,q:r.q}]))
  const members=new Map()
  for(const pathway of base.nodes){
    for(const name of pathway.genes){
      const list=members.get(name)||[];list.push(pathway);members.set(name,list)
    }
  }
  const genes=[...members].filter(([,linked])=>!c.hideSingleGenes||linked.length>1)
    .sort((a,b)=>b[1].length-a[1].length || (rankMap.get(a[0])?.rank??Infinity)-(rankMap.get(b[0])?.rank??Infinity) || a[0].localeCompare(b[0]))
    .slice(0,c.maxGenes)
  const geneNodes=genes.map(([name,links],i)=>({id:`g${i}`,type:'gene',name,description:'',
    degree:links.length,rank:rankMap.get(name)?.rank??null,score:rankMap.get(name)?.score??null,
    q:rankMap.get(name)?.q??null}))
  const selected=new Map(geneNodes.map(n=>[n.name,n]))
  const edges=[]
  for(const pathway of base.nodes)for(const name of pathway.genes){
    const n=selected.get(name);if(n)edges.push({source:pathway.id,target:n.id,type:'gene_membership',gene:name})
  }
  return {...base,view:'bipartite',nodes:[...base.nodes,...geneNodes],edges,
    shownGenes:geneNodes.length,totalEligibleGenes:members.size,
    descriptor:'Edges mean membership of an exact GMT gene in a measured pathway; no gene interactions are inferred.'}
}
export function buildPathwayNetwork(report,geneSets,config={},input){
  const c=normalizePathwayNetworkConfig(config)
  return c.view==='bipartite'?membershipGeneNetwork(report,geneSets,c,input):membershipPathwayNetwork(report,geneSets,c,input)
}

/** Deterministic bounded force layout. Coordinates are display-only and do not encode scientific distances. */
export function layoutPathwayNetwork(graph,{width=1000,height=570}={}){
  const nodes=graph.nodes||[],n=nodes.length,edgeList=graph.edges||[]
  const placed=nodes.map((node,i)=>{
    const h=hash(`${graph.config.seed}|${node.type}|${node.name}|${node.id}`)
    const angle=(i/Math.max(1,n)*Math.PI*2)+((h%1000)/1000-0.5)*0.65
    const radial= Math.min(width,height)*(.19+((h>>>13)%1000)/1000*.16)
    return {...node,x:width/2+Math.cos(angle)*radial,y:height/2+Math.sin(angle)*radial,vx:0,vy:0}
  })
  const index=new Map(placed.map((x,i)=>[x.id,i]))
  const pairs=edgeList.map(e=>({...e,a:index.get(e.source),b:index.get(e.target)})).filter(e=>e.a!==undefined&&e.b!==undefined)
  const iterations=n>190?60: n>90?85:120
  const repulsion=graph.view==='bipartite'?1400:2200
  for(let t=0;t<iterations;t++){
    const forces=placed.map(()=>[0,0])
    for(let i=0;i<n;i++)for(let j=i+1;j<n;j++){
      const a=placed[i],b=placed[j];let dx=a.x-b.x,dy=a.y-b.y
      if(Math.abs(dx)+Math.abs(dy)<0.001){dx=.1*(1+i);dy=.2*(1+j)}
      const dist2=Math.max(100,dx*dx+dy*dy),dist=Math.sqrt(dist2)
      const force=repulsion/dist2
      const fx=force*dx/dist,fy=force*dy/dist
      forces[i][0]+=fx;forces[i][1]+=fy;forces[j][0]-=fx;forces[j][1]-=fy
    }
    for(const edge of pairs){const a=placed[edge.a],b=placed[edge.b],dx=b.x-a.x,dy=b.y-a.y,d=Math.max(1,Math.hypot(dx,dy))
      const desired=graph.view==='overlap'?90:72
      const strength=graph.view==='overlap'?Math.min(.095,.03+(edge.similarity||0)*.1):.04
      const force=(d-desired)*strength
      const fx=force*dx/d,fy=force*dy/d
      forces[edge.a][0]+=fx;forces[edge.a][1]+=fy;forces[edge.b][0]-=fx;forces[edge.b][1]-=fy
    }
    for(let i=0;i<n;i++){
      const a=placed[i];forces[i][0]+=(width/2-a.x)*.0018;forces[i][1]+=(height/2-a.y)*.0018
      a.vx=clamp((a.vx+forces[i][0])*.78,-12,12)
      a.vy=clamp((a.vy+forces[i][1])*.78,-12,12)
      a.x=clamp(a.x+a.vx,35,width-35);a.y=clamp(a.y+a.vy,35,height-35)
    }
  }
  return {...graph,width,height,nodes:placed,edges:edgeList}
}

function finiteScore(v){return valid(v)?v:null}
function nodeColor(node,graph){
  if(node.type==='gene')return '#7787a6'
  if(graph.view==='overlap'&&graph.reportMode==='ora')return '#4262bf'
  const s=finiteScore(node.normalizedScore)
  return s===null?'#4262bf':s<0?'#d97735':'#315dce'
}
function radius(n,graph){
  if(n.type==='gene')return 4.6+Math.min(3,(n.degree||1)*.7)
  const q=Math.max(1e-12,Number(n.q_value)||1e-12)
  return 7.5+Math.min(12,Math.sqrt(-Math.log10(q))*3)
}
/** Fixed-size markers and text. Zoom projects world coordinates; it does not scale the SVG. */
export function pathwayNetworkSvg(graph,{view={cx:500,cy:285,k:1},selectedId='',selectedGene=''}={}){
  const w=graph.width||1000,h=graph.height||570,k=num(view.k,1,1,24),cx=num(view.cx,w/2,-w*5,w*6),cy=num(view.cy,h/2,-h*5,h*6)
  const transform=(x,y)=>({x:w/2+(x-cx)*k,y:h/2+(y-cy)*k})
  const byId=new Map(graph.nodes.map(n=>[n.id,n]))
  const linked=new Set(selectedGene?graph.edges.filter(e=>e.gene===selectedGene).map(e=>e.source):[])
  const edgeSvg=graph.edges.map(e=>{
    const a=byId.get(e.source),b=byId.get(e.target);if(!a||!b)return ''
    const A=transform(a.x,a.y),B=transform(b.x,b.y)
    if((A.x<-50&&B.x<-50)||(A.x>w+50&&B.x>w+50)||(A.y<-50&&B.y<-50)||(A.y>h+50&&B.y>h+50))return ''
    const emphasis=(selectedId&&(a.id===selectedId||b.id===selectedId))||(selectedGene&&(e.gene===selectedGene||linked.has(a.id)&&linked.has(b.id)))
    const width=graph.view==='overlap'?0.7+Math.min(3.5,(e.similarity||0)*4):.85
    return `<line x1="${fix(A.x)}" y1="${fix(A.y)}" x2="${fix(B.x)}" y2="${fix(B.y)}" stroke="${emphasis?'#315dce':'#9aa9c3'}" stroke-opacity="${emphasis?'.95':'.34'}" stroke-width="${fix(width)}"><title>${esc(graph.view==='overlap'?`${e.overlapGenes} common genes; ${graph.config.metric} ${fix(e.similarity)}`:`Gene membership: ${e.gene}`)}</title></line>`
  }).join('')
  const annotated=graph.nodes.filter(n=>n.type==='pathway').slice(0,k>=2?graph.nodes.length:8)
  const labels=new Set(annotated.map(n=>n.id))
  const nodeSvg=graph.nodes.map(n=>{
    const p=transform(n.x,n.y),r=radius(n,graph),selected=n.id===selectedId||n.type==='gene'&&n.name===selectedGene
    if(p.x<-r||p.x>w+r||p.y<-r||p.y>h+r)return ''
    const label=(selected||labels.has(n.id))&&n.type==='pathway'||selected&&n.type==='gene'
    const tooltip=n.type==='pathway'?`${n.name}; BH q ${n.q_value}; ${n.genes.length} displayed-overlap genes`:`${n.name}; ${n.degree} connected pathways`
    const border=selected?'#112c69':'#ffffff'
    return `<g data-network-node="${esc(n.id)}" tabindex="0" role="button" aria-label="${esc(tooltip)}" aria-pressed="${selected?'true':'false'}" style="cursor:pointer"><title>${esc(tooltip)}</title><circle cx="${fix(p.x)}" cy="${fix(p.y)}" r="${fix(r)}" fill="${nodeColor(n,graph)}" stroke="${border}" stroke-width="${selected?3:1.3}"/><circle cx="${fix(p.x)}" cy="${fix(p.y)}" r="${fix(Math.max(r+5,12))}" fill="transparent"/>${label?`<text x="${fix(p.x+r+4)}" y="${fix(p.y-5)}" font-size="12" fill="#172033" paint-order="stroke" stroke="#ffffff" stroke-width="3">${esc(n.name.length>36?n.name.slice(0,33)+'…':n.name)}</text>`:''}</g>`
  }).join('')
  return `<svg class="pathway-network-svg" role="img" aria-label="${esc(graph.view==='overlap'?'Pathways connected by shared measured genes':'Genes linked to the pathways containing them')}" viewBox="0 0 ${w} ${h}" xmlns="http://www.w3.org/2000/svg"><rect width="${w}" height="${h}" fill="#fafcff"/><g>${edgeSvg}</g><g>${nodeSvg}</g></svg>`
}
export function pathwayNetworkNodesTsv(graph){
  const columns=['node_id','node_type','name','pathway_id','BH_q','NES','fold_enrichment','gene_rank','gene_score','gene_q','gene_count','degree']
  return [columns.join('\t'),...graph.nodes.map(n=>[n.id,n.type,n.name,n.description||'',n.q_value??'',n.normalizedScore??'',n.foldEnrichment??'',n.rank??'',n.score??'',n.q??'',n.genes?.length??'',n.degree??''].map(tsv).join('\t'))].join('\n')+'\n'
}
export function pathwayNetworkEdgesTsv(graph){
  const columns=['source','target','edge_type','overlap_genes','similarity','metric','shared_gene_ids','member_gene']
  return [columns.join('\t'),...graph.edges.map(e=>[e.source,e.target,e.type,e.overlapGenes??'',e.similarity??'',e.metric??'',e.commonGenes?.join(';')??'',e.gene??''].map(tsv).join('\t'))].join('\n')+'\n'
}
export function pathwayNetworkJson(graph,report){return JSON.stringify({schema:PATHWAY_NETWORK_SCHEMA,generatedAt:new Date().toISOString(),
  graph:{view:graph.view,config:graph.config,nodes:graph.nodes.map(({vx,vy,...n})=>n),edges:graph.edges},
  method:report?.method,source:report?.source,geneSetFile:report?.geneSetFile,
  organism:report?.organism||null,geneSetRelease:report?.geneSetRelease||null,
  interpretation:'Graph layout is descriptive. Edges show gene sharing or membership, not interactions or independent significance.'},null,2)+'\n'}

function networkState(container,graph,onPathway){
  const viewport={cx:graph.width/2,cy:graph.height/2,k:1},selected={id:'',gene:''}
  let dragging=null,mode='pan',box=null,suppressClick=false
  const svgHolder=container.querySelector('.pathway-network-canvas')
  function paint(){svgHolder.innerHTML=pathwayNetworkSvg(graph,{view:viewport,selectedId:selected.id,selectedGene:selected.gene})+(box?`<div class="pathway-network-selection" style="left:${fix(box.x)}%;top:${fix(box.y)}%;width:${fix(box.w)}%;height:${fix(box.h)}%"></div>`:'')}
  const point=e=>{const bounds=svgHolder.getBoundingClientRect();return {x:((e.clientX-bounds.left)/(bounds.width||1))*graph.width,y:((e.clientY-bounds.top)/(bounds.height||1))*graph.height}}
  function focus(factor,at={x:graph.width/2,y:graph.height/2}){
    const old=viewport.k,next=clamp(old*factor,1,24);if(next===old)return
    viewport.cx+=((at.x-graph.width/2)/old)-((at.x-graph.width/2)/next)
    viewport.cy+=((at.y-graph.height/2)/old)-((at.y-graph.height/2)/next)
    viewport.k=next;paint()
  }
  function selectedLabel(n){
    if(n.type==='pathway'){
      selected.id=n.id;selected.gene='';onPathway?.(n.reportIndex)
      const detail=container.querySelector('.pathway-network-inspection');if(detail)detail.textContent=`${n.name} · q ${n.q_value} · ${n.genes.length} genes for this network. The pathway gene inspector is linked below.`
    } else {
      selected.gene=n.name;selected.id=''
      const connected=graph.edges.filter(e=>e.gene===n.name).map(e=>graph.nodes.find(p=>p.id===e.source)?.name).filter(Boolean)
      const detail=container.querySelector('.pathway-network-inspection');if(detail)detail.textContent=`${n.name} · measured rank ${n.rank??'unavailable'} · ${connected.length} pathway memberships: ${connected.join('; ')||'none'}`
    }
    paint()
  }
  function clickNode(e){const el=e.target.closest?.('[data-network-node]');if(!el||!svgHolder.contains(el))return false
    const n=graph.nodes.find(x=>x.id===el.dataset.networkNode);if(!n)return false;selectedLabel(n);return true}
  svgHolder.addEventListener('click',e=>{if(suppressClick){suppressClick=false;return}if(!dragging)clickNode(e)})
  svgHolder.addEventListener('keydown',e=>{if(!['Enter',' '].includes(e.key))return; if(clickNode(e))e.preventDefault()})
  svgHolder.addEventListener('wheel',e=>{e.preventDefault();focus(e.deltaY<0?1.24:1/1.24,point(e))},{passive:false})
  svgHolder.addEventListener('pointerdown',e=>{
    if(e.button!==0)return
    const p=point(e);dragging={start:p,last:p,moved:false}
    svgHolder.setPointerCapture?.(e.pointerId)
  })
  svgHolder.addEventListener('pointermove',e=>{
    if(!dragging)return
    const p=point(e);const moved=Math.hypot(p.x-dragging.start.x,p.y-dragging.start.y)
    if(moved>5)dragging.moved=true
    if(mode==='pan'&&dragging.moved){viewport.cx-=(p.x-dragging.last.x)/viewport.k;viewport.cy-=(p.y-dragging.last.y)/viewport.k;paint()}
    if(mode==='box'&&dragging.moved){const x=Math.min(p.x,dragging.start.x),y=Math.min(p.y,dragging.start.y)
      box={x:x/graph.width*100,y:y/graph.height*100,w:Math.abs(p.x-dragging.start.x)/graph.width*100,h:Math.abs(p.y-dragging.start.y)/graph.height*100};paint()}
    dragging.last=p
  })
  function finish(e){if(!dragging)return
    const moved=dragging.moved,begin=dragging.start,end=dragging.last;dragging=null
    if(moved)suppressClick=true
    if(mode==='box'&&moved&&Math.abs(end.x-begin.x)>10&&Math.abs(end.y-begin.y)>10){
      const cx=(begin.x+end.x)/2,cy=(begin.y+end.y)/2,original=viewport.k
      viewport.cx+=(cx-graph.width/2)/original;viewport.cy+=(cy-graph.height/2)/original
      viewport.k=clamp(original*Math.min(graph.width/Math.abs(end.x-begin.x),graph.height/Math.abs(end.y-begin.y)),1,24)
    }
    box=null;paint()
  }
  svgHolder.addEventListener('pointerup',finish)
  svgHolder.addEventListener('pointercancel',finish)
  container.querySelectorAll('[data-network-tool]').forEach(button=>button.addEventListener('click',()=>{
    const action=button.dataset.networkTool
    if(action==='reset'){viewport.cx=graph.width/2;viewport.cy=graph.height/2;viewport.k=1;selected.id='';selected.gene='';paint()}
    else if(action==='in')focus(1.4)
    else if(action==='out')focus(1/1.4)
    else {mode=action;container.querySelectorAll('[data-network-tool="pan"],[data-network-tool="box"]').forEach(x=>x.setAttribute('aria-pressed',String(x.dataset.networkTool===mode)))}
  }))
  paint()
  return {view:viewport,selected}
}

/** Mount once per pathway-view paint. Full results persist independently of network display settings. */
export function mountPathwayNetwork(root,{report,geneSets,input,config,onConfig=()=>{},onPathway=()=>{},exportFile=()=>{}}={}){
  const slot=root.querySelector('#pathway-network-widget');if(!slot)return
  let c=normalizePathwayNetworkConfig(config)
  function render(){
    let graph,error=''
    try{graph=layoutPathwayNetwork(buildPathwayNetwork(report,geneSets,c,input));graph.reportMode=report.mode}
    catch(e){error=str(e?.message||e)}
    if(!graph){slot.innerHTML=`<p class="analysis-warnings" role="status">Pathway network unavailable: ${esc(error)}</p>`;return}
    slot.innerHTML=`<section class="pathway-network" aria-label="Descriptive pathway networks">
      <div class="pathway-network-heading"><div><h6>Gene-sharing network</h6><p class="method-note">A descriptive graph of gene-set overlap. Lines are not protein interactions, regulatory effects, or new statistical tests.</p></div></div>
      <div class="pathway-network-controls">
        <label>Network view<select id="pathway-network-view"><option value="overlap"${c.view==='overlap'?' selected':''}>Pathway overlap</option><option value="bipartite"${c.view==='bipartite'?' selected':''}>Genes ↔ pathways</option></select></label>
        <label>Pathways to display<input id="pathway-network-count" type="number" min="2" max="250" value="${c.maxPathways}"/></label>
        <label>BH q cutoff<input id="pathway-network-q" type="number" min="0" max="1" step="0.01" value="${c.qCutoff}"/></label>
        <label>Gene basis<select id="pathway-network-genes"><option value="leading"${c.genes==='leading'?' selected':''}>Leading edge / ORA selected</option><option value="matched"${c.genes==='matched'?' selected':''}>All GMT matched genes</option></select></label>
      </div>
      <details class="pathway-network-advanced"><summary>Network options · overlap metric and rendering limits</summary><div class="pathway-network-controls">
        <label>Overlap metric<select id="pathway-network-metric"><option value="jaccard"${c.metric==='jaccard'?' selected':''}>Jaccard</option><option value="overlap"${c.metric==='overlap'?' selected':''}>Overlap coefficient</option></select></label>
        <label>Minimum shared genes<input id="pathway-network-min-common" type="number" min="1" max="500" value="${c.minCommon}"/></label>
        <label>Minimum overlap score<input id="pathway-network-min-sim" type="number" min="0" max="1" step="0.01" value="${c.minSimilarity}"/></label>
        <label>Maximum gene nodes (display only)<input id="pathway-network-max-genes" type="number" min="1" max="500" value="${c.maxGenes}"/></label>
        <label class="pathway-network-checkbox"><input id="pathway-network-hide-single" type="checkbox"${c.hideSingleGenes?' checked':''}/> Hide genes connected to one pathway</label>
        <label>Layout seed<input id="pathway-network-seed" type="number" min="1" max="4294967295" value="${c.seed}"/></label>
      </div></details>
      <div class="pathway-network-summary" role="status">${graph.nodes.filter(n=>n.type==='pathway').length} pathways · ${graph.nodes.filter(n=>n.type==='gene').length} displayed gene nodes · ${graph.edges.length} edges. ${graph.excludedExactSets.length?graph.excludedExactSets.length+' unmatched/ambiguous GMT pathways excluded.':''}</div>
      <div class="pathway-network-tools"><button type="button" class="secondary-button" data-network-tool="in">Zoom in</button><button type="button" class="secondary-button" data-network-tool="out">Zoom out</button><button type="button" class="secondary-button" data-network-tool="pan" aria-pressed="true">Pan</button><button type="button" class="secondary-button" data-network-tool="box" aria-pressed="false">Box zoom</button><button type="button" class="secondary-button" data-network-tool="reset">Reset</button></div>
      <div class="pathway-network-canvas" aria-label="Pathway network graph. Wheel zoom, drag to pan or box zoom. Select a node for details."></div>
      <div class="pathway-network-inspection" role="status">Select a pathway node to inspect its genes, or select a gene node to see connected pathways.</div>
      <p class="method-note">Node size reflects the pathway BH q-value. Blue and orange distinguish signed NES in ranked analysis; gene nodes are grey. The force layout is descriptive and deterministic for the saved layout seed. Display limits do not change tested pathways or the scientific results.</p>
      <div class="pathway-network-export"><button id="pathway-network-export-nodes" class="secondary-button" type="button">Export nodes TSV</button><button id="pathway-network-export-edges" class="secondary-button" type="button">Export edges TSV</button><button id="pathway-network-export-svg" class="secondary-button" type="button">Export SVG</button><button id="pathway-network-export-json" class="secondary-button" type="button">Export network JSON</button></div>
    </section>`
    networkState(slot,graph,onPathway)
    const controls={view:'pathway-network-view',maxPathways:'pathway-network-count',qCutoff:'pathway-network-q',genes:'pathway-network-genes',metric:'pathway-network-metric',minCommon:'pathway-network-min-common',minSimilarity:'pathway-network-min-sim',maxGenes:'pathway-network-max-genes',seed:'pathway-network-seed'}
    for(const [field,id] of Object.entries(controls))slot.querySelector('#'+id)?.addEventListener('change',e=>{
      c=normalizePathwayNetworkConfig({...c,[field]:e.currentTarget.value});onConfig(c);render()
    })
    slot.querySelector('#pathway-network-hide-single')?.addEventListener('change',e=>{c={...c,hideSingleGenes:e.currentTarget.checked};onConfig(c);render()})
    slot.querySelector('#pathway-network-export-nodes')?.addEventListener('click',()=>exportFile('4c-insight-pathway-network-nodes.tsv',pathwayNetworkNodesTsv(graph)))
    slot.querySelector('#pathway-network-export-edges')?.addEventListener('click',()=>exportFile('4c-insight-pathway-network-edges.tsv',pathwayNetworkEdgesTsv(graph)))
    slot.querySelector('#pathway-network-export-json')?.addEventListener('click',()=>exportFile('4c-insight-pathway-network.json',pathwayNetworkJson(graph,report)))
    slot.querySelector('#pathway-network-export-svg')?.addEventListener('click',()=>exportFile('4c-insight-pathway-network.svg',slot.querySelector('.pathway-network-svg')?.outerHTML||''))
  }
  render()
}
