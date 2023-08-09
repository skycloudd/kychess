use chess::{Board, CacheTable, ChessMove, Color, Game, MoveGen, Piece, Square, EMPTY};

const INFINITY: i32 = 1_000_000;

fn main() {
    let mut game = <Game as std::str::FromStr>::from_str(
        // "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        // "rn1Rnk1r/p1p2ppp/2q5/8/8/1Pb2N2/P1P1QPPP/1RB3K1 w - - 2 18",
        "rn1Rnk1r/p1p2ppp/2q5/8/8/BPb2N2/P1P1QPPP/1R4K1 b - - 3 18",
    )
    .unwrap();

    println!("position: {}", game.current_position());

    while game.result().is_none() {
        let (best_move, best_score) = search_root(&game.current_position(), 6);

        println!("played {} {}", best_move, best_score);

        if !game.make_move(best_move) {
            break;
        }

        if game.can_declare_draw() {
            game.declare_draw();
        }
    }

    println!("game over: {:?}", game.result().unwrap());
}

fn search_root(pos: &Board, depth: u8) -> (ChessMove, i32) {
    let mut cache = CacheTable::new(1 << 20, 0);

    let mut best_move = None;
    let mut best_score = -INFINITY - 1;

    let mut legal_moves = MoveGen::new_legal(pos);

    let targets = pos.color_combined(!pos.side_to_move());
    legal_moves.set_iterator_mask(*targets);

    iterate_legals(
        &mut legal_moves,
        &mut best_move,
        &mut best_score,
        pos,
        &mut cache,
        depth,
    );

    legal_moves.set_iterator_mask(!EMPTY);

    iterate_legals(
        &mut legal_moves,
        &mut best_move,
        &mut best_score,
        pos,
        &mut cache,
        depth,
    );

    (best_move.unwrap(), best_score)
}

fn iterate_legals(
    legal_moves: &mut MoveGen,
    best_move: &mut Option<ChessMove>,
    best_score: &mut i32,
    pos: &Board,
    cache: &mut CacheTable<i32>,
    depth: u8,
) {
    for legal in legal_moves {
        let new_pos = pos.make_move_new(legal);

        let position_hash = new_pos.get_hash();

        let score = match cache.get(position_hash) {
            Some(score) => score,
            None => {
                let score = -negamax(&new_pos, cache, depth - 1, -INFINITY, INFINITY);

                cache.add(position_hash, score);

                score
            }
        };

        println!("{} {}", legal, score);

        if score > *best_score {
            *best_score = score;
            *best_move = Some(legal);
        }
    }
}

fn negamax(pos: &Board, cache: &mut CacheTable<i32>, depth: u8, mut alpha: i32, beta: i32) -> i32 {
    if depth == 0 {
        return evaluate(pos);
    }

    for legal in MoveGen::new_legal(pos) {
        let new_pos = pos.make_move_new(legal);

        let position_hash = new_pos.get_hash();

        let score = match cache.get(position_hash) {
            Some(score) => score,
            None => {
                let score = -negamax(&new_pos, cache, depth - 1, -beta, -alpha);

                cache.add(position_hash, score);

                score
            }
        };

        if score >= beta {
            return beta;
        }

        if score > alpha {
            alpha = score;
        }
    }

    alpha
}

fn evaluate(pos: &Board) -> i32 {
    let score = match pos.status() {
        chess::BoardStatus::Ongoing => {
            let mut score = 0;

            for sq in 0..64 {
                let square = unsafe { Square::new(sq) }; // safety: square is always 0..=63

                if let (Some(piece), Some(piece_colour)) =
                    (pos.piece_on(square), pos.color_on(square))
                {
                    let piece_score = match piece {
                        Piece::Pawn => 100,
                        Piece::Knight => 320,
                        Piece::Bishop => 330,
                        Piece::Rook => 500,
                        Piece::Queen => 900,
                        Piece::King => 20000,
                    } + piece_square(&piece, piece_colour, square);

                    score += match piece_colour {
                        Color::White => piece_score,
                        Color::Black => -piece_score,
                    };
                }
            }

            score
        }
        chess::BoardStatus::Stalemate => 0,
        chess::BoardStatus::Checkmate => match pos.side_to_move() {
            Color::White => -INFINITY,
            Color::Black => INFINITY,
        },
    };

    match pos.side_to_move() {
        Color::White => score,
        Color::Black => -score,
    }
}

fn piece_square(piece: &Piece, piece_colour: Color, square: Square) -> i32 {
    let table = match piece {
        Piece::Pawn => PAWN_TABLE,
        Piece::Knight => KNIGHT_TABLE,
        Piece::Bishop => BISHOP_TABLE,
        Piece::Rook => ROOK_TABLE,
        Piece::Queen => QUEEN_TABLE,
        Piece::King => KING_TABLE,
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
