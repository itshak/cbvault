#!/usr/bin/env python3
"""CC0 promotion overflow corpus. Requires chess==1.11.2; no private inputs."""
import argparse
import collections
import hashlib
import json
import random
from pathlib import Path
import chess
import chess.pgn
from importlib import import_module
standard = import_module("promotion-standard")

SEED = 20261126
ROLES = {chess.QUEEN: 9, chess.ROOK: 10, chess.BISHOP: 10, chess.KNIGHT: 10}
ORDER = sorted(chess.SQUARES, key=lambda s: (chess.square_file(s), chess.square_rank(s)))
INDEX = {s:i for i,s in enumerate(ORDER)}
rng = random.Random(SEED)

def delta(a,b):
    return ((chess.square_file(b)-chess.square_file(a))%8,
            (chess.square_rank(b)-chess.square_rank(a))%8)

def role_pairs(role):
    out = collections.defaultdict(list)
    for a in ORDER:
        b=chess.Board.empty()
        b.set_piece_at(a,chess.Piece(role,chess.WHITE))
        for t in b.attacks(a):
            out[delta(a,t)].append((a,t))
    return out

def white_board(role, count, ordinal, pairs):
    # No castling; the white king stays on the e-h files to prevent CTG's
    # normalization from mirroring the specified file-major ordinal.
    candidates=[(a,t) for a,t in pairs if INDEX[a]>=ordinal and
                63-INDEX[a]>=count-ordinal-1]
    for _ in range(3000):
        a,t=rng.choice(candidates)
        ray=set(chess.SquareSet(chess.between(a,t)))
        forbidden=ray|{a,t}
        lo=[s for s in ORDER[:INDEX[a]] if s not in forbidden]
        hi=[s for s in ORDER[INDEX[a]+1:] if s not in forbidden]
        if len(lo)<ordinal or len(hi)<count-ordinal-1: continue
        pieces=[a]+rng.sample(lo,ordinal)+rng.sample(hi,count-ordinal-1)
        if role==chess.BISHOP and count==10 and len({(chess.square_file(s)+chess.square_rank(s))%2 for s in pieces})<2: continue
        b=chess.Board.empty()
        for s in pieces:b.set_piece_at(s,chess.Piece(role,chess.WHITE))
        white_kings=[s for s in ORDER if chess.square_file(s)>=4 and s not in pieces and s not in forbidden]
        rng.shuffle(white_kings)
        for wk in white_kings[:4]:
            b.set_piece_at(wk,chess.Piece(chess.KING,chess.WHITE))
            safe=[s for s in ORDER if not b.piece_at(s) and s not in forbidden and not b.is_attacked_by(chess.WHITE,s)]
            rng.shuffle(safe)
            for bk in safe[:3]:
                b.set_piece_at(bk,chess.Piece(chess.KING,chess.BLACK))
                move=chess.Move(a,t)
                if b.is_valid() and b.is_legal(move):
                    return b.copy(stack=False),move
                b.remove_piece_at(bk)
            b.remove_piece_at(wk)
    raise RuntimeError(f"cannot generate {role=} {count=} {ordinal=} {pairs[0]=}")

def promotion_board(role,count,capture):
    # count is the number of pieces AFTER promotion.
    for _ in range(5000):
        f=rng.randrange(8)
        a=chess.square(f,6)
        tf=rng.choice([i for i in (f-1,f+1) if 0<=i<8]) if capture else f
        t=chess.square(tf,7)
        pieces=rng.sample([s for s in ORDER if s not in (a,t)],count-1)
        b=chess.Board.empty()
        for s in pieces:b.set_piece_at(s,chess.Piece(role,chess.WHITE))
        b.set_piece_at(a,chess.Piece(chess.PAWN,chess.WHITE))
        if capture:b.set_piece_at(t,chess.Piece(chess.ROOK,chess.BLACK))
        for wk in rng.sample([s for s in ORDER if not b.piece_at(s) and chess.square_file(s)>=4],4):
            b.set_piece_at(wk,chess.Piece(chess.KING,chess.WHITE))
            safe=[s for s in ORDER if not b.piece_at(s) and not b.is_attacked_by(chess.WHITE,s)]
            for bk in rng.sample(safe,min(4,len(safe))):
                b.set_piece_at(bk,chess.Piece(chess.KING,chess.BLACK))
                m=chess.Move(a,t,promotion=role)
                if b.is_valid() and b.is_legal(m):
                    next=b.copy(stack=False);next.push(m)
                    if role!=chess.BISHOP or count!=10 or len({(chess.square_file(s)+chess.square_rank(s))%2 for s in next.pieces(role,chess.WHITE)})==2:
                        # Require a legal reply and a subsequent move of the newly promoted piece.
                        replies=list(next.legal_moves);rng.shuffle(replies)
                        for reply in replies:
                            after=next.copy(stack=False);after.push(reply)
                            follow=[mv for mv in after.legal_moves if mv.from_square==t]
                            if follow:return b.copy(stack=False),[m,reply,sorted(follow,key=lambda x:x.uci())[0]]
                b.remove_piece_at(bk)
            b.remove_piece_at(wk)
    raise RuntimeError(f"cannot generate promotion {role=} {count=} {capture=}")

