<script lang="ts">
  import { onMount, onDestroy, afterUpdate } from 'svelte';
  import type { Chart } from 'chart.js';
  import type { MarketState } from '../../lib/types';

  export let marketState: MarketState | null;
  export let selectedStockId: string;

  let depthCanvas: HTMLCanvasElement;
  let depthChart: Chart | null = null;

  onMount(async () => {
    const { Chart: ChartJS, registerables } = await import('chart.js');
    ChartJS.register(...registerables);

    if (depthCanvas && !depthChart) {
      const ctx = depthCanvas.getContext('2d');
      if (ctx) {
        depthChart = new ChartJS(ctx, {
          type: 'line',
          data: { datasets: [] },
          options: {
            responsive: true,
            maintainAspectRatio: false,
            interaction: {
              intersect: false,
              mode: 'index',
            },
            scales: {
              x: {
                type: 'linear',
                position: 'bottom',
                title: {
                  display: true,
                  text: 'Price ($)',
                  color: '#d1d4dc',
                  font: { size: 12, weight: 'bold' }
                },
                grid: { 
                  color: 'rgba(255, 255, 255, 0.1)',
                  drawBorder: false
                },
                ticks: { 
                  color: '#d1d4dc',
                  callback: function(value: any) {
                    return '$' + (value as number).toFixed(2);
                  }
                }
              },
              y: {
                type: 'linear',
                position: 'right',
                title: {
                  display: true,
                  text: 'Cumulative Volume',
                  color: '#d1d4dc',
                  font: { size: 12, weight: 'bold' }
                },
                grid: { 
                  color: 'rgba(255, 255, 255, 0.1)',
                  drawBorder: false
                },
                ticks: { 
                  color: '#d1d4dc',
                  callback: function(value: any) {
                    return (value as number).toLocaleString();
                  }
                }
              }
            },
            plugins: {
              legend: {
                display: true,
                position: 'top',
                labels: {
                  color: '#d1d4dc',
                  font: { size: 11 },
                  usePointStyle: true,
                  pointStyle: 'line'
                }
              },
              tooltip: {
                backgroundColor: 'rgba(26, 31, 46, 0.95)',
                titleColor: '#d1d4dc',
                bodyColor: '#d1d4dc',
                borderColor: '#2a2e39',
                borderWidth: 1,
                callbacks: {
                  title: function(context: any) {
                    return `Price: $${context[0].parsed.x.toFixed(2)}`;
                  },
                  label: function(context: any) {
                    const label = context.dataset.label || '';
                    return `${label}: ${context.parsed.y.toLocaleString()} shares`;
                  }
                }
              }
            },
            elements: {
              point: {
                radius: 0,
                hoverRadius: 4
              },
              line: {
                tension: 0
              }
            }
          }
        });
      }
    }
    updateDepthChart();
  });

  afterUpdate(() => {
    updateDepthChart();
  });

  onDestroy(() => {
    depthChart?.destroy();
  });

  function updateDepthChart() {
    if (!depthChart || !marketState) return;

    const orderBook = marketState.order_books?.[selectedStockId];
    if (!orderBook) return;

    // Process and sort bids (descending - best to worst)
    const bids = Object.entries(orderBook.bids || {})
      .map(([price, level]) => [parseFloat(price) / 100, level.total_volume] as [number, number])
      .sort((a, b) => b[0] - a[0]);

    // Process and sort asks (ascending - best to worst)
    const asks = Object.entries(orderBook.asks || {})
      .map(([price, level]) => [parseFloat(price) / 100, level.total_volume] as [number, number])
      .sort((a, b) => a[0] - b[0]);

    // Create cumulative volume data for bids
    const bidData: {x: number, y: number}[] = [];
    let cumulativeBidVolume = 0;
    for (const [price, volume] of bids) { // Bids are sorted descending
      cumulativeBidVolume += volume;
      bidData.push({ x: price, y: cumulativeBidVolume });
    }

    // Create cumulative volume data for asks
    const askData: {x: number, y: number}[] = [];
    let cumulativeAskVolume = 0;
    for (const [price, volume] of asks) { // Asks are sorted ascending
      cumulativeAskVolume += volume;
      askData.push({ x: price, y: cumulativeAskVolume });
    }

    // Get current price for reference line
    const currentPrice = marketState.last_traded_price?.[selectedStockId] || 0;

    // Calculate mid price for centering
    const bestBid = bids[0]?.[0] || currentPrice;
    const bestAsk = asks[0]?.[0] || currentPrice;
    const midPrice = (bestBid + bestAsk) / 2;

    // Find max cumulative volume for scaling
    const maxVolume = Math.max(cumulativeBidVolume, cumulativeAskVolume, 1);

    depthChart.data.datasets = [
      {
        label: '📈 Bids',
        data: bidData.sort((a, b) => a.x - b.x), // Sort bids ascending for charting
        borderColor: '#26a69a',
        backgroundColor: 'rgba(38, 166, 154, 0.1)',
        fill: 'origin',
        borderWidth: 2.5,
        stepped: true,
        pointRadius: 0,
        pointHoverRadius: 4
      },
      {
        label: '📉 Asks',
        data: askData,
        borderColor: '#ef5350',
        backgroundColor: 'rgba(239, 83, 80, 0.1)',
        fill: 'origin',
        borderWidth: 2.5,
        stepped: true,
        pointRadius: 0,
        pointHoverRadius: 4
      }
    ];

    // Add current price line if we have a valid price
    if (currentPrice > 0) {
      depthChart.data.datasets.push({
        label: 'Current Price',
        data: [
          { x: currentPrice, y: 0 },
          { x: currentPrice, y: maxVolume * 1.1 }
        ],
        borderColor: 'rgba(255, 255, 255, 0.6)',
        backgroundColor: 'transparent',
        borderWidth: 1,
        borderDash: [5, 5],
        pointRadius: 0,
        fill: false
      });
    }

    // Set appropriate scale bounds - show ±50% around mid-price
    const priceRange = midPrice * 0.5; // 50% of mid-price on each side
    depthChart.options.scales!.x!.min = midPrice - priceRange;
    depthChart.options.scales!.x!.max = midPrice + priceRange;
    depthChart.options.scales!.y!.max = maxVolume * 1.1;

    depthChart.update('none');
  }
