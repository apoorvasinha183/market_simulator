//! Visual playground: multi-ticker edition with debug logging for asks rendering
//! Select a symbol from the drop-down to inspect its live book & price.

use eframe::egui;
use egui::{Color32, FontId, RichText, Rounding, Stroke, Vec2};
use egui_plot::{Legend, Line, Plot, PlotBounds, PlotPoints, Points};
use market_simulator::{
    simulation::orchestra::{Orchestra, ShadowBookHandle, MarketState},
    AgentType,
    simulators::order_book::{OrderBook, PriceLevel},
};
//use egui_plot::PlotItem;
use std::collections::HashMap;
use std::time::Instant;

// Add debug logging
fn debug_order_book(order_book: &OrderBook, stock_id: u64) {
    println!("=== DEBUG ORDER BOOK FOR STOCK {} ===", stock_id);
    println!("Bids count: {}", order_book.bids.len());
    println!("Asks count: {}", order_book.asks.len());

    println!("First 5 bids:");
    for (price, level) in order_book.bids.iter().rev().take(5) {
        println!(
            "  ${:.2} -> {} shares",
            *price as f64 / 100.0,
            level.total_volume
        );
    }

    println!("First 5 asks:");
    for (price, level) in order_book.asks.iter().take(5) {
        println!(
            "  ${:.2} -> {} shares",
            *price as f64 / 100.0,
            level.total_volume
        );
    }
    println!("=== END DEBUG ===");
}

// -----------------------------------------------------------------------------
//  Helpers
// -----------------------------------------------------------------------------
fn format_number(n: i32) -> String {
    let negative = n.is_negative();
    let mut s = n.abs().to_string();
    let mut out = String::new();
    while s.len() > 3 {
        let tail = s.split_off(s.len() - 3);
        out = format!(",{tail}{out}");
    }
    out = format!("{s}{out}");
    if negative { format!("-{out}") } else { out }
}
fn format_number_u64(n: u64) -> String {
    format_number(n as i32)
}

// -----------------------------------------------------------------------------
//  GUI state
// -----------------------------------------------------------------------------
struct AgentVisualizer {
    shadow_handle: ShadowBookHandle,
    price_histories: HashMap<u64, Vec<f64>>,
    selected_id: u64,
    is_market_running: bool,
    last_update: Instant,
    theme_dark: bool,
    animation_time: f64,
    ath: f64,
    atl: f64,
    debug_counter: u32,
}

// -----------------------------------------------------------------------------
//  eframe::App
// -----------------------------------------------------------------------------
impl eframe::App for AgentVisualizer {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.animation_time = ctx.input(|i| i.time);
        self.apply_custom_style(ctx);

        // Step simulator every 100 ms
        if self.is_market_running {
            self.record_prices();
            self.last_update = Instant::now();
        }
        ctx.request_repaint();

        let (market_state, order_book) = {
            let state_guard = self.shadow_handle.read().unwrap();
            let book = state_guard.order_books.get(&self.selected_id).cloned();
            (state_guard.clone(), book)
        };


        /* ───────────────────────── TOP BAR ────────────────────────── */
        egui::TopBottomPanel::top("top_panel")
            .min_height(60.0)
            .show(ctx, |ui| {
                self.render_top_bar(ui, &market_state);
            });

        let Some(order_book) = order_book else {
            return;
        };

        // Debug logging every 60 frames (roughly once per second)
        self.debug_counter += 1;

        /* ───────────────────────── BOTTOM PANEL ───────────────────── */
        self.render_order_book_tables(ctx, &order_book, &market_state);

        /* ───────────────────────── CENTRAL PANEL ──────────────────── */
        self.render_plots(ctx, &order_book, &market_state);
    }
}

