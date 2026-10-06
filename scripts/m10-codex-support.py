"""Isolated Codex test helpers. Never load or serialize authentication data."""
import hashlib
import http.server
import json
import os
from pathlib import Path
import subprocess
import threading


def scoped_environment(root, codex):
    home = root / 'home'
    home.mkdir(exist_ok=True)
    codex_home = root / 'codex'
    codex_home.mkdir(exist_ok=True)
    return {'PATH': str(Path(codex).parent) + ':' + os.environ['PATH'],
            'HOME': str(home), 'CODEX_HOME': str(codex_home),
            'CODEX_SQLITE_HOME': str(root / 'sqlite'),
            'XDG_CONFIG_HOME': str(root / 'config'), 'XDG_DATA_HOME': str(root / 'data'),
            'XDG_STATE_HOME': str(root / 'state'), 'TMPDIR': str(root / 'tmp'),
            'TERM': 'xterm-256color', 'LANG': 'C.UTF-8',
            'CODEX_ROLLOUT_TRACE_ROOT': str(root / 'traces')}


def install_binary(binary, env):
    digest = hashlib.sha256(Path(binary).read_bytes()).hexdigest()
    result = subprocess.run([str(binary), '__install', '--sha256', digest], env=env,
                            capture_output=True, timeout=30)
    assert result.returncode == 0, result.stderr.decode(errors='replace')
    return Path(env['HOME']) / '.local/bin/ttc'


def base_arguments(codex, root, mode):
    return [str(codex), '-a', 'never', 'exec', '--ephemeral',
            '--skip-git-repo-check', '-C', str(root / 'workspace'), '-s', mode,
            '--json', '--color', 'never', '-c', 'history.persistence="none"',
            '-c', 'analytics.enabled=false', '-c', 'check_for_update_on_startup=false',
            '-c', 'features.daemon_auto_start=false', '-c', 'features.apps=false', '-c', 'features.plugins=false',
            '-c', 'sandbox_workspace_write.exclude_slash_tmp=true',
            '-c', 'sandbox_workspace_write.exclude_tmpdir_env_var=true',
            '-c', 'tool_output_token_limit=50000']


class ModelServer:
    """Fake upstream only: actual Codex dispatches tools and runs the TTC hook."""
    def __init__(self, command):
        self.command = command
        self.requests = []
        parent = self
        class Handler(http.server.BaseHTTPRequestHandler):
            def log_message(self, *_):
                pass
            def do_POST(self):
                assert self.path.endswith('/responses')
                body = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
                parent.requests.append(body)
                output = [{'type': 'function_call', 'id': 'fc_fixture',
                           'call_id': 'call_fixture', 'name': 'exec_command',
                           'arguments': json.dumps({'cmd': parent.command, 'yield_time_ms': 1000,
                                                   'max_output_tokens': 50000})}]
                if len(parent.requests) > 1:
                    output = [{'type': 'message', 'id': 'msg_fixture', 'role': 'assistant',
                               'status': 'completed', 'content': [{'type': 'output_text', 'text': 'Done.'}]}]
                response = {'id': 'resp_fixture_' + str(len(parent.requests)), 'object': 'response',
                            'status': 'completed', 'output': output,
                            'usage': {'input_tokens': 1, 'output_tokens': 1, 'total_tokens': 2}}
                events = [('response.created', {'type': 'response.created', 'response': dict(response, status='in_progress', output=[])}),
                          ('response.output_item.done', {'type': 'response.output_item.done', 'output_index': 0, 'item': output[0]}),
                          ('response.completed', {'type': 'response.completed', 'response': response})]
                self.send_response(200)
                self.send_header('Content-Type', 'text/event-stream')
                self.end_headers()
                for event, value in events:
                    self.wfile.write(('event: ' + event + '\ndata: ' + json.dumps(value) + '\n\n').encode())
                self.wfile.flush()
        self.server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
    def __enter__(self):
        self.thread.start()
        return self
    def __exit__(self, *_):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join()
    def arguments(self):
        return ['-m', 'ttc-runtime-fixture', '-c', 'model_provider="ttc_fixture"',
                '-c', 'model_providers.ttc_fixture.name="TTC offline fixture"',
                '-c', f'model_providers.ttc_fixture.base_url="http://127.0.0.1:{self.server.server_port}/v1"',
                '-c', 'model_providers.ttc_fixture.wire_api="responses"',
                '-c', 'model_providers.ttc_fixture.request_max_retries=0']


