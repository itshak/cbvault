// Copyright (c) 2026 the oschess-cb-bridge contributors
// Modified for cbvault (MIT License). See docs/provenance.md.

//! Small synthetic 2CBH databases for tests, written to a temporary directory.

use cbvault_chess::movetable::{self, Captured, CastleSide, Color as MColor, MoveWord, Piece as MPiece, Sq};
use cbvault_format::twocbh::checksum;
use gigachess::{Board, Color, Move, Role, Square};

use crate::TempDb;

/// A square from its name: `sq("e4")`.
pub fn sq(name: &str) -> Sq {
    let b = name.as_bytes();
    assert!(b.len() == 2 && (b'a'..=b'h').contains(&b[0]) && (b'1'..=b'8').contains(&b[1]), "square {name}");
    (b[1] - b'1') * 8 + (b[0] - b'a')
}

/// The word of a move that neither captures nor promotes. Panics when no word names the move.
pub fn quiet(color: MColor, piece: MPiece, from: &str, to: &str) -> u16 {
    let mv =
        MoveWord::Normal { color, piece, from: sq(from), to: sq(to), captured: Captured::Nothing, promotion: None };
    movetable::encode(mv).unwrap_or_else(|| panic!("no word for {mv:?}"))
}

fn role_to_piece(r: Role) -> MPiece {
    match r {
        Role::King => MPiece::King,
        Role::Queen => MPiece::Queen,
        Role::Rook => MPiece::Rook,
        Role::Bishop => MPiece::Bishop,
        Role::Knight => MPiece::Knight,
        Role::Pawn => MPiece::Pawn,
    }
}

/// The words of `ucis`, moves in UCI, played from `board`, which they advance.
/// Castling in standard chess is given as `e1g1`, `e1c1`, `e8g8`, `e8c8` or king-to-rook.
pub fn words(board: &mut Board, ucis: &str) -> Vec<u16> {
    let mut out = Vec::new();
    for uci in ucis.split_whitespace() {
        if uci == "--" || uci == "0000" {
            board.play(Move::NULL).expect("null move");
            out.push(movetable::NULL_MOVE);
            continue;
        }
        let color = if board.turn() == Color::White { MColor::White } else { MColor::Black };
        let legal_moves = board.legal_moves();
        let found = legal_moves.iter().copied().find(|mv| {
            let from_str = format!("{}", mv.from());
            let to_str = format!("{}", mv.to());
            let mut text = format!("{from_str}{to_str}");
            if let Some(p) = mv.promotion() {
                text.push(match p {
                    Role::Queen => 'q',
                    Role::Rook => 'r',
                    Role::Bishop => 'b',
                    Role::Knight => 'n',
                    _ => panic!("promotion to {p:?}"),
                });
            }
            if text == uci {
                return true;
            }
            // Standard castling notation e1g1 / e1c1 vs king-to-rook
            if board.piece_at(mv.from()).is_some_and(|p| p.role == Role::King) {
                let rank = if board.turn() == Color::White { 0 } else { 7 };
                let is_short = (uci == "e1g1" && rank == 0) || (uci == "e8g8" && rank == 7);
                let is_long = (uci == "e1c1" && rank == 0) || (uci == "e8c8" && rank == 7);
                if is_short && mv.to() == Square::from_coords(7, rank) {
                    return true;
                }
                if is_long && mv.to() == Square::from_coords(0, rank) {
                    return true;
                }
            }
            false
        });
        let mv = found.unwrap_or_else(|| {
            panic!("illegal fixture move {uci} in {}", board.to_fen());
        });

        let p = board.piece_at(mv.from()).unwrap();
        let word = if p.role == Role::King
            && (mv.to().file().abs_diff(mv.from().file()) > 1
                || (board.piece_at(mv.to()).is_some_and(|dest| dest.role == Role::Rook && dest.color == p.color)))
        {
            let is_short = mv.to().file() > mv.from().file();
            MoveWord::Castle { color, side: if is_short { CastleSide::Short } else { CastleSide::Long } }
        } else {
            let captured = match board.piece_at(mv.to()) {
                Some(cap) => match cap.role {
                    Role::Queen => Captured::Queen,
                    Role::Rook => Captured::Rook,
                    Role::Bishop => Captured::Bishop,
                    Role::Knight => Captured::Knight,
                    _ => Captured::Pawn,
                },
                None if p.role == Role::Pawn && mv.from().file() != mv.to().file() => Captured::EnPassant,
                None => Captured::Nothing,
            };
            MoveWord::Normal {
                color,
                piece: role_to_piece(p.role),
                from: mv.from().index() as u8,
                to: mv.to().index() as u8,
                captured,
                promotion: mv.promotion().map(role_to_piece),
            }
        };
        board.play(mv).unwrap();
        out.push(movetable::encode(word).unwrap_or_else(|| panic!("failed to encode word for {uci}")));
    }
    out
}

