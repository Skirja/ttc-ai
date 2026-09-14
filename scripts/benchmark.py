#!/usr/bin/env python3
"""Reproducible offline benchmark using fake tools; does not claim billing savings."""
import json,os,pathlib,statistics,subprocess,tempfile,time,resource
ROOT=pathlib.Path(__file__).resolve().parents[1]
BIN=ROOT/'target/release/ttc'
def timed(args,env,cwd):
    start=time.perf_counter_ns();p=subprocess.run(args,cwd=cwd,env=env,capture_output=True)
    if p.returncode:raise RuntimeError(p.stderr.decode(errors='replace'))
    return (time.perf_counter_ns()-start)/1e6,p
with tempfile.TemporaryDirectory(prefix='ttc-bench-') as temp:
    d=pathlib.Path(temp);env=os.environ.copy();env.update(TTC_DATA_DIR=str(d/'data'),TTC_CONFIG=str(d/'none.toml'))
    startup=[timed([str(BIN),'--version'],env,d)[0] for _ in range(30)]
    classify=[timed([str(BIN),'explain','cargo test','--json'],env,d)[0] for _ in range(30)]
    (d/'package.json').write_text(json.dumps({'scripts':{'check':'biome check . && tsc --noEmit && cargo clippy && go vet ./...'}}))
    resolve=[timed([str(BIN),'explain','pnpm check','--json'],env,d)[0] for _ in range(30)]
    rows=[]
    cases=[('cargo','test','cargo test',('test example_%d ... ok\n',10000),'test result: ok. 10000 passed; 0 failed\n'),('go','test','go test',('--- PASS: TestExample%d (0.00s)\n',10000),'PASS\nok example.org/pkg 0.020s\n'),('vitest','run','vitest',(' ✓ tests/example%d.test.ts (1 test)\n',1000),'Test Files 1000 passed\nTests 1000 passed\n'),('docker','build','docker build',('#%d DONE 0.2s\n',10000),'exporting image sha256:abc\n')]
    for exe,arg,label,(template,count),summary in cases:
        raw=''.join(template%i for i in range(count))+summary
        (d/'raw.log').write_text(raw)
        tool=d/exe;tool.write_text('#!/bin/sh\ncat raw.log\n');tool.chmod(0o755)
        ms,p=timed([str(BIN),'run','--',str(tool),arg],env,d)
        size=len(p.stdout)+len(p.stderr)
        rows.append({'command':label,'raw_bytes':len(raw.encode()),'ttc_bytes':size,'reduction_percent':round(100*(1-size/len(raw.encode())),2),'elapsed_ms':round(ms,3)})
    # Increasing-size logs verify capture limit fallback and bound process memory.
    memory=[]
    tool=d/'cargo';tool.write_text('#!/bin/sh\ncat raw.log\n');tool.chmod(0o755)
    for mb in [1,16,80]:
        with (d/'raw.log').open('wb') as f:
            block=(b'test large_case ... ok\n'*1024)
            for _ in range((mb*1024*1024)//len(block)):f.write(block)
        # GNU time reports child peak RSS on Linux; no dependency in normal runtime.
        rssfile=d/'rss.txt';cmd=[str(BIN),'run','--',str(tool),'test']
        if pathlib.Path('/usr/bin/time').exists():cmd=['/usr/bin/time','-f','%M','-o',str(rssfile)]+cmd
        start=time.perf_counter();p=subprocess.run(cmd,cwd=d,env=env,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        memory.append({'input_mib':mb,'exit':p.returncode,'elapsed_ms':round((time.perf_counter()-start)*1000,2),'peak_rss_kib':int(rssfile.read_text().strip()) if rssfile.exists() else None})
    result={'method':'Synthetic deterministic fixtures, local machine, 30 startup samples; elapsed includes fake child and disk capture. Today is UTC. No provider billing claim.','startup_median_ms':round(statistics.median(startup),3),'classification_median_ms':round(statistics.median(classify),3),'composite_resolution_median_ms':round(statistics.median(resolve),3),'output_reduction':rows,'large_log_memory':memory,'binary_bytes':BIN.stat().st_size}
    out=ROOT/'docs/benchmarks.json';out.write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
