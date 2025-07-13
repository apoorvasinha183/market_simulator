<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { Chart, registerables, type ChartConfiguration } from 'chart.js';
  import 'chartjs-adapter-date-fns';
  import { TimeFrame, type Candle, type MarketState, type WebSocketData, type Stock } from '../lib/types';

  let CandlestickController, CandlestickElement;

  Chart.register(...registerables);

  let marketState: MarketState | null = null;
  let candleData: { [key: string]: Candle[] } | null = null;
  let priceHistoryData: { [key: string]: [number, number][] } | null = null;
  let socket: WebSocket | null = null;
  let reconnectInterval: number | null = null;

  let selectedStockId: string = '1';
  let selectedTimeFrame: TimeFrame = TimeFrame.TenSeconds;
  let stockMap: Map<string, Stock> = new Map();
  let stockTickers: string[] = [];

  let lastTradedPrice: number | null = null;
  let cumulativeVolume: number | null = null;
  let midPrice: string = 'N/A';
  let spread: string = 'N/A';

  // Reactive variables for selected stock info
  $: selectedStock = stockMap.get(selectedStockId);
  $: selectedStockTicker = selectedStock?.ticker || 'N/A';
  $: selectedStockCompanyName = selectedStock?.company_name || 'N/A';

  $: {
    if (marketState && selectedStockId) {
        lastTradedPrice = marketState.last_traded_price?.[selectedStockId] ?? null;
        cumulativeVolume = marketState.cumulative_volume?.[selectedStockId] ?? null;
        midPrice = marketState.mid_prices?.[selectedStockId] ? formatNumber(marketState.mid_prices[selectedStockId]) : 'N/A';
        spread = marketState.spreads?.[selectedStockId] ? formatNumber(marketState.spreads[selectedStockId]) : 'N/A';
    }
  }

  let priceHistory: { x: number; y: number }[] = [];
  let priceChart: Chart | null = null;
  let chartCanvas: HTMLCanvasElement;

  let candlestickChart: Chart | null = null;
  let candlestickCanvas: HTMLCanvasElement;
  let showCandlestickChart: boolean = true;

  const MAX_PRICE_HISTORY = Infinity; // Keep all history

  function connectWebSocket() {
    if (socket) socket.close();
    socket = new WebSocket('ws://127.0.0.1:6969/ws');

    socket.onopen = () => {
      console.log('WebSocket connection opened');
      if (reconnectInterval) {
        clearInterval(reconnectInterval);
        reconnectInterval = null;
      }
    };

    socket.onmessage = (event) => {
      const message = JSON.parse(event.data);
      if (message.type === 'snapshot') processSnapshot(message);
      else if (message.type === 'update') processUpdate(message.data);
    };

    socket.onclose = (event) => {
      console.log('WebSocket connection closed', event.code, event.reason);
      if (!reconnectInterval) {
        reconnectInterval = window.setInterval(connectWebSocket, 3000);
      }
    };

    socket.onerror = (error) => {
      console.error('WebSocket error:', error);
      socket?.close();
    };
  }

  onMount(async () => {
    const financial = await import('chartjs-chart-financial');
    CandlestickController = financial.CandlestickController;
    CandlestickElement = financial.CandlestickElement;
    Chart.register(CandlestickController, CandlestickElement);

    initializeCharts();
    connectWebSocket();
  });

  onDestroy(() => {
      if (reconnectInterval) clearInterval(reconnectInterval);
      socket?.close();
      priceChart?.destroy();
      candlestickChart?.destroy();
  });

  function initializeCharts() {
    initializeLineChart();
    initializeCandlestickChart();
  }

  function processSnapshot(data: WebSocketData) {
    marketState = data.market_state;
    candleData = data.candle_data;
    priceHistoryData = data.price_history;

    if (marketState?.stocks?.stocks) {
      // Force reactivity by creating new Map
      const newStockMap = new Map();
      marketState.stocks.stocks.forEach(s => newStockMap.set(s.id.toString(), s));
      stockMap = newStockMap;
      
      stockTickers = Array.from(stockMap.values()).map(s => s.ticker).sort();
      
      // Force-set to the first available stock on initial load to ensure UI consistency.
      if (stockTickers.length > 0) {
        const firstStock = Array.from(stockMap.values()).find(s => s.ticker === stockTickers[0]);
        if (firstStock) {
            selectedStockId = firstStock.id.toString();
        }
      }
    }
    updateUIData();
    updateCandlestickChart(true);
    updateLineChart(true);
  }

  function processUpdate(data: any) {
    const updateData = data;
    if (updateData.market_state) {
        marketState = updateData.market_state;
        if (priceHistoryData && marketState) {
            for (const stockId in marketState.last_traded_price) {
                if (!priceHistoryData[stockId]) priceHistoryData[stockId] = [];
                const newPrice = marketState.last_traded_price[stockId];
                priceHistoryData[stockId].push([Date.now(), newPrice]);
                if (priceHistoryData[stockId].length > MAX_PRICE_HISTORY) {
            // This block is now effectively disabled but kept for clarity
            // priceHistoryData[stockId] = priceHistoryData[stockId].slice(-MAX_PRICE_HISTORY);
          }
            }
        }
        updateUIData();
    }
    if (updateData.candle_data) {
        if (!candleData) candleData = {}; // Initialize if null
        Object.keys(updateData.candle_data).forEach(key => {
            const newCandles: Candle[] = updateData.candle_data[key];
            if (!candleData) return; // Type guard

            if (!candleData[key]) {
                candleData[key] = [];
            }

            newCandles.forEach((newCandle: Candle) => {
                if (!candleData) return; // Type guard
                const lastCandle = candleData[key][candleData[key].length - 1];
                if (lastCandle && lastCandle.timestamp === newCandle.timestamp) {
                    // Update the last candle
                    candleData[key][candleData[key].length - 1] = newCandle;
                } else {
                    // Append as a new candle
                    candleData[key].push(newCandle);
                }
            });
        });
        updateCandlestickChart(false); // Incremental update
    }
  }

  function updateUIData() {
    if (marketState && selectedStockId) {
      lastTradedPrice = marketState.last_traded_price[selectedStockId] || null;
      cumulativeVolume = marketState.cumulative_volume[selectedStockId] || null;

      if (lastTradedPrice !== null) {
        const newPricePoint = { x: Date.now(), y: lastTradedPrice };
        priceHistory = [...priceHistory, newPricePoint];
        if (priceHistory.length > MAX_PRICE_HISTORY) {
          // This block is now effectively disabled
          // priceHistory = priceHistory.slice(-MAX_PRICE_HISTORY);
        }
        updateLineChart(false);
      }
    }
  }

  function initializeLineChart() {
    if (chartCanvas && !priceChart) {
      priceChart = new Chart(chartCanvas, {
        type: 'line',
        data: { datasets: [{ label: 'Price', data: [], borderColor: 'rgb(75, 192, 192)' }] },
        options: { scales: { x: { type: 'time', ticks: { display: false } }, y: { beginAtZero: false } } }
      });
    }
  }

  function initializeCandlestickChart() {
    if (candlestickCanvas && !candlestickChart) {
      candlestickChart = new Chart(candlestickCanvas, {
        type: 'candlestick',
        data: { datasets: [{ label: 'Candles', data: [] }] },
        options: { scales: { x: { type: 'time' }, y: { beginAtZero: false } } }
      });
    }
  }

  function updateLineChart(fullRedraw: boolean) {
    if (!priceChart) return;
    if (fullRedraw && priceHistoryData?.[selectedStockId]) {
      priceHistory = priceHistoryData[selectedStockId].map(([timestamp, price]) => ({ x: timestamp, y: price }));
    }
    priceChart.data.datasets[0].data = priceHistory;
    priceChart.update('none');
  }

  const timeFrameMap: { [key: string]: string } = {
    "HundredMillis": "100ms",
    "OneSecond": "1s",
    "TenSeconds": "10s",
    "OneMinute": "1m",
    "FiveMinutes": "5m",
    "ThirtyMinutes": "30m",
  };

  function updateCandlestickChart(fullRedraw: boolean) {
    if (!candlestickChart || !candleData) return;

    const backendTimeFrame = timeFrameMap[selectedTimeFrame];
    if (!backendTimeFrame) {
        // This case should ideally not happen if the enums are aligned
        candlestickChart.data.datasets[0].data = [];
        candlestickChart.update('none');
        return;
    }

    const key = `${selectedStockId}-${backendTimeFrame}`;
    const candles = candleData[key];

    if (candles) {
      const ohlcData = candles.map(c => ({ x: c.timestamp, o: c.open, h: c.high, l: c.low, c: c.close }));
      candlestickChart.data.datasets[0].data = ohlcData;
    } else {
      // If no candles for this key, clear the chart
      candlestickChart.data.datasets[0].data = [];
    }
    candlestickChart.update('none');
  }

  function getStockIdFromTicker(ticker: string): string {
    const stock = Array.from(stockMap.values()).find(s => s.ticker === ticker);
    return stock ? stock.id.toString() : '1';
  }

  function formatNumber(num: number | null): string {
    if (num === null) return 'N/A';
    return num.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 });
  }

