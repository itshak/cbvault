#!/usr/bin/env python3
"""Reuse exact counts over 48 sequential and scattered page requests."""
import json,os,pathlib,statistics,subprocess,sys
tier,root,exe,report=sys.argv[1:]
m=json.loads(pathlib.Path(__file__).with_name("native-manifest.json").read_text())["tiers"][tier]
rows=[]
for ext in ["cbh","2cbh"]:
    db=str(pathlib.Path(root)/(m["stem"]+"."+ext))
    for mapped in [True,False]:
        env=dict(os.environ,CBVAULT_NO_MMAP="0" if mapped else "1")
        env.pop("CBVAULT_MMAP",None)
        for filt in ["all","broad","selective"]:
            samples={"fresh":[],"plan":[]}
            sig=None
            for rep in range(-1,3):
                for mode in (["fresh","plan"] if rep%2==0 else ["plan","fresh"]):
                    r=json.loads(subprocess.check_output([exe,db,"4",filt,mode],env=env,text=True))
                    cur=tuple(r[k] for k in ("count","total","fingerprint"))
                    if sig is None:sig=cur
                    assert sig==cur
                    if rep>=0:samples[mode].append(r)
            row=dict(format=ext,mapped=mapped,filter=filt,signature=sig)
            for mode,rs in samples.items():
                row[mode]={k:statistics.median(r[k] for r in rs) for k in ("setup_seconds","seconds","p50_ns","p95_ns")}
            rows.append(row)
            pathlib.Path(report).write_text(json.dumps(dict(tier=tier,summary=rows),indent=2)+"\n")
            print(json.dumps(row),flush=True)
