#!/usr/bin/env python3
"""Paired complete PGN exports: strict byte oracle, mapped and positioned."""
import hashlib,json,pathlib,statistics,subprocess,sys,tempfile,time
tier,root,base,head,report=sys.argv[1:]
m=json.loads(pathlib.Path(__file__).with_name("native-manifest.json").read_text())["tiers"][tier]
db=str(pathlib.Path(root)/(m["stem"]+".2cbh"))
rows=[]
with tempfile.TemporaryDirectory() as tmp:
    out=pathlib.Path(tmp)/"out.pgn"
    for mapped in [True,False]:
        for threads in [1,4]:
            samples={"base":[],"head":[]}
            for rep in range(-1,4):
                for side in (["base","head"] if rep%2==0 else ["head","base"]):
                    cmd=[base if side=="base" else head,"pgn",db,str(out),"--threads",str(threads),"--json"]
                    if not mapped:cmd.append("--no-mmap")
                    t=time.perf_counter()
                    r=subprocess.run(cmd,capture_output=True,text=True,check=True)
                    elapsed=time.perf_counter()-t
                    stats=json.loads(r.stderr)
                    assert stats["games"]==m["games"] and stats["failures"]==0
                    with out.open("rb") as f:
                        assert hashlib.file_digest(f,"sha256").hexdigest()==m["export_sha256"]["2cbh"]
                    out.unlink()
                    if rep>=0:samples[side].append(elapsed)
            row=dict(mapped=mapped,threads=threads,samples=samples)
            row.update({side:statistics.median(rs) for side,rs in samples.items()})
            row["speedup"]=row["base"]/row["head"]
            rows.append(row)
            pathlib.Path(report).write_text(json.dumps(dict(tier=tier,summary=rows),indent=2)+"\n")
            print(json.dumps(row),flush=True)
