#!/usr/bin/env python3
"""Paired random-access mapped/unmapped API workloads with pinned fingerprints."""
import argparse,json,os,pathlib,statistics,subprocess,tempfile
p=argparse.ArgumentParser()
for arg in ("tier","fixtures","base","head","report"):p.add_argument("--"+arg,required=True)
a=p.parse_args()
m=json.loads(pathlib.Path(__file__).with_name("native-manifest.json").read_text())["tiers"][a.tier]
bins={"base":a.base,"head":a.head}
report={"tier":a.tier,"status":"running","summary":[]}
path=pathlib.Path(a.report);path.parent.mkdir(parents=True,exist_ok=True)
def save():path.write_text(json.dumps(report,indent=2)+"\n")
def run(exe,db,op,mapped,alloc=False):
    env=dict(os.environ,CBVAULT_NO_MMAP="0" if mapped else "1")
    env.pop("CBVAULT_MMAP",None)
    with tempfile.TemporaryDirectory() as d:
        rss=pathlib.Path(d)/"rss"
        cmd=["/usr/bin/time","-f","%M","-o",str(rss),exe,str(db),str(m["games"]),op,"1"]
        if alloc:cmd.append("--alloc")
        r=subprocess.run(cmd,env=env,text=True,capture_output=True)
        if r.returncode:raise RuntimeError(f"{cmd}: {r.stderr}")
        data=json.loads(r.stdout)
        want=m["workloads"][op]
        assert all(data[k]==want[k] for k in ("count","units","fingerprint")),(op,data,want)
        data["peak_rss_kib"]=int(rss.read_text().strip())
        return data
for ext in ("cbh","2cbh"):
    db=pathlib.Path(a.fixtures)/(m["stem"]+"."+ext)
    for op in ("random-moves","random-pgn","random-pgn-noann"):
        for mapped in (True,False):
            samples={"base":[],"head":[]}
            for rep in range(-1,5):
                for side in (("base","head") if rep%2==0 else ("head","base")):
                    r=run(bins[side],db,op,mapped)
                    if rep>=0:samples[side].append(r)
            row={"format":ext,"workload":op,"mapped":mapped}
            for side in bins:
                row[side]={k:statistics.median(r[k] for r in samples[side]) for k in ("seconds","p50_ns","p95_ns","peak_rss_kib")}
                row[side]["allocation_calls"]=run(bins[side],db,op,mapped,True)["allocation_calls"]
            row["speedup"]=row["base"]["seconds"]/row["head"]["seconds"]
            report["summary"].append(row);save();print(json.dumps(row),flush=True)
report["status"]="passed";save()
