import threading
import queue
import uuid
from .broker import Broker

class RLGateway:
    _instance = None
    _lock = threading.Lock()

    def __new__(cls, *args, **kwargs):
        if not cls._instance:
            with cls._lock:
                if not cls._instance:
                    cls._instance = super(RLGateway, cls).__new__(cls)
        return cls._instance

    def __init__(self, host="localhost", port=50051):
        if not hasattr(self, 'is_initialized'):
            self.broker = Broker(host, port)
            self._agents = {}
            self._running = True
            
            self.broker.connect()
            
            self._dispatcher_thread = threading.Thread(target=self._dispatch_messages, daemon=True)
            self._dispatcher_thread.start()
            
            self.is_initialized = True

    def register_agent(self):
        agent_id = str(uuid.uuid4())
        self._agents[agent_id] = queue.Queue()
        print(f"[Gateway] Registered new agent: {agent_id}")
        return agent_id

    def submit_order(self, agent_id, stock_id, side, order_type, volume, price=0.0):
        if agent_id not in self._agents:
            raise ValueError("Agent not registered.")
        self.broker.send_order(agent_id, stock_id, side, order_type, volume, price)

    def get_update(self, agent_id, block=True, timeout=None):
        if agent_id not in self._agents:
            raise ValueError("Agent not registered.")
        try:
            return self._agents[agent_id].get(block=block, timeout=timeout)
        except queue.Empty:
            return None

    def _dispatch_messages(self):
        while self._running:
            message = self.broker.get_raw_update(block=True, timeout=1)
            if message is None:
                continue

            event_type = message.WhichOneof('event')
            
            if event_type == "order_ack":
                client_id = message.order_ack.client_id
                if client_id in self._agents:
                    self._agents[client_id].put(message.order_ack)
            
            elif event_type == "trade_update":
                client_id = message.trade_update.client_id
                if client_id in self._agents:
                    self._agents[client_id].put(message.trade_update)

            elif event_type == "market_update":
                for agent_queue in self._agents.values():
                    agent_queue.put(message.market_update)

    def shutdown(self):
        self._running = False
        self.broker.stop()
        self._dispatcher_thread.join()
