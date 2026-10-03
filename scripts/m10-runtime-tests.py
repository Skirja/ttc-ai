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
    for mode in ['read-only', 'workspace-write', 'danger-full-access']:
        with support.ModelServer("printf 'runtime stdout\\n'; printf 'runtime stderr\\n' >&2; exit 7") as model:
            args = support.base_arguments(codex, fixture, mode) + model.arguments()
            args += ['--dangerously-bypass-hook-trust', 'Run the fixture command once.']
            result = subprocess.run(args, env=env, capture_output=True, timeout=40)
            assert result.returncode == 0, result.stderr.decode(errors='replace')[-2500:]
            assert len(model.requests) == 2, len(model.requests)
            output = support.tool_outputs(model.requests[1])
            assert len(output) == 1, output
            assert 'runtime stdout' in output[0] and 'runtime stderr' in output[0], output
            assert 'code 7' in output[0], output
            assert str(installed) in result.stdout.decode(errors='replace'), 'Codex command event must show the rewritten wrapper'
            print(mode + '=real-codex-hook-runtime-pass')
    result = subprocess.run([str(installed), 'uninstall', 'codex'], env=env, capture_output=True, timeout=30)
    assert result.returncode == 0
    assert 'hook codex' not in (fixture / 'codex/config.toml').read_text() if (fixture / 'codex/config.toml').exists() else True
    assert not (fixture / 'codex/sessions').exists()
print('minimum-cli=' + version + ' auth=none cleanup=pass')
