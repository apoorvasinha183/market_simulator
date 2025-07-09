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
        self.incoming_updates_queue = queue.Queue()

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
                self.incoming_updates_queue.put(update)
        except grpc.RpcError as e:
            if self.is_running:
                print(f"[Broker] Error listening for updates: {e}")

    def _generate_requests(self):
        """
        Generator that yields messages from outgoing_messages.
        """
        while self.is_running:
            try:
                message = self.outgoing_messages.get(timeout=1)
                print(f"[Broker] _generate_requests yielding message for stock {message.submit_order.stock_id}")
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

    def get_raw_update(self, block=True, timeout=1.0):
        try:
            return self.incoming_updates_queue.get(block=block, timeout=timeout)
        except queue.Empty:
            return None

    # ──────────────────────────────────────────────────────────────────────────
    # public API
    # ──────────────────────────────────────────────────────────────────────────
    def connect(self):
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

        except grpc.RpcError as e:
            print(f"[Broker] Failed to connect to gRPC server: {e.status()}")

    def stop(self):
        """
        Stops the broker and closes the gRPC channel.
        """
        self.is_running = False
        if self.channel:
            self.channel.close()
        print("[Broker] Connection closed.")
