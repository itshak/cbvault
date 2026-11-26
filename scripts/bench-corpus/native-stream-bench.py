#!/usr/bin/env python3
"""Paired keyed-stream, exact position search and cancellation measurements."""
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
report = {"tier": a.tier, "samples": [], "summary": [], "status": "running"}
a.report.parent.mkdir(parents=True, exist_ok=True)


def save():
    a.report.write_text(json.dumps(report, indent=2) + "\n")


def run(exe, db, op, threads, mapped, alloc=False):
    env = dict(os.environ)
    env.pop("CBVAULT_MMAP", None)
    env["CBVAULT_NO_MMAP"] = "0" if mapped else "1"
    with tempfile.TemporaryDirectory() as tmp:
        rss = pathlib.Path(tmp) / "rss"
        args = [str(exe), str(db), str(m["games"]), op, str(threads)]
        if alloc:
            args.append("--alloc")
        r = subprocess.run(["/usr/bin/time", "-f", "%M", "-o", str(rss), *args],
                           env=env, capture_output=True, text=True)
        if r.returncode:
            raise RuntimeError(f"{args}: {r.stderr} {r.stdout}")
        data = json.loads(r.stdout)
        data["peak_rss_kib"] = int(rss.read_text().strip())
        return data


signatures = {}
ops = ["stream-keys", "position-root", "position-e4", "stream-cancel"]
for ext in ("cbh", "2cbh"):
    db = a.fixtures / (m["stem"] + "." + ext)
    for op in ops:
        configs = [(True, n) for n in ((1, 2, 4) if op == "stream-keys" else (1, 4))]
        # Pread performance is measured, not just checked for legality.
        if a.tier == "smoke":
            configs.extend([(False, 1), (False, 4)])
        elif a.tier == "standard" and op == "stream-keys":
            configs.append((False, 4))
        for mapped, threads in configs:
            rows = []
            for rep in range(-1, 3):
                for side in (("base", "head") if rep % 2 == 0 else ("head", "base")):
                    data = run(bins[side], db, op, threads, mapped)
                    if op == "stream-cancel":
                        assert data["count"] >= 1024, data
                        if side == "head" and ext == "2cbh":
                            assert data["count"] == 1024, "v2 delivered past cancellation"
                    else:
                        sig = (data["count"], data["units"], data["fingerprint"])
                        expected = signatures.setdefault(op, sig)
                        assert sig == expected, (ext, op, side, mapped, threads, sig, expected)
                        assert data["units"] == m["mainline_plies"], (op, data)
                        if op == "stream-keys":
                            assert data["count"] == m["games"]
                        else:
                            assert data["count"] > 0
                    if rep >= 0:
                        rows.append(dict(side=side, repetition=rep, **data))
            allocs = {}
            for side in bins:
                data = run(bins[side], db, op, threads, mapped, True)
                if op != "stream-cancel":
                    assert (data["count"], data["units"], data["fingerprint"]) == signatures[op]
                allocs[side] = data["allocation_calls"]
            row = dict(format=ext, workload=op, mapped=mapped, threads=threads)
            for side in bins:
                samples = [r for r in rows if r["side"] == side]
                row[side] = {k: statistics.median(r[k] for r in samples)
                             for k in ("seconds", "count", "cancel_tail_ns", "peak_rss_kib")}
                row[side]["allocation_calls"] = allocs[side]
                row[side]["seconds_range"] = [min(r["seconds"] for r in samples),
                                                max(r["seconds"] for r in samples)]
            row["speedup"] = row["base"]["seconds"] / max(row["head"]["seconds"], 1e-12)
            report["summary"].append(row)
            report["samples"].append(dict(format=ext, workload=op, mapped=mapped, threads=threads, rows=rows))
            save()
            print(json.dumps(row), flush=True)
report["signatures"] = signatures
report["status"] = "passed"
save()
