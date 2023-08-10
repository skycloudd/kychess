use crate::search::SearchSummary;
use crate::{Information, INFINITY};
use chess::ChessMove;
use crossbeam_channel::Sender;
use std::thread::{self, JoinHandle};
use std::time::Duration;
use vampirc_uci::{parse, UciInfoAttribute, UciMessage, UciTimeControl};

pub struct Uci {
    control_handle: Option<JoinHandle<()>>,
    report_handle: Option<JoinHandle<()>>,
    control_tx: Option<Sender<UciControl>>,
}

impl Uci {
    pub fn new() -> Self {
        Self {
            control_handle: None,
            report_handle: None,
            control_tx: None,
        }
    }

    pub fn init(&mut self, report_tx: Sender<Information>) {
        self.report_thread(report_tx);
        self.control_thread();
    }

    pub fn send(&self, msg: UciControl) {
        if let Some(tx) = &self.control_tx {
            tx.send(msg).unwrap();
        }
    }

    fn report_thread(&mut self, report_tx: Sender<Information>) {
        let mut incoming_data = String::from("");

        let report_handle = thread::spawn(move || {
            let mut quit = false;

            while !quit {
                std::io::stdin().read_line(&mut incoming_data).unwrap();

                let msgs = parse(&incoming_data);

                for msg in msgs {
                    let report = match msg {
                        vampirc_uci::UciMessage::Uci => UciReport::Uci,

                        vampirc_uci::UciMessage::Debug(debug) => UciReport::Debug(debug),

                        vampirc_uci::UciMessage::IsReady => UciReport::IsReady,

                        vampirc_uci::UciMessage::Position {
                            startpos,
                            fen,
                            moves,
                        } => {
                            let fen = if startpos {
                                "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
                                    .to_string()
                            } else {
                                fen.unwrap().to_string()
                            };

                            UciReport::Position(fen, moves)
                        }

                        UciMessage::UciNewGame => UciReport::UciNewGame,

                        UciMessage::Stop => UciReport::Stop,

                        UciMessage::Quit => {
                            quit = true;
                            UciReport::Quit
                        }

                        UciMessage::Go {
                            time_control,
                            search_control: _,
                        } => match time_control {
                            Some(tc) => match tc {
                                UciTimeControl::Ponder => todo!(),

                                UciTimeControl::Infinite => UciReport::GoInfinite,

                                UciTimeControl::TimeLeft {
                                    white_time,
                                    black_time,
                                    white_increment,
                                    black_increment,
                                    moves_to_go,
                                } => UciReport::GoGameTime(GameTime {
                                    white_time: white_time
                                        .map(|t| t.to_std().unwrap_or(Duration::from_secs(0))),
                                    black_time: black_time
                                        .map(|t| t.to_std().unwrap_or(Duration::from_secs(0))),
                                    white_increment: white_increment
                                        .map(|t| t.to_std().unwrap_or(Duration::from_secs(0))),
                                    black_increment: black_increment
                                        .map(|t| t.to_std().unwrap_or(Duration::from_secs(0))),
                                    moves_to_go,
                                }),

                                UciTimeControl::MoveTime(movetime) => UciReport::GoMoveTime(
                                    movetime.to_std().unwrap_or(Duration::from_secs(0)),
                                ),
                            },
                            None => todo!(),
                        },

                        _ => UciReport::Unknown,
                    };

                    report_tx.send(Information::UciInformation(report)).unwrap();
                }

                incoming_data = String::from("");
            }
        });

        self.report_handle = Some(report_handle);
    }

    fn control_thread(&mut self) {
        let (control_tx, control_rx) = crossbeam_channel::unbounded::<UciControl>();

        let control_handle = thread::spawn(move || {
            let mut quit = false;

            while !quit {
                let control = control_rx.recv().unwrap();

                match control {
                    UciControl::Identify => {
                        println!("{}", UciMessage::id_name("kychess"));
                        println!("{}", UciMessage::id_author("skycloudd"));
                        println!("{}", UciMessage::UciOk);
                    }
                    UciControl::Ready => println!("{}", UciMessage::ReadyOk),
                    UciControl::Quit => quit = true,
                    UciControl::BestMove(bm) => {
                        println!("{}", UciMessage::best_move(bm));
                    }
                    UciControl::SearchSummary(summary) => {
                        let attrs = vec![
                            UciInfoAttribute::Depth(summary.depth),
                            UciInfoAttribute::SelDepth(summary.seldepth),
                            UciInfoAttribute::Time(
                                vampirc_uci::Duration::from_std(summary.time).unwrap(),
                            ),
                            if summary.cp > INFINITY / 2 || summary.cp < -INFINITY / 2 {
                                let mate_in_plies = INFINITY - summary.cp.abs();

                                let mate_in = if mate_in_plies % 2 == 0 {
                                    mate_in_plies / 2
                                } else {
                                    mate_in_plies / 2 + 1
                                };

                                UciInfoAttribute::Score {
                                    cp: None,
                                    mate: Some(mate_in as i8),
                                    lower_bound: None,
                                    upper_bound: None,
                                }
                            } else {
                                UciInfoAttribute::Score {
                                    cp: Some(summary.cp),
                                    mate: None,
                                    lower_bound: None,
                                    upper_bound: None,
                                }
                            },
                            UciInfoAttribute::Nodes(summary.nodes),
                            UciInfoAttribute::Nps(summary.nps),
                            UciInfoAttribute::Pv(summary.pv),
                        ];

                        println!("{}", UciMessage::Info(attrs));
                    }
                    UciControl::ExtraInfo(info) => {
                        println!("{}", UciMessage::info_string(info));
                    }
                }
            }
        });

        self.control_handle = Some(control_handle);
        self.control_tx = Some(control_tx);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum UciReport {
    Uci,
    Debug(bool),
    IsReady,
    Position(String, Vec<ChessMove>),
    UciNewGame,
    Stop,
    Quit,
    GoInfinite,
    GoMoveTime(Duration),
    GoGameTime(GameTime),
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct GameTime {
    pub white_time: Option<Duration>,
    pub black_time: Option<Duration>,
    pub white_increment: Option<Duration>,
    pub black_increment: Option<Duration>,
    pub moves_to_go: Option<u8>,
}

pub enum UciControl {
    Identify,
    Ready,
    Quit,
    BestMove(ChessMove),
    SearchSummary(SearchSummary),
    ExtraInfo(String),
}
