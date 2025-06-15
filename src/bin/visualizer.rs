// src/bin/visual_order.rs

use eframe::egui;
use egui::{ComboBox, Color32, FontId, RichText, Rounding, Stroke, Vec2};
use egui_plot::{Legend, Line, Plot, PlotBounds, PlotPoints, Points};
// NEW: We need the stock module to get the Symbol type and the registry
use market_simulator::{
    stocks::{registry, Symbol},
    AgentType, Market, Marketable, OrderBook,
};
use std::collections::HashMap;
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
    // CHANGED: State is now per-symbol
    price_histories: HashMap<Symbol, Vec<f64>>,
    aths: HashMap<Symbol, f64>,
    atls: HashMap<Symbol, f64>,
    // NEW: State for tracking the selected symbol
    selected_symbol: Symbol,
    available_symbols: Vec<Symbol>,

    is_market_running: bool,
    last_update: Instant,
    theme_dark: bool,
    animation_time: f64,
}

impl eframe::App for AgentVisualizer {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.animation_time = ctx.input(|i| i.time);
        self.apply_custom_style(ctx);

        if self.is_market_running && self.last_update.elapsed() > Duration::from_millis(100) {
            self.simulator.step(); // The step function runs the whole market

            if let Some(market) = self.simulator.as_any().downcast_ref::<Market>() {
                // After the step, update the data for ALL symbols
                for symbol in &self.available_symbols {
                    if let Some(new_price) = market.current_price(symbol) {
                        if let (Some(history), Some(ath), Some(atl)) = (
                            self.price_histories.get_mut(symbol),
                            self.aths.get_mut(symbol),
                            self.atls.get_mut(symbol),
                        ) {
                            *ath = ath.max(new_price);
                            *atl = atl.min(new_price);
                            if history.last() != Some(&new_price) {
                                history.push(new_price);
                                if history.len() > 1000 {
                                    history.remove(0);
                                }
                            }
                        }
                    }
                }
            }
            self.last_update = Instant::now();
        }
        ctx.request_repaint();

        egui::TopBottomPanel::top("top_panel")
            .min_height(60.0)
            .show(ctx, |ui| {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
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

                    // --- NEW: Dropdown for selecting the symbol ---
                    ui.add_space(20.0);
                    ComboBox::from_label("Symbol")
                        .selected_text(self.selected_symbol.clone())
                        .show_ui(ui, |ui| {
                            for symbol in &self.available_symbols {
                                ui.selectable_value(
                                    &mut self.selected_symbol,
                                    symbol.clone(),
                                    symbol.clone(),
                                );
                            }
                        });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(if self.theme_dark { "☀ Light" } else { "🌙 Dark" }).clicked() {
                            self.theme_dark = !self.theme_dark;
                        }
                        ui.separator();
                        let start_stop_button = if self.is_market_running {
                            egui::Button::new(RichText::new("⏸ Pause").color(Color32::WHITE))
                                .fill(Color32::from_rgb(220, 53, 69))
                        } else {
                            egui::Button::new(RichText::new("▶ Start").color(Color32::WHITE))
                                .fill(Color32::from_rgb(40, 167, 69))
                        };
                        if ui.add(start_stop_button.rounding(Rounding::same(8.0))).clicked() {
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

        // CHANGED: The entire UI now renders based on the selected symbol
        if let Some(market) = self.simulator.as_any().downcast_ref::<Market>() {
            if let Some(order_book) = market.get_order_book(&self.selected_symbol) {
                egui::TopBottomPanel::bottom("bottom_panel")
                    .resizable(true)
                    .min_height(250.0)
                    .show(ctx, |ui| {
                        ui.add_space(8.0);
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            self.render_market_status(ui, market, order_book);
                            ui.add_space(12.0);
                            self.render_order_book_display(ui, order_book);
                        });
                    });

                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.columns(2, |columns| {
                        self.render_depth_chart(&mut columns[0], market, order_book);
                        self.render_price_chart(&mut columns[1]);
                    });
                });
            }
        }
    }
}

