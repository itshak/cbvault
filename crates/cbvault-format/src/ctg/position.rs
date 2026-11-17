//! The position key of a `.ctg` entry: a prefix-coded board followed by padding,
//! the en-passant file and the castling rights.
//!
//! Original cbvault code, clean-room: every rule here was established by
//! probing a real book (see `docs/research/ctg-observations-2026-11-14.md`).
//!
//! The book stores positions **with White to move** only, and a position with no
//! castling rights and the white king on files a–d **mirrored** to files e–h.
//! This module encodes and decodes the stored form. It is chess-free: turning a
//! real position into the stored form (colour flip, mirror) is the chess layer's
//! job (`cbvault-chess`).

/// Largest encoded position the 5-bit length field can describe, header byte
/// included.
pub const MAX_ENCODED: usize = 31;

/// Header flag: the key ends with 4 castling bits.
const FLAG_CASTLING: u8 = 0x40;
/// Header flag: the key carries a 3-bit en-passant file.
const FLAG_EN_PASSANT: u8 = 0x20;

/// White may castle king side.
pub const CASTLE_WHITE_KING: u8 = 1;
/// White may castle queen side.
pub const CASTLE_WHITE_QUEEN: u8 = 2;
/// Black may castle king side.
pub const CASTLE_BLACK_KING: u8 = 4;
/// Black may castle queen side.
pub const CASTLE_BLACK_QUEEN: u8 = 8;

/// A piece kind as the book codes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    /// A pawn.
    Pawn,
    /// A knight.
    Knight,
    /// A bishop.
    Bishop,
    /// A rook.
    Rook,
    /// A queen.
    Queen,
    /// A king.
    King,
}

/// A piece on a square.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Man {
    /// `true` for a white piece.
    pub white: bool,
    /// What the piece is.
    pub kind: Kind,
}

/// A position in the book's stored form.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RawPosition {
    /// Squares indexed `rank * 8 + file` (a1 = 0, h8 = 63). This is the usual
    /// little-endian mapping, not the file-major order the key is written in.
    pub squares: [Option<Man>; 64],
    /// Castling rights, a mask of the `CASTLE_*` constants.
    pub castling: u8,
    /// The en-passant file (0 = a), when the key carries one.
    pub ep_file: Option<u8>,
}

impl RawPosition {
    /// An empty board with no rights.
    pub fn empty() -> RawPosition {
        RawPosition { squares: [None; 64], castling: 0, ep_file: None }
    }
}

/// Why a key could not be decoded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyError {
    /// The length field is below 2 or above the bytes available.
    Length,
    /// The bits ran out before 64 squares were read.
    ShortBoard,
    /// The bits after the board are fewer than its flags announce.
    ShortTail,
}

/// The prefix code of a square's content: `(bits, length)`, read MSB first.
///
/// `0` empty; `11c` pawn; `1001c` knight; `1010c` bishop; `1011c` rook;
/// `10000c` king; `10001c` queen; where `c` is 0 for White and 1 for Black.
fn code_of(man: Option<Man>) -> (u8, u8) {
    let Some(man) = man else { return (0, 1) };
    let c = u8::from(!man.white);
    match man.kind {
        Kind::Pawn => (0b110 | c, 3),
        Kind::Knight => (0b10010 | c, 5),
        Kind::Bishop => (0b10100 | c, 5),
        Kind::Rook => (0b10110 | c, 5),
        Kind::King => (0b100000 | c, 6),
        Kind::Queen => (0b100010 | c, 6),
    }
}

/// Square indices in the order the key writes them: a1, a2, …, a8, b1, …, h8.
fn key_order() -> impl Iterator<Item = usize> {
    (0..8).flat_map(|file| (0..8).map(move |rank| rank * 8 + file))
}

/// Reads bits MSB first from a byte slice.
struct BitReader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl BitReader<'_> {
    fn bit(&mut self) -> Option<u8> {
        let byte = *self.bytes.get(self.pos / 8)?;
        let b = (byte >> (7 - self.pos % 8)) & 1;
        self.pos += 1;
        Some(b)
    }

    fn total(&self) -> usize {
        self.bytes.len() * 8
    }
}

/// Reads one square's content.
fn read_man(r: &mut BitReader<'_>) -> Option<Option<Man>> {
    if r.bit()? == 0 {
        return Some(None);
    }
    let kind = if r.bit()? == 1 {
        Kind::Pawn
    } else if r.bit()? == 1 {
        if r.bit()? == 0 { Kind::Bishop } else { Kind::Rook }
    } else if r.bit()? == 1 {
        Kind::Knight
    } else if r.bit()? == 0 {
        Kind::King
    } else {
        Kind::Queen
    };
    let white = r.bit()? == 0;
    Some(Some(Man { white, kind }))
}

