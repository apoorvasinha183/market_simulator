#!/usr/bin/env python3
"""
Train an agent that actually invests money instead of just holding cash.
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
from rl_agent.aggressive_portfolio_env import AggressivePortfolioEnv, ForceInvestmentEnv, SmartAggressiveEnv
from rl_agent.ppo_agent import PPOAgent

def train_aggressive_agent(env_type="aggressive", stock_id=1, steps=25000):
    """Train an agent that's forced to actually invest money."""
    
    print(f"=== Training {env_type.upper()} Agent (Stock {stock_id}) ===")
    print("No more sitting on cash like a coward! 💪")
    
    # Configuration
    host = os.environ.get("GRPC_HOST", "localhost")
    port = int(os.environ.get("GRPC_PORT", 50051))
    
    model_save_path = f"./aggressive_models_{env_type}"
    os.makedirs(model_save_path, exist_ok=True)
    
    # Initialize gateway and environment
    gateway = RLGateway(host=host, port=port)
    
    # Choose environment type
    if env_type == "aggressive":
        env = AggressivePortfolioEnv(gateway, target_stock_id=stock_id, history_length=30)
        print("Using AggressivePortfolioEnv - No hold action, cash penalties")
    elif env_type == "forced":
        env = ForceInvestmentEnv(gateway, target_stock_id=stock_id, min_investment_ratio=0.5, history_length=30)
        print("Using ForceInvestmentEnv - Must invest at least 50%")
    elif env_type == "smart":
        env = SmartAggressiveEnv(gateway, target_stock_id=stock_id, history_length=30)
        print("Using SmartAggressiveEnv - Balanced aggressive approach")
    else:
        raise ValueError(f"Unknown env_type: {env_type}")
    
    agent = PPOAgent(
        input_dims=env.observation_space.shape[1],
        n_actions=env.action_space.n,
        learning_rate=3e-4
    )
    
    print(f"Environment: {env.action_space.n} actions (NO HOLD ACTION!)")
    print(f"Agent will be FORCED to trade!")
    
    # Enhanced logging
    log_file = os.path.join(model_save_path, f"aggressive_training_stock_{stock_id}.csv")
    with open(log_file, 'w', newline='') as f:
        writer = csv.writer(f)
        writer.writerow([
            'step', 'action', 'reward', 'portfolio_value', 'cash', 'inventory',
            'cash_ratio', 'utilization_ratio', 'action_executed', 'steps_since_trade',
            'stock_price', 'timestamp'
        ])
    
    # Training loop
    state, _ = env.reset()
    total_reward = 0
    learn_iters = 0
    
    # Tracking variables
    action_counts = {}
    successful_trades = 0
    total_trades_attempted = 0
    
    print(f"\nStarting aggressive training for {steps} steps...")
    
    try:
        for step in range(steps):
            # Get action from agent
            state_tensor = torch.from_numpy(state).float().unsqueeze(0).to(agent.device)
            action, prob, _, val = agent.policy.get_action(state_tensor)
            
            action = action.item()
            prob = prob.item()
            val = val.item()
            
            # Track actions
            action_counts[action] = action_counts.get(action, 0) + 1
            total_trades_attempted += 1
            
            # Execute step
            next_state, reward, done, _, info = env.step(action)
            total_reward += reward
            
            # Track successful trades
            if info.get('action_executed', False):
                successful_trades += 1
            
            # Store transition
            agent.store_transition(state, action, prob, val, reward, done)
            
            # Get current state for logging
            current_portfolio = gateway.evaluate_portfolio(env.agent_id)
            current_cash = gateway._agent_state[env.agent_id]['cash']
            current_inventory = gateway._agent_state[env.agent_id]['inventory'].get(stock_id, 0)
            l1_data = gateway.get_l1_data(stock_id)
            current_price = l1_data.last_traded_price if l1_data else 0.0
            
            # Log every 100 steps
            if (step + 1) % 100 == 0:
                with open(log_file, 'a', newline='') as f:
                    writer = csv.writer(f)
                    writer.writerow([
                        step + 1, action, reward, current_portfolio, current_cash, current_inventory,
                        info.get('cash_ratio', 0), info.get('utilization_ratio', 0),
                        info.get('action_executed', False), info.get('steps_since_trade', 0),
                        current_price, time.time()
                    ])
            
            # Learn every 2048 steps
            if (step + 1) % 2048 == 0:
                agent.learn()
                learn_iters += 1
                
                # Calculate metrics
                trade_success_rate = (successful_trades / total_trades_attempted) * 100 if total_trades_attempted > 0 else 0
                cash_ratio = current_cash / current_portfolio if current_portfolio > 0 else 1.0
                invested_ratio = 1.0 - cash_ratio
                
                print(f"\nStep {step+1} - Learning Update #{learn_iters}:")
                print(f"  💰 Portfolio: ${current_portfolio:.2f}")
                print(f"  💵 Cash: ${current_cash:.2f} ({cash_ratio:.1%})")
                print(f"  📈 Invested: {invested_ratio:.1%}")
                print(f"  📊 Inventory: {current_inventory} shares")
                print(f"  🎯 Reward: {total_reward:.2f}")
                print(f"  ✅ Trade Success: {trade_success_rate:.1f}% ({successful_trades}/{total_trades_attempted})")
                
                # Action distribution
                total_actions = sum(action_counts.values())
                print(f"  🎲 Action Distribution:")
                for act in sorted(action_counts.keys()):
                    count = action_counts[act]
                    pct = (count / total_actions) * 100
                    action_name = get_action_name(act)
                    print(f"    {act} ({action_name}): {pct:.1f}%")
                
                # Environment-specific info
                if 'under_invested' in info and info['under_invested']:
                    print(f"  ⚠️  UNDER-INVESTED! Required: {info.get('required_investment', 0):.1%}")
                
                # Reset counters
                total_reward = 0
                successful_trades = 0
                total_trades_attempted = 0
                action_counts = {}
                
                # Save model periodically
                if learn_iters % 5 == 0:
                    model_path = os.path.join(model_save_path, f"aggressive_model_iter_{learn_iters}.pth")
                    agent.save_model(model_path)
                    print(f"  💾 Model saved: {model_path}")
            
            state = next_state
            
    except KeyboardInterrupt:
        print("\n🛑 Training interrupted by user")
    
    # Final summary
    print(f"\n=== Final Results ===")
    final_portfolio = gateway.evaluate_portfolio(env.agent_id)
    final_cash = gateway._agent_state[env.agent_id]['cash']
    final_inventory = gateway._agent_state[env.agent_id]['inventory'].get(stock_id, 0)
    final_cash_ratio = final_cash / final_portfolio if final_portfolio > 0 else 1.0
    
    print(f"💰 Final Portfolio: ${final_portfolio:.2f}")
    print(f"💵 Final Cash: ${final_cash:.2f} ({final_cash_ratio:.1%})")
    print(f"📊 Final Inventory: {final_inventory} shares")
    print(f"📈 Investment Ratio: {1.0 - final_cash_ratio:.1%}")
    
    if final_cash_ratio < 0.5:
        print("🎉 SUCCESS: Agent learned to invest money!")
    else:
        print("😞 Still too much cash - need more aggressive training")
    
    # Save final model
    final_model_path = os.path.join(model_save_path, f"aggressive_final_stock_{stock_id}.pth")
    agent.save_model(final_model_path)
    print(f"💾 Final model saved: {final_model_path}")
    
    gateway.shutdown()

