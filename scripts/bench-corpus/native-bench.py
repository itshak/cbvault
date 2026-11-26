#!/usr/bin/env python3
"""Paired native corpus gate. Timings are observations, not pass/fail thresholds."""
import argparse
import hashlib
import json
import pathlib
import statistics
import subprocess
import tempfile
import time

HERE = pathlib.Path(__file__).resolve().parent
MANIFEST = HERE / "native-manifest.json"

def checked_json(cmd, stderr=False):
    t = time.perf_counter()
    p = subprocess.run([str(x) for x in cmd], capture_output=True, text=True)
    if p.returncode:
        raise RuntimeError(f"{cmd}: {p.stdout} {p.stderr}")
    return json.loads(p.stderr if stderr else p.stdout), time.perf_counter() - t

def require(ok, message):
    if not ok:
        raise ValueError(message)

def main():
    p = argparse.ArgumentParser()
    p.add_argument("--tier", required=True)
    p.add_argument("--fixtures", type=pathlib.Path, required=True)
    p.add_argument("--base", type=pathlib.Path, required=True)
    p.add_argument("--head", type=pathlib.Path, required=True)
    p.add_argument("--report", type=pathlib.Path, required=True)
    p.add_argument("--repetitions", type=int, default=5)
    a = p.parse_args()
    require(a.repetitions > 0, "positive repetitions required")
    m = json.loads(MANIFEST.read_text())["tiers"][a.tier]
    bins = {"base": a.base.resolve(), "head": a.head.resolve()}
    results = []
    with tempfile.TemporaryDirectory() as temp:
        out = pathlib.Path(temp) / "out.pgn"
        for ext in ["cbh", "2cbh"]:
            db = a.fixtures / (m["stem"] + "." + ext)
            # Verify source counts and moves independently of baseline output.
            for side, exe in bins.items():
                info, _ = checked_json([exe, "info", db, "--json"])
                require(info["records"] == m["slots"][ext], f"{side}/{ext}: slot count")
                for threads, flags in [(1, []), (2, []), (1, ["--no-mmap"])]:
                    args = ["--threads", str(threads), *flags, "--json"]
                    v, _ = checked_json([exe, "verify", db, *args])
                    for k, expected in {"games": m["games"], "main_line_plies": m["mainline_plies"],
                                         "all_plies": m["all_plies"], "failed": 0,
                                         "unknown_kind": 0, "deleted": 0, "guiding_texts": 0}.items():
                        require(v[k] == expected, f"{side}/{ext}: {k}={v[k]} expected {expected}")
                    e, _ = checked_json([exe, "pgn", db, out, *args], stderr=True)
                    require(e["games"] == m["games"] and e["failures"] == 0, f"{side}/{ext}: export")
                    with out.open("rb") as f:
                        h = hashlib.file_digest(f, "sha256").hexdigest()
                    require(h == m["export_sha256"][ext], f"{side}/{ext}: output bytes changed")
                    out.unlink()
            # Warm-up and alternate order on the same runner, separately by generation.
            for repetition in range(-1, a.repetitions):
                order = ["base", "head"] if repetition % 2 == 0 else ["head", "base"]
                for side in order:
                    for workload in ["verify", "pgn"]:
                        args = [bins[side], workload, db]
                        if workload == "pgn":
                            args += [out]
                        args += ["--threads", "2", "--json"]
                        v, wall = checked_json(args, stderr=workload == "pgn")
                        require(v["games"] == m["games"] and v.get("failed", v.get("failures")) == 0,
                                f"{side}/{ext}: {workload} failure")
                        if workload == "pgn":
                            with out.open("rb") as f:
                                h = hashlib.file_digest(f, "sha256").hexdigest()
                            require(h == m["export_sha256"][ext], f"{side}/{ext}: timed export changed")
                            out.unlink()
                        if repetition >= 0:
                            results.append(dict(side=side, format=ext, workload=workload,
                                                repetition=repetition, wall_seconds=wall, cli=v))
    summary = []
    for ext in ["cbh", "2cbh"]:
        for workload in ["verify", "pgn"]:
            t = {side: [r["wall_seconds"] for r in results if r["side"] == side and
                        r["format"] == ext and r["workload"] == workload] for side in bins}
            summary.append(dict(format=ext, workload=workload,
                                base_median=statistics.median(t["base"]),
                                head_median=statistics.median(t["head"]),
                                speedup=statistics.median(t["base"])/statistics.median(t["head"]),
                                base_range=[min(t["base"]), max(t["base"])],
                                head_range=[min(t["head"]), max(t["head"])]))
    a.report.parent.mkdir(parents=True, exist_ok=True)
    a.report.write_text(json.dumps(dict(tier=a.tier, archive_sha256=m["sha256"],
                                      private_gold="NOT RUN", results=results, summary=summary), indent=2))
    print(json.dumps(summary, indent=2))

if __name__ == "__main__":
    main()
