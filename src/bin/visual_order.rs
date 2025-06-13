// src/bin/visual_order.rs

use eframe::egui;
use egui::{Color32, RichText, Stroke};
use egui_plot::{Legend, Line, Plot, PlotBounds, PlotPoints};
use market_simulator::{AgentType, Market, Marketable};
use std::time::{Duration, Instant};

struct AgentVisualizer {
    simulator: Box<dyn Marketable>,
    price_history: Vec<f64>,
    is_market_running: bool,
    last_update: Instant,
}

impl eframe::App for AgentVisualizer {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.is_market_running && self.last_update.elapsed() > Duration::from_millis(100) {
            let new_price = self.simulator.step();
            if self.price_history.last() != Some(&new_price) {
                self.price_history.push(new_price);
            }
            self.last_update = Instant::now();
        }
        ctx.request_repaint();

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
                            // Bids Panel
                            ui.vertical(|ui| {
                                ui.set_width(ui.available_width() / 2.0 - 5.0);
                                ui.label(RichText::new("Bids (Top 10)").color(Color32::GREEN).strong());
                                egui::Grid::new("bids_grid").show(ui, |ui| {
                                    ui.label(RichText::new("Price").underline());
                                    ui.label(RichText::new("Volume").underline());
                                    ui.end_row();
                                    for (price, level) in order_book.bids.iter().rev().take(10) {
                                        ui.label(format!("{:.2}", *price as f64 / 100.0));
                                        ui.label(level.total_volume.to_string());
                                        ui.end_row();
                                    }
                                });
                            });
                            ui.separator();
                            // Asks Panel
                            ui.vertical(|ui| {
                                ui.label(RichText::new("Asks (Top 10)").color(Color32::RED).strong());
                                egui::Grid::new("asks_grid").show(ui, |ui| {
                                    ui.label(RichText::new("Price").underline());
                                    ui.label(RichText::new("Volume").underline());
                                    ui.end_row();
                                    for (price, level) in order_book.asks.iter().take(10) {
                                        ui.label(format!("{:.2}", *price as f64 / 100.0));
                                        ui.label(level.total_volume.to_string());
                                        ui.end_row();
                                    }
                                });
                            });
                        });
                        ui.separator();

                        let best_bid = order_book.bids.keys().last().cloned();
                        let best_ask = order_book.asks.keys().next().cloned();
                        let total_inventory = market.get_total_inventory();

                        if let (Some(bid), Some(ask)) = (best_bid, best_ask) {
                            if ask > bid {
                                let spread = (ask - bid) as f64 / 100.0;
                                ui.vertical_centered(|ui| {
                                    ui.label(format!(
                                        "Bid: {:.2} | Ask: {:.2} | Spread: ${:.2} | Cum Vol: {} | Net Inventory: {}",
                                        bid as f64 / 100.0,
                                        ask as f64 / 100.0,
                                        spread,
                                        market.cumulative_volume(),
                                        total_inventory
                                    ));
                                });
                            }
                        }
                    });
                });

            egui::CentralPanel::default().show(ctx, |ui| {
                ui.columns(2, |columns| {
                    // --- Depth Plot ---
                    Plot::new("order_book_plot")
                        .legend(Legend::default())
                        .show(&mut columns[0], |plot_ui| {
                            // --- Asks ---
                            let mut ask_pts = Vec::new();
                            let mut cum_ask = 0.0;
                            for (&px, lvl) in order_book.asks.iter() {
                                let p = px as f64 / 100.0;
                                ask_pts.push([p, cum_ask]);
                                cum_ask += lvl.total_volume as f64;
                                ask_pts.push([p, cum_ask]);
                            }
                            plot_ui.line(
                                Line::new(PlotPoints::from(ask_pts))
                                    .fill(0.0) // Fill down to the x-axis
                                    .color(Color32::from_rgba_unmultiplied(255, 80, 80, 60))
                                    .name("Cumulative Asks"),
                            );

                            // --- Bids ---
                            let mut bid_pts = Vec::new();
                            let mut cum_bid = 0.0;
                            for (&px, lvl) in order_book.bids.iter().rev() {
                                let p = px as f64 / 100.0;
                                bid_pts.push([p, cum_bid]);
                                cum_bid += lvl.total_volume as f64;
                                bid_pts.push([p, cum_bid]);
                            }
                            plot_ui.line(
                                Line::new(PlotPoints::from(bid_pts))
                                    .fill(0.0) // Fill down to the x-axis
                                    .color(Color32::from_rgba_unmultiplied(80, 255, 80, 60))
                                    .name("Cumulative Bids"),
                            );

                            // --- THE FIX: Set plot bounds manually based on last traded price ---
                            let center_px = market.current_price();
                            let half_win = 20.00; // Show +/- $2.50
                            //let y_max = (cum_ask.max(cum_bid) * 1.05).max(100.0); // Add a minimum y-height
                            let y_max = 2_000_000.00;
                            plot_ui.set_plot_bounds(PlotBounds::from_min_max(
                                [center_px - half_win, 0.0],
                                [center_px + half_win, y_max],
                            ));
                        });

                    // --- Price History Plot ---
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
}

impl AgentVisualizer {
    fn reset_simulation(&mut self) {
        self.is_market_running = false;
        self.simulator.reset();
        self.price_history = vec![self.simulator.current_price()];
    }
}

fn main() -> Result<(), eframe::Error> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1200.0, 800.0]),
        ..Default::default()
    };
    let participants = vec![
        AgentType::MarketMaker,
        AgentType::DumbLimit,
        AgentType::DumbMarket,
        AgentType::WhaleAgent
    ];
    let simulator: Box<dyn Marketable> = Box::new(Market::new(&participants));
    let app_state = AgentVisualizer {
        price_history: vec![simulator.current_price()],
        simulator,
        is_market_running: false,
        last_update: Instant::now(),
    };
    eframe::run_native(
        "Live Agent-Based Market Visualizer",
        native_options,
        Box::new(|_cc| Box::new(app_state)),
    )
}
