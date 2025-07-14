import time
import random
import threading
import os
from market_gateway_client.rl_gateway import RLGateway

# --- Configuration ---
NUM_AGENTS = 2
STOCK_IDS = list(range(1, 21)) # Stocks from 1 to 20
MIN_VOLUME = 10000
MAX_VOLUME = 10000000
ORDER_TYPES = ["Market", "Limit"]
SIDES = ["Buy", "Sell"]

# --- Agent Logic ---
def run_agent_activity(agent_id: str, gateway: RLGateway):
    print(f"[Agent {agent_id[:8]}] Starting activity...")
    while True:
        try:
            stock_id = random.choice(STOCK_IDS)
            order_type = random.choice(ORDER_TYPES)
            side = random.choice(SIDES)
            volume = random.randint(MIN_VOLUME, MAX_VOLUME)
            price = 0.0

            if order_type == "Limit":
                l1_data = gateway.get_l1_data(stock_id)
                if l1_data:
                    if side == "Buy":
                        price = l1_data.best_bid_price * (1 - random.uniform(0.001, 0.005)) # slightly lower than best bid
                    else: # Sell
                        price = l1_data.best_ask_price * (1 + random.uniform(0.001, 0.005)) # slightly higher than best ask
                    price = round(price, 2)
                else:
                    # Fallback if no L1 data is available yet
                    price = round(150.0 + random.uniform(-10.0, 10.0), 2)


            gateway.submit_order(agent_id, stock_id, side, order_type, volume, price)
            # print(f"[Agent {agent_id[:8]}] Submitted {order_type} {side} {volume}@{price} for stock {stock_id}") # Suppressed for cleaner output

            # Simulate some thinking time for the agent
            time.sleep(random.uniform(0.1, 0.5))

            # Optionally, check for updates (acks/trades) for this agent
            update = gateway.get_update(agent_id, block=False) # Non-blocking check
            if update:
                # Check the type of update and print accordingly
                if hasattr(update, 'order_id') and hasattr(update, 'status'): # Likely an OrderAck
                    print(f"[Agent {agent_id[:8]}] ACK: Order {update.order_id} Status: {update.status} Details: {update.details}")
                elif hasattr(update, 'order_id') and hasattr(update, 'volume_filled'): # Likely a TradeUpdate
                    print(f"[Agent {agent_id[:8]}] TRADE: Order {update.order_id} Stock {update.stock_id} Filled {update.volume_filled}@{update.price}")
                else:
                    print(f"[Agent {agent_id[:8]}] Received unknown update type: {update}")

        except Exception as e:
            print(f"[Agent {agent_id[:8]}] Error: {e}")
            time.sleep(1) # Wait before retrying

def log_l1_data(gateway: RLGateway):
    while True:
        try:
            stock_id = random.choice(STOCK_IDS)
            l1_data = gateway.get_l1_data(stock_id)
            if l1_data:
                print(f"[L1 Data] Stock: {l1_data.stock_id}, Bid: {l1_data.best_bid_price:.2f} ({l1_data.best_bid_volume}), Ask: {l1_data.best_ask_price:.2f} ({l1_data.best_ask_volume}), Last: {l1_data.last_traded_price:.2f}")
            time.sleep(2) # Adjust sleep time as needed
        except Exception as e:
            print(f"[L1 Logger] Error: {e}")
            time.sleep(1)

# --- Main Entry Point ---
if __name__ == "__main__":
    print("--- Starting Multi-Agent Demo ---")
    print("Make sure the Rust gRPC server is running in a separate terminal.")

    # --- Connection Configuration ---
    # Read host and port from environment variables, with fallbacks for local development.
    host = os.environ.get("GRPC_HOST", "localhost")
    port = int(os.environ.get("GRPC_PORT", 50051))
    print(f"Attempting to connect to gRPC server at {host}:{port}...")


    # Initialize the RLGateway (it's a singleton)
    gateway = RLGateway(host=host, port=port)

    agent_threads = []
    agent_ids = []

    # Register agents and start their activity threads
    for i in range(NUM_AGENTS):
        agent_id = gateway.register_agent()
        agent_ids.append(agent_id)
        thread = threading.Thread(target=run_agent_activity, args=(agent_id, gateway), daemon=True)
        agent_threads.append(thread)
        thread.start()

    # Start the L1 data logger
    l1_logger_thread = threading.Thread(target=log_l1_data, args=(gateway,), daemon=True)
    l1_logger_thread.start()

    print(f"Started {NUM_AGENTS} agents. Press Ctrl+C to stop.")

    try:
        # Keep the main thread alive so daemon threads can run
        while True:
            time.sleep(1)
    except KeyboardInterrupt:
        print("--- Stopping Multi-Agent Demo ---")
        gateway.shutdown()
        # Give threads a moment to clean up (optional, as they are daemon threads)
        time.sleep(0.5)
