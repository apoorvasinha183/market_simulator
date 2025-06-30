
import grpc
import threading

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

    def _listen_for_updates(self, stream):
        """
        Private method to run in a separate thread, listening for messages from the server.
        """
        print("[Broker] Listening for messages from the server...")
        try:
            for update in stream:
                # For now, we just print the received updates
                print(f"[Broker] Received update: {update}")
        except grpc.RpcError as e:
            if self.is_running:
                print(f"[Broker] Error listening for updates: {e.status()}")

    def run(self):
        """
        Establishes the connection and starts the listener thread.
        """
        address = f"{self.host}:{self.port}"
        print(f"[Broker] Connecting to gRPC server at {address}...")

        try:
            # Create a gRPC channel and a stub
            self.channel = grpc.insecure_channel(address)
            self.stub = market_gateway_pb2_grpc.MarketGatewayStub(self.channel)
            self.is_running = True

            # For a bidirectional stream, we need to pass an iterator
            # that will yield messages to send to the server.
            # For now, we'll use an empty iterator as we are not sending anything yet.
            def empty_iterator():
                yield from ()

            # Call the EventStream RPC. This returns an iterator for server messages.
            server_stream = self.stub.EventStream(empty_iterator())
            print("[Broker] gRPC connection successful. Event stream established.")

            # Start a background thread to listen for server updates
            listener_thread = threading.Thread(target=self._listen_for_updates, args=(server_stream,))
            listener_thread.daemon = True
            listener_thread.start()

            # Keep the main thread alive to maintain the connection
            # In a real application, this would be a more sophisticated loop.
            print("[Broker] Running. Press Ctrl+C to stop.")
            while self.is_running:
                threading.Event().wait(1)

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

