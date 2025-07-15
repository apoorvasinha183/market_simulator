
<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import Header from '$lib/components/Header.svelte';
  import PriceChart from '$lib/components/PriceChart.svelte';
  import OrderBook from '$lib/components/OrderBook.svelte';
  import OrderEntry from '$lib/components/OrderEntry.svelte';
  import TradeHistory from '$lib/components/TradeHistory.svelte';
  import MarketInfoBar from '$lib/components/MarketInfoBar.svelte';
  import { TimeFrame, type WebSocketData, type MarketState, type Stock, type Candle } from '../lib/types';

  // State variables
  let marketState: MarketState | null = null;
  let candleData: { [key: string]: Candle[] } = {};
  let priceHistoryData: { [key: string]: [number, number][] } = {};
  let socket: WebSocket | null = null;
  let reconnectInterval: number | null = null;

  let selectedStockId: string = '1';
  let selectedTimeFrame: TimeFrame = TimeFrame.TenSeconds;
  let stockMap: Map<string, Stock> = new Map();
  
  let showCandlestickChart: boolean = true;

  const MAX_DATA_POINTS = 1000; // Define the window size

  // Reactive UI-bound variables
  $: selectedStock = stockMap.get(selectedStockId);

  // --- WebSocket and Data Processing ---

  function connectWebSocket() {
    if (socket) socket.close();
    socket = new WebSocket('ws://127.0.0.1:6969/ws');

    socket.onopen = () => {
      console.log('WebSocket connection established.');
      if (reconnectInterval) {
        clearInterval(reconnectInterval);
        reconnectInterval = null;
      }
    };

    socket.onmessage = (event) => {
      const message: WebSocketData = JSON.parse(event.data);
      if (message.type === 'snapshot') {
        processSnapshot(message);
      } else if (message.type === 'update') {
        processUpdate(message.data);
      }
    };

    socket.onclose = () => {
      console.log('WebSocket closed. Reconnecting...');
      if (!reconnectInterval) {
        reconnectInterval = window.setInterval(connectWebSocket, 3000);
      }
    };

    socket.onerror = (error) => {
      console.error('WebSocket error:', error);
      socket?.close();
    };
  }

  // Correctly process the initial full state snapshot
  function processSnapshot(data: WebSocketData) {
    marketState = data.market_state;
    candleData = data.candle_data || {};
    priceHistoryData = data.price_history || {};

    if (marketState?.stocks?.stocks) {
      const newStockMap = new Map<string, Stock>();
      marketState.stocks.stocks.forEach(s => newStockMap.set(s.id.toString(), s));
      stockMap = newStockMap;
      if (!stockMap.has(selectedStockId) && stockMap.size > 0) {
        selectedStockId = stockMap.keys().next().value;
      }
    }
  }

  // Correctly process incremental updates
  function processUpdate(data: any) {
    if (data.market_state) {
      // Use a functional update for reactivity with deep objects
      marketState = {
        ...marketState,
        ...data.market_state,
        order_books: { ...marketState?.order_books, ...data.market_state.order_books },
        last_traded_price: { ...marketState?.last_traded_price, ...data.market_state.last_traded_price },
        cumulative_volume: { ...marketState?.cumulative_volume, ...data.market_state.cumulative_volume },
        mid_prices: { ...marketState?.mid_prices, ...data.market_state.mid_prices },
        spreads: { ...marketState?.spreads, ...data.market_state.spreads },
      };

      if (priceHistoryData && data.market_state.last_traded_price) {
          for (const stockId in data.market_state.last_traded_price) {
              if (Object.prototype.hasOwnProperty.call(data.market_state.last_traded_price, stockId)) {
                  if (!priceHistoryData[stockId]) {
                      priceHistoryData[stockId] = [];
                  }
                  const newPrice = data.market_state.last_traded_price[stockId];
                  
                  // Sliding window logic for price history
                  if (priceHistoryData[stockId].length >= MAX_DATA_POINTS) {
                      priceHistoryData[stockId] = priceHistoryData[stockId].slice(-MAX_DATA_POINTS); // Keep only the last N points
                  }
                  priceHistoryData[stockId].push([Date.now(), newPrice]);
              }
          }
          priceHistoryData = { ...priceHistoryData }; // Trigger reactivity
      }
    }
    if (data.candle_data) {
      Object.keys(data.candle_data).forEach(key => {
        const newCandles: Candle[] = data.candle_data[key];
        if (!candleData[key]) {
          candleData[key] = [];
        }
        newCandles.forEach((newCandle: Candle) => {
          // Sliding window logic for candle data
          if (candleData[key].length >= MAX_DATA_POINTS) {
              candleData[key] = candleData[key].slice(-MAX_DATA_POINTS); // Keep only the last N points
          }

          const lastCandle = candleData[key][candleData[key].length - 1];
          if (lastCandle && lastCandle.timestamp === newCandle.timestamp) {
            candleData[key][candleData[key].length - 1] = newCandle; // Update last candle
          } else {
            candleData[key].push(newCandle); // Append new candle
          }
        });
      });
      candleData = { ...candleData }; // Trigger reactivity
    }
  }

  onMount(() => {
    connectWebSocket();
  });

  onDestroy(() => {
    if (reconnectInterval) clearInterval(reconnectInterval);
    socket?.close();
  });

  // --- Event Handlers ---

  function handleStockChange(event: CustomEvent<{ stockId: string }>) {
    selectedStockId = event.detail.stockId;
  }

  function handleTimeFrameChange(event: CustomEvent<{ timeframe: TimeFrame }>) {
    selectedTimeFrame = event.detail.timeframe;
  }

  function handleChartTypeToggle() {
    showCandlestickChart = !showCandlestickChart;
  }
</script>

<div class="trading-cockpit">
  <Header 
    {stockMap} 
    {selectedStockId} 
    {selectedTimeFrame}
    on:stockChange={handleStockChange}
    on:timeframeChange={handleTimeFrameChange}
    on:chartTypeToggle={handleChartTypeToggle}
    isCandlestick={showCandlestickChart}
  />

  <MarketInfoBar {marketState} {selectedStockId} />

  <main class="main-content">
    <div class="chart-panel">
      <PriceChart 
        {selectedStockId} 
        {selectedTimeFrame}
        {candleData}
        {priceHistoryData}
        isCandlestick={showCandlestickChart}
      />
    </div>
    <div class="side-panel">
      <OrderBook {marketState} {selectedStockId} />
      <TradeHistory />
    </div>
  </main>

  <footer class="action-panel">
    <OrderEntry />
  </footer>
</div>

<style>
  :global(body) {
    margin: 0;
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen,
      Ubuntu, Cantarell, 'Open Sans', 'Helvetica Neue', sans-serif;
    background-color: #131722;
    color: #d1d4dc;
  }

  .trading-cockpit {
    display: flex;
    flex-direction: column;
    height: 100vh;
    width: 100vw;
  }

  .main-content {
    flex-grow: 1;
    display: grid;
    grid-template-columns: 1fr 350px;
    gap: 8px;
    padding: 8px;
    overflow: hidden;
  }

  .chart-panel {
    display: flex;
    flex-direction: column;
    background-color: #1c212e;
    border-radius: 4px;
    border: 1px solid #2a2e39;
  }

  .side-panel {
    display: grid;
    grid-template-rows: 1fr auto;
    gap: 8px;
    overflow: hidden;
  }

  .action-panel {
    flex-shrink: 0;
    padding: 0 8px 8px 8px;
  }
</style>
