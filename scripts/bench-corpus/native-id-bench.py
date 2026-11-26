#!/usr/bin/env python3
"""ID-only metadata search versus the old materialized named scan."""
import argparse
import json
import os
import pathlib
import statistics
import subprocess
import tempfile

p = argparse.ArgumentParser()
p.add_argument("--tier", required=True)
p.add_argument("--fixtures", type=pathlib.Path, required=True)
p.add_argument("--base", type=pathlib.Path, required=True)
p.add_argument("--head", type=pathlib.Path, required=True)
p.add_argument("--report", type=pathlib.Path, required=True)
a = p.parse_args()
m = json.loads(pathlib.Path(__file__).with_name("native-manifest.json").read_text())["tiers"][a.tier]
bins = {"base": a.base.resolve(), "head": a.head.resolve()}
report = {"tier": a.tier, "status": "running", "summary": [], "samples": []}
a.report.parent.mkdir(parents=True, exist_ok=True)
expected = {k: (v["count"], v["units"], v["fingerprint"]) for k, v in m["workloads"].items()}
def save():
    a.report.write_text(json.dumps(report, indent=2) + "\n")
def run(exe, db, op, threads, mapped, alloc=False):
    env = dict(os.environ, CBVAULT_NO_MMAP="0" if mapped else "1")
    env.pop("CBVAULT_MMAP", None)
    with tempfile.TemporaryDirectory() as d:
        rss = pathlib.Path(d) / "rss"
        args = [str(exe), str(db), op, str(threads)]
        if alloc:
            args.append("--alloc")
        data = json.loads(subprocess.check_output(["/usr/bin/time","-f","%M","-o",str(rss),*args],env=env,text=True))
        data["peak_rss_kib"] = int(rss.read_text().strip())
        sig = tuple(data[k] for k in ("count","units","fingerprint"))
        assert sig == expected.setdefault(op,sig), (op,sig,expected[op])
        if op == "all":
            assert data["count"] == m["games"]
        return data
for ext in ("cbh","2cbh"):
    db = a.fixtures / (m["stem"]+"."+ext)
    for op in ("all","broad","selective"):
        for mapped in (True,False):
            for n in (1,2,4):
                # The old borrowed classic name API cannot run without mmap.
                # Check candidate against the exact mapped oracle, not a fake speedup.
                active = {"head": bins["head"]} if ext == "cbh" and not mapped else bins
                rows = []
                for rep in range(-1,5):
                    for side in (("base","head") if rep%2==0 else ("head","base")):
                        if side not in active: continue
                        data = run(bins[side],db,op,n,mapped)
                        if rep>=0:rows.append(dict(side=side,repetition=rep,**data))
                row = dict(format=ext,workload=op,mapped=mapped,threads=n)
                for side,exe in active.items():
                    samples = [r for r in rows if r["side"]==side]
                    row[side] = {k:statistics.median(r[k] for r in samples) for k in ("seconds","peak_rss_kib")}
                    row[side]["allocation_calls"] = run(exe,db,op,n,mapped,True)["allocation_calls"]
                    row[side]["seconds_range"] = [min(r["seconds"] for r in samples),max(r["seconds"] for r in samples)]
                if "base" in row:
                    row["speedup"]=row["base"]["seconds"]/max(row["head"]["seconds"],1e-12)
                else:
                    row["base"] = {"status": "unsupported: classic borrowed names require mmap"}
                    row["speedup"] = None
                report["summary"].append(row)
                report["samples"].append(dict(format=ext,workload=op,mapped=mapped,threads=n,rows=rows))
                save()
                print(json.dumps(row),flush=True)
report["status"]="passed"
save()
