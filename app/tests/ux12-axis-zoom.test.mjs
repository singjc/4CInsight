import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { renderResultsOverview } from '../results-overview.js'
import { axisZoomWindow, axisPanWindow, axisProjection } from '../results-interactions.js'

const result={
 gene:{analysis:{tests:[{feature:'GENE1',beta_group:-2.4,q_value:.002},{feature:'GENE2',beta_group:1.6,q_value:.04}]}},
 transcript:{analysis:{tests:[{feature:'TRANSCRIPT1',beta_group:.8,q_value:.03}]}},
 bcr:{analysis:{sample_summaries:[{sample_id:'BCR-S1',group:'Case',rarefied_richness:110,clonality:.3,public_fraction:.08},{sample_id:'BCR-S2',group:'Control',rarefied_richness:900,clonality:.8,public_fraction:.6}]}},
 tcrAlpha:{analysis:{sample_summaries:[{sample_id:'TA-S1',group:'Case',clonality:.2},{sample_id:'TA-S2',group:'Control',clonality:.7}]}},
 tcrBeta:{analysis:{sample_summaries:[{sample_id:'TB-S1',group:'Case',clonality:.2},{sample_id:'TB-S2',group:'Control',clonality:.7}]}},
 isotype:{analysis:{tests:[{feature:'IgM',beta_group:-.5,q_value:.05},{feature:'IgA',beta_group:.2,q_value:.01}]}},
 immune:{analysis:{tests:[{feature:'NK cells',beta_group:2,q_value:.001}]}}
}

test('UX12: overview includes data-space points, stable axes and label metadata in all seven modalities',()=>{
 const html=renderResultsOverview(result,{fdr:.05,absLog2Fc:0})
 assert.equal((html.match(/data-axis-kind="volcano"/g)||[]).length,2)
 assert.equal((html.match(/data-axis-kind="samples"/g)||[]).length,3)
 assert.equal((html.match(/data-axis-kind="effects"/g)||[]).length,2)
 for(const name of ['GENE1','TRANSCRIPT1','BCR-S1','TA-S1','TB-S1'])assert.match(html,new RegExp(`data-point-label="${name}"`))
 assert.match(html,/data-axis-ref-y=/)
 assert.match(html,/data-axis-median=/)
 assert.match(html,/data-axis-bar=/)
})
test('UX12: normalized axis zoom keeps original axis geometry and clamps to 16x',()=>{
 const base={x:0,y:0,width:1,height:1}
 const v=axisZoomWindow(base,.5,.7,.2,true)
 assert.deepEqual(v,{x:.35,y:.1,width:.5,height:.5})
 const one=axisZoomWindow(base,.5,.7,.2,false)
 assert.equal(one.y,0);assert.equal(one.height,1)
 let tiny=base;for(let i=0;i<20;i++)tiny=axisZoomWindow(tiny,.5)
 assert.equal(tiny.width,1/16)
 assert.deepEqual(axisZoomWindow(tiny,100),base)
})
test('UX12: data projection and pan alter data coordinates without changing the svg frame',()=>{
 const view={x:.25,y:.25,width:.5,height:.5}
 assert.equal(axisProjection(.5,view,48,642),345)
 const shifted=axisPanWindow(view,100,-40,600,200,true)
 assert.ok(shifted.x<view.x)
 assert.ok(shifted.y<view.y)
 assert.equal(axisPanWindow(view,100,-40,600,200,false).y,0)
})
test('UX12: only this source changes plot coordinates, not fitted result values or scientific engine',()=>{
 const source=readFileSync(new URL('../results-interactions.js',import.meta.url),'utf8')
 assert.match(source,/data-axis-annotations/)
 assert.match(source,/data-axis-current-ticks/)
 assert.match(source,/axisRedrawLabels/)
 assert.match(source,/state\.original\.width/)
 assert.match(source,/dataset.pointLabel/)
 assert.doesNotMatch(source,/\bfetch\s*\(|WebSocket|readAsArrayBuffer/)
})
