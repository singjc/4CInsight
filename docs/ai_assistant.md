# 4C Insight AI Assistant — A2 on-demand JCode companion

## Goal

Users should open **AI Assistant**, select a model, and chat without running a service or copying access tokens.

The main 4C Insight analysis remains browser-only. The optional AI Companion is a **separate native component** because normal websites cannot start JCode or inspect the operating system. It is started by the browser through Native Messaging **only when an approved request occurs**. It has no listening port or permanent service.

## Connection and user consent

1. Install a **signed Firefox extension** and the matching, verified native companion **once**. Firefox may ask for permission. The companion installer must be approved by the operating system. These components are **not silently installed** from the website.
2. Open 4C Insight → AI Assistant → **Check connection**. The page sends a request to the extension. Firefox starts the native companion for that request and stops it afterward.
3. If JCode is installed, choose OpenAI, Claude or Ollama. Select **Open JCode sign-in** to start JCode's login in a visible terminal where supported. A local Ollama model must already be available.
4. Enter a question. Cloud providers receive the question and any aggregate data the user explicitly approved. No source file is shared automatically.

The login button supports Linux desktop terminals in A2. macOS and Windows are not packaged or verified. A different desktop terminal may require the user to start `jcode login --provider <provider>` once.

## What A2 implements

- Firefox MV2 development extension in `tools/ai-companion/extension-firefox/`.
- User-initiated page → content script → extension background → Firefox Native Messaging → per-request Python host.
- `status`, `login`, and `chat` operations only. No arbitrary command, file, or network endpoint.
- Allowed providers: `openai`, `claude`, `ollama`.
- JCode is launched with `--tool-profile none` from a temporary working directory. Provider login uses the installed JCode program; the site does not request model API keys.
- The chat and context-preview state are in page memory only. Neither is written to the portable state, History, or active session database.
- Only a fixed, user-approved summary can be supplied; patient IDs, gene identifiers, and raw source paths are not inferred or sent automatically.

## Security limits

Native Messaging is permission-controlled, not a substitute for OS sandboxing. Existing JCode global configuration, skills, user-level hooks, and credentials can still influence its behaviour. Do not treat `--tool-profile none` as a security boundary. The native process inherits the user's credentials as required by the selected provider.

The extension accepts requests only from the exact 4C Insight GitHub Pages path (or explicitly limited localhost development paths). The host repeats provider and payload validation. There is no localhost port, token or CORS bridge. If a trusted application page is compromised, these allowlisted operations remain available; users must continue to review any context that they share.

Do not automatically download and execute JCode install scripts. An end-user installer should bundle a pinned, verified JCode release per OS/CPU or link to a reviewed, signed upstream installer. Installation requires explicit user consent.

## Development-only Firefox verification

The source extension in `tools/ai-companion/extension-firefox/` is **unsigned** and cannot be installed permanently in standard Firefox. To develop, use `about:debugging` → **This Firefox** → **Load Temporary Add-on** → select `manifest.json`. Temporary extensions do not survive browser restart.

The Linux native host can be packaged as a `.deb`. The system manifest belongs at `/usr/lib/mozilla/native-messaging-hosts/org.fourcinsight.jcode.json`, and its `path` must reference an executable file at `/usr/lib/4c-insight-ai/native_host.py`. Verify the package contents and hashes before installing; the development build is not signed.

In the finished product, publish a Mozilla-signed Firefox extension and verified OS installers. Only then should the AI tab offer a production **Install companion** action linked to those verified releases. Standard Firefox cannot silently approve an unsigned extension.

The user does **not** need to start `python3 tools/ai-assistant-bridge/bridge.py` or paste a token. The A1 bridge is no longer part of the A2 workflow.

## Release validation

- Install and uninstall the signed add-on and native package.
- Confirm that a status request starts and exits the native host automatically.
- Check absent JCode, installed JCode, missing provider credentials, and real authenticated chat.
- Check Linux distribution variants and sandboxed Firefox separately.
- Repeat tests on Windows and macOS before declaring them supported.
- Confirm no open localhost listener, token field, or implicit source-data transmission.
- Confirm that all existing scientific tests and browser-only functionality remain unchanged.

Documentation: [Mozilla Native Messaging](https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/Native_messaging) · [Firefox extension signing](https://support.mozilla.org/en-US/kb/add-on-signing-in-firefox) · [JCode](https://jcode.sh/docs)