impl AgentVisualizer {
    fn reset_simulation(&mut self) {
        self.is_market_running = false;
        self.simulator.reset();
        
        if let Some(market) = self.simulator.as_any().downcast_ref::<Market>() {
            for symbol in &self.available_symbols {
                if let Some(initial_price) = market.current_price(symbol) {
                    self.price_histories.insert(symbol.clone(), vec![initial_price]);
                    self.aths.insert(symbol.clone(), initial_price);
                    self.atls.insert(symbol.clone(), initial_price);
                }
            }
        }
    }

    fn apply_custom_style(&self, ctx: &egui::Context) {
        let mut style = (*ctx.style()).clone();
        if self.theme_dark {
            style.visuals.dark_mode = true;
            style.visuals.panel_fill = Color32::from_rgb(32, 32, 36);
            style.visuals.window_fill = Color32::from_rgb(40, 40, 44);
            style.visuals.extreme_bg_color = Color32::from_rgb(24, 24, 28);
        } else {
            style.visuals.dark_mode = false;
            style.visuals.panel_fill = Color32::from_rgb(248, 249, 250);
            style.visuals.window_fill = Color32::WHITE;
        }
        style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(108, 117, 125);
        style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(90, 98, 104);
        style.visuals.widgets.noninteractive.rounding = Rounding::same(6.0);
        style.visuals.widgets.inactive.rounding = Rounding::same(6.0);
        style.visuals.widgets.hovered.rounding = Rounding::same(6.0);
        style.visuals.widgets.active.rounding = Rounding::same(6.0);
        ctx.set_style(style);
    }
    
    // --- NEW: UI rendering logic broken into smaller functions for clarity ---

