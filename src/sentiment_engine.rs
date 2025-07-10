// src/sentiment_engine.rs
use crate::events::MarketEvent;
use crate::stocks::StockMarket;
use crossbeam_channel::Sender;
use std::io;
use std::net::UdpSocket;
use std::thread;

/// The SentimentEngine is responsible for listening to external sentiment data
/// and broadcasting it into the simulation's central event bus.
pub struct SentimentEngine;

impl SentimentEngine {
    /// Creates a UDP socket with SO_REUSEPORT enabled for multiple processes
    fn create_reusable_socket(port: u16) -> io::Result<UdpSocket> {
        let socket = socket2::Socket::new(
            socket2::Domain::IPV4,
            socket2::Type::DGRAM,
            Some(socket2::Protocol::UDP),
        )?;

        // Enable SO_REUSEPORT for multiple processes
        socket.set_reuse_port(true)?;

        // Optional: also set SO_REUSEADDR
        socket.set_reuse_address(true)?;

        // Bind to the address
        let addr = format!("127.0.0.1:{}", port);
        let socket_addr: std::net::SocketAddr = addr.parse().unwrap();
        socket.bind(&socket_addr.into())?;

        // Convert to std::net::UdpSocket
        Ok(socket.into())
    }

    /// Spawns listener threads for each stock.
    ///
    /// # Arguments
    /// * `stock_market` - A reference to the stock market definitions to get port info.
    /// * `event_tx` - The sender for the main event bus.
    pub fn run(stock_market: &StockMarket, event_tx: Sender<MarketEvent>) {
        println!("[SentimentEngine] Starting sentiment listeners...");
        for stock in &stock_market.stocks {
            let tx_clone = event_tx.clone();
            let port = stock.sentiment_port as u16;
            let stock_id = stock.id;
            let stock_ticker = stock.ticker.clone();

            thread::spawn(move || {
                let socket = match Self::create_reusable_socket(port) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!(
                            "[SentimentEngine] Failed to create reusable socket for {} on port {}: {}",
                            stock_ticker, port, e
                        );
                        return;
                    }
                };

                println!(
                    "[SentimentEngine] Process {} listening for {} sentiment on 127.0.0.1:{}",
                    std::process::id(),
                    stock_ticker,
                    port
                );

                let mut buf = [0; 32]; // Small buffer for a single float string
                loop {
                    match socket.recv_from(&mut buf) {
                        Ok((size, _src)) => {
                            if let Ok(s) = std::str::from_utf8(&buf[..size]) {
                                if let Ok(score) = s.trim().parse::<f64>() {
                                    /*
                                    println!(
                                        "[SentimentEngine] PID {} received sentiment {} for {} from {}",
                                        std::process::id(),
                                        score,
                                        stock_ticker,
                                        src
                                    ); */
                                    let event = MarketEvent::SentimentUpdate { stock_id, score };
                                    if tx_clone.send(event).is_err() {
                                        // Main bus closed, shut down thread
                                        break;
                                    }
                                } else {
                                    eprintln!(
                                        "[SentimentEngine] Invalid sentiment score format for {}: '{}'",
                                        stock_ticker, s
                                    );
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
