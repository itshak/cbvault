//! Game quotations (type `13`): a game quoted in a comment, with a header of
//! its own. The layout is in `docs/format-notes.md`; what is not understood
//! is kept in the annotation's data. [`Quotation::chessbase_text`] is the
//! reading form's text, as ChessBase's own PGN writes it — its evidence is
//! ChessBase's own export, which this reproduces field for field.
//!
//! Ported from `cbformat`'s `game/annotations/quote.rs` (MIT,
//! `oschess-cb-bridge` @ `ca9e8f8e`), the classic parser only: the 2CBH
//! quotation's moves and set-up come with 2CBH reading. See
//! `docs/provenance.md`.

use super::decode;
use crate::game::{Date, Eco};

/// A player of a quoted game.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QuotedPlayer {
    /// The last name.
    pub last: String,
    /// The first name.
    pub first: String,
    /// The rating, 0 when unknown.
    pub elo: u16,
}

/// A decoded game quotation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Quotation {
    /// The quoted game's white player.
    pub white: QuotedPlayer,
    /// The quoted game's black player.
    pub black: QuotedPlayer,
    /// The quoted game's event.
    pub event: String,
    /// The quoted game's site.
    pub site: String,
    /// The quoted game's date.
    pub date: Date,
    /// The event's type byte: bit `0x20` blitz, `0x40` rapid, `0x80`
    /// correspondence; the low bits the kind of event.
    pub kind: u8,
    /// The number of rounds.
    pub round: u8,
    /// Stored as a signed byte; ChessBase shows a negative one as its 16-bit
    /// two's complement.
    pub subround: i8,
    /// 0 black won, 1 draw, 2 white won.
    pub result: u8,
    /// The quoted game's ECO.
    pub eco: Eco,
}

/// Reads bytes in order, `None` past the end.
struct Cursor<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let s = self.b.get(self.i..self.i.checked_add(n)?)?;
        self.i += n;
        Some(s)
    }
    fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }
    fn be16(&mut self) -> Option<u16> {
        let b = self.take(2)?;
        Some(u16::from_be_bytes([b[0], b[1]]))
    }
    fn be32(&mut self) -> Option<i32> {
        Some(i32::from_be_bytes(self.take(4)?.try_into().ok()?))
    }
    /// A classic string: a length byte, the text and a zero.
    fn classic(&mut self) -> Option<String> {
        let n = self.u8()? as usize;
        let s = self.take(n)?;
        self.take(1)?;
        Some(decode(s))
    }
}

impl Quotation {
    /// A classic quotation's data, the bytes after the type. Its header is
    /// read; the moves of a classic quotation are not understood and stay in
    /// the data. `None` when it does not have the known layout.
    pub fn parse_classic(data: &[u8]) -> Option<Quotation> {
        let mut c = Cursor { b: data, i: 0 };
        c.take(6)?; // size, mode, unknown
        let (white, black) = (c.classic()?, c.classic()?);
        let (white_elo, black_elo, eco) = (c.be16()?, c.be16()?, c.be16()?);
        let (event, site) = (c.classic()?, c.classic()?);
        let date = c.be32()?;
        let kind = c.be16()?.to_le_bytes()[0];
        c.take(2 + 4)?; // nation, unknown and rounds
        let subround = c.u8()? as i8;
        let round = c.u8()?;
        let result = c.u8()?;
        let split = |s: &str| match s.split_once(',') {
            Some((last, first)) => (last.to_string(), first.to_string()),
            None => (s.to_string(), String::new()),
        };
        let ((wl, wf), (bl, bf)) = (split(&white), split(&black));
        Some(Quotation {
            white: QuotedPlayer { last: wl, first: wf, elo: white_elo },
            black: QuotedPlayer { last: bl, first: bf, elo: black_elo },
            event,
            site,
            date: Date(date),
            kind,
            round,
            subround,
            result,
            eco: Eco::from_field(eco),
        })
    }

