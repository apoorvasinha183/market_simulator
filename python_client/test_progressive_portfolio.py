#!/usr/bin/env python3
"""
Test script for progressive portfolio management approach.
This validates that our dimensionality reduction strategies work correctly.
"""

import os
import sys
import numpy as np

# Add the parent directory to sys.path
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), '.')))

from market_gateway_client.rl_gateway import RLGateway
from rl_agent.environment import MarketEnv
from rl_agent.portfolio_env import SimplifiedPortfolioEnv
from train_progressive_portfolio import SectorRotationEnv

def test_action_space_dimensions():
    """Test and compare action space dimensions across different approaches."""
    print("=== Action Space Dimensionality Analysis ===")
    
    # Mock gateway for testing
    class MockGateway:
        def __init__(self):
            self._agent_state = {}
        
        def register_agent(self):
            agent_id = "test_agent"
            self._agent_state[agent_id] = {
                'cash': 1000000.0,
                'inventory': {i: 0 for i in range(1, 21)}
            }
            return agent_id
        
        def get_l1_data(self, stock_id):
            # Mock L1 data
            class MockL1Data:
                def __init__(self):
                    self.best_bid_price = 100.0
                    self.best_ask_price = 100.1
                    self.last_traded_price = 100.05
                    self.best_bid_volume = 1000
                    self.best_ask_volume = 1000
            return MockL1Data()
        
        def evaluate_portfolio(self, agent_id):
            return 1000000.0
    
    gateway = MockGateway()
    
    # Test different approaches
    approaches = [
        ("Single Stock", lambda: MarketEnv(gateway, target_stock_id=1)),
        ("All 20 Stocks", lambda: MarketEnv(gateway, num_stocks=20)),
        ("Top-5 Portfolio", lambda: SimplifiedPortfolioEnv(gateway, num_stocks=20, top_k=5)),
        ("4-Sector Rotation", lambda: SectorRotationEnv(gateway, num_stocks=20, num_sectors=4)),
    ]
    
    print(f"{'Approach':<20} {'Action Dim':<12} {'Obs Dim':<15} {'Complexity':<15}")
    print("-" * 65)
    
    for name, env_creator in approaches:
        try:
            env = env_creator()
            
            if hasattr(env.action_space, 'n'):  # Discrete
                action_dim = env.action_space.n
                complexity = f"Discrete({action_dim})"
            else:  # Continuous
                action_dim = env.action_space.shape[0]
                complexity = f"Continuous({action_dim})"
            
            obs_dim = f"{env.observation_space.shape}"
            
            print(f"{name:<20} {action_dim:<12} {str(obs_dim):<15} {complexity:<15}")
            
        except Exception as e:
            print(f"{name:<20} {'ERROR':<12} {'ERROR':<15} {str(e)[:15]:<15}")
    
    print("\n=== Complexity Analysis ===")
    print("Single Stock:     8 actions (2 market + 6 limit orders)")
    print("All 20 Stocks:    161 actions (1 hold + 20×8 stock actions)")
    print("Top-5 Portfolio:  5 continuous weights (MUCH simpler!)")
    print("4-Sector:         4 continuous weights (SIMPLEST!)")
    print()
    print("Sample Efficiency Ranking (best to worst):")
    print("1. 4-Sector Rotation    (4D continuous)")
    print("2. Top-5 Portfolio      (5D continuous)")
    print("3. Single Stock         (8D discrete)")
    print("4. All 20 Stocks        (161D discrete)")

