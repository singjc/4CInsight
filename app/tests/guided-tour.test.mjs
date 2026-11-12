import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
import {dirname,join} from 'node:path';
import {TOUR_STEPS,tourStepsForTab,tourStepLabel,tourPosition} from '../guided-tour.js';

const root=dirname(dirname(fileURLToPath(import.meta.url)));
const read=p=>readFileSync(join(root,p),'utf8');

test('guided walkthrough covers the active scientific and project workflows',()=>{
  const tabs=new Set(TOUR_STEPS.map(s=>s.tab));
  for(const key of ['project','metadata','qc','compare','results','longitudinal','hits','history','docs','settings'])assert.ok(tabs.has(key),key);
  for(const view of ['overview','details','pathways'])assert.ok(TOUR_STEPS.some(s=>s.tab==='hits'&&s.view===view),view);
  assert.equal(TOUR_STEPS[0].id,'welcome');
  assert.equal(new Set(TOUR_STEPS.map(s=>s.id)).size,TOUR_STEPS.length);
});

test('tour this tab is restricted to the current workspace and prioritizes active subview',()=>{
  assert.deepEqual(tourStepsForTab('compare').map(s=>s.id),['compare']);
  const pathways=tourStepsForTab('hits','pathways');
  assert.equal(pathways[0].id,'pathways');
  assert.ok(pathways.every(s=>s.tab==='hits'));
  assert.equal(tourStepLabel(2,14),'Step 3 of 14');
});

test('popover position remains inside viewport near the spotlight',()=>{
  for(const rect of [{left:0,top:0,bottom:28,width:90},{left:100,top:690,bottom:720,width:80},{left:1000,top:400,bottom:430,width:70}]){
    const p=tourPosition(rect,{width:1024,height:768});
    assert.ok(p.left>=12 && p.left+p.width<=1024-12,p.left);
    assert.ok(p.top>=12 && p.top+205<=768-12,p.top);
  }
});

test('read-only launchers, render hook, and tour-only state navigation are present',()=>{
  const main=read('main.js'),html=read('index.html'),css=read('guided-tour.css');
  assert.match(main,/import \{ createGuidedTour \} from '\.\/guided-tour\.js\?v=0\.22\.8'/);
  assert.match(main,/guidedTourController\?\.refresh\(\)/);
  assert.match(main,/createGuidedTour\(\{/);
  assert.match(main,/if \(!guidedTourController\?\.active\) scheduleLocalSessionSave\(\)/);
  assert.match(html,/id="guided-tour-start"/);
  assert.match(html,/id="guided-tour-current"/);
  assert.match(html,/guided-tour\.css\?v=0\.22\.8/);
  assert.match(css,/\.tour-highlight/);
  assert.doesNotMatch(main,/aiAssistantView|mountAiAssistant/);
});

test('tour never requests scientific computation, raw files, APIs, or forced auto-launch',()=>{
  const source=read('guided-tour.js');
  assert.doesNotMatch(source,/\bfetch\s*\(|workerRequest|runSignificantHits|runJointStructure|loadApplicationState|clearSavedActiveSession|saveApplicationState|localStorage|sessionStorage/);
  assert.doesNotMatch(source,/addEventListener\(['"]load['"]|DOMContentLoaded/);
  assert.match(source,/e\.key==='Escape'/);
  assert.match(source,/aria-modal/);
  assert.match(source,/close\(restore=true\)/);
  assert.match(source,/priorFocus\.focus\(\)/);
});
