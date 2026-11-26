#!/usr/bin/env python3
"""Reachable CTG move-ordinal coverage from eight real promotion histories."""
import argparse
import collections
import hashlib
import json
from pathlib import Path
import random
import chess
import chess.pgn

ROLES = {chess.QUEEN: 9, chess.ROOK: 10, chess.BISHOP: 10, chess.KNIGHT: 10}
SEED = 20261127


def norm_square(square, flip, mirror):
    f, r = chess.square_file(square), chess.square_rank(square)
    return (7-f if mirror else f, 7-r if flip else r)


def cases(board, role, color, prefix, found):
    if board.turn != color or board.is_game_over(claim_draw=False):
        return
    pieces = board.pieces(role, color)
    count = len(pieces)
    if count < 3:
        return
    flip = color == chess.BLACK
    mirror = not board.castling_rights and chess.square_file(board.king(color)) < 4
    squares = sorted(pieces, key=lambda s: norm_square(s, flip, mirror))
    ordinal = {s: i+1 for i, s in enumerate(squares)}
    for m in board.legal_moves:
        if m.from_square not in ordinal:
            continue
        a, b = norm_square(m.from_square, flip, mirror), norm_square(m.to_square, flip, mirror)
        k = (chess.piece_name(role), "white" if color else "black", count,
             ordinal[m.from_square], (b[0]-a[0])%8, (b[1]-a[1])%8)
        if k not in found or len(prefix)+1 < len(found[k]):
            found[k] = [*prefix, m]


def main():
    p = argparse.ArgumentParser()
    p.add_argument("standard", type=Path)
    p.add_argument("output", type=Path)
    p.add_argument("manifest", type=Path)
    a = p.parse_args()
    rng = random.Random(SEED)
    found = {}
    with a.standard.open() as f:
        while g := chess.pgn.read_game(f):
            assert not g.errors
            color = "white standard-start" in g.headers["Event"]
            role = next(r for r in ROLES if f"eight {chess.piece_name(r)} promotions" in g.headers["Event"])
            b = chess.Board()
            prefix = []
            for m in g.mainline_moves():
                cases(b, role, color, prefix, found)
                assert b.is_legal(m)
                b.push(m)
                prefix.append(m)
            assert len(b.pieces(role, color)) == ROLES[role]
            # Explore cooperative continuations from every maximum-material
            # position; do not allow captures of any target role piece.
            for _ in range(16):
                x = b.copy()
                path = prefix[:]
                for _ in range(80):
                    if x.is_game_over(claim_draw=False):
                        break
                    cases(x, role, color, path, found)
                    choices = list(x.legal_moves)
                    rng.shuffle(choices)
                    chosen = None
                    for m in choices:
                        x.push(m)
                        keep = len(x.pieces(role,color)) == ROLES[role] and not x.is_game_over(claim_draw=False)
                        x.pop()
                        if keep:
                            chosen = m
                            break
                    if chosen is None:
                        break
                    x.push(chosen)
                    path.append(chosen)
    counts = collections.Counter()
    plies = 0
    with a.output.open("w", encoding="utf-8", newline="\n") as out:
        for idx, (key, moves) in enumerate(sorted(found.items()), 100001):
            role,color,count,ordinal,df,dr = key
            g = chess.pgn.Game()
            g.headers.update({"Event":f"Promotion overflow {idx}: {color} {role} {count} ordinal {ordinal} delta {df},{dr}",
                              "Site":"CC0 synthetic standard start", "Date":"2026.??.??", "Round":"1",
                              "White":"Fixture White", "Black":"Fixture Black"})
            b = chess.Board()
            node = g
            for m in moves:
                assert not b.is_game_over(claim_draw=False) and b.is_legal(m)
                node = node.add_main_variation(m)
                b.push(m)
            g.headers["Result"] = b.result(claim_draw=False) if b.is_game_over(claim_draw=False) else "1/2-1/2"
            out.write(g.accept(chess.pgn.StringExporter(headers=True,variations=False,comments=False))+"\n\n")
            counts[f"{color}:{role}"]+=1
            plies += len(moves)
    report = {"seed":SEED,"games":len(found),"plies":plies,"coverage":dict(counts),
              "matrix_target":5908,"matrix_reached":len(found),
              "sha256":hashlib.sha256(a.output.read_bytes()).hexdigest(),
              "notes":"All games from standard initial position. One legal mainline per reachable count/ordinal/delta, not a claim that all 5908 setup cells are reached. Full move depth required for CTG."}
    a.manifest.write_text(json.dumps(report,indent=2)+"\n")
    print(json.dumps(report,indent=2))

if __name__=="__main__":main()
