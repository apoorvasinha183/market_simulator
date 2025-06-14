// src/bin/visual_order.rs

use eframe::egui;
use egui::{Color32, FontId, RichText, Rounding, Stroke, Vec2};
use egui_plot::{Legend, Line, Plot, PlotBounds, PlotPoints, Points};
use market_simulator::{AgentType, Market, Marketable};
use std::time::{Duration, Instant};

// Helper function to format numbers with thousand separators
fn format_number(n: i32) -> String {
    let s = n.to_string();
    let mut result = String::new();
    let chars: Vec<char> = s.chars().collect();

    for (i, c) in chars.iter().enumerate() {
        if i > 0 && (chars.len() - i) % 3 == 0 {
            result.push(',');
        }
        result.push(*c);
    }
    result
}

fn format_number_u64(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    let chars: Vec<char> = s.chars().collect();

    for (i, c) in chars.iter().enumerate() {
        if i > 0 && (chars.len() - i) % 3 == 0 {
            result.push(',');
        }
        result.push(*c);
    }
    result
}

struct AgentVisualizer {
    simulator: Box<dyn Marketable>,
    price_history: Vec<f64>,
    is_market_running: bool,
    last_update: Instant,
    theme_dark: bool,
    animation_time: f64,
    // --- NEW: Added fields to track session high and low ---
    ath: f64,
    atl: f64,
}

impl eframe::App for AgentVisualizer {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Update animation time
        self.animation_time = ctx.input(|i| i.time);

        // Apply custom styling
        self.apply_custom_style(ctx);

        if self.is_market_running && self.last_update.elapsed() > Duration::from_millis(100) {
            let new_price = self.simulator.step();

            // --- NEW: Update ATH and ATL with the new price ---
            self.ath = self.ath.max(new_price);
            self.atl = self.atl.min(new_price);

            if self.price_history.last() != Some(&new_price) {
                self.price_history.push(new_price);
                // Keep only last 1000 points for performance
                if self.price_history.len() > 1000 {
                    self.price_history.remove(0);
                }
            }
            self.last_update = Instant::now();
        }
        ctx.request_repaint();

