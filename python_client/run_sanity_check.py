import time
import random
import threading
from collections import defaultdict

from market_gateway_client.rl_gateway import RLGateway

# --- Configuration ---
NUM_AGENTS = 7
FIXED_QUANTITY = 1000
STOCK_ID_POOL = list(range(1, 21))  # Stocks from 1 to 20

# --- Global State for Verification ---
# Use a lock to protect shared global state if multiple threads write to it
# For this simple case, each agent only writes to its own inventory, so a lock might not be strictly necessary
# but it's good practice for shared mutable state.
agent_inventories = defaultdict(lambda: defaultdict(int))
agent_orders_completed = threading.Semaphore(0)
inventory_lock = threading.Lock() # To protect agent_inventories during updates

# --- Agent Logic ---
def run_sanity_agent_activity(agent_id: str, gateway: RLGateway, stock_id_to_buy: int, quantity_to_buy: int):
    print(f"[Agent {agent_id[:8]}] Starting activity for stock {stock_id_to_buy} with quantity {quantity_to_buy}...")

    # Submit a single Market Buy order
    gateway.submit_order(agent_id, stock_id_to_buy, "Buy", "Market", quantity_to_buy, 0.0)
    print(f"[Agent {agent_id[:8]}] Submitted Market Buy {quantity_to_buy} for stock {stock_id_to_buy}")

    total_filled = 0
    order_acknowledged = False

    while total_filled < quantity_to_buy:
        update = gateway.get_update(agent_id, block=True, timeout=5) # Blocking call, with timeout

        if update:
            if hasattr(update, 'order_id') and hasattr(update, 'status'): # OrderAck
                if not order_acknowledged:
                    print(f"[Agent {agent_id[:8]}] ACK: Order {update.order_id} Status: {update.status} Details: {update.details}")
                    order_acknowledged = True
            elif hasattr(update, 'order_id') and hasattr(update, 'volume_filled'): # TradeUpdate
                print(f"[Agent {agent_id[:8]}] TRADE: Order {update.order_id} Stock {update.stock_id} Filled {update.volume_filled}@{update.price}")
                with inventory_lock:
                    agent_inventories[agent_id][update.stock_id] += update.volume_filled
                    total_filled += update.volume_filled
                print(f"[Agent {agent_id[:8]}] Current filled for stock {stock_id_to_buy}: {total_filled}/{quantity_to_buy}")
            # else:
            #     print(f"[Agent {agent_id[:8]}] Received unknown update type: {update}")
        else:
            print(f"[Agent {agent_id[:8]}] No update received within timeout. Current filled: {total_filled}/{quantity_to_buy}")
            # If no update for a while, and not fully filled, might indicate a problem
            if total_filled < quantity_to_buy:
                print(f"[Agent {agent_id[:8]}] WARNING: Order not fully filled. Exiting loop.")
                break # Exit if no updates and not filled

    if total_filled >= quantity_to_buy:
        print(f"[Agent {agent_id[:8]}] Order for stock {stock_id_to_buy} fully filled!")
    else:
        print(f"[Agent {agent_id[:8]}] Order for stock {stock_id_to_buy} NOT fully filled. Acquired: {total_filled}/{quantity_to_buy}")

    agent_orders_completed.release() # Signal that this agent has finished its activity

# --- Main Entry Point ---
if __name__ == "__main__":
    print("--- Starting Sanity Check Demo ---")
    print("Make sure the Rust gRPC server is running in a separate terminal.")

    gateway = RLGateway()

    agent_threads = []
    agents_to_test = [] # List of (agent_id, stock_id, quantity) tuples

    # Register agents and assign them a stock and quantity
    for i in range(NUM_AGENTS):
        agent_id = gateway.register_agent()
        stock_id = random.choice(STOCK_ID_POOL)
        agents_to_test.append((agent_id, stock_id, FIXED_QUANTITY))
        print(f"Registered agent {agent_id[:8]} to buy {FIXED_QUANTITY} of stock {stock_id}")

    # Start agent activity threads
    for agent_id, stock_id, quantity in agents_to_test:
        thread = threading.Thread(
            target=run_sanity_agent_activity,
            args=(agent_id, gateway, stock_id, quantity),
            daemon=True
        )
        agent_threads.append(thread)
        thread.start()

    print(f"Started {NUM_AGENTS} agents. Waiting for all orders to complete...")

    try:
        # Wait for all agents to signal completion
        for _ in range(NUM_AGENTS):
            agent_orders_completed.acquire()
        print("\n--- All Agents Completed Their Orders ---")

        # --- Verification ---
        print("\n--- Verifying Acquired Quantities ---")
        all_passed = True
        with inventory_lock: # Acquire lock before reading global inventory
            for agent_id, expected_stock_id, expected_quantity in agents_to_test:
                acquired_quantity = agent_inventories[agent_id][expected_stock_id]
                if acquired_quantity == expected_quantity:
                    print(f"[VERIFIED] Agent {agent_id[:8]} acquired {acquired_quantity} of stock {expected_stock_id} (Expected: {expected_quantity})")
                else:
                    print(f"[FAILED] Agent {agent_id[:8]} acquired {acquired_quantity} of stock {expected_stock_id} (Expected: {expected_quantity})")
                    all_passed = False
        
        if all_passed:
            print("\n--- SANITY CHECK PASSED! All agents acquired the exact quantities. ---")
        else:
            print("\n--- SANITY CHECK FAILED! Some agents did not acquire the exact quantities. ---")

    except KeyboardInterrupt:
        print("\n--- Stopping Sanity Check Demo ---")
    finally:
        gateway.shutdown()
        time.sleep(0.5) # Give threads a moment to clean up
        print("Demo stopped.")