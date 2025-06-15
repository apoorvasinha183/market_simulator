// src/bin/visualizer.rs

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui::{self, CentralPanel, Color32, Context, RichText, Ui};
use eframe::{App, Frame, NativeOptions};
use market_simulator::{
    stocks::{definitions::Symbol, registry},
    AgentType, Market, Marketable, OrderBook, Trade,
};
use egui_plot::{Legend, Line, Plot, PlotPoints, Points};
use std::collections::VecDeque;
use std::time::Instant;

const MAX_HISTORY: usize = 1000;

struct VisualizerApp {
    market: Market,
    price_history: VecDeque<(f64, f64)>, 
    trade_history: VecDeque<(f64, f64, f64)>, 
    last_price: f64,
    start_time: Instant,
    tick_count: u64,
    paused: bool,
    symbol_to_display: Symbol,
    last_trade_time: f64,
    pulse_duration: f64,
}

impl VisualizerApp {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let participants = vec![AgentType::MarketMaker, AgentType::DumbLimit, AgentType::DumbMarket, AgentType::IPO, AgentType::WhaleAgent];
        let symbol_to_display = registry::get_tradable_universe().first().unwrap().symbol.clone();
        let initial_price = registry::get_tradable_universe().first().unwrap().initial_price;
        let market = Market::new(&participants);
        
        let mut price_history = VecDeque::with_capacity(MAX_HISTORY);
        price_history.push_back((0.0, initial_price));

        Self {
            market,
            price_history,
            trade_history: VecDeque::with_capacity(MAX_HISTORY),
            last_price: initial_price,
            start_time: Instant::now(),
            tick_count: 0,
            paused: false,
            symbol_to_display,
            last_trade_time: -1.0,
            pulse_duration: 0.5,
        }
    }
}

impl App for VisualizerApp {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        if !self.paused {
            let new_price = self.market.step();
            let current_time = self.start_time.elapsed().as_secs_f64();

            if new_price > 0.0 {
                self.last_price = new_price;
            }

            self.price_history.push_back((current_time, self.last_price));
            if self.price_history.len() > MAX_HISTORY {
                self.price_history.pop_front();
            }

            if let Some(order_book) = self.market.get_order_book(&self.symbol_to_display) {
                // FIXED: Use .back() to get the last element of a VecDeque, not .last()
                if let Some(latest_trade) = order_book.get_trades().back() {
                    let trade_time = self.start_time.elapsed().as_secs_f64();
                    if trade_time > self.last_trade_time {
                        let trade_volume = latest_trade.volume as f64;
                        self.trade_history.push_back((trade_time, latest_trade.price as f64 / 100.0, trade_volume));
                        if self.trade_history.len() > MAX_HISTORY {
                            self.trade_history.pop_front();
                        }
                        self.last_trade_time = trade_time;
                    }
                }
            }
            self.tick_count += 1;
        }

        CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Real-Time Market Simulator");
                if ui.button(if self.paused { "▶ Resume" } else { "❚❚ Pause" }).clicked() {
                    self.paused = !self.paused;
                }
                if ui.button("↺ Reset").clicked() {
                    self.market.reset();
                    self.price_history.clear();
                    self.trade_history.clear();
                    self.tick_count = 0;
                    self.start_time = Instant::now();
                    self.last_price = registry::get_tradable_universe().first().unwrap().initial_price;
                    self.price_history.push_back((0.0, self.last_price));
                }
            });

            ui.horizontal(|ui| {
                ui.label(format!("Ticks: {}", self.tick_count));
                ui.label(format!("Time: {:.2}s", self.start_time.elapsed().as_secs_f64()));
                ui.label(format!(
                    "Total Inventory ({}): {}",
                    self.symbol_to_display,
                    self.market.get_total_inventory(&self.symbol_to_display)
                ));
                 ui.label(format!(
                    "Cumulative Volume ({}): {}",
                    self.symbol_to_display,
                    self.market.cumulative_volume(&self.symbol_to_display)
                ));
            });

            self.draw_price_chart(ui);

            if let Some(order_book) = self.market.get_order_book(&self.symbol_to_display) {
                draw_order_book_depth(ui, order_book);
            }
        });

        ctx.request_repaint_after(std::time::Duration::from_millis(16));
    }
}

