import grpc
import threading
import queue
import time
import subprocess
import os
import signal

from market_gateway_client.generated import market_gateway_pb2
from market_gateway_client.generated import market_gateway_pb2_grpc

class NonInteractiveBroker:
    """A modified version of the Broker for automated testing."""
    def __init__(self, host="localhost", port=50051):
        self.host = host
        self.port = port
        self.channel = None
        self.stub = None
        self.is_running = False
        self.outgoing_messages = queue.Queue()
        self.incoming_messages = queue.Queue()
        self.server_process = None

    def _listen_for_updates(self, stream):
        try:
            for update in stream:
                self.incoming_messages.put(update)
        except grpc.RpcError:
            # Expected when the server shuts down
            pass

    def _generate_requests(self):
        while self.is_running:
            try:
                message = self.outgoing_messages.get(timeout=0.1)
                yield message
            except queue.Empty:
                continue

    def start_server_and_connect(self):
        # 1. Build the server
        print("Building the Rust gRPC server...")
        build_process = subprocess.run(["cargo", "build", "--bin", "grpc_server"], capture_output=True, text=True)
        if build_process.returncode != 0:
            print("Error building server:", build_process.stderr)
            raise RuntimeError("Failed to build gRPC server.")

        # 2. Run the server as a background process
        print("Starting the gRPC server...")
        self.server_process = subprocess.Popen(["./target/debug/grpc_server"], stdin=subprocess.PIPE)
        time.sleep(3) # Give the server a moment to start up

        # 3. Connect the client
        address = f"{self.host}:{self.port}"
        self.channel = grpc.insecure_channel(address)
        self.stub = market_gateway_pb2_grpc.MarketGatewayStub(self.channel)
        self.is_running = True

        server_stream = self.stub.EventStream(self._generate_requests())
        
        listener_thread = threading.Thread(
            target=self._listen_for_updates, args=(server_stream,), daemon=True
        )
        listener_thread.start()
        print("Broker connected.")

    def stop(self):
        print("Stopping broker and server...")
        self.is_running = False
        if self.channel:
            self.channel.close()
        if self.server_process:
            # Close stdin to signal the server to shut down gracefully
            if self.server_process.stdin:
                self.server_process.stdin.close()
            self.server_process.wait(timeout=5)
        print("Broker and server stopped.")

    def submit_order(self, volume):
        request = market_gateway_pb2.SubmitOrderRequest(
            client_id="test_client",
            stock_id=1,
            side="Buy",
            volume=volume,
            order_type="Market",
        )
        self.outgoing_messages.put(market_gateway_pb2.FromPython(submit_order=request))

# --- The Test --- 
def test_e2e_order_submission():
    broker = NonInteractiveBroker()
    try:
        broker.start_server_and_connect()
        
        # Submit an order
        test_volume = 123
        broker.submit_order(test_volume)
        print(f"Submitted order for {test_volume} shares.")

        # Wait for the acknowledgement
        try:
            ack = broker.incoming_messages.get(timeout=5) # 5 second timeout
            print(f"Received acknowledgement: {ack}")
            
            # Assert the details of the acknowledgement
            assert ack.order_ack is not None
            assert ack.order_ack.status == "Accepted"
            assert "Buy order sent to market" in ack.order_ack.details

        except queue.Empty:
            assert False, "Test failed: Did not receive an order acknowledgement in time."

    finally:
        broker.stop()
