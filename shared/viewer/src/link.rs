//! Where the viewer gets the sim's state from. Two ways in:
//!
//! - `Link::in_process()`: the sim and viewer are compiled into one program. The sim calls `link.send(state)`
//!   from its own thread; nothing goes over the network.
//! - `Link::listen()`: the viewer runs as its own program and listens on ws://127.0.0.1:8765. The sim connects
//!   and sends its state as JSON every frame. Only the loopback address is bound, so nothing outside this PC
//!   can reach it.

use crate::state::SimState;
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::Instant;

pub const PORT: u16 = 8765;

#[derive(Default)]
pub struct Shared {
    pub latest: Option<(SimState, Instant)>,
    pub connected: bool,
    pub error: Option<String>,
    pub listening: bool,
}

/// A handle to the latest sim state. Cheap to clone; every clone sees the same state.
#[derive(Clone, Default)]
pub struct Link(pub(crate) Arc<Mutex<Shared>>);

impl Link {
    /// For a sim built into the same program: call `send` with each new state.
    pub fn in_process() -> Link {
        Link::default()
    }

    /// For the standalone viewer: accept the sim over a localhost WebSocket on `PORT`.
    pub fn listen() -> Link {
        let link = Link::default();
        link.0.lock().unwrap().listening = true;
        start(link.0.clone());
        link
    }

    /// Hand the viewer a new state. Safe to call from any thread, as often as you like.
    pub fn send(&self, state: SimState) {
        let mut s = self.0.lock().unwrap();
        s.connected = true;
        s.latest = Some((state, Instant::now()));
    }
}

fn start(shared: Arc<Mutex<Shared>>) {
    let s = shared;
    std::thread::spawn(move || {
        let listener = match TcpListener::bind(("127.0.0.1", PORT)) {
            Ok(l) => l,
            Err(e) => {
                s.lock().unwrap().error = Some(format!("Can't listen on port {PORT}: {e}"));
                return;
            }
        };
        for stream in listener.incoming().flatten() {
            let s2 = s.clone();
            std::thread::spawn(move || {
                let Ok(mut ws) = tungstenite::accept(stream) else { return };
                s2.lock().unwrap().connected = true;
                while let Ok(msg) = ws.read() {
                    if let tungstenite::Message::Text(text) = msg {
                        if let Ok(state) = serde_json::from_str::<SimState>(&text) {
                            s2.lock().unwrap().latest = Some((state, Instant::now()));
                        }
                    }
                }
                s2.lock().unwrap().connected = false;
            });
        }
    });
}
