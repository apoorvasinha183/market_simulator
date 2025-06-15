// src/bin/visualizer.rs

use eframe::egui;
use egui::{Color32, FontId, Frame, Margin, ProgressBar, RichText, Rounding, Stroke, Vec2};
use egui_plot::{Legend, Line, Plot, PlotPoints};
use market_simulator::{GBMSimulator, Greeks, Marketable, OptionPricer, OptionType};
use std::time::{Duration, Instant};

// Custom color palette
struct Theme {
    primary: Color32,
    secondary: Color32,
    accent: Color32,
    success: Color32,
    warning: Color32,
    danger: Color32,
    background: Color32,
    surface: Color32,
    surface_variant: Color32,
    text_primary: Color32,
    text_secondary: Color32,
}

impl Theme {
    fn new() -> Self {
        Self {
            primary: Color32::from_rgb(79, 70, 229),          // Indigo
            secondary: Color32::from_rgb(99, 102, 241),       // Indigo lighter
            accent: Color32::from_rgb(236, 72, 153),          // Pink
            success: Color32::from_rgb(34, 197, 94),          // Green
            warning: Color32::from_rgb(251, 191, 36),         // Amber
            danger: Color32::from_rgb(239, 68, 68),           // Red
            background: Color32::from_rgb(15, 23, 42),        // Slate 900
            surface: Color32::from_rgb(30, 41, 59),           // Slate 800
            surface_variant: Color32::from_rgb(51, 65, 85),   // Slate 700
            text_primary: Color32::from_rgb(248, 250, 252),   // Slate 50
            text_secondary: Color32::from_rgb(203, 213, 225), // Slate 300
        }
    }
}

struct VisualizerApp {
    // World state
    stock_simulator: Box<dyn Marketable>,
    option_pricer: OptionPricer,
    theme: Theme,

    // --- State for Multi-Run ---
    run_price_histories: Vec<Vec<f64>>,
    current_run_history: Vec<f64>,

    // Current values for the real-time data panel
    current_option_price: f64,
    current_greeks: Greeks,

    // --- State for non-blocking Batch Mode ---
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

    // UI state
    #[allow(dead_code)]
    show_parameters: bool,
}

impl eframe::App for VisualizerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Set custom style
        self.setup_custom_style(ctx);

        // --- Non-blocking batch processing ---
        if self.is_batch_running {
            let runs_per_frame = 20;
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
                self.is_batch_running = false;
            }
        }

        // Interactive, animated mode
        if self.is_playing && self.last_update.elapsed() > Duration::from_millis(50) {
            let current_day = self.current_run_history.len() - 1;

            if current_day < self.time_to_expiration_days as usize {
                let new_stock_price = self.stock_simulator.step();
                self.current_run_history.push(new_stock_price);

                let (new_option_price, new_greeks) = self
                    .option_pricer
                    .calculate_price_and_greeks(new_stock_price, current_day as u32 + 1);
                self.current_option_price = new_option_price;
                self.current_greeks = new_greeks;
            } else {
                self.is_playing = false;
                if self.current_run_history.len() > 1 {
                    self.run_price_histories
                        .push(self.current_run_history.clone());
                }
                self.current_run_history.clear();
            }
            self.last_update = Instant::now();
        }
        ctx.request_repaint();

        // Main UI
        egui::CentralPanel::default().show(ctx, |ui| {
            self.render_header(ui);
            ui.add_space(16.0);

            // Main content area with proper spacing
            ui.horizontal(|ui| {
                // Left sidebar for controls
                ui.vertical(|ui| {
                    ui.set_width(320.0);
                    self.render_controls_panel(ui);
                    ui.add_space(12.0);
                    self.render_option_data_panel(ui);
                });

                ui.add_space(16.0);
                ui.separator();
                ui.add_space(16.0);

                // Right side for the chart
                ui.vertical(|ui| {
                    self.render_chart_panel(ui);
                });
            });
        });
    }
}

impl VisualizerApp {
    fn render_greek_row(&self, ui: &mut egui::Ui, label: &str, value: f64, color: Color32) {
        ui.label(RichText::new(label).color(self.theme.text_secondary));
        ui.label(RichText::new(format!("{:.4}", value)).color(color).strong());
        ui.end_row();
    }
    fn setup_custom_style(&self, ctx: &egui::Context) {
        let mut style = (*ctx.style()).clone();

        // Set background colors
        style.visuals.window_fill = self.theme.background;
        style.visuals.panel_fill = self.theme.background;
        style.visuals.faint_bg_color = self.theme.surface;
        style.visuals.extreme_bg_color = self.theme.surface_variant;

        // Set widget colors
        style.visuals.widgets.inactive.bg_fill = self.theme.surface;
        style.visuals.widgets.hovered.bg_fill = self.theme.surface_variant;
        style.visuals.widgets.active.bg_fill = self.theme.primary;

        // Set button styling
        style.visuals.widgets.inactive.rounding = Rounding::same(8.0);
        style.visuals.widgets.hovered.rounding = Rounding::same(8.0);
        style.visuals.widgets.active.rounding = Rounding::same(8.0);

        ctx.set_style(style);
    }

