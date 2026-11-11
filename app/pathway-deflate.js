/** Small, dependency-free RFC 1951 raw DEFLATE decoder for browser ZIP entries.
 * Used only when DecompressionStream('deflate-raw') is not supported.
 * It does not use Node modules, external services, or global data.
 * Output is bounded by the ZIP entry's declared uncompressed length.
 */
const LEN_BASE = [3,4,5,6,7,8,9,10,11,13,15,17,19,23,27,31,35,43,51,59,67,83,99,115,131,163,195,227,258]
const LEN_EXTRA= [0,0,0,0,0,0,0,0,1,1,1,1,2,2,2,2,3,3,3,3,4,4,4,4,5,5,5,5,0]
const DIST_BASE= [1,2,3,4,5,7,9,13,17,25,33,49,65,97,129,193,257,385,513,769,1025,1537,2049,3073,4097,6145,8193,12289,16385,24577]
const DIST_EXTRA=[0,0,0,0,1,1,2,2,3,3,4,4,5,5,6,6,7,7,8,8,9,9,10,10,11,11,12,12,13,13]
const ORDER=[16,17,18,0,8,7,9,6,10,5,11,4,12,3,13,2,14,1,15]

function huffman(lengths){
  const counts=new Uint16Array(16),next=new Uint16Array(16),table=new Uint16Array(1<<16)
  let max=0
  for(const len of lengths){if(!Number.isInteger(len)||len<0||len>15)throw new Error('Invalid DEFLATE Huffman length');if(len){counts[len]++;max=Math.max(max,len)}}
  let remaining=1
  for(let len=1;len<=15;len++){
    remaining=remaining*2-counts[len]
    if(remaining<0)throw new Error('Oversubscribed DEFLATE Huffman tree')
  }
  let code=0
  for(let len=1;len<=15;len++){code=(code+counts[len-1])*2;next[len]=code}
  lengths.forEach((len,symbol)=>{
    if(!len)return
    let bits=next[len]++,reversed=0
    for(let i=0;i<len;i++){reversed=(reversed<<1)|(bits&1);bits>>>=1}
    table[(1<<len)|reversed]=symbol+1
  })
  return {table,max}
}
const FIXED_LIT=huffman(Array.from({length:288},(_,i)=>i<144?8:i<256?9:i<280?7:8))
const FIXED_DIST=huffman(Array(32).fill(5))

export function inflateRawPortable(input,expectedLength){
  const source=input instanceof Uint8Array?input:new Uint8Array(input)
  if(!Number.isSafeInteger(expectedLength)||expectedLength<0)throw new Error('Invalid uncompressed GMT length')
  const output=new Uint8Array(expectedLength)
  let sourcePosition=0,buffer=0,bitCount=0,out=0
  const read=n=>{
    while(bitCount<n){
      if(sourcePosition>=source.length)throw new Error('Truncated DEFLATE stream')
      buffer|=source[sourcePosition++]<<bitCount;bitCount+=8
    }
    const result=buffer&((1<<n)-1)
    buffer>>>=n;bitCount-=n
    return result
  }
  const decode=tree=>{
    let value=0
    for(let len=1;len<=tree.max;len++){
      value|=read(1)<<(len-1)
      const hit=tree.table[(1<<len)|value]
      if(hit)return hit-1
    }
    throw new Error('Invalid DEFLATE Huffman code')
  }
  const append=value=>{
    if(out>=output.length)throw new Error('GMT ZIP output exceeds declared length')
    output[out++]=value
  }
  let last=0
  do {
    last=read(1)
    const type=read(2)
    if(type===0){
      const drop=bitCount&7
      if(drop)read(drop)
      const size=read(16),inverse=read(16)
      if(((size^inverse)&65535)!==65535)throw new Error('Invalid stored DEFLATE block')
      if(out+size>output.length)throw new Error('GMT ZIP output exceeds declared length')
      for(let i=0;i<size;i++)output[out++]=read(8)
      continue
    }
    if(type===3)throw new Error('Reserved DEFLATE block type')
    let literals=FIXED_LIT,distances=FIXED_DIST
    if(type===2){
      const nl=read(5)+257,nd=read(5)+1,nc=read(4)+4
      const lengths=new Uint8Array(19)
      for(let i=0;i<nc;i++)lengths[ORDER[i]]=read(3)
      const codeTree=huffman(lengths),codes=[]
      while(codes.length<nl+nd){
        const symbol=decode(codeTree)
        if(symbol<16)codes.push(symbol)
        else {
          if(symbol===16&&!codes.length)throw new Error('Invalid DEFLATE repeat code')
          const repetitions=symbol===16?read(2)+3:symbol===17?read(3)+3:read(7)+11
          const value=symbol===16?codes[codes.length-1]:0
          if(codes.length+repetitions>nl+nd)throw new Error('Invalid DEFLATE code length run')
          for(let i=0;i<repetitions;i++)codes.push(value)
        }
      }
      if(!codes[256])throw new Error('DEFLATE end-of-block symbol missing')
      literals=huffman(codes.slice(0,nl));distances=huffman(codes.slice(nl))
    }
    while(true){
      const symbol=decode(literals)
      if(symbol<256){append(symbol);continue}
      if(symbol===256)break
      const l=symbol-257
      if(l<0||l>=LEN_BASE.length)throw new Error('Reserved DEFLATE length code')
      const count=LEN_BASE[l]+read(LEN_EXTRA[l]),distanceCode=decode(distances)
      if(distanceCode>=DIST_BASE.length)throw new Error('Reserved DEFLATE distance code')
      const distance=DIST_BASE[distanceCode]+read(DIST_EXTRA[distanceCode])
      if(distance>out||out+count>output.length)throw new Error('Invalid DEFLATE back-reference or output length')
      for(let i=0;i<count;i++){output[out]=output[out-distance];out++}
    }
  }while(!last)
  if(out!==output.length)throw new Error('GMT ZIP output length mismatch')
  return output
}
