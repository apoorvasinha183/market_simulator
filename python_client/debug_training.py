#!/usr/bin/env python3
"""
Debug script to understand why portfolio value isn't changing during training.
"""

import os
import sys
import torch
import numpy as np

# Add the parent directory to sys.path
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), '.')))

from market_gateway_client.rl_gateway import RLGateway
from rl_agent.environment import MarketEnv
from rl_agent.ppo_agent import PPOAgent

def debug_single_stock_training(stock_id: int = 1, debug_steps: int = 100):
    """Debug single stock training to see what's happening."""
    print(f"=== Debugging Single Stock Training (Stock {stock_id}) ===")
    
    # Connect to gateway
    host = os.environ.get("GRPC_HOST", "localhost")
    port = int(os.environ.get("GRPC_PORT", 50051))
    
    gateway = RLGateway(host=host, port=port)
    env = MarketEnv(gateway, target_stock_id=stock_id, history_length=30)
    
    agent = PPOAgent(
        input_dims=env.observation_space.shape[1],
        n_actions=env.action_space.n,
        learning_rate=3e-4
    )
    
    print(f"Environment created:")
    print(f"  Action space: {env.action_space.n} discrete actions")
    print(f"  Observation space: {env.observation_space.shape}")
    print(f"  Agent ID: {env.agent_id}")
    
    # Reset environment
    state, _ = env.reset()
    initial_portfolio = gateway.evaluate_portfolio(env.agent_id)
    initial_cash = gateway._agent_state[env.agent_id]['cash']
    initial_inventory = gateway._agent_state[env.agent_id]['inventory'][stock_id]
    
    print(f"\nInitial State:")
    print(f"  Portfolio Value: ${initial_portfolio:.2f}")
    print(f"  Cash: ${initial_cash:.2f}")
    print(f"  Stock {stock_id} Inventory: {initial_inventory}")
    
    # Get initial L1 data
    l1_data = gateway.get_l1_data(stock_id)
    if l1_data:
        print(f"  Stock {stock_id} Price: ${l1_data.last_traded_price:.2f}")
        print(f"  Bid: ${l1_data.best_bid_price:.2f} ({l1_data.best_bid_volume})")
        print(f"  Ask: ${l1_data.best_ask_price:.2f} ({l1_data.best_ask_volume})")
    else:
        print(f"  No L1 data for stock {stock_id}")
    
    print(f"\n=== Running {debug_steps} Debug Steps ===")
    
    action_counts = {}
    reward_history = []
    portfolio_history = []
    
    for step in range(debug_steps):
        # Get action from agent
        state_tensor = torch.from_numpy(state).float().unsqueeze(0).to(agent.device)
        action, prob, _, val = agent.policy.get_action(state_tensor)
        action = action.item()
        
        # Track action distribution
        action_counts[action] = action_counts.get(action, 0) + 1
        
        # Execute step
        next_state, reward, done, _, _ = env.step(action)
        
        # Get updated state
        new_portfolio = gateway.evaluate_portfolio(env.agent_id)
        new_cash = gateway._agent_state[env.agent_id]['cash']
        new_inventory = gateway._agent_state[env.agent_id]['inventory'][stock_id]
        
        reward_history.append(reward)
        portfolio_history.append(new_portfolio)
        
        # Print detailed info every 20 steps
        if (step + 1) % 20 == 0:
            print(f"\nStep {step + 1}:")
            print(f"  Action: {action} (prob: {prob:.3f})")
            print(f"  Reward: {reward:.2f}")
            print(f"  Portfolio: ${new_portfolio:.2f} (change: ${new_portfolio - initial_portfolio:.2f})")
            print(f"  Cash: ${new_cash:.2f} (change: ${new_cash - initial_cash:.2f})")
            print(f"  Inventory: {new_inventory} (change: {new_inventory - initial_inventory})")
            
            # Check L1 data changes
            current_l1 = gateway.get_l1_data(stock_id)
            if current_l1 and l1_data:
                price_change = current_l1.last_traded_price - l1_data.last_traded_price
                print(f"  Price: ${current_l1.last_traded_price:.2f} (change: ${price_change:.2f})")
        
        state = next_state
    
    print(f"\n=== Debug Summary ===")
    print(f"Action Distribution:")
    for action, count in sorted(action_counts.items()):
        action_name = get_action_name(action)
        percentage = (count / debug_steps) * 100
        print(f"  Action {action} ({action_name}): {count} times ({percentage:.1f}%)")
    
    print(f"\nReward Statistics:")
    print(f"  Total Reward: {sum(reward_history):.2f}")
    print(f"  Average Reward: {np.mean(reward_history):.2f}")
    print(f"  Reward Range: {min(reward_history):.2f} to {max(reward_history):.2f}")
    
    print(f"\nPortfolio Statistics:")
    portfolio_changes = np.diff(portfolio_history)
    print(f"  Initial Portfolio: ${portfolio_history[0]:.2f}")
    print(f"  Final Portfolio: ${portfolio_history[-1]:.2f}")
    print(f"  Total Change: ${portfolio_history[-1] - portfolio_history[0]:.2f}")
    print(f"  Number of Changes: {np.count_nonzero(portfolio_changes)}")
    
    if np.count_nonzero(portfolio_changes) == 0:
        print(f"\n❌ ISSUE IDENTIFIED: Portfolio never changed!")
        print(f"Possible causes:")
        print(f"  1. Orders not being executed (market conditions)")
        print(f"  2. All actions are 'Hold' (action 0)")
        print(f"  3. Invalid orders (insufficient cash, etc.)")
        print(f"  4. Market data not updating")
    else:
        print(f"\n✅ Portfolio is changing - training should work")
    
    gateway.shutdown()

def get_action_name(action: int) -> str:
    """Convert action number to human-readable name."""
    if action == 0:
        return "Hold"
    elif action == 1:
        return "Market Buy"
    elif action == 2:
        return "Market Sell"
    elif action >= 3 and action <= 5:
        return f"Limit Buy {action-2}"
    elif action >= 6 and action <= 8:
        return f"Limit Sell {action-5}"
    else:
        return "Unknown"

def analyze_reward_components():
    """Analyze what contributes to the reward signal."""
    print("\n=== Reward Component Analysis ===")
    
    print("In your environment, reward comes from:")
    print("1. Portfolio value changes: reward += (new_value - old_value)")
    print("2. Invalid action penalties: reward = -0.1 for invalid trades")
    print("3. Constraint violations: insufficient cash, short selling, etc.")
    print()
    print("If portfolio value isn't changing but rewards are:")
    print("• Agent is learning to avoid invalid actions (good!)")
    print("• Penalty rewards are decreasing (progress!)")
    print("• But no actual trading is happening (problem!)")

if __name__ == "__main__":
    print("Portfolio Training Debug Tool")
    print("=" * 50)
    
    try:
        debug_single_stock_training(stock_id=1, debug_steps=100)
        analyze_reward_components()
        
        print("\n" + "=" * 50)
        print("RECOMMENDATIONS:")
        print("1. Check if your market simulator is running and generating trades")
        print("2. Verify L1 data is updating with price movements")
        print("3. Ensure orders are being processed by the matching engine")
        print("4. Consider adding more aggressive exploration in early training")
        
    except Exception as e:
        print(f"Debug failed: {e}")
        print("Make sure your market simulator backend is running!")