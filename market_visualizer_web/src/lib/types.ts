// src/lib/types.ts

export enum TimeFrame {
    TenSeconds = "TenSeconds",
    OneMinute = "OneMinute",
    FiveMinutes = "FiveMinutes",
    ThirtyMinutes = "ThirtyMinutes",
}

export interface Candle {
    stock_id: number;
    timeframe: TimeFrame;
    timestamp: number; // Unix timestamp for the start of the candle's period
    open: number;
    high: number;
    low: number;
    close: number;
    volume: number;
}

export interface Stock {
    ticker: string;
    id: number;
    company_name: string;
    total_float: number;
    initial_price: number;
    sentiment_port: number;
}

export interface PriceLevel {
    total_volume: number;
    orders: any[]; // Simplified for frontend, actual Order objects are complex
}

export interface OrderBook {
    bids: { [price: string]: PriceLevel };
    asks: { [price: string]: PriceLevel };
}

export interface MarketState {
    order_books: { [stockId: string]: OrderBook };
    stocks: { stocks: Stock[] };
    last_traded_price: { [stockId: string]: number };
    cumulative_volume: { [stockId: string]: number };
    mid_prices: { [stockId: string]: number };
    spreads: { [stockId: string]: number };
}

export interface WebSocketData {
    market_state: MarketState;
    candle_data: { [key: string]: Candle[] };
    price_history: { [key: string]: [number, number][] };
}
