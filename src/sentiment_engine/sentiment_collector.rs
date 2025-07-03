use crate::stocks::definitions::StockMarket;
use std::{
    collections::HashMap,
    net::UdpSocket,
    thread,
    sync::{Arc, RwLock},
};
use socket2::{Socket, SockAddr};

#[derive(Debug)]
pub struct SentimentEngine {
    current_sentiment: Arc<RwLock<HashMap<u64, f64>>>,
}

impl SentimentEngine {
    /// Creates a new SentimentEngine and spawns listener threads for each stock.
    pub fn new(stock_market: &StockMarket, storage_duration: u64) -> Self {
        let (tx, rx) = unbounded();
        let mut history = HashMap::new();

        for stock in &stock_market.stocks {
            // Pre-populate the history map to ensure every stock has an entry.
            history.insert(stock.id, VecDeque::new());

            let thread_tx = tx.clone();
            let port = stock.sentiment_port;
            let stock_id = stock.id;
            // Spawn a thread for each stock to listen for sentiment updates. (Mother of all deadlocks if not careful)
            thread::spawn(move || {
                let socket = match UdpSocket::bind(format!("127.0.0.1:{}", port)) {
                    Ok(s) => {
                        println!("[SentimentCollector] Listening for sentiment on 127.0.0.1:{}", port);
                        s
                    },
                    Err(e) => {
                        eprintln!("Failed to bind UDP socket for stock {}: {}", stock_id, e);
                        return;
                    }
                };
                let mut buf = [0; 1024];

                // This thread  only listens and sends data, it doesn't manage state.
                loop {
                    match socket.recv_from(&mut buf) {
                        Ok((size, _)) => {
                            let data = String::from_utf8_lossy(&buf[..size]);
                            if let Ok(value) = data.trim().parse::<f64>() {
                                // Send the update to the main thread.
                                // If send fails, the receiver has been dropped, so we can exit.
                                if thread_tx.send((stock_id, value)).is_err() {
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Error receiving data for stock {}: {}", stock_id, e);
                            break;
                        }
                    }
                }
            });
        }

        SentimentEngine {
            history,
            rx,
            storage_duration: storage_duration as usize,
        }
    }

    /// Processes all pending sentiment updates from the network threads.
    /// This method is non-blocking and should be called periodically in the main loop.
    pub fn update_history(&mut self) {
        //  Centralized state management.
        while let Ok((stock_id, value)) = self.rx.try_recv() {
            if let Some(values) = self.history.get_mut(&stock_id) {
                values.push_back(value);
                // Keep only the last `storage_duration` values.
                while values.len() > self.storage_duration {
                    values.pop_front();
                }
            }
        }
    }

    /// A simple getter for the sentiment history.
    pub fn get_history(&self, stock_id: u64) -> Option<&VecDeque<f64>> {
        self.history.get(&stock_id)
    }
}
