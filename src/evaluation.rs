use chess::{Board, Color, Piece, Square};

use crate::INFINITY;

pub fn evaluate_position(board: &Board) -> i32 {
    let score = match board.status() {
        chess::BoardStatus::Ongoing => {
            let mut score = 0;

            let is_endgame = is_endgame(board);

            for sq in 0..64 {
                let square = unsafe { Square::new(sq) }; // safety: square is always 0..=63

                if let (Some(piece), Some(piece_colour)) =
                    (board.piece_on(square), board.color_on(square))
                {
                    let piece_score = match piece {
                        Piece::Pawn => 100,
                        Piece::Knight => 320,
                        Piece::Bishop => 330,
                        Piece::Rook => 500,
                        Piece::Queen => 900,
                        Piece::King => 20000,
                    } + piece_square(&piece, piece_colour, square, is_endgame);

                    score += match piece_colour {
                        Color::White => piece_score,
                        Color::Black => -piece_score,
                    };
                }
            }

            score
        }
        chess::BoardStatus::Stalemate => 0,
        chess::BoardStatus::Checkmate => match board.side_to_move() {
            Color::White => -INFINITY,
            Color::Black => INFINITY,
        },
    };

    match board.side_to_move() {
        Color::White => score,
        Color::Black => -score,
    }
}

fn piece_square(piece: &Piece, piece_colour: Color, square: Square, is_endgame: bool) -> i32 {
    let table = match piece {
        Piece::Pawn => PAWN_TABLE,
        Piece::Knight => KNIGHT_TABLE,
        Piece::Bishop => BISHOP_TABLE,
        Piece::Rook => ROOK_TABLE,
        Piece::Queen => QUEEN_TABLE,
        Piece::King => {
            if is_endgame {
                KING_TABLE_ENDGAME
            } else {
                KING_TABLE
            }
        }
    };

    let index = match piece_colour {
        Color::White => 63 - square.to_index(),
        Color::Black => square.to_index(),
    };

    table[index]
}

const PAWN_TABLE: [i32; 64] = [
    0, 0, 0, 0, 0, 0, 0, 0, 50, 50, 50, 50, 50, 50, 50, 50, 10, 10, 20, 30, 30, 20, 10, 10, 5, 5,
    10, 25, 25, 10, 5, 5, 0, 0, 0, 20, 20, 0, 0, 0, 5, -5, -10, 0, 0, -10, -5, 5, 5, 10, 10, -20,
    -20, 10, 10, 5, 0, 0, 0, 0, 0, 0, 0, 0,
];

const KNIGHT_TABLE: [i32; 64] = [
    -50, -40, -30, -30, -30, -30, -40, -50, -40, -20, 0, 0, 0, 0, -20, -40, -30, 0, 10, 15, 15, 10,
    0, -30, -30, 5, 15, 20, 20, 15, 5, -30, -30, 0, 15, 20, 20, 15, 0, -30, -30, 5, 10, 15, 15, 10,
    5, -30, -40, -20, 0, 5, 5, 0, -20, -40, -50, -40, -30, -30, -30, -30, -40, -50,
];

const BISHOP_TABLE: [i32; 64] = [
    -20, -10, -10, -10, -10, -10, -10, -20, -10, 0, 0, 0, 0, 0, 0, -10, -10, 0, 5, 10, 10, 5, 0,
    -10, -10, 5, 5, 10, 10, 5, 5, -10, -10, 0, 10, 10, 10, 10, 0, -10, -10, 10, 10, 10, 10, 10, 10,
    -10, -10, 5, 0, 0, 0, 0, 5, -10, -20, -10, -10, -10, -10, -10, -10, -20,
];

const ROOK_TABLE: [i32; 64] = [
    0, 0, 0, 0, 0, 0, 0, 0, 5, 10, 10, 10, 10, 10, 10, 5, -5, 0, 0, 0, 0, 0, 0, -5, -5, 0, 0, 0, 0,
    0, 0, -5, -5, 0, 0, 0, 0, 0, 0, -5, -5, 0, 0, 0, 0, 0, 0, -5, -5, 0, 0, 0, 0, 0, 0, -5, 0, 0,
    0, 5, 5, 0, 0, 0,
];

const QUEEN_TABLE: [i32; 64] = [
    -20, -10, -10, -5, -5, -10, -10, -20, -10, 0, 0, 0, 0, 0, 0, -10, -10, 0, 5, 5, 5, 5, 0, -10,
    -5, 0, 5, 5, 5, 5, 0, -5, 0, 0, 5, 5, 5, 5, 0, -5, -10, 5, 5, 5, 5, 5, 0, -10, -10, 0, 5, 0, 0,
    0, 0, -10, -20, -10, -10, -5, -5, -10, -10, -20,
];

const KING_TABLE: [i32; 64] = [
    -30, -40, -40, -50, -50, -40, -40, -30, -30, -40, -40, -50, -50, -40, -40, -30, -30, -40, -40,
    -50, -50, -40, -40, -30, -30, -40, -40, -50, -50, -40, -40, -30, -20, -30, -30, -40, -40, -30,
    -30, -20, -10, -20, -20, -20, -20, -20, -20, -10, 20, 20, 0, 0, 0, 0, 20, 20, 20, 30, 10, 0, 0,
    10, 30, 20,
];

const KING_TABLE_ENDGAME: [i32; 64] = [
    -50, -40, -30, -20, -20, -30, -40, -50, -30, -20, -10, 0, 0, -10, -20, -30, -30, -10, 20, 30,
    30, 20, -10, -30, -30, -10, 30, 40, 40, 30, -10, -30, -30, -10, 30, 40, 40, 30, -10, -30, -30,
    -10, 20, 30, 30, 20, -10, -30, -30, -30, 0, 0, 0, 0, -30, -30, -50, -30, -30, -30, -30, -30,
    -30, -50,
];

fn is_endgame(board: &Board) -> bool {
    if board.pieces(Piece::Queen).0.count_ones() == 0 {
        true
    } else {
        let white_queens = (board.pieces(Piece::Queen) & board.color_combined(Color::White))
            .0
            .count_ones();
        let black_queens = (board.pieces(Piece::Queen) & board.color_combined(Color::Black))
            .0
            .count_ones();

        let white_endgame = if white_queens == 1 {
            let white_minor_pieces = ((board.pieces(Piece::Knight) | board.pieces(Piece::Bishop))
                & board.color_combined(Color::White))
            .0
            .count_ones();

            let white_rooks = (board.pieces(Piece::Rook) & board.color_combined(Color::White))
                .0
                .count_ones();

            if white_minor_pieces <= 1 && white_rooks == 0 {
                true
            } else {
                false
            }
        } else {
            false
        };

        let black_endgame = if black_queens == 1 {
            let black_minor_pieces = ((board.pieces(Piece::Knight) | board.pieces(Piece::Bishop))
                & board.color_combined(Color::Black))
            .0
            .count_ones();

            let black_rooks = (board.pieces(Piece::Rook) & board.color_combined(Color::Black))
                .0
                .count_ones();

            if black_minor_pieces <= 1 && black_rooks == 0 {
                true
            } else {
                false
            }
        } else {
            false
        };

        white_endgame && black_endgame
    }
}
