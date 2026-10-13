//! Local connection to the sim. The viewer listens on ws://127.0.0.1:8765; the sim (a browser page) connects
//! and sends its state as JSON every frame. Only the loopback address is bound, so nothing outside this PC
//! can reach it.

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
}

pub fn start() -> Arc<Mutex<Shared>> {
    let shared = Arc::new(Mutex::new(Shared::default()));
    let s = shared.clone();
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
    shared
}
