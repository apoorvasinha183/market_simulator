// src/agents/customer_agent.rs

use crossbeam_channel::{Receiver, Sender};
use dashmap::DashMap;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::thread;
use tokio::runtime::Runtime;
use tokio::sync::mpsc as tokio_mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, Streaming, transport::Server};

use crate::agents::agent_trait::Agent;
use crate::simulation::orchestra::{MarketState, ShadowBookHandle};
use crate::types::order::{Order, OrderRequest, Side, Trade};

// This is the namespace that tonic-build creates from our .proto file
pub mod market_gateway {
    tonic::include_proto!("market_gateway");
}

use market_gateway::market_gateway_server::{MarketGateway, MarketGatewayServer};
use market_gateway::{FromPython, OrderAck, ToPython, TradeUpdate};

// The gRPC server implementation.
#[derive(Clone)]
pub struct CustomerAgentServer {
    order_request_sender: Sender<OrderRequest>,
    client_id_queues: Arc<DashMap<u64, Mutex<VecDeque<String>>>>,
    agent_id: usize,
    grpc_response_sender: Arc<Mutex<Option<tokio::sync::mpsc::Sender<Result<ToPython, Status>>>>>,
}

#[tonic::async_trait]
impl MarketGateway for CustomerAgentServer {
    type EventStreamStream = ReceiverStream<Result<ToPython, Status>>;

