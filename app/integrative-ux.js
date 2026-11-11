import {bindPlotNavigation, bindPlotToolbar, plotNavigationToolbar, PLOT_ZOOM_MAX} from './plot-navigation.js?v=0.22.8'
/** Integrative Overview presentation only. Does not fit, label, or change scientific results.
 * All data comes from completed validated local analyses.
 */
const isFiniteNumber = n => typeof n === 'number' && Number.isFinite(n)
const defaultEscape = text => String(text ?? '').replaceAll('&','&amp;').replaceAll('<','&lt;')
  .replaceAll('>','&gt;').replaceAll('"','&quot;').replaceAll("'",'&#39;')
const number = x => Number.isInteger(x) && x >= 0 ? x.toLocaleString('en-GB') : '—'
const fixed = (x,n=3) => isFiniteNumber(x) ? x.toFixed(n) : '—'
const percent = x => isFiniteNumber(x) && x>=0 && x<=1 ? `${(100*x).toFixed(1)}%` : 'Not available'

export function integrativeSummaryHtml({eligible,retained,modalities,clusters,ari,escape=defaultEscape}={}) {
  const excluded = Number.isInteger(eligible)&&Number.isInteger(retained)&&eligible>=retained ? eligible-retained : null
  const metrics=[
    ['Aligned observations',number(eligible),'After metadata exclusions'],
    ['Observations in joint model',number(retained),'Complete fixed-view distance matrix'],
    ['Not in joint model',number(excluded),'See source coverage and exclusions'],
    ['Data types fitted',Number.isInteger(modalities)?`${modalities} / 7`:'—','Selected source modalities'],
  ]
  return `<div class="integrative-study-summary"><div class="summary-grid compact-grid">${metrics.map(([label,value,detail])=>`<div class="summary-card"><span>${escape(label)}</span><strong>${escape(value)}</strong><small>${escape(detail)}</small></div>`).join('')}</div>
    <p class="integrative-study-takeaway">${clusters===null?'Cluster counts are not available. Review the joint model in Details.':`The joint model has ${number(clusters)} descriptive clusters.`} ${isFiniteNumber(ari)?`Mean patient-subset cluster agreement (adjusted Rand index): ${fixed(ari)}. This measures repeat stability; it does not validate biological or clinical subtypes.`:'Patient-subset cluster agreement has not been assessed.'} Review missing observations before you interpret the clusters.</p></div>`
}

/** Fixed-cohort deletion sensitivity from the scientific structure, not refitted clinical labels. */
export function integrativeInfluenceHtml(structure,escape=defaultEscape,format=fixed) {
  const names={gene:'Gene expression',transcript:'Transcript expression',bcr:'BCR repertoire',tcrAlpha:'TCR alpha',tcrBeta:'TCR beta',isotype:'BCR isotypes',immune:'Immune infiltration'}
  const rows=structure?.fixed_ablation
  if(!Array.isArray(rows)||!rows.length) return '<p class="method-note">The leave-one-data-type-out comparison is unavailable. At least three fitted data types are needed for a comparison with two remaining data types.</p>'
  return `<p class="method-note">Each row shows what happens to the fixed cohort distances when one data type is removed. Lower distance correlation or lower nearest-neighbor retention means greater sensitivity to that data type. These measures do not test clinical-group differences or independent biomarker importance.</p>
    <div class="integrative-ablation-grid" role="list">${rows.map(r=>{
      const available=r.available && isFiniteNumber(r.distance_correlation) && isFiniteNumber(r.nearest_neighbor_retention)
      return `<article class="integrative-ablation-tile" role="listitem"><strong>${escape(names[r.omitted]||r.omitted)}</strong>${available?`<span>Distance correlation <b>${escape(format(r.distance_correlation,3))}</b></span>
        <span>Same nearest neighbor <b>${percent(r.nearest_neighbor_retention)}</b></span>
        <div class="bio-meter" role="img" aria-label="Nearest neighbor retention ${percent(r.nearest_neighbor_retention)}"><i style="width:${(100*r.nearest_neighbor_retention).toFixed(1)}%;background:#3159d9"></i></div>`:`<small>Not available${r.reason?`: ${escape(r.reason)}`:''}</small>`}</article>`
    }).join('')}</div><p class="method-note">These are same-cohort leave-one-out distance comparisons. See Details for exact pair counts, full values and export.</p>`
}

