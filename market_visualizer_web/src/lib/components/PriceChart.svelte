
<script lang="ts">
  import { onMount, onDestroy, afterUpdate } from 'svelte';
  import type { Chart } from 'chart.js';
  import type { Candle, TimeFrame } from '../../lib/types';

  export let selectedStockId: string;
  export let selectedTimeFrame: TimeFrame;
  export let candleData: { [key: string]: Candle[] } = {};
  export let priceHistoryData: { [key: string]: [number, number][] } = {};
  export let isCandlestick: boolean;

  let chartCanvas: HTMLCanvasElement;
  let chart: Chart | null = null;

  const timeFrameMap: { [key: string]: string } = {
    "HundredMillis": "100ms",
    "OneSecond": "1s",
    "TenSeconds": "10s",
    "OneMinute": "1m",
    "FiveMinutes": "5m",
    "ThirtyMinutes": "30m",
  };

  onMount(async () => {
    const { Chart: ChartJS, registerables } = await import('chart.js');
    const { CandlestickController, CandlestickElement } = await import('chartjs-chart-financial');
    await import('chartjs-adapter-date-fns');
    ChartJS.register(...registerables, CandlestickController, CandlestickElement);

    if (chartCanvas && !chart) {
      const ctx = chartCanvas.getContext('2d');
      if (ctx) {
        chart = new ChartJS(ctx, {
          type: 'line', // Default type
          data: { datasets: [] },
          options: {
            responsive: true,
            maintainAspectRatio: false,
            scales: {
              x: { type: 'time', grid: { color: 'rgba(255, 255, 255, 0.1)' }, ticks: { display: false } },
              y: { grid: { color: 'rgba(255, 255, 255, 0.1)' }, ticks: { color: '#d1d4dc' } },
            },
            plugins: { legend: { display: false } },
          },
        });
        updateChart(); // Initial data load
      }
    }
  });

  afterUpdate(() => {
    if (chart) {
      updateChart();
    }
  });

  onDestroy(() => {
    chart?.destroy();
  });

  function updateChart() {
    if (!chart) return;

    if (isCandlestick) {
      chart.config.type = 'candlestick';
      const backendTimeFrame = timeFrameMap[selectedTimeFrame];
      const key = `${selectedStockId}-${backendTimeFrame}`;
      const candles = candleData[key] || [];
      chart.data.datasets = [{
        label: 'Price',
        data: candles.map(c => ({ x: c.timestamp, o: c.open, h: c.high, l: c.low, c: c.close }))
      }];
    } else {
      chart.config.type = 'line';
      const history = (priceHistoryData?.[selectedStockId] || []).map(([timestamp, price]) => ({ x: timestamp, y: price }));
      
      // Logic to emphasize the last point
      const pointRadii = new Array(history.length).fill(0);
      if (history.length > 0) {
        pointRadii[history.length - 1] = 5; // Larger radius for the last point
      }

      chart.data.labels = history.map(d => d.x); // Explicitly set labels for the line chart

      chart.data.datasets = [{
        label: 'Price',
        data: history,
        borderColor: 'rgb(75, 192, 192)',
        pointRadius: pointRadii,
        pointBackgroundColor: 'rgb(255, 99, 132)',
        pointBorderColor: 'rgb(255, 99, 132)',
      }];
    }
    chart.update('none');
  }
</script>

<div class="chart-container">
  <canvas bind:this={chartCanvas}></canvas>
</div>

<style>
  .chart-container {
    position: relative;
    width: 100%;
    height: 100%;
  }
</style>