</script>

<main>
  <div class="header">
    <div class="title-section">
      <h1>Market Visualizer</h1>
      <div class="stock-info">
        <h2>{selectedStockTicker}</h2>
        <p>{selectedStockCompanyName}</p>
      </div>
    </div>
    <div class="controls">
      <a href="/orderbook">Order Book</a>
      <label for="stock-select">Select Stock:</label>
      <select id="stock-select" bind:value={selectedStockId} on:change={(e) => {
        selectedStockId = (e.target as HTMLSelectElement).value;
        // No longer manually clearing history. Let the update functions handle it.
        updateCandlestickChart(true); // full redraw
        updateLineChart(true); // full redraw
      }}>
        {#each stockTickers as ticker}
          <option value={getStockIdFromTicker(ticker)}>{ticker}</option>
        {/each}
      </select>
      <label for="timeframe-select">Timeframe:</label>
      <select id="timeframe-select" bind:value={selectedTimeFrame} on:change={() => updateCandlestickChart(true)}>
        {#each Object.values(TimeFrame) as tf}
          <option value={tf}>{tf}</option>
        {/each}
      </select>
      <button on:click={() => showCandlestickChart = !showCandlestickChart}>
        {showCandlestickChart ? 'Show Line Chart' : 'Show Candlestick Chart'}
      </button>
    </div>
  </div>

  <div class="market-summary">
    <div>Last: <span class="value">${formatNumber(lastTradedPrice)}</span></div>
    <div>Volume: <span class="value">{formatNumber(cumulativeVolume)}</span></div>
    <div>Mid: <span class="value">${midPrice}</span></div>
    <div>Spread: <span class="value">${spread}</span></div>
  </div>

  <div class="content-grid">
    <div class="chart-container">
      <div style="display: {showCandlestickChart ? 'block' : 'none'};">
          <h2>Candlestick Chart ({selectedStockTicker} - {selectedTimeFrame})</h2>
          <canvas bind:this={candlestickCanvas}></canvas>
      </div>
      <div style="display: {!showCandlestickChart ? 'block' : 'none'};">
          <h2>Price Chart ({selectedStockTicker})</h2>
          <canvas bind:this={chartCanvas}></canvas>
      </div>
    </div>
  </div>
</main>

<style>
  :global(body) {
    margin: 0;
    font-family: 'Arial', sans-serif;
    background-color: #f0f2f5;
    color: #333;
  }

  .header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 1em 2em;
    background-color: #fff;
    border-bottom: 1px solid #eee;
    box-shadow: 0 2px 4px rgba(0, 0, 0, 0.05);
  }

  .title-section {
    display: flex;
    align-items: baseline;
    gap: 1em;
  }

  h1 {
    color: #333;
    margin: 0;
    font-size: 1.8em;
  }

  .stock-info h2 {
    margin: 0;
    font-size: 1.5em;
    color: #007bff;
  }

  .stock-info p {
    margin: 0;
    font-size: 0.9em;
    color: #666;
  }

  .controls {
    display: flex;
    align-items: center;
    gap: 1em;
  }

  .controls label {
    font-weight: bold;
    color: #555;
  }

  .controls select,
  .controls button {
    padding: 0.5em 1em;
    border-radius: 5px;
    border: 1px solid #ccc;
    background-color: #f8f8f8;
    cursor: pointer;
    font-size: 1em;
  }

  .controls button:hover {
    background-color: #e0e0e0;
  }

  .market-summary {
    display: flex;
    justify-content: space-around;
    padding: 1em 2em;
    background-color: #e9ecef;
    border-bottom: 1px solid #dee2e6;
    font-size: 1.1em;
    font-weight: bold;
    color: #555;
  }

  .market-summary .value {
    color: #007bff;
  }

  .content-grid {
    display: grid;
    grid-template-columns: 1fr;
    gap: 2em;
    padding: 2em;
  }

  .chart-container {
    background-color: #fff;
    padding: 1.5em;
    border-radius: 8px;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
  }

  h2 {
    color: #555;
    margin-top: 0;
    margin-bottom: 1em;
    font-size: 1.4em;
    border-bottom: 1px solid #eee;
    padding-bottom: 0.5em;
  }
</style>