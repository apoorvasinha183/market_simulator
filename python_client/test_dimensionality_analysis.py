#!/usr/bin/env python3
"""
Standalone test script for analyzing the curse of dimensionality in portfolio management.
This doesn't require gRPC or the full market simulator.
"""

import numpy as np
from typing import List, Tuple

def analyze_action_space_complexity():
    """Analyze action space complexity for different approaches."""
    print("=== Action Space Dimensionality Analysis ===")
    
    # Define different approaches
    approaches = [
        ("Single Stock Trading", 1, 8, "discrete"),  # 1 stock, 8 actions per stock
        ("5-Stock Portfolio", 5, 8, "discrete"),     # 5 stocks, 8 actions per stock  
        ("20-Stock Portfolio", 20, 8, "discrete"),   # 20 stocks, 8 actions per stock
        ("Top-5 Continuous", 5, 1, "continuous"),    # 5 continuous weights
        ("Sector Rotation", 4, 1, "continuous"),     # 4 sector weights
        ("Factor Allocation", 3, 1, "continuous"),   # 3 factor exposures
    ]
    
    print(f"{'Approach':<20} {'Dimensions':<12} {'Type':<12} {'Total Actions':<15} {'Complexity'}")
    print("-" * 80)
    
    for name, dims, actions_per_dim, action_type in approaches:
        if action_type == "discrete":
            total_actions = 1 + (dims * actions_per_dim)  # +1 for hold action
            complexity = f"O({total_actions})"
        else:  # continuous
            total_actions = dims
            complexity = f"O(∞^{dims})"
        
        print(f"{name:<20} {dims:<12} {action_type:<12} {total_actions:<15} {complexity}")
    
    return approaches

def calculate_sample_complexity():
    """Calculate estimated sample complexity for different approaches."""
    print("\n=== Sample Complexity Analysis ===")
    
    def estimate_samples_needed(dimensions: int, action_type: str) -> int:
        """Estimate samples needed based on dimensionality."""
        if action_type == "discrete":
            # For discrete spaces: roughly linear in action count
            return dimensions * 1000
        else:  # continuous
            # For continuous spaces: exponential in dimensions
            return int(10000 * (2 ** dimensions))
    
    approaches = [
        ("Single Stock", 8, "discrete"),
        ("Top-5 Portfolio", 5, "continuous"), 
        ("Sector Rotation", 4, "continuous"),
        ("20-Stock Discrete", 161, "discrete"),
        ("20-Stock Continuous", 20, "continuous"),
    ]
    
    print(f"{'Approach':<20} {'Dimensions':<12} {'Est. Samples':<15} {'Training Time'}")
    print("-" * 65)
    
    for name, dims, action_type in approaches:
        samples = estimate_samples_needed(dims, action_type)
        
        # Estimate training time (assuming 1000 samples/minute)
        time_minutes = samples / 1000
        if time_minutes < 60:
            time_str = f"{time_minutes:.0f} min"
        elif time_minutes < 1440:  # 24 hours
            time_str = f"{time_minutes/60:.1f} hours"
        else:
            time_str = f"{time_minutes/1440:.1f} days"
        
        print(f"{name:<20} {dims:<12} {samples:,}{'':6} {time_str}")

def demonstrate_portfolio_allocation():
    """Demonstrate different portfolio allocation strategies."""
    print("\n=== Portfolio Allocation Strategies ===")
    
    # Simulate 4 sectors with different characteristics
    sectors = ["Technology", "Finance", "Healthcare", "Energy"]
    
    # Different allocation strategies
    strategies = {
        "Equal Weight": [0.25, 0.25, 0.25, 0.25],
        "Growth Focused": [0.5, 0.2, 0.2, 0.1],
        "Defensive": [0.2, 0.3, 0.4, 0.1],
        "Energy Rotation": [0.1, 0.2, 0.2, 0.5],
        "Concentrated": [0.8, 0.1, 0.05, 0.05],
    }
    
    print(f"{'Strategy':<15} {'Tech':<8} {'Finance':<8} {'Health':<8} {'Energy':<8} {'Diversity'}")
    print("-" * 70)
    
    for strategy_name, weights in strategies.items():
        # Calculate diversity (entropy-based)
        weights_array = np.array(weights)
        entropy = -np.sum(weights_array * np.log(weights_array + 1e-8))
        max_entropy = np.log(len(weights_array))
        diversity = entropy / max_entropy
        
        weight_strs = [f"{w:.2f}" for w in weights]
        print(f"{strategy_name:<15} {weight_strs[0]:<8} {weight_strs[1]:<8} {weight_strs[2]:<8} {weight_strs[3]:<8} {diversity:.3f}")

