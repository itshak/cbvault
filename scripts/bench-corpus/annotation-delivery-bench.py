#!/usr/bin/env python3
"""Paired native annotation delivery, exact raw payload fingerprints."""
import json, os, pathlib, statistics, subprocess, sys
tier, root, base, head, report = sys.argv[1:]
m = json.loads(pathlib.Path(__file__).with_name("native-manifest.json").read_text())["tiers"][tier]
db = str(pathlib.Path(root)/(m["stem"]+".2cbh"))
rows = []
signature = None
for mapped in [True, False]:
    env = dict(os.environ, CBVAULT_NO_MMAP="0" if mapped else "1")
    env.pop("CBVAULT_MMAP", None)
    for threads in [2,4]:
        samples = {"base":[], "head":[]}
        for rep in range(-1, 3):
            for side in (["base","head"] if rep%2==0 else ["head","base"]):
                r=json.loads(subprocess.check_output([base if side=="base" else head,db,str(threads)],env=env,text=True))
                assert r["games"]==m["games"] and r["plies"]==m["mainline_plies"]
                assert r["annotated"]>0 and r["bytes"]>0
                sig=tuple(r[k] for k in ("games","plies","annotated","bytes","fingerprint"))
                if signature is None: signature=sig
                assert signature==sig
                if rep>=0:samples[side].append(r["seconds"])
        row=dict(mapped=mapped,threads=threads,samples=samples,signature=signature)
        row.update({side:statistics.median(rs) for side,rs in samples.items()})
        row["speedup"]=row["base"]/row["head"]
        rows.append(row)
        pathlib.Path(report).write_text(json.dumps(dict(tier=tier,summary=rows),indent=2)+"\n")
        print(json.dumps(row),flush=True)
