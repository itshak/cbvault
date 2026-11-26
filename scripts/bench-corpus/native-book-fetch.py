#!/usr/bin/env python3
"""Download and verify the separately packaged, multipart CTG fixture."""
import argparse
import hashlib
import json
import pathlib
import shutil
import subprocess
import tarfile
import tempfile

parser = argparse.ArgumentParser()
parser.add_argument("manifest", type=pathlib.Path)
parser.add_argument("output", type=pathlib.Path)
args = parser.parse_args()
manifest = json.loads(args.manifest.read_text())
members = manifest["members"]
args.output.mkdir(parents=True, exist_ok=False)
need = sum(p["bytes"] for p in manifest["parts"]) + sum(p["bytes"] for p in members.values())
if shutil.disk_usage(args.output).free < need + 512 * 1024**2:
    raise RuntimeError("insufficient disk for verified CTG fixture")
with tempfile.TemporaryDirectory(dir=args.output.parent) as scratch:
    paths = []
    combined = hashlib.sha256()
    for i, part in enumerate(manifest["parts"]):
        path = pathlib.Path(scratch) / f"part-{i:02}"
        subprocess.run(["curl", "--fail", "--location", "--retry", "3", part["url"], "-o", str(path)], check=True)
        assert path.stat().st_size == part["bytes"], "part size mismatch"
        h = hashlib.sha256()
        with path.open("rb") as f:
            while chunk := f.read(1024**2):
                h.update(chunk)
                combined.update(chunk)
        assert h.hexdigest() == part["sha256"], "part hash mismatch"
        paths.append(str(path))
    assert combined.hexdigest() == manifest["archive_sha256"], "combined hash mismatch"
    cat = subprocess.Popen(["cat", *paths], stdout=subprocess.PIPE)
    zstd = subprocess.Popen(["zstd", "-dc"], stdin=cat.stdout, stdout=subprocess.PIPE)
    cat.stdout.close()
    found = set()
    try:
        with tarfile.open(fileobj=zstd.stdout, mode="r|") as tar:
            for m in tar:
                assert m.isfile() and m.name in members and m.name not in found, "unexpected archive member"
                assert pathlib.Path(m.name).name == m.name
                assert m.size == members[m.name]["bytes"]
                h = hashlib.sha256()
                with tar.extractfile(m) as src, (args.output / m.name).open("xb") as dst:
                    while chunk := src.read(1024**2):
                        h.update(chunk)
                        dst.write(chunk)
                assert h.hexdigest() == members[m.name]["sha256"], "native member hash mismatch"
                found.add(m.name)
        # Drain the compressed stream to its checksum trailer.
        while zstd.stdout.read(1024**2):
            pass
    finally:
        zstd.stdout.close()
        rc1 = zstd.wait()
        rc2 = cat.wait()
    assert rc1 == rc2 == 0 and found == set(members), "incomplete book"
print(json.dumps({"verified_members": sorted(found), "archive_sha256": manifest["archive_sha256"]}))
