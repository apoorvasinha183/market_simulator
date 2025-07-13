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
        data: {
          datasets: [{
            label: 'Price',
            data: [],
            borderColor: 'rgb(75, 192, 192)'
          }]
        },
        options: {
          scales: {
            x: {
              type: 'time',
              ticks: {
                display: false
              }
            },
            y: {
              beginAtZero: false
            }
          }
        }
      });
    }
  }

  function initializeCandlestickChart() {
    if (candlestickCanvas && !candlestickChart) {
      candlestickChart = new Chart(candlestickCanvas, {
        type: 'candlestick',
        data: {
          datasets: [{
            label: 'Candles',
            data: []
          }]
        },
        options: {
          scales: {
            x: {
              type: 'time'
            },
            y: {
              beginAtZero: false
            }
          }
        }
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

<main class="dark-theme">
  <div class="header">
    <div class="title-section">
      <h1>Market Visualizer</h1>
      <div class="stock-info">
        <h2>{selectedStockTicker}</h2>
        <p>{selectedStockCompanyName}</p>
      </div>
    </div>
    <div class="controls">
      <a href="/orderbook" class="button">Order Book</a>
      <div class="select-wrapper">
        <label for="stock-select">Stock:</label>
        <select id="stock-select" bind:value={selectedStockId} on:change={(e) => {
          selectedStockId = (e.target as HTMLSelectElement).value;
          updateCandlestickChart(true);
          updateLineChart(true);
        }}>
          {#each stockTickers as ticker}
            <option value={getStockIdFromTicker(ticker)}>{ticker}</option>
          {/each}
        </select>
      </div>
      <div class="select-wrapper">
        <label for="timeframe-select">Timeframe:</label>
        <select id="timeframe-select" bind:value={selectedTimeFrame} on:change={() => updateCandlestickChart(true)}>
          {#each Object.values(TimeFrame) as tf}
            <option value={tf}>{tf}</option>
          {/each}
        </select>
      </div>
      <button class="button" on:click={() => showCandlestickChart = !showCandlestickChart}>
        {showCandlestickChart ? 'Show Line Chart' : 'Show Candlestick Chart'}
      </button>
    </div>
  </div>

  <div class="market-summary-cards">
    <div class="card summary-card">
      <span class="label">Last:</span>
      <span class="value">${formatNumber(lastTradedPrice)}</span>
    </div>
    <div class="card summary-card">
      <span class="label">Volume:</span>
      <span class="value">{formatNumber(cumulativeVolume)}</span>
    </div>
    <div class="card summary-card">
      <span class="label">Mid:</span>
      <span class="value">${midPrice}</span>
    </div>
    <div class="card summary-card">
      <span class="label">Spread:</span>
      <span class="value">${spread}</span>
    </div>
  </div>

  <div class="content-grid">
    <div class="card chart-container">
      <div style="display: {showCandlestickChart ? 'block' : 'none'}">
        <h2>Candlestick Chart ({selectedStockTicker} - {selectedTimeFrame})</h2>
        <canvas bind:this={candlestickCanvas}></canvas>
      </div>
      <div style="display: {!showCandlestickChart ? 'block' : 'none'}">
        <h2>Price Chart ({selectedStockTicker})</h2>
        <canvas bind:this={chartCanvas}></canvas>
      </div>
    </div>
  </div>
</main>

<style>
  :global(body) {
    margin: 0;
    font-family: 'Inter', sans-serif;
    background-color: #1a1a2e;
    color: #e0e0e0;
  }

  .dark-theme {
    background-color: #1a1a2e;
    color: #e0e0e0;
  }

  .header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 1.5em 2.5em;
    background-color: #16213e;
    border-bottom: 1px solid #0f3460;
    box-shadow: 0 4px 8px rgba(0, 0, 0, 0.2);
  }

  .title-section {
    display: flex;
    align-items: baseline;
    gap: 1.5em;
  }

  h1 {
    color: #e0e0e0;
    margin: 0;
    font-size: 2.2em;
    font-weight: 700;
  }

  .stock-info h2 {
    margin: 0;
    font-size: 1.8em;
    color: #e94560;
    font-weight: 600;
  }

  .stock-info p {
    margin: 0;
    font-size: 1em;
    color: #a0a0a0;
  }

  .controls {
    display: flex;
    align-items: center;
    gap: 1.2em;
  }

  .controls label {
    font-weight: 500;
    color: #b0b0b0;
    font-size: 0.95em;
  }

  .button {
    padding: 0.6em 1.2em;
    border-radius: 6px;
    border: none;
    background-color: #0f3460;
    color: #e0e0e0;
    cursor: pointer;
    font-size: 0.95em;
    font-weight: 500;
    transition: background-color 0.2s ease;
    text-decoration: none;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  .button:hover {
    background-color: #1a527f;
  }

  .select-wrapper {
    display: flex;
    align-items: center;
    gap: 0.5em;
  }

  .controls select {
    padding: 0.5em 1em;
    border-radius: 6px;
    border: 1px solid #0f3460;
    background-color: #16213e;
    color: #e0e0e0;
    cursor: pointer;
    font-size: 0.95em;
    appearance: none;
    /* Remove default arrow */
    -webkit-appearance: none;
    -moz-appearance: none;
    background-image: url('data:image/svg+xml;charset=US-ASCII,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20width%3D%22292.4%22%20height%3D%22292.4%22%3E%3Cpath%20fill%3D%22%23e0e0e0%22%20d%3D%22M287%2C197.3L159.3%2C69.6c-3.7-3.7-9.7-3.7-13.4%2C0L5.3%2C197.3c-3.7%2C3.7-3.7%2C9.7%2C0%2C13.4l13.4%2C13.4c3.7%2C3.7%2C9.7%2C3.7%2C13.4%2C0l110.7-110.7l110.7%2C110.7c3.7%2C3.7%2C9.7%2C3.7%2C13.4%2C0l13.4-13.4C290.7%2C207%2C290.7%2C201%2C287%2C197.3z%22%2F%3E%3C%2Fsvg%3E');
    background-repeat: no-repeat;
    background-position: right 0.7em top 50%;
    background-size: 0.65em auto;
  }

  .market-summary-cards {
    display: flex;
    justify-content: space-around;
    padding: 1.5em 2.5em;
    background-color: #16213e;
    border-bottom: 1px solid #0f3460;
    box-shadow: 0 2px 4px rgba(0, 0, 0, 0.1);
    gap: 1.5em;
  }

  .summary-card {
    flex: 1;
    text-align: center;
    padding: 1em;
    background-color: #0f3460;
    border-radius: 8px;
    box-shadow: 0 2px 4px rgba(0, 0, 0, 0.2);
    display: flex;
    flex-direction: column;
    align-items: center;
  }

  .summary-card .label {
    font-size: 0.9em;
    color: #b0b0b0;
    margin-bottom: 0.3em;
  }

  .summary-card .value {
    font-size: 1.6em;
    font-weight: 700;
    color: #e94560;
    /* Accent color */
  }

  .content-grid {
    display: grid;
    grid-template-columns: 1fr;
    gap: 2em;
    padding: 2.5em;
  }

  .card {
    background-color: #16213e;
    padding: 2em;
    border-radius: 10px;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
    border: 1px solid #0f3460;
  }

  h2 {
    color: #e0e0e0;
    margin-top: 0;
    margin-bottom: 1.5em;
    font-size: 1.8em;
    border-bottom: 1px solid #0f3460;
    padding-bottom: 0.8em;
    font-weight: 600;
  }
</style>