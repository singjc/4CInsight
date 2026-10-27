/* 4C Insight cross-origin-isolation service worker.
 * It only proxies same-origin requests and adds the headers needed by future
 * threaded WebAssembly builds. No application data are uploaded or cached.
 */
self.addEventListener('install', () => self.skipWaiting())
self.addEventListener('activate', (event) => event.waitUntil(self.clients.claim()))

self.addEventListener('fetch', (event) => {
  const url = new URL(event.request.url)
  if (url.origin !== self.location.origin) return

  event.respondWith(
    fetch(event.request, { cache: 'no-store' }).then((response) => {
      if (!response || response.type === 'opaque') return response
      const headers = new Headers(response.headers)
      headers.set('Cross-Origin-Opener-Policy', 'same-origin')
      headers.set('Cross-Origin-Embedder-Policy', 'require-corp')
      headers.set('Cross-Origin-Resource-Policy', 'same-origin')
      return new Response(response.body, {
        status: response.status,
        statusText: response.statusText,
        headers,
      })
    }),
  )
})
