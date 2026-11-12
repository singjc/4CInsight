/** 4C Insight guided walkthrough. UI-only: does not read source files or run analysis. */
export const TOUR_STEPS = Object.freeze([
  {id:'welcome',tab:'project',target:'[data-tab="project"]',title:'Welcome to 4C Insight',body:'This workflow runs in your browser. The tour shows the main pages without using or changing your analysis data.'},
  {id:'files',tab:'project',target:'#input-drop-zone',title:'1. Add your project files',body:'Select or drop the local workbooks. 4C Insight identifies the data type from the file name and contents. Files are not uploaded to a server.'},
  {id:'metadata',tab:'metadata',target:'[data-tab="metadata"]',title:'2. Check sample metadata',body:'Create or inspect sample metadata. Match input files to observations and check sample and patient identifiers before a comparison.'},
  {id:'qc',tab:'qc',target:'[data-tab="qc"]',title:'3. Explore and check data quality',body:'Review the measured data and sample structure. Check missing data and quality results before you interpret patterns.'},
  {id:'compare',tab:'compare',target:'[data-tab="compare"]',title:'4. Compare predefined groups',body:'Use metadata to define cases and controls. Run corrected comparisons and review effect sizes and adjusted p-values.'},
  {id:'results',tab:'results',target:'[data-tab="results"]',title:'5. Examine detailed results',body:'Start with the Overview, then open detailed plots and sortable tables. Use the plot controls to zoom, pan, select, and export.'},
  {id:'longitudinal',tab:'longitudinal',target:'[data-tab="longitudinal"]',title:'6. Review repeated observations',body:'Explore changes across visits and timepoints when suitable patient and visit metadata are available.'},
  {id:'joint',tab:'hits',view:'overview',target:'#integrative-view-overview',title:'7. Integrate the seven data types',body:'Run the browser-local joint analysis. Check how many observations are included and which data types contribute to sample patterns.'},
  {id:'details',tab:'hits',view:'details',target:'#integrative-view-details',title:'8. Inspect integrative details',body:'Review source coverage, omitted observations, clustering diagnostics, and feature associations. Clusters are descriptive, not validated clinical groups.'},
  {id:'pathways',tab:'hits',view:'pathways',target:'#integrative-view-pathways',title:'9. Explore pathways',body:'Choose a measured gene ranking or a predefined contrast, load gene sets, and inspect enrichment results. Network edges show shared genes, not physical interactions.'},
  {id:'history',tab:'history',target:'[data-tab="history"]',title:'10. Return to saved results',body:'History contains separate browser-local analysis snapshots. Your original input files stay on your computer.'},
  {id:'docs',tab:'docs',target:'[data-tab="docs"]',title:'11. Use the method guides',body:'Search the documentation for worked examples, analysis methods, interpretation limits, and references.'},
  {id:'settings',tab:'settings',target:'[data-tab="settings"]',title:'12. Review device settings',body:'See browser-reported resources and set display limits. Some browsers do not report available device memory.'},
  {id:'save',tab:'project',target:'#global-state-actions',title:'Save or restore your work',body:'The header has Save state, Load state, and Clear state. The portable state contains settings and file references, not original source bytes. Automatic browser-local session storage is separate.'},
]);

export const tourStepsForTab = (tab,view='overview') => {
  const matches=TOUR_STEPS.filter(step=>step.tab===tab && step.id!=='welcome' && step.id!=='save');
  if(tab==='hits') return matches.sort((a,b)=>Number(b.view===view)-Number(a.view===view));
  return matches.length?matches:[TOUR_STEPS[0]];
};
export const tourStepLabel = (index,total) => `Step ${index+1} of ${total}`;
export function tourPosition(target,viewport,{width=360,height=205,gap=14}={}) {
  const vw=Math.max(280,viewport.width),vh=Math.max(240,viewport.height),r=target;
  const w=Math.min(width,vw-24),x=Math.max(12,Math.min(vw-w-12,r.left+r.width/2-w/2));
  const below=r.bottom+gap,above=r.top-height-gap;
  const y=below+height<=vh-12?below:above>=12?above:Math.max(12,vh-height-12);
  return {left:Math.round(x),top:Math.round(y),width:Math.round(w)};
}

