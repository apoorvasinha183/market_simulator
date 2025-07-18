import gymnasium as gym
import numpy as np
from collections import deque
from typing import Dict, List, Tuple
import uuid

from market_gateway_client.rl_gateway import RLGateway
import gymnasium as gym

class PortfolioEnv(gym.Env):
    """
    A portfolio management environment that addresses the curse of dimensionality
    by using hierarchical actions and portfolio-level decisions.
    
    This environment uses several techniques to manage complexity:
    1. Hierarchical actions (sector -> stock selection)
    2. Portfolio rebalancing instead of individual orders
    3. Reduced action space through clustering
    4. Multi-objective rewards (return + risk)
    """
    
    def __init__(self, gateway: RLGateway, num_stocks=20, history_length=30, 
                 num_sectors=4, rebalance_frequency=10):
        super().__init__()
        self.gateway = gateway
        self.agent_id = self.gateway.register_agent()
        self.num_stocks = num_stocks
        self.history_length = history_length
        self.num_sectors = num_sectors
        self.rebalance_frequency = rebalance_frequency
        self.step_count = 0
        
        self.stock_ids = list(range(1, self.num_stocks + 1))
        
        # Group stocks into sectors for hierarchical decision making
        self.sectors = self._create_sectors()
        
        # Features: price returns, volatility, volume, technical indicators
        self.num_features_per_stock = 8  # Reduced feature set
        self.num_features_per_step = (self.num_stocks * self.num_features_per_stock) + 1  # +1 for portfolio value
        
        # Observation space: (history, features)
        self.observation_space = gym.spaces.Box(
            low=-np.inf, high=np.inf,
            shape=(self.history_length, self.num_features_per_step),
            dtype=np.float32
        )
        
        # Hierarchical Action Space:
        # 1. Portfolio allocation across sectors (continuous)
        # 2. Stock selection within sectors (discrete)
        self.action_space = gym.spaces.Dict({
            'sector_weights': gym.spaces.Box(
                low=0.0, high=1.0, 
                shape=(self.num_sectors,), 
                dtype=np.float32
            ),
            'stock_selection': gym.spaces.MultiDiscrete([
                len(sector) for sector in self.sectors.values()
            ])
        })
        
        self.observation_history = deque(maxlen=self.history_length)
        self.portfolio_value_history = deque(maxlen=100)
        self.current_positions = {stock_id: 0 for stock_id in self.stock_ids}
        
    def _create_sectors(self) -> Dict[str, List[int]]:
        """Group stocks into sectors for hierarchical decision making."""
        stocks_per_sector = self.num_stocks // self.num_sectors
        sectors = {}
        
        for i in range(self.num_sectors):
            start_idx = i * stocks_per_sector
            end_idx = start_idx + stocks_per_sector
            if i == self.num_sectors - 1:  # Last sector gets remaining stocks
                end_idx = self.num_stocks
            
            sector_name = f"sector_{i}"
            sectors[sector_name] = self.stock_ids[start_idx:end_idx]
            
        return sectors
    
    def _get_observation(self) -> np.array:
        """Get enhanced observation with technical indicators and risk metrics."""
        obs_vector = []
        
        # Get recent price history for technical indicators
        price_history = {}
        for stock_id in self.stock_ids:
            l1_data = self.gateway.get_l1_data(stock_id)
            if l1_data:
                price_history[stock_id] = l1_data.last_traded_price
            else:
                price_history[stock_id] = 100.0  # Default price
        
        # Calculate features for each stock
        for stock_id in self.stock_ids:
            l1_data = self.gateway.get_l1_data(stock_id)
            
            if l1_data:
                # Price-based features
                current_price = l1_data.last_traded_price
                spread = (l1_data.best_ask_price - l1_data.best_bid_price) / current_price if current_price > 0 else 0
                
                # Volume features
                total_volume = l1_data.best_bid_volume + l1_data.best_ask_volume
                volume_imbalance = (l1_data.best_bid_volume - l1_data.best_ask_volume) / total_volume if total_volume > 0 else 0
                
                # Position features
                current_position = self.current_positions[stock_id]
                position_pnl = current_position * (current_price - 100.0)  # Assuming 100 as reference price
                
                obs_vector.extend([
                    current_price / 100.0,  # Normalized price
                    spread,
                    volume_imbalance,
                    total_volume / 10000.0,  # Normalized volume
                    current_position / 1000.0,  # Normalized position
                    position_pnl / 10000.0,  # Normalized P&L
                    l1_data.best_bid_price / 100.0,  # Normalized bid
                    l1_data.best_ask_price / 100.0,  # Normalized ask
                ])
            else:
                obs_vector.extend([1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0])  # Default values
        
        # Add portfolio-level features
        portfolio_value = self.gateway.evaluate_portfolio(self.agent_id)
        obs_vector.append(portfolio_value / 100000.0)  # Normalized portfolio value
        
        return np.array(obs_vector, dtype=np.float32)
    
    def reset(self, seed=None, options=None):
        super().reset(seed=seed)
        self.observation_history.clear()
        self.portfolio_value_history.clear()
        self.current_positions = {stock_id: 0 for stock_id in self.stock_ids}
        self.step_count = 0
        
        # Pre-fill the history
        for _ in range(self.history_length):
            self.observation_history.append(self._get_observation())
        
        initial_obs = np.array(self.observation_history, dtype=np.float32)
        return initial_obs, {}
    
    def step(self, action):
        """Execute portfolio rebalancing action."""
        self.step_count += 1
        
        # Only rebalance at specified frequency
        if self.step_count % self.rebalance_frequency == 0:
            self._execute_portfolio_rebalancing(action)
        
        # Update observation
        self.observation_history.append(self._get_observation())
        
        # Calculate reward
        reward = self._calculate_portfolio_reward()
        
        obs = np.array(self.observation_history, dtype=np.float32)
        return obs, reward, False, False, {}
    
    def _execute_portfolio_rebalancing(self, action):
        """Execute hierarchical portfolio rebalancing."""
        sector_weights = action['sector_weights']
        stock_selections = action['stock_selection']
        
        # Normalize sector weights to sum to 1
        sector_weights = sector_weights / np.sum(sector_weights)
        
        # Get current portfolio value
        portfolio_value = self.gateway.evaluate_portfolio(self.agent_id)
        
        # Calculate target positions for each stock
        target_positions = {}
        
        for i, (sector_name, stock_list) in enumerate(self.sectors.items()):
            sector_allocation = portfolio_value * sector_weights[i]
            
            # Select stock within sector
            selected_stock_idx = stock_selections[i] % len(stock_list)
            selected_stock = stock_list[selected_stock_idx]
            
            # Get current price
            l1_data = self.gateway.get_l1_data(selected_stock)
            current_price = l1_data.last_traded_price if l1_data else 100.0
            
            # Calculate target shares
            target_shares = int(sector_allocation / current_price) if current_price > 0 else 0
            target_positions[selected_stock] = target_shares
        
        # Execute rebalancing orders
        for stock_id, target_position in target_positions.items():
            current_position = self.current_positions[stock_id]
            position_diff = target_position - current_position
            
            if abs(position_diff) > 10:  # Only trade if significant difference
                side = "Buy" if position_diff > 0 else "Sell"
                volume = abs(position_diff)
                
                # Check constraints
                if self._can_execute_trade(stock_id, side, volume):
                    self.gateway.submit_order(
                        agent_id=self.agent_id,
                        stock_id=stock_id,
                        side=side,
                        order_type="Market",
                        volume=volume,
                        price=0.0
                    )
                    self.current_positions[stock_id] = target_position
    
    def _can_execute_trade(self, stock_id: int, side: str, volume: int) -> bool:
        """Check if trade can be executed given constraints."""
        current_cash = self.gateway._agent_state[self.agent_id]['cash']
        current_inventory = self.current_positions[stock_id]
        
        if side == "Buy":
            l1_data = self.gateway.get_l1_data(stock_id)
            estimated_cost = volume * (l1_data.best_ask_price if l1_data else 100.0)
            return current_cash >= estimated_cost
        else:  # Sell
            return current_inventory >= volume
    
    def _calculate_portfolio_reward(self) -> float:
        """Calculate multi-objective reward combining return and risk."""
        current_portfolio_value = self.gateway.evaluate_portfolio(self.agent_id)
        self.portfolio_value_history.append(current_portfolio_value)
        
        if len(self.portfolio_value_history) < 2:
            return 0.0
        
        # Calculate return
        returns = np.diff(list(self.portfolio_value_history))
        current_return = returns[-1] if len(returns) > 0 else 0.0
        
        # Calculate risk (volatility penalty)
        if len(returns) >= 10:
            volatility = np.std(returns[-10:])
            risk_penalty = -0.1 * volatility  # Penalize high volatility
        else:
            risk_penalty = 0.0
        
        # Combine return and risk
        reward = current_return + risk_penalty
        
        # Add diversification bonus
        active_positions = sum(1 for pos in self.current_positions.values() if pos > 0)
        diversification_bonus = 0.01 * min(active_positions, 5)  # Bonus for up to 5 positions
        
        return reward + diversification_bonus