    fn render_order_book_display(&self, ui: &mut egui::Ui, order_book: &OrderBook) {
        ui.vertical_centered(|ui| { ui.label( RichText::new("📊 Live Order Book").font(FontId::proportional(18.0)).strong()); });
        ui.add_space(8.0);
        ui.horizontal_top(|ui| {
            // Bids Panel
            ui.vertical(|ui| {
                ui.set_width(ui.available_width() / 2.0 - 10.0);
                let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, Rounding::same(6.0), Color32::from_rgba_unmultiplied(40, 167, 69, 30));
                ui.label(RichText::new(" Bids").color(Color32::from_rgb(40, 167, 69)).strong());
                
                egui::Grid::new("bids_grid").spacing([20.0, 4.0]).show(ui, |ui| {
                    ui.label(RichText::new("Price").underline().strong()); ui.label(RichText::new("Volume").underline().strong()); ui.end_row();
                    for (price, level) in order_book.bids.iter().rev().take(10) {
                        ui.label(RichText::new(format!("${:.2}", *price as f64 / 100.0)).color(Color32::from_rgb(40, 167, 69)).monospace());
                        ui.label(RichText::new(format_number_u64(level.total_volume)).monospace());
                        ui.end_row();
                    }
                });
            });
            ui.add_space(20.0);
            // Asks Panel
            ui.vertical(|ui| {
                 let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.0), egui::Sense::hover());
                 ui.painter().rect_filled(rect, Rounding::same(6.0), Color32::from_rgba_unmultiplied(220, 53, 69, 30));
                 ui.label(RichText::new(" Asks").color(Color32::from_rgb(220, 53, 69)).strong());

                egui::Grid::new("asks_grid").spacing([20.0, 4.0]).show(ui, |ui| {
                    ui.label(RichText::new("Price").underline().strong()); ui.label(RichText::new("Volume").underline().strong()); ui.end_row();
                    for (price, level) in order_book.asks.iter().take(10) {
                        ui.label(RichText::new(format!("${:.2}", *price as f64 / 100.0)).color(Color32::from_rgb(220, 53, 69)).monospace());
                        ui.label(RichText::new(format_number_u64(level.total_volume)).monospace());
                        ui.end_row();
                    }
                });
            });
        });
    }

    fn render_depth_chart(&self, ui: &mut egui::Ui, market: &Market, order_book: &OrderBook) {
        ui.group(|ui| {
            ui.vertical_centered(|ui| { ui.label(RichText::new("📈 Order Book Depth").font(FontId::proportional(16.0)).strong()); });
            Plot::new("order_book_plot").legend(Legend::default()).show_axes([true, true]).show_grid([true, true]).show(ui, |plot_ui| {
                let mut ask_pts = Vec::new(); let mut cum_ask = 0.0;
                for (&px, lvl) in order_book.asks.iter() { let p = px as f64 / 100.0; ask_pts.push([p, cum_ask]); cum_ask += lvl.total_volume as f64; ask_pts.push([p, cum_ask]); }
                plot_ui.line(Line::new(PlotPoints::from(ask_pts)).fill(0.0).color(Color32::from_rgb(220, 53, 69)).stroke(Stroke::new(2.5, Color32::from_rgb(220, 53, 69))).name("📈 Asks"));
                let mut bid_pts = Vec::new(); let mut cum_bid = 0.0;
                for (&px, lvl) in order_book.bids.iter().rev() { let p = px as f64 / 100.0; bid_pts.push([p, cum_bid]); cum_bid += lvl.total_volume as f64; bid_pts.push([p, cum_bid]); }
                plot_ui.line(Line::new(PlotPoints::from(bid_pts)).fill(0.0).color(Color32::from_rgb(40, 167, 69)).stroke(Stroke::new(2.5, Color32::from_rgb(40, 167, 69))).name("📉 Bids"));
                if let Some(center_px) = market.current_price(&self.selected_symbol) {
                    plot_ui.set_plot_bounds(PlotBounds::from_min_max([center_px - 25.0, 0.0], [center_px + 25.0, cum_ask.max(cum_bid) * 1.2]));
                }
            });
        });
    }

    fn render_price_chart(&self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.vertical_centered(|ui| { ui.label(RichText::new("💹 Price History").font(FontId::proportional(16.0)).strong()); });
            Plot::new("price_history_plot").legend(Legend::default()).show_axes([true, true]).show_grid([true, true]).show(ui, |plot_ui| {
                if let Some(history) = self.price_histories.get(&self.selected_symbol) {
                    plot_ui.line(Line::new(PlotPoints::from_ys_f64(history)).color(Color32::from_rgb(0, 123, 255)).stroke(Stroke::new(3.0, Color32::from_rgb(0, 123, 255))).fill(-1.0).name("💰 Last Traded Price"));
                    if let Some(&last_price) = history.last() {
                        let x = (history.len().saturating_sub(1)) as f64;
                        let pulse = (self.animation_time * 4.0).sin().abs(); let radius = 4.0 + pulse * 4.0; let alpha = (128.0 + pulse * 127.0) as u8;
                        plot_ui.points(Points::new(vec![[x, last_price]]).radius(radius).color(Color32::from_rgba_unmultiplied(255, 215, 0, alpha)));
                        let ring_radius = 6.0 + pulse * 2.0;
                        plot_ui.points(Points::new(vec![[x, last_price]]).radius(ring_radius).color(Color32::from_rgba_unmultiplied(255, 215, 0, (64.0 * (1.0 - pulse)) as u8)).filled(false));
                    }
                }
            });
        });
    }
    
    fn render_market_status(&self, ui: &mut egui::Ui, market: &Market, order_book: &OrderBook) {
        let best_bid = order_book.bids.keys().last().cloned();
        let best_ask = order_book.asks.keys().next().cloned();
        let total_inventory = market.get_total_inventory(&self.selected_symbol);

        if let (Some(bid), Some(ask)) = (best_bid, best_ask) {
            if ask > bid {
                let spread = (ask - bid) as f64 / 100.0;
                let spread_pct = (spread / (ask as f64 / 100.0)) * 100.0;
                let status_color = if self.is_market_running { Color32::from_rgb(40, 167, 69) } else { Color32::from_rgb(108, 117, 125) };
                
                ui.horizontal(|ui| {
                    let circle_center = ui.cursor().min + Vec2::new(6.0, 8.0);
                    ui.painter().circle_filled(circle_center, 4.0, status_color);
                    ui.add_space(16.0);
                    ui.label(RichText::new(if self.is_market_running { "🟢 LIVE" } else { "⏸️ PAUSED" }).color(status_color).strong());
                    ui.separator();
                    ui.label("💰"); ui.label(RichText::new("Bid:").strong()); ui.label(RichText::new(format!("${:.2}", bid as f64 / 100.0)).color(Color32::from_rgb(40, 167, 69)).monospace());
                    ui.separator();
                    ui.label("💸"); ui.label(RichText::new("Ask:").strong()); ui.label(RichText::new(format!("${:.2}", ask as f64 / 100.0)).color(Color32::from_rgb(220, 53, 69)).monospace());
                    ui.separator();
                    ui.label("📊"); ui.label(RichText::new("Spread:").strong()); ui.label(RichText::new(format!("${:.2} ({:.2}%)", spread, spread_pct)).color(Color32::from_rgb(255, 193, 7)).monospace());
                    ui.separator();
                    if let (Some(ath), Some(atl)) = (self.aths.get(&self.selected_symbol), self.atls.get(&self.selected_symbol)) {
                        ui.label("🚀"); ui.label(RichText::new("ATH:").strong()); ui.label(RichText::new(format!("${:.2}", ath)).color(Color32::from_rgb(40, 167, 69)).monospace());
                        ui.separator();
                        ui.label("⚓"); ui.label(RichText::new("ATL:").strong()); ui.label(RichText::new(format!("${:.2}", atl)).color(Color32::from_rgb(220, 53, 69)).monospace());
                        ui.separator();
                    }
                    ui.label("📈"); ui.label(RichText::new("Volume:").strong()); ui.label(RichText::new(format_number_u64(market.cumulative_volume(&self.selected_symbol))).monospace());
                    ui.separator();
                    ui.label("⚖️"); ui.label(RichText::new("Net Inventory:").strong());
                    let inventory_str = if total_inventory >= 0 { format!("+{}", format_number(total_inventory as i32)) } else { format_number(total_inventory as i32) };
                    ui.label(RichText::new(inventory_str).color(if total_inventory > 0 { Color32::from_rgb(40, 167, 69) } else if total_inventory < 0 { Color32::from_rgb(220, 53, 69) } else { Color32::GRAY }).monospace());
                });
            }
        }
    }
}

