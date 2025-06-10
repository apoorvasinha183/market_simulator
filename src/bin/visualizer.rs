// src/bin/visualizer.rs

use eframe::egui;
use egui::{Color32, FontId, Frame, RichText};
use egui_plot::{Legend, Line, Plot, PlotPoints};
use market_simulator::{Greeks, Marketable, OptionPricer, OptionType, GBMSimulator};
use std::time::{Duration, Instant};

struct VisualizerApp {
    // World state
    stock_simulator: Box<dyn Marketable>,
    option_pricer: OptionPricer,

    // History for the stock plot
    price_history: Vec<f64>,

    // Current values for the data panel
    current_option_price: f64,
    current_greeks: Greeks,

    // UI state for controlling the simulation parameters
    strike_price: f64,
    time_to_expiration_days: u32,
    risk_free_rate: f64,
    option_type: OptionType,
    initial_volatility: f64,
    volatility_window: usize,
    
    // UI state for the app itself
    is_playing: bool,
    last_update: Instant,
}

impl eframe::App for VisualizerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.is_playing && self.last_update.elapsed() > Duration::from_millis(50) {
            let current_day = self.price_history.len() - 1;

            if current_day < self.time_to_expiration_days as usize {
                let new_stock_price = self.stock_simulator.step();
                self.price_history.push(new_stock_price);

                let (new_option_price, new_greeks) = self.option_pricer.calculate_price_and_greeks(
                    new_stock_price,
                    current_day as u32 + 1,
                );

                self.current_option_price = new_option_price;
                self.current_greeks = new_greeks;

            } else {
                self.is_playing = false;
            }
            self.last_update = Instant::now();
        }
        ctx.request_repaint();

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Stateful Option Pricer with Dynamic Volatility");

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

            ui.collapsing("Simulation Parameters", |ui| {
                ui.add(egui::DragValue::new(&mut self.strike_price).prefix("Strike: $"));
                ui.add(egui::DragValue::new(&mut self.time_to_expiration_days).suffix(" days"));
                ui.add(egui::DragValue::new(&mut self.risk_free_rate).speed(0.001).prefix("Risk-Free Rate: "));
                ui.add(egui::DragValue::new(&mut self.initial_volatility).speed(0.001).prefix("Initial Volatility: "));
                ui.add(egui::DragValue::new(&mut self.volatility_window).suffix(" day window"));
                
                ui.horizontal(|ui| {
                    ui.label("Option Type:");
                    ui.radio_value(&mut self.option_type, OptionType::Call, "Call");
                    ui.radio_value(&mut self.option_type, OptionType::Put, "Put");
                });
            });
            ui.separator();

            ui.horizontal_top(|ui| {
                Plot::new("stock_plot")
                    .width(600.0)
                    .height(400.0)
                    .legend(Legend::default())
                    .show(ui, |plot_ui| {
                        plot_ui.line(Line::new(PlotPoints::from_ys_f64(&self.price_history)).name("Stock Price"));
                });
                
                Frame::dark_canvas(ui.style())
                    .inner_margin(egui::Margin::same(10.0))
                    .show(ui, |ui| {
                        ui.set_width(300.0);
                        ui.vertical_centered(|ui| {
                            ui.heading("Option Data");
                        });
                        ui.separator();

                        let mono_font = FontId::monospace(14.0);
                        
                        // --- CORRECTED LAYOUT SECTION ---
                        // Price and Greeks are now all inside the same Grid.
                        egui::Grid::new("greeks_grid")
                            .num_columns(2)
                            .spacing([40.0, 4.0])
                            .striped(true)
                            .show(ui, |ui| {
                                // Add Price as the first row of the grid.
                                ui.label(RichText::new("Price:").size(16.0).strong());
                                ui.label(RichText::new(format!("{:2}", self.current_option_price))
                                    .font(mono_font.clone())
                                    .color(Color32::LIGHT_GREEN)
                                    .size(16.0));
                                ui.end_row();

                                // Add a separator row for visual clarity
                                ui.label(""); // empty cell
                                ui.separator();
                                ui.end_row();

                                ui.label("Delta:");
                                ui.label(RichText::new(format!("{:.4}", self.current_greeks.delta)).font(mono_font.clone()));
                                ui.end_row();

                                ui.label("Gamma:");
                                ui.label(RichText::new(format!("{:.4}", self.current_greeks.gamma)).font(mono_font.clone()));
                                ui.end_row();

                                ui.label("Vega:");
                                ui.label(RichText::new(format!("{:.4}", self.current_greeks.vega)).font(mono_font.clone()));
                                ui.end_row();

                                ui.label("Theta:");
                                ui.label(RichText::new(format!("{:.4}", self.current_greeks.theta)).font(mono_font.clone()));
                                ui.end_row();

                                ui.label("Rho:");
                                ui.label(RichText::new(format!("{:.4}", self.current_greeks.rho)).font(mono_font.clone()));
                                ui.end_row();
                            });
                    });
            });
        });
    }
}

impl VisualizerApp {
    fn reset_simulation(&mut self) {
        // Reset the stock price generator
        self.stock_simulator.reset();
        
        // Re-create the OptionPricer with the latest settings from the UI
        self.option_pricer = OptionPricer::new(
            self.option_type,
            self.strike_price,
            self.time_to_expiration_days as f64 / 252.0,
            self.risk_free_rate,
            self.initial_volatility,
            self.volatility_window,
        );
        
        // Reset the historical data vectors
        let initial_stock_price = self.stock_simulator.current_price();
        self.price_history = vec![initial_stock_price];
        
        // On reset, recalculate the initial price/greeks and set the current state.
        let (price, greeks) = self.option_pricer.calculate_price_and_greeks(initial_stock_price, 0);
        self.current_option_price = price;
        self.current_greeks = greeks;

        self.is_playing = false;
    }
}

fn main() -> Result<(), eframe::Error> {
    // We create a concrete simulator instance...
    let stock_simulator: Box<dyn Marketable> = Box::new(GBMSimulator::new(150.0, 0.08, 0.20));
    let initial_stock_price = stock_simulator.current_price();

    // Initial parameters for the UI
    let option_type = OptionType::Call;
    let strike_price = 160.0;
    let time_to_expiration_days = 90;
    let initial_volatility = 0.20;
    let risk_free_rate = 0.05;
    let volatility_window = 20;

    // Create the stateful OptionPricer
    let mut option_pricer = OptionPricer::new(
        option_type,
        strike_price,
        time_to_expiration_days as f64 / 252.0,
        risk_free_rate,
        initial_volatility,
        volatility_window
    );

    // Calculate initial state for display
    let (initial_option_price, initial_greeks) = option_pricer.calculate_price_and_greeks(initial_stock_price, 0);

    let app_state = VisualizerApp {
        stock_simulator,
        option_pricer,
        price_history: vec![initial_stock_price],
        current_option_price: initial_option_price,
        current_greeks: initial_greeks,
        strike_price,
        time_to_expiration_days,
        risk_free_rate,
        option_type,
        initial_volatility,
        volatility_window,
        is_playing: false,
        last_update: Instant::now(),
    };

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 600.0])
            .with_title("Stateful Pricer Visualizer"),
        ..Default::default()
    };

    eframe::run_native(
        "Stateful Pricer Visualizer App",
        native_options,
        Box::new(|_cc| Box::new(app_state)),
    )
}