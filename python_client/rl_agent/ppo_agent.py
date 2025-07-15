
import torch
import torch.optim as optim
import numpy as np

from .model import ActorCritic

class PPOAgent:
    def __init__(self, input_dims, n_actions, gamma=0.99, gae_lambda=0.95, 
                 policy_clip=0.2, learning_rate=3e-4, n_epochs=10, batch_size=64):
        self.device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
        print(f"[PPOAgent] Using device: {self.device}")

        self.gamma = gamma
        self.gae_lambda = gae_lambda
        self.policy_clip = policy_clip
        self.n_epochs = n_epochs
        self.batch_size = batch_size

        self.policy = ActorCritic(input_dims, n_actions).to(self.device)
        self.optimizer = optim.Adam(self.policy.parameters(), lr=learning_rate)

        self.memory = []

    def store_transition(self, state, action, probs, vals, reward, done):
        self.memory.append((state, action, probs, vals, reward, done))

    def learn(self):
        if not self.memory:
            return

        # Unpack memory
        states, actions, old_probs, vals, rewards, dones = zip(*self.memory)

        # Convert to tensors
        states = torch.tensor(np.array(states), dtype=torch.float32).to(self.device)
        actions = torch.tensor(actions).to(self.device)
        old_probs = torch.tensor(old_probs).to(self.device)
        vals = torch.tensor(vals).to(self.device)

        # Calculate advantages using GAE
        advantages = np.zeros(len(rewards))
        gae = 0
        for t in reversed(range(len(rewards) - 1)):
            delta = rewards[t] + self.gamma * vals[t+1] * (1-int(dones[t])) - vals[t]
            gae = delta + self.gamma * self.gae_lambda * (1-int(dones[t])) * gae
            advantages[t] = gae
        advantages = torch.tensor(advantages, dtype=torch.float32).to(self.device)
        
        returns = advantages + vals

        # PPO Update
        for _ in range(self.n_epochs):
            for i in range(0, len(states), self.batch_size):
                batch_indices = np.arange(i, i + self.batch_size)

                batch_states = states[batch_indices]
                batch_actions = actions[batch_indices]
                batch_old_probs = old_probs[batch_indices]
                batch_advantages = advantages[batch_indices]
                batch_returns = returns[batch_indices]

                # Get new policy values
                _, new_log_probs, entropy, critic_value = self.policy.get_action(batch_states, batch_actions)
                critic_value = critic_value.squeeze()

                # Ratio for PPO loss
                prob_ratio = (new_log_probs - batch_old_probs).exp()
                
                # Clipped surrogate loss
                surr1 = prob_ratio * batch_advantages
                surr2 = torch.clamp(prob_ratio, 1.0 - self.policy_clip, 1.0 + self.policy_clip) * batch_advantages
                actor_loss = -torch.min(surr1, surr2).mean()

                # Critic loss
                critic_loss = (batch_returns - critic_value)**2
                critic_loss = critic_loss.mean()

                # Total loss
                total_loss = actor_loss + 0.5 * critic_loss - 0.01 * entropy.mean()

                # Backpropagation
                self.optimizer.zero_grad()
                total_loss.backward()
                self.optimizer.step()

        self.memory = [] # Clear memory after learning

    def save_model(self, path):
        torch.save(self.policy.state_dict(), path)

    def load_model(self, path):
        self.policy.load_state_dict(torch.load(path, map_location=self.device))
