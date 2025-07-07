import grpc
import threading
import queue
import time
import random  # For random delays

# Import the generated files
from .generated import market_gateway_pb2
from .generated import market_gateway_pb2_grpc


class Broker:
    """
    The Broker class is the main client interface that connects to the Rust gRPC server.
    It manages the connection and handles the bidirectional stream of events.
    """
    def __init__(self, host="localhost", port=50051):
        self.host = host
        self.port = port
        self.channel = None
        self.stub = None
        self.is_running = False
        self.outgoing_messages = queue.Queue()

    # ──────────────────────────────────────────────────────────────────────────
    # internal helpers
    # ──────────────────────────────────────────────────────────────────────────
    def _listen_for_updates(self, stream):
        """
        Runs in a separate thread, listening for messages from the server.
        """
        print("[Broker] Listening for messages from the server...")
        try:
            for update in stream:
                print(f"[Broker] Received update: {update}")
        except grpc.RpcError as e:
            if self.is_running:
                print(f"[Broker] Error listening for updates: {e.status()}")

    def _generate_requests(self):
        """
        Generator that yields messages from outgoing_messages.
        """
        while self.is_running:
            try:
                message = self.outgoing_messages.get(timeout=1)
                yield message
            except queue.Empty:
                continue

    def send_order(self, client_id, stock_id, side, order_type, volume, price=0.0):
        """
        Public method to send an order to the market.
        """
        submit_order_request = market_gateway_pb2.SubmitOrderRequest(
            client_id=client_id,
            stock_id=stock_id,
            side=side,
            price=price,
            volume=volume,
            order_type=order_type,
        )
        from_python_message = market_gateway_pb2.FromPython(
            submit_order=submit_order_request
        )
        self.outgoing_messages.put(from_python_message)
        print(f"[Broker] Enqueued order: {order_type} {side} {volume}@{price} for stock {stock_id}")

    def _order_generator_thread(self, num_orders=100):
        """
        Background thread: generates and enqueues various types of orders.
        """
        print(f"[Broker] 🔫  Order generator thread started (generating {num_orders} orders).")
        stock_ids = [1, 2, 3,4] # Example stock IDs
        sides = ["Buy", "Sell"]
        order_types = ["Market", "Limit"]
        buy_bias = 0.7
        for i in range(num_orders):
            client_id = f"customer_agent_{random.randint(0, 2)}" # Example client IDs
            stock_id = random.choice(stock_ids)
            #stock_id = 3
            #side = random.choice(sides)
            side = "Buy" if random.random() < buy_bias else "Sell"
            #side = sides[0]
            #order_type = random.choice(order_types)
            order_type = order_types[0]
            volume = random.randint(5000, 50000) # Adjusted volume for more impact

            price = 0.0
            if order_type == "Limit":
                # Generate a more dynamic price for limit orders
                base_price = 150.0 # Still a base, but now with wider swings
                price = round(base_price + random.uniform(-10.0, 10.0), 2)

            self.send_order(client_id, stock_id, side, order_type, volume, price)
            time.sleep(random.uniform(0.05, 0.5)) # Faster order generation
        print("[Broker] Order generator thread finished.")

    # ──────────────────────────────────────────────────────────────────────────
    # public API
    # ──────────────────────────────────────────────────────────────────────────
    def run(self):
        """
        Establishes the connection and starts the listener thread.
        """
        address = f"{self.host}:{self.port}"
        print(f"[Broker] Connecting to gRPC server at {address}...")
        import time
        time.sleep(2) # Give the server a moment to start

        try:
            self.channel = grpc.insecure_channel(address)
            self.stub = market_gateway_pb2_grpc.MarketGatewayStub(self.channel)
            self.is_running = True

            # Start bidirectional stream
            server_stream = self.stub.EventStream(self._generate_requests())
            print("[Broker] gRPC connection successful. Event stream established.")

            # Listener for server updates
            listener_thread = threading.Thread(
                target=self._listen_for_updates, args=(server_stream,), daemon=True
            )
            listener_thread.start()

            # Launch order generator thread
            order_gen_thread = threading.Thread(
                target=self._order_generator_thread, args=(1000,), daemon=True
            )
            order_gen_thread.start()
            print("[Broker] Started dynamic order generation.")

            # Keep main thread alive until order generation is done
            while order_gen_thread.is_alive():
                time.sleep(0.1)

        except grpc.RpcError as e:
            print(f"[Broker] Failed to connect to gRPC server: {e.status()}")
        except KeyboardInterrupt:
            print("[Broker] Shutting down.")
        finally:
            self.stop()

    def stop(self):
        """
        Stops the broker and closes the gRPC channel.
        """
        self.is_running = False
        if self.channel:
            self.channel.close()
        print("[Broker] Connection closed.")
