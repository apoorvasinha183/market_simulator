import gymnasium as gym
import numpy as np
from collections import deque
import uuid

from market_gateway_client.rl_gateway import RLGateway

# Define the number of price levels for limit orders
NUM_LIMIT_PRICE_LEVELS = 3

class AggressivePortfolioEnv(gym.Env):
    """
    A portfolio environment that FORCES the agent to invest money.
    No more sitting on cash like a coward!
    """
    def __init__(self, gateway: RLGateway, num_stocks=20, history_length=30, target_stock_id: int = None):
        super().__init__()
        self.gateway = gateway
        self.agent_id = self.gateway.register_agent()
        self.history_length = history_length

        if target_stock_id is not None:
            self.num_stocks = 1
            self.stock_ids = [target_stock_id]
            print(f"AggressivePortfolioEnv initialized for single stock: {target_stock_id}")
        else:
            self.num_stocks = num_stocks
            self.stock_ids = list(range(1, self.num_stocks + 1))
            print(f"AggressivePortfolioEnv initialized for {self.num_stocks} stocks.")

        # Calculate total features per step
        self.num_features_per_step = (self.num_stocks * 6) + 1 

        # Observation space: (history, features)
        self.observation_space = gym.spaces.Box(
            low=-np.inf, high=np.inf,
            shape=(self.history_length, self.num_features_per_step),
            dtype=np.float32
        )

        # Action space: Keep hold action but penalize cash hoarding
        self.num_actions_per_stock = 2 + (2 * NUM_LIMIT_PRICE_LEVELS) # Market orders + Num Price levels times the limit orders
        self.action_space = gym.spaces.Discrete(1 + (self.num_stocks * self.num_actions_per_stock))

        self.observation_history = deque(maxlen=self.history_length)
        self.portfolio_value = 0.0
        self.cash_penalty_threshold = 0.9  # Penalize if >80% cash
        self.steps_since_trade = 0
        self.max_steps_without_trade = 20  # Force trade every 50 steps

    def _get_observation(self):
        """Constructs the observation vector from the latest L1 data, cash, and inventory."""
        obs_vector = []
        
        # Add L1 market data for the relevant stock(s)
        for stock_id in self.stock_ids:
            l1_data = self.gateway.get_l1_data(stock_id)
            if l1_data:
                obs_vector.extend([
                    l1_data.best_bid_price,
                    l1_data.best_bid_volume,
                    l1_data.best_ask_price,
                    l1_data.best_ask_volume,
                    l1_data.last_traded_price
                ])
            else:
                obs_vector.extend([0.0, 0.0, 0.0, 0.0, 0.0])
        
        # Add agent's cash balance
        agent_state = self.gateway._agent_state[self.agent_id]
        obs_vector.append(agent_state['cash'])

        # Add agent's inventory for each relevant stock
        for stock_id in self.stock_ids:
            obs_vector.append(agent_state['inventory'].get(stock_id, 0))

        return np.array(obs_vector, dtype=np.float32)

    def reset(self, seed=None, options=None):
        super().reset(seed=seed)
        self.observation_history.clear()
        self.steps_since_trade = 0

        # Pre-fill the history with initial observations
        for _ in range(self.history_length):
            self.observation_history.append(self._get_observation())

        self.portfolio_value = self.gateway.evaluate_portfolio(self.agent_id)
        
        initial_obs = np.array(self.observation_history, dtype=np.float32)
        info = {}
        return initial_obs, info

    def step(self, action):
        """Execute one time step - FORCE TRADING!"""
        reward = 0.0
        
        # Decode the action (no hold action available!)
        if self.num_stocks == 1:
            stock_idx = 0
            action_type_idx = action
        else:
            stock_idx = action // self.num_actions_per_stock
            action_type_idx = action % self.num_actions_per_stock
        
        stock_id = self.stock_ids[stock_idx]
        
        # Determine order type, side, and price level
        order_type = "Hold"  # Default fallback
        side = None
        price = 0.0
        volume = 100  # Fixed volume : TODO: Maybe think about parametrizing this as well
        
        if action_type_idx == 0:  # Market Buy
            order_type = "Market"
            side = "Buy"
        elif action_type_idx == 1:  # Market Sell
            order_type = "Market"
            side = "Sell"
        elif action_type_idx >= 2 and action_type_idx < (2 + NUM_LIMIT_PRICE_LEVELS):  # Limit Buy
            order_type = "Limit"
            side = "Buy"
            price_level_offset = action_type_idx - 2
            l1_data = self.gateway.get_l1_data(stock_id)
            if l1_data and l1_data.best_bid_price > 0:
                price = round(l1_data.best_bid_price - (price_level_offset * 0.01), 2)
            else:
                price = 150.00  # Fallback
            price = max(0.01, price) # Don't send stupid limit orders
        elif action_type_idx >= (2 + NUM_LIMIT_PRICE_LEVELS):  # Limit Sell
            order_type = "Limit"
            side = "Sell"
            price_level_offset = action_type_idx - (2 + NUM_LIMIT_PRICE_LEVELS)
            l1_data = self.gateway.get_l1_data(stock_id)
            if l1_data and l1_data.best_ask_price > 0:
                price = round(l1_data.best_ask_price + (price_level_offset * 0.01), 2)
            else:
                price = 150.00  # Fallback
            price = max(0.01, price) # Don't send stupid limit orders

        # Get current state before trade
        current_cash = self.gateway._agent_state[self.agent_id]['cash']
        current_inventory = self.gateway._agent_state[self.agent_id]['inventory'].get(stock_id, 0)
        l1_data = self.gateway.get_l1_data(stock_id)
        
        # AGGRESSIVE TRADING LOGIC
        action_executed = False
        
        if side == "Buy":
            estimated_cost = volume * (price if order_type == "Limit" else (l1_data.best_ask_price if l1_data else 150.00))
            if current_cash >= estimated_cost:
                self.gateway.submit_order(
                    agent_id=self.agent_id,
                    stock_id=stock_id,
                    side=side,
                    order_type=order_type,
                    volume=volume,
                    price=price
                )
                action_executed = True
                self.steps_since_trade = 0
            else:
                # Not enough cash - small penalty but encourage smaller trades
                reward -= 0.05
                
        elif side == "Sell":
            if current_inventory >= volume:
                self.gateway.submit_order(
                    agent_id=self.agent_id,
                    stock_id=stock_id,
                    side=side,
                    order_type=order_type,
                    volume=volume,
                    price=price
                )
                action_executed = True
                self.steps_since_trade = 0
            else:
                # Don't have enough inventory - small penalty
                # Since we are working with a fixed volume this will never happen
                reward -= 0.05

        # Update observation history
        self.observation_history.append(self._get_observation())
        
        # Calculate portfolio-based reward
        new_portfolio_value = self.gateway.evaluate_portfolio(self.agent_id)
        portfolio_change = new_portfolio_value - self.portfolio_value
        reward += portfolio_change
        
        # AGGRESSIVE REWARD STRUCTURE
        
        # 1. Cash Hoarding Penalty - Punish sitting on cash!
        cash_ratio = current_cash / new_portfolio_value if new_portfolio_value > 0 else 1.0
        if cash_ratio > self.cash_penalty_threshold:
            cash_penalty = -0.1 * (cash_ratio - self.cash_penalty_threshold)
            reward += cash_penalty
            
        # 2. Trading Bonus - Reward actual trading
        if action_executed:
            reward += 0.02  # Small bonus for executing trades
            
        # 3. Inactivity Penalty - Punish not trading for too long
        self.steps_since_trade += 1
        if self.steps_since_trade > self.max_steps_without_trade:
            reward -= 0.1  # Escalating penalty for inactivity
            
        # 4. Portfolio Utilization Bonus - Reward being invested
        total_portfolio = new_portfolio_value
        invested_value = total_portfolio - current_cash
        utilization_ratio = invested_value / total_portfolio if total_portfolio > 0 else 0.0
        
        if utilization_ratio > 0.2:  # Reward being >20% invested
            reward += 0.01 * utilization_ratio
            
        # 5. Diversification Bonus (for multi-stock)
        if self.num_stocks > 1:
            active_positions = sum(1 for pos in self.gateway._agent_state[self.agent_id]['inventory'].values() if pos > 0)
            if active_positions > 1:
                reward += 0.005 * active_positions  # Bonus for diversification

        self.portfolio_value = new_portfolio_value

        terminated = False
        truncated = False
        info = {
            'action_executed': action_executed,
            'cash_ratio': cash_ratio,
            'utilization_ratio': utilization_ratio,
            'steps_since_trade': self.steps_since_trade
        }

        obs = np.array(self.observation_history, dtype=np.float32)
        return obs, reward, terminated, truncated, info


