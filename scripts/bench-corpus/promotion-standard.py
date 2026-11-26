#!/usr/bin/env python3
"""Additional promotion transitions from the normal initial position (CC0)."""
import random, chess
ROLES={chess.QUEEN:9,chess.ROOK:10,chess.BISHOP:10,chess.KNIGHT:10}

def make(role, side, seed):
    r=random.Random(seed)
    def mirror(m):
        return chess.Move(chess.square_mirror(m.from_square),chess.square_mirror(m.to_square),promotion=m.promotion) if not side else m
    def move(s):return mirror(chess.Move.from_uci(s))
    def normalized(b):return b if side else b.mirror()
    def preserve(b):return len(b.pieces(role,side))+len(b.pieces(chess.PAWN,side))==ROLES[role]
    def score(b):
        bb=normalized(b)
        # Prefer advanced pawns, clear promotion squares and legal forward moves.
        pawns=bb.pieces(chess.PAWN,chess.WHITE)
        advance=sum(chess.square_rank(s) for s in pawns)
        blockers=sum(bool(bb.piece_at(chess.square(chess.square_file(s),7))) for s in pawns)
        return 35*len(bb.pieces(role,chess.WHITE))+advance*2-blockers*0.7
    def replies(b,next_move=None):
        choices=[]
        for m in b.legal_moves:
            x=b.copy(stack=False);x.push(m)
            if not preserve(x) or x.is_game_over(claim_draw=False):continue
            if next_move is not None and not x.is_legal(next_move):continue
            # Avoid giving check unless no alternative. Clear opposing back rank.
            bb=normalized(x)
            rank8=sum(1 for s in chess.SquareSet(chess.BB_RANK_8) if bb.piece_at(s) and bb.color_at(s)==chess.BLACK)
            value=score(x)-rank8*2-(15 if x.is_check() else 0)+r.random()*3
            choices.append((value,m))
        return sorted(choices,key=lambda q:q[0],reverse=True)
    for attempt in range(250):
        b=chess.Board()
        if not side:
            b.push(chess.Move.from_uci("g1f3"))
        valid=True
        for f in (0,2,4,6):
            a=chess.FILE_NAMES[f];c=chess.FILE_NAMES[f+1]
            seq=[move(a+"2"+a+"4"),move(c+"7"+c+"5"),move(a+"4"+c+"5"),move(a+"7"+a+"5"),move(c+"2"+c+"4")]
            for m in seq:
                if not b.is_legal(m):valid=False;break
                b.push(m)
            if not valid:break
            next_m=move(c+"4"+a+"5")
            opts=replies(b,next_m)
            if not opts:valid=False;break
            b.push(r.choice(opts[:min(3,len(opts))])[1]);b.push(next_m)
            next_m=move(chess.FILE_NAMES[f+2]+"2"+chess.FILE_NAMES[f+2]+"4") if f<6 else None
            opts=replies(b,next_m)
            if not opts:valid=False;break
            b.push(r.choice(opts[:min(4,len(opts))])[1])
        if not valid or not preserve(b):continue
        visited={}
        for _ in range(300):
            if len(b.pieces(role,side))==ROLES[role] and not b.pieces(chess.PAWN,side):
                return b.move_stack[:]
            if b.is_game_over(claim_draw=False):break
            if b.turn!=side:
                opts=replies(b)
                if not opts:break
                b.push(r.choice(opts[:min(4,len(opts))])[1]);continue
            choices=[]
            for m in b.legal_moves:
                if m.promotion and m.promotion!=role:continue
                x=b.copy(stack=False);x.push(m)
                if not preserve(x) or x.is_game_over(claim_draw=False):continue
                if not replies(x):continue
                key=x.board_fen()
                value=(score(x)-score(b))*10-visited.get(key,0)*18+r.random()*5
                if b.piece_type_at(m.from_square)==chess.PAWN:value+=20
                if m.promotion:value+=100
                choices.append((value,m,key))
            if not choices:break
            choices.sort(key=lambda q:q[0],reverse=True)
            _,m,key=choices[0];visited[key]=visited.get(key,0)+1;b.push(m)
    raise RuntimeError(f"standard start promotion search failed for {role=} {side=}")
if __name__=="__main__":
    for role in ROLES:
        for side in (True,False):
            moves=make(role,side,20261126+role*2+int(side))
            b=chess.Board()
            for m in moves:assert b.is_legal(m);b.push(m)
            print(chess.piece_name(role),side,len(moves),b.fen(),flush=True)
