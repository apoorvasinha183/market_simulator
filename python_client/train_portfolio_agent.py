import os
import sys
import torch
import numpy as np
import argparse
import csv
import time
from stable_baselines3 import PPO, SAC
from stable_baselines3.common.env_util import make_vec_env
from stable_baselines3.common.callbacks import BaseCallback

# Add the parent directory to sys.path
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), '.')))

from market_gateway_client.rl_gateway import RLGateway
from rl_agent.portfolio_env import PortfolioEnv, SimplifiedPortfolioEnv

class PortfolioCallback(BaseCallback):
    """Custom callback for portfolio training metrics."""
    
    def __init__(self, log_dir: str, verbose=0):
        super().__init__(verbose)
        self.log_dir = log_dir
        self.episode_rewards = []
        self.portfolio_values = []
        
    def _on_step(self) -> bool:
        # Log portfolio metrics
        if 'episode' in self.locals:
            episode_reward = self.locals.get('episode_reward', 0)
            if episode_reward != 0:
                self.episode_rewards.append(episode_reward)
                
                # Log to CSV
                with open(os.path.join(self.log_dir, 'portfolio_metrics.csv'), 'a', newline='') as f:
                    writer = csv.writer(f)
                    writer.writerow([len(self.episode_rewards), episode_reward, time.time()])
        
        return True

def train_hierarchical_portfolio_agent():
    """Train agent with hierarchical action space for portfolio management."""
    print("=== Training Hierarchical Portfolio Agent ===")
    
    # Configuration
    host = os.environ.get("GRPC_HOST", "localhost")
    port = int(os.environ.get("GRPC_PORT", 50051))
    
    model_save_path = "./portfolio_models/hierarchical"
    os.makedirs(model_save_path, exist_ok=True)
    
    # Initialize environment
    gateway = RLGateway(host=host, port=port)
    env = PortfolioEnv(gateway, num_stocks=10, num_sectors=3)  # Reduced complexity
    
    # Note: Hierarchical actions require custom algorithm
    # For now, we'll use a simplified approach
    print("Hierarchical portfolio management requires custom implementation")
    print("This would involve:")
    print("1. Sector allocation network")
    print("2. Stock selection network within sectors")
    print("3. Coordinated training between both networks")
    
    gateway.shutdown()

def train_simplified_portfolio_agent():
    """Train agent with simplified top-K stock selection."""
    print("=== Training Simplified Portfolio Agent ===")
    
    # Configuration
    host = os.environ.get("GRPC_HOST", "localhost")
    port = int(os.environ.get("GRPC_PORT", 50051))
    
    model_save_path = "./portfolio_models/simplified"
    os.makedirs(model_save_path, exist_ok=True)
    
    # Initialize environment
    gateway = RLGateway(host=host, port=port)
    env = SimplifiedPortfolioEnv(gateway, num_stocks=20, top_k=5)
    
    # Use SAC for continuous action space
    model = SAC(
        "MlpPolicy",
        env,
        verbose=1,
        learning_rate=3e-4,
        buffer_size=50000,
        learning_starts=1000,
        batch_size=256,
        tau=0.005,
        gamma=0.99,
        train_freq=1,
        gradient_steps=1,
        tensorboard_log=f"{model_save_path}/tensorboard/"
    )
    
    # Setup callback
    callback = PortfolioCallback(model_save_path)
    
    try:
        print("Starting training...")
        model.learn(
            total_timesteps=100000,
            callback=callback,
            progress_bar=True
        )
        
        # Save final model
        model.save(f"{model_save_path}/sac_portfolio_final")
        print(f"Model saved to {model_save_path}/sac_portfolio_final")
        
    except KeyboardInterrupt:
        print("Training interrupted by user")
    finally:
        gateway.shutdown()

