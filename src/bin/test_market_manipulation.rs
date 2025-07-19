use market_simulator::{AgentType, simulation::orchestra::Orchestra};
use std::{io, thread};

fn main() {
    println!("🦹‍♂️ MARKET MANIPULATION TEST - ETF Feedback Loop Demonstration");
    println!("================================================================");
    println!();
    println!("🎯 EXPERIMENT: ETF Dump → Constituent Cascade → Feedback Loop");
    println!("📊 PLAN: Watch how ETF selling triggers constituent selling spiral");
    println!();

    let participants = vec![
        // Market infrastructure
        AgentType::MarketMaker,
        // ETF Maintenance agents (our victims)
        AgentType::ETFMaintenanceAgent { etf_stock_id: 25 }, // YEET ETF (LEAPS = 40%!)
        AgentType::ETFMaintenanceAgent { etf_stock_id: 22 }, // CASINO ETF (YOLO = 25%)
        AgentType::ETFMaintenanceAgent { etf_stock_id: 24 }, // COPE ETF (GIGA = 30%)
        // Regular ETF agents for comparison
        AgentType::ETFAgent { etf_stock_id: 21 }, // BUBBLE ETF (diversified)
        AgentType::ETFAgent { etf_stock_id: 23 }, // RETAIL ETF
        // THE MANIPULATORS 😈
        AgentType::WhaleAgent,    // Will target concentrated holdings
        AgentType::WhaleAgent,    // Multiple whales for coordination
        AgentType::MomentumAgent, // Will amplify the manipulation
        // Innocent bystanders
        AgentType::DumbMarket,
    ];

    let orchestra = Orchestra::new(participants, 100, 10);

    thread::spawn(move || {
        orchestra.run();
    });

    println!("🔥 MANIPULATION IN PROGRESS:");
    println!("┌─────────────────────────────────────────────────────────────┐");
    println!("│ WATCH FOR:                                                  │");
    println!("│                                                             │");
    println!("│ 🎯 Whale agents buying LEAPS, YOLO, GIGA heavily           │");
    println!("│ 📈 Stock prices moving dramatically                        │");
    println!("│ 🚨 ETF Maintenance agents going CRAZY with arbitrage       │");
    println!("│ 💥 'CREATION/REDEMPTION' messages flooding the screen      │");
    println!("│ 🔄 Rebalancing attempts as agents try to stay neutral      │");
    println!("│                                                             │");
    println!("│ EXPECTED CHAOS:                                             │");
    println!("│ • YEET ETF maintenance agent: Frantic LEAPS arbitrage      │");
    println!("│ • CASINO ETF maintenance agent: YOLO/DEGEN chaos           │");
    println!("│ • COPE ETF maintenance agent: GIGA manipulation response   │");
    println!("│                                                             │");
    println!("│ The more concentrated the ETF holding, the more chaos! 😈  │");
    println!("└─────────────────────────────────────────────────────────────┘");
    println!();
    println!("🎪 MARKET MANIPULATION TARGETS:");
    println!("• LEAPS → YEET ETF (40% weight) - NUCLEAR OPTION");
    println!("• YOLO → CASINO ETF (25% weight) - HIGH IMPACT");
    println!("• DEGEN → CASINO ETF (25% weight) - DOUBLE TROUBLE");
    println!("• GIGA → COPE ETF (30% weight) - BIG MONEY MOVES");
    println!();
    println!("💰 Real market manipulation at work!");
    println!("Press Enter to stop the chaos...");

    let mut buffer = String::new();
    io::stdin().read_line(&mut buffer).unwrap();
    println!("🛑 Stopping market manipulation (SEC is coming!)");
}
