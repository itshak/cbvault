//! The move byte of a book entry: which piece moves and by how much.
//!
//! Original cbvault code, clean-room. The table was established by probing a
//! real book (see `docs/research/ctg-observations-2026-11-14.md`) in three passes:
//!
//! 1. each byte's candidate meanings, drawn from pseudo-legal moves, were
//!    intersected over every occurrence on a third of the pages, and meanings
//!    pinned to another byte were excluded: 139 bytes resolved, no contradiction;
//! 2. the 23 bytes left (second-queen moves, marked "child existence" below)
//!    were resolved over the whole book by the one legal move whose resulting
//!    position is also in the book (every decoded move's resulting position is:
//!    28,263,243/28,263,243), with a single candidate on every occurrence;
//! 3. three more second-queen bytes (`0x1a`, `0xab`, `0xee`) were resolved the
//!    same way on two other books, which also agreed with the second pass on
//!    every byte they share;
//! 4. five rare move bytes (`0x19`, `0x2b`, `0x89`, `0xb4` for third knights,
//!    `0xbc` for second queen) were resolved by table symmetry and confirmed
//!    via unique legal child existence on a 3.4M-game Mega Database CTG book.
//!
//! 5. nine third-piece codes were confirmed on the public 3,410,497-game
//!    native book: all 12 occurrences had exactly one legal, previously
//!    unassigned code with a child key found in the complete book.
//!
//! Bytes never seen are `None`.

use super::position::Kind;

/// What a move byte means, for the side to move of the stored position (always
/// White).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MoveCode {
    /// The kind of the moving piece.
    pub kind: Kind,
    /// Which piece of that kind moves: 0 is the first one met in the key's
    /// square order (a1, a2, …, a8, b1, …, h8).
    pub ordinal: u8,
    /// File delta, modulo 8.
    pub dfile: u8,
    /// Rank delta, modulo 8.
    pub drank: u8,
}

/// The meaning of `byte`, when it was established.
///
/// The target square is `((file + dfile) % 8, (rank + drank) % 8)`; a delta of
/// +4 and of -4 coincide modulo 8, and only one of the two targets is on the
/// board. Castling is the king moving two files (`dfile` 2 or 6).
pub fn move_code(byte: u8) -> Option<MoveCode> {
    MOVE_TABLE[usize::from(byte)]
}

/// The annotation byte that follows each move byte.
///
/// It reads as two parts. The **low 5 bits** are a move-symbol code, numbered as
/// PGN NAGs. ChessBase shows 1 as `!`, 2 as `?`, 3 as `!!`, 4 as `??`, 5 as `!?`
/// and 6 as `?!` (checked against its display of two real books and of a book
/// the contributor annotated for the purpose). Other codes are not confirmed, so
/// [`Annotation::symbol`] does not name them. The **high 3 bits** are a class
/// whose meaning is not established; it does not affect the move's colour (see
/// [`super::colour()`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Annotation(pub u8);

impl Annotation {
    /// The move-symbol code (low 5 bits), 0 when there is none.
    pub fn symbol_code(self) -> u8 {
        self.0 & 0x1F
    }

    /// The class (high 3 bits). Its meaning is not established.
    pub fn class(self) -> u8 {
        self.0 >> 5
    }

    /// The symbol ChessBase shows, for the codes confirmed so far.
    pub fn symbol(self) -> Option<&'static str> {
        match self.symbol_code() {
            1 => Some("!"),
            2 => Some("?"),
            3 => Some("!!"),
            4 => Some("??"),
            5 => Some("!?"),
            6 => Some("?!"),
            _ => None,
        }
    }
}

