//! The final position of a game: mate, stalemate, check, or none of them.

use cbvault_chess::start::{Start, start_board};
use cbvault_format::game::Eco;
use gigachess::{Board, Move};

use crate::bridge::GameRef;

/// How a game's main line ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ending {
    /// The side to move is checkmated.
    Mate,
    /// The side to move is stalemated.
    Stalemate,
    /// The side to move is in check and has a legal move.
    Check,
    /// None of the above.
    Other,
}

/// The ending of `game`'s main line, or `None` when its start position or a
/// move cannot be replayed. `Board::play` keeps the checkers cache, so
/// `in_check` is exact on the final board (ADR-003).
///
/// A Chess960 game starts from the board its header's start number builds,
/// not from [`GameRef::start_fen`]: that FEN writes the castling rights as
/// `KQkq`, which a parser reads as rooks on the a- and h-files, so the rights
/// of a position whose rooks stand elsewhere would be lost and the first
/// castling of the game would not replay.
pub(crate) fn ending(game: &GameRef<'_>) -> Option<Ending> {
    let mut board = match (game.header.eco(), game.start_fen) {
        (Eco::Chess960(n), _) => start_board(&Start::Chess960(n)).ok()?,
        (_, Some(fen)) => gigachess::fen::parse_fen(fen).ok()?,
        (_, None) => Board::startpos(),
    };
    for &word in game.moves {
        board.play(Move::from_word(word)).ok()?;
    }
    let check = board.in_check();
    let stuck = board.legal_moves().is_empty();
    Some(match (check, stuck) {
        (true, true) => Ending::Mate,
        (false, true) => Ending::Stalemate,
        (true, false) => Ending::Check,
        (false, false) => Ending::Other,
    })
}