fn draw_order_book_depth(ui: &mut Ui, order_book: &OrderBook) {
     ui.collapsing("Order Book Depth", |ui| {
        let (bids, asks) = order_book.get_depth();

        let bid_points: PlotPoints = bids
            .into_iter()
            .map(|(price, volume)| [price as f64 / 100.0, volume as f64])
            .collect();
        
        let ask_points: PlotPoints = asks
            .into_iter()
            .map(|(price, volume)| [price as f64 / 100.0, volume as f64])
            .collect();

        let bid_line = Line::new(bid_points).color(Color32::GREEN).name("Bids");
        let ask_line = Line::new(ask_points).color(Color32::RED).name("Asks");

        Plot::new("Depth Chart")
            .legend(Legend::default())
            .height(200.0)
            .show(ui, |plot_ui| {
                plot_ui.line(bid_line);
                plot_ui.line(ask_line);
            });
    });
}

impl VisualizerApp {
    fn draw_price_chart(&self, ui: &mut Ui) {
        let line = Line::new(
            self.price_history
                .iter()
                .map(|&(time, price)| [time, price])
                .collect::<PlotPoints>(),
        )
        .color(Color32::from_rgb(100, 200, 255))
        .name("Price");

        let plot = Plot::new("Price History")
            .legend(Legend::default())
            .height(ui.available_height() / 2.0)
            .x_axis_label("Time (s)")
            .y_axis_label("Price ($)")
            .show_grid(true);

        plot.show(ui, |plot_ui| {
            plot_ui.line(line);

            for &(time, price, volume) in &self.trade_history {
                let radius = (volume.log10() * 2.0).max(1.5);
                let alpha = (volume.log10() / 5.0 * 255.0).min(255.0) as u8;

                plot_ui.points(
                    Points::new(vec![[time, price]])
                        .radius(radius as f32)
                        .color(Color32::from_rgba_unmultiplied(255, 0, 0, alpha)),
                );
            }
            
            let current_time = self.start_time.elapsed().as_secs_f64();
            if current_time - self.last_trade_time < self.pulse_duration {
                let time_since_trade = current_time - self.last_trade_time;
                let pulse = (time_since_trade / self.pulse_duration * std::f64::consts::PI).sin();

                if let Some(&(_, last_price, last_volume)) = self.trade_history.back() {
                    let base_radius = (last_volume.log10() * 2.0).max(1.5);
                    let ring_radius = base_radius + pulse * 10.0;
                    
                    plot_ui.points(
                        Points::new(vec![[self.last_trade_time, last_price]])
                            .radius(ring_radius as f32)
                            .color(Color32::from_rgba_unmultiplied(255, 215, 0, (64.0 * (1.0 - pulse)) as u8))
                            .shape(egui_plot::MarkerShape::Circle),
                    );
                }
            }
            
            let last_price_text = RichText::new(format!("${:.2}", self.last_price))
                .color(Color32::WHITE)
                .background_color(Color32::from_rgba_unmultiplied(0, 0, 0, 128));

            if let Some(&(x, _)) = self.price_history.back() {
                plot_ui.text(
                    egui_plot::Text::new(
                        egui_plot::PlotPoint::new(x, self.last_price),
                        last_price_text,
                    )
                    .anchor(egui::Align2::LEFT_BOTTOM),
                );
            }
        });
    }
}

fn main() -> Result<(), eframe::Error> {
    let native_options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([600.0, 400.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Market Simulator",
        native_options,
        Box::new(|cc| Box::new(VisualizerApp::new(cc))),
    )
}