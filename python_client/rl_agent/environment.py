import gymnasium as gym
import numpy as np
from collections import deque
import uuid

from market_gateway_client.rl_gateway import RLGateway

# Define the number of price levels for limit orders
NUM_LIMIT_PRICE_LEVELS = 3 # e.g., Best Bid/Ask, Best Bid/Ask +/- 0.01, Best Bid/Ask +/- 0.02

class MarketEnv(gym.Env):
    """
    A custom Gymnasium environment for the CHADSDAQ market simulator.

    Observation Space:
        A history of L1 market data for all stocks, plus agent's cash and inventory.
        Shape: (history_length, num_features_per_step)
        Features per stock: [best_bid, best_bid_vol, best_ask, best_ask_vol, last_price]
        Total features per step: (num_stocks * 5) + 1 (cash) + num_stocks (inventory)

    Action Space:
        Discrete space with:
        - 1 Hold action
        - num_stocks * (1 Market Buy + 1 Market Sell) actions
        - num_stocks * (NUM_LIMIT_PRICE_LEVELS Limit Buy + NUM_LIMIT_PRICE_LEVELS Limit Sell) actions
    """
    def __init__(self, gateway: RLGateway, num_stocks=20, history_length=30):
        super().__init__()
        self.gateway = gateway
        self.agent_id = self.gateway.register_agent()
        self.num_stocks = num_stocks
        self.history_length = history_length
        self.stock_ids = list(range(1, self.num_stocks + 1))

        # Calculate total features per step
        # 5 L1 features per stock + 1 cash feature + 1 inventory feature per stock
        self.num_features_per_step = (self.num_stocks * 5) + 1 + self.num_stocks

        # Observation space: (history, features)
        self.observation_space = gym.spaces.Box(
            low=-np.inf, high=np.inf,
            shape=(self.history_length, self.num_features_per_step),
            dtype=np.float32
        )
        print(f"DEBUG: Inside MarketEnv.__init__, self.observation_space set to: {self.observation_space}") # DEBUG PRINT

        # Action space: 1 (Hold) + num_stocks * (2 Market + 2*NUM_LIMIT_PRICE_LEVELS Limit) actions
        self.num_actions_per_stock = 2 + (2 * NUM_LIMIT_PRICE_LEVELS)
        self.action_space = gym.spaces.Discrete(1 + (self.num_stocks * self.num_actions_per_stock))

        self.observation_history = deque(maxlen=self.history_length)
        self.portfolio_value = 0.0

    def _get_observation(self):
        """Constructs the observation vector from the latest L1 data, cash, and inventory."""
        obs_vector = []
        
        # Add L1 market data for all stocks
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
                # If no data, use zeros (or a more sophisticated imputation)
                obs_vector.extend([0.0, 0.0, 0.0, 0.0, 0.0])
        
        # Add agent's cash balance
        agent_state = self.gateway._agent_state[self.agent_id]
        obs_vector.append(agent_state['cash'])

        # Add agent's inventory for each stock
        for stock_id in self.stock_ids:
            obs_vector.append(agent_state['inventory'][stock_id])

        return np.array(obs_vector, dtype=np.float32)

    def reset(self, seed=None, options=None):
        super().reset(seed=seed)
        self.observation_history.clear()

        # Pre-fill the history with initial observations
        for _ in range(self.history_length):
            self.observation_history.append(self._get_observation())

        self.portfolio_value = self.gateway.evaluate_portfolio(self.agent_id)
        
        initial_obs = np.array(self.observation_history, dtype=np.float32)
        info = {}
        return initial_obs, info

    def step(self, action):
        """Execute one time step within the environment."""
        reward = 0.0 # Initialize reward for this step
        
        order_type = "Hold" # Default to Hold
        side = None
        price = 0.0
        volume = 100 # Fixed volume for now
        stock_id = None

        action_prevented = False

        if action > 0: # Action 0 is "Hold"
            # Decode the action
            action_idx_in_group = action - 1 # Adjust for the global Hold action
            stock_idx = action_idx_in_group // self.num_actions_per_stock
            action_type_idx = action_idx_in_group % self.num_actions_per_stock
            stock_id = self.stock_ids[stock_idx]

            # Determine order type, side, and price level
            if action_type_idx == 0: # Market Buy
                order_type = "Market"
                side = "Buy"
            elif action_type_idx == 1: # Market Sell
                order_type = "Market"
                side = "Sell"
            elif action_type_idx >= 2 and action_type_idx < (2 + NUM_LIMIT_PRICE_LEVELS): # Limit Buy
                order_type = "Limit"
                side = "Buy"
                price_level_offset = action_type_idx - 2
                l1_data = self.gateway.get_l1_data(stock_id)
                if l1_data and l1_data.best_bid_price > 0:
                    price = round(l1_data.best_bid_price - (price_level_offset * 0.01), 2)
                else:
                    price = round(self.gateway.get_l1_data(stock_id).last_traded_price - (price_level_offset * 0.01), 2) if self.gateway.get_l1_data(stock_id) else 150.00 # Fallback
                price = max(0.01, price) # Ensure price is not negative
            elif action_type_idx >= (2 + NUM_LIMIT_PRICE_LEVELS) and action_type_idx < (2 + 2 * NUM_LIMIT_PRICE_LEVELS): # Limit Sell
                order_type = "Limit"
                side = "Sell"
                price_level_offset = action_type_idx - (2 + NUM_LIMIT_PRICE_LEVELS)
                l1_data = self.gateway.get_l1_data(stock_id)
                if l1_data and l1_data.best_ask_price > 0:
                    price = round(l1_data.best_ask_price + (price_level_offset * 0.01), 2)
                else:
                    price = round(self.gateway.get_l1_data(stock_id).last_traded_price + (price_level_offset * 0.01), 2) if self.gateway.get_l1_data(stock_id) else 150.00 # Fallback
                price = max(0.01, price) # Ensure price is not negative
            
            # --- Financial Constraint Checks ---
            current_cash = self.gateway._agent_state[self.agent_id]['cash']
            current_inventory = self.gateway._agent_state[self.agent_id]['inventory'][stock_id]
            l1_data = self.gateway.get_l1_data(stock_id)

            if side == "Buy":
                # Estimate cost using the price we intend to submit (for Limit) or best ask (for Market)
                estimated_cost = volume * (price if order_type == "Limit" else (l1_data.best_ask_price if l1_data else 150.00))
                if current_cash < estimated_cost:
                    reward = -0.1 # Small penalty for invalid action
                    action_prevented = True
            elif side == "Sell":
                # Forbid short selling: agent must own the stock to sell it
                if current_inventory < volume:
                    reward = -0.1 # Small penalty for invalid action
                    action_prevented = True
            
            if not action_prevented: 
                self.gateway.submit_order(
                    agent_id=self.agent_id,
                    stock_id=stock_id,
                    side=side,
                    order_type=order_type,
                    volume=volume,
                    price=price
                )
            else:
                # If action was prevented, effectively treat it as a Hold for reward calculation
                action = 0 

        # Update the observation history
        self.observation_history.append(self._get_observation())
        
        # Calculate reward (additional reward from portfolio change)
        new_portfolio_value = self.gateway.evaluate_portfolio(self.agent_id)
        reward += (new_portfolio_value - self.portfolio_value)
        self.portfolio_value = new_portfolio_value

        # For now, we assume the episode doesn't terminate.
        # In a real scenario, you might have termination conditions
        # (e.g., portfolio value drops too low, or after a fixed time).
        terminated = False
        truncated = False
        info = {}

        obs = np.array(self.observation_history, dtype=np.float32)
        return obs, reward, terminated, truncated, info