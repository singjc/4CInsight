/* 4C Insight page-to-extension relay. Never accepts traffic from other origins. */
(() => {
  const CHANNEL = '4c-insight-ai-v2';
  const exactSite = location.origin === 'https://singjc.github.io' &&
    (location.pathname === '/4CInsight/' || location.pathname.startsWith('/4CInsight/'));
  const localDev = ['http://localhost', 'http://127.0.0.1'].includes(location.origin.replace(/:\d+$/, '')) &&
    (location.pathname === '/' || location.pathname.startsWith('/4CInsight/'));
  if (!(exactSite || localDev) || window.top !== window) return;
  window.addEventListener('message', event => {
    if (event.source !== window || event.origin !== location.origin) return;
    const data = event.data;
    if (!data || data.channel !== CHANNEL || data.direction !== 'request' ||
        typeof data.id !== 'string' || !/^a2-[a-zA-Z0-9-]{1,70}$/.test(data.id) ||
        !['status', 'chat', 'login'].includes(data.op)) return;
    // The background and native helper repeat all validation.
    browser.runtime.sendMessage({op:data.op,payload:data.payload || {}})
      .then(reply => window.postMessage({channel:CHANNEL,direction:'response',id:data.id,
        ok:reply?.ok===true,result:reply?.result||{},error:reply?.error||'Companion unavailable'},location.origin))
      .catch(() => window.postMessage({channel:CHANNEL,direction:'response',id:data.id,
        ok:false,error:'AI Companion is not installed or cannot start.'},location.origin));
  });
})();