fn main() -> Result<(), eframe::Error> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1600.0, 900.0]).with_min_inner_size([1200.0, 700.0]),
        ..Default::default()
    };

    let participants = vec![AgentType::MarketMaker, AgentType::DumbLimit, AgentType::DumbMarket, AgentType::Ipo, AgentType::WhaleAgent];
    let simulator: Box<dyn Marketable> = Box::new(Market::new(&participants));
    
    // --- NEW: Initialize state for all available symbols ---
    let available_symbols = registry::get_tradable_universe().into_iter().map(|s| s.symbol).collect::<Vec<_>>();
    let mut price_histories = HashMap::new();
    let mut aths = HashMap::new();
    let mut atls = HashMap::new();
    
    if let Some(market) = simulator.as_any().downcast_ref::<Market>() {
        for symbol in &available_symbols {
            if let Some(initial_price) = market.current_price(symbol) {
                price_histories.insert(symbol.clone(), vec![initial_price]);
                aths.insert(symbol.clone(), initial_price);
                atls.insert(symbol.clone(), initial_price);
            }
        }
    }

    let selected_symbol = available_symbols.first().cloned().unwrap_or_default();
    
    let app_state = AgentVisualizer {
        simulator,
        price_histories,
        aths,
        atls,
        selected_symbol,
        available_symbols,
        is_market_running: false,
        last_update: Instant::now(),
        theme_dark: true,
        animation_time: 0.0,
    };

    eframe::run_native( "🚀 Live Agent-Based Market Visualizer", native_options, Box::new(|_cc| Box::new(app_state)), )
}
