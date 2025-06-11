// src/bin/visual_order.rs

use eframe::egui;
use egui::{Color32, RichText, Stroke};
use egui_plot::{Bar, BarChart, Legend, Line, Plot, PlotPoints};
// The visualizer only needs to know about the final products, not the agent details.
use market_simulator::{AgentType, Market, Marketable, OrderBook};
use std::time::{Duration, Instant};

struct AgentVisualizer {
    simulator: Box<dyn Marketable>,
    price_history: Vec<f64>,
    is_market_running: bool,
    last_update: Instant,
}

impl eframe::App for AgentVisualizer {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // --- The simulation logic is now just one clean line ---
        // The Market engine handles all the complex agent interactions internally.
        if self.is_market_running && self.last_update.elapsed() > Duration::from_millis(100) {
            let new_price = self.simulator.step();
            if self.price_history.last() != Some(&new_price) {
                self.price_history.push(new_price);
            }
            self.last_update = Instant::now();
        }
        ctx.request_repaint();


        // --- UI Rendering ---
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Live Agent-Based Market");
                ui.separator();

                if ui.button(if self.is_market_running { "⏸ Stop Market" } else { "▶ Start Market" }).clicked() {
                    self.is_market_running = !self.is_market_running;
                }
                if ui.button("🔄 Reset World").clicked() {
                    self.reset_simulation();
                }
            });
        });

        // We get the order book by calling our trait method and downcasting.
        // This is safe because we know our `main` function creates a `Market`.
        if let Some(market) = self.simulator.as_any().downcast_ref::<Market>() {
            let order_book = market.get_order_book();
            
            egui::TopBottomPanel::bottom("bottom_panel")
                .resizable(true)
                .min_height(200.0)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.vertical_centered(|ui| { ui.heading("Live Order Book"); });
                        ui.separator();
                        ui.horizontal_top(|ui| {
                            ui.vertical(|ui| {
                                ui.set_width(ui.available_width() / 2.0 - 5.0);
                                ui.label(RichText::new("Bids (Top 10)").color(Color32::GREEN).strong());
                                egui::Grid::new("bids_grid").show(ui, |ui| {
                                    ui.label(RichText::new("Price").underline());
                                    ui.label(RichText::new("Volume").underline());
                                    ui.end_row();
                                    for (price, volume) in order_book.bids.iter().rev().take(10) {
                                        ui.label(format!("{:.2}", *price as f64 / 100.0));
                                        ui.label(volume.to_string());
                                        ui.end_row();
                                    }
                                });
                            });
                            ui.separator();
                            ui.vertical(|ui| {
                                ui.label(RichText::new("Asks (Top 10)").color(Color32::RED).strong());
                                egui::Grid::new("asks_grid").show(ui, |ui| {
                                    ui.label(RichText::new("Price").underline());
                                    ui.label(RichText::new("Volume").underline());
                                    ui.end_row();
                                    for (price, volume) in order_book.asks.iter().take(10) {
                                        ui.label(format!("{:.2}", *price as f64 / 100.0));
                                        ui.label(volume.to_string());
                                        ui.end_row();
                                    }
                                });
                            });
                        });
                        ui.separator();
                        let best_bid = order_book.bids.keys().last().cloned();
                        let best_ask = order_book.asks.keys().next().cloned();
                        if let (Some(bid), Some(ask)) = (best_bid, best_ask) {
                            let spread = (ask - bid) as f64 / 100.0;
                            ui.vertical_centered(|ui| {
                                ui.label(format!("Best Bid: {:.2}  |  Best Ask: {:.2}  |  Spread: ${:.2}", bid as f64 / 100.0, ask as f64 / 100.0, spread));
                            });
                        }
                    });
                });
        
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.columns(2, |columns| {
                    Plot::new("order_book_plot").legend(Legend::default()).show(&mut columns[0], |plot_ui| {
                        let mut cumulative_ask_volume = 0.0;
                        let ask_bars: Vec<Bar> = order_book.asks.iter().map(|(&p, &v)| { cumulative_ask_volume += v as f64; Bar::new(p as f64 / 100.0, cumulative_ask_volume).width(0.04).fill(Color32::from_rgba_unmultiplied(255, 80, 80, 60)) }).collect();
                        plot_ui.bar_chart(BarChart::new(ask_bars).name("Cumulative Asks").color(Color32::RED));
                        let mut cumulative_bid_volume = 0.0;
                        let bid_bars: Vec<Bar> = order_book.bids.iter().rev().map(|(&p, &v)| { cumulative_bid_volume += v as f64; Bar::new(p as f64 / 100.0, cumulative_bid_volume).width(0.04).fill(Color32::from_rgba_unmultiplied(80, 255, 80, 60)) }).collect();
                        plot_ui.bar_chart(BarChart::new(bid_bars).name("Cumulative Bids").color(Color32::GREEN));
                    });
                    Plot::new("price_history_plot").legend(Legend::default()).show(&mut columns[1], |plot_ui| {
                        let line = Line::new(PlotPoints::from_ys_f64(&self.price_history)).color(Color32::LIGHT_BLUE).stroke(Stroke::new(2.0, Color32::LIGHT_BLUE));
                        plot_ui.line(line.name("Last Traded Price"));
                    });
                });
            });
        }
    }
}

impl AgentVisualizer {
    fn reset_simulation(&mut self) {
        self.is_market_running = false;
        // This now correctly calls the reset method defined in the Marketable trait
        // and implemented by your Market struct.
        self.simulator.reset();
        self.price_history = vec![self.simulator.current_price()];
    }
}

fn main() -> Result<(), eframe::Error> {
    // --- THIS IS THE FIX ---
    // The `..Default::default()` is added to the initializer to fill in
    // all the missing fields the compiler was complaining about.
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1000.0, 800.0]),
        ..Default::default()
    };

    // =================================================================
    // THIS IS THE ONE AND ONLY PLACE TO DECIDE MARKET PARTICIPANTS
    // =================================================================
    let participants = vec![
        AgentType::DumbMarket,
        AgentType::DumbLimit,
    ];

    // The visualizer calls the `Market::new` constructor from the library.
    let simulator: Box<dyn Marketable> = Box::new(Market::new(&participants));

    let app_state = AgentVisualizer {
        price_history: vec![simulator.current_price()],
        simulator,
        is_market_running: false,
        last_update: Instant::now(),
    };

    eframe::run_native(
        "Live Market Visualizer",
        native_options,
        Box::new(|_cc| Box::new(app_state)),
    )
}
