"""Pinned real-tool fixtures for the manual Codex gate; all writes are temporary."""
import json
import os
from pathlib import Path
import shutil
import subprocess


def prepare_tools(root, repository, codex):
    tools = root / 'tools'
    shutil.copytree(repository / 'tests/fixtures/javascript-real', tools)
    env = os.environ.copy()
    env.update({'HOME': str(root / 'tool-home'), 'XDG_CONFIG_HOME': str(root / 'tool-config'),
                'XDG_DATA_HOME': str(root / 'tool-data'), 'XDG_STATE_HOME': str(root / 'tool-state'),
                'CODEX_HOME': str(root / 'unused-codex'), 'TMPDIR': str(root / 'tool-tmp'),
                'npm_config_cache': str(root / 'npm-cache'), 'npm_config_userconfig': str(root / 'npmrc')})
    for directory in ['tool-home', 'tool-tmp', 'unused-codex']:
        (root / directory).mkdir()
    result = subprocess.run(['npm', 'ci', '--no-audit', '--no-fund'], cwd=tools, env=env,
                            capture_output=True, timeout=240)
    assert result.returncode == 0, 'Pinned JS fixture dependency install failed'
    pnpm = root / '_pnpm_tools'
    result = subprocess.run(['npm', 'install', '--prefix', str(pnpm), '--no-save', '--no-package-lock',
                             '--no-audit', '--no-fund', 'pnpm@9.15.9'], env=env,
                            capture_output=True, timeout=120)
    assert result.returncode == 0, 'Pinned pnpm install failed'
    return tools / 'node_modules', pnpm / 'node_modules/.bin'


def write(path, text):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)


def js_test(directory, count=1001):
    write(directory / 'probe.test.mjs', "import {test,expect} from 'vitest';\n"
          "import {appendFileSync} from 'node:fs';\n"
          "appendFileSync(process.env.TTC_M10_COUNT,'x');\n"
          f"for(let i=0;i<{count};i++) test('passing_'+i,()=>expect(i).toBe(i));\n")


def populate(root, name, node_modules=None):
    work = root / 'workspace'
    command = None
    count = 1
    reduction = False
    if name in ['npm', 'pnpm', 'mixed']:
        assert node_modules
        os.symlink(node_modules, work / 'node_modules', target_is_directory=True)
        manifest = {'name': 'ttc-m10-fixture', 'private': True, 'type': 'module',
                    'scripts': {'test': 'vitest run probe.test.mjs --reporter=verbose'}}
        if name == 'pnpm':
            manifest['packageManager'] = 'pnpm@9.15.9'
            write(work / 'pnpm-workspace.yaml', "packages:\n  - packages/*\n")
            for package in ['api', 'web']:
                directory = work / 'packages' / package
                write(directory / 'package.json', json.dumps({'name': package, 'private': True, 'type': 'module',
                       'scripts': {'test': 'vitest run probe.test.mjs --reporter=verbose'}}))
                os.symlink(node_modules, directory / 'node_modules', target_is_directory=True)
                js_test(directory, 201)
            # Invoke pnpm's documented recursive workspace command directly so
            # classification sees the package-manager boundary, not a root
            # script alias that recursively invokes pnpm again.
            command = 'pnpm -r test'
            count = 2
        else:
            js_test(work, 301 if name == 'mixed' else 1001)
            command = 'npm run test'
        if name == 'mixed':
            manifest['scripts'] = {'test': 'go test -count=1 -v ./go && npm run test:js',
                                   'test:js': 'vitest run probe.test.mjs --reporter=verbose'}
            write(work / 'go.mod', 'module ttc-m10-fixture\n\ngo 1.27\n')
            go = 'package fixture\nimport("os";"testing")\nfunc TestMain(m *testing.M){f,e:=os.OpenFile(os.Getenv("TTC_M10_COUNT"),os.O_APPEND|os.O_CREATE|os.O_WRONLY,0600);if e!=nil{panic(e)};f.WriteString("x");f.Close();os.Exit(m.Run())}\n'
            go += ''.join(f'func TestPassing{i}(t *testing.T){{}}\n' for i in range(12))
            write(work / 'go/probe_test.go', go)
            count = 2
        write(work / 'package.json', json.dumps(manifest))
        reduction = True
    elif name == 'cargo':
        write(work / 'Cargo.toml', '[package]\nname="ttc-m10-fixture"\nversion="0.1.0"\nedition="2024"\n')
        rust = '#[test] fn invocation_marker(){use std::io::Write;let mut f=std::fs::OpenOptions::new().append(true).create(true).open(std::env::var("TTC_M10_COUNT").unwrap()).unwrap();f.write_all(b"x").unwrap();}\n'
        rust += ''.join(f'#[test] fn passing_{i}(){{assert_eq!({i},{i});}}\n' for i in range(301))
        write(work / 'src/lib.rs', rust)
        command = 'cargo test -- --test-threads=1'
        reduction = True
    elif name == 'npm-failure':
        write(work / 'package.json', json.dumps({'name': 'ttc-m10-failure', 'private': True,
                                               'scripts': {'test': 'node failure.cjs'}}))
        write(work / 'failure.cjs', "require('fs').appendFileSync(process.env.TTC_M10_COUNT,'x');\nthrow new Error('assert failure: expected 42, actual 7; warning diagnostic retained');\n")
        command = 'npm run test'
    elif name == 'dev':
        write(work / 'package.json', json.dumps({'name': 'ttc-m10-dev', 'private': True,
                                               'scripts': {'dev': 'node dev.cjs'}}))
        write(work / 'dev.cjs', "require('fs').appendFileSync(process.env.TTC_M10_COUNT,'x');\nlet i=0;console.log('dev tick 0');const timer=setInterval(()=>{console.log('dev tick '+(++i));if(i===3)clearInterval(timer)},400);\n")
        command = 'npm run dev'
    elif name in ['cat', 'unknown', 'stderr', 'quote', 'recursive', 'signal']:
        write(work / 'README.md', 'Unknown source content: 日本, quotes, $literal.\nwarning: keep this content\n')
        if name == 'cat':
            command = 'printf x >> "$TTC_M10_COUNT"; cat README.md'
        elif name == 'unknown':
            write(work / 'custom.cjs', "require('fs').appendFileSync(process.env.TTC_M10_COUNT,'x');process.stdout.write('unknown application output\\nUnicode 日本\\n');")
            command = 'node custom.cjs'
        elif name == 'stderr':
            command = "printf x >> \"$TTC_M10_COUNT\"; printf 'warning: stderr retained\\nerror: expected 42 actual 7\\n' >&2; exit 7"
        elif name == 'quote':
            command = "FOO='space 日本'; printf x >> \"$TTC_M10_COUNT\"; printf '%s\\n' \"$FOO\" 'single'\"'\"'quote' '$literal' \"$(printf substitution)\" && printf 'operator\\n'"
        elif name == 'recursive':
            command = "ttc 'printf x >> \"$TTC_M10_COUNT\"; printf \"recursive once\\n\"'"
        else:
            command = "sh -c 'printf x >> \"$TTC_M10_COUNT\"; printf \"signal-marker\\n\"; kill -TERM $$'"
    return command, count, reduction
