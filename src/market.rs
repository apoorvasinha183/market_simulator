// src/market.rs
/* 
                                          ::  .::                                                   
 =======----==-==============++=-..@@@@@@@@@+@@@@@@@@@@@@%=  :=++++++++++++++++++++++++++++++++++++ 
 ============-====-=========-:   @@@#+#@-:#%@+=+#*++@*@%@@@@@@..:=+++++++++++++++++++++++++++++++++ 
 --=-===============-=====-. *@@%@*#=%*=@* %#@%#@@#*+@-#+@*#@%@@@.-++++++++++++++++++++++++++++++++ 
 --=-==-=======-========-: #@@@@+#@%@@*%#@%##@@%@@@@@@@@@@@@@*@%@@*-+++++++++++++++++++++++++++++++ 
 =----===========---====:=@@+=%@@@@@*--#@+#*++:+=+**+*%@@@@@@@@@@@@*-++++++++++++++++++++++++++++++ 
 --=------=-=======-===. @++++--.=::-+#**+=:::-=+-:==-:-=--+---:: %@=-=++++++++++++++==+++=++++++++ 
 ----=----=-========--- @@@@@@@@@@@@@%@*-.::--:.=+++=+*+++++++=.:. .@#-=++++++++++++++++++++==+++++ 
 ---=--==------==--===.%@*+*=+:.-*#%%#+*-:=-:--:.:++=====++++**:=:   @%=+++++=+++++++++++++++++==++ 
 ---=----===-----====- @@*%%##%#@@@%@*:=#:-:=+=*+--*###*#*%%%##=+==.  @*==+++++====+++++++++++++++= 
 -----------=------==:-@##=#@%###=+==:+*@*--.:-==--:=-=+******=%=+=:  +@==+=+++++++++++++++++++++++ 
 --------=-=--=------ @%#@%@#@%**###@@@.  .==+: -*%#++*=:+=+*= =#*# =. @=+++++++++++=++++++++++++++ 
 -----------=--==---- @%@%@%%#*#=*%#=   -=--:.++%@@@@@%##*+%# . .@#-+= @+=++++++++++++++=++++++++++ 
 -----------------=-- @@%%#*++#@*@*  ::==+-+#@@%=+**#%@@@@@#@%%**@ =:  .%=+++++++++++++++++++=+++++ 
 ---------------=---- @@@@%%*#+@*+- ..:=-:+:    .+**+-==#%%%@@%@@= @==  #===+++++++++++++++++++=+=+ 
 ---------------=---- @@ @@@@@%@@*::.=+++--%*%@@@@@#@@@@@#@++%#@-=@@@@@*:#==+++++++++++++++++++++++ 
 -------------------- *@   :.+#@*=------= :+##@:.:-=++*@@@@*.  *  @#%@@@@@@=++++=++++++++++++++++++ 
 ----------------==-:+% -@.   @*--....--+#%*++#%@@@%@@@@@@@*:.-#* @@#%*   #+=+++========+++++++++++ 
 -------------------.@.    .. @-=-.----.: .:---==**#%%%@#*:*=:-*% @@@@@@@--=====================+++ 
 -------------------.-#@@@@:@ @@@*--=+=+=-::--.==+**##***=--::==** @@: @ **=====================+++ 
 -------------------.*@@*:  = +@@%-=-+--==*-.-=+####**#%@@:    .:** @@-::*+======================++ 
 -------------------:* @@ : - #@@%=* .:..- +*#%*#++*#@@@# :-#@%+--#+ @#= **+======================+ 
 ----:--------------::=+.:@% .@@.-%%-=#-*=:#@+--= +@@%+++* :@%. .=%# *-=  @+++++++++=============== 
 --------------------::- .:- =@@#+*%@%=*@.#@.   #@@@%#=*+@@*-= .=@:@ @.   @=+======+++============= 
 ----------------------:%+=-*%+*.+.=#+@%*@@@+ =@@@@@@@@@@@@@@@@@..*-@@@  %#=+========+============= 
 ----------------------:*:  *@*@@=#**#=%#%@#= #@@%+. -.=***#+#%@# @ *@  %@+==========+++=========== 
 --------:-:-----------:+-. @%-*-***#-=@@@@@@#.@@@@@@*@-@%@@@@@*.-#%%@  %++++=++=======+++++++++++= 
 -------------:--------:==.:@ *-**=%%@#-*#@@*+-#@%#-*@@ -    #@@@-  @. %+-===+===+======++=======++ 
 ----------------------:+- -@:*-:-+@%*%%@@@@%%+%@%@*:-*@@@@@+    @@@@=@=+====+=+=+===+============+ 
 -----------------:-----=: +@#@=@=.-@@@#@##@@%=@@@@@@@ :. :-#@%@   @@=@.========++++=============== 
 ----------------------=-: :@+=.+@*%#+#*%@%@%*+%@%@%%@@@@@%@%#-:   @.@+-=============+=+++++++++=+= 
 --------------:----:-:+=:: @@@@%:=%%%%#@@%@@%+%@%@@#@@%@*%@@@@*#@@@ @:-==============+============ 
 ----------------------=:--. :@@@@%%+**%@@%@%*:%#%@%%###%%#%%**  ++@ @:=================+=========+ 
 ----------------------=-:.::   *@@@@@%=*%@@@%*@@#@@%#@@#@%@%@#=:*@+::===================++===++++= 
 -----------------:--:=---::::..   +@@@@@#*@@#-**#@@%@@@%%%#*%@=@%@-+ *====================+++====+ 
 -------------------:-=::-:::-=:.-.    %@@@@@%%=**#%#%@@@@%@@@@#@%@% :#============================ 
 ------------------:.+---:-::-:-++=-:-:.:-@@@@%#*=-+:*+=*%#%@%%*+ +   +============================ 
 :-:--------------:.++--.:::=:-:-=*--===-:. @@@%@##+%@+@@#**%@@*%%**=*+============================ 
 :-:-----------:.. #*--::..::-=-+--+*+++++=-:*@@@@@%@##@#@%#%@@*#=** +============================= 
 --------::.    :%**--:.--.:-:*--:---===:==--: :#@@@@@@@%#@@%%@@%@%==+============================= 
                                               .::..:::+******-=:                                   
*/
/*
Greetings, they call me CHADSDAQ . I am the Market, the heart of this trading simulation.
I manage the order books, process trades, and keep everything running smoothly.
I am the core of the financial system , it is important you reader have blind faith in my fairness and correctness.
Remember ,crying in the casino is not allowed. Good luck, and may the odds be ever in your favor.

*/ 
// Multi-ticker, multi-threaded matching engine.
// The Market is a "dumb" manager of order books, decoupled from agents.
// It receives orders via a channel and publishes updates to agents and a shadow book reconciler.
// If this file doesn't work , they will show this as an example of how to generate combinatorial deadlocks .
use std::collections::HashMap; 
use std::thread;