def get_action_name(action: int) -> str:
    """Convert action number to human-readable name."""
    action_names = {
        0: "Market Buy",
        1: "Market Sell", 
        2: "Limit Buy -1¢",
        3: "Limit Buy -2¢", 
        4: "Limit Buy -3¢",
        5: "Limit Sell +1¢",
        6: "Limit Sell +2¢",
        7: "Limit Sell +3¢"
    }
    return action_names.get(action, f"Unknown({action})")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Train Aggressive Trading Agent")
    parser.add_argument('--env-type', type=str, choices=['aggressive', 'forced', 'smart'], 
                        default='aggressive', help='Type of aggressive environment')
    parser.add_argument('--stock-id', type=int, default=1, help='Stock ID to trade')
    parser.add_argument('--steps', type=int, default=25000, help='Training steps')
    
    args = parser.parse_args()
    
    print("🚀 AGGRESSIVE TRADING AGENT TRAINING")
    print("=" * 50)
    print("Finally! An agent that will actually invest money!")
    print(f"Environment: {args.env_type}")
    print(f"Stock: {args.stock_id}")
    print(f"Steps: {args.steps:,}")
    print()
    
    train_aggressive_agent(
        env_type=args.env_type,
        stock_id=args.stock_id, 
        steps=args.steps
    )