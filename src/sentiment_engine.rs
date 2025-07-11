// src/sentiment_engine.rs
use crate::events::MarketEvent;
use crate::stocks::StockMarket;
use crossbeam_channel::Sender;
use std::io;
use std::net::UdpSocket;
use std::thread;

use std::net::{IpAddr, Ipv4Addr};

/// The SentimentEngine is responsible for listening to external sentiment data
/// and broadcasting it into the simulation's central event bus.
pub struct SentimentEngine;

impl SentimentEngine {
    /// Joins a multicast group on a specific port.
    fn join_multicast_group(port: u16) -> io::Result<UdpSocket> {
        let multicast_addr: Ipv4Addr = "224.0.0.123".parse().unwrap();
        let bind_addr: Ipv4Addr = "0.0.0.0".parse().unwrap(); // Bind to all interfaces

        let socket = socket2::Socket::new(
            socket2::Domain::IPV4,
            socket2::Type::DGRAM,
            Some(socket2::Protocol::UDP),
        )?;

        socket.set_reuse_address(true)?;
        #[cfg(unix)] // SO_REUSEPORT is not available on all platforms
        socket.set_reuse_port(true)?;

        let addr = std::net::SocketAddr::new(IpAddr::V4(bind_addr), port);
        socket.bind(&addr.into())?;

        socket.join_multicast_v4(&multicast_addr, &bind_addr)?;

        Ok(socket.into())
    }

    /// Spawns listener threads for each stock.
    ///
    /// # Arguments
    /// * `stock_market` - A reference to the stock market definitions to get port info.
    /// * `event_tx` - The sender for the main event bus.
    pub fn run(stock_market: &StockMarket, event_tx: Sender<MarketEvent>) {
        println!("[SentimentEngine] Starting multicast sentiment listeners...");
        for stock in &stock_market.stocks {
            let tx_clone = event_tx.clone();
            let port = stock.sentiment_port as u16;
            let stock_id = stock.id;
            let stock_ticker = stock.ticker.clone();

            thread::spawn(move || {
                let socket = match Self::join_multicast_group(port) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!(
                            "[SentimentEngine] Failed to join multicast group for {} on port {}: {}",
                            stock_ticker, port, e
                        );
                        return;
                    }
                };

                println!(
                    "[SentimentEngine] Process {} listening for {} multicast on 224.0.0.123:{}",
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
