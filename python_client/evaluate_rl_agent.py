import os
import torch
import time
from tqdm import tqdm # Corrected import

from market_gateway_client.rl_gateway import RLGateway
from rl_agent.environment import MarketEnv
from rl_agent.ppo_agent import PPOAgent

if __name__ == "__main__":
    print("--- Starting RL Agent Evaluation ---")

    # --- Configuration ---
    host = os.environ.get("GRPC_HOST", "localhost")
    port = int(os.environ.get("GRPC_PORT", 50051))
    model_path = "./rl_agent_models/ppo_model_10.pth" # CHANGE THIS to the model you want to evaluate
    num_eval_steps = 10_000

    # --- Initialization ---
    if not os.path.exists(model_path):
        print(f"Error: Model file not found at {model_path}")
        exit()

    print(f"Connecting to gRPC server at {host}:{port}...")
    gateway = RLGateway(host=host, port=port)
    env = MarketEnv(gateway)

    agent = PPOAgent(
        input_dims=env.observation_space.shape[1],
        n_actions=env.action_space.n
    )
    agent.load_model(model_path)
    print(f"Loaded model from {model_path}")

    # --- Starting Evaluation Loop ---
    state, _ = env.reset()
    total_reward = 0

    # Wrap the loop with tqdm and set postfix
    pbar = tqdm(range(num_eval_steps), desc="Evaluating Agent") # Assign tqdm instance to pbar
    for i in pbar: # Iterate over pbar
        state_tensor = torch.from_numpy(state).float().unsqueeze(0).to(agent.device)
        
        # We don't need gradients during evaluation
        with torch.no_grad():
            action, _, _, _ = agent.policy.get_action(state_tensor)
        
        action = action.item()
        next_state, reward, _, _, _ = env.step(action)
        total_reward += reward

        # Update tqdm postfix with current total reward on the pbar instance
        pbar.set_postfix(portfolio_value=f"{total_reward:.2f}")

        state = next_state
        #time.sleep(0.1) # Slow down for observation

    print("--- Evaluation Finished ---")
    print(f"Final Portfolio Value (Total Reward): {total_reward:.4f}")
    gateway.shutdown()