        // Enhanced top panel with gradient background
        egui::TopBottomPanel::top("top_panel")
            .min_height(60.0)
            .show(ctx, |ui| {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    // Market title with larger font and color
                    ui.label(
                        RichText::new("🚀 Live Agent-Based Market")
                            .font(FontId::proportional(24.0))
                            .color(if self.theme_dark {
                                Color32::WHITE
                            } else {
                                Color32::from_rgb(40, 40, 40)
                            })
                            .strong(),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Theme toggle button
                        if ui
                            .button(if self.theme_dark {
                                "☀ Light"
                            } else {
                                "🌙 Dark"
                            })
                            .clicked()
                        {
                            self.theme_dark = !self.theme_dark;
                        }

                        ui.separator();

                        // Enhanced control buttons with icons and colors
                        let start_stop_button = if self.is_market_running {
                            egui::Button::new(RichText::new("⏸ Pause").color(Color32::WHITE))
                                .fill(Color32::from_rgb(220, 53, 69))
                        } else {
                            egui::Button::new(RichText::new("▶ Start").color(Color32::WHITE))
                                .fill(Color32::from_rgb(40, 167, 69))
                        };

                        if ui
                            .add(start_stop_button.rounding(Rounding::same(8.0)))
                            .clicked()
                        {
                            self.is_market_running = !self.is_market_running;
                        }

                        let reset_button =
                            egui::Button::new(RichText::new("🔄 Reset").color(Color32::WHITE))
                                .fill(Color32::from_rgb(108, 117, 125))
                                .rounding(Rounding::same(8.0));

                        if ui.add(reset_button).clicked() {
                            self.reset_simulation();
                        }
                    });
                });
                ui.add_space(8.0);
            });

        if let Some(market) = self.simulator.as_any().downcast_ref::<Market>() {
            let order_book = market.get_order_book();

            // Enhanced bottom panel with better styling
            egui::TopBottomPanel::bottom("bottom_panel")
                .resizable(true)
                .min_height(250.0)
                .show(ctx, |ui| {
                    ui.add_space(8.0);

                    egui::ScrollArea::vertical().show(ui, |ui| {
                        // Market status bar
                        self.render_market_status(ui, market);

                        ui.add_space(12.0);

                        // Enhanced order book display
                        ui.vertical_centered(|ui| {
                            ui.label(
                                RichText::new("📊 Live Order Book")
                                    .font(FontId::proportional(18.0))
                                    .strong(),
                            );
                        });

                        ui.add_space(8.0);

                        ui.horizontal_top(|ui| {
                            // Enhanced Bids Panel
                            ui.vertical(|ui| {
                                ui.set_width(ui.available_width() / 2.0 - 10.0);

                                // Bids header with background
                                let bids_rect = ui.available_rect_before_wrap();
                                let bids_bg_rect = egui::Rect::from_min_size(
                                    bids_rect.min,
                                    Vec2::new(bids_rect.width(), 30.0),
                                );
                                ui.painter().rect_filled(
                                    bids_bg_rect,
                                    Rounding::same(6.0),
                                    Color32::from_rgba_unmultiplied(40, 167, 69, 30),
                                );

                                ui.add_space(4.0);
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(" Bids")
                                            .color(Color32::from_rgb(40, 167, 69))
                                            .strong(),
                                    );
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            ui.label(
                                                RichText::new("").color(Color32::GRAY).italics(),
                                            );
                                        },
                                    );
                                });
                                ui.add_space(4.0);

                                // Enhanced grid with alternating row colors
                                let mut row_count = 0;
                                egui::Grid::new("bids_grid")
                                    .spacing([20.0, 4.0])
                                    .show(ui, |ui| {
                                        ui.label(RichText::new("Price").underline().strong());
                                        ui.label(RichText::new("Volume").underline().strong());
                                        ui.end_row();

                                        if order_book.bids.is_empty() {
                                            // Show N/A when no bids
                                            ui.label(
                                                RichText::new("N/A").color(Color32::GRAY).italics(),
                                            );
                                            ui.label(
                                                RichText::new("N/A").color(Color32::GRAY).italics(),
                                            );
                                            ui.end_row();
                                        } else {
                                            for (price, level) in
                                                order_book.bids.iter().rev().take(10)
                                            {
                                                // Alternating row background
                                                if row_count % 2 == 0 {
                                                    let row_rect = ui.available_rect_before_wrap();
                                                    ui.painter().rect_filled(
                                                        row_rect,
                                                        Rounding::same(3.0),
                                                        Color32::from_rgba_unmultiplied(
                                                            0, 0, 0, 10,
                                                        ),
                                                    );
                                                }

                                                ui.label(
                                                    RichText::new(format!(
                                                        "${:.2}",
                                                        *price as f64 / 100.0
                                                    ))
                                                    .color(Color32::from_rgb(40, 167, 69))
                                                    .monospace(),
                                                );
                                                ui.label(
                                                    RichText::new(format_number(
                                                        level.total_volume as i32,
                                                    ))
                                                    .monospace(),
                                                );
                                                ui.end_row();
                                                row_count += 1;
                                            }
                                        }
                                    });
                            });

                            ui.add_space(20.0);

                            // Enhanced Asks Panel
                            ui.vertical(|ui| {
                                // Asks header with background
                                let asks_rect = ui.available_rect_before_wrap();
                                let asks_bg_rect = egui::Rect::from_min_size(
                                    asks_rect.min,
                                    Vec2::new(asks_rect.width(), 30.0),
                                );
                                ui.painter().rect_filled(
                                    asks_bg_rect,
                                    Rounding::same(6.0),
                                    Color32::from_rgba_unmultiplied(220, 53, 69, 30),
                                );

                                ui.add_space(4.0);
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(" Asks")
                                            .color(Color32::from_rgb(220, 53, 69))
                                            .strong(),
                                    );
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            ui.label(
                                                RichText::new("").color(Color32::GRAY).italics(),
                                            );
                                        },
                                    );
                                });
                                ui.add_space(4.0);

                                let mut row_count = 0;
                                egui::Grid::new("asks_grid")
                                    .spacing([20.0, 4.0])
                                    .show(ui, |ui| {
                                        ui.label(RichText::new("Price").underline().strong());
                                        ui.label(RichText::new("Volume").underline().strong());
                                        ui.end_row();

                                        if order_book.asks.is_empty() {
                                            // Show N/A when no asks
                                            ui.label(
                                                RichText::new("N/A").color(Color32::GRAY).italics(),
                                            );
                                            ui.label(
                                                RichText::new("N/A").color(Color32::GRAY).italics(),
                                            );
                                            ui.end_row();
                                        } else {
                                            for (price, level) in order_book.asks.iter().take(10) {
                                                if row_count % 2 == 0 {
                                                    let row_rect = ui.available_rect_before_wrap();
                                                    ui.painter().rect_filled(
                                                        row_rect,
                                                        Rounding::same(3.0),
                                                        Color32::from_rgba_unmultiplied(
                                                            0, 0, 0, 10,
                                                        ),
                                                    );
                                                }

                                                ui.label(
                                                    RichText::new(format!(
                                                        "${:.2}",
                                                        *price as f64 / 100.0
                                                    ))
                                                    .color(Color32::from_rgb(220, 53, 69))
                                                    .monospace(),
                                                );
                                                ui.label(
                                                    RichText::new(format_number(
                                                        level.total_volume as i32,
                                                    ))
                                                    .monospace(),
                                                );
                                                ui.end_row();
                                                row_count += 1;
                                            }
                                        }
                                    });
                            });
                        });
                    });
                });

            // Enhanced central panel with better plot styling
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.columns(2, |columns| {
                    // Enhanced Depth Chart
                    columns[0].group(|ui| {
                        ui.vertical_centered(|ui| {
                            ui.label(
                                RichText::new("📈 Order Book Depth")
                                    .font(FontId::proportional(16.0))
                                    .strong(),
                            );
                        });

                        Plot::new("order_book_plot")
                            .legend(Legend::default())
                            .show_axes([true, true])
                            .show_grid([true, true])
                            .show(ui, |plot_ui| {
                                // Enhanced ask curve with gradient
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
                                        .fill(0.0)
                                        .color(Color32::from_rgb(220, 53, 69))
                                        .stroke(Stroke::new(2.5, Color32::from_rgb(220, 53, 69)))
                                        .name("📈 Asks"),
                                );

                                // Enhanced bid curve
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
                                        .fill(0.0)
                                        .color(Color32::from_rgb(40, 167, 69))
                                        .stroke(Stroke::new(2.5, Color32::from_rgb(40, 167, 69)))
                                        .name("📉 Bids"),
                                );

                                let center_px = market.current_price();
                                let half_win = 20.00;
                                let y_max = 2_000_000.00;
                                plot_ui.set_plot_bounds(PlotBounds::from_min_max(
                                    [center_px - half_win, 0.0],
                                    [center_px + half_win, y_max],
                                ));
                            });
                    });

                    // Enhanced Price History Chart
                    columns[1].group(|ui| {
                        ui.vertical_centered(|ui| {
                            ui.label(
                                RichText::new("💹 Price History")
                                    .font(FontId::proportional(16.0))
                                    .strong(),
                            );
                        });

                        Plot::new("price_history_plot")
                            .legend(Legend::default())
                            .show_axes([true, true])
                            .show_grid([true, true])
                            .show(ui, |plot_ui| {
                                // Enhanced price line with gradient effect
                                let line = Line::new(PlotPoints::from_ys_f64(&self.price_history))
                                    .color(Color32::from_rgb(0, 123, 255))
                                    .stroke(Stroke::new(3.0, Color32::from_rgb(0, 123, 255)))
                                    .fill(-1.0); // Fill below the line
                                plot_ui.line(line.name("💰 Last Traded Price"));

                                // Enhanced blinking indicator
                                if let Some(&last_price) = self.price_history.last() {
                                    let x = (self.price_history.len() - 1) as f64;

                                    // More sophisticated pulsing animation
                                    let pulse = (self.animation_time * 4.0).sin().abs();
                                    let radius = 4.0 + pulse * 4.0;
                                    let alpha = (128.0 + pulse * 127.0) as u8;

                                    let last_point_marker = Points::new(vec![[x, last_price]])
                                        .radius(radius as f32)
                                        .color(Color32::from_rgba_unmultiplied(255, 215, 0, alpha));

                                    plot_ui.points(last_point_marker);

                                    // Add a subtle ring effect
                                    let ring_radius = 6.0 + pulse * 2.0;
                                    let ring_marker = Points::new(vec![[x, last_price]])
                                        .radius(ring_radius as f32)
                                        .color(Color32::from_rgba_unmultiplied(
                                            255,
                                            215,
                                            0,
                                            (64.0 * (1.0 - pulse)) as u8,
                                        ))
                                        .filled(false);
                                    plot_ui.points(ring_marker);
                                }
                            });
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
        let initial_price = self.simulator.current_price();
        self.price_history = vec![initial_price];
        // --- NEW: Reset ATH and ATL on simulation reset ---
        self.ath = initial_price;
        self.atl = initial_price;
    }

    fn apply_custom_style(&self, ctx: &egui::Context) {
        let mut style = (*ctx.style()).clone();

        if self.theme_dark {
            // Dark theme
            style.visuals.dark_mode = true;
            style.visuals.panel_fill = Color32::from_rgb(32, 32, 36);
            style.visuals.window_fill = Color32::from_rgb(40, 40, 44);
            style.visuals.extreme_bg_color = Color32::from_rgb(24, 24, 28);
        } else {
            // Light theme with custom colors
            style.visuals.dark_mode = false;
            style.visuals.panel_fill = Color32::from_rgb(248, 249, 250);
            style.visuals.window_fill = Color32::WHITE;
        }

        // Enhanced button styling
        style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(108, 117, 125);
        style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(90, 98, 104);

        // Rounded corners
        style.visuals.widgets.noninteractive.rounding = Rounding::same(6.0);
        style.visuals.widgets.inactive.rounding = Rounding::same(6.0);
        style.visuals.widgets.hovered.rounding = Rounding::same(6.0);
        style.visuals.widgets.active.rounding = Rounding::same(6.0);

        ctx.set_style(style);
    }

    fn render_market_status(&self, ui: &mut egui::Ui, market: &Market) {
        let order_book = market.get_order_book();
        let best_bid = order_book.bids.keys().last().cloned();
        let best_ask = order_book.asks.keys().next().cloned();
        let total_inventory = market.get_total_inventory();

        if let (Some(bid), Some(ask)) = (best_bid, best_ask) {
            if ask > bid {
                let spread = (ask - bid) as f64 / 100.0;
                let spread_pct = (spread / (ask as f64 / 100.0)) * 100.0;

                // Status indicator
                let status_color = if self.is_market_running {
                    Color32::from_rgb(40, 167, 69)
                } else {
                    Color32::from_rgb(108, 117, 125)
                };

                ui.horizontal(|ui| {
                    // Market status indicator
                    ui.add_space(8.0);
                    let circle_center = ui.cursor().min + Vec2::new(6.0, 8.0);
                    ui.painter().circle_filled(circle_center, 4.0, status_color);
                    ui.add_space(16.0);

                    ui.label(
                        RichText::new(if self.is_market_running {
                            "🟢 LIVE"
                        } else {
                            "⏸️ PAUSED"
                        })
                        .color(status_color)
                        .strong(),
                    );

                    ui.separator();

                    // Market metrics with better formatting
                    ui.horizontal(|ui| {
                        ui.label("💰");
                        ui.label(RichText::new("Bid:").strong());
                        ui.label(
                            RichText::new(format!("${:.2}", bid as f64 / 100.0))
                                .color(Color32::from_rgb(40, 167, 69))
                                .monospace(),
                        );
                    });

                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.label("💸");
                        ui.label(RichText::new("Ask:").strong());
                        ui.label(
                            RichText::new(format!("${:.2}", ask as f64 / 100.0))
                                .color(Color32::from_rgb(220, 53, 69))
                                .monospace(),
                        );
                    });

                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.label("📊");
                        ui.label(RichText::new("Spread:").strong());
                        ui.label(
                            RichText::new(format!("${:.2} ({:.2}%)", spread, spread_pct))
                                .color(Color32::from_rgb(255, 193, 7))
                                .monospace(),
                        );
                    });
                    
                    // --- NEW: Added ATH and ATL Display ---
                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.label("🚀");
                        ui.label(RichText::new("ATH:").strong());
                        ui.label(
                            RichText::new(format!("${:.2}", self.ath))
                                .color(Color32::from_rgb(40, 167, 69)) // Green for high
                                .monospace(),
                        );
                    });

                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.label("⚓");
                        ui.label(RichText::new("ATL:").strong());
                        ui.label(
                            RichText::new(format!("${:.2}", self.atl))
                                .color(Color32::from_rgb(220, 53, 69)) // Red for low
                                .monospace(),
                        );
                    });
                    // --- END OF NEW ---

                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.label("📈");
                        ui.label(RichText::new("Volume:").strong());
                        ui.label(
                            RichText::new(format_number_u64(market.cumulative_volume()))
                                .monospace(),
                        );
                    });

                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.label("⚖️");
                        ui.label(RichText::new("Net Inventory:").strong());
                        let inventory_str = if total_inventory >= 0 {
                            format!("+{}", format_number(total_inventory as i32))
                        } else {
                            format_number(total_inventory as i32)
                        };
                        ui.label(
                            RichText::new(inventory_str)
                                .color(if total_inventory > 0 {
                                    Color32::from_rgb(40, 167, 69)
                                } else if total_inventory < 0 {
                                    Color32::from_rgb(220, 53, 69)
                                } else {
                                    Color32::GRAY
                                })
                                .monospace(),
                        );
                    });
                });
            }
        }
    }
}

fn main() -> Result<(), eframe::Error> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 900.0])
            .with_min_inner_size([1000.0, 700.0]),
        ..Default::default()
    };

    let participants = vec![
        AgentType::MarketMaker,
        AgentType::DumbLimit,
        AgentType::DumbMarket,
        AgentType::WhaleAgent,
    ];

    let simulator: Box<dyn Marketable> = Box::new(Market::new(&participants));
    // --- NEW: Capture initial price to set starting ATH/ATL ---
    let initial_price = simulator.current_price();
    let app_state = AgentVisualizer {
        price_history: vec![initial_price],
        simulator,
        is_market_running: false,
        last_update: Instant::now(),
        theme_dark: true, // Start with dark theme
        animation_time: 0.0,
        ath: initial_price,
        atl: initial_price,
    };

    eframe::run_native(
        "🚀 Live Agent-Based Market Visualizer",
        native_options,
        Box::new(|_cc| Box::new(app_state)),
    )
}
