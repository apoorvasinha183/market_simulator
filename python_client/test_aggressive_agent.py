#!/usr/bin/env python3
"""
Quick test script to see if the aggressive agent actually invests money.
"""

import os
import sys
import torch
import numpy as np

# Add the parent directory to sys.path
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), '.')))

from market_gateway_client.rl_gateway import RLGateway
from rl_agent.aggressive_portfolio_env import AggressivePortfolioEnv
from rl_agent.ppo_agent import PPOAgent

def quick_test_aggressive_agent(stock_id=1, test_steps=200):
    """Quick test to see if agent invests money."""
    
    print(f"🧪 QUICK TEST: Aggressive Agent (Stock {stock_id})")
    print("=" * 50)
    
    # Connect to gateway
    host = os.environ.get("GRPC_HOST", "localhost")
    port = int(os.environ.get("GRPC_PORT", 50051))
    
    try:
        gateway = RLGateway(host=host, port=port)
        env = AggressivePortfolioEnv(gateway, target_stock_id=stock_id, history_length=10)
        
        agent = PPOAgent(
            input_dims=env.observation_space.shape[1],
            n_actions=env.action_space.n,
            learning_rate=3e-4
        )
        
        print(f"✅ Connected to market simulator")
        print(f"📊 Action space: {env.action_space.n} actions")
        print(f"🎯 Environment: AggressivePortfolioEnv")
        
        # Reset environment
        state, _ = env.reset()
        
        # Get initial state
        initial_portfolio = gateway.evaluate_portfolio(env.agent_id)
        initial_cash = gateway._agent_state[env.agent_id]['cash']
        initial_inventory = gateway._agent_state[env.agent_id]['inventory'].get(stock_id, 0)
        
        print(f"\n📈 INITIAL STATE:")
        print(f"  Portfolio: ${initial_portfolio:.2f}")
        print(f"  Cash: ${initial_cash:.2f} ({initial_cash/initial_portfolio:.1%})")
        print(f"  Inventory: {initial_inventory} shares")
        
        # Get stock price
        l1_data = gateway.get_l1_data(stock_id)
        if l1_data:
            print(f"  Stock Price: ${l1_data.last_traded_price:.2f}")
        
        print(f"\n🚀 RUNNING {test_steps} TEST STEPS...")
        
        # Track metrics
        action_counts = {}
        rewards = []
        cash_ratios = []
        trades_executed = 0
        
        for step in range(test_steps):
            # Get action
            state_tensor = torch.from_numpy(state).float().unsqueeze(0).to(agent.device)
            action, prob, _, val = agent.policy.get_action(state_tensor)
            action = action.item()
            
            # Track actions
            action_counts[action] = action_counts.get(action, 0) + 1
            
            # Execute step
            next_state, reward, done, _, info = env.step(action)
            rewards.append(reward)
            
            # Track metrics
            current_cash = gateway._agent_state[env.agent_id]['cash']
            current_portfolio = gateway.evaluate_portfolio(env.agent_id)
            cash_ratio = current_cash / current_portfolio if current_portfolio > 0 else 1.0
            cash_ratios.append(cash_ratio)
            
            if info.get('action_executed', False):
                trades_executed += 1
            
            # Print progress every 50 steps
            if (step + 1) % 50 == 0:
                current_inventory = gateway._agent_state[env.agent_id]['inventory'].get(stock_id, 0)
                print(f"  Step {step+1}: Portfolio=${current_portfolio:.2f}, Cash={cash_ratio:.1%}, Inventory={current_inventory}, Reward={reward:.3f}")
            
            state = next_state
        
        # Final results
        final_portfolio = gateway.evaluate_portfolio(env.agent_id)
        final_cash = gateway._agent_state[env.agent_id]['cash']
        final_inventory = gateway._agent_state[env.agent_id]['inventory'].get(stock_id, 0)
        final_cash_ratio = final_cash / final_portfolio if final_portfolio > 0 else 1.0
        
        print(f"\n📊 FINAL RESULTS:")
        print(f"  Portfolio: ${final_portfolio:.2f} (change: ${final_portfolio - initial_portfolio:.2f})")
        print(f"  Cash: ${final_cash:.2f} ({final_cash_ratio:.1%})")
        print(f"  Inventory: {final_inventory} shares (change: {final_inventory - initial_inventory})")
        print(f"  Trades Executed: {trades_executed}/{test_steps} ({trades_executed/test_steps:.1%})")
        
        print(f"\n🎲 ACTION DISTRIBUTION:")
        total_actions = sum(action_counts.values())
        for action in sorted(action_counts.keys()):
            count = action_counts[action]
            pct = (count / total_actions) * 100
            action_name = get_action_name(action)
            print(f"  {action} ({action_name}): {count} times ({pct:.1f}%)")
        
        print(f"\n💰 REWARD ANALYSIS:")
        print(f"  Total Reward: {sum(rewards):.2f}")
        print(f"  Average Reward: {np.mean(rewards):.3f}")
        print(f"  Best Reward: {max(rewards):.3f}")
        print(f"  Worst Reward: {min(rewards):.3f}")
        
        print(f"\n💵 CASH ANALYSIS:")
        print(f"  Average Cash Ratio: {np.mean(cash_ratios):.1%}")
        print(f"  Final Cash Ratio: {final_cash_ratio:.1%}")
        
        # Verdict
        print(f"\n🏆 VERDICT:")
        if final_cash_ratio < 0.8:
            print(f"  ✅ SUCCESS! Agent is investing money ({1-final_cash_ratio:.1%} invested)")
        else:
            print(f"  ❌ FAIL! Agent is still hoarding cash ({final_cash_ratio:.1%} in cash)")
        
        if trades_executed > test_steps * 0.1:
            print(f"  ✅ Agent is actively trading ({trades_executed} trades)")
        else:
            print(f"  ❌ Agent is not trading enough ({trades_executed} trades)")
        
        if final_inventory != initial_inventory:
            print(f"  ✅ Inventory changed - agent is taking positions")
        else:
            print(f"  ❌ No inventory change - agent not taking positions")
        
        gateway.shutdown()
        
    except Exception as e:
        print(f"❌ Test failed: {e}")
        print("Make sure your market simulator is running!")

def get_action_name(action: int) -> str:
    """Convert action number to human-readable name."""
    if action == 0:
        return "Hold"
    elif action == 1:
        return "Market Buy"
    elif action == 2:
        return "Market Sell"
    elif action >= 3 and action <= 5:
        return f"Limit Buy -{action-2}¢"
    elif action >= 6 and action <= 8:
        return f"Limit Sell +{action-5}¢"
    else:
        return f"Unknown({action})"

if __name__ == "__main__":
    print("🧪 AGGRESSIVE AGENT QUICK TEST")
    print("This will test if the agent actually invests money")
    print()
    
    quick_test_aggressive_agent(stock_id=1, test_steps=200)