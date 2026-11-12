#!/usr/bin/env python3
"""4C Insight native-messaging helper: stdio only, one message per browser request.

It is started by Firefox. No HTTP listener, access token, service, or arbitrary
command interface. This is a tool-free JCode invocation, not an OS sandbox.
"""
from __future__ import annotations
import json
import os
from pathlib import Path
import re
import shutil
import struct
import subprocess
import sys
import tempfile

MAX_REQUEST_BYTES = 24_000
MAX_REPLY_BYTES = 900_000
PROVIDERS = frozenset({'openai', 'claude', 'ollama'})
ANSI = re.compile(r'\x1b\[[0-9;]*[A-Za-z]')
HOST_NAME = 'org.fourcinsight.jcode'


def find_jcode() -> str | None:
    candidates = [shutil.which('jcode'), str(Path.home() / '.local/bin/jcode'),
                  str(Path.home() / '.cargo/bin/jcode')]
    for path in candidates:
        if path and Path(path).is_file() and os.access(path, os.X_OK):
            return str(Path(path).resolve())
    return None


def validated_payload(payload: object) -> tuple[list[str], str]:
    if not isinstance(payload, dict) or set(payload) - {'provider', 'model', 'message', 'history', 'context'}:
        raise ValueError('Invalid chat fields')
    provider = payload.get('provider')
    if provider not in PROVIDERS:
        raise ValueError('Unsupported model provider')
    model = payload.get('model', '')
    if not isinstance(model, str) or (model and not re.fullmatch(r'[a-zA-Z0-9._:/+\-]{1,100}', model)):
        raise ValueError('Invalid model ID')
    message = payload.get('message')
    if not isinstance(message, str) or not (1 <= len(message.strip()) <= 4000):
        raise ValueError('Question must contain 1 to 4,000 characters')
    history = payload.get('history', [])
    if not isinstance(history, list) or len(history) > 6:
        raise ValueError('History must contain at most six messages')
    text = ['You are a read-only analysis explainer in 4C Insight.',
            'Use concise ASD-STE100 language. Do not claim you read project files.',
            'Do not claim disease validation from exploratory results.',
            'Do not request credentials. Never claim to have executed tools.']
    for m in history:
        if not isinstance(m, dict) or set(m) != {'role','content'} or m.get('role') not in ('user','assistant') or not isinstance(m.get('content'),str) or len(m['content']) > 2000:
            raise ValueError('Invalid conversation history')
        text.append(f"Previous {m['role']} message: {m['content']}")
    context = payload.get('context')
    if context is not None:
        allowed = {'context_type','integrative_result_available','compare_groups_result_available',
                   'pathway_result_available','pathway_sets_tested','pathway_evidence_type'}
        if not isinstance(context,dict) or set(context) != allowed or context.get('context_type') != 'aggregate_4c_insight_summary_v1':
            raise ValueError('Only the approved aggregate context is allowed')
        if not all(isinstance(context[k],bool) for k in ('integrative_result_available','compare_groups_result_available','pathway_result_available')):
            raise ValueError('Invalid summary flags')
        num=context.get('pathway_sets_tested')
        if num is not None and (not isinstance(num,int) or isinstance(num,bool) or not 0 <= num <= 1_000_000):
            raise ValueError('Invalid pathway count')
        if context.get('pathway_evidence_type') not in ('exploratory','defined_comparison','unverified'):
            raise ValueError('Invalid evidence type')
        text.append('Approved aggregate summary: '+json.dumps(context,separators=(',',':')))
    text.append('Current question: '+message.strip())
    args = ['--tool-profile','none','--provider',provider]
    if model:
        args.extend(['--model',model])
    args.extend(['run','\n\n'.join(text)])
    return args, message.strip()


