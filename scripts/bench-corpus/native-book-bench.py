#!/usr/bin/env python3
"""Strict real-book verification and paired CTG opening/probe measurements."""
import argparse
import json
import pathlib
import statistics
import subprocess
import tempfile
import time

p = argparse.ArgumentParser()
p.add_argument("--book", type=pathlib.Path, required=True)
p.add_argument("--base", type=pathlib.Path, required=True)
p.add_argument("--head", type=pathlib.Path, required=True)
p.add_argument("--base-workload", type=pathlib.Path, required=True)
p.add_argument("--head-workload", type=pathlib.Path, required=True)
p.add_argument("--report", type=pathlib.Path, required=True)
a = p.parse_args()
a.report.parent.mkdir(parents=True, exist_ok=True)
report = {"full_verification": {}, "probes": [], "status": "running"}

def save():
    a.report.write_text(json.dumps(report, indent=2) + "\n")

def run(args):
    with tempfile.TemporaryDirectory() as t:
        rss = pathlib.Path(t) / "rss"
        start = time.perf_counter()
        proc = subprocess.run(
            ["/usr/bin/time", "-f", "%M", "-o", str(rss), *map(str, args)],
            capture_output=True, text=True,
        )
        elapsed = time.perf_counter() - start
        try:
            data = json.loads(proc.stdout)
        except json.JSONDecodeError:
            data = {"stdout": proc.stdout}
        return {"returncode": proc.returncode, "wall_seconds": elapsed,
                "peak_rss_kib": int(rss.read_text().strip().splitlines()[-1]) if rss.exists() else None,
                "data": data, "stderr": proc.stderr}

for side, exe in [("base", a.base), ("head", a.head)]:
    report["full_verification"][side] = run([exe, "book", "verify", a.book, "--children", "--json"])
    save()
full = report["full_verification"]
for side, r in full.items():
    data = r["data"]
    if r["returncode"] != 0 or any(data.get(k, -1) != 0 for k in (
        "damaged_pages", "bad_keys", "invalid_positions", "key_mismatches", "moves_unknown",
        "moves_illegal", "children_missing",
    )) or data.get("entries", 0) == 0 or data.get("key_round_trips") != data.get("entries") or data.get("children_found") != data.get("moves_decoded"):
        report["status"] = "failed structural verification"
        save()
        raise SystemExit(f"strict CTG verification failed for {side}")
keys = ("entries", "key_round_trips", "moves_decoded", "promotions", "children_found")
assert all(full["base"]["data"][k] == full["head"]["data"][k] for k in keys)
expected = None
bins = {"base": a.base_workload, "head": a.head_workload}
for repetition in range(-1, 3):
    order = ("base", "head") if repetition % 2 == 0 else ("head", "base")
    for side in order:
        r = run([bins[side], a.book])
        data = r["data"]
        sig = tuple(data.get(k) for k in ("positions", "pages", "hits", "misses", "children", "fingerprint"))
        if expected is None:
            expected = sig
        assert r["returncode"] == 0 and sig == expected
        assert data["positions"] == full[side]["data"]["entries"]
        if repetition >= 0:
            report["probes"].append({"side": side, "repetition": repetition, **r})
            save()
report["summary"] = {}
for side in bins:
    rows = [r for r in report["probes"] if r["side"] == side]
    report["summary"][side] = {
        k: statistics.median(r["data"][k] for r in rows)
        for k in ("open_seconds", "seconds", "p50_ns", "p95_ns", "p99_ns")
    }
report["status"] = "passed"
save()
print(json.dumps(report["summary"], indent=2))
