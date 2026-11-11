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

/** Descriptive connected components of the *displayed* graph; never infer pathways or statistical modules. */
export function pathwayNetworkComponents(graph){
  const nodes=graph?.nodes||[],edges=graph?.edges||[]
  const byId=new Map(nodes.map(n=>[n.id,n]))
  const links=new Map(nodes.map(n=>[n.id,new Set()]))
  for(const e of edges){
    if(!links.has(e.source)||!links.has(e.target))continue
    links.get(e.source).add(e.target);links.get(e.target).add(e.source)
  }
  const visited=new Set(),groups=[]
  for(const start of nodes){
    if(visited.has(start.id))continue
    const pending=[start.id],members=[];visited.add(start.id)
    for(let i=0;i<pending.length;i++){
      const id=pending[i];members.push(byId.get(id))
      for(const next of links.get(id)||[])if(!visited.has(next)){visited.add(next);pending.push(next)}
    }
    const memberIds=new Set(members.map(n=>n.id))
    const pathways=members.filter(n=>n.type==='pathway').sort((a,b)=>
      (a.q_value??Infinity)-(b.q_value??Infinity)||a.name.localeCompare(b.name))
    const genes=members.filter(n=>n.type==='gene')
    const visibleGenes=new Set(genes.map(n=>n.name))
    const occurrence=new Map()
    for(const path of pathways)for(const gene of new Set(path.genes||[])){
      if(graph.view==='bipartite'&&!visibleGenes.has(gene))continue
      occurrence.set(gene,(occurrence.get(gene)||0)+1)
    }
    const sharedGenes=[...occurrence].filter(([,count])=>count>1).map(([gene])=>gene).sort()
    groups.push({id:members.map(n=>n.id).sort()[0],nodeIds:[...memberIds].sort(),
      pathways:pathways.map(n=>({id:n.id,name:n.name,reportIndex:n.reportIndex,q_value:n.q_value})),
      displayedGenes:genes.length,sharedGenes,edgeCount:edges.filter(e=>memberIds.has(e.source)&&memberIds.has(e.target)).length})
  }
  groups.sort((a,b)=>b.pathways.length-a.pathways.length||b.edgeCount-a.edgeCount||a.id.localeCompare(b.id))
  return groups.map((g,i)=>({...g,number:i+1}))
}

/** Project collision-aware SVG labels into a fixed coordinate frame. Display-only geometry. */
export function planPathwayNetworkLabels(graph,{view={cx:500,cy:285,k:1},selectedId='',selectedGene='',visibleNodeIds=null}={}){
  const w=graph?.width||1000,h=graph?.height||570,k=num(view.k,1,1,24)
  const cx=num(view.cx,w/2,-w*5,w*6),cy=num(view.cy,h/2,-h*5,h*6)
  const active=visibleNodeIds?new Set(visibleNodeIds):null
  const projected=(graph?.nodes||[]).map(n=>({node:n,x:w/2+(n.x-cx)*k,y:h/2+(n.y-cy)*k,r:radius(n,graph)}))
    .filter(p=>p.x>-30&&p.x<w+30&&p.y>-30&&p.y<h+30)
  const selected=n=>n.id===selectedId||(n.type==='gene'&&n.name===selectedGene)
  const priority=projected.filter(p=>(p.node.type==='pathway'||selected(p.node))&&(!active||active.has(p.node.id)||selected(p.node)))
    .sort((a,b)=>Number(selected(b.node))-Number(selected(a.node))||
      (a.node.q_value??Infinity)-(b.node.q_value??Infinity)||
      (b.node.degree??0)-(a.node.degree??0)||a.node.name.localeCompare(b.node.name))
  const budget=k<1.5?4:k<2.5?11:k<4?24:k<7?46:85
  const labels=[],bounds=[]
  const intersect=(a,b,pad=2)=>a.x<b.x+b.w+pad&&a.x+a.w+pad>b.x&&a.y<b.y+b.h+pad&&a.y+a.h+pad>b.y
  for(const p of priority){
    if(labels.length>=budget)break
    const nearby=projected.filter(q=>q.node.id!==p.node.id&&Math.hypot(q.x-p.x,q.y-p.y)<45).length
    // In a dense group, prefer the focus panel over crowded text. Selected labels are never discarded by this rule.
    if(!selected(p.node) && k<3 && nearby>=5)continue
    const short=p.node.name.length>(k<2?29:45)?p.node.name.slice(0,(k<2?26:42))+'…':p.node.name
    const tw=short.length*6.5+5,th=16,offset=p.r+6
    const choices=[
      {x:p.x+offset,y:p.y-th-1},
      {x:p.x-offset-tw,y:p.y-th-1},
      {x:p.x-tw/2,y:p.y-offset-th},
      {x:p.x-tw/2,y:p.y+offset+2},
      {x:p.x+offset,y:p.y+2},
      {x:p.x-offset-tw,y:p.y+2},
      {x:p.x+offset+5,y:p.y-offset-th},
      {x:p.x-offset-tw-5,y:p.y+offset+2},
    ]
    const box=choices.find(b=>{
      const rect={...b,w:tw,h:th}
      if(rect.x<4||rect.x+tw>w-4||rect.y<4||rect.y+th>h-4)return false
      if(bounds.some(other=>intersect(rect,other,4)))return false
      return !projected.some(q=>q.node.id!==p.node.id&&Math.max(rect.x-q.x,0,q.x-(rect.x+tw))**2+
        Math.max(rect.y-q.y,0,q.y-(rect.y+th))**2<(q.r+3)**2)
    })
    if(box){bounds.push({...box,w:tw,h:th});labels.push({nodeId:p.node.id,text:short,x:box.x,y:box.y+12,box:{...box,w:tw,h:th},selected:selected(p.node)})}
  }
  return labels
}

