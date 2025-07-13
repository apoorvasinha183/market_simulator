<script lang="ts">
  import { onMount } from 'svelte';
  import { Chart, registerables, type ChartConfiguration } from 'chart.js';
  import { type MarketState, type Stock } from '../../lib/types';

  Chart.register(...registerables);

  let marketState: MarketState | null = null;
  let socket: WebSocket | null = null;
  let reconnectInterval: number | null = null;
  let selectedStockId: string = '1';
  let stockMap: Map<string, Stock> = new Map();
  let stockTickers: string[] = [];
  let orderBookChart: Chart | null = null;
  let orderBookCanvas: HTMLCanvasElement;
  let bids: [number, number][] = [];
  let asks: [number, number][] = [];

  function connectWebSocket() {
    if (socket) {
      socket.close();
    }
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
      if (message.type === 'snapshot') {
        marketState = message.market_state;
        if (marketState && marketState.stocks && marketState.stocks.stocks) {
          stockMap.clear();
          marketState.stocks.stocks.forEach(s => stockMap.set(s.id.toString(), s));
          stockTickers = Array.from(stockMap.values()).map(s => s.ticker).sort();
          if (!stockMap.has(selectedStockId) && stockTickers.length > 0) {
            selectedStockId = Array.from(stockMap.values()).find(s => s.ticker === stockTickers[0])?.id.toString() || '1';
          }
        }
        updateOrderBookChart();
      } else if (message.type === 'update') {
        marketState = message.data.market_state;
        updateOrderBookChart();
      }
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

  onMount(() => {
    // Don't connect here anymore
    return () => {
      if (reconnectInterval) clearInterval(reconnectInterval);
      socket?.close();
      orderBookChart?.destroy();
    };
  });

  function updateOrderBookChart() {
    if (!marketState || !orderBookChart || !marketState.order_books || !selectedStockId) return;

    const book = marketState.order_books[selectedStockId];
    if (!book) return;

    bids = Object.entries(book.bids).map(([p, l]) => [parseFloat(p) / 100, l.total_volume]).sort((a, b) => b[0] - a[0]);
    asks = Object.entries(book.asks).map(([p, l]) => [parseFloat(p) / 100, l.total_volume]).sort((a, b) => a[0] - b[0]);

    let cumulativeVolume = 0;
    const askPoints = Object.entries(book.asks)
        .sort((a, b) => parseFloat(a[0]) - parseFloat(b[0]))
        .flatMap(([price, level]) => {
            const priceF = parseFloat(price) / 100;
            const prevCumulative = cumulativeVolume;
            cumulativeVolume += level.total_volume;
            return [{x: priceF, y: prevCumulative}, {x: priceF, y: cumulativeVolume}];
        });

    cumulativeVolume = 0;
    const bidPoints = Object.entries(book.bids)
        .sort((a, b) => parseFloat(b[0]) - parseFloat(a[0]))
        .flatMap(([price, level]) => {
            const priceF = parseFloat(price) / 100;
            const prevCumulative = cumulativeVolume;
            cumulativeVolume += level.total_volume;
            return [{x: priceF, y: prevCumulative}, {x: priceF, y: cumulativeVolume}];
        });

    orderBookChart.data.datasets[0].data = askPoints;
    orderBookChart.data.datasets[1].data = bidPoints;
    orderBookChart.update('none');
  }

  $: if (orderBookCanvas && !orderBookChart) {
    const orderBookConfig: ChartConfiguration = {
      type: 'line',
      data: {
        datasets: [
          {
            label: 'Asks',
            data: [],
            borderColor: 'red',
            stepped: true,
            fill: true,
            backgroundColor: 'rgba(255, 0, 0, 0.2)',
          },
          {
            label: 'Bids',
            data: [],
            borderColor: 'green',
            stepped: true,
            fill: true,
            backgroundColor: 'rgba(0, 255, 0, 0.2)',
          },
        ],
      },
      options: {
        scales: {
          x: {
            type: 'linear',
            title: {
              display: true,
              text: 'Price'
            }
          },
          y: {
            title: {
              display: true,
              text: 'Cumulative Volume'
            }
          }
        }
      }
    };
    orderBookChart = new Chart(orderBookCanvas, orderBookConfig);
    connectWebSocket(); // Connect only after chart is initialized
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
  <h1>Order Book</h1>
  <select bind:value={selectedStockId}>
    {#each stockTickers as ticker}
      <option value={getStockIdFromTicker(ticker)}>{ticker}</option>
    {/each}
  </select>
  <div class="chart-container">
    <canvas bind:this={orderBookCanvas}></canvas>
  </div>
  <div class="order-book-container">
      <div class="bids-table">
        <h2>Bids</h2>
        <table>
          <thead>
            <tr>
              <th>Price</th>
              <th>Volume</th>
            </tr>
          </thead>
          <tbody>
            {#each bids as [price, volume]}
              <tr>
                <td>{price.toFixed(2)}</td>
                <td>{formatNumber(volume)}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>

      <div class="asks-table">
        <h2>Asks</h2>
        <table>
          <thead>
            <tr>
              <th>Price</th>
              <th>Volume</th>
            </tr>
          </thead>
          <tbody>
            {#each asks as [price, volume]}
              <tr>
                <td>{price.toFixed(2)}</td>
                <td>{formatNumber(volume)}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    </div>
</main>

<style>
  .chart-container {
    width: 80%;
    height: 400px;
  }
  .order-book-container {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1.5em;
  }

  table {
    width: 100%;
    border-collapse: collapse;
    margin-top: 1em;
  }

  th,
  td {
    padding: 0.8em;
    border: 1px solid #eee;
    text-align: left;
  }

  th {
    background-color: #f8f9fa;
    font-weight: bold;
    color: #444;
  }

  .bids-table td:first-child {
    color: #28a745; /* Green for bids */
    font-weight: bold;
  }

  .asks-table td:first-child {
    color: #dc3545; /* Red for asks */
    font-weight: bold;
  }
</style>