def train_sector_rotation_agent():
    """Train agent for sector rotation strategy."""
    print("=== Training Sector Rotation Agent ===")
    
    # This approach reduces dimensionality by:
    # 1. Grouping stocks into sectors
    # 2. Making sector-level allocation decisions
    # 3. Using momentum/mean reversion within sectors
    
    class SectorRotationEnv(SimplifiedPortfolioEnv):
        def __init__(self, *args, **kwargs):
            super().__init__(*args, **kwargs)
            
            # Define sectors (simplified)
            self.sectors = {
                'tech': [1, 2, 3, 4, 5],
                'finance': [6, 7, 8, 9, 10],
                'healthcare': [11, 12, 13, 14, 15],
                'energy': [16, 17, 18, 19, 20]
            }
            
            # Action space: allocation weights for sectors
            self.action_space = gym.spaces.Box(
                low=0.0, high=1.0,
                shape=(len(self.sectors),),
                dtype=np.float32
            )
        
        def _get_sector_performance(self, sector_stocks):
            """Calculate sector performance metrics."""
            total_return = 0.0
            total_volume = 0.0
            
            for stock_id in sector_stocks:
                l1_data = self.gateway.get_l1_data(stock_id)
                if l1_data:
                    # Simple momentum calculation
                    total_return += l1_data.last_traded_price
                    total_volume += l1_data.best_bid_volume + l1_data.best_ask_volume
            
            return total_return / len(sector_stocks), total_volume / len(sector_stocks)
        
        def step(self, action):
            """Execute sector rotation strategy."""
            # Normalize sector weights
            sector_weights = action / np.sum(action)
            
            portfolio_value = self.gateway.evaluate_portfolio(self.agent_id)
            
            # Allocate to best stock in each sector
            for i, (sector_name, sector_stocks) in enumerate(self.sectors.items()):
                sector_allocation = portfolio_value * sector_weights[i]
                
                # Find best performing stock in sector
                best_stock = None
                best_performance = -float('inf')
                
                for stock_id in sector_stocks:
                    l1_data = self.gateway.get_l1_data(stock_id)
                    if l1_data:
                        # Simple performance metric
                        performance = l1_data.last_traded_price * (l1_data.best_bid_volume + l1_data.best_ask_volume)
                        if performance > best_performance:
                            best_performance = performance
                            best_stock = stock_id
                
                # Execute trade for best stock in sector
                if best_stock and sector_allocation > 1000:  # Minimum allocation threshold
                    l1_data = self.gateway.get_l1_data(best_stock)
                    if l1_data and l1_data.last_traded_price > 0:
                        target_shares = int(sector_allocation / l1_data.last_traded_price)
                        current_shares = self.gateway._agent_state[self.agent_id]['inventory'][best_stock]
                        
                        share_diff = target_shares - current_shares
                        
                        if abs(share_diff) > 10:
                            side = "Buy" if share_diff > 0 else "Sell"
                            volume = abs(share_diff)
                            
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
    
    # Configuration
    host = os.environ.get("GRPC_HOST", "localhost")
    port = int(os.environ.get("GRPC_PORT", 50051))
    
    model_save_path = "./portfolio_models/sector_rotation"
    os.makedirs(model_save_path, exist_ok=True)
    
    # Initialize environment
    gateway = RLGateway(host=host, port=port)
    env = SectorRotationEnv(gateway, num_stocks=20, top_k=4)  # 4 sectors
    
    # Use PPO for this approach
    model = PPO(
        "MlpPolicy",
        env,
        verbose=1,
        learning_rate=3e-4,
        n_steps=2048,
        batch_size=64,
        n_epochs=10,
        gamma=0.99,
        gae_lambda=0.95,
        clip_range=0.2,
        tensorboard_log=f"{model_save_path}/tensorboard/"
    )
    
    try:
        print("Starting sector rotation training...")
        model.learn(total_timesteps=50000, progress_bar=True)
        
        model.save(f"{model_save_path}/ppo_sector_rotation")
        print(f"Model saved to {model_save_path}/ppo_sector_rotation")
        
    except KeyboardInterrupt:
        print("Training interrupted by user")
    finally:
        gateway.shutdown()

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Train Portfolio Management RL Agents")
    parser.add_argument('--strategy', type=str, choices=['hierarchical', 'simplified', 'sector_rotation'], 
                        default='simplified', help='Portfolio management strategy to train')
    
    args = parser.parse_args()
    
    print(f"Training strategy: {args.strategy}")
    print("=" * 50)
    
    if args.strategy == 'hierarchical':
        train_hierarchical_portfolio_agent()
    elif args.strategy == 'simplified':
        train_simplified_portfolio_agent()
    elif args.strategy == 'sector_rotation':
        train_sector_rotation_agent()
    
    print("Training completed!")