use crate::simulation::orchestra::{AgentResponseChannels, MarketState, ShadowBookHandle};
use crate::{
    Marketable, OrderBook,
    stocks::definitions::StockMarket,
    types::{Order, OrderRequest, Trade},
};
use crossbeam_channel::{Receiver, Sender, unbounded};

// An enum to represent the events that the main matching engine sends to the ShadowWorker.
#[derive(Debug, Clone)]
enum ShadowEvent {
    LimitOrder(Order),
    MarketOrder(Order),
    CancelOrder { order_id: u64, agent_id: usize },
    Trade(Trade),
}

// -----------------------------------------------------------------------------
//  Market
// -----------------------------------------------------------------------------
pub struct Market {
    // --- Core State (The "Source of Truth") ---
    order_books: HashMap<u64, OrderBook>, // id → book
    last_traded_price: HashMap<u64, f64>, // id → dollars
    cumulative_volume: HashMap<u64, u64>, // id → shares
    order_id_counter: u64,

    // --- Communication Infrastructure ---
    order_rx: Receiver<OrderRequest>,
    agent_channels: HashMap<usize, AgentResponseChannels>,
    shadow_update_tx: Sender<ShadowEvent>,
    vip_shadow_update_tx: Sender<ShadowEvent>, // <-- ADDED: For the rich folks
                                               //update_threshold: usize, // How many trades before we update the shadow book
}

