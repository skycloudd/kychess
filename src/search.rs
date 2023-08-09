use crate::evaluation::evaluate_position;
use crate::{Information, INFINITY};
use chess::{Board, CacheTable, ChessMove, MoveGen, EMPTY};
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

        if refs.search_state.nodes & 0x7ff == 0 {
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
            }
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
            if let Some((tt_value, _)) = tt_entry.get(depth, refs.search_state.ply) {
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

            let score = -Self::negamax(refs, &mut node_pv, depth - 1, -beta, -alpha);

            refs.search_state.ply -= 1;

            *refs.board = old_pos;

            if score > best_eval_score {
                best_eval_score = score;

                best_move = Some(legal);
            }

            if score >= beta {
                refs.transposition_table.add(
                    position_hash,
                    SearchData::create(
                        depth,
                        refs.search_state.ply,
                        HashFlag::Beta,
                        score,
                        best_move.unwrap(),
                    ),
                );

                return beta;
            }

            if score > alpha {
                alpha = score;

                hash_flag = HashFlag::Exact;

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
            }
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
}

#[derive(Clone, Copy)]
pub enum SearchMode {
    Infinite,
    MoveTime,
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
        }
    }
}

#[derive(Debug)]
pub enum SearchInformation {
    BestMove(ChessMove),
    Summary(SearchSummary),
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

    pub fn get(&self, depth: u8, ply: u8) -> Option<(i32, ChessMove)> {
        let mut value = None;

        if self.depth > depth {
            let mut v = self.eval;

            if v > INFINITY / 2 {
                v -= ply as i32;
            } else if v < -INFINITY / 2 {
                v += ply as i32;
            }

            value = Some((v, self.best_move));
        }

        value
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