    /// A 2CBH quotation's data, the bytes after the type code.
    pub fn parse_v2(data: &[u8]) -> Option<Quotation> {
        // Real 2CBH quotations start `[1, 1]` or `[1, 2]`; ordinary metadata
        // shares the `[1, 1]` prefix but carries no player names (games
        // 773687, 831403), so the names decide.
        if data.len() < 16 || !(data.starts_with(&[1, 1]) || data.starts_with(&[1, 2])) {
            return None;
        }
        let mut i = 10; // skip 10-byte prefix
        let mut get_str = |b: &[u8]| -> Option<String> {
            let l = *b.get(i)? as usize;
            i += 1;
            let s = b.get(i..i + l)?;
            i += l;
            let trimmed = s.strip_suffix(b"\0").unwrap_or(s);
            Some(decode(trimmed))
        };
        let white_last = get_str(data)?;
        let white_first = get_str(data)?;
        let black_last = get_str(data)?;
        let black_first = get_str(data)?;
        if white_last.is_empty() && black_last.is_empty() {
            return None;
        }
        let site = get_str(data)?;
        let event = get_str(data)?;
        let date_bytes: [u8; 4] = data.get(i..i + 4)?.try_into().ok()?;
        let date = i32::from_le_bytes(date_bytes);
        let kind = *data.get(i + 4).unwrap_or(&0);
        let white_elo = data.get(i + 28..i + 30).and_then(|b| b.try_into().ok()).map(u16::from_le_bytes).unwrap_or(0);
        let black_elo = data.get(i + 30..i + 32).and_then(|b| b.try_into().ok()).map(u16::from_le_bytes).unwrap_or(0);
        let result_byte = *data.get(i + 34).unwrap_or(&3);
        let round = *data.get(i + 43).unwrap_or(&0);
        let subround = *data.get(i + 44).unwrap_or(&0) as i8;
        let result = match result_byte {
            0 => 0, // 0-1
            1 => 1, // draw (1/2-1/2)
            2 => 2, // 1-0
            _ => 3, // unknown / line
        };
        Some(Quotation {
            white: QuotedPlayer { last: white_last, first: white_first, elo: white_elo },
            black: QuotedPlayer { last: black_last, first: black_first, elo: black_elo },
            event,
            site,
            date: Date(date),
            kind,
            round,
            subround,
            result,
            eco: Eco::None,
        })
    }

    /// Parses either a classic or 2CBH quotation.
    pub fn parse(data: &[u8]) -> Option<Quotation> {
        if data.starts_with(&[1, 2]) {
            Self::parse_v2(data).or_else(|| Self::parse_classic(data))
        } else {
            Self::parse_classic(data).or_else(|| Self::parse_v2(data))
        }
    }

    /// The quotation as ChessBase writes it in its own PGN export: the result,
    /// both players as `Last,F (Elo)`, the event with its site and its speed
    /// unless the title holds them, the year unless the title holds it, and
    /// the round. A draw is `½-½`, as Mega Database 2025's own export writes
    /// it (2,451 of them; the ancestor's notes saw `1/2` in Update41).
    ///
    /// Writes into the caller's buffer — the export path reuses one — with no
    /// allocation; [`Quotation::chessbase_text`] returns the same text owned.
    pub fn chessbase_text_into(&self, out: &mut String) {
        out.clear();
        // A result ChessBase does not know is written as nothing at all: the
        // Mega's `Kan,I-Botvinnik,M Moscow training m2 1953` quote of game
        // 134450, `Lasker,E-Parnall,G GBR tour sim Great Britain 1898` of
        // 139965 — a `*` would be ours alone.
        match self.result {
            0 => out.push_str("0-1 "),
            1 => out.push_str("\u{bd}-\u{bd} "),
            2 => out.push_str("1-0 "),
            _ => {}
        }
        out.push_str(self.white.last.trim());
        if !self.white.first.trim().is_empty() {
            let initial = self.white.first.trim().chars().next().expect("a first name");
            out.push(',');
            out.push(initial);
        }
        if self.white.elo > 0 {
            out.push_str(" (");
            push_number(out, u64::from(self.white.elo));
            out.push(')');
        }
        out.push('-');
        out.push_str(self.black.last.trim());
        if !self.black.first.trim().is_empty() {
            let initial = self.black.first.trim().chars().next().expect("a first name");
            out.push(',');
            out.push(initial);
        }
        if self.black.elo > 0 {
            out.push_str(" (");
            push_number(out, u64::from(self.black.elo));
            out.push(')');
        }
        // The event is written as stored, its trailing space and all: that
        // space is what the export shows before the year (`Leningrad masters
        //  1933`, `Paris  1934`). The site is written as stored too, but it is
        // left out where the event already holds it (game 327778: the event
        // `17th Championship of Moscow ` and the site `Moscow ` ship once).
        let event = self.event.as_str();
        let has_event = !event.is_empty();
        let title_start = out.len();
        if has_event {
            out.push(' ');
            out.push_str(event);
        }
        let site = self.site.as_str();
        if !site.trim().is_empty() && !event.contains(site) {
            if has_event || !out.ends_with(' ') {
                out.push(' ');
            }
            out.push_str(site);
        }
        for (bit, label) in [(0x20, "blitz"), (0x40, "rapid")] {
            if self.kind & bit != 0 && !contains_ignore_case(event, label) {
                out.push(' ');
                out.push_str(label);
            }
        }
        // The year is left out where the title already holds it — the event,
        // the site and the speed labels, as the accumulated title does.
        let year = self.date.year();
        if year > 0 && !contains_number(&out[title_start..], year) {
            // An empty event and site leave their separator behind, as the
            // export writes `Spielmann,A-NN  1922` (game 63811); when the
            // title holds a site the empty event writes nothing extra.
            if !has_event && out.len() == title_start {
                out.push(' ');
            }
            out.push(' ');
            push_number(out, u64::from(year));
        }
        let sub = i16::from(self.subround) as u16;
        match (self.round, sub) {
            (0, 0) => {}
            (0, s) => {
                out.push_str(" [");
                push_number(out, u64::from(s));
                out.push(']');
            }
            (r, 0) => {
                out.push_str(" (");
                push_number(out, u64::from(r));
                out.push(')');
            }
            (r, s) => {
                out.push_str(" (");
                push_number(out, u64::from(r));
                out.push('.');
                push_number(out, u64::from(s));
                out.push(')');
            }
        }
    }

