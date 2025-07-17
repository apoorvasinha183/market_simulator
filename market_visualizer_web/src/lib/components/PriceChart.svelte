
<script lang="ts">
  import { onMount, onDestroy, afterUpdate } from 'svelte';
  import type { Chart } from 'chart.js';
  import { TimeFrame, type Candle } from '../../lib/types';

  export let selectedStockId: string;
  export let selectedTimeFrame: TimeFrame;
  export let candleData: { [key: string]: Candle[] } = {};
  export let priceHistoryData: { [key: string]: [number, number][] } = {};
  export let isCandlestick: boolean;

  let priceCanvas: HTMLCanvasElement;
  let volumeCanvas: HTMLCanvasElement;
  let priceChart: Chart | null = null;
  let volumeChart: Chart | null = null;

  const timeFrameMap = {
    [TimeFrame.TenSeconds]: "10s",
    [TimeFrame.OneMinute]: "1m",
    [TimeFrame.FiveMinutes]: "5m",
    [TimeFrame.ThirtyMinutes]: "30m",
  };

  let blinkOn = true;
  let blinker: NodeJS.Timeout;

  onMount(async () => {
    blinker = setInterval(() => {
      blinkOn = !blinkOn;
      updateCharts();
    }, 500);

    const { Chart: ChartJS, registerables } = await import('chart.js');
    const { CandlestickController, CandlestickElement } = await import('chartjs-chart-financial');
    const { BarController, BarElement } = await import('chart.js');
    await import('chartjs-adapter-date-fns');
    ChartJS.register(...registerables, CandlestickController, CandlestickElement, BarController, BarElement);

    if (priceCanvas && !priceChart) {
      const priceCtx = priceCanvas.getContext('2d');
      if (priceCtx) {
        priceChart = new ChartJS(priceCtx, {
          type: 'line', // Default type
          data: { datasets: [] },
          options: {
            responsive: true,
            maintainAspectRatio: false,
            scales: {
              x: { type: 'time', grid: { color: 'rgba(255, 255, 255, 0.1)' }, ticks: { display: false } },
              y: { position: 'right', grid: { color: 'rgba(255, 255, 255, 0.1)' }, ticks: { color: '#d1d4dc' } },
            },
            plugins: {
              legend: { display: false },
              tooltip: {
                callbacks: {
                  label: function(context: any) {
                    const raw = context.raw;
                    if (raw && typeof raw === 'object' && 'o' in raw) {
                      return [
                        `Open: ${raw.o.toFixed(2)}`,
                        `High: ${raw.h.toFixed(2)}`,
                        `Low: ${raw.l.toFixed(2)}`,
                        `Close: ${raw.c.toFixed(2)}`,
                      ];
                    }
                    return `${context.dataset.label}: ${context.formattedValue}`;
                  }
                }
              }
            },
          },
        });
      }
    }

    if (volumeCanvas && !volumeChart) {
      const volumeCtx = volumeCanvas.getContext('2d');
      if (volumeCtx) {
        volumeChart = new ChartJS(volumeCtx, {
          type: 'bar',
          data: { datasets: [] },
          options: {
            responsive: true,
            maintainAspectRatio: false,
            scales: {
              x: { type: 'time', grid: { display: false }, ticks: { color: '#d1d4dc', maxRotation: 0, minRotation: 0 } },
              y: { position: 'right', grid: { color: 'rgba(255, 255, 255, 0.1)' }, ticks: { color: '#d1d4dc', maxTicksLimit: 5 } },
            },
            plugins: {
              legend: { display: false },
              tooltip: {
                callbacks: {
                  label: function(context: any) {
                    return `Volume: ${context.raw.y.toLocaleString()}`;
                  }
                }
              }
            },
          },
        });
      }
    }
    updateCharts();
  });

  afterUpdate(() => {
    updateCharts();
  });

  onDestroy(() => {
    priceChart?.destroy();
    volumeChart?.destroy();
    clearInterval(blinker);
  });

  function updateCharts() {
    if (!priceChart || !volumeChart) return;

    if (isCandlestick) {
      priceChart.config.type = 'candlestick';
      const backendTimeFrame = timeFrameMap[selectedTimeFrame];
      const key = `${selectedStockId}-${backendTimeFrame}`;
      const candles = candleData[key] || [];
      
      priceChart.data.datasets = [{
        label: 'Price',
        data: candles.map(c => ({ x: c.timestamp * 1000, o: c.open, h: c.high, l: c.low, c: c.close })),
      }];

      volumeChart.data.datasets = [{
        label: 'Volume',
        data: candles.map(c => ({ x: c.timestamp * 1000, y: c.volume })),
        backgroundColor: candles.map(c => c.close >= c.open ? 'rgba(38, 166, 154, 0.5)' : 'rgba(239, 83, 80, 0.5)'),
      }];

    } else {
      priceChart.config.type = 'line';
      const history = (priceHistoryData?.[selectedStockId] || []).map(([timestamp, price]) => ({ x: timestamp * 1000, y: price }));
      
      const pointRadii = new Array(history.length).fill(0);
      const pointColors = new Array(history.length).fill('rgba(0,0,0,0)');

      if (history.length > 0) {
        pointRadii[history.length - 1] = blinkOn ? 5 : 0;
        pointColors[history.length - 1] = 'rgb(255, 99, 132)';
      }

      priceChart.data.datasets = [{
        label: 'Price',
        data: history,
        borderColor: 'rgb(75, 192, 192)',
        tension: 0.1,
        pointRadius: pointRadii,
        pointBackgroundColor: pointColors,
        pointBorderColor: pointColors,
      }];
      
      // Clear volume chart when in line mode
      volumeChart.data.datasets = [{ label: 'Volume', data: [] }];
    }

    // Synchronize charts
    if (priceChart.options.scales?.x) {
        volumeChart.options.scales.x.min = priceChart.options.scales.x.min;
        volumeChart.options.scales.x.max = priceChart.options.scales.x.max;
    }

    priceChart.update('none');
    volumeChart.update('none');
  }
</script>

<div class="chart-grid-container">
  <div class="price-chart-container">
    <canvas bind:this={priceCanvas}></canvas>
  </div>
  <div class="volume-chart-container">
    <canvas bind:this={volumeCanvas}></canvas>
  </div>
</div>

<style>
  .chart-grid-container {
    display: grid;
    grid-template-rows: 70% 30%;
    position: relative;
    width: 100%;
    height: 100%;
  }
  .price-chart-container, .volume-chart-container {
    position: relative;
    width: 100%;
    height: 100%;
  }
</style>
