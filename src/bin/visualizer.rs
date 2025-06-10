// src/bin/visualizer.rs

use eframe::egui;
use egui::{Color32, FontId, Frame, ProgressBar, RichText, Stroke};
use egui_plot::{Legend, Line, Plot, PlotPoints};
use market_simulator::{Greeks, Marketable, OptionPricer, OptionType, GBMSimulator};
use std::time::{Duration, Instant};

struct VisualizerApp {
    // World state
    stock_simulator: Box<dyn Marketable>,
    option_pricer: OptionPricer,

    // --- State for Multi-Run ---
    run_price_histories: Vec<Vec<f64>>,
    current_run_history: Vec<f64>,

    // Current values for the real-time data panel
    current_option_price: f64,
    current_greeks: Greeks,

    // --- NEW: State for non-blocking Batch Mode ---
    num_runs_to_batch: usize,
    is_batch_running: bool,
    batch_runs_done: usize,

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
        // --- NEW: Non-blocking batch processing ---
        // If a batch is running, process a small chunk on each UI frame.
        if self.is_batch_running {
            let runs_per_frame = 20; // Process this many simulations per frame to keep UI smooth
            let end_run = (self.batch_runs_done + runs_per_frame).min(self.num_runs_to_batch);

            for _ in self.batch_runs_done..end_run {
                self.stock_simulator.reset();
                let mut path = Vec::with_capacity(self.time_to_expiration_days as usize + 1);
                path.push(self.stock_simulator.current_price());
                for _ in 0..self.time_to_expiration_days {
                    path.push(self.stock_simulator.step());
                }
                self.run_price_histories.push(path);
            }

            self.batch_runs_done = end_run;

            if self.batch_runs_done >= self.num_runs_to_batch {
                self.is_batch_running = false; // Batch is complete!
            }
        }
        
        // This is the interactive, animated mode
        if self.is_playing && self.last_update.elapsed() > Duration::from_millis(50) {
            let current_day = self.current_run_history.len() - 1;

            if current_day < self.time_to_expiration_days as usize {
                let new_stock_price = self.stock_simulator.step();
                self.current_run_history.push(new_stock_price);

                let (new_option_price, new_greeks) = self.option_pricer.calculate_price_and_greeks(
                    new_stock_price,
                    current_day as u32 + 1,
                );
                self.current_option_price = new_option_price;
                self.current_greeks = new_greeks;
            } else {
                self.is_playing = false;
                if self.current_run_history.len() > 1 {
                     self.run_price_histories.push(self.current_run_history.clone());
                }
                self.current_run_history.clear();
            }
            self.last_update = Instant::now();
        }
        ctx.request_repaint();

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Monte Carlo Option Simulator");
                ui.add_space(20.0);
                