impl Market {
    // ---------------------------------------------------------------------
    //  Construction
    // ---------------------------------------------------------------------
    pub fn new(
        stocks: &StockMarket,
        order_rx: Receiver<OrderRequest>,
        agent_channels: HashMap<usize, AgentResponseChannels>,
        shadow_book_handle: ShadowBookHandle,
        update_threshold: usize,
        vip_book_handle: ShadowBookHandle,
        vip_update_threshold: usize, // <-- ADDED: The threshold for swapping buffers.
    ) -> Self {
        /* 1. Build the Market's internal "source of truth" state */
        let mut order_books = HashMap::new();
        let mut last_traded_price = HashMap::new();
        let mut cumulative_volume = HashMap::new();

        for s in stocks.get_all_stocks() {
            order_books.insert(s.id, OrderBook::new());
            last_traded_price.insert(s.id, s.initial_price);
            cumulative_volume.insert(s.id, 0);
        }

        /* 2. CORRECTED: Time-Zero State Synchronization */
        // The Market, as the source of truth, populates the initial state of the
        // empty, agent-facing shadow book it was given by the Orchestra.
        //println!("[Market] Synchronizing initial state to the shadow book...");
        {
            // Scoped block to ensure the write lock is released immediately.
            let mut state_lock = shadow_book_handle.write().unwrap();
            state_lock.stocks = stocks.clone();
            state_lock.order_books = order_books.clone();
            state_lock.last_traded_price = last_traded_price.clone();
            state_lock.cumulative_volume = cumulative_volume.clone();
        } // Write lock is released here.
        //println!("[Market] Shadow book synchronized.");
        //println!("[Market] Synchronizing initial state to the rich people's book...");
        {
            // Scoped block to ensure the write lock is released immediately.
            let mut state_lock = vip_book_handle.write().unwrap();
            state_lock.stocks = stocks.clone();
            state_lock.order_books = order_books.clone();
            state_lock.last_traded_price = last_traded_price.clone();
            state_lock.cumulative_volume = cumulative_volume.clone();
        } // Write lock is released here.
        //println!("[Market] Vip book synchronized(meh).");

        /* 3. Create the channel for the shadow reconciliation worker */
        let (shadow_update_tx, shadow_update_rx) = unbounded::<ShadowEvent>();
        // Optional the premium folks get their own channels since the normal worker folk eat messages.
        let (vip_shadow_update_tx, vip_shadow_update_rx) = unbounded::<ShadowEvent>();
        /* 4. Spawn the ShadowWorker Thread */
        Self::spawn_shadow_worker(
            shadow_update_rx,
            shadow_book_handle.clone(),
            update_threshold,
        );
        // Rich people ..pfft
        Self::spawn_shadow_worker(vip_shadow_update_rx, vip_book_handle, vip_update_threshold);
        //println!("[Market] Shadow book reconciliation worker thread spawned.");

        Self {
            order_books,
            last_traded_price,
            cumulative_volume,
            order_id_counter: 0,
            order_rx,
            agent_channels,
            shadow_update_tx,
            vip_shadow_update_tx,
        }
    }

