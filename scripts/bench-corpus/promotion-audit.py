#!/usr/bin/env python3
"""Independent PGN source/native-export parity and CTG edge oracle (chess 1.11.2)."""
import argparse
import collections
import json
import re
import sys
from pathlib import Path

import chess
import chess.pgn

CASE = re.compile(r"^(Synthetic cbvault fixture|Promotion overflow) (\d+)\b")


def games(path):
    with path.open(encoding="utf-8-sig") as f:
        while game := chess.pgn.read_game(f):
            if game.errors:
                raise ValueError(f"{path}: {game.headers}: {game.errors}")
            yield game


def case_id(game):
    match = CASE.match(game.headers.get("Event", ""))
    if not match:
        raise ValueError(f"missing source case ID: {game.headers}")
    return ("original" if match[1].startswith("Synthetic") else "overflow", int(match[2]))


def tree(node, board):
    # Preserve mainline and alternative order. UCI Chess960 normalizes
    # both standard and 960 castling to the king-to-rook representation.
    result = []
    for child in node.variations:
        move = child.move
        if not board.is_legal(move):
            raise ValueError(f"illegal move {board.fen()} {move}")
        uci = board.uci(move, chess960=True)
        board.push(move)
        result.append((uci, tree(child, board)))
        board.pop()
    return result


def census(path):
    seen = {}
    for g in games(path):
        key = case_id(g)
        if key in seen:
            raise ValueError(f"{path}: duplicate {key}")
        b = g.board()
        if not b.is_valid():
            raise ValueError(f"{path}: invalid starting position {key}: {b.fen()}")
        seen[key] = (b.fen(en_passant="legal").split()[:4], tree(g, b))
    return seen


def audit(source, native):
    expected = census(source)
    actual = census(native)
    missing = sorted(expected.keys() - actual.keys())
    extra = sorted(actual.keys() - expected.keys())
    differences = sorted(k for k in expected.keys() & actual.keys() if expected[k] != actual[k])
    print(json.dumps(dict(expected=len(expected), actual=len(actual), missing=missing,
                          extra=extra, position_or_move_tree_mismatches=differences), indent=2))
    return not (missing or extra or differences)


def edges(source, out):
    # All edges including alternatives, not only moves previously unknown.
    # FEN counters are immaterial to CTG; castling uses X-FEN for 960.
    count = 0
    for g in games(source):
        label = g.headers["Event"].replace("\t", " ")
        stack = [(g, g.board())]
        while stack:
            node, b = stack.pop()
            for child in node.variations:
                m = child.move
                assert b.is_legal(m)
                print(label, b.fen(en_passant="fen"), b.uci(m, chess960=True), sep="\t", file=out)
                count += 1
                nxt = b.copy(stack=False)
                nxt.push(m)
                stack.append((child, nxt))
    print(f"CTG expected edges: {count}", file=sys.stderr)


def main():
    p = argparse.ArgumentParser()
    sub = p.add_subparsers(dest="action", required=True)
    q = sub.add_parser("compare")
    q.add_argument("source", type=Path)
    q.add_argument("native_export", type=Path)
    q = sub.add_parser("edges")
    q.add_argument("source", type=Path)
    a = p.parse_args()
    if a.action == "compare":
        return 0 if audit(a.source, a.native_export) else 1
    edges(a.source, sys.stdout)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
