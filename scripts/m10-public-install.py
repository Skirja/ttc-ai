"""Post-publication smoke of the real latest installer, entirely in temporary state."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

repository = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location('support', repository / 'scripts/m10-codex-support.py')
support = importlib.util.module_from_spec(spec)
spec.loader.exec_module(support)
codex = shutil.which('codex')
assert codex, 'Installed Codex CLI required for public hook install smoke'
with tempfile.TemporaryDirectory(prefix='ttc-public-install-') as temporary:
    root = Path(temporary)
    env = support.scoped_environment(root, codex)
    (root / 'tmp').mkdir()
    # This is the public URL; there is deliberately no test endpoint override.
    latest = subprocess.check_output(
        ['curl', '--proto', '=https', '--proto-redir', '=https', '--tlsv1.2', '-fsSL',
         '-o', os.devnull, '-w', '%{url_effective}', 'https://github.com/Skirja/ttc-ai/releases/latest'],
        env=env, text=True, timeout=60).strip()
    assert latest.startswith('https://github.com/Skirja/ttc-ai/releases/tag/v'), 'Public latest did not resolve to a stable tag'
    tag = latest.rsplit('/', 1)[-1]
    installer = root / 'install.sh'
    url = f'https://github.com/Skirja/ttc-ai/releases/download/{tag}/install.sh'
    checksums_path = root / 'SHA256SUMS'
    checksums_url = f'https://github.com/Skirja/ttc-ai/releases/download/{tag}/SHA256SUMS'
    subprocess.run(['curl', '--proto', '=https', '--proto-redir', '=https', '--tlsv1.2', '-fsSL', checksums_url,
                    '-o', str(checksums_path)], check=True, env=env, timeout=60)
    subprocess.run(['curl', '--proto', '=https', '--proto-redir', '=https', '--tlsv1.2', '-fsSL', url,
                    '-o', str(installer)], check=True, env=env, timeout=60)
    checksum_line = next((line for line in checksums_path.read_text().splitlines()
                          if line.split(maxsplit=1)[-1].lstrip('*') == 'install.sh'), None)
    assert checksum_line and len(checksum_line.split(maxsplit=1)[0]) == 64, 'Public installer checksum missing or invalid'
    assert hashlib.sha256(installer.read_bytes()).hexdigest() == checksum_line.split(maxsplit=1)[0].lower(), \
        'Public installer checksum mismatch'
    installed = Path(env['HOME']) / '.local/bin/ttc'
    checksums = []
    for _ in range(2):
        result = subprocess.run(['sh', str(installer)], env=env, capture_output=True, timeout=120)
        assert result.returncode == 0, 'Public latest installer failed'
        checksums.append(hashlib.sha256(installed.read_bytes()).hexdigest())
    assert checksums[0] == checksums[1], 'Latest changed during public smoke'
    version = subprocess.check_output([str(installed), '--version'], env=env, text=True).strip()
    subprocess.run([str(installed), 'install', 'codex'], env=env, capture_output=True, check=True, timeout=30)
    subprocess.run([str(installed), 'install', 'codex'], env=env, capture_output=True, check=True, timeout=30)
    subprocess.run([str(installed), 'uninstall', 'codex'], env=env, capture_output=True, check=True, timeout=30)
    subprocess.run([str(installed), 'uninstall'], env=env, capture_output=True, check=True, timeout=30)
    assert not installed.exists()
print(json.dumps({'public_latest': 'pass', 'tag': tag, 'version': version, 'binary_sha256': checksums[0],
                  'reinstall': 'pass', 'codex_install_uninstall': 'pass', 'cleanup': 'pass'}))
