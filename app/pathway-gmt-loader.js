/** Browser-local GMT and ZIP reader. Network is accessed only on a user's download click. */
import {REACTOME_GMT_URL} from './pathway-enrichment.js?v=0.22.8'
import {inflateRawPortable} from './pathway-deflate.js?v=0.22.8'

const ZIP_LOCAL=0x04034b50,ZIP_CENTRAL=0x02014b50,ZIP_END=0x06054b50
const decodeUtf8=bytes=>new TextDecoder('utf-8',{fatal:true}).decode(bytes)
function crc32(bytes) {
  let crc=0xffffffff
  for(const value of bytes){crc^=value;for(let i=0;i<8;i++)crc=(crc>>>1)^((crc&1)?0xedb88320:0)}
  return (crc^0xffffffff)>>>0
}
/** Read a GMT entry from a standard single-volume ZIP (stored or DEFLATE). */
export async function unpackGmtZip(input) {
  const bytes=input instanceof Uint8Array?input:new Uint8Array(input)
  const view=new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength)
  const u16=i=>{if(i<0||i+2>bytes.length)throw new Error('Truncated ZIP file');return view.getUint16(i,true)}
  const u32=i=>{if(i<0||i+4>bytes.length)throw new Error('Truncated ZIP file');return view.getUint32(i,true)}
  if(bytes.length<22)throw new Error('Not a valid ZIP archive')
  let end=-1
  for(let i=bytes.length-22;i>=Math.max(0,bytes.length-22-65535);i--){
    if(u32(i)===ZIP_END && i+22+u16(i+20)===bytes.length){end=i;break}
  }
  if(end<0)throw new Error('ZIP central directory not found')
  if(u16(end+4)!==0||u16(end+6)!==0||u16(end+8)!==u16(end+10))throw new Error('Multi-volume ZIP archives are not supported')
  const entries=u16(end+10),size=u32(end+12),offset=u32(end+16)
  if(entries===0xffff||size===0xffffffff||offset===0xffffffff)throw new Error('ZIP64 archives are not supported; select a GMT instead')
  if(offset+size>end)throw new Error('Invalid ZIP central directory length')
  let pos=offset
  const matches=[]
  for(let i=0;i<entries;i++){
    if(u32(pos)!==ZIP_CENTRAL)throw new Error('Invalid ZIP directory entry')
    const flags=u16(pos+8),method=u16(pos+10),checksum=u32(pos+16)
    const compressedSize=u32(pos+20),originalSize=u32(pos+24)
    const nameLength=u16(pos+28),extraLength=u16(pos+30),commentLength=u16(pos+32)
    const localOffset=u32(pos+42),next=pos+46+nameLength+extraLength+commentLength
    if(next>offset+size)throw new Error('Truncated ZIP directory')
    const name=decodeUtf8(bytes.subarray(pos+46,pos+46+nameLength))
    if(/\.gmt$/i.test(name)&&!name.endsWith('/')){
      if(flags&1)throw new Error('Encrypted ZIP files are not supported')
      if(originalSize===0xffffffff||compressedSize===0xffffffff)throw new Error('ZIP64 gene sets are not supported')
      if(![0,8].includes(method))throw new Error('Unsupported ZIP compression; use a plain GMT')
      if(u32(localOffset)!==ZIP_LOCAL)throw new Error('Invalid ZIP local entry')
      const dataStart=localOffset+30+u16(localOffset+26)+u16(localOffset+28)
      if(dataStart+compressedSize>bytes.length)throw new Error('Truncated GMT data')
      matches.push({name,method,checksum,originalSize,compressed:bytes.subarray(dataStart,dataStart+compressedSize)})
    }
    pos=next
  }
  if(pos!==offset+size)throw new Error('Invalid ZIP entry boundaries')
  if(!matches.length)throw new Error('The ZIP archive contains no GMT file')
  let selected=matches.find(row=>/(^|\/)ReactomePathways\.gmt$/i.test(row.name))
  if(!selected){if(matches.length!==1)throw new Error('The ZIP contains several GMT files. Extract and select one GMT');selected=matches[0]}
  let plain
  if(selected.method===0)plain=selected.compressed
  else{
    // Node 18 and some browsers expose DecompressionStream but reject 'deflate-raw'.
    // A portable RFC 1951 decoder is used when the format is not supported.
    let decoder=null
    if(typeof DecompressionStream==='function'){
      try{decoder=new DecompressionStream('deflate-raw')}catch{decoder=null}
    }
    if(decoder){
      const stream=new Blob([selected.compressed]).stream().pipeThrough(decoder)
      plain=new Uint8Array(await new Response(stream).arrayBuffer())
    }else plain=inflateRawPortable(selected.compressed,selected.originalSize)
  }
  if(plain.length!==selected.originalSize||crc32(plain)!==selected.checksum)throw new Error('GMT ZIP integrity check failed')
  return {text:decodeUtf8(plain),filename:selected.name.split('/').pop()}
}
/** Custom GMT or standard GMT ZIP. No file content leaves the browser. */
export async function readGmtSelection(file){
  if(!file)throw new Error('Select a GMT or ZIP file')
  if(/\.zip$/i.test(file.name))return {...await unpackGmtZip(await file.arrayBuffer()),archiveName:file.name}
  if(!/\.(gmt|txt)$/i.test(file.name))throw new Error('Choose a .gmt, .txt, or .zip gene-set file')
  return {text:await file.text(),filename:file.name,archiveName:null}
}
/** Download and unpack official Reactome current. Cross-origin access requires CORS. */
export async function fetchDefaultReactomeGmt(fetcher=fetch){
  let response
  try{response=await fetcher(REACTOME_GMT_URL,{method:'GET',mode:'cors',credentials:'omit',cache:'no-store'})}
  catch{throw new Error('The browser could not read the Reactome download (network or CORS). Use the manual ZIP download link and import that ZIP')}
  if(!response?.ok)throw new Error(`Reactome download failed (HTTP ${response?.status??'unknown'}). Use the manual ZIP download link`)
  const entry=await unpackGmtZip(await response.arrayBuffer())
  return {...entry,archiveName:'ReactomePathways.gmt.zip',retrievedAt:new Date().toISOString(),url:REACTOME_GMT_URL}
}
