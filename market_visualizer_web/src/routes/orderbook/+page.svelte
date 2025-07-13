<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { Chart, registerables } from 'chart.js';
  import type { MarketState, OrderBook, Stock, WebSocketData } from '../../lib/types';

  Chart.register(...registerables);

  let marketState: MarketState | null = null;
  let socket: WebSocket | null = null;
  let reconnectInterval: number | null = null;

  let selectedStockId: string = '1';
  let stockMap: Map<string, Stock> = new Map();
  let stockTickers: string[] = [];
  
  // Reactive variables for the UI
  $: selectedStock = stockMap.get(selectedStockId);
  $: selectedStockTicker = selectedStock?.ticker || 'N/A';
  $: selectedStockCompanyName = selectedStock?.company_name || 'N/A';
  
  let orderBookChart: Chart | null = null;
  let orderBookCanvas: HTMLCanvasElement;

  // Reactive variables for bids and asks tables
  let bids: [number, number][] = [];
  let asks: [number, number][] = [];

  // Calculate max volume for depth bar visualization
  $: maxVolume = Math.max(
    ...bids.map(([, volume]) => volume),
    ...asks.map(([, volume]) => volume),
    1 // Ensure it's never zero to avoid division by zero
  );

  function connectWebSocket() {
    if (socket) socket.close();
    socket = new WebSocket('ws://127.0.0.1:6969/ws');

    socket.onopen = () => {
      console.log('WebSocket connection opened for Order Book');
      if (reconnectInterval) {
        clearInterval(reconnectInterval);
        reconnectInterval = null;
      }
    };

    socket.onmessage = (event) => {
      const message: WebSocketData = JSON.parse(event.data);
      if (message.type === 'snapshot') processSnapshot(message);
      else if (message.type === 'update') processUpdate(message);
    };

    socket.onclose = () => {
      console.log('WebSocket connection closed. Attempting to reconnect...');
      if (!reconnectInterval) {
        reconnectInterval = window.setInterval(connectWebSocket, 3000);
      }
    };

    socket.onerror = (error) => {
      console.error('WebSocket error:', error);
      socket?.close();
    };
  }

  onMount(() => {
    initializeChart();
    connectWebSocket();
  });

  onDestroy(() => {
    if (reconnectInterval) clearInterval(reconnectInterval);
    socket?.close();
    orderBookChart?.destroy();
  });

  function processSnapshot(data: WebSocketData) {
    if (data.market_state) marketState = data.market_state;

    if (marketState?.stocks?.stocks) {
      const newStockMap = new Map<string, Stock>();
      marketState.stocks.stocks.forEach(s => newStockMap.set(s.id.toString(), s));
      stockMap = newStockMap;
      stockTickers = Array.from(stockMap.values()).map(s => s.ticker).sort();

      if (stockTickers.length > 0 && !stockMap.has(selectedStockId)) {
        selectedStockId = getStockIdFromTicker(stockTickers[0]) || '1';
      }
    }
    updateOrderBook();
  }

  function processUpdate(data: WebSocketData) {
    const update = data.data; // The actual update is nested
    if (update.market_state) {
        // A more robust way to merge states
        marketState = { ...marketState, ...update.market_state };
        if (update.market_state.order_books) {
            marketState.order_books = { ...marketState.order_books, ...update.market_state.order_books };
        }
    }
    updateOrderBook();
  }

  function initializeChart() {
    if (orderBookCanvas && !orderBookChart) {
      orderBookChart = new Chart(orderBookCanvas, {
        type: 'line',
        options: {
          responsive: true,
          maintainAspectRatio: false,
          scales: {
            x: { type: 'linear', title: { display: true, text: 'Price', color: '#e0e0e0' }, ticks: { color: '#b0b0b0' }, grid: { color: 'rgba(255, 255, 255, 0.1)' } },
            y: { title: { display: true, text: 'Cumulative Volume', color: '#e0e0e0' }, ticks: { color: '#b0b0b0' }, grid: { color: 'rgba(255, 255, 255, 0.1)' } }
          },
          plugins: { legend: { labels: { color: '#e0e0e0' } } },
          elements: { line: { borderWidth: 2 } }
        },
        data: {
          labels: [],
          datasets: [
            { label: 'Bids', data: [], borderColor: '#00b894', backgroundColor: 'rgba(0, 184, 148, 0.2)', fill: true, stepped: true },
            { label: 'Asks', data: [], borderColor: '#ff7675', backgroundColor: 'rgba(255, 118, 117, 0.2)', fill: true, stepped: true }
          ]
        }
      });
    }
  }

  function updateOrderBook() {
    if (!marketState?.order_books?.[selectedStockId]) {
      bids = [];
      asks = [];
      if(orderBookChart) {
        orderBookChart.data.datasets[0].data = [];
        orderBookChart.data.datasets[1].data = [];
        orderBookChart.update('none');
      }
      return;
    }

    const orderBook: OrderBook = marketState.order_books[selectedStockId];

    // Process and sort bids (descending)
    bids = Object.entries(orderBook.bids)
      .map(([price, level]) => [parseFloat(price), level.total_volume] as [number, number])
      .sort((a, b) => b[0] - a[0]);

    // Process and sort asks (ascending)
    asks = Object.entries(orderBook.asks)
      .map(([price, level]) => [parseFloat(price), level.total_volume] as [number, number])
      .sort((a, b) => a[0] - b[0]);

    updateOrderBookChart();
  }

  function updateOrderBookChart() {
    if (!orderBookChart) return;
    
    // Create cumulative volume data for the chart
    const bidData: {x: number, y: number}[] = [];
    let cumulativeBidVolume = 0;
    for (const [price, volume] of bids) { // Bids are already sorted descending
        cumulativeBidVolume += volume;
        bidData.push({ x: price, y: cumulativeBidVolume });
    }

    const askData: {x: number, y: number}[] = [];
    let cumulativeAskVolume = 0;
    for (const [price, volume] of asks) { // Asks are already sorted ascending
        cumulativeAskVolume += volume;
        askData.push({ x: price, y: cumulativeAskVolume });
    }

    orderBookChart.data.datasets[0].data = bidData.sort((a, b) => a.x - b.x); // Sort bids ascending for charting
    orderBookChart.data.datasets[1].data = askData;
    orderBookChart.update('none');
  }

  function getStockIdFromTicker(ticker: string): string | undefined {
    const stock = Array.from(stockMap.values()).find(s => s.ticker === ticker);
    return stock ? stock.id.toString() : undefined;
  }

  function formatNumber(num: number | null): string {
    if (num === null) return 'N/A';
    return num.toLocaleString(undefined, { minimumFractionDigits: 0, maximumFractionDigits: 0 });
  }

