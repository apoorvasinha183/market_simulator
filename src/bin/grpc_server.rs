use market_simulator::{AgentType, simulation::orchestra::Orchestra};
use std::{io, thread};

fn main() {
    println!("Starting gRPC Market Server...");

    let participants = vec![
        AgentType::CustomerAgent, // This agent will host the gRPC server
        AgentType::MarketMaker,
        AgentType::Thermodynamic {
            initial_temperature: 0.2,
            specific_heat: 0.1,
            initial_chemical_potential: 0.0,
        }, // Meme Trader
        AgentType::Thermodynamic {
            initial_temperature: 0.1,
            specific_heat: 1.0,
            initial_chemical_potential: 0.0,
        }, // Value Trader
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

    // Graceful shutdown mechanism:
    // The server will run as long as stdin is open. Closing it from the
    // test script will cause this to exit, allowing for a clean shutdown.
    println!("Server running. Close stdin to shut down.");
    let mut buffer = String::new();
    io::stdin().read_line(&mut buffer).unwrap();
    println!("Stdin closed, shutting down server.");
}