def simulate_learning_curves():
    """Simulate learning curves for different approaches."""
    print("\n=== Simulated Learning Curves ===")
    
    # Simulate learning progress for different approaches
    steps = np.arange(0, 50000, 1000)
    
    # Different learning rates based on complexity
    def learning_curve(steps, max_reward, learning_rate, noise_level):
        """Generate a realistic learning curve."""
        progress = 1 - np.exp(-learning_rate * steps / 10000)
        noise = np.random.normal(0, noise_level, len(steps))
        return max_reward * progress + noise
    
    np.random.seed(42)  # For reproducible results
    
    curves = {
        "Single Stock": learning_curve(steps, 100, 0.8, 5),
        "Sector Rotation": learning_curve(steps, 95, 1.2, 3),
        "Top-5 Portfolio": learning_curve(steps, 90, 0.6, 8),
        "20-Stock Portfolio": learning_curve(steps, 110, 0.2, 15),
    }
    
    # Print final performance
    print(f"{'Approach':<20} {'Final Reward':<15} {'Learning Speed':<15} {'Stability'}")
    print("-" * 65)
    
    for name, curve in curves.items():
        final_reward = curve[-1]
        learning_speed = "Fast" if curve[10] > 20 else "Medium" if curve[10] > 10 else "Slow"
        stability = "High" if np.std(curve[-10:]) < 5 else "Medium" if np.std(curve[-10:]) < 10 else "Low"
        
        print(f"{name:<20} {final_reward:.1f}{'':10} {learning_speed:<15} {stability}")
    
    return steps, curves

def analyze_risk_return_tradeoffs():
    """Analyze risk-return tradeoffs for different portfolio approaches."""
    print("\n=== Risk-Return Analysis ===")
    
    # Simulate portfolio performance metrics
    portfolios = {
        "Single Stock": {"return": 0.12, "volatility": 0.25, "sharpe": 0.48},
        "Equal Weight (20)": {"return": 0.10, "volatility": 0.18, "sharpe": 0.56},
        "Top-5 Selection": {"return": 0.11, "volatility": 0.20, "sharpe": 0.55},
        "Sector Rotation": {"return": 0.09, "volatility": 0.15, "sharpe": 0.60},
        "Factor-Based": {"return": 0.08, "volatility": 0.12, "sharpe": 0.67},
    }
    
    print(f"{'Portfolio':<18} {'Return':<8} {'Volatility':<12} {'Sharpe':<8} {'Risk-Adj Return'}")
    print("-" * 70)
    
    for name, metrics in portfolios.items():
        risk_adj_return = metrics["return"] - 0.5 * metrics["volatility"]  # Simple risk adjustment
        
        print(f"{name:<18} {metrics['return']:.1%}{'':3} {metrics['volatility']:.1%}{'':7} "
              f"{metrics['sharpe']:.2f}{'':4} {risk_adj_return:.1%}")

def recommend_implementation_path():
    """Recommend the best implementation path."""
    print("\n=== Recommended Implementation Path ===")
    
    phases = [
        {
            "phase": "Phase 1: Single Stock Mastery",
            "complexity": "Low",
            "time": "1-2 weeks",
            "description": "Master single stock trading with 8 discrete actions",
            "benefits": ["Learn basic RL concepts", "Fast training", "Easy debugging"],
            "action_space": "8 discrete actions"
        },
        {
            "phase": "Phase 2: Sector Rotation",
            "complexity": "Medium",
            "time": "2-3 weeks", 
            "description": "4-sector allocation with continuous weights",
            "benefits": ["Manageable complexity", "Portfolio thinking", "Good performance"],
            "action_space": "4 continuous weights"
        },
        {
            "phase": "Phase 3: Top-K Selection",
            "complexity": "Medium-High",
            "time": "3-4 weeks",
            "description": "Select and weight top 5 stocks dynamically",
            "benefits": ["Stock selection skills", "Higher potential returns", "Adaptive"],
            "action_space": "5 continuous weights"
        },
        {
            "phase": "Phase 4: Hierarchical Portfolio",
            "complexity": "High",
            "time": "4-6 weeks",
            "description": "Full hierarchical sector → stock allocation",
            "benefits": ["Professional approach", "Scalable", "Interpretable"],
            "action_space": "4 sectors + 5 stocks/sector"
        }
    ]
    
    for i, phase in enumerate(phases, 1):
        print(f"\n{phase['phase']}")
        print(f"  Complexity: {phase['complexity']}")
        print(f"  Time: {phase['time']}")
        print(f"  Action Space: {phase['action_space']}")
        print(f"  Description: {phase['description']}")
        print(f"  Benefits: {', '.join(phase['benefits'])}")
    
    print(f"\n{'Key Insights:'}")
    print("• Start simple - single stock trading builds intuition")
    print("• Sector rotation offers best complexity/performance tradeoff")
    print("• Avoid jumping directly to 20-stock portfolio")
    print("• Each phase builds skills needed for the next")

