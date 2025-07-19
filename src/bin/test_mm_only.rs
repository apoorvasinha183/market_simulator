use market_simulator::{AgentType, simulation::orchestra::Orchestra};
use std::{io, thread};

fn main() {
    println!("Testing MARKET MAKER ONLY - no other agents should be active");

    // ONLY market maker - no other agents
    let participants = vec![AgentType::MarketMaker];

    let orchestra = Orchestra::new(participants, 100, 10);

    // Run the orchestra in a separate thread
    thread::spawn(move || {
        orchestra.run();
    });

    println!("Market maker only test running...");
    println!("There should be NO TRADES happening - only limit orders placed");
    println!("Press Enter to stop...");

    let mut buffer = String::new();
    io::stdin().read_line(&mut buffer).unwrap();
    println!("Stopping test.");
}
