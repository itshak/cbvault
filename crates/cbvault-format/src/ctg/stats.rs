//! The 33-byte statistics block that closes every book entry.
//!
//! Original cbvault code, clean-room (see
//! `docs/research/ctg-observations-2026-11-14.md`). Fields whose meaning is not
//! established are exposed raw, under names that say so.

/// Size of the block in bytes.
pub const STATS_LEN: usize = 33;

/// Offset 30 bit: ChessBase's **"Main move"** mark on the move that leads to
/// this position (the move is then shown green). Established by marking a move
/// in a test book and reading the byte back.
pub const MAIN_MOVE: u8 = 0x80;

/// Offset 30 bit: ChessBase's **"Don't play in tournament"** mark on the move
/// that leads to this position (the move is then shown red). Established the
/// same way.
pub const DONT_PLAY: u8 = 0x40;

/// The statistics of a stored position (big-endian fields).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Stats {
    /// Games that reached the position.
    pub games: u32,
    /// Games won by the side to move of the stored position.
    pub wins: u32,
    /// Games lost by that side.
    pub losses: u32,
    /// Drawn games.
    pub draws: u32,
    /// Offset 12, u32: **meaning not established.** It equals
    /// `10 * wins + 5 * draws` on every entry of one book and on most entries
    /// of others, but not on all.
    pub raw_score: u32,
    /// Games in which the side to move has a rating.
    pub rated_games: u32,
    /// Sum of those ratings of the side to move.
    pub rating_sum: u32,
    /// Games in which the other side (the one that just moved) has a rating.
    pub opponent_rated_games: u32,
    /// Sum of those ratings of the other side.
    ///
    /// ChessBase's "Av" for a book move is this pair's average, truncated, in
    /// the position the move leads to: it matched on all 20 root moves of a
    /// real book.
    pub opponent_rating_sum: u32,
    /// Offsets 30–32. Offset 30 holds the move marks [`MAIN_MOVE`] and
    /// [`DONT_PLAY`] of the move that leads to this position (see
    /// [`Stats::main_move`], [`Stats::dont_play`] and [`super::colour()`]).
    /// Offset 31 is not established. Offset 32 holds the position evaluation
    /// set on the move leading here, if any (see [`Stats::evaluation`]).
    pub raw_tail: [u8; 3],
}

fn u24(b: &[u8; STATS_LEN], at: usize) -> u32 {
    u32::from(b[at]) << 16 | u32::from(b[at + 1]) << 8 | u32::from(b[at + 2])
}