    /// [`Quotation::chessbase_text_into`], returning the text owned.
    pub fn chessbase_text(&self) -> String {
        let mut out = String::new();
        self.chessbase_text_into(&mut out);
        out
    }
}

/// A number in decimal, without the formatting machinery.
#[inline]
fn push_number(out: &mut String, n: u64) {
    let mut digits = [0u8; 20];
    let mut i = digits.len();
    let mut n = n;
    loop {
        i -= 1;
        digits[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    for &b in &digits[i..] {
        out.push(char::from(b));
    }
}

/// Whether `haystack` holds `needle`, ignoring ASCII case. A case-insensitive
/// search over the event, without the `to_lowercase()` allocation the older
/// writer made per quotation.
#[inline]
fn contains_ignore_case(haystack: &str, needle: &str) -> bool {
    let haystack = haystack.as_bytes();
    let needle = needle.as_bytes();
    if needle.is_empty() || needle.len() > haystack.len() {
        return needle.is_empty();
    }
    haystack.windows(needle.len()).any(|window| window.eq_ignore_ascii_case(needle))
}

/// Whether `text` holds the number `n` in decimal, the test the year needs.
#[inline]
fn contains_number(text: &str, n: u16) -> bool {
    let mut digits = [0u8; 5];
    let mut i = digits.len();
    let mut n = u32::from(n);
    loop {
        i -= 1;
        digits[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    text.as_bytes().windows(digits.len() - i).any(|w| w == &digits[i..])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A classic quotation record: a six-byte header, then each field of the
    /// layout `docs/format-notes.md` gives, every string a length byte, the
    /// text and a zero, every integer big-endian.
    fn classic(white: &str, black: &str, event: &str) -> Vec<u8> {
        let mut d = vec![0x60, 0, 0, 0, 0, 0];
        for s in [white, black] {
            d.push(s.len() as u8);
            d.extend_from_slice(s.as_bytes());
            d.push(0);
        }
        d.extend_from_slice(&2450u16.to_be_bytes()); // white's rating
        d.extend_from_slice(&2500u16.to_be_bytes()); // black's rating
        d.extend_from_slice(&43264u16.to_be_bytes()); // ECO D37: (337 + 1) * 128
        for s in [event, "Wijk aan Zee"] {
            d.push(s.len() as u8);
            d.extend_from_slice(s.as_bytes());
            d.push(0);
        }
        d.extend_from_slice(&(1999i32 << 9 | 1 << 5 | 1).to_be_bytes()); // packed date 1999.01.01
        d.extend_from_slice(&0u16.to_be_bytes()); // kind
        d.extend_from_slice(&[0, 0, 0, 0, 0, 0]); // nation, unknown, rounds
        d.extend_from_slice(&[0, 1, 2]); // subround, round, result
        d
    }

    #[test]
    fn a_classical_quotation_reads_its_header() {
        let q =
            Quotation::parse_classic(&classic("Kasparov, Garry", "Karpov, Anatoly", "Linares")).expect("the layout");
        assert_eq!(q.white.last, "Kasparov");
        assert_eq!(q.white.first, " Garry");
        assert_eq!(q.white.elo, 2450);
        assert_eq!(q.black.last, "Karpov");
        assert_eq!(q.black.elo, 2500);
        assert_eq!(q.event, "Linares");
        assert_eq!(q.site, "Wijk aan Zee");
        assert_eq!(q.date.year(), 1999);
        assert_eq!(q.result, 2);
        assert_eq!(q.eco.pgn().as_deref(), Some("D37"));
    }

    #[test]
    fn chessbase_text_is_what_chessbase_writes() {
        let q =
            Quotation::parse_classic(&classic("Kasparov, Garry", "Karpov, Anatoly", "Linares")).expect("the layout");
        assert_eq!(q.chessbase_text(), "1-0 Kasparov,G (2450)-Karpov,A (2500) Linares Wijk aan Zee 1999 (1)");
    }

    #[test]
    fn a_truncated_quotation_is_not_decoded() {
        assert!(Quotation::parse_classic(&[0x60, 0, 0, 0]).is_none());
        assert!(Quotation::parse_classic(&[]).is_none());
    }
}
