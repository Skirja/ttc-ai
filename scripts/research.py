#!/usr/bin/env python3
"""Freeze public upstream evidence and enumerate parity; no code is vendored."""
import concurrent.futures, json, pathlib, urllib.request, datetime
ROOT=pathlib.Path(__file__).resolve().parents[1]
OUT=ROOT/'docs/research'
def get(url):
    return urllib.request.urlopen(urllib.request.Request(url,headers={'User-Agent':'ttc-ai-independent-research'}),timeout=30).read()
def main():
    release=json.loads(get('https://api.github.com/repos/rtk-ai/rtk/releases/latest'))
    refs={}
    for ref in [release['tag_name'],'develop']:
        refs[ref]=json.loads(get(f'https://api.github.com/repos/rtk-ai/rtk/git/trees/{ref}?recursive=1'))
    codex=json.loads(get('https://api.github.com/repos/openai/codex/commits/main'))['sha']
    snapshot={'checked_at':datetime.datetime.now(datetime.timezone.utc).isoformat(),'rtk_release':release['tag_name'],'rtk_release_sha':refs[release['tag_name']]['sha'],'rtk_develop_sha':refs['develop']['sha'],'codex_source_sha':codex,'installed_codex':'0.154.0-alpha.6.2','source_note':'Public main is not proof of installed binary equivalence.'}
    OUT.mkdir(parents=True,exist_ok=True)
    (OUT/'upstream.json').write_text(json.dumps(snapshot,indent=2)+'\n')
    paths=[x['path'] for x in refs[release['tag_name']]['tree'] if (x['path'].startswith('src/cmds/') and x['path'].endswith('.rs')) or (x['path'].startswith('src/filters/') and x['path'].endswith('.toml')) or (x['path'].startswith('hooks/') and x['path'].endswith('README.md'))]
    rows=['# Upstream parity ledger','',f"Reference: RTK {release['tag_name']} at `{snapshot['rtk_release_sha']}`.",'','Recognition is not semantic-filter parity. Pending rows are release blockers for full parity.','', '| RTK capability source | TTC equivalent / improvement | Implemented | Tested |','|---|---|---|---|']
    for p in paths:
        rows.append(f"| [{p}](https://github.com/rtk-ai/rtk/blob/{snapshot['rtk_release_sha']}/{p}) | See support matrix; retain raw on uncertainty | Pending audit | Pending |")
    (OUT/'parity.md').write_text('\n'.join(rows)+'\n')
    issues=[2358,1489,2879,3675,2094,2832,259,444,2878]
    def issue(n):
        try:
            d=json.loads(get(f'https://api.github.com/repos/rtk-ai/rtk/issues/{n}'))
            return {'number':n,'title':d['title'],'state':d['state'],'updated_at':d['updated_at'],'url':d['html_url'],'summary':d['body'][:25000]}
        except Exception as e:return {'number':n,'error':str(e)}
    data=list(concurrent.futures.ThreadPoolExecutor(max_workers=4).map(issue,issues))
    # Keep metadata and links; independently summarize behavior in research notes.
    for d in data:d.pop('summary',None)
    (OUT/'issues.json').write_text(json.dumps(data,indent=2)+'\n')
    print(json.dumps(snapshot,indent=2)); print(f'{len(paths)} parity entries, {len(data)} issue references')
if __name__=='__main__':main()
