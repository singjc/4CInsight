# AI Assistant — JCode setup and privacy (A1)

## What this is

The **AI Assistant** is an optional view in 4C Insight. It uses the locally installed **JCode** agent harness to provide explanations of analysis methods and results. The app stays browser-first: scientific analyses, source files, History and raw data never move to an application server. Connecting the assistant is a separate, explicit decision.

A1 supports **read-only chat**. It does not provide JCode with 4C Insight files, a shell tool, a scientific-tool registry, or access to browser data. The bridge launches JCode with `--tool-profile none` from a temporary working directory. **This is not an operating-system sandbox:** external JCode configuration, provider adapters and user-defined hooks may still execute independently. Users who need stronger isolation must run JCode in a reviewed container or OS account with restricted permissions. Do not assume the assistant is a certified clinical tool.

## What was borrowed from the earlier OpenDIA GUI design

The OpenDIA GUI v13.6–13.9 assistant used provider selection, session-only chat, user-reviewed context and permission boundaries. This version adopts those UI principles. The older OpenDIA GUI code was not copied because its archived source was not available in this workspace.

## 1. Install and check JCode

Choose the operating system in AI Assistant. OS detection is approximate; the browser cannot determine whether JCode is installed or which download matches the CPU. The app provides official commands to **copy and review**. It never executes an installer, downloads an executable in the background, or requests administrator privileges.

- Linux and macOS: download the install script from `https://jcode.sh/install`, inspect it, then run it if approved. Homebrew is an alternative for macOS.
- Windows 11: download and review `https://jcode.sh/install.ps1` in PowerShell, then run the reviewed file. The optional bridge also requires Python 3.10+.
- Validate locally with `jcode --version`. JCode may be installed in `~/.local/bin` or `~/.cargo/bin`, or found on PATH.

JCode official instructions: https://jcode.sh/docs. The built-in official install commands and latest releases can change; follow the current official documentation rather than trusting a stale copied command.

## 2. Connect a model provider

Use your own terminal to perform provider login:

```text
jcode login --provider openai
jcode login --provider claude
jcode login --provider ollama
```

Run **only the command for the chosen provider**. JCode manages credentials. Never type OAuth codes, API keys or passwords into the 4C Insight page. OpenAI ChatGPT sign-in and OpenAI API access have different billing. Claude subscription login and Claude API access also differ. Check current provider terms.

For Ollama, install and run Ollama separately, then run `ollama list` to see installed models. `ollama pull <model-name>` installs a chosen local model. Choose its exact ID in the AI Assistant model field or let JCode use the configured default. Large local models consume disk and RAM.

You can check provider access in the terminal with `jcode auth-test --all-configured`.

## 3. Start the optional bridge

The companion is a small Python 3.10+ script shipped with the 4C Insight source. It uses only Python's standard library. From the repository root run:

```bash
python3 tools/ai-assistant-bridge/bridge.py
```

On Windows with the Python launcher:

```powershell
py -3 tools/ai-assistant-bridge/bridge.py
```

The bridge binds **127.0.0.1:8765 only**, prints a random access token in the terminal, and allows requests only from `https://singjc.github.io` or a localhost development page. Paste that token into AI Assistant. Press **Check JCode connection**. The assistant can now detect the local JCode binary via the running bridge; the browser **cannot** discover or install local programs without that bridge.

Security controls:

- No `shell=True`, browser-supplied command, arbitrary filesystem route or automatic file upload.
- Only the `/status` and `/chat` routes exist. Provider values are limited to OpenAI, Claude and Ollama.
- The bridge requires an origin check and a one-time session token for both routes.
- Browser requests do not include cookies. The token and chat history stay in browser-tab memory, not project state, IndexedDB or History.
- JCode is started with `--tool-profile none` in a temporary directory; the optional user-level JCode hooks and provider configuration remain outside this control.
- One request runs at a time, with a 120-second model timeout; source files are not exposed to it.
- Cloud providers receive the typed prompt and any **explicitly selected** aggregate context. Ollama can run locally, depending on how the user configures it.

Modern browsers may restrict HTTPS-to-loopback requests or private-network access despite CORS headers. If this happens, check your browser's security policy; **do not disable browser security** to force the connection.

## 4. Ask about results

By default, the assistant sees **only what you type**. To add limited application context, check **Include the aggregate analysis summary** and examine the JSON preview first. Only fixed flags (result available or absent), the number of tested pathways and the evidence classification are included. **No raw gene expression, metadata rows, patient IDs, source filenames or individual gene scores are included.**

Example question: “What does a negative PCo2 correlation mean?” It means that a gene score decreases as the selected PCo2 coordinate increases. The axis sign is arbitrary, so it does not mean that the gene is downregulated in disease.

Example pathway question: “Why do 199 permutations produce tied q-values?” With an add-one empirical p-value, the minimum value is `1/(199+1) = 0.005`. BH correction across overlapping pathways can produce ties; this alone does not establish a disease association.

**Interpretation boundaries:** The assistant cannot validate the biological conclusions, infer causality, or claim that pathway overlap proves physical gene interactions. Corrected case/control results and exploratory PCoA rankings are different evidence types. Verify any statistical or biomedical claim against the original tables and methods.

## 5. Stop or disconnect

Use **Clear conversation** to remove chat text from this page. Stop the bridge in its terminal with Ctrl+C to revoke the current token and end local access. Reloading 4C Insight also clears the in-memory token and chat. JCode provider credentials remain in JCode's own configuration. Uninstall or change them through JCode, not through the 4C Insight browser.

## Future capability and review gates

A2 could add selected scientific evidence retrieval, model catalog discovery, structured tool calls, and per-action permission prompts. Those features **are not present in A1**. Before allowing tools or file modifications, implement an independent deterministic allowlist, explicit per-action approval, provenance capture, sandbox restrictions, scientific validation and cross-platform security tests.
