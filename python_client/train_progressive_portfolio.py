import os
import sys
import torch
import numpy as np
import argparse
import csv
import time
import gymnasium as gym
from typing import Dict, Any, List

# Add the parent directory to sys.path
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), '.')))

from market_gateway_client.rl_gateway import RLGateway
from rl_agent.environment import MarketEnv
from rl_agent.portfolio_env import SimplifiedPortfolioEnv
from rl_agent.ppo_agent import PPOAgent

class ProgressivePortfolioTrainer:
    """
    Progressive training approach that addresses the curse of dimensionality:
    1. Phase 1: Single stock mastery (your existing approach)
    2. Phase 2: Top-K portfolio (5 stocks)
    3. Phase 3: Sector-based portfolio (4 sectors)
    """
    
    def __init__(self, host="localhost", port=50051):
        self.host = host
        self.port = port
        self.gateway = None
        self.model_save_path = "./progressive_portfolio_models"
        os.makedirs(self.model_save_path, exist_ok=True)
        
    def phase1_single_stock_training(self, stock_id: int = 1, steps: int = 50000):
        """Phase 1: Master single stock trading first."""
        print(f"=== Phase 1: Single Stock Training (Stock {stock_id}) ===")
        
        self.gateway = RLGateway(host=self.host, port=self.port)
        env = MarketEnv(self.gateway, target_stock_id=stock_id, history_length=30)
        
        agent = PPOAgent(
            input_dims=env.observation_space.shape[1],
            n_actions=env.action_space.n,
            learning_rate=3e-4
        )
        
        # Setup logging
        phase1_log = os.path.join(self.model_save_path, "phase1_single_stock.csv")
        with open(phase1_log, 'w', newline='') as f:
            writer = csv.writer(f)
            writer.writerow(['step', 'reward', 'portfolio_value', 'timestamp'])
        
        print(f"Training on stock {stock_id} for {steps} steps...")
        state, _ = env.reset()
        total_reward = 0
        
        try:
            for step in range(steps):
                state_tensor = torch.from_numpy(state).float().unsqueeze(0).to(agent.device)
                action, prob, _, val = agent.policy.get_action(state_tensor)
                
                action = action.item()
                prob = prob.item()
                val = val.item()
                
                next_state, reward, done, _, _ = env.step(action)
                total_reward += reward
                
                agent.store_transition(state, action, prob, val, reward, done)
                
                # Learn every 2048 steps
                if (step + 1) % 2048 == 0:
                    agent.learn()
                    portfolio_value = self.gateway.evaluate_portfolio(env.agent_id)
                    
                    print(f"Step {step+1}: Reward={total_reward:.2f}, Portfolio=${portfolio_value:.2f}")
                    
                    # Log progress
                    with open(phase1_log, 'a', newline='') as f:
                        writer = csv.writer(f)
                        writer.writerow([step+1, total_reward, portfolio_value, time.time()])
                    
                    total_reward = 0
                
                state = next_state
                
        except KeyboardInterrupt:
            print("Phase 1 training interrupted")
        
        # Save model
        phase1_model_path = os.path.join(self.model_save_path, "phase1_single_stock.pth")
        agent.save_model(phase1_model_path)
        print(f"Phase 1 model saved to {phase1_model_path}")
        
        self.gateway.shutdown()
        return agent
    
    def phase2_top_k_portfolio(self, k: int = 5, steps: int = 75000, pretrained_agent=None):
        """Phase 2: Top-K stock portfolio management."""
        print(f"=== Phase 2: Top-{k} Portfolio Training ===")
        
        self.gateway = RLGateway(host=self.host, port=self.port)
        env = SimplifiedPortfolioEnv(self.gateway, num_stocks=20, top_k=k, history_length=30)
        
        # Create agent for continuous action space
        from rl_agent.continuous_ppo_agent import ContinuousPPOAgent
        agent = ContinuousPPOAgent(
            input_dims=env.observation_space.shape[1],
            action_dims=env.action_space.shape[0],
            learning_rate=3e-4
        )
        
        # Setup logging
        phase2_log = os.path.join(self.model_save_path, f"phase2_top_{k}_portfolio.csv")
        with open(phase2_log, 'w', newline='') as f:
            writer = csv.writer(f)
            writer.writerow(['step', 'reward', 'portfolio_value', 'diversification', 'timestamp'])
        
        print(f"Training top-{k} portfolio for {steps} steps...")
        state, _ = env.reset()
        total_reward = 0
        
        try:
            for step in range(steps):
                state_tensor = torch.from_numpy(state).float().unsqueeze(0).to(agent.device)
                action, log_prob, val = agent.get_action(state_tensor)
                
                action_np = action.cpu().numpy().flatten()
                log_prob = log_prob.item()
                val = val.item()
                
                next_state, reward, done, _, _ = env.step(action_np)
                total_reward += reward
                
                agent.store_transition(state, action_np, log_prob, val, reward, done)
                
                # Learn every 2048 steps
                if (step + 1) % 2048 == 0:
                    agent.learn()
                    portfolio_value = self.gateway.evaluate_portfolio(env.agent_id)
                    
                    # Calculate diversification metric
                    agent_state = self.gateway._agent_state[env.agent_id]
                    active_positions = sum(1 for pos in agent_state['inventory'].values() if pos > 0)
                    diversification = active_positions / k
                    
                    print(f"Step {step+1}: Reward={total_reward:.2f}, Portfolio=${portfolio_value:.2f}, Div={diversification:.2f}")
                    
                    # Log progress
                    with open(phase2_log, 'a', newline='') as f:
                        writer = csv.writer(f)
                        writer.writerow([step+1, total_reward, portfolio_value, diversification, time.time()])
                    
                    total_reward = 0
                
                state = next_state
                
        except KeyboardInterrupt:
            print("Phase 2 training interrupted")
        
        # Save model
        phase2_model_path = os.path.join(self.model_save_path, f"phase2_top_{k}_portfolio.pth")
        agent.save_model(phase2_model_path)
        print(f"Phase 2 model saved to {phase2_model_path}")
        
        self.gateway.shutdown()
        return agent
    
    def phase3_sector_rotation(self, num_sectors: int = 4, steps: int = 100000):
        """Phase 3: Sector-based portfolio management."""
        print(f"=== Phase 3: Sector Rotation Training ({num_sectors} sectors) ===")
        
        self.gateway = RLGateway(host=self.host, port=self.port)
        env = SectorRotationEnv(self.gateway, num_stocks=20, num_sectors=num_sectors)
        
        from rl_agent.continuous_ppo_agent import ContinuousPPOAgent
        agent = ContinuousPPOAgent(
            input_dims=env.observation_space.shape[1],
            action_dims=env.action_space.shape[0],
            learning_rate=3e-4
        )
        
        # Setup logging
        phase3_log = os.path.join(self.model_save_path, f"phase3_sector_rotation.csv")
        with open(phase3_log, 'w', newline='') as f:
            writer = csv.writer(f)
            writer.writerow(['step', 'reward', 'portfolio_value', 'sector_diversity', 'timestamp'])
        
        print(f"Training sector rotation for {steps} steps...")
        state, _ = env.reset()
        total_reward = 0
        
        try:
            for step in range(steps):
                state_tensor = torch.from_numpy(state).float().unsqueeze(0).to(agent.device)
                action, log_prob, val = agent.get_action(state_tensor)
                
                action_np = action.cpu().numpy().flatten()
                log_prob = log_prob.item()
                val = val.item()
                
                next_state, reward, done, _, _ = env.step(action_np)
                total_reward += reward
                
                agent.store_transition(state, action_np, log_prob, val, reward, done)
                
                # Learn every 2048 steps
                if (step + 1) % 2048 == 0:
                    agent.learn()
                    portfolio_value = self.gateway.evaluate_portfolio(env.agent_id)
                    
                    # Calculate sector diversity (how evenly distributed across sectors)
                    sector_diversity = self._calculate_sector_diversity(action_np)
                    
                    print(f"Step {step+1}: Reward={total_reward:.2f}, Portfolio=${portfolio_value:.2f}, SectorDiv={sector_diversity:.2f}")
                    
                    # Log progress
                    with open(phase3_log, 'a', newline='') as f:
                        writer = csv.writer(f)
                        writer.writerow([step+1, total_reward, portfolio_value, sector_diversity, time.time()])
                    
                    total_reward = 0
                
                state = next_state
                
        except KeyboardInterrupt:
            print("Phase 3 training interrupted")
        
        # Save model
        phase3_model_path = os.path.join(self.model_save_path, "phase3_sector_rotation.pth")
        agent.save_model(phase3_model_path)
        print(f"Phase 3 model saved to {phase3_model_path}")
        
        self.gateway.shutdown()
        return agent
    
    def _calculate_sector_diversity(self, sector_weights: np.ndarray) -> float:
        """Calculate how evenly distributed the sector weights are (entropy-based)."""
        # Normalize weights
        weights = sector_weights / np.sum(sector_weights) if np.sum(sector_weights) > 0 else sector_weights
        
        # Calculate entropy (higher = more diverse)
        entropy = -np.sum(weights * np.log(weights + 1e-8))
        max_entropy = np.log(len(weights))  # Maximum possible entropy
        
        return entropy / max_entropy if max_entropy > 0 else 0.0


