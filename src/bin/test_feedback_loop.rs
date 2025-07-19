use market_simulator::{AgentType, simulation::orchestra::Orchestra};
use std::{io, thread};

fn main() {
    println!("🌪️  ETF FEEDBACK LOOP TEST - Cascade Demonstration");
    println!("===================================================");
    println!();
    println!("🎯 OBJECTIVE: Demonstrate ETF → Constituent → ETF feedback loops");
    println!("📊 MECHANISM: Panic selling triggers cascading effects across ETFs");
    println!();

    // Create a system designed to show feedback loops
    let participants = vec![
        // Core market infrastructure
        AgentType::MarketMaker,
        // ETF Maintenance agents (victims of the feedback loop)
        AgentType::ETFMaintenanceAgent { etf_stock_id: 25 }, // YEET ETF (LEAPS=40%, YOLO=30%, DEGEN=20%, ROCKET=10%)
        AgentType::ETFMaintenanceAgent { etf_stock_id: 22 }, // CASINO ETF (YOLO=25%, DEGEN=25%, ROCKET=20%, MOON=15%, FOMO=15%)
        AgentType::ETFMaintenanceAgent { etf_stock_id: 24 }, // COPE ETF (GIGA=30%, ROCKET=25%, ALPHA=20%, SIGMA=15%, ATH=10%)
        // Regular ETF agents for comparison
        AgentType::ETFAgent { etf_stock_id: 21 }, // BUBBLE ETF (diversified - should be more stable)
        // PANIC AGENTS - The feedback loop triggers! 🚨
        // These monitor key stocks and panic sell when prices drop
        AgentType::PanicAgent {
            monitored_stocks: vec![17, 3, 20], // LEAPS, YOLO, DEGEN - key ETF constituents
        },
        AgentType::PanicAgent {
            monitored_stocks: vec![7, 8, 16], // ROCKET, MOON, FOMO - more ETF constituents
        },
        // Market participants to create initial price movements
        AgentType::WhaleAgent,    // Will create initial disturbance
        AgentType::MomentumAgent, // Will amplify movements
        AgentType::DumbMarket,    // Background noise
    ];

    let orchestra = Orchestra::new(participants, 100, 10);

    thread::spawn(move || {
        orchestra.run();
    });

    println!("🔥 FEEDBACK LOOP EXPERIMENT RUNNING:");
    println!("┌─────────────────────────────────────────────────────────────┐");
    println!("│ EXPECTED SEQUENCE:                                          │");
    println!("│                                                             │");
    println!("│ 1️⃣  Whale/Momentum agents create price movements            │");
    println!("│ 2️⃣  If LEAPS/YOLO/DEGEN drop 5%+ → Panic Agents trigger    │");
    println!("│ 3️⃣  Panic Agents amplify selling (2x volume)               │");
    println!("│ 4️⃣  ETF Maintenance Agents detect ETF vs NAV mismatch      │");
    println!("│ 5️⃣  Maintenance Agents sell more constituents to rebalance │");
    println!("│ 6️⃣  Constituent prices drop further → NAV drops            │");
    println!("│ 7️⃣  FEEDBACK LOOP: Lower NAV justifies lower ETF price     │");
    println!("│ 8️⃣  Process repeats → CASCADE EFFECT! 🌪️                   │");
    println!("│                                                             │");
    println!("│ CROSS-ETF CONTAMINATION:                                    │");
    println!("│ • LEAPS drops → Affects YEET ETF (40% weight)              │");
    println!("│ • YOLO drops → Affects YEET (30%) + CASINO (25%)           │");
    println!("│ • DEGEN drops → Affects YEET (20%) + CASINO (25%)          │");
    println!("│ • ROCKET drops → Affects YEET (10%) + CASINO (20%) + COPE  │");
    println!("└─────────────────────────────────────────────────────────────┘");
    println!();
    println!("🔍 WATCH FOR THESE MESSAGES:");
    println!("• '[Panic Agent X] PANIC TRIGGER: Stock Y dropped Z%'");
    println!("• '[Panic Agent X] 🚨 PANIC SELLING: ... (amplification)'");
    println!("• '[ETF Maintenance Agent X] CREATION/REDEMPTION Arbitrage'");
    println!("• '[ETF Maintenance Agent X] Rebalancing: ...'");
    println!();
    println!("📈 KEY STOCKS TO MONITOR:");
    println!("• LEAPS (ID:17) - 40% of YEET ETF");
    println!("• YOLO (ID:3) - 30% of YEET + 25% of CASINO");
    println!("• DEGEN (ID:20) - 20% of YEET + 25% of CASINO");
    println!("• ROCKET (ID:7) - 10% of YEET + 20% of CASINO + 25% of COPE");
    println!();
    println!("🌪️  The more interconnected the holdings, the bigger the cascade!");
    println!("Press Enter to stop the feedback loop experiment...");

    let mut buffer = String::new();
    io::stdin().read_line(&mut buffer).unwrap();
    println!("🛑 Stopping feedback loop test");
}