// -----------------------------------------------------------------------------
//  Internal helpers
// -----------------------------------------------------------------------------
impl AgentVisualizer {
    fn render_top_bar(&mut self, ui: &mut egui::Ui, market_state: &MarketState) {
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

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Add debug button
                if ui.button("🐛 Debug").clicked() {
                    if let Some(order_book) = market_state.order_books.get(&self.selected_id) {
                        debug_order_book(order_book, self.selected_id);
                    }
                }
                ui.separator();

                // theme toggle
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

                // start / pause
                let start_stop = if self.is_market_running {
                    egui::Button::new(RichText::new("⏸ Pause").color(Color32::WHITE))
                        .fill(Color32::from_rgb(220, 53, 69))
                } else {
                    egui::Button::new(RichText::new("▶ Start").color(Color32::WHITE))
                        .fill(Color32::from_rgb(40, 167, 69))
                };
                if ui.add(start_stop.rounding(Rounding::same(8.0))).clicked() {
                    self.is_market_running = !self.is_market_running;
                }

                // reset (disabled for now)
                ui.add_enabled(false, egui::Button::new(RichText::new("🔄 Reset").color(Color32::WHITE))
                    .fill(Color32::from_rgb(108, 117, 125))
                    .rounding(Rounding::same(8.0)));

                ui.separator();

                // ▼ symbol picker (ticker text)
                let ids: Vec<u64> = market_state.stocks.get_all_ids();
                egui::ComboBox::from_id_source("symbol_combo")
                    .selected_text(format!("🪙 {}", market_state.stocks.get_ticker_by_id(self.selected_id).unwrap_or(&"UNKNOWN".to_string())))
                    .show_ui(ui, |ui| {
                        for id in ids {
                            ui.selectable_value(
                                &mut self.selected_id,
                                id,
                                market_state.stocks.get_ticker_by_id(id).unwrap_or(&"UNKNOWN".to_string()),
                            );
                        }
                    });
            });
        });
        ui.add_space(8.0);
    }

    fn record_prices(&mut self) {
        let market_state = self.shadow_handle.read().unwrap();
        for (&id, &px) in market_state.last_traded_price.iter() {
            let hist = self.price_histories.entry(id).or_default();
            if hist.last() != Some(&px) {
                hist.push(px);
                if hist.len() > 1_000 {
                    hist.remove(0);
                }
            }
        }
        if let Some(hist) = self.price_histories.get(&self.selected_id) {
            if let Some((&last, tail)) = hist.split_last() {
                self.ath = tail.iter().fold(last, |a, &p| a.max(p));
                self.atl = tail.iter().fold(last, |a, &p| a.min(p));
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
        for w in [
            &mut style.visuals.widgets.noninteractive,
            &mut style.visuals.widgets.inactive,
            &mut style.visuals.widgets.hovered,
            &mut style.visuals.widgets.active,
        ] {
            w.rounding = Rounding::same(6.0);
        }
        ctx.set_style(style);
    }

    /* ------------ order-book tables + status bar ------------ */
    fn render_order_book_tables(
        &self,
        ctx: &egui::Context,
        order_book: &OrderBook,
        market_state: &MarketState,
    ) {
        egui::TopBottomPanel::bottom("bottom_panel")
            .resizable(true)
            .min_height(250.0)
            .show(ctx, |ui| {
                ui.add_space(8.0);
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.render_market_status(ui, market_state);
                    ui.add_space(12.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new("📊 Live Order Book")
                                .font(FontId::proportional(18.0))
                                .strong(),
                        );
                    });
                    ui.add_space(8.0);
                    ui.horizontal_top(|ui| {
                        // Make sure both tables get equal space
                        let available_width = ui.available_width();
                        let table_width = (available_width - 40.0) / 2.0; // 20px spacing on each side

                        ui.allocate_ui_with_layout(
                            egui::Vec2::new(table_width, ui.available_height()),
                            egui::Layout::top_down(egui::Align::LEFT),
                            |ui| {
                                self.render_side_table(ui, &order_book.bids, true);
                            },
                        );

                        ui.add_space(20.0);

                        ui.allocate_ui_with_layout(
                            egui::Vec2::new(table_width, ui.available_height()),
                            egui::Layout::top_down(egui::Align::LEFT),
                            |ui| {
                                self.render_side_table(ui, &order_book.asks, false);
                            },
                        );
                    });
                });
            });
    }

    fn render_side_table(
        &self,
        ui: &mut egui::Ui,
        book_side: &std::collections::BTreeMap<u64, PriceLevel>,
        is_bid: bool,
    ) {
        let (title, col, rgb) = if is_bid {
            ("📈 Bids", "bids_grid", (40, 167, 69))
        } else {
            ("📉 Asks", "asks_grid", (220, 53, 69))
        };

        ui.vertical(|ui| {
            ui.set_width(ui.available_width());

            // header with better visibility
            let rect = ui.available_rect_before_wrap();
            let header = egui::Rect::from_min_size(rect.min, Vec2::new(rect.width(), 35.0));
            ui.painter().rect_filled(
                header,
                Rounding::same(6.0),
                Color32::from_rgba_unmultiplied(rgb.0, rgb.1, rgb.2, 40),
            );
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(title)
                        .color(Color32::from_rgb(rgb.0, rgb.1, rgb.2))
                        .font(FontId::proportional(16.0))
                        .strong(),
                );
            });
            ui.add_space(6.0);

            let mut row = 0;
            egui::Grid::new(col).spacing([20.0, 6.0]).show(ui, |ui| {
                ui.label(RichText::new("Price").underline().strong());
                ui.label(RichText::new("Volume").underline().strong());
                ui.end_row();

                if book_side.is_empty() {
                    ui.label(RichText::new("No orders").color(Color32::GRAY).italics());
                    ui.label(RichText::new("—").color(Color32::GRAY).italics());
                    ui.end_row();
                } else {
                    let iter: Box<dyn Iterator<Item = (&u64, &PriceLevel)>> = if is_bid {
                        Box::new(book_side.iter().rev())
                    } else {
                        Box::new(book_side.iter())
                    };

                    let entries: Vec<_> = iter.take(10).collect();

                    for (price, lvl) in entries {
                        if row % 2 == 0 {
                            let r = ui.available_rect_before_wrap();
                            ui.painter().rect_filled(
                                r,
                                Rounding::same(3.0),
                                Color32::from_rgba_unmultiplied(0, 0, 0, 20),
                            );
                        }
                        ui.label(
                            RichText::new(format!("${:.2}", *price as f64 / 100.0))
                                .color(Color32::from_rgb(rgb.0, rgb.1, rgb.2))
                                .font(FontId::monospace(14.0))
                                .strong(),
                        );
                        ui.label(
                            RichText::new(format_number(lvl.total_volume as i32))
                                .font(FontId::monospace(14.0)),
                        );
                        ui.end_row();
                        row += 1;
                    }
                }
            });
        });
    }

    fn render_plots(&self, ctx: &egui::Context, order_book: &OrderBook, market_state: &MarketState) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.columns(2, |cols| {
                // ─── Depth Chart ─────────────────────────────────────────
                cols[0].group(|ui| {
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new("📈 Order Book Depth")
                                .font(FontId::proportional(16.0))
                                .strong(),
                        );
                    });

                    Plot::new("depth_plot")
                        .legend(Legend::default())
                        .show_axes([true, true])
                        .show_grid([true, true])
                        .show(ui, |p| {
                            // ASKS curve + fill
                            if !order_book.asks.is_empty() {
                                let mut ask_pts = Vec::new();
                                let mut cum: u64 = 0;
                                for (&px, lvl) in order_book.asks.iter() {
                                    let price = px as f64 / 100.0;
                                    ask_pts.push([price, cum as f64]);
                                    cum += lvl.total_volume as u64;
                                    ask_pts.push([price, cum as f64]);
                                }
                                p.line(
                                    Line::new(PlotPoints::from(ask_pts))
                                        .fill(0.0)
                                        .stroke(Stroke::new(2.5, Color32::from_rgb(220, 53, 69)))
                                        .color(Color32::from_rgb(220, 53, 69))
                                        .name("📈 Asks"),
                                );
                            }

                            // BIDS curve + fill
                            if !order_book.bids.is_empty() {
                                let mut bid_pts = Vec::new();
                                let mut cum: u64 = 0;
                                for (&px, lvl) in order_book.bids.iter().rev() {
                                    let price = px as f64 / 100.0;
                                    bid_pts.push([price, cum as f64]);
                                    cum += lvl.total_volume as u64;
                                    bid_pts.push([price, cum as f64]);
                                }
                                p.line(
                                    Line::new(PlotPoints::from(bid_pts))
                                        .fill(0.0)
                                        .stroke(Stroke::new(2.5, Color32::from_rgb(40, 167, 69)))
                                        .color(Color32::from_rgb(40, 167, 69))
                                        .name("📉 Bids"),
                                );
                            }

                            // Current price indicator
                            let cp = *market_state.last_traded_price.get(&self.selected_id).unwrap_or(&0.0);
                            p.line(
                                Line::new(PlotPoints::from(vec![[cp, 0.0], [cp, 1_000_000.0]]))
                                    .color(Color32::from_rgba_unmultiplied(255, 255, 255, 100))
                                    .stroke(Stroke::new(
                                        1.0,
                                        Color32::from_rgba_unmultiplied(255, 255, 255, 150),
                                    ))
                                    .style(egui_plot::LineStyle::Dashed { length: 10.0 })
                                    .name("Current Price"),
                            );

                            // Zoom bounds around current price
                            let center = *market_state.last_traded_price.get(&self.selected_id).unwrap_or(&0.0);
                            p.set_plot_bounds(PlotBounds::from_min_max(
                                [center - 20.0, 0.0],
                                [center + 20.0, 2_000_000.0],
                            ));
                        });
                });

                // ─── Price History ────────────────────────────────────────
                cols[1].group(|ui| {
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new("💹 Price History")
                                .font(FontId::proportional(16.0))
                                .strong(),
                        );
                    });

                    let empty: Vec<f64> = Vec::new();
                    let history = self
                        .price_histories
                        .get(&self.selected_id)
                        .unwrap_or(&empty);

                    Plot::new("hist_plot")
                        .legend(Legend::default())
                        .show_axes([true, true])
                        .show_grid([true, true])
                        .show(ui, |p| {
                            let line = Line::new(PlotPoints::from_ys_f64(history))
                                .color(Color32::from_rgb(0, 123, 255))
                                .stroke(Stroke::new(3.0, Color32::from_rgb(0, 123, 255)))
                                .fill(-1.0);
                            p.line(line.name("price"));

                            if let Some(&last) = history.last() {
                                let x = (history.len() - 1) as f64;
                                let pulse = (self.animation_time * 4.0).sin().abs();
                                let radius = 4.0 + pulse * 4.0;
                                let alpha = (128.0 + pulse * 127.0) as u8;
                                p.points(
                                    Points::new(vec![[x, last]])
                                        .radius(radius as f32)
                                        .color(Color32::from_rgba_unmultiplied(255, 215, 0, alpha)),
                                );
                            }
                        });
                });
            });
        });
    }

    /* ---------------- status bar ---------------- */
    fn render_market_status(&self, ui: &mut egui::Ui, market_state: &MarketState) {
        let Some(ob) = market_state.order_books.get(&self.selected_id) else {
            return;
        };
        let best_bid = ob.bids.keys().last().copied();
        let best_ask = ob.asks.keys().next().copied();
        // total_inventory is not available in MarketState, so we'll remove it for now
        // let total_inv = market_state.total_inventory();

        // Always render the status bar, even if bid/ask are missing
        let col = if self.is_market_running {
            Color32::from_rgb(40, 167, 69)
        } else {
            Color32::from_rgb(108, 117, 125)
        };

        ui.horizontal(|ui| {
            ui.add_space(8.0);
            let c = ui.cursor().min + Vec2::new(6.0, 8.0);
            ui.painter().circle_filled(c, 4.0, col);
            ui.add_space(16.0);
            ui.label(
                RichText::new(if self.is_market_running {
                    "🟢 LIVE"
                } else {
                    "⏸️ PAUSED"
                })
                .color(col)
                .strong(),
            );

            ui.separator();

            // Bid metric - fixed width
            if let Some(bid) = best_bid {
                metric_fixed_width(
                    ui,
                    "Bid",
                    &format!("${:.2}", bid as f64 / 100.0),
                    Color32::from_rgb(40, 167, 69),
                    80.0,
                );
            } else {
                metric_fixed_width(ui, "Bid", "N/A", Color32::GRAY, 80.0);
            }

            // Ask metric - fixed width
            if let Some(ask) = best_ask {
                metric_fixed_width(
                    ui,
                    "Ask",
                    &format!("${:.2}", ask as f64 / 100.0),
                    Color32::from_rgb(220, 53, 69),
                    80.0,
                );
            } else {
                metric_fixed_width(ui, "Ask", "N/A", Color32::GRAY, 80.0);
            }

            // Spread metric - fixed width
            if let (Some(bid), Some(ask)) = (best_bid, best_ask) {
                if ask > bid {
                    let spread = (ask - bid) as f64 / 100.0;
                    let spread_pct = spread / (ask as f64 / 100.0) * 100.0;
                    metric_fixed_width(
                        ui,
                        "Spread",
                        &format!("${:.2} ({:.2}%)", spread, spread_pct),
                        Color32::from_rgb(255, 193, 7),
                        140.0,
                    );
                } else {
                    // Crossed market - warning
                    metric_fixed_width(
                        ui,
                        "Spread",
                        "CROSSED",
                        Color32::from_rgb(255, 100, 100),
                        140.0,
                    );
                }
            } else {
                metric_fixed_width(ui, "Spread", "N/A", Color32::GRAY, 140.0);
            }

            // ATH/ATL - fixed width
            metric_fixed_width(
                ui,
                "ATH",
                &format!("${:.2}", self.ath),
                Color32::from_rgb(40, 167, 69),
                80.0,
            );
            metric_fixed_width(
                ui,
                "ATL",
                &format!("${:.2}", self.atl),
                Color32::from_rgb(220, 53, 69),
                80.0,
            );

            // Volume - fixed width
            metric_fixed_width(
                ui,
                "Volume",
                &format_number_u64(market_state.cumulative_volume.get(&self.selected_id).copied().unwrap_or(0)),
                Color32::WHITE,
                100.0,
            );

            
        });

        fn metric_fixed_width(ui: &mut egui::Ui, label: &str, val: &str, col: Color32, width: f32) {
            ui.separator();
            ui.allocate_ui_with_layout(
                Vec2::new(width, ui.available_height()),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(label).strong());
                        ui.label(RichText::new(val).color(col).monospace());
                    });
                },
            );
        }
    }
}

