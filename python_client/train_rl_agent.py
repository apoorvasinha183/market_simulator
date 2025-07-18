import os
import sys
import torch
import numpy as np
import argparse
import csv # Import csv module
import time # Import time for timestamp

# Add the parent directory (python_client) to sys.path
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), '.')))

from market_gateway_client.rl_gateway import RLGateway
from rl_agent.environment import MarketEnv
from rl_agent.ppo_agent import PPOAgent

if __name__ == "__main__":
    print("--- Starting RL Agent Training Playground ---")

    # --- Argument Parsing ---
    parser = argparse.ArgumentParser(description="Train a PPO agent in the Market Simulation Environment.")
    parser.add_argument('--history_length', type=int, default=30, 
                        help='Number of past observations to include in the state (agent memory).')
    parser.add_argument('--stock-id', type=int, default=None, 
                        help='Optional: Train on a specific stock ID. If not provided, train on all stocks.')
    args = parser.parse_args()

    # --- Configuration ---
    host = os.environ.get("GRPC_HOST", "localhost")
    port = int(os.environ.get("GRPC_PORT", 50051))
    
    num_training_steps = 100_000
    learning_interval = 2048 # Number of steps to collect before learning
    save_interval = 10 # Save model every N learning cycles
    model_save_path = "./rl_agent_models"
    os.makedirs(model_save_path, exist_ok=True)

    # Learning curve file setup
    learning_curve_file = os.path.join(model_save_path, "learning_curve.csv")
    file_exists = os.path.exists(learning_curve_file)
    csvfile = open(learning_curve_file, 'a', newline='')
    csv_writer = csv.writer(csvfile)
    if not file_exists:
        csv_writer.writerow(['learn_iteration', 'cumulative_reward', 'timestamp'])

    # --- Initialization ---
    print(f"Connecting to gRPC server at {host}:{port}...")
    gateway = RLGateway(host=host, port=port)
    
    # Pass history_length and target_stock_id to the environment constructor
    env = MarketEnv(gateway, history_length=args.history_length, target_stock_id=args.stock_id)

    agent = PPOAgent(
        input_dims=env.observation_space.shape[1],
        n_actions=env.action_space.n
    )

    print("--- Starting Training Loop ---")
    state, _ = env.reset()
    score = 0
    learn_iters = 0

    try:
        for i in range(num_training_steps):
            state_tensor = torch.from_numpy(state).float().unsqueeze(0).to(agent.device)
            action, prob, _, val = agent.policy.get_action(state_tensor)
            
            action = action.item()
            prob = prob.item()
            val = val.item()

            next_state, reward, done, _, _ = env.step(action)
            score += reward

            agent.store_transition(state, action, prob, val, reward, done)

            if (i + 1) % learning_interval == 0:
                print(f"Step {i+1}: Learning... Current Score: {score}")
                agent.learn()
                learn_iters += 1
                
                # Save learning curve data
                csv_writer.writerow([learn_iters, score, time.time()])
                csvfile.flush() # Ensure data is written to disk

                score = 0 # Reset score after learning

                if learn_iters % save_interval == 0:
                    save_path = os.path.join(model_save_path, f"ppo_model_{learn_iters}.pth")
                    print(f"Saving model to {save_path}")
                    agent.save_model(save_path)

            state = next_state

    except KeyboardInterrupt:
        print("--- Training Interrupted ---")
    finally:
        print("--- Training Finished ---")
        csvfile.close() # Close the CSV file
        gateway.shutdown()