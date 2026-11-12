# AI Assistant — local Ollama only

4C Insight uses a local Ollama model to answer questions about analyses. The AI Assistant does not require JCode, an extension, an API key, WebGPU, or a separate 4C Insight bridge.

## First-time setup

1. Install Ollama from <https://ollama.com/download> if it is not already installed.
2. Start the Ollama application normally (Windows/macOS), or ensure the Linux `ollama` systemd service is active. On Linux, the official installation script normally enables and starts the service when systemd is available.
3. Open 4C Insight → **AI Assistant**. The page automatically checks `http://127.0.0.1:11434/api/version` and lists models from `/api/tags`.
4. Select an installed **local** model. If the list is empty, expand **Download another local model**, choose one of the recommended models, and approve the download. The download uses the local Ollama `/api/pull` endpoint.
5. Ask a question. Optionally review and approve the aggregate summary before including it. No source files are automatically sent.

### When the website cannot connect

The browser can check a running Ollama service, but **cannot inspect installed applications or start a stopped service**. A failed browser connection can mean Ollama is not installed, Ollama is stopped, browser permissions blocked the request, or Ollama rejected the website origin.

On Linux with systemd, the following commands can help verify and start the service:

```bash
systemctl is-active ollama
sudo systemctl start ollama
sudo systemctl enable ollama
```

Do not run `ollama serve` at the same time as an active service.

On macOS/Windows, open Ollama from the normal application menu.

### Hosted website origin and browser permission

Ollama normally accepts CORS access from local origins, but **4C Insight is hosted on `https://singjc.github.io`**. Ollama may need to allow that exact origin through `OLLAMA_ORIGINS=https://singjc.github.io`, and the browser may ask permission for loopback access. A system administrator can configure environment variables for Ollama and restart the service. Follow <https://docs.ollama.com/faq> for OS-specific instructions. Do not set `OLLAMA_ORIGINS=*` or expose Ollama with `OLLAMA_HOST=0.0.0.0`.

For Linux systemd, use a service override such as:

```ini
[Service]
Environment="OLLAMA_ORIGINS=https://singjc.github.io"
Environment="OLLAMA_NO_CLOUD=1"
```

**Review existing overrides first.** An `Environment` value with the same variable name can replace an existing one. After changes, reload and restart Ollama. The website cannot do this for you.

### Local-only privacy requirements

Ollama supports cloud models and web search in recent versions. To keep inference local, disable cloud features in the Ollama server using `OLLAMA_NO_CLOUD=1` or `disable_ollama_cloud: true` in `~/.ollama/server.json`, then restart the service. 4C Insight excludes models tagged `:cloud`, rejects models marked remote by the Ollama API, never sends requests to a remote inference endpoint, and uses no model tools. **The website cannot independently verify server-wide cloud-disable settings.**

The initial download obtains model weights from Ollama's remote registry. This is not an upload of your cohort data. Inference uses the local Ollama API. Chat remains in browser memory for the current page; clearing or reloading the page removes that page's chat. The app can optionally send a small, reviewed aggregate summary but does not read source files for chat.

### Technical and scientific limits

- The browser connects only to `127.0.0.1:11434`. No configurable remote inference URL is present.
- The browser may block localhost requests from HTTPS due to local network permission or cross-origin access rules.
- On a compatible machine Ollama can use CPU inference; WebGPU is not required by 4C Insight for this connection.
- The selected model's answer is an explanation, not a new statistical result or clinical validation. Verify claims against the original 4C Insight tables.
- The assistant cannot run commands, modify data, or launch applications. No cloud fallback is used if Ollama cannot connect.

**References:** <https://docs.ollama.com/faq>, <https://docs.ollama.com/api/tags>, <https://docs.ollama.com/api/chat>, <https://developer.mozilla.org/en-US/docs/Web/Security/Defenses/Local_network_access>.
