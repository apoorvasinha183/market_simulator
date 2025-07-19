use market_simulator::{AgentType, simulation::orchestra::Orchestra};
use std::{io, thread};

fn main() {
    println!("Testing ETF MAINTENANCE AGENTS - Specialized price conjunction keepers");
    println!("=================================================================");

    // Create a sophisticated ETF ecosystem:
    let participants = vec![
        // Core market infrastructure
        AgentType::MarketMaker,
        // Regular ETF agents (50bps threshold, 100ms checks)
        AgentType::ETFAgent { etf_stock_id: 21 }, // BUBBLE - regular arbitrage
        AgentType::ETFAgent { etf_stock_id: 22 }, // CASINO - regular arbitrage
        // ETF Maintenance agents (10bps threshold, 10ms checks)
        AgentType::ETFMaintenanceAgent { etf_stock_id: 23 }, // RETAIL - tight maintenance
        AgentType::ETFMaintenanceAgent { etf_stock_id: 24 }, // COPE - tight maintenance
        AgentType::ETFMaintenanceAgent { etf_stock_id: 25 }, // YEET - tight maintenance
        // Market participants to create price movements
        AgentType::DumbMarket,
        AgentType::WhaleAgent,
        AgentType::MomentumAgent,
    ];

    let orchestra = Orchestra::new(participants, 100, 10);

    // Run the orchestra in a separate thread
    thread::spawn(move || {
        orchestra.run();
    });

    println!();
    println!("🎯 COMPARISON TEST RUNNING:");
    println!("┌─────────────────────────────────────────────────────────────┐");
    println!("│ Regular ETF Agents (BUBBLE, CASINO):                       │");
    println!("│  • 50bps threshold (0.5% price divergence)                 │");
    println!("│  • Check every 100ms                                       │");
    println!("│  • Basic arbitrage when big opportunities arise            │");
    println!("│                                                             │");
    println!("│ ETF Maintenance Agents (RETAIL, COPE, YEET):               │");
    println!("│  • 10bps threshold (0.1% price divergence)                 │");
    println!("│  • Check every 10ms (10x faster)                          │");
    println!("│  • Sophisticated inventory management                      │");
    println!("│  • Creation/Redemption mechanisms                          │");
    println!("│  • Automatic rebalancing                                   │");
    println!("└─────────────────────────────────────────────────────────────┘");
    println!();
    println!("📊 EXPECTED BEHAVIOR:");
    println!("• Regular agents: Occasional arbitrage messages (>50bps)");
    println!("• Maintenance agents: Frequent CREATION/REDEMPTION messages (>10bps)");
    println!("• Maintenance agents should keep their ETFs much tighter to NAV");
    println!("• Portfolio updates showing profit tracking");
    println!();
    println!("🔍 WATCH FOR:");
    println!("• '[ETF Agent X] Arbitrage: ...' (regular agents)");
    println!(
        "• '[ETF Maintenance Agent X] CREATION/REDEMPTION Arbitrage: ...' (maintenance agents)"
    );
    println!("• '[ETF Maintenance Agent X] Portfolio: ...' (profit tracking)");
    println!("• '[ETF Maintenance Agent X] Rebalancing: ...' (inventory management)");
    println!();
    println!("Press Enter to stop...");

    let mut buffer = String::new();
    io::stdin().read_line(&mut buffer).unwrap();
    println!("Stopping test.");
}
