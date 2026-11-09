import test from 'node:test'
import assert from 'node:assert/strict'
import { readIntegrativeMetadataColumns, validIntegrativeIdentityRoles } from '../integrative-metadata-columns.js'

function makeFile(name, text, chunks = [3, 2, 5, 1, 7]) {
  const bytes = new TextEncoder().encode(text)
  let reads = 0, cancelled = false
  return {
    name,
    get reads() { return reads },
    get cancelled() { return cancelled },
    stream() {
      let pos = 0
      return new ReadableStream({
        pull(controller) {
          reads++
          if (pos >= bytes.length) { controller.close(); return }
          const n = chunks[(reads - 1) % chunks.length]
          controller.enqueue(bytes.slice(pos, pos + n))
          pos += n
        },
        cancel() { cancelled = true },
      })
    },
  }
}

test('TSV header loads without parsing the data rows', async () => {
  const file = makeFile('patients.tsv', '4C ID\tpatient_id\tvisit\n' + 'X\tY\tZ\n'.repeat(300), [100])
  assert.deepEqual(await readIntegrativeMetadataColumns(file), ['4C ID', 'patient_id', 'visit'])
  assert.equal(file.cancelled, true)
  assert.ok(file.reads < 10, 'Do not read the full cohort merely to obtain column names')
})

test('CSV quoted commas, UTF8 BOM, and split multibyte characters', async () => {
  const file = makeFile('metadata.CSV', '\uFEFF"patient, code",sample_id,évidence\r\nX,Y,Z\r\n', [1, 1, 1])
  assert.deepEqual(await readIntegrativeMetadataColumns(file), ['patient, code', 'sample_id', 'évidence'])
})

test('quoted newline in header does not end record early', async () => {
  const file = makeFile('sample.csv', '"batch\ncode",sample_id\r\nX,Y\r\n')
  // Metadata headers cannot contain line breaks, even when quoted.
  await assert.rejects(readIntegrativeMetadataColumns(file), /column name|line breaks/i)
})

test('duplicate metadata columns are rejected', async () => {
  const file = makeFile('sample.tsv', 'sample_id\tsample_id\nA\tB\n')
  await assert.rejects(readIntegrativeMetadataColumns(file), /unique/)
})

test('empty header and missing file fail with clear errors', async () => {
  await assert.rejects(readIntegrativeMetadataColumns(makeFile('empty.tsv','')), /empty/)
  await assert.rejects(readIntegrativeMetadataColumns(null), /local metadata file/)
})

test('identity roles support the same column for observation and patient', () => {
  assert.deepEqual(validIntegrativeIdentityRoles(['4C ID', 'visit'],
    {observation:'4C ID',subject:'4C ID',time:'visit'}),
    {observation:'4C ID',subject:'4C ID',time:'visit'})
})

test('stale metadata role selections are cleared on a new file', () => {
  assert.deepEqual(validIntegrativeIdentityRoles(['sample_id','patient_id'],
    {observation:'previous',subject:'old',time:'last'},'sample_id'),
    {observation:'sample_id',subject:'',time:''})
})

test('the visit cannot reuse the observation or patient identity column', () => {
  assert.deepEqual(validIntegrativeIdentityRoles(['sample_id','patient_id'],
    {observation:'sample_id',subject:'patient_id',time:'patient_id'}),
    {observation:'sample_id',subject:'patient_id',time:''})
})


test('Excel metadata must use the native WASM table parser, not text decoding',async()=>{
  const file=makeFile('cohort.xlsx','PK\u0003\u0004...')
  await assert.rejects(readIntegrativeMetadataColumns(file),/WASM metadata parser/)
  assert.equal(file.reads,0)
})