export function createGuidedTour({current,navigate,doc=document,win=window}) {
  const $=s=>doc.querySelector(s);
  const layers=Array.from({length:4},(_,i)=>{const el=doc.createElement('div');el.className='tour-shade';el.dataset.side=String(i);doc.body.append(el);return el;});
  const focusFrame=doc.createElement('div');focusFrame.className='tour-highlight';doc.body.append(focusFrame);
  const panel=doc.createElement('section');panel.className='guided-tour-panel';panel.setAttribute('role','dialog');panel.setAttribute('aria-modal','true');panel.setAttribute('aria-labelledby','tour-title');panel.setAttribute('aria-describedby','tour-body');
  panel.innerHTML=`<div class="guided-tour-heading"><span id="tour-progress" class="guided-tour-progress"></span><button id="tour-close" type="button" aria-label="Close guided tour">×</button></div><h2 id="tour-title"></h2><p id="tour-body"></p><div class="guided-tour-actions"><button type="button" id="tour-skip">Skip tour</button><div><button type="button" id="tour-back">Back</button><button type="button" id="tour-next" class="guided-tour-primary">Next</button></div></div>`;
  doc.body.append(panel);
  let steps=[],index=0,active=false,origin=null,priorFocus=null,frame=0,scrollStep='';
  const hide=()=>{layers.forEach(e=>e.hidden=true);focusFrame.hidden=true;panel.hidden=true;};
  hide();
  function locate(step){return $(step.target)||$(`[data-tab="${step.tab}"]`)||$('#main-content .page-heading')||$('#main-content');}
  function draw(){
    if(!active)return;
    const step=steps[index],loc=current();
    if(loc.tab!==step.tab||(step.view&&loc.view!==step.view)){navigate(step);return;}
    const target=locate(step);if(!target)return;
    if(scrollStep!==`${index}:${step.id}`){scrollStep=`${index}:${step.id}`;target.scrollIntoView?.({block:'center',behavior:'auto'});}
    const r=target.getBoundingClientRect(),vw=win.innerWidth,vh=win.innerHeight,pad=6;
    const x=Math.max(6,Math.min(vw-6,r.left-pad)),y=Math.max(6,Math.min(vh-6,r.top-pad));
    const right=Math.max(x,Math.min(vw-6,r.right+pad)),bottom=Math.max(y,Math.min(vh-6,r.bottom+pad));
    const rectangles=[[0,0,vw,y],[0,y,x,bottom-y],[right,y,vw-right,bottom-y],[0,bottom,vw,vh-bottom]];
    layers.forEach((e,i)=>{const [a,b,w,h]=rectangles[i];Object.assign(e.style,{left:`${a}px`,top:`${b}px`,width:`${Math.max(0,w)}px`,height:`${Math.max(0,h)}px`});e.hidden=false;});
    Object.assign(focusFrame.style,{left:`${x}px`,top:`${y}px`,width:`${right-x}px`,height:`${bottom-y}px`});focusFrame.hidden=false;
    $('#tour-title').textContent=step.title;$('#tour-body').textContent=step.body;$('#tour-progress').textContent=tourStepLabel(index,steps.length);
    $('#tour-back').disabled=index===0;$('#tour-next').textContent=index===steps.length-1?'Finish':'Next';
    panel.hidden=false;
    if(!panel.contains(doc.activeElement))panel.querySelector('#tour-next').focus();
    const panelSize=panel.getBoundingClientRect(),pos=tourPosition({left:x,top:y,bottom,width:right-x,height:bottom-y},{width:vw,height:vh},{width:360,height:Math.max(192,panelSize.height)});
    panel.style.left=`${pos.left}px`;panel.style.top=`${pos.top}px`;panel.style.width=`${pos.width}px`;
  }
  function refresh(){if(!active)return;win.cancelAnimationFrame?.(frame);frame=win.requestAnimationFrame(draw);}
  function start(mode='all'){
    if(active)close(false);
    origin={...current()};priorFocus=doc.activeElement;
    steps=mode==='tab'?tourStepsForTab(origin.tab,origin.view):TOUR_STEPS;
    index=0;active=true;scrollStep='';refresh();
  }
  function close(restore=true){
    if(!active)return;active=false;win.cancelAnimationFrame?.(frame);hide();
    if(restore&&origin&&JSON.stringify(current())!==JSON.stringify(origin))navigate(origin);
    if(priorFocus?.isConnected)priorFocus.focus();origin=null;
  }
  function move(amount){if(!active)return;const n=index+amount;if(n<0)return;if(n>=steps.length){close();return;}index=n;refresh();}
  panel.querySelector('#tour-next').addEventListener('click',()=>move(1));
  panel.querySelector('#tour-back').addEventListener('click',()=>move(-1));
  panel.querySelector('#tour-skip').addEventListener('click',()=>close());
  panel.querySelector('#tour-close').addEventListener('click',()=>close());
  $('#guided-tour-start')?.addEventListener('click',()=>start('all'));
  $('#guided-tour-current')?.addEventListener('click',()=>start('tab'));
  const onKey=e=>{
    if(!active)return;
    if(e.key==='Escape'){e.preventDefault();close();}
    else if(e.key==='ArrowRight'){e.preventDefault();move(1);}
    else if(e.key==='ArrowLeft'){e.preventDefault();move(-1);}
    else if(e.key==='Tab'){
      const focusable=[...panel.querySelectorAll('button:not([disabled])')];const n=focusable.indexOf(doc.activeElement);
      if(e.shiftKey&&n<=0){e.preventDefault();focusable[focusable.length-1].focus();}
      else if(!e.shiftKey&&(n===focusable.length-1||n<0)){e.preventDefault();focusable[0].focus();}
    }
  };
  doc.addEventListener('keydown',onKey,true);
  win.addEventListener('resize',refresh);win.addEventListener('scroll',refresh,true);
  return {start,close,refresh,get active(){return active;}};
}
