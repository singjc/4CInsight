import test from 'node:test'
import assert from 'node:assert/strict'
import {deflateRawSync} from 'node:zlib'
import {unpackGmtZip,readGmtSelection,fetchDefaultReactomeGmt} from '../pathway-gmt-loader.js'

const gmt='IMMUNE_SIGNAL\tReactome Pathway\tIGJ\tMZB1\tLCK\nT_CELL\tReactome Pathway\tCD3D\tCD3E\tLCK\n'
function crc32(bytes){let crc=0xffffffff;for(const b of bytes){crc^=b;for(let j=0;j<8;j++)crc=(crc>>>1)^((crc&1)?0xedb88320:0)}return (crc^0xffffffff)>>>0}
function zip(entries){
  const parts=[],central=[];let offset=0
  for(const entry of entries){
    const name=Buffer.from(entry.name),content=Buffer.from(entry.text),deflated=entry.method===8?deflateRawSync(content):content,checksum=crc32(content)
    const local=Buffer.alloc(30);local.writeUInt32LE(0x04034b50,0);local.writeUInt16LE(20,4);local.writeUInt16LE(entry.method,8);local.writeUInt32LE(checksum,14);local.writeUInt32LE(deflated.length,18);local.writeUInt32LE(content.length,22);local.writeUInt16LE(name.length,26)
    const row=Buffer.alloc(46);row.writeUInt32LE(0x02014b50,0);row.writeUInt16LE(20,4);row.writeUInt16LE(20,6);row.writeUInt16LE(entry.method,10);row.writeUInt32LE(checksum,16);row.writeUInt32LE(deflated.length,20);row.writeUInt32LE(content.length,24);row.writeUInt16LE(name.length,28);row.writeUInt32LE(offset,42)
    parts.push(local,name,deflated);central.push(row,name);offset+=local.length+name.length+deflated.length
  }
  const cd=Buffer.concat(central),end=Buffer.alloc(22);end.writeUInt32LE(0x06054b50,0);end.writeUInt16LE(entries.length,8);end.writeUInt16LE(entries.length,10);end.writeUInt32LE(cd.length,12);end.writeUInt32LE(offset,16)
  return Buffer.concat([...parts,cd,end])
}
test('unpack a deflated Reactome-style GMT and preserve gene text',async()=>{
  const result=await unpackGmtZip(zip([{name:'ReactomePathways.gmt',text:gmt,method:8}]))
  assert.equal(result.filename,'ReactomePathways.gmt');assert.equal(result.text,gmt)
})
test('support stored ZIP and ignore unrelated metadata entries',async()=>{
  const result=await unpackGmtZip(zip([{name:'notes.txt',text:'metadata',method:0},{name:'folder/custom.gmt',text:gmt,method:0}]))
  assert.equal(result.filename,'custom.gmt');assert.equal(result.text,gmt)
})
test('prefer ReactomePathways.gmt when a ZIP has multiple GMT entries',async()=>{
  const result=await unpackGmtZip(zip([{name:'other.gmt',text:'X\tY\tZ\n',method:0},{name:'ReactomePathways.gmt',text:gmt,method:8}]))
  assert.equal(result.text,gmt)
})
test('reject ambiguous custom ZIP with more than one gene-set file',async()=>{
  await assert.rejects(unpackGmtZip(zip([{name:'a.gmt',text:gmt,method:0},{name:'b.gmt',text:gmt,method:0}])),/several GMT/)
})
test('reject malformed and corrupted ZIP',async()=>{
  await assert.rejects(unpackGmtZip(new Uint8Array([1,2,3])),/ZIP/)
  const corrupted=zip([{name:'x.gmt',text:gmt,method:0}]);corrupted[35]^=0xff
  await assert.rejects(unpackGmtZip(corrupted),/integrity/)
})
test('accept uploaded ZIP or plain GMT with no network calls',async()=>{
  const compressed=zip([{name:'user.gmt',text:gmt,method:8}]);let used=0
  const file={name:'custom.zip',arrayBuffer:async()=>{used++;return compressed},text:async()=>{throw new Error('Do not use text() on ZIP')}}
  const fromZip=await readGmtSelection(file);assert.equal(fromZip.filename,'user.gmt');assert.equal(used,1)
  const fromPlain=await readGmtSelection({name:'custom.gmt',text:async()=>gmt});assert.equal(fromPlain.text,gmt)
  await assert.rejects(readGmtSelection({name:'x.csv',text:async()=>gmt}),/Choose a/)
})
test('default fetch sends no gene data and returns retrieval provenance',async()=>{
  const compressed=zip([{name:'ReactomePathways.gmt',text:gmt,method:8}]);let called=0
  const data=await fetchDefaultReactomeGmt(async(url,options)=>{
    called++;assert.match(url,/reactome\.org\/download\/current\/ReactomePathways\.gmt\.zip/)
    assert.equal(options.method,'GET');assert.equal(options.mode,'cors');assert.equal(options.credentials,'omit')
    assert.equal(options.body,undefined)
    return {ok:true,arrayBuffer:async()=>compressed}
  })
  assert.equal(called,1);assert.equal(data.text,gmt);assert.ok(data.retrievedAt)
})
test('show a manual ZIP fallback when the browser blocks CORS',async()=>{
  await assert.rejects(fetchDefaultReactomeGmt(async()=>{throw new TypeError('Failed to fetch')}),/manual ZIP download/)
})

test('Node 18-compatible fallback when DecompressionStream rejects deflate-raw',async()=>{
  const original=globalThis.DecompressionStream
  globalThis.DecompressionStream=class {constructor(format){throw new TypeError(`The argument 'format' is invalid. Received '${format}'`)}}
  try {
    const result=await unpackGmtZip(zip([{name:'ReactomePathways.gmt',text:gmt.repeat(80),method:8}]))
    assert.equal(result.text,gmt.repeat(80))
  } finally {globalThis.DecompressionStream=original}
})
test('portable fallback when DecompressionStream is missing, with stored, fixed and dynamic Huffman blocks',async()=>{
  const original=globalThis.DecompressionStream
  globalThis.DecompressionStream=undefined
  try {
    const result=await unpackGmtZip(zip([{name:'custom.gmt',text:gmt.repeat(1000),method:8}]))
    assert.equal(result.text,gmt.repeat(1000))
    const {inflateRawPortable}=await import('../pathway-deflate.js')
    const {deflateRawSync}=await import('node:zlib')
    for (const content of [gmt,gmt.repeat(1000),'A'.repeat(25000)+'\n'+gmt,'ABCDEFGHIJK'.repeat(5000)]) {
      const encoded=new TextEncoder().encode(content)
      for(const options of [{level:0},{strategy:4},{level:6},{level:9}]) {
        const zipped=deflateRawSync(encoded,options)
        const decoded=inflateRawPortable(zipped,encoded.length)
        assert.deepEqual(decoded,encoded)
      }
    }
  } finally {globalThis.DecompressionStream=original}
})
test('portable raw DEFLATE rejects output beyond original ZIP length and invalid stream',async()=>{
  const {inflateRawPortable}=await import('../pathway-deflate.js')
  const raw=deflateRawSync(Buffer.from(gmt))
  assert.throws(()=>inflateRawPortable(raw,gmt.length-1),/output|length/i)
  assert.throws(()=>inflateRawPortable(new Uint8Array([7]),gmt.length),/Reserved DEFLATE block type/)
})
