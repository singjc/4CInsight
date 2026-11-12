/* A native-messaging host starts for each user request. No HTTP port or persistent process. */
const HOST = 'org.fourcinsight.jcode';
const ALLOWED_OPS = new Set(['status','chat','login']);
function trustedSender(sender) {
  const url = sender?.tab?.url || sender?.url || '';
  try {
    const u = new URL(url);
    if (u.protocol === 'https:' && u.host === 'singjc.github.io' &&
       (u.pathname === '/4CInsight/' || u.pathname.startsWith('/4CInsight/'))) return true;
    return (u.protocol === 'http:' && ['localhost','127.0.0.1'].includes(u.hostname) &&
       (u.pathname === '/' || u.pathname.startsWith('/4CInsight/')));
  } catch { return false; }
}
browser.runtime.onMessage.addListener((message,sender) => {
  if (!trustedSender(sender) || !message || !ALLOWED_OPS.has(message.op) ||
      !message.payload || typeof message.payload !== 'object' || Array.isArray(message.payload))
    return Promise.resolve({ok:false,error:'Request refused'});
  return browser.runtime.sendNativeMessage(HOST,{version:2,op:message.op,payload:message.payload})
    .then(result => result && typeof result.ok === 'boolean' ? result : {ok:false,error:'Invalid companion response'})
    .catch(() => ({ok:false,error:'Native companion not installed. Run the verified installer and reload Firefox.'}));
});
