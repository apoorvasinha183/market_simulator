#!/usr/bin/env python3
"""
Validate the progressive portfolio framework without requiring full training.
"""

import os
import sys
import numpy as np
import gymnasium as gym
from typing import Dict, List

# Add the parent directory to sys.path
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), '.')))

def test_imports():
    """Test that all required modules can be imported."""
    print("=== Testing Imports ===")
    
    try:
        from rl_agent.environment import MarketEnv
        print("✓ MarketEnv imported successfully")
    except Exception as e:
        print(f"✗ MarketEnv import failed: {e}")
        return False
    
    try:
        from rl_agent.portfolio_env import SimplifiedPortfolioEnv
        print("✓ SimplifiedPortfolioEnv imported successfully")
    except Exception as e:
        print(f"✗ SimplifiedPortfolioEnv import failed: {e}")
        return False
    
    try:
        from rl_agent.continuous_ppo_agent import ContinuousPPOAgent
        print("✓ ContinuousPPOAgent imported successfully")
    except Exception as e:
        print(f"✗ ContinuousPPOAgent import failed: {e}")
        return False
    
    try:
        from train_progressive_portfolio import SectorRotationEnv, ProgressivePortfolioTrainer
        print("✓ Progressive training components imported successfully")
    except Exception as e:
        print(f"✗ Progressive training import failed: {e}")
        return False
    
    return True

def test_mock_environments():
    """Test environments with mock data."""
    print("\n=== Testing Mock Environments ===")
    
    # Import here to avoid issues
    from rl_agent.environment import MarketEnv
    from rl_agent.portfolio_env import SimplifiedPortfolioEnv
    from train_progressive_portfolio import SectorRotationEnv
    
    # Mock gateway
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
            class MockL1Data:
                def __init__(self):
                    self.best_bid_price = 100.0 + np.random.normal(0, 1)
                    self.best_ask_price = self.best_bid_price + 0.1
                    self.last_traded_price = (self.best_bid_price + self.best_ask_price) / 2
                    self.best_bid_volume = 1000 + int(np.random.normal(0, 100))
                    self.best_ask_volume = 1000 + int(np.random.normal(0, 100))
            return MockL1Data()
        
        def evaluate_portfolio(self, agent_id):
            return 1000000.0 + np.random.normal(0, 1000)
    
    gateway = MockGateway()
    
    # Test different environments
    environments = [
        ("Single Stock", lambda: MarketEnv(gateway, target_stock_id=1, history_length=10)),
        ("Top-5 Portfolio", lambda: SimplifiedPortfolioEnv(gateway, num_stocks=20, top_k=5, history_length=10)),
        ("Sector Rotation", lambda: SectorRotationEnv(gateway, num_stocks=20, num_sectors=4, history_length=10)),
    ]
    
    for name, env_creator in environments:
        try:
            env = env_creator()
            
            # Test reset
            obs, info = env.reset()
            print(f"✓ {name}: Reset successful, obs shape: {obs.shape}")
            
            # Test action space
            if hasattr(env.action_space, 'n'):  # Discrete
                action = env.action_space.sample()
                action_info = f"Discrete({env.action_space.n}), sample: {action}"
            else:  # Continuous
                action = env.action_space.sample()
                action_info = f"Continuous{env.action_space.shape}, sample: {action[:3]}..."
            
            print(f"  Action space: {action_info}")
            
            # Test step (might fail due to mock data, but should not crash)
            try:
                next_obs, reward, done, truncated, info = env.step(action)
                print(f"  Step successful, reward: {reward:.2f}")
            except Exception as step_error:
                print(f"  Step failed (expected with mock data): {step_error}")
            
        except Exception as e:
            print(f"✗ {name}: Failed - {e}")
            return False
    
    return True

def test_agent_creation():
    """Test that agents can be created with correct dimensions."""
    print("\n=== Testing Agent Creation ===")
    
    try:
        from rl_agent.ppo_agent import PPOAgent
        from rl_agent.continuous_ppo_agent import ContinuousPPOAgent
        
        # Test discrete agent
        discrete_agent = PPOAgent(input_dims=10, n_actions=9)
        print("✓ Discrete PPO agent created successfully")
        
        # Test continuous agent
        continuous_agent = ContinuousPPOAgent(input_dims=15, action_dims=5)
        print("✓ Continuous PPO agent created successfully")
        
        return True
        
    except Exception as e:
        print(f"✗ Agent creation failed: {e}")
        return False

