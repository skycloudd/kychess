use chess::Board;
use search::{Search, SearchCommand, SearchInformation, SearchMode, SearchParams};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use uci::{GameTime, Uci, UciControl, UciReport};
use vampirc_uci::UciMessage;

mod evaluation;
mod search;
mod uci;

const INFINITY: i32 = 10000;

fn main() {
    let mut engine = Engine::new();

    engine.main_loop();
}

struct Engine {
    board: Arc<Mutex<Board>>,
    search: Search,
    uci: Uci,
    info_rx: Option<crossbeam_channel::Receiver<Information>>,
    debug: bool,
    quit: bool,
}

impl Engine {
    fn new() -> Self {
        Self {
            board: Arc::new(Mutex::new(Board::default())),
            search: Search::new(),
            uci: Uci::new(),
            info_rx: None,
            debug: false,
            quit: false,
        }
    }

    fn main_loop(&mut self) {
        let (info_tx, info_rx) = crossbeam_channel::unbounded::<Information>();

        self.info_rx = Some(info_rx);

        self.uci.init(info_tx.clone());

        let history = Arc::new(Mutex::new(Vec::new()));

        self.search
            .init(info_tx, Arc::clone(&self.board), Arc::clone(&history));

        while !self.quit {
            let information = self.info_rx.as_ref().unwrap().recv().unwrap();

            if self.debug {
                println!("{}", UciMessage::info_string(format!("{:?}", &information)));
            }

            match information {
                Information::UciInformation(uci_report) => match uci_report {
                    UciReport::Uci => {
                        self.uci.send(UciControl::Identify);
                    }
                    UciReport::Debug(debug) => self.debug = debug,
                    UciReport::IsReady => self.uci.send(UciControl::Ready),
                    UciReport::Position(fen, moves) => {
                        let mut board = self.board.lock().unwrap();

                        *board = Board::from_str(&fen).unwrap();

                        for mov in moves {
                            *board = board.make_move_new(mov);
                        }
                    }
                    UciReport::UciNewGame => {
                        let mut board = self.board.lock().unwrap();

                        *board = Board::default();
                    }
                    UciReport::Stop => self.search.send(SearchCommand::Stop),
                    UciReport::Quit => self.quit(),
                    UciReport::GoInfinite => self.search.send(SearchCommand::Start(SearchParams {
                        search_mode: SearchMode::Infinite,
                        move_time: Duration::default(),
                        game_time: GameTime::default(),
                    })),
                    UciReport::GoMoveTime(move_time) => {
                        self.search.send(SearchCommand::Start(SearchParams {
                            search_mode: SearchMode::MoveTime,
                            move_time: move_time - Duration::from_millis(50),
                            game_time: GameTime::default(),
                        }))
                    }
                    UciReport::GoGameTime(game_time) => {
                        self.search.send(SearchCommand::Start(SearchParams {
                            search_mode: SearchMode::GameTime,
                            move_time: Duration::default(),
                            game_time,
                        }))
                    }
                    UciReport::Unknown => (),
                },
                Information::SearchInformation(search_info) => match search_info {
                    SearchInformation::BestMove(bm) => self.uci.send(UciControl::BestMove(bm)),
                    SearchInformation::Summary(summary) => {
                        self.uci.send(UciControl::SearchSummary(summary))
                    }
                    SearchInformation::ExtraInfo(info) => {
                        self.uci.send(UciControl::ExtraInfo(info))
                    }
                },
            }
        }
    }

    fn quit(&mut self) {
        self.uci.send(UciControl::Quit);
        self.search.send(SearchCommand::Quit);
        self.quit = true;
    }
}

#[derive(Debug)]
pub enum Information {
    SearchInformation(SearchInformation),
    UciInformation(UciReport),
}