def capture_board(role,count,ordinal):
    # Opponent captures the chosen file-major piece; the survivor with the
    # highest remaining ordinal then moves. Both colored versions are emitted.
    for _ in range(5000):
        pieces=rng.sample(ORDER,count)
        if role==chess.BISHOP and count==10 and len({(chess.square_file(s)+chess.square_rank(s))%2 for s in pieces})<2:continue
        pieces.sort(key=INDEX.get)
        t=pieces[ordinal]
        b=chess.Board.empty()
        for s in pieces:b.set_piece_at(s,chess.Piece(role,chess.WHITE))
        b.turn=chess.BLACK
        # Use a black knight for capture, independent of the captured role.
        probe=chess.Board.empty();probe.set_piece_at(t,chess.Piece(chess.KNIGHT,chess.BLACK))
        source=[s for s in probe.attacks(t) if s not in pieces]
        if not source:continue
        a=rng.choice(source);b.set_piece_at(a,chess.Piece(chess.KNIGHT,chess.BLACK))
        for wk in rng.sample([s for s in ORDER if not b.piece_at(s) and chess.square_file(s)>=4 and not b.is_attacked_by(chess.BLACK,s)],4):
            b.set_piece_at(wk,chess.Piece(chess.KING,chess.WHITE))
            safe=[s for s in ORDER if not b.piece_at(s) and not b.is_attacked_by(chess.WHITE,s) and chess.square_distance(s,wk)>1]
            for bk in rng.sample(safe,min(4,len(safe))):
                b.set_piece_at(bk,chess.Piece(chess.KING,chess.BLACK))
                m=chess.Move(a,t)
                if b.is_valid() and b.is_legal(m):
                    after=b.copy(stack=False);after.push(m)
                    last=max(after.pieces(role,chess.WHITE),key=INDEX.get)
                    follow=[mv for mv in after.legal_moves if mv.from_square==last]
                    if follow:return b.copy(stack=False),[m,sorted(follow,key=lambda x:x.uci())[0]]
                b.remove_piece_at(bk)
            b.remove_piece_at(wk)
    raise RuntimeError(f"capture setup failed {role=} {count=} {ordinal=}")

def transform(b,moves,black):
    if not black:return b,moves
    return b.mirror(),[chess.Move(chess.square_mirror(m.from_square),chess.square_mirror(m.to_square),promotion=m.promotion) for m in moves]

def emit(out,b,moves,label,idx,alternatives=False):
    assert b.is_valid(),b.fen()
    game=chess.pgn.Game()
    if b.fen()!=chess.STARTING_FEN:game.setup(b)
    game.headers.update({"Event":f"Promotion overflow {idx}: {label}","Site":"Synthetic CC0 fixture laboratory","Date":"2026.??.??","Round":str(idx),"White":"Fixture White","Black":"Fixture Black","Result":"*"})
    node=game
    for move in moves:
        assert b.is_legal(move),(label,b.fen(),move)
        node=node.add_main_variation(move)
        b.push(move)
    if alternatives:
        start=game.board()
        for alt in sorted(start.legal_moves,key=lambda m:m.uci()):
            if alt!=moves[0] and alt.promotion:
                game.add_variation(alt)
    out.write(game.accept(chess.pgn.StringExporter(headers=True,variations=True,comments=True))+"\n\n")

