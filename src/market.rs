// src/market.rs

use crate::{
    Agent, AgentType, DumbAgent, DumbLimitAgent, IpoAgent, MarketMakerAgent, MarketView,
    Marketable, Order, OrderBook, OrderRequest, Side, Trade, WhaleAgent,
};
use std::any::Any;
use std::collections::HashMap;

/// This is the main simulation engine. It owns the world state and participants.
pub struct Market {
    order_book: OrderBook,
    agents: HashMap<usize, Box<dyn Agent>>,
    last_traded_price: f64,
    initial_agent_types: Vec<AgentType>,
    order_id_counter: u64,
    cumulative_volume: u64,
}

impl Market {
    pub fn new(participant_types: &[AgentType]) -> Self {
        let mut agents = HashMap::new();
        let mut agent_id_counter: usize = 0;

        for agent_type in participant_types {
            let agent = Self::create_agent_from_type(*agent_type, agent_id_counter);
            agents.insert(agent_id_counter, agent);
            agent_id_counter += 1;
        }

        Self {
            order_book: OrderBook::new(),
            agents,
            last_traded_price: 150.00,
            initial_agent_types: participant_types.to_vec(),
            order_id_counter: 0,
            cumulative_volume: 0,
        }
    }

    fn create_agent_from_type(agent_type: AgentType, id: usize) -> Box<dyn Agent> {
        match agent_type {
            AgentType::DumbMarket => Box::new(DumbAgent::new(id)),
            AgentType::DumbLimit => Box::new(DumbLimitAgent::new(id)),
            AgentType::MarketMaker => Box::new(MarketMakerAgent::new(id)),
            AgentType::IPO => Box::new(IpoAgent::new(id)),
            AgentType::WhaleAgent => Box::new(WhaleAgent::new(id)),
        }
    }

    fn next_order_id(&mut self) -> u64 {
        self.order_id_counter += 1;
        self.order_id_counter
    }

    pub fn get_order_book(&self) -> &OrderBook {
        &self.order_book
    }
    pub fn cumulative_volume(&self) -> u64 {
        self.cumulative_volume
    }
    pub fn get_total_inventory(&self) -> i64 {
        self.agents
            .values()
            .map(|agent| agent.get_inventory())
            .sum()
    }
}

impl Marketable for Market {
    fn step(&mut self) -> f64 {
        // --- Phase 1: Agent Decisions ---
        let market_view = MarketView {
            order_book: &self.order_book,
            //last_traded_price: self.last_traded_price,
        };
        let mut all_requests = Vec::new();
        let mut agent_ids: Vec<usize> = self.agents.keys().cloned().collect();
        agent_ids.sort_unstable();
        for id in &agent_ids {
            if let Some(agent) = self.agents.get_mut(id) {
                all_requests.extend(agent.decide_actions(&market_view));
            }
        }

        let mut trades_this_tick: Vec<Trade> = Vec::new();

        // --- Phase 2: Process All Requests ---
        for request in all_requests {
            match request {
                OrderRequest::LimitOrder {
                    agent_id,
                    side,
                    price,
                    volume,
                } => {
                    let mut order = Order {
                        id: self.next_order_id(),
                        agent_id,
                        side,
                        price,
                        volume,
                        filled: 0,
                    };
                    if let Some(agent) = self.agents.get_mut(&agent_id) {
                        agent.acknowledge_order(order);
                    }
                    trades_this_tick.extend(self.order_book.process_limit_order(&mut order));
                }
                OrderRequest::MarketOrder {
                    agent_id,
                    side,
                    volume,
                } => {
                    let order = Order {
                        id: self.next_order_id(),
                        agent_id,
                        side,
                        price: (self.last_traded_price * 100.0).round() as u64,
                        volume,
                        filled: 0,
                    };
                    if let Some(agent) = self.agents.get_mut(&agent_id) {
                        agent.acknowledge_order(order);
                    }
                    trades_this_tick
                        .extend(self.order_book.process_market_order(agent_id, side, volume));
                }
                OrderRequest::CancelOrder { agent_id, order_id } => {
                    self.order_book.cancel_order(order_id, agent_id);
                }
            }
        }

        // --- Phase 3: Margin Call Phase ---
        let mut margin_requests = Vec::new();
        for id in &agent_ids {
            if let Some(agent) = self.agents.get_mut(id) {
                margin_requests.extend(agent.margin_call());
            }
        }
        for request in margin_requests {
            if let OrderRequest::MarketOrder {
                agent_id,
                side,
                volume,
            } = request
            {
                trades_this_tick
                    .extend(self.order_book.process_market_order(agent_id, side, volume));
            }
        }

        // --- Phase 4: Update Portfolios from all trades this tick ---
        for trade in &trades_this_tick {
            if let Some(taker) = self.agents.get_mut(&trade.taker_agent_id) {
                let change = if trade.taker_side == Side::Buy {
                    trade.volume as i64
                } else {
                    -(trade.volume as i64)
                };
                taker.update_portfolio(change, trade);
            }
            if let Some(maker) = self.agents.get_mut(&trade.maker_agent_id) {
                let change = if trade.taker_side == Side::Sell {
                    trade.volume as i64
                } else {
                    -(trade.volume as i64)
                };
                maker.update_portfolio(change, trade);
            }
        }

        // --- Phase 5: Update Market-Level State ---
        if let Some(last_trade) = trades_this_tick.last() {
            self.last_traded_price = last_trade.price as f64 / 100.0;
        }
        let tick_volume: u64 = trades_this_tick.iter().map(|t| t.volume).sum();
        self.cumulative_volume = self.cumulative_volume.saturating_add(tick_volume);

        self.last_traded_price
    }