/// The words of `ucis` played in a Chess960 game from position `position` (0..959).
pub fn words_960(board: &mut Board, position: u16, ucis: &str) -> Vec<u16> {
    let mut out = Vec::new();
    for uci in ucis.split_whitespace() {
        if uci == "--" || uci == "0000" {
            board.play(Move::NULL).expect("null move");
            out.push(movetable::NULL_MOVE);
            continue;
        }
        let color = if board.turn() == Color::White { MColor::White } else { MColor::Black };
        let legal_moves = board.legal_moves();
        let found = legal_moves.iter().copied().find(|mv| {
            let from_str = format!("{}", mv.from());
            let to_str = format!("{}", mv.to());
            let mut text = format!("{from_str}{to_str}");
            if let Some(p) = mv.promotion() {
                text.push(match p {
                    Role::Queen => 'q',
                    Role::Rook => 'r',
                    Role::Bishop => 'b',
                    Role::Knight => 'n',
                    _ => panic!("promotion to {p:?}"),
                });
            }
            text == uci
        });
        let mv = found.unwrap_or_else(|| {
            panic!("illegal fixture 960 move {uci} in {}", board.to_fen());
        });
        let p = board.piece_at(mv.from()).unwrap();
        let word = if p.role == Role::King
            && (board.piece_at(mv.to()).is_some_and(|dest| dest.role == Role::Rook && dest.color == p.color)
                || mv.to().file().abs_diff(mv.from().file()) > 1)
        {
            let is_short = mv.to().file() > mv.from().file();
            MoveWord::Castle960 { position, color, side: if is_short { CastleSide::Short } else { CastleSide::Long } }
        } else {
            let captured = match board.piece_at(mv.to()) {
                Some(cap) => match cap.role {
                    Role::Queen => Captured::Queen,
                    Role::Rook => Captured::Rook,
                    Role::Bishop => Captured::Bishop,
                    Role::Knight => Captured::Knight,
                    _ => Captured::Pawn,
                },
                None if p.role == Role::Pawn && mv.from().file() != mv.to().file() => Captured::EnPassant,
                None => Captured::Nothing,
            };
            MoveWord::Normal {
                color,
                piece: role_to_piece(p.role),
                from: mv.from().index() as u8,
                to: mv.to().index() as u8,
                captured,
                promotion: mv.promotion().map(role_to_piece),
            }
        };
        board.play(mv).unwrap();
        out.push(movetable::encode(word).unwrap_or_else(|| panic!("failed to encode 960 word for {uci}")));
    }
    out
}

/// Words as little-endian bytes, the layout of a move record's content.
pub fn bytes(words: &[u16]) -> Vec<u8> {
    words.iter().flat_map(|w| w.to_le_bytes()).collect()
}

/// A complete move record: the frame around `content`, with variant `tag`.
pub fn framed(tag: u16, content: &[u8]) -> Vec<u8> {
    framed_with_spare(tag, content, 0)
}

/// Complete frame with `spare` (B) padding bytes.
pub fn framed_with_spare(tag: u16, content: &[u8], spare: usize) -> Vec<u8> {
    let mut r = vec![0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11];
    r.extend((content.len() as i32).to_le_bytes());
    r.extend((spare as i32).to_le_bytes());
    r.extend(checksum(content).to_be_bytes());
    r.extend(tag.to_le_bytes());
    r.extend(content);
    r.resize(r.len() + spare, 0);
    r.extend((content.len() as i64 + spare as i64 + 34).to_le_bytes());
    r
}

