use crate::evaluation::evaluate_position;
use crate::uci::GameTime;
use crate::{Information, INFINITY};
use chess::{Board, CacheTable, ChessMove, Color, MoveGen, Piece, EMPTY};
use crossbeam_channel::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub struct Search {
    handle: Option<JoinHandle<()>>,
    control_tx: Option<Sender<SearchCommand>>,
}

impl Search {
    pub fn new() -> Self {
        Self {
            handle: None,
            control_tx: None,
        }
    }

    pub fn init(&mut self, info_tx: Sender<Information>, board: Arc<Mutex<Board>>) {
        let (control_tx, control_rx) = crossbeam_channel::unbounded::<SearchCommand>();

        let h = thread::spawn(move || {
            let mut search_params = None;

            let mut quit = false;
            let mut halt = true;

            while !quit {
                let cmd = control_rx.recv().unwrap();

                match cmd {
                    SearchCommand::Start(sp) => {
                        search_params = Some(sp);
                        halt = false;
                    }
                    SearchCommand::Stop => halt = true,
                    SearchCommand::Quit => quit = true,
                    SearchCommand::Nothing => (),
                }

                if !halt && !quit {
                    let board = Arc::clone(&board);

                    let mut refs = SearchRefs {
                        board: &mut board.lock().unwrap(),
                        search_params: search_params.as_ref().unwrap(),
                        transposition_table: &mut CacheTable::new(1 << 21, SearchData::default()),
                        search_state: &mut SearchState::new(),
                        control_rx: &control_rx,
                        report_tx: &info_tx,
                    };

                    let (best_move, terminate) = Self::iterative_deepening(&mut refs);

                    let info = SearchInformation::BestMove(best_move);
                    info_tx.send(Information::SearchInformation(info)).unwrap();

                    match terminate {
                        SearchTerminate::Stop => {
                            halt = true;
                        }
                        SearchTerminate::Quit => {
                            halt = true;
                            quit = true;
                        }
                        SearchTerminate::Nothing => (),
                    }
                }
            }
        });

        self.handle = Some(h);
        self.control_tx = Some(control_tx);
    }

    pub fn send(&self, cmd: SearchCommand) {
        if let Some(tx) = &self.control_tx {
            tx.send(cmd).unwrap();
        }
    }

    fn iterative_deepening(refs: &mut SearchRefs) -> (ChessMove, SearchTerminate) {
        let mut depth = 1;
        let mut best_move = None;
        let mut root_pv = Vec::new();
        let mut stop = false;

        if refs.search_params.search_mode == SearchMode::GameTime {
            // calculate total available time for this move
            let game_time = &refs.search_params.game_time;

            let is_white = refs.board.side_to_move() == chess::Color::White;

            let clock = if is_white {
                game_time.white_time.unwrap()
            } else {
                game_time.black_time.unwrap()
            };

            let increment = if is_white {
                game_time
                    .white_increment
                    .unwrap_or(Duration::from_millis(0))
            } else {
                game_time
                    .black_increment
                    .unwrap_or(Duration::from_millis(0))
            };

            let base_time = match game_time.moves_to_go {
                Some(mtg) => {
                    if mtg == 0 {
                        clock
                    } else {
                        clock / mtg as u32
                    }
                }
                None => clock / 20,
            };

            let time_slice = base_time + increment - Duration::from_millis(100);

            let factor = 0.4;

            refs.search_state.allocated_time = time_slice.mul_f64(factor);

            refs.report_tx
                .send(Information::SearchInformation(
                    SearchInformation::ExtraInfo(format!(
                        "allocated time: {:?}",
                        refs.search_state.allocated_time
                    )),
                ))
                .unwrap();
        }

        let alpha = -INFINITY;
        let beta = INFINITY;

        refs.search_state.start_time = Some(Instant::now());

        while (depth < 255) && !stop {
            refs.search_state.depth = depth;

            let eval = Self::negamax(refs, &mut root_pv, depth, alpha, beta);

            if refs.search_state.terminate == SearchTerminate::Nothing {
                if !root_pv.is_empty() {
                    best_move = Some(root_pv[0]);
                }

                let elapsed = refs.search_state.start_time.unwrap().elapsed();

                let summary = SearchSummary {
                    depth,
                    seldepth: refs.search_state.seldepth,
                    time: elapsed,
                    cp: eval,
                    nodes: refs.search_state.nodes,
                    nps: (refs.search_state.nodes as f64 / elapsed.as_secs_f64()) as u64,
                    pv: root_pv.clone(),
                };

                let info = SearchInformation::Summary(summary);

                refs.report_tx
                    .send(Information::SearchInformation(info))
                    .unwrap();

                depth += 1;
            }

            if refs.search_state.terminate != SearchTerminate::Nothing {
                stop = true;
            }
        }

        (best_move.unwrap(), refs.search_state.terminate)
    }