export function pathwayNetworkGroupsTsv(graph){
  const columns=['group','pathway_count','displayed_gene_nodes','edge_count','pathways','shared_measured_genes']
  return [columns.join('\t'),...pathwayNetworkComponents(graph).map(g=>[
    g.number,g.pathways.length,g.displayedGenes,g.edgeCount,g.pathways.map(p=>p.name).join(';'),g.sharedGenes.join(';')
  ].map(tsv).join('\t'))].join('\n')+'\n'
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
export function pathwayNetworkSvg(graph,{view={cx:500,cy:285,k:1},selectedId='',selectedGene='',visibleNodeIds=null}={}){
  const w=graph.width||1000,h=graph.height||570,k=num(view.k,1,1,24),cx=num(view.cx,w/2,-w*5,w*6),cy=num(view.cy,h/2,-h*5,h*6)
  const transform=(x,y)=>({x:w/2+(x-cx)*k,y:h/2+(y-cy)*k})
  const byId=new Map(graph.nodes.map(n=>[n.id,n]))
  const linked=new Set(selectedGene?graph.edges.filter(e=>e.gene===selectedGene).map(e=>e.source):[])
  const active=visibleNodeIds?new Set(visibleNodeIds):null
  const edgeSvg=graph.edges.map(e=>{
    const a=byId.get(e.source),b=byId.get(e.target);if(!a||!b)return ''
    const A=transform(a.x,a.y),B=transform(b.x,b.y)
    if((A.x<-50&&B.x<-50)||(A.x>w+50&&B.x>w+50)||(A.y<-50&&B.y<-50)||(A.y>h+50&&B.y>h+50))return ''
    const emphasis=(selectedId&&(a.id===selectedId||b.id===selectedId))||(selectedGene&&(e.gene===selectedGene||linked.has(a.id)&&linked.has(b.id)))
    const faded=active&&(!active.has(a.id)||!active.has(b.id))
    const width=graph.view==='overlap'?0.7+Math.min(3.5,(e.similarity||0)*4):.85
    return `<line x1="${fix(A.x)}" y1="${fix(A.y)}" x2="${fix(B.x)}" y2="${fix(B.y)}" stroke="${emphasis?'#315dce':'#9aa9c3'}" stroke-opacity="${faded?'.07':emphasis?'.95':'.34'}" stroke-width="${fix(width)}"><title>${esc(graph.view==='overlap'?`${e.overlapGenes} common genes; ${graph.config.metric} ${fix(e.similarity)}`:`Gene membership: ${e.gene}`)}</title></line>`
  }).join('')
  const nodeSvg=graph.nodes.map(n=>{
    const p=transform(n.x,n.y),r=radius(n,graph),selected=n.id===selectedId||n.type==='gene'&&n.name===selectedGene
    if(p.x<-r||p.x>w+r||p.y<-r||p.y>h+r)return ''
    const tooltip=n.type==='pathway'?`${n.name}; BH q ${n.q_value}; ${n.genes.length} displayed-overlap genes`:`${n.name}; ${n.degree} connected pathways`
    const border=selected?'#112c69':'#ffffff'
    const faded=active&&!active.has(n.id)&&!selected
    return `<g data-network-node="${esc(n.id)}" tabindex="0" role="button" aria-label="${esc(tooltip)}" aria-pressed="${selected?'true':'false'}" opacity="${faded?'.18':'1'}" style="cursor:pointer"><title>${esc(tooltip)}</title><circle cx="${fix(p.x)}" cy="${fix(p.y)}" r="${fix(r)}" fill="${nodeColor(n,graph)}" stroke="${border}" stroke-width="${selected?3:1.3}"/><circle cx="${fix(p.x)}" cy="${fix(p.y)}" r="${fix(Math.max(r+5,12))}" fill="transparent"/></g>`
  }).join('')
  const annotations=planPathwayNetworkLabels(graph,{view,selectedId,selectedGene,visibleNodeIds})
    .map(label=>`<text x="${fix(label.x)}" y="${fix(label.y)}" font-size="12" fill="#172033" pointer-events="none" paint-order="stroke" stroke="#ffffff" stroke-width="3">${esc(label.text)}</text>`).join('')
  return `<svg class="pathway-network-svg" role="img" aria-label="${esc(graph.view==='overlap'?'Pathways connected by shared measured genes':'Genes linked to the pathways containing them')}" viewBox="0 0 ${w} ${h}" xmlns="http://www.w3.org/2000/svg"><rect width="${w}" height="${h}" fill="#fafcff"/><g>${edgeSvg}</g><g>${nodeSvg}</g><g class="pathway-network-labels" aria-hidden="true">${annotations}</g></svg>`
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

/** Fit all displayed nodes in a fixed SVG coordinate frame. This does not change layout, data, or marker size. */
export function pathwayNetworkFitView(graph,{padding=45}={}){
  const nodes=(graph?.nodes||[]).filter(n=>valid(n.x)&&valid(n.y))
  const w=Number(graph?.width)||1000,h=Number(graph?.height)||570
  if(!nodes.length)return {cx:w/2,cy:h/2,k:1}
  const minX=Math.min(...nodes.map(n=>n.x)),maxX=Math.max(...nodes.map(n=>n.x))
  const minY=Math.min(...nodes.map(n=>n.y)),maxY=Math.max(...nodes.map(n=>n.y))
  const pad=clamp(Number(padding)||45,20,Math.min(w,h)*.3)
  const k=clamp(Math.min((w-2*pad)/Math.max(1,maxX-minX),(h-2*pad)/Math.max(1,maxY-minY)),1,24)
  return {cx:(minX+maxX)/2,cy:(minY+maxY)/2,k}
}
/** Read-only selection card: immediately visible above the graph, including in the expanded view.
 * Pathway selections link to the existing full gene inspector; genes link to measured pathways.
 */
export function pathwayNetworkSelectionHtml(graph,node){
  if(!node)return '<strong>Inspect a network node</strong><p class="method-note">Select a blue/orange pathway node to see its enrichment evidence, or a grey gene node to see the pathways that contain it.</p>'
  if(node.type==='pathway'){
    const q=valid(node.q_value)?node.q_value.toPrecision(3):'not available'
    const statistic=graph.reportMode==='ora'?'ORA fold enrichment':'Normalized enrichment score (NES)'
    const stat=graph.reportMode==='ora'?node.foldEnrichment:node.normalizedScore
    const preview=(node.genes||[]).slice(0,12)
    return `<div class="pathway-network-node-heading"><strong>${esc(node.name)}</strong><span class="tag neutral">Selected pathway</span></div>
      <p class="method-note">BH q: <strong>${esc(q)}</strong> · ${statistic}: <strong>${valid(stat)?esc(fix(stat)):'unavailable'}</strong> · ${node.genes.length} measured genes in the current network basis.</p>
      <p class="method-note">${preview.length?'Network genes: '+preview.map(esc).join(', ')+(node.genes.length>preview.length?' …':''):'No network genes under these filters.'} This is gene-set sharing, not a physical interaction.</p>
      <button type="button" class="secondary-button" data-network-open-pathway="${esc(node.id)}">Open full pathway gene table</button>`
  }
  const paths=graph.edges.filter(e=>e.type==='gene_membership'&&e.gene===node.name)
    .map(e=>graph.nodes.find(n=>n.id===e.source)).filter(n=>n?.type==='pathway')
    .sort((a,b)=>(a.q_value??Infinity)-(b.q_value??Infinity)||a.name.localeCompare(b.name))
  return `<div class="pathway-network-node-heading"><strong>${esc(node.name)}</strong><span class="tag neutral">Selected measured gene</span></div>
    <p class="method-note">Original gene rank: <strong>${node.rank??'unavailable'}</strong> · ${graph.reportMode==='ora'?'Gene score':'Original ranking score'}: <strong>${valid(node.score)?esc(fix(node.score)):'unavailable'}</strong> · Connected pathways in displayed network: <strong>${paths.length}</strong>.</p>
    <div class="pathway-network-connected-list">${paths.length?paths.map(p=>`<button type="button" class="secondary-button" data-network-open-pathway="${esc(p.id)}">${esc(p.name)} · q ${valid(p.q_value)?esc(p.q_value.toPrecision(3)):'—'}</button>`).join(''):'<span>No connected pathways are displayed.</span>'}</div>
    <p class="method-note">These are memberships in GMT gene sets, not protein interactions or disease associations.</p>`
}
export function networkState(container,graph,onPathway){
  const viewport={cx:graph.width/2,cy:graph.height/2,k:1},selected={id:'',gene:''}
  container.dataset.networkSelectedPathway=''
  const groups=pathwayNetworkComponents(graph),groupById=new Map(groups.map(g=>[g.id,g]))
  let focusedGroup=null
  let dragging=null,mode='pan',box=null,lastPointerActivation=-Infinity,lastDragEnd=-Infinity
  const svgHolder=container.querySelector('.pathway-network-canvas')
  function paint(){svgHolder.innerHTML=pathwayNetworkSvg(graph,{view:viewport,selectedId:selected.id,selectedGene:selected.gene,visibleNodeIds:focusedGroup?.nodeIds||null})+(box?`<div class="pathway-network-selection" style="left:${fix(box.x)}%;top:${fix(box.y)}%;width:${fix(box.w)}%;height:${fix(box.h)}%"></div>`:'')}
  const point=e=>{const bounds=svgHolder.getBoundingClientRect();return {x:((e.clientX-bounds.left)/(bounds.width||1))*graph.width,y:((e.clientY-bounds.top)/(bounds.height||1))*graph.height}}
  function focus(factor,at={x:graph.width/2,y:graph.height/2}){
    const old=viewport.k,next=clamp(old*factor,1,24);if(next===old)return
    viewport.cx+=((at.x-graph.width/2)/old)-((at.x-graph.width/2)/next)
    viewport.cy+=((at.y-graph.height/2)/old)-((at.y-graph.height/2)/next)
    viewport.k=next;paint()
  }
  const groupSelect=container.querySelector('#pathway-network-group-select')
  const groupDetail=container.querySelector('.pathway-network-group-detail')
  const groupStatus=container.querySelector('.pathway-network-focus-status')
  function showGroup(){
    if(groupStatus)groupStatus.textContent=focusedGroup?`Showing Group ${focusedGroup.number} of ${groups.length} · ${focusedGroup.pathways.length} pathways`:'All groups displayed' 
    if(!groupDetail)return
    if(!focusedGroup){groupDetail.textContent='Select a connected group to focus its nodes and inspect its pathways. Each group is based only on visible network edges.';return}
    groupDetail.innerHTML=`<p><strong>Group ${focusedGroup.number}</strong> · ${focusedGroup.pathways.length} pathways · ${focusedGroup.edgeCount} edges · ${focusedGroup.sharedGenes.length} shared measured genes</p>
      <p class="method-note">${focusedGroup.sharedGenes.length?'Shared measured genes: '+esc(focusedGroup.sharedGenes.join(', ')):'No shared genes across two displayed pathways in this group.'}</p>
      <div class="pathway-network-group-paths">${focusedGroup.pathways.map(p=>`<button type="button" class="secondary-button" data-network-group-path="${esc(p.id)}">${esc(p.name)}</button>`).join('')}</div>`
    groupDetail.querySelectorAll('[data-network-group-path]').forEach(button=>button.addEventListener('click',()=>{
      const node=graph.nodes.find(n=>n.id===button.dataset.networkGroupPath)
      if(node)selectedLabel(node)
    }))
  }
  groupSelect?.addEventListener('change',e=>{
    focusedGroup=groupById.get(e.currentTarget.value)||null
    const area=focusedGroup?graph.nodes.filter(n=>focusedGroup.nodeIds.includes(n.id)):graph.nodes
    const fitted=pathwayNetworkFitView({...graph,nodes:area})
    viewport.cx=fitted.cx;viewport.cy=fitted.cy;viewport.k=fitted.k
    showGroup();paint()
  })
  container.querySelector('#pathway-network-show-all')?.addEventListener('click',()=>{
    if(groupSelect)groupSelect.value=''
    focusedGroup=null
    const fitted=pathwayNetworkFitView(graph)
    viewport.cx=fitted.cx;viewport.cy=fitted.cy;viewport.k=fitted.k
    showGroup();paint()
  })
  showGroup()
  const inspection=container.querySelector('.pathway-network-inspection')
  function showSelection(n){if(inspection)inspection.innerHTML=pathwayNetworkSelectionHtml(graph,n)}
  showSelection(null)
  function openFullInspector(){
    const dialog=container.querySelector('#pathway-network-dialog')
    if(dialog?.open)dialog.close()
    const inspector=container.closest('.pathway-results')?.querySelector('#pathway-gene-inspector') ||
      container.ownerDocument?.querySelector('#pathway-gene-inspector')
    inspector?.scrollIntoView?.({block:'start',behavior:'auto'})
    inspector?.querySelector('h6,h5,strong,button')?.focus?.({preventScroll:true})
  }
  function selectedLabel(n,{openDetails=false}={}){
    if(n.type==='pathway'){
      selected.id=n.id;selected.gene='';container.dataset.networkSelectedPathway=n.id;onPathway?.(n.reportIndex)
    } else {selected.gene=n.name;selected.id='';container.dataset.networkSelectedPathway=''}
    showSelection(n);paint()
    if(openDetails && n.type==='pathway')openFullInspector()
  }
  inspection?.addEventListener('click',event=>{
    const button=event.target.closest?.('[data-network-open-pathway]')
    if(!button)return
    const node=graph.nodes.find(n=>n.id===button.dataset.networkOpenPathway)
    if(node?.type==='pathway')selectedLabel(node,{openDetails:true})
  })
  function nodeAt(target){
    const el=target?.closest?.('[data-network-node]')
    if(!el||!svgHolder.contains(el))return null
    return graph.nodes.find(n=>n.id===el.dataset.networkNode)||null
  }
  function clickNode(e){const n=nodeAt(e.target);if(!n)return false;selectedLabel(n);return true}
  // Pointer capture retargets a native click to the canvas, not the original SVG node.
  // Pointerup selects the node saved on pointerdown. Click remains a fallback for
  // scripted clicks and older input devices; ignore the paired native click.
  svgHolder.addEventListener('click',e=>{
    if(e.detail>0&&((e.timeStamp>=lastPointerActivation&&e.timeStamp-lastPointerActivation<500)||
        (e.timeStamp>=lastDragEnd&&e.timeStamp-lastDragEnd<500)))return
    if(!dragging)clickNode(e)
  })
  svgHolder.addEventListener('keydown',e=>{
    if(!['Enter',' '].includes(e.key))return
    const node=nodeAt(e.target)
    if(clickNode(e)){
      e.preventDefault()
      // SVG redraw replaces the focused node; restore keyboard focus.
      const replacement=[...svgHolder.querySelectorAll('[data-network-node]')].find(el=>el.dataset.networkNode===node?.id)
      replacement?.focus?.({preventScroll:true})
    }
  })
  svgHolder.addEventListener('wheel',e=>{e.preventDefault();focus(e.deltaY<0?1.24:1/1.24,point(e))},{passive:false})
  svgHolder.addEventListener('pointerdown',e=>{
    if(e.button!==0)return
    const p=point(e),node=nodeAt(e.target)
    dragging={start:p,last:p,moved:false,nodeId:node?.id||'',pointerId:e.pointerId}
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
    const {moved,start:begin,last:end,nodeId,pointerId}=dragging;dragging=null
    if(moved)lastDragEnd=e.timeStamp
    // A stationary mouse/touch tap is a selection, even when pointer capture
    // directs pointerup and click at the plot container instead of the node.
    if(!moved&&nodeId&&e.type==='pointerup'){
      const node=graph.nodes.find(n=>n.id===nodeId)
      if(node){lastPointerActivation=e.timeStamp;selectedLabel(node)}
    }
    if(svgHolder.hasPointerCapture?.(pointerId))svgHolder.releasePointerCapture(pointerId)
    if(mode==='box'&&moved&&Math.abs(end.x-begin.x)>10&&Math.abs(end.y-begin.y)>10){
      const cx=(begin.x+end.x)/2,cy=(begin.y+end.y)/2,original=viewport.k
      viewport.cx+=(cx-graph.width/2)/original;viewport.cy+=(cy-graph.height/2)/original
      viewport.k=clamp(original*Math.min(graph.width/Math.abs(end.x-begin.x),graph.height/Math.abs(end.y-begin.y)),1,24)
    }
    box=null
    if(moved)paint() // Keep the node DOM intact for the subsequent native click event.
  }
  svgHolder.addEventListener('pointerup',finish)
  svgHolder.addEventListener('pointercancel',finish)
  function fitNodes(){
    const fitted=pathwayNetworkFitView(graph)
    viewport.cx=fitted.cx;viewport.cy=fitted.cy;viewport.k=fitted.k;paint()
  }
  container.querySelectorAll('[data-network-tool]').forEach(button=>button.addEventListener('click',()=>{
    const action=button.dataset.networkTool
    if(action==='reset'){viewport.cx=graph.width/2;viewport.cy=graph.height/2;viewport.k=1;selected.id='';selected.gene='';container.dataset.networkSelectedPathway='';showSelection(null);paint()}
    else if(action==='fit')fitNodes()
    else if(action==='in')focus(1.4)
    else if(action==='out')focus(1/1.4)
    else {mode=action;container.querySelectorAll('[data-network-tool="pan"],[data-network-tool="box"]').forEach(x=>x.setAttribute('aria-pressed',String(x.dataset.networkTool===mode)))}
  }))
  paint()
  return {view:viewport,selected}
}

/** Move the active graph and its bound controls into a native dialog; never clone the graph. */
export function mountExpandedNetwork(slot){
  const expand=slot.querySelector('#pathway-network-expand')
  const dialog=slot.querySelector('#pathway-network-dialog')
  const home=slot.querySelector('.pathway-network-home')
  const stage=dialog?.querySelector('.pathway-network-modal-stage')
  if(!expand||!dialog||!home||!stage)return
  const moved=['.pathway-network-focus','.pathway-network-tools','.pathway-network-canvas','.pathway-network-inspection','.pathway-network-legend','.pathway-network-components','.pathway-network-help','.pathway-network-export']
    .map(selector=>home.querySelector(selector)).filter(Boolean)
  let active=false
  function restore(){
    if(!active)return
    for(const element of moved)home.append(element)
    active=false
    expand.setAttribute('aria-expanded','false')
    expand.focus()
  }
  expand.addEventListener('click',()=>{
    if(active)return
    if(typeof dialog.showModal!=='function'){
      const status=slot.querySelector('.pathway-network-inspection')
      if(status)status.textContent='Expanded plots require a browser with native dialog support. Use zoom and fit controls in this plot.'
      return
    }
    for(const element of moved)stage.append(element)
    try{
      dialog.showModal()
      active=true
      expand.setAttribute('aria-expanded','true')
      dialog.querySelector('#pathway-network-close')?.focus()
    }catch(error){
      for(const element of moved)home.append(element)
      active=false
      expand.setAttribute('aria-expanded','false')
    }
  })
  dialog.addEventListener('close',restore)
  dialog.querySelector('#pathway-network-close')?.addEventListener('click',()=>dialog.close())
  dialog.querySelector('#pathway-network-show-genes')?.addEventListener('click',()=>{
    if(!slot.dataset.networkSelectedPathway){
      const status=slot.querySelector('.pathway-network-inspection')
      if(status)status.innerHTML='<strong>Select a pathway first.</strong><p class="method-note">Select a blue/orange pathway node, then open its full gene table.</p>'
      return
    }
    dialog.close()
    const inspector=slot.closest('.pathway-results')?.querySelector('#pathway-gene-inspector') || slot.ownerDocument?.querySelector('#pathway-gene-inspector')
    inspector?.scrollIntoView?.({block:'start'})
  })
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
    const groups=pathwayNetworkComponents(graph)
    const alone=groups.filter(g=>g.pathways.length===1).length
    slot.innerHTML=`<section class="pathway-network" aria-label="Descriptive pathway networks">
      <div class="pathway-network-heading"><div><h6>Gene-sharing network</h6><p class="method-note">A descriptive graph of gene-set overlap. Lines are not protein interactions, regulatory effects, or new statistical tests.</p></div><button type="button" id="pathway-network-expand" class="secondary-button pathway-network-expand" aria-haspopup="dialog" aria-expanded="false" aria-controls="pathway-network-dialog">Expand plot</button></div>
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
      <div class="pathway-network-summary" role="status">${graph.nodes.filter(n=>n.type==='pathway').length} pathways · ${graph.nodes.filter(n=>n.type==='gene').length} displayed gene nodes · ${graph.edges.length} edges · ${groups.length} connected groups (${alone} one-pathway groups). ${graph.excludedExactSets.length?graph.excludedExactSets.length+' unmatched/ambiguous GMT pathways excluded.':''}</div>
      <div class="pathway-network-home">
      <div class="pathway-network-focus"><label>Focus connected group<select id="pathway-network-group-select"><option value="">All displayed groups</option>${groups.map(g=>`<option value="${esc(g.id)}">Group ${g.number} · ${g.pathways.length} pathway${g.pathways.length===1?'':'s'} · ${g.edgeCount} edges</option>`).join('')}</select></label>
        <button type="button" class="secondary-button" id="pathway-network-show-all">Show all groups</button>
        <span class="pathway-network-focus-status" role="status">All groups displayed</span></div>
      <div class="pathway-network-tools"><button type="button" class="secondary-button" data-network-tool="in">Zoom in</button><button type="button" class="secondary-button" data-network-tool="out">Zoom out</button><button type="button" class="secondary-button" data-network-tool="pan" aria-pressed="true">Pan</button><button type="button" class="secondary-button" data-network-tool="box" aria-pressed="false">Box zoom</button><button type="button" class="secondary-button" data-network-tool="fit">Fit nodes</button><button type="button" class="secondary-button" data-network-tool="reset">Reset</button></div>
      <div class="pathway-network-inspection" role="region" aria-label="Selected pathway or gene evidence" aria-live="polite"></div>
      <div class="pathway-network-canvas" aria-label="Pathway network graph. Wheel zoom, drag to pan or box zoom. Select a node for details."></div>
      <div class="pathway-network-legend" aria-label="Network symbols explained">
        <strong>Network legend</strong> <span><i class="pathway-network-swatch pathway-positive" aria-hidden="true"></i> ${graph.reportMode==='ora'?'ORA pathway':'Positive NES pathway'}</span>
        ${graph.reportMode==='ora'?'':`<span><i class="pathway-network-swatch pathway-negative" aria-hidden="true"></i> Negative NES pathway</span>`}
        ${graph.view==='bipartite'?'<span><i class="pathway-network-swatch pathway-gene" aria-hidden="true"></i> Measured gene</span>':''}
        <small>Larger pathway circle: lower BH q (capped). ${graph.view==='overlap'?'Thicker line: greater gene overlap.':'Line: gene belongs to pathway.'} Node positions and connected groups are descriptive, not evidence of interaction or disease mechanism.</small>
      </div>
      <details class="pathway-network-components"><summary>Connected group details · ${groups.length} groups, ${alone} single-pathway groups</summary>
        <div class="pathway-network-group-detail" aria-live="polite"></div>
        <p class="method-note">Connected groups depend on the visible node and overlap thresholds. They are not independently tested biological modules.</p>
      </details>
      <p class="method-note pathway-network-help">Node size reflects the pathway BH q-value. Blue and orange distinguish signed NES in ranked analysis; gene nodes are grey. The force layout is descriptive and deterministic for the saved layout seed. Display limits do not change tested pathways or the scientific results.</p>
      <div class="pathway-network-export"><button id="pathway-network-export-nodes" class="secondary-button" type="button">Export nodes TSV</button><button id="pathway-network-export-edges" class="secondary-button" type="button">Export edges TSV</button><button id="pathway-network-export-svg" class="secondary-button" type="button">Export SVG</button><button id="pathway-network-export-json" class="secondary-button" type="button">Export network JSON</button><button id="pathway-network-export-groups" class="secondary-button" type="button">Export groups TSV</button></div>
      </div>
      <dialog class="pathway-network-dialog" id="pathway-network-dialog" aria-labelledby="pathway-network-modal-title" aria-describedby="pathway-network-modal-description">
        <div class="pathway-network-dialog-heading"><div><h6 id="pathway-network-modal-title">Expanded gene-sharing network</h6><p id="pathway-network-modal-description">${esc(graph.view==='overlap'?'Pathway overlap':'Genes and pathways')} · select nodes, zoom, pan, or export. Layout is descriptive.</p></div><div class="pathway-network-dialog-actions"><button type="button" class="secondary-button" id="pathway-network-show-genes">Inspect selected pathway</button><button type="button" class="secondary-button" id="pathway-network-close">Close plot</button></div></div>
        <div class="pathway-network-modal-stage"></div>
      </dialog>
    </section>`
    networkState(slot,graph,onPathway)
    mountExpandedNetwork(slot)
    const controls={view:'pathway-network-view',maxPathways:'pathway-network-count',qCutoff:'pathway-network-q',genes:'pathway-network-genes',metric:'pathway-network-metric',minCommon:'pathway-network-min-common',minSimilarity:'pathway-network-min-sim',maxGenes:'pathway-network-max-genes',seed:'pathway-network-seed'}
    for(const [field,id] of Object.entries(controls))slot.querySelector('#'+id)?.addEventListener('change',e=>{
      c=normalizePathwayNetworkConfig({...c,[field]:e.currentTarget.value});onConfig(c);render()
    })
    slot.querySelector('#pathway-network-hide-single')?.addEventListener('change',e=>{c={...c,hideSingleGenes:e.currentTarget.checked};onConfig(c);render()})
    slot.querySelector('#pathway-network-export-nodes')?.addEventListener('click',()=>exportFile('4c-insight-pathway-network-nodes.tsv',pathwayNetworkNodesTsv(graph)))
    slot.querySelector('#pathway-network-export-edges')?.addEventListener('click',()=>exportFile('4c-insight-pathway-network-edges.tsv',pathwayNetworkEdgesTsv(graph)))
    slot.querySelector('#pathway-network-export-json')?.addEventListener('click',()=>exportFile('4c-insight-pathway-network.json',pathwayNetworkJson(graph,report)))
    slot.querySelector('#pathway-network-export-groups')?.addEventListener('click',()=>exportFile('4c-insight-pathway-network-groups.tsv',pathwayNetworkGroupsTsv(graph)))
    slot.querySelector('#pathway-network-export-svg')?.addEventListener('click',()=>exportFile('4c-insight-pathway-network.svg',slot.querySelector('.pathway-network-svg')?.outerHTML||''))
  }
  render()
}
