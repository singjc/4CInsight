export async function saveTextFile(filename, text, mime = 'application/json') {
  if ('showSaveFilePicker' in window) {
    try {
      const handle = await window.showSaveFilePicker({
        suggestedName: filename,
        types: [{ description: '4C Insight project manifest', accept: { [mime]: ['.json'] } }],
      })
      const writable = await handle.createWritable()
      await writable.write(text)
      await writable.close()
      return
    } catch (error) {
      if (error instanceof DOMException && error.name === 'AbortError') return
    }
  }

  const blob = new Blob([text], { type: `${mime};charset=utf-8` })
  const url = URL.createObjectURL(blob)
  const anchor = document.createElement('a')
  anchor.href = url
  anchor.download = filename
  document.body.appendChild(anchor)
  anchor.click()
  anchor.remove()
  URL.revokeObjectURL(url)
}
