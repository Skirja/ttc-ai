"""Manual authenticated gate. Auth is mounted read-only and never copied/read here."""
import argparse
import hashlib
import importlib.util
import json
import os
import signal
from pathlib import Path
import re
import signal
import shutil
import subprocess
import sys
import tempfile
sys.dont_write_bytecode = True

repository = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location('support', repository / 'scripts/m10-codex-support.py')
support = importlib.util.module_from_spec(spec)
spec.loader.exec_module(support)
fixture_spec = importlib.util.spec_from_file_location('fixtures', repository / 'scripts/m10-e2e-fixtures.py')
fixtures = importlib.util.module_from_spec(fixture_spec)
fixture_spec.loader.exec_module(fixtures)

MATRIX = ['sandbox-read-only', 'sandbox-workspace-write', 'sandbox-danger-full-access']
CASES = ['npm', 'npm-failure', 'pnpm', 'mixed', 'cargo', 'cat', 'unknown', 'stderr', 'quote', 'recursive', 'dev', 'signal']
active_process = None


def cleanup_signal(signum, _frame):
    # Raise through TemporaryDirectory so its finally cleanup runs on ordinary
    # terminal/CI termination signals as well as Python exceptions.
    if active_process is not None and active_process.poll() is None:
        try:
            os.killpg(active_process.pid, signal.SIGTERM)
            active_process.wait(timeout=10)
        except (OSError, subprocess.TimeoutExpired):
            try:
                os.killpg(active_process.pid, signal.SIGKILL)
                active_process.wait(timeout=2)
            except OSError:
                pass
            except subprocess.TimeoutExpired:
                pass
    raise SystemExit(128 + signum)


def source_digest():
    files = sorted([*repository.joinpath('src').rglob('*.rs'),
                    *[repository / name for name in ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'install.sh',
                                                      'scripts/m10-e2e.py', 'scripts/m10-e2e-fixtures.py',
                                                      'scripts/m10-codex-support.py', 'scripts/m10-install-auth-codex.sh']]])
    digest = hashlib.sha256()
    for path in files:
        digest.update(str(path.relative_to(repository)).encode() + b'\0' + path.read_bytes() + b'\0')
    return digest.hexdigest()


def tool_payloads(records):
    result = []
    seen = set()
    for record in records:
        for item in record['request']['input']:
            if item.get('type') in ['function_call_output', 'custom_tool_call_output'] and item['call_id'] not in seen:
                output = item['output']
                if isinstance(output, list):
                    assert all(block.get('type') in ['input_text', 'output_text'] and isinstance(block.get('text'), str) for block in output), 'Non-text model-facing output'
                    output = ''.join(block['text'] for block in output)
                assert isinstance(output, str)
                seen.add(item['call_id'])
                result.append(output)
    return result


