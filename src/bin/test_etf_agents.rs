use market_simulator::{AgentType, simulation::orchestra::Orchestra};
use std::{io, thread};

fn main() {
    println!("Testing ETF AGENTS - arbitrage bots for ETFs");

    // Create agents: Market maker + ETF agents for each ETF
    let participants = vec![
        AgentType::MarketMaker,
        // ETF agents for each ETF (IDs 21-25 from stock.csv)
        AgentType::ETFAgent { etf_stock_id: 21 }, // BUBBLE
        AgentType::ETFAgent { etf_stock_id: 22 }, // CASINO  
        AgentType::ETFAgent { etf_stock_id: 23 }, // RETAIL
        AgentType::ETFAgent { etf_stock_id: 24 }, // COPE
        AgentType::ETFAgent { etf_stock_id: 25 }, // YEET
        // Add some other agents to create price movements
        AgentType::DumbMarket,
        AgentType::WhaleAgent,
    ];

    let orchestra = Orchestra::new(participants, 100, 10);

    // Run the orchestra in a separate thread
    thread::spawn(move || {
        orchestra.run();
    });

    println!("ETF agents test running...");
    println!("ETF agents should:");
    println!("  - Calculate NAV for their ETFs based on constituent prices");
    println!("  - Detect when ETF price diverges from NAV");
    println!("  - Execute arbitrage trades to bring prices back in line");
    println!("  - Example: If BUBBLE constituents drop 5%, ETF agent should sell BUBBLE and buy constituents");
    println!();
    println!("Watch for arbitrage messages in the logs!");
    println!("Press Enter to stop...");
    
    let mut buffer = String::new();
    io::stdin().read_line(&mut buffer).unwrap();
    println!("Stopping test.");
}