// -----------------------------------------------------------------------------
//  Entry point
// -----------------------------------------------------------------------------
fn main() -> Result<(), eframe::Error> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1_400.0, 900.0])
            .with_min_inner_size([1_000.0, 700.0]),
        ..Default::default()
    };

    let participants = vec![
        AgentType::DumbMarket,
        AgentType::DumbLimit,
        AgentType::MarketMaker,
        AgentType::WhaleAgent,
    ];

    let orchestra = Orchestra::new(participants, 100, 100);
    let shadow_handle = orchestra.get_shadow_handle();

    std::thread::spawn(move || {
        orchestra.run();
    });

    let initial_state = {
        let state_guard = shadow_handle.read().unwrap();
        state_guard.clone()
    };
    let first_id = *initial_state.order_books.keys().next().expect("empty universe");
    let first_px = *initial_state.last_traded_price.get(&first_id).unwrap_or(&0.0);

    let mut hist = HashMap::new();
    hist.insert(first_id, vec![first_px]);

    let app_state = AgentVisualizer {
        shadow_handle,
        price_histories: hist,
        selected_id: first_id,
        is_market_running: false,
        last_update: Instant::now(),
        theme_dark: true,
        animation_time: 0.0,
        ath: first_px,
        atl: first_px,
        debug_counter: 0,
    };

    eframe::run_native(
        "🚀 Live Agent-Based Market Visualizer",
        native_options,
        Box::new(|_cc| Box::new(app_state)),
    )
}