    fn negamax(
        refs: &mut SearchRefs,
        pv: &mut Vec<ChessMove>,
        mut depth: u8,
        mut alpha: i32,
        beta: i32,
    ) -> i32 {
        let is_root = refs.search_state.ply == 0;
        let mut do_pvs = false;

        if refs.search_state.nodes & 0x7ff == 0 {
            check_terminate(refs);
        }

        if refs.search_state.terminate != SearchTerminate::Nothing {
            return 0;
        }

        if refs.search_state.ply >= 255 {
            // return Search::quiescence(alpha, beta, pv, refs);
            return evaluate_position(refs.board);
        }

        let is_check = *refs.board.checkers() != EMPTY;

        if is_check {
            depth += 1;
        }

        if depth <= 0 {
            return Search::quiescence(alpha, beta, pv, refs);
        }

        refs.search_state.nodes += 1;

        let position_hash = refs.board.get_hash();

        if let Some(tt_entry) = refs.transposition_table.get(position_hash) {
            if let Some((tt_value, _)) = tt_entry.get(depth, refs.search_state.ply, alpha, beta) {
                if !is_root {
                    return tt_value;
                }
            }
        }

        let mut best_eval_score = -INFINITY - 1;
        let mut best_move = None;

        let mut hash_flag = HashFlag::Alpha;

        let mut legal_moves_found = 0;

        for legal in MoveGen::new_legal(refs.board) {
            let old_pos = *refs.board;
            *refs.board = refs.board.make_move_new(legal);

            legal_moves_found += 1;
            refs.search_state.ply += 1;

            if refs.search_state.ply > refs.search_state.seldepth {
                refs.search_state.seldepth = refs.search_state.ply;
            }

            let mut node_pv = Vec::new();

            let mut eval_score = 0;

            if !is_draw(refs) {
                if do_pvs {
                    eval_score = -Self::negamax(refs, &mut node_pv, depth - 1, -alpha - 1, -alpha);

                    if eval_score > alpha && eval_score < beta {
                        eval_score = -Self::negamax(refs, &mut node_pv, depth - 1, -beta, -alpha);
                    }
                } else {
                    eval_score = -Self::negamax(refs, &mut node_pv, depth - 1, -beta, -alpha);
                }
            }

            refs.search_state.ply -= 1;

            *refs.board = old_pos;

            if eval_score > best_eval_score {
                best_eval_score = eval_score;

                best_move = Some(legal);
            }

            if eval_score >= beta {
                refs.transposition_table.add(
                    position_hash,
                    SearchData::create(
                        depth,
                        refs.search_state.ply,
                        HashFlag::Beta,
                        beta,
                        best_move.unwrap(),
                    ),
                );

                return beta;
            }

            if eval_score > alpha {
                alpha = eval_score;

                hash_flag = HashFlag::Exact;

                do_pvs = true;

                pv.clear();
                pv.push(legal);
                pv.append(&mut node_pv);
            }
        }

        if legal_moves_found == 0 {
            if is_check {
                return -INFINITY + refs.search_state.ply as i32;
            } else {
                return 0;
            }
        }

        refs.transposition_table.add(
            position_hash,
            SearchData::create(
                depth,
                refs.search_state.ply,
                hash_flag,
                alpha,
                best_move.unwrap(),
            ),
        );

        alpha
    }

