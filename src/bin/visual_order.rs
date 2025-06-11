// src/bin/visual_order.rs

use eframe::egui;
use egui::{Color32, RichText, Stroke};
use egui_plot::{Bar, BarChart, Legend, Line, Plot, PlotPoints};
// We need to import the Trade struct to handle the output from the matching engine
use market_simulator::{Agent, DumbAgent, DumbLimitAgent, MarketView, OrderBook, OrderRequest, Side, Trade};
use std::time::{Duration, Instant};

/// The main application struct now holds the entire simulation state.
struct OrderBookVisualizer {
    order_book: OrderBook,
    agents: Vec<Box<dyn Agent>>,
    price_history: Vec<f64>,
    last_traded_price: f64,
    is_market_running: bool,
    last_update: Instant,
}

impl eframe::App for OrderBookVisualizer {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // --- This is the main simulation loop ---
        if self.is_market_running && self.last_update.elapsed() > Duration::from_millis(100) {
            // 1. Create a read-only view of the market for the agents
            let market_view = MarketView {
                order_book: &self.order_book,
            };

            // 2. Poll all agents for their desired actions
            let mut all_requests = Vec::new();
            for agent in self.agents.iter_mut() {
                all_requests.extend(agent.decide_actions(&market_view));
            }

            // 3. Process the actions against the order book
            let mut trades_this_tick: Vec<Trade> = Vec::new();
            for request in all_requests {
                match request {
                    OrderRequest::MarketOrder { agent_id: _, side, volume } => {
                        trades_this_tick.extend(self.order_book.process_market_order(side, volume));
                    },
                    OrderRequest::LimitOrder { agent_id: _, side, price, volume } => {
                        self.order_book.add_limit_order(price, volume, side);
                    },
                }
            }

            // 4. Update price and history based on the trades that occurred
            if let Some(last_trade) = trades_this_tick.last() {
                self.last_traded_price = last_trade.price as f64 / 100.0;
                // Only add to history if the price actually changed
                if self.price_history.last() != Some(&self.last_traded_price) {
                    self.price_history.push(self.last_traded_price);
                }
            }
            
            self.last_update = Instant::now();
        }
        ctx.request_repaint();


        // --- UI Rendering ---
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Live Agent-Based Order Book");
                ui.separator();

