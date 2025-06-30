// src/agents/customer_agent.rs

use std::sync::{Arc, Mutex};
use std::thread;
use crossbeam_channel::{Receiver, Sender};
use tokio::runtime::Runtime;
use tokio::sync::mpsc as tokio_mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{transport::Server, Request, Response, Status, Streaming};

use crate::agents::agent_trait::{Agent, MarketView};
use crate::simulation::orchestra::ShadowBookHandle;
use crate::types::order::{Order, OrderRequest, Side, Trade};

// This is the namespace that tonic-build creates from our .proto file
pub mod market_gateway {
    tonic::include_proto!("market_gateway");
}

use market_gateway::market_gateway_server::{MarketGateway, MarketGatewayServer};
use market_gateway::{FromPython, ToPython, OrderAck, TradeUpdate};

// The gRPC server implementation.
// It holds a sender to pass incoming orders to the agent's main logic.
#[derive(Clone)]
pub struct CustomerAgentServer {
    order_request_sender: Sender<OrderRequest>,
    agent_id: usize,
}

#[tonic::async_trait]
impl MarketGateway for CustomerAgentServer {
    type EventStreamStream = ReceiverStream<Result<ToPython, Status>>;

    async fn event_stream(
        &self,
        request: Request<Streaming<FromPython>>,
    ) -> Result<Response<Self::EventStreamStream>, Status> {
        println!("[CustomerAgent {}] Python client connected!", self.agent_id);
        let mut stream = request.into_inner();

        // This channel will be used to send events (Acks, Trades) back to the Python client.
        let (tx, rx) = tokio_mpsc::channel(100);

        // --- Task to handle incoming messages from Python ---
        let sender_clone = self.order_request_sender.clone();
        let agent_id_clone = self.agent_id;
        tokio::spawn(async move {
            while let Some(result) = stream.message().await.ok().flatten() {
                if let Some(event) = result.event {
                    match event {
                        market_gateway::from_python::Event::SubmitOrder(req) => {
                            println!("[CustomerAgent {}] Received order from Python client: {:?}", agent_id_clone, req);
                            // Convert the gRPC request to the simulation's internal OrderRequest
                            let order_type = if req.order_type.to_lowercase() == "market" {
                                OrderRequest::MarketOrder {
                                    agent_id: agent_id_clone,
                                    stock_id: req.stock_id,
                                    side: if req.side.to_lowercase() == "buy" { Side::Buy } else { Side::Sell },
                                    volume: req.volume,
                                }
                            } else {
                                OrderRequest::LimitOrder {
                                    agent_id: agent_id_clone,
                                    stock_id: req.stock_id,
                                    side: if req.side.to_lowercase() == "buy" { Side::Buy } else { Side::Sell },
                                    price: (req.price * 100.0) as u64, // Convert to cents
                                    volume: req.volume,
                                }
                            };
                            // Send it to the main market
                            sender_clone.send(order_type).unwrap();
                        }
                    }
                }
            }
        });

        // For now, we just return the stream receiver. The agent's main loop
        // will be responsible for pushing messages into `tx`.
        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

// The main struct for our agent, which will be managed by the Orchestra.
#[derive(Clone)]
pub struct CustomerAgent {
    id: usize,
    order_channel: Sender<OrderRequest>,
    ack_channel: Arc<Mutex<Receiver<Order>>>,
    port_channel: Arc<Mutex<Receiver<Trade>>>,
    // These fields are not used by this agent but are required by the trait.
    view_handle: ShadowBookHandle,
    open_orders: Arc<Mutex<Vec<Order>>>,
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
            view_handle,
            open_orders: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

// Implementation of the synchronous Agent trait.
impl Agent for CustomerAgent {
    fn run(&mut self) {
        println!("[CustomerAgent {}] Starting...", self.id);

        // Create a new Tokio runtime within this thread.
        let rt = Runtime::new().unwrap();

        // Enter the runtime context.
        rt.block_on(async {
            let addr = "[::1]:50051".parse().unwrap();
            let server = CustomerAgentServer {
                order_request_sender: self.order_channel.clone(),
                agent_id: self.id,
            };

            println!("[CustomerAgent {}] gRPC server listening on {}", self.id, addr);

            // Start the gRPC server.
            // This will block the async task, but the `run` method will return,
            // allowing the Orchestra to continue. The server itself runs on the
            // background threads of the tokio runtime.
            if let Err(e) = Server::builder()
                .add_service(MarketGatewayServer::new(server))
                .serve(addr)
                .await
            {
                eprintln!("[CustomerAgent {}] Error running gRPC server: {}", self.id, e);
            }
        });
    }

    // --- The rest of the trait methods are stubs for now ---

    fn decide_actions(&mut self) {
        // This agent is reactive, it doesn't decide actions on its own.
        // It only reacts to incoming gRPC requests.
        // We can use this method to poll for updates and send them to the client.
        thread::sleep(std::time::Duration::from_millis(100));
    }

    fn buy_stock(&mut self, _stock_id: u64, _volume: u64) { /* No-op */ }
    fn sell_stock(&mut self, _stock_id: u64, _volume: u64) { /* No-op */ }
    fn acknowledge_order(&mut self) { /* No-op */ }
    fn margin_call(&mut self) { /* No-op */ }
    fn update_portfolio(&mut self) { /* No-op */ }
    fn evaluate_port(&mut self, _market_view: &MarketView) -> f64 { 0.0 }
    fn get_pending_orders(&self) -> Vec<Order> { vec![] }
    fn cancel_open_order(&mut self, _order_id: u64) { /* No-op */ }
    fn get_id(&self) -> usize { self.id }
    fn get_inventory(&self) -> i64 { 0 }
    fn clone_agent(&self) -> Box<dyn Agent> { Box::new(self.clone()) }
}
