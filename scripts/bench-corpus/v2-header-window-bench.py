#!/usr/bin/env python3
"""Paired 2CBH named scan, replay and conversion, mapped and positioned."""
import json, os, pathlib, statistics, subprocess, sys
tier, root, base, head, report = sys.argv[1:]
m = json.loads(pathlib.Path(__file__).with_name("native-manifest.json").read_text())["tiers"][tier]
db = str(pathlib.Path(root) / (m["stem"] + ".2cbh"))
rows = []
signatures = {}
for mapped in [True, False]:
    env = dict(os.environ, CBVAULT_NO_MMAP="0" if mapped else "1")
    env.pop("CBVAULT_MMAP", None)
    for threads in [1, 4]:
        for op in ["named-broad", "named-selective", "position-root", "position-e4", "stream-keys"]:
            if op == "stream-keys" and threads == 1:
                continue
            samples = {"base": [], "head": []}
            for rep in range(-1, 3):
                for side in (["base", "head"] if rep % 2 == 0 else ["head", "base"]):
                    directory = pathlib.Path(base if side == "base" else head)
                    if op.startswith("named-"):
                        cmd = [str(directory / "header_window_workload"), db, str(threads), op[6:]]
                    else:
                        cmd = [str(directory / "native_workload"), db, str(m["games"]), op, str(threads)]
                    r = json.loads(subprocess.check_output(cmd, env=env, text=True))
                    sig = tuple(r[k] for k in ("count", "units", "fingerprint"))
                    assert sig == signatures.setdefault(op, sig), (op, sig, signatures[op])
                    assert r["units"] == (m["games"] if op.startswith("named-") else m["mainline_plies"])
                    if rep >= 0:
                        samples[side].append(r["seconds"])
            row = dict(op=op, mapped=mapped, threads=threads, samples=samples,
                       base=statistics.median(samples["base"]), head=statistics.median(samples["head"]))
            row["speedup"] = row["base"] / row["head"]
            rows.append(row)
            pathlib.Path(report).write_text(json.dumps(dict(tier=tier, summary=rows, signatures=signatures), indent=2) + "\n")
            print(json.dumps(row), flush=True)