const color = ['#3159d9','#d97706','#15803d','#a21caf','#0f766e','#b91c1c','#64748b','#2563eb','#854d0e','#be185d','#0369a1','#4d7c0f']
function pointColor(point,report) {
  const meta=report?.metadata
  if(meta?.kind==='categorical'&&!meta.too_many_categories){
    const idx=(meta.categories||[]).findIndex(c=>c.value===point.annotation)
    return idx>=0?color[idx%color.length]:'#94a3b8'
  }
  if(meta?.kind==='numeric') {
    const x=point.annotation===''?NaN:Number(point.annotation)
    const [lo,hi]=meta.range||[]
    if(!Number.isFinite(x)||!Number.isFinite(lo)||!Number.isFinite(hi))return '#94a3b8'
    const t=hi>lo?Math.max(0,Math.min(1,(x-lo)/(hi-lo))):.5
    return `rgb(${Math.round(220-184*t)},${Math.round(235-156*t)},${Math.round(251-28*t)})`
  }
  return color[((point.cluster-1)%color.length+color.length)%color.length]
}
export function integrativeClusterCautionHtml(report,escape=defaultEscape){
  const small=(report?.membership||[]).filter(r=>Number.isInteger(r.observations)&&r.observations>0&&r.observations<=2)
  if(!small.length)return ''
  const single=small.filter(r=>r.observations===1)
  const names=small.map(r=>`cluster ${r.cluster} (${r.observations} observation${r.observations===1?'':'s'})`).join(', ')
  const singleton=single.length?` ${single.length} cluster${single.length===1?' contains':'s contain'} only one observation and cannot establish a biological subtype.`:''
  const sampleActions=single.flatMap(r=>(report?.points||[]).filter(p=>p.cluster===r.cluster)).map(p=>
    `<button type="button" class="secondary-button" data-integrative-inspect-sample="${escape(p.observation)}">Inspect ${escape(p.observation)}</button>`).join(' ')
  return `<div class="integrative-cluster-caution" role="note"><strong>Review small descriptive clusters:</strong> ${escape(names)}.${singleton} The adjusted Rand index can be high when most observations belong to one group. Check source coverage, sample metadata and QC before any biological interpretation. ${sampleActions}</div>`
}