    async fn event_stream(
        &self,
        request: Request<Streaming<FromPython>>,
    ) -> Result<Response<Self::EventStreamStream>, Status> {
        println!(
            "
[CustomerAgent Server] gRPC HANDLER CALLED. A client has connected.
"
        );
        let mut stream = request.into_inner();
        let (tx, rx) = tokio_mpsc::channel(10000);
        self.grpc_response_sender
            .lock()
            .unwrap()
            .replace(tx.clone()); // Store the sender

        // --- Task to handle incoming messages from Python ---
        let sender_clone = self.order_request_sender.clone();
        let agent_id_clone = self.agent_id;
        let queues_clone = self.client_id_queues.clone();

        tokio::spawn(async move {
            //println!("[CustomerAgent Server] TASK SPAWNED to listen for messages from Python client.");
            //println!("[CustomerAgent Server] Task spawned to handle incoming Python messages.");
            while let Some(result) = stream.message().await.ok().flatten() {
                //println!("[CustomerAgent Server] Received a message from gRPC stream.");
                if let Some(event) = result.event {
                    match event {
                        market_gateway::from_python::Event::SubmitOrder(req) => {
                            //println!(
                            //    "[CustomerAgent Server] Received SubmitOrder from Python: client_id={}, stock_id={}, side={}, type={}, vol={}",
                            //    req.client_id, req.stock_id, req.side, req.order_type, req.volume
                            //);
                            let side = match req.side.as_str() {
                                "Buy" => Side::Buy,
                                "Sell" => Side::Sell,
                                _ => {
                                    eprintln!(
                                        "[CustomerAgent Server] Invalid side received: {}",
                                        req.side
                                    );
                                    return;
                                }
                            };

                            let order_request = match req.order_type.as_str() {
                                "Market" => OrderRequest::MarketOrder {
                                    order_id: 0,
                                    agent_id: agent_id_clone,
                                    stock_id: req.stock_id,
                                    side,
                                    volume: req.volume,
                                },
                                "Limit" => {
                                    let price_in_cents = (req.price * 100.0).round() as u64;
                                    OrderRequest::LimitOrder {
                                        order_id: 0,
                                        agent_id: agent_id_clone,
                                        stock_id: req.stock_id,
                                        side,
                                        price: crate::agents::quantize_price(price_in_cents),
                                        volume: req.volume,
                                    }
                                }
                                _ => {
                                    eprintln!(
                                        "[CustomerAgent Server] Invalid order_type received: {}",
                                        req.order_type
                                    );
                                    return;
                                }
                            };

                            // HANDSHAKE STEP 1: Push the python client_id to the queue for this stock
                            //println!(
                            //    "[CustomerAgent Server] Pushing client_id {} for stock_id {} to queue.",
                            //    req.client_id, req.stock_id
                            //);
                            queues_clone
                                .entry(req.stock_id)
                                .or_default()
                                .lock()
                                .unwrap()
                                .push_back(req.client_id.clone());

                            //println!("[CustomerAgent Server] Sending order request to internal market channel.");
                            if let Err(e) = sender_clone.send(order_request.clone()) {
                                eprintln!(
                                    "[CustomerAgent Server] FAILED to send order to internal market channel: {}",
                                    e
                                );
                            } else {
                                //println!("[CustomerAgent Server] Successfully sent order to internal market channel.");
                            }
                        }
                    }
                }
            }
            eprintln!(
                "[CustomerAgent Server] Incoming gRPC stream from Python ended or errored unexpectedly."
            );
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

// The main struct for our agent.
#[derive(Clone)]
pub struct CustomerAgent {
    id: usize,
    order_channel: Sender<OrderRequest>,
    ack_channel: Arc<Mutex<Receiver<Order>>>,
    port_channel: Arc<Mutex<Receiver<Trade>>>,
    _view_handle: ShadowBookHandle,
    _open_orders: Arc<Mutex<Vec<Order>>>,

    // NEW STATE MANAGEMENT
    order_id_to_client_id: Arc<DashMap<u64, String>>,
    client_id_queues: Arc<DashMap<u64, Mutex<VecDeque<String>>>>,

    // Holds the sender for the single gRPC response stream back to Python
    grpc_response_sender: Arc<Mutex<Option<tokio::sync::mpsc::Sender<Result<ToPython, Status>>>>>,
}

impl CustomerAgent {
    pub fn new(
        id: usize,
        order_channel: Sender<OrderRequest>,
        ack_channel: Receiver<Order>,
        port_channel: Receiver<Trade>,
        view_handle: ShadowBookHandle,
    ) -> Self {
        Self {
            id,
            order_channel,
            ack_channel: Arc::new(Mutex::new(ack_channel)),
            port_channel: Arc::new(Mutex::new(port_channel)),
            _view_handle: view_handle,
            _open_orders: Arc::new(Mutex::new(Vec::new())),
            order_id_to_client_id: Arc::new(DashMap::new()),
            client_id_queues: Arc::new(DashMap::new()),
            grpc_response_sender: Arc::new(Mutex::new(None)),
        }
    }
}

impl Agent for CustomerAgent {
    fn run(&mut self) {
        //println!("[CustomerAgent {}] Starting...", self.id);
        let rt = Runtime::new().unwrap();

        let ack_rx_clone = self.ack_channel.clone();
        let map_clone = self.order_id_to_client_id.clone();
        let queues_clone = self.client_id_queues.clone();
        let response_sender_clone = self.grpc_response_sender.clone();

        thread::spawn(move || {
            let rx = ack_rx_clone.lock().unwrap();
            //println!("[CustomerAgent ACK Listener] Waiting for ACKs...");
            while let Ok(order_ack) = rx.recv() {
                //println!("[CustomerAgent ACK Listener] Received ACK: {:?}", order_ack);
                if let Some(queue_lock) = queues_clone.get(&order_ack.stock_id) {
                    //println!("[CustomerAgent ACK Listener] Found queue for stock_id {}.", order_ack.stock_id);
                    if let Some(client_id) = queue_lock.lock().unwrap().pop_front() {
                        //println!("[CustomerAgent ACK Listener] Popped client_id {} from queue for stock_id {}.", client_id, order_ack.stock_id);
                        // Create the permanent mapping
                        map_clone.insert(order_ack.id, client_id.clone());
                        //println!("[CustomerAgent ACK Listener] Mapped order_id {} to client_id {}. Current map size: {}. Map content: {:?}", order_ack.id, client_id, map_clone.len(), map_clone);

                        let ack_msg = OrderAck {
                            client_id: client_id.clone(),
                            order_id: order_ack.id,
                            status: "Confirmed".to_string(),
                            details: "Order confirmed by market".to_string(),
                        };
                        let response_msg = ToPython {
                            event: Some(market_gateway::to_python::Event::OrderAck(ack_msg)),
                        };
                        //println!("[CustomerAgent ACK Listener] Attempting to send OrderAck for order_id {} to gRPC stream...", order_ack.id);
                        if let Some(sender) = response_sender_clone.lock().unwrap().as_ref() {
                            if let Err(e) = sender.blocking_send(Ok(response_msg)) {
                                eprintln!(
                                    "[CustomerAgent ACK Listener] Failed to send OrderAck for order_id {} to gRPC stream: {:?}",
                                    order_ack.id, e
                                );
                            } else {
                                //println!("[CustomerAgent ACK Listener] Successfully sent OrderAck for order_id {} to client {}.", order_ack.id, client_id);
                            }
                        } else {
                            eprintln!(
                                "[CustomerAgent ACK Listener] No gRPC sender available for OrderAck."
                            );
                        }
                    } else {
                        //println!("[CustomerAgent ACK Listener] Queue for stock_id {} was empty, but received ACK. This should not happen.", order_ack.stock_id);
                    }
                } else {
                    //println!("[CustomerAgent ACK Listener] No queue found for stock_id {}. This should not happen.", order_ack.stock_id);
                }
            }
            //println!("[CustomerAgent ACK Listener] ACK channel closed.");
        });

        // --- Spawn Trade/Fill Listener Thread ---
        let agent_id_for_trade_listener = self.id; // Clone self.id here
        let trade_rx_clone = self.port_channel.clone();
        let map_clone_trade = self.order_id_to_client_id.clone();
        let response_sender_clone_trade = self.grpc_response_sender.clone();

        thread::spawn(move || {
            let rx = trade_rx_clone.lock().unwrap();
            //println!("[CustomerAgent Trade Listener] Waiting for trades...");
            while let Ok(trade) = rx.recv() {
                //println!("[CustomerAgent Trade Listener] Received trade: {:?}", trade);

                let mut client_id_to_send: Option<String> = None;
                let mut order_id_to_send: u64 = 0;

                // Check if our agent was the taker
                if trade.taker_agent_id == agent_id_for_trade_listener {
                    // Use cloned id
                    //println!("[CustomerAgent Trade Listener] Our agent was the taker. Attempting lookup for taker_order_id: {}.", trade.taker_order_id);
                    if let Some(entry) = map_clone_trade.get(&trade.taker_order_id) {
                        client_id_to_send = Some(entry.value().clone());
                        order_id_to_send = trade.taker_order_id;
                        //println!("[CustomerAgent Trade Listener] Found client_id {} for taker_order_id {}.", client_id_to_send.as_ref().unwrap(), trade.taker_order_id);
                    } else {
                        //println!("[CustomerAgent Trade Listener] No client_id found for taker_order_id: {}. Lookup failed.", trade.taker_order_id);
                    }
                }

                // If not found as taker, check if our agent was the maker
                if client_id_to_send.is_none()
                    && trade.maker_agent_id == agent_id_for_trade_listener
                {
                    // Use cloned id
                    //println!("[CustomerAgent Trade Listener] Our agent was the maker. Attempting lookup for maker_order_id: {}.", trade.maker_order_id);
                    if let Some(entry) = map_clone_trade.get(&trade.maker_order_id) {
                        client_id_to_send = Some(entry.value().clone());
                        order_id_to_send = trade.maker_order_id;
                        //println!("[CustomerAgent Trade Listener] Found client_id {} for maker_order_id {}.", client_id_to_send.as_ref().unwrap(), trade.maker_order_id);
                    } else {
                        //println!("[CustomerAgent Trade Listener] No client_id found for maker_order_id: {}. Lookup failed.", trade.maker_order_id);
                    }
                }

                if let Some(client_id) = client_id_to_send {
                    let trade_msg = TradeUpdate {
                        client_id: client_id.clone(), // Clone client_id here
                        order_id: order_id_to_send,
                        stock_id: trade.stock_id,
                        price: trade.price as f64 / 100.0,
                        volume_filled: trade.volume,
                        new_total_filled: 0,    // Placeholder
                        is_fully_filled: false, // Placeholder
                    };
                    let response_msg = ToPython {
                        event: Some(market_gateway::to_python::Event::TradeUpdate(
                            trade_msg.clone(),
                        )),
                    };
                    //println!("[CustomerAgent Trade Listener] Attempting to send TradeUpdate for order_id {} to gRPC stream...", order_id_to_send);
                    if let Some(sender) = response_sender_clone_trade.lock().unwrap().as_ref() {
                        if let Err(e) = sender.blocking_send(Ok(response_msg)) {
                            eprintln!(
                                "[CustomerAgent Trade Listener] Failed to send trade update to gRPC stream: {:?}",
                                e
                            );
                        } else {
                            //println!("[CustomerAgent Trade Listener] Successfully sent TradeUpdate for order_id {} to client {}.", order_id_to_send, trade_msg.client_id);
                        }
                    } else {
                        eprintln!(
                            "[CustomerAgent Trade Listener] No gRPC sender available for TradeUpdate."
                        );
                    }
                } else {
                    //println!("[CustomerAgent Trade Listener] No client_id found for either taker_order_id or maker_order_id. Skipping TradeUpdate.");
                }
            }
            //println!("[CustomerAgent Trade Listener] Trade channel closed.");
        });

        // --- Spawn gRPC Server ---
        rt.block_on(async {
            let addr = "0.0.0.0:50051".parse().unwrap();
            let server = CustomerAgentServer {
                order_request_sender: self.order_channel.clone(),
                client_id_queues: self.client_id_queues.clone(),
                agent_id: self.id,
                grpc_response_sender: self.grpc_response_sender.clone(),
            };

            println!(
                "[CustomerAgent {}] gRPC server listening on {}",
                self.id, addr
            );

            if let Err(e) = Server::builder()
                .add_service(MarketGatewayServer::new(server))
                .serve(addr)
                .await
            {
                eprintln!(
                    "[CustomerAgent {}] Error running gRPC server: {}",
                    self.id, e
                );
            }
        });
    }

    // --- The rest of the trait methods are stubs for now ---
    fn decide_actions(&mut self) {
        thread::sleep(std::time::Duration::from_millis(100));
    }
    fn buy_stock(&mut self, _stock_id: u64, _volume: u64) {}
    fn sell_stock(&mut self, _stock_id: u64, _volume: u64) {}
    fn acknowledge_order(&mut self) {}
    fn margin_call(&mut self) {}
    fn update_portfolio(&mut self) {}
    fn evaluate_port(&mut self, _market_view: &MarketState) -> f64 {
        0.0
    }
    fn get_pending_orders(&self) -> Vec<Order> {
        vec![]
    }
    fn cancel_open_order(&mut self, _order_id: u64) {}
    fn get_id(&self) -> usize {
        self.id
    }
    fn get_inventory(&self) -> i64 {
        0
    }
    fn clone_agent(&self) -> Box<dyn Agent> {
        Box::new(self.clone())
    }
}
