
import torch
import torch.nn as nn
from torch.distributions import Categorical

class ActorCritic(nn.Module):
    """
    An Actor-Critic neural network with an LSTM layer for processing time-series data.
    """
    def __init__(self, input_dims, n_actions, lstm_hidden_size=256, fc_dims=128):
        super(ActorCritic, self).__init__()

        self.lstm = nn.LSTM(input_size=input_dims, hidden_size=lstm_hidden_size, batch_first=True)
        
        self.actor_head = nn.Sequential(
            nn.Linear(lstm_hidden_size, fc_dims),
            nn.ReLU(),
            nn.Linear(fc_dims, n_actions)
        )
        
        self.critic_head = nn.Sequential(
            nn.Linear(lstm_hidden_size, fc_dims),
            nn.ReLU(),
            nn.Linear(fc_dims, 1) # Outputs a single value for the state
        )

    def forward(self, state):
        # state shape: (batch_size, sequence_length, input_dims)
        lstm_out, (hidden, cell) = self.lstm(state)
        
        # We use the output from the last time step for our policy and value
        last_time_step_out = lstm_out[:, -1, :]
        
        action_logits = self.actor_head(last_time_step_out)
        state_value = self.critic_head(last_time_step_out)
        
        return action_logits, state_value

    def get_action(self, state, action=None):
        action_logits, state_value = self.forward(state)
        action_probs = torch.softmax(action_logits, dim=-1)
        action_dist = Categorical(action_probs)

        if action is None:
            action = action_dist.sample()

        log_prob = action_dist.log_prob(action)
        entropy = action_dist.entropy()

        return action, log_prob, entropy, state_value