def execute_chat(payload: object, binary: str) -> dict:
    args,_ = validated_payload(payload)
    with tempfile.TemporaryDirectory(prefix='4c-ai-request-') as wd:
        try:
            p=subprocess.run([binary,*args],cwd=wd,capture_output=True,text=True,timeout=120,check=False)
        except subprocess.TimeoutExpired as exc:
            raise RuntimeError('JCode request timed out') from exc
        except OSError as exc:
            raise RuntimeError('JCode could not start') from exc
    if p.returncode:
        # Never echo potentially sensitive provider stderr into the browser.
        raise RuntimeError('JCode returned an error. Check account login in JCode.')
    answer=ANSI.sub('',p.stdout).strip()[:12000]
    if not answer:
        raise RuntimeError('JCode returned no response')
    return {'answer':answer}


def open_login(provider: object, binary: str, platform: str | None = None) -> dict:
    if provider not in PROVIDERS:
        raise ValueError('Unsupported model provider')
    platform = platform or sys.platform
    args = [binary, 'login', '--provider', provider]
    # Only an explicit click may call this operation. Never run a shell.
    if platform.startswith('linux'):
        for term, prefix in [('x-terminal-emulator',['-e']), ('konsole',['-e']),
                             ('gnome-terminal',['--']), ('xterm',['-e'])]:
            path=shutil.which(term)
            if path:
                subprocess.Popen([path,*prefix,*args],cwd=str(Path.home()),
                                 stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,
                                 start_new_session=True,close_fds=True)
                return {'message':f'JCode {provider} sign-in opened in a terminal window.'}
    # Never claim a login window opened when the platform launch is unavailable.
    raise RuntimeError(f'Automatic provider login is not available in this desktop environment. Run: jcode login --provider {provider}')


def handle(request: object) -> dict:
    if not isinstance(request,dict) or set(request) != {'version','op','payload'} or request['version'] != 2:
        raise ValueError('Invalid companion request')
    op=request['op']
    payload=request['payload']
    if not isinstance(payload,dict):
        raise ValueError('Invalid request payload')
    if op not in {'status','chat','login'}:
        raise ValueError('Unsupported operation')
    binary=find_jcode()
    if op=='status':
        if payload:
            raise ValueError('Status takes no arguments')
        if not binary:
            return {'installed':False,'version':''}
        try:
            with tempfile.TemporaryDirectory(prefix='4c-ai-version-') as wd:
                p=subprocess.run([binary,'--version'],cwd=wd,capture_output=True,text=True,timeout=6)
            version=ANSI.sub('',p.stdout).strip().splitlines()[0][:100] if p.returncode==0 else ''
        except (OSError, subprocess.TimeoutExpired, IndexError):
            version=''
        return {'installed':True,'version':version}
    if not binary:
        raise RuntimeError('JCode is not installed. Install JCode before connecting a provider.')
    if op=='login':
        if set(payload) != {'provider'}:
            raise ValueError('Login requires a provider')
        return open_login(payload['provider'],binary)
    return execute_chat(payload,binary)


def read_message(stream=sys.stdin.buffer) -> dict:
    header=stream.read(4)
    if len(header)!=4:
        raise ValueError('Missing native message header')
    length=struct.unpack('=I',header)[0]
    if not 1 <= length <= MAX_REQUEST_BYTES:
        raise ValueError('Invalid native message length')
    data=stream.read(length)
    if len(data)!=length:
        raise ValueError('Incomplete native message')
    return json.loads(data.decode('utf-8'))


def write_message(message: dict,stream=sys.stdout.buffer) -> None:
    value=json.dumps(message,ensure_ascii=False,separators=(',',':')).encode('utf-8')
    if len(value)>MAX_REPLY_BYTES:
        value=b'{"ok":false,"error":"Reply exceeds native-messaging limit"}'
    stream.write(struct.pack('=I',len(value))+value)
    stream.flush()


def main() -> None:
    try:
        result=handle(read_message())
        write_message({'ok':True,'result':result})
    except (ValueError,KeyError,TypeError,RuntimeError,UnicodeDecodeError,json.JSONDecodeError) as exc:
        # Exceptions must not leak source data or provider stderr.
        write_message({'ok':False,'error':str(exc)[:200]})
    except Exception:
        write_message({'ok':False,'error':'AI Companion failed unexpectedly'})


if __name__ == '__main__':
    main()