def main():
    ap=argparse.ArgumentParser()
    ap.add_argument("original",type=Path);ap.add_argument("output",type=Path)
    ap.add_argument("manifest",type=Path);ap.add_argument("--ctg-standard",type=Path);a=ap.parse_args()
    original=a.original.read_text()
    counts=collections.Counter();coverage=[];idx=0
    with a.output.open("w",encoding="utf-8",newline="\n") as out:
        out.write(original.rstrip()+"\n\n")
        for role,maximum in ROLES.items():
            name=chess.piece_name(role)
            # Every attainable file-major ordinal and modulo delta for each
            # material count >=3, not only the highest-numbered piece.
            for count in range(3,maximum+1):
                for ordinal in range(count):
                    for (df,dr),pairs in sorted(role_pairs(role).items()):
                        b,m=white_board(role,count,ordinal,pairs)
                        for black in (False,True):
                            bb,mm=transform(b.copy(stack=False),[m],black)
                            color="black" if black else "white"
                            idx+=1;emit(out,bb,mm,f"{color} {name} count {count} ordinal {ordinal+1} delta {df},{dr}",idx)
                            counts[f"{color}:{name}:move"]+=1
                        coverage.append([name,count,ordinal+1,df,dr])
            for count in range(2,maximum+1):
                for capture in (False,True):
                    b,m=promotion_board(role,count,capture)
                    for black in (False,True):
                        bb,mm=transform(b.copy(stack=False),m,black)
                        color="black" if black else "white"
                        idx+=1;emit(out,bb,mm,f"{color} promotion to {name} {count} {'capture' if capture else 'quiet'} then move",idx,True)
                        counts[f"{color}:{name}:promotion"]+=1
            for count in range(3,maximum+1):
                for ordinal in range(count):
                    b,m=capture_board(role,count,ordinal)
                    # Mirror and swap side to move together.
                    for black in (False,True):
                        bb,mm=transform(b.copy(stack=False),m,black)
                        color="black" if black else "white"
                        idx+=1;emit(out,bb,mm,f"{color} {name} count {count} capture ordinal {ordinal+1} then highest survivor moves",idx)
                        counts[f"{color}:{name}:capture-renumber"]+=1
            for side in (True,False):
                moves=standard.make(role,side,SEED+role*2+int(side))
                board=chess.Board()
                for m in moves:assert board.is_legal(m);board.push(m)
                assert len(board.pieces(role,side))==maximum and not board.pieces(chess.PAWN,side)
                assert sum(m.promotion==role for m in moves)==8
                color="white" if side else "black"
                idx+=1;emit(out,chess.Board(),moves,f"{color} standard-start eight {name} promotions to {maximum}",idx)
                counts[f"{color}:{name}:standard-start"]+=1
    games=plies=variations=0
    with a.output.open(encoding="utf-8") as f:
        while g:=chess.pgn.read_game(f):
            assert not g.errors,g.errors
            assert g.board().is_valid(),g.headers
            games+=1;plies+=sum(1 for _ in g.mainline_moves())
            stack=[g]
            while stack:
                node=stack.pop()
                for child in node.variations:
                    assert node.board().is_legal(child.move)
                    variations+=1;stack.append(child)
    assert games==120+idx,(games,idx)
    result={"seed":SEED,"original_games":120,"added_games":idx,"games":games,"mainline_plies":plies,"all_plies":variations,"coverage":dict(counts),"move_matrix_cells":len(coverage),"sha256":hashlib.sha256(a.output.read_bytes()).hexdigest(),"limitations":"Synthetic legal setup positions (historical reachability not proved) plus eight legally replayed standard-start games reaching maximum promotion material. Native ChessBase conversion and CTG import behavior pending."}
    if a.ctg_standard:
        n=0
        with a.output.open(encoding="utf-8") as f,a.ctg_standard.open("w",encoding="utf-8",newline="\n") as out:
            while g:=chess.pgn.read_game(f):
                if "standard-start eight" not in g.headers["Event"]:continue
                assert g.board().fen()==chess.STARTING_FEN
                end=g.end().board()
                g.headers["Result"]=end.result(claim_draw=False) if end.is_game_over(claim_draw=False) else "1/2-1/2"
                out.write(g.accept(chess.pgn.StringExporter(headers=True,variations=True,comments=True))+"\n\n")
                n+=1
        assert n==8
        result["standard_ctg_source"]={"games":n,"sha256":hashlib.sha256(a.ctg_standard.read_bytes()).hexdigest(),"notes":"Same eight standard-start lines, completed game result tags for CTG import; configure full game depth."}
    a.manifest.write_text(json.dumps(result,indent=2)+"\n")
    print(json.dumps(result,indent=2))

if __name__=="__main__":main()