class SimplifiedPortfolioEnv(gym.Env):
    """
    A simplified portfolio environment that reduces dimensionality through:
    1. Top-K stock selection (only trade top K stocks)
    2. Discrete allocation levels
    3. Sector rotation strategy
    """
    
    def __init__(self, gateway: RLGateway, num_stocks=20, top_k=5, history_length=30):
        super().__init__()
        self.gateway = gateway
        self.agent_id = self.gateway.register_agent()
        self.num_stocks = num_stocks
        self.top_k = top_k
        self.history_length = history_length
        
        self.stock_ids = list(range(1, self.num_stocks + 1))
        
        # Simplified observation: only top-K stocks + portfolio metrics
        self.num_features_per_step = (self.top_k * 5) + 3  # 5 features per stock + 3 portfolio metrics
        
        self.observation_space = gym.spaces.Box(
            low=-np.inf, high=np.inf,
            shape=(self.history_length, self.num_features_per_step),
            dtype=np.float32
        )
        
        # Simplified action space: allocation weights for top-K stocks
        self.action_space = gym.spaces.Box(
            low=0.0, high=1.0,
            shape=(self.top_k,),
            dtype=np.float32
        )
        
        self.observation_history = deque(maxlen=self.history_length)
        self.current_top_k = []
        
    def _select_top_k_stocks(self) -> List[int]:
        """Select top K stocks based on momentum/volume criteria."""
        stock_scores = []
        
        for stock_id in self.stock_ids:
            l1_data = self.gateway.get_l1_data(stock_id)
            if l1_data:
                # Simple scoring: combine price momentum and volume
                volume_score = l1_data.best_bid_volume + l1_data.best_ask_volume
                price_score = l1_data.last_traded_price
                total_score = volume_score * 0.3 + price_score * 0.7
                stock_scores.append((stock_id, total_score))
            else:
                stock_scores.append((stock_id, 0.0))
        
        # Sort by score and take top K
        stock_scores.sort(key=lambda x: x[1], reverse=True)
        return [stock_id for stock_id, _ in stock_scores[:self.top_k]]
    
    def _get_observation(self) -> np.array:
        """Get simplified observation for top-K stocks."""
        obs_vector = []
        
        # Update top-K selection periodically
        if len(self.current_top_k) == 0 or np.random.random() < 0.1:  # 10% chance to reselect
            self.current_top_k = self._select_top_k_stocks()
        
        # Features for top-K stocks
        for stock_id in self.current_top_k:
            l1_data = self.gateway.get_l1_data(stock_id)
            if l1_data:
                obs_vector.extend([
                    l1_data.last_traded_price / 100.0,
                    l1_data.best_bid_price / 100.0,
                    l1_data.best_ask_price / 100.0,
                    l1_data.best_bid_volume / 1000.0,
                    l1_data.best_ask_volume / 1000.0,
                ])
            else:
                obs_vector.extend([1.0, 1.0, 1.0, 1.0, 1.0])
        
        # Portfolio-level features
        agent_state = self.gateway._agent_state[self.agent_id]
        portfolio_value = self.gateway.evaluate_portfolio(self.agent_id)
        total_inventory = sum(agent_state['inventory'].values())
        
        obs_vector.extend([
            agent_state['cash'] / 100000.0,  # Normalized cash
            portfolio_value / 100000.0,     # Normalized portfolio value
            total_inventory / 1000.0,       # Normalized total inventory
        ])
        
        return np.array(obs_vector, dtype=np.float32)
    
    def reset(self, seed=None, options=None):
        super().reset(seed=seed)
        self.observation_history.clear()
        self.current_top_k = []
        
        for _ in range(self.history_length):
            self.observation_history.append(self._get_observation())
        
        initial_obs = np.array(self.observation_history, dtype=np.float32)
        return initial_obs, {}
    
    def step(self, action):
        """Execute simplified portfolio allocation."""
        # Normalize action to sum to 1
        action = action / np.sum(action) if np.sum(action) > 0 else np.ones_like(action) / len(action)
        
        # Execute trades for top-K stocks
        portfolio_value = self.gateway.evaluate_portfolio(self.agent_id)
        
        for i, stock_id in enumerate(self.current_top_k):
            target_allocation = portfolio_value * action[i]
            l1_data = self.gateway.get_l1_data(stock_id)
            
            if l1_data and l1_data.last_traded_price > 0:
                target_shares = int(target_allocation / l1_data.last_traded_price)
                current_shares = self.gateway._agent_state[self.agent_id]['inventory'][stock_id]
                
                share_diff = target_shares - current_shares
                
                if abs(share_diff) > 5:  # Only trade if significant difference
                    side = "Buy" if share_diff > 0 else "Sell"
                    volume = abs(share_diff)
                    
                    self.gateway.submit_order(
                        agent_id=self.agent_id,
                        stock_id=stock_id,
                        side=side,
                        order_type="Market",
                        volume=volume,
                        price=0.0
                    )
        
        # Update observation
        self.observation_history.append(self._get_observation())
        
        # Calculate reward (portfolio return)
        new_portfolio_value = self.gateway.evaluate_portfolio(self.agent_id)
        reward = new_portfolio_value - portfolio_value
        
        obs = np.array(self.observation_history, dtype=np.float32)
        return obs, reward, False, False, {}