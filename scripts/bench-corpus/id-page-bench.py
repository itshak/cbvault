#!/usr/bin/env python3
"""Exact count/page parity, allocation-independent RSS and scan time."""
import json, os, pathlib, statistics, subprocess, sys, tempfile
tier, root, exe, report = sys.argv[1:]
m = json.loads(pathlib.Path(__file__).with_name("native-manifest.json").read_text())["tiers"][tier]
rows = []
for ext in ["cbh", "2cbh"]:
    db = str(pathlib.Path(root) / (m["stem"] + "." + ext))
    for mapped in [True, False]:
        env = dict(os.environ, CBVAULT_NO_MMAP="0" if mapped else "1")
        env.pop("CBVAULT_MMAP", None)
        for threads in [1, 4]:
            for filt in ["all", "broad", "selective"]:
                for offset in [0, m["games"] // 2, m["games"] - 30]:
                    samples = {"full": [], "page": []}
                    sig = None
                    for rep in range(-1, 3):
                        for mode in (["full", "page"] if rep % 2 == 0 else ["page", "full"]):
                            with tempfile.TemporaryDirectory() as tmp:
                                rss = pathlib.Path(tmp) / "rss"
                                cmd = ["/usr/bin/time", "-f", "%M", "-o", str(rss), exe, db, str(threads), filt, str(offset), "100", mode]
                                r = json.loads(subprocess.check_output(cmd, env=env, text=True))
                                r["peak_rss_kib"] = int(rss.read_text())
                            current = tuple(r[k] for k in ("count", "total", "records", "fingerprint"))
                            if sig is None:
                                sig = current
                            assert sig == current
                            assert r["records"] == m["games"]
                            assert r["count"] == min(100, max(0, r["total"] - offset))
                            if rep >= 0:
                                samples[mode].append(r)
                    row = dict(format=ext, mapped=mapped, threads=threads, filter=filt, offset=offset, signature=sig)
                    for mode, rs in samples.items():
                        row[mode] = {k: statistics.median(r[k] for r in rs) for k in ["seconds", "peak_rss_kib"]}
                    row["speedup"] = row["full"]["seconds"] / row["page"]["seconds"]
                    rows.append(row)
                    pathlib.Path(report).write_text(json.dumps(dict(tier=tier, summary=rows), indent=2) + "\n")
                    print(json.dumps(row), flush=True)