export function integrativeScatterMarkup(report,axes=[],escape=defaultEscape,format=fixed) {
  if(!report?.points?.length)return '<p class="method-note">No joint coordinates are available.</p>'
  const pts=report.points.filter(p=>isFiniteNumber(p.x)&&isFiniteNumber(p.y))
  if(!pts.length)return '<p class="method-note">No finite PCoA coordinates are available.</p>'
  const width=650,height=365,pad=49
  const xs=pts.map(p=>p.x),ys=pts.map(p=>p.y)
  const extent=values=>{let min=Infinity,max=-Infinity;for(const v of values){if(v<min)min=v;if(v>max)max=v}return min===max?[min-1,max+1]:[min-(max-min)*.05,max+(max-min)*.05]}
  const [xmin,xmax]=extent(xs),[ymin,ymax]=extent(ys)
  const sx=x=>pad+(x-xmin)/(xmax-xmin)*(width-2*pad)
  const sy=y=>height-pad-(y-ymin)/(ymax-ymin)*(height-2*pad)
  const covered=new Map((report.coverage||[]).map(row=>[row.observation,row]))
  const circles=pts.map((p,i)=>{
    const row=covered.get(p.observation)||{}
    const x=sx(p.x).toFixed(2),y=sy(p.y).toFixed(2),obs=escape(p.observation)
    return `<circle data-integrative-point="${i}" data-observation="${obs}" tabindex="0" role="button" aria-label="Inspect observation ${obs}" cx="${x}" cy="${y}" r="4.2" fill="${pointColor(p,report)}" opacity="0.87" stroke="#fff" stroke-width="0.65"><title>${obs} · Patient: ${escape(row.patient||'Not selected')} · Visit: ${escape(row.timepoint||'Not selected')} · Cluster ${escape(p.cluster)} · PCo1 ${escape(format(p.x,4))}; PCo2 ${escape(format(p.y,4))}</title></circle><text class="integrative-point-label" data-integrative-label="${i}" x="${x}" y="${y}" style="display:none">${obs}</text>`
  }).join('')
  const meta=report.metadata
  const colored=Boolean(meta&&(meta.kind==='numeric'||(!meta.too_many_categories&&meta.kind==='categorical')))
  const clusterLegend=(report.membership||[]).map(row=>{
    const members=(report.coverage||[]).filter(p=>p.cluster===row.cluster)
    const havePatientIds=members.length===row.observations&&members.every(p=>String(p.patient||'').trim())
    const patientCount=havePatientIds?new Set(members.map(p=>String(p.patient).trim())).size:null
    return `<span>${colored?'':`<i style="background:${color[(row.cluster-1)%color.length]}"></i>`}Cluster ${escape(row.cluster)}: ${number(row.observations)} observations${patientCount===null?'':` · ${number(patientCount)} distinct patients`}</span>`
  }).join('')
  const context=colored?(meta.kind==='numeric'?`<span>Colour: ${escape(report.column)} (numeric, ${escape(format(meta.range?.[0],2))} to ${escape(format(meta.range?.[1],2))}); grey = missing</span>`:
    `<span>Colour: ${escape(report.column)}</span>${meta.categories.map((r,i)=>`<span><i style="background:${color[i%color.length]}"></i>${escape(r.value)} (${number(r.n)})</span>`).join('')}<span>Grey: missing</span>`):`<span>Colour: descriptive joint clusters</span>`
  const choices=pts.map((p,i)=>`<option value="${i}">${escape(p.observation)} · cluster ${escape(p.cluster)}</option>`).join('')
  const xMean=pts.reduce((sum,p)=>sum+p.x,0)/pts.length,yMean=pts.reduce((sum,p)=>sum+p.y,0)/pts.length
  const farthest=pts.map((p,i)=>({p,i,score:(p.x-xMean)**2+(p.y-yMean)**2})).sort((a,b)=>b.score-a.score||a.i-b.i).slice(0,5)
  const farthestTable=farthest.map(({p,i})=>`<tr><td>${escape(p.observation)}</td><td>${escape(p.cluster)}</td><td>${escape(format(p.x,3))}</td><td>${escape(format(p.y,3))}</td><td><button type="button" class="secondary-button" data-scatter-pick="${i}">Inspect</button></td></tr>`).join('')
  return `<div class="bio-scatter integrative-interactive-scatter" data-integrative-scatter="true" data-plot-nav-root="true">
    ${plotNavigationToolbar()}
    <svg viewBox="0 0 ${width} ${height}" role="img" aria-label="Joint principal coordinates plot with sample IDs on zoom"><defs><clipPath id="integrative-scatter-clip"><rect x="${pad}" y="${pad}" width="${width-2*pad}" height="${height-2*pad}" /></clipPath></defs>
      <rect x="${pad}" y="${pad}" width="${width-2*pad}" height="${height-2*pad}" fill="#fff" stroke="#e3e8f1"/>
      <g data-scatter-clip="true" clip-path="url(#integrative-scatter-clip)"><g data-scatter-points="true">${circles}</g></g>
      <text x="${width/2}" y="${height-9}" text-anchor="middle">Joint PCo1 (${percent(axes[0])} of positive inertia)</text><text x="15" y="${height/2}" transform="rotate(-90 15 ${height/2})" text-anchor="middle">Joint PCo2 (${percent(axes[1])} of positive inertia)</text>
    </svg><div class="bio-legend integrative-annotation-legend">${context}</div><div class="bio-legend integrative-cluster-legend">${clusterLegend}</div>
    <div class="integrative-sample-inspection"><label>Inspect one sample <select data-scatter-inspector="true"><option value="">Select a sample</option>${choices}</select></label><p data-scatter-inspection="true" class="method-note" role="status">Select or click a sample to view its ID, patient, visit and joint cluster.</p><div class="integrative-sample-metadata" data-scatter-metadata="true" hidden></div>
      <details class="integrative-distant-observations"><summary>Inspect distant plotted observations</summary><p class="method-note">These observations are farthest from the centre of the two-dimensional PCoA plot. This is not an outlier test, a full-distance ranking, or a quality-control failure.</p>
        <div class="table-scroll"><table><thead><tr><th>Observation</th><th>Cluster</th><th>PCo1</th><th>PCo2</th><th>Inspect</th></tr></thead><tbody>${farthestTable}</tbody></table></div></details>
    </div>
  </div>`
}