</script>

<main class="dark-theme">
  <div class="header">
    <div class="title-section">
      <h1>Order Book</h1>
      <div class="stock-info">
        <h2>{selectedStockTicker}</h2>
        <p>{selectedStockCompanyName}</p>
      </div>
    </div>
    <div class="controls">
      <a href="/" class="button">Market Overview</a>
      <div class="select-wrapper">
        <label for="stock-select">Stock:</label>
        <select id="stock-select" bind:value={selectedStockId} on:change={updateOrderBook}>
          {#each stockTickers as ticker}
            <option value={getStockIdFromTicker(ticker)}>{ticker}</option>
          {/each}
        </select>
      </div>
    </div>
  </div>

  <div class="content-grid">
    <div class="card chart-container">
      <h2>Order Book Depth</h2>
      <canvas bind:this={orderBookCanvas}></canvas>
    </div>

    <div class="order-book-tables">
      <div class="card bids-table">
        <h2>Bids</h2>
        <table>
          <thead>
            <tr><th>Price</th><th>Volume</th></tr>
          </thead>
          <tbody>
            {#each bids as [price, volume]}
              <tr>
                <td>
                  <div class="price-cell">
                    <span class="price-value bid-price">{price.toFixed(2)}</span>
                    <div class="volume-bar-bg">
                      <div class="volume-bar bid-bar" style="width: {(volume / maxVolume) * 100}%;"></div>
                    </div>
                  </div>
                </td>
                <td>{formatNumber(volume)}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>

      <div class="card asks-table">
        <h2>Asks</h2>
        <table>
          <thead>
            <tr><th>Price</th><th>Volume</th></tr>
          </thead>
          <tbody>
            {#each asks as [price, volume]}
              <tr>
                <td>
                  <div class="price-cell">
                    <span class="price-value ask-price">{price.toFixed(2)}</span>
                    <div class="volume-bar-bg">
                      <div class="volume-bar ask-bar" style="width: {(volume / maxVolume) * 100}%;"></div>
                    </div>
                  </div>
                </td>
                <td>{formatNumber(volume)}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    </div>
  </div>
</main>

<style>
  :global(body) { margin: 0; font-family: 'Inter', sans-serif; background-color: #1a1a2e; color: #e0e0e0; }
  .dark-theme { background-color: #1a1a2e; color: #e0e0e0; }
  .header { display: flex; justify-content: space-between; align-items: center; padding: 1.5em 2.5em; background-color: #16213e; border-bottom: 1px solid #0f3460; box-shadow: 0 4px 8px rgba(0, 0, 0, 0.2); }
  .title-section { display: flex; align-items: baseline; gap: 1.5em; }
  h1 { color: #e0e0e0; margin: 0; font-size: 2.2em; font-weight: 700; }
  .stock-info h2 { margin: 0; font-size: 1.8em; color: #e94560; font-weight: 600; }
  .stock-info p { margin: 0; font-size: 1em; color: #a0a0a0; }
  .controls { display: flex; align-items: center; gap: 1.2em; }
  .controls label { font-weight: 500; color: #b0b0b0; font-size: 0.95em; }
  .button { padding: 0.6em 1.2em; border-radius: 6px; border: none; background-color: #0f3460; color: #e0e0e0; cursor: pointer; font-size: 0.95em; font-weight: 500; transition: background-color 0.2s ease; text-decoration: none; display: inline-flex; align-items: center; justify-content: center; }
  .button:hover { background-color: #1a527f; }
  .select-wrapper { display: flex; align-items: center; gap: 0.5em; }
  .controls select { padding: 0.5em 1em; border-radius: 6px; border: 1px solid #0f3460; background-color: #16213e; color: #e0e0e0; cursor: pointer; font-size: 0.95em; appearance: none; -webkit-appearance: none; -moz-appearance: none; background-image: url('data:image/svg+xml;charset=US-ASCII,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20width%3D%22292.4%22%20height%3D%22292.4%22%3E%3Cpath%20fill%3D%22%23e0e0e0%22%20d%3D%22M287%2C197.3L159.3%2C69.6c-3.7-3.7-9.7-3.7-13.4%2C0L5.3%2C197.3c-3.7%2C3.7-3.7%2C9.7%2C0%2C13.4l13.4%2C13.4c3.7%2C3.7%2C9.7%2C3.7%2C13.4%2C0l110.7-110.7l110.7%2C110.7c3.7%2C3.7%2C9.7%2C3.7%2C13.4%2C0l13.4-13.4C290.7%2C207%2C290.7%2C201%2C287%2C197.3z%22%2F%3E%3C%2Fsvg%3E'); background-repeat: no-repeat; background-position: right 0.7em top 50%; background-size: 0.65em auto; }
  .content-grid { display: grid; grid-template-columns: 1fr; gap: 2.5em; padding: 2.5em; }
  .card { background-color: #16213e; padding: 2em; border-radius: 10px; box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3); border: 1px solid #0f3460; }
  h2 { color: #e0e0e0; margin-top: 0; margin-bottom: 1.5em; font-size: 1.8em; border-bottom: 1px solid #0f3460; padding-bottom: 0.8em; font-weight: 600; }
  .chart-container { height: 400px; }
  .order-book-tables { display: grid; grid-template-columns: 1fr 1fr; gap: 2.5em; margin-top: 2.5em; }
  table { width: 100%; border-collapse: collapse; font-size: 0.95em; }
  th, td { padding: 0.8em 1em; text-align: left; border-bottom: 1px solid #0f3460; }
  th { background-color: #0f3460; font-weight: 600; color: #e0e0e0; position: sticky; top: 0; z-index: 1; }
  tbody tr:last-child td { border-bottom: none; }
  .price-cell { display: flex; align-items: center; position: relative; }
  .price-value { font-weight: 700; font-size: 1.1em; flex-shrink: 0; width: 80px; text-align: right; padding-right: 10px; }
  .bid-price { color: #00b894; }
  .ask-price { color: #ff7675; }
  .volume-bar-bg { position: absolute; top: 0; left: 0; right: 0; bottom: 0; z-index: -1; }
  .volume-bar { height: 100%; transition: width 0.1s ease-out; opacity: 0.2; }
  .bid-bar { background-color: #00b894; }
  .ask-bar { background-color: #ff7675; }
  td { color: #c0c0c0; }
</style>