/// Corrupted checksum frame for error testing.
pub fn corrupted_checksum_frame(tag: u16, content: &[u8]) -> Vec<u8> {
    let mut r = vec![0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11];
    r.extend((content.len() as i32).to_le_bytes());
    r.extend(0i32.to_le_bytes());
    r.extend(0xdeadbeefcafebabeu64.to_be_bytes()); // corrupt
    r.extend(tag.to_le_bytes());
    r.extend(content);
    r.extend((content.len() as i64 + 34).to_le_bytes());
    r
}

/// Corrupted trailer frame for error testing.
pub fn corrupted_trailer_frame(tag: u16, content: &[u8]) -> Vec<u8> {
    let mut r = vec![0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11];
    r.extend((content.len() as i32).to_le_bytes());
    r.extend(0i32.to_le_bytes());
    r.extend(checksum(content).to_be_bytes());
    r.extend(tag.to_le_bytes());
    r.extend(content);
    r.extend(12345i64.to_le_bytes()); // wrong trailer
    r
}

/// Truncated frame for error testing.
pub fn truncated_frame(tag: u16, content: &[u8], len: usize) -> Vec<u8> {
    let mut r = framed(tag, content);
    r.truncate(len);
    r
}

/// A `.2lid` header with one entity type of `container` bytes per entity and `count` entities.
pub fn lid_header(container: i32, count: i64) -> Vec<u8> {
    let mut d = Vec::new();
    d.extend(184i32.to_be_bytes());
    d.extend(1i32.to_be_bytes());
    d.extend(container.to_be_bytes());
    d.extend(count.to_be_bytes());
    d.extend((-1i64).to_be_bytes());
    d.resize(184, 0);
    d
}

/// An annotation record's content: position blocks, each a position (−1 for
/// the game) and its annotations, then the end marker.
pub fn annotations(blocks: &[(i32, Vec<Vec<u8>>)]) -> Vec<u8> {
    let mut v = Vec::new();
    for (position, anns) in blocks {
        v.extend(position.to_le_bytes());
        v.extend((anns.len() as i32).to_le_bytes());
        for a in anns {
            v.extend(a);
        }
    }
    v.extend(0x7fff_ffffi32.to_le_bytes());
    v
}

/// A text annotation: after the move, or before it; `language` as 0 = neutral, 1 = English.
pub fn text_annotation(before: bool, language: u16, text: &str) -> Vec<u8> {
    let mut v = (if before { 0x82u16 } else { 0x02 }).to_le_bytes().to_vec();
    v.extend([0, 0]);
    v.extend(language.to_le_bytes());
    v.extend((text.len() as i32).to_le_bytes());
    v.extend(text.as_bytes());
    v
}

/// A symbols annotation: NAGs on the move, on the position, and a prefix.
pub fn symbols_annotation(on_move: u8, on_position: u8, prefix: u8) -> Vec<u8> {
    vec![3, 0, on_move, on_position, prefix]
}

/// A square numbered from 1, file by file, as annotations store it.
fn cb_square(name: &str) -> u8 {
    let s = sq(name);
    (s % 8) * 8 + s / 8 + 1
}

/// A coloured-squares annotation from (colour, square) pairs.
pub fn squares_annotation(items: &[(u8, &str)]) -> Vec<u8> {
    let data: Vec<u8> = items.iter().flat_map(|&(c, s)| [c, cb_square(s)]).collect();
    let mut v = vec![4, 0];
    v.extend((data.len() as i32).to_le_bytes());
    v.extend(data);
    v
}

/// An arrows annotation from (colour, from, to) triples.
pub fn arrows_annotation(items: &[(u8, &str, &str)]) -> Vec<u8> {
    let data: Vec<u8> = items.iter().flat_map(|&(c, f, t)| [c, cb_square(f), cb_square(t)]).collect();
    let mut v = vec![5, 0];
    v.extend((data.len() as i32).to_le_bytes());
    v.extend(data);
    v
}

/// Database builder for 2CBH sets.
pub struct Builder {
    records: Vec<[u8; 192]>,
    cbg: Vec<u8>,
    cba: Option<Vec<u8>>,
    lid: Option<Vec<u8>>,
    players: Vec<(String, String)>,
    tournaments: Vec<(String, String, i32, u8)>,
    tags: Vec<String>,
    sources: Vec<String>,
}

