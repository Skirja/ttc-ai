#!/usr/bin/env python3
"""Read pinned upstream interfaces and record hashes, never vendor implementations."""
import concurrent.futures,hashlib,json,pathlib,re,urllib.request
ROOT=pathlib.Path(__file__).resolve().parents[1];snap=json.loads((ROOT/'docs/research/upstream.json').read_text());ref=snap['rtk_release_sha']
ledger=(ROOT/'docs/research/parity.md').read_text();paths=re.findall(r'\| \[([^\]]+)\]',ledger)
def audit(path):
    url=f'https://raw.githubusercontent.com/rtk-ai/rtk/{ref}/{path}'
    try:
        data=urllib.request.urlopen(url,timeout=30).read();s=data.decode()
        return {'path':path,'url':url,'sha256':hashlib.sha256(data).hexdigest(),'bytes':len(data),'functions':re.findall(r'pub(?:\([^)]*\))? fn (\w+)',s),'section_names':re.findall(r'^\[([^\]]+)\]',s,re.M),'has_rewrite':('rewrite' in s),'has_execution':('Command::' in s),'has_recovery':('recall' in s or 'tee' in s)}
    except Exception as e:return {'path':path,'error':str(e)}
rows=list(concurrent.futures.ThreadPoolExecutor(max_workers=8).map(audit,paths))
(ROOT/'docs/research/interfaces.json').write_text(json.dumps(rows,indent=2)+'\n')
for r in rows:print(r['path'],r.get('functions',r.get('section_names',r.get('error'))))
