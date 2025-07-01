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

    def _spam_orders(self, volume, num_iterations):
        """
        Background thread: enqueue a fixed number of market-buy orders.
        """
        submit_order_request = market_gateway_pb2.SubmitOrderRequest(
            client_id="customer_agent_0",
            stock_id=1,
            side="Buy",
            price=0.0,          # Market order
            volume=volume,
            order_type="Market",
        )
        from_python_message = market_gateway_pb2.FromPython(
            submit_order=submit_order_request
        )

        print(f"[Broker] 🔫  Spam thread started (volume={volume}, iterations={num_iterations})")
        for i in range(num_iterations):
            self.outgoing_messages.put(from_python_message)
            # Introduce a random delay between 0.5 and 2 seconds
            print(f"Sending {i+1}-th order")
            time.sleep(random.uniform(0.5, 10))
        print("[Broker] Spam thread finished.")

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

            # Launch spam thread with fixed parameters
            spam_thread = threading.Thread(
                target=self._spam_orders, args=(1000000000, 200,), daemon=True
            )
            spam_thread.start()
            print("[Broker] Started deterministic order spamming.")

            # Keep main thread alive until spamming is done
            while spam_thread.is_alive():
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