                if ui.button(if self.is_market_running { "⏸ Stop Market" } else { "▶ Start Market" }).clicked() {
                    self.is_market_running = !self.is_market_running;
                }
                if ui.button("🔄 Reset World").clicked() {
                    self.reset_simulation();
                }
            });
        });

        // Bottom panel for the live order book data
        egui::TopBottomPanel::bottom("bottom_panel")
            .resizable(true)
            .min_height(200.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading("Live Order Book");
                    });
                    ui.separator();

                    ui.horizontal_top(|ui| {
                        // BIDS PANEL (LEFT)
                        ui.vertical(|ui| {
                            ui.set_width(ui.available_width() / 2.0 - 5.0);
                            ui.label(RichText::new("Bids (Top 10)").color(Color32::GREEN).strong());
                            egui::Grid::new("bids_grid").show(ui, |ui| {
                                ui.label(RichText::new("Price").underline());
                                ui.label(RichText::new("Volume").underline());
                                ui.end_row();
                                for (price, volume) in self.order_book.bids.iter().rev().take(10) {
                                    ui.label(format!("{:.2}", *price as f64 / 100.0));
                                    ui.label(volume.to_string());
                                    ui.end_row();
                                }
                            });
                        });
                        ui.separator();
                        // ASKS PANEL (RIGHT)
                        ui.vertical(|ui| {
                            ui.label(RichText::new("Asks (Top 10)").color(Color32::RED).strong());
                            egui::Grid::new("asks_grid").show(ui, |ui| {
                                ui.label(RichText::new("Price").underline());
                                ui.label(RichText::new("Volume").underline());
                                ui.end_row();
                                for (price, volume) in self.order_book.asks.iter().take(10) {
                                    ui.label(format!("{:.2}", *price as f64 / 100.0));
                                    ui.label(volume.to_string());
                                    ui.end_row();
                                }
                            });
                        });
                    });
                    ui.separator();

                    // SPREAD DISPLAY
                    let best_bid = self.order_book.bids.keys().last().cloned();
                    let best_ask = self.order_book.asks.keys().next().cloned();
                    if let (Some(bid), Some(ask)) = (best_bid, best_ask) {
                        let spread = (ask - bid) as f64 / 100.0;
                        ui.vertical_centered(|ui| {
                            ui.label(format!(
                                "Best Bid: {:.2}  |  Best Ask: {:.2}  |  Spread: ${:.2}",
                                bid as f64 / 100.0, ask as f64 / 100.0, spread
                            ));
                        });
                    }
                });
            });
        
        // Central panel now shows TWO plots
        egui::CentralPanel::default().show(ctx, |ui| {
            // --- LAYOUT FIX: Use ui.columns to divide the space ---
            ui.columns(2, |columns| {
                // Plot 1: The Depth Chart in the first column
                // We pass `&mut columns[0]` which is the `ui` for the first column.
                Plot::new("order_book_plot")
                    .legend(Legend::default())
                    .show(&mut columns[0], |plot_ui| {
                        let mut cumulative_ask_volume = 0.0;
                        let ask_bars: Vec<Bar> = self.order_book.asks.iter().map(|(&price_cents, &volume)| {
                            cumulative_ask_volume += volume as f64;
                            Bar::new(price_cents as f64 / 100.0, cumulative_ask_volume).width(0.04).fill(Color32::from_rgba_unmultiplied(255, 80, 80, 60))
                        }).collect();
                        plot_ui.bar_chart(BarChart::new(ask_bars).name("Cumulative Asks").color(Color32::RED));

                        let mut cumulative_bid_volume = 0.0;
                        let bid_bars: Vec<Bar> = self.order_book.bids.iter().rev().map(|(&price_cents, &volume)| {
                            cumulative_bid_volume += volume as f64;
                            Bar::new(price_cents as f64 / 100.0, cumulative_bid_volume).width(0.04).fill(Color32::from_rgba_unmultiplied(80, 255, 80, 60))
                        }).collect();
                        plot_ui.bar_chart(BarChart::new(bid_bars).name("Cumulative Bids").color(Color32::GREEN));
                    });

                // Plot 2: The Live Price Chart in the second column
                // We pass `&mut columns[1]` for the second column.
                Plot::new("price_history_plot")
                    .legend(Legend::default())
                    .show(&mut columns[1], |plot_ui| {
                        let line = Line::new(PlotPoints::from_ys_f64(&self.price_history))
                            .color(Color32::LIGHT_BLUE)
                            .stroke(Stroke::new(2.0, Color32::LIGHT_BLUE));
                        plot_ui.line(line.name("Last Traded Price"));
                    });
            });
        });
    }
}

// Add a helper function to reset the simulation state
impl OrderBookVisualizer {
    fn reset_simulation(&mut self) {
        self.is_market_running = false;
        self.order_book = OrderBook::new_random();
        self.last_traded_price = 150.0;
        self.price_history = vec![150.0];
    }
}

fn main() -> Result<(), eframe::Error> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1000.0, 800.0]),
        ..Default::default()
    };

    // --- Create our initial population of agents ---
    let mut agents: Vec<Box<dyn Agent>> = Vec::new();
    for i in 0..10 { agents.push(Box::new(DumbAgent::new(i))); }
    for i in 10..15 { agents.push(Box::new(DumbLimitAgent::new(i))); }

    let app_state = OrderBookVisualizer {
        order_book: OrderBook::new_random(),
        agents,
        price_history: vec![150.0],
        last_traded_price: 150.0,
        is_market_running: false,
        last_update: Instant::now(),
    };

    eframe::run_native(
        "Live Agent-Based Visualizer",
        native_options,
        Box::new(|_cc| Box::new(app_state)),
    )
}
