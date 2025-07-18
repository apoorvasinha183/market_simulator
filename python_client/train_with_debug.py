#!/usr/bin/env python3
"""
Enhanced training script with detailed debugging and monitoring.
"""

import os
import sys
import torch
import numpy as np
import argparse
import csv
import time

# Add the parent directory to sys.path
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), '.')))

from market_gateway_client.rl_gateway import RLGateway
from rl_agent.environment import MarketEnv
from rl_agent.ppo_agent import PPOAgent

def train_with_detailed_monitoring(stock_id: int = 1, steps: int = 25000):
    """Train with detailed monitoring to understand what's happening."""
    print(f"=== Enhanced Training: Stock {stock_id} ===")
    
    # Configuration
    host = os.environ.get("GRPC_HOST", "localhost")
    port = int(os.environ.get("GRPC_PORT", 50051))
    
    model_save_path = "./debug_models"
    os.makedirs(model_save_path, exist_ok=True)
    
    # Initialize
    gateway = RLGateway(host=host, port=port)
    env = MarketEnv(gateway, target_stock_id=stock_id, history_length=30)
    
    agent = PPOAgent(
        input_dims=env.observation_space.shape[1],
        n_actions=env.action_space.n,
        learning_rate=3e-4
    )
    
    # Enhanced logging
    log_file = os.path.join(model_save_path, f"enhanced_training_stock_{stock_id}.csv")
    with open(log_file, 'w', newline='') as f:
        writer = csv.writer(f)
        writer.writerow([
            'step', 'action', 'reward', 'portfolio_value', 'portfolio_change',
            'cash', 'cash_change', 'inventory', 'inventory_change',
            'stock_price', 'price_change', 'timestamp'
        ])
    
    print(f"Starting enhanced training for {steps} steps...")
    print(f"Logging to: {log_file}")
    
    # Initialize tracking variables
    state, _ = env.reset()
    
    # Baseline values
    baseline_portfolio = gateway.evaluate_portfolio(env.agent_id)
    baseline_cash = gateway._agent_state[env.agent_id]['cash']
    baseline_inventory = gateway._agent_state[env.agent_id]['inventory'][stock_id]
    baseline_l1 = gateway.get_l1_data(stock_id)
    baseline_price = baseline_l1.last_traded_price if baseline_l1 else 0.0
    
    print(f"\nBaseline State:")
    print(f"  Portfolio: ${baseline_portfolio:.2f}")
    print(f"  Cash: ${baseline_cash:.2f}")
    print(f"  Inventory: {baseline_inventory}")
    print(f"  Stock Price: ${baseline_price:.2f}")
    
    # Training loop with enhanced monitoring
    total_reward = 0
    action_counts = {}
    successful_trades = 0
    
    try:
        for step in range(steps):
            # Get action
            state_tensor = torch.from_numpy(state).float().unsqueeze(0).to(agent.device)
            action, prob, _, val = agent.policy.get_action(state_tensor)
            
            action = action.item()
            prob = prob.item()
            val = val.item()
            
            # Track action distribution
            action_counts[action] = action_counts.get(action, 0) + 1
            
            # Execute step
            next_state, reward, done, _, _ = env.step(action)
            total_reward += reward
            
            # Get current state
            current_portfolio = gateway.evaluate_portfolio(env.agent_id)
            current_cash = gateway._agent_state[env.agent_id]['cash']
            current_inventory = gateway._agent_state[env.agent_id]['inventory'][stock_id]
            current_l1 = gateway.get_l1_data(stock_id)
            current_price = current_l1.last_traded_price if current_l1 else 0.0
            
            # Calculate changes
            portfolio_change = current_portfolio - baseline_portfolio
            cash_change = current_cash - baseline_cash
            inventory_change = current_inventory - baseline_inventory
            price_change = current_price - baseline_price
            
            # Track successful trades
            if abs(cash_change) > 1.0 or inventory_change != 0:
                successful_trades += 1
            
            # Store transition
            agent.store_transition(state, action, prob, val, reward, done)
            
            # Log detailed data every step (for first 1000 steps) or every 100 steps
            if step < 1000 or (step + 1) % 100 == 0:
                with open(log_file, 'a', newline='') as f:
                    writer = csv.writer(f)
                    writer.writerow([
                        step + 1, action, reward, current_portfolio, portfolio_change,
                        current_cash, cash_change, current_inventory, inventory_change,
                        current_price, price_change, time.time()
                    ])
            
            # Learn every 2048 steps
            if (step + 1) % 2048 == 0:
                agent.learn()
                
                # Detailed progress report
                print(f"\nStep {step + 1} - Learning Update:")
                print(f"  Cumulative Reward: {total_reward:.2f}")
                print(f"  Portfolio: ${current_portfolio:.2f} (Δ${portfolio_change:.2f})")
                print(f"  Cash: ${current_cash:.2f} (Δ${cash_change:.2f})")
                print(f"  Inventory: {current_inventory} (Δ{inventory_change})")
                print(f"  Stock Price: ${current_price:.2f} (Δ${price_change:.2f})")
                print(f"  Successful Trades: {successful_trades}")
                
                # Action distribution
                total_actions = sum(action_counts.values())
                print(f"  Action Distribution:")
                for act, count in sorted(action_counts.items()):
                    pct = (count / total_actions) * 100
                    action_name = get_action_name(act)
                    print(f"    {act} ({action_name}): {pct:.1f}%")
                
                # Reset counters
                total_reward = 0
                successful_trades = 0
                
                # Update baselines for next period
                baseline_portfolio = current_portfolio
                baseline_cash = current_cash
                baseline_inventory = current_inventory
                baseline_price = current_price
            
            state = next_state
            
    except KeyboardInterrupt:
        print("\nTraining interrupted by user")
    
    # Final summary
    print(f"\n=== Training Summary ===")
    final_portfolio = gateway.evaluate_portfolio(env.agent_id)
    final_cash = gateway._agent_state[env.agent_id]['cash']
    final_inventory = gateway._agent_state[env.agent_id]['inventory'][stock_id]
    
    print(f"Final State:")
    print(f"  Portfolio: ${final_portfolio:.2f}")
    print(f"  Cash: ${final_cash:.2f}")
    print(f"  Inventory: {final_inventory}")
    
    # Save model
    model_path = os.path.join(model_save_path, f"debug_model_stock_{stock_id}.pth")
    agent.save_model(model_path)
    print(f"Model saved to: {model_path}")
    
    gateway.shutdown()

def get_action_name(action: int) -> str:
    """Convert action number to human-readable name."""
    action_names = {
        0: "Hold",
        1: "Market Buy", 
        2: "Market Sell",
        3: "Limit Buy -1¢",
        4: "Limit Buy -2¢", 
        5: "Limit Buy -3¢",
        6: "Limit Sell +1¢",
        7: "Limit Sell +2¢",
        8: "Limit Sell +3¢"
    }
    return action_names.get(action, f"Unknown({action})")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Enhanced Training with Debug Info")
    parser.add_argument('--stock-id', type=int, default=1, help='Stock ID to train on')
    parser.add_argument('--steps', type=int, default=25000, help='Number of training steps')
    
    args = parser.parse_args()
    
    print("Enhanced Training with Detailed Monitoring")
    print("=" * 50)
    print("This will help us understand why portfolio values aren't changing")
    print()
    
    train_with_detailed_monitoring(stock_id=args.stock_id, steps=args.steps)