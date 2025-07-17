// src/agents/agent_type.rs

#[derive(Debug, Clone, Copy)]
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
    Thermodynamic {
        initial_temperature: f64,
        specific_heat: f64,
        initial_chemical_potential: f64,
    },
}