    fn quiescence(
        mut alpha: i32,
        beta: i32,
        pv: &mut Vec<ChessMove>,
        refs: &mut SearchRefs,
    ) -> i32 {
        refs.search_state.nodes += 1;

        if refs.search_state.nodes & 0x7ff == 0 {
            check_terminate(refs);
        }

        if refs.search_state.terminate != SearchTerminate::Nothing {
            return 0;
        }

        if refs.search_state.ply >= 255 {
            return evaluate_position(refs.board);
        }

        let eval_score = evaluate_position(refs.board);

        if eval_score >= beta {
            return beta;
        }

        if eval_score > alpha {
            alpha = eval_score
        }

        let mut legal_moves = MoveGen::new_legal(refs.board);

        let targets = refs.board.color_combined(!refs.board.side_to_move());
        legal_moves.set_iterator_mask(*targets);

        for legal in legal_moves {
            let old_board = *refs.board;
            *refs.board = refs.board.make_move_new(legal);

            refs.search_state.ply += 1;

            if refs.search_state.ply > refs.search_state.seldepth {
                refs.search_state.seldepth = refs.search_state.ply;
            }

            let mut node_pv: Vec<ChessMove> = Vec::new();

            let score = -Self::quiescence(-beta, -alpha, &mut node_pv, refs);

            refs.search_state.ply -= 1;

            *refs.board = old_board;

            if score >= beta {
                return beta;
            }

            if score > alpha {
                alpha = score;

                pv.clear();
                pv.push(legal);
                pv.append(&mut node_pv);
            }
        }

        alpha
    }
}

fn is_draw(refs: &mut SearchRefs) -> bool {
    // is_repition(refs) || (refs.halfmove_clock >= 100) ||
    is_insufficient_material(refs)
}

fn is_insufficient_material(refs: &mut SearchRefs) -> bool {
    let white_pawn_count = (refs.board.pieces(Piece::Pawn)
        & refs.board.color_combined(Color::White))
    .0
    .count_ones();

    let black_pawn_count = (refs.board.pieces(Piece::Pawn)
        & refs.board.color_combined(Color::Black))
    .0
    .count_ones();

    let white_bishop_count = (refs.board.pieces(Piece::Bishop)
        & refs.board.color_combined(Color::White))
    .0
    .count_ones();
    let black_bishop_count = (refs.board.pieces(Piece::Bishop)
        & refs.board.color_combined(Color::Black))
    .0
    .count_ones();

    let white_knight_count = (refs.board.pieces(Piece::Knight)
        & refs.board.color_combined(Color::White))
    .0
    .count_ones();
    let black_knight_count = (refs.board.pieces(Piece::Knight)
        & refs.board.color_combined(Color::Black))
    .0
    .count_ones();

    let white_rook_count = (refs.board.pieces(Piece::Rook)
        & refs.board.color_combined(Color::White))
    .0
    .count_ones();
    let black_rook_count = (refs.board.pieces(Piece::Rook)
        & refs.board.color_combined(Color::Black))
    .0
    .count_ones();

    let white_queen_count = (refs.board.pieces(Piece::Queen)
        & refs.board.color_combined(Color::White))
    .0
    .count_ones();
    let black_queen_count = (refs.board.pieces(Piece::Queen)
        & refs.board.color_combined(Color::Black))
    .0
    .count_ones();

    if white_pawn_count > 0
        || black_pawn_count > 0
        || white_rook_count > 0
        || black_rook_count > 0
        || white_queen_count > 0
        || black_queen_count > 0
    {
        return false;
    }

    if white_bishop_count <= 1 && black_bishop_count == 0 {
        return true;
    }

    if white_bishop_count == 0 && black_bishop_count <= 1 {
        return true;
    }

    if white_knight_count <= 1 && black_knight_count == 0 {
        return true;
    }

    if white_knight_count == 0 && black_knight_count <= 1 {
        return true;
    }

    false
}

fn check_terminate(refs: &mut SearchRefs) {
    match refs.control_rx.try_recv().unwrap_or(SearchCommand::Nothing) {
        SearchCommand::Stop => refs.search_state.terminate = SearchTerminate::Stop,
        SearchCommand::Quit => refs.search_state.terminate = SearchTerminate::Quit,

        SearchCommand::Start(_) | SearchCommand::Nothing => (),
    };

    match refs.search_params.search_mode {
        SearchMode::Infinite => (),
        SearchMode::MoveTime => {
            if let Some(start_time) = refs.search_state.start_time {
                if start_time.elapsed() > refs.search_params.move_time {
                    refs.search_state.terminate = SearchTerminate::Stop;
                }
            }
        }
        SearchMode::GameTime => {
            let elapsed = refs.search_state.start_time.unwrap().elapsed();
            let allocated = refs.search_state.allocated_time;

            let critical_time = Duration::from_secs(5);
            let ok_time = Duration::from_secs(30);

            let overshoot_factor = match allocated {
                x if x > ok_time => 2.0,
                x if x > critical_time && x <= ok_time => 1.5,
                x if x <= critical_time => 1.0,
                _ => 1.0,
            };

            if elapsed >= (allocated.mul_f64(overshoot_factor)) {
                refs.search_state.terminate = SearchTerminate::Stop
            }
        }
    }
}

