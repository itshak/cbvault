//! The position evaluation a book can store (ChessBase's "+−, =, …" menu).
//!
//! Original cbvault code, clean-room. Established by setting evaluations in a
//! test book in ChessBase and reading the bytes back (see
//! `docs/research/ctg-observations-2026-11-14.md`).
//!
//! The code sits in offset 32 of the statistics of the position the evaluated
//! move leads to, numbered as PGN NAGs. Because the book stores every position
//! with White to move, the **advantage family** (codes 14–19: ⩲ ⩱ ± ∓ +− −+) is
//! stored for the stored board. When Black is to move in the real position,
//! the advantage belongs to the other colour: 1.d4 marked ⩲ is stored as 15, and
//! 7.a3 marked +− is stored as 19. The other codes seen (∞, ⇆, →, ↑, N and the
//! compensation sign) are stored as they are, whoever is to move.

/// A stored evaluation code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Evaluation(pub u8);

impl Evaluation {
    /// The evaluation as it reads for the real position, given whether Black is
    /// to move there (the stored board was then flipped).
    pub fn for_real_position(self, black_to_move: bool) -> Evaluation {
        match self.0 {
            14..=19 if black_to_move => Evaluation(self.0 ^ 1),
            _ => self,
        }
    }

    /// The symbol ChessBase shows, for the codes confirmed so far.
    ///
    /// 16/17 were only seen with White to move; their flip is inferred from the
    /// pairs 14/15 and 18/19, which were seen flipped. ChessBase draws
    /// compensation (44) as a single sign, written here `=/∞`.
    pub fn symbol(self) -> Option<&'static str> {
        Some(match self.0 {
            13 => "∞",
            14 => "⩲",
            15 => "⩱",
            16 => "±",
            17 => "∓",
            18 => "+−",
            19 => "−+",
            36 => "↑",
            40 => "→",
            44 => "=/∞",
            132 => "⇆",
            146 => "N",
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_advantage_family_flips_when_black_is_to_move() {
        // 1.d4 marked ⩲ (Black to move after it): stored 15.
        assert_eq!(Evaluation(15).for_real_position(true).symbol(), Some("⩲"));
        // 7.a3 marked +−: stored 19.
        assert_eq!(Evaluation(19).for_real_position(true).symbol(), Some("+−"));
        // White to move: as stored (1...Nf6 ±, 3...Ng4 ⩲, 7...Ngxe5 ∓).
        assert_eq!(Evaluation(16).for_real_position(false).symbol(), Some("±"));
        assert_eq!(Evaluation(14).for_real_position(false).symbol(), Some("⩲"));
        assert_eq!(Evaluation(17).for_real_position(false).symbol(), Some("∓"));
    }

    #[test]
    fn neutral_codes_do_not_flip() {
        // 2.c4 ∞, 3.dxe5 ⇆, 6.Nbd2 → were all set on Black-to-move positions.
        for (code, sym) in [(13, "∞"), (132, "⇆"), (40, "→")] {
            assert_eq!(Evaluation(code).for_real_position(true).symbol(), Some(sym));
        }
        assert_eq!(Evaluation(36).symbol(), Some("↑"));
        assert_eq!(Evaluation(146).symbol(), Some("N"));
        // 5.Nf3 marked with compensation, Black to move after it: stored 44.
        assert_eq!(Evaluation(44).for_real_position(true).symbol(), Some("=/∞"));
    }
}