fn u32be(b: &[u8; STATS_LEN], at: usize) -> u32 {
    u32::from_be_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

impl Stats {
    /// Parses the block.
    pub fn parse(b: &[u8; STATS_LEN]) -> Stats {
        Stats {
            games: u24(b, 0),
            wins: u24(b, 3),
            losses: u24(b, 6),
            draws: u24(b, 9),
            raw_score: u32be(b, 12),
            rated_games: u24(b, 16),
            rating_sum: u32be(b, 19),
            opponent_rated_games: u24(b, 23),
            opponent_rating_sum: u32be(b, 26),
            raw_tail: [b[30], b[31], b[32]],
        }
    }

    /// The stored evaluation, if one was set. Use
    /// [`super::Evaluation::for_real_position`] to orient it.
    pub fn evaluation(&self) -> Option<super::Evaluation> {
        (self.raw_tail[2] != 0).then_some(super::Evaluation(self.raw_tail[2]))
    }

    /// Whether the move leading here is marked "Main move".
    pub fn main_move(&self) -> bool {
        self.raw_tail[0] & MAIN_MOVE != 0
    }

    /// Whether the move leading here is marked "Don't play in tournament".
    pub fn dont_play(&self) -> bool {
        self.raw_tail[0] & DONT_PLAY != 0
    }

    /// Games with a result: `wins + draws + losses`. This is ChessBase's "N",
    /// and its percentages are taken over it; `games` (which can be larger)
    /// is the total it shows in parentheses.
    pub fn results(&self) -> u32 {
        self.wins.saturating_add(self.draws).saturating_add(self.losses)
    }

    /// The side to move's score in per cent over [`Stats::results`].
    pub fn score_percent(&self) -> Option<f64> {
        let n = self.results();
        (n > 0).then(|| (f64::from(self.wins) + f64::from(self.draws) / 2.0) * 100.0 / f64::from(n))
    }

    /// The share of draws in per cent over [`Stats::results`].
    pub fn draw_percent(&self) -> Option<f64> {
        let n = self.results();
        (n > 0).then(|| f64::from(self.draws) * 100.0 / f64::from(n))
    }

    /// The side to move's average rating (truncated), when any game has one.
    pub fn average_rating(&self) -> Option<u32> {
        (self.rated_games > 0).then(|| self.rating_sum / self.rated_games)
    }

    /// The other side's average rating (truncated), when any game has one.
    pub fn opponent_average_rating(&self) -> Option<u32> {
        (self.opponent_rated_games > 0).then(|| self.opponent_rating_sum / self.opponent_rated_games)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_synthetic_block_parses_field_by_field() {
        // Synthetic values, one per field, so a misplaced offset shows up.
        let mut b = [0u8; STATS_LEN];
        b[0..3].copy_from_slice(&[0x01, 0x02, 0x03]); // games
        b[3..6].copy_from_slice(&[0x00, 0x10, 0x00]); // wins
        b[6..9].copy_from_slice(&[0x00, 0x08, 0x00]); // losses
        b[9..12].copy_from_slice(&[0x00, 0x20, 0x00]); // draws
        b[12..16].copy_from_slice(&[0x00, 0x01, 0x40, 0x00]); // raw score
        b[16..19].copy_from_slice(&[0x00, 0x00, 0x64]); // rated games: 100
        b[19..23].copy_from_slice(&250_000u32.to_be_bytes()); // rating sum
        b[23..26].copy_from_slice(&[0x00, 0x00, 0x32]); // other side's rated games: 50
        b[26..30].copy_from_slice(&120_000u32.to_be_bytes()); // other side's rating sum
        b[30..33].copy_from_slice(&[0x80, 0x00, 0x0e]);
        let s = Stats::parse(&b);
        assert_eq!((s.games, s.wins, s.losses, s.draws), (0x01_0203, 0x1000, 0x0800, 0x2000));
        assert_eq!(s.raw_score, 0x0001_4000);
        assert_eq!((s.rated_games, s.average_rating()), (100, Some(2_500)));
        assert_eq!((s.opponent_rated_games, s.opponent_average_rating()), (50, Some(2_400)));
        assert!(s.main_move() && !s.dont_play());
        assert_eq!(s.evaluation(), Some(super::super::Evaluation(14)));
    }

    #[test]
    fn no_rated_games_means_no_average() {
        assert_eq!(Stats::default().average_rating(), None);
        assert_eq!(Stats::default().opponent_average_rating(), None);
        assert_eq!(Stats::default().score_percent(), None);
    }

    #[test]
    fn move_marks_read_from_offset_30() {
        // Test book: after 6.Nbd2 ("Main move") offset 30 is 0x80,
        // after 6...Qe7 ("Don't play in tournament") it is 0x40.
        let main = Stats { raw_tail: [0x80, 0, 0], ..Stats::default() };
        let dont = Stats { raw_tail: [0x40, 0, 0], ..Stats::default() };
        assert!(main.main_move() && !main.dont_play());
        assert!(dont.dont_play() && !dont.main_move());
        assert!(!Stats::default().main_move() && !Stats::default().dont_play());
    }

    #[test]
    fn chessbase_percentages_are_over_games_with_a_result() {
        // Fritz 20, 1.e4 e5 2.Nf3 Nc6 3.Bb5 f5 4.d3 as ChessBase shows it: N 216
        // (222 games), 54.4% for the mover, 32.9% draws. The stored position is
        // after the move, so its side to move scores 45.6%.
        let s = Stats { games: 222, wins: 63, draws: 71, losses: 82, ..Stats::default() };
        assert_eq!(s.results(), 216);
        assert!((s.score_percent().unwrap() - 45.6).abs() < 0.05);
        assert!((s.draw_percent().unwrap() - 32.9).abs() < 0.05);
    }
}
