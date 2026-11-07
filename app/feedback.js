// UX15: optional, privacy-first issue reporting for the browser-only app.
// This module never sends data, error logs, or project files automatically.
const REPOSITORY = 'https://github.com/singjc/4CInsight';
const ISSUE_URL = `${REPOSITORY}/issues/new`;
const DETAILS_LIMIT = 4000;
const seenErrors = new Set();
let lastError = '';
let scanQueued = false;

export function composeFeedback({ kind = 'bug', title = '', description = '', steps = '', expected = '', diagnostic = '', includeDiagnostic = false, version = '' } = {}) {
  const type = kind === 'feature' ? 'Feature request' : 'Bug report';
  const safe = (value, limit = DETAILS_LIMIT) => String(value ?? '').trim().slice(0, limit);
  const heading = safe(title, 110);
  if (!heading) throw new Error('Add a short title before continuing.');
  const body = [
    `### Type\n${type}`,
    `### Description\n${safe(description) || 'Not provided'}`,
    ...(kind === 'feature' ? [] : [
      `### Steps to reproduce\n${safe(steps) || 'Not provided'}`,
      `### Expected behavior\n${safe(expected) || 'Not provided'}`,
    ]),
    `### Application version\n${safe(version, 80) || 'Not provided'}`,
    ...(includeDiagnostic && safe(diagnostic) ? [`### User-reviewed diagnostic details\n\`\`\`text\n${safe(diagnostic, 2500).replaceAll('```', '[code fence removed]')}\n\`\`\``] : []),
    '\n---\nPlease do not include patient identifiers, source file paths, passwords, or private data.',
  ].join('\n\n');
  return { title: `[4C Insight] ${kind === 'feature' ? '[Feature] ' : '[Bug] '}${heading}`, body };
}

export function githubIssueDraftURL(issue) {
  const q = new URLSearchParams({ title: issue.title, body: issue.body });
  return `${ISSUE_URL}?${q.toString()}`;
}

export function currentVersion(doc = document) {
  const script = doc.querySelector('script[src*="main.js"]');
  const value = script?.getAttribute('src')?.match(/[?&]v=([^&]+)/)?.[1];
  return value ? decodeURIComponent(value).slice(0, 80) : 'Not available';
}

function show(dialog) {
  if (!dialog?.open) dialog?.showModal?.();
}
function close(dialog) { if (dialog?.open) dialog.close(); }

function reportError(error) {
  const raw = String(error ?? '').trim().slice(0, 4000);
  if (!raw || seenErrors.has(raw)) return;
  seenErrors.add(raw);
  lastError = raw;
  const form = document.querySelector('#feedback-form');
  // Do not overwrite a report the user is writing.
  if (!form || !document.querySelector('#feedback-dialog')?.open) show(document.querySelector('#feedback-error-prompt'));
}

function scanAppErrors() {
  const main = document.querySelector('#main-content');
  if (!main) return;
  const banner = main.querySelector('.error-banner[role="alert"]');
  if (banner?.textContent?.trim()) reportError(banner.textContent);
  // Partial modality failures can appear under scientific results rather than
  // in the top error banner. Ordinary scientific warnings are not errors.
  for (const row of main.querySelectorAll('.analysis-warnings > div')) {
    const message = row.textContent?.trim() || '';
    if (/\b(?:failed|failure|error|panic|unreachable executed|out of memory)\b/i.test(message)) {
      reportError(message);
    }
  }
}

function initFeedback() {
  const feedbackDialog = document.querySelector('#feedback-dialog');
  const errorDialog = document.querySelector('#feedback-error-prompt');
  const form = document.querySelector('#feedback-form');
  if (!feedbackDialog || !errorDialog || !form) return;
  const summary = document.querySelector('#feedback-form-error');
  const diag = document.querySelector('#feedback-diagnostic');

  function openFeedback(kind = 'bug', fromError = false) {
    close(errorDialog);
    if (!feedbackDialog.open) form.reset();
    const selected = form.querySelector(`input[name="feedback-kind"][value="${kind}"]`);
    if (selected) selected.checked = true;
    if (fromError && !feedbackDialog.open) {
      form.elements.namedItem('feedback-title').value = 'Problem during analysis';
    }
    summary.textContent = '';
    diag.value = fromError ? lastError : '';
    // Error details remain opt-in and fully editable before the user opens GitHub.
    diag.hidden = true;
    document.querySelector('#feedback-diagnostic-label').hidden = true;
    document.querySelector('#feedback-include-diagnostic').checked = false;
    form.querySelectorAll('.feedback-bug-fields').forEach((node) => { node.hidden = kind === 'feature'; });
    show(feedbackDialog);
  }

  document.querySelector('#feedback-report')?.addEventListener('click', () => openFeedback());
  document.querySelector('#feedback-error-report')?.addEventListener('click', () => openFeedback('bug', true));
  document.querySelector('#feedback-error-dismiss')?.addEventListener('click', () => close(errorDialog));
  document.querySelector('#feedback-dismiss')?.addEventListener('click', () => close(feedbackDialog));
  document.querySelector('#feedback-include-diagnostic')?.addEventListener('change', (event) => {
    diag.hidden = !event.currentTarget.checked;
    document.querySelector('#feedback-diagnostic-label').hidden = !event.currentTarget.checked;
  });
  form.querySelectorAll('input[name="feedback-kind"]').forEach((radio) => radio.addEventListener('change', () => {
    const isFeature = form.querySelector('input[name="feedback-kind"]:checked')?.value === 'feature';
    form.querySelectorAll('.feedback-bug-fields').forEach((node) => { node.hidden = isFeature; });
  }));
  function makeIssue() {
    return composeFeedback({
      kind: form.querySelector('input[name="feedback-kind"]:checked')?.value,
      title: form.elements.namedItem('feedback-title').value,
      description: form.elements.namedItem('feedback-description').value,
      steps: form.elements.namedItem('feedback-steps').value,
      expected: form.elements.namedItem('feedback-expected').value,
      diagnostic: diag.value,
      includeDiagnostic: document.querySelector('#feedback-include-diagnostic').checked,
      version: currentVersion(),
    });
  }
  document.querySelector('#feedback-send')?.addEventListener('click', () => {
    try {
      const issue = makeIssue();
      summary.textContent = 'GitHub will open with an editable draft. Sign in and select Submit new issue to post it.';
      // The URL is created only after the user chooses to send this reviewed text.
      const link = document.createElement('a');
      link.href = githubIssueDraftURL(issue);
      link.target = '_blank';
      link.rel = 'noopener noreferrer';
      link.click();
    } catch (error) { summary.textContent = String(error.message || error); }
  });
  document.querySelector('#feedback-download')?.addEventListener('click', () => {
    try {
      const issue = makeIssue();
      const blob = new Blob([`# ${issue.title}\n\n${issue.body}\n`], { type: 'text/markdown;charset=utf-8' });
      const url = URL.createObjectURL(blob);
      const link = document.createElement('a');
      link.href = url;
      link.download = '4c-insight-feedback.md';
      link.click();
      setTimeout(() => URL.revokeObjectURL(url), 3000);
      summary.textContent = 'Report saved on this device. No issue was submitted.';
    } catch (error) { summary.textContent = String(error.message || error); }
  });

  window.addEventListener('error', (event) => {
    if (event.message) reportError(event.message);
  });
  window.addEventListener('unhandledrejection', (event) => {
    const error = event.reason instanceof Error ? event.reason.message : String(event.reason ?? 'Analysis stopped unexpectedly');
    reportError(error);
  });
  // The app displays handled scientific errors in its existing error banner.
  // Observing that banner covers these without changing the analysis engine.
  const main = document.querySelector('#main-content');
  if (main) new MutationObserver(() => {
    if (scanQueued) return;
    scanQueued = true;
    queueMicrotask(() => { scanQueued = false; scanAppErrors(); });
  }).observe(main, { childList: true, subtree: true });
  scanAppErrors();
}

if (typeof document !== 'undefined') {
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', initFeedback, { once: true });
  else initFeedback();
}