export function bindIntegrativeScatter(root,onInspect=null) {
  const wrap=root.querySelector('[data-integrative-scatter]')
  if(!wrap)return
  const svg=wrap.querySelector('svg'),grp=wrap.querySelector('[data-scatter-points]')
  if(!svg||!grp)return
  const points=[...wrap.querySelectorAll('[data-integrative-point]')]
  const labels=[...wrap.querySelectorAll('[data-integrative-label]')]
  const inspector=wrap.querySelector('[data-scatter-inspector]'),status=wrap.querySelector('[data-scatter-inspection]')
  const pad=49,w=650,h=365,plotW=w-2*pad,plotH=h-2*pad
  let zoom=1,panX=0,panY=0
  const clamp=(v,a,b)=>Math.max(a,Math.min(b,v))
  function update(){
    const halfX=(zoom-1)*plotW/2,halfY=(zoom-1)*plotH/2
    panX=clamp(panX,-halfX,halfX);panY=clamp(panY,-halfY,halfY)
    const tx=(pad+plotW/2)*(1-zoom)+panX,ty=(pad+plotH/2)*(1-zoom)+panY
    grp.setAttribute('transform',`translate(${tx.toFixed(3)} ${ty.toFixed(3)}) scale(${zoom.toFixed(3)})`)
    const occupied=[]
    // Spatial buckets keep dense overlapping samples unlabelled without
    // quadratic all-pairs checks on large cohorts.
    const neighbors=new Map(),bucketW=27,bucketH=17
    const coords=points.map((point,i)=>({
      i,x:Number(point.getAttribute('cx'))*zoom+tx,
      y:Number(point.getAttribute('cy'))*zoom+ty,
    }))
    for(const entry of coords){
      const key=`${Math.floor(entry.x/bucketW)},${Math.floor(entry.y/bucketH)}`
      if(!neighbors.has(key))neighbors.set(key,[])
      neighbors.get(key).push(entry)
    }
    labels.forEach((label,i)=>{
      label.style.display='none'
      if(zoom<2) return
      const p=points[i],{x,y}=coords[i]
      const cx=Math.floor(x/bucketW),cy=Math.floor(y/bucketH)
      let crowded=false
      for(let a=-1;a<=1&&!crowded;a++)for(let b=-1;b<=1&&!crowded;b++){
        for(const q of neighbors.get(`${cx+a},${cy+b}`)||[]){
          if(q.i!==i&&Math.abs(q.x-x)<bucketW&&Math.abs(q.y-y)<bucketH){crowded=true;break}
        }
      }
      if(crowded)return
      const name=p.dataset.observation||''
      const len=Math.min(170,Math.max(22,name.length*6.3)),left=x+8,top=y-15
      if(x<pad||x>w-pad||y<pad||y>h-pad||left+len>w-pad)return
      if(occupied.some(r=>left<r.right+4&&left+len+4>r.left&&top<r.bottom+3&&top+13>r.top-3))return
      occupied.push({left,right:left+len,top,bottom:top+13})
      label.setAttribute('x',(Number(p.getAttribute('cx'))+8/zoom).toFixed(3))
      label.setAttribute('y',(Number(p.getAttribute('cy'))-6/zoom).toFixed(3))
      label.style.fontSize=`${(11/zoom).toFixed(3)}px`
      label.setAttribute('stroke-width',(2.5/zoom).toFixed(3))
      label.style.display='inline'
    })
  }
  const reset=()=>{zoom=1;panX=0;panY=0;update()}
  function zoomAt(factor,ax=.5,ay=.5){
    const next=clamp(zoom*factor,1,PLOT_ZOOM_MAX)
    const actual=next/zoom
    panX=panX*actual+(pad+ax*plotW-(pad+plotW/2))*(1-actual)
    panY=panY*actual+(pad+ay*plotH-(pad+plotH/2))*(1-actual)
    zoom=next;update()
  }
  function boxAt(b){
    const x=(b.x0+b.x1)/2,y=(b.y0+b.y1)/2
    const current=zoom
    zoomAt(1/Math.max(b.x1-b.x0,b.y1-b.y0),x,y)
    if(zoom===current)return
    panX+=(pad+plotW/2)-(pad+x*plotW)
    panY+=(pad+plotH/2)-(pad+y*plotH)
    update()
  }
  const navigation=bindPlotNavigation(svg,{
    plot:{left:pad,right:w-pad,top:pad,bottom:h-pad},
    zoom:(factor,x,y)=>zoomAt(1/factor,x,y),reset,box:boxAt,
    pan:(dx,dy)=>{const rect=svg.getBoundingClientRect();panX+=dx*w/Math.max(1,rect.width);panY+=dy*h/Math.max(1,rect.height);update()}
  })
  bindPlotToolbar(wrap,navigation)
  wrap.querySelectorAll('[data-scatter-pick]').forEach(button=>button.addEventListener('click',()=>{if(inspector){inspector.value=button.dataset.scatterPick;inspector.dispatchEvent(new Event('change'))}}))
  const selectObservation = id => {
    const index=points.findIndex(point=>point.dataset.observation===id)
    if(index<0||!inspector)return
    inspector.value=String(index)
    inspector.dispatchEvent(new Event('change'))
    inspector.scrollIntoView?.({block:'nearest'})
  }
  points.forEach(point=>{
    point.addEventListener('click',()=>selectObservation(point.dataset.observation))
    point.addEventListener('keydown',event=>{if(event.key==='Enter'||event.key===' '){event.preventDefault();selectObservation(point.dataset.observation)}})
  })
  root.querySelectorAll('[data-integrative-inspect-sample]').forEach(button=>button.addEventListener('click',()=>selectObservation(button.dataset.integrativeInspectSample)))
  inspector?.addEventListener('change',()=>{
    const index=Number(inspector.value),valid=inspector.value!==''&&Number.isInteger(index)&&index>=0&&index<points.length
    points.forEach((p,i)=>{p.setAttribute('r',valid&&i===index?'7':'4.2');p.setAttribute('stroke',valid&&i===index?'#111827':'#fff');p.setAttribute('stroke-width',valid&&i===index?'2':'0.65')})
    if(!valid){status.textContent='Select a sample to view its ID and cluster.';return}
    const title=points[index].querySelector('title')
    status.textContent=title?.textContent||points[index].dataset.observation||''
    if(typeof onInspect==='function')onInspect(points[index].dataset.observation)
  })
  update()
}
