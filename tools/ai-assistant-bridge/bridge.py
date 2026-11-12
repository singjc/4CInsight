#!/usr/bin/env python3
"""Optional 4C Insight -> JCode localhost chat bridge (stdlib only).

No arbitrary command endpoint, no files, no browser-supplied environment variables.
Only a locally installed JCode binary is started, with --tool-profile none.
Not an OS sandbox: existing JCode account configuration and hooks may still apply.
"""
from __future__ import annotations

import json
import os
import re
import secrets
import shutil
import subprocess
import sys
import tempfile
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import urlsplit

PORT = 8765
PROVIDERS = frozenset({'openai', 'claude', 'ollama'})
BUSY = threading.BoundedSemaphore(1)
ANSI = re.compile(r'\x1b\[[0-9;]*[A-Za-z]')
SESSION_TOKEN = secrets.token_urlsafe(32)


def allowed_origin(origin: str | None) -> bool:
    if not origin:
        return False
    if origin == 'https://singjc.github.io':
        return True
    try:
        url = urlsplit(origin)
        return url.scheme == 'http' and url.hostname in {'localhost', '127.0.0.1'} and url.port is not None and url.username is None and url.password is None and not url.path and not url.query and not url.fragment
    except ValueError:
        return False


def find_jcode() -> str | None:
    # Fixed local candidates; never take a program path from a browser request.
    candidates = [shutil.which('jcode'), str(Path.home() / '.local/bin/jcode'), str(Path.home() / '.cargo/bin/jcode')]
    for path in candidates:
        if path and Path(path).is_file() and os.access(path, os.X_OK):
            return str(Path(path).resolve())
    return None


def jcode_version(binary: str) -> str:
    try:
        with tempfile.TemporaryDirectory(prefix='4c-ai-status-') as wd:
            result = subprocess.run([binary, '--version'], cwd=wd, capture_output=True, text=True, timeout=6, check=False)
        if result.returncode == 0:
            return ANSI.sub('', result.stdout).strip().splitlines()[0][:100]
    except (OSError, subprocess.TimeoutExpired, IndexError):
        pass
    return 'JCode installed (version unavailable)'


def payload_for_jcode(payload: object) -> tuple[list[str], str]:
    if not isinstance(payload, dict):
        raise ValueError('A JSON object is required')
    provider = payload.get('provider')
    if provider not in PROVIDERS:
        raise ValueError('Unsupported provider')
    model = payload.get('model', '')
    if not isinstance(model, str) or (model and not re.fullmatch(r'[a-zA-Z0-9._:/+\-]{1,100}', model)):
        raise ValueError('Invalid model ID')
    message = payload.get('message')
    if not isinstance(message, str) or not (1 <= len(message.strip()) <= 4000):
        raise ValueError('Enter a question of 1–4,000 characters')
    history = payload.get('history', [])
    if not isinstance(history, list) or len(history) > 6:
        raise ValueError('History must have at most six messages')
    lines = ['You are a read-only analysis explainer in 4C Insight.',
             'Use concise ASD-STE100 language. Do not claim to have accessed any project files.',
             'Do not claim statistical validation from exploratory results. Respond only to the question and user-approved context.',
             'You cannot use tools or edit the project. No requests for credentials.']
    for item in history:
        if not isinstance(item, dict) or item.get('role') not in {'user', 'assistant'} or not isinstance(item.get('content'), str) or len(item['content']) > 2000:
            raise ValueError('Invalid conversation history')
        lines.append(f"Previous {item['role']} message: {item['content']}")
    context = payload.get('context')
    if context is not None:
        if not isinstance(context, dict) or set(context) != {'context_type','integrative_result_available','compare_groups_result_available','pathway_result_available','pathway_sets_tested','pathway_evidence_type'} or context.get('context_type') != 'aggregate_4c_insight_summary_v1':
            raise ValueError('Only approved aggregate summary fields are allowed')
        if any(not isinstance(context.get(k), bool) for k in ('integrative_result_available','compare_groups_result_available','pathway_result_available')):
            raise ValueError('Invalid summary flags')
        count = context.get('pathway_sets_tested')
        if count is not None and (not isinstance(count, int) or isinstance(count, bool) or not 0 <= count <= 1000000):
            raise ValueError('Invalid pathway count')
        if context.get('pathway_evidence_type') not in {'exploratory', 'defined_comparison','unverified'}:
            raise ValueError('Invalid pathway evidence type')
        lines.append('Approved aggregate summary: ' + json.dumps(context, separators=(',', ':')))
    lines.append('Current question: ' + message.strip())
    args = ['--tool-profile', 'none', '--provider', provider]
    if model:
        args.extend(['--model', model])
    args.extend(['run', '\n\n'.join(lines)])
    return args, message.strip()


