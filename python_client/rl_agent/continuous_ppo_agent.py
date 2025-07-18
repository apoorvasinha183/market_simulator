import torch
import torch.nn as nn
import torch.optim as optim
import numpy as np
from torch.distributions import Normal

class ContinuousActorCritic(nn.Module):
    """
    Actor-Critic network for continuous action spaces (portfolio weights).
    """
    def __init__(self, input_dims, action_dims, lstm_hidden_size=256, fc_dims=128):
        super(ContinuousActorCritic, self).__init__()
        
        self.lstm = nn.LSTM(input_size=input_dims, hidden_size=lstm_hidden_size, batch_first=True)
        
        # Actor head - outputs mean and log_std for continuous actions
        self.actor_mean = nn.Sequential(
            nn.Linear(lstm_hidden_size, fc_dims),
            nn.ReLU(),
            nn.Linear(fc_dims, action_dims),
            nn.Softmax(dim=-1)  # Ensure weights sum to 1 for portfolio allocation
        )
        
        self.actor_log_std = nn.Sequential(
            nn.Linear(lstm_hidden_size, fc_dims),
            nn.ReLU(),
            nn.Linear(fc_dims, action_dims)
        )
        
        # Critic head
        self.critic_head = nn.Sequential(
            nn.Linear(lstm_hidden_size, fc_dims),
            nn.ReLU(),
            nn.Linear(fc_dims, 1)
        )
    
    def forward(self, state):
        # state shape: (batch_size, sequence_length, input_dims)
        lstm_out, (hidden, cell) = self.lstm(state)
        
        # Use the output from the last time step
        last_time_step_out = lstm_out[:, -1, :]
        
        action_mean = self.actor_mean(last_time_step_out)
        action_log_std = self.actor_log_std(last_time_step_out)
        action_std = torch.exp(action_log_std.clamp(-20, 2))  # Clamp for numerical stability
        
        state_value = self.critic_head(last_time_step_out)
        
        return action_mean, action_std, state_value
    
    def get_action(self, state, action=None):
        action_mean, action_std, state_value = self.forward(state)
        action_dist = Normal(action_mean, action_std)
        
        if action is None:
            action = action_dist.sample()
            # Ensure actions are positive and sum to 1 (for portfolio weights)
            action = torch.abs(action)
            action = action / torch.sum(action, dim=-1, keepdim=True)
        
        log_prob = action_dist.log_prob(action).sum(dim=-1)
        entropy = action_dist.entropy().sum(dim=-1)
        
        return action, log_prob, entropy, state_value


class ContinuousPPOAgent:
    """
    PPO Agent for continuous action spaces (portfolio management).
    """
    def __init__(self, input_dims, action_dims, gamma=0.99, gae_lambda=0.95,
                 policy_clip=0.2, learning_rate=3e-4, n_epochs=10, batch_size=64):
        self.device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
        print(f"[ContinuousPPOAgent] Using device: {self.device}")
        
        self.gamma = gamma
        self.gae_lambda = gae_lambda
        self.policy_clip = policy_clip
        self.n_epochs = n_epochs
        self.batch_size = batch_size
        
        self.policy = ContinuousActorCritic(input_dims, action_dims).to(self.device)
        self.optimizer = optim.Adam(self.policy.parameters(), lr=learning_rate)
        
        self.memory = []
    
    def get_action(self, state):
        """Get action for continuous action space."""
        with torch.no_grad():
            action, log_prob, _, value = self.policy.get_action(state)
        return action, log_prob, value
    
    def store_transition(self, state, action, log_prob, value, reward, done):
        self.memory.append((state, action, log_prob, value, reward, done))
    
    def learn(self):
        if not self.memory:
            return
        
        # Unpack memory
        states, actions, old_log_probs, values, rewards, dones = zip(*self.memory)
        
        # Convert to tensors
        states = torch.tensor(np.array(states), dtype=torch.float32).to(self.device)
        actions = torch.tensor(np.array(actions), dtype=torch.float32).to(self.device)
        old_log_probs = torch.tensor(old_log_probs, dtype=torch.float32).to(self.device)
        values = torch.tensor(values, dtype=torch.float32).to(self.device)
        
        # Calculate advantages using GAE
        advantages = np.zeros(len(rewards))
        gae = 0
        for t in reversed(range(len(rewards) - 1)):
            delta = rewards[t] + self.gamma * values[t+1] * (1-int(dones[t])) - values[t]
            gae = delta + self.gamma * self.gae_lambda * (1-int(dones[t])) * gae
            advantages[t] = gae
        
        advantages = torch.tensor(advantages, dtype=torch.float32).to(self.device)
        returns = advantages + values
        
        # Normalize advantages
        advantages = (advantages - advantages.mean()) / (advantages.std() + 1e-8)
        
        # PPO Update
        for _ in range(self.n_epochs):
            for i in range(0, len(states), self.batch_size):
                batch_indices = slice(i, min(i + self.batch_size, len(states)))
                
                batch_states = states[batch_indices]
                batch_actions = actions[batch_indices]
                batch_old_log_probs = old_log_probs[batch_indices]
                batch_advantages = advantages[batch_indices]
                batch_returns = returns[batch_indices]
                
                # Get new policy values
                _, new_log_probs, entropy, critic_value = self.policy.get_action(batch_states, batch_actions)
                critic_value = critic_value.squeeze()
                
                # Ratio for PPO loss
                prob_ratio = (new_log_probs - batch_old_log_probs).exp()
                
                # Clipped surrogate loss
                surr1 = prob_ratio * batch_advantages
                surr2 = torch.clamp(prob_ratio, 1.0 - self.policy_clip, 1.0 + self.policy_clip) * batch_advantages
                actor_loss = -torch.min(surr1, surr2).mean()
                
                # Critic loss
                critic_loss = (batch_returns - critic_value).pow(2).mean()
                
                # Total loss
                total_loss = actor_loss + 0.5 * critic_loss - 0.01 * entropy.mean()
                
                # Backpropagation
                self.optimizer.zero_grad()
                total_loss.backward()
                torch.nn.utils.clip_grad_norm_(self.policy.parameters(), 0.5)  # Gradient clipping
                self.optimizer.step()
        
        self.memory = []  # Clear memory after learning
    
    def save_model(self, path):
        torch.save(self.policy.state_dict(), path)
    
    def load_model(self, path):
        self.policy.load_state_dict(torch.load(path, map_location=self.device))