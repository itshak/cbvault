#!/usr/bin/env python3
"""Same binary, paired old per-query pool and new context; exact result parity."""
import json, os, pathlib, statistics, subprocess, sys
exe, root, tier, report = sys.argv[1:]
m=json.loads(pathlib.Path(__file__).with_name("native-manifest.json").read_text())["tiers"][tier]
rows=[]
for ext in ["cbh","2cbh"]:
    db=str(pathlib.Path(root)/(m["stem"]+"."+ext))
    for mapped in [True,False]:
        env=dict(os.environ,CBVAULT_NO_MMAP="0" if mapped else "1")
        env.pop("CBVAULT_MMAP",None)
        for n in [1,2,4]:
            for width,repeats in [(16384,60),(65536,30),(m["games"],8)]:
                samples={"fresh":[],"reuse":[]}
                sig=None
                for rep in range(-1,5):
                    for mode in (["fresh","reuse"] if rep%2==0 else ["reuse","fresh"]):
                        r=json.loads(subprocess.check_output([exe,db,str(n),str(width),mode,str(repeats)],env=env,text=True))
                        current=(r["count"],r["fingerprint"])
                        if sig is None:sig=current
                        assert current==sig,(ext,n,width,mode,r,sig)
                        if rep>=0:samples[mode].append(r)
                row={"format":ext,"mapped":mapped,"threads":n,"width":min(width,m["games"]),"queries":repeats,"fingerprint":sig}
                for mode,vals in samples.items():
                    row[mode]={k:statistics.median(v[k] for v in vals) for k in ("seconds","setup_seconds","p50_ns","p95_ns")}
                row["speedup"]=row["fresh"]["seconds"]/row["reuse"]["seconds"]
                rows.append(row)
                pathlib.Path(report).write_text(json.dumps({"tier":tier,"summary":rows},indent=2)+"\n")
                print(json.dumps(row),flush=True)