def run_chat(payload: object, binary: str) -> str:
    args, _ = payload_for_jcode(payload)
    # No shell=True, no browser-set path, no repository working directory.
    # Existing JCode user-level config and hooks are outside this bridge's control.
    with tempfile.TemporaryDirectory(prefix='4c-ai-chat-') as wd:
        try:
            result = subprocess.run([binary, *args], capture_output=True, text=True, cwd=wd, timeout=120, check=False)
        except subprocess.TimeoutExpired as exc:
            raise RuntimeError('JCode timed out after 120 seconds. Check the model locally.') from exc
        except OSError as exc:
            raise RuntimeError('JCode could not start. Check your local installation.') from exc
    if result.returncode:
        # Never reflect raw stderr: provider output may contain sensitive text.
        raise RuntimeError('JCode returned an error. Use jcode auth-test --all-configured in your terminal.')
    reply = ANSI.sub('', result.stdout).strip()
    if not reply:
        raise RuntimeError('JCode returned no text. Check its provider configuration.')
    return reply[:12000]


class Handler(BaseHTTPRequestHandler):
    server_version = '4CInsightLocalAIBridge/1'
    sys_version = ''

    def log_message(self, *_args):
        return  # Do not log prompts, tokens, or client headers.

    def origin(self):
        return self.headers.get('Origin')

    def authorised_origin(self) -> bool:
        return allowed_origin(self.origin()) and self.headers.get('Host') in {f'127.0.0.1:{PORT}', f'localhost:{PORT}'}

    def send_headers(self, code: int, length: int):
        self.send_response(code)
        if self.authorised_origin():
            self.send_header('Access-Control-Allow-Origin', self.origin())
            self.send_header('Access-Control-Allow-Headers', 'X-4C-Session, Content-Type')
            self.send_header('Access-Control-Allow-Methods', 'GET, POST, OPTIONS')
            self.send_header('Access-Control-Allow-Private-Network', 'true')
            self.send_header('Vary', 'Origin')
        self.send_header('Cache-Control', 'no-store')
        self.send_header('X-Content-Type-Options', 'nosniff')
        self.send_header('Content-Type', 'application/json; charset=utf-8')
        self.send_header('Content-Length', str(length))
        self.end_headers()

    def send_json(self, code, obj):
        value = json.dumps(obj, ensure_ascii=False).encode('utf-8')
        self.send_headers(code, len(value))
        self.wfile.write(value)

    def do_OPTIONS(self):
        self.send_headers(204 if self.authorised_origin() else 403, 0)

    def authorised(self) -> bool:
        return self.authorised_origin() and secrets.compare_digest(self.headers.get('X-4C-Session', ''), SESSION_TOKEN)

    def do_GET(self):
        if not self.authorised():
            self.send_json(403, {'error':'Forbidden'}); return
        if self.path != '/status':
            self.send_json(404, {'error':'Unknown endpoint'}); return
        binary = find_jcode()
        self.send_json(200, {'bridge':'ready','installed': bool(binary),'version':jcode_version(binary) if binary else '', 'mode':'tool-free single turn','providers':sorted(PROVIDERS)})

    def do_POST(self):
        if not self.authorised():
            self.send_json(403, {'error':'Forbidden'}); return
        if self.path != '/chat':
            self.send_json(404, {'error':'Unknown endpoint'}); return
        length = self.headers.get('Content-Length', '')
        if not length.isdigit() or not (1 <= int(length) <= 24000):
            self.send_json(413, {'error':'Invalid request size'}); return
        try:
            payload = json.loads(self.rfile.read(int(length)))
            payload_for_jcode(payload)
        except (ValueError, UnicodeDecodeError, json.JSONDecodeError) as exc:
            self.send_json(400, {'error': str(exc)[:160]}); return
        binary = find_jcode()
        if not binary:
            self.send_json(409, {'error':'JCode not installed or not on bridge PATH'});return
        if not BUSY.acquire(blocking=False):
            self.send_json(429, {'error':'Another JCode request is running'});return
        try:
            self.send_json(200, {'answer':run_chat(payload,binary)})
        except RuntimeError as exc:
            self.send_json(502, {'error':str(exc)[:180]})
        finally:
            BUSY.release()


def main():
    print('4C Insight local JCode bridge — chat-only mode', flush=True)
    print('Listening only on http://127.0.0.1:8765',flush=True)
    print('Allowed origin: https://singjc.github.io (or a localhost development page)',flush=True)
    print('Access token: ' + SESSION_TOKEN, flush=True)
    print('No source files are read or uploaded by this bridge. Stop with Ctrl+C.',flush=True)
    try:
        server = ThreadingHTTPServer(('127.0.0.1', PORT), Handler)
    except OSError as exc:
        print('Could not bind loopback port 8765: ' + str(exc), file=sys.stderr)
        return 1
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
