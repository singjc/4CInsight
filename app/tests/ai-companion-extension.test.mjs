import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {join} from 'node:path';
import {fileURLToPath} from 'node:url';
import vm from 'node:vm';
const ROOT=join(fileURLToPath(new URL('../../',import.meta.url)),'tools/ai-companion/extension-firefox');
const read=f=>readFileSync(join(ROOT,f),'utf8');

test('manifest has fixed extension identity and no broad host permissions',()=>{
  const manifest=JSON.parse(read('manifest.json'));
  assert.equal(manifest.browser_specific_settings.gecko.id,'jcode-companion@4cinsight.org');
  assert.ok(manifest.permissions.includes('nativeMessaging'));
  assert.ok(manifest.content_scripts[0].all_frames===false);
  assert.ok(!manifest.permissions.includes('<all_urls>'));
  assert.ok(!manifest.permissions.includes('tabs'));
});
test('extension refuses foreign page and cannot launch arbitrary native host action',async()=>{
  let listener,call;
  const browser={runtime:{onMessage:{addListener:f=>listener=f},sendNativeMessage:async(...args)=>{call=args;return {ok:true,result:{installed:true}}}}};
  vm.runInNewContext(read('background.js'),{browser,URL,Promise,Set});
  assert.equal((await listener({op:'status',payload:{}},{tab:{url:'https://evil.example/4CInsight/'}})).ok,false);
  assert.equal((await listener({op:'shell',payload:{}},{tab:{url:'https://singjc.github.io/4CInsight/'}})).ok,false);
  const r=await listener({op:'status',payload:{}},{tab:{url:'https://singjc.github.io/4CInsight/'}});
  assert.equal(r.ok,true);
  assert.equal(call[0],'org.fourcinsight.jcode');
  assert.equal(call[1].op,'status');
});
test('page relay accepts exact trusted origin and rejects foreign and arbitrary actions',async()=>{
  let handler,request,posted;
  const win={top:null,addEventListener:(_type,f)=>handler=f,postMessage:(m,origin)=>{posted={m,origin}}};win.top=win;
  const location={origin:'https://singjc.github.io',pathname:'/4CInsight/'};
  const browser={runtime:{sendMessage:async m=>{request=m;return {ok:true,result:{installed:true}}}}};
  vm.runInNewContext(read('content.js'),{window:win,location,browser,Promise});
  assert.equal(typeof handler,'function');
  handler({source:win,origin:'https://evil.example',data:{channel:'4c-insight-ai-v2',direction:'request',id:'a2-1-x',op:'status'}});
  assert.equal(request,undefined);
  handler({source:win,origin:location.origin,data:{channel:'4c-insight-ai-v2',direction:'request',id:'a2-1-x',op:'launch_shell'}});
  assert.equal(request,undefined);
  handler({source:win,origin:location.origin,data:{channel:'4c-insight-ai-v2',direction:'request',id:'a2-1-x',op:'status',payload:{}}});
  await new Promise(r=>setImmediate(r));
  assert.equal(request.op,'status');assert.equal(posted.origin,location.origin);
  assert.equal(posted.m.ok,true);
});