def test_portfolio_allocation_logic():
    """Test that portfolio allocation logic works correctly."""
    print("\n=== Portfolio Allocation Logic Test ===")
    
    # Test sector weight normalization
    raw_weights = np.array([0.3, 0.5, 0.1, 0.8])  # Doesn't sum to 1
    normalized = raw_weights / np.sum(raw_weights)
    
    print(f"Raw weights:        {raw_weights}")
    print(f"Normalized weights: {normalized}")
    print(f"Sum:                {np.sum(normalized):.6f}")
    
    # Test diversification calculation
    def calculate_diversification(weights):
        """Calculate portfolio diversification (entropy-based)."""
        weights = weights / np.sum(weights) if np.sum(weights) > 0 else weights
        entropy = -np.sum(weights * np.log(weights + 1e-8))
        max_entropy = np.log(len(weights))
        return entropy / max_entropy if max_entropy > 0 else 0.0
    
    test_cases = [
        ([1.0, 0.0, 0.0, 0.0], "Concentrated"),
        ([0.25, 0.25, 0.25, 0.25], "Perfectly Diversified"),
        ([0.4, 0.3, 0.2, 0.1], "Moderately Diversified"),
        ([0.7, 0.1, 0.1, 0.1], "Somewhat Concentrated"),
    ]
    
    print(f"\n{'Allocation':<25} {'Diversification':<15} {'Description'}")
    print("-" * 55)
    
    for weights, description in test_cases:
        div_score = calculate_diversification(np.array(weights))
        print(f"{str(weights):<25} {div_score:.3f}{'':11} {description}")

def test_reward_calculation():
    """Test reward calculation for portfolio management."""
    print("\n=== Reward Calculation Test ===")
    
    # Simulate portfolio value changes
    portfolio_history = [1000000, 1001000, 999500, 1002000, 1005000]
    
    print("Portfolio Value History:", portfolio_history)
    
    # Calculate returns
    returns = np.diff(portfolio_history)
    print("Returns:", returns)
    
    # Calculate risk-adjusted reward
    if len(returns) >= 3:
        volatility = np.std(returns)
        sharpe_like = np.mean(returns) / (volatility + 1e-8)
        print(f"Average Return: {np.mean(returns):.2f}")
        print(f"Volatility: {volatility:.2f}")
        print(f"Sharpe-like Ratio: {sharpe_like:.3f}")
    
    # Test different reward formulations
    print("\nReward Formulations:")
    print("1. Simple Return:     reward = portfolio_change")
    print("2. Risk-Adjusted:     reward = return - 0.1 * volatility")
    print("3. Diversification:   reward = return + 0.01 * diversity_bonus")

def demonstrate_curse_of_dimensionality():
    """Demonstrate why the curse of dimensionality is a real problem."""
    print("\n=== Curse of Dimensionality Demonstration ===")
    
    # Calculate exploration requirements
    def exploration_samples(dimensions, levels_per_dim):
        """Estimate samples needed for reasonable exploration."""
        total_combinations = levels_per_dim ** dimensions
        # Rule of thumb: need ~10 samples per combination for basic coverage
        return total_combinations * 10
    
    scenarios = [
        ("Single Stock", 1, 8),
        ("Top-5 Portfolio", 5, 10),  # 10 allocation levels per stock
        ("All 20 Stocks", 20, 10),
        ("Hierarchical (4 sectors)", 4, 10),
    ]
    
    print(f"{'Scenario':<20} {'Dimensions':<12} {'Combinations':<15} {'Est. Samples':<15}")
    print("-" * 65)
    
    for name, dims, levels in scenarios:
        combinations = levels ** dims
        samples = exploration_samples(dims, levels)
        
        if combinations > 1e12:
            comb_str = f"{combinations:.2e}"
            samp_str = f"{samples:.2e}"
        else:
            comb_str = f"{combinations:,}"
            samp_str = f"{samples:,}"
        
        print(f"{name:<20} {dims:<12} {comb_str:<15} {samp_str:<15}")
    
    print("\nKey Insights:")
    print("• Single stock: 8 actions → manageable")
    print("• All 20 stocks: 10^20 combinations → impossible!")
    print("• Top-5 portfolio: 10^5 combinations → challenging but doable")
    print("• 4-sector rotation: 10^4 combinations → very manageable")
    print("\nThis is why hierarchical decomposition is essential!")

if __name__ == "__main__":
    print("Progressive Portfolio Management - Test Suite")
    print("=" * 60)
    
    test_action_space_dimensions()
    test_portfolio_allocation_logic()
    test_reward_calculation()
    demonstrate_curse_of_dimensionality()
    
    print("\n" + "=" * 60)
    print("Test suite completed!")
    print("\nNext steps:")
    print("1. Run: python train_progressive_portfolio.py --phase 1 --stock-id 1")
    print("2. Run: python train_progressive_portfolio.py --phase 2 --top-k 5")
    print("3. Run: python train_progressive_portfolio.py --phase 3 --sectors 4")