class SectorRotationEnv(SimplifiedPortfolioEnv):
    """
    Sector rotation environment that groups stocks into sectors
    and makes allocation decisions at the sector level.
    """
    
    def __init__(self, gateway: RLGateway, num_stocks=20, num_sectors=4, history_length=30):
        self.num_sectors = num_sectors
        
        # Initialize parent with reduced complexity
        super().__init__(gateway, num_stocks, top_k=num_sectors, history_length=history_length)
        
        # Define sectors (simplified grouping)
        self.sectors = self._create_sectors()
        
        # Action space: allocation weights for sectors
        self.action_space = gym.spaces.Box(
            low=0.0, high=1.0,
            shape=(self.num_sectors,),
            dtype=np.float32
        )
        
        # Observation space: sector-level features + portfolio metrics
        self.num_features_per_step = (self.num_sectors * 6) + 3  # 6 features per sector + 3 portfolio metrics
        self.observation_space = gym.spaces.Box(
            low=-np.inf, high=np.inf,
            shape=(self.history_length, self.num_features_per_step),
            dtype=np.float32
        )
    
    def _create_sectors(self) -> Dict[str, List[int]]:
        """Create sector groupings."""
        stocks_per_sector = self.num_stocks // self.num_sectors
        sectors = {}
        
        sector_names = ['Tech', 'Finance', 'Healthcare', 'Energy']
        
        for i in range(self.num_sectors):
            start_idx = i * stocks_per_sector
            end_idx = start_idx + stocks_per_sector
            if i == self.num_sectors - 1:  # Last sector gets remaining stocks
                end_idx = self.num_stocks
            
            sector_name = sector_names[i] if i < len(sector_names) else f"Sector_{i}"
            sectors[sector_name] = list(range(start_idx + 1, end_idx + 1))  # Stock IDs start from 1
            
        return sectors
    
    def _get_observation(self) -> np.array:
        """Get sector-level observation."""
        obs_vector = []
        
        # Calculate features for each sector
        for sector_name, stock_list in self.sectors.items():
            sector_price = 0.0
            sector_volume = 0.0
            sector_spread = 0.0
            sector_momentum = 0.0
            sector_position = 0.0
            valid_stocks = 0
            
            for stock_id in stock_list:
                l1_data = self.gateway.get_l1_data(stock_id)
                if l1_data:
                    sector_price += l1_data.last_traded_price
                    sector_volume += l1_data.best_bid_volume + l1_data.best_ask_volume
                    if l1_data.last_traded_price > 0:
                        sector_spread += (l1_data.best_ask_price - l1_data.best_bid_price) / l1_data.last_traded_price
                    
                    # Simple momentum (could be improved with actual price history)
                    sector_momentum += l1_data.last_traded_price - 100.0  # Relative to reference price
                    
                    # Current position in this stock
                    sector_position += self.gateway._agent_state[self.agent_id]['inventory'].get(stock_id, 0)
                    valid_stocks += 1
            
            # Average sector metrics
            if valid_stocks > 0:
                obs_vector.extend([
                    sector_price / valid_stocks / 100.0,  # Normalized average price
                    sector_volume / valid_stocks / 1000.0,  # Normalized average volume
                    sector_spread / valid_stocks,  # Average spread
                    sector_momentum / valid_stocks / 100.0,  # Normalized momentum
                    sector_position / 1000.0,  # Normalized total position
                    valid_stocks / len(stock_list),  # Data availability ratio
                ])
            else:
                obs_vector.extend([1.0, 0.0, 0.0, 0.0, 0.0, 0.0])  # Default values
        
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
    
    def step(self, action):
        """Execute sector rotation strategy."""
        # Normalize sector weights to sum to 1
        sector_weights = action / np.sum(action) if np.sum(action) > 0 else np.ones_like(action) / len(action)
        
        portfolio_value = self.gateway.evaluate_portfolio(self.agent_id)
        
        # Allocate to best stock in each sector
        for i, (sector_name, stock_list) in enumerate(self.sectors.items()):
            sector_allocation = portfolio_value * sector_weights[i]
            
            # Find best performing stock in sector (simple heuristic)
            best_stock = self._select_best_stock_in_sector(stock_list)
            
            if best_stock and sector_allocation > 1000:  # Minimum allocation threshold
                l1_data = self.gateway.get_l1_data(best_stock)
                if l1_data and l1_data.last_traded_price > 0:
                    target_shares = int(sector_allocation / l1_data.last_traded_price)
                    current_shares = self.gateway._agent_state[self.agent_id]['inventory'].get(best_stock, 0)
                    
                    share_diff = target_shares - current_shares
                    
                    if abs(share_diff) > 10:  # Only trade if significant difference
                        side = "Buy" if share_diff > 0 else "Sell"
                        volume = abs(share_diff)
                        
                        # Check if we can execute the trade
                        if self._can_execute_trade(best_stock, side, volume):
                            self.gateway.submit_order(
                                agent_id=self.agent_id,
                                stock_id=best_stock,
                                side=side,
                                order_type="Market",
                                volume=volume,
                                price=0.0
                            )
        
        # Update observation and calculate reward
        self.observation_history.append(self._get_observation())
        new_portfolio_value = self.gateway.evaluate_portfolio(self.agent_id)
        reward = new_portfolio_value - portfolio_value
        
        obs = np.array(self.observation_history, dtype=np.float32)
        return obs, reward, False, False, {}
    
    def _select_best_stock_in_sector(self, stock_list: List[int]) -> int:
        """Select the best performing stock in a sector."""
        best_stock = None
        best_score = -float('inf')
        
        for stock_id in stock_list:
            l1_data = self.gateway.get_l1_data(stock_id)
            if l1_data:
                # Simple scoring: combine price momentum and volume
                volume_score = l1_data.best_bid_volume + l1_data.best_ask_volume
                price_momentum = l1_data.last_traded_price - 100.0  # Relative to reference
                score = volume_score * 0.3 + price_momentum * 0.7
                
                if score > best_score:
                    best_score = score
                    best_stock = stock_id
        
        return best_stock or stock_list[0]  # Fallback to first stock
    
    def _can_execute_trade(self, stock_id: int, side: str, volume: int) -> bool:
        """Check if trade can be executed given constraints."""
        current_cash = self.gateway._agent_state[self.agent_id]['cash']
        current_inventory = self.gateway._agent_state[self.agent_id]['inventory'].get(stock_id, 0)
        
        if side == "Buy":
            l1_data = self.gateway.get_l1_data(stock_id)
            estimated_cost = volume * (l1_data.best_ask_price if l1_data else 100.0)
            return current_cash >= estimated_cost
        else:  # Sell
            return current_inventory >= volume


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Progressive Portfolio Management Training")
    parser.add_argument('--phase', type=int, choices=[1, 2, 3], default=1,
                        help='Training phase: 1=Single Stock, 2=Top-K Portfolio, 3=Sector Rotation')
    parser.add_argument('--stock-id', type=int, default=1,
                        help='Stock ID for Phase 1 training')
    parser.add_argument('--top-k', type=int, default=5,
                        help='Number of top stocks for Phase 2')
    parser.add_argument('--sectors', type=int, default=4,
                        help='Number of sectors for Phase 3')
    parser.add_argument('--steps', type=int, default=50000,
                        help='Number of training steps')
    
    args = parser.parse_args()
    
    trainer = ProgressivePortfolioTrainer()
    
    print(f"Starting Progressive Portfolio Training - Phase {args.phase}")
    print("=" * 60)
    
    if args.phase == 1:
        trainer.phase1_single_stock_training(stock_id=args.stock_id, steps=args.steps)
    elif args.phase == 2:
        trainer.phase2_top_k_portfolio(k=args.top_k, steps=args.steps)
    elif args.phase == 3:
        trainer.phase3_sector_rotation(num_sectors=args.sectors, steps=args.steps)
    
    print("Training completed!")