                if ui.button(if self.is_playing { "⏸ Pause" } else { "▶ Play" }).clicked() {
                    if !self.is_playing && self.current_run_history.is_empty() { self.start_new_run(); }
                    self.is_playing = !self.is_playing;
                    self.last_update = Instant::now();
                }
                if ui.button("▶+ Start New Run").clicked() {
                    self.start_new_run();
                    self.is_playing = true;
                }
                if ui.button("🗑 Clear All Runs").clicked() { self.clear_all_runs(); }
            });

            // --- Controls for Batch Mode ---
            ui.horizontal(|ui| {
                ui.label("Batch size:");
                // Disable the input box while a batch is running
                ui.add_enabled(!self.is_batch_running, egui::DragValue::new(&mut self.num_runs_to_batch).speed(1.0).clamp_range(1..=10000));
                // Disable the button while a batch is running
                if ui.add_enabled(!self.is_batch_running, egui::Button::new("⚡ Run Batch")).clicked() {
                    self.run_batch_simulations();
                }
            });

            // --- NEW: Display the Progress Bar ---
            if self.is_batch_running {
                let progress = self.batch_runs_done as f32 / self.num_runs_to_batch as f32;
                let progress_text = format!("Running Batch... {}/{}", self.batch_runs_done, self.num_runs_to_batch);
                ui.add(ProgressBar::new(progress).text(progress_text));
            }
            
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
            
            Frame::dark_canvas(ui.style())
                .inner_margin(egui::Margin::symmetric(12.0, 8.0))
                .show(ui, |ui| {
                    ui.heading("Option Data (Current Run)");
                    ui.separator();
                    ui.horizontal(|ui| {
                        let mono_font = FontId::monospace(14.0);
                        let big_font = FontId::monospace(18.0);
                        ui.vertical(|ui| {
                            ui.label("Price:");
                            ui.label(RichText::new(format!("$ {:.2}", self.current_option_price)).font(big_font).color(Color32::LIGHT_GREEN));
                        });
                        ui.add(egui::Separator::default().vertical());
                        egui::Grid::new("greeks_grid").num_columns(2).spacing([20.0, 2.0]).show(ui, |ui| {
                            ui.label(RichText::new("Delta:").strong());
                            ui.label(RichText::new(format!("{:.4}", self.current_greeks.delta)).font(mono_font.clone()));
                            ui.end_row();
                            ui.label(RichText::new("Gamma:").strong());
                            ui.label(RichText::new(format!("{:.4}", self.current_greeks.gamma)).font(mono_font.clone()));
                            ui.end_row();
                            ui.label(RichText::new("Vega:").strong());
                            ui.label(RichText::new(format!("{:.4}", self.current_greeks.vega)).font(mono_font.clone()));
                            ui.end_row();
                            ui.label(RichText::new("Theta:").strong());
                            ui.label(RichText::new(format!("{:.4}", self.current_greeks.theta)).font(mono_font.clone()));
                            ui.end_row();
                            ui.label(RichText::new("Rho:").strong());
                            ui.label(RichText::new(format!("{:.4}", self.current_greeks.rho)).font(mono_font.clone()));
                            ui.end_row();
                        });
                    });
                });
            ui.add_space(4.0);

            Frame::dark_canvas(ui.style()).show(ui, |ui| {
                Plot::new("stock_plot")
                    .height(ui.available_height())
                    .width(ui.available_width())
                    .legend(Legend::default())
                    .show(ui, |plot_ui| {
                        for history in self.run_price_histories.iter() {
                            let line = Line::new(PlotPoints::from_ys_f64(history))
                                .color(Color32::from_gray(100).additive())
                                .stroke(Stroke::new(1.0, Color32::from_gray(100).additive()));
                            plot_ui.line(line);
                        }
                        if !self.current_run_history.is_empty() {
                            let active_line = Line::new(PlotPoints::from_ys_f64(&self.current_run_history))
                                .color(Color32::LIGHT_BLUE)
                                .stroke(Stroke::new(2.0, Color32::LIGHT_BLUE))
                                .name("Current Run");
                            plot_ui.line(active_line);
                        }
                });
            });
        });
    }
}

impl VisualizerApp {
    fn start_new_run(&mut self) {
        if self.current_run_history.len() > 1 {
            self.run_price_histories.push(self.current_run_history.clone());
        }
        self.stock_simulator.reset();
        self.option_pricer = OptionPricer::new(
            self.option_type, self.strike_price, self.time_to_expiration_days as f64 / 252.0,
            self.risk_free_rate, self.initial_volatility, self.volatility_window,
        );
        let initial_stock_price = self.stock_simulator.current_price();
        self.current_run_history = vec![initial_stock_price];
        let (price, greeks) = self.option_pricer.calculate_price_and_greeks(initial_stock_price, 0);
        self.current_option_price = price;
        self.current_greeks = greeks;
        self.is_playing = false;
    }

    fn clear_all_runs(&mut self) {
        self.run_price_histories.clear();
        self.current_run_history.clear();
        self.start_new_run();
    }

    // UPDATED Logic for Batch Mode
    fn run_batch_simulations(&mut self) {
        self.clear_all_runs(); // Start from a clean slate
        self.is_playing = false; // Stop any interactive run
        self.batch_runs_done = 0; // Reset progress
        self.is_batch_running = true; // Kick off the batch process
    }
}

fn main() -> Result<(), eframe::Error> {
    let stock_simulator: Box<dyn Marketable> = Box::new(GBMSimulator::new(150.0, 0.08, 0.20));
    let initial_stock_price = stock_simulator.current_price();

    let option_type = OptionType::Call;
    let strike_price = 160.0;
    let time_to_expiration_days = 90;
    let initial_volatility = 0.20;
    let risk_free_rate = 0.05;
    let volatility_window = 20;

    let mut option_pricer = OptionPricer::new(
        option_type, strike_price, time_to_expiration_days as f64 / 252.0,
        risk_free_rate, initial_volatility, volatility_window
    );

    let (initial_option_price, initial_greeks) = option_pricer.calculate_price_and_greeks(initial_stock_price, 0);

    let app_state = VisualizerApp {
        stock_simulator,
        option_pricer,
        run_price_histories: Vec::new(),
        current_run_history: vec![initial_stock_price],
        current_option_price: initial_option_price,
        current_greeks: initial_greeks,
        num_runs_to_batch: 100,
        is_batch_running: false, // Start not running a batch
        batch_runs_done: 0,
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
            .with_inner_size([900.0, 700.0])
            .with_title("Stateful Pricer Visualizer"),
        ..Default::default()
    };

    eframe::run_native(
        "Stateful Pricer Visualizer App",
        native_options,
        Box::new(|_cc| Box::new(app_state)),
    )
}