use chess::{Board, CacheTable, ChessMove, Color, Game, MoveGen, Piece, Square, EMPTY};

const VALUE_MATED: i32 = std::i32::MIN / 2;
const VALUE_MATE: i32 = std::i32::MAX / 2;

fn main() {
    // let mut game = Game::new();

    let mut game = <Game as std::str::FromStr>::from_str(
        "rn1Rnk1r/p1p2ppp/2q5/8/8/1Pb2N2/P1P1QPPP/1RB3K1 w - - 2 18",
        // "5Kbk/6pp/6P1/8/8/8/8/7R w - -",
        // "7k/3R3P/5K2/8/2b5/8/8/8 w - - 7 7",
        // "7k/8/8/6K1/5R2/8/8/8 w - - 9 8",
    )
    .unwrap();

    println!("position: {}", game.current_position());

    while game.result().is_none() {
        let best_move = search_root(&game.current_position(), 6);

        println!("{} ", best_move);

        if !game.make_move(best_move) {
            break;
        }

        if game.can_declare_draw() {
            game.declare_draw();
        }
    }

    println!("game over: {:?}", game.result().unwrap());
}

fn search_root(pos: &Board, depth: u8) -> ChessMove {
    let mut cache = CacheTable::new(1 << 20, 0);

    let mut best_move = None;
    let mut best_score = VALUE_MATED;

    let mut legal_moves = MoveGen::new_legal(pos);

    if legal_moves.len() == 1 {
        return legal_moves.next().unwrap();
    }

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

    // println!(
    //     "info depth {} nodes {} score cp {:?} pv {}",
    //     depth,
    //     nodes_searched,
    //     best_score,
    //     best_move.unwrap()
    // );

    best_move.unwrap()
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
                let score = -negamax(&new_pos, cache, depth - 1, VALUE_MATED, VALUE_MATE);

                cache.add(position_hash, score);

                score
            }
        };

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

    let mut best_score = VALUE_MATED;

    let legal_moves = MoveGen::new_legal(pos);

    for legal in legal_moves {
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
            return score;
        }

        if score > best_score {
            best_score = score;
        }

        if score > alpha {
            alpha = score;
        }
    }

    best_score
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
            Color::White => VALUE_MATED,
            Color::Black => VALUE_MATE,
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
    0, 0, 0, 0, 0, 0, 0, 0, //
    50, 50, 50, 50, 50, 50, 50, 50, //
    10, 10, 20, 30, 30, 20, 10, 10, //
    5, 5, 10, 25, 25, 10, 5, 5, //
    0, 0, 0, 20, 20, 0, 0, 0, //
    5, -5, -10, 0, 0, -10, -5, 5, //
    5, 10, 10, -20, -20, 10, 10, 5, //
    0, 0, 0, 0, 0, 0, 0, 0, //
];

const KNIGHT_TABLE: [i32; 64] = [
    -50, -40, -30, -30, -30, -30, -40, -50, //
    -40, -20, 0, 0, 0, 0, -20, -40, //
    -30, 0, 10, 15, 15, 10, 0, -30, //
    -30, 5, 15, 20, 20, 15, 5, -30, //
    -30, 0, 15, 20, 20, 15, 0, -30, //
    -30, 5, 10, 15, 15, 10, 5, -30, //
    -40, -20, 0, 5, 5, 0, -20, -40, //
    -50, -40, -30, -30, -30, -30, -40, -50, //
];

const BISHOP_TABLE: [i32; 64] = [
    -20, -10, -10, -10, -10, -10, -10, -20, //
    -10, 0, 0, 0, 0, 0, 0, -10, //
    -10, 0, 5, 10, 10, 5, 0, -10, //
    -10, 5, 5, 10, 10, 5, 5, -10, //
    -10, 0, 10, 10, 10, 10, 0, -10, //
    -10, 10, 10, 10, 10, 10, 10, -10, //
    -10, 5, 0, 0, 0, 0, 5, -10, //
    -20, -10, -10, -10, -10, -10, -10, -20, //
];

const ROOK_TABLE: [i32; 64] = [
    0, 0, 0, 0, 0, 0, 0, 0, //
    5, 10, 10, 10, 10, 10, 10, 5, //
    -5, 0, 0, 0, 0, 0, 0, -5, //
    -5, 0, 0, 0, 0, 0, 0, -5, //
    -5, 0, 0, 0, 0, 0, 0, -5, //
    -5, 0, 0, 0, 0, 0, 0, -5, //
    -5, 0, 0, 0, 0, 0, 0, -5, //
    0, 0, 0, 5, 5, 0, 0, 0, //
];

const QUEEN_TABLE: [i32; 64] = [
    -20, -10, -10, -5, -5, -10, -10, -20, -10, 0, 0, 0, 0, 0, 0, -10, -10, 0, 5, 5, 5, 5, 0, -10,
    -5, 0, 5, 5, 5, 5, 0, -5, 0, 0, 5, 5, 5, 5, 0, -5, -10, 5, 5, 5, 5, 5, 0, -10, -10, 0, 5, 0, 0,
    0, 0, -10, -20, -10, -10, -5, -5, -10, -10, -20,
];

const KING_TABLE: [i32; 64] = [
    -30, -40, -40, -50, -50, -40, -40, -30, //
    -30, -40, -40, -50, -50, -40, -40, -30, //
    -30, -40, -40, -50, -50, -40, -40, -30, //
    -30, -40, -40, -50, -50, -40, -40, -30, //
    -20, -30, -30, -40, -40, -30, -30, -20, //
    -10, -20, -20, -20, -20, -20, -20, -10, //
    20, 20, 0, 0, 0, 0, 20, 20, //
    20, 30, 10, 0, 0, 10, 30, 20, //
];