/// Every byte value, indexed by the byte.
const MOVE_TABLE: [Option<MoveCode>; 256] = [
    Some(MoveCode { kind: Kind::Pawn, ordinal: 4, dfile: 1, drank: 1 }), // 0x00
    Some(MoveCode { kind: Kind::Knight, ordinal: 1, dfile: 6, drank: 7 }), // 0x01
    None,                                                                // 0x02
    Some(MoveCode { kind: Kind::Queen, ordinal: 1, dfile: 2, drank: 0 }), // 0x03 (child existence)
    Some(MoveCode { kind: Kind::Pawn, ordinal: 1, dfile: 0, drank: 1 }), // 0x04
    Some(MoveCode { kind: Kind::Queen, ordinal: 0, dfile: 0, drank: 1 }), // 0x05
    Some(MoveCode { kind: Kind::Pawn, ordinal: 3, dfile: 7, drank: 1 }), // 0x06
    None,                                                                // 0x07
    Some(MoveCode { kind: Kind::Queen, ordinal: 1, dfile: 4, drank: 0 }), // 0x08 (child existence)
    Some(MoveCode { kind: Kind::Bishop, ordinal: 1, dfile: 6, drank: 6 }), // 0x09
    Some(MoveCode { kind: Kind::King, ordinal: 0, dfile: 0, drank: 7 }), // 0x0a
    Some(MoveCode { kind: Kind::Rook, ordinal: 2, dfile: 7, drank: 0 }), // 0x0b (scale unique child)
    Some(MoveCode { kind: Kind::Pawn, ordinal: 0, dfile: 7, drank: 1 }), // 0x0c
    Some(MoveCode { kind: Kind::Bishop, ordinal: 0, dfile: 3, drank: 3 }), // 0x0d
    Some(MoveCode { kind: Kind::Rook, ordinal: 1, dfile: 3, drank: 0 }), // 0x0e
    Some(MoveCode { kind: Kind::Knight, ordinal: 0, dfile: 6, drank: 7 }), // 0x0f
    None,                                                                // 0x10
    None,                                                                // 0x11
    Some(MoveCode { kind: Kind::Bishop, ordinal: 0, dfile: 7, drank: 7 }), // 0x12
    Some(MoveCode { kind: Kind::King, ordinal: 0, dfile: 0, drank: 1 }), // 0x13
    Some(MoveCode { kind: Kind::Pawn, ordinal: 7, dfile: 1, drank: 1 }), // 0x14
    Some(MoveCode { kind: Kind::Bishop, ordinal: 0, dfile: 5, drank: 5 }), // 0x15
    None,                                                                // 0x16
    None,                                                                // 0x17
    Some(MoveCode { kind: Kind::Pawn, ordinal: 6, dfile: 0, drank: 1 }), // 0x18
    Some(MoveCode { kind: Kind::Knight, ordinal: 2, dfile: 6, drank: 1 }), // 0x19
    Some(MoveCode { kind: Kind::Queen, ordinal: 1, dfile: 0, drank: 6 }), // 0x1a (child existence, other books)
    Some(MoveCode { kind: Kind::Bishop, ordinal: 0, dfile: 7, drank: 1 }), // 0x1b
    None,                                                                // 0x1c
    Some(MoveCode { kind: Kind::Bishop, ordinal: 1, dfile: 7, drank: 7 }), // 0x1d
    Some(MoveCode { kind: Kind::Queen, ordinal: 2, dfile: 1, drank: 7 }), // 0x1e (scale unique child)
    None,                                                                // 0x1f
    None,                                                                // 0x20
    Some(MoveCode { kind: Kind::Rook, ordinal: 1, dfile: 7, drank: 0 }), // 0x21
    Some(MoveCode { kind: Kind::Bishop, ordinal: 1, dfile: 6, drank: 2 }), // 0x22
    Some(MoveCode { kind: Kind::Queen, ordinal: 1, dfile: 6, drank: 6 }), // 0x23 (child existence)
    Some(MoveCode { kind: Kind::Pawn, ordinal: 7, dfile: 7, drank: 1 }), // 0x24
    None,                                                                // 0x25
    Some(MoveCode { kind: Kind::Bishop, ordinal: 0, dfile: 1, drank: 7 }), // 0x26
    Some(MoveCode { kind: Kind::Pawn, ordinal: 2, dfile: 7, drank: 1 }), // 0x27
    Some(MoveCode { kind: Kind::Queen, ordinal: 0, dfile: 5, drank: 5 }), // 0x28
    Some(MoveCode { kind: Kind::Queen, ordinal: 0, dfile: 6, drank: 0 }), // 0x29
    Some(MoveCode { kind: Kind::Knight, ordinal: 1, dfile: 1, drank: 6 }), // 0x2a
    Some(MoveCode { kind: Kind::Knight, ordinal: 2, dfile: 7, drank: 2 }), // 0x2b
    None,                                                                // 0x2c
    Some(MoveCode { kind: Kind::Pawn, ordinal: 5, dfile: 1, drank: 1 }), // 0x2d
    Some(MoveCode { kind: Kind::Bishop, ordinal: 0, dfile: 1, drank: 1 }), // 0x2e
    Some(MoveCode { kind: Kind::Queen, ordinal: 0, dfile: 1, drank: 0 }), // 0x2f
    Some(MoveCode { kind: Kind::Knight, ordinal: 1, dfile: 7, drank: 6 }), // 0x30
    Some(MoveCode { kind: Kind::Queen, ordinal: 0, dfile: 3, drank: 0 }), // 0x31
    Some(MoveCode { kind: Kind::Bishop, ordinal: 1, dfile: 5, drank: 5 }), // 0x32
    None,                                                                // 0x33
    Some(MoveCode { kind: Kind::Knight, ordinal: 0, dfile: 1, drank: 2 }), // 0x34
    None,                                                                // 0x35
    Some(MoveCode { kind: Kind::Knight, ordinal: 0, dfile: 2, drank: 1 }), // 0x36
    Some(MoveCode { kind: Kind::Queen, ordinal: 0, dfile: 0, drank: 4 }), // 0x37
    Some(MoveCode { kind: Kind::Queen, ordinal: 1, dfile: 4, drank: 4 }), // 0x38 (child existence)
    Some(MoveCode { kind: Kind::Queen, ordinal: 0, dfile: 5, drank: 0 }), // 0x39
    Some(MoveCode { kind: Kind::Bishop, ordinal: 0, dfile: 6, drank: 6 }), // 0x3a
    Some(MoveCode { kind: Kind::Queen, ordinal: 1, dfile: 3, drank: 5 }), // 0x3b (child existence)
    Some(MoveCode { kind: Kind::Bishop, ordinal: 0, dfile: 3, drank: 5 }), // 0x3c
    Some(MoveCode { kind: Kind::Knight, ordinal: 2, dfile: 6, drank: 7 }), // 0x3d (scale unique child)
    None,                                                                // 0x3e
    None,                                                                // 0x3f
    None,                                                                // 0x40
    Some(MoveCode { kind: Kind::Queen, ordinal: 1, dfile: 5, drank: 5 }), // 0x41 (child existence)
    Some(MoveCode { kind: Kind::Queen, ordinal: 0, dfile: 1, drank: 7 }), // 0x42
    None,                                                                // 0x43
    Some(MoveCode { kind: Kind::King, ordinal: 0, dfile: 1, drank: 7 }), // 0x44
    Some(MoveCode { kind: Kind::Queen, ordinal: 0, dfile: 3, drank: 3 }), // 0x45
    Some(MoveCode { kind: Kind::Queen, ordinal: 2, dfile: 5, drank: 5 }), // 0x46 (scale unique child)
    None,                                                                // 0x47
    None,                                                                // 0x48
    None,                                                                // 0x49
    Some(MoveCode { kind: Kind::Pawn, ordinal: 7, dfile: 0, drank: 2 }), // 0x4a
    Some(MoveCode { kind: Kind::Queen, ordinal: 0, dfile: 3, drank: 5 }), // 0x4b
    Some(MoveCode { kind: Kind::Knight, ordinal: 1, dfile: 1, drank: 2 }), // 0x4c
    Some(MoveCode { kind: Kind::Queen, ordinal: 1, dfile: 0, drank: 1 }), // 0x4d (child existence)
    Some(MoveCode { kind: Kind::Queen, ordinal: 2, dfile: 6, drank: 6 }), // 0x4e (scale unique child)
    Some(MoveCode { kind: Kind::Bishop, ordinal: 2, dfile: 7, drank: 7 }), // 0x4f (scale unique child)
    Some(MoveCode { kind: Kind::Rook, ordinal: 0, dfile: 0, drank: 6 }), // 0x50
    None,                                                                // 0x51
    Some(MoveCode { kind: Kind::Rook, ordinal: 0, dfile: 6, drank: 0 }), // 0x52
    None,                                                                // 0x53
    Some(MoveCode { kind: Kind::Bishop, ordinal: 1, dfile: 7, drank: 1 }), // 0x54
    Some(MoveCode { kind: Kind::Pawn, ordinal: 2, dfile: 0, drank: 1 }), // 0x55
    None,                                                                // 0x56
    Some(MoveCode { kind: Kind::Queen, ordinal: 2, dfile: 0, drank: 6 }), // 0x57 (scale unique child)
    None,                                                                // 0x58
    None,                                                                // 0x59
    None,                                                                // 0x5a
    None,                                                                // 0x5b
    Some(MoveCode { kind: Kind::Pawn, ordinal: 6, dfile: 1, drank: 1 }), // 0x5c
    None,                                                                // 0x5d
    None,                                                                // 0x5e
    Some(MoveCode { kind: Kind::Pawn, ordinal: 4, dfile: 0, drank: 2 }), // 0x5f
    None,                                                                // 0x60
    Some(MoveCode { kind: Kind::Queen, ordinal: 0, dfile: 6, drank: 6 }), // 0x61
    Some(MoveCode { kind: Kind::Pawn, ordinal: 1, dfile: 0, drank: 2 }), // 0x62
    Some(MoveCode { kind: Kind::Queen, ordinal: 1, dfile: 1, drank: 7 }), // 0x63 (child existence)
    None,                                                                // 0x64
    None,                                                                // 0x65
    Some(MoveCode { kind: Kind::Bishop, ordinal: 0, dfile: 5, drank: 3 }), // 0x66
    Some(MoveCode { kind: Kind::King, ordinal: 0, dfile: 1, drank: 1 }), // 0x67
    None,                                                                // 0x68
    Some(MoveCode { kind: Kind::Rook, ordinal: 1, dfile: 0, drank: 7 }), // 0x69
    Some(MoveCode { kind: Kind::Bishop, ordinal: 0, dfile: 4, drank: 4 }), // 0x6a
    Some(MoveCode { kind: Kind::King, ordinal: 0, dfile: 2, drank: 0 }), // 0x6b
    None,                                                                // 0x6c
    None,                                                                // 0x6d
    Some(MoveCode { kind: Kind::Rook, ordinal: 0, dfile: 5, drank: 0 }), // 0x6e
    Some(MoveCode { kind: Kind::Queen, ordinal: 1, dfile: 7, drank: 7 }), // 0x6f (child existence)
    None,                                                                // 0x70
    None,                                                                // 0x71
    Some(MoveCode { kind: Kind::Bishop, ordinal: 1, dfile: 1, drank: 7 }), // 0x72
    None,                                                                // 0x73
    Some(MoveCode { kind: Kind::Queen, ordinal: 0, dfile: 2, drank: 0 }), // 0x74
    None,                                                                // 0x75
    None,                                                                // 0x76
    None,                                                                // 0x77
    None,                                                                // 0x78
    Some(MoveCode { kind: Kind::Bishop, ordinal: 1, dfile: 2, drank: 6 }), // 0x79
    Some(MoveCode { kind: Kind::Rook, ordinal: 0, dfile: 0, drank: 3 }), // 0x7a
    Some(MoveCode { kind: Kind::Rook, ordinal: 1, dfile: 0, drank: 6 }), // 0x7b
    Some(MoveCode { kind: Kind::Pawn, ordinal: 2, dfile: 1, drank: 1 }), // 0x7c
    Some(MoveCode { kind: Kind::Rook, ordinal: 1, dfile: 0, drank: 1 }), // 0x7d
    Some(MoveCode { kind: Kind::Queen, ordinal: 0, dfile: 5, drank: 3 }), // 0x7e
    Some(MoveCode { kind: Kind::Rook, ordinal: 0, dfile: 1, drank: 0 }), // 0x7f
    Some(MoveCode { kind: Kind::Queen, ordinal: 0, dfile: 2, drank: 6 }), // 0x80
    Some(MoveCode { kind: Kind::Rook, ordinal: 0, dfile: 0, drank: 1 }), // 0x81
    Some(MoveCode { kind: Kind::Pawn, ordinal: 5, dfile: 7, drank: 1 }), // 0x82
    None,                                                                // 0x83
    None,                                                                // 0x84
    Some(MoveCode { kind: Kind::Knight, ordinal: 0, dfile: 7, drank: 2 }), // 0x85
    Some(MoveCode { kind: Kind::Rook, ordinal: 0, dfile: 7, drank: 0 }), // 0x86
    Some(MoveCode { kind: Kind::Rook, ordinal: 0, dfile: 0, drank: 5 }), // 0x87
    None,                                                                // 0x88
    Some(MoveCode { kind: Kind::Knight, ordinal: 2, dfile: 1, drank: 6 }), // 0x89
    Some(MoveCode { kind: Kind::Knight, ordinal: 0, dfile: 1, drank: 6 }), // 0x8a
    Some(MoveCode { kind: Kind::Pawn, ordinal: 0, dfile: 1, drank: 1 }), // 0x8b
    Some(MoveCode { kind: Kind::King, ordinal: 0, dfile: 7, drank: 7 }), // 0x8c
    None,                                                                // 0x8d
    Some(MoveCode { kind: Kind::Queen, ordinal: 1, dfile: 6, drank: 2 }), // 0x8e (child existence)
    Some(MoveCode { kind: Kind::Queen, ordinal: 0, dfile: 7, drank: 0 }), // 0x8f
    None,                                                                // 0x90
    None,                                                                // 0x91
    Some(MoveCode { kind: Kind::Queen, ordinal: 1, dfile: 1, drank: 1 }), // 0x92 (child existence)
    None,                                                                // 0x93
    Some(MoveCode { kind: Kind::Queen, ordinal: 0, dfile: 0, drank: 3 }), // 0x94
    None,                                                                // 0x95
    Some(MoveCode { kind: Kind::Pawn, ordinal: 1, dfile: 1, drank: 1 }), // 0x96
    Some(MoveCode { kind: Kind::King, ordinal: 0, dfile: 7, drank: 0 }), // 0x97
    Some(MoveCode { kind: Kind::Rook, ordinal: 0, dfile: 3, drank: 0 }), // 0x98
    Some(MoveCode { kind: Kind::Rook, ordinal: 0, dfile: 0, drank: 4 }), // 0x99
    Some(MoveCode { kind: Kind::Queen, ordinal: 0, dfile: 0, drank: 6 }), // 0x9a
    Some(MoveCode { kind: Kind::Pawn, ordinal: 2, dfile: 0, drank: 2 }), // 0x9b
    None,                                                                // 0x9c
    Some(MoveCode { kind: Kind::Queen, ordinal: 0, dfile: 0, drank: 2 }), // 0x9d
    None,                                                                // 0x9e
    Some(MoveCode { kind: Kind::Bishop, ordinal: 1, dfile: 4, drank: 4 }), // 0x9f
    Some(MoveCode { kind: Kind::Queen, ordinal: 1, dfile: 0, drank: 3 }), // 0xa0 (child existence)
    None,                                                                // 0xa1
    Some(MoveCode { kind: Kind::Queen, ordinal: 0, dfile: 2, drank: 2 }), // 0xa2
    Some(MoveCode { kind: Kind::Pawn, ordinal: 7, dfile: 0, drank: 1 }), // 0xa3
    None,                                                                // 0xa4
    Some(MoveCode { kind: Kind::Rook, ordinal: 1, dfile: 0, drank: 5 }), // 0xa5
    None,                                                                // 0xa6
    None,                                                                // 0xa7
    None,                                                                // 0xa8
    Some(MoveCode { kind: Kind::Rook, ordinal: 1, dfile: 2, drank: 0 }), // 0xa9
    None,                                                                // 0xaa
    Some(MoveCode { kind: Kind::Queen, ordinal: 1, dfile: 2, drank: 6 }), // 0xab (child existence, other books)
    None,                                                                // 0xac
    Some(MoveCode { kind: Kind::Rook, ordinal: 1, dfile: 4, drank: 0 }), // 0xad
    Some(MoveCode { kind: Kind::Queen, ordinal: 1, dfile: 3, drank: 3 }), // 0xae (child existence)
    None,                                                                // 0xaf
    Some(MoveCode { kind: Kind::Queen, ordinal: 1, dfile: 0, drank: 4 }), // 0xb0 (child existence)
    Some(MoveCode { kind: Kind::Pawn, ordinal: 5, dfile: 0, drank: 2 }), // 0xb1
    Some(MoveCode { kind: Kind::Bishop, ordinal: 0, dfile: 2, drank: 6 }), // 0xb2
    None,                                                                // 0xb3
    Some(MoveCode { kind: Kind::Knight, ordinal: 2, dfile: 7, drank: 6 }), // 0xb4
    Some(MoveCode { kind: Kind::Rook, ordinal: 1, dfile: 5, drank: 0 }), // 0xb5
    None,                                                                // 0xb6
    Some(MoveCode { kind: Kind::Queen, ordinal: 0, dfile: 0, drank: 5 }), // 0xb7
    None,                                                                // 0xb8
    Some(MoveCode { kind: Kind::Bishop, ordinal: 1, dfile: 3, drank: 3 }), // 0xb9
    None,                                                                // 0xba
    Some(MoveCode { kind: Kind::Pawn, ordinal: 4, dfile: 0, drank: 1 }), // 0xbb
    Some(MoveCode { kind: Kind::Queen, ordinal: 1, dfile: 5, drank: 0 }), // 0xbc (child existence)
    Some(MoveCode { kind: Kind::Queen, ordinal: 1, dfile: 0, drank: 2 }), // 0xbd (child existence)
    Some(MoveCode { kind: Kind::King, ordinal: 0, dfile: 1, drank: 0 }), // 0xbe
    None,                                                                // 0xbf
    None,                                                                // 0xc0
    Some(MoveCode { kind: Kind::Bishop, ordinal: 0, dfile: 2, drank: 2 }), // 0xc1
    Some(MoveCode { kind: Kind::Bishop, ordinal: 1, dfile: 2, drank: 2 }), // 0xc2
    Some(MoveCode { kind: Kind::Bishop, ordinal: 0, dfile: 6, drank: 2 }), // 0xc3
    Some(MoveCode { kind: Kind::Rook, ordinal: 1, dfile: 1, drank: 0 }), // 0xc4
    Some(MoveCode { kind: Kind::Rook, ordinal: 1, dfile: 0, drank: 4 }), // 0xc5
    Some(MoveCode { kind: Kind::Queen, ordinal: 1, dfile: 0, drank: 5 }), // 0xc6 (child existence)
    Some(MoveCode { kind: Kind::Pawn, ordinal: 6, dfile: 7, drank: 1 }), // 0xc7
    Some(MoveCode { kind: Kind::Pawn, ordinal: 6, dfile: 0, drank: 2 }), // 0xc8
    Some(MoveCode { kind: Kind::Queen, ordinal: 1, dfile: 0, drank: 7 }), // 0xc9 (child existence)
    Some(MoveCode { kind: Kind::Bishop, ordinal: 1, dfile: 5, drank: 3 }), // 0xca
    Some(MoveCode { kind: Kind::Pawn, ordinal: 5, dfile: 0, drank: 1 }), // 0xcb
    Some(MoveCode { kind: Kind::Bishop, ordinal: 1, dfile: 3, drank: 5 }), // 0xcc
    Some(MoveCode { kind: Kind::Rook, ordinal: 0, dfile: 2, drank: 0 }), // 0xcd
    None,                                                                // 0xce
    Some(MoveCode { kind: Kind::Pawn, ordinal: 3, dfile: 0, drank: 1 }), // 0xcf
    None,                                                                // 0xd0
    Some(MoveCode { kind: Kind::Pawn, ordinal: 1, dfile: 7, drank: 1 }), // 0xd1
    Some(MoveCode { kind: Kind::Knight, ordinal: 1, dfile: 2, drank: 1 }), // 0xd2
    Some(MoveCode { kind: Kind::Knight, ordinal: 1, dfile: 6, drank: 1 }), // 0xd3
    Some(MoveCode { kind: Kind::Queen, ordinal: 2, dfile: 2, drank: 0 }), // 0xd4 (scale unique child)
    None,                                                                // 0xd5
    Some(MoveCode { kind: Kind::Rook, ordinal: 2, dfile: 0, drank: 7 }), // 0xd6 (scale unique child)
    Some(MoveCode { kind: Kind::Queen, ordinal: 0, dfile: 7, drank: 1 }), // 0xd7
    Some(MoveCode { kind: Kind::Rook, ordinal: 1, dfile: 6, drank: 0 }), // 0xd8
    Some(MoveCode { kind: Kind::Queen, ordinal: 0, dfile: 6, drank: 2 }), // 0xd9
    Some(MoveCode { kind: Kind::Knight, ordinal: 0, dfile: 7, drank: 6 }), // 0xda
    Some(MoveCode { kind: Kind::Pawn, ordinal: 0, dfile: 0, drank: 2 }), // 0xdb
    None,                                                                // 0xdc
    None,                                                                // 0xdd
    Some(MoveCode { kind: Kind::Pawn, ordinal: 4, dfile: 7, drank: 1 }), // 0xde
    Some(MoveCode { kind: Kind::King, ordinal: 0, dfile: 7, drank: 1 }), // 0xdf
    Some(MoveCode { kind: Kind::Knight, ordinal: 1, dfile: 2, drank: 7 }), // 0xe0
    Some(MoveCode { kind: Kind::Rook, ordinal: 0, dfile: 0, drank: 7 }), // 0xe1
    None,                                                                // 0xe2
    Some(MoveCode { kind: Kind::Rook, ordinal: 1, dfile: 0, drank: 3 }), // 0xe3
    None,                                                                // 0xe4
    Some(MoveCode { kind: Kind::Queen, ordinal: 0, dfile: 4, drank: 0 }), // 0xe5
    Some(MoveCode { kind: Kind::Pawn, ordinal: 3, dfile: 0, drank: 2 }), // 0xe6
    Some(MoveCode { kind: Kind::Queen, ordinal: 0, dfile: 4, drank: 4 }), // 0xe7
    Some(MoveCode { kind: Kind::Rook, ordinal: 0, dfile: 0, drank: 2 }), // 0xe8
    Some(MoveCode { kind: Kind::Knight, ordinal: 0, dfile: 2, drank: 7 }), // 0xe9
    None,                                                                // 0xea
    Some(MoveCode { kind: Kind::Pawn, ordinal: 3, dfile: 1, drank: 1 }), // 0xeb
    Some(MoveCode { kind: Kind::Pawn, ordinal: 0, dfile: 0, drank: 1 }), // 0xec
    Some(MoveCode { kind: Kind::Queen, ordinal: 0, dfile: 7, drank: 7 }), // 0xed
    Some(MoveCode { kind: Kind::Queen, ordinal: 1, dfile: 7, drank: 1 }), // 0xee (child existence, other books)
    Some(MoveCode { kind: Kind::Rook, ordinal: 0, dfile: 4, drank: 0 }), // 0xef
    Some(MoveCode { kind: Kind::Queen, ordinal: 1, dfile: 7, drank: 0 }), // 0xf0 (child existence)
    Some(MoveCode { kind: Kind::Queen, ordinal: 0, dfile: 1, drank: 1 }), // 0xf1
    None,                                                                // 0xf2
    Some(MoveCode { kind: Kind::Knight, ordinal: 1, dfile: 7, drank: 2 }), // 0xf3
    Some(MoveCode { kind: Kind::Rook, ordinal: 1, dfile: 0, drank: 2 }), // 0xf4
    Some(MoveCode { kind: Kind::Bishop, ordinal: 1, dfile: 1, drank: 1 }), // 0xf5
    Some(MoveCode { kind: Kind::King, ordinal: 0, dfile: 6, drank: 0 }), // 0xf6
    Some(MoveCode { kind: Kind::Knight, ordinal: 0, dfile: 6, drank: 1 }), // 0xf7
    Some(MoveCode { kind: Kind::Queen, ordinal: 1, dfile: 1, drank: 0 }), // 0xf8 (child existence)
    Some(MoveCode { kind: Kind::Queen, ordinal: 1, dfile: 6, drank: 0 }), // 0xf9 (child existence)
    Some(MoveCode { kind: Kind::Queen, ordinal: 1, dfile: 3, drank: 0 }), // 0xfa (child existence)
    Some(MoveCode { kind: Kind::Queen, ordinal: 1, dfile: 2, drank: 2 }), // 0xfb (child existence)
    None,                                                                // 0xfc
    Some(MoveCode { kind: Kind::Queen, ordinal: 0, dfile: 0, drank: 7 }), // 0xfd
    Some(MoveCode { kind: Kind::Queen, ordinal: 1, dfile: 5, drank: 3 }), // 0xfe (child existence)
    None,                                                                // 0xff
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_twenty_root_moves_are_all_established() {
        // The bytes of the 20 legal moves of the initial position, sorted.
        let root = [
            0x04, 0x18, 0x34, 0x4a, 0x4c, 0x55, 0x5f, 0x62, 0x85, 0x9b, 0xa3, 0xb1, 0xbb, 0xc8, 0xcb, 0xcf, 0xdb, 0xe6,
            0xec, 0xf3,
        ];
        let codes: Vec<MoveCode> = root.iter().map(|&b| move_code(b).unwrap()).collect();
        let pawns = codes.iter().filter(|c| c.kind == Kind::Pawn).count();
        let knights = codes.iter().filter(|c| c.kind == Kind::Knight).count();
        assert_eq!((pawns, knights), (16, 4));
    }

    #[test]
    fn annotation_bytes_split_into_class_and_symbol() {
        // The four values seen at a real book's root, as ChessBase showed them.
        assert_eq!((Annotation(0xe1).class(), Annotation(0xe1).symbol()), (7, Some("!")));
        assert_eq!((Annotation(0xc0).class(), Annotation(0xc0).symbol()), (6, None));
        assert_eq!((Annotation(0x25).class(), Annotation(0x25).symbol()), (1, Some("!?")));
        assert_eq!((Annotation(0x22).class(), Annotation(0x22).symbol()), (1, Some("?")));
        assert_eq!(Annotation(0x06).symbol(), Some("?!"));
        assert_eq!(Annotation(0x03).symbol(), Some("!!"));
        assert_eq!(Annotation(0x04).symbol(), Some("??"));
        assert_eq!(Annotation(0x07).symbol(), None, "unconfirmed codes stay unnamed");
        assert_eq!(Annotation(0x07).symbol_code(), 7);
    }

    #[test]
    fn castling_is_the_king_moving_two_files() {
        assert_eq!(move_code(0x6b), Some(MoveCode { kind: Kind::King, ordinal: 0, dfile: 2, drank: 0 }));
        assert_eq!(move_code(0xf6), Some(MoveCode { kind: Kind::King, ordinal: 0, dfile: 6, drank: 0 }));
    }

    #[test]
    fn no_two_bytes_share_a_meaning() {
        let all: Vec<MoveCode> = (0..=255u8).filter_map(move_code).collect();
        for (i, a) in all.iter().enumerate() {
            assert!(!all[i + 1..].contains(a), "{a:?} appears twice");
        }
        assert_eq!(all.len(), 179); // 170 original + 9 independently verified codes.
    }

    #[test]
    fn rare_promoted_move_codes_are_mapped() {
        assert_eq!(move_code(0x19), Some(MoveCode { kind: Kind::Knight, ordinal: 2, dfile: 6, drank: 1 }));
        assert_eq!(move_code(0x2b), Some(MoveCode { kind: Kind::Knight, ordinal: 2, dfile: 7, drank: 2 }));
        assert_eq!(move_code(0x89), Some(MoveCode { kind: Kind::Knight, ordinal: 2, dfile: 1, drank: 6 }));
        assert_eq!(move_code(0xb4), Some(MoveCode { kind: Kind::Knight, ordinal: 2, dfile: 7, drank: 6 }));
        assert_eq!(move_code(0xbc), Some(MoveCode { kind: Kind::Queen, ordinal: 1, dfile: 5, drank: 0 }));
    }
}