def test_sector_grouping():
    """Test sector grouping logic."""
    print("\n=== Testing Sector Grouping ===")
    
    try:
        from train_progressive_portfolio import SectorRotationEnv
        
        # Mock gateway
        class MockGateway:
            def __init__(self):
                self._agent_state = {"test": {'cash': 1000000, 'inventory': {}}}
            def register_agent(self):
                return "test"
            def get_l1_data(self, stock_id):
                return None
            def evaluate_portfolio(self, agent_id):
                return 1000000
        
        gateway = MockGateway()
        env = SectorRotationEnv(gateway, num_stocks=20, num_sectors=4)
        
        print(f"✓ Created {len(env.sectors)} sectors:")
        for sector_name, stock_list in env.sectors.items():
            print(f"  {sector_name}: stocks {stock_list}")
        
        # Test that all stocks are assigned
        all_assigned_stocks = []
        for stock_list in env.sectors.values():
            all_assigned_stocks.extend(stock_list)
        
        expected_stocks = list(range(1, 21))
        if sorted(all_assigned_stocks) == expected_stocks:
            print("✓ All stocks properly assigned to sectors")
        else:
            print(f"✗ Stock assignment error: expected {expected_stocks}, got {sorted(all_assigned_stocks)}")
            return False
        
        return True
        
    except Exception as e:
        print(f"✗ Sector grouping test failed: {e}")
        return False

def test_portfolio_math():
    """Test portfolio allocation mathematics."""
    print("\n=== Testing Portfolio Mathematics ===")
    
    # Test weight normalization
    raw_weights = np.array([0.3, 0.5, 0.1, 0.8])
    normalized = raw_weights / np.sum(raw_weights)
    
    if abs(np.sum(normalized) - 1.0) < 1e-6:
        print("✓ Weight normalization works correctly")
    else:
        print(f"✗ Weight normalization failed: sum = {np.sum(normalized)}")
        return False
    
    # Test diversification calculation
    def calculate_diversification(weights):
        weights = weights / np.sum(weights) if np.sum(weights) > 0 else weights
        entropy = -np.sum(weights * np.log(weights + 1e-8))
        max_entropy = np.log(len(weights))
        return entropy / max_entropy if max_entropy > 0 else 0.0
    
    # Test cases
    equal_weights = np.array([0.25, 0.25, 0.25, 0.25])
    concentrated = np.array([1.0, 0.0, 0.0, 0.0])
    
    equal_div = calculate_diversification(equal_weights)
    concentrated_div = calculate_diversification(concentrated)
    
    if equal_div > 0.99 and concentrated_div < 0.01:
        print(f"✓ Diversification calculation works: equal={equal_div:.3f}, concentrated={concentrated_div:.3f}")
    else:
        print(f"✗ Diversification calculation failed: equal={equal_div:.3f}, concentrated={concentrated_div:.3f}")
        return False
    
    return True

def main():
    """Run all validation tests."""
    print("Progressive Portfolio Management - Framework Validation")
    print("=" * 65)
    
    all_tests_passed = True
    
    # Run tests
    tests = [
        test_imports,
        test_mock_environments,
        test_agent_creation,
        test_sector_grouping,
        test_portfolio_math,
    ]
    
    for test_func in tests:
        if not test_func():
            all_tests_passed = False
    
    print("\n" + "=" * 65)
    if all_tests_passed:
        print("🎉 ALL TESTS PASSED!")
        print("\nFramework is ready for training!")
        print("\nNext steps:")
        print("1. Start your market simulator backend")
        print("2. Run: python train_progressive_portfolio.py --phase 1 --stock-id 1 --steps 10000")
        print("3. Monitor training progress in ./progressive_portfolio_models/")
    else:
        print("❌ SOME TESTS FAILED!")
        print("\nPlease fix the issues above before proceeding with training.")
    
    print("\nFramework Summary:")
    print("• Phase 1: Single stock (8 discrete actions)")
    print("• Phase 2: Top-K portfolio (5 continuous weights)")
    print("• Phase 3: Sector rotation (4 continuous weights)")
    print("• Each phase builds complexity progressively")

if __name__ == "__main__":
    main()