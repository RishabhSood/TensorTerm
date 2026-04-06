use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use color_eyre::Result;
use crossterm::event::{self, Event as CrosstermEvent, KeyEvent, KeyEventKind, MouseEvent};

#[derive(Debug)]
pub enum Event {
    Tick,
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize(u16, u16),
}

pub struct EventHandler {
    rx: mpsc::Receiver<Event>,
    _tx: mpsc::Sender<Event>,
}

impl EventHandler {
    pub fn new(tick_rate_ms: u64) -> Self {
        let tick_rate = Duration::from_millis(tick_rate_ms);
        let (tx, rx) = mpsc::channel();
        let event_tx = tx.clone();

        thread::spawn(move || {
            let mut last_tick = Instant::now();
            loop {
                let timeout = tick_rate
                    .checked_sub(last_tick.elapsed())
                    .unwrap_or(Duration::ZERO);

                if event::poll(timeout).expect("event poll failed") {
                    match event::read().expect("event read failed") {
                        CrosstermEvent::Key(e) if e.kind == KeyEventKind::Press => {
                            if event_tx.send(Event::Key(e)).is_err() {
                                return;
                            }
                        }
                        CrosstermEvent::Mouse(e) => {
                            if event_tx.send(Event::Mouse(e)).is_err() {
                                return;
                            }
                        }
                        CrosstermEvent::Resize(w, h) => {
                            if event_tx.send(Event::Resize(w, h)).is_err() {
                                return;
                            }
                        }
                        _ => {}
                    }
                }

                if last_tick.elapsed() >= tick_rate {
                    if event_tx.send(Event::Tick).is_err() {
                        return;
                    }
                    last_tick = Instant::now();
                }
            }
        });

        Self { rx, _tx: tx }
    }

    pub fn next(&self) -> Result<Event> {
        Ok(self.rx.recv()?)
    }
}
