// src/agents/agent_type.rs

#[derive(Debug, Clone)]
pub enum AgentType {
    DumbMarket,
    DumbLimit,
    MarketMaker,
    IPO,
    WhaleAgent,
    Astrologer,
    MomentumAgent,
    CustomerAgent,
    WebProxyAgent,
    ETFAgent { etf_stock_id: u64 }, // Manages a specific ETF
    ETFMaintenanceAgent { etf_stock_id: u64 }, // Specialized ETF price maintenance
    PanicAgent { monitored_stocks: Vec<u64> }, // Creates feedback loops through panic selling
    Thermodynamic {
        initial_temperature: f64,
        specific_heat: f64,
        initial_chemical_potential: f64,
    },
}
