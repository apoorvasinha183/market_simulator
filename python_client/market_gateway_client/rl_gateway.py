import threading
import queue
import uuid
from collections import defaultdict
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
            self._l1_data = {}
            self._l1_data_lock = threading.Lock()
            self._agent_state = defaultdict(lambda: {'cash': 1_000_000.0, 'inventory': defaultdict(int)})
            self._pending_orders = {}
            self._state_lock = threading.Lock() # Lock for _agent_state and _pending_orders

            self.broker.connect()

            self._dispatcher_thread = threading.Thread(target=self._dispatch_messages, daemon=True)
            self._dispatcher_thread.start()

            self.is_initialized = True

    def register_agent(self):
        agent_id = str(uuid.uuid4())
        with self._state_lock:
            self._agents[agent_id] = queue.Queue()
            # Initialize cash and inventory for the new agent
            self._agent_state[agent_id]['cash'] = 1_000_000.0 # Starting cash
            self._agent_state[agent_id]['inventory'] = defaultdict(int)
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

    def get_l1_data(self, stock_id):
        with self._l1_data_lock:
            return self._l1_data.get(stock_id)

    def evaluate_portfolio(self, agent_id):
        with self._state_lock:
            agent_data = self._agent_state[agent_id]
            cash = agent_data['cash']
            inventory_value = 0.0

            for stock_id, quantity in agent_data['inventory'].items():
                l1_data = self.get_l1_data(stock_id)
                if l1_data:
                    # Use mid-price for valuation if available, otherwise last traded
                    if l1_data.best_bid_price > 0 and l1_data.best_ask_price > 0:
                        mid_price = (l1_data.best_bid_price + l1_data.best_ask_price) / 2.0
                    else:
                        mid_price = l1_data.last_traded_price
                    inventory_value += quantity * mid_price
                # else: if no L1 data, that stock's value is not added to portfolio
            return cash + inventory_value

    def _dispatch_messages(self):
        while self._running:
            message = self.broker.get_raw_update(block=True, timeout=1)
            if message is None:
                continue

            event_type = message.WhichOneof('event')

            with self._state_lock: # Lock for state updates
                if event_type == "order_ack":
                    ack = message.order_ack
                    client_id = ack.client_id
                    if client_id in self._agents:
                        # Store pending order details for later trade processing
                        self._pending_orders[ack.order_id] = {
                            'stock_id': ack.stock_id,
                            'side': ack.side,
                            'volume': ack.volume,
                            'filled_volume': 0 # Track filled volume for this order
                        }
                        self._agents[client_id].put(ack)

                elif event_type == "trade_update":
                    trade = message.trade_update
                    client_id = trade.client_id
                    if client_id in self._agents:
                        agent_data = self._agent_state[client_id]
                        
                        # Retrieve original order details
                        original_order = self._pending_orders.get(trade.order_id)
                        if original_order:
                            stock_id = original_order['stock_id']
                            side = original_order['side']
                            volume_filled = trade.volume_filled
                            trade_price = trade.price

                            # Update cash and inventory
                            if side == "Buy":
                                agent_data['cash'] -= volume_filled * trade_price
                                agent_data['inventory'][stock_id] += volume_filled
                            elif side == "Sell":
                                agent_data['cash'] += volume_filled * trade_price
                                agent_data['inventory'][stock_id] -= volume_filled
                            
                            # Update filled volume for the pending order
                            original_order['filled_volume'] += volume_filled
                            if original_order['filled_volume'] >= original_order['volume']:
                                del self._pending_orders[trade.order_id] # Order fully filled

                        self._agents[client_id].put(trade)

                elif event_type == "market_update":
                    with self._l1_data_lock:
                        self._l1_data[message.market_update.stock_id] = message.market_update

    def shutdown(self):
        self._running = False
        self.broker.stop()
        self._dispatcher_thread.join()