// Represents a statistical "cloud" of buy or sell interest.
pub struct PriceDistribution {
    /// The center of mass of the distribution (e.g., $150.00).
    pub center_price: f64,

    /// Represents the "diameter" or tightness of the cloud.
    /// A smaller value means orders are tightly clustered.
    pub std_dev: f64,

    /// The total volume or number of shares in the entire cloud.
    pub total_volume: f64,

    /// A parameter for skewness, to model the "heavy walls".
    /// (e.g., > 0 means a right-heavy tail, < 0 means a left-heavy tail).This is typical of any market
    pub skew: f64,
}

// The OrderBook holds the two opposing distributions.
pub struct OrderBook {
    pub bids: PriceDistribution,
    pub asks: PriceDistribution,
    pub last_traded_price: f64,
}

// This is our main simulator struct that will implement the Marketable trait.
pub struct OrderBookSimulator {
    pub order_book: OrderBook,
    // Parameters to control the "drift" of the clouds
    sentiment_drift: f64, 
}