/// Decodes a key (`key[0]` is the header byte; `key.len()` must equal its
/// length field).
pub fn decode(key: &[u8]) -> Result<RawPosition, KeyError> {
    let header = *key.first().ok_or(KeyError::Length)?;
    let len = usize::from(header & 0x1F);
    if len < 2 || len != key.len() {
        return Err(KeyError::Length);
    }
    let mut r = BitReader { bytes: &key[1..], pos: 0 };
    let mut pos = RawPosition::empty();
    for sq in key_order() {
        pos.squares[sq] = read_man(&mut r).ok_or(KeyError::ShortBoard)?;
    }
    let total = r.total();
    let mut tail_end = total;
    if header & FLAG_CASTLING != 0 {
        let start = tail_end.checked_sub(4).filter(|&s| s >= r.pos).ok_or(KeyError::ShortTail)?;
        let mut t = BitReader { bytes: &key[1..], pos: start };
        // Stored order: black O-O, black O-O-O, white O-O, white O-O-O.
        for right in [CASTLE_BLACK_KING, CASTLE_BLACK_QUEEN, CASTLE_WHITE_KING, CASTLE_WHITE_QUEEN] {
            if t.bit().ok_or(KeyError::ShortTail)? == 1 {
                pos.castling |= right;
            }
        }
        tail_end = start;
    }
    if header & FLAG_EN_PASSANT != 0 {
        let start = tail_end.checked_sub(3).filter(|&s| s >= r.pos).ok_or(KeyError::ShortTail)?;
        let mut t = BitReader { bytes: &key[1..], pos: start };
        let mut file = 0u8;
        for _ in 0..3 {
            file = (file << 1) | t.bit().ok_or(KeyError::ShortTail)?;
        }
        pos.ep_file = Some(file);
    }
    Ok(pos)
}

/// Accumulates bits MSB first into a fixed buffer.
struct BitWriter {
    buf: [u8; MAX_ENCODED],
    pos: usize,
}

impl BitWriter {
    fn push(&mut self, bits: u8, len: u8) -> Option<()> {
        for i in (0..len).rev() {
            let byte = self.buf.get_mut(self.pos / 8)?;
            if (bits >> i) & 1 == 1 {
                *byte |= 1 << (7 - self.pos % 8);
            }
            self.pos += 1;
        }
        Some(())
    }
}

/// How the zero padding between the board and the tail is sized.
///
/// Observed (20,000 pages of a real book): the key is padded to a whole byte,
/// except that a key with **no flags** whose bits already end on a byte boundary
/// gets a full zero byte. The aligned en-passant-only case was never observed,
/// which is why [`encode_with`] takes the choice as a parameter and the book
/// reader tries both for that one case.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlignedPad {
    /// Add nothing when already aligned.
    None,
    /// Add a whole zero byte when already aligned.
    FullByte,
}

/// The padding rule as observed for a key's flags, and whether the aligned case
/// for those flags was actually observed.
pub fn observed_aligned_pad(castling: bool, ep: bool) -> (AlignedPad, bool) {
    match (castling, ep) {
        (false, false) => (AlignedPad::FullByte, true),
        (true, false) => (AlignedPad::None, true),
        // Never seen aligned; `None` matches the castling case.
        (_, true) => (AlignedPad::None, false),
    }
}

/// Encodes `pos` into `out` with the observed padding rule, returning the key
/// length (header included). `None` when the key would exceed [`MAX_ENCODED`].
pub fn encode(pos: &RawPosition, out: &mut [u8; MAX_ENCODED]) -> Option<usize> {
    let (pad, _) = observed_aligned_pad(pos.castling != 0, pos.ep_file.is_some());
    encode_with(pos, pad, out)
}

