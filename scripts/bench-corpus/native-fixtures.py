#!/usr/bin/env python3
"""Verify pinned archives and extract only regular, explicitly named fixture members."""
import argparse
import hashlib
import json
import pathlib
import re
import shutil
import subprocess
import tarfile

EXTENSIONS = set("cbh cbg cba cbp cbt cbc cbs cbj cbe cbl cbm cbtt cib cib2 cit cit2 2cbh 2cbg 2cba 2lid 2lgd 2lcd ctg cto ctb pgn".split())
REQUIRED = {"cbh", "cbg", "cba", "cbp", "cbt", "2cbh", "2cbg", "2cba", "2lid", "2lgd", "2lcd"}

def digest(path):
    with path.open("rb") as f:
        return hashlib.file_digest(f, "sha256").hexdigest()

def extract(archive, expected, stem, dest):
    if not re.fullmatch(r"[a-z0-9-]+", stem):
        raise ValueError("invalid fixture name")
    if digest(archive) != expected:
        raise ValueError(f"archive checksum mismatch: {archive}")
    dest.mkdir(parents=True, exist_ok=False)
    proc = subprocess.Popen(["zstd", "-dc", "--", str(archive)], stdout=subprocess.PIPE)
    members = {}
    try:
        with tarfile.open(fileobj=proc.stdout, mode="r|") as tar:
            for member in tar:
                name = member.name
                if not member.isfile() or "/" in name or "\\" in name:
                    raise ValueError(f"unsafe member: {name}")
                if name.startswith("._") or name == stem + ".ini":
                    continue
                # Only the fixture's known companion files may enter the public corpus.
                prefix, sep, ext = name.rpartition(".")
                if not sep or prefix != stem or ext.lower() not in EXTENSIONS:
                    raise ValueError(f"unexpected member: {name}")
                if name in members:
                    raise ValueError(f"duplicate member: {name}")
                if member.size > shutil.disk_usage(dest).free - 256 * 1024**2:
                    raise ValueError("insufficient extraction space")
                path = dest / name
                with tar.extractfile(member) as src, path.open("xb") as out:
                    shutil.copyfileobj(src, out)
                members[name] = {"bytes": path.stat().st_size, "sha256": digest(path)}
        if proc.wait() != 0:
            raise ValueError("zstd decompression failed")
        if not REQUIRED <= {n.rpartition(".")[2] for n in members}:
            raise ValueError("missing required native database companions")
        ctg = {n.rpartition(".")[2] for n in members} & {"ctg", "cto", "ctb"}
        if ctg and ctg != {"ctg", "cto", "ctb"}:
            raise ValueError("incomplete CTG family")
        return members
    except BaseException:
        proc.kill()
        proc.wait()
        shutil.rmtree(dest)
        raise
    finally:
        proc.stdout.close()

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("archive", type=pathlib.Path)
    parser.add_argument("sha256")
    parser.add_argument("stem")
    parser.add_argument("dest", type=pathlib.Path)
    args = parser.parse_args()
    print(json.dumps(extract(args.archive, args.sha256, args.stem, args.dest), indent=2, sort_keys=True))

if __name__ == "__main__":
    main()
