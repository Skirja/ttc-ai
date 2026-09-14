#!/usr/bin/env python3
"""Compare real npm/pnpm lifecycle execution with and without TTC, offline."""
import json,os,pathlib,shutil,subprocess,tempfile
ROOT=pathlib.Path(__file__).resolve().parents[1];BIN=ROOT/'target/release/ttc'
rows=[]
for manager in ['npm','pnpm']:
    executable=shutil.which(manager)
    if not executable:
        rows.append({'manager':manager,'status':'not installed'});continue
    for fail in [False,True]:
        with tempfile.TemporaryDirectory(prefix='ttc-manager-') as temp:
            d=pathlib.Path(temp);(d/'bin').mkdir()
            cargo=d/'bin/cargo';cargo.write_text('''#!/bin/sh
printf '%s:%s\\n' "$npm_lifecycle_event" "$*" >> trace
printf 'test example ... ok\\n'
printf 'warning: intentional diagnostic\\n' >&2
if [ "$1" = test ] && [ "$FAIL_TEST" = 1 ]; then exit 7; fi
''');cargo.chmod(0o755)
            (d/'package.json').write_text(json.dumps({'name':'ttc-integration','version':'1.0.0','scripts':{'precheck':'cargo check --pre','check':'cargo check && cargo test','postcheck':'cargo check --post'}}))
            env=os.environ.copy();env.update(PATH=str(d/'bin')+os.pathsep+env['PATH'],FAIL_TEST='1' if fail else '0',TTC_DATA_DIR=str(d/'data'),TTC_CONFIG=str(d/'none'),npm_config_cache=str(d/'npm-cache'),COREPACK_ENABLE_NETWORK='0',NO_COLOR='1')
            args=[executable,'run','check','--','--max-warnings','0']
            original=subprocess.run(args,cwd=d,env=env,capture_output=True,timeout=30)
            trace=(d/'trace').read_bytes();(d/'trace').unlink()
            wrapped=subprocess.run([str(BIN),'run','--']+args,cwd=d,env=env,capture_output=True,timeout=30)
            assert wrapped.returncode==original.returncode,(manager,wrapped.stderr)
            assert (d/'trace').read_bytes()==trace,(manager,trace,(d/'trace').read_bytes())
            assert b'warning: intentional diagnostic' in wrapped.stderr
            rows.append({'manager':manager,'failure_case':fail,'original_exit':original.returncode,'ttc_exit':wrapped.returncode,'lifecycle_and_argv_identical':True,'trace':trace.decode().splitlines()})
(ROOT/'docs/real-manager-results.json').write_text(json.dumps(rows,indent=2)+'\n');print(json.dumps(rows,indent=2))
