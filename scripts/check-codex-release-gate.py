"""Validate sanitized manual evidence against production inputs; never run auth in CI."""
import hashlib
import json
from pathlib import Path
import sys

root = Path(__file__).resolve().parent.parent
REQUIRED = {'npm', 'npm-failure', 'pnpm', 'mixed', 'cargo', 'cat', 'unknown', 'stderr', 'quote',
            'recursive', 'dev', 'signal', 'sandbox-read-only', 'sandbox-workspace-write', 'sandbox-danger-full-access'}
REDUCTION = {'npm', 'pnpm', 'mixed', 'cargo'}
FIXTURE_INVOCATIONS = {'npm': 1, 'npm-failure': 1, 'pnpm': 2, 'mixed': 2, 'cargo': 1,
                        'cat': 1, 'unknown': 1, 'stderr': 1, 'quote': 1, 'recursive': 1,
                        'dev': 1, 'signal': 1,
                        'sandbox-read-only': 0, 'sandbox-workspace-write': 0,
                        'sandbox-danger-full-access': 0}


def digest():
    files = sorted([*root.joinpath('src').rglob('*.rs'),
                    *[root / name for name in ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'install.sh',
                                                'scripts/m10-e2e.py', 'scripts/m10-e2e-fixtures.py',
                                                'scripts/m10-codex-support.py', 'scripts/m10-install-auth-codex.sh']]])
    result = hashlib.sha256()
    for path in files:
        result.update(str(path.relative_to(root)).encode() + b'\0' + path.read_bytes() + b'\0')
    return result.hexdigest()


def validate(report):
    assert report['schema_version'] == 1
    assert report['complete'] is True and report['cleanup'] is True and report['authenticated'] is True
    assert report['trust'] == 'official-tui' and report['auth_isolation'] == 'read-only-mount'
    assert report['source_digest'] == digest(), 'Manual Codex evidence does not match production source'
    assert len(report['binary_sha256']) == 64 and all(ch in '0123456789abcdef' for ch in report['binary_sha256'])
    assert report['codex_version'] == 'codex-cli 0.160.0'
    versions = report['tool_versions']
    assert versions['node'] == 'v24.21.0'
    assert versions['npm'] and versions['vitest'].startswith('vitest/5.0.1 ')
    assert versions['pnpm'] == '9.15.9'
    assert versions['rustc'].startswith('rustc 1.98.1 ')
    assert versions['cargo'].startswith('cargo 1.98.1 ')
    assert versions['go'].startswith('go version go1.27.1 ')
    assert versions['codex_cli_asset_sha256'] == '4fcc47ab57f52ff75363951a8761146cd10c8288bd86fed45487dbb204a16b71'
    package_lock = root / 'tests/fixtures/javascript-real/package-lock.json'
    assert versions['fixture_package_lock_sha256'] == hashlib.sha256(package_lock.read_bytes()).hexdigest()
    cases = {case['name']: case for case in report['cases']}
    assert len(cases) == len(report['cases']) and set(cases) == REQUIRED
    for name, case in cases.items():
        baseline, wrapped = case['baseline'], case['ttc']
        assert baseline['model'] == wrapped['model'] and baseline['model']
        assert isinstance(case['output_content_equal'], bool)
        for value in [baseline, wrapped]:
            assert value['invocation_count'] == value['native_shell_invocations'] == 1
            assert value['model_facing_bytes'] > 0 and value['following_input_tokens'] > 0
            assert value['exit_codes'] and isinstance(value['exit_codes'][-1], int)
            assert value['fixture_invocations'] == FIXTURE_INVOCATIONS[name]
        assert baseline['exit_codes'] == wrapped['exit_codes']
        if name not in REDUCTION:
            assert case['output_content_equal'] is True
        if name in REDUCTION:
            assert wrapped['model_facing_bytes'] * 5 <= baseline['model_facing_bytes']
            assert wrapped['following_input_tokens'] < baseline['following_input_tokens']
        if name in {'npm-failure', 'stderr'}:
            assert baseline['exit_codes'][-1] != 0 and wrapped['exit_codes'][-1] != 0
        if name == 'signal':
            assert baseline['exit_codes'][-1] == wrapped['exit_codes'][-1] == 143
        if name.startswith('sandbox-'):
            assert case['sandbox'] == name.removeprefix('sandbox-')
            assert case['permission_outcomes']
            assert all(len(pair) == 2 and pair[0] == pair[1]
                       for pair in case['permission_outcomes'].values())
            mode = case['sandbox']
            assert case['permission_outcomes']['workspace=yes'] == [mode != 'read-only'] * 2
            assert case['permission_outcomes']['workspace=no'] == [mode == 'read-only'] * 2
            assert case['permission_outcomes']['outside=yes'] == [mode == 'danger-full-access'] * 2
            assert case['permission_outcomes']['outside=no'] == [mode != 'danger-full-access'] * 2
    assert cases['npm']['baseline']['model_facing_bytes'] >= 10_000
    assert cases['pnpm']['baseline']['model_facing_bytes'] >= 10_000
    return report


if __name__ == '__main__':
    report_path = Path(sys.argv[1]) if len(sys.argv) > 1 else root / 'ai_docs/M10_CODEX_EVIDENCE.json'
    try:
        validate(json.loads(report_path.read_text()))
    except (AssertionError, KeyError, ValueError, OSError) as error:
        raise SystemExit('Codex manual release gate failed: ' + str(error)) from None
    print('Codex manual release gate matches source: ' + digest())
