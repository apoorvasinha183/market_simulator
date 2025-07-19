//! Visual playground: multi-ticker edition with debug logging for asks rendering
//! Select a symbol from the drop-down to inspect its live book & price.

use eframe::egui;
use egui::{Color32, FontId, RichText, Rounding, Stroke, Vec2};
use egui_plot::{BoxElem, BoxPlot, BoxSpread, Legend, Line, Plot, PlotBounds, PlotPoints, Points};
use market_simulator::{
    AgentType,
    simulation::{
        candle_analyzer::CandleDataHandle,
        orchestra::{MarketState, Orchestra, ShadowBookHandle},
    },
    simulators::order_book::{OrderBook, PriceLevel},
    types::candle::{Candle as AppCandle, TimeFrame},
};
//use egui_plot::PlotItem;
use core_affinity;
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
fn format_number(n: u64) -> String {
    let mut s = n.to_string();
    let mut out = String::new();
    while s.len() > 3 {
        let tail = s.split_off(s.len() - 3);
        out = format!(",{tail}{out}");
    }
    format!("{s}{out}")
}

// -----------------------------------------------------------------------------
//  GUI state
// -----------------------------------------------------------------------------
struct AgentVisualizer {
    shadow_handle: ShadowBookHandle,
    candle_data_handle: CandleDataHandle, // Add this
    price_histories: HashMap<u64, Vec<f64>>,
    candle_history: HashMap<(u64, TimeFrame), Vec<AppCandle>>,
    selected_id: u64,
    selected_timeframe: TimeFrame, // Add this
    is_market_running: bool,
    last_update: Instant,
    theme_dark: bool,
    animation_time: f64,
    all_time_highs: HashMap<u64, f64>,
    all_time_lows: HashMap<u64, f64>,
    debug_counter: u32,
    show_candlestick: bool, // Add this
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
            self.record_data();
            self.last_update = Instant::now();
        }
        ctx.request_repaint();

        let market_state_guard = self.shadow_handle.read().unwrap();

        let selected_order_book = market_state_guard.book(self.selected_id);
        let current_last_traded_price = *market_state_guard
            .last_traded_price
            .get(&self.selected_id)
            .unwrap_or(&0.0);
        let market_state_for_render = market_state_guard.clone(); // Clone the entire state for rendering

        drop(market_state_guard);

        let Some(order_book) = selected_order_book else {
            return;
        };

        // Debug logging every 60 frames (roughly once per second)
        self.debug_counter += 1;

        /* ───────────────────────── TOP BAR ────────────────────────── */
        egui::TopBottomPanel::top("top_panel")
            .min_height(60.0)
            .show(ctx, |ui| {
                self.render_top_bar(ui, &market_state_for_render);
            });

        /* ───────────────────────── BOTTOM PANEL ───────────────────── */
        egui::TopBottomPanel::bottom("bottom_panel")
            .resizable(true)
            .min_height(250.0)
            .show(ctx, |ui| {
                self.render_market_status(
                    ui,
                    &market_state_for_render,
                    &order_book,
                    current_last_traded_price,
                );
                ui.add_space(12.0);
                egui::ScrollArea::vertical().show(ui, |ui| {
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

        /* ───────────────────────── CENTRAL PANEL ──────────────────── */
        self.render_plots(
            ctx,
            &order_book,
            &market_state_for_render,
            current_last_traded_price,
        );
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
                    if let Some(order_book) =
                        market_state.order_books.get(&self.selected_id).cloned()
                    {
                        debug_order_book(&order_book, self.selected_id);
                    }
                }

                // Candlestick toggle
                if ui
                    .button(if self.show_candlestick {
                        "💹 Price"
                    } else {
                        "🕯️ Candles"
                    })
                    .clicked()
                {
                    self.show_candlestick = !self.show_candlestick;
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
                ui.add_enabled(
                    false,
                    egui::Button::new(RichText::new("🔄 Reset").color(Color32::WHITE))
                        .fill(Color32::from_rgb(108, 117, 125))
                        .rounding(Rounding::same(8.0)),
                );

                ui.separator();

                // ▼ symbol picker (ticker text)
                let ids: Vec<u64> = market_state.stocks.get_all_ids();
                egui::ComboBox::from_id_source("symbol_combo")
                    .selected_text(format!(
                        "🪙 {}",
                        market_state
                            .stocks
                            .get_ticker_by_id(self.selected_id)
                            .unwrap_or(&"UNKNOWN".to_string())
                    ))
                    .show_ui(ui, |ui| {
                        for id in ids {
                            ui.selectable_value(
                                &mut self.selected_id,
                                id,
                                market_state
                                    .stocks
                                    .get_ticker_by_id(id)
                                    .unwrap_or(&"UNKNOWN".to_string()),
                            );
                        }
                    });

                // ▼ timeframe picker
                egui::ComboBox::from_id_source("timeframe_combo")
                    .selected_text(format!("🕰️ {}", self.selected_timeframe))
                    .show_ui(ui, |ui| {
                        for timeframe in TimeFrame::all() {
                            ui.selectable_value(
                                &mut self.selected_timeframe,
                                timeframe,
                                timeframe.to_string(),
                            );
                        }
                    });
            });
        });
        ui.add_space(8.0);
    }

    fn record_data(&mut self) {
        let market_state = self.shadow_handle.read().unwrap();

        // Record price history
        for (&id, &px) in &market_state.last_traded_price {
            let hist = self.price_histories.entry(id).or_default();
            if hist.last() != Some(&px) {
                hist.push(px);
                if hist.len() > 1_000 {
                    hist.remove(0);
                }
            }
        }

        // Record candle history
        let candle_data = self.candle_data_handle.clone();
        for item in candle_data.iter() {
            let key = *item.key();
            let value = item.value().clone();
            self.candle_history.insert(key, value.into_iter().collect());
        }

        // Update ATH/ATL for all stocks
        for (&id, &px) in &market_state.last_traded_price {
            self.all_time_highs
                .entry(id)
                .and_modify(|ath| *ath = ath.max(px))
                .or_insert(px);
            self.all_time_lows
                .entry(id)
                .and_modify(|atl| *atl = atl.min(px))
                .or_insert(px);
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
    #[allow(dead_code)]
    fn render_order_book_tables(
        &self,
        ctx: &egui::Context,
        order_book: &OrderBook,
        market_state: &MarketState,
        current_last_traded_price: f64,
    ) {
        egui::TopBottomPanel::bottom("bottom_panel")
            .resizable(true)
            .min_height(250.0)
            .show(ctx, |ui| {
                ui.add_space(8.0);
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.render_market_status(
                        ui,
                        market_state,
                        order_book,
                        current_last_traded_price,
                    );
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
                            RichText::new(format_number(lvl.total_volume as u64))
                                .font(FontId::monospace(14.0)),
                        );
                        ui.end_row();
                        row += 1;
                    }
                }
            });
        });
    }

    // Replace the render_plots function with this version that properly handles height allocation

    fn render_plots(
        &self,
        ctx: &egui::Context,
        order_book: &OrderBook,
        _market_state: &MarketState,
        current_last_traded_price: f64,
    ) {
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

                    Plot::new(format!("depth_plot_{}", self.selected_id))
                        .legend(Legend::default())
                        .show_axes([true, true])
                        .show_grid([true, true])
                        .auto_bounds(egui::Vec2b::new(true, false))
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
                            let cp = current_last_traded_price;
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

                            // Calculate mid-price for centering the x-axis
                            let mid_price = if let (Some(&best_bid), Some(&best_ask)) =
                                (order_book.bids.keys().last(), order_book.asks.keys().next())
                            {
                                (best_bid as f64 / 100.0 + best_ask as f64 / 100.0) / 2.0
                            } else {
                                current_last_traded_price
                            };

                            // Calculate max cumulative volume for y-axis scaling
                            let max_cum_volume = {
                                let mut max_vol: f64 = 0.0;
                                let mut cum_bid: u64 = 0;
                                for (_, lvl) in order_book.bids.iter().rev() {
                                    cum_bid += lvl.total_volume as u64;
                                    max_vol = max_vol.max(cum_bid as f64);
                                }
                                let mut cum_ask: u64 = 0;
                                for (_, lvl) in order_book.asks.iter() {
                                    cum_ask += lvl.total_volume as u64;
                                    max_vol = max_vol.max(cum_ask as f64);
                                }
                                max_vol
                            };

                            // Set plot bounds dynamically
                            let x_range = 50.0; // Adjust this value to control the visible price range
                            p.set_plot_bounds(PlotBounds::from_min_max(
                                [mid_price - x_range, 0.0],
                                [mid_price + x_range, max_cum_volume * 1.1], // Add 10% buffer to max volume
                            ));
                        });
                });

                // ─── Price & Volume History ────────────────────────────────
                cols[1].group(|ui| {
                    // Get the available height and reserve space for titles and spacing
                    let available_height = ui.available_height();
                    let title_height = 30.0; // Approximate height for title
                    let spacing = 8.0; // Space between elements

                    // Calculate heights properly accounting for titles and spacing
                    let (price_plot_height, volume_plot_height) = if self.show_candlestick {
                        let remaining_height =
                            available_height - (title_height * 2.0) - (spacing * 3.0);
                        (remaining_height * 0.65, remaining_height * 0.35) // Adjust ratio as needed
                    } else {
                        (available_height - title_height - spacing, 0.0)
                    };

                    // Use a vertical layout for the two plots
                    ui.vertical(|ui| {
                        // --- Price History Plot ---
                        ui.vertical_centered(|ui| {
                            ui.label(
                                RichText::new(if self.show_candlestick {
                                    "🕯️ Candlestick Chart"
                                } else {
                                    "💹 Price History"
                                })
                                .font(FontId::proportional(16.0))
                                .strong(),
                            );
                        });

                        let price_plot = Plot::new("hist_plot")
                            .legend(Legend::default())
                            .height(price_plot_height)
                            .show_axes([true, true])
                            .show_grid([true, true])
                            .x_axis_label("Time")
                            .y_axis_label("Price ($)");

                        let _price_plot_response = price_plot.show(ui, |p| {
                            if self.show_candlestick {
                                let empty: Vec<AppCandle> = Vec::new();
                                let history = self
                                    .candle_history
                                    .get(&(self.selected_id, self.selected_timeframe))
                                    .unwrap_or(&empty);

                                if self.debug_counter % 60 == 0 {
                                    //println!("[DEBUG] Rendering plot for stock {}. Found {} candles for TimeFrame::{:?}.", self.selected_id, history.len(), self.selected_timeframe);
                                }

                                if !history.is_empty() {
                                    let box_plot = BoxPlot::new(
                                        history
                                            .iter()
                                            .map(|c| {
                                                let q1 = c.open.min(c.close);
                                                let q3 = c.open.max(c.close);

                                                // Enhanced color scheme with better contrast
                                                let color = if c.close >= c.open {
                                                    Color32::from_rgb(34, 197, 94) // Brighter green for bullish
                                                } else {
                                                    Color32::from_rgb(239, 68, 68) // Brighter red for bearish
                                                };

                                                let spread = BoxSpread {
                                                    lower_whisker: c.low,
                                                    quartile1: q1,
                                                    median: (c.open + c.close) / 2.0,
                                                    quartile3: q3,
                                                    upper_whisker: c.high,
                                                };

                                                BoxElem::new(c.timestamp as f64, spread)
                                                    .box_width(c.timeframe.to_millis() as f64 * 0.7)
                                                    .whisker_width(0.8)
                                                    .stroke(Stroke::new(1.2, color))
                                                    .fill(color.linear_multiply(0.8))
                                                    .name(format!("Candle {}", c.timestamp))
                                            })
                                            .collect(),
                                    )
                                    .name("Candlesticks");
                                    p.box_plot(box_plot);

                                    // Add current price line for candlestick view
                                    if current_last_traded_price > 0.0 {
                                        let latest_timestamp =
                                            history.iter().map(|c| c.timestamp).max().unwrap_or(0)
                                                as f64;
                                        let earliest_timestamp =
                                            history.iter().map(|c| c.timestamp).min().unwrap_or(0)
                                                as f64;

                                        p.line(
                                            Line::new(PlotPoints::from(vec![
                                                [earliest_timestamp, current_last_traded_price],
                                                [latest_timestamp, current_last_traded_price],
                                            ]))
                                            .color(Color32::from_rgb(255, 193, 7))
                                            .stroke(Stroke::new(
                                                1.5,
                                                Color32::from_rgb(255, 193, 7),
                                            ))
                                            .style(egui_plot::LineStyle::Dashed { length: 8.0 })
                                            .name("Current Price"),
                                        );
                                    }
                                } else {
                                    // Show placeholder when no candle data is available
                                    p.text(
                                        egui_plot::Text::new(
                                            egui_plot::PlotPoint::new(
                                                0.0,
                                                current_last_traded_price,
                                            ),
                                            "No candle data available",
                                        )
                                        .color(Color32::GRAY)
                                        .anchor(egui::Align2::CENTER_CENTER),
                                    );
                                }
                            } else {
                                // Existing line chart logic...
                                let empty: Vec<f64> = Vec::new();
                                let history = self
                                    .price_histories
                                    .get(&self.selected_id)
                                    .unwrap_or(&empty);

                                if !history.is_empty() {
                                    let line = Line::new(PlotPoints::from_ys_f64(history))
                                        .color(Color32::from_rgb(0, 123, 255))
                                        .stroke(Stroke::new(2.5, Color32::from_rgb(0, 123, 255)))
                                        .fill(-1.0);
                                    p.line(line.name("Price"));

                                    if let Some(&last) = history.last() {
                                        let x = (history.len() - 1) as f64;
                                        let pulse = (self.animation_time * 4.0).sin().abs();
                                        let radius = 4.0 + pulse * 3.0;
                                        let alpha = (128.0 + pulse * 127.0) as u8;
                                        p.points(
                                            Points::new(vec![[x, last]])
                                                .radius(radius as f32)
                                                .color(Color32::from_rgba_unmultiplied(
                                                    255, 215, 0, alpha,
                                                )),
                                        );
                                    }
                                }
                            }
                        });

                        // --- Volume History Plot ---
                        if self.show_candlestick {
                            ui.add_space(spacing);
                            ui.vertical_centered(|ui| {
                                ui.label(
                                    RichText::new("📊 Volume")
                                        .font(FontId::proportional(14.0))
                                        .strong(),
                                );
                            });

                            let volume_plot = Plot::new("volume_plot")
                                .height(volume_plot_height)
                                .link_axis("hist_plot", true, false) // Link X axis to the price plot
                                .show_axes([true, true])
                                .show_grid([false, true])
                                .x_axis_label("Time")
                                .y_axis_label("Volume");

                            volume_plot.show(ui, |p| {
                                let empty: Vec<AppCandle> = Vec::new();
                                let history = self
                                    .candle_history
                                    .get(&(self.selected_id, self.selected_timeframe))
                                    .unwrap_or(&empty);

                                if !history.is_empty() {
                                    let bar_chart = egui_plot::BarChart::new(
                                        history
                                            .iter()
                                            .map(|c| {
                                                // Enhanced color scheme matching the candlesticks
                                                let color = if c.close >= c.open {
                                                    Color32::from_rgb(34, 197, 94) // Bullish volume
                                                } else {
                                                    Color32::from_rgb(239, 68, 68) // Bearish volume
                                                };

                                                egui_plot::Bar::new(
                                                    c.timestamp as f64,
                                                    c.volume as f64,
                                                )
                                                .width(c.timeframe.to_millis() as f64 * 0.7)
                                                .fill(color.linear_multiply(0.7))
                                                .stroke(Stroke::new(0.5, color))
                                            })
                                            .collect(),
                                    )
                                    .name("Volume");
                                    p.bar_chart(bar_chart);
                                }
                            });
                        }
                    });
                });
            });
        });
    }

    /* ---------------- status bar ---------------- */
    fn render_market_status(
        &self,
        ui: &mut egui::Ui,
        market_state: &MarketState,
        order_book: &OrderBook,
        current_last_traded_price: f64,
    ) {
        let best_bid = order_book.bids.keys().last().copied();
        let best_ask = order_book.asks.keys().next().copied();

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

            // Last Traded Price metric - fixed width
            metric_fixed_width(
                ui,
                "Last",
                &format!("${:.2}", current_last_traded_price),
                Color32::from_rgb(0, 123, 255),
                80.0,
            );

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
                &format!(
                    "${:.2}",
                    self.all_time_highs
                        .get(&self.selected_id)
                        .copied()
                        .unwrap_or(0.0)
                ),
                Color32::from_rgb(40, 167, 69),
                80.0,
            );
            metric_fixed_width(
                ui,
                "ATL",
                &format!(
                    "${:.2}",
                    self.all_time_lows
                        .get(&self.selected_id)
                        .copied()
                        .unwrap_or(0.0)
                ),
                Color32::from_rgb(220, 53, 69),
                80.0,
            );

            // Volume - fixed width
            metric_fixed_width(
                ui,
                "Volume",
                &format_number(
                    market_state
                        .cumulative_volume
                        .get(&self.selected_id)
                        .copied()
                        .unwrap_or(0),
                ),
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
    let _cores = core_affinity::get_core_ids().unwrap_or_default();

    let participants = vec![
        AgentType::CustomerAgent, // This agent will host the gRPC server
        AgentType::MarketMaker,
        AgentType::MomentumAgent,
        AgentType::Astrologer,
         
        AgentType::Thermodynamic {
            initial_temperature: 0.2,
            specific_heat: 0.1,
            initial_chemical_potential: 0.0,
        }, // Meme Trader
        AgentType::Thermodynamic {
            initial_temperature: 0.1,
            specific_heat: 1.0,
            initial_chemical_potential: 0.0,
        }, // Value Trader
        AgentType::WhaleAgent,
        AgentType::WebProxyAgent,
    ];

    /*let participants = vec![
        AgentType::IPO,
    ];*/

    let orchestra = Orchestra::new(participants, 1, 1);
    let shadow_handle = orchestra.get_shadow_handle();

    let candle_data_handle = orchestra.get_candle_data_handle();

    std::thread::spawn(move || {
        orchestra.run();
    });

    let initial_state = {
        let state_guard = shadow_handle.read().unwrap();
        state_guard.clone()
    };
    let first_id = *initial_state
        .order_books
        .keys()
        .next()
        .expect("empty universe");
    let first_px = *initial_state
        .last_traded_price
        .get(&first_id)
        .unwrap_or(&0.0);

    let mut price_histories = HashMap::new();
    price_histories.insert(first_id, vec![first_px]);

    let mut all_time_highs = HashMap::new();
    all_time_highs.insert(first_id, first_px);

    let mut all_time_lows = HashMap::new();
    all_time_lows.insert(first_id, first_px);

    let app_state = AgentVisualizer {
        shadow_handle,
        candle_data_handle,
        price_histories,
        candle_history: HashMap::new(),
        selected_id: first_id,
        selected_timeframe: TimeFrame::TenSeconds,
        is_market_running: false,
        last_update: Instant::now(),
        theme_dark: true,
        animation_time: 0.0,
        all_time_highs,
        all_time_lows,
        debug_counter: 0,
        show_candlestick: false,
    };

    eframe::run_native(
        "🚀 Live Agent-Based Market Visualizer",
        native_options,
        Box::new(|_cc| Box::new(app_state)),
    )
}
