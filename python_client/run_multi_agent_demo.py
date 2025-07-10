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
BASE_PRICE = 150.0
PRICE_VARIATION = 10.0
ORDER_TYPES = ["Market", "Limit"]
SIDES = ["Buy", "Sell"]

# --- Agent Logic ---
def run_agent_activity(agent_id: str, gateway: RLGateway):
    print(f"[Agent {agent_id[:8]}] Starting activity...")
    while True:
        try:
            stock_id = random.choice(STOCK_IDS)
            #order_type = random.choice(ORDER_TYPES)
            order_type= ORDER_TYPES[0]
            #side = random.choice(SIDES)
            side = SIDES[0]
            volume = random.randint(MIN_VOLUME, MAX_VOLUME)
            price = 0.0

            if order_type == "Limit":
                price = round(BASE_PRICE + random.uniform(-PRICE_VARIATION, PRICE_VARIATION), 2)

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
                elif hasattr(update, 'stock_id') and hasattr(update, 'best_bid_price'): # Likely a MarketUpdate
                    # Market updates can be very frequent, so print sparingly or only for debugging
                    # print(f"[Agent {agent_id[:8]}] MARKET: Stock {update.stock_id} Bid: {update.best_bid_price} Ask: {update.best_ask_price}")
                    pass # Suppress frequent market updates for cleaner output
                else:
                    print(f"[Agent {agent_id[:8]}] Received unknown update type: {update}")

        except Exception as e:
            print(f"[Agent {agent_id[:8]}] Error: {e}")
            time.sleep(1) # Wait before retrying

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
        print("Demo stopped.")
