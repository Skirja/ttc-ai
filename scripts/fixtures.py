#!/usr/bin/env python3
"""Materialize independently authored golden examples; review changes before accepting."""
import json,pathlib,re
root=pathlib.Path(__file__).resolve().parents[1]
s=(root/'src/filtering/rules.rs').read_text()
rules=re.findall(r'family: "([^"]+)".*?example: "([^"\n]+)".*?kind: Kind::(Passing|Progress)',s,re.S)
assert rules
out=root/'tests/fixtures/semantic';out.mkdir(parents=True,exist_ok=True)
for i,(family,example,kind) in enumerate(rules):
    for case in ['success','failure','warning','large','edge']:
        count=1000 if case=='large' else 1
        noise=(example+'\n')*count
        diagnostic={'success':'tool summary: completed\n','failure':'error: assertion failed\n  src/auth.rs:42:7\nexpected: 401\nactual: 200\n','warning':'warning: deprecated API\nsecurity warning: dependency issue\n','large':'tool summary: completed\n','edge':'Unicode αβ\nANSI \u001b[31merror\u001b[0m\nno trailing newline'}[case]
        expected=diagnostic+f'\nTTC: {count if kind=="Passing" else 0} passing records, {count if kind=="Progress" else 0} progress lines compacted\n'
        (out/f'{i:02d}-{family}-{case}.json').write_text(json.dumps({'family':family,'case':case,'raw':noise+diagnostic,'expected':expected},ensure_ascii=False,indent=2)+'\n')
print(f'{len(rules)} semantic rules × 5 cases = {len(rules)*5} golden fixtures')
