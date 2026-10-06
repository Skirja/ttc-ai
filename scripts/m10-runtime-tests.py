"""Run real pinned Codex without login; upstream model responses are fixtures."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
sys.dont_write_bytecode = True
import tempfile

root = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location('support', root / 'scripts/m10-codex-support.py')
support = importlib.util.module_from_spec(spec)
spec.loader.exec_module(support)
binary = Path(sys.argv[1]).resolve()
sections = subprocess.check_output(['readelf', '-SW', str(binary)])
assert b'.debug_info' not in sections, 'Codex runtime test requires a release-mode TTC binary'
codex = Path(os.environ.get('TTC_M10_CODEX', root / 'target/m10-tools/codex-0.154.0/bin/codex')).resolve()
assert codex.is_file(), 'Install pinned Codex with scripts/m10-install-codex.sh first'
version = subprocess.check_output([str(codex), '--version'], text=True).strip()
assert version == 'codex-cli 0.154.0', version

with tempfile.TemporaryDirectory(prefix='ttc-m10-runtime-') as temporary:
    fixture = Path(temporary)
    workspace = fixture / 'workspace'
    workspace.mkdir()
    env = support.scoped_environment(fixture, codex)
    (fixture / 'tmp').mkdir()
    installed = support.install_binary(binary, env)
    result = subprocess.run([str(installed), 'install', 'codex'], env=env, capture_output=True, timeout=30)
    assert result.returncode == 0, result.stderr.decode(errors='replace')
    subprocess.run([str(installed), 'uninstall', 'codex'], env=env, capture_output=True, check=True, timeout=30)
    ci_read_only = os.environ.get('TTC_M10_CI_RUNTIME') == '1'
    modes = ['read-only'] if ci_read_only else ['read-only', 'workspace-write', 'danger-full-access']
    for mode in modes:
        outside = fixture / 'outside-write'
        command = "printf 'runtime stdout\\n'; sleep 0.05; printf 'runtime stderr\\n' >&2; "
        command += "if touch workspace-write 2>/dev/null; then printf 'workspace=yes\\n'; else printf 'workspace=no\\n'; fi; "
        command += "if touch '" + str(outside) + "' 2>/dev/null; then printf 'outside=yes\\n'; else printf 'outside=no\\n'; fi; exit 7"
        bodies = []
        for wrapped in [False, True]:
            (workspace / 'workspace-write').unlink(missing_ok=True)
            outside.unlink(missing_ok=True)
            if wrapped:
                result = subprocess.run([str(installed), 'install', 'codex'], env=env, capture_output=True, timeout=30)
                assert result.returncode == 0
            with support.ModelServer(command) as model:
                args = support.base_arguments(codex, fixture, mode)
                if ci_read_only:
                    # GitHub-hosted runners block the unprivileged network
                    # namespace needed by Codex's default bubblewrap backend.
                    # Landlock still enforces the read-only policy tested here;
                    # the authenticated local gate covers all three modes.
                    args += ['-c', 'features.use_legacy_landlock=true']
                args += model.arguments()
                if wrapped:
                    args += ['--dangerously-bypass-hook-trust']
                args += ['Run the fixture command once.']
                result = subprocess.run(args, env=env, capture_output=True, timeout=40)
                assert result.returncode == 0, result.stderr.decode(errors='replace')[-2500:]
                assert len(model.requests) == 2, len(model.requests)
                output = support.tool_outputs(model.requests[1])
                assert len(output) == 1, output
                assert 'runtime stdout' in output[0] and 'runtime stderr' in output[0], output
                assert 'code 7' in output[0], output
                body = output[0].split('\nOutput:\n', 1)[1]
                assert body.count('runtime stdout') == 1
                assert ('workspace=yes' in body) == (mode != 'read-only'), body
                assert ('outside=yes' in body) == (mode == 'danger-full-access'), body
                bodies.append(body)
                if wrapped:
                    assert str(installed) in result.stdout.decode(errors='replace'), 'Missing actual rewritten command event'
            if wrapped:
                subprocess.run([str(installed), 'uninstall', 'codex'], env=env, capture_output=True, check=True, timeout=30)
        assert bodies[0] == bodies[1], 'Passthrough differs from direct sandbox baseline'
        print(mode + '=real-codex-hook-runtime-pass permission-equivalence=pass')
    assert 'hook codex' not in (fixture / 'codex/config.toml').read_text() if (fixture / 'codex/config.toml').exists() else True
    assert not (fixture / 'codex/sessions').exists()
print('minimum-cli=' + version + ' auth=none cleanup=pass')