class ForceInvestmentEnv(AggressivePortfolioEnv):
    """
    Even MORE aggressive - literally forces the agent to invest a minimum amount.
    """
    
    def __init__(self, gateway: RLGateway, min_investment_ratio=0.5, **kwargs):
        super().__init__(gateway, **kwargs)
        self.min_investment_ratio = min_investment_ratio  # Must invest at least 50%
        self.forced_investment_penalty = -1.0  # Heavy penalty for not investing
        
    def step(self, action):
        obs, reward, terminated, truncated, info = super().step(action)
        
        # FORCE MINIMUM INVESTMENT
        current_cash = self.gateway._agent_state[self.agent_id]['cash']
        portfolio_value = self.gateway.evaluate_portfolio(self.agent_id)
        
        cash_ratio = current_cash / portfolio_value if portfolio_value > 0 else 1.0
        
        if cash_ratio > (1.0 - self.min_investment_ratio):
            # Not invested enough - HEAVY penalty
            under_investment = cash_ratio - (1.0 - self.min_investment_ratio)
            reward += self.forced_investment_penalty * under_investment
            
            # Add info for debugging
            info['under_invested'] = True
            info['required_investment'] = self.min_investment_ratio
            info['actual_investment'] = 1.0 - cash_ratio
        else:
            info['under_invested'] = False
            
        return obs, reward, terminated, truncated, info