def main():
    """Run the complete dimensionality analysis."""
    print("Portfolio Management RL: Curse of Dimensionality Analysis")
    print("=" * 65)
    
    analyze_action_space_complexity()
    calculate_sample_complexity()
    demonstrate_portfolio_allocation()
    
    steps, curves = simulate_learning_curves()
    
    analyze_risk_return_tradeoffs()
    recommend_implementation_path()
    
    print("\n" + "=" * 65)
    print("Analysis Complete!")
    print("\nKey Takeaways:")
    print("1. Dimensionality grows exponentially → Use hierarchical decomposition")
    print("2. Sector rotation (4D) is the sweet spot for learning")
    print("3. Single stock mastery is essential foundation")
    print("4. Progressive complexity prevents overwhelming the agent")
    
    # Optional: Create a simple visualization if matplotlib is available
    try:
        import matplotlib.pyplot as plt
        
        plt.figure(figsize=(12, 8))
        
        # Plot 1: Learning curves
        plt.subplot(2, 2, 1)
        for name, curve in curves.items():
            plt.plot(steps, curve, label=name, linewidth=2)
        plt.xlabel('Training Steps')
        plt.ylabel('Cumulative Reward')
        plt.title('Learning Curves by Approach')
        plt.legend()
        plt.grid(True, alpha=0.3)
        
        # Plot 2: Complexity vs Performance
        plt.subplot(2, 2, 2)
        approaches = ["Single Stock", "Sector Rotation", "Top-5", "20-Stock"]
        complexity = [8, 4, 5, 161]
        performance = [85, 95, 90, 70]  # Estimated performance scores
        
        plt.scatter(complexity, performance, s=100, alpha=0.7)
        for i, approach in enumerate(approaches):
            plt.annotate(approach, (complexity[i], performance[i]), 
                        xytext=(5, 5), textcoords='offset points')
        plt.xlabel('Action Space Complexity')
        plt.ylabel('Performance Score')
        plt.title('Complexity vs Performance')
        plt.grid(True, alpha=0.3)
        
        # Plot 3: Risk-Return scatter
        plt.subplot(2, 2, 3)
        returns = [0.12, 0.10, 0.11, 0.09, 0.08]
        risks = [0.25, 0.18, 0.20, 0.15, 0.12]
        labels = ["Single Stock", "Equal Weight", "Top-5", "Sector Rotation", "Factor-Based"]
        
        plt.scatter(risks, returns, s=100, alpha=0.7)
        for i, label in enumerate(labels):
            plt.annotate(label, (risks[i], returns[i]), 
                        xytext=(5, 5), textcoords='offset points')
        plt.xlabel('Volatility (Risk)')
        plt.ylabel('Expected Return')
        plt.title('Risk-Return Tradeoff')
        plt.grid(True, alpha=0.3)
        
        # Plot 4: Sample complexity
        plt.subplot(2, 2, 4)
        dimensions = [1, 4, 5, 8, 20]
        samples = [8000, 160000, 320000, 64000, 10485760]
        approach_names = ["Single", "Sector", "Top-5", "Discrete-8", "20-Stock"]
        
        plt.bar(approach_names, samples, alpha=0.7)
        plt.yscale('log')
        plt.ylabel('Estimated Samples Needed (log scale)')
        plt.title('Sample Complexity by Approach')
        plt.xticks(rotation=45)
        
        plt.tight_layout()
        plt.savefig('portfolio_dimensionality_analysis.png', dpi=300, bbox_inches='tight')
        print(f"\nVisualization saved as 'portfolio_dimensionality_analysis.png'")
        
    except ImportError:
        print("\nNote: Install matplotlib to generate visualizations")

if __name__ == "__main__":
    main()