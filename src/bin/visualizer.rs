// src/bin/visualizer.rs

use eframe::egui;
use egui_plot::{Legend, Line, Plot, PlotPoints};
// Import all the necessary components from our refactored library hub
use market_simulator::{
    calculate_option_price,
    Marketable,       // The all-important trait
    OptionType,
    GBMSimulator,     // The concrete implementation we'll use for now
};
use std::time::{Duration, Instant};

struct VisualizerApp {
    // THE KEY CHANGE: The app now owns a "trait object".
    // It can hold a GBMSimulator, an OrderBookSimulator, or anything that implements Marketable.
    stock_simulator: Box<dyn Marketable>,

    // History vectors remain the same
    price_history: Vec<f64>,
    option_price_history: Vec<f64>,

    // Option parameters controlled by the UI
    strike_price: f64,
    time_to_expiration_days: u32,
    volatility: f64,
    risk_free_rate: f64,
    option_type: OptionType,

    // UI state
    is_playing: bool,
    last_update: Instant,
}

impl eframe::App for VisualizerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.is_playing && self.last_update.elapsed() > Duration::from_millis(50) {
            let current_day = self.price_history.len() - 1;

            // This logic correctly stops the simulation after the option expires
            if current_day < self.time_to_expiration_days as usize {
                // We call .step() on the trait object, not on a specific struct
                let new_stock_price = self.stock_simulator.step();
                self.price_history.push(new_stock_price);
                // hueeee
                let days_remaining = self.time_to_expiration_days.saturating_sub(current_day as u32 + 1);
                let time_to_expiration_years = days_remaining as f64 / 252.0;

                let new_option_price = calculate_option_price(
                    self.option_type,
                    new_stock_price,
                    self.strike_price,
                    time_to_expiration_years,
                    self.risk_free_rate,
                    self.volatility,
                );
                self.option_price_history.push(new_option_price);
            } else {
                // If expired, just stop playing
                self.is_playing = false;
            }

            self.last_update = Instant::now();
        }
        ctx.request_repaint();

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Pluggable Stock & Option Simulator");

            ui.horizontal(|ui| {
                if ui.button(if self.is_playing { "⏸ Pause" } else { "▶ Play" }).clicked() {
                    self.is_playing = !self.is_playing;
                    self.last_update = Instant::now();
                }
                if ui.button("⏹ Reset").clicked() {
                    self.reset_simulation();
                }
            });
            ui.separator();

            // UI Controls Panel (no changes needed here)
            ui.collapsing("Simulation Parameters", |ui| {
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut self.strike_price).prefix("Strike: $"));
                    ui.add(egui::DragValue::new(&mut self.time_to_expiration_days).suffix(" days"));
                });
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut self.volatility).speed(0.001).prefix("Volatility: "));
                    ui.add(egui::DragValue::new(&mut self.risk_free_rate).speed(0.001).prefix("Risk-Free Rate: "));
                });
                ui.horizontal(|ui| {
                    ui.label("Option Type:");
                    ui.radio_value(&mut self.option_type, OptionType::Call, "Call");
                    ui.radio_value(&mut self.option_type, OptionType::Put, "Put");
                });
            });
            ui.separator();

            // Plots Panel (no changes needed here)
            ui.horizontal_top(|ui| {
                Plot::new("stock_plot").width(500.0).height(400.0).legend(Legend::default()).show(ui, |plot_ui| {
                    plot_ui.line(Line::new(PlotPoints::from_ys_f64(&self.price_history)).name("Stock Price"));
                });
                Plot::new("option_plot").width(500.0).height(400.0).legend(Legend::default()).show(ui, |plot_ui| {
                    plot_ui.line(Line::new(PlotPoints::from_ys_f64(&self.option_price_history)).name("Option Price"));
                });
            });
        });
    }
}

impl VisualizerApp {
    // The reset logic is now much cleaner
    fn reset_simulation(&mut self) {
        // We call the trait's reset method
        self.stock_simulator.reset();
        
        let initial_stock_price = self.stock_simulator.current_price();
        self.price_history = vec![initial_stock_price];
        
        let time_to_expiration_years = self.time_to_expiration_days as f64 / 252.0;
        let initial_option_price = calculate_option_price(
            self.option_type, initial_stock_price, self.strike_price, 
            time_to_expiration_years, self.risk_free_rate, self.volatility
        );
        self.option_price_history = vec![initial_option_price];

        self.is_playing = false;
    }
}

fn main() -> Result<(), eframe::Error> {
    // We create a concrete simulator instance...
    let gbm_simulator = GBMSimulator::new(150.0, 0.08, 0.20);
    // ...and immediately put it behind the 'Marketable' trait interface.
    let stock_simulator: Box<dyn Marketable> = Box::new(gbm_simulator);

    let initial_stock_price = stock_simulator.current_price();

    // Initial Option Parameters
    let option_type = OptionType::Call;
    let strike_price = 160.0;
    let time_to_expiration_days = 90;
    let volatility = 0.20;
    let risk_free_rate = 0.05;

    let time_to_expiration_years = time_to_expiration_days as f64 / 252.0;
    let initial_option_price = calculate_option_price(
        option_type, initial_stock_price, strike_price, 
        time_to_expiration_years, risk_free_rate, volatility
    );

    let app_state = VisualizerApp {
        stock_simulator, // Pass the trait object to the app
        price_history: vec![initial_stock_price],
        strike_price,
        time_to_expiration_days,
        volatility,
        risk_free_rate,
        option_type,
        option_price_history: vec![initial_option_price],
        is_playing: false,
        last_update: Instant::now(),
    };

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 800.0])
            .with_title("Pluggable Stock and Option Visualizer"),
        ..Default::default()
    };

    eframe::run_native(
        "Pluggable Visualizer App",
        native_options,
        Box::new(|_cc| Box::new(app_state)),
    )
}