</script>

<div class="depth-chart-container">
  <div class="depth-header">
    <h3>📊 Order Book Depth</h3>
    <div class="depth-stats">
      {#if marketState?.order_books?.[selectedStockId]}
        {@const orderBook = marketState.order_books[selectedStockId]}
        {@const bidCount = Object.keys(orderBook.bids || {}).length}
        {@const askCount = Object.keys(orderBook.asks || {}).length}
        <span class="stat">
          <span class="stat-label">Bids:</span>
          <span class="stat-value bid-color">{bidCount}</span>
        </span>
        <span class="stat">
          <span class="stat-label">Asks:</span>
          <span class="stat-value ask-color">{askCount}</span>
        </span>
      {/if}
    </div>
  </div>
  <div class="chart-wrapper">
    <canvas bind:this={depthCanvas}></canvas>
  </div>
</div>

<style>
  .depth-chart-container {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--bg-secondary, #1a1f2e);
  }

  .depth-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 12px 16px;
    border-bottom: 1px solid var(--border-color, #2a2e39);
    background: var(--bg-tertiary, #252a3a);
  }

  .depth-header h3 {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
    color: var(--text-primary, #d1d4dc);
  }

  .depth-stats {
    display: flex;
    gap: 16px;
    font-size: 12px;
  }

  .stat {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .stat-label {
    color: var(--text-secondary, #848e9c);
  }

  .stat-value {
    font-weight: 600;
    font-family: 'SF Mono', 'Monaco', 'Inconsolata', 'Roboto Mono', monospace;
  }

  .bid-color {
    color: var(--accent-green, #26a69a);
  }

  .ask-color {
    color: var(--accent-red, #ef5350);
  }

  .chart-wrapper {
    flex: 1;
    position: relative;
    padding: 8px;
    min-height: 0;
  }
</style>