    fn render_header(&mut self, ui: &mut egui::Ui) {
        Frame::none()
            .fill(self.theme.surface)
            .rounding(Rounding::same(12.0))
            .inner_margin(Margin::symmetric(20.0, 16.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Title with icon
                    ui.label(
                        RichText::new("📈 Monte Carlo Options Simulator")
                            .font(FontId::proportional(24.0))
                            .color(self.theme.text_primary)
                            .strong(),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Control buttons with custom styling
                        if self
                            .styled_button(
                                ui,
                                if self.is_playing {
                                    "⏸ Pause"
                                } else {
                                    "▶ Play"
                                },
                                self.theme.primary,
                            )
                            .clicked()
                        {
                            if !self.is_playing && self.current_run_history.is_empty() {
                                self.start_new_run();
                            }
                            self.is_playing = !self.is_playing;
                            self.last_update = Instant::now();
                        }

                        ui.add_space(8.0);

                        if self
                            .styled_button(ui, "🆕 New Run", self.theme.success)
                            .clicked()
                        {
                            self.start_new_run();
                            self.is_playing = true;
                        }

                        ui.add_space(8.0);

                        if self
                            .styled_button(ui, "🗑 Clear All", self.theme.danger)
                            .clicked()
                        {
                            self.clear_all_runs();
                        }
                    });
                });
            });
    }

    fn render_controls_panel(&mut self, ui: &mut egui::Ui) {
        // Batch Controls Card
        Frame::none()
            .fill(self.theme.surface)
            .rounding(Rounding::same(12.0))
            .inner_margin(Margin::symmetric(16.0, 12.0))
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("Batch Simulation")
                            .font(FontId::proportional(16.0))
                            .color(self.theme.text_primary)
                            .strong(),
                    );
                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Batch size:").color(self.theme.text_secondary));
                        ui.add_enabled(
                            !self.is_batch_running,
                            egui::DragValue::new(&mut self.num_runs_to_batch)
                                .speed(10.0)
                                .clamp_range(1..=10000),
                        );
                    });

                    ui.add_space(8.0);

                    if self
                        .styled_button(
                            ui,
                            "⚡ Run Batch",
                            if self.is_batch_running {
                                self.theme.text_secondary
                            } else {
                                self.theme.warning
                            },
                        )
                        .clicked()
                        && !self.is_batch_running
                    {
                        self.run_batch_simulations();
                    }

                    // Progress bar
                    if self.is_batch_running {
                        ui.add_space(8.0);
                        let progress = self.batch_runs_done as f32 / self.num_runs_to_batch as f32;
                        let progress_text =
                            format!("{}/{} runs", self.batch_runs_done, self.num_runs_to_batch);

                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new("Progress")
                                    .color(self.theme.text_secondary)
                                    .small(),
                            );
                            let progress_bar = ProgressBar::new(progress)
                                .text(progress_text)
                                .fill(self.theme.accent);
                            ui.add(progress_bar);
                        });
                    }
                })
            });

        ui.add_space(12.0);

        // Parameters Card
        Frame::none()
            .fill(self.theme.surface)
            .rounding(Rounding::same(12.0))
            .inner_margin(Margin::symmetric(16.0, 12.0))
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("Simulation Parameters")
                            .font(FontId::proportional(16.0))
                            .color(self.theme.text_primary)
                            .strong(),
                    );
                    ui.add_space(8.0);

                    ui.collapsing("📊 Market Parameters", |ui| {
                        ui.horizontal(|ui| {
                            ui.set_width(ui.available_width());
                            ui.label(
                                RichText::new("Strike Price:").color(self.theme.text_secondary),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.add(
                                        egui::DragValue::new(&mut self.strike_price)
                                            .speed(1.0)
                                            .prefix("$"),
                                    );
                                },
                            );
                        });
                        ui.add_space(4.0);

                        ui.horizontal(|ui| {
                            ui.set_width(ui.available_width());
                            ui.label(
                                RichText::new("Days to Expiry:").color(self.theme.text_secondary),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.add(
                                        egui::DragValue::new(&mut self.time_to_expiration_days)
                                            .speed(1.0)
                                            .suffix(" days"),
                                    );
                                },
                            );
                        });
                        ui.add_space(4.0);

                        ui.horizontal(|ui| {
                            ui.set_width(ui.available_width());
                            ui.label(
                                RichText::new("Risk-Free Rate:").color(self.theme.text_secondary),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.add(
                                        egui::DragValue::new(&mut self.risk_free_rate)
                                            .speed(0.001)
                                            .suffix("%"),
                                    );
                                },
                            );
                        });
                        ui.add_space(4.0);

                        ui.horizontal(|ui| {
                            ui.set_width(ui.available_width());
                            ui.label(
                                RichText::new("Initial Volatility:")
                                    .color(self.theme.text_secondary),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.add(
                                        egui::DragValue::new(&mut self.initial_volatility)
                                            .speed(0.001)
                                            .suffix("%"),
                                    );
                                },
                            );
                        });
                        ui.add_space(4.0);

                        ui.horizontal(|ui| {
                            ui.set_width(ui.available_width());
                            ui.label(
                                RichText::new("Volatility Window:")
                                    .color(self.theme.text_secondary),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.add(
                                        egui::DragValue::new(&mut self.volatility_window)
                                            .speed(1.0)
                                            .suffix(" days"),
                                    );
                                },
                            );
                        });
                        ui.add_space(8.0);

                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("Option Type:").color(self.theme.text_secondary),
                            );
                            ui.radio_value(&mut self.option_type, OptionType::Call, "📈 Call");
                            ui.radio_value(&mut self.option_type, OptionType::Put, "📉 Put");
                        });
                    });
                })
            });
    }

    fn render_option_data_panel(&self, ui: &mut egui::Ui) {
        Frame::none()
            .fill(self.theme.surface)
            .rounding(Rounding::same(12.0))
            .inner_margin(Margin::symmetric(16.0, 12.0))
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("Option Metrics")
                            .font(FontId::proportional(16.0))
                            .color(self.theme.text_primary)
                            .strong(),
                    );
                    ui.add_space(8.0);

                    // Option Price - Featured prominently
                    Frame::none()
                        .fill(self.theme.primary.linear_multiply(0.1))
                        .rounding(Rounding::same(8.0))
                        .inner_margin(Margin::symmetric(12.0, 8.0))
                        .show(ui, |ui| {
                            ui.vertical_centered(|ui| {
                                ui.label(
                                    RichText::new("Current Price").color(self.theme.text_secondary),
                                );
                                ui.label(
                                    RichText::new(format!("${:.2}", self.current_option_price))
                                        .font(FontId::proportional(28.0))
                                        .color(self.theme.success)
                                        .strong(),
                                );
                            });
                        });

                    ui.add_space(12.0);

                    // Greeks in a clean grid
                    ui.label(
                        RichText::new("The Greeks")
                            .color(self.theme.text_primary)
                            .strong(),
                    );
                    ui.add_space(4.0);

                    egui::Grid::new("greeks_grid")
                        .num_columns(2)
                        .spacing([16.0, 8.0])
                        .show(ui, |ui| {
                            self.render_greek_row(
                                ui,
                                "Δ Delta",
                                self.current_greeks.delta,
                                self.theme.primary,
                            );
                            self.render_greek_row(
                                ui,
                                "Γ Gamma",
                                self.current_greeks.gamma,
                                self.theme.secondary,
                            );
                            self.render_greek_row(
                                ui,
                                "ν Vega",
                                self.current_greeks.vega,
                                self.theme.accent,
                            );
                            self.render_greek_row(
                                ui,
                                "Θ Theta",
                                self.current_greeks.theta,
                                self.theme.warning,
                            );
                            self.render_greek_row(
                                ui,
                                "ρ Rho",
                                self.current_greeks.rho,
                                self.theme.success,
                            );
                        });
                })
            });
    }

    fn render_chart_panel(&self, ui: &mut egui::Ui) {
        Frame::none()
            .fill(self.theme.surface)
            .rounding(Rounding::same(12.0))
            .inner_margin(Margin::symmetric(16.0, 12.0))
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("Price Simulation")
                            .font(FontId::proportional(16.0))
                            .color(self.theme.text_primary)
                            .strong(),
                    );
                    ui.add_space(8.0);

                    let chart_height = ui.available_height() - 40.0;

                    Frame::none()
                        .fill(Color32::from_rgb(10, 15, 25))
                        .rounding(Rounding::same(8.0))
                        .inner_margin(Margin::symmetric(8.0, 8.0))
                        .show(ui, |ui| {
                            Plot::new("stock_plot")
                                .height(chart_height)
                                .width(ui.available_width())
                                .legend(Legend::default())
                                .show_background(false)
                                .show(ui, |plot_ui| {
                                    // Historical runs with subtle styling
                                    for (i, history) in self.run_price_histories.iter().enumerate()
                                    {
                                        let alpha = (255.0 * (0.1 + 0.02 * (i % 10) as f32)) as u8;
                                        let line = Line::new(PlotPoints::from_ys_f64(history))
                                            .color(Color32::from_rgba_unmultiplied(
                                                100, 100, 120, alpha,
                                            ))
                                            .stroke(Stroke::new(
                                                1.0,
                                                Color32::from_rgba_unmultiplied(
                                                    100, 100, 120, alpha,
                                                ),
                                            ));
                                        plot_ui.line(line);
                                    }

                                    // Current run with vibrant styling
                                    if !self.current_run_history.is_empty() {
                                        let active_line = Line::new(PlotPoints::from_ys_f64(
                                            &self.current_run_history,
                                        ))
                                        .color(self.theme.accent)
                                        .stroke(Stroke::new(3.0, self.theme.accent))
                                        .name("🎯 Current Run");
                                        plot_ui.line(active_line);
                                    }

                                    // Strike price line (solid line instead of dashed)
                                    let strike_line = Line::new(PlotPoints::from_iter(
                                        (0..self.time_to_expiration_days + 10)
                                            .map(|x| [x as f64, self.strike_price]),
                                    ))
                                    .color(self.theme.warning)
                                    .stroke(Stroke::new(2.0, self.theme.warning))
                                    .name("💰 Strike Price");
                                    plot_ui.line(strike_line);
                                });
                        });
                })
            });
    }

    fn styled_button(&self, ui: &mut egui::Ui, text: &str, color: Color32) -> egui::Response {
        // Use egui's built-in button but with custom styling
        let button = egui::Button::new(RichText::new(text).color(Color32::WHITE).strong())
            .fill(color)
            .rounding(Rounding::same(8.0))
            .min_size(Vec2::new(ui.available_width().min(120.0), 32.0));

        ui.add(button)
    }

    fn start_new_run(&mut self) {
        if self.current_run_history.len() > 1 {
            self.run_price_histories
                .push(self.current_run_history.clone());
        }
        self.stock_simulator.reset();
        self.option_pricer = OptionPricer::new(
            self.option_type,
            self.strike_price,
            self.time_to_expiration_days as f64 / 252.0,
            self.risk_free_rate,
            self.initial_volatility,
            self.volatility_window,
        );
        let initial_stock_price = self.stock_simulator.current_price();
        self.current_run_history = vec![initial_stock_price];
        let (price, greeks) = self
            .option_pricer
            .calculate_price_and_greeks(initial_stock_price, 0);
        self.current_option_price = price;
        self.current_greeks = greeks;
        self.is_playing = false;
    }

    fn clear_all_runs(&mut self) {
        self.run_price_histories.clear();
        self.current_run_history.clear();
        self.start_new_run();
    }

    fn run_batch_simulations(&mut self) {
        self.clear_all_runs();
        self.is_playing = false;
        self.batch_runs_done = 0;
        self.is_batch_running = true;
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
        option_type,
        strike_price,
        time_to_expiration_days as f64 / 252.0,
        risk_free_rate,
        initial_volatility,
        volatility_window,
    );

    let (initial_option_price, initial_greeks) =
        option_pricer.calculate_price_and_greeks(initial_stock_price, 0);

    let app_state = VisualizerApp {
        stock_simulator,
        option_pricer,
        theme: Theme::new(),
        run_price_histories: Vec::new(),
        current_run_history: vec![initial_stock_price],
        current_option_price: initial_option_price,
        current_greeks: initial_greeks,
        num_runs_to_batch: 100,
        is_batch_running: false,
        batch_runs_done: 0,
        strike_price,
        time_to_expiration_days,
        risk_free_rate,
        option_type,
        initial_volatility,
        volatility_window,
        is_playing: false,
        last_update: Instant::now(),
        show_parameters: false,
    };

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_title("Monte Carlo Options Simulator")
            .with_min_inner_size([800.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Monte Carlo Options Simulator",
        native_options,
        Box::new(|_cc| Box::new(app_state)),
    )
}
