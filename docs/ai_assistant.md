# AI Assistant — browser-first, no JCode installation

The AI Assistant is an optional explanation tool. The scientific calculations remain in 4C Insight. An AI response is not a replacement for statistical tests, independent validation, or clinical interpretation.

## Start here: on-device AI

1. Open **AI Assistant**. The default mode is **On-device · Private**.
2. Select **Check browser compatibility**. WebLLM needs a compatible WebGPU browser and GPU. 4C Insight does not guess GPU memory from total system RAM.
3. Choose a listed model. The choices have approximate GPU-memory requirements, not guaranteed download sizes.
4. Select **Download and start model**. Review the model name before approving this download. The browser obtains the WebLLM library and weights from external hosts. This download does not include your cohort data.
5. When the model reports ready, enter a question. Model inference runs inside the browser. The browser may reuse downloaded models from its cache; clearing site storage can remove them.
6. The model is unloaded from active memory when you select **Unload from memory**. Cached model files may remain until you clear browser site data.

If WebGPU is not supported, on-device AI cannot run with the current WebLLM engine. No CPU-only mode is claimed in this release. Try a compatible browser/device or use a separately configured advanced provider. A smaller model does not guarantee compatibility.

## What is sent to the model?

Questions are sent only after you select **Send question**. Source files, patient identifiers and raw data are never attached automatically. The checkbox **Include the approved aggregate analysis summary** is off by default. When enabled, the app shows exactly which fixed fields will be sent. These fields only say whether analysis outputs are available, the number of tested pathways, and whether pathway evidence is exploratory or from a defined comparison.

Chat history and API keys are kept only in this open page's JavaScript memory. They are not placed in the saved 4C Insight project JSON, browser local/session storage, or the History workspace. Other scripts in the page or browser extensions can still access browser memory; do not treat a browser page as an API-key vault.

## Optional: OpenAI API and Claude API

The standard ChatGPT or Claude chat subscription **is not a general-purpose API login for this static third-party website**. API access and billing are separate. Depending on your subscription, you may have separate API credits. Check your provider's current terms and billing.

Select **Online or Ollama · Advanced**, select **OpenAI API** or **Claude API**, and enter a model ID and a dedicated API key. The page shows a specific security warning and requires confirmation. **This is an advanced experimental connection**. Your key lives in webpage JavaScript memory until you select **Forget API key**, reload or close the page. Direct browser API keys can be stolen by malicious browser extensions, compromised third-party scripts, or injected code. For sensitive projects, do not put an API key into this page. Use a future trusted service with provider sign-in instead.

- OpenAI: the browser sends an authenticated request directly to `https://api.openai.com/v1/chat/completions`. API usage may incur charges.
- Claude: the browser uses `https://api.anthropic.com/v1/messages` and `anthropic-dangerous-direct-browser-access: true`. Anthropic can block browser calls for some organizations. API usage may incur charges.
- 4C Insight never stores or manages the user's provider subscription credentials.

## Optional: existing Ollama

If you already installed and started Ollama, the advanced mode can call its local endpoint at `http://127.0.0.1:11434`. Choose **Find running Ollama models**. Ollama may need its `OLLAMA_ORIGINS` setting configured to permit the 4C Insight page. The browser cannot install, start, or configure Ollama. Using local Ollama does not make OpenAI or Claude subscription models available.

## Privacy and operational notes

- The normal analysis workflows still run in the browser; no required 4C AI server, browser extension, local bridge, JCode, or native helper is used.
- On-device inference processes questions and optional aggregate context locally. Model software and weights require an initial network download.
- Online mode transmits questions, recent chat history, and optional user-approved aggregate context to the selected provider, subject to its privacy and retention policy.
- AI output is not a verified result. In particular, post-fit PCoA correlations and pathway overlaps are descriptive, not disease validation.
- No raw cohort files or patient-level records are automatically shared with any provider. The user can still type sensitive text into the question field, so avoid doing so in online mode.

## Technical notes for maintainers

The app uses a pinned browser import of WebLLM and a runtime-validated list of model IDs. Browser downloads and caching are managed by WebLLM. Model availability and browser GPU support must be verified in Firefox and Chromium before release. The cloud API paths are deliberately fixed, never accept arbitrary proxy URLs and have no stored credential persistence. The optional Ollama adapter does not run an installer.

### References

- WebLLM: https://webllm.mlc.ai/docs/user/get_started.html
- WebLLM model API: https://webllm.mlc.ai/docs/user/basic_usage.html
- OpenAI browser-key warning: https://help.openai.com/en/articles/5112595-best-practices-for-api-key-safety
- Claude direct-browser warning: https://platform.claude.com/docs/en/manage-claude/authentication
- Ollama API and CORS: https://docs.ollama.com/api/introduction
