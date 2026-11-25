#!/usr/bin/env python3
"""Extract complete game prefixes from generator output (Event header per game).
Only use with this generator's output, not arbitrary third-party PGN.
"""
import sys
limit=int(sys.argv[1])
if limit<1: raise SystemExit("positive count required")
n=0
for line in sys.stdin.buffer:
    if line.startswith(b"[Event "):
        if n==limit: break
        n+=1
    if not n: raise SystemExit("missing initial Event header")
    sys.stdout.buffer.write(line)
if n!=limit: raise SystemExit(f"expected {limit}, got {n}")