/// Sets the entity IDs on a 192-byte 2CBH header record.
pub fn set_entities(rec: &mut [u8; 192], white: i64, black: i64, tournament: i64) {
    rec[0x18..0x20].copy_from_slice(&white.to_le_bytes());
    rec[0x20..0x28].copy_from_slice(&black.to_le_bytes());
    rec[0x28..0x30].copy_from_slice(&tournament.to_le_bytes());
}

/// Sets the Elo ratings on a 192-byte 2CBH header record.
pub fn set_elos(rec: &mut [u8; 192], white_elo: i16, black_elo: i16) {
    rec[0x60..0x62].copy_from_slice(&white_elo.to_le_bytes());
    rec[0x70..0x72].copy_from_slice(&black_elo.to_le_bytes());
}

impl Default for Builder {
    fn default() -> Self {
        Builder {
            records: Vec::new(),
            cbg: vec![0; 12],
            cba: None,
            lid: None,
            players: Vec::new(),
            tournaments: Vec::new(),
            tags: Vec::new(),
            sources: Vec::new(),
        }
    }
}

impl Builder {
    /// Creates a new empty database builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a player entity and returns its 0-based id.
    pub fn player(&mut self, last: &str, first: &str) -> i64 {
        let id = self.players.len() as i64;
        self.players.push((last.to_string(), first.to_string()));
        id
    }

    /// Adds a tournament entity and returns its 0-based id.
    pub fn tournament(&mut self, title: &str, place: &str, date: i32) -> i64 {
        self.tournament_with_type(title, place, date, 0)
    }

    /// Adds a tournament entity with style/time control byte.
    pub fn tournament_with_type(&mut self, title: &str, place: &str, date: i32, event_type: u8) -> i64 {
        let id = self.tournaments.len() as i64;
        self.tournaments.push((title.to_string(), place.to_string(), date, event_type));
        id
    }

    /// Adds a game tag (title) entity and returns its 0-based id.
    pub fn tag(&mut self, title: &str) -> i64 {
        let id = self.tags.len() as i64;
        self.tags.push(title.to_string());
        id
    }

    /// Adds an annotator or source entity and returns its 0-based id.
    pub fn source(&mut self, name: &str) -> i64 {
        let id = self.sources.len() as i64;
        self.sources.push(name.to_string());
        id
    }

    /// Appends a move record of variant `tag` holding `words`, and returns its offset in `.2cbg`.
    pub fn moves(&mut self, tag: u16, words: &[u16]) -> i64 {
        let offset = self.cbg.len() as i64;
        let mut content = Vec::new();
        if words.first() != Some(&0xfffc) && words.first() != Some(&0xfffb) {
            content.extend(0xfffcu16.to_le_bytes());
        }
        content.extend(bytes(words));
        if words.last() != Some(&0xffff) {
            content.extend(0xffffu16.to_le_bytes());
        }
        self.cbg.extend(framed(tag, &content));
        offset
    }

    /// Appends raw move content with variant `tag`.
    pub fn move_bytes(&mut self, tag: u16, content: &[u8]) -> i64 {
        let offset = self.cbg.len() as i64;
        self.cbg.extend(framed(tag, content));
        offset
    }

    /// Appends arbitrary pre-framed bytes directly to `.2cbg` and returns the offset.
    pub fn move_frame_raw(&mut self, frame: &[u8]) -> i64 {
        let offset = self.cbg.len() as i64;
        self.cbg.extend_from_slice(frame);
        offset
    }

    /// Appends an annotation record and returns its offset in `.2cba`.
    pub fn annotations(&mut self, content: &[u8]) -> i64 {
        let cba = self.cba.get_or_insert_with(|| vec![0; 12]);
        let offset = cba.len() as i64;
        cba.extend(framed(0x2000, content));
        offset
    }

    /// Appends a game whose move record is at `moves` and annotation record at `annotations`.
    pub fn annotated_game(&mut self, moves: i64, annotations: i64) -> &mut [u8; 192] {
        let rec = self.game(moves);
        rec[0x10..0x18].copy_from_slice(&annotations.to_le_bytes());
        rec
    }