    /// Spawns the dedicated thread responsible for updating the public-facing shadow book.
    fn spawn_shadow_worker(
        update_rx: Receiver<ShadowEvent>,
        shadow_book_handle: ShadowBookHandle,
        update_threshold: usize, // <-- ADDED: The threshold for swapping buffers.
    ) {
        thread::spawn(move || {
            // --- Worker-Private State ---
            // 1. The BACK BUFFER: a private copy of the order books for processing.
            //    Initialized by cloning the already-synced front buffer.
            let mut back_buffer: MarketState = shadow_book_handle.read().unwrap().clone();

            // 2. A temporary log to hold events during the catch-up phase after a swap.
            let mut event_log: Vec<ShadowEvent> = Vec::with_capacity(update_threshold);

            // 3. Counter for events since the last swap.
            let mut event_counter = 0;

            //println!("[ShadowWorker] Online with update threshold: {}. Waiting for market events...", update_threshold);

            while let Ok(event) = update_rx.recv() {
                // Always log the event first.
                event_log.push(event.clone());

                // --- Apply event to the private BACK BUFFER (no locking required) ---
                match event {
                    ShadowEvent::LimitOrder(mut order) => {
                        if let Some(book) = back_buffer.order_books.get_mut(&order.stock_id) {
                            book.process_limit_order(&mut order);
                        }
                    }
                    ShadowEvent::MarketOrder(order) => {
                        if let Some(book) = back_buffer.order_books.get_mut(&order.stock_id) {
                            book.process_market_order(order.agent_id, order.side, order.volume);
                        }
                    }
                    ShadowEvent::CancelOrder { order_id, agent_id } => {
                        for book in back_buffer.order_books.values_mut() {
                            if book.cancel_order(order_id, agent_id) {
                                break;
                            }
                        }
                    }
                    ShadowEvent::Trade(trade) => {
                        if let Some(price_mut) =
                            back_buffer.last_traded_price.get_mut(&trade.stock_id)
                        {
                            *price_mut = trade.price as f64 / 100.0;
                        }
                        if let Some(vol_mut) =
                            back_buffer.cumulative_volume.get_mut(&trade.stock_id)
                        {
                            *vol_mut += trade.volume;
                        }
                    }
                }

                event_counter += 1;

                // --- Check if it's time to swap buffers ---
                if event_counter >= update_threshold {
                    //println!("[ShadowWorker] Update threshold reached. Swapping buffers...");

                    // --- Step A: Acquire lock and SWAP buffers ---
                    // The write lock is held for the shortest possible time.
                    {
                        let mut state_lock = shadow_book_handle.write().unwrap();
                        // Instantly swap our up-to-date back_buffer with the stale front_buffer.
                        // Agents now see the new state. We now hold the old, stale state.
                        std::mem::swap(&mut back_buffer, &mut *state_lock);
                    } // Lock is released here.

                    // --- Step B: Catch up the new back_buffer (the old front_buffer) ---
                    // Replay all the events we logged since the last swap onto our new back_buffer
                    // to bring it up to the current state.
                    //println!("[ShadowWorker] Replaying {} logged events to catch up...", event_log.len());
                    for logged_event in &event_log {
                        match logged_event {
                            ShadowEvent::LimitOrder(order) => {
                                if let Some(book) = back_buffer.order_books.get_mut(&order.stock_id)
                                {
                                    // Need a mutable clone to pass to process_limit_order
                                    let mut o = order.clone();
                                    book.process_limit_order(&mut o);
                                }
                            }
                            ShadowEvent::MarketOrder(order) => {
                                if let Some(book) = back_buffer.order_books.get_mut(&order.stock_id)
                                {
                                    book.process_market_order(
                                        order.agent_id,
                                        order.side,
                                        order.volume,
                                    );
                                }
                            }
                            ShadowEvent::CancelOrder { order_id, agent_id } => {
                                for book in back_buffer.order_books.values_mut() {
                                    if book.cancel_order(*order_id, *agent_id) {
                                        break;
                                    }
                                }
                            }
                            ShadowEvent::Trade(trade) => {
                                if let Some(price_mut) =
                                    back_buffer.last_traded_price.get_mut(&trade.stock_id)
                                {
                                    *price_mut = trade.price as f64 / 100.0;
                                }
                                if let Some(vol_mut) =
                                    back_buffer.cumulative_volume.get_mut(&trade.stock_id)
                                {
                                    *vol_mut += trade.volume;
                                }
                            }
                        }
                    }

                    // --- Step C: Reset for the next cycle ---
                    event_log.clear();
                    event_counter = 0;
                    //println!("[ShadowWorker] Catch-up complete. Resuming normal operation.");
                }
            }
            //println!("[ShadowWorker] Channel closed. Shutting down.");
        });
    }

    #[inline]
    fn next_order_id(&mut self) -> u64 {
        self.order_id_counter += 1;
        self.order_id_counter
    }

