use market_simulator::{
    AgentType,
    simulation::orchestra::Orchestra,
};
use std::thread;

fn main() {
    println!("Starting gRPC Market Server...");

    let participants = vec![
        AgentType::CustomerAgent, // This agent will host the gRPC server
        AgentType::MarketMaker,
        AgentType::DumbMarket,
        AgentType::DumbLimit,
        AgentType::WhaleAgent,
    ];

    let orchestra = Orchestra::new(participants, 1000, 100);

    // Run the orchestra (market simulation and other agents) in a separate thread
    thread::spawn(move || {
        orchestra.run();
    });

    // The CustomerAgent's run method will start the gRPC server.
    // Since the Orchestra runs agents in their own threads, the CustomerAgent
    // will start its gRPC server in its dedicated thread.
    // We just need to ensure the main thread doesn't exit immediately.
    // In a real application, you might have a graceful shutdown mechanism here.
    loop {
        thread::park(); // Park the main thread indefinitely
    }
}