    /// Appends a game record pointing to `moves_offset`.
    pub fn game(&mut self, moves_offset: i64) -> &mut [u8; 192] {
        let mut rec = [0u8; 192];
        rec[0] = 1;
        rec[2] = 1;
        rec[3] = 1;
        rec[0x08..0x10].copy_from_slice(&moves_offset.to_le_bytes());
        rec[0x58] = 2; // 1-0 result
        self.records.push(rec);
        self.records.last_mut().unwrap()
    }

    /// Appends a guiding text record.
    pub fn text(&mut self, title_tag: i64, author_player: i64, body_offset: i64) -> &mut [u8; 192] {
        let mut rec = [0u8; 192];
        rec[0] = 2; // text bit
        rec[2] = 3; // guiding text kind
        rec[3] = 1;
        rec[0x08..0x10].copy_from_slice(&body_offset.to_le_bytes());
        rec[0x18..0x20].copy_from_slice(&title_tag.to_le_bytes());
        rec[0x20..0x28].copy_from_slice(&author_player.to_le_bytes());
        self.records.push(rec);
        self.records.last_mut().unwrap()
    }

    /// Appends an analysis record.
    pub fn analysis(&mut self, title_tag: i64, author_player: i64, moves_offset: i64) -> &mut [u8; 192] {
        self.analysis_full(title_tag, author_player, moves_offset, 0, 0)
    }

    /// Appends an analysis record with date and move count (Recipe 6.1).
    pub fn analysis_full(
        &mut self,
        title_tag: i64,
        author_player: i64,
        moves_offset: i64,
        packed_date: u32,
        move_count: u16,
    ) -> &mut [u8; 192] {
        let mut rec = [0u8; 192];
        rec[0] = 1;
        rec[2] = 2; // analysis kind
        rec[3] = 1;
        rec[0x08..0x10].copy_from_slice(&moves_offset.to_le_bytes());
        rec[0x18..0x20].copy_from_slice(&title_tag.to_le_bytes());
        rec[0x28..0x30].copy_from_slice(&author_player.to_le_bytes());
        rec[0x4c..0x50].copy_from_slice(&packed_date.to_le_bytes());
        rec[0x50..0x52].copy_from_slice(&move_count.to_le_bytes());
        self.records.push(rec);
        self.records.last_mut().unwrap()
    }

    /// Appends a deleted game record.
    pub fn deleted_game(&mut self, moves_offset: i64) -> &mut [u8; 192] {
        let rec = self.game(moves_offset);
        rec[0] |= 0x80;
        rec
    }

    /// Replaces the `.2lid` content.
    pub fn lid(&mut self, bytes: Vec<u8>) -> &mut Self {
        self.lid = Some(bytes);
        self
    }