def tool_outputs(request):
    return [item['output'] for item in request['input'] if item.get('type') in ['function_call_output', 'custom_tool_call_output']]


def trust_hook(codex, root, env, provider_arguments=(), launcher=()):
    """Review the vetted TTC-only hook using Codex's actual TUI trust flow."""
    import fcntl
    import pty
    import re
    import select
    import struct
    import termios
    import time
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 40, 140, 0, 0))
    args = [str(codex), '--no-daemon', '--no-alt-screen', '-C', str(root / 'workspace'),
            '-a', 'never', '-c', 'history.persistence="none"', '-c', 'analytics.enabled=false',
            '-c', 'check_for_update_on_startup=false', '-c', 'features.apps=false', '-c', 'features.plugins=false',
            *provider_arguments]
    process = subprocess.Popen([*launcher, *args], env=env, stdin=slave, stdout=slave, stderr=slave)
    os.close(slave)
    transcript = ''
    def wait_for(pattern, timeout=15):
        nonlocal transcript
        deadline = time.monotonic() + timeout
        segment = ''
        while time.monotonic() < deadline:
            if process.poll() is not None:
                raise AssertionError('Codex trust UI exited before review completed')
            ready, _, _ = select.select([master], [], [], 0.1)
            if ready:
                data = os.read(master, 65536)
                if b'\x1b[6n' in data:
                    os.write(master, b'\x1b[1;1R')
                data = re.sub(r'\x1b\[[0-?]*[ -/]*[@-~]', '', data.decode(errors='replace'))
                segment += data
                transcript += data
                if re.search(pattern, segment):
                    return segment
        raise AssertionError('Codex trust UI did not reach expected review state: ' + pattern)
    try:
        wait_for(r'enter\s*confirm')
        time.sleep(3)
        os.write(master, b'2\r')
        time.sleep(3)
        os.write(master, b'/hooks')
        time.sleep(0.5)
        os.write(master, b'\r')
        # The actual UI persists trust; no hash/receipt is forged by this helper.
        wait_for(r'PreToolUse\s+1\s+1\s+(?:0\s+)?Before')
        os.write(master, b'\x1b')
    finally:
        process.terminate()
        try:
            process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=10)
        os.close(master)
    return 'official-tui-trust=pass'


def inference_records(trace_root):
    records = []
    for trace in sorted(Path(trace_root).rglob('trace.jsonl')):
        by_id = {}
        for line in trace.read_text().splitlines():
            payload = json.loads(line)['payload']
            kind = payload['type']
            if kind not in ['inference_started', 'inference_completed']:
                continue
            record = by_id.setdefault(payload['inference_call_id'], {})
            field = 'request_payload' if kind == 'inference_started' else 'response_payload'
            relative = Path(payload[field]['path'])
            assert not relative.is_absolute() and '..' not in relative.parts
            value = json.loads((trace.parent / relative).read_text())
            if kind == 'inference_started':
                record['request'] = value
                record['model'] = payload['model']
            else:
                record['tokens'] = value['token_usage']
        records.extend(by_id.values())
    return records


def authentication_mount(root):
    """Bind existing auth read-only; never open/copy credentials in Python."""
    import shutil
    auth_home = Path(os.environ.get('CODEX_HOME') or Path.home() / '.codex')
    auth = auth_home / 'auth.json'
    assert auth.is_file(), 'Existing file-backed Codex login is required'
    bwrap = shutil.which('bwrap')
    assert bwrap, 'bubblewrap is required for isolated authenticated E2E'
    return [bwrap, '--die-with-parent', '--unshare-user', '--ro-bind', '/', '/',
            '--bind', str(root), str(root), '--ro-bind', str(auth), str(root / 'codex/auth.json'),
            '--dev', '/dev', '--proc', '/proc']
