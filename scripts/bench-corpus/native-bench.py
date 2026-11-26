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
    with tempfile.TemporaryDirectory() as tmp:
        rss = pathlib.Path(tmp) / "rss"
        args = [str(x) for x in cmd]
        if pathlib.Path("/usr/bin/time").is_file():
            args = ["/usr/bin/time", "-f", "%M", "-o", str(rss), *args]
        p = subprocess.run(args, capture_output=True, text=True)
        wall = time.perf_counter() - t
        if p.returncode:
            raise RuntimeError(f"{cmd}: {p.stdout} {p.stderr}")
        report = json.loads(p.stderr if stderr else p.stdout)
        report["peak_rss_kib"] = int(rss.read_text().strip()) if rss.exists() else None
        return report, wall

def require(ok, message):
    if not ok:
        raise ValueError(message)

def extra_workloads(a, m):
    """Paired API workloads; allocation passes are separate from timing."""
    bins = {"base": a.base_workload.resolve(), "head": a.head_workload.resolve()}
    ops = ("headers", "broad", "selective", "random-moves", "random-pgn", "random-pgn-noann")
    results = []
    fingerprints = {op: (v["count"], v["units"], v["fingerprint"])
                    for op, v in m["workloads"].items()}
    for ext in ("cbh", "2cbh"):
        db = a.fixtures / (m["stem"] + "." + ext)
        for op in ops:
            threads = (1, 2, 4) if op in ("broad", "selective") else (1,)
            for n in threads:
                for repetition in range(-1, a.repetitions):
                    order = ("base", "head") if repetition % 2 == 0 else ("head", "base")
                    for side in order:
                        args = [bins[side], db, m["games"], op, n]
                        v, wall = checked_json(args)
                        signature = (v["count"], v["units"], v["fingerprint"])
                        # A single expected result across both generations, every
                        # worker configuration and both revisions.
                        want = fingerprints[op]
                        require(want == signature, f"{side}/{ext}/{op}/{n}: {signature} != {want}")
                        if op == "headers":
                            require(v["count"] == m["games"], "header count differs from corpus")
                        if op.startswith("random-"):
                            require(v["count"] == 2048, "random sample incomplete")
                        if repetition >= 0:
                            results.append(dict(side=side, format=ext, workload=op,
                                                threads=n, repetition=repetition,
                                                wall_seconds=wall, metrics=v, allocation_pass=False))
                for side, exe in bins.items():
                    v, wall = checked_json([exe, db, m["games"], op, n, "--alloc"])
                    require((v["count"], v["units"], v["fingerprint"]) == fingerprints[op],
                            f"{side}/{ext}/{op}: allocation pass changed output")
                    results.append(dict(side=side, format=ext, workload=op,
                                        threads=n, repetition=None, wall_seconds=wall,
                                        metrics=v, allocation_pass=True))
    summary = []
    for ext in ("cbh", "2cbh"):
        for op in ops:
            for n in ((1, 2, 4) if op in ("broad", "selective") else (1,)):
                row = dict(format=ext, workload=op, threads=n)
                for side in bins:
                    matching = [r for r in results if r["side"] == side and r["format"] == ext
                                and r["workload"] == op and r["threads"] == n]
                    timed = [r["metrics"] for r in matching if not r["allocation_pass"]]
                    rss = [r["peak_rss_kib"] for r in timed if r["peak_rss_kib"] is not None]
                    row[side] = dict(median_seconds=statistics.median(r["seconds"] for r in timed),
                                     p95_ns=statistics.median(r["p95_ns"] for r in timed),
                                     peak_rss_kib=max(rss, default=None),
                                     allocation_calls=next(r["metrics"]["allocation_calls"] for r in matching
                                                           if r["allocation_pass"]))
                row["speedup"] = row["base"]["median_seconds"] / max(row["head"]["median_seconds"], 1e-12)
                summary.append(row)
    return dict(samples=results, summary=summary)


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--tier", required=True)
    p.add_argument("--fixtures", type=pathlib.Path, required=True)
    p.add_argument("--base", type=pathlib.Path, required=True)
    p.add_argument("--head", type=pathlib.Path, required=True)
    p.add_argument("--report", type=pathlib.Path, required=True)
    p.add_argument("--base-workload", type=pathlib.Path)
    p.add_argument("--head-workload", type=pathlib.Path)
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
                for threads, flags in [(1, []), (2, []), (4, []), (1, ["--no-mmap"])]:
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
    workloads = []
    if a.base_workload or a.head_workload:
        require(a.base_workload and a.head_workload, "both workload binaries are required")
        workloads = extra_workloads(a, m)
    a.report.parent.mkdir(parents=True, exist_ok=True)
    a.report.write_text(json.dumps(dict(tier=a.tier, archive_sha256=m["sha256"],
                                      private_gold="NOT RUN", results=results, summary=summary, workloads=workloads), indent=2))
    print(json.dumps(summary, indent=2))

if __name__ == "__main__":
    main()
