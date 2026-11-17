//! The colour ChessBase gives a book move.
//!
//! Original cbvault code, clean-room. The rule was read off ChessBase's display
//! of two real books (HIARCS 15 and Fritz 20; 11 screenshots taken by the contributor) and
//! of a test book the contributor annotated with `!!` and `??`. It reproduces every move
//! shown. See `docs/research/ctg-observations-2026-11-14.md`.
//!
//! The colour depends on two bytes in two entries: the move's annotation byte
//! (in the position the move is played from) and offset 30 of the statistics of
//! the position the move leads to:
//!
//! Offset 30 holds ChessBase's move marks: `0x80` is "Main move" and `0x40`
//! "Don't play in tournament" (see [`super::stats::MAIN_MOVE`]).
//!
//! 1. **red** when that offset 30 is `0x40`;
//! 2. otherwise **blue** when the move's symbol is `?`, `??`, `!?` or `?!`;
//! 3. otherwise, for a move with no symbol or with `!`: **green** when offset 30
//!    is `0x80`, **black** when it is `0x00`; `!!` with `0x00` is black too.
//!
//! The annotation's class (its high 3 bits) played no part in any observed case.
//! Position evaluations (offset 32) never change the colour. Combinations
//! nothing showed (other symbols, other offset 30 values, `!!` or `??` with
//! `0x80`) give no colour rather than a guess.

use super::moves::Annotation;
use super::stats::Stats;

/// A move colour as ChessBase shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Colour {
    /// Green.
    Green,
    /// Blue.
    Blue,
    /// Red.
    Red,
    /// Black (plain text).
    Black,
}

impl Colour {
    /// The colour's name in lower case.
    pub fn as_str(self) -> &'static str {
        match self {
            Colour::Green => "green",
            Colour::Blue => "blue",
            Colour::Red => "red",
            Colour::Black => "black",
        }
    }
}

/// The colour of a move with annotation `annotation` leading to a position with
/// statistics `after`, when the combination was observed.
pub fn colour(annotation: Annotation, after: &Stats) -> Option<Colour> {
    let flag = after.raw_tail[0];
    let code = annotation.symbol_code();
    match (flag, code) {
        (0x40, 0 | 2) => Some(Colour::Red),
        (0x00 | 0x80, 2 | 5 | 6) | (0x00, 4) => Some(Colour::Blue),
        (0x80, 0 | 1) => Some(Colour::Green),
        (0x00, 0 | 1 | 3) => Some(Colour::Black),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn after(flag: u8) -> Stats {
        Stats { raw_tail: [flag, 0, 0], ..Stats::default() }
    }

    /// One case per screenshot row kind, with the move it came from.
    #[test]
    fn the_observed_cases_reproduce_chessbase() {
        // HIARCS root: 1.e4! green, 1.c4 green, 1.e3!? blue, 1.Nc3? red.
        assert_eq!(colour(Annotation(0xe1), &after(0x80)), Some(Colour::Green));
        assert_eq!(colour(Annotation(0xc0), &after(0x80)), Some(Colour::Green));
        assert_eq!(colour(Annotation(0x25), &after(0x80)), Some(Colour::Blue));
        assert_eq!(colour(Annotation(0x22), &after(0x40)), Some(Colour::Red));
        // HIARCS 9.e5? blue (same byte as a red move, different flag).
        assert_eq!(colour(Annotation(0x22), &after(0x80)), Some(Colour::Blue));
        // HIARCS 1.d4 f5 2.h3 Nf6: class 1, no symbol, green.
        assert_eq!(colour(Annotation(0x20), &after(0x80)), Some(Colour::Green));
        // Fritz 20: 1.b3 b6 2.e4 black; 4...Bc5 red; 4.Bxc6? blue; 4.d4?! blue.
        assert_eq!(colour(Annotation(0x00), &after(0x00)), Some(Colour::Black));
        assert_eq!(colour(Annotation(0x00), &after(0x40)), Some(Colour::Red));
        assert_eq!(colour(Annotation(0x02), &after(0x00)), Some(Colour::Blue));
        assert_eq!(colour(Annotation(0x06), &after(0x80)), Some(Colour::Blue));
        // Test book: 8.axb4?? blue, 8...Nd3#!! black.
        assert_eq!(colour(Annotation(0x04), &after(0x00)), Some(Colour::Blue));
        assert_eq!(colour(Annotation(0x03), &after(0x00)), Some(Colour::Black));
        // Test book: 4.Bf4! black (no "Main move" mark).
        assert_eq!(colour(Annotation(0x01), &after(0x00)), Some(Colour::Black));
    }

    #[test]
    fn unobserved_combinations_have_no_colour() {
        assert_eq!(colour(Annotation(0x03), &after(0x80)), None, "!! with 0x80 was never shown");
        assert_eq!(colour(Annotation(0x00), &after(0xc0)), None, "flag 0xc0 was never seen");
    }
}
