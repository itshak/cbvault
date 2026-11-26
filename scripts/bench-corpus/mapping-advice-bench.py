#!/usr/bin/env python3
"""Same binary, warm filesystem cache, varying only kernel access advice.

Each process has fresh mappings. The filesystem page cache is not flushed.
Report both latency and process RSS/faults; no claim about cold physical I/O.
"""
import json,os,pathlib,statistics,subprocess,sys,tempfile
tier,root,exe,report=sys.argv[1:]
m=json.loads(pathlib.Path(__file__).with_name("native-manifest.json").read_text())["tiers"][tier]
rows=[]
for ext in ("cbh","2cbh"):
    db=str(pathlib.Path(root)/(m["stem"]+"."+ext))
    for op in ("random-moves","random-pgn","random-pgn-noann","broad","stream-keys","position-e4"):
        n=1 if op.startswith("random-") else 4
        modes=("sequential","normal","random","none","positioned")
        if ext=="cbh" and op=="broad": modes=modes[:-1]
        samples={mode:[] for mode in modes}
        sig=None
        for rep in range(-1,5):
            order=modes if rep<0 else modes[rep%len(modes):]+modes[:rep%len(modes)]
            for mode in order:
                env=dict(os.environ,CBVAULT_MMAP_ADVICE=mode,CBVAULT_NO_MMAP="1" if mode=="positioned" else "0")
                env.pop("CBVAULT_MMAP",None)
                with tempfile.TemporaryDirectory() as tmp:
                    timefile=pathlib.Path(tmp)/"time"
                    r=json.loads(subprocess.check_output(["/usr/bin/time","-f","%M %F %R","-o",str(timefile),exe,db,str(m["games"]),op,str(n)],env=env,text=True))
                    r["rss_kib"],r["major_faults"],r["minor_faults"]=map(int,timefile.read_text().split())
                cur=tuple(r[k] for k in ("count","units","fingerprint"))
                if sig is None:sig=cur
                assert cur==sig,(ext,op,mode,cur,sig)
                if op in m["workloads"]:
                    w=m["workloads"][op]
                    assert cur==(w["count"],w["units"],w["fingerprint"])
                if op=="stream-keys":
                    assert r["count"]==m["games"] and r["units"]==m["mainline_plies"]
                if rep>=0:samples[mode].append(r)
        row=dict(format=ext,workload=op,threads=n,signature=sig,samples=samples,summary={})
        for mode,rs in samples.items():
            row["summary"][mode]={k:statistics.median(r[k] for r in rs) for k in ("seconds","p50_ns","p95_ns","p99_ns","rss_kib","major_faults","minor_faults")}
        rows.append(row)
        pathlib.Path(report).write_text(json.dumps(dict(tier=tier,cache="warm filesystem; fresh process",rows=rows),indent=2)+"\n")
        print(json.dumps({k:v for k,v in row.items() if k!="samples"}),flush=True)
