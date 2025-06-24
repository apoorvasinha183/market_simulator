/*
This should be the first file for you to refer to when you want to understand how the market simulation works.
This file is the central place where all the moving parts of the market i.e.
agents, stocks and market are initialized and run.
Right now it simply runs the market simulations by invoking the initis ,
and then the .run methods in every object.
Let me break it down :
1. All stock instruments in the instrument universe are initialized. They are derived from a common csv file .
There is a parallel process that will generate stock sentiment . Every ticker will pub;ish their sentiment on a port.
2. Creates as many crossbeam channels to enable message passing between threads . There is a single mpsc channel 
where each agent submits their actions to the market and the market recieves them. The market also establishes 
a private channel with each agent where it sends them the acknowledgements or force a protfolio update. 
3. In the run routine all the pertual functions in agents and markets are invoked. This is achieved by thread splitting.

4. TODO : Expose a method for an external python based agent to connect to the market . I will create a new kind of agent
that specializes in this bullshit.
5. TODO: hANDLER managementfor the threads .
*/
/* 
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;    

use crate::agents::agent_trait::{Agent, MarketView};
use crate::agents::dumb_agent::DumbAgent;
use crate::agents::ipo_agent::IpoAgent;
use crate::agents::market_maker_agent::MarketMakerAgent;
use crate::agents::whale_agent::WhaleAgent;
use crate::market::Market;
use crate::stocks::{default_stock_universe, StockMarket};   
use crate::types::order::{Order, OrderRequest};
use crate::types::order::Side;
use crate::types::order::Trade;
use crate::types::order_book::OrderBook;
use crate::simulators::market_trait::Marketable;
use crate::simulators::order_book::OrderBook as OrderBookSimulator;
use crate::simulators::gbm::GBMSimulator;
use crate::pricing::{Greeks, OptionPricer};
*/
use crossbeam_channel::{unbounded, Sender};
pub struct Orchestra {
    //market: Market,
    //agents: Vec<Box<dyn Agent>>,
    //order_books: HashMap<u64, OrderBook>,
}
impl Orchestra {
    // Initiate the orchetra with predefined agents and the market ,
    // We also create the common channel for agents to submit their acctions to the market . An unbounded crossbeam channel
    // Market is the recepient of that channel and therefore we have to pass rx to the market and tx's to the agents.
    // The market in turn also establishes a private channl where it will sent the agents the acknowledgemnts of the order 
    // and the portfolio updates if applicable.

    // Step 1 : Refactor the  agents trait to accept an outboudn channel to the market and a private channel .
    // Step 2 : Refactor the market to accept the inbound orders channels and the private channels to send the agents.
    // Step 3 : The market should maintain the tx channels for each agent by id perhaps to simplify the lookup for sending the 
    pub fn new() -> Self {
        // Open a channel for agents to submit their actions to the market
        let (tx_market, rx_market) = unbounded::<OrderRequest>();
    }

}