    /// Writes the database files to `temp_db` with base name.
    pub fn write_to(&self, temp_db: &TempDb) {
        let dir = temp_db.dir();
        let name = temp_db.base();
        let stem = name.file_name().unwrap().to_str().unwrap();

        let mut cbh = vec![0u8; 192];
        cbh[0x0a..0x0c].copy_from_slice(&192i16.to_le_bytes());
        cbh[0x0d] = 5;
        let total_records = (self.records.len() + 1) as u32;
        cbh[0x10..0x14].copy_from_slice(&total_records.to_le_bytes());

        let mut cba = self.cba.clone();
        for rec in &self.records {
            let mut rec = *rec;
            if let Some(cba) = &mut cba
                && rec[0x10..0x18] == [0; 8]
            {
                rec[0x10..0x18].copy_from_slice(&(cba.len() as i64).to_le_bytes());
                cba.extend(framed(0x2000, &annotations(&[])));
            }
            cbh.extend(rec);
        }
        if let Some(mut cba) = cba {
            let total = cba.len() as i64;
            cba[..8].copy_from_slice(&total.to_le_bytes());
            cba[8..10].copy_from_slice(&12i16.to_le_bytes());
            std::fs::write(dir.join(format!("{stem}.2cba")), cba).unwrap();
        }
        let mut cbg = self.cbg.clone();
        let total = cbg.len() as i64;
        cbg[..8].copy_from_slice(&total.to_le_bytes());
        cbg[8..10].copy_from_slice(&12i16.to_le_bytes());
        cbg[10..12].copy_from_slice(&[0, 5]);
        std::fs::write(dir.join(format!("{stem}.2cbh")), cbh).unwrap();
        std::fs::write(dir.join(format!("{stem}.2cbg")), cbg).unwrap();

        let lid_data = if let Some(ref lid) = self.lid {
            lid.clone()
        } else {
            let ntypes = if self.tags.is_empty() && self.sources.is_empty() { 2usize } else { 4usize };
            let header_size = 184usize;
            let container_size = 256usize;
            let n_players = self.players.len();
            let n_tournaments = self.tournaments.len();
            let n_tags = self.tags.len();
            let n_sources = self.sources.len();
            let n_blocks = n_players.max(n_tournaments).max(n_tags).max(n_sources);

            let mut d = Vec::new();
            d.extend((header_size as i32).to_be_bytes());
            d.extend((ntypes as i32).to_be_bytes());
            // Type 0: Player
            d.extend((container_size as i32).to_be_bytes());
            d.extend((n_players as i64).to_be_bytes());
            d.extend((-1i64).to_be_bytes());
            // Type 1: Tournament
            d.extend((container_size as i32).to_be_bytes());
            d.extend((n_tournaments as i64).to_be_bytes());
            d.extend((-1i64).to_be_bytes());
            if ntypes >= 4 {
                // Type 2: Game Tag
                d.extend((container_size as i32).to_be_bytes());
                d.extend((n_tags as i64).to_be_bytes());
                d.extend((-1i64).to_be_bytes());
                // Type 3: Annotator / Source
                d.extend((container_size as i32).to_be_bytes());
                d.extend((n_sources as i64).to_be_bytes());
                d.extend((-1i64).to_be_bytes());
            }
            d.resize(header_size, 0);

            let encode_str = |s: &str| -> Vec<u8> {
                let mut v = (s.len() as i32).to_le_bytes().to_vec();
                v.extend_from_slice(s.as_bytes());
                v
            };

            for i in 0..n_blocks {
                // Type 0 container
                let mut p_block = vec![0u8; container_size];
                if i < n_players {
                    let (ref last, ref first) = self.players[i];
                    let mut payload = encode_str(last);
                    payload.extend(encode_str(first));
                    p_block[..4].copy_from_slice(&(payload.len() as i32).to_le_bytes());
                    p_block[4..4 + payload.len()].copy_from_slice(&payload);
                }
                d.extend(p_block);

                // Type 1 container
                let mut t_block = vec![0u8; container_size];
                if i < n_tournaments {
                    let (ref title, ref place, date, event_type) = self.tournaments[i];
                    let mut payload = encode_str(place);
                    payload.extend(encode_str(title));
                    payload.extend(date.to_le_bytes());
                    payload.push(event_type);
                    t_block[..4].copy_from_slice(&(payload.len() as i32).to_le_bytes());
                    t_block[4..4 + payload.len()].copy_from_slice(&payload);
                }
                d.extend(t_block);

                if ntypes >= 4 {
                    // Type 2 container: Game Tag
                    let mut tag_block = vec![0u8; container_size];
                    if i < n_tags {
                        let title = &self.tags[i];
                        let mut payload = 1i32.to_le_bytes().to_vec(); // count = 1
                        payload.extend(0i32.to_le_bytes()); // language = 0
                        payload.extend(encode_str(title));
                        tag_block[..4].copy_from_slice(&(payload.len() as i32).to_le_bytes());
                        tag_block[4..4 + payload.len()].copy_from_slice(&payload);
                    }
                    d.extend(tag_block);

                    // Type 3 container: Annotator / Source
                    let mut src_block = vec![0u8; container_size];
                    if i < n_sources {
                        let name = &self.sources[i];
                        let payload = encode_str(name);
                        src_block[..4].copy_from_slice(&(payload.len() as i32).to_le_bytes());
                        src_block[4..4 + payload.len()].copy_from_slice(&payload);
                    }
                    d.extend(src_block);
                }
            }
            d
        };
        std::fs::write(dir.join(format!("{stem}.2lid")), lid_data).unwrap();
    }
}