pub enum SearchCommand {
    Start(SearchParams),
    Stop,
    Quit,
    Nothing,
}

#[derive(Clone, Copy, PartialEq)]
enum SearchTerminate {
    Stop,
    Quit,
    Nothing,
}

pub struct SearchParams {
    pub search_mode: SearchMode, // search mode
    pub move_time: Duration,     // maximum time to search per move
    pub game_time: GameTime,     // time left in the game
}

#[derive(Clone, Copy, PartialEq)]
pub enum SearchMode {
    Infinite,
    MoveTime,
    GameTime,
}

pub struct SearchRefs<'a> {
    board: &'a mut Board,
    search_params: &'a SearchParams,
    transposition_table: &'a mut CacheTable<SearchData>,
    search_state: &'a mut SearchState,
    control_rx: &'a Receiver<SearchCommand>,
    report_tx: &'a Sender<Information>,
}

struct SearchState {
    seldepth: u8,                // max depth searched
    start_time: Option<Instant>, // time search started
    nodes: u64,                  // number of nodes searched
    depth: u8,                   // current depth
    ply: u8,                     // current number of plies from root
    terminate: SearchTerminate,  // terminate flag
    allocated_time: Duration,    // time allocated to search
}

impl SearchState {
    fn new() -> Self {
        Self {
            seldepth: 0,
            start_time: None,
            nodes: 0,
            depth: 0,
            ply: 0,
            terminate: SearchTerminate::Nothing,
            allocated_time: Duration::from_secs(0),
        }
    }
}

#[derive(Debug)]
pub enum SearchInformation {
    BestMove(ChessMove),
    Summary(SearchSummary),
    ExtraInfo(String),
}

#[derive(Debug)]
pub struct SearchSummary {
    pub depth: u8,          // depth reached during search
    pub seldepth: u8,       // maximum selective depth reached
    pub time: Duration,     // how long the search took
    pub cp: i32,            // centipawns score
    pub nodes: u64,         // nodes searched
    pub nps: u64,           // nodes per second
    pub pv: Vec<ChessMove>, // Principal Variation
}

#[derive(Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct SearchData {
    depth: u8,
    eval: i32,
    best_move: ChessMove,
    flag: HashFlag,
}

impl SearchData {
    pub fn create(depth: u8, ply: u8, flag: HashFlag, eval: i32, best_move: ChessMove) -> Self {
        let mut v = eval;

        if v > INFINITY / 2 {
            v += ply as i32;
        } else if v < -INFINITY / 2 {
            v -= ply as i32;
        }

        Self {
            depth,
            eval: v,
            best_move,
            flag,
        }
    }

    pub fn get(&self, depth: u8, ply: u8, alpha: i32, beta: i32) -> Option<(i32, ChessMove)> {
        let mut value = None;

        if self.depth > depth {
            match self.flag {
                HashFlag::Exact => {
                    let mut v = self.eval;

                    if v > INFINITY / 2 {
                        v -= ply as i32;
                    } else if v < -INFINITY / 2 {
                        v += ply as i32;
                    }

                    value = Some(v);
                }
                HashFlag::Alpha => {
                    if self.eval <= alpha {
                        value = Some(alpha);
                    }
                }
                HashFlag::Beta => {
                    if self.eval >= beta {
                        value = Some(beta);
                    }
                }
                HashFlag::Nothing => (),
            }
        }

        value.map(|v| (v, self.best_move))
    }
}

#[derive(Copy, Clone, PartialEq, PartialOrd, Default)]
pub enum HashFlag {
    #[default]
    Nothing,
    Exact,
    Alpha,
    Beta,
}