    /// The core logic of the matching engine for a single incoming order.
    fn process_request(&mut self, req: OrderRequest) {
        let mut trades = Vec::<Trade>::new();

        match req {
            OrderRequest::LimitOrder {
                agent_id,
                stock_id,
                side,
                price,
                volume,
            } => {
                let mut order = Order {
                    id: self.next_order_id(),
                    agent_id,
                    stock_id,
                    side,
                    price,
                    volume,
                    filled: 0,
                };
                if let Some(ch) = self.agent_channels.get(&agent_id) {
                    ch.ack_tx.send(order.clone()).unwrap();
                }
                if let Some(book) = self.order_books.get_mut(&stock_id) {
                    trades.extend(book.process_limit_order(&mut order));
                }
                self.shadow_update_tx
                    .send(ShadowEvent::LimitOrder(order))
                    .unwrap();
                self.vip_shadow_update_tx
                    .send(ShadowEvent::LimitOrder(order.clone()))
                    .unwrap();
            }
            OrderRequest::MarketOrder {
                agent_id,
                stock_id,
                side,
                volume,
            } => {
                let px_cents = (self
                    .last_traded_price
                    .get(&stock_id)
                    .copied()
                    .unwrap_or(150.0)
                    * 100.0)
                    .round() as u64;
                let order = Order {
                    id: self.next_order_id(),
                    agent_id,
                    stock_id,
                    side,
                    volume,
                    price: px_cents,
                    filled: 0,
                };
                if let Some(ch) = self.agent_channels.get(&agent_id) {
                    ch.ack_tx.send(order.clone()).unwrap();
                }
                if let Some(book) = self.order_books.get_mut(&stock_id) {
                    trades.extend(book.process_market_order(agent_id, side, volume));
                }
                self.shadow_update_tx
                    .send(ShadowEvent::MarketOrder(order))
                    .unwrap();
                self.vip_shadow_update_tx
                    .send(ShadowEvent::MarketOrder(order.clone()))
                    .unwrap();
            }
            OrderRequest::CancelOrder { agent_id, order_id } => {
                for book in self.order_books.values_mut() {
                    if book.cancel_order(order_id, agent_id) {
                        break;
                    }
                }
                self.shadow_update_tx
                    .send(ShadowEvent::CancelOrder { order_id, agent_id })
                    .unwrap();
                self.vip_shadow_update_tx
                    .send(ShadowEvent::CancelOrder { order_id, agent_id })
                    .unwrap();
            }
        }

        // Post-trade processing
        for tr in &trades {
            if let Some(taker_ch) = self.agent_channels.get(&tr.taker_agent_id) {
                taker_ch.trade_tx.send(tr.clone()).unwrap();
            }
            if let Some(maker_ch) = self.agent_channels.get(&tr.maker_agent_id) {
                maker_ch.trade_tx.send(tr.clone()).unwrap();
            }
            self.shadow_update_tx
                .send(ShadowEvent::Trade(tr.clone()))
                .unwrap();
            self.vip_shadow_update_tx
                .send(ShadowEvent::Trade(tr.clone()))
                .unwrap();
        }

        // Update internal market statistics
        if let Some(last) = trades.last() {
            self.last_traded_price
                .insert(last.stock_id, last.price as f64 / 100.0);
        }
        for tr in &trades {
            *self.cumulative_volume.entry(tr.stock_id).or_insert(0) += tr.volume;
        }
    }
}

impl Marketable for Market {
    fn run(&mut self) {
        //println!("[Market] Matching engine online. Waiting for orders...");
        while let Ok(req) = self.order_rx.recv() {
            self.process_request(req);
        }
        //println!("[Market] Order channel closed. Shutting down matching engine.");
    }

    // Stubs for remaining trait methods that are no longer relevant to this design.
    fn step(&mut self) -> f64 {
        0.0
    }
    fn current_price(&self) -> f64 {
        0.0
    }
    fn reset(&mut self) {}
    fn get_order_book(&self) -> Option<&OrderBook> {
        None
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
