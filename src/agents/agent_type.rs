// src/agents/agent_type.rs

#[derive(Debug, Clone, Copy)]
pub enum AgentType {
    DumbMarket,
    DumbLimit,
    // We can add more here later, like MarketMaker, Institutional, etc.
}