    fn current_price(&self) -> f64 {
        self.last_traded_price
    }

    fn reset(&mut self) {
        let mut agents = HashMap::new();
        for (id, agent_type) in self.initial_agent_types.iter().enumerate() {
            let agent = Self::create_agent_from_type(*agent_type, id);
            agents.insert(id, agent);
        }
        self.agents = agents;
        self.order_book = OrderBook::new();
        self.last_traded_price = 150.00;
        self.order_id_counter = 0;
        self.cumulative_volume = 0;
    }

    fn get_order_book(&self) -> Option<&OrderBook> {
        Some(&self.order_book)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

// -----------------------------------------------------------------------------
//  Integration Tests
// -----------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    /// A controllable agent for testing specific scenarios.
    #[derive(Default)]
    struct TestAgent {
        id: usize,
        inventory: i64,
        requests: Vec<OrderRequest>,
        acknowledged_orders: HashMap<u64, Order>,
    }

    impl Agent for TestAgent {
        fn decide_actions(&mut self, _market_view: &MarketView) -> Vec<OrderRequest> {
            self.requests.drain(..).collect()
        }
        fn acknowledge_order(&mut self, order: Order) {
            self.acknowledged_orders.insert(order.id, order);
        }
        fn update_portfolio(&mut self, change: i64, _trade: &Trade) {
            self.inventory += change;
        }
        fn get_inventory(&self) -> i64 {
            self.inventory
        }

        // --- THIS IS THE FIX: Correctly implement get_pending_orders ---
        fn get_pending_orders(&self) -> Vec<Order> {
            self.acknowledged_orders.values().cloned().collect()
        }

        // --- Unused methods for this test agent ---
        fn margin_call(&mut self) -> Vec<OrderRequest> {
            vec![]
        }
        fn buy_stock(&mut self, _vol: u64) -> Vec<OrderRequest> {
            vec![]
        }
        fn sell_stock(&mut self, _vol: u64) -> Vec<OrderRequest> {
            vec![]
        }
        fn cancel_open_order(&mut self, _id: u64) -> Vec<OrderRequest> {
            vec![]
        }
        fn get_id(&self) -> usize {
            self.id
        }
        fn clone_agent(&self) -> Box<dyn Agent> {
            Box::new(TestAgent::default())
        }
        fn evaluate_port(&self, market_view: &MarketView) -> f64 {
            let price_cents = match market_view.get_mid_price() {
                Some(p) => p,
                None => return 0.0, // or whatever you deem appropriate
            };
            let value_cents = (self.inventory as i128)
                .checked_mul(price_cents as i128)
                .expect("portfolio value overflow");
            (value_cents as f64) / 100.0
        }
    }

    #[test]
    fn test_end_to_end_trade_and_inventory_update() {
        // Arrange
        let agent_types = vec![]; // We'll add agents manually
        let mut market = Market::new(&agent_types);

        let mut seller = Box::new(TestAgent {
            id: 0,
            inventory: 100,
            ..Default::default()
        });
        seller.requests.push(OrderRequest::LimitOrder {
            agent_id: 0,
            side: Side::Sell,
            price: 15000,
            volume: 50,
        });

        let mut buyer = Box::new(TestAgent {
            id: 1,
            inventory: 0,
            ..Default::default()
        });
        buyer.requests.push(OrderRequest::MarketOrder {
            agent_id: 1,
            side: Side::Buy,
            volume: 30,
        });

        market.agents.insert(0, seller);
        market.agents.insert(1, buyer);

        // Act
        market.step();

        // Assert
        let seller_final = market.agents.get(&0).unwrap();
        let buyer_final = market.agents.get(&1).unwrap();

        assert_eq!(
            seller_final.get_inventory(),
            70,
            "Seller's inventory should decrease by 30."
        );
        assert_eq!(
            buyer_final.get_inventory(),
            30,
            "Buyer's inventory should increase by 30."
        );
        assert_eq!(
            market.last_traded_price, 150.00,
            "Last traded price should be updated."
        );
    }

    #[test]
    fn test_order_acknowledgement_flow() {
        // Arrange
        let agent_types = vec![];
        let mut market = Market::new(&agent_types);

        let mut agent = Box::new(TestAgent {
            id: 0,
            ..Default::default()
        });
        agent.requests.push(OrderRequest::LimitOrder {
            agent_id: 0,
            side: Side::Buy,
            price: 14900,
            volume: 10,
        });

        market.agents.insert(0, agent);

        // Act
        market.step();

        // Assert
        // --- THIS IS THE FIX: Use the public API of the trait ---
        let agent_final = market.agents.get(&0).unwrap();
        let pending_orders = agent_final.get_pending_orders();

        assert_eq!(
            pending_orders.len(),
            1,
            "Agent should have one acknowledged order."
        );
        let ack_order = &pending_orders[0];
        assert_eq!(
            ack_order.id, 1,
            "The acknowledged order should have a valid, non-zero ID."
        );
    }
}
