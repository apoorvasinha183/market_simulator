// src/sentiment_engine.rs

use crate::events::MarketEvent;
use crate::stocks::StockMarket;
use crossbeam_channel::Sender;
use std::net::UdpSocket;
use std::thread;

/// The SentimentEngine is responsible for listening to external sentiment data
/// and broadcasting it into the simulation's central event bus.
pub struct SentimentEngine;

impl SentimentEngine {
    /// Spawns listener threads for each stock.
    ///
    /// # Arguments
    /// * `stock_market` - A reference to the stock market definitions to get port info.
    /// * `event_tx` - The sender for the main event bus.
    pub fn run(stock_market: &StockMarket, event_tx: Sender<MarketEvent>) {
        println!("[SentimentEngine] Starting sentiment listeners...");
        for stock in &stock_market.stocks {
            let tx_clone = event_tx.clone();
            let port = stock.sentiment_port;
            let stock_id = stock.id;
            let stock_ticker = stock.ticker.clone();

            thread::spawn(move || {
                let addr = format!("127.0.0.1:{}", port);
                let socket = match UdpSocket::bind(&addr) {
                    Ok(s) => s,
                    Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                        println!(
                            "[SentimentEngine] Warning: Address {} already in use. Skipping listener for {}.",
                            addr, stock_ticker
                        );
                        return;
                    }
                    Err(e) => {
                        eprintln!(
                            "[SentimentEngine] Failed to bind UDP socket for {} on port {}: {}",
                            stock_ticker, port, e
                        );
                        return;
                    }
                };
                println!(
                    "[SentimentEngine] Listening for {} sentiment on {}",
                    stock_ticker, addr
                );

                let mut buf = [0; 32]; // Small buffer for a single float string
                loop {
                    match socket.recv_from(&mut buf) {
                        Ok((size, _)) => {
                            if let Ok(s) = std::str::from_utf8(&buf[..size]) {
                                if let Ok(score) = s.trim().parse::<f64>() {
                                    let event = MarketEvent::SentimentUpdate { stock_id, score };
                                    if tx_clone.send(event).is_err() {
                                        // Main bus closed, shut down thread
                                        break;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!(
                                "[SentimentEngine] Error receiving data for {}: {}",
                                stock_ticker, e
                            );
                            break;
                        }
                    }
                }
            });
        }
    }
}