def run_variant(binary, codex, root, env, launcher, command, mode, wrapped, expected_count=1):
    count = root / 'workspace/count'
    count.unlink(missing_ok=True)
    shutil.rmtree(root / 'traces', ignore_errors=True)
    if wrapped:
        result = subprocess.run([str(binary), 'install', 'codex'], env=env, capture_output=True, timeout=30)
        assert result.returncode == 0, 'Hook install failed'
        print(support.trust_hook(codex, root, env, launcher=launcher), flush=True)
        # UI trust is config state; its temporary session artifacts are discarded.
        shutil.rmtree(root / 'codex/sessions', ignore_errors=True)
        shutil.rmtree(root / 'sqlite', ignore_errors=True)
        shutil.rmtree(root / 'traces', ignore_errors=True)
    prompt = ('Execute exactly the following command once with exec_command, max_output_tokens=50000, '
              'and yield_time_ms=1000. Do not run any other command and do not retry on failure. '
              'Do not request escalation; keep default sandbox permissions. If it returns a session, poll with write_stdin until finished. Then answer Done.\n' + command)
    args = [*launcher, *support.base_arguments(codex, root, mode), prompt]
    global active_process
    child = subprocess.Popen(args, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
    active_process = child
    try:
        stdout, stderr = child.communicate(timeout=180)
        result = subprocess.CompletedProcess(args, child.returncode, stdout, stderr)
    except subprocess.TimeoutExpired as error:
        events = []
        for line in (error.stdout or b'').splitlines():
            try:
                event = json.loads(line)
            except ValueError:
                continue
            item = event.get('item', {})
            events.append({'type': event.get('type'), 'item_type': item.get('type'),
                           'status': item.get('status'), 'exit_code': item.get('exit_code')})
        print('Timed-out protocol states=' + json.dumps(events), flush=True)
        try:
            os.killpg(child.pid, signal.SIGTERM)
            child.wait(timeout=10)
        except (OSError, subprocess.TimeoutExpired):
            try:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait(timeout=2)
            except OSError:
                pass
            except subprocess.TimeoutExpired:
                pass
        raise AssertionError('Codex timed out; temporary state cleaned') from None
    finally:
        active_process = None
    assert result.returncode == 0, 'Authenticated Codex exec failed (details intentionally not exported)'
    records = support.inference_records(root / 'traces')
    assert records, 'Missing real inference trace'
    outputs = tool_payloads(records)
    assert outputs, 'Missing model-facing tool output; item-types=' + str(sorted({item.get('type','unknown') for record in records for item in record['request']['input']})) + '; count-present=' + str(count.exists())
    if any(re.search(r'tokens truncated|truncated output|output truncated|output was truncated', output, re.I) for output in outputs):
        print('model-tool-output-truncated lengths=' + str([len(output.encode()) for output in outputs]), flush=True)
        raise AssertionError('Codex tool output truncated before the following model request')
    following = [record for record in records if support.tool_outputs(record['request'])]
    assert following and all(record.get('tokens') for record in following), 'Missing following-request usage'
    model = following[-1]['model']
    tokens = following[-1]['tokens']['input_tokens']
    assert isinstance(tokens, int) and tokens > 0
    bodies = []
    exit_codes = []
    for output in outputs:
        body = output.split('\nOutput:\n', 1)[-1]
        try:
            carrier = json.loads(body)
        except json.JSONDecodeError:
            carrier = None
        if isinstance(carrier, dict) and {'chunk_id', 'wall_time_seconds', 'output'} <= carrier.keys():
            assert isinstance(carrier['output'], str)
            bodies.append(carrier['output'])
            if carrier.get('exit_code') is not None:
                exit_codes.append(carrier['exit_code'])
        else:
            bodies.append(body)
            exit_codes.extend(int(code) for code in re.findall(r'Process exited with code (\d+)', output))
    body = ''.join(bodies)
    if expected_count:
        assert count.read_bytes() == b'x' * expected_count, 'Original command invocation count must be one'
    else:
        assert body.count('invocation-marker\n') == 1, 'Read-only invocation marker must appear once'
    commands = {}
    native_exit_codes = []
    native_states = []
    for line in result.stdout.splitlines():
        event = json.loads(line)
        item = event.get('item', {})
        if item.get('type') == 'command_execution':
            commands[item['id']] = item['command']
            native_states.append({'status': item.get('status'), 'keys': sorted(item.keys()),
                                  'exit_code': item.get('exit_code')})
            if item.get('exit_code') is not None:
                native_exit_codes.append(item['exit_code'])
    assert len(commands) == 1, 'Exactly one native shell execution event is required'
    if not exit_codes:
        print('exit-status-diagnostic native=' + json.dumps(native_states) +
              ' payloads=' + json.dumps([{'bytes': len(output.encode()),
                  'keys': sorted(json.loads(output.split('\nOutput:\n', 1)[-1]).keys())
                  if output.split('\nOutput:\n', 1)[-1].startswith('{') and
                  isinstance(json.loads(output.split('\nOutput:\n', 1)[-1]), dict) else [],
                  'exit_markers': re.findall(r'exit(?:ed)?(?:\s+with\s+code)?\s*[:=]?\s*(-?\d+)', output, re.I)}
                 for output in outputs]), flush=True)
    if wrapped and not command.startswith('ttc '):
        assert str(binary) in result.stdout.decode(errors='replace'), 'Missing actual rewritten command event'
    if wrapped:
        uninstall = subprocess.run([str(binary), 'uninstall', 'codex'], env=env, capture_output=True, timeout=30)
        assert uninstall.returncode == 0, 'Uninstall after actual trust failed'
    assert not (root / 'codex/sessions').exists(), 'Ephemeral exec persisted sessions'
    return {'model': model, 'model_facing_bytes': sum(len(output.encode()) for output in outputs),
            'following_input_tokens': tokens, 'body': body, 'invocation_count': 1, 'fixture_invocations': expected_count,
            'tool_outputs': len(outputs), 'exit_codes': exit_codes or native_exit_codes,
            'native_shell_invocations': len(commands)}


def main():
    for signal_number in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
        signal.signal(signal_number, cleanup_signal)
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', default=str(repository / 'target/x86_64-unknown-linux-gnu/release/ttc'))
    parser.add_argument('--report', default=str(repository / 'target/m10-evidence/codex-e2e.json'))
    parser.add_argument('--case', default='all', choices=['all', 'probe', *CASES, *MATRIX])
    options = parser.parse_args()
    binary = Path(options.binary).resolve()
    assert binary.is_file()
    assert b'.debug_info' not in subprocess.check_output(['readelf', '-SW', str(binary)]), 'Release-mode binary required'
    codex = Path(os.environ.get('TTC_M10_AUTH_CODEX') or shutil.which('codex')).resolve()
    version = subprocess.check_output([str(codex), '--version'], text=True).strip()
    parts=version.removeprefix('codex-cli ').split('.')
    assert version == 'codex-cli 0.160.0', 'Authenticated E2E requires pinned Codex CLI 0.160.0'
    report = {'schema_version': 1, 'source_digest': source_digest(), 'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
              'codex_version': version, 'authenticated': True, 'cases': [], 'complete': False}
    selected = CASES + MATRIX if options.case == 'all' else [options.case]
    with tempfile.TemporaryDirectory(prefix='ttc-m10-auth-') as temporary:
        scratch = Path(temporary)
        node_modules = pnpm_bin = None
        if any(case in ['npm', 'pnpm', 'mixed'] for case in selected):
            print('Preparing pinned real-tool fixtures', flush=True)
            node_modules, pnpm_bin = fixtures.prepare_tools(scratch, repository, codex)
        if node_modules and pnpm_bin:
            tool_versions = {
                'node': subprocess.check_output(['node', '--version'], text=True).strip(),
                'npm': subprocess.check_output(['npm', '--version'], text=True).strip(),
                'vitest': subprocess.check_output([str(node_modules / '.bin/vitest'), '--version'],
                                                   text=True, cwd=scratch / 'tools').strip(),
                'pnpm': subprocess.check_output([str(pnpm_bin / 'pnpm'), '--version'], text=True).strip(),
                'rustc': subprocess.check_output(['rustup', 'run', '1.98.1', 'rustc', '--version'], text=True).strip(),
                'cargo': subprocess.check_output(['rustup', 'run', '1.98.1', 'cargo', '--version'], text=True).strip(),
                'go': subprocess.check_output(['go', 'version'], text=True).strip(),
                'codex_cli_asset_sha256': '4fcc47ab57f52ff75363951a8761146cd10c8288bd86fed45487dbb204a16b71',
                'fixture_package_lock_sha256': hashlib.sha256(
                    (repository / 'tests/fixtures/javascript-real/package-lock.json').read_bytes()).hexdigest(),
            }
            assert tool_versions['node'] == 'v24.21.0', 'Authenticated fixture requires Node.js 24.21.0'
            assert tool_versions['vitest'].startswith('vitest/5.0.1 '), 'Fixture lock must select Vitest 5.0.1'
            assert tool_versions['pnpm'] == '9.15.9', 'Fixture pnpm version must match its pinned package'
            assert tool_versions['rustc'].startswith('rustc 1.98.1 ')
            assert tool_versions['cargo'].startswith('cargo 1.98.1 ')
            assert tool_versions['go'].startswith('go version go1.27.1 ')
            report['tool_versions'] = tool_versions
        rustup_home = subprocess.check_output(['rustup', 'show', 'home'], text=True).strip()
        for name in selected:
            root = scratch / name
            root.mkdir()
            (root / 'workspace').mkdir()
            (root / 'tmp').mkdir()
            env = support.scoped_environment(root, codex)
            installed = support.install_binary(binary, env)
            env['PATH'] = str(installed.parent) + ':' + (str(pnpm_bin) + ':' if pnpm_bin else '') + env['PATH']
            env.update({'TTC_M10_COUNT': str(root / 'workspace/count'), 'CI': '1', 'NO_COLOR': '1',
                        'RUSTUP_HOME': rustup_home, 'RUSTUP_TOOLCHAIN': '1.98.1', 'CARGO_HOME': str(root / 'cargo'),
                        'GOCACHE': str(root / 'go-cache'), 'GOMODCACHE': str(root / 'go-mod'), 'GOTOOLCHAIN': 'local',
                        'npm_config_cache': str(root / 'npm-cache'), 'npm_config_userconfig': str(root / 'npmrc')})
            launcher = support.authentication_mount(root)
            mode = name.removeprefix('sandbox-') if name in MATRIX else 'danger-full-access'
            if name in MATRIX:
                outside = root / 'outside-write'
                command = "printf 'invocation-marker\\n'; if touch workspace-write 2>/dev/null; then printf 'workspace=yes\\n'; else printf 'workspace=no\\n'; fi; if touch '" + str(outside) + "' 2>/dev/null; then printf 'outside=yes\\n'; else printf 'outside=no\\n'; fi"
                expected_count, reduction = 0, False
            elif name == 'probe':
                command = r"printf x >> count; printf 'model-facing probe\n'; exit 7"
                expected_count, reduction = 1, False
            else:
                command, expected_count, reduction = fixtures.populate(root, name, node_modules)
            if name not in MATRIX:
                prerequisite = subprocess.run(['/bin/sh', '-c', command], cwd=root / 'workspace', env=env,
                                              capture_output=True, timeout=120)
                assert (root / 'workspace/count').is_file(), 'Fixture prerequisite did not invoke the intended runner'
                assert (root / 'workspace/count').read_bytes() == b'x' * expected_count
                assert prerequisite.returncode == 0 if reduction else True
            print('Running authenticated case ' + name, flush=True)
            baseline = run_variant(installed, codex, root, env, launcher, command, mode, False, expected_count)
            if name in MATRIX:
                (root / 'workspace/workspace-write').unlink(missing_ok=True)
                (root / 'outside-write').unlink(missing_ok=True)
            wrapped = run_variant(installed, codex, root, env, launcher, command, mode, True, expected_count)
            assert baseline['model'] == wrapped['model']
            if name in MATRIX:
                print(name + ' exit-diagnostic=' + repr((baseline['exit_codes'], wrapped['exit_codes'])) +
                      ' permission-outcomes=' + json.dumps([
                          {marker: marker in body for marker in ['workspace=yes', 'workspace=no', 'outside=yes', 'outside=no']}
                          for body in [baseline['body'], wrapped['body']]]), flush=True)
            assert baseline['exit_codes'] == wrapped['exit_codes'], 'Exit status differs from baseline'
            left, right = baseline.pop('body'), wrapped.pop('body')
            assert left and right, 'Missing child output in actual model request'
            output_content_equal = left == right
            permission_outcomes = None
            if reduction:
                print(name + ' reduction-diagnostic bytes=' + str(baseline['model_facing_bytes']) + '/' +
                      str(wrapped['model_facing_bytes']) + ' tokens=' + str(baseline['following_input_tokens']) + '/' +
                      str(wrapped['following_input_tokens']) + ' compact-marker=' + str('TTC:' in right), flush=True)
                assert wrapped['model_facing_bytes'] * 5 <= baseline['model_facing_bytes'], 'Byte reduction below 80%'
                assert wrapped['following_input_tokens'] < baseline['following_input_tokens'], 'Following-request tokens did not decrease'
                assert 'TTC:' in right, 'Missing actual compaction'
                if name == 'mixed':
                    assert right.count('passing_') < left.count('passing_'), 'JavaScript family not compacted'
                    assert right.count('--- PASS:') < left.count('--- PASS:'), 'Go family not compacted'
            else:
                assert output_content_equal, 'Raw output differs from baseline'
                assert 'TTC:' not in right
            if name in MATRIX:
                assert ('workspace=yes' in right) == (mode != 'read-only')
                assert ('outside=yes' in right) == (mode == 'danger-full-access')
                permission_outcomes = {
                    label: [label in value for value in [left, right]]
                    for label in ['workspace=yes', 'workspace=no', 'outside=yes', 'outside=no']
                }
            if name in ['npm-failure', 'stderr']:
                assert 'expected 42' in right and 'actual 7' in right
                assert wrapped['exit_codes'][-1] != 0
            if name == 'dev':
                for tick in range(4):
                    marker = 'dev tick ' + str(tick)
                    assert left.count(marker) == right.count(marker) == 1
            if name == 'signal':
                assert 'signal-marker\n' in left and 'signal-marker\n' in right
            report['cases'].append({'name': name, 'sandbox': mode,
                                    'output_content_equal': output_content_equal,
                                    'permission_outcomes': permission_outcomes,
                                    'baseline': baseline, 'ttc': wrapped})
            print(name + '=pass bytes=' + str(baseline['model_facing_bytes']) + '/' + str(wrapped['model_facing_bytes']) +
                  ' tokens=' + str(baseline['following_input_tokens']) + '/' + str(wrapped['following_input_tokens']), flush=True)
            shutil.rmtree(root)
    report['cleanup'] = True
    report['complete'] = options.case == 'all'
    report['trust'] = 'official-tui'
    report['auth_isolation'] = 'read-only-mount'
    path = Path(options.report)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(report, indent=2) + '\n')
    print('authenticated-cases=pass cleanup=pass report=' + str(path), flush=True)


if __name__ == '__main__':
    main()