class SmartAggressiveEnv(AggressivePortfolioEnv):
    """
    Balanced aggressive approach - encourages trading but not recklessly.
    """
    
    def __init__(self, gateway: RLGateway, **kwargs):
        super().__init__(gateway, **kwargs)
        self.trade_history = deque(maxlen=100)  # Track recent trades
        self.price_history = deque(maxlen=50)   # Track price movements
        
    def step(self, action):
        # Store price before action
        l1_data = self.gateway.get_l1_data(self.stock_ids[0])
        if l1_data:
            self.price_history.append(l1_data.last_traded_price)
        
        obs, reward, terminated, truncated, info = super().step(action)
        
        # SMART TRADING REWARDS
        
        # 1. Momentum Trading Bonus
        if len(self.price_history) >= 5:
            recent_prices = list(self.price_history)[-5:]
            price_trend = (recent_prices[-1] - recent_prices[0]) / recent_prices[0]
            
            if info['action_executed']:
                # Reward trading in direction of momentum
                if action in [0, 2, 3, 4] and price_trend > 0.001:  # Buy on uptrend
                    reward += 0.05
                elif action in [1, 5, 6, 7] and price_trend < -0.001:  # Sell on downtrend
                    reward += 0.05
                    
        # 2. Risk Management - Don't go all-in on one position
        current_inventory = self.gateway._agent_state[self.agent_id]['inventory'].get(self.stock_ids[0], 0)
        portfolio_value = self.gateway.evaluate_portfolio(self.agent_id)
        
        if current_inventory > 0:
            position_size = (current_inventory * l1_data.last_traded_price) / portfolio_value if l1_data and portfolio_value > 0 else 0
            if position_size > 0.8:  # More than 80% in one stock
                reward -= 0.1  # Risk penalty
                
        # 3. Profit Taking Bonus
        if len(self.trade_history) >= 2:
            # TODO: Simple profit tracking (would need more sophisticated implementation)
            pass
            
        return obs, reward, terminated, truncated, info