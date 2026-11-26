#!/usr/bin/env python3
"""Paired first-hit, full scan and callback-cancel performance."""
import json,os,pathlib,statistics,subprocess,sys
tier,root,base,head,report=sys.argv[1:]
m=json.loads(pathlib.Path(__file__).with_name("native-manifest.json").read_text())["tiers"][tier]
rows=[]
for ext in ["cbh","2cbh"]:
    db=str(pathlib.Path(root)/(m["stem"]+"."+ext))
    for mapped in [True,False]:
        env=dict(os.environ,CBVAULT_NO_MMAP="0" if mapped else "1")
        env.pop("CBVAULT_MMAP",None)
        for threads in [1,4]:
            for limit in [0,1024]:
                samples={"base":[],"head":[]}
                sig=None
                for i in range(-1,3):
                    for side in (["base","head"] if i%2==0 else ["head","base"]):
                        r=json.loads(subprocess.check_output([base if side=="base" else head,db,str(threads),str(limit)],env=env,text=True))
                        assert r["failures"]==0
                        if not limit:
                            assert r["complete"] and r["games"]==m["games"]
                            current=tuple(r[k] for k in ("games","plies","hits","fingerprint"))
                            if sig is None:sig=current
                            assert current==sig
                        elif side=="head":
                            assert not r["complete"] and r["hits"]==limit and r["games"]<=2048*threads
                        if i>=0:samples[side].append(r)
                row={"format":ext,"mapped":mapped,"threads":threads,"limit":limit,"samples":samples}
                for side,rs in samples.items():
                    row[side]={k:statistics.median(r[k] for r in rs) for k in ("games","hits","seconds","first_hit_ns")}
                rows.append(row)
                pathlib.Path(report).write_text(json.dumps({"tier":tier,"summary":rows},indent=2)+"\n")
                print(json.dumps({k:v for k,v in row.items() if k!="samples"}),flush=True)
