import test from 'node:test'
import assert from 'node:assert/strict'
import {
  completeWindow,validateHeatmapWindow,boxToWindow,heatmapDisplayScale,
  distanceFill,heatmapCellSize,clusteredSampleIds,heatmapViewerMarkup,rankTypicalJointDistances,jointDistanceReviewMarkup,
} from '../joint-heatmap-viewer.js'

const ids=['S0','S1','S2','S3','S4','S5']
const order=[5,1,4,0,3,2]
const n=ids.length
const distances=Array.from({length:n*n},(_,k)=>{
  const i=Math.floor(k/n),j=k%n
  return Math.abs(i-j)
})
const result={observations:ids,distances,clustering:{leaf_order:order,merges:[]}}

test('full window keeps every observation; no sample is omitted by the viewer',()=>{
  const w=completeWindow(n)
  assert.deepEqual(w,{rowStart:0,rowEnd:n,columnStart:0,columnEnd:n})
  assert.deepEqual(clusteredSampleIds(result,w).rows,order.map(i=>ids[i]))
  assert.deepEqual(clusteredSampleIds(result,w).columns,order.map(i=>ids[i]))
  assert.throws(()=>validateHeatmapWindow({...w,rowEnd:1},n),/at least two/)
})

test('drag selection maps to original clustering indices, allowing asymmetric cropped regions',()=>{
  const w=completeWindow(100)
  const selected=boxToWindow(w,{x0:200,x1:500,y0:550,y1:900},1000,1000,100)
  assert.deepEqual(selected,{columnStart:20,columnEnd:50,rowStart:55,rowEnd:90})
  const deeper=boxToWindow(selected,{x0:0,x1:500,y0:0,y1:1000},1000,1000,100)
  assert.equal(deeper.columnStart,20)
  assert.equal(deeper.columnEnd,35)
  assert.equal(deeper.rowEnd,90)
  assert.equal(deeper.rowStart,55)
  assert.equal(boxToWindow(w,{x0:2,x1:3,y0:1,y1:2},1000,1000,100),null)
})

test('drag direction, out-of-bounds positions and invalid selection are safe',()=>{
  assert.deepEqual(boxToWindow(completeWindow(20),{x0:101,x1:20,y0:80,y1:10},100,100,20),
    {columnStart:4,columnEnd:20,rowStart:2,rowEnd:16})
  assert.equal(boxToWindow(completeWindow(20),{x0:0,x1:0,y0:0,y1:100},100,100,20),null)
  assert.throws(()=>boxToWindow(completeWindow(20),{x0:NaN,x1:10,y0:10,y1:20},100,100,20),/Invalid/)
})

test('color units are median-normalized distances; white is zero, darker blue is farther',()=>{
  const scale=heatmapDisplayScale(distances,n,'max')
  assert.equal(scale.maximum,5)
  assert.equal(scale.cap,5)
  assert.match(scale.unit,/unitless/)
  assert.equal(distanceFill(0,scale,true),'#ffffff')
  assert.equal(distanceFill(0,scale),'rgb(253,253,252)')
  assert.equal(distanceFill(5,scale),'rgb(72,72,252)')
  assert.equal(distanceFill(10,scale),'rgb(72,72,252)')
  assert.equal(distanceFill(NaN,scale),'#e5e8ed')
})

test('95th percentile saturates high distances without changing original inputs',()=>{
  const d=Array.from({length:10*10},(_,k)=>{
    const i=Math.floor(k/10),j=k%10
    return i===j?0:((i===0||j===0)?100:1)
  })
  const old=[...d]
  const p=heatmapDisplayScale(d,10,'p95')
  const max=heatmapDisplayScale(d,10,'max')
  assert.ok(p.cap<=max.cap)
  assert.deepEqual(d,old)
  assert.throws(()=>heatmapDisplayScale([0,1],2),/complete square/)
  assert.throws(()=>heatmapDisplayScale([0,-1,-1,0],2),/nonnegative/)
})

test('zoom increases cell size and names are from the original linkage order',()=>{
  const wide=heatmapCellSize(completeWindow(217))
  const close=heatmapCellSize({rowStart:30,rowEnd:42,columnStart:10,columnEnd:24})
  assert.ok(close>wide)
  const crop=clusteredSampleIds(result,{rowStart:1,rowEnd:3,columnStart:3,columnEnd:6})
  assert.deepEqual(crop.rows,['S1','S4'])
  assert.deepEqual(crop.columns,['S0','S3','S2'])
})

test('full viewer has expandable control, two dendrograms, box zoom, clear distance legend and status',()=>{
  const html=heatmapViewerMarkup()
  for(const token of ['joint-heatmap-expand','joint-heatmap-back','joint-heatmap-reset',
    'joint-top-tree','joint-left-tree','joint-column-labels','joint-row-labels',
    'joint-heatmap-matrix','joint-heatmap-selection','joint-heatmap-scale','role="status"']) {
    assert.ok(html.includes(token),token)
  }
  assert.match(html,/unitless/)
  assert.match(html,/original clustering/)
  assert.doesNotMatch(html,/I2A|I2B|I2C/)
})


test('distance QC review ranks large typical distances without declaring outliers or injecting HTML',()=>{
  const review=rankTypicalJointDistances(result,2)
  assert.equal(review.length,2)
  assert.ok(review[0].median_distance>=review[1].median_distance)
  assert.equal(review[0].measured_pairs,5)
  const html=jointDistanceReviewMarkup({...result,observations:['<secret&>',...ids.slice(1)]})
  assert.match(html,/not an outlier test/)
  assert.match(html,/&lt;secret&amp;&gt;/)
  assert.doesNotMatch(html,/<secret&>/)
})