/// Encodes `pos` with an explicit choice for the aligned padding case.
pub fn encode_with(pos: &RawPosition, aligned: AlignedPad, out: &mut [u8; MAX_ENCODED]) -> Option<usize> {
    let mut w = BitWriter { buf: [0; MAX_ENCODED], pos: 8 };
    for sq in key_order() {
        let (bits, len) = code_of(pos.squares[sq]);
        w.push(bits, len)?;
    }
    let ep_bits = if pos.ep_file.is_some() { 3 } else { 0 };
    let castle_bits = if pos.castling != 0 { 4 } else { 0 };
    let used = w.pos + ep_bits + castle_bits;
    let pad = match (used % 8, aligned) {
        (0, AlignedPad::None) => 0,
        (0, AlignedPad::FullByte) => 8,
        (rem, _) => 8 - rem,
    };
    w.pos += pad;
    if let Some(file) = pos.ep_file {
        w.push(file & 7, 3)?;
    }
    if pos.castling != 0 {
        let c = pos.castling;
        let bit = |m: u8| u8::from(c & m != 0);
        let bits = bit(CASTLE_BLACK_KING) << 3
            | bit(CASTLE_BLACK_QUEEN) << 2
            | bit(CASTLE_WHITE_KING) << 1
            | bit(CASTLE_WHITE_QUEEN);
        w.push(bits, 4)?;
    }
    let len = w.pos / 8;
    if len > MAX_ENCODED || !w.pos.is_multiple_of(8) {
        return None;
    }
    let mut header = u8::try_from(len).ok()?;
    if pos.castling != 0 {
        header |= FLAG_CASTLING;
    }
    if pos.ep_file.is_some() {
        header |= FLAG_EN_PASSANT;
    }
    w.buf[0] = header;
    out.copy_from_slice(&w.buf);
    Some(len)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn man(white: bool, kind: Kind) -> Option<Man> {
        Some(Man { white, kind })
    }

    fn start() -> RawPosition {
        let back =
            [Kind::Rook, Kind::Knight, Kind::Bishop, Kind::Queen, Kind::King, Kind::Bishop, Kind::Knight, Kind::Rook];
        let mut p = RawPosition::empty();
        for (file, &kind) in back.iter().enumerate() {
            p.squares[file] = man(true, kind);
            p.squares[8 + file] = man(true, Kind::Pawn);
            p.squares[48 + file] = man(false, Kind::Pawn);
            p.squares[56 + file] = man(false, kind);
        }
        p.castling = 0xF;
        p
    }

    /// The key of the initial position, as found in a real book (it is only the
    /// encoding of the board, the same in every book).
    const ROOT_KEY: [u8; 22] = [
        0x56, 0xb6, 0x0f, 0x79, 0x60, 0xf3, 0xa6, 0x0f, 0x58, 0xb0, 0x78, 0xe0, 0xc1, 0xe1, 0xa6, 0x0f, 0x59, 0x60,
        0xf3, 0xb6, 0x0f, 0x7f,
    ];

    #[test]
    fn the_initial_position_encodes_to_the_probed_root_key() {
        let mut out = [0; MAX_ENCODED];
        let len = encode(&start(), &mut out).unwrap();
        assert_eq!(&out[..len], &ROOT_KEY);
    }

    #[test]
    fn the_probed_root_key_decodes_to_the_initial_position() {
        assert_eq!(decode(&ROOT_KEY).unwrap(), start());
    }

    #[test]
    fn a_flagless_aligned_key_gets_a_whole_zero_byte() {
        // Two kings, three pawns and 59 empties: 12 + 9 + 59 = 80 board bits,
        // 88 with the header byte, so the bits end on a byte boundary.
        let mut p = RawPosition::empty();
        p.squares[4] = man(true, Kind::King);
        p.squares[60] = man(false, Kind::King);
        for sq in [8, 9, 10] {
            p.squares[sq] = man(true, Kind::Pawn);
        }
        let mut out = [0; MAX_ENCODED];
        let len = encode(&p, &mut out).unwrap();
        assert_eq!(len, 12, "88 bits plus a whole padding byte");
        assert_eq!(out[11], 0);
        assert_eq!(decode(&out[..len]).unwrap(), p);
        let len = encode_with(&p, AlignedPad::None, &mut out).unwrap();
        assert_eq!(len, 11);
    }

    #[test]
    fn en_passant_and_castling_round_trip() {
        let mut p = start();
        p.squares[12] = None;
        p.squares[36] = man(true, Kind::Pawn);
        p.squares[51] = None;
        p.squares[35] = man(false, Kind::Pawn);
        p.ep_file = Some(3);
        p.castling = CASTLE_WHITE_KING | CASTLE_BLACK_QUEEN;
        let mut out = [0; MAX_ENCODED];
        let len = encode(&p, &mut out).unwrap();
        assert_eq!(out[0] & 0x60, 0x60);
        assert_eq!(decode(&out[..len]).unwrap(), p);
    }

    #[test]
    fn malformed_keys_are_typed_errors() {
        assert_eq!(decode(&[]), Err(KeyError::Length));
        assert_eq!(decode(&[0x05, 0, 0]), Err(KeyError::Length));
        assert_eq!(decode(&[0x02, 0xFF]), Err(KeyError::ShortBoard));
        // 64 empties need 8 bytes; announce castling with no room for it.
        let mut key = [0u8; 9];
        key[0] = 0x40 | 9;
        assert_eq!(decode(&key), Err(KeyError::ShortTail));
    }
}
