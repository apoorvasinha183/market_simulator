import unittest
import time
from market_gateway_client.rl_gateway import RLGateway

class TestRLGateway(unittest.TestCase):

    def test_multi_agent_order_submission(self):
        """Tests that the gateway can register multiple agents and submit orders."""
        gateway = RLGateway()
        
        agent1_id = gateway.register_agent()
        agent2_id = gateway.register_agent()

        # We submit orders and assume the server is running to process them.
        # The test verifies that the client-side logic runs without errors.
        gateway.submit_order(agent1_id, 1, "Buy", "Market", 100)
        gateway.submit_order(agent2_id, 2, "Sell", "Limit", 50, 150.0)

        # Give a moment for the dispatcher thread to process potential messages
        time.sleep(1)

        gateway.shutdown()

if __name__ == '__main__